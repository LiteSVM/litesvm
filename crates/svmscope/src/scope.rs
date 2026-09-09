//! The crate's front door: [`Scope`] (an RPC-backed client that caches what it
//! fetches) and [`Replay`] (a transaction's reconstructed world, fetched once,
//! replayable any number of times with zero further RPC).
//!
//! ```no_run
//! use svmscope::{Mutation, Scope};
//!
//! let scope = Scope::new("https://api.mainnet-beta.solana.com");
//! let mut replay = scope.replay("<signature>")?;   // all RPC happens here
//! replay.advance_seconds(30 * 86_400);             // +30 days
//! let out = replay.simulate(&[Mutation::lamports("<account>", 0)])?;
//! println!("success: {}", out.result.success);
//! # Ok::<(), svmscope::Error>(())
//! ```
use {
    crate::{
        analyze::{build_overview, AccountOverview, Analysis, ProgramInfo, SigInfo},
        cpi_tree, decode, diffs,
        error::{Error, Result},
        fidelity::{AccountState, Fidelity},
        fixture::{Fixture, OnchainRecord},
        idl,
        replay::{PreState, ReplayContext, TimeTravel},
        session::Replay,
        utils, CapturedTransaction,
    },
    serde_json::json,
    solana_address::Address,
    solana_client::{
        rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig, rpc_request::RpcRequest,
    },
    solana_transaction::versioned::VersionedTransaction,
    std::{
        collections::HashMap,
        str::FromStr,
        sync::Mutex,
        thread,
        time::{Duration, Instant},
    },
};

/// An RPC-backed client with caches. Everything the crate fetches — transaction
/// JSON, program IDLs — is fetched once per `Scope` and reused, so
/// `analyze(sig)` followed by `replay(sig)` costs one transaction fetch, and
/// repeated simulations cost zero.
pub struct Scope {
    client: RpcClient,

    archive: Option<RpcClient>,

    /// getTransaction (json encoding) responses by signature.
    tx_cache: Mutex<HashMap<String, serde_json::Value>>,
    /// On-chain IDL by program id; `None` = checked, program publishes none.
    idl_cache: Mutex<HashMap<String, Option<serde_json::Value>>>,
}

fn status_is_confirmed(status: &serde_json::Value) -> bool {
    match status
        .get("confirmationStatus")
        .and_then(serde_json::Value::as_str)
    {
        Some("confirmed" | "finalized") => true,
        Some("processed") => false,
        None => status.get("confirmations").is_some_and(|confirmation| {
            confirmation.is_null() || confirmation.as_u64().is_some_and(|count| count > 1)
        }),
        Some(_) => false,
    }
}

/// A fetched account, as `(owner, lamports, executable, data)`.
type RawAccount = (String, u64, bool, Vec<u8>);

impl Scope {
    /// A scope talking to the given RPC endpoint.
    pub fn new(rpc_url: impl Into<String>) -> Scope {
        Scope::from_client(RpcClient::new(rpc_url.into()))
    }

    /// A scope over an existing client (custom commitment/timeout config).
    pub fn from_client(client: RpcClient) -> Scope {
        Scope {
            client,
            archive: None,
            tx_cache: Mutex::new(HashMap::new()),
            idl_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Register a program's IDL for every API on this scope — `analyze`,
    /// `diagnose`, `preflight_overview`, and the replays and traces built from
    /// signatures — so instructions, accounts, arguments, events and errors of
    /// that program are named even when it publishes no IDL on-chain (a local
    /// or private deployment). Takes precedence over the on-chain lookup.
    pub fn add_idl(&self, program: impl Into<String>, idl: serde_json::Value) {
        self.idl_cache
            .lock()
            .unwrap()
            .insert(program.into(), Some(idl));
    }

    /// Attach an archival RPC endpoint (e.g. Alchemy's Account Archive) so
    /// `replay_at_slot` can fetch account state as of a transaction's slot.
    /// Slot of `address`'s most recent on-chain write (its latest signature),
    /// or `None` if it has none or the lookup fails. One cheap RPC call.
    /// The RPC endpoint this scope talks to.
    pub fn rpc_url(&self) -> String {
        self.client.url()
    }

    /// Slot of `address`'s most recent on-chain write, or `None`.
    pub fn last_write_slot(&self, address: &str) -> Option<u64> {
        let resp: serde_json::Value = self
            .client
            .send(
                RpcRequest::GetSignaturesForAddress,
                serde_json::json!([address, { "limit": 1 }]),
            )
            .ok()?;
        resp["result"]
            .as_array()
            .or_else(|| resp.as_array())
            .and_then(|a| a.first())
            .and_then(|e| e["slot"].as_u64())
    }

    /// Attach an archival RPC that honours a historical `slot` on
    /// `getAccountInfo` (e.g. Alchemy's Account Archive). Replays at a slot
    /// then fetch every account as of that slot: exact, not reconstructed.
    pub fn with_archive(mut self, archive_url: impl Into<String>) -> Scope {
        self.archive = Some(RpcClient::new(archive_url.into()));
        self
    }

    /// The underlying RPC client — the escape hatch for anything the crate
    /// doesn't wrap.
    pub fn client(&self) -> &RpcClient {
        &self.client
    }

    /// Accept a transaction signature OR an account/program address. A 32-byte
    /// value parses as an address and resolves to its most recent transaction;
    /// a 64-byte signature is used as-is.
    fn resolve_signature(&self, input: &str) -> Result<String> {
        let input = input.trim();
        if Address::from_str(input).is_ok() {
            let resp: serde_json::Value = self
                .client
                .send(
                    RpcRequest::GetSignaturesForAddress,
                    json!([input, { "limit": 1 }]),
                )
                .map_err(Error::rpc)?;
            return resp
                .as_array()
                .and_then(|a| a.first())
                .and_then(|s| s["signature"].as_str())
                .map(String::from)
                .ok_or_else(|| Error::NoSignatures(input.to_string()));
        }
        Ok(input.to_string())
    }

    /// The transaction's `getTransaction` JSON, fetched once and cached.
    fn fetch_transaction_json(&self, signature: &str) -> Result<Option<serde_json::Value>> {
        if let Some(tx) = self.tx_cache.lock().unwrap().get(signature) {
            return Ok(Some(tx.clone()));
        }

        let tx: serde_json::Value = self
            .client
            .send(
                RpcRequest::GetTransaction,
                json!([
                    signature,
                    {
                        "encoding": "json",
                        "commitment": "confirmed",
                        "maxSupportedTransactionVersion": 0
                    }
                ]),
            )
            .map_err(Error::rpc)?;

        // A recently confirmed transaction may not be indexed yet.
        if tx.is_null() {
            return Ok(None);
        }

        self.tx_cache
            .lock()
            .unwrap()
            .insert(signature.to_string(), tx.clone());

        Ok(Some(tx))
    }

    fn transaction_json(&self, signature: &str) -> Result<serde_json::Value> {
        self.fetch_transaction_json(signature)?
            .ok_or_else(|| Error::TransactionNotFound(signature.to_string()))
    }

    /// A program's on-chain IDL, fetched once and cached (`None` = has none).
    /// Two layers: per-`Scope` (this request) and process-wide with a 10-minute
    /// TTL, so a server handling many requests for the same programs fetches
    /// each IDL once, not once per request.
    fn idl_for(&self, program: &str) -> Option<serde_json::Value> {
        type IdlCache = HashMap<String, (Instant, Option<serde_json::Value>)>;
        static GLOBAL: std::sync::LazyLock<Mutex<IdlCache>> =
            std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
        const TTL: Duration = Duration::from_secs(600);

        let mut cache = self.idl_cache.lock().unwrap();
        if let Some(v) = cache.get(program) {
            return v.clone();
        }
        if let Ok(g) = GLOBAL.lock() {
            if let Some((at, v)) = g.get(program) {
                if at.elapsed() < TTL {
                    cache.insert(program.to_string(), v.clone());
                    return v.clone();
                }
            }
        }
        let fetched = Address::from_str(program)
            .ok()
            .and_then(|a| crate::rpc::fetch_idl_json(&self.client, a));
        cache.insert(program.to_string(), fetched.clone());
        if let Ok(mut g) = GLOBAL.lock() {
            if g.len() >= 512 {
                g.clear();
            }
            g.insert(program.to_string(), (Instant::now(), fetched.clone()));
        }
        fetched
    }

    /// Decode a transaction: the full CPI tree with IDL-named instructions,
    /// balance and token diffs, per-program compute units, logs, and every
    /// touched account. `input` may be a signature or an address (resolved to
    /// its latest transaction). Decode only — replay via [`Scope::replay`].
    pub fn analyze(&self, input: &str) -> Result<Analysis> {
        let signature = self.resolve_signature(input)?;
        let tx = self.transaction_json(&signature)?;
        let account_keys = utils::resolve_account_keys(&tx);

        let mut cpi_tree = cpi_tree::build_cpi_tree(&tx);
        let logs: Vec<String> = tx["meta"]["logMessages"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|l| l.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        cpi_tree::attach_compute(&mut cpi_tree, &logs);
        // Decode each instruction — name, arguments, and named accounts — from
        // native layouts (always) or the program's on-chain Anchor IDL (cached).
        {
            let mut idls = self.idl_cache.lock().unwrap();
            for e in &mut cpi_tree {
                let (name, args, accounts) = crate::rpc::enrich(
                    &self.client,
                    &mut idls,
                    &e.program,
                    &e.data,
                    &e.account_indexes,
                    &account_keys,
                );
                e.name = name;
                e.args = args;
                e.accounts = accounts;
            }
        }
        cpi_tree::mark_introspection(&mut cpi_tree);
        Ok(Analysis {
            overview: build_overview(&tx, &cpi_tree, account_keys.len()),
            cpi_tree,
            balance_change: diffs::account_diffs(&tx),
            token_change: diffs::token_diffs(&tx),
            compute: crate::compute::cu_per_program(&tx),
            logs: tx["meta"]["logMessages"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|l| l.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            replay: None,
            accounts: crate::rpc::describe_accounts(&self.client, &account_keys),
            signature,
        })
    }

    /// Diagnose a transaction: **why did it fail, and how do I fix it?** Reads the
    /// *recorded* on-chain outcome (not a drift-prone re-simulation), resolves the
    /// error to a plain name and message — from the Anchor logs, or the failing
    /// program's on-chain IDL — and suggests a concrete fix. Free.
    pub fn diagnose(&self, input: &str) -> Result<crate::Diagnosis> {
        let signature = self.resolve_signature(input)?;
        let tx = self.transaction_json(&signature)?;
        Ok(crate::diagnose::diagnose_tx(&tx, |program| {
            self.idl_for(program)
        }))
    }

    /// Reconstruct the transaction's world for local replay — every touched
    /// account, every program ELF, the on-chain outcome, and the IDLs needed to
    /// name errors and fields. **All RPC happens here**; every run of the
    /// returned [`Replay`] is local and free.
    pub fn replay(&self, input: &str) -> Result<Replay> {
        let signature = self.resolve_signature(input)?;
        let tx = self.transaction_json(&signature)?;
        let account_keys = utils::resolve_account_keys(&tx);
        let pre = PreState::from_meta(&tx, &account_keys);
        let mut ctx = crate::rpc::build_context(
            &self.client,
            &signature,
            &account_keys,
            tx["slot"].as_u64(),
            &pre,
        )?;
        self.preload_idls(&mut ctx);
        Ok(Replay {
            recorded: Some(OnchainRecord::from_tx_json(&tx)),
            ctx,
            time_travel: TimeTravel::default(),
            fidelity: Fidelity::Current,
        })
    }

    /// Replay a transaction as of its own slot, at the best fidelity the
    /// available data allows — see [`Replay::fidelity`] for what you actually got.
    /// Mutations compose on top: the returned [`Replay`]'s [`Replay::simulate`] /
    /// [`Replay::verify`] apply what-if changes to that same state, so you can
    /// mutate at a specific slot too.
    ///
    /// Two tiers, chosen automatically:
    /// - **Exact** — when an archival endpoint is set via [`Scope::with_archive`]
    ///   *and* it honors the historical `slot` parameter (e.g. Alchemy's Account
    ///   Archive). Accounts and program ELFs are loaded at the end of the slot
    ///   *before* the transaction's (its true pre-block boundary — fetching at
    ///   the transaction's own slot would return its post-state), then the
    ///   transaction's recorded pre-balances correct any same-slot predecessor's
    ///   balance effects. Same-slot predecessor *data* writes are the one
    ///   remaining gap of a slot-granular archive.
    /// - **Reconstructed** — the free path, no archive required. Accounts load at
    ///   current state, then SOL and SPL-token balances are rewound to their
    ///   pre-transaction values from the transaction's own metadata, and the clock
    ///   is set to the transaction's slot. Faithful for balances and time; account
    ///   *data* (pool reserves, oracle prices) is still current — [`Fidelity`]
    ///   reports this honestly rather than pretending the replay is exact.
    pub fn replay_at_slot(&self, input: &str) -> Result<Replay> {
        let signature = self.resolve_signature(input)?;
        let tx = self.transaction_json(&signature)?;
        let slot = tx["slot"]
            .as_u64()
            .ok_or_else(|| Error::MalformedRpcResponse("transaction has no slot".into()))?;
        let account_keys = utils::resolve_account_keys(&tx);
        let pre = PreState::from_meta(&tx, &account_keys);

        // Exact tier: only when an archive is set AND actually honors the slot.
        let archive = self
            .archive
            .as_ref()
            .filter(|a| crate::rpc::archive_honors_slot(a, slot));

        let (mut ctx, fidelity) = match archive {
            Some(archive) => {
                // Fetch at S-1: the archive answers "≤ slot", and at S that
                // includes this transaction's own writes (post-state). The
                // recorded pre-balances in `pre` then correct any same-slot
                // predecessor's balance effects on top.
                let ctx = crate::rpc::build_context_at_slot(
                    archive,
                    &signature,
                    &account_keys,
                    slot.saturating_sub(1),
                    slot,
                    tx["blockTime"].as_i64(),
                    &pre,
                )?;
                (ctx, Fidelity::Exact { slot })
            }
            None => {
                // Free reconstruction: current accounts + metadata balance rewind.
                let ctx = crate::rpc::build_context(
                    &self.client,
                    &signature,
                    &account_keys,
                    Some(slot),
                    &pre,
                )?;
                (ctx, Fidelity::Reconstructed { slot })
            }
        };

        self.preload_idls(&mut ctx);

        let mut replay = Replay {
            recorded: Some(OnchainRecord::from_tx_json(&tx)),
            ctx,
            time_travel: TimeTravel::default(),
            fidelity: Fidelity::Current,
        };
        replay.set_fidelity(fidelity);
        // Anchor the clock to the transaction's slot/time for both tiers.
        replay.warp_to_slot(slot);
        if let Some(ts) = tx["blockTime"].as_i64() {
            replay.warp_to_timestamp(ts);
        }
        Ok(replay)
    }

    /// Replay a transaction against archival account state at a **slot you
    /// choose** — the "what if this ran at slot N?" primitive. Every account and
    /// program ELF is loaded as of `slot`, and the clock is set to `slot`.
    ///
    /// Unlike [`Scope::replay_at_slot`] (which reconstructs the transaction's own
    /// slot for free from metadata), an *arbitrary* slot needs real historical
    /// account state, so this **requires** an archival endpoint set via
    /// [`Scope::with_archive`] that honors the historical `slot` parameter (e.g.
    /// Alchemy's Account Archive). A non-archival endpoint is detected and
    /// refused rather than silently returning current state.
    pub fn replay_at(&self, input: &str, slot: u64) -> Result<Replay> {
        let archive = self.archive.as_ref().ok_or_else(|| {
            Error::InvalidSpec(
                "replaying at an arbitrary slot needs historical account state — set an archival \
                 endpoint with Scope::with_archive(url) (e.g. Alchemy PAYG)"
                    .into(),
            )
        })?;
        if !crate::rpc::archive_honors_slot(archive, slot) {
            return Err(Error::InvalidSpec(format!(
                "the archive endpoint ignored historical slot {slot} and returned current state — \
                 replay_at needs an endpoint with account archival; a public node or Helius will not work"
            )));
        }

        let signature = self.resolve_signature(input)?;
        let tx = self.transaction_json(&signature)?;
        let account_keys = utils::resolve_account_keys(&tx);
        // The transaction's own metadata pre-state is only valid at its own slot;
        // at an arbitrary slot the archive is authoritative, so pass none.
        let pre = PreState::default();
        let block_time = archive.get_block_time(slot).ok();
        // An arbitrary slot means "the world as of end of slot N" — state and
        // clock share the same boundary, no pre-balance patching.
        let mut ctx = crate::rpc::build_context_at_slot(
            archive,
            &signature,
            &account_keys,
            slot,
            slot,
            block_time,
            &pre,
        )?;
        self.preload_idls(&mut ctx);

        let mut replay = Replay {
            recorded: Some(OnchainRecord::from_tx_json(&tx)),
            ctx,
            time_travel: TimeTravel::default(),
            fidelity: Fidelity::Exact { slot },
        };
        replay.warp_to_slot(slot);
        if let Some(ts) = block_time {
            replay.warp_to_timestamp(ts);
        }
        Ok(replay)
    }

    /// Reconstruct the world for an **unsigned / not-yet-sent** transaction
    /// (base64 wire bytes) — the pre-flight "what will this do if I send it
    /// now?" primitive. Current on-chain state IS its pre-state, so no drift.
    ///
    /// Accepts either a full serialized `VersionedTransaction` or a bare
    /// serialized message — the "Base64 message" wallets in developer mode show
    /// on the approval screen (and what Solana Explorer's inspector takes). A
    /// bare message is wrapped with placeholder signatures before simulation.
    pub fn preflight(&self, tx_b64: &str) -> Result<Replay> {
        self.preflight_tx(parse_unsigned(tx_b64)?)
    }

    /// [`Scope::preflight`] for an already-deserialized transaction.
    pub fn preflight_tx(
        &self,
        tx: solana_transaction::versioned::VersionedTransaction,
    ) -> Result<Replay> {
        let mut ctx = crate::rpc::preflight_context(&self.client, tx)?;
        self.preload_idls(&mut ctx);
        Ok(Replay {
            recorded: None,
            ctx,
            time_travel: TimeTravel::default(),
            fidelity: Fidelity::Current,
        })
    }

    /// See [`Scope::preflight`]: parse base64 into a transaction, accepting a
    /// bare message too. Exposed for testing.
    #[doc(hidden)]
    pub fn parse_unsigned_b64(
        tx_b64: &str,
    ) -> Result<solana_transaction::versioned::VersionedTransaction> {
        parse_unsigned(tx_b64)
    }

    /// The pre-sign overview of an unsigned transaction: serialized size, fee
    /// breakdown (base + ComputeBudget priority fee), fee payer with live
    /// balance, IDL-named instructions, and plain-English actions with danger
    /// flags (delegations, authority changes, closes). Pairs with
    /// [`Scope::preflight`] — decode what signing would do, then simulate it.
    pub fn preflight_overview(&self, tx: &VersionedTransaction) -> crate::PreflightOverview {
        let mut idls = self.idl_cache.lock().unwrap();
        crate::rpc::preflight_overview(&self.client, &mut idls, tx)
    }

    /// Freeze a transaction's world into a portable, self-contained [`Fixture`]:
    /// capture once, then replay deterministically forever with no RPC.
    pub fn capture(&self, input: &str) -> Result<Fixture> {
        self.replay(input)?.to_fixture()
    }

    /// Load the IDLs a replay will want — for every loaded program and every
    /// distinct data-account owner — so error names, explanations, and
    /// named-field asserts all resolve without further RPC.
    fn preload_idls(&self, ctx: &mut ReplayContext) {
        for program in ctx.interesting_programs() {
            if let Some(idl) = self.idl_for(&program) {
                ctx.add_idl(program, idl);
            }
        }
    }

    /// An explorer-style overview of any account or program address.
    pub fn account(&self, address: &str) -> Result<AccountOverview> {
        let address = address.trim();
        if Address::from_str(address).is_err() {
            return Err(Error::InvalidAddress(address.to_string()));
        }
        let Some((owner, lamports, executable, data)) = self.account_raw(address)? else {
            return Ok(AccountOverview {
                address: address.to_string(),
                exists: false,
                owner: String::new(),
                lamports: 0,
                executable: false,
                data_len: 0,
                program: None,
                idl_name: None,
                decoded: None,
            });
        };

        let mut ov = AccountOverview {
            address: address.to_string(),
            exists: true,
            owner: owner.clone(),
            lamports,
            executable,
            data_len: data.len(),
            program: None,
            idl_name: None,
            decoded: None,
        };

        if executable {
            ov.program = self.program_info(&data, &owner);
            ov.idl_name = self.idl_for(address).and_then(|idl| {
                idl.get("metadata")
                    .and_then(|m| m.get("name"))
                    .or_else(|| idl.get("name"))
                    .and_then(|n| n.as_str())
                    .map(String::from)
            });
        } else {
            // Reuse the decoder for recognized data accounts (SPL / IDL).
            ov.decoded = crate::rpc::describe_accounts(&self.client, &[address.to_string()])
                .into_iter()
                .next()
                .and_then(|a| a.decoded);
        }
        Ok(ov)
    }

    /// Recent transactions that touched an account or program — what an
    /// explorer shows on an address page. Newest first.
    pub fn signatures(&self, address: &str, limit: usize) -> Result<Vec<SigInfo>> {
        let address = address.trim();
        if Address::from_str(address).is_err() {
            return Err(Error::InvalidAddress(address.to_string()));
        }
        let resp: serde_json::Value = self
            .client
            .send(
                RpcRequest::GetSignaturesForAddress,
                json!([address, { "limit": limit }]),
            )
            .map_err(Error::rpc)?;
        let arr = resp.as_array().ok_or_else(|| {
            Error::MalformedRpcResponse("getSignaturesForAddress: not an array".into())
        })?;
        Ok(arr
            .iter()
            .map(|s| SigInfo {
                signature: s["signature"].as_str().unwrap_or_default().to_string(),
                slot: s["slot"].as_u64(),
                err: !s["err"].is_null(),
                block_time: s["blockTime"].as_i64(),
            })
            .collect())
    }

    /// Decode one account, optionally with a caller-supplied IDL.
    ///
    /// The on-chain IDL is the happy path, but plenty of programs never publish
    /// one — including your own during development. Passing the IDL JSON (from
    /// `target/idl/<program>.json`) gives full named-field decoding anyway.
    pub fn decode_account(
        &self,
        address: &str,
        user_idl: Option<&serde_json::Value>,
    ) -> Result<decode::AccountInfo> {
        let address = address.trim();
        if Address::from_str(address).is_err() {
            return Err(Error::InvalidAddress(address.to_string()));
        }
        let mut info = crate::rpc::describe_accounts(&self.client, &[address.to_string()])
            .into_iter()
            .next()
            .ok_or_else(|| Error::AccountNotFound(address.to_string()))?;

        // A supplied IDL wins: it's authoritative for this program, and it beats
        // the inferred layout we may have fallen back to.
        if let Some(idl) = user_idl {
            if let Ok(Some((_, _, _, bytes))) = self.account_raw(address) {
                if let Some(d) = idl::decode_with_idl(idl, &bytes) {
                    info.decoded = Some(d);
                }
            }
        }
        Ok(info)
    }

    /// The instructions a program exposes, from its on-chain IDL — the input to
    /// a transaction builder.
    pub fn program_instructions(&self, program_id: &str) -> Result<Vec<idl::IdlInstruction>> {
        let program_id = program_id.trim();
        if Address::from_str(program_id).is_err() {
            return Err(Error::InvalidAddress(program_id.to_string()));
        }
        let idl = self
            .idl_for(program_id)
            .ok_or_else(|| Error::NoIdl(program_id.to_string()))?;
        Ok(idl::instructions(&idl))
    }

    /// A program's complete on-chain IDL, when the program publishes one.
    pub fn program_idl(&self, program_id: &str) -> Result<Option<serde_json::Value>> {
        let program_id = program_id.trim();
        if Address::from_str(program_id).is_err() {
            return Err(Error::InvalidAddress(program_id.to_string()));
        }
        Ok(self.idl_for(program_id))
    }

    /// Fetch an account's raw state: (owner, lamports, executable, data).
    /// `None` if it doesn't exist.
    /// `Ok(None)` means the account genuinely does not exist; `Err` means the
    /// RPC call itself failed. Keeping these distinct stops a network outage from
    /// masquerading as "account not found".
    /// The account's current raw state on-chain — data bytes, lamports, owner —
    /// or `None` if it doesn't exist. The free anchor and comparison point for
    /// historical reconstruction (see the [`reconstruct`](crate::reconstruct)
    /// module).
    pub fn account_data(&self, address: &str) -> Result<Option<AccountState>> {
        Ok(self
            .account_raw(address)?
            .map(|(owner, lamports, _executable, data)| AccountState {
                data,
                lamports,
                owner,
            }))
    }

    fn account_raw(&self, address: &str) -> Result<Option<RawAccount>> {
        use base64::Engine;
        let resp: serde_json::Value = self
            .client
            .send(
                RpcRequest::GetAccountInfo,
                json!([address, { "encoding": "base64" }]),
            )
            .map_err(Error::rpc)?;
        let v = &resp["value"];
        if v.is_null() {
            return Ok(None);
        }
        let owner = match v["owner"].as_str() {
            Some(o) => o.to_string(),
            None => return Ok(None),
        };
        let Some(lamports) = v["lamports"].as_u64() else {
            return Ok(None);
        };
        let executable = v["executable"].as_bool().unwrap_or(false);
        let data = v["data"][0]
            .as_str()
            .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
            .unwrap_or_default();
        Ok(Some((owner, lamports, executable, data)))
    }

    /// Program deployment details from the (upgradeable) loader accounts.
    fn program_info(&self, program_data_bytes: &[u8], owner: &str) -> Option<ProgramInfo> {
        const UPGRADEABLE: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
        const LOADER_V2: &str = "BPFLoader2111111111111111111111111111111111";

        if owner == UPGRADEABLE && program_data_bytes.len() >= 36 {
            // Program account: [0..4]=variant, [4..36]=programdata address.
            let pd_bytes: [u8; 32] = program_data_bytes[4..36].try_into().ok()?;
            let pd_addr = Address::from(pd_bytes).to_string();
            if let Ok(Some((_, _, _, pd))) = self.account_raw(&pd_addr) {
                // ProgramData: [0..4]=variant, [4..12]=slot, [12]=Option tag, [13..45]=authority.
                let slot = pd
                    .get(4..12)
                    .and_then(|s| s.try_into().ok())
                    .map(u64::from_le_bytes);
                let (upgradeable, authority) = match pd.get(13..45) {
                    Some(a) if pd[12] == 1 => {
                        let a: [u8; 32] = a.try_into().ok()?;
                        (true, Some(Address::from(a).to_string()))
                    }
                    _ => (false, None),
                };
                return Some(ProgramInfo {
                    program_data: pd_addr,
                    upgradeable,
                    upgrade_authority: authority,
                    last_deployed_slot: slot,
                });
            }
            return Some(ProgramInfo {
                program_data: pd_addr,
                upgradeable: true,
                upgrade_authority: None,
                last_deployed_slot: None,
            });
        }
        if owner == LOADER_V2 {
            return Some(ProgramInfo {
                program_data: String::new(),
                upgradeable: false,
                upgrade_authority: None,
                last_deployed_slot: None,
            });
        }
        None
    }

    fn wait_for_transaction(&self, signature: &str) -> Result<serde_json::Value> {
        const TIMEOUT: Duration = Duration::from_secs(20);
        const POLL_INTERVAL: Duration = Duration::from_millis(100);

        let deadline = Instant::now() + TIMEOUT;
        let mut confirmed = false;

        loop {
            if Instant::now() >= deadline {
                return if confirmed {
                    Err(Error::TransactionMetadataUnavailable {
                        signature: signature.to_string(),
                    })
                } else {
                    Err(Error::ConfirmationTimeout {
                        signature: signature.to_string(),
                    })
                };
            }

            if !confirmed {
                let response: serde_json::Value = self
                    .client
                    .send(
                        RpcRequest::GetSignatureStatuses,
                        json!([
                            [signature],
                            {
                                "searchTransactionHistory": true,
                            }
                        ]),
                    )
                    .map_err(Error::rpc)?;

                let statuses = response
                    .get("value")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| {
                        Error::MalformedRpcResponse(
                            "getSignatureStatuses: missing value array".into(),
                        )
                    })?;

                let status = statuses.first().ok_or_else(|| {
                    Error::MalformedRpcResponse("getSignatureStatuses: empty value array".into())
                })?;

                if !status.is_null() && status_is_confirmed(status) {
                    confirmed = true;
                }
            }

            if confirmed {
                if let Some(tx) = self.fetch_transaction_json(signature)? {
                    return Ok(tx);
                }
            }

            let remaining = deadline.saturating_duration_since(Instant::now());

            if !remaining.is_zero() {
                thread::sleep(POLL_INTERVAL.min(remaining));
            }
        }
    }

    /// Submit a signed transaction while retaining its pre-transaction world
    /// for replay, mutation, and time travel after it lands.
    pub fn send_and_capture(&self, tx: VersionedTransaction) -> Result<CapturedTransaction> {
        let mut replay = self.preflight_tx(tx.clone())?;
        let signature = self
            .client
            .send_transaction_with_config(
                &tx,
                RpcSendTransactionConfig {
                    // A reverting transaction must still land so the capture can
                    // capture its program failure as data.
                    skip_preflight: true,
                    ..RpcSendTransactionConfig::default()
                },
            )
            .map_err(Error::rpc)?;

        let tx_json = self.wait_for_transaction(&signature.to_string())?;

        replay.set_recorded(OnchainRecord::from_tx_json(&tx_json));

        Ok(CapturedTransaction {
            signature: signature.to_string(),
            replay,
        })
    }
}

/// Decode base64 into an unsigned transaction for preflight. Accepts either a
/// full serialized `VersionedTransaction`, or a bare serialized message — the
/// "Base64 message" wallets in developer mode show on the approval screen (and
/// what Solana Explorer's inspector takes). A bare message is wrapped with
/// placeholder signatures, which is fine for simulation: sigverify is off.
fn parse_unsigned(tx_b64: &str) -> Result<solana_transaction::versioned::VersionedTransaction> {
    use {base64::Engine, bincode::Options};
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(tx_b64.trim())
        .map_err(|e| Error::TxDecode(format!("bad base64: {e}")))?;
    // Strict parse: Solana's wire format is bincode fixint, and rejecting
    // trailing bytes matters here — the two shapes are prefix-ambiguous (a bare
    // message's first byte can read as a signature count), so a lax parse can
    // "succeed" wrongly. Full consumption plus the signatures==header invariant
    // disambiguates in practice.
    let strict = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes();
    let as_tx = strict.deserialize::<solana_transaction::versioned::VersionedTransaction>(&bytes);
    if let Ok(tx) = &as_tx {
        if tx.signatures.len() == tx.message.header().num_required_signatures as usize {
            return Ok(as_tx.unwrap());
        }
    }
    if let Ok(message) = strict.deserialize::<solana_message::VersionedMessage>(&bytes) {
        // The "Base64 message" a wallet in developer mode shows pre-signing.
        let n = message.header().num_required_signatures as usize;
        return Ok(solana_transaction::versioned::VersionedTransaction {
            signatures: vec![Default::default(); n],
            message,
        });
    }
    // A transaction whose signature count disagrees with its header is unusual
    // but parseable — accept it rather than refuse (sigverify is off anyway).
    match as_tx {
        Ok(tx) => Ok(tx),
        Err(tx_err) => Err(Error::TxDecode(format!(
            "not a serialized transaction ({tx_err}) and not a bare message either — paste \
             either Buffer.from(tx.serialize()).toString('base64') or a wallet's base64 message"
        ))),
    }
}

#[cfg(test)]
mod wait_tests {
    use {super::*, serde_json::json};

    #[test]
    fn confirmed_success_is_landed() {
        let status = json!({
            "confirmationStatus": "confirmed",
            "err": null
        });

        assert!(status_is_confirmed(&status));
    }

    #[test]
    fn confirmed_program_failure_is_still_landed() {
        let status = json!({
            "confirmationStatus": "confirmed",
            "err": {
                "InstructionError": [0, {"Custom": 6001}]
            }
        });

        assert!(status_is_confirmed(&status));
    }

    #[test]
    fn finalized_is_landed() {
        let status = json!({
            "confirmationStatus": "finalized",
            "err": null
        });

        assert!(status_is_confirmed(&status));
    }

    #[test]
    fn processed_is_not_confirmed() {
        let status = json!({
            "confirmationStatus": "processed",
            "err": null
        });

        assert!(!status_is_confirmed(&status));
    }

    #[test]
    fn legacy_rooted_status_is_confirmed() {
        let status = json!({
            "confirmationStatus": null,
            "confirmations": null,
            "err": null
        });

        assert!(status_is_confirmed(&status));
    }
}

#[cfg(test)]
mod preflight_input_tests {
    use {
        super::*,
        base64::Engine,
        solana_message::{Message, VersionedMessage},
        solana_signer::Signer,
        solana_transaction::versioned::VersionedTransaction,
    };

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn transfer_message() -> VersionedMessage {
        let from = solana_keypair::Keypair::new();
        let to = solana_keypair::Keypair::new();
        let ix =
            solana_system_interface::instruction::transfer(&from.pubkey(), &to.pubkey(), 1_000_000);
        VersionedMessage::Legacy(Message::new(&[ix], Some(&from.pubkey())))
    }

    #[test]
    fn accepts_a_full_serialized_transaction() {
        let message = transfer_message();
        let tx = VersionedTransaction {
            signatures: vec![Default::default(); 1],
            message,
        };
        let parsed = parse_unsigned(&b64(&bincode::serialize(&tx).unwrap())).unwrap();
        assert_eq!(parsed.message, tx.message);
    }

    #[test]
    fn accepts_a_bare_wallet_message_and_wraps_it() {
        // What a wallet's developer-mode "Base64 message" is: the serialized
        // message alone, no signatures.
        let message = transfer_message();
        let parsed = parse_unsigned(&b64(&bincode::serialize(&message).unwrap())).unwrap();
        assert_eq!(parsed.message, message);
        assert_eq!(
            parsed.signatures.len(),
            message.header().num_required_signatures as usize
        );
    }

    #[test]
    fn rejects_garbage_with_a_helpful_error() {
        let err = parse_unsigned(&b64(b"not a transaction at all")).unwrap_err();
        assert!(err.to_string().contains("base64 message"), "got: {err}");
        // Not base64 at all.
        assert!(parse_unsigned("%%%not-base64%%%").is_err());
    }
}

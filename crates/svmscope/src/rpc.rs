//! Everything that talks to a Solana node: fetching transactions, accounts,
//! program binaries and IDLs, and assembling a replay context from them.
//! Nothing outside this module holds a client, so a consumer that only
//! replays fixtures pays for the dependency but never touches a node.

use {
    crate::{
        analyze::PreflightOverview,
        cpi_tree::{IxAccount, IxArg},
        decode::{self, AccountInfo},
        error::{Error, Result},
        idl, ixname, preflight,
        replay::{
            b64_decode, with_rent_sysvar, Loaded, LoadedAccounts, PreState, ReplayContext,
            TimeTravel, BPF_LOADER_2, BPF_LOADER_UPGRADEABLE, NATIVE_LOADER, SPL_TOKEN_PROGRAM,
        },
    },
    base64::Engine,
    serde_json::{json, Value},
    solana_account::Account,
    solana_address::Address,
    solana_client::{rpc_client::RpcClient, rpc_request::RpcRequest},
    solana_transaction::versioned::VersionedTransaction,
    std::{collections::HashMap, str::FromStr},
};

/// Fetch a program's on-chain IDL. There are two publishing mechanisms and
/// Explorer reads both, so we do too: the legacy Anchor IDL account
/// (`anchor:idl` seed), then the newer Program Metadata program; then the
/// IDLs bundled with the crate, and as a last resort the handler names an
/// Anchor binary still carries in its read-only data.
pub(crate) fn fetch_idl_json(client: &RpcClient, program_id: Address) -> Option<Value> {
    let id = program_id.to_string();
    idl::anchor_idl_address(&program_id)
        // Most programs don't publish here — a missing account is the common case.
        .and_then(|a| client.get_account_data(&a).ok())
        .and_then(|data| idl::idl_from_anchor_account(&data))
        .or_else(|| {
            idl::program_metadata_idl_address(&program_id)
                .and_then(|a| client.get_account_data(&a).ok())
                .and_then(|data| idl::idl_from_program_metadata(&data))
        })
        .or_else(|| crate::bundled_idls::bundled_idl(&id))
        .or_else(|| {
            fetch_program_elf(client, &id).and_then(|elf| idl::synthesize_from_elf(&elf, &id))
        })
}

/// An IDL lookup that fetches each program's IDL at most once, remembering
/// programs that have none (`None`) so they are not asked again.
pub(crate) fn idl_lookup<'a>(
    client: &'a RpcClient,
    cache: &'a mut HashMap<String, Option<Value>>,
) -> impl FnMut(&str) -> Option<Value> + 'a {
    move |p: &str| {
        cache
            .entry(p.to_string())
            .or_insert_with(|| {
                Address::from_str(p)
                    .ok()
                    .and_then(|a| fetch_idl_json(client, a))
            })
            .clone()
    }
}

/// The pre-sign overview of an unsigned transaction: ALT resolution, IDL
/// lookups and one balance read are the only node calls.
pub(crate) fn preflight_overview(
    client: &RpcClient,
    idl_cache: &mut HashMap<String, Option<Value>>,
    tx: &VersionedTransaction,
) -> PreflightOverview {
    let alt = resolve_alt_addresses(client, tx);
    let fee_payer_balance = tx
        .message
        .static_account_keys()
        .first()
        .and_then(|k| client.get_balance(k).ok());
    let mut lookup = idl_lookup(client, idl_cache);
    preflight::build_overview(tx, alt, &mut lookup, fee_payer_balance)
}

/// Process-wide cache of program ELFs, keyed by programdata address and the
/// slot it was last upgraded at. Program binaries are the largest thing a replay
/// downloads (Jupiter alone is over a megabyte) and they change only on upgrade,
/// so one 45-byte header fetch per program replaces the full download whenever
/// the upgrade slot is unchanged.
type ElfCache = HashMap<(String, u64), std::sync::Arc<Vec<u8>>>;
static ELF_CACHE: std::sync::LazyLock<std::sync::Mutex<ElfCache>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
const ELF_CACHE_MAX_ENTRIES: usize = 256;

/// The ELF inside an upgradeable program's programdata account, served from
/// [`ELF_CACHE`] when the account's upgrade slot has not changed.
fn fetch_programdata_elf_cached(client: &RpcClient, pd_addr: &str) -> Option<Vec<u8>> {
    // Header only: [0..4] variant, [4..12] last-upgrade slot, [12..45] authority.
    let header: serde_json::Value = client
        .send(
            RpcRequest::GetAccountInfo,
            json!([pd_addr, { "encoding": "base64", "commitment": "confirmed", "dataSlice": { "offset": 0, "length": 45 } }]),
        )
        .ok()?;
    let head = b64_decode(header["value"]["data"][0].as_str()?);
    let slot = head
        .get(4..12)
        .and_then(|b| b.try_into().ok())
        .map(u64::from_le_bytes)?;
    let key = (pd_addr.to_string(), slot);
    if let Ok(cache) = ELF_CACHE.lock() {
        if let Some(elf) = cache.get(&key) {
            return Some(elf.as_ref().clone());
        }
    }
    let elf = fetch_account_data(client, pd_addr)
        .filter(|d| d.len() > 45)
        .map(|d| d[45..].to_vec())?;
    if let Ok(mut cache) = ELF_CACHE.lock() {
        if cache.len() >= ELF_CACHE_MAX_ENTRIES {
            cache.clear();
        }
        cache.insert(key, std::sync::Arc::new(elf.clone()));
    }
    Some(elf)
}

/// A program's ELF given its program id, whichever loader owns it: the
/// programdata account for the upgradeable loader (served from [`ELF_CACHE`]),
/// or the program account itself for the legacy loaders.
pub(crate) fn fetch_program_elf(client: &RpcClient, program: &str) -> Option<Vec<u8>> {
    let data = fetch_account_data(client, program)?;
    if data.starts_with(b"\x7fELF") {
        return Some(data);
    }
    // Upgradeable loader program account: [0..4]=variant (2 = Program), [4..36]=programdata.
    if data.len() == 36 && data[..4] == [2, 0, 0, 0] {
        let pd: [u8; 32] = data[4..36].try_into().ok()?;
        return fetch_programdata_elf_cached(client, &Address::from(pd).to_string());
    }
    None
}

/// Fetch a single account's raw data via getAccountInfo.
fn fetch_account_data(client: &RpcClient, address: &str) -> Option<Vec<u8>> {
    let resp: serde_json::Value = client
        .send(
            RpcRequest::GetAccountInfo,
            json!([address, { "encoding": "base64", "commitment": "confirmed" }]),
        )
        .ok()?;
    let data_b64 = resp["value"]["data"][0].as_str()?;
    Some(b64_decode(data_b64))
}

fn fetch_account_at_slot(
    archive: &RpcClient,
    address: &str,
    slot: u64,
) -> Option<serde_json::Value> {
    let resp: serde_json::Value = archive
        .send(
            RpcRequest::GetAccountInfo,
            json!([address, { "encoding": "base64", "slot": slot }]),
        )
        .ok()?;
    let v = &resp["value"];
    if v.is_null() {
        None
    } else {
        Some(v.clone())
    }
}

/// How far behind the transaction's slot the archive probe asks. A non-archive
/// answers every query at its current tip, so probing at the transaction's own
/// slot is fooled whenever that slot *is* the tip (a transaction from the latest
/// finalized slot, or anything on a fresh localnet). Probing ~10k slots
/// (about an hour) earlier leaves no tip a non-archive could answer from.
const ARCHIVE_PROBE_LAG: u64 = 10_000;

/// Verify the endpoint actually honors historical `slot` queries before we
/// trust anything it returns. A non-archival RPC (Helius, a public node)
/// silently *ignores* the `slot` param and answers at the current tip — which
/// would make a "replay at slot" quietly wrong. We probe an always-present
/// account (the SPL Token program) at a slot well before the transaction's and
/// confirm the response was evaluated no later than that.
pub(crate) fn archive_honors_slot(archive: &RpcClient, slot: u64) -> bool {
    let probe = slot.saturating_sub(ARCHIVE_PROBE_LAG);
    let resp: serde_json::Value = match archive.send(
        RpcRequest::GetAccountInfo,
        json!([SPL_TOKEN_PROGRAM, { "encoding": "base64", "slot": probe }]),
    ) {
        Ok(v) => v,
        Err(_) => return false,
    };
    // An archive evaluates the query at (≤) the requested slot; a non-archive
    // ignores `slot` and answers at the current tip, which is past `probe`
    // by construction (the transaction itself landed after it).
    match resp["context"]["slot"].as_u64() {
        Some(ctx_slot) => ctx_slot <= probe,
        None => false,
    }
}

fn fetch_account_data_at_slot(archive: &RpcClient, address: &str, slot: u64) -> Option<Vec<u8>> {
    Some(b64_decode(
        fetch_account_at_slot(archive, address, slot)?["data"][0].as_str()?,
    ))
}

fn fetch_loaded_at_slot(
    archive: &RpcClient,
    account_keys: &[String],
    slot: u64,
) -> Result<LoadedAccounts> {
    let mut out: Vec<(Address, Loaded)> = Vec::new();
    let mut existing: HashMap<String, ()> = HashMap::new();

    for key in account_keys {
        let Some(acc) = fetch_account_at_slot(archive, key, slot) else {
            continue;
        };
        existing.insert(key.clone(), ());
        let Ok(address) = Address::from_str(key) else {
            continue;
        };
        let owner = acc["owner"].as_str().unwrap_or_default();
        let executable = acc["executable"].as_bool().unwrap_or(false);

        if executable {
            let elf: Option<Vec<u8>> = if owner == NATIVE_LOADER {
                None
            } else if owner == BPF_LOADER_2 {
                Some(b64_decode(acc["data"][0].as_str().unwrap_or_default()))
            } else if owner == BPF_LOADER_UPGRADEABLE {
                let prog = b64_decode(acc["data"][0].as_str().unwrap_or_default());
                if prog.len() >= 36 {
                    let pd_bytes: [u8; 32] = prog[4..36].try_into().unwrap();
                    let pd_addr = Address::from(pd_bytes);
                    // programdata AT THE SAME SLOT → the ELF that was live then
                    fetch_account_data_at_slot(archive, &pd_addr.to_string(), slot)
                        .filter(|d| d.len() > 45)
                        .map(|d| d[45..].to_vec())
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(elf) = elf {
                out.push((address, Loaded::Program(elf)));
            }
            continue;
        }

        let Ok(owner_addr) = Address::from_str(owner) else {
            continue;
        };
        out.push((
            address,
            Loaded::Data(Account {
                lamports: acc["lamports"].as_u64().unwrap_or(0),
                data: b64_decode(acc["data"][0].as_str().unwrap_or_default()),
                owner: owner_addr,
                executable: false,
                rent_epoch: 0,
            }),
        ));
    }
    Ok((out, existing))
}

/// Fetch each account's on-chain state and resolve program ELFs into loadables.
///
/// This does all the network I/O; building a fresh SVM from the result is then
/// free, which is what lets a whole scenario suite run without re-fetching.
/// Returns the loadables plus the set of addresses that actually exist on-chain
/// (non-null) — including programs whose ELF we couldn't resolve. The caller uses
/// that set to tell a genuinely-closed account (safe to reconstruct) apart from a
/// program that merely failed to load (must NOT be reconstructed as data).
fn fetch_loaded(client: &RpcClient, account_keys: &[String]) -> Result<LoadedAccounts> {
    let resp: serde_json::Value = client
        .send(
            RpcRequest::GetMultipleAccounts,
            json!([
                account_keys,
                { "encoding": "base64", "commitment": "confirmed" }
            ]),
        )
        .map_err(Error::rpc)?;

    let accounts = resp["value"]
        .as_array()
        .ok_or_else(|| Error::MalformedRpcResponse("getMultipleAccounts: no value array".into()))?;
    let mut out: Vec<(Address, Loaded)> = Vec::new();
    let mut existing: HashMap<String, ()> = HashMap::new();

    for (i, acc) in accounts.iter().enumerate() {
        if acc.is_null() {
            continue; // account doesn't exist (created during the tx, etc.)
        }
        existing.insert(account_keys[i].clone(), ());
        let Ok(address) = Address::from_str(&account_keys[i]) else {
            continue;
        };
        let owner = acc["owner"].as_str().unwrap_or_default();
        let executable = acc["executable"].as_bool().unwrap_or(false);

        if executable {
            // Resolve the program's ELF bytecode based on which loader owns it.
            let elf: Option<Vec<u8>> = if owner == NATIVE_LOADER {
                None // native programs are built into LiteSVM
            } else if owner == BPF_LOADER_2 {
                Some(b64_decode(acc["data"][0].as_str().unwrap_or_default()))
            } else if owner == BPF_LOADER_UPGRADEABLE {
                // Upgradeable: program account is a pointer; bytes 4..36 = the
                // programdata address, ELF starts at offset 45 inside it.
                let prog = b64_decode(acc["data"][0].as_str().unwrap_or_default());
                if prog.len() >= 36 {
                    let pd_bytes: [u8; 32] = prog[4..36].try_into().unwrap();
                    let pd_addr = Address::from(pd_bytes);
                    fetch_programdata_elf_cached(client, &pd_addr.to_string())
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(elf) = elf {
                out.push((address, Loaded::Program(elf)));
            }
            continue;
        }

        let Ok(owner_addr) = Address::from_str(owner) else {
            continue;
        };
        out.push((
            address,
            Loaded::Data(Account {
                lamports: acc["lamports"].as_u64().unwrap_or(0),
                data: b64_decode(acc["data"][0].as_str().unwrap_or_default()),
                owner: owner_addr,
                executable: false,
                rent_epoch: 0,
            }),
        ));
    }
    Ok((out, existing))
}

/// Fetch everything needed to replay `signature` (transaction + all touched
/// accounts + program ELFs, including Address Lookup Table accounts) once.
/// `pre_state` restores pre-transaction token balances so swaps replay faithfully
/// instead of slipping. (`_tx_slot` is the transaction's original slot, kept for
/// the signature's sake but no longer used to set the clock — see below.)
pub(crate) fn build_context(
    client: &RpcClient,
    signature: &str,
    account_keys: &[String],
    _tx_slot: Option<u64>,
    pre_state: &PreState,
) -> Result<ReplayContext> {
    // Replay runs against CURRENT account state — an RPC returns today's accounts,
    // not the tx-time snapshot. Anchoring the clock to the transaction's ORIGINAL
    // slot then contradicts that state: a program that checks the clock against an
    // account's (now newer) last-updated timestamp reverts with InvalidTimestamp
    // (e.g. Orca's oracle check). So we anchor the clock to NOW, consistent with the
    // accounts we actually loaded, and let time travel warp forward from there.
    let slot = client.get_slot().ok().or(_tx_slot);
    let block_time = slot.and_then(|s| client.get_block_time(s).ok());
    let tx = fetch_transaction(client, signature)?;
    let mut all_keys = account_keys.to_vec();
    if let Some(lookups) = tx.message.address_table_lookups() {
        for l in lookups {
            all_keys.push(l.account_key.to_string());
        }
    }
    with_rent_sysvar(&mut all_keys);
    let (mut loaded, existing) = fetch_loaded(client, &all_keys)?;

    if !pre_state.is_empty() {
        // Reconstruct accounts that existed at tx-time but are *closed now* (null
        // on-chain), so the transaction can load — a missing fee payer alone fails
        // it at cu=0. Crucially we key off `existing` (what getMultipleAccounts
        // actually returned), not what loaded: a program whose ELF failed to
        // resolve still exists, and must not be rebuilt as a data account.
        for key in account_keys {
            if existing.contains_key(key) {
                continue;
            }
            if let (Some(acc), Ok(addr)) = (pre_state.reconstruct(key), Address::from_str(key)) {
                loaded.push((addr, Loaded::Data(acc)));
            }
        }

        // Rewind still-existing accounts to their pre-transaction balances,
        // reconstructed from the transaction's own metadata (free on any RPC):
        // SOL/lamport balances from `preBalances`, SPL token amounts from
        // `preTokenBalances`. This is metadata reconstruction — faithful for
        // balances, though account *data* is still current-state.
        for (addr, l) in loaded.iter_mut() {
            if let Loaded::Data(acc) = l {
                let key = addr.to_string();
                if let Some(&lamports) = pre_state.lamports.get(&key) {
                    acc.lamports = lamports;
                }
                if let Some(&amt) = pre_state.token_amounts.get(&key) {
                    if acc.data.len() >= 72 {
                        acc.data[64..72].copy_from_slice(&amt.to_le_bytes());
                    }
                }
            }
        }
    }

    Ok(ReplayContext {
        signature: signature.to_string(),
        tx,
        loaded,
        slot,
        block_time,
        time_travel: TimeTravel::default(),
        idls: HashMap::new(),
        feature_toggles: Vec::new(),
    })
}

/// `state_slot` is the archival boundary accounts are fetched at; `clock_slot`
/// anchors the replay clock. They differ when replaying a transaction *at its
/// own slot S*: the archive answers "latest version at or before S", which for
/// accounts the transaction itself wrote is **post**-transaction state — so the
/// caller passes `state_slot = S - 1` (end of the previous slot) and patches
/// same-slot predecessors' balance effects from the transaction's own recorded
/// pre-balances via `pre_state`. Same-slot predecessor *data* writes remain the
/// one disclosed gap of a slot-granular archive.
pub(crate) fn build_context_at_slot(
    archive: &RpcClient,
    signature: &str,
    account_keys: &[String],
    state_slot: u64,
    clock_slot: u64,
    tx_block_time: Option<i64>,
    pre_state: &PreState,
) -> Result<ReplayContext> {
    let tx = fetch_transaction(archive, signature)?;
    let mut all_keys = account_keys.to_vec();

    if let Some(lookups) = tx.message.address_table_lookups() {
        for l in lookups {
            all_keys.push(l.account_key.to_string());
        }
    };
    with_rent_sysvar(&mut all_keys);
    let (mut loaded, existing) = fetch_loaded_at_slot(archive, &all_keys, state_slot)?;

    if !pre_state.is_empty() {
        for key in account_keys {
            if existing.contains_key(key) {
                continue;
            }
            if let (Some(acc), Ok(addr)) = (pre_state.reconstruct(key), Address::from_str(key)) {
                loaded.push((addr, Loaded::Data(acc)));
            }
        }

        // The transaction's own recorded pre-balances are ground truth at its
        // boundary — they override the archive wherever they disagree (a
        // same-slot predecessor transaction wrote the account after S-1).
        for (addr, l) in loaded.iter_mut() {
            if let Loaded::Data(acc) = l {
                let key = addr.to_string();
                if let Some(&lamports) = pre_state.lamports.get(&key) {
                    acc.lamports = lamports;
                }
                if let Some(&amt) = pre_state.token_amounts.get(&key) {
                    if acc.data.len() >= 72 {
                        acc.data[64..72].copy_from_slice(&amt.to_le_bytes());
                    }
                }
            }
        }
    }

    Ok(ReplayContext {
        signature: signature.to_string(),
        tx,
        loaded,
        slot: Some(clock_slot),
        block_time: tx_block_time,
        time_travel: TimeTravel::default(),
        idls: HashMap::new(),
        feature_toggles: Vec::new(),
    })
}

/// Resolve an (unsigned) transaction's Address Lookup Table references to concrete
/// addresses — writable first, then readonly, the order the runtime resolves them.
/// An ALT account stores its address list at offset 56, 32 bytes each.
pub(crate) fn resolve_alt_addresses(
    client: &RpcClient,
    tx: &VersionedTransaction,
) -> (Vec<String>, Vec<String>) {
    let mut writable = Vec::new();
    let mut readonly = Vec::new();
    if let Some(lookups) = tx.message.address_table_lookups() {
        for l in lookups {
            let Some(data) = fetch_account_data(client, &l.account_key.to_string()) else {
                continue;
            };
            let read = |idx: u8| -> Option<String> {
                let off = 56 + idx as usize * 32;
                let bytes: [u8; 32] = data.get(off..off + 32)?.try_into().ok()?;
                Some(Address::from(bytes).to_string())
            };
            for &idx in &l.writable_indexes {
                if let Some(a) = read(idx) {
                    writable.push(a);
                }
            }
            for &idx in &l.readonly_indexes {
                if let Some(a) = read(idx) {
                    readonly.push(a);
                }
            }
        }
    }
    (writable, readonly)
}

/// Build a replay context for an **unsigned / pre-flight** transaction — one that
/// hasn't been sent yet. Resolves its accounts (incl. ALTs), loads their *current*
/// on-chain state (which, for a not-yet-sent tx, IS the pre-state — no drift, no
/// archival), and sets the slot to the current slot so ALT resolution works.
pub(crate) fn preflight_context(
    client: &RpcClient,
    tx: VersionedTransaction,
) -> Result<ReplayContext> {
    let mut keys: Vec<String> = tx
        .message
        .static_account_keys()
        .iter()
        .map(|k| k.to_string())
        .collect();
    let (writable, readonly) = resolve_alt_addresses(client, &tx);
    keys.extend(writable);
    keys.extend(readonly);

    // Also load the ALT accounts themselves so LiteSVM can resolve the lookups.
    let mut all_keys = keys.clone();
    if let Some(lookups) = tx.message.address_table_lookups() {
        for l in lookups {
            all_keys.push(l.account_key.to_string());
        }
    }
    with_rent_sysvar(&mut all_keys);
    let (loaded, _existing) = fetch_loaded(client, &all_keys)?;
    let slot = client.get_slot().ok();
    // Real wall-clock time for that slot — a pre-flight simulation should run
    // "now", and any date-based program logic depends on this being accurate.
    let block_time = slot.and_then(|s| client.get_block_time(s).ok());
    let signature = tx
        .signatures
        .first()
        .map(|s| s.to_string())
        .unwrap_or_default();
    Ok(ReplayContext {
        signature,
        tx,
        loaded,
        slot,
        block_time,
        time_travel: TimeTravel::default(),
        idls: HashMap::new(),
        feature_toggles: Vec::new(),
    })
}

/// Fetch the raw transaction (base64 wire bytes) and deserialize it.
fn fetch_transaction(client: &RpcClient, signature: &str) -> Result<VersionedTransaction> {
    let resp: serde_json::Value = client
        .send(
            RpcRequest::GetTransaction,
            json!([signature, { "encoding": "base64", "maxSupportedTransactionVersion": 0 }]),
        )
        .map_err(Error::rpc)?;
    if resp.is_null() {
        return Err(Error::TransactionNotFound(signature.to_string()));
    }
    let tx_b64 = resp["transaction"][0].as_str().ok_or_else(|| {
        Error::MalformedRpcResponse(format!("no base64 transaction in response for {signature}"))
    })?;
    let bytes = b64_decode(tx_b64);
    bincode::deserialize::<VersionedTransaction>(&bytes)
        .map_err(|e| Error::TxDecode(format!("{signature}: {e}")))
}

/// Fetch each account's on-chain state and decode any recognized layouts.
///
/// Parallel to `account_keys`; accounts that don't exist are skipped.
pub(crate) fn describe_accounts(client: &RpcClient, account_keys: &[String]) -> Vec<AccountInfo> {
    let resp: serde_json::Value = match client.send(
        RpcRequest::GetMultipleAccounts,
        json!([account_keys, { "encoding": "base64" }]),
    ) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let values = match resp["value"].as_array() {
        Some(v) => v,
        None => return vec![],
    };

    // Cache each program's IDL so we fetch it at most once per analysis (an IDL
    // fetch is an RPC call). `None` = we already checked and it has no on-chain IDL.
    let mut idl_cache: std::collections::HashMap<String, Option<serde_json::Value>> =
        std::collections::HashMap::new();

    let mut out = Vec::new();
    for (i, acc) in values.iter().enumerate() {
        if acc.is_null() {
            continue;
        }
        let owner = acc["owner"].as_str().unwrap_or_default().to_string();
        let lamports = acc["lamports"].as_u64().unwrap_or(0);
        let executable = acc["executable"].as_bool().unwrap_or(false);
        let data = acc["data"][0]
            .as_str()
            .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
            .unwrap_or_default();

        // Try the built-in SPL layouts first; fall back to the program's Anchor
        // IDL (fetched + cached) so arbitrary program accounts decode too.
        let decoded = if executable {
            None
        } else {
            // Built-in layouts → the program's on-chain IDL → structural inference.
            decode::decode(&owner, &data)
                .or_else(|| {
                    let idl = idl_cache.entry(owner.clone()).or_insert_with(|| {
                        std::str::FromStr::from_str(&owner)
                            .ok()
                            .and_then(|a| fetch_idl_json(client, a))
                    });
                    idl.as_ref()
                        .and_then(|idl| idl::decode_with_idl(idl, &data))
                })
                .or_else(|| decode::infer_layout(&data))
        };

        let Some(address) = account_keys.get(i) else {
            break; // RPC returned more entries than keys requested — stop, don't panic
        };
        out.push(AccountInfo {
            address: address.clone(),
            owner,
            lamports,
            executable,
            data_len: data.len(),
            decoded,
        });
    }
    out
}

/// Decode an instruction fully: its name, its arguments, and its accounts named
/// from the IDL (Anchor) or a known layout (native). `idl_cache` avoids re-fetching
/// an IDL for every instruction of the same program.
pub(crate) fn enrich(
    client: &RpcClient,
    idl_cache: &mut HashMap<String, Option<Value>>,
    program: &str,
    data: &[u8],
    account_indexes: &[usize],
    account_keys: &[String],
) -> (Option<String>, Vec<IxArg>, Vec<IxAccount>) {
    let mut lookup = |p: &str| -> Option<Value> {
        idl_cache
            .entry(p.to_string())
            .or_insert_with(|| {
                Address::from_str(p)
                    .ok()
                    .and_then(|a| fetch_idl_json(client, a))
            })
            .clone()
    };
    ixname::enrich_with(&mut lookup, program, data, account_indexes, account_keys)
}

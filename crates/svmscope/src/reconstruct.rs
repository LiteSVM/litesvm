//! Historical state reconstruction — rebuild an account's state at a past slot
//! by replaying its own write history forward, instead of buying it from a paid
//! archive. This is the free, self-owned path to "replay at any slot".
//!
//! The raw ledger (which transactions touched an account, and their contents)
//! comes from a [`LedgerSource`]: a standard Solana RPC for recent history, or
//! **Old Faithful** — the free, open, Solana-Foundation-funded archive of the
//! whole chain — for deep history. Both expose the same `getSignaturesForAddress`
//! / `getTransaction` methods, so the engine is source-agnostic; only the reach
//! differs. That is what lets this be built and tested against an ordinary RPC
//! today, and pointed at Old Faithful for full history later.

use {
    crate::{
        error::{Error, Result},
        fidelity::AccountState,
        mutation::Mutation,
        scope::Scope,
    },
    serde_json::{json, Value},
    solana_client::{rpc_client::RpcClient, rpc_request::RpcRequest},
    std::collections::HashMap,
};

/// One transaction that touched an account, and where it landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteRef {
    /// The transaction signature.
    pub signature: String,
    /// The slot it landed in.
    pub slot: u64,
    /// Whether it failed (a failed transaction changes no account data, only fees).
    pub failed: bool,
}

/// A source of raw ledger data for reconstruction. A standard RPC implements it
/// for whatever history it retains; an Old Faithful node implements it for all
/// history. The reconstruction engine is written against this trait, not against
/// any one provider.
pub trait LedgerSource {
    /// Signatures that touched `address`, newest-first, paging back from the
    /// `before` signature when given. At most `limit` returned.
    fn signatures_for_address(
        &self,
        address: &str,
        before: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WriteRef>>;

    /// The full `getTransaction` JSON for a signature.
    fn transaction(&self, signature: &str) -> Result<Value>;
}

/// A [`LedgerSource`] backed by a standard Solana JSON-RPC endpoint. Its reach is
/// whatever history that endpoint keeps; swap in an Old Faithful source for the
/// full chain without changing the reconstruction engine.
pub struct RpcLedger {
    client: RpcClient,
}

impl RpcLedger {
    /// A ledger source over the given RPC endpoint.
    pub fn new(url: impl Into<String>) -> RpcLedger {
        RpcLedger {
            client: RpcClient::new(url.into()),
        }
    }
}

impl LedgerSource for RpcLedger {
    fn signatures_for_address(
        &self,
        address: &str,
        before: Option<&str>,
        limit: usize,
    ) -> Result<Vec<WriteRef>> {
        let mut opts = json!({ "limit": limit });
        if let Some(b) = before {
            opts["before"] = json!(b);
        }
        let resp: Value = self
            .client
            .send(RpcRequest::GetSignaturesForAddress, json!([address, opts]))
            .map_err(Error::rpc)?;
        let arr = resp.as_array().ok_or_else(|| {
            Error::MalformedRpcResponse("getSignaturesForAddress: not an array".into())
        })?;
        Ok(arr
            .iter()
            .filter_map(|s| {
                Some(WriteRef {
                    signature: s["signature"].as_str()?.to_string(),
                    slot: s["slot"].as_u64()?,
                    failed: !s["err"].is_null(),
                })
            })
            .collect())
    }

    fn transaction(&self, signature: &str) -> Result<Value> {
        let resp: Value = self
            .client
            .send(
                RpcRequest::GetTransaction,
                json!([
                    signature,
                    { "encoding": "json", "commitment": "confirmed", "maxSupportedTransactionVersion": 0 }
                ]),
            )
            .map_err(Error::rpc)?;
        if resp.is_null() {
            return Err(Error::TransactionNotFound(signature.to_string()));
        }
        Ok(resp)
    }
}

/// The successful transactions that wrote `address` strictly before `slot`,
/// **oldest-first** — the ordered write history to replay when reconstructing the
/// account's state going into `slot`. Pages back through the ledger source until
/// it gathers `max` writes, exhausts history, or hits `max_pages` (a guard so a
/// very active account can't page unboundedly on a first pass).
pub fn write_history(
    src: &dyn LedgerSource,
    address: &str,
    before_slot: u64,
    max: usize,
    max_pages: usize,
) -> Result<Vec<WriteRef>> {
    let page = 1000usize.min(max.max(1));
    let mut out: Vec<WriteRef> = Vec::new();
    let mut before: Option<String> = None;

    for _ in 0..max_pages {
        let batch = src.signatures_for_address(address, before.as_deref(), page)?;
        let Some(last) = batch.last() else {
            break; // history exhausted
        };
        before = Some(last.signature.clone());
        for w in &batch {
            if w.slot < before_slot && !w.failed {
                out.push(w.clone());
                if out.len() >= max {
                    break;
                }
            }
        }
        if out.len() >= max {
            break;
        }
    }

    // The source returns newest-first; the replay wants oldest-first.
    out.reverse();
    Ok(out)
}

/// The result of reconstructing an account's state at a slot.
#[derive(Debug, Clone)]
pub struct Reconstructed {
    /// The account reconstructed.
    pub address: String,
    /// The slot the state is reconstructed as-of (state going *into* this slot).
    pub before_slot: u64,
    /// The reconstructed state, or `None` if the account did not exist at the slot.
    pub state: Option<AccountState>,
    /// How many writes were successfully replayed into the reconstruction.
    pub writes_replayed: usize,
    /// How many writes in range were skipped (couldn't be loaded/replayed) — a
    /// direct measure of how approximate the result is.
    pub writes_skipped: usize,
}

/// Reconstruct `address`'s state going into `before_slot` by replaying its write
/// history forward with LiteSVM — the free alternative to buying the state from
/// an archive.
///
/// Each write is replayed with the account's running reconstructed state injected
/// (via a data + lamports mutation), then the account is read out again to carry
/// forward. Co-accounts are loaded at current state (best-effort), so this is
/// **exact** where an update depends on the account itself plus the instruction
/// data, and **approximate** where it depends on deeply-coupled co-account state
/// — `writes_skipped` reports how much couldn't be replayed.
///
/// For an exact result the history must reach back to the account's creation;
/// raise `max_writes` / `max_pages` for accounts with long histories.
pub fn reconstruct_account(
    scope: &Scope,
    ledger: &dyn LedgerSource,
    address: &str,
    before_slot: u64,
    max_writes: usize,
    max_pages: usize,
) -> Result<Reconstructed> {
    let history = write_history(ledger, address, before_slot, max_writes, max_pages)?;

    let mut state: Option<AccountState> = None;
    let mut replayed = 0usize;
    let mut skipped = 0usize;

    for w in &history {
        let replay = match scope.replay(&w.signature) {
            Ok(r) => r,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        // Inject the account's running reconstructed state before replaying this
        // write; the very first write starts from whatever the account was
        // (empty at creation).
        let muts: Vec<Mutation> = match &state {
            Some(s) => vec![
                Mutation::data(address.to_string(), s.data.clone()),
                Mutation::lamports(address.to_string(), s.lamports),
            ],
            None => Vec::new(),
        };
        match replay.account_after(&muts, address) {
            Ok(after) => {
                state = after; // Some = new state; None = closed by this write
                replayed += 1;
            }
            Err(_) => skipped += 1,
        }
    }

    Ok(Reconstructed {
        address: address.to_string(),
        before_slot,
        state,
        writes_replayed: replayed,
        writes_skipped: skipped,
    })
}

// --- exact recursive reconstruction ----------------------------------------

/// A reconstructed account plus whether it was reconstructed *exactly*.
#[derive(Debug, Clone)]
pub struct Recon {
    /// The reconstructed state, or `None` if the account did not exist at the slot.
    pub state: Option<AccountState>,
    /// `true` only if every input in the dependency cone was itself reconstructed
    /// exactly within budget — no co-account fell back to current state.
    pub exact: bool,
}

/// Well-known infrastructure accounts (programs, sysvars) that are static or
/// runtime-provided: never the coupled economic state we reconstruct, and
/// recursing into them only wastes budget. `scope.replay` loads them correctly
/// as-is (current program ELF / runtime sysvars).
fn is_infra(address: &str) -> bool {
    address.starts_with("Sysvar")
        || matches!(
            address,
            "11111111111111111111111111111111"                 // System
                | "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" // SPL Token
                | "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb" // Token-2022
                | "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL" // Associated Token
                | "ComputeBudget111111111111111111111111111111"
                | "NativeLoader1111111111111111111111111111111"
                | "BPFLoader2111111111111111111111111111111111"
                | "BPFLoaderUpgradeab1e11111111111111111111111"
        )
}

/// The single most-recent successful write to `address` strictly before `slot`.
fn last_write(
    src: &dyn LedgerSource,
    address: &str,
    slot: u64,
    max_pages: usize,
) -> Result<Option<WriteRef>> {
    Ok(write_history(src, address, slot, 1, max_pages)?
        .into_iter()
        .next())
}

/// Recursive, memoized dependency-cone reconstruction — the exact engine. To get
/// an account's state at a slot it finds the last write, recursively reconstructs
/// **that write's inputs at that write's slot**, replays the write against those
/// exact inputs, and reads the account back out. Because every recursion targets
/// a strictly-earlier slot the dependency graph is a DAG, so it terminates;
/// memoization shares sub-results; a replay budget bounds the cone.
pub struct Reconstructor<'a> {
    scope: &'a Scope,
    ledger: &'a dyn LedgerSource,
    memo: HashMap<(String, u64), Recon>,
    budget: usize,
    max_pages: usize,
    replays: usize,
}

impl<'a> Reconstructor<'a> {
    /// A reconstructor over `scope` (for replay) and `ledger` (for history),
    /// allowed at most `budget` transaction replays before falling back to
    /// current state for the remaining cone (keeping it tractable, and honest).
    pub fn new(scope: &'a Scope, ledger: &'a dyn LedgerSource, budget: usize) -> Self {
        Reconstructor {
            scope,
            ledger,
            memo: HashMap::new(),
            budget,
            max_pages: 20,
            replays: 0,
        }
    }

    /// How many transaction replays the reconstruction actually performed.
    pub fn replays(&self) -> usize {
        self.replays
    }

    /// Reconstruct `address`'s exact state going into `before_slot`.
    pub fn reconstruct(&mut self, address: &str, before_slot: u64) -> Result<Recon> {
        let key = (address.to_string(), before_slot);
        if let Some(r) = self.memo.get(&key) {
            return Ok(r.clone());
        }

        let recon = self.compute(address, before_slot)?;
        self.memo.insert(key, recon.clone());
        Ok(recon)
    }

    fn compute(&mut self, address: &str, before_slot: u64) -> Result<Recon> {
        // The last write before the slot; none means the account didn't exist yet.
        let Some(last) = last_write(self.ledger, address, before_slot, self.max_pages)? else {
            return Ok(Recon {
                state: None,
                exact: true,
            });
        };

        // Out of budget: fall back to current state, honestly marked inexact.
        if self.replays >= self.budget {
            let state = self.scope.account_data(address)?;
            return Ok(Recon {
                state,
                exact: false,
            });
        }

        // Reconstruct every input of the last write at that write's slot, then
        // replay the write against those exact inputs.
        let tx = self.ledger.transaction(&last.signature)?;
        let keys = crate::utils::resolve_account_keys(&tx);
        let mut muts: Vec<Mutation> = Vec::new();
        let mut cone_exact = true;

        for b in &keys {
            if is_infra(b) {
                continue; // static program / runtime sysvar — loaded correctly as-is
            }
            let rec_b = self.reconstruct(b, last.slot)?;
            cone_exact &= rec_b.exact;
            if let Some(s) = rec_b.state {
                muts.push(Mutation::data(b.clone(), s.data));
                muts.push(Mutation::lamports(b.clone(), s.lamports));
            }
        }

        self.replays += 1;
        let replay = match self.scope.replay(&last.signature) {
            Ok(r) => r,
            Err(_) => {
                let state = self.scope.account_data(address)?;
                return Ok(Recon {
                    state,
                    exact: false,
                });
            }
        };
        let state = replay.account_after(&muts, address)?;
        Ok(Recon {
            state,
            exact: cone_exact,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A canned ledger for testing the walker without a network — `sigs` are
    /// newest-first, as a real `getSignaturesForAddress` returns them.
    struct MockLedger {
        sigs: Vec<WriteRef>,
    }

    impl LedgerSource for MockLedger {
        fn signatures_for_address(
            &self,
            _address: &str,
            before: Option<&str>,
            limit: usize,
        ) -> Result<Vec<WriteRef>> {
            let start = match before {
                None => 0,
                Some(b) => self
                    .sigs
                    .iter()
                    .position(|w| w.signature == b)
                    .map(|i| i + 1)
                    .unwrap_or(self.sigs.len()),
            };
            Ok(self.sigs[start..].iter().take(limit).cloned().collect())
        }
        fn transaction(&self, _signature: &str) -> Result<Value> {
            Ok(json!({}))
        }
    }

    fn w(sig: &str, slot: u64, failed: bool) -> WriteRef {
        WriteRef {
            signature: sig.to_string(),
            slot,
            failed,
        }
    }

    #[test]
    fn write_history_keeps_pre_slot_successes_oldest_first() {
        // newest-first, spanning the target slot 100, with a failed tx mixed in.
        let ledger = MockLedger {
            sigs: vec![
                w("s120", 120, false), // after the slot → excluded
                w("s090", 90, false),
                w("s080", 80, true), // failed → excluded
                w("s070", 70, false),
                w("s060", 60, false),
            ],
        };
        let hist = write_history(&ledger, "Acc", 100, 100, 10).unwrap();
        let sigs: Vec<&str> = hist.iter().map(|w| w.signature.as_str()).collect();
        // oldest-first, only successful writes strictly before slot 100.
        assert_eq!(sigs, vec!["s060", "s070", "s090"]);
    }

    #[test]
    fn infra_accounts_are_recognised_and_skipped() {
        assert!(is_infra("11111111111111111111111111111111")); // System
        assert!(is_infra("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")); // SPL Token
        assert!(is_infra("SysvarC1ock11111111111111111111111111111111")); // a sysvar
        assert!(is_infra("ComputeBudget111111111111111111111111111111"));
        // A normal PDA / data account is NOT infra — it gets reconstructed.
        assert!(!is_infra("Cd2zEXTrYoV4UcDxZJumwZsz4A1bSZfRuZpEu5RJDDVk"));
    }

    #[test]
    fn last_write_returns_the_most_recent_before_the_slot() {
        let ledger = MockLedger {
            sigs: vec![
                w("s150", 150, false),
                w("s099", 99, false), // most recent before 100
                w("s050", 50, false),
            ],
        };
        let lw = last_write(&ledger, "Acc", 100, 10).unwrap().unwrap();
        assert_eq!(lw.signature, "s099");
        assert!(last_write(&ledger, "Acc", 40, 10).unwrap().is_none());
    }

    #[test]
    fn write_history_respects_the_max_cap() {
        let ledger = MockLedger {
            sigs: (0..50)
                .map(|i| w(&format!("s{i:02}"), 50 - i, false))
                .collect(),
        };
        let hist = write_history(&ledger, "Acc", 1000, 5, 10).unwrap();
        assert_eq!(hist.len(), 5);
        // still oldest-first
        assert!(hist[0].slot < hist[hist.len() - 1].slot);
    }
}

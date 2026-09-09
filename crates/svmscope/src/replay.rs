//! Stage 3 — Deterministic replay.
//!
//! Loads the transaction's accounts AND programs into an in-process SVM
//! (LiteSVM) so the transaction can be re-executed locally.

use {
    crate::{
        analyze::ReplayResult,
        check::{AssertOutcome, ScenarioOutcome},
        error::{Error, Result},
        fixture::{Fixture, FixtureEntry},
        mutation::{
            field_bytes, find_field, read_field_int, read_u64_at, AccountAssert, Expect, Mutation,
            StateCheck,
        },
    },
    base64::Engine,
    litesvm::{types::TransactionResult, LiteSVM},
    solana_account::Account,
    solana_address::Address,
    solana_clock::Clock,
    solana_transaction::versioned::VersionedTransaction,
    std::{collections::HashMap, str::FromStr},
};

pub(crate) const NATIVE_LOADER: &str = "NativeLoader1111111111111111111111111111111";
pub(crate) const BPF_LOADER_2: &str = "BPFLoader2111111111111111111111111111111111";
pub(crate) const BPF_LOADER_UPGRADEABLE: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

pub(crate) fn b64_decode(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .unwrap_or_default()
}

/// A ready-to-load account: either raw data, or a program's ELF bytecode.
#[derive(Clone)]
pub(crate) enum Loaded {
    Data(Account),
    Program(Vec<u8>),
}

/// The loadables plus the set of addresses that actually exist on-chain.
pub(crate) type LoadedAccounts = (Vec<(Address, Loaded)>, HashMap<String, ()>);

/// A Solana runtime feature gate to flip for a replay: activate one that isn't
/// live on mainnet yet ("will my tx still work when this ships?"), or deactivate
/// an active one. Most execution-affecting SIMDs land on-chain as one of these.
#[derive(Clone, Debug)]
pub struct FeatureToggle {
    /// The feature gate's account address (its on-chain pubkey).
    pub id: Address,
    /// `true` = activate the gate, `false` = deactivate it.
    pub active: bool,
}

/// Build a fresh LiteSVM from already-fetched loadables (no network).
///
/// `features` optionally overrides the runtime feature set: we start from the
/// mainnet-active set (the replay baseline) and flip each requested gate, so a
/// transaction can be re-executed as if a feature were (in)active.
/// A fresh SVM holding `loaded`, with BPF register tracing on when `tracing`
/// is set (the profiler's world). Tracing costs memory per executed
/// instruction, so it is never on for ordinary replays.
fn svm_from_loaded_with(
    loaded: &[(Address, Loaded)],
    features: &[FeatureToggle],
    slot: u64,
    tracing: bool,
) -> LiteSVM {
    // Disable sigverify + blockhash check: we're replaying an already-signed
    // transaction, so its original blockhash won't be valid in a fresh SVM.
    #[cfg(feature = "profiler")]
    let base = LiteSVM::new_debuggable(tracing);
    #[cfg(not(feature = "profiler"))]
    let base = {
        debug_assert!(!tracing, "profiler feature is off");
        LiteSVM::new()
    };
    let mut svm = base
        .with_sigverify(false)
        .with_blockhash_check(false)
        // The debugger splits logs per step; LiteSVM's default 10 KB cap would
        // silently drop the tail of a long transaction's logs.
        .with_log_bytes_limit(None);
    if !features.is_empty() {
        // Start from mainnet's active gates and flip the requested ones. The
        // pubkey types differ across crates (Address vs solana_pubkey::Pubkey),
        // so bridge by their shared 32-byte representation.
        let mut fs = LiteSVM::mainnet_feature_set();
        for t in features {
            let pk = solana_pubkey::Pubkey::new_from_array(t.id.to_bytes());
            if t.active {
                fs.activate(&pk, slot);
            } else {
                fs.deactivate(&pk);
            }
        }
        // `with_builtins` is what rebuilds the program-runtime environment (CU
        // model, syscalls, feature-gated builtins) from the feature set — setting
        // the set alone doesn't, so execution would otherwise ignore the toggle.
        svm = svm
            .with_feature_set(fs)
            .with_builtins()
            .with_feature_accounts();
    }
    for (addr, l) in loaded {
        match l {
            // A load failure here (malformed account, unloadable ELF) surfaces
            // through the replay result itself — a library must not print.
            // Sysvar accounts (the Rent sysvar is always in the world, see
            // `with_rent_sysvar`) refresh LiteSVM's sysvar cache on insert, so
            // the runtime and programs read the cluster's parameters.
            Loaded::Data(a) => {
                let _ = svm.set_account(*addr, a.clone());
            }
            Loaded::Program(elf) => {
                let _ = svm.add_program(*addr, elf);
            }
        }
    }
    svm
}

/// Per-account load info for building a fidelity report: the address, what kind
/// of load it was, and a blake3 content hash of the bytes actually loaded.
pub(crate) struct LoadedInfo {
    pub address: String,
    pub is_program: bool,
    pub owner_is_system: bool,
    pub data_len: usize,
    pub hash: String,
}

/// The reconstructed world a transaction ran in, fetched once. Run any number of
/// scenarios against it with [`ReplayContext::run`] — each gets a pristine SVM.
#[derive(Clone)]
pub(crate) struct ReplayContext {
    /// The replayed transaction's signature (empty for pre-flight transactions).
    pub(crate) signature: String,
    pub(crate) tx: VersionedTransaction,
    pub(crate) loaded: Vec<(Address, Loaded)>,
    /// The slot the transaction landed in. The SVM clock is set to this so
    /// Address Lookup Table resolution — and slot-sensitive program logic like
    /// oracle staleness checks — sees the same world the transaction ran in.
    /// Without it, LiteSVM's default slot is below recent mainnet slots, and any
    /// ALT extended more recently fails with `InvalidAddressLookupTableIndex`.
    pub(crate) slot: Option<u64>,
    /// The chain's actual wall-clock time at that slot, from the RPC. Estimating
    /// it from genesis drifts by months (slots aren't reliably 400ms), and any
    /// program comparing against a real date would then be wrong.
    pub(crate) block_time: Option<i64>,
    /// Optional clock warp applied on top of `slot` (see [`TimeTravel`]).
    pub(crate) time_travel: TimeTravel,
    /// IDLs by owner program, used to resolve named-field assertions
    /// (`assert pool.reserveA`) against accounts the built-in decoders don't
    /// recognize. Populated by the library layer, which has RPC access.
    pub(crate) idls: HashMap<String, serde_json::Value>,
    /// Runtime feature gates to flip before replaying (empty = mainnet-as-is).
    pub(crate) feature_toggles: Vec<FeatureToggle>,
}

/// An account that the transaction changed, with raw before/after bytes. The
/// caller turns these into named field diffs using the account's layout.
pub(crate) struct RawAccountDiff {
    pub(crate) address: String,
    pub(crate) owner: String,
    pub(crate) lamports_before: u64,
    pub(crate) lamports_after: u64,
    pub(crate) data_before: Vec<u8>,
    pub(crate) data_after: Vec<u8>,
}

/// Move the SVM's clock forward (or to an absolute point) so time-gated program
/// logic can be tested without waiting: unstake after an epoch, claim after a
/// vesting cliff, withdraw after a cooldown, settle after an auction ends.
///
/// Relative fields add to the transaction's own clock; absolute fields replace it.
/// Slots and epochs are kept consistent with each other where possible
/// (Solana's ~432,000 slots per epoch, ~400ms per slot).
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimeTravel {
    /// Jump forward this many epochs (the common case for staking).
    #[serde(default)]
    pub epochs: Option<i64>,
    /// Jump forward this many slots.
    #[serde(default)]
    pub slots: Option<i64>,
    /// Jump forward this many seconds (vesting cliffs, cooldowns).
    #[serde(default)]
    pub seconds: Option<i64>,
    /// Set the unix timestamp outright.
    #[serde(default)]
    pub at_unix_timestamp: Option<i64>,
    /// Set the slot outright.
    #[serde(default)]
    pub at_slot: Option<u64>,
    /// Set the epoch outright.
    #[serde(default)]
    pub at_epoch: Option<u64>,
}

/// Slots per epoch on mainnet, used to keep slot/epoch/time coherent when warping.
const SLOTS_PER_EPOCH: i64 = 432_000;
/// Approximate seconds per slot.
const SECS_PER_SLOT: f64 = 0.4;

impl TimeTravel {
    /// True when no clock or balance rewind is requested.
    pub fn is_noop(&self) -> bool {
        self.epochs.is_none()
            && self.slots.is_none()
            && self.seconds.is_none()
            && self.at_unix_timestamp.is_none()
            && self.at_slot.is_none()
            && self.at_epoch.is_none()
    }

    /// Apply this warp to a clock, advancing the related fields together so a
    /// program that reads epoch *and* timestamp sees a consistent world.
    fn apply(&self, clock: &mut Clock) {
        // Saturating throughout: the warp values come from untrusted JSON, and an
        // extreme epoch/slot count must clamp, never overflow-panic (debug) or
        // wrap to a nonsense clock (release).
        let mut slot_delta: i64 = 0;
        if let Some(e) = self.epochs {
            slot_delta = slot_delta.saturating_add(e.saturating_mul(SLOTS_PER_EPOCH));
        }
        if let Some(s) = self.slots {
            slot_delta = slot_delta.saturating_add(s);
        }
        if slot_delta != 0 {
            clock.slot = clock.slot.saturating_add_signed(slot_delta);
            clock.epoch = clock
                .epoch
                .saturating_add_signed(slot_delta / SLOTS_PER_EPOCH);
            clock.unix_timestamp = clock
                .unix_timestamp
                .saturating_add((slot_delta as f64 * SECS_PER_SLOT) as i64);
        }
        if let Some(secs) = self.seconds {
            clock.unix_timestamp = clock.unix_timestamp.saturating_add(secs);
            // Keep slots roughly in step so slot-based checks move too.
            let by = (secs as f64 / SECS_PER_SLOT) as i64;
            clock.slot = clock.slot.saturating_add_signed(by);
            clock.epoch = clock.epoch.saturating_add_signed(by / SLOTS_PER_EPOCH);
        }
        // Absolute settings win over the relative jumps above.
        if let Some(t) = self.at_unix_timestamp {
            clock.unix_timestamp = t;
        }
        if let Some(s) = self.at_slot {
            clock.slot = s;
        }
        if let Some(e) = self.at_epoch {
            clock.epoch = e;
        }
        clock.leader_schedule_epoch = clock.epoch + 1;
        // The epoch's start can't be after "now" once we've moved.
        clock.epoch_start_timestamp = clock.epoch_start_timestamp.min(clock.unix_timestamp);
    }

    /// A human summary of where we travelled to, for the UI.
    pub(crate) fn describe(&self, clock: &Clock) -> String {
        format!(
            "slot {} · epoch {} · {}",
            clock.slot,
            clock.epoch,
            chrono_like(clock.unix_timestamp)
        )
    }
}

/// Format a unix timestamp as UTC without pulling in a date crate.
fn chrono_like(ts: i64) -> String {
    // days since epoch → civil date (Howard Hinnant's algorithm)
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        secs / 3600,
        (secs % 3600) / 60
    )
}

impl ReplayContext {
    #[cfg(feature = "profiler")]
    /// Warp the clock for subsequent runs (see [`TimeTravel`]).
    /// The ELF loaded for `program` in this replay's world (the on-chain
    /// bytes, or a replacement), if it is a program.
    pub(crate) fn program_elf(&self, program: &str) -> Option<&[u8]> {
        self.loaded.iter().find_map(|(addr, l)| match l {
            Loaded::Program(elf) if addr.to_string() == program => Some(elf.as_slice()),
            _ => None,
        })
    }

    pub(crate) fn set_time_travel(&mut self, tt: TimeTravel) {
        self.time_travel = tt;
    }

    /// Replace a loaded program's ELF bytecode (for A/B patch comparison).
    /// Returns the previous ELF, or `None` if `program_id` isn't loaded here as
    /// an executable program.
    pub(crate) fn replace_program(&mut self, program_id: &str, elf: Vec<u8>) -> Option<Vec<u8>> {
        for (addr, l) in self.loaded.iter_mut() {
            if addr.to_string() == program_id {
                if let Loaded::Program(existing) = l {
                    return Some(std::mem::replace(existing, elf));
                }
            }
        }
        None
    }

    /// Enumerate every loaded account with the facts a fidelity certificate
    /// needs — provenance and hashing are derived from this in the scope layer.
    pub(crate) fn loaded_info(&self) -> Vec<LoadedInfo> {
        self.loaded
            .iter()
            .map(|(addr, l)| match l {
                Loaded::Data(a) => LoadedInfo {
                    address: addr.to_string(),
                    is_program: false,
                    owner_is_system: a.owner == Address::default(),
                    data_len: a.data.len(),
                    hash: solana_blake3_hasher::hash(&a.data).to_string(),
                },
                Loaded::Program(elf) => LoadedInfo {
                    address: addr.to_string(),
                    is_program: true,
                    owner_is_system: false,
                    data_len: elf.len(),
                    hash: solana_blake3_hasher::hash(elf).to_string(),
                },
            })
            .collect()
    }

    /// Flip runtime feature gates for subsequent runs (see [`FeatureToggle`]).
    /// Every run rebuilds its SVM, so this takes effect on the next replay.
    pub(crate) fn set_feature_toggles(&mut self, toggles: Vec<FeatureToggle>) {
        self.feature_toggles = toggles;
    }

    /// Seed the clock with the transaction's real slot, and derive the epoch and
    /// wall-clock time from it. LiteSVM starts at epoch 0 / timestamp 0, which
    /// would make any date-based program logic nonsense, so we anchor to reality:
    /// Solana's genesis (2020-03-16) plus ~400ms per slot.
    fn base_clock(&self, clock: &mut Clock) {
        let Some(slot) = self.slot else { return };
        clock.slot = slot;
        clock.epoch = slot / SLOTS_PER_EPOCH as u64;
        // Prefer the real block time; fall back to a genesis estimate only if the
        // RPC didn't give us one (that estimate can be months off).
        const GENESIS_UNIX: i64 = 1_584_368_940; // 2020-03-16, mainnet genesis
        clock.unix_timestamp = self
            .block_time
            .unwrap_or_else(|| GENESIS_UNIX + (slot as f64 * SECS_PER_SLOT) as i64);
        clock.epoch_start_timestamp =
            clock.unix_timestamp - ((slot % SLOTS_PER_EPOCH as u64) as f64 * SECS_PER_SLOT) as i64;
        clock.leader_schedule_epoch = clock.epoch + 1;
    }

    /// A human description of the clock this context replays with, e.g.
    /// "slot 487,817,148 · epoch 1128 · 2026-09-01 04:12 UTC".
    pub(crate) fn describe_clock(&self) -> String {
        let svm = LiteSVM::new();
        let mut clock = svm.get_sysvar::<Clock>();
        self.base_clock(&mut clock);
        self.time_travel.apply(&mut clock);
        self.time_travel.describe(&clock)
    }

    /// [`Self::fresh_svm`], optionally with BPF register tracing enabled.
    pub(crate) fn fresh_svm_with(&self, tracing: bool) -> LiteSVM {
        let mut svm = svm_from_loaded_with(
            &self.loaded,
            &self.feature_toggles,
            self.slot.unwrap_or(0),
            tracing,
        );
        let mut clock = svm.get_sysvar::<Clock>();
        self.base_clock(&mut clock);
        if !self.time_travel.is_noop() {
            self.time_travel.apply(&mut clock);
        }
        svm.set_sysvar::<Clock>(&clock);
        svm
    }

    /// Replay and report **what changed** among the loaded accounts — each one's
    /// lamports and raw data, before and after. The caller decodes the bytes into
    /// named fields. Accounts *created* by the transaction aren't in the loaded
    /// set, so they don't appear here.
    pub(crate) fn run_with_diff(
        &self,
        mutations: &[Mutation],
    ) -> Result<(ReplayResult, Vec<RawAccountDiff>)> {
        let (result, svm) = self.run_full(mutations)?;
        let mut diffs = Vec::new();
        for (addr, l) in &self.loaded {
            let Loaded::Data(before) = l else { continue };
            let Some(after) = svm.get_account(addr) else {
                continue;
            };
            if after.lamports == before.lamports && after.data == before.data {
                continue; // untouched
            }
            diffs.push(RawAccountDiff {
                address: addr.to_string(),
                owner: after.owner.to_string(),
                lamports_before: before.lamports,
                lamports_after: after.lamports,
                data_before: before.data.clone(),
                data_after: after.data.clone(),
            });
        }
        Ok((result, diffs))
    }

    /// Replay with `mutations`, then read one account's raw post-execution state
    /// — the primitive historical reconstruction chains: inject an account's
    /// reconstructed bytes, replay the next write, read the account out again.
    pub(crate) fn run_and_read_account(
        &self,
        mutations: &[Mutation],
        address: &str,
    ) -> Result<(ReplayResult, Option<Account>)> {
        let (result, svm) = self.run_full(mutations)?;
        let acc = Address::from_str(address)
            .ok()
            .and_then(|a| svm.get_account(&a));
        Ok((result, acc))
    }

    /// The pre-transaction state of an account (for delta assertions).
    pub(crate) fn pre_account(&self, address: &str) -> Option<&Account> {
        let addr = Address::from_str(address).ok()?;
        self.loaded.iter().find_map(|(a, l)| match l {
            Loaded::Data(acc) if *a == addr => Some(acc),
            _ => None,
        })
    }

    /// Whether an address is part of the replay's world at all (a loaded data
    /// account or program). An assertion against an address that is *not* known
    /// is a typo, not an account that happens to be empty — the difference
    /// between a hard error and a check that silently reads zero and passes.
    pub(crate) fn is_known(&self, address: &str) -> bool {
        match Address::from_str(address) {
            Ok(addr) => self.loaded.iter().any(|(a, _)| *a == addr),
            Err(_) => false,
        }
    }

    /// Register a program's IDL for named-field assertion resolution.
    pub(crate) fn add_idl(&mut self, owner: String, idl: serde_json::Value) {
        self.idls.insert(owner, idl);
    }

    /// The registered IDLs, by program id.
    pub(crate) fn idl_map(&self) -> &HashMap<String, serde_json::Value> {
        &self.idls
    }

    /// Every program whose IDL could matter here: loaded programs (error-name
    /// resolution) plus each distinct data-account owner (named-field decode).
    pub(crate) fn interesting_programs(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        for (addr, l) in &self.loaded {
            match l {
                Loaded::Program(_) => {
                    seen.insert(addr.to_string());
                }
                Loaded::Data(acc) => {
                    seen.insert(acc.owner.to_string());
                }
            }
        }
        seen.into_iter().collect()
    }

    /// Add one feature toggle on top of those already set.
    pub(crate) fn push_feature_toggle(&mut self, t: FeatureToggle) {
        self.feature_toggles.push(t);
    }

    /// Decode an account's pre-state into named fields: built-in layouts (SPL
    /// token & friends) first, then the owner program's registered IDL. The
    /// pre-state layout is authoritative for both sides of a delta — an assert
    /// shouldn't silently change meaning because the tx rewrote the account.
    pub(crate) fn decode_pre(
        &self,
        address: &str,
    ) -> Result<(crate::decode::DecodedAccount, &Account)> {
        let pre = self
            .pre_account(address)
            .ok_or_else(|| Error::AccountNotFound(format!("{address} (no pre-state)")))?;
        let owner = pre.owner.to_string();
        crate::decode::decode_bytes(&owner, &pre.data)
            .or_else(|| {
                self.idls
                    .get(&owner)
                    .and_then(|i| crate::idl::decode_with_idl(i, &pre.data))
            })
            .map(|dec| (dec, pre))
            .ok_or_else(|| Error::UndecodableAccount {
                address: address.to_string(),
                owner,
            })
    }
}

/// The transaction's *pre-execution* state, recovered for free from its own
/// metadata (`meta.preTokenBalances`) — no archival RPC.
///
/// Live account state drifts after a transaction lands, which is why replaying a
/// swap against current state usually slips. But `getTransaction` reports the
/// exact SPL token amount each account held *just before* the transaction ran, so
/// we restore those — making constant-product pool reserves faithful again.
///
/// (Only token accounts that existed pre-transaction appear in `preTokenBalances`,
/// so restoring them is safe — it never resurrects an account the tx creates.)
pub(crate) const SPL_TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// The Rent sysvar. Always loaded with the world (see [`with_rent_sysvar`]) so
/// the replay enforces the *cluster's* rent parameters, not LiteSVM's defaults.
pub(crate) const SYSVAR_RENT: &str = "SysvarRent111111111111111111111111111111111";

/// Add the Rent sysvar to a key list unless the transaction already references
/// it. Mainnet lowered rent (6,333 lamports per byte-year at a 1.0 exemption
/// threshold, versus the 3,480 × 2.0 default LiteSVM starts from), so an account
/// created since then holds *less* than the default minimum: every
/// `ConstraintRentExempt` check and every system-program rent check on such an
/// account would fail in a replay of a transaction that succeeded on-chain.
pub(crate) fn with_rent_sysvar(keys: &mut Vec<String>) {
    if !keys.iter().any(|k| k == SYSVAR_RENT) {
        keys.push(SYSVAR_RENT.to_string());
    }
}

/// The rent parameters encoded in a Rent sysvar account (`u64` lamports per
/// byte-year, `f64` exemption threshold, `u8` burn percent, little-endian).
/// Test-only: production code loads the sysvar bytes verbatim and lets LiteSVM
/// decode them.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RentParams {
    pub(crate) lamports_per_byte_year: u64,
    pub(crate) exemption_threshold: f64,
    pub(crate) burn_percent: u8,
}

#[cfg(test)]
impl RentParams {
    /// The rent-exempt minimum for an account with `data_len` bytes of data.
    pub(crate) fn minimum_balance(&self, data_len: usize) -> u64 {
        const ACCOUNT_STORAGE_OVERHEAD: u64 = 128;
        (((ACCOUNT_STORAGE_OVERHEAD + data_len as u64) * self.lamports_per_byte_year) as f64
            * self.exemption_threshold) as u64
    }
}

#[cfg(test)]
pub(crate) fn parse_rent(data: &[u8]) -> Option<RentParams> {
    if data.len() < 17 {
        return None;
    }
    Some(RentParams {
        lamports_per_byte_year: u64::from_le_bytes(data[0..8].try_into().ok()?),
        exemption_threshold: f64::from_le_bytes(data[8..16].try_into().ok()?),
        burn_percent: data[16],
    })
}

/// Pre-transaction token-account details, enough to rebuild a closed one.
struct TokenInfo {
    mint: String,
    owner: String, // the token authority (SPL "owner" field), not the program
    amount: u64,
}

#[derive(Default)]
pub(crate) struct PreState {
    /// account address -> raw SPL token amount before the transaction.
    pub(crate) token_amounts: HashMap<String, u64>,
    /// account address -> its lamports before the transaction.
    pub(crate) lamports: HashMap<String, u64>,
    /// token account address -> details, to reconstruct it if it's since closed.
    token_info: HashMap<String, TokenInfo>,
}

impl PreState {
    pub(crate) fn from_meta(tx: &serde_json::Value, account_keys: &[String]) -> PreState {
        let mut ps = PreState::default();

        // Pre-transaction lamports, parallel to the resolved account list.
        if let Some(pre) = tx["meta"]["preBalances"].as_array() {
            for (i, v) in pre.iter().enumerate() {
                if let (Some(addr), Some(l)) = (account_keys.get(i), v.as_u64()) {
                    ps.lamports.insert(addr.clone(), l);
                }
            }
        }
        // Pre-transaction token balances (amount + enough to rebuild the account).
        if let Some(pre) = tx["meta"]["preTokenBalances"].as_array() {
            for e in pre {
                let idx = e["accountIndex"].as_u64().unwrap_or(u64::MAX) as usize;
                let amt = e["uiTokenAmount"]["amount"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok());
                let (Some(addr), Some(amt)) = (account_keys.get(idx), amt) else {
                    continue;
                };
                ps.token_amounts.insert(addr.clone(), amt);
                if let (Some(mint), Some(owner)) = (e["mint"].as_str(), e["owner"].as_str()) {
                    ps.token_info.insert(
                        addr.clone(),
                        TokenInfo {
                            mint: mint.to_string(),
                            owner: owner.to_string(),
                            amount: amt,
                        },
                    );
                }
            }
        }
        ps
    }

    /// Reconstruct an account that existed before the transaction but has since
    /// been closed (so `getMultipleAccounts` returns null). Returns `None` for
    /// accounts that never existed (the transaction creates those itself).
    pub(crate) fn reconstruct(&self, address: &str) -> Option<Account> {
        // A closed token account: rebuild the full SPL layout from meta.
        if let Some(info) = self.token_info.get(address) {
            let mut data = vec![0u8; 165];
            let mint = Address::from_str(&info.mint).ok()?;
            let owner = Address::from_str(&info.owner).ok()?;
            data[0..32].copy_from_slice(mint.as_array());
            data[32..64].copy_from_slice(owner.as_array());
            data[64..72].copy_from_slice(&info.amount.to_le_bytes());
            data[108] = 1; // AccountState::Initialized
            return Some(Account {
                lamports: self.lamports.get(address).copied().unwrap_or(2_039_280),
                data,
                owner: Address::from_str(SPL_TOKEN_PROGRAM).ok()?,
                executable: false,
                rent_epoch: 0,
            });
        }
        // A closed system account (e.g. a fee payer that has since drained): a
        // system-owned account with its pre-transaction lamports is faithful.
        match self.lamports.get(address) {
            Some(&l) if l > 0 => Some(Account {
                lamports: l,
                data: vec![],
                owner: Address::default(), // system program (all-zero id)
                executable: false,
                rent_epoch: 0,
            }),
            _ => None,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.token_amounts.is_empty() && self.lamports.is_empty()
    }
}

fn b64_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

impl ReplayContext {
    /// Freeze this context into a portable, self-contained [`Fixture`] — the tx
    /// wire bytes plus every account/program, so it can replay offline forever.
    ///
    /// The captured slot and block time come from the context itself (`self.slot`
    /// / `self.block_time`) — the exact clock the live replay ran with — so a
    /// reloaded fixture reproduces the live outcome instead of drifting to the
    /// transaction's original slot.
    pub(crate) fn to_fixture(&self) -> Result<Fixture> {
        let tx_bytes = bincode::serialize(&self.tx)
            .map_err(|e| Error::Fixture(format!("serialize transaction: {e}")))?;
        let entries = self
            .loaded
            .iter()
            .map(|(addr, l)| match l {
                Loaded::Data(a) => FixtureEntry::Data {
                    address: addr.to_string(),
                    owner: a.owner.to_string(),
                    lamports: a.lamports,
                    data_b64: b64_encode(&a.data),
                },
                Loaded::Program(elf) => FixtureEntry::Program {
                    address: addr.to_string(),
                    elf_b64: b64_encode(elf),
                },
            })
            .collect();
        Ok(Fixture {
            version: crate::fixture::FIXTURE_VERSION,
            signature: self.signature.clone(),
            captured_slot: self.slot,
            captured_block_time: self.block_time,
            tx_b64: b64_encode(&tx_bytes),
            entries,
            idls: self
                .idls
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            recorded: None, // the Replay layer fills this in — it knows the outcome
        })
    }

    /// Rebuild a replay context from a frozen fixture — **no network**. This is
    /// what makes fixture-backed suites deterministic and CI-safe.
    pub(crate) fn from_fixture(fx: &Fixture) -> Result<ReplayContext> {
        let tx_bytes = base64::engine::general_purpose::STANDARD
            .decode(&fx.tx_b64)
            .map_err(|e| Error::Fixture(format!("bad tx base64: {e}")))?;
        let tx: VersionedTransaction = bincode::deserialize(&tx_bytes)
            .map_err(|e| Error::Fixture(format!("deserialize tx: {e}")))?;

        let mut loaded = Vec::with_capacity(fx.entries.len());
        for e in &fx.entries {
            match e {
                FixtureEntry::Data {
                    address,
                    owner,
                    lamports,
                    data_b64,
                } => {
                    let addr = Address::from_str(address)
                        .map_err(|_| Error::Fixture(format!("bad address {address}")))?;
                    let owner_addr = Address::from_str(owner)
                        .map_err(|_| Error::Fixture(format!("bad owner {owner}")))?;
                    let data = base64::engine::general_purpose::STANDARD
                        .decode(data_b64)
                        .map_err(|e| {
                            Error::Fixture(format!("bad data base64 for {address}: {e}"))
                        })?;
                    loaded.push((
                        addr,
                        Loaded::Data(Account {
                            lamports: *lamports,
                            data,
                            owner: owner_addr,
                            executable: false,
                            rent_epoch: 0,
                        }),
                    ));
                }
                FixtureEntry::Program { address, elf_b64 } => {
                    let addr = Address::from_str(address)
                        .map_err(|_| Error::Fixture(format!("bad address {address}")))?;
                    let elf = base64::engine::general_purpose::STANDARD
                        .decode(elf_b64)
                        .map_err(|e| {
                            Error::Fixture(format!("bad elf base64 for {address}: {e}"))
                        })?;
                    loaded.push((addr, Loaded::Program(elf)));
                }
            }
        }
        Ok(ReplayContext {
            signature: fx.signature.clone(),
            tx,
            loaded,
            slot: fx.captured_slot,
            block_time: fx.captured_block_time,
            time_travel: TimeTravel::default(),
            // v2 fixtures carry their IDLs, so named-field asserts, error
            // names, and decoded diffs resolve offline too.
            idls: fx
                .idls
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            feature_toggles: Vec::new(),
        })
    }
}

/// Encode `value` as the little-endian bytes of the field's declared type,
/// rejecting anything that doesn't fit exactly. Supports the same fixed-width
/// integer/bool set the named-field *asserts* can read, so the two sides of the
/// DSL stay symmetric.
fn encode_field_value(f: &crate::decode::Field, value: i128) -> Result<Vec<u8>> {
    let out_of_range = || Error::FieldValueOutOfRange {
        field: f.name.clone(),
        ty: f.ty.clone(),
        value,
    };
    match (f.ty.as_str(), f.size) {
        ("bool", _) => match value {
            0 | 1 => Ok(vec![value as u8]),
            _ => Err(out_of_range()),
        },
        ("u8" | "u16" | "u32" | "u64" | "u128", n @ 1..=16) => {
            crate::idl_encode::int_bytes(value, n, false).ok_or_else(out_of_range)
        }
        ("i8" | "i16" | "i32" | "i64" | "i128", n @ 1..=16) => {
            crate::idl_encode::int_bytes(value, n, true).ok_or_else(out_of_range)
        }
        (ty, _) => Err(Error::NonNumericField {
            field: f.name.clone(),
            ty: ty.to_string(),
        }),
    }
}

/// Apply one mutation to the SVM. Callers validate first (see
/// [`ReplayContext::validate_mutations`]), so a failure here is a hard error,
/// never folded into a "failed replay" — a typo'd address must not satisfy an
/// `expect: revert` scenario.
fn apply_mutation(svm: &mut LiteSVM, m: &Mutation) -> Result<()> {
    // Instruction mutations rewrite the transaction, not an account; see
    // `ReplayContext::tx_with_mutations`.
    if m.address().is_empty() {
        return Ok(());
    }
    let address = m.address();
    let addr =
        Address::from_str(address).map_err(|_| Error::InvalidAddress(address.to_string()))?;
    let mut account = svm
        .get_account(&addr)
        .ok_or_else(|| Error::MutationTargetMissing(address.to_string()))?;

    match m {
        Mutation::Lamports { value, .. } => account.lamports = *value,
        Mutation::Data { bytes, .. } => account.data = bytes.clone(),
        Mutation::DataPatch { offset, bytes, .. } => {
            let end = offset
                .checked_add(bytes.len())
                .filter(|&e| e <= account.data.len())
                .ok_or_else(|| Error::PatchOutOfRange {
                    address: address.to_string(),
                    offset: *offset,
                    len: bytes.len(),
                    size: account.data.len(),
                })?;
            account.data[*offset..end].copy_from_slice(bytes);
        }
        // Named-field mutations are resolved into concrete patches by the
        // context (which holds the layouts/IDLs) before application; reaching
        // here unresolved is a caller bug, reported rather than guessed at.
        Mutation::Field { field, .. } => {
            return Err(Error::InvalidSpec(format!(
                "field mutation \"{field}\" was not resolved before application"
            )));
        }
        Mutation::Owner { owner, .. } => {
            account.owner =
                Address::from_str(owner).map_err(|_| Error::InvalidAddress(owner.to_string()))?;
        }
        // Instruction mutations were returned above.
        Mutation::IxArg { .. }
        | Mutation::IxData { .. }
        | Mutation::SkipIx { .. }
        | Mutation::IxDataReplace { .. }
        | Mutation::MoveIx { .. } => return Ok(()),
    }
    svm.set_account(addr, account).map_err(|e| {
        Error::MalformedRpcResponse(format!("set_account failed for {address}: {e:?}"))
    })
}

/// Convert a raw transaction result into a `ReplayResult`.
///
/// This does NOT print — it just extracts the data. The caller (CLI or a UI)
/// decides how to display it.
fn to_replay_result(result: TransactionResult) -> ReplayResult {
    match result {
        Ok(meta) => ReplayResult {
            success: true,
            logs: meta.logs,
            compute_units: meta.compute_units_consumed,
            ..Default::default()
        },
        Err(failed) => ReplayResult {
            success: false,
            error: Some(format!("{:?}", failed.err)),
            logs: failed.meta.logs,
            compute_units: failed.meta.compute_units_consumed,
            ..Default::default()
        },
    }
}

// ---------------------------------------------------------------------------
// Scenario testing: assert expected outcomes across a suite of what-ifs.
// ---------------------------------------------------------------------------

impl ReplayContext {
    /// Resolve a named-field mutation into a concrete byte patch using the
    /// account's decoded layout — the same resolution (exact name or
    /// unambiguous final dot-segment) the Check DSL's named-field asserts use.
    /// Offsets come from the *pre-transaction* layout; a preceding `Data`
    /// mutation that replaces the account with a different layout is on the
    /// caller.
    fn resolve_mutation(&self, m: &Mutation) -> Result<Mutation> {
        if let Mutation::IxArg { index, arg, value } = m {
            let ixs = self.tx.message.instructions();
            let ix = ixs.get(*index).ok_or_else(|| {
                Error::InvalidSpec(format!(
                    "instruction index {index} out of range ({} instructions)",
                    ixs.len()
                ))
            })?;
            let keys = self.tx.message.static_account_keys();
            let program = keys
                .get(ix.program_id_index as usize)
                .map(|p| p.to_string())
                .unwrap_or_default();
            let idl = self.idls.get(&program).ok_or_else(|| {
                Error::InvalidSpec(format!(
                    "instruction {index}: program {program} has no IDL, so arguments cannot be re-encoded"
                ))
            })?;
            let def = crate::idl::find_ix(idl, &ix.data).ok_or_else(|| {
                Error::InvalidSpec(format!(
                    "instruction {index}: data does not match any IDL instruction"
                ))
            })?;
            let (offset, size, label) = crate::idl::ix_arg_span(&def, arg).ok_or_else(|| {
                Error::InvalidSpec(format!(
                    "instruction {index}: argument \"{arg}\" is not a fixed-size scalar at a known offset"
                ))
            })?;
            let bytes = crate::idl::encode_fixed(&label, size, value).ok_or_else(|| {
                Error::InvalidSpec(format!(
                    "instruction {index}: value {value} does not fit argument \"{arg}\" ({label})"
                ))
            })?;
            return Ok(Mutation::IxData {
                index: *index,
                offset,
                bytes,
            });
        }
        let Mutation::Field {
            address,
            field,
            value,
        } = m
        else {
            return Ok(m.clone());
        };
        let (decoded, _pre) = self.decode_pre(address)?;
        let f = find_field(&decoded, field)?;
        let bytes = encode_field_value(f, *value)?;
        Ok(Mutation::DataPatch {
            address: address.clone(),
            offset: f.offset,
            bytes,
        })
    }

    /// The transaction with any instruction-data mutations applied (already
    /// resolved to [`Mutation::IxData`]). Account mutations are ignored here.
    fn tx_with_mutations(&self, resolved: &[Mutation]) -> Result<VersionedTransaction> {
        use solana_message::VersionedMessage;
        let mut tx = self.tx.clone();
        // Data edits (offset patches and whole replacements) apply in the
        // order they were listed, so a patch after a replace lands on the new
        // bytes and a replace after a patch overrides it — never silently the
        // other way round.
        for m in resolved {
            match m {
                Mutation::IxData {
                    index,
                    offset,
                    bytes,
                } => {
                    let ixs: &mut Vec<_> = match &mut tx.message {
                        VersionedMessage::Legacy(m) => &mut m.instructions,
                        VersionedMessage::V0(m) => &mut m.instructions,
                        VersionedMessage::V1(m) => &mut m.instructions,
                    };
                    let ix = ixs.get_mut(*index).ok_or_else(|| {
                        Error::InvalidSpec(format!("instruction index {index} out of range"))
                    })?;
                    let end = offset
                        .checked_add(bytes.len())
                        .filter(|&e| e <= ix.data.len())
                        .ok_or_else(|| {
                            Error::InvalidSpec(format!(
                                "instruction {index}: patch at {offset}+{} exceeds data length {}",
                                bytes.len(),
                                ix.data.len()
                            ))
                        })?;
                    ix.data[*offset..end].copy_from_slice(bytes);
                }
                Mutation::IxDataReplace { index, bytes } => {
                    let ixs: &mut Vec<_> = match &mut tx.message {
                        VersionedMessage::Legacy(m) => &mut m.instructions,
                        VersionedMessage::V0(m) => &mut m.instructions,
                        VersionedMessage::V1(m) => &mut m.instructions,
                    };
                    let ix = ixs.get_mut(*index).ok_or_else(|| {
                        Error::InvalidSpec(format!("instruction index {index} out of range"))
                    })?;
                    ix.data = bytes.clone();
                }
                _ => {}
            }
        }
        // Skips next, so every index above still meant the original position.
        let skipped: std::collections::BTreeSet<usize> = resolved
            .iter()
            .filter_map(|m| match m {
                Mutation::SkipIx { index } => Some(*index),
                _ => None,
            })
            .collect();
        if !skipped.is_empty() {
            let ixs: &mut Vec<_> = match &mut tx.message {
                VersionedMessage::Legacy(m) => &mut m.instructions,
                VersionedMessage::V0(m) => &mut m.instructions,
                VersionedMessage::V1(m) => &mut m.instructions,
            };
            if let Some(&bad) = skipped.iter().find(|&&i| i >= ixs.len()) {
                return Err(Error::InvalidSpec(format!(
                    "instruction index {bad} out of range"
                )));
            }
            if skipped.len() >= ixs.len() {
                return Err(Error::InvalidSpec("cannot skip every instruction".into()));
            }
            let mut i = 0;
            ixs.retain(|_| {
                let keep = !skipped.contains(&i);
                i += 1;
                keep
            });
        }
        // Moves last, in the post-skip numbering.
        for m in resolved {
            if let Mutation::MoveIx { from, to } = m {
                let ixs: &mut Vec<_> = match &mut tx.message {
                    VersionedMessage::Legacy(m) => &mut m.instructions,
                    VersionedMessage::V0(m) => &mut m.instructions,
                    VersionedMessage::V1(m) => &mut m.instructions,
                };
                if *from >= ixs.len() || *to >= ixs.len() {
                    return Err(Error::InvalidSpec(format!(
                        "move {from} → {to}: out of range for {} instructions",
                        ixs.len()
                    )));
                }
                let ix = ixs.remove(*from);
                ixs.insert(*to, ix);
            }
        }
        Ok(tx)
    }

    /// The transaction with `mutations` applied to its instruction data, for
    /// callers that drive the SVM themselves (the profiler).
    pub(crate) fn tx_for(&self, mutations: &[Mutation]) -> Result<VersionedTransaction> {
        let resolved: Vec<Mutation> = mutations
            .iter()
            .map(|m| self.resolve_mutation(m))
            .collect::<Result<_>>()?;
        self.tx_with_mutations(&resolved)
    }

    /// Run mutations and keep the post-replay SVM so state can be inspected.
    /// A mutation that can't be applied is a hard `Err`, never a failed replay.
    fn run_full(&self, mutations: &[Mutation]) -> Result<(ReplayResult, LiteSVM)> {
        let (result, svm, _) = self.run_full_with_pre(mutations)?;
        Ok((result, svm))
    }

    /// [`run_full`] that also returns the world the transaction actually
    /// started from — the loaded accounts with every mutation applied. Delta
    /// and "unchanged" assertions must be measured from this, not from the
    /// unmutated load, or a scenario that edits a balance before the run
    /// reports the edit as the transaction's effect (and a loss check passes
    /// on a transaction that lost funds).
    fn run_full_with_pre(
        &self,
        mutations: &[Mutation],
    ) -> Result<(ReplayResult, LiteSVM, HashMap<Address, Account>)> {
        let Prepared { mut svm, tx, .. } = self.prepare(mutations, false)?;
        let pre = self.loaded_state_of(&svm);
        let result = to_replay_result(svm.send_transaction(tx));
        Ok((result, svm, pre))
    }

    /// A fresh SVM with every mutation applied, and the transaction as
    /// mutated — the one setup every run, trace and profile starts from.
    pub(crate) fn prepare(&self, mutations: &[Mutation], tracing: bool) -> Result<Prepared> {
        let mut svm = self.fresh_svm_with(tracing);
        let resolved: Vec<Mutation> = mutations
            .iter()
            .map(|m| self.resolve_mutation(m))
            .collect::<Result<_>>()?;
        for m in &resolved {
            apply_mutation(&mut svm, m)?;
        }
        let tx = self.tx_with_mutations(&resolved)?;
        Ok(Prepared { svm, tx, resolved })
    }

    /// Every loaded data account as `svm` currently holds it.
    pub(crate) fn loaded_state_of(&self, svm: &LiteSVM) -> HashMap<Address, Account> {
        self.loaded
            .iter()
            .filter_map(|(a, l)| match l {
                Loaded::Data(_) => svm.get_account(a).map(|acc| (*a, acc)),
                _ => None,
            })
            .collect()
    }

    /// Check every mutation against the loaded state without running anything:
    /// the address parses, the target account is loaded, and a patch stays in
    /// bounds. Lets a suite fail fast — before any scenario executes.
    pub(crate) fn validate_mutations(&self, mutations: &[Mutation]) -> Result<()> {
        for m in mutations {
            if matches!(
                m,
                Mutation::IxArg { .. }
                    | Mutation::IxData { .. }
                    | Mutation::SkipIx { .. }
                    | Mutation::IxDataReplace { .. }
                    | Mutation::MoveIx { .. }
            ) {
                // Resolving is the validation: index, IDL, offset and fit.
                let r = self.resolve_mutation(m)?;
                self.tx_with_mutations(&[r])?;
                continue;
            }
            let address = m.address();
            let addr = Address::from_str(address)
                .map_err(|_| Error::InvalidAddress(address.to_string()))?;
            let data_len = self
                .loaded
                .iter()
                .find_map(|(a, l)| match l {
                    Loaded::Data(acc) if *a == addr => Some(acc.data.len()),
                    Loaded::Program(_) if *a == addr => Some(0),
                    _ => None,
                })
                .ok_or_else(|| Error::MutationTargetMissing(address.to_string()))?;
            // A named-field mutation validates by fully resolving: the account
            // decodes, the field exists unambiguously, and the value fits.
            let m = self.resolve_mutation(m)?;
            if let Mutation::DataPatch { offset, bytes, .. } = &m {
                if offset
                    .checked_add(bytes.len())
                    .filter(|&e| e <= data_len)
                    .is_none()
                {
                    return Err(Error::PatchOutOfRange {
                        address: address.to_string(),
                        offset: *offset,
                        len: bytes.len(),
                        size: data_len,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Run a suite of scenarios against one pre-fetched context and report pass/fail.
///
/// Every scenario's mutations are validated **before anything runs** — a typo'd
/// mutation address fails the whole suite with a hard error instead of showing
/// up as a revert that an `expect: revert` scenario would silently accept.
///
/// A scenario with no outcome check implicitly asserts success.
pub(crate) fn run_suite(
    ctx: &ReplayContext,
    recorded: Option<&crate::fixture::OnchainRecord>,
    scenarios: &[crate::check::Scenario],
) -> Result<Vec<ScenarioOutcome>> {
    use crate::check::CheckKind;

    for s in scenarios {
        ctx.validate_mutations(&s.mutations)?;
    }
    scenarios
        .iter()
        .map(|s| {
            let (actual, svm, pre) = ctx.run_full_with_pre(&s.mutations)?;

            // Transaction-level outcome: every Outcome check must hold; a
            // scenario that declares none implicitly asserts success — unless it
            // carries a `matches_onchain` check, which is itself the outcome
            // assertion (and would otherwise be contradicted for a transaction
            // whose recorded outcome is a revert).
            let outcomes: Vec<&Expect> = s
                .checks
                .iter()
                .filter_map(|c| match &c.0 {
                    CheckKind::Outcome(e) => Some(e),
                    _ => None,
                })
                .collect();
            let has_matches_onchain = s
                .checks
                .iter()
                .any(|c| matches!(&c.0, CheckKind::MatchesOnchain));
            let implicit = [if has_matches_onchain {
                Expect::Any
            } else {
                Expect::Success
            }];
            let effective: &[&Expect] = if outcomes.is_empty() {
                &[&implicit[0]]
            } else {
                &outcomes
            };
            let outcome_pass = effective.iter().all(|e| e.matches(&actual));
            let expect = effective
                .iter()
                .map(|e| e.describe())
                .collect::<Vec<_>>()
                .join(" & ");

            // Result- and state-level checks all report as assert outcomes.
            let mut asserts: Vec<AssertOutcome> = Vec::new();
            for c in &s.checks {
                match &c.0 {
                    CheckKind::Outcome(_) => {}
                    CheckKind::LogContains(t) => asserts.push(AssertOutcome {
                        description: format!("logs contain \"{t}\""),
                        pass: actual.error.as_deref().is_some_and(|e| e.contains(t))
                            || actual.logs.iter().any(|l| l.contains(t)),
                    }),
                    CheckKind::ComputeUnits(c) => asserts.push(AssertOutcome {
                        description: format!("compute units {} {}", c.op.symbol(), c.value),
                        pass: c.op.test_i128(actual.compute_units as i128, c.value),
                    }),
                    CheckKind::MatchesOnchain => asserts.push(match recorded {
                        Some(r) => AssertOutcome {
                            description: format!(
                                "matches the on-chain outcome ({})",
                                if r.success { "success" } else { "failure" }
                            ),
                            pass: actual.success == r.success,
                        },
                        None => AssertOutcome {
                            description:
                                "matches the on-chain outcome — no recorded outcome available \
                                 (pre-flight tx, or a v1 fixture; re-capture to record it)"
                                    .into(),
                            pass: false,
                        },
                    }),
                    CheckKind::Account(list) => {
                        for a in list {
                            asserts.push(match a.eval(&svm, ctx, &pre) {
                                Ok(pass) => AssertOutcome {
                                    description: a.describe(),
                                    pass,
                                },
                                // A failed eval (missing account, unknown field,
                                // bad offset) is a failed assertion — and the
                                // description says why.
                                Err(e) => AssertOutcome {
                                    description: format!("{} — {e}", a.describe()),
                                    pass: false,
                                },
                            });
                        }
                    }
                }
            }

            let pass = outcome_pass && asserts.iter().all(|a| a.pass);
            Ok(ScenarioOutcome {
                name: s.name.clone(),
                expect,
                pass,
                actual,
                asserts,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Debugger primitives: prefix replays with post-state capture.
// ---------------------------------------------------------------------------

/// One prefix run for the step debugger.
pub(crate) struct RawStepRun {
    /// Top-level instruction indexes that were executed, in message order.
    pub(crate) keep: Vec<usize>,
    /// Account state after each inner instruction (CPI) of this top-level
    /// instruction, in completion order — from the runtime observer, single
    /// run only. Empty when unavailable (prefix mode).
    pub(crate) inner_posts: Vec<InnerSnapshot>,
    /// The runtime outcome (logs, inner instructions, CU, error).
    pub(crate) result: litesvm::types::TransactionResult,
    /// Every account after the run, when it succeeded. `None` on failure: a
    /// failed prefix commits nothing.
    pub(crate) post: Option<Vec<(Address, Account)>>,
}

const COMPUTE_BUDGET_ID: &str = "ComputeBudget111111111111111111111111111111";

impl ReplayContext {
    /// The transaction's signature (empty for pre-flight transactions).
    pub(crate) fn signature(&self) -> &str {
        &self.signature
    }

    /// The transaction being replayed.
    pub(crate) fn transaction(&self) -> &VersionedTransaction {
        &self.tx
    }

    /// The message's full account list in runtime order: static keys, then
    /// ALT-writable, then ALT-readonly — resolved offline from the loaded
    /// lookup-table accounts (address list at offset 56, 32 bytes each).
    pub(crate) fn message_account_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self
            .tx
            .message
            .static_account_keys()
            .iter()
            .map(|a| a.to_string())
            .collect();
        let Some(lookups) = self.tx.message.address_table_lookups() else {
            return keys;
        };
        let (mut writable, mut readonly) = (Vec::new(), Vec::new());
        for l in lookups {
            let data = self.loaded.iter().find_map(|(a, ld)| match ld {
                Loaded::Data(acc) if *a == l.account_key => Some(&acc.data),
                _ => None,
            });
            let Some(data) = data else { continue };
            let read = |idx: u8| -> Option<String> {
                let off = 56 + idx as usize * 32;
                let bytes: [u8; 32] = data.get(off..off + 32)?.try_into().ok()?;
                Some(Address::from(bytes).to_string())
            };
            writable.extend(l.writable_indexes.iter().filter_map(|&i| read(i)));
            readonly.extend(l.readonly_indexes.iter().filter_map(|&i| read(i)));
        }
        keys.extend(writable);
        keys.extend(readonly);
        keys
    }

    /// The pre-transaction state of an account, owned. `None` if it did not
    /// exist (or is a program).
    pub(crate) fn pre_account_owned(&self, address: &str) -> Option<Account> {
        self.pre_account(address).cloned()
    }

    /// The transaction with only the top-level instructions at `keep`, in
    /// order. Signatures and header are untouched: sigverify is off, so the
    /// runtime only checks that the signature count matches the header.
    fn tx_with_instructions(
        &self,
        base: &VersionedTransaction,
        keep: &[usize],
    ) -> VersionedTransaction {
        use solana_message::VersionedMessage;
        let all = base.message.instructions();
        let subset: Vec<_> = keep.iter().filter_map(|&i| all.get(i).cloned()).collect();
        let mut tx = base.clone();
        match &mut tx.message {
            VersionedMessage::Legacy(m) => m.instructions = subset,
            VersionedMessage::V0(m) => m.instructions = subset,
            VersionedMessage::V1(m) => m.instructions = subset,
        }
        tx
    }

    /// Replay every instruction prefix `0..=k` against one fresh SVM (read-only
    /// simulations, so the state never advances) and capture the post-state of
    /// each. ComputeBudget instructions are kept in every prefix because the
    /// runtime derives the transaction's limits from the whole message.
    pub(crate) fn trace_raw(&self, mutations: &[Mutation]) -> Result<Vec<RawStepRun>> {
        self.trace_raw_prefixes(mutations)
    }

    fn trace_raw_prefixes(&self, mutations: &[Mutation]) -> Result<Vec<RawStepRun>> {
        let Prepared { svm, tx: base, .. } = self.prepare(mutations, false)?;
        let keys = base.message.static_account_keys();
        let n = base.message.instructions().len();
        let is_cb: Vec<bool> = base
            .message
            .instructions()
            .iter()
            .map(|ix| {
                keys.get(ix.program_id_index as usize)
                    .map(|p| p.to_string() == COMPUTE_BUDGET_ID)
                    .unwrap_or(false)
            })
            .collect();
        let mut runs = Vec::with_capacity(n);
        for k in 0..n {
            let keep: Vec<usize> = (0..n).filter(|&i| i <= k || is_cb[i]).collect();
            let tx = self.tx_with_instructions(&base, &keep);
            let (result, post) = match svm.simulate_transaction(tx) {
                Ok(info) => (
                    Ok(info.meta),
                    Some(
                        info.post_accounts
                            .into_iter()
                            .map(|(a, acc)| (a, Account::from(acc)))
                            .collect(),
                    ),
                ),
                Err(failed) => (Err(failed), None),
            };
            runs.push(RawStepRun {
                keep,
                inner_posts: Vec::new(),
                result,
                post,
            });
        }
        Ok(runs)
    }
}

/// The `ReplayResult` view of a raw runtime result (shared with the debugger).
/// Account state after one inner instruction (a CPI), from the runtime observer.
#[derive(Clone)]
pub(crate) struct InnerSnapshot {
    /// Stack height of the instruction (2 = called by a top-level instruction).
    pub(crate) height: u8,
    /// The program that ran.
    pub(crate) program: String,
    /// Whether it succeeded.
    #[allow(dead_code)]
    pub(crate) ok: bool,
    /// Every transaction account as it stood when this instruction finished.
    pub(crate) accounts: Vec<(Address, Account)>,
    /// Every transaction account as it stood when this instruction started —
    /// the baseline its own changes are measured from.
    pub(crate) entry: Vec<(Address, Account)>,
}

/// A run's starting point: the SVM with mutations applied, the mutated
/// transaction, and the resolved mutations that produced both.
pub(crate) struct Prepared {
    pub(crate) svm: LiteSVM,
    pub(crate) tx: VersionedTransaction,
    #[allow(dead_code)]
    pub(crate) resolved: Vec<Mutation>,
}

/// The top-level instruction a failed transaction result names, if any.
pub(crate) fn failed_instruction_index(
    result: &litesvm::types::TransactionResult,
) -> Option<usize> {
    match result {
        Err(f) => match &f.err {
            solana_transaction_error::TransactionError::InstructionError(i, _) => Some(*i as usize),
            _ => None,
        },
        Ok(_) => None,
    }
}

pub(crate) fn replay_result_of(result: &litesvm::types::TransactionResult) -> ReplayResult {
    to_replay_result(result.clone())
}

impl AccountAssert {
    /// Evaluate against the post-replay `svm`; `ctx` supplies pre-transaction values
    /// for the delta checks.
    pub(crate) fn eval(
        &self,
        svm: &LiteSVM,
        ctx: &ReplayContext,
        pre: &HashMap<Address, Account>,
    ) -> Result<bool> {
        // The state the transaction started from: mutated where a mutation
        // applied, loaded otherwise.
        let pre_of = |address: &str| -> Option<Account> {
            Address::from_str(address)
                .ok()
                .and_then(|a| pre.get(&a).cloned())
                .or_else(|| ctx.pre_account(address).cloned())
        };
        let addr = Address::from_str(&self.address)
            .map_err(|_| Error::InvalidAddress(self.address.clone()))?;
        let acc = svm.get_account(&addr);
        // A check against an address the replay never loaded is a typo, not an
        // account that is legitimately empty. Erroring here (rather than reading
        // a silent zero) is what stops a mistyped assert from passing vacuously —
        // the same guarantee the mutation path gives via MutationTargetMissing.
        // A known account that is now gone (closed/drained) still reads zero,
        // which is its true post-transaction state.
        if acc.is_none() && !ctx.is_known(&self.address) {
            return Err(Error::AccountNotFound(self.address.clone()));
        }
        match &self.check {
            StateCheck::Lamports { op, value } => {
                Ok(op.test_i128(acc.map(|a| a.lamports).unwrap_or(0) as i128, *value))
            }
            StateCheck::U64At { offset, op, value } => {
                let acc = acc.ok_or_else(|| Error::AccountNotFound(self.address.clone()))?;
                if offset
                    .checked_add(8)
                    .filter(|&e| e <= acc.data.len())
                    .is_none()
                {
                    return Err(Error::OutOfRange {
                        what: format!("u64@{offset}"),
                        len: acc.data.len(),
                    });
                }
                Ok(op.test_i128(read_u64_at(&acc.data, *offset) as i128, *value))
            }
            StateCheck::LamportsDelta { op, value } => {
                let pre = pre_of(&self.address).map(|a| a.lamports).unwrap_or(0) as i128;
                let post = acc.map(|a| a.lamports).unwrap_or(0) as i128;
                Ok(op.test_i128(post - pre, *value))
            }
            StateCheck::TokenDelta { op, value } => {
                let pre = pre_of(&self.address)
                    .map(|a| read_u64_at(&a.data, 64))
                    .unwrap_or(0) as i128;
                let post = acc.map(|a| read_u64_at(&a.data, 64)).unwrap_or(0) as i128;
                Ok(op.test_i128(post - pre, *value))
            }
            StateCheck::Field { name, op, value } => {
                let (dec, _) = ctx.decode_pre(&self.address)?;
                let f = find_field(&dec, name)?;
                let acc = acc.ok_or_else(|| Error::AccountNotFound(self.address.clone()))?;
                Ok(op.test_i128(read_field_int(&acc.data, f)?, *value))
            }
            StateCheck::FieldDelta { name, op, value } => {
                let (dec, _) = ctx.decode_pre(&self.address)?;
                let f = find_field(&dec, name)?;
                let pre = pre_of(&self.address).ok_or_else(|| {
                    Error::AccountNotFound(format!("{} (no pre-state)", self.address))
                })?;
                let acc = acc.ok_or_else(|| Error::AccountNotFound(self.address.clone()))?;
                let delta = read_field_int(&acc.data, f)? - read_field_int(&pre.data, f)?;
                Ok(op.test_i128(delta, *value))
            }
            StateCheck::FieldUnchanged { name } => {
                let (dec, _) = ctx.decode_pre(&self.address)?;
                let f = find_field(&dec, name)?;
                let pre = pre_of(&self.address).ok_or_else(|| {
                    Error::AccountNotFound(format!("{} (no pre-state)", self.address))
                })?;
                let acc = acc.ok_or_else(|| Error::AccountNotFound(self.address.clone()))?;
                Ok(field_bytes(&acc.data, f)? == field_bytes(&pre.data, f)?)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str, ty: &str, size: usize) -> crate::decode::Field {
        crate::decode::Field {
            name: name.into(),
            offset: 0,
            ty: ty.into(),
            size,
            value: String::new(),
            editable: true,
            note: None,
        }
    }

    #[test]
    fn rent_sysvar_from_the_loaded_world_replaces_litesvm_defaults() {
        // Mainnet's post-reduction parameters: 6,333 / byte-year, threshold 1.0.
        let mut data = Vec::new();
        data.extend_from_slice(&6_333u64.to_le_bytes());
        data.extend_from_slice(&1.0f64.to_le_bytes());
        data.push(50);
        let rent = parse_rent(&data).unwrap();
        assert_eq!(rent.lamports_per_byte_year, 6_333);
        assert_eq!(
            rent.minimum_balance(165),
            1_855_569,
            "a token account's minimum on mainnet"
        );
        assert!(parse_rent(&[0u8; 5]).is_none());

        let rent_addr = Address::from_str(SYSVAR_RENT).unwrap();
        let loaded = vec![(
            rent_addr,
            Loaded::Data(Account {
                lamports: 1_009_200,
                data: data.clone(),
                owner: Address::from_str("Sysvar1111111111111111111111111111111111111").unwrap(),
                executable: false,
                rent_epoch: 0,
            }),
        )];
        let svm = svm_from_loaded_with(&loaded, &[], 0, false);
        let stored = svm
            .get_account(&rent_addr)
            .expect("rent sysvar account in the world");
        assert_eq!(parse_rent(&stored.data), Some(rent));
        // The runtime-side cache took it too, not just the account bytes.
        assert_eq!(svm.minimum_balance_for_rent_exemption(165), 1_855_569);
        // Without it, LiteSVM's default (3,480 × 2.0) applies.
        let default = svm_from_loaded_with(&[], &[], 0, false);
        let default_rent = parse_rent(&default.get_account(&rent_addr).unwrap().data).unwrap();
        assert_eq!(default_rent.minimum_balance(165), 2_039_280);

        let mut keys = vec!["X".to_string(), SYSVAR_RENT.to_string()];
        with_rent_sysvar(&mut keys);
        assert_eq!(
            keys.len(),
            2,
            "no duplicate when the tx already references it"
        );
        let mut keys = vec!["X".to_string()];
        with_rent_sysvar(&mut keys);
        assert_eq!(keys.last().map(String::as_str), Some(SYSVAR_RENT));
    }

    #[test]
    fn field_values_encode_le_at_exact_width() {
        assert_eq!(
            encode_field_value(&f("c", "u8", 1), 255).unwrap(),
            vec![255]
        );
        assert_eq!(
            encode_field_value(&f("c", "u64", 8), u64::MAX as i128).unwrap(),
            u64::MAX.to_le_bytes().to_vec()
        );
        assert_eq!(
            encode_field_value(&f("c", "i32", 4), -5).unwrap(),
            (-5i32).to_le_bytes().to_vec()
        );
        assert_eq!(
            encode_field_value(&f("c", "i64", 8), i64::MIN as i128).unwrap(),
            i64::MIN.to_le_bytes().to_vec()
        );
        assert_eq!(encode_field_value(&f("c", "bool", 1), 1).unwrap(), vec![1]);
    }

    #[test]
    fn field_values_out_of_range_are_hard_errors_not_truncations() {
        for (ty, size, value) in [
            ("u8", 1usize, 256i128),
            ("u8", 1, -1),
            ("u16", 2, 65_536),
            ("u64", 8, -1),
            ("u64", 8, u64::MAX as i128 + 1),
            ("i8", 1, 128),
            ("i8", 1, -129),
            ("i64", 8, i64::MAX as i128 + 1),
            ("bool", 1, 2),
        ] {
            assert!(
                matches!(
                    encode_field_value(&f("c", ty, size), value),
                    Err(Error::FieldValueOutOfRange { .. })
                ),
                "{ty} should reject {value}"
            );
        }
    }

    #[test]
    fn non_numeric_field_mutations_are_rejected() {
        assert!(matches!(
            encode_field_value(&f("owner", "pubkey", 32), 1),
            Err(Error::NonNumericField { .. })
        ));
        assert!(matches!(
            encode_field_value(&f("x", "coption-u64", 8), 1),
            Err(Error::NonNumericField { .. })
        ));
    }

    /// A real SPL token account layout (165 bytes) with amount 1234 @ offset 64,
    /// decoded through the same path named-field assertions use.
    fn token_decoded() -> (crate::decode::DecodedAccount, Vec<u8>) {
        let mut data = vec![0u8; 165];
        data[64..72].copy_from_slice(&1234u64.to_le_bytes());
        let dec = crate::decode::decode_bytes("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA", &data)
            .expect("token account decodes");
        (dec, data)
    }

    #[test]
    fn field_resolves_by_name_and_dot_segment() {
        let (dec, data) = token_decoded();
        let f = find_field(&dec, "amount").unwrap();
        assert_eq!(f.offset, 64);
        assert_eq!(read_field_int(&data, f).unwrap(), 1234);
        // `vault.amount` reaches the same field by its final dot-segment.
        assert_eq!(find_field(&dec, "vault.amount").unwrap().offset, 64);
    }

    #[test]
    fn feature_toggle_changes_the_effective_feature_set() {
        // The get_sysvar syscall gate is active on mainnet (our replay baseline).
        let feat = Address::from_str("CLCoTADvV64PSrnR6QXty6Fwrt9Xc6EdxSJE4wLRePjq").unwrap();
        let pk = solana_pubkey::Pubkey::new_from_array(feat.to_bytes());

        // Baseline (no toggles) has it active…
        let base = svm_from_loaded_with(&[], &[], 0, false);
        assert!(
            base.get_feature_set_ref().is_active(&pk),
            "gate should be active by default"
        );

        // …and our deactivate toggle actually removes it from the runtime set.
        let off = svm_from_loaded_with(
            &[],
            &[FeatureToggle {
                id: feat,
                active: false,
            }],
            0,
            false,
        );
        assert!(
            !off.get_feature_set_ref().is_active(&pk),
            "toggle should deactivate the gate"
        );
    }

    #[test]
    fn unknown_field_errors_and_lists_candidates() {
        let (dec, _) = token_decoded();
        let e = find_field(&dec, "reserveA").unwrap_err().to_string();
        assert!(e.contains("no field \"reserveA\""), "{e}");
        assert!(e.contains("amount"), "should list available fields: {e}");
    }

    #[test]
    fn non_integer_field_refuses_comparison() {
        let (dec, data) = token_decoded();
        let owner = find_field(&dec, "owner").unwrap();
        assert!(read_field_int(&data, owner)
            .unwrap_err()
            .to_string()
            .contains("pubkey"));
    }

    #[test]
    fn signed_fields_sign_extend() {
        let f = crate::decode::Field {
            name: "start_ts".into(),
            offset: 0,
            ty: "i64".into(),
            size: 8,
            value: String::new(),
            editable: true,
            note: None,
        };
        assert_eq!(read_field_int(&(-42i64).to_le_bytes(), &f).unwrap(), -42);
        assert_eq!(read_field_int(&7i64.to_le_bytes(), &f).unwrap(), 7);
    }

    #[test]
    fn typoed_mutation_is_a_hard_error_not_a_passing_revert() {
        // The v0.1 hole: a mutation targeting an unloaded (typo'd) address was
        // folded into ReplayResult{success:false}, which `expect: revert`
        // scenarios happily accepted. It must be a hard Err now.
        let ctx = ReplayContext {
            signature: String::new(),
            tx: VersionedTransaction::default(),
            loaded: Vec::new(),
            slot: None,
            block_time: None,
            time_travel: TimeTravel::default(),
            idls: HashMap::new(),
            feature_toggles: Vec::new(),
        };
        // (Not the system program — that one exists as a builtin in a fresh SVM.)
        let typo = Mutation::lamports(Address::from([7u8; 32]).to_string(), 0);

        assert!(matches!(
            ctx.run_full(std::slice::from_ref(&typo)),
            Err(Error::MutationTargetMissing(_))
        ));

        // A whole suite fails fast — before any scenario executes.
        let suite = [crate::check::Scenario::new("drain")
            .mutate(typo)
            .check(crate::check::Check::revert())];
        assert!(matches!(
            run_suite(&ctx, None, &suite),
            Err(Error::MutationTargetMissing(_))
        ));
    }

    #[test]
    fn patch_bounds_are_validated_up_front() {
        let addr = Address::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();
        let ctx = ReplayContext {
            signature: String::new(),
            tx: VersionedTransaction::default(),
            loaded: vec![(
                addr,
                Loaded::Data(Account {
                    lamports: 1,
                    data: vec![0u8; 8],
                    owner: Address::default(),
                    executable: false,
                    rent_epoch: 0,
                }),
            )],
            slot: None,
            block_time: None,
            time_travel: TimeTravel::default(),
            idls: HashMap::new(),
            feature_toggles: Vec::new(),
        };
        // In range: fine. Out of range (offset 4 + 8 bytes > size 8): typed error.
        assert!(ctx
            .validate_mutations(&[Mutation::patch(addr.to_string(), 0, vec![0; 8])])
            .is_ok());
        assert!(matches!(
            ctx.validate_mutations(&[Mutation::patch(addr.to_string(), 4, vec![0; 8])]),
            Err(Error::PatchOutOfRange {
                offset: 4,
                len: 8,
                size: 8,
                ..
            })
        ));
        // usize overflow in offset + len must not panic either.
        assert!(ctx
            .validate_mutations(&[Mutation::patch(addr.to_string(), usize::MAX, vec![0; 8])])
            .is_err());
    }

    #[test]
    fn fixture_roundtrip_preserves_slot_and_clock() {
        // The core fixture-fidelity invariant: freezing a context and reloading it
        // must reproduce the exact slot AND wall-clock the live replay ran at — not
        // drop the block time (which used to happen, so time-gated programs behaved
        // differently after freezing).
        let ctx = ReplayContext {
            signature: "SIG".into(),
            tx: VersionedTransaction::default(),
            loaded: Vec::new(),
            slot: Some(295_000_123),
            block_time: Some(1_787_600_325),
            time_travel: TimeTravel::default(),
            idls: HashMap::new(),
            feature_toggles: Vec::new(),
        };

        let fx = ctx.to_fixture().unwrap();
        assert_eq!(fx.captured_slot, Some(295_000_123));
        assert_eq!(fx.captured_block_time, Some(1_787_600_325));

        // Survives a JSON round-trip (the on-disk format)…
        let json = fx.to_json().unwrap();
        let fx2 = Fixture::from_json(&json).unwrap();
        // …and reloads into a context with the same clock.
        let restored = ReplayContext::from_fixture(&fx2).unwrap();
        assert_eq!(restored.slot, Some(295_000_123));
        assert_eq!(restored.block_time, Some(1_787_600_325));
    }

    #[test]
    fn chrono_like_formats_utc() {
        assert_eq!(chrono_like(0), "1970-01-01 00:00 UTC");
        // 2020-03-16 14:29:00 UTC — Solana mainnet genesis.
        assert_eq!(chrono_like(1_584_368_940), "2020-03-16 14:29 UTC");
        // Pre-epoch timestamps wrap to the previous civil day, not garbage.
        assert_eq!(chrono_like(-1), "1969-12-31 23:59 UTC");
    }

    fn clock() -> Clock {
        Clock {
            slot: 1_000 * SLOTS_PER_EPOCH as u64,
            epoch: 1_000,
            epoch_start_timestamp: 2_000_000_000,
            unix_timestamp: 2_000_000_000,
            leader_schedule_epoch: 1_001,
        }
    }

    #[test]
    fn noop_time_travel_changes_nothing() {
        let tt = TimeTravel::default();
        assert!(tt.is_noop());
        let mut c = clock();
        tt.apply(&mut c);
        assert_eq!(
            (c.slot, c.epoch, c.unix_timestamp),
            (clock().slot, 1_000, 2_000_000_000)
        );
    }

    #[test]
    fn epoch_jump_advances_slot_epoch_and_time_together() {
        let tt = TimeTravel {
            epochs: Some(2),
            ..Default::default()
        };
        let mut c = clock();
        tt.apply(&mut c);
        assert_eq!(c.epoch, 1_002);
        assert_eq!(c.slot, clock().slot + 2 * SLOTS_PER_EPOCH as u64);
        let expected_secs = (2.0 * SLOTS_PER_EPOCH as f64 * SECS_PER_SLOT) as i64;
        assert_eq!(c.unix_timestamp, 2_000_000_000 + expected_secs);
        assert_eq!(c.leader_schedule_epoch, c.epoch + 1);
    }

    #[test]
    fn seconds_jump_moves_slots_in_step() {
        let tt = TimeTravel {
            seconds: Some(40),
            ..Default::default()
        };
        let mut c = clock();
        tt.apply(&mut c);
        assert_eq!(c.unix_timestamp, 2_000_000_040);
        assert_eq!(c.slot, clock().slot + 100); // 40s / 0.4s per slot
    }

    #[test]
    fn extreme_warp_saturates_instead_of_overflowing() {
        // A single absurd jump, and the applied clock, must clamp rather than
        // panic (debug) or wrap (release).
        let tt = TimeTravel {
            epochs: Some(i64::MAX),
            seconds: Some(i64::MAX),
            slots: Some(i64::MAX),
            ..Default::default()
        };
        let mut c = clock();
        tt.apply(&mut c); // must not panic
        assert!(c.unix_timestamp >= clock().unix_timestamp);
    }

    #[test]
    fn absolute_settings_override_relative_jumps() {
        let tt = TimeTravel {
            seconds: Some(1_000_000),
            at_unix_timestamp: Some(3_000_000_000),
            at_epoch: Some(7),
            ..Default::default()
        };
        let mut c = clock();
        tt.apply(&mut c);
        assert_eq!(c.unix_timestamp, 3_000_000_000);
        assert_eq!(c.epoch, 7);
        // The epoch's start can never be after "now".
        assert!(c.epoch_start_timestamp <= c.unix_timestamp);
    }

    #[test]
    fn read_u64_at_handles_bounds() {
        let mut data = vec![0u8; 72];
        data[64..72].copy_from_slice(&42u64.to_le_bytes());
        assert_eq!(read_u64_at(&data, 64), 42);
        assert_eq!(read_u64_at(&data, 70), 0); // out of range → 0, not a panic
    }

    #[test]
    fn reconstructs_closed_token_account_from_meta() {
        let tx = serde_json::json!({
            "meta": {
                "preBalances": [2_039_280],
                "preTokenBalances": [{
                    "accountIndex": 0,
                    "mint": "So11111111111111111111111111111111111111112",
                    "owner": "Vote111111111111111111111111111111111111111",
                    "uiTokenAmount": { "amount": "12345", "decimals": 9 }
                }]
            }
        });
        let keys = vec!["4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T".to_string()];
        let pre = PreState::from_meta(&tx, &keys);
        let acc = pre
            .reconstruct(&keys[0])
            .expect("should rebuild the SPL account");
        assert_eq!(acc.data.len(), 165);
        assert_eq!(read_u64_at(&acc.data, 64), 12345);
        assert_eq!(acc.data[108], 1); // AccountState::Initialized
        assert_eq!(acc.owner.to_string(), SPL_TOKEN_PROGRAM);
    }

    #[test]
    fn never_resurrects_an_account_the_tx_creates() {
        let pre = PreState::default();
        assert!(pre.reconstruct("SomeAddr").is_none());
    }

    #[test]
    fn captures_pre_transaction_lamports_per_account() {
        // `preBalances` is parallel to the resolved account list; `from_meta`
        // must key each pre-tx balance by its address so the reconstruction loop
        // in `build_context` can rewind a still-existing account's lamports.
        let tx = serde_json::json!({
            "meta": { "preBalances": [5_000_000_000u64, 2_039_280u64] }
        });
        let keys = vec![
            "4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T".to_string(),
            "So11111111111111111111111111111111111111112".to_string(),
        ];
        let pre = PreState::from_meta(&tx, &keys);
        assert_eq!(pre.lamports.get(&keys[0]).copied(), Some(5_000_000_000));
        assert_eq!(pre.lamports.get(&keys[1]).copied(), Some(2_039_280));
        assert!(!pre.is_empty());
    }
}

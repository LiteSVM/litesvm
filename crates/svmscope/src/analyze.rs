//! The analysis and report types — what a decoded transaction, an address
//! overview, and an enriched simulation look like to a consumer.

use {
    crate::{
        compute,
        cpi_tree::{self, IxAccount, IxArg},
        decode, diffs,
    },
    serde::Serialize,
};

/// The full analysis of a transaction — the payload the CLI prints and the API serves.
#[derive(Debug, Serialize)]
pub struct Analysis {
    /// The resolved transaction signature (echoed back — useful when the input
    /// was an address that we resolved to its latest transaction).
    pub signature: String,
    /// Headline facts: success, fee, slot, version, programs invoked.
    pub overview: Overview,
    /// The full cross-program invocation tree, instructions named where possible.
    pub cpi_tree: Vec<cpi_tree::CpiEntry>,
    /// SOL balance changes per touched account.
    pub balance_change: Vec<diffs::BalanceChange>,
    /// SPL token balance changes per touched token account.
    pub token_change: Vec<diffs::TokenChange>,
    /// Compute units consumed, attributed per program.
    pub compute: Vec<compute::CuUsage>,
    /// The program logs the transaction actually produced on-chain. Available
    /// immediately from the transaction metadata — no replay needed.
    pub logs: Vec<String>,
    /// Local replay is opt-in (it's the slow, drift-prone part) — `analyze`
    /// leaves this `None`; run it on demand via [`crate::Replay`].
    pub replay: Option<ReplayResult>,
    /// The transaction's accounts, with decoded fields where the layout is known.
    /// Drives the what-if editor: pick an account, edit named fields.
    pub accounts: Vec<decode::AccountInfo>,
}

/// Headline facts about the transaction as it actually ran on-chain.
#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    /// The transaction succeeded on-chain (meta.err is null).
    pub success: bool,
    /// Fee paid, in lamports.
    pub fee: u64,
    /// Slot the transaction landed in, if reported.
    pub slot: Option<u64>,
    /// On-chain compute units consumed, if reported.
    pub compute_units: Option<u64>,
    /// The account that paid the fee (the first signer).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_payer: Option<String>,
    /// Transaction version — "legacy" or "v0".
    pub version: String,
    /// Block time (unix seconds), when the RPC reports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_time: Option<i64>,
    /// The recent blockhash the transaction was signed against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_blockhash: Option<String>,
    /// Number of accounts the transaction touched (static + lookup-table loaded).
    pub account_count: usize,
    /// Top-level programs invoked, in order (the instruction programs).
    pub top_programs: Vec<String>,
}

pub(crate) fn build_overview(
    tx: &serde_json::Value,
    cpi_tree: &[cpi_tree::CpiEntry],
    account_count: usize,
) -> Overview {
    let top_programs = cpi_tree
        .iter()
        .filter(|e| e.stack_height == 1)
        .map(|e| e.program.clone())
        .collect();
    // `version` is a number (0) for v0, or the string "legacy".
    let version = match &tx["version"] {
        serde_json::Value::Number(n) => format!("v{n}"),
        serde_json::Value::String(s) => s.clone(),
        _ => "legacy".to_string(),
    };
    Overview {
        success: tx["meta"]["err"].is_null(),
        fee: tx["meta"]["fee"].as_u64().unwrap_or(0),
        slot: tx["slot"].as_u64(),
        compute_units: tx["meta"]["computeUnitsConsumed"].as_u64(),
        fee_payer: tx["transaction"]["message"]["accountKeys"][0]
            .as_str()
            .map(String::from),
        version,
        block_time: tx["blockTime"].as_i64(),
        recent_blockhash: tx["transaction"]["message"]["recentBlockhash"]
            .as_str()
            .map(String::from),
        account_count,
        top_programs,
    }
}

/// An account/program's on-chain overview — what an explorer's address page shows.
#[derive(Debug, Serialize)]
pub struct AccountOverview {
    /// The address that was looked up, as base58.
    pub address: String,
    /// Whether the account exists on the queried cluster.
    pub exists: bool,
    /// The program that owns the account.
    pub owner: String,
    /// The account's SOL balance in lamports.
    pub lamports: u64,
    /// Whether the account holds an executable program.
    pub executable: bool,
    /// Length of the account's data in bytes.
    pub data_len: usize,
    /// Program deployment details, when the address is an executable program.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<ProgramInfo>,
    /// The program's name from its on-chain Anchor IDL, if published.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idl_name: Option<String>,
    /// Decoded fields for a recognized data account (SPL or IDL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decoded: Option<decode::DecodedAccount>,
}

/// Deployment details for an executable program account.
#[derive(Debug, Clone, Serialize)]
pub struct ProgramInfo {
    /// The programdata account holding the ELF (for upgradeable programs).
    pub program_data: String,
    /// Whether the program was deployed with the upgradeable loader.
    pub upgradeable: bool,
    /// Who may upgrade the program; `None` when upgrades are burned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgrade_authority: Option<String>,
    /// Slot of the most recent deployment, if reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_deployed_slot: Option<u64>,
}

/// One entry in an address's recent transaction history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SigInfo {
    /// The transaction's signature, as base58.
    pub signature: String,
    /// Slot the transaction landed in, if reported.
    pub slot: Option<u64>,
    /// Whether the transaction failed on-chain.
    pub err: bool,
    /// Unix timestamp, when the RPC reports it.
    pub block_time: Option<i64>,
}

/// A simulation result enriched with what a developer actually needs: a
/// human-readable failure reason and the field-level account diff.
#[derive(Debug, Serialize)]
pub struct SimulationReport {
    /// The underlying replay outcome (logs, success, CU).
    pub replay: ReplayResult,
    /// Where the clock was warped to, when time travel was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock: Option<String>,
    /// The failure explained in plain language, resolved from the program's IDL
    /// when possible ("Overflow — counter overflowed") instead of Custom(6000).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explain: Option<Explanation>,
    /// Every account the transaction changed, before → after.
    pub diffs: Vec<AccountDiff>,
    /// The pre-sign overview (size, fees, named instructions, actions and
    /// warnings) — populated only on the preflight path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preflight: Option<PreflightOverview>,
}

/// A program failure translated into plain language.
#[derive(Debug, Clone, Serialize)]
pub struct Explanation {
    /// e.g. "Overflow" (from the IDL) or "InsufficientFunds".
    pub title: String,
    /// The human message, e.g. "counter overflowed".
    pub detail: String,
    /// Which program raised it, when we can attribute it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// The raw error, kept for reference.
    pub raw: String,
}

/// One account's before → after, with named field changes where the layout is known.
#[derive(Debug, Clone, Serialize)]
pub struct AccountDiff {
    /// The changed account's address, as base58.
    pub address: String,
    /// The program that owns the account.
    pub owner: String,
    /// Lamport balance before the transaction.
    pub lamports_before: u64,
    /// Lamport balance after the transaction.
    pub lamports_after: u64,
    /// Named fields that changed (empty when the layout isn't recognized).
    pub fields: Vec<FieldDiff>,
    /// True when data changed but we couldn't decode it into fields.
    pub raw_data_changed: bool,
}

/// One named field's before → after inside an [`AccountDiff`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldDiff {
    /// The field's name (e.g. "amount").
    pub name: String,
    /// The field's type label (e.g. "u64").
    #[serde(rename = "type")]
    pub ty: String,
    /// The value before the transaction, formatted for display.
    pub before: String,
    /// The value after the transaction, formatted for display.
    pub after: String,
}

/// An account's role in the transaction — the signer/writable classification
/// Solana Explorer's inspector shows, derived from the message header + key
/// ordering (static writable-signers, readonly-signers, writable-unsigned,
/// readonly-unsigned, then ALT writable, then ALT readonly).
#[derive(Debug, Clone, Serialize)]
pub struct AccountRole {
    /// The account address, base58.
    pub address: String,
    /// Must sign the transaction.
    pub signer: bool,
    /// The transaction may modify it.
    pub writable: bool,
    /// Invoked as a program by some instruction.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub program: bool,
    /// The fee payer (first writable signer).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub fee_payer: bool,
}

/// One decoded top-level instruction of a pre-flight transaction.
#[derive(Debug, Clone, Serialize)]
pub struct PreflightIx {
    /// Position in the message (0-based).
    pub index: usize,
    /// The program id, base58.
    pub program: String,
    /// The instruction's name from a native layout or the program's IDL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Decoded arguments, where the layout/IDL is known.
    pub args: Vec<IxArg>,
    /// The instruction's accounts, IDL-named where known.
    pub accounts: Vec<IxAccount>,
}

/// The pre-sign overview block of a [`crate::SimulationReport`].
#[derive(Debug, Clone, Serialize)]
pub struct PreflightOverview {
    /// Serialized transaction size in bytes (signatures included).
    pub serialized_size: usize,
    /// The network's hard cap (1232 bytes).
    pub max_size: usize,
    /// Base fee: 5000 lamports per required signature.
    pub base_fee_lamports: u64,
    /// Priority fee from the ComputeBudget instructions, when a price is set.
    /// Computed from the requested compute-unit limit — or a defaulted one,
    /// flagged in `priority_fee_note`.
    pub priority_fee_lamports: u64,
    /// Present when the priority fee had to assume a defaulted CU limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority_fee_note: Option<String>,
    /// The requested compute-unit limit, when one was set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_unit_limit: Option<u64>,
    /// The fee payer (first required signer).
    pub fee_payer: String,
    /// The fee payer's live balance, when it could be fetched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_payer_balance: Option<u64>,
    /// Whether the balance covers base + priority fees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_payer_can_pay: Option<bool>,
    /// Top-level instructions, decoded and named.
    pub instructions: Vec<PreflightIx>,
    /// Every account with its signer/writable role (Explorer's account table).
    pub accounts: Vec<AccountRole>,
    /// Per-program compute units, from the simulation. Populated after the
    /// simulation runs (empty until then).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub compute: Vec<crate::compute::CuUsage>,
    /// Plain-English "what signing this does" lines.
    pub actions: Vec<String>,
    /// Danger flags: authority changes, delegations, closes, reassignments.
    pub warnings: Vec<String>,
}

/// The outcome of one local replay run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Default)]
pub struct ReplayResult {
    /// Whether the transaction executed without error.
    pub success: bool,
    /// The failure, formatted, when `success` is false.
    pub error: Option<String>,
    /// The failure resolved to its human name via the program's IDL, e.g.
    /// "SlippageToleranceExceeded" for a bare `Custom(6001)`. Filled in by the
    /// library layer (which has RPC access); `None` when unresolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_name: Option<String>,
    /// The program logs the replay produced.
    pub logs: Vec<String>,
    /// Compute units the replay consumed.
    pub compute_units: u64,
}

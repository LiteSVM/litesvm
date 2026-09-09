#![deny(unreachable_pub)]
#![warn(missing_docs)]

//! Decode, replay and mutate Solana transactions in LiteSVM.
//!
//! Start from a mainnet signature. The crate fetches the transaction and
//! everything it touched, rebuilds that world inside a [`LiteSVM`](litesvm::LiteSVM)
//! instance, and lets you run it again as often as you like, offline:
//!
//! 1. **Decode**: the full cross-program invocation tree, every instruction,
//!    account and argument named from the program's on-chain IDL or a known
//!    native layout, balance and token changes, compute units per program.
//! 2. **Replay**: the real program binaries, executed locally against the
//!    reconstructed pre-transaction state.
//! 3. **Mutate**: change an account by field name, warp the clock, flip a
//!    feature gate, edit an instruction argument, then replay again and see
//!    what changes.
//!
//! # Quickstart
//!
//! Two nouns: a [`Scope`] talks to a node and caches, a [`Replay`] is one
//! transaction's reconstructed world. All RPC happens in [`Scope::replay`];
//! every run after that is local and free.
//!
//! ```no_run
//! use svmscope::{Mutation, Scope};
//!
//! let scope = Scope::new("https://api.mainnet-beta.solana.com");
//! let sig = "your transaction signature";
//!
//! // 1. Decode: CPI tree, named instructions, balance/token diffs, CU per program.
//! let analysis = scope.analyze(sig)?;
//! println!("fee: {} lamports, {} top-level instructions",
//!          analysis.overview.fee, analysis.cpi_tree.len());
//!
//! // 2. Replay the real programs locally.
//! let mut replay = scope.replay(sig)?;
//! println!("replay success: {}", replay.run()?.result.success);
//!
//! // 3. What-if: zero out an account, jump 30 days ahead, and replay again.
//! replay.advance_seconds(30 * 86_400);
//! let what_if = replay.simulate(&[Mutation::lamports("SomeAccount111...", 0)])?;
//! println!("mutated replay success: {}", what_if.result.success);
//! # Ok::<(), svmscope::Error>(())
//! ```
//!
//! # Step through a transaction
//!
//! [`Replay::trace`] runs the transaction one instruction at a time and
//! records, for every top-level instruction and every CPI, the accounts it
//! changed and how. Two traces diff against each other, so a mutation's
//! effect can be pinned to the exact step where the outcome diverged.
//!
//! # Hermetic testing
//!
//! Replays against live RPC state drift as the chain moves on. To pin a
//! transaction's world forever, [`Scope::capture`] snapshots the transaction,
//! every account it touched, every program ELF and every IDL into a
//! [`Fixture`]; [`Replay::from_fixture`] rebuilds the world from that file and
//! runs scenario suites ([`Scenario`], [`Check`]) against it, offline and
//! deterministic, so a suite is a real regression test you can commit.
//!
//! # Historical state
//!
//! [`Scope::replay_at_slot`] reconstructs each account as it was at the
//! transaction's own slot, from the accounts' later writes and an optional
//! archive node, and reports what it could and could not verify in a
//! [`FidelityCertificate`]. Every replay carries its [`Fidelity`].
//!
//! # Building transactions from an IDL
//!
//! Against a local validator, [`Scope::program_with_idl`] gives an IDL-driven
//! builder: pick a method, supply accounts and JSON arguments, sign, submit,
//! and get back a [`CapturedTransaction`] whose replay holds the exact
//! pre-transaction world.
//!
//! # Public API
//!
//! Consumer-facing types are re-exported from the crate root. The [`idl`],
//! [`report`], [`scan`], [`reconstruct`] and [`spec`] modules expose the
//! specialised APIs; every other module is private.
//!
//! # Errors
//!
//! A reverting replay is not an error: it comes back as a successful
//! observation with `result.success == false`. [`Error`] always means the
//! crate itself could not do what was asked (RPC failure, unknown transaction,
//! a mutation targeting an account that is not loaded, an unknown field name).
//!
//! # Feature flags
//!
//! - `profiler`: the compute profiler ([`profile`]) traces every BPF
//!   instruction a replay executes and attributes it to functions and syscalls.
//!   Turns on LiteSVM's `register-tracing` feature.
//!
//! All node access lives in one private module, so a consumer that only
//! replays fixtures never touches the network.

mod analyze;
mod bundled_idls;
mod check;
mod compute;
mod cpi_tree;
mod decode;
mod diagnose;
mod diffs;
mod error;
mod fidelity;
mod fixture;
pub mod idl;
mod idl_encode;
mod idl_model;
mod invariant;
pub(crate) mod ixname;
mod mutation;
mod preflight;
#[cfg(feature = "profiler")]
pub mod profile;
mod program;
pub mod reconstruct;
mod replay;
pub mod report;
mod rpc;
pub mod scan;
mod scope;
mod search;
mod session;
pub mod spec;
mod submit;
mod trace;
pub(crate) mod utils;
#[cfg(test)]
mod wire_format_tests;

pub use {
    analyze::{
        AccountDiff, AccountOverview, AccountRole, Analysis, Explanation, FieldDiff, Overview,
        PreflightIx, PreflightOverview, ProgramInfo, ReplayResult, SigInfo, SimulationReport,
    },
    check::{AccountCheck, AssertOutcome, Check, Cmp, Scenario, ScenarioOutcome},
    compute::CuUsage,
    cpi_tree::{CpiEntry, IxAccount, IxArg},
    decode::{AccountInfo, DecodedAccount, Field},
    diagnose::Diagnosis,
    diffs::{BalanceChange, TokenChange},
    error::{Error, Result},
    fidelity::{AccountProvenance, AccountState, Fidelity, FidelityCertificate, Provenance},
    fixture::{Fixture, FixtureEntry, OnchainRecord, FIXTURE_VERSION},
    invariant::Invariant,
    mutation::Mutation,
    preflight::compute_breakdown,
    program::{MethodBuilder, ProgramClient},
    replay::{FeatureToggle, TimeTravel},
    scan::{scan_breaking_points, BreakingPoint, ScanOptions},
    scope::Scope,
    search::Threshold,
    session::{PatchComparison, Replay, Replayed},
    submit::CapturedTransaction,
    trace::{
        DecodedEvent, DriftedAccount, ReturnData, Step, StepAccountState, StepDiff, StepError,
        StepSummary, Trace, TraceDiff,
    },
};

/// Compile-checks every Rust example in the README as part of `cargo test`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;

/// Resolve a cluster name or explicit RPC URL to an endpoint. Precedence:
/// explicit `rpc` URL > `cluster` name > `default`.
///
/// Clusters: `mainnet`, `devnet`, `testnet`, `localnet` (127.0.0.1:8899). A value
/// starting with `http` in either field is used verbatim; an unknown cluster
/// name is an error.
pub fn resolve_rpc_url(cluster: Option<&str>, rpc: Option<&str>, default: &str) -> Result<String> {
    if let Some(u) = rpc {
        if u.starts_with("http") {
            return Ok(u.to_string());
        }
    }
    match cluster.map(|c| c.trim().to_ascii_lowercase()).as_deref() {
        None | Some("") => Ok(default.to_string()),
        Some("mainnet") | Some("mainnet-beta") | Some("m") => {
            Ok("https://api.mainnet-beta.solana.com".into())
        }
        Some("devnet") | Some("d") => Ok("https://api.devnet.solana.com".into()),
        Some("testnet") | Some("t") => Ok("https://api.testnet.solana.com".into()),
        Some("localnet") | Some("local") | Some("localhost") | Some("l") => {
            Ok("http://127.0.0.1:8899".into())
        }
        Some(other) if other.starts_with("http") => Ok(other.to_string()),
        Some(other) => Err(Error::InvalidSpec(format!(
            "unknown cluster '{other}' (expected mainnet, devnet, testnet, or localnet)"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_rpc_url_precedence() {
        // Explicit URL beats everything.
        assert_eq!(
            resolve_rpc_url(Some("devnet"), Some("http://my"), "http://def").unwrap(),
            "http://my"
        );
        // Cluster names map to public endpoints.
        assert_eq!(
            resolve_rpc_url(Some("devnet"), None, "http://def").unwrap(),
            "https://api.devnet.solana.com"
        );
        assert_eq!(
            resolve_rpc_url(Some("m"), None, "http://def").unwrap(),
            "https://api.mainnet-beta.solana.com"
        );
        assert_eq!(
            resolve_rpc_url(Some("localnet"), None, "http://def").unwrap(),
            "http://127.0.0.1:8899"
        );
        // Nothing specified → the default; an unknown cluster is an error.
        assert_eq!(
            resolve_rpc_url(None, None, "http://def").unwrap(),
            "http://def"
        );
        assert!(matches!(
            resolve_rpc_url(Some("nope"), None, "http://def"),
            Err(Error::InvalidSpec(_))
        ));
        // A non-http "rpc" value is ignored, not used verbatim.
        assert_eq!(
            resolve_rpc_url(None, Some("garbage"), "http://def").unwrap(),
            "http://def"
        );
    }
}

#![deny(unreachable_pub)]
#![warn(missing_docs)]

//! Decode, replay and mutate Solana transactions in LiteSVM.

mod analyze;
mod bundled_idls;
mod check;
mod compute;
mod cpi_tree;
mod decode;
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
mod search;
pub(crate) mod utils;

pub use {
    analyze::{
        AccountDiff, AccountOverview, AccountRole, Analysis, Explanation, FieldDiff, Overview,
        PreflightIx, PreflightOverview, ProgramInfo, ReplayResult, SigInfo, SimulationReport,
    },
    check::{AccountCheck, AssertOutcome, Check, Cmp, Scenario, ScenarioOutcome},
    compute::CuUsage,
    cpi_tree::{CpiEntry, IxAccount, IxArg},
    decode::{AccountInfo, DecodedAccount, Field},
    diffs::{BalanceChange, TokenChange},
    error::{Error, Result},
    fidelity::{AccountProvenance, AccountState, Fidelity, FidelityCertificate, Provenance},
    fixture::{Fixture, FixtureEntry, OnchainRecord, FIXTURE_VERSION},
    invariant::Invariant,
    mutation::Mutation,
    search::Threshold,
};

/// Compile-checks every Rust example in the README as part of `cargo test`.

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

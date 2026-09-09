//! Auto breaking-point scan — the deterministic "beast" that finds *every*
//! single-change way a transaction can break, driven by each account's schema.
//!
//! For every touched account it tests, by type:
//! - **existence** — close the account (0 lamports);
//! - **owner program** — reassign the program that owns it;
//! - **every decoded field** — numeric fields binary-searched for the threshold
//!   that flips the outcome; pubkey fields (authority, mint, delegate…)
//!   substituted; enum/flag bytes (state…) swept for a breaking value.
//!
//! The replay's world is fetched once; every candidate is a *local* LiteSVM run,
//! so a whole sweep costs no extra RPC. Free — pure current-state replay.

use {
    crate::{
        decode::DecodedAccount, error::Result, mutation::Mutation, scope::Scope,
        search::search_threshold, Replay,
    },
    serde::Serialize,
};

const SYSTEM: &str = "11111111111111111111111111111111";
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// One way the transaction can be broken.
#[derive(Debug, Clone, Serialize)]
pub struct BreakingPoint {
    /// The account whose change breaks it.
    pub account: String,
    /// The field or knob — `existence`, `owner program`, `SOL balance`, `state`,
    /// `authority`, `reserveA`…
    pub field: String,
    /// A human description: "is closed", "changes", "is frozen", "drops below N".
    pub condition: String,
    /// The field's current value where relevant (0 otherwise).
    pub current: u64,
}

/// How much of the transaction to sweep.
#[derive(Debug, Clone, Copy)]
pub struct ScanOptions {
    /// Maximum accounts to scan (touched-account order).
    pub max_accounts: usize,
    /// Maximum decoded fields to probe per account.
    pub max_fields_per_account: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            max_accounts: 32,
            max_fields_per_account: 24,
        }
    }
}

/// Well-known infrastructure (programs, sysvars): always present, never the
/// interesting break surface — skip so the list stays meaningful.
fn is_infra(address: &str) -> bool {
    address.starts_with("Sysvar")
        || matches!(
            address,
            SYSTEM
                | TOKEN
                | "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
                | "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
                | "ComputeBudget111111111111111111111111111111"
                | "NativeLoader1111111111111111111111111111111"
                | "BPFLoader2111111111111111111111111111111111"
                | "BPFLoaderUpgradeab1e11111111111111111111111"
        )
}

/// Scan a transaction for every single-change way it can be broken.
pub fn scan_breaking_points(
    scope: &Scope,
    signature: &str,
    accounts: &[String],
    opts: ScanOptions,
) -> Result<Vec<BreakingPoint>> {
    let replay = scope.replay(signature)?;
    let baseline = replay.run()?.result.success;

    let mut out: Vec<BreakingPoint> = Vec::new();

    for account in accounts.iter().take(opts.max_accounts) {
        if is_infra(account) {
            continue;
        }
        let Ok(info) = scope.decode_account(account, None) else {
            continue;
        };
        if info.executable {
            continue;
        }

        // Existence — close the account (0 lamports) / a real SOL floor.
        if info.lamports > 0 {
            let acct = account.clone();
            if let Some(th) = search_threshold(0, info.lamports.saturating_mul(2), |v| {
                sim(&replay, &[Mutation::lamports(acct.clone(), v)])
            })? {
                if th.flips_at <= 1 {
                    out.push(bp(account, "existence", "is closed", info.lamports));
                } else {
                    out.push(bp(
                        account,
                        "SOL balance",
                        threshold_phrase(th.flips_at, !th.low_success),
                        info.lamports,
                    ));
                }
            }
        }

        // Owner program — reassign it (any program that checks ownership breaks).
        let other_owner = if info.owner == SYSTEM { TOKEN } else { SYSTEM };
        if flips(
            &replay,
            baseline,
            &[Mutation::owner(account.clone(), other_owner)],
        )? {
            out.push(bp(account, "owner program", "changes", 0));
        }

        let Some(decoded) = info.decoded else {
            continue;
        };

        // Every decoded field, dispatched by type.
        let mut tried = 0usize;
        for f in &decoded.fields {
            if tried >= opts.max_fields_per_account {
                break;
            }
            if !f.editable {
                continue; // coption / non-scalar fields we can't safely perturb
            }
            let probed = match f.ty.as_str() {
                "u64" => probe_u64(&replay, account, f, &mut out)?,
                "u8" => probe_u8(&replay, baseline, account, f, &mut out)?,
                "pubkey" => probe_pubkey(&replay, baseline, account, f, &decoded, &mut out)?,
                _ => false,
            };
            if probed {
                tried += 1;
            }
        }
    }

    Ok(out)
}

/// A numeric field: binary-search its threshold. Returns whether it was probed.
fn probe_u64(
    replay: &Replay,
    account: &str,
    f: &crate::decode::Field,
    out: &mut Vec<BreakingPoint>,
) -> Result<bool> {
    let Ok(current) = f.value.parse::<u64>() else {
        return Ok(false);
    };
    if current == 0 {
        return Ok(false);
    }
    let (acct, off) = (account.to_string(), f.offset);
    if let Some(th) = search_threshold(0, current.saturating_mul(2), |v| {
        sim(
            replay,
            &[Mutation::patch(acct.clone(), off, v.to_le_bytes().to_vec())],
        )
    })? {
        out.push(bp(
            account,
            f.name.clone(),
            threshold_phrase(th.flips_at, !th.low_success),
            current,
        ));
    }
    Ok(true)
}

/// A small enum/flag byte: try a few distinct values for one that breaks it.
fn probe_u8(
    replay: &Replay,
    baseline: bool,
    account: &str,
    f: &crate::decode::Field,
    out: &mut Vec<BreakingPoint>,
) -> Result<bool> {
    let current = f.value.parse::<u8>().ok();
    // Try the recognizable "frozen" (2) first so a token state reports as frozen.
    for cand in [2u8, 0, 255] {
        if Some(cand) == current {
            continue;
        }
        if flips(
            replay,
            baseline,
            &[Mutation::patch(account.to_string(), f.offset, vec![cand])],
        )? {
            let cond = if f.name == "state" && cand == 2 {
                "is frozen".to_string()
            } else {
                format!("changes (e.g. set to {cand})")
            };
            out.push(bp(
                account,
                f.name.clone(),
                cond,
                current.unwrap_or(0) as u64,
            ));
            break;
        }
    }
    Ok(true)
}

/// A pubkey field (authority, mint, delegate…): substitute it and see if the
/// program's check breaks.
fn probe_pubkey(
    replay: &Replay,
    baseline: bool,
    account: &str,
    f: &crate::decode::Field,
    _d: &DecodedAccount,
    out: &mut Vec<BreakingPoint>,
) -> Result<bool> {
    let sentinel = vec![0x11u8; f.size.max(32)];
    if flips(
        replay,
        baseline,
        &[Mutation::patch(account.to_string(), f.offset, sentinel)],
    )? {
        let label = if f.name == "owner" {
            "authority"
        } else {
            f.name.as_str()
        };
        out.push(bp(account, label, "changes", 0));
    }
    Ok(true)
}

fn bp(
    account: &str,
    field: impl Into<String>,
    condition: impl Into<String>,
    current: u64,
) -> BreakingPoint {
    BreakingPoint {
        account: account.to_string(),
        field: field.into(),
        condition: condition.into(),
        current,
    }
}

/// Simulate one mutation set; the transaction's success.
fn sim(replay: &Replay, muts: &[Mutation]) -> Result<bool> {
    Ok(replay.simulate(muts)?.result.success)
}

/// Whether a mutation set flips the outcome vs `baseline`.
fn flips(replay: &Replay, baseline: bool, muts: &[Mutation]) -> Result<bool> {
    Ok(sim(replay, muts)? != baseline)
}

fn threshold_phrase(flips_at: u64, breaks_below: bool) -> String {
    if breaks_below {
        format!("drops below {flips_at}")
    } else {
        format!("reaches {flips_at} or above")
    }
}

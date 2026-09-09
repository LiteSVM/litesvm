//! The pre-sign overview: what a not-yet-sent transaction *is* and what signing
//! it would do — size, fees, fee payer, IDL-named instructions, and
//! plain-English actions with danger flags. This is the layer a wallet's
//! "Base64 message" deserves beyond raw simulation logs: the person pasting it
//! is asking "what am I about to sign, and is it safe?"

use {
    crate::{
        analyze::{AccountRole, PreflightIx, PreflightOverview},
        cpi_tree::{IxAccount, IxArg},
        ixname,
    },
    solana_transaction::versioned::VersionedTransaction,
};

const SYSTEM: &str = "11111111111111111111111111111111";
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const ATA: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const COMPUTE_BUDGET: &str = "ComputeBudget111111111111111111111111111111";

/// Solana's transaction size ceiling (bytes over the wire).
const MAX_TX_SIZE: usize = 1232;
/// Lamports per required signature.
const LAMPORTS_PER_SIGNATURE: u64 = 5_000;
/// The runtime's per-transaction compute ceiling, used to bound a defaulted
/// compute-unit limit when only a price was set.
const MAX_COMPUTE_UNITS: u64 = 1_400_000;
/// Default compute-unit limit granted per (non-ComputeBudget) instruction when
/// no SetComputeUnitLimit is present.
const DEFAULT_CU_PER_IX: u64 = 200_000;

fn short(a: &str) -> String {
    if a.len() > 12 {
        format!("{}…{}", &a[..4], &a[a.len() - 4..])
    } else {
        a.to_string()
    }
}

fn arg<'a>(args: &'a [IxArg], name: &str) -> Option<&'a str> {
    args.iter()
        .find(|a| a.name.eq_ignore_ascii_case(name))
        .map(|a| a.value.as_str())
}

fn lamports_to_sol(l: u64) -> String {
    let sol = l as f64 / 1e9;
    if sol >= 0.0001 {
        format!("{sol:.4} SOL")
    } else {
        format!("{l} lamports")
    }
}

fn account_addr(accounts: &[IxAccount], i: usize) -> String {
    accounts
        .get(i)
        .map(|a| short(&a.address))
        .unwrap_or_else(|| "?".into())
}

/// Per-program compute breakdown for a preflight simulation — fills
/// [`PreflightOverview::compute`] once the simulation has produced logs and a
/// total. Same attribution as the analyze view's Compute Units panel.
pub fn compute_breakdown(logs: &[String], total_cu: u64) -> Vec<crate::compute::CuUsage> {
    crate::compute::cu_from_logs(logs, total_cu)
}

/// The full base58 address at an instruction-account position (for identity
/// checks); [`account_addr`] gives the shortened display form.
fn full_addr(accounts: &[IxAccount], i: usize) -> &str {
    accounts.get(i).map(|a| a.address.as_str()).unwrap_or("")
}

/// Translate one decoded instruction into action lines and warnings. Only
/// well-understood native shapes produce output — an unknown instruction adds
/// nothing rather than guessing.
///
/// `fee_payer` and `wrap_targets` (accounts that get a SyncNative, i.e. wrapped
/// SOL) let the descriptions distinguish routine self-operations — wrapping
/// your own SOL, closing your own temp account with the rent returned to you —
/// from the genuinely alarming shapes (rent swept to *another* address, a
/// delegation, an authority change). Flagging routine swap plumbing as a
/// warning would be a false alarm, which on a safety feature is worse than
/// silence.
#[allow(clippy::too_many_arguments)]
fn describe(
    program: &str,
    name: Option<&str>,
    args: &[IxArg],
    accounts: &[IxAccount],
    fee_payer: &str,
    wrap_targets: &std::collections::HashSet<String>,
    actions: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let name = name.unwrap_or("");
    match (program, name) {
        (SYSTEM, "Transfer") => {
            let amt = arg(args, "lamports")
                .and_then(|v| v.parse::<u64>().ok())
                .map(lamports_to_sol)
                .unwrap_or_else(|| "SOL".into());
            // A transfer into an account that's about to be SyncNative'd is the
            // wrapped-SOL wrap step — your own SOL, not a payment out.
            if wrap_targets.contains(full_addr(accounts, 1)) {
                actions.push(format!("wraps {amt} (your own SOL, for the swap)"));
            } else {
                actions.push(format!(
                    "sends {amt} from {} to {}",
                    account_addr(accounts, 0),
                    account_addr(accounts, 1)
                ));
            }
        }
        (SYSTEM, "Create Account") | (SYSTEM, "CreateAccount") => {
            actions.push(format!("creates account {}", account_addr(accounts, 1)));
        }
        (SYSTEM, "Assign") => warnings.push(format!(
            "reassigns ownership of {} to another program",
            account_addr(accounts, 0)
        )),
        (TOKEN | TOKEN_2022, "Transfer" | "Transfer Checked" | "TransferChecked") => {
            let amt = arg(args, "amount").unwrap_or("tokens").to_string();
            actions.push(format!(
                "sends {amt} tokens from {} to {}",
                account_addr(accounts, 0),
                // TransferChecked inserts the mint at position 1.
                if name.contains("Checked") {
                    account_addr(accounts, 2)
                } else {
                    account_addr(accounts, 1)
                }
            ));
        }
        (TOKEN | TOKEN_2022, "Approve" | "Approve Checked" | "ApproveChecked") => {
            let amt = arg(args, "amount").unwrap_or("an amount of").to_string();
            warnings.push(format!(
                "delegates {amt} tokens of {} to {} — the delegate can move them without you",
                account_addr(accounts, 0),
                account_addr(accounts, if name.contains("Checked") { 2 } else { 1 })
            ));
        }
        (TOKEN | TOKEN_2022, "Set Authority" | "SetAuthority") => warnings.push(format!(
            "changes the authority of {} — control of the account moves",
            account_addr(accounts, 0)
        )),
        (TOKEN | TOKEN_2022, "Close Account" | "CloseAccount") => {
            // Closing your own token account with the rent returned to you is
            // routine (every wrapped-SOL swap ends this way). Only a close that
            // sends the rent to a *different* address is worth flagging.
            if full_addr(accounts, 1) == fee_payer {
                actions.push(format!(
                    "closes token account {} (rent returned to you)",
                    account_addr(accounts, 0)
                ));
            } else {
                warnings.push(format!(
                    "closes token account {} and sends its rent to {} — a different address",
                    account_addr(accounts, 0),
                    account_addr(accounts, 1)
                ));
            }
        }
        (TOKEN | TOKEN_2022, "Revoke") => actions.push(format!(
            "revokes the delegate on {}",
            account_addr(accounts, 0)
        )),
        (ATA, "Create" | "Create Idempotent") => {
            actions.push("creates a token account (≈0.002 SOL rent, refundable on close)".into());
        }
        _ => {}
    }
}

/// Build the pre-sign overview for a parsed unsigned transaction. All decoding
/// is local; the only RPC calls are the (cached) IDL fetches for non-native
/// programs, ALT resolution for v0 messages, and one balance lookup.
pub(crate) fn build_overview(
    tx: &VersionedTransaction,
    alt: (Vec<String>, Vec<String>),
    lookup: &mut dyn FnMut(&str) -> Option<serde_json::Value>,
    fee_payer_balance: Option<u64>,
) -> PreflightOverview {
    // Full runtime account ordering: static keys, then ALT writable, then
    // ALT readonly — instruction indexes point into this combined list.
    let mut keys: Vec<String> = tx
        .message
        .static_account_keys()
        .iter()
        .map(|k| k.to_string())
        .collect();
    let (alt_w, alt_r) = alt;
    keys.extend(alt_w);
    keys.extend(alt_r);

    let serialized_size = bincode::serialize(tx).map(|b| b.len()).unwrap_or(0);
    let n_sigs = tx.message.header().num_required_signatures as u64;
    let base_fee_lamports = n_sigs * LAMPORTS_PER_SIGNATURE;
    let fee_payer = keys.first().cloned().unwrap_or_default();

    // Decode every top-level instruction, and pick up the ComputeBudget
    // requests along the way. Descriptions come in a second pass, since a
    // Transfer's meaning depends on a SyncNative that appears *later*.
    let mut instructions = Vec::new();
    let mut wrap_targets: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut program_idx: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut cu_limit: Option<u64> = None;
    let mut cu_price_micro: Option<u64> = None;
    let mut non_cb_ix = 0u64;

    for (index, ix) in tx.message.instructions().iter().enumerate() {
        program_idx.insert(ix.program_id_index as usize);
        let program = keys
            .get(ix.program_id_index as usize)
            .cloned()
            .unwrap_or_default();
        let account_indexes: Vec<usize> = ix.accounts.iter().map(|&i| i as usize).collect();
        let (name, args, accounts) =
            ixname::enrich_with(lookup, &program, &ix.data, &account_indexes, &keys);

        if program == COMPUTE_BUDGET {
            // Tags: 2 = SetComputeUnitLimit(u32), 3 = SetComputeUnitPrice(u64).
            match ix.data.first() {
                Some(2) if ix.data.len() >= 5 => {
                    cu_limit = Some(u32::from_le_bytes(ix.data[1..5].try_into().unwrap()) as u64);
                }
                Some(3) if ix.data.len() >= 9 => {
                    cu_price_micro = Some(u64::from_le_bytes(ix.data[1..9].try_into().unwrap()));
                }
                _ => {}
            }
        } else {
            non_cb_ix += 1;
        }

        // SyncNative marks its account as a wrapped-SOL account being funded —
        // so a Transfer into it reads as a wrap, not a payment.
        if matches!(program.as_str(), TOKEN | TOKEN_2022) && name.as_deref() == Some("Sync Native")
        {
            if let Some(a) = accounts.first() {
                wrap_targets.insert(a.address.clone());
            }
        }

        instructions.push(PreflightIx {
            index,
            program,
            name,
            args,
            accounts,
        });
    }

    // Second pass: plain-English actions and danger flags, now that wrap targets
    // are known.
    let mut actions = Vec::new();
    let mut warnings = Vec::new();
    for ix in &instructions {
        describe(
            &ix.program,
            ix.name.as_deref(),
            &ix.args,
            &ix.accounts,
            &fee_payer,
            &wrap_targets,
            &mut actions,
            &mut warnings,
        );
    }

    // Priority fee = price (µ-lamports / CU) × CU limit. When no limit was
    // requested, the runtime defaults to 200k per instruction capped at 1.4M —
    // say so instead of silently guessing.
    let (priority_fee_lamports, priority_fee_note) = match cu_price_micro {
        Some(price) => {
            let (limit, note) = match cu_limit {
                Some(l) => (l, None),
                None => (
                    (non_cb_ix * DEFAULT_CU_PER_IX).min(MAX_COMPUTE_UNITS),
                    Some("no compute-unit limit was requested — priority fee estimated at the runtime default".to_string()),
                ),
            };
            ((price * limit).div_ceil(1_000_000), note)
        }
        None => (0, None),
    };

    let fee_payer_can_pay =
        fee_payer_balance.map(|b| b >= base_fee_lamports + priority_fee_lamports);

    // Account roles from the message header. `keys` is already in canonical
    // runtime order: static keys, then ALT-writable, then ALT-readonly.
    let header = tx.message.header();
    let n_static = tx.message.static_account_keys().len();
    let s = header.num_required_signatures as usize;
    let ro_signed = header.num_readonly_signed_accounts as usize;
    let ro_unsigned = header.num_readonly_unsigned_accounts as usize;
    let n_alt_writable = keys.len().saturating_sub(n_static)
        - tx.message
            .address_table_lookups()
            .map(|ls| ls.iter().map(|l| l.readonly_indexes.len()).sum())
            .unwrap_or(0)
            .min(keys.len().saturating_sub(n_static));
    let accounts: Vec<AccountRole> = keys
        .iter()
        .enumerate()
        .map(|(i, address)| {
            let (signer, writable) = if i < n_static {
                let signer = i < s;
                let writable = if i < s {
                    i < s - ro_signed // writable signers come before readonly signers
                } else {
                    i < n_static - ro_unsigned // writable unsigned before readonly unsigned
                };
                (signer, writable)
            } else {
                // ALT: writable block first, then readonly.
                (false, i < n_static + n_alt_writable)
            };
            AccountRole {
                address: address.clone(),
                signer,
                writable,
                program: program_idx.contains(&i),
                fee_payer: i == 0,
            }
        })
        .collect();

    PreflightOverview {
        serialized_size,
        max_size: MAX_TX_SIZE,
        base_fee_lamports,
        priority_fee_lamports,
        priority_fee_note,
        compute_unit_limit: cu_limit,
        fee_payer,
        fee_payer_balance,
        fee_payer_can_pay,
        instructions,
        accounts,
        compute: Vec::new(),
        actions,
        warnings,
    }
}

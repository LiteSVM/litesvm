//! Human names for instructions — the difference between a bare program id and
//! "Jupiter · Route V2". Native programs are decoded from their well-known
//! layouts; Anchor programs from their on-chain IDL's instruction discriminators.

use {
    crate::{
        cpi_tree::{IxAccount, IxArg},
        idl,
    },
    serde_json::Value,
    std::collections::HashMap,
};

const SYSTEM: &str = "11111111111111111111111111111111";
const COMPUTE_BUDGET: &str = "ComputeBudget111111111111111111111111111111";
const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const ATA: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const MEMO: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";
const MEMO_V1: &str = "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo";
const STAKE: &str = "Stake11111111111111111111111111111111111111";
const VOTE: &str = "Vote111111111111111111111111111111111111111";
const ALT: &str = "AddressLookupTab1e1111111111111111111111111";
const LOADER_UPGRADEABLE: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const RAYDIUM_AMM: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const SERUM_V3: &str = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";
const OPENBOOK_V1: &str = "srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX";

/// Little-endian u32 tag at the start of the data (the enum layout of the
/// bincode-serialised native programs: Stake, Vote, ALT, the loaders).
fn u32_tag(data: &[u8]) -> Option<u32> {
    data.get(..4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

fn stake_ix(data: &[u8]) -> Option<&'static str> {
    Some(match u32_tag(data)? {
        0 => "Initialize",
        1 => "Authorize",
        2 => "Delegate Stake",
        3 => "Split",
        4 => "Withdraw",
        5 => "Deactivate",
        6 => "Set Lockup",
        7 => "Merge",
        8 => "Authorize With Seed",
        9 => "Initialize Checked",
        10 => "Authorize Checked",
        11 => "Authorize Checked With Seed",
        12 => "Set Lockup Checked",
        13 => "Get Minimum Delegation",
        14 => "Deactivate Delinquent",
        15 => "Redelegate",
        16 => "Move Stake",
        17 => "Move Lamports",
        _ => return None,
    })
}

fn vote_ix(data: &[u8]) -> Option<&'static str> {
    Some(match u32_tag(data)? {
        0 => "Initialize Account",
        1 => "Authorize",
        2 => "Vote",
        3 => "Withdraw",
        4 => "Update Validator Identity",
        5 => "Update Commission",
        6 => "Vote Switch",
        7 => "Authorize Checked",
        8 => "Update Vote State",
        9 => "Update Vote State Switch",
        10 => "Authorize With Seed",
        11 => "Authorize Checked With Seed",
        12 => "Compact Update Vote State",
        13 => "Compact Update Vote State Switch",
        14 => "Tower Sync",
        15 => "Tower Sync Switch",
        _ => return None,
    })
}

fn alt_ix(data: &[u8]) -> Option<&'static str> {
    Some(match u32_tag(data)? {
        0 => "Create Lookup Table",
        1 => "Freeze Lookup Table",
        2 => "Extend Lookup Table",
        3 => "Deactivate Lookup Table",
        4 => "Close Lookup Table",
        _ => return None,
    })
}

fn loader_ix(data: &[u8]) -> Option<&'static str> {
    Some(match u32_tag(data)? {
        0 => "Initialize Buffer",
        1 => "Write",
        2 => "Deploy With Max Data Len",
        3 => "Upgrade",
        4 => "Set Authority",
        5 => "Close",
        6 => "Extend Program",
        7 => "Set Authority Checked",
        8 => "Migrate",
        _ => return None,
    })
}

/// Raydium AMM v4 — native, single-byte tag, no IDL anywhere.
fn raydium_amm_ix(data: &[u8]) -> Option<&'static str> {
    Some(match data.first()? {
        0 => "Initialize",
        1 => "Initialize2",
        2 => "Monitor Step",
        3 => "Deposit",
        4 => "Withdraw",
        5 => "Migrate To OpenBook",
        6 => "Set Params",
        7 => "Withdraw Pnl",
        8 => "Withdraw Srm",
        9 => "Swap Base In",
        10 => "Pre Initialize",
        11 => "Swap Base Out",
        12 => "Simulate Info",
        13 => "Admin Cancel Orders",
        14 => "Create Config Account",
        15 => "Update Config Account",
        _ => return None,
    })
}

/// Serum DEX v3 / OpenBook v1 — a version byte then a u32 tag.
fn serum_ix(data: &[u8]) -> Option<&'static str> {
    Some(match u32_tag(data.get(1..)?)? {
        0 => "Initialize Market",
        1 => "New Order",
        2 => "Match Orders",
        3 => "Consume Events",
        4 => "Cancel Order",
        5 => "Settle Funds",
        6 => "Cancel Order By Client Id",
        7 => "Disable Market",
        8 => "Sweep Fees",
        9 => "New Order V2",
        10 => "New Order V3",
        11 => "Cancel Order V2",
        12 => "Cancel Order By Client Id V2",
        13 => "Send Take",
        14 => "Close Open Orders",
        15 => "Init Open Orders",
        16 => "Prune",
        17 => "Consume Events Permissioned",
        18 => "Cancel Orders By Client Ids",
        19 => "Replace Order By Client Id",
        20 => "Replace Orders By Client Ids",
        _ => return None,
    })
}

/// Decode a Token-program instruction (Tokenkeg / Token-2022) by its leading tag.
fn token_ix(data: &[u8]) -> Option<&'static str> {
    Some(match data.first()? {
        0 => "Initialize Mint",
        1 => "Initialize Account",
        3 => "Transfer",
        4 => "Approve",
        5 => "Revoke",
        6 => "Set Authority",
        7 => "Mint To",
        8 => "Burn",
        9 => "Close Account",
        10 => "Freeze Account",
        11 => "Thaw Account",
        12 => "Transfer Checked",
        13 => "Approve Checked",
        14 => "Mint To Checked",
        15 => "Burn Checked",
        16 => "Initialize Account 2",
        17 => "Sync Native",
        18 => "Initialize Account 3",
        19 => "Initialize Multisig 2",
        20 => "Initialize Mint 2",
        21 => "Get Account Data Size",
        22 => "Initialize Immutable Owner",
        23 => "Amount To Ui Amount",
        24 => "Ui Amount To Amount",
        // Token-2022 only.
        25 => "Initialize Mint Close Authority",
        26 => "Transfer Fee Extension",
        27 => "Confidential Transfer Extension",
        28 => "Default Account State Extension",
        29 => "Reallocate",
        30 => "Memo Transfer Extension",
        31 => "Create Native Mint",
        32 => "Initialize Non Transferable Mint",
        33 => "Interest Bearing Mint Extension",
        34 => "Cpi Guard Extension",
        35 => "Initialize Permanent Delegate",
        36 => "Transfer Hook Extension",
        37 => "Confidential Transfer Fee Extension",
        38 => "Withdraw Excess Lamports",
        39 => "Metadata Pointer Extension",
        40 => "Group Pointer Extension",
        41 => "Group Member Pointer Extension",
        42 => "Confidential Mint Burn Extension",
        43 => "Scaled Ui Amount Extension",
        44 => "Pausable Extension",
        _ => return None,
    })
}

/// Decode a System-program instruction by its 4-byte little-endian tag.
fn system_ix(data: &[u8]) -> Option<&'static str> {
    let tag = u32::from_le_bytes(data.get(0..4)?.try_into().ok()?);
    Some(match tag {
        0 => "Create Account",
        1 => "Assign",
        2 => "Transfer",
        3 => "Create Account With Seed",
        4 => "Advance Nonce Account",
        5 => "Withdraw Nonce Account",
        6 => "Initialize Nonce Account",
        7 => "Authorize Nonce Account",
        8 => "Allocate",
        9 => "Allocate With Seed",
        10 => "Assign With Seed",
        11 => "Transfer With Seed",
        12 => "Upgrade Nonce Account",
        _ => return None,
    })
}

/// Decode a Compute-Budget instruction by its leading tag.
fn compute_budget_ix(data: &[u8]) -> Option<&'static str> {
    Some(match data.first()? {
        1 => "Request Heap Frame",
        2 => "Set Compute Unit Limit",
        3 => "Set Compute Unit Price",
        4 => "Set Loaded Accounts Data Size Limit",
        _ => return None,
    })
}

/// Turn an IDL instruction name (`routeV2`, `shared_accounts_route`) into a
/// display label (`Route V2`, `Shared Accounts Route`).
fn titleize(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        if ch == '_' || ch == '-' {
            out.push(' ');
            prev_lower = false;
            continue;
        }
        // Split camelCase (lower→UPPER) only; keep digits attached ("v2" → "V2").
        if ch.is_uppercase() && prev_lower {
            out.push(' ');
        }
        out.push(ch);
        prev_lower = ch.is_lowercase();
    }
    // Capitalize the first letter of each word.
    out.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Positional account role names for a native program's instruction, so the
/// call trace reads "Source / Destination / Authority", not bare addresses.
fn native_account_names(program: &str, data: &[u8]) -> Vec<&'static str> {
    let v = |s: &[&'static str]| s.to_vec();
    match program {
        TOKEN | TOKEN_2022 => match data.first() {
            Some(1) => v(&["Account", "Mint", "Owner", "Rent Sysvar"]),
            Some(3) => v(&["Source", "Destination", "Authority"]),
            Some(4) => v(&["Source", "Delegate", "Authority"]),
            Some(7) => v(&["Mint", "Destination", "Authority"]),
            Some(8) => v(&["Account", "Mint", "Authority"]),
            Some(9) => v(&["Account", "Destination", "Authority"]),
            Some(12) => v(&["Source", "Mint", "Destination", "Authority"]),
            Some(14) => v(&["Mint", "Destination", "Authority"]),
            Some(15) => v(&["Account", "Mint", "Authority"]),
            _ => vec![],
        },
        SYSTEM => match data
            .get(0..4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        {
            Some(0) => v(&["Funder", "New Account"]),
            Some(2) => v(&["From", "To"]),
            _ => vec![],
        },
        ATA => v(&[
            "Funder",
            "Associated Token Account",
            "Wallet",
            "Mint",
            "System Program",
            "Token Program",
        ]),
        _ => vec![],
    }
}

/// Decode a native instruction's key arguments (the amounts developers look for).
fn native_args(program: &str, data: &[u8]) -> Vec<IxArg> {
    let u64_at = |o: usize| {
        data.get(o..o + 8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    };
    let u32_at = |o: usize| {
        data.get(o..o + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    };
    let arg = |name: &str, ty: &str, v: u64| IxArg {
        name: name.into(),
        ty: ty.into(),
        value: v.to_string(),
    };
    match program {
        TOKEN | TOKEN_2022 => match data.first() {
            Some(3) | Some(4) | Some(7) | Some(8) => u64_at(1)
                .map(|a| vec![arg("amount", "u64", a)])
                .unwrap_or_default(),
            Some(12) | Some(13) | Some(14) | Some(15) => {
                let mut out = vec![];
                if let Some(a) = u64_at(1) {
                    out.push(arg("amount", "u64", a));
                }
                if let Some(&d) = data.get(9) {
                    out.push(arg("decimals", "u8", d as u64));
                }
                out
            }
            _ => vec![],
        },
        SYSTEM => match data
            .get(0..4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        {
            Some(2) => u64_at(4)
                .map(|l| vec![arg("lamports", "u64", l)])
                .unwrap_or_default(),
            _ => vec![],
        },
        COMPUTE_BUDGET => match data.first() {
            Some(2) => u32_at(1)
                .map(|u| vec![arg("units", "u32", u as u64)])
                .unwrap_or_default(),
            Some(3) => u64_at(1)
                .map(|p| vec![arg("micro_lamports", "u64", p)])
                .unwrap_or_default(),
            Some(4) => u32_at(1)
                .map(|b| vec![arg("bytes", "u32", b as u64)])
                .unwrap_or_default(),
            _ => vec![],
        },
        _ => vec![],
    }
}

/// [`enrich`] without RPC: resolves IDLs from an already-loaded map only. Used
/// by the debugger and by fixture replays, where every relevant IDL was
/// captured up front.
pub(crate) fn enrich_offline(
    idls: &HashMap<String, Value>,
    program: &str,
    data: &[u8],
    account_indexes: &[usize],
    account_keys: &[String],
) -> (Option<String>, Vec<IxArg>, Vec<IxAccount>) {
    let mut lookup = |p: &str| idls.get(p).cloned();
    enrich_with(&mut lookup, program, data, account_indexes, account_keys)
}

pub(crate) fn enrich_with(
    lookup: &mut dyn FnMut(&str) -> Option<Value>,
    program: &str,
    data: &[u8],
    account_indexes: &[usize],
    account_keys: &[String],
) -> (Option<String>, Vec<IxArg>, Vec<IxAccount>) {
    let addresses: Vec<String> = account_indexes
        .iter()
        .map(|&i| account_keys.get(i).cloned().unwrap_or_default())
        .collect();

    let is_native = matches!(
        program,
        TOKEN
            | TOKEN_2022
            | SYSTEM
            | COMPUTE_BUDGET
            | ATA
            | MEMO
            | MEMO_V1
            | STAKE
            | VOTE
            | ALT
            | LOADER_UPGRADEABLE
            | RAYDIUM_AMM
            | SERUM_V3
            | OPENBOOK_V1
    );

    // Anchor's `emit_cpi!` invokes the program itself with the event bytes,
    // prefixed by a fixed 8-byte "event CPI" discriminator. Name it so the tree
    // says what it is instead of `?`.
    const ANCHOR_EVENT_CPI: [u8; 8] = [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];
    if data.len() >= 8 && data[..8] == ANCHOR_EVENT_CPI {
        let accounts = addresses
            .into_iter()
            .enumerate()
            .map(|(i, address)| IxAccount {
                name: (i == 0).then(|| "Event Authority".to_string()),
                address,
            })
            .collect();
        return (Some("Emit Event".into()), vec![], accounts);
    }

    // Name + args + per-position account names.
    let (name, args, names): (Option<String>, Vec<IxArg>, Vec<Option<String>>) = if is_native {
        let name = match program {
            TOKEN | TOKEN_2022 => token_ix(data).map(String::from),
            SYSTEM => system_ix(data).map(String::from),
            COMPUTE_BUDGET => compute_budget_ix(data).map(String::from),
            // ATA discriminants: (empty data or) 0 = Create, 1 = CreateIdempotent,
            // 2 = RecoverNested.
            ATA => Some(
                match data.first() {
                    Some(1) => "Create Idempotent",
                    Some(2) => "Recover Nested",
                    _ => "Create",
                }
                .into(),
            ),
            STAKE => stake_ix(data).map(String::from),
            VOTE => vote_ix(data).map(String::from),
            ALT => alt_ix(data).map(String::from),
            LOADER_UPGRADEABLE => loader_ix(data).map(String::from),
            RAYDIUM_AMM => raydium_amm_ix(data).map(String::from),
            SERUM_V3 | OPENBOOK_V1 => serum_ix(data).map(String::from),
            _ => Some("Memo".into()),
        };
        let names = native_account_names(program, data)
            .into_iter()
            .map(|n| Some(n.to_string()))
            .collect();
        (name, native_args(program, data), names)
    } else {
        // Anchor program: resolve (and cache) its IDL, then match by discriminator.
        let idl = lookup(program);
        match idl.as_ref().and_then(|i| idl::find_ix(i, data)) {
            Some(ix) => {
                let name = ix.name.as_deref().map(titleize);
                let args = idl::decode_ix_args(&ix, data)
                    .into_iter()
                    .map(|(name, ty, value)| IxArg { name, ty, value })
                    .collect();
                // An Anchor account *group* (nested `accounts`) occupies one
                // position per leaf in the real account list, so flatten to
                // leaves with the group name as a prefix — otherwise every
                // account after a group is named after the wrong entry.
                fn leaves(
                    nodes: &[crate::idl_model::AccountNode],
                    prefix: &str,
                    out: &mut Vec<Option<String>>,
                ) {
                    for n in nodes {
                        let own = n.name.as_deref().map(titleize);
                        match &n.children {
                            Some(kids) => {
                                let p = match &own {
                                    Some(g) => format!("{prefix}{g} · "),
                                    None => prefix.to_string(),
                                };
                                leaves(kids, &p, out);
                            }
                            None => out.push(own.map(|o| format!("{prefix}{o}"))),
                        }
                    }
                }
                let mut named: Vec<Option<String>> = Vec::new();
                leaves(&ix.accounts, "", &mut named);
                (name, args, named)
            }
            None => (None, vec![], vec![]),
        }
    };

    // Pair each account with its role name; extras beyond the named list are the
    // program's "remaining accounts" (Anchor's variadic tail).
    let named_len = names.len();
    let accounts = addresses
        .into_iter()
        .enumerate()
        .map(|(i, address)| {
            let name = names.get(i).cloned().flatten().or_else(|| {
                (!is_native && !names.is_empty() && i >= named_len)
                    .then(|| format!("Remaining Account #{}", i - named_len + 1))
            });
            IxAccount { name, address }
        })
        .collect();

    (name, args, accounts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titleize_handles_camel_snake_and_digits() {
        assert_eq!(titleize("routeV2"), "Route V2");
        assert_eq!(titleize("route_v2"), "Route V2");
        assert_eq!(titleize("swapV2"), "Swap V2");
        assert_eq!(titleize("shared_accounts_route"), "Shared Accounts Route");
        assert_eq!(titleize("swap"), "Swap");
    }

    #[test]
    fn native_program_instructions_decode() {
        assert_eq!(token_ix(&[12]), Some("Transfer Checked"));
        assert_eq!(token_ix(&[3]), Some("Transfer"));
        assert_eq!(system_ix(&[2, 0, 0, 0]), Some("Transfer"));
        assert_eq!(compute_budget_ix(&[2]), Some("Set Compute Unit Limit"));
        assert_eq!(
            compute_budget_ix(&[4]),
            Some("Set Loaded Accounts Data Size Limit")
        );
        assert_eq!(token_ix(&[]), None);
    }

    #[test]
    fn native_account_layouts() {
        // Token TransferChecked → Source / Mint / Destination / Authority.
        assert_eq!(
            native_account_names(TOKEN, &[12]),
            vec!["Source", "Mint", "Destination", "Authority"]
        );
        // System Transfer → From / To.
        assert_eq!(
            native_account_names(SYSTEM, &[2, 0, 0, 0]),
            vec!["From", "To"]
        );
    }

    #[test]
    fn native_argument_decoding() {
        // Token Transfer: amount u64 at offset 1.
        let mut d = vec![3u8];
        d.extend_from_slice(&1_000_000u64.to_le_bytes());
        let a = native_args(TOKEN, &d);
        assert_eq!(
            (a[0].name.as_str(), a[0].value.as_str()),
            ("amount", "1000000")
        );
        // Compute Budget SetComputeUnitLimit: u32 at offset 1.
        let mut c = vec![2u8];
        c.extend_from_slice(&169_062u32.to_le_bytes());
        assert_eq!(native_args(COMPUTE_BUDGET, &c)[0].value, "169062");
    }
}

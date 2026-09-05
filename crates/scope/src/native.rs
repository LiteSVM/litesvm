//! Instruction names, argument decoding and account roles for the native and
//! SPL programs that publish no IDL: System, Compute Budget, Token, Token-2022,
//! Associated Token and Memo.

use crate::DecodedArg;

pub(crate) const SYSTEM: &str = "11111111111111111111111111111111";
pub(crate) const COMPUTE_BUDGET: &str = "ComputeBudget111111111111111111111111111111";
pub(crate) const TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub(crate) const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
pub(crate) const ATA: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const MEMO: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";
const MEMO_V1: &str = "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo";

/// Anchor's `emit_cpi!` invokes the program itself with the event bytes,
/// prefixed by this fixed 8-byte "event CPI" discriminator.
pub(crate) const ANCHOR_EVENT_CPI: [u8; 8] = [0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d];

pub(crate) fn is_native(program: &str) -> bool {
    matches!(
        program,
        TOKEN | TOKEN_2022 | SYSTEM | COMPUTE_BUDGET | ATA | MEMO | MEMO_V1
    )
}

fn token_ix(data: &[u8]) -> Option<&'static str> {
    Some(match data.first()? {
        0 => "Initialize Mint",
        1 => "Initialize Account",
        2 => "Initialize Multisig",
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
        _ => return None,
    })
}

fn system_ix(data: &[u8]) -> Option<&'static str> {
    Some(match u32::from_le_bytes(data.get(0..4)?.try_into().ok()?) {
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

fn compute_budget_ix(data: &[u8]) -> Option<&'static str> {
    Some(match data.first()? {
        1 => "Request Heap Frame",
        2 => "Set Compute Unit Limit",
        3 => "Set Compute Unit Price",
        4 => "Set Loaded Accounts Data Size Limit",
        _ => return None,
    })
}

/// `routeV2` / `shared_accounts_route` → `Route V2` / `Shared Accounts Route`.
pub(crate) fn titleize(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        if ch == '_' || ch == '-' {
            out.push(' ');
            prev_lower = false;
            continue;
        }
        if ch.is_uppercase() && prev_lower {
            out.push(' ');
        }
        out.push(ch);
        prev_lower = ch.is_lowercase();
    }
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

/// Positional account roles for a native instruction.
fn account_names(program: &str, data: &[u8]) -> Vec<&'static str> {
    let v = |s: &[&'static str]| s.to_vec();
    match program {
        TOKEN | TOKEN_2022 => match data.first() {
            Some(0) => v(&["Mint", "Rent Sysvar"]),
            Some(1) => v(&["Account", "Mint", "Owner", "Rent Sysvar"]),
            Some(3) => v(&["Source", "Destination", "Authority"]),
            Some(4) => v(&["Source", "Delegate", "Authority"]),
            Some(5) => v(&["Source", "Authority"]),
            Some(6) => v(&["Account", "Authority"]),
            Some(7) => v(&["Mint", "Destination", "Authority"]),
            Some(8) => v(&["Account", "Mint", "Authority"]),
            Some(9) => v(&["Account", "Destination", "Authority"]),
            Some(10) | Some(11) => v(&["Account", "Mint", "Authority"]),
            Some(12) => v(&["Source", "Mint", "Destination", "Authority"]),
            Some(13) => v(&["Source", "Mint", "Delegate", "Authority"]),
            Some(14) => v(&["Mint", "Destination", "Authority"]),
            Some(15) => v(&["Account", "Mint", "Authority"]),
            Some(17) => v(&["Account"]),
            Some(18) | Some(16) => v(&["Account", "Mint"]),
            Some(20) => v(&["Mint"]),
            Some(21) => v(&["Mint"]),
            Some(22) => v(&["Account"]),
            _ => vec![],
        },
        SYSTEM => match data
            .get(0..4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        {
            Some(0) => v(&["Funder", "New Account"]),
            Some(1) => v(&["Account"]),
            Some(2) => v(&["From", "To"]),
            Some(3) => v(&["Funder", "New Account", "Base"]),
            Some(4) => v(&["Nonce Account", "Recent Blockhashes Sysvar", "Authority"]),
            Some(5) => v(&[
                "Nonce Account",
                "Recipient",
                "Recent Blockhashes Sysvar",
                "Rent Sysvar",
                "Authority",
            ]),
            Some(6) => v(&["Nonce Account", "Recent Blockhashes Sysvar", "Rent Sysvar"]),
            Some(7) => v(&["Nonce Account", "Authority"]),
            Some(8) => v(&["Account"]),
            Some(11) => v(&["From", "Base", "To"]),
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

/// The amounts and limits developers look for in native instructions.
fn args(program: &str, data: &[u8]) -> Vec<DecodedArg> {
    let u64_at = |o: usize| {
        data.get(o..o + 8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    };
    let u32_at = |o: usize| {
        data.get(o..o + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    };
    let arg = |name: &str, ty: &str, v: u64| DecodedArg {
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
            Some(0) | Some(20) => data
                .get(1)
                .map(|&d| vec![arg("decimals", "u8", d as u64)])
                .unwrap_or_default(),
            _ => vec![],
        },
        SYSTEM => match data
            .get(0..4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        {
            Some(0) => {
                let mut out = vec![];
                if let Some(l) = u64_at(4) {
                    out.push(arg("lamports", "u64", l));
                }
                if let Some(s) = u64_at(12) {
                    out.push(arg("space", "u64", s));
                }
                out
            }
            Some(2) => u64_at(4)
                .map(|l| vec![arg("lamports", "u64", l)])
                .unwrap_or_default(),
            Some(8) => u64_at(4)
                .map(|s| vec![arg("space", "u64", s)])
                .unwrap_or_default(),
            _ => vec![],
        },
        COMPUTE_BUDGET => match data.first() {
            Some(1) => u32_at(1)
                .map(|b| vec![arg("bytes", "u32", b as u64)])
                .unwrap_or_default(),
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

/// Name, arguments and positional account roles of a native instruction, or
/// `None` when `program` is not a native program this module knows.
pub(crate) fn decode(
    program: &str,
    data: &[u8],
) -> Option<(Option<String>, Vec<DecodedArg>, Vec<&'static str>)> {
    if !is_native(program) {
        return None;
    }
    let name = match program {
        TOKEN | TOKEN_2022 => token_ix(data).map(String::from),
        SYSTEM => system_ix(data).map(String::from),
        COMPUTE_BUDGET => compute_budget_ix(data).map(String::from),
        // ATA discriminants: (empty data or) 0 = Create, 1 = CreateIdempotent, 2 = RecoverNested.
        ATA => Some(
            match data.first() {
                Some(1) => "Create Idempotent",
                Some(2) => "Recover Nested",
                _ => "Create",
            }
            .into(),
        ),
        _ => Some("Memo".into()),
    };
    Some((name, args(program, data), account_names(program, data)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titleize_handles_camel_snake_and_digits() {
        assert_eq!(titleize("routeV2"), "Route V2");
        assert_eq!(titleize("route_v2"), "Route V2");
        assert_eq!(titleize("shared_accounts_route"), "Shared Accounts Route");
        assert_eq!(titleize("swap"), "Swap");
    }

    #[test]
    fn native_instructions_decode() {
        assert_eq!(token_ix(&[12]), Some("Transfer Checked"));
        assert_eq!(system_ix(&[2, 0, 0, 0]), Some("Transfer"));
        assert_eq!(
            compute_budget_ix(&[4]),
            Some("Set Loaded Accounts Data Size Limit")
        );
        assert_eq!(token_ix(&[]), None);
        assert_eq!(
            account_names(TOKEN, &[12]),
            vec!["Source", "Mint", "Destination", "Authority"]
        );
        assert_eq!(account_names(SYSTEM, &[2, 0, 0, 0]), vec!["From", "To"]);
        let mut d = vec![3u8];
        d.extend_from_slice(&1_000_000u64.to_le_bytes());
        let a = args(TOKEN, &d);
        assert_eq!(
            (a[0].name.as_str(), a[0].value.as_str()),
            ("amount", "1000000")
        );
        let mut c = vec![2u8];
        c.extend_from_slice(&169_062u32.to_le_bytes());
        assert_eq!(args(COMPUTE_BUDGET, &c)[0].value, "169062");
        assert!(decode("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4", &[]).is_none());
    }
}

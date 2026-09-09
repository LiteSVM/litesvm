//! Balance changes (SOL and SPL tokens) for every account a transaction touched.

use {crate::utils::resolve_account_keys, serde_json::Value};

/// A change in an account's SOL (lamport) balance.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BalanceChange {
    /// The account whose balance changed.
    pub address: String,
    /// Signed lamport change (post − pre).
    pub delta: i64,
    /// Post-transaction lamport balance (what an explorer shows alongside the change).
    pub post: u64,
}

/// A change in an SPL token account's balance, in raw base units (client
/// divides by 10^decimals for display).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TokenChange {
    /// The token account whose balance changed.
    pub address: String,
    /// The token account's owner (the wallet).
    pub owner: String,
    /// The token's mint address.
    pub mint: String,
    /// The mint's decimals (for converting raw units to display units).
    pub decimals: u8,
    /// Signed change in raw base units, as a string (can exceed i64).
    pub delta_raw: String,
    /// Post-transaction balance in raw base units, as a string.
    pub post_raw: String,
}

/// SPL token balance changes, computed from meta.pre/postTokenBalances.
///
/// The two arrays are keyed by `accountIndex`; we pair them up, compute the
/// signed delta per account, and drop anything that didn't move.
pub(crate) fn token_diffs(tx: &Value) -> Vec<TokenChange> {
    let account_keys = resolve_account_keys(tx);
    let empty = vec![];
    let pre = tx["meta"]["preTokenBalances"].as_array().unwrap_or(&empty);
    let post = tx["meta"]["postTokenBalances"].as_array().unwrap_or(&empty);

    // index -> raw amount (i128) for pre and post
    let raw = |entry: &Value| -> i128 {
        entry["uiTokenAmount"]["amount"]
            .as_str()
            .and_then(|s| s.parse::<i128>().ok())
            .unwrap_or(0)
    };
    let idx = |entry: &Value| entry["accountIndex"].as_u64().unwrap_or(u64::MAX) as usize;

    let mut pre_map: std::collections::HashMap<usize, i128> = std::collections::HashMap::new();
    for e in pre {
        pre_map.insert(idx(e), raw(e));
    }

    let mut out: Vec<TokenChange> = Vec::new();
    let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for e in post {
        let i = idx(e);
        // Skip an entry with no resolvable account index rather than emitting a
        // row with an empty address (a malformed/missing `accountIndex`).
        if i >= account_keys.len() {
            continue;
        }
        seen.insert(i);
        let post_raw = raw(e);
        let pre_raw = pre_map.get(&i).copied().unwrap_or(0);
        let delta = post_raw - pre_raw;
        if delta == 0 {
            continue;
        }
        out.push(TokenChange {
            address: account_keys.get(i).cloned().unwrap_or_default(),
            owner: e["owner"].as_str().unwrap_or_default().to_string(),
            mint: e["mint"].as_str().unwrap_or_default().to_string(),
            decimals: e["uiTokenAmount"]["decimals"].as_u64().unwrap_or(0) as u8,
            delta_raw: delta.to_string(),
            post_raw: post_raw.to_string(),
        });
    }
    // Accounts present in pre but not post (fully drained/closed).
    for e in pre {
        let i = idx(e);
        if i >= account_keys.len() || seen.contains(&i) {
            continue;
        }
        let pre_raw = raw(e);
        if pre_raw == 0 {
            continue;
        }
        out.push(TokenChange {
            address: account_keys.get(i).cloned().unwrap_or_default(),
            owner: e["owner"].as_str().unwrap_or_default().to_string(),
            mint: e["mint"].as_str().unwrap_or_default().to_string(),
            decimals: e["uiTokenAmount"]["decimals"].as_u64().unwrap_or(0) as u8,
            delta_raw: (-pre_raw).to_string(),
            post_raw: "0".to_string(),
        });
    }
    out
}

/// The lamport balance change for every account that changed.
pub(crate) fn account_diffs(tx: &Value) -> Vec<BalanceChange> {
    let empty = vec![];
    let pre = tx["meta"]["preBalances"].as_array().unwrap_or(&empty);
    let post = tx["meta"]["postBalances"].as_array().unwrap_or(&empty);
    let account_keys = resolve_account_keys(tx);

    let mut balance_change: Vec<BalanceChange> = Vec::new();

    for i in 0..pre.len().min(post.len()).min(account_keys.len()) {
        let pre_balance = pre[i].as_u64().unwrap_or(0);
        let post_balance = post[i].as_u64().unwrap_or(0);
        let delta = post_balance as i64 - pre_balance as i64;

        if delta != 0 {
            balance_change.push(BalanceChange {
                address: account_keys[i].clone(),
                delta,
                post: post_balance,
            });
        }
    }

    balance_change
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    fn tx() -> Value {
        json!({
            "transaction": { "message": { "accountKeys": ["A", "B", "C"] } },
            "meta": {
                "preBalances":  [100, 50, 5],
                "postBalances": [90, 50, 15],
                "preTokenBalances": [
                    { "accountIndex": 1, "mint": "M", "owner": "O",
                      "uiTokenAmount": { "amount": "1000", "decimals": 6 } },
                    { "accountIndex": 2, "mint": "M", "owner": "O2",
                      "uiTokenAmount": { "amount": "7", "decimals": 6 } }
                ],
                "postTokenBalances": [
                    { "accountIndex": 1, "mint": "M", "owner": "O",
                      "uiTokenAmount": { "amount": "400", "decimals": 6 } }
                ]
            }
        })
    }

    #[test]
    fn lamport_deltas_skip_unchanged_accounts() {
        let d = account_diffs(&tx());
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].address.as_str(), d[0].delta), ("A", -10));
        assert_eq!((d[1].address.as_str(), d[1].delta), ("C", 10));
    }

    #[test]
    fn token_deltas_include_drained_accounts() {
        let d = token_diffs(&tx());
        assert_eq!(d.len(), 2);
        assert_eq!(
            (d[0].address.as_str(), d[0].delta_raw.as_str()),
            ("B", "-600")
        );
        // Present pre but not post: fully drained, delta is the whole balance.
        assert_eq!(
            (
                d[1].address.as_str(),
                d[1].delta_raw.as_str(),
                d[1].post_raw.as_str()
            ),
            ("C", "-7", "0")
        );
    }

    #[test]
    fn tolerates_missing_meta() {
        assert!(account_diffs(&json!({})).is_empty());
        assert!(token_diffs(&json!({})).is_empty());
    }
}

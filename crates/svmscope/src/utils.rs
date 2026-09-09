use {
    crate::error::{Error, Result},
    serde_json::Value,
};

/// Full account list for a transaction: static keys, then accounts loaded
/// from Address Lookup Tables (writable, then readonly).
///
/// Missing `loadedAddresses` is normal (legacy / simple transactions have no
/// lookup tables), so we simply skip it — it is not an error.
pub(crate) fn resolve_account_keys(tx: &Value) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();

    let mut push_all = |v: &Value| {
        if let Some(arr) = v.as_array() {
            keys.extend(arr.iter().filter_map(|k| k.as_str().map(String::from)));
        }
    };
    push_all(&tx["transaction"]["message"]["accountKeys"]);
    push_all(&tx["meta"]["loadedAddresses"]["writable"]);
    push_all(&tx["meta"]["loadedAddresses"]["readonly"]);

    keys
}

pub(crate) fn camel_to_snake(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    for (index, character) in name.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index != 0 {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}

/// Decode a hex string, tolerating `0x`, spaces, and underscores.
pub(crate) fn hex_decode(s: &str) -> Result<Vec<u8>> {
    let s = s.trim().trim_start_matches("0x").replace([' ', '_'], "");
    // Guard ASCII before byte-slicing below: a multi-byte char would otherwise
    // pass the even-length check and panic on a non-char-boundary slice.
    if s.is_empty() || !s.len().is_multiple_of(2) || !s.is_ascii() {
        return Err(Error::InvalidSpec(
            "hex bytes must be a non-empty, even-length hex string".into(),
        ));
    }
    let bytes = s.as_bytes();
    (0..bytes.len())
        .step_by(2)
        .map(|i| {
            let pair = std::str::from_utf8(&bytes[i..i + 2]).expect("ascii checked above");
            u8::from_str_radix(pair, 16)
                .map_err(|_| Error::InvalidSpec(format!("invalid hex: {s}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn static_then_writable_then_readonly() {
        let tx = json!({
            "transaction": { "message": { "accountKeys": ["A", "B"] } },
            "meta": { "loadedAddresses": { "writable": ["W"], "readonly": ["R"] } }
        });
        assert_eq!(resolve_account_keys(&tx), vec!["A", "B", "W", "R"]);
    }

    #[test]
    fn legacy_transaction_without_lookup_tables() {
        let tx = json!({
            "transaction": { "message": { "accountKeys": ["A"] } },
            "meta": {}
        });
        assert_eq!(resolve_account_keys(&tx), vec!["A"]);
    }

    #[test]
    fn malformed_input_yields_empty_list() {
        assert!(resolve_account_keys(&json!({})).is_empty());
    }
}

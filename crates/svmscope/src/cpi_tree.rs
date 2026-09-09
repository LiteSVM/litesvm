//! Builds the CPI (cross-program invocation) call tree of a transaction.

use {crate::utils::resolve_account_keys, serde_json::Value};

/// One account an instruction touches, with its IDL role name where known.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IxAccount {
    /// The account's IDL role name (e.g. "authority"), where known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The account's address, as base58.
    pub address: String,
}

/// One decoded instruction argument.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IxArg {
    /// The argument's IDL name.
    pub name: String,
    /// The argument's type label (e.g. "u64").
    #[serde(rename = "type")]
    pub ty: String,
    /// The decoded value, formatted for display.
    pub value: String,
}

/// One instruction in the transaction's cross-program invocation tree.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CpiEntry {
    /// Zero-based position among the top-level instructions.
    pub index: usize,
    /// The invoked program's address, as base58.
    pub program: String,
    /// Invocation depth: 1 = top-level, 2+ = invoked by another program.
    pub stack_height: u64,
    /// Decoded instruction name (e.g. "Route V2"), filled in by `analyze` where an
    /// IDL or a known native layout lets us name it. `None` = couldn't decode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The instruction's accounts, named from the IDL / native layout where known.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<IxAccount>,
    /// Decoded instruction arguments (name/type/value).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<IxArg>,
    /// First eight bytes of the instruction data, hex — the discriminator a
    /// program introspecting the transaction through the Instructions sysvar
    /// would match on. `None` for empty data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<String>,
    /// True when the instruction is handed the Instructions sysvar
    /// (`Sysvar1nstructions1111111111111111111111111`): the program reads the
    /// other instructions of this transaction — flash-loan repay checks,
    /// precompile signature checks, guards against sandwiching.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub introspects: bool,
    /// Compute units this invocation consumed, from the `Program X consumed N of
    /// M compute units` log line. `None` for programs that log no such line
    /// (the builtins) or when the logs were truncated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_units: Option<u64>,
    /// Raw instruction data, kept only so `analyze` can decode it; never serialized.
    #[serde(skip)]
    pub(crate) data: Vec<u8>,
    /// This instruction's account indexes into the resolved account list; used by
    /// `analyze` to resolve addresses, never serialized.
    #[serde(skip)]
    pub(crate) account_indexes: Vec<usize>,
}

/// Programs the runtime executes natively as precompiles. They enter no VM,
/// emit no `Program … invoke` log line, and never reach the instruction
/// observer, so anything that pairs message instructions with logged or
/// observed execution must skip them.
pub(crate) fn is_precompile(program: &str) -> bool {
    matches!(
        program,
        "Ed25519SigVerify111111111111111111111111111"
            | "KeccakSecp256k11111111111111111111111111111"
            | "Secp256r1SigVerify1111111111111111111111111"
    )
}

/// The Instructions sysvar: a program given this account reads the
/// transaction's other instructions.
pub(crate) const INSTRUCTIONS_SYSVAR: &str = "Sysvar1nstructions1111111111111111111111111";

/// Mark every instruction that is handed the Instructions sysvar. Call after
/// the accounts are resolved to addresses.
pub(crate) fn mark_introspection(tree: &mut [CpiEntry]) {
    for e in tree.iter_mut() {
        e.introspects = e.accounts.iter().any(|a| a.address == INSTRUCTIONS_SYSVAR);
    }
}

/// Attach per-invocation compute from the transaction logs. The `invoke`
/// lines appear in exactly the order the tree lists instructions, so the two
/// are walked together; pairing stops at the first program mismatch (truncated
/// logs) rather than guessing.
pub(crate) fn attach_compute(tree: &mut [CpiEntry], logs: &[String]) {
    let spans = crate::cpi_tree::spans_from_logs(logs, 0);
    for (entry, span) in tree.iter_mut().zip(spans.iter()) {
        if span.program != entry.program {
            break;
        }
        entry.compute_units = span.cu_consumed;
    }
}

/// Build the CPI call tree as a flat list; nesting is carried by `stack_height`
/// (1 = top-level instruction, 2 = a CPI, 3 = a nested CPI, ...).
pub(crate) fn build_cpi_tree(tx: &Value) -> Vec<CpiEntry> {
    let empty = vec![];
    let instructions = tx["transaction"]["message"]["instructions"]
        .as_array()
        .unwrap_or(&empty);

    // Full account list, including accounts loaded from Address Lookup Tables.
    let account_keys = resolve_account_keys(tx);
    let program_at = |ix: &Value| -> Option<String> {
        let i = ix["programIdIndex"].as_u64()? as usize;
        account_keys.get(i).cloned()
    };

    // Instruction data is base58 in `json` encoding; keep the raw bytes so the
    // caller can decode an instruction name from them.
    let data_of = |ix: &Value| -> Vec<u8> {
        ix["data"]
            .as_str()
            .and_then(|s| bs58::decode(s).into_vec().ok())
            .unwrap_or_default()
    };
    let accts_of = |ix: &Value| -> Vec<usize> {
        ix["accounts"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|i| i.as_u64().map(|v| v as usize))
                    .collect()
            })
            .unwrap_or_default()
    };
    let entry = |index, program, stack_height, ix: &Value| CpiEntry {
        discriminator: {
            let d = data_of(ix);
            (!d.is_empty()).then(|| d.iter().take(8).map(|b| format!("{b:02x}")).collect())
        },
        introspects: false,
        compute_units: None,
        index,
        program,
        stack_height,
        name: None,
        accounts: vec![],
        args: vec![],
        data: data_of(ix),
        account_indexes: accts_of(ix),
    };

    let mut entries: Vec<CpiEntry> = Vec::new();

    for (index, ix) in instructions.iter().enumerate() {
        let Some(program) = program_at(ix) else {
            continue;
        };

        // Top-level instruction.
        entries.push(entry(index, program, 1, ix));

        // No inner group just means this instruction made no CPIs — that's normal.
        let my_group = tx["meta"]["innerInstructions"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .find(|group| group["index"].as_u64() == Some(index as u64));

        if let Some(my_group) = my_group {
            for inner in my_group["instructions"].as_array().unwrap_or(&empty) {
                let Some(program) = program_at(inner) else {
                    continue;
                };
                // Old transactions (pre-v1.14.6 meta) report no stackHeight; a
                // direct CPI (depth 2) is the faithful default there.
                let stack_height = inner["stackHeight"].as_u64().unwrap_or(2);
                entries.push(entry(index, program, stack_height, inner));
            }
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    fn tx(inner: Value) -> Value {
        json!({
            "transaction": { "message": {
                "accountKeys": ["Payer111", "ProgA", "ProgB"],
                "instructions": [{ "programIdIndex": 1 }]
            }},
            "meta": { "innerInstructions": inner }
        })
    }

    #[test]
    fn builds_tree_with_stack_heights() {
        let t = tx(json!([{ "index": 0, "instructions": [
            { "programIdIndex": 2, "stackHeight": 2 },
            { "programIdIndex": 1, "stackHeight": 3 }
        ]}]));
        let tree = build_cpi_tree(&t);
        assert_eq!(tree.len(), 3);
        assert_eq!(
            (tree[0].program.as_str(), tree[0].stack_height),
            ("ProgA", 1)
        );
        assert_eq!(
            (tree[1].program.as_str(), tree[1].stack_height),
            ("ProgB", 2)
        );
        assert_eq!(
            (tree[2].program.as_str(), tree[2].stack_height),
            ("ProgA", 3)
        );
    }

    #[test]
    fn missing_stack_height_defaults_to_direct_cpi() {
        let t = tx(json!([{ "index": 0, "instructions": [{ "programIdIndex": 2 }] }]));
        let tree = build_cpi_tree(&t);
        assert_eq!(tree[1].stack_height, 2);
    }

    #[test]
    fn tolerates_missing_meta_and_instructions() {
        assert!(build_cpi_tree(&json!({})).is_empty());
        let no_inner = json!({
            "transaction": { "message": {
                "accountKeys": ["Payer111", "ProgA"],
                "instructions": [{ "programIdIndex": 1 }]
            }},
            "meta": {}
        });
        let tree = build_cpi_tree(&no_inner);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].stack_height, 1);
    }
}

/// What the runtime says about one invocation, recovered from its log lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LogSpan {
    pub program: String,
    pub depth: u8,
    pub start: usize,
    pub end: usize,
    pub cu_consumed: Option<u64>,
    pub success: bool,
    /// The `Program X failed: …` tail, when it failed.
    pub failure: Option<String>,
}

/// Walk `logs[from..]` and recover every invocation in pre-order. The runtime
/// emits exactly one `Program <id> invoke [d]` per invocation and closes it with
/// `Program <id> success` or `Program <id> failed: …`, so a stack reproduces the
/// tree. Indices are absolute into `logs`.
pub(crate) fn spans_from_logs(logs: &[String], from: usize) -> Vec<LogSpan> {
    let mut spans: Vec<LogSpan> = Vec::new();
    let mut stack: Vec<usize> = Vec::new(); // indexes into `spans`
    for (i, line) in logs.iter().enumerate().skip(from) {
        let Some(rest) = line.strip_prefix("Program ") else {
            continue;
        };
        // `Program log:` / `data:` / `return:` carry program-chosen payloads;
        // a program that logs the text " invoke [1]" must not open a span.
        if rest.starts_with("log: ") || rest.starts_with("data: ") || rest.starts_with("return: ") {
            continue;
        }
        if let Some(pos) = rest.find(" invoke [") {
            let program = rest[..pos].to_string();
            let depth = rest[pos + 9..]
                .trim_end_matches(']')
                .parse::<u8>()
                .unwrap_or(stack.len() as u8 + 1);
            spans.push(LogSpan {
                program,
                depth,
                start: i,
                end: i + 1,
                ..Default::default()
            });
            stack.push(spans.len() - 1);
        } else if let Some(pos) = rest.find(" consumed ") {
            let program = &rest[..pos];
            let cu = rest[pos + 10..]
                .split(' ')
                .next()
                .and_then(|n| n.parse::<u64>().ok());
            if let Some(&top) = stack.iter().rev().find(|&&s| spans[s].program == program) {
                spans[top].cu_consumed = cu;
            }
        } else if let Some(program) = rest.strip_suffix(" success") {
            if let Some(idx) = pop_matching(&mut stack, &spans, program) {
                spans[idx].success = true;
                spans[idx].end = i + 1;
            }
        } else if let Some(pos) = rest.find(" failed: ") {
            let program = &rest[..pos];
            if let Some(idx) = pop_matching(&mut stack, &spans, program) {
                spans[idx].success = false;
                spans[idx].failure = Some(rest[pos + 9..].to_string());
                spans[idx].end = i + 1;
            }
        }
    }
    // Anything still open (truncated logs) ends at the last line.
    for idx in stack {
        spans[idx].end = logs.len();
    }
    spans
}

fn pop_matching(stack: &mut Vec<usize>, spans: &[LogSpan], program: &str) -> Option<usize> {
    let pos = stack.iter().rposition(|&s| spans[s].program == program)?;
    let idx = stack[pos];
    stack.truncate(pos);
    Some(idx)
}

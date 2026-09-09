//! "Why did my transaction fail — and how do I fix it?" — the everyday front
//! door. Reads the *recorded* on-chain failure (not a drift-prone re-simulation),
//! resolves the error to a plain name and message (Anchor logs first, then the
//! program's IDL), and adds a concrete fix. Deterministic and free.

use {crate::idl::error_for_code, serde::Serialize, serde_json::Value};

/// A plain-English diagnosis of a transaction's outcome.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnosis {
    /// Whether the transaction failed on-chain.
    pub failed: bool,
    /// A short headline — the resolved error name, or "Transaction succeeded".
    pub headline: String,
    /// Plain-English explanation of what happened.
    pub explanation: String,
    /// A concrete suggested fix, when the failure is a recognized pattern.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    /// The program that failed, when identifiable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// The custom error code, when the failure carried one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<i64>,
    /// The failing instruction's index, when the error is an InstructionError.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction_index: Option<i64>,
    /// The tail of the program logs, for context.
    pub logs: Vec<String>,
}

/// Diagnose a transaction from its `getTransaction` JSON. `idl_for(program)`
/// resolves a program id to its IDL JSON (for naming custom error codes).
pub(crate) fn diagnose_tx(tx: &Value, idl_for: impl Fn(&str) -> Option<Value>) -> Diagnosis {
    let meta = &tx["meta"];
    let logs: Vec<String> = meta["logMessages"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|l| l.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let err = &meta["err"];

    if err.is_null() && meta.is_object() {
        return Diagnosis {
            failed: false,
            headline: "Transaction succeeded".into(),
            explanation: "This transaction landed successfully on-chain — no error to diagnose."
                .into(),
            fix: None,
            program: None,
            error_code: None,
            instruction_index: None,
            logs: tail(&logs),
        };
    }
    if !meta.is_object() {
        return Diagnosis {
            failed: true,
            headline: "Outcome unavailable".into(),
            explanation:
                "The RPC returned no metadata for this transaction, so its outcome can't be read."
                    .into(),
            fix: Some("Try a different RPC endpoint, or a transaction that has finalized.".into()),
            program: None,
            error_code: None,
            instruction_index: None,
            logs: tail(&logs),
        };
    }

    let (instruction_index, error_code) = parse_instruction_error(err);
    let program = logs.iter().rev().find_map(|l| parse_failed_program(l));
    let anchor = logs.iter().find_map(|l| parse_anchor_error(l));

    // Resolve the error to a name + message: Anchor logs are richest; otherwise
    // name the custom code via the failing program's IDL.
    let (name, message): (Option<String>, Option<String>) = if let Some((n, _num, m)) = &anchor {
        (Some(n.clone()), Some(m.clone()))
    } else {
        // Try the failing program's IDL, then the shared Anchor framework map
        // (framework error codes mean the same thing across every program, so it
        // resolves even when the program isn't identified or has no IDL).
        let from_idl = match (&program, error_code) {
            (Some(prog), Some(code)) => idl_for(prog)
                .and_then(|idl| error_for_code(&idl, code as u64))
                .map(|e| (e.name, e.msg)),
            _ => None,
        };
        match from_idl.or_else(|| {
            error_code
                .and_then(framework_error)
                .map(|(n, m)| (n.to_string(), m.to_string()))
        }) {
            Some((n, m)) => (Some(n), Some(m)),
            None => (None, None),
        }
    };

    // When the error can't be named (no IDL / no Anchor log), interpret it from
    // context: an aggregator swap that ran every hop and then reverted on its own
    // final check is, in practice, a slippage / minimum-output failure. Lead with
    // that meaning instead of a bare code.
    let is_swap = logs.iter().any(|l| {
        let ll = l.to_lowercase();
        ll.contains("log:") && (ll.contains("swap") || ll.contains("route"))
    });
    let swap_slippage = name.is_none() && error_code.is_some() && is_swap;

    let headline = if let Some(n) = &name {
        n.clone()
    } else if swap_slippage {
        "Slippage — output below your minimum (likely)".to_string()
    } else {
        describe_error(err)
    };

    let explanation = if swap_slippage {
        let c = error_code.unwrap_or_default();
        format!(
            "This is a token swap (through an aggregator). Every hop of the route executed, then \
             the router reverted at the end with its own **error {c} (0x{c:x})** — on a completed swap that \
             almost always means the final output landed **below your minimum** (slippage). The \
             program publishes no IDL, so the exact name isn't on-chain, but the pattern is clear."
        )
    } else {
        build_explanation(
            name.as_deref(),
            message.as_deref(),
            error_code,
            instruction_index,
            program.as_deref(),
        )
    };
    let fix = fix_hint(
        &headline,
        message.as_deref(),
        &logs,
        error_code,
        name.is_some(),
    );

    Diagnosis {
        failed: true,
        headline,
        explanation,
        fix,
        program,
        error_code,
        instruction_index,
        logs: tail(&logs),
    }
}

/// `{"InstructionError": [index, {"Custom": code}]}` → (index, code).
fn parse_instruction_error(err: &Value) -> (Option<i64>, Option<i64>) {
    let arr = match err.get("InstructionError").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return (None, None),
    };
    let index = arr.first().and_then(|v| v.as_i64());
    let code = arr
        .get(1)
        .and_then(|v| v.get("Custom"))
        .and_then(|v| v.as_i64());
    (index, code)
}

/// "Program <id> failed: ..." → the program id.
fn parse_failed_program(log: &str) -> Option<String> {
    let rest = log.strip_prefix("Program ")?;
    let (prog, tail) = rest.split_once(' ')?;
    if tail.starts_with("failed") {
        Some(prog.to_string())
    } else {
        None
    }
}

/// Parse an Anchor error log → (name, number, message).
fn parse_anchor_error(log: &str) -> Option<(String, i64, String)> {
    if !log.contains("Error Code:") || !log.contains("Error Number:") {
        return None;
    }
    let name = between(log, "Error Code: ", ".")?.trim().to_string();
    let number = between(log, "Error Number: ", ".")?
        .trim()
        .parse::<i64>()
        .ok()?;
    let message = log
        .split("Error Message: ")
        .nth(1)
        .map(|m| m.trim_end_matches('.').trim().to_string())
        .unwrap_or_default();
    Some((name, number, message))
}

/// The Anchor framework error codes (accurate, from anchor-lang's `ErrorCode`).
/// These are the constraint/account failures every Anchor program shares — a
/// custom code in one of these framework ranges resolves to the same name
/// regardless of which program raised it. Program-specific errors (>= 6000) are
/// deliberately not here; they need the program's own IDL.
fn framework_error(code: i64) -> Option<(&'static str, &'static str)> {
    Some(match code {
        100 => ("InstructionMissing", "8-byte instruction identifier not provided"),
        102 => ("InstructionDidNotDeserialize", "The program could not deserialize the given instruction"),
        103 => ("InstructionDidNotSerialize", "The program could not serialize the given instruction"),
        2000 => ("ConstraintMut", "A `mut` constraint was violated — an account expected to be writable wasn't marked writable"),
        2001 => ("ConstraintHasOne", "A `has_one` constraint was violated — a stored field didn't match the account you passed"),
        2002 => ("ConstraintSigner", "A `signer` constraint was violated — an account that must sign didn't"),
        2003 => ("ConstraintRaw", "A raw constraint was violated"),
        2004 => ("ConstraintOwner", "An `owner` constraint was violated — an account is owned by the wrong program"),
        2005 => ("ConstraintRentExempt", "A rent-exemption constraint was violated"),
        2006 => ("ConstraintSeeds", "A `seeds` constraint was violated — the PDA you passed doesn't match the expected seeds"),
        2007 => ("ConstraintExecutable", "An `executable` constraint was violated"),
        2011 => ("ConstraintClose", "A `close` constraint was violated"),
        2012 => ("ConstraintAddress", "An `address` constraint was violated — an account address didn't match the required one"),
        2014 => ("ConstraintTokenMint", "A token mint constraint was violated — the token account's mint is wrong"),
        2015 => ("ConstraintTokenOwner", "A token owner constraint was violated"),
        2019 => ("ConstraintSpace", "A `space` constraint was violated"),
        2500 => ("RequireViolated", "A `require!` expression was violated"),
        2501 => ("RequireEqViolated", "A `require_eq!` expression was violated"),
        2502 => ("RequireKeysEqViolated", "A `require_keys_eq!` expression was violated"),
        2503 => ("RequireNeqViolated", "A `require_neq!` expression was violated"),
        2504 => ("RequireKeysNeqViolated", "A `require_keys_neq!` expression was violated"),
        2505 => ("RequireGtViolated", "A `require_gt!` expression was violated"),
        2506 => ("RequireGteViolated", "A `require_gte!` expression was violated"),
        3001 => ("AccountDiscriminatorNotFound", "No 8-byte discriminator on the account — it's likely uninitialized or the wrong account"),
        3002 => ("AccountDiscriminatorMismatch", "The account's discriminator didn't match — you passed the wrong account type"),
        3003 => ("AccountDidNotDeserialize", "Failed to deserialize the account — its data doesn't match the expected layout"),
        3005 => ("AccountNotEnoughKeys", "Not enough account keys given to the instruction"),
        3006 => ("AccountNotMutable", "The given account is not mutable"),
        3007 => ("AccountOwnedByWrongProgram", "The account is owned by a different program than expected"),
        3008 => ("InvalidProgramId", "A program id was not as expected"),
        3009 => ("InvalidProgramExecutable", "A program account is not executable"),
        3010 => ("AccountNotSigner", "The given account did not sign"),
        3011 => ("AccountNotSystemOwned", "The account is not owned by the System program"),
        3012 => ("AccountNotInitialized", "The program expected this account to already be initialized"),
        3014 => ("AccountNotAssociatedTokenAccount", "The given account is not the associated token account"),
        3016 => ("AccountReallocExceedsLimit", "The account reallocation exceeds the per-instruction growth limit"),
        4100 => ("DeclaredProgramIdMismatch", "The declared program id does not match the actual program id"),
        4102 => ("InvalidNumericConversion", "A numeric conversion overflowed or was invalid"),
        _ => return None,
    })
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let after = &s[s.find(start)? + start.len()..];
    Some(&after[..after.find(end).unwrap_or(after.len())])
}

/// A fallback description for non-custom errors (`InsufficientFundsForFee`, a
/// named `InstructionError` variant, a bare string).
fn describe_error(err: &Value) -> String {
    if let Some(s) = err.as_str() {
        return s.to_string();
    }
    if let Some(ie) = err.get("InstructionError").and_then(|v| v.as_array()) {
        if let Some(name) = ie.get(1).and_then(|v| v.as_str()) {
            return name.to_string();
        }
        if let Some(c) = ie
            .get(1)
            .and_then(|v| v.get("Custom"))
            .and_then(|v| v.as_i64())
        {
            return format!("custom error {c} (0x{c:x})");
        }
    }
    // First key of an object error (e.g. "InsufficientFundsForFee").
    err.as_object()
        .and_then(|o| o.keys().next())
        .cloned()
        .unwrap_or_else(|| "Transaction failed".to_string())
}

fn build_explanation(
    name: Option<&str>,
    message: Option<&str>,
    code: Option<i64>,
    ix: Option<i64>,
    program: Option<&str>,
) -> String {
    let where_ = match (ix, program) {
        (Some(i), Some(p)) => format!("Instruction {i} (program {}) reverted", short(p)),
        (Some(i), None) => format!("Instruction {i} reverted"),
        (None, Some(p)) => format!("Program {} reverted", short(p)),
        (None, None) => "The transaction reverted".to_string(),
    };
    // Logs print custom errors in hex ("failed: custom program error: 0x1771"),
    // so show both bases or the card looks like it disagrees with the log.
    let with = match (name, code, message) {
        (Some(n), Some(c), Some(m)) if !m.is_empty() => {
            format!(" with **{n}** (error {c} = 0x{c:x}): {m}")
        }
        (Some(n), Some(c), _) => format!(" with **{n}** (error {c} = 0x{c:x})"),
        (Some(n), None, Some(m)) if !m.is_empty() => format!(" with **{n}**: {m}"),
        (Some(n), None, _) => format!(" with **{n}**"),
        // Unresolved custom code — no public IDL to name it.
        (None, Some(c), _) => {
            format!(" with the program's own **custom error {c}** (0x{c:x}; no public IDL was available to name it)")
        }
        (None, None, _) => " and failed".to_string(),
    };
    format!("{where_}{with}.")
}

/// Deterministic fix suggestions from recognized failure patterns.
fn fix_hint(
    headline: &str,
    message: Option<&str>,
    logs: &[String],
    code: Option<i64>,
    resolved: bool,
) -> Option<String> {
    // Only match on the error name + message + non-"Program …" log lines, so a
    // hex error code in the failure line can't be mistaken for a pattern.
    let log_text: String = logs
        .iter()
        .filter(|l| !l.starts_with("Program ") || l.contains("log:"))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let blob = format!("{headline} {} {log_text}", message.unwrap_or("")).to_lowercase();
    let table: &[(&[&str], &str)] = &[
        (
            &["slippage", "exceededslippage", "price impact", "min out"],
            "The pool price moved between your quote and execution. Increase the slippage tolerance, or re-quote closer to send time.",
        ),
        (
            &["insufficient funds", "insufficient lamports", "not enough", "insufficient balance"],
            "An account didn't hold enough SOL or tokens. Fund the account, or lower the amount being moved.",
        ),
        (
            &["blockhash not found", "blockhashnotfound", "expired", "block height exceeded"],
            "The transaction's blockhash expired before it landed. Fetch a fresh recent blockhash and resend promptly.",
        ),
        (
            &["accountnotfound", "could not find account", "uninitialized", "account does not exist", "notinitialized"],
            "A required account doesn't exist yet. Create/initialize it (or its associated token account) before this instruction.",
        ),
        (
            &["already in use", "alreadyinitialized", "already initialized"],
            "The account you're creating already exists. Skip creation, or use the existing account.",
        ),
        (
            &["overflow", "underflow", "divisionbyzero", "arithmetic"],
            "An arithmetic overflow/underflow — usually a decimals or amount mismatch. Check token decimals and the magnitudes you're passing.",
        ),
        (
            &["invalidaccountdata", "accountownedbywrongprogram", "wrong program"],
            "An account isn't in the state/owner this program expects — often it was closed, reallocated, or you passed the wrong one. Double-check the account addresses.",
        ),
        (
            &["signature verification failed", "missing required signature", "signer"],
            "A required signer didn't sign. Make sure every account marked as a signer is included and signs the transaction.",
        ),
    ];
    for (needles, fix) in table {
        if needles.iter().any(|n| blob.contains(n)) {
            return Some((*fix).to_string());
        }
    }
    // A swap that reverted with an unnamed custom error is, in practice, almost
    // always a slippage / minimum-output failure — say so even without the IDL.
    if code.is_some() && !resolved && (blob.contains("swap") || blob.contains("route")) {
        return Some(
            "This transaction is a token swap that reverted. On a DEX or aggregator, an unnamed custom error here is almost always a slippage / minimum-output failure — the price moved past your limit between quote and execution. Fix: increase your slippage tolerance, or re-quote right before sending.".into(),
        );
    }
    // No recognized pattern: give a useful generic pointer for a custom error.
    match (code, resolved) {
        (Some(c), false) => Some(format!(
            "The program rejected the inputs with its own error {c} (0x{c:x}), but it publishes no on-chain IDL, so it cannot be named here. Check the program's source or docs for what error {c} means, and the values this instruction expects."
        )),
        (Some(_), true) => Some(
            "The program rejected the inputs with this error. Review what that instruction requires and adjust the accounts or arguments accordingly.".into(),
        ),
        _ => None,
    }
}

fn tail(logs: &[String]) -> Vec<String> {
    let n = logs.len();
    logs[n.saturating_sub(16)..].to_vec()
}

fn short(a: &str) -> String {
    if a.len() > 12 {
        format!("{}…{}", &a[..4], &a[a.len() - 4..])
    } else {
        a.to_string()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn diagnoses_an_anchor_slippage_failure_with_a_fix() {
        let tx = json!({
            "meta": {
                "err": { "InstructionError": [2, { "Custom": 6001 }] },
                "logMessages": [
                    "Program Whirl invoke [1]",
                    "Program log: AnchorError occurred. Error Code: SlippageToleranceExceeded. Error Number: 6001. Error Message: Slippage tolerance exceeded.",
                    "Program Whirl failed: custom program error: 0x1771"
                ]
            }
        });
        let d = diagnose_tx(&tx, |_| None);
        assert!(d.failed);
        assert_eq!(d.headline, "SlippageToleranceExceeded");
        assert_eq!(d.error_code, Some(6001));
        assert_eq!(d.instruction_index, Some(2));
        assert!(d.explanation.contains("Instruction 2"));
        assert!(d
            .fix
            .as_deref()
            .unwrap()
            .to_lowercase()
            .contains("slippage"));
    }

    #[test]
    fn names_a_custom_code_via_the_idl_when_no_anchor_log() {
        let tx = json!({
            "meta": {
                "err": { "InstructionError": [0, { "Custom": 6024 }] },
                "logMessages": ["Program Pmp invoke [1]", "Program Pmp failed: custom program error: 0x1788"]
            }
        });
        let idl = json!({ "errors": [{ "code": 6024, "name": "PoolPaused", "msg": "The pool is paused" }] });
        let d = diagnose_tx(&tx, |_| Some(idl.clone()));
        assert_eq!(d.headline, "PoolPaused");
        assert!(d.explanation.contains("The pool is paused"));
    }

    #[test]
    fn resolves_an_anchor_framework_error_without_an_idl() {
        // Error 3012 (AccountNotInitialized) is a framework code — nameable with
        // no IDL and no Anchor log line.
        let tx = json!({
            "meta": {
                "err": { "InstructionError": [1, { "Custom": 3012 }] },
                "logMessages": ["Program Foo invoke [1]", "Program Foo failed: custom program error: 0xbc4"]
            }
        });
        let d = diagnose_tx(&tx, |_| None);
        assert_eq!(d.headline, "AccountNotInitialized");
        assert!(d.explanation.to_lowercase().contains("initialized"));
    }

    #[test]
    fn does_not_misname_a_program_specific_custom_error() {
        // 6006 is program-specific (>= 6000) — must NOT be mapped to a framework name.
        let tx = json!({
            "meta": { "err": { "InstructionError": [0, { "Custom": 6006 }] }, "logMessages": [] }
        });
        let d = diagnose_tx(&tx, |_| None);
        assert_eq!(d.headline, "custom error 6006 (0x1776)");
    }

    #[test]
    fn reports_success_cleanly() {
        let tx = json!({ "meta": { "err": null, "logMessages": [] } });
        let d = diagnose_tx(&tx, |_| None);
        assert!(!d.failed);
        assert_eq!(d.headline, "Transaction succeeded");
    }
}

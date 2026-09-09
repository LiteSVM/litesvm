//! The JSON suite-file format: scenarios and mutations as data, so a suite
//! can live in a `.json` file or arrive over the wire.
//!
//! Mutations and scenarios arrive as JSON (from the browser, or a `.json` test
//! file); these `Deserialize` types convert them into the strongly-typed
//! [`crate::Scenario`] / [`crate::mutation::Mutation`] the engine runs.

use {
    crate::{
        check::{Check, CheckKind, Scenario},
        error::{Error, Result},
        mutation::{AccountAssert, CmpOp, Expect, Mutation, StateCheck},
        replay::{FeatureToggle, TimeTravel},
        utils::hex_decode,
    },
    serde::Deserialize,
    solana_address::Address,
    std::str::FromStr,
};

/// A runtime feature-gate toggle for a replay: `{"id":"<feature pubkey>","active":true}`.
/// `active` true = activate the gate (e.g. test a not-yet-live feature early),
/// false = deactivate an active one.
#[derive(Deserialize, Clone)]
pub struct FeatureInput {
    /// The feature gate's address, as base58.
    pub id: String,
    /// True to activate the gate, false to deactivate it.
    #[serde(default)]
    pub active: bool,
}

impl FeatureInput {
    /// Convert into the engine's [`FeatureToggle`], validating the id.
    pub fn into_toggle(self) -> Result<FeatureToggle> {
        let id = Address::from_str(self.id.trim()).map_err(|_| {
            Error::InvalidSpec(format!("bad feature id (not a pubkey): {}", self.id))
        })?;
        Ok(FeatureToggle {
            id,
            active: self.active,
        })
    }
}

/// Convert a list of feature inputs into engine toggles, failing on the first bad id.
pub fn feature_toggles(features: Vec<FeatureInput>) -> Result<Vec<FeatureToggle>> {
    features
        .into_iter()
        .map(FeatureInput::into_toggle)
        .collect()
}

/// One what-if mutation. `kind` selects the variant:
/// `{"kind":"lamports","address":..,"lamports":..}`,
/// `{"kind":"data","address":..,"offset":..,"bytes_hex":".."}` (patch at offset), or
/// `{"kind":"field","address":..,"field":..,"value":..}` (set a named field), or
/// `{"kind":"skipix","index":..}` (remove top-level instruction `index`),
/// `{"kind":"ixdata","index":..,"bytes_hex":".."}` (replace its data wholesale), or
/// `{"kind":"moveix","from":..,"to":..}` (move it to another position).
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum MutationInput {
    /// Set an account's lamport balance.
    Lamports {
        /// The account to mutate.
        address: String,
        /// The new lamport balance.
        lamports: u64,
    },
    /// Patch an account's data at an offset.
    Data {
        /// The account to mutate.
        address: String,
        /// Byte offset where the patch begins.
        offset: usize,
        /// The bytes to write, as hex (`0x`, spaces, underscores tolerated).
        bytes_hex: String,
    },
    /// Set a named argument of a top-level instruction, re-encoded through the
    /// program's IDL (fixed-size scalar arguments only).

    /// Remove a top-level instruction (by its original index) before replaying.
    SkipIx {
        /// Zero-based position among the transaction's top-level instructions.
        index: usize,
    },
    /// Replace a top-level instruction's data wholesale (hex, any length).
    IxData {
        /// Zero-based position among the transaction's top-level instructions.
        index: usize,
        /// The new data, hex-encoded.
        bytes_hex: String,
    },
    /// Move a top-level instruction to another position.
    MoveIx {
        /// Current position.
        from: usize,
        /// Destination position.
        to: usize,
    },

    /// Set a top-level instruction's named argument (fixed-size scalars, via the IDL).
    IxArg {
        /// Zero-based top-level instruction index.
        index: usize,
        /// The argument's IDL name.
        arg: String,
        /// The new value: a number, a bool, or a base58 string for a pubkey.
        value: serde_json::Value,
    },
    /// Set a named integer/bool field, resolved through the account's decoded
    /// layout (SPL layouts or the owner program's IDL) — no byte offsets.
    Field {
        /// The account to mutate.
        address: String,
        /// The field's name (exact, or an unambiguous final dot-segment).
        field: String,
        /// The new value, a JSON number or a decimal string (for values beyond
        /// what JavaScript numbers represent exactly); must fit the field's type.
        value: serde_json::Value,
    },
}

impl MutationInput {
    /// Convert into the engine's [`Mutation`], decoding hex bytes.
    pub fn into_mutation(self) -> Result<Mutation> {
        Ok(match self {
            MutationInput::Lamports { address, lamports } => Mutation::Lamports {
                address,
                value: lamports,
            },
            MutationInput::Data {
                address,
                offset,
                bytes_hex,
            } => Mutation::DataPatch {
                address,
                offset,
                bytes: hex_decode(&bytes_hex)?,
            },
            MutationInput::IxArg { index, arg, value } => Mutation::IxArg { index, arg, value },
            MutationInput::SkipIx { index } => Mutation::SkipIx { index },
            MutationInput::IxData { index, bytes_hex } => Mutation::IxDataReplace {
                index,
                bytes: hex_decode(&bytes_hex)?,
            },
            MutationInput::MoveIx { from, to } => Mutation::MoveIx { from, to },
            MutationInput::Field {
                address,
                field,
                value,
            } => Mutation::Field {
                address,
                field,
                value: value
                    .as_i64()
                    .map(i128::from)
                    .or_else(|| value.as_u64().map(i128::from))
                    .or_else(|| value.as_str().and_then(|s| s.trim().parse::<i128>().ok()))
                    .ok_or_else(|| {
                        crate::Error::InvalidSpec(format!(
                            "field value must be an integer, got {value}"
                        ))
                    })?,
            },
        })
    }
}

/// A post-replay state assertion. `kind` is one of:
/// - `"u64"` — little-endian u64 at `offset`.
/// - `"lamports"` — the account's lamports.
/// - `"token_amount"` — SPL token amount (u64 @ 64); shorthand for `u64` at offset 64.
/// - `"lamports_delta"` — change in lamports (post − pre); `value` may be negative.
/// - `"token_delta"` — change in SPL token amount (post − pre); `value` may be negative.
/// - `"field"` — the named `field` of the account's decoded layout (SPL layouts,
///   or the owner program's IDL), e.g. `"field":"pool.reserveA"`. Matched by
///   exact name or final dot-segment; integer/bool fields only, signed included.
/// - `"field_delta"` — change in the named `field` (post − pre); may be negative.
/// - `"field_unchanged"` — the named `field` is byte-for-byte identical before and
///   after; any field type (pubkeys included). Takes no `value`.
///
/// `op` is one of `== != < <= > >=` (default `==`).
#[derive(Deserialize)]
pub struct AssertInput {
    /// The account the assertion reads.
    pub address: String,
    /// The assertion kind (see the type docs); defaults to `"u64"`.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Byte offset for the `u64` kind.
    #[serde(default)]
    pub offset: usize,
    /// Field name for the `field` / `field_delta` kinds.
    #[serde(default)]
    pub field: Option<String>,
    /// Comparison operator: `==` `!=` `<` `<=` `>` `>=` (default `==`).
    #[serde(default = "default_op")]
    pub op: String,
    /// Signed so deltas can be negative; non-delta kinds require it to be ≥ 0.
    /// Required for every kind except `field_unchanged`.
    #[serde(default)]
    pub value: Option<i64>,
}

fn default_kind() -> String {
    "u64".into()
}
fn default_op() -> String {
    "==".into()
}

impl AssertInput {
    fn into_assert(self) -> Result<AccountAssert> {
        let op = match self.op.as_str() {
            "==" | "eq" => CmpOp::Eq,
            "!=" | "ne" => CmpOp::Ne,
            "<" | "lt" => CmpOp::Lt,
            "<=" | "le" => CmpOp::Le,
            ">" | "gt" => CmpOp::Gt,
            ">=" | "ge" => CmpOp::Ge,
            other => return Err(Error::InvalidSpec(format!("unknown assert op: {other}"))),
        };
        // Every kind but `field_unchanged` compares against a value.
        let value = || -> Result<i64> {
            self.value.ok_or_else(|| {
                Error::InvalidSpec(format!("assert kind \"{}\" needs a \"value\"", self.kind))
            })
        };
        // Non-delta kinds compare against an unsigned value.
        let unsigned = || -> Result<u64> {
            u64::try_from(value()?)
                .map_err(|_| Error::InvalidSpec(format!("{} value must be ≥ 0", self.kind)))
        };
        // Field kinds compare signed (an i64 timestamp is a legitimate target).
        let field = || -> Result<String> {
            match &self.field {
                Some(f) if !f.trim().is_empty() => Ok(f.trim().to_string()),
                _ => Err(Error::InvalidSpec(format!(
                    "assert kind \"{}\" needs a \"field\" name",
                    self.kind
                ))),
            }
        };
        let check = match self.kind.as_str() {
            "lamports" => StateCheck::Lamports {
                op,
                value: unsigned()? as i128,
            },
            "u64" => StateCheck::U64At {
                offset: self.offset,
                op,
                value: unsigned()? as i128,
            },
            "token_amount" => StateCheck::U64At {
                offset: 64,
                op,
                value: unsigned()? as i128,
            },
            "lamports_delta" => StateCheck::LamportsDelta {
                op,
                value: value()? as i128,
            },
            "token_delta" => StateCheck::TokenDelta {
                op,
                value: value()? as i128,
            },
            "field" => StateCheck::Field {
                name: field()?,
                op,
                value: value()? as i128,
            },
            "field_delta" => StateCheck::FieldDelta {
                name: field()?,
                op,
                value: value()? as i128,
            },
            "field_unchanged" => StateCheck::FieldUnchanged { name: field()? },
            other => return Err(Error::InvalidSpec(format!("unknown assert kind: {other}"))),
        };
        Ok(AccountAssert {
            address: self.address,
            check,
        })
    }
}

/// One test scenario: a name, the mutations to apply, the transaction-level
/// outcome to assert, and optional post-replay state assertions.
///
/// `expect` is `"success"`, `"revert"` (or `"fail"`), or `"any"`. When reverting,
/// an optional `contains` requires the error/logs to include that text.
#[derive(Deserialize)]
pub struct ScenarioInput {
    /// The scenario's name, shown in outcomes.
    pub name: String,
    /// The transaction-level expectation: `"success"`, `"revert"`/`"fail"`,
    /// or `"any"` (the default).
    #[serde(default = "default_expect")]
    pub expect: String,
    /// For a revert expectation: text the error/logs must include.
    #[serde(default)]
    pub contains: Option<String>,
    /// Mutations applied to the world before replaying.
    #[serde(default)]
    pub mutations: Vec<MutationInput>,
    /// Post-replay state assertions.
    #[serde(default)]
    pub asserts: Vec<AssertInput>,
}

fn default_expect() -> String {
    "any".into()
}

impl ScenarioInput {
    /// Convert the JSON shape into an engine [`Scenario`].
    pub fn into_scenario(self) -> Result<Scenario> {
        let expect = match self.expect.trim() {
            "success" | "pass" => Expect::Success,
            "revert" | "fail" => match self.contains {
                Some(s) if !s.is_empty() => Expect::RevertContains(s),
                _ => Expect::Revert,
            },
            "any" => Expect::Any,
            // A typo like "sucess" must not silently become Any and let the
            // scenario pass vacuously — that is exactly the silent-pass footgun
            // the crate promises to eliminate.
            other => {
                return Err(Error::InvalidSpec(format!(
                    "unknown expect \"{other}\" (use success, revert, or any)"
                )))
            }
        };
        let mutations = self
            .mutations
            .into_iter()
            .map(MutationInput::into_mutation)
            .collect::<Result<_>>()?;
        let mut checks = vec![Check(CheckKind::Outcome(expect))];
        for a in self.asserts {
            checks.push(Check(CheckKind::Account(vec![a.into_assert()?])));
        }
        Ok(Scenario {
            name: self.name,
            mutations,
            checks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_decode_tolerates_prefix_spaces_underscores() {
        assert_eq!(
            hex_decode("0xDEAD_beef").unwrap(),
            vec![0xde, 0xad, 0xbe, 0xef]
        );
        assert_eq!(hex_decode("00 ff").unwrap(), vec![0x00, 0xff]);
    }

    #[test]
    fn hex_decode_rejects_bad_input() {
        assert!(hex_decode("").is_err());
        assert!(hex_decode("abc").is_err()); // odd length
        assert!(hex_decode("zz").is_err());
    }

    #[test]
    fn hex_decode_rejects_multibyte_without_panicking() {
        // A multi-byte char has an even byte length but no ASCII char boundary at
        // the slice points — this must be a typed error, never a slice panic.
        assert!(hex_decode("aΩb").is_err());
        assert!(hex_decode("0x€€").is_err());
    }

    #[test]
    fn unknown_expect_string_is_an_error_not_a_vacuous_pass() {
        let s: ScenarioInput =
            serde_json::from_str(r#"{"name":"typo","expect":"sucess"}"#).unwrap();
        assert!(matches!(s.into_scenario(), Err(Error::InvalidSpec(_))));
    }

    #[test]
    fn field_mutation_parses() {
        let m: MutationInput =
            serde_json::from_str(r#"{"kind":"field","address":"X","field":"count","value":-5}"#)
                .unwrap();
        match m.into_mutation().unwrap() {
            Mutation::Field {
                address,
                field,
                value,
            } => {
                assert_eq!(
                    (address.as_str(), field.as_str(), value),
                    ("X", "count", -5)
                );
            }
            other => panic!("expected Field, got {other:?}"),
        }
    }

    #[test]
    fn data_mutation_becomes_patch() {
        let m: MutationInput = serde_json::from_str(
            r#"{"kind":"data","address":"X","offset":64,"bytes_hex":"0000000000000000"}"#,
        )
        .unwrap();
        match m.into_mutation().unwrap() {
            Mutation::DataPatch {
                address,
                offset,
                bytes,
            } => {
                assert_eq!(address, "X");
                assert_eq!(offset, 64);
                assert_eq!(bytes, vec![0u8; 8]);
            }
            _ => panic!("expected DataPatch"),
        }
    }

    #[test]
    fn assert_kinds_and_ops_resolve() {
        let a: AssertInput =
            serde_json::from_str(r#"{"address":"X","kind":"token_amount","op":">=","value":5}"#)
                .unwrap();
        // token_amount is shorthand for u64 at the SPL amount offset.
        match a.into_assert().unwrap().check {
            StateCheck::U64At {
                offset: 64,
                op: CmpOp::Ge,
                value: 5,
            } => {}
            _ => panic!("expected U64At @64 >= 5"),
        }

        let d: AssertInput =
            serde_json::from_str(r#"{"address":"X","kind":"token_delta","op":"<","value":-3}"#)
                .unwrap();
        match d.into_assert().unwrap().check {
            StateCheck::TokenDelta {
                op: CmpOp::Lt,
                value: -3,
            } => {}
            _ => panic!("expected TokenDelta < -3"),
        }
    }

    #[test]
    fn field_asserts_resolve_and_allow_negatives() {
        let a: AssertInput = serde_json::from_str(
            r#"{"address":"X","kind":"field","field":"pool.reserveA","op":">=","value":1000}"#,
        )
        .unwrap();
        match a.into_assert().unwrap().check {
            StateCheck::Field {
                name,
                op: CmpOp::Ge,
                value: 1000,
            } => assert_eq!(name, "pool.reserveA"),
            _ => panic!("expected Field >= 1000"),
        }

        // Deltas — and signed fields like i64 timestamps — may go negative.
        let d: AssertInput = serde_json::from_str(
            r#"{"address":"X","kind":"field_delta","field":"reserveA","value":-500}"#,
        )
        .unwrap();
        match d.into_assert().unwrap().check {
            StateCheck::FieldDelta {
                name,
                op: CmpOp::Eq,
                value: -500,
            } => assert_eq!(name, "reserveA"),
            _ => panic!("expected FieldDelta == -500"),
        }
    }

    #[test]
    fn field_unchanged_needs_no_value_and_other_kinds_do() {
        let a: AssertInput =
            serde_json::from_str(r#"{"address":"X","kind":"field_unchanged","field":"authority"}"#)
                .unwrap();
        match a.into_assert().unwrap().check {
            StateCheck::FieldUnchanged { name } => assert_eq!(name, "authority"),
            other => panic!("expected FieldUnchanged, got {other:?}"),
        }
        let missing: AssertInput =
            serde_json::from_str(r#"{"address":"X","kind":"lamports"}"#).unwrap();
        let err = missing.into_assert().unwrap_err().to_string();
        assert!(err.contains("needs a \"value\""), "{err}");
    }

    #[test]
    fn field_assert_requires_a_field_name() {
        let a: AssertInput =
            serde_json::from_str(r#"{"address":"X","kind":"field","value":1}"#).unwrap();
        assert!(a.into_assert().unwrap_err().to_string().contains("field"));
    }

    #[test]
    fn non_delta_assert_rejects_negative_value() {
        let a: AssertInput =
            serde_json::from_str(r#"{"address":"X","kind":"lamports","value":-1}"#).unwrap();
        assert!(a.into_assert().is_err());
    }

    #[test]
    fn unknown_op_and_kind_error() {
        let a: AssertInput =
            serde_json::from_str(r#"{"address":"X","op":"~=","value":1}"#).unwrap();
        assert!(a.into_assert().is_err());
        let k: AssertInput =
            serde_json::from_str(r#"{"address":"X","kind":"balancez","value":1}"#).unwrap();
        assert!(k.into_assert().is_err());
    }

    #[test]
    fn scenario_expect_revert_with_contains() {
        let s: ScenarioInput = serde_json::from_str(
            r#"{"name":"drain","expect":"revert","contains":"Slippage","mutations":[],"asserts":[]}"#,
        )
        .unwrap();
        let scenario = s.into_scenario().unwrap();
        match &scenario.checks[0].0 {
            CheckKind::Outcome(Expect::RevertContains(t)) => assert_eq!(t, "Slippage"),
            other => panic!("expected RevertContains, got {other:?}"),
        }
    }
}

/// A suite as JSON: the fixture or signature to replay, and the scenarios to run.
///
/// Provide `fixture` (a path to a frozen fixture file) for a deterministic,
/// offline run — the CI-safe path — or `signature` to fetch live state via RPC.
#[derive(Deserialize)]
pub struct SuiteRequest {
    /// Transaction signature for the live-RPC path.
    #[serde(default)]
    pub signature: Option<String>,
    /// Path to a frozen fixture file for the offline path.
    #[serde(default)]
    pub fixture: Option<String>,
    /// Cluster name (mainnet/devnet/testnet/localnet) for the live-signature path.
    #[serde(default)]
    pub cluster: Option<String>,
    /// Explicit RPC URL override for the live-signature path.
    #[serde(default)]
    pub rpc: Option<String>,
    /// Optional clock warp applied to every scenario in the suite.
    #[serde(default)]
    pub time_travel: TimeTravel,
    /// Optional runtime feature-gate toggles applied to every scenario.
    #[serde(default)]
    pub features: Vec<FeatureInput>,
    /// The scenarios to run.
    pub scenarios: Vec<ScenarioInput>,
}

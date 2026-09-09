//! Mutations and account assertions — the what-if vocabulary shared by
//! replays, checks and the suite format.

use crate::{
    analyze::ReplayResult,
    error::{Error, Result},
};

/// A change to apply to an account before replaying.
///
/// `Data` replaces an account's bytes wholesale; `DataPatch` overwrites a slice
/// at an offset; `Field` sets a *named* field with no offsets at all (how you'd
/// flip a token balance, an oracle price, or a vesting timestamp by name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mutation {
    /// Set an account's lamport balance.
    Lamports {
        /// The account to mutate.
        address: String,
        /// The new lamport balance.
        value: u64,
    },
    /// Replace an account's data wholesale.
    Data {
        /// The account to mutate.
        address: String,
        /// The complete replacement data.
        bytes: Vec<u8>,
    },
    /// Overwrite a slice of an account's data at an offset.
    DataPatch {
        /// The account to mutate.
        address: String,
        /// Byte offset where the patch begins.
        offset: usize,
        /// The bytes to write at that offset.
        bytes: Vec<u8>,
    },
    /// Set a named argument of a top-level instruction, re-encoded through the
    /// program's IDL — "what if the slippage were 5%?". Resolves to
    /// [`Mutation::IxData`] before application; only fixed-size scalar
    /// arguments preceded by fixed-size arguments can be located.
    IxArg {
        /// Zero-based index of the top-level instruction.
        index: usize,
        /// The argument's IDL name.
        arg: String,
        /// The new value (number, bool, or base58 string for a pubkey).
        value: serde_json::Value,
    },
    /// Overwrite bytes of a top-level instruction's data at an offset.
    IxData {
        /// Zero-based index of the top-level instruction.
        index: usize,
        /// Byte offset into the instruction data.
        offset: usize,
        /// The bytes to write.
        bytes: Vec<u8>,
    },
    /// Remove top-level instruction `index` (its original position) from the
    /// transaction before replaying: "what if this instruction were not
    /// here?" Later instructions shift up; instruction-index mutations still
    /// refer to original positions.
    SkipIx {
        /// Zero-based position among the transaction's top-level instructions.
        index: usize,
    },
    /// Replace top-level instruction `index`'s data wholesale (any length),
    /// for arguments the fixed-offset patch cannot express.
    IxDataReplace {
        /// Zero-based position among the transaction's top-level instructions.
        index: usize,
        /// The new instruction data.
        bytes: Vec<u8>,
    },
    /// Move a top-level instruction from position `from` to position `to`
    /// (positions after skips are applied): "what if it ran earlier/later?"
    MoveIx {
        /// Current position.
        from: usize,
        /// Destination position.
        to: usize,
    },
    /// Set a **named** integer/bool field — no byte offsets. The field resolves
    /// through the account's decoded layout (SPL layouts, or the owner
    /// program's IDL) exactly like [`crate::Check`]'s named-field asserts:
    /// matched by exact name or unambiguous final dot-segment, with unknown
    /// names erroring and listing the fields that do exist.
    Field {
        /// The account to mutate.
        address: String,
        /// The field's name (e.g. `"amount"`, or a dotted path like
        /// `"pool.reserve_a"`).
        field: String,
        /// The new value. Must fit the field's declared type — an
        /// out-of-range value is a hard error, never a silent truncation.
        value: i128,
    },
    /// Reassign the program that **owns** an account (not a data field — the
    /// account's owner program itself). Breaks any program that checks it owns
    /// its accounts.
    Owner {
        /// The account to mutate.
        address: String,
        /// The new owner program, base58.
        owner: String,
    },
}

impl Mutation {
    /// Set an account's lamports.
    pub fn lamports(address: impl Into<String>, value: u64) -> Mutation {
        Mutation::Lamports {
            address: address.into(),
            value,
        }
    }

    /// Replace an account's data wholesale.
    pub fn data(address: impl Into<String>, bytes: Vec<u8>) -> Mutation {
        Mutation::Data {
            address: address.into(),
            bytes,
        }
    }

    /// Reassign an account's owner program.
    pub fn owner(address: impl Into<String>, owner: impl Into<String>) -> Mutation {
        Mutation::Owner {
            address: address.into(),
            owner: owner.into(),
        }
    }

    /// Overwrite a slice of an account's data at `offset`.
    pub fn patch(address: impl Into<String>, offset: usize, bytes: Vec<u8>) -> Mutation {
        Mutation::DataPatch {
            address: address.into(),
            offset,
            bytes,
        }
    }

    /// Set a named integer/bool field of a decoded account — the mutation-side
    /// twin of `Check::account(addr).field(name, …)`. Resolved through SPL
    /// layouts or the program's IDL; the value must fit the field's type.
    ///
    /// ```no_run
    /// # use svmscope::Mutation;
    /// let m = Mutation::field("CounterPda111", "count", 99);
    /// ```
    pub fn field(
        address: impl Into<String>,
        field: impl Into<String>,
        value: impl Into<i128>,
    ) -> Mutation {
        Mutation::Field {
            address: address.into(),
            field: field.into(),
            value: value.into(),
        }
    }

    /// Set a top-level instruction's named argument (see [`Mutation::IxArg`]).
    pub fn ix_arg(index: usize, arg: impl Into<String>, value: serde_json::Value) -> Mutation {
        Mutation::IxArg {
            index,
            arg: arg.into(),
            value,
        }
    }

    pub(crate) fn address(&self) -> &str {
        match self {
            Mutation::Lamports { address, .. }
            | Mutation::Data { address, .. }
            | Mutation::DataPatch { address, .. }
            | Mutation::Field { address, .. }
            | Mutation::Owner { address, .. } => address,
            Mutation::IxArg { .. }
            | Mutation::IxData { .. }
            | Mutation::SkipIx { .. }
            | Mutation::IxDataReplace { .. }
            | Mutation::MoveIx { .. } => "",
        }
    }
}

/// The outcome a scenario asserts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Expect {
    /// The transaction must succeed.
    Success,
    /// The transaction must fail (any error).
    Revert,
    /// The transaction must fail *and* the error or a log line contains this text
    /// (e.g. an error code `Custom(6025)` or a message `DivisionByZero`).
    RevertContains(String),
    /// No assertion — just report what happened.
    Any,
}

impl Expect {
    pub(crate) fn matches(&self, r: &ReplayResult) -> bool {
        match self {
            Expect::Success => r.success,
            Expect::Revert => !r.success,
            Expect::RevertContains(s) => {
                !r.success
                    && (r.error.as_deref().is_some_and(|e| e.contains(s))
                        || r.logs.iter().any(|l| l.contains(s)))
            }
            Expect::Any => true,
        }
    }

    pub(crate) fn describe(&self) -> String {
        match self {
            Expect::Success => "succeeds".into(),
            Expect::Revert => "reverts".into(),
            Expect::RevertContains(s) => format!("reverts with \"{s}\""),
            Expect::Any => "any outcome".into(),
        }
    }
}

/// A comparison operator for numeric state assertions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl CmpOp {
    pub(crate) fn test_i128(self, a: i128, b: i128) -> bool {
        match self {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
            CmpOp::Gt => a > b,
            CmpOp::Ge => a >= b,
        }
    }
    pub(crate) fn symbol(self) -> &'static str {
        match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        }
    }
}

/// A check on an account's state *after* the transaction replays. This is what
/// makes a scenario a real test — asserting on resulting state, not just pass/fail.
#[derive(Debug, Clone)]
pub(crate) enum StateCheck {
    /// The account's lamports satisfy `op value`.
    Lamports { op: CmpOp, value: i128 },
    /// The little-endian u64 at `offset` satisfies `op value`
    /// (e.g. an SPL token amount at offset 64).
    U64At {
        offset: usize,
        op: CmpOp,
        value: i128,
    },
    /// The *change* in lamports (post - pre) satisfies `op value` (may be negative)
    /// — e.g. "the fee vault gained ≥ N": `LamportsDelta { Ge, N }`.
    LamportsDelta { op: CmpOp, value: i128 },
    /// The *change* in SPL token amount (u64 @ 64, post - pre) satisfies `op value`.
    TokenDelta { op: CmpOp, value: i128 },
    /// A named field of the account's decoded layout satisfies `op value` —
    /// `pool.reserveA >= 1_000` instead of `u64@72`. The name resolves against
    /// the pre-state layout (built-in decoders, then the owner program's IDL);
    /// matched by exact name or by final dot-segment.
    Field {
        name: String,
        op: CmpOp,
        value: i128,
    },
    /// The *change* in a named field (post - pre) satisfies `op value`.
    FieldDelta {
        name: String,
        op: CmpOp,
        value: i128,
    },
    /// The named field's raw bytes are identical before and after — works for
    /// every field type (pubkeys, options, integers), not just numeric ones.
    FieldUnchanged { name: String },
}

/// A post-replay assertion targeting one account.
#[derive(Debug, Clone)]
pub(crate) struct AccountAssert {
    pub(crate) address: String,
    pub(crate) check: StateCheck,
}

/// Read a little-endian u64 at `offset`, or 0 if out of range.
pub(crate) fn read_u64_at(data: &[u8], offset: usize) -> u64 {
    match data.get(offset..offset + 8) {
        Some(s) => u64::from_le_bytes(s.try_into().unwrap()),
        None => 0,
    }
}

/// Find a field in a decoded layout by exact name, or — so `pool.reserveA`
/// works when the field is just `reserveA` — by its final dot-segment. An
/// ambiguous short name (several fields end in it) is an error naming the
/// candidates, not a silent first-match.
pub(crate) fn find_field<'a>(
    dec: &'a crate::decode::DecodedAccount,
    name: &'a str,
) -> Result<&'a crate::decode::Field> {
    if let Some(f) = dec.fields.iter().find(|f| f.name == name) {
        return Ok(f);
    }
    let last = name.rsplit('.').next().unwrap_or(name);
    let hits: Vec<&crate::decode::Field> = dec
        .fields
        .iter()
        .filter(|f| f.name.rsplit('.').next().unwrap_or(&f.name) == last)
        .collect();
    match hits.len() {
        1 => Ok(hits[0]),
        0 => Err(Error::UnknownField {
            field: name.to_string(),
            type_name: dec.type_name.clone(),
            available: dec.fields.iter().map(|f| f.name.clone()).take(12).collect(),
        }),
        _ => Err(Error::AmbiguousField {
            field: name.to_string(),
            candidates: hits.iter().map(|f| f.name.clone()).collect(),
        }),
    }
}

/// Read a named integer field's bytes as a number. Unsigned ints zero-extend,
/// signed ints sign-extend, bool reads as 0/1; anything else (pubkey, string,
/// 128-bit ints) can't be compared numerically and says so.
pub(crate) fn read_field_int(data: &[u8], f: &crate::decode::Field) -> Result<i128> {
    let bytes = f
        .offset
        .checked_add(f.size)
        .and_then(|end| data.get(f.offset..end))
        .ok_or_else(|| Error::OutOfRange {
            what: format!("{} @{}", f.name, f.offset),
            len: data.len(),
        })?;
    let le = |b: &[u8]| -> u128 { b.iter().rev().fold(0u128, |acc, &x| (acc << 8) | x as u128) };
    match (f.ty.as_str(), f.size) {
        ("bool", _) => Ok(bytes.first().is_some_and(|&b| b != 0) as i128),
        ("u8" | "u16" | "u32" | "u64", _) => Ok(le(bytes) as i128),
        ("i8" | "i16" | "i32" | "i64", n) => {
            let raw = le(bytes);
            let shift = 128 - 8 * n as u32;
            Ok(((raw as i128) << shift) >> shift) // sign-extend from n bytes
        }
        (ty, _) => Err(Error::NonNumericField {
            field: f.name.clone(),
            ty: ty.to_string(),
        }),
    }
}

impl AccountAssert {
    pub(crate) fn describe(&self) -> String {
        let a = short(&self.address);
        match &self.check {
            StateCheck::Lamports { op, value } => format!("{a} lamports {} {value}", op.symbol()),
            StateCheck::U64At { offset, op, value } => {
                format!("{a} u64@{offset} {} {value}", op.symbol())
            }
            StateCheck::LamportsDelta { op, value } => {
                format!("{a} lamports Δ {} {value}", op.symbol())
            }
            StateCheck::TokenDelta { op, value } => format!("{a} token Δ {} {value}", op.symbol()),
            StateCheck::Field { name, op, value } => format!("{a} {name} {} {value}", op.symbol()),
            StateCheck::FieldDelta { name, op, value } => {
                format!("{a} {name} Δ {} {value}", op.symbol())
            }
            StateCheck::FieldUnchanged { name } => format!("{a} {name} unchanged"),
        }
    }
}

/// The raw bytes a decoded field occupies — for `coption-*` fields that is the
/// 4-byte tag plus the payload, so a `None → Some(x)` flip counts as a change.
pub(crate) fn field_bytes<'a>(data: &'a [u8], f: &crate::decode::Field) -> Result<&'a [u8]> {
    let span = if f.ty.starts_with("coption-") {
        f.size + 4
    } else {
        f.size
    };
    f.offset
        .checked_add(span)
        .and_then(|end| data.get(f.offset..end))
        .ok_or_else(|| Error::OutOfRange {
            what: format!("{} @{}", f.name, f.offset),
            len: data.len(),
        })
}

fn short(s: &str) -> String {
    // Count by chars, not bytes: an address string is normally ASCII, but this
    // also formats arbitrary user-supplied strings (a typo'd assert address),
    // and byte-slicing one at a non-char-boundary would panic.
    let count = s.chars().count();
    if count > 12 {
        let head: String = s.chars().take(4).collect();
        let tail: String = s.chars().skip(count - 4).collect();
        format!("{head}…{tail}")
    } else {
        s.to_string()
    }
}

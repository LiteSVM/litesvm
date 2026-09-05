//! Name everything in a LiteSVM transaction.
//!
//! `litesvm-cpi-tree` turns a transaction's logs into a tree of program
//! invocations. This crate fills that tree in: every instruction's *name*, its
//! *arguments*, the *role* of each account it touches, the *events* it emitted
//! and, when it failed, the *error* by name — from the programs' Anchor IDLs
//! and from hand-written layouts for the native and SPL programs that have
//! none. It also decodes account data (SPL token accounts and mints, lookup
//! tables, stake and nonce accounts, and any Anchor account type by its
//! discriminator).
//!
//! Everything is pure and offline. IDL JSON comes from the caller through an
//! [`IdlRegistry`]; there is no RPC dependency and nothing is fetched.
//!
//! ```ignore
//! use litesvm_scope::{IdlRegistry, ScopeExt};
//!
//! let mut idls = IdlRegistry::new();
//! idls.insert(program_id, &serde_json::from_str(include_str!("idl.json"))?);
//!
//! let meta = svm.send_transaction(tx.clone())?;
//! let decoded = meta.decode(&svm, &tx, &idls);
//! for ix in &decoded.instructions {
//!     println!("{} {:?} {:?}", ix.program, ix.name, ix.args);
//! }
//! ```
//!
//! # How instructions become a tree
//!
//! Top-level instructions come from the message; CPIs come from the runtime's
//! inner-instruction list, nested by stack height. That structure is exact.
//! The logs, parsed by `litesvm-cpi-tree`, walk the *same* pre-order, so each
//! frame's outcome, compute units and emitted data are attached to the
//! instruction it belongs to (program ids are checked at every step; a
//! mismatch stops the attachment rather than mislabel anything).

use {solana_address::Address, std::collections::HashMap};

mod errors;
mod ext;
mod idl;
mod idl_model;
mod layout;
mod native;
mod tree;

pub use {
    errors::{describe_transaction_error, error_name},
    ext::ScopeExt,
    layout::decode_native_account,
    tree::{decode_instructions, format_instructions, resolve_account_keys},
};

/// Parsed IDLs keyed by program id. Insert every program you can name;
/// programs without an IDL still get native decoding where applicable.
#[derive(Default, Clone)]
pub struct IdlRegistry {
    idls: HashMap<Address, idl_model::IdlModel>,
}

impl IdlRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a program's IDL JSON (Anchor pre-0.30 and 0.30+ formats are
    /// both accepted). Parsing is best-effort: malformed entries degrade at
    /// the use site instead of rejecting the whole IDL.
    pub fn insert(&mut self, program: Address, idl: &serde_json::Value) {
        self.idls.insert(program, idl_model::IdlModel::parse(idl));
    }

    pub fn contains(&self, program: &Address) -> bool {
        self.idls.contains_key(program)
    }

    pub fn len(&self) -> usize {
        self.idls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.idls.is_empty()
    }

    pub(crate) fn get(&self, program: &Address) -> Option<&idl_model::IdlModel> {
        self.idls.get(program)
    }
}

/// One field of a decoded account or event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Dotted path for nested structs (`config.fee_bps`), indexed for arrays.
    pub name: String,
    /// Byte offset within the data.
    pub offset: usize,
    /// Type label: `u64`, `i64`, `bool`, `pubkey`, `string`, `enum Mode`,
    /// `coption-pubkey`, …
    pub ty: String,
    /// Size in bytes of the payload (for `coption-*`, the size after the tag).
    pub size: usize,
    /// The value, formatted for display.
    pub value: String,
    /// A human hint (what a numeric state code means, an enum's variant index).
    pub note: Option<String>,
}

/// An account decoded into named fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAccount {
    /// The account type: `SPL Token Account`, or the IDL account name.
    pub type_name: String,
    pub fields: Vec<Field>,
}

/// An Anchor event decoded through its program's IDL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedEvent {
    pub name: String,
    pub fields: Vec<Field>,
}

/// One decoded instruction argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedArg {
    pub name: String,
    /// Type label, e.g. `u64`.
    pub ty: String,
    /// The value formatted for display; empty when the argument sits after a
    /// variable-length one and cannot be located.
    pub value: String,
}

/// One account an instruction touches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAccountRef {
    /// The role name from the IDL (`authority`) or native layout (`Source`);
    /// `Remaining Account #n` for an Anchor program's variadic tail.
    pub name: Option<String>,
    pub address: Address,
    pub signer: bool,
    pub writable: bool,
}

/// An error, named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedError {
    /// The program that returned it, when attributable.
    pub program: Option<Address>,
    /// The custom error code, when the error carried one.
    pub code: Option<u64>,
    /// `SlippageToleranceExceeded`, `ConstraintRentExempt`, `InsufficientFunds`,
    /// `ProgramFailedToComplete`, … or `Custom error N` when nothing names it.
    pub name: String,
    /// The plain-language message from the IDL or the error table.
    pub message: Option<String>,
}

/// One instruction — top-level or CPI — with everything named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedInstruction {
    pub program: Address,
    /// Display name: `Transfer Checked`, `Route V2`, `Emit Event`. `None` when
    /// neither an IDL nor a native layout could name it.
    pub name: Option<String>,
    /// The IDL's own spelling (`route_v2`), for programmatic matching.
    pub idl_name: Option<String>,
    pub args: Vec<DecodedArg>,
    pub accounts: Vec<DecodedAccountRef>,
    /// 1 for top-level instructions, 2+ for CPIs.
    pub stack_height: u8,
    /// Compute units the program consumed, from its `consumed` log line.
    pub compute_units: Option<u64>,
    /// `Some(false)` when this instruction's own frame failed; `None` when
    /// the logs did not cover it (truncated, or not attributable).
    pub success: Option<bool>,
    /// The failure, named, on the instruction whose frame failed.
    pub error: Option<DecodedError>,
    /// Events this instruction emitted (`emit!`), decoded through its IDL.
    pub events: Vec<DecodedEvent>,
    pub children: Vec<DecodedInstruction>,
}

impl DecodedInstruction {
    /// Pre-order walk over this instruction and its CPIs.
    pub fn walk(&self) -> impl Iterator<Item = &DecodedInstruction> {
        let mut out = Vec::new();
        fn go<'a>(ix: &'a DecodedInstruction, out: &mut Vec<&'a DecodedInstruction>) {
            out.push(ix);
            for c in &ix.children {
                go(c, out);
            }
        }
        go(self, &mut out);
        out.into_iter()
    }
}

/// A whole transaction, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedTransaction {
    pub instructions: Vec<DecodedInstruction>,
    /// The transaction-level error, when it failed.
    pub error: Option<DecodedError>,
}

impl DecodedTransaction {
    /// The deepest instruction whose frame failed, if any.
    pub fn failing_instruction(&self) -> Option<&DecodedInstruction> {
        self.instructions
            .iter()
            .flat_map(|ix| ix.walk())
            .filter(|ix| ix.success == Some(false))
            .last()
    }

    /// Render as `cargo tree`-style box art.
    pub fn pretty(&self) -> String {
        format_instructions(&self.instructions)
    }
}

/// Decode an account's data: the hand-written native layouts first, then the
/// owner program's IDL by discriminator.
pub fn decode_account(
    registry: &IdlRegistry,
    owner: &Address,
    data: &[u8],
) -> Option<DecodedAccount> {
    decode_native_account(owner, data).or_else(|| {
        registry
            .get(owner)
            .and_then(|m| idl::decode_account(m, data))
    })
}

/// Decode an Anchor event payload through `program`'s IDL, or through every
/// registered IDL when the emitting program is unknown.
pub fn decode_event(
    registry: &IdlRegistry,
    program: Option<&Address>,
    data: &[u8],
) -> Option<DecodedEvent> {
    match program {
        Some(p) => registry.get(p).and_then(|m| idl::decode_event(m, data)),
        None => registry
            .idls
            .values()
            .find_map(|m| idl::decode_event(m, data)),
    }
}

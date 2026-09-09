//! On-chain Anchor IDL fetching, parsing, and account/instruction decoding.
//!
//! Anchor programs can publish their IDL to a derived on-chain account
//! (zlib-compressed JSON). This module locates and inflates it, resolves
//! custom error codes to their names, lists a program's instructions with
//! discriminators and account/argument specs, and decodes account data into
//! named fields using the IDL's type definitions.
//!
//! The JSON itself is parsed once into the typed `idl_model` and all
//! walking happens over that model; the `serde_json::Value` signatures below
//! are the stable seam callers (and the public API) hold IDLs as.

use {
    crate::{
        decode::{DecodedAccount, Field},
        idl_model::{AccountNode, FieldDef, IdlModel, IdlType, IxDef, SeedDef},
    },
    flate2::bufread::ZlibDecoder,
    serde_json::Value,
    solana_address::Address,
    std::{io::Read, str::FromStr},
};

/// The Program Metadata program (`solana-program/program-metadata`) that Anchor
/// 0.31+ and `anchor idl` publish IDLs to, and that Solana Explorer reads. A
/// program's canonical IDL lives at a PDA of `[program, "idl"-padded-16]` under
/// this program, with zlib-compressed JSON in its data.
const PROGRAM_METADATA_PROGRAM: &str = "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S";

/// Recover an Anchor program's instruction names from its compiled binary. The
/// dispatcher of every Anchor program (unless built with `no-log-ix-name`)
/// logs `Instruction: <Name>` on entry, so those literals sit in the ELF's
/// read-only data. Rust packs adjacent string literals with no terminator, so
/// each literal's true end is unknown; every identifier prefix is offered as a
/// candidate and only the one whose `sha256("global:<snake>")` matches real
/// instruction data ever surfaces. Produces a minimal IDL (names and
/// discriminators only, no accounts or args).
pub(crate) fn synthesize_from_elf(elf: &[u8], program: &str) -> Option<Value> {
    use sha2::{Digest, Sha256};
    const TAG: &[u8] = b"Instruction: ";
    let mut names = std::collections::BTreeSet::new();
    let mut at = 0;
    while let Some(pos) = elf[at..].windows(TAG.len()).position(|w| w == TAG) {
        let start = at + pos + TAG.len();
        let ident: Vec<u8> = elf[start..]
            .iter()
            .take(48)
            .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
            .copied()
            .collect();
        for n in 1..=ident.len() {
            if let Ok(s) = std::str::from_utf8(&ident[..n]) {
                if s.as_bytes()[0].is_ascii_alphabetic() {
                    names.insert(crate::utils::camel_to_snake(s));
                }
            }
        }
        at = start;
    }
    if names.is_empty() {
        return None;
    }
    let instructions: Vec<Value> = names
        .into_iter()
        .map(|name| {
            let disc: Vec<u8> = Sha256::digest(format!("global:{name}").as_bytes())[..8].to_vec();
            serde_json::json!({ "name": name, "discriminator": disc, "accounts": [], "args": [] })
        })
        .collect();
    Some(serde_json::json!({
        "address": program,
        "metadata": { "name": "recovered", "origin": "svmscope-elf",
                      "note": "instruction names recovered from the program binary; no accounts or args" },
        "instructions": instructions,
        "accounts": [],
        "types": [],
    }))
}

/// Legacy: the Anchor IDL account, a `create_with_seed(..,"anchor:idl",..)`
/// account holding `[8 disc][32 authority][u32 len][zlib(json)]`.
pub(crate) fn anchor_idl_address(program_id: &Address) -> Option<Address> {
    let base = Address::find_program_address(&[], program_id).0;
    Address::create_with_seed(&base, "anchor:idl", program_id).ok()
}

/// Parse the legacy Anchor IDL account's data.
pub(crate) fn idl_from_anchor_account(data: &[u8]) -> Option<Value> {
    let len_bytes: [u8; 4] = data.get(40..44)?.try_into().ok()?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    inflate_idl_json(data.get(44..44 + len)?)
}

/// Newer: the Program Metadata program's canonical `idl` account for a
/// program.
pub(crate) fn program_metadata_idl_address(program_id: &Address) -> Option<Address> {
    let meta = Address::from_str(PROGRAM_METADATA_PROGRAM).ok()?;
    let mut seed = b"idl".to_vec();
    seed.resize(16, 0); // canonical seed is a fixed 16-byte buffer
    Some(Address::find_program_address(&[program_id.as_ref(), &seed], &meta).0)
}

/// Parse a Program Metadata `idl` account. Its inline data is zlib-compressed
/// JSON after a small fixed header; we scan for the zlib magic and inflate
/// (robust to header-size variants), falling back to raw JSON for the
/// uncompressed (`--compression none`) case.
pub(crate) fn idl_from_program_metadata(data: &[u8]) -> Option<Value> {
    for off in 0..data.len().min(256) {
        if data[off] == 0x78 && matches!(data.get(off + 1), Some(0x01 | 0x9c | 0xda)) {
            if let Some(v) = inflate_idl_json(&data[off..]) {
                return Some(v);
            }
        }
    }
    // Uncompressed fallback: parse from the first `{`.
    let start = data.iter().position(|&b| b == b'{')?;
    serde_json::from_slice::<Value>(&data[start..]).ok()
}

/// Inflate a zlib stream into JSON, refusing to allocate more than
/// [`MAX_IDL_JSON`]. On-chain IDL bytes are untrusted; without a ceiling a
/// small high-ratio zlib payload could inflate toward gigabytes and OOM the
/// process. A truncated read past the cap simply fails to parse and returns
/// `None`, the same as any other malformed IDL.
fn inflate_idl_json(compressed: &[u8]) -> Option<Value> {
    let mut out = Vec::new();
    ZlibDecoder::new(compressed)
        .take(MAX_IDL_JSON)
        .read_to_end(&mut out)
        .ok()?;
    serde_json::from_slice::<Value>(&out).ok()
}

/// Largest inflated IDL JSON we accept. Real IDLs are a few hundred KiB at most;
/// this leaves generous headroom while capping a decompression bomb.
const MAX_IDL_JSON: u64 = 16 * 1024 * 1024;

/// A program error resolved from an IDL's `errors[]` — the difference between
/// `Custom(6024)` and "Slippage exceeded".
#[derive(serde::Serialize, Clone)]
pub struct IdlError {
    /// The numeric custom error code (e.g. 6024).
    pub code: u64,
    /// The error's name in the IDL (e.g. "SlippageToleranceExceeded").
    pub name: String,
    /// The error's human message from the IDL.
    pub msg: String,
}

/// Look up a custom error code in an IDL.
pub(crate) fn error_for_code(idl: &Value, code: u64) -> Option<IdlError> {
    IdlModel::parse(idl)
        .errors
        .into_iter()
        .find(|e| e.code == Some(code))
        .map(|e| IdlError {
            code,
            name: e.name,
            msg: e.msg,
        })
}

/// One instruction a program exposes, for the transaction builder.
#[derive(serde::Serialize)]
pub struct IdlInstruction {
    /// The instruction's IDL method name.
    pub name: String,
    /// 8-byte Anchor discriminator.
    pub discriminator: Vec<u8>,
    /// The instruction's doc lines from the IDL.
    pub docs: Vec<String>,
    /// The accounts the instruction takes, in order.
    pub accounts: Vec<IdlAccountSpec>,
    /// The arguments the instruction takes, in order.
    pub args: Vec<IdlArg>,
}

/// One account an IDL instruction requires.
#[derive(serde::Serialize)]
pub struct IdlAccountSpec {
    /// The account's IDL name (e.g. "authority").
    pub name: String,
    /// Whether the instruction may write to the account.
    pub writable: bool,
    /// Whether the account must sign the transaction.
    pub signer: bool,
    /// True when the IDL says this account is a PDA.
    pub pda: bool,
    /// The PDA's seeds, so the client can derive the address instead of making
    /// the user paste it: `const` carries literal bytes, `account` names another
    /// account in this instruction whose key is the seed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub seeds: Vec<PdaSeed>,
    /// A fixed address, when the IDL pins one (e.g. system_program).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

/// One seed of a PDA derivation.
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PdaSeed {
    /// Literal bytes.
    Const {
        /// The literal seed bytes.
        bytes: Vec<u8>,
    },
    /// The public key of another account in the same instruction.
    Account {
        /// The referenced account's IDL path.
        path: String,
    },
}

/// One argument an IDL instruction takes.
#[derive(serde::Serialize)]
pub struct IdlArg {
    /// The argument's IDL name.
    pub name: String,
    /// Wire type label, e.g. "u64" / "pubkey" / "string".
    #[serde(rename = "type")]
    pub ty: String,
}

/// Extract a program's instructions from its IDL, for building transactions.
pub fn instructions(idl: &Value) -> Vec<IdlInstruction> {
    IdlModel::parse(idl)
        .instructions
        .iter()
        .filter_map(|ix| {
            Some(IdlInstruction {
                name: ix.name.clone()?,
                discriminator: ix.discriminator.lossy_bytes()?,
                docs: ix.docs.clone(),
                accounts: ix.accounts.iter().map(account_spec).collect(),
                args: ix
                    .args
                    .iter()
                    .map(|arg| IdlArg {
                        name: arg.name.clone().unwrap_or_default(),
                        ty: arg_label(arg.ty.as_ref()),
                    })
                    .collect(),
            })
        })
        .collect()
}

/// The display label for an argument's (possibly missing) type.
fn arg_label(ty: Option<&IdlType>) -> String {
    ty.map(IdlType::label).unwrap_or_else(|| "unknown".into())
}

fn account_spec(node: &AccountNode) -> IdlAccountSpec {
    IdlAccountSpec {
        name: node.name.clone().unwrap_or_default(),
        writable: node.writable_modern(),
        signer: node.signer_modern(),
        pda: node.pda_present,
        seeds: node
            .seeds
            .iter()
            .map(|seed| match seed {
                SeedDef::Const { bytes } => PdaSeed::Const {
                    bytes: bytes.clone(),
                },
                SeedDef::Account { path } => PdaSeed::Account { path: path.clone() },
            })
            .collect(),
        address: node.address.clone(),
    }
}

/// Find the IDL instruction whose 8-byte discriminator matches this instruction's
/// data — the entry that names it and describes its args and accounts.
pub(crate) fn find_ix(idl: &Value, data: &[u8]) -> Option<IxDef> {
    IdlModel::parse(idl).instructions.into_iter().find(|ix| {
        ix.discriminator
            .lossy_bytes()
            .is_some_and(|b| !b.is_empty() && data.get(..b.len()) == Some(b.as_slice()))
    })
}

/// How many leading bytes of instruction data the discriminator occupies: 8
/// for Anchor, 1/2/4/8 for a Shank `discriminant`.
pub(crate) fn disc_len(idl_ix: &IxDef) -> usize {
    idl_ix
        .discriminator
        .lossy_bytes()
        .map(|b| b.len())
        .filter(|n| *n > 0)
        .unwrap_or(8)
}

/// Borsh-decode an Anchor instruction's arguments — the bytes after the 8-byte
/// discriminator — using the IDL instruction's `args`. Returns `(name, type,
/// value)` per arg, stopping at the first variable-length arg (offsets past it
/// are no longer trustworthy), same rule as the account-field walker.
pub(crate) fn decode_ix_args(idl_ix: &IxDef, data: &[u8]) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut off = disc_len(idl_ix); // skip the discriminator
    for arg in &idl_ix.args {
        let name = arg.name.clone().unwrap_or_default();
        match arg.ty.as_ref().and_then(resolve_fixed) {
            Some(kind) => {
                let sz = kind.size();
                let Some(bytes) = data.get(off..off + sz) else {
                    break;
                };
                out.push((name, kind.label(), read_value(bytes, kind)));
                off += sz;
            }
            None => {
                // Variable-length / composite arg — name it, but the value and
                // everything after it can't be read from a fixed offset.
                out.push((name, arg_label(arg.ty.as_ref()), String::new()));
                break;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// IDL account decoding — turn raw program-account bytes into named fields.
// ---------------------------------------------------------------------------

/// A fixed-size scalar type we know how to read from account bytes.
#[derive(Clone, Copy)]
enum Kind {
    U(usize), // unsigned int, `usize` = byte width (1,2,4,8,16)
    I(usize), // signed int
    Bool,
    Pubkey,
    Bytes(usize), // fixed byte array [u8; N]
}

impl Kind {
    /// The display label the UI shows in the "Type" column.
    fn label(self) -> String {
        match self {
            Kind::U(n) => format!("u{}", n * 8),
            Kind::I(n) => format!("i{}", n * 8),
            Kind::Bool => "bool".into(),
            Kind::Pubkey => "pubkey".into(),
            Kind::Bytes(n) => format!("[u8; {n}]"),
        }
    }
    fn size(self) -> usize {
        match self {
            Kind::U(n) | Kind::I(n) | Kind::Bytes(n) => n,
            Kind::Bool => 1,
            Kind::Pubkey => 32,
        }
    }
    /// Whether the UI should let the user edit this field.
    fn editable(self) -> bool {
        !matches!(self, Kind::Bytes(_))
    }
}

/// Resolve an IDL type to a fixed-size [`Kind`]: integer/bool/pubkey scalars
/// and fixed arrays of them resolve; anything variable-length or composite
/// (`vec`/`string`/`option`/`defined` — the last is inlined by the walker
/// itself, floats excluded as before) returns `None`, which tells the caller
/// offsets are no longer trustworthy.
fn resolve_fixed(ty: &IdlType) -> Option<Kind> {
    match ty {
        IdlType::Bool => Some(Kind::Bool),
        IdlType::U(n) => Some(Kind::U(*n)),
        IdlType::I(n) => Some(Kind::I(*n)),
        IdlType::Pubkey { .. } => Some(Kind::Pubkey),
        IdlType::Array { inner, len } => {
            let inner = resolve_fixed(inner)?;
            let count = usize::try_from(*len).ok()?;
            // Untrusted IDL: a bogus element count must not overflow the byte size.
            let size = inner.size().checked_mul(count)?;
            Some(Kind::Bytes(size))
        }
        _ => None,
    }
}

/// Format a fixed-size scalar's bytes into a human-readable value string.
fn read_value(bytes: &[u8], kind: Kind) -> String {
    match kind {
        Kind::U(_) => {
            let mut buf = [0u8; 16];
            buf[..bytes.len()].copy_from_slice(bytes);
            u128::from_le_bytes(buf).to_string()
        }
        Kind::I(n) => {
            let mut buf = [0u8; 16];
            buf[..bytes.len()].copy_from_slice(bytes);
            // sign-extend if the value is negative
            if bytes[n - 1] & 0x80 != 0 {
                for b in &mut buf[n..] {
                    *b = 0xff;
                }
            }
            i128::from_le_bytes(buf).to_string()
        }
        Kind::Bool => (bytes[0] != 0).to_string(),
        Kind::Pubkey => {
            let arr: [u8; 32] = bytes.try_into().unwrap();
            Address::from(arr).to_string()
        }
        Kind::Bytes(_) => bytes.iter().map(|b| format!("{b:02x}")).collect(),
    }
}

/// Read a little-endian u32 at `offset` (borsh length prefixes).
fn read_u32_at(data: &[u8], offset: usize) -> Option<u32> {
    let b = data.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(b.try_into().ok()?))
}

/// Deepest struct/enum/array nesting we inline before giving up. A hostile or
/// buggy IDL can define a self-referential type (`A { x: A }`); without a bound
/// this recurses until the stack overflows and aborts the process. Real Anchor
/// layouts nest only a handful deep, so this never trips on honest input.
const MAX_WALK_DEPTH: usize = 32;

/// Most elements of a fixed array we expand into indexed fields. A `[T; N]` with
/// an enormous `N` (an untrusted IDL can claim `[Empty; u64::MAX]`, and an empty
/// struct consumes zero bytes) would otherwise spin ~forever; past the cap we
/// stop cleanly the same way an oversized `vec` does.
const MAX_ARRAY_ELEMS: u64 = 1024;

/// Walk a struct's fields, appending a `Field` for each and moving `*offset`
/// forward. When a field is itself a struct, it calls itself to read that struct
/// inline (that's the recursion). Returns `false` the moment it hits something
/// variable-length or unknown — the caller stops too, because every offset after
/// that point would be wrong.
/// Total field visits one decode may perform, across every level of
/// recursion. Depth and per-array caps alone do not bound work: an array of
/// a struct that is itself an array of an empty struct multiplies visits
/// without consuming a byte of account data. A hostile on-chain IDL is
/// untrusted input; past this budget the decode stops and reports what it has.
const MAX_WALK_VISITS: usize = 20_000;

fn walk_fields(
    fields: &[FieldDef],
    model: &IdlModel,
    data: &[u8],
    offset: &mut usize,
    prefix: &str,
    out: &mut Vec<Field>,
    depth: usize,
) -> bool {
    let mut budget = MAX_WALK_VISITS;
    walk_fields_budgeted(fields, model, data, offset, prefix, out, depth, &mut budget)
}

#[allow(clippy::too_many_arguments)]
fn walk_fields_budgeted(
    fields: &[FieldDef],
    model: &IdlModel,
    data: &[u8],
    offset: &mut usize,
    prefix: &str,
    out: &mut Vec<Field>,
    depth: usize,
    budget: &mut usize,
) -> bool {
    if depth > MAX_WALK_DEPTH {
        return false;
    }
    for f in fields {
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        // The field's display name, e.g. "bump" at the top level or
        // "flat_fees.numerator" inside a nested struct.
        let fname = match &f.name {
            Some(n) => format!("{prefix}{n}"),
            None => return false,
        };
        let Some(ty) = &f.ty else {
            return false;
        };

        // Is this field a nested struct? If so, read its fields inline, right
        // here at the current cursor, by calling ourselves with a dotted prefix.
        if let IdlType::Defined(tname) = ty {
            if let Some(sub) = model.struct_fields(tname) {
                if !walk_fields_budgeted(
                    sub,
                    model,
                    data,
                    offset,
                    &format!("{fname}."),
                    out,
                    depth + 1,
                    budget,
                ) {
                    return false; // the nested struct hit something variable
                }
                continue;
            }
            // An enum is a 1-byte variant tag, then that variant's payload (if any),
            // so we can read the tag, name the variant, and walk its fields.
            if let Some(variants) = model.enum_variants(tname) {
                let Some(&tag) = data.get(*offset) else {
                    return false;
                };
                let variant = variants.get(tag as usize);
                let vname = variant.and_then(|v| v.name.as_deref()).unwrap_or("unknown");
                out.push(Field {
                    name: fname.clone(),
                    offset: *offset,
                    ty: format!("enum {tname}"),
                    size: 1,
                    value: vname.to_string(),
                    editable: false,
                    note: Some(format!("variant {tag}")),
                });
                *offset += 1;
                // Tuple/struct variants carry fields; walk them so the cursor stays true.
                if let Some(vfields) = variant.and_then(|v| v.fields.as_ref()) {
                    // Tuple variants are bare types; give them positional names.
                    let named: Vec<FieldDef> = vfields
                        .iter()
                        .enumerate()
                        .map(|(i, vf)| {
                            if vf.name_key {
                                FieldDef {
                                    name: vf.name.clone(),
                                    ty: vf.ty.clone(),
                                }
                            } else {
                                FieldDef {
                                    name: Some(i.to_string()),
                                    ty: Some(vf.whole.clone()),
                                }
                            }
                        })
                        .collect();
                    if !walk_fields_budgeted(
                        &named,
                        model,
                        data,
                        offset,
                        &format!("{fname}."),
                        out,
                        depth + 1,
                        budget,
                    ) {
                        return false;
                    }
                }
                continue;
            }
            return false; // unknown defined type → stop
        }

        // A fixed array of structs (e.g. `reward_infos: [WhirlpoolRewardInfo; 3]`)
        // expands into indexed sub-fields, read inline like a nested struct.
        if let IdlType::Array { inner, len } = ty {
            if let IdlType::Defined(tname) = inner.as_ref() {
                let Some(sub) = model.struct_fields(tname) else {
                    return false;
                };
                if *len > MAX_ARRAY_ELEMS {
                    return false;
                }
                for i in 0..*len {
                    if !walk_fields_budgeted(
                        sub,
                        model,
                        data,
                        offset,
                        &format!("{fname}[{i}]."),
                        out,
                        depth + 1,
                        budget,
                    ) {
                        return false;
                    }
                }
                continue;
            }
        }

        // Variable-length borsh types carry their length in the data, so we can
        // read them and keep walking — the offsets after them are still correct.
        //
        // string: u32 length + utf8 bytes
        if matches!(ty, IdlType::Str) {
            let Some(len) = read_u32_at(data, *offset) else {
                return false;
            };
            let start = *offset + 4;
            let end = start + len as usize;
            if end > data.len() {
                return false;
            }
            let text = String::from_utf8_lossy(&data[start..end]).to_string();
            out.push(Field {
                name: fname,
                offset: *offset,
                ty: "string".into(),
                size: 4 + len as usize,
                value: text,
                editable: false,
                note: None,
            });
            *offset = end;
            continue;
        }

        // option<T>: 1-byte tag, then T when present.
        if let IdlType::Option(inner) = ty {
            let Some(&tag) = data.get(*offset) else {
                return false;
            };
            *offset += 1;
            if tag == 0 {
                out.push(Field {
                    name: fname,
                    offset: *offset - 1,
                    ty: "option".into(),
                    size: 1,
                    value: "none".into(),
                    editable: false,
                    note: None,
                });
                continue;
            }
            // Present: fall through by walking the inner type as a single field.
            let one = [FieldDef {
                name: Some(fname),
                ty: Some((**inner).clone()),
            }];
            if !walk_fields_budgeted(&one, model, data, offset, "", out, depth + 1, budget) {
                return false;
            }
            continue;
        }

        // vec<T>: u32 count, then the elements. Walk each so the cursor stays true.
        if let IdlType::Vec(inner) = ty {
            let Some(count) = read_u32_at(data, *offset) else {
                return false;
            };
            out.push(Field {
                name: format!("{fname}.len"),
                offset: *offset,
                ty: "u32".into(),
                size: 4,
                value: count.to_string(),
                editable: false,
                note: None,
            });
            *offset += 4;
            // Cap how many elements we expand so a huge vec can't flood the UI;
            // beyond the cap we can't know the size, so stop cleanly.
            const MAX_ELEMS: u32 = 32;
            if count > MAX_ELEMS {
                return false;
            }
            for i in 0..count {
                let one = [FieldDef {
                    name: Some(format!("{fname}[{i}]")),
                    ty: Some((**inner).clone()),
                }];
                if !walk_fields_budgeted(&one, model, data, offset, "", out, depth + 1, budget) {
                    return false;
                }
            }
            continue;
        }

        // Otherwise it's a plain scalar or fixed array. Read it and advance.
        let kind = match resolve_fixed(ty) {
            Some(k) => k,
            None => return false, // unknown type → stop
        };
        let size = kind.size();
        let Some(end) = offset.checked_add(size) else {
            return false;
        };
        if end > data.len() {
            return false;
        }
        out.push(Field {
            name: fname,
            offset: *offset,
            ty: kind.label(),
            size,
            value: read_value(&data[*offset..end], kind),
            editable: kind.editable(),
            note: None,
        });
        *offset = end;
    }
    true
}

/// Decode an Anchor event payload (the base64 body of a `Program data:` log
/// line) by its 8-byte discriminator. Supports the new IDL format (events carry
/// an explicit discriminator and their fields live in `types`) and the legacy
/// one (`sha256("event:Name")[..8]`, fields inline on the event).
pub(crate) fn decode_event(idl: &Value, data: &[u8]) -> Option<DecodedAccount> {
    use sha2::{Digest, Sha256};
    if data.len() < 8 {
        return None;
    }
    let disc = &data[..8];
    let events = idl.get("events")?.as_array()?;
    let ev = events.iter().find(|e| {
        let Some(n) = e.get("name").and_then(|n| n.as_str()) else {
            return false;
        };
        match e.get("discriminator").and_then(|d| d.as_array()) {
            Some(arr) => {
                let bytes: Vec<u8> = arr
                    .iter()
                    .filter_map(|b| b.as_u64())
                    .map(|b| b as u8)
                    .collect();
                bytes == disc
            }
            None => Sha256::digest(format!("event:{n}").as_bytes())[..8] == *disc,
        }
    })?;
    let name = ev.get("name")?.as_str()?.to_string();
    let model = IdlModel::parse(idl);
    let (fields_def, model) = match model.type_def(&name).and_then(|t| t.raw_fields.clone()) {
        Some(f) => (f, model),
        None => {
            // Legacy: fields inline on the event. Build a one-type model so the
            // same walker can read them.
            let synth = serde_json::json!({
                "types": [{ "name": name, "type": { "kind": "struct", "fields": ev.get("fields").cloned().unwrap_or(Value::Array(vec![])) } }]
            });
            let m2 = IdlModel::parse(&synth);
            let f = m2.type_def(&name).and_then(|t| t.raw_fields.clone())?;
            (f, m2)
        }
    };
    let mut fields: Vec<Field> = Vec::new();
    let mut offset = 8usize;
    walk_fields(&fields_def, &model, data, &mut offset, "", &mut fields, 0);
    Some(DecodedAccount {
        type_name: name,
        fields,
    })
}

/// Where a named instruction argument sits in the instruction data: `(offset,
/// size, type label)`. Only resolvable while every preceding argument is a
/// fixed-size scalar; a `vec`/`string`/`option` before it makes the offset
/// unknowable without a full decode.
pub(crate) fn ix_arg_span(idl_ix: &IxDef, arg: &str) -> Option<(usize, usize, String)> {
    let mut off = disc_len(idl_ix);
    for a in &idl_ix.args {
        let kind = a.ty.as_ref().and_then(resolve_fixed)?;
        if a.name.as_deref() == Some(arg) {
            return Some((off, kind.size(), kind.label()));
        }
        off = off.checked_add(kind.size())?;
    }
    None
}

/// Encode a JSON value as the little-endian bytes of a fixed scalar (`u64`,
/// `i32`, `bool`, `pubkey`, …), sized exactly `size`.
pub(crate) fn encode_fixed(label: &str, size: usize, value: &Value) -> Option<Vec<u8>> {
    let as_i128 = |v: &Value| -> Option<i128> {
        if let Some(n) = v.as_i64() {
            return Some(n as i128);
        }
        if let Some(n) = v.as_u64() {
            return Some(n as i128);
        }
        v.as_str()?.trim().parse::<i128>().ok()
    };
    Some(match label {
        "bool" => vec![u8::from(
            value.as_bool().or_else(|| as_i128(value).map(|n| n != 0))?,
        )],
        "pubkey" => {
            let s = value.as_str()?;
            Address::from_str(s).ok()?.to_bytes().to_vec()
        }
        l if l.starts_with('u') => crate::idl_encode::int_bytes(as_i128(value)?, size, false)?,
        l if l.starts_with('i') => crate::idl_encode::int_bytes(as_i128(value)?, size, true)?,
        _ => return None,
    })
}

/// Decode an account's raw bytes using its program's Anchor IDL.
///
/// Returns `None` if the leading 8-byte discriminator doesn't match any account
/// type in the IDL. Fields are walked from offset 8 (Anchor prepends the
/// discriminator) and stop at the first variable-length field.
pub(crate) fn decode_with_idl(idl: &Value, data: &[u8]) -> Option<DecodedAccount> {
    if data.len() < 8 {
        return None;
    }
    let disc = &data[0..8];
    let model = IdlModel::parse(idl);

    // 1. Match the discriminator to an account name.
    let type_name = model
        .accounts
        .iter()
        .find(|a| a.matches(disc))?
        .name
        .clone()?;

    // 2. Find that type's field layout in the IDL's `types`. (The old code read
    //    `type.fields` here without checking the kind; `raw_fields` keeps that.)
    let type_def = model.type_def(&type_name)?;
    let fields_def = type_def.raw_fields.as_ref()?;

    // 3. Walk the fields from offset 8 (Anchor prepends the discriminator),
    //    reading each into `fields` and stopping at the first variable field.
    let mut fields: Vec<Field> = Vec::new();
    let mut offset = 8usize;
    walk_fields(fields_def, &model, data, &mut offset, "", &mut fields, 0);

    Some(DecodedAccount { type_name, fields })
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    // Builds an account IDL whose sole account type is `Node`, matched by the
    // discriminator 1..=8, with the given field/type shape.
    fn account_idl(node_fields: Value, extra_types: Value) -> Value {
        let mut types = vec![json!({
            "name": "Node",
            "type": { "kind": "struct", "fields": node_fields }
        })];
        if let Some(arr) = extra_types.as_array() {
            types.extend(arr.iter().cloned());
        }
        json!({
            "accounts": [{ "name": "Node", "discriminator": [1,2,3,4,5,6,7,8] }],
            "types": types,
        })
    }

    // These decode calls must terminate. A hang or stack overflow fails the test
    // by never returning (the process would abort), which is the regression we
    // are guarding against — a hostile on-chain IDL must not be able to wedge the
    // decoder.

    #[test]
    fn self_referential_idl_type_terminates() {
        let idl = account_idl(
            json!([{ "name": "next", "type": { "defined": { "name": "Node" } } }]),
            json!([]),
        );
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        data.resize(8 + 4096, 0);
        // Returns (Some or None) — the point is that it returns at all.
        let _ = decode_with_idl(&idl, &data);
    }

    #[test]
    fn mutually_recursive_idl_types_terminate() {
        let idl = account_idl(
            json!([{ "name": "b", "type": { "defined": { "name": "B" } } }]),
            json!([{
                "name": "B",
                "type": { "kind": "struct", "fields": [
                    { "name": "a", "type": { "defined": { "name": "Node" } } }
                ] }
            }]),
        );
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        data.resize(8 + 4096, 0);
        let _ = decode_with_idl(&idl, &data);
    }

    #[test]
    fn huge_fixed_array_of_empty_struct_terminates() {
        let idl = account_idl(
            json!([{
                "name": "items",
                "type": { "array": [{ "defined": { "name": "Empty" } }, u64::MAX] }
            }]),
            json!([{
                "name": "Empty",
                "type": { "kind": "struct", "fields": [] }
            }]),
        );
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        data.resize(8 + 64, 0);
        let _ = decode_with_idl(&idl, &data);
    }

    #[test]
    fn oversized_fixed_array_size_does_not_overflow() {
        // resolve_fixed must not overflow computing element_size * count.
        let ty = json!({ "array": ["u64", u64::MAX] });
        assert!(resolve_fixed(&IdlType::parse(&ty)).is_none());
    }
}

#[cfg(test)]
mod recovery_tests {
    use {
        super::*,
        sha2::{Digest, Sha256},
    };

    #[test]
    fn legacy_anchor_idl_matches_by_derived_discriminator() {
        // Pre-0.30 IDL: camelCase names, no `discriminator` key.
        let idl = serde_json::json!({ "instructions": [
            { "name": "initializeV2", "accounts": [], "args": [] },
            { "name": "swap", "accounts": [], "args": [] }
        ]});
        let mut data = Sha256::digest(b"global:initialize_v2")[..8].to_vec();
        data.extend([9, 9, 9]);
        let ix = find_ix(&idl, &data).expect("legacy discriminator derived");
        assert_eq!(ix.name.as_deref(), Some("initializeV2"));
        assert_eq!(disc_len(&ix), 8);
        assert!(find_ix(&idl, &[0u8; 8]).is_none());
    }

    #[test]
    fn instruction_names_recovered_from_packed_rodata() {
        // Two Anchor log literals packed back to back, then unrelated text.
        let elf = b"\x7fELF....Instruction: BuyInstruction: SellAnchorError occurred...".to_vec();
        let idl = synthesize_from_elf(&elf, "Prog").expect("names found");
        let disc = |n: &str| Sha256::digest(format!("global:{n}").as_bytes())[..8].to_vec();
        assert_eq!(
            find_ix(&idl, &disc("buy")).unwrap().name.as_deref(),
            Some("buy")
        );
        assert_eq!(
            find_ix(&idl, &disc("sell")).unwrap().name.as_deref(),
            Some("sell")
        );
        // The over-long packed prefix is a candidate but never matches real data.
        assert!(find_ix(&idl, &disc("sell_anchor_error")).is_some());
        assert!(find_ix(&idl, &[0u8; 8]).is_none());
        assert!(synthesize_from_elf(b"no log literals here", "Prog").is_none());
    }
}

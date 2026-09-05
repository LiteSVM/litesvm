//! Decoding against a parsed Anchor IDL: instruction discriminators and Borsh
//! arguments, account layouts, events and custom error codes. Pure: the IDL
//! JSON comes from the caller (see [`crate::IdlRegistry`]); nothing here
//! touches the network.

use {
    crate::{
        idl_model::{FieldDef, IdlModel, IdlType, IxDef},
        DecodedAccount, DecodedArg, DecodedError, DecodedEvent, Field,
    },
    sha2::{Digest, Sha256},
    solana_address::Address,
};

/// The IDL instruction whose 8-byte discriminator opens `data`.
pub(crate) fn find_ix<'a>(model: &'a IdlModel, data: &[u8]) -> Option<&'a IxDef> {
    let disc = data.get(0..8)?;
    model
        .instructions
        .iter()
        .find(|ix| ix.discriminator.lossy_bytes().is_some_and(|b| b == disc))
}

/// Borsh-decode an instruction's arguments (the bytes after the discriminator)
/// using its IDL definition. Stops at the first variable-length argument:
/// offsets past it cannot be trusted, so later args are named without values.
pub(crate) fn decode_ix_args(ix: &IxDef, data: &[u8]) -> Vec<DecodedArg> {
    let mut out = Vec::new();
    let mut off = 8usize;
    for arg in &ix.args {
        let name = arg.name.clone().unwrap_or_default();
        match arg.ty.as_ref().and_then(resolve_fixed) {
            Some(kind) => {
                let sz = kind.size();
                let Some(bytes) = data.get(off..off + sz) else {
                    break;
                };
                out.push(DecodedArg {
                    name,
                    ty: kind.label(),
                    value: read_value(bytes, kind),
                });
                off += sz;
            }
            None => {
                out.push(DecodedArg {
                    name,
                    ty: arg_label(arg.ty.as_ref()),
                    value: String::new(),
                });
                break;
            }
        }
    }
    out
}

/// A custom error code resolved through the IDL's `errors[]`.
pub(crate) fn error_for_code(
    model: &IdlModel,
    program: Address,
    code: u64,
) -> Option<DecodedError> {
    model
        .errors
        .iter()
        .find(|e| e.code == Some(code))
        .map(|e| DecodedError {
            program: Some(program),
            code: Some(code),
            name: e.name.clone(),
            message: Some(e.msg.clone()),
        })
}

fn arg_label(ty: Option<&IdlType>) -> String {
    ty.map(IdlType::label).unwrap_or_else(|| "unknown".into())
}

/// A fixed-size scalar we can read straight out of account or instruction bytes.
#[derive(Clone, Copy)]
enum Kind {
    U(usize),
    I(usize),
    Bool,
    Pubkey,
    Bytes(usize),
}

impl Kind {
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
}

/// Integer/bool/pubkey scalars and fixed arrays of them resolve; anything
/// variable-length or composite returns `None` (the walker handles those).
fn resolve_fixed(ty: &IdlType) -> Option<Kind> {
    match ty {
        IdlType::Bool => Some(Kind::Bool),
        IdlType::U(n) => Some(Kind::U(*n)),
        IdlType::I(n) => Some(Kind::I(*n)),
        IdlType::Pubkey { .. } => Some(Kind::Pubkey),
        IdlType::Array { inner, len } => {
            let inner = resolve_fixed(inner)?;
            let count = usize::try_from(*len).ok()?;
            // Untrusted IDL: a bogus element count must not overflow the size.
            Some(Kind::Bytes(inner.size().checked_mul(count)?))
        }
        _ => None,
    }
}

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

fn read_u32_at(data: &[u8], offset: usize) -> Option<u32> {
    let b = data.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(b.try_into().ok()?))
}

/// Deepest struct/enum/array nesting inlined before giving up: a hostile IDL
/// can define a self-referential type, which would otherwise recurse until the
/// stack overflows.
const MAX_WALK_DEPTH: usize = 32;
/// Most elements of a fixed array expanded into indexed fields (an untrusted
/// `[Empty; u64::MAX]` consumes zero bytes per element and would spin forever).
const MAX_ARRAY_ELEMS: u64 = 1024;
/// Most `vec` elements expanded before stopping cleanly.
const MAX_VEC_ELEMS: u32 = 32;

/// Walk a struct's fields from `*offset`, appending a [`Field`] per leaf and
/// inlining nested structs, enums, options, vecs, strings and fixed arrays.
/// Returns `false` at the first thing it cannot size — every later offset
/// would be wrong, so the caller stops too.
fn walk_fields(
    fields: &[FieldDef],
    model: &IdlModel,
    data: &[u8],
    offset: &mut usize,
    prefix: &str,
    out: &mut Vec<Field>,
    depth: usize,
) -> bool {
    if depth > MAX_WALK_DEPTH {
        return false;
    }
    for f in fields {
        let fname = match &f.name {
            Some(n) => format!("{prefix}{n}"),
            None => return false,
        };
        let Some(ty) = &f.ty else {
            return false;
        };

        if let IdlType::Defined(tname) = ty {
            if let Some(sub) = model.struct_fields(tname) {
                if !walk_fields(
                    sub,
                    model,
                    data,
                    offset,
                    &format!("{fname}."),
                    out,
                    depth + 1,
                ) {
                    return false;
                }
                continue;
            }
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
                    note: Some(format!("variant {tag}")),
                });
                *offset += 1;
                if let Some(vfields) = variant.and_then(|v| v.fields.as_ref()) {
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
                    if !walk_fields(
                        &named,
                        model,
                        data,
                        offset,
                        &format!("{fname}."),
                        out,
                        depth + 1,
                    ) {
                        return false;
                    }
                }
                continue;
            }
            return false;
        }

        if let IdlType::Array { inner, len } = ty {
            if let IdlType::Defined(tname) = inner.as_ref() {
                let Some(sub) = model.struct_fields(tname) else {
                    return false;
                };
                if *len > MAX_ARRAY_ELEMS {
                    return false;
                }
                for i in 0..*len {
                    if !walk_fields(
                        sub,
                        model,
                        data,
                        offset,
                        &format!("{fname}[{i}]."),
                        out,
                        depth + 1,
                    ) {
                        return false;
                    }
                }
                continue;
            }
        }

        if matches!(ty, IdlType::Str) {
            let Some(len) = read_u32_at(data, *offset) else {
                return false;
            };
            let start = *offset + 4;
            let end = start + len as usize;
            if end > data.len() {
                return false;
            }
            out.push(Field {
                name: fname,
                offset: *offset,
                ty: "string".into(),
                size: 4 + len as usize,
                value: String::from_utf8_lossy(&data[start..end]).to_string(),
                note: None,
            });
            *offset = end;
            continue;
        }

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
                    note: None,
                });
                continue;
            }
            let one = [FieldDef {
                name: Some(fname),
                ty: Some((**inner).clone()),
            }];
            if !walk_fields(&one, model, data, offset, "", out, depth + 1) {
                return false;
            }
            continue;
        }

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
                note: None,
            });
            *offset += 4;
            if count > MAX_VEC_ELEMS {
                return false;
            }
            for i in 0..count {
                let one = [FieldDef {
                    name: Some(format!("{fname}[{i}]")),
                    ty: Some((**inner).clone()),
                }];
                if !walk_fields(&one, model, data, offset, "", out, depth + 1) {
                    return false;
                }
            }
            continue;
        }

        let Some(kind) = resolve_fixed(ty) else {
            return false;
        };
        let size = kind.size();
        if *offset + size > data.len() {
            return false;
        }
        out.push(Field {
            name: fname,
            offset: *offset,
            ty: kind.label(),
            size,
            value: read_value(&data[*offset..*offset + size], kind),
            note: None,
        });
        *offset += size;
    }
    true
}

/// Decode an account's bytes by matching its 8-byte discriminator against the
/// IDL's account types and walking that type's fields from offset 8.
pub(crate) fn decode_account(model: &IdlModel, data: &[u8]) -> Option<DecodedAccount> {
    if data.len() < 8 {
        return None;
    }
    let type_name = model
        .accounts
        .iter()
        .find(|a| a.matches(&data[0..8]))?
        .name
        .clone()?;
    let fields_def = model.type_def(&type_name)?.raw_fields.as_ref()?;
    let mut fields = Vec::new();
    let mut offset = 8usize;
    walk_fields(fields_def, model, data, &mut offset, "", &mut fields, 0);
    Some(DecodedAccount { type_name, fields })
}

/// Decode an Anchor event payload (the bytes of a `Program data:` log line, or
/// an `emit_cpi!` instruction's data after its own 8-byte prefix) by its
/// discriminator: explicit in new-format IDLs, `sha256("event:Name")[..8]` in
/// legacy ones.
pub(crate) fn decode_event(model: &IdlModel, data: &[u8]) -> Option<DecodedEvent> {
    if data.len() < 8 {
        return None;
    }
    let disc = &data[..8];
    let ev = model.events.iter().find(|e| {
        let Some(name) = e.name.as_deref() else {
            return false;
        };
        match e.discriminator.lossy_bytes() {
            Some(bytes) => bytes == disc,
            None => Sha256::digest(format!("event:{name}").as_bytes())[..8] == *disc,
        }
    })?;
    let name = ev.name.clone()?;
    let fields_def = match model.type_def(&name).and_then(|t| t.raw_fields.as_ref()) {
        Some(f) => f.clone(),
        None => ev.fields.clone()?,
    };
    let mut fields = Vec::new();
    let mut offset = 8usize;
    walk_fields(&fields_def, model, data, &mut offset, "", &mut fields, 0);
    Some(DecodedEvent { name, fields })
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    fn account_idl(node_fields: serde_json::Value, extra_types: serde_json::Value) -> IdlModel {
        let mut types = vec![json!({
            "name": "Node",
            "type": { "kind": "struct", "fields": node_fields }
        })];
        if let Some(arr) = extra_types.as_array() {
            types.extend(arr.iter().cloned());
        }
        IdlModel::parse(&json!({
            "accounts": [{ "name": "Node", "discriminator": [1,2,3,4,5,6,7,8] }],
            "types": types,
        }))
    }

    // A hostile on-chain IDL must not be able to wedge the decoder: these must
    // return (Some or None), never hang or overflow the stack.

    #[test]
    fn self_referential_idl_type_terminates() {
        let idl = account_idl(
            json!([{ "name": "next", "type": { "defined": { "name": "Node" } } }]),
            json!([]),
        );
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        data.resize(8 + 4096, 0);
        let _ = decode_account(&idl, &data);
    }

    #[test]
    fn mutually_recursive_idl_types_terminate() {
        let idl = account_idl(
            json!([{ "name": "b", "type": { "defined": { "name": "B" } } }]),
            json!([{ "name": "B", "type": { "kind": "struct", "fields": [
                { "name": "a", "type": { "defined": { "name": "Node" } } }
            ] } }]),
        );
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        data.resize(8 + 4096, 0);
        let _ = decode_account(&idl, &data);
    }

    #[test]
    fn huge_fixed_array_of_empty_struct_terminates() {
        let idl = account_idl(
            json!([{ "name": "items", "type": { "array": [{ "defined": { "name": "Empty" } }, u64::MAX] } }]),
            json!([{ "name": "Empty", "type": { "kind": "struct", "fields": [] } }]),
        );
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        data.resize(8 + 64, 0);
        let _ = decode_account(&idl, &data);
    }

    #[test]
    fn oversized_fixed_array_size_does_not_overflow() {
        assert!(resolve_fixed(&IdlType::parse(&json!({ "array": ["u64", u64::MAX] }))).is_none());
    }

    #[test]
    fn walks_scalars_strings_options_vecs_and_enums() {
        let idl = account_idl(
            json!([
                { "name": "count", "type": "u64" },
                { "name": "owner", "type": "pubkey" },
                { "name": "label", "type": "string" },
                { "name": "maybe", "type": { "option": "u8" } },
                { "name": "list", "type": { "vec": "u16" } },
                { "name": "mode", "type": { "defined": { "name": "Mode" } } },
                { "name": "delta", "type": "i32" }
            ]),
            json!([{ "name": "Mode", "type": { "kind": "enum", "variants": [{ "name": "Off" }, { "name": "On" }] } }]),
        );
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        data.extend_from_slice(&42u64.to_le_bytes());
        data.extend_from_slice(&[7u8; 32]);
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(b"hi");
        data.push(1);
        data.push(9);
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&300u16.to_le_bytes());
        data.extend_from_slice(&400u16.to_le_bytes());
        data.push(1);
        data.extend_from_slice(&(-5i32).to_le_bytes());
        let dec = decode_account(&idl, &data).unwrap();
        let get = |n: &str| {
            dec.fields
                .iter()
                .find(|f| f.name == n)
                .map(|f| f.value.clone())
                .unwrap_or_default()
        };
        assert_eq!(dec.type_name, "Node");
        assert_eq!(get("count"), "42");
        assert_eq!(get("owner"), Address::from([7u8; 32]).to_string());
        assert_eq!(get("label"), "hi");
        assert_eq!(get("maybe"), "9");
        assert_eq!(get("list.len"), "2");
        assert_eq!(get("list[1]"), "400");
        assert_eq!(get("mode"), "On");
        assert_eq!(get("delta"), "-5");
    }

    #[test]
    fn instruction_args_and_errors_resolve() {
        let model = IdlModel::parse(&json!({
            "instructions": [{
                "name": "transfer_out",
                "discriminator": [9, 9, 9, 9, 9, 9, 9, 9],
                "accounts": [{ "name": "vault", "writable": true }, { "name": "authority", "signer": true }],
                "args": [{ "name": "amount", "type": "u64" }, { "name": "memo", "type": "string" }, { "name": "after", "type": "u8" }]
            }],
            "errors": [{ "code": 6001, "name": "SlippageExceeded", "msg": "Slippage tolerance exceeded" }]
        }));
        let mut data = vec![9u8; 8];
        data.extend_from_slice(&1_500u64.to_le_bytes());
        let ix = find_ix(&model, &data).unwrap();
        assert_eq!(ix.name.as_deref(), Some("transfer_out"));
        let args = decode_ix_args(ix, &data);
        assert_eq!(
            (args[0].name.as_str(), args[0].value.as_str()),
            ("amount", "1500")
        );
        // The string stops the fixed walk: named, no value, nothing after it.
        assert_eq!(
            (args[1].name.as_str(), args[1].value.as_str()),
            ("memo", "")
        );
        assert_eq!(args.len(), 2);
        assert!(find_ix(&model, &[1, 2, 3]).is_none());
        let err = error_for_code(&model, Address::default(), 6001).unwrap();
        assert_eq!(err.name, "SlippageExceeded");
        assert!(error_for_code(&model, Address::default(), 6002).is_none());
    }

    #[test]
    fn events_decode_in_both_idl_formats() {
        let modern = IdlModel::parse(&json!({
            "events": [{ "name": "Swapped", "discriminator": [1, 1, 1, 1, 1, 1, 1, 1] }],
            "types": [{ "name": "Swapped", "type": { "kind": "struct", "fields": [{ "name": "amount", "type": "u64" }] } }]
        }));
        let mut data = vec![1u8; 8];
        data.extend_from_slice(&77u64.to_le_bytes());
        let ev = decode_event(&modern, &data).unwrap();
        assert_eq!(
            (ev.name.as_str(), ev.fields[0].value.as_str()),
            ("Swapped", "77")
        );

        let legacy = IdlModel::parse(&json!({
            "events": [{ "name": "Claimed", "fields": [{ "name": "who", "type": "publicKey", "index": false }] }]
        }));
        let mut data = Sha256::digest(b"event:Claimed")[..8].to_vec();
        data.extend_from_slice(&[3u8; 32]);
        let ev = decode_event(&legacy, &data).unwrap();
        assert_eq!(ev.name, "Claimed");
        assert_eq!(ev.fields[0].value, Address::from([3u8; 32]).to_string());
        assert!(decode_event(&legacy, &[0u8; 8]).is_none());
    }
}

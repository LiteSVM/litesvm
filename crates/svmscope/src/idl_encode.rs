//! Dynamic Borsh encoding for values described by an Anchor IDL.
//!
//! The encoder walks the typed [`crate::idl_model`] — argument values arrive as
//! JSON, the shapes come from the model, and every mismatch is a typed
//! [`Error::ArgumentEncoding`] carrying the dotted path (`config.limits[1]`)
//! where it happened.

use {
    crate::{
        error::{Error, Result},
        idl_model::{FieldDef, IdlModel, IdlType, IxDef, TypeBody, VariantDef},
    },
    serde_json::{Map, Value},
    solana_address::Address,
    std::str::FromStr,
};

pub(crate) fn encode_arguments(
    model: &IdlModel,
    instruction: &IxDef,
    args: &Map<String, Value>,
    output: &mut Vec<u8>,
) -> Result<()> {
    for argument in &instruction.args {
        let name = argument
            .name
            .as_deref()
            .ok_or_else(|| Error::InvalidSpec("IDL argument is missing its name".into()))?;
        let ty = argument
            .ty
            .as_ref()
            .ok_or_else(|| Error::InvalidSpec(format!("IDL argument {name} has no type")))?;
        let value = args.get(name).ok_or_else(|| Error::MissingArgument {
            method: instruction
                .name
                .clone()
                .unwrap_or_else(|| "<unknown>".to_string()),
            argument: name.to_string(),
        })?;
        encode_value(model, name, ty, value, output)?;
    }
    Ok(())
}

fn encode_value(
    model: &IdlModel,
    path: &str,
    ty: &IdlType,
    value: &Value,
    output: &mut Vec<u8>,
) -> Result<()> {
    match ty {
        IdlType::Bool => {
            output.push(
                value
                    .as_bool()
                    .ok_or_else(|| encoding_error(path, "expected bool"))? as u8,
            );
            Ok(())
        }
        IdlType::U(width) => encode_unsigned(path, *width, value, output),
        IdlType::I(width) => encode_signed(path, *width, value, output),
        IdlType::F32 => {
            let number = value
                .as_f64()
                .ok_or_else(|| encoding_error(path, "expected f32"))?
                as f32;
            output.extend_from_slice(&number.to_le_bytes());
            Ok(())
        }
        IdlType::F64 => {
            let number = value
                .as_f64()
                .ok_or_else(|| encoding_error(path, "expected f64"))?;
            output.extend_from_slice(&number.to_le_bytes());
            Ok(())
        }
        IdlType::Pubkey { .. } => {
            let address = value
                .as_str()
                .and_then(|address| Address::from_str(address).ok())
                .ok_or_else(|| encoding_error(path, "expected a base58 public key"))?;
            output.extend_from_slice(address.as_ref());
            Ok(())
        }
        IdlType::Str => {
            let string = value
                .as_str()
                .ok_or_else(|| encoding_error(path, "expected string"))?;
            write_len(path, string.len(), output)?;
            output.extend_from_slice(string.as_bytes());
            Ok(())
        }
        IdlType::Bytes => {
            let bytes = json_bytes(path, value)?;
            write_len(path, bytes.len(), output)?;
            output.extend_from_slice(&bytes);
            Ok(())
        }
        IdlType::Vec(inner) => {
            let values = value
                .as_array()
                .ok_or_else(|| encoding_error(path, "expected an array"))?;
            write_len(path, values.len(), output)?;
            for (index, value) in values.iter().enumerate() {
                encode_value(model, &format!("{path}[{index}]"), inner, value, output)?;
            }
            Ok(())
        }
        IdlType::Option(inner) => {
            if value.is_null() {
                output.push(0);
            } else {
                output.push(1);
                encode_value(model, path, inner, value, output)?;
            }
            Ok(())
        }
        IdlType::Array { inner, len } => {
            let expected = usize::try_from(*len)
                .map_err(|_| encoding_error(path, "array type has an invalid length"))?;
            let values = value
                .as_array()
                .ok_or_else(|| encoding_error(path, "expected an array"))?;
            if values.len() != expected {
                return Err(encoding_error(
                    path,
                    format!("expected {expected} elements, got {}", values.len()),
                ));
            }
            for (index, value) in values.iter().enumerate() {
                encode_value(model, &format!("{path}[{index}]"), inner, value, output)?;
            }
            Ok(())
        }
        IdlType::ArrayMalformed { missing_elem: true } => Err(encoding_error(
            path,
            "array type is missing its element type",
        )),
        IdlType::ArrayMalformed {
            missing_elem: false,
        } => Err(encoding_error(path, "array type has an invalid length")),
        IdlType::Defined(name) => encode_defined(model, path, name, value, output),
        IdlType::Unknown {
            raw,
            primitive: true,
            ..
        } => Err(encoding_error(path, format!("unsupported primitive {raw}"))),
        IdlType::Unknown {
            raw,
            primitive: false,
            ..
        } => Err(encoding_error(path, format!("unsupported IDL type {raw}"))),
    }
}

/// Encode an unsigned integer of `width` bytes, range-checked exactly like the
/// old per-type `try_from` (no silent truncation).
/// The little-endian bytes of an integer that fits `width` bytes as
/// unsigned (`signed == false`) or two's-complement signed — or `None` when
/// it does not. The single range-check-and-emit used by every encoder in the
/// crate (instruction arguments, account fields, fixed IDL scalars).
pub(crate) fn int_bytes(value: i128, width: usize, signed: bool) -> Option<Vec<u8>> {
    if signed {
        int_bytes_i(value, width)
    } else {
        int_bytes_u(u128::try_from(value).ok()?, width)
    }
}

/// Little-endian bytes of an unsigned integer that fits `width` bytes.
pub(crate) fn int_bytes_u(value: u128, width: usize) -> Option<Vec<u8>> {
    if width == 0 || width > 16 {
        return None;
    }
    if width < 16 && value >= (1u128 << (width * 8)) {
        return None;
    }
    Some(value.to_le_bytes()[..width].to_vec())
}

/// Little-endian two's-complement bytes of a signed integer that fits `width` bytes.
pub(crate) fn int_bytes_i(value: i128, width: usize) -> Option<Vec<u8>> {
    if width == 0 || width > 16 {
        return None;
    }
    if width < 16 {
        let bound = 1i128 << (width * 8 - 1);
        if value < -bound || value >= bound {
            return None;
        }
    }
    Some(value.to_le_bytes()[..width].to_vec())
}

fn encode_unsigned(path: &str, width: usize, value: &Value, output: &mut Vec<u8>) -> Result<()> {
    let expected = || encoding_error(path, format!("expected u{}", width * 8));
    let parsed = parse_u128(value).ok_or_else(expected)?;
    output.extend_from_slice(&int_bytes_u(parsed, width).ok_or_else(expected)?);
    Ok(())
}

/// Encode a signed integer of `width` bytes; the low bytes of the (range-
/// checked) two's-complement i128 are exactly the narrow encoding.
fn encode_signed(path: &str, width: usize, value: &Value, output: &mut Vec<u8>) -> Result<()> {
    let expected = || encoding_error(path, format!("expected i{}", width * 8));
    let parsed = parse_i128(value).ok_or_else(expected)?;
    output.extend_from_slice(&int_bytes_i(parsed, width).ok_or_else(expected)?);
    Ok(())
}

fn encode_defined(
    model: &IdlModel,
    path: &str,
    name: &str,
    value: &Value,
    output: &mut Vec<u8>,
) -> Result<()> {
    let definition = model
        .type_def(name)
        .ok_or_else(|| encoding_error(path, format!("defined type {name} was not found")))?;
    match &definition.body {
        TypeBody::NoBody => Err(encoding_error(
            path,
            format!("defined type {name} has no body"),
        )),
        TypeBody::Struct { fields } => encode_struct(model, path, fields.as_deref(), value, output),
        TypeBody::Enum { variants } => encode_enum(model, path, variants.as_deref(), value, output),
        TypeBody::Other { kind: Some(kind) } => Err(encoding_error(
            path,
            format!("unsupported defined type kind {kind}"),
        )),
        TypeBody::Other { kind: None } => Err(encoding_error(
            path,
            format!("defined type {name} has no kind"),
        )),
    }
}

fn encode_struct(
    model: &IdlModel,
    path: &str,
    fields: Option<&[FieldDef]>,
    value: &Value,
    output: &mut Vec<u8>,
) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| encoding_error(path, "expected an object"))?;
    for field in fields.into_iter().flatten() {
        let name = field
            .name
            .as_deref()
            .ok_or_else(|| encoding_error(path, "struct field has no name"))?;
        let ty = field
            .ty
            .as_ref()
            .ok_or_else(|| encoding_error(path, format!("struct field {name} has no type")))?;
        let value = object
            .get(name)
            .ok_or_else(|| encoding_error(&format!("{path}.{name}"), "missing field"))?;
        encode_value(model, &format!("{path}.{name}"), ty, value, output)?;
    }
    Ok(())
}

fn encode_enum(
    model: &IdlModel,
    path: &str,
    variants: Option<&[VariantDef]>,
    value: &Value,
    output: &mut Vec<u8>,
) -> Result<()> {
    let (variant_name, payload) = if let Some(name) = value.as_str() {
        (name, None)
    } else {
        let object = value
            .as_object()
            .ok_or_else(|| encoding_error(path, "expected a variant name or object"))?;
        if object.len() != 1 {
            return Err(encoding_error(
                path,
                "enum objects must contain exactly one variant",
            ));
        }
        let (name, payload) = object.iter().next().expect("length checked");
        (name.as_str(), Some(payload))
    };
    let variants = variants.ok_or_else(|| encoding_error(path, "enum has no variants"))?;
    let (index, variant) = variants
        .iter()
        .enumerate()
        .find(|(_, variant)| variant.name.as_deref() == Some(variant_name))
        .ok_or_else(|| encoding_error(path, format!("unknown enum variant {variant_name}")))?;
    output.push(
        u8::try_from(index).map_err(|_| encoding_error(path, "enum has more than 256 variants"))?,
    );

    let Some(fields) = &variant.fields else {
        return Ok(());
    };
    if fields.is_empty() {
        return Ok(());
    }
    let payload = payload.ok_or_else(|| encoding_error(path, "enum variant needs a payload"))?;
    if fields.iter().all(|field| field.name_key) {
        let object = payload
            .as_object()
            .ok_or_else(|| encoding_error(path, "expected an object variant payload"))?;
        for field in fields {
            let name = field
                .name
                .as_deref()
                .ok_or_else(|| encoding_error(path, "named enum variant field has no name"))?;
            let ty = field
                .ty
                .as_ref()
                .ok_or_else(|| encoding_error(path, format!("enum field {name} has no type")))?;
            let value = object
                .get(name)
                .ok_or_else(|| encoding_error(&format!("{path}.{name}"), "missing field"))?;
            encode_value(model, &format!("{path}.{name}"), ty, value, output)?;
        }
    } else {
        let values = payload
            .as_array()
            .ok_or_else(|| encoding_error(path, "expected an array variant payload"))?;
        if values.len() != fields.len() {
            return Err(encoding_error(
                path,
                format!(
                    "expected {} tuple fields, got {}",
                    fields.len(),
                    values.len()
                ),
            ));
        }
        for (index, (field, value)) in fields.iter().zip(values).enumerate() {
            // A `{name, type}` object uses its `type`; a bare type value IS the
            // type (the old `field.get("type").unwrap_or(field)` fallback).
            let ty = field.ty.clone().unwrap_or_else(|| field.whole.clone());
            encode_value(model, &format!("{path}[{index}]"), &ty, value, output)?;
        }
    }
    Ok(())
}

fn parse_u128(value: &Value) -> Option<u128> {
    value
        .as_u64()
        .map(u128::from)
        .or_else(|| value.as_str()?.parse().ok())
}

fn parse_i128(value: &Value) -> Option<i128> {
    value
        .as_i64()
        .map(i128::from)
        .or_else(|| value.as_u64().map(i128::from))
        .or_else(|| value.as_str()?.parse().ok())
}

fn write_len(path: &str, length: usize, output: &mut Vec<u8>) -> Result<()> {
    let length = u32::try_from(length)
        .map_err(|_| encoding_error(path, "length exceeds Borsh u32 limit"))?;
    output.extend_from_slice(&length.to_le_bytes());
    Ok(())
}

fn json_bytes(path: &str, value: &Value) -> Result<Vec<u8>> {
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| encoding_error(path, "byte values must be between 0 and 255"))
            })
            .collect();
    }
    if let Some(text) = value.as_str() {
        // One hex reader for the whole crate: `0x` optional, spaces and
        // underscores tolerated, same as suite files.
        return crate::utils::hex_decode(text)
            .map_err(|e| encoding_error(path, format!("invalid hex bytes: {e}")));
    }
    Err(encoding_error(
        path,
        "expected a byte array or 0x-prefixed hex string",
    ))
}

fn encoding_error(path: &str, reason: impl Into<String>) -> Error {
    Error::ArgumentEncoding {
        argument: path.to_string(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    /// Parse a bare instruction JSON the way the old tests passed one in.
    fn instruction_model(instruction: Value) -> IdlModel {
        IdlModel::parse(&json!({ "instructions": [instruction] }))
    }

    #[test]
    fn encodes_nested_anchor_types_as_borsh() {
        let idl = json!({
            "types": [
                {
                    "name": "Config",
                    "type": {
                        "kind": "struct",
                        "fields": [
                            { "name": "enabled", "type": "bool" },
                            { "name": "limits", "type": { "array": ["u16", 2] } },
                            { "name": "mode", "type": { "defined": { "name": "Mode" } } }
                        ]
                    }
                },
                {
                    "name": "Mode",
                    "type": {
                        "kind": "enum",
                        "variants": [
                            { "name": "Off" },
                            { "name": "Fixed", "fields": ["u64"] },
                            {
                                "name": "Range",
                                "fields": [
                                    { "name": "min", "type": "i32" },
                                    { "name": "max", "type": "i32" }
                                ]
                            }
                        ]
                    }
                }
            ]
        });
        let instruction = json!({
            "name": "configure",
            "args": [
                { "name": "config", "type": { "defined": "Config" } },
                { "name": "memo", "type": { "option": "string" } },
                { "name": "payload", "type": "bytes" },
                { "name": "amounts", "type": { "vec": "u8" } }
            ]
        });
        let args = json!({
            "config": {
                "enabled": true,
                "limits": [10, 20],
                "mode": { "Range": { "min": -5, "max": 9 } }
            },
            "memo": "ok",
            "payload": "0xdeadbeef",
            "amounts": [7, 8, 9]
        });
        let model = IdlModel::parse(&idl);
        let ix_model = instruction_model(instruction);
        let ix = ix_model.instruction("configure").unwrap();
        let mut output = Vec::new();
        encode_arguments(&model, ix, args.as_object().unwrap(), &mut output).unwrap();

        let mut expected = vec![1];
        expected.extend_from_slice(&10_u16.to_le_bytes());
        expected.extend_from_slice(&20_u16.to_le_bytes());
        expected.push(2);
        expected.extend_from_slice(&(-5_i32).to_le_bytes());
        expected.extend_from_slice(&9_i32.to_le_bytes());
        expected.push(1);
        expected.extend_from_slice(&2_u32.to_le_bytes());
        expected.extend_from_slice(b"ok");
        expected.extend_from_slice(&4_u32.to_le_bytes());
        expected.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        expected.extend_from_slice(&3_u32.to_le_bytes());
        expected.extend_from_slice(&[7, 8, 9]);
        assert_eq!(output, expected);
    }

    #[test]
    fn accepts_large_integers_as_decimal_strings() {
        let instruction = json!({
            "name": "large",
            "args": [
                { "name": "unsigned", "type": "u128" },
                { "name": "signed", "type": "i128" }
            ]
        });
        let args = json!({
            "unsigned": "340282366920938463463374607431768211455",
            "signed": "-170141183460469231731687303715884105728"
        });
        let model = IdlModel::parse(&json!({}));
        let ix_model = instruction_model(instruction);
        let ix = ix_model.instruction("large").unwrap();
        let mut output = Vec::new();
        encode_arguments(&model, ix, args.as_object().unwrap(), &mut output).unwrap();

        assert_eq!(&output[..16], &u128::MAX.to_le_bytes());
        assert_eq!(&output[16..], &i128::MIN.to_le_bytes());
    }

    #[test]
    fn reports_argument_path_for_bad_nested_value() {
        let idl = json!({
            "types": [{
                "name": "Config",
                "type": {
                    "kind": "struct",
                    "fields": [{ "name": "count", "type": "u8" }]
                }
            }]
        });
        let instruction = json!({
            "name": "configure",
            "args": [{ "name": "config", "type": { "defined": "Config" } }]
        });
        let args = json!({ "config": { "count": 300 } });
        let model = IdlModel::parse(&idl);
        let ix_model = instruction_model(instruction);
        let ix = ix_model.instruction("configure").unwrap();
        let error =
            encode_arguments(&model, ix, args.as_object().unwrap(), &mut Vec::new()).unwrap_err();

        assert!(matches!(
            error,
            Error::ArgumentEncoding { argument, .. } if argument == "config.count"
        ));
    }
}

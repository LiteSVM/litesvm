//! Typed model of an Anchor IDL, parsed once per program.
//!
//! Parsing is infallible and best-effort: malformed entries degrade at the
//! *use* site (an instruction that never matches, a field walk that stops),
//! never at parse time, because an on-chain IDL is untrusted input. Both Anchor pre-0.30 and 0.30+ spellings are accepted:
//! `isMut`/`isSigner` and `writable`/`signer`, `{"defined":"X"}` and
//! `{"defined":{"name":"X"}}`, `publicKey` and `pubkey`, instructions with and
//! without a `discriminator` array.

use serde_json::Value;

/// A whole parsed IDL. Missing sections parse as empty.
#[derive(Clone)]
pub(crate) struct IdlModel {
    pub(crate) instructions: Vec<IxDef>,
    pub(crate) accounts: Vec<AccountDef>,
    pub(crate) types: Vec<TypeDef>,
    pub(crate) errors: Vec<ErrorDef>,
    pub(crate) events: Vec<EventDef>,
}

/// One event definition. New-format IDLs carry an explicit `discriminator` and
/// keep the fields in `types`; legacy IDLs derive the discriminator from the
/// name (`sha256("event:Name")[..8]`) and list the fields inline.
#[derive(Clone)]
pub(crate) struct EventDef {
    pub(crate) name: Option<String>,
    pub(crate) discriminator: DiscField,
    /// Inline `fields`, when the (legacy) event declares them.
    pub(crate) fields: Option<Vec<FieldDef>>,
}

/// One instruction definition.
#[derive(Clone)]
pub(crate) struct IxDef {
    pub(crate) name: Option<String>,
    pub(crate) discriminator: DiscField,
    pub(crate) accounts: Vec<AccountNode>,
    pub(crate) args: Vec<ArgDef>,
}

/// The `discriminator` field, kept loose: a malformed one simply never
/// matches.
#[derive(Clone)]
pub(crate) enum DiscField {
    /// No `discriminator` key.
    Absent,
    /// A `discriminator` key whose value isn't an array.
    NotArray,
    /// An array; each element is `Some(n)` for a JSON number, else `None`.
    Bytes(Vec<Option<u64>>),
}

impl DiscField {
    /// The lossy byte view the matcher uses: non-numbers skipped, numbers
    /// truncated with `as u8`.
    pub(crate) fn lossy_bytes(&self) -> Option<Vec<u8>> {
        match self {
            DiscField::Bytes(entries) => {
                Some(entries.iter().filter_map(|e| e.map(|v| v as u8)).collect())
            }
            _ => None,
        }
    }
}

/// One entry of an instruction's account list. Only the name is needed to
/// label the account at that position; a nested *group* keeps its group name.
#[derive(Clone)]
pub(crate) struct AccountNode {
    pub(crate) name: Option<String>,
}

/// One instruction argument.
#[derive(Clone)]
pub(crate) struct ArgDef {
    pub(crate) name: Option<String>,
    /// `None` iff the `type` key is absent.
    pub(crate) ty: Option<IdlType>,
}

/// A parsed IDL type expression.
#[derive(Clone)]
pub(crate) enum IdlType {
    /// `bool`.
    Bool,
    /// Unsigned integer of the given byte width (1, 2, 4, 8, 16).
    U(usize),
    /// Signed integer of the given byte width.
    I(usize),
    /// `f32`.
    F32,
    /// `f64`.
    F64,
    /// `pubkey` / legacy `publicKey` (`camel` remembers the spelling so labels
    /// render exactly as the IDL wrote them).
    Pubkey {
        /// True for the legacy `publicKey` spelling.
        camel: bool,
    },
    /// `string`.
    Str,
    /// `bytes`.
    Bytes,
    /// `{"vec": T}`.
    Vec(Box<IdlType>),
    /// `{"option": T}`.
    Option(Box<IdlType>),
    /// A well-formed `{"array": [T, N]}`.
    Array {
        /// Element type.
        inner: Box<IdlType>,
        /// Element count.
        len: u64,
    },
    /// An `{"array": [...]}` whose element type or length is missing/invalid.
    ArrayMalformed,
    /// `{"defined": "X"}` or `{"defined": {"name": "X"}}`.
    Defined(String),
    /// Anything else, with a display label (the raw spelling for bare
    /// strings, else "array"/"unknown").
    Unknown { label: String },
}

impl IdlType {
    /// Parse a type expression. Never fails; unrecognized shapes become
    /// [`IdlType::Unknown`].
    pub(crate) fn parse(ty: &Value) -> IdlType {
        if let Some(s) = ty.as_str() {
            return match s {
                "bool" => IdlType::Bool,
                "u8" => IdlType::U(1),
                "u16" => IdlType::U(2),
                "u32" => IdlType::U(4),
                "u64" => IdlType::U(8),
                "u128" => IdlType::U(16),
                "i8" => IdlType::I(1),
                "i16" => IdlType::I(2),
                "i32" => IdlType::I(4),
                "i64" => IdlType::I(8),
                "i128" => IdlType::I(16),
                "f32" => IdlType::F32,
                "f64" => IdlType::F64,
                "pubkey" => IdlType::Pubkey { camel: false },
                "publicKey" => IdlType::Pubkey { camel: true },
                "string" => IdlType::Str,
                "bytes" => IdlType::Bytes,
                other => IdlType::Unknown {
                    label: other.to_string(),
                },
            };
        }
        if let Some(inner) = ty.get("vec") {
            return IdlType::Vec(Box::new(IdlType::parse(inner)));
        }
        if let Some(inner) = ty.get("option") {
            return IdlType::Option(Box::new(IdlType::parse(inner)));
        }
        if let Some(arr) = ty.get("array").and_then(Value::as_array) {
            let Some(first) = arr.first() else {
                return IdlType::ArrayMalformed;
            };
            let Some(len) = arr.get(1).and_then(Value::as_u64) else {
                return IdlType::ArrayMalformed;
            };
            return IdlType::Array {
                inner: Box::new(IdlType::parse(first)),
                len,
            };
        }
        if let Some(name) = defined_name(ty) {
            return IdlType::Defined(name.to_string());
        }
        IdlType::Unknown {
            label: if ty.get("array").is_some() {
                "array".into()
            } else {
                "unknown".into()
            },
        }
    }

    /// The display label (raw spellings preserved, composites collapsed).
    pub(crate) fn label(&self) -> String {
        match self {
            IdlType::Bool => "bool".into(),
            IdlType::U(n) => format!("u{}", n * 8),
            IdlType::I(n) => format!("i{}", n * 8),
            IdlType::F32 => "f32".into(),
            IdlType::F64 => "f64".into(),
            IdlType::Pubkey { camel: true } => "publicKey".into(),
            IdlType::Pubkey { camel: false } => "pubkey".into(),
            IdlType::Str => "string".into(),
            IdlType::Bytes => "bytes".into(),
            IdlType::Vec(_) => "vec".into(),
            IdlType::Option(_) => "option".into(),
            IdlType::Array { .. } | IdlType::ArrayMalformed => "array".into(),
            IdlType::Defined(name) => name.clone(),
            IdlType::Unknown { label, .. } => label.clone(),
        }
    }
}

/// Get a `defined` type's name from either IDL shape.
fn defined_name(ty: &Value) -> Option<&str> {
    let d = ty.get("defined")?;
    d.as_str().or_else(|| d.get("name").and_then(Value::as_str))
}

/// One account *type* definition (the IDL's top-level `accounts` — the things
/// discriminators map to), as opposed to an instruction's account list.
#[derive(Clone)]
pub(crate) struct AccountDef {
    pub(crate) name: Option<String>,
    pub(crate) discriminator: DiscField,
}

impl AccountDef {
    /// Whether this account type's discriminator matches the given 8 bytes,
    /// with the old comparison semantics: all entries numeric and equal.
    pub(crate) fn matches(&self, disc: &[u8]) -> bool {
        match &self.discriminator {
            DiscField::Bytes(entries) => {
                entries.len() == 8
                    && entries
                        .iter()
                        .zip(disc)
                        .all(|(e, b)| *e == Some(u64::from(*b)))
            }
            _ => false,
        }
    }
}

/// One entry in the IDL's `types` section.
#[derive(Clone)]
pub(crate) struct TypeDef {
    pub(crate) name: Option<String>,
    pub(crate) body: TypeBody,
    /// `type.fields` parsed regardless of `kind` — the account decoder reads
    /// fields without checking the kind.
    pub(crate) raw_fields: Option<Vec<FieldDef>>,
}

/// A type definition's body.
#[derive(Clone)]
pub(crate) enum TypeBody {
    /// The entry has no `type` key at all.
    NoBody,
    /// `kind: "struct"`; `fields` is `Some` iff a `fields` array is present.
    Struct {
        /// The struct's fields, when declared.
        fields: Option<Vec<FieldDef>>,
    },
    /// `kind: "enum"`; `variants` is `Some` iff a `variants` array is present.
    Enum {
        /// The enum's variants, when declared.
        variants: Option<Vec<VariantDef>>,
    },
    /// Any other (or missing) `kind`.
    Other,
}

/// One struct field (or named enum-variant field).
#[derive(Clone)]
pub(crate) struct FieldDef {
    pub(crate) name: Option<String>,
    /// `None` iff the `type` key is absent.
    pub(crate) ty: Option<IdlType>,
}

/// One enum variant.
#[derive(Clone)]
pub(crate) struct VariantDef {
    pub(crate) name: Option<String>,
    /// `Some` iff a `fields` array is present.
    pub(crate) fields: Option<Vec<VariantField>>,
}

/// One enum-variant field, which the IDL writes either as a `{name, type}`
/// object (named variant) or as a bare type value (tuple variant).
#[derive(Clone)]
pub(crate) struct VariantField {
    /// True when a `name` key is present (any value) — the named/tuple switch.
    pub(crate) name_key: bool,
    /// The name, when present *and* a string.
    pub(crate) name: Option<String>,
    /// The `type` key's parse, when that key is present.
    pub(crate) ty: Option<IdlType>,
    /// The whole field value's parse — what the tuple paths fall back to.
    pub(crate) whole: IdlType,
}

/// One custom error definition.
#[derive(Clone)]
pub(crate) struct ErrorDef {
    /// `None` when the `code` is missing or non-numeric (entry never matches).
    pub(crate) code: Option<u64>,
    pub(crate) name: String,
    pub(crate) msg: String,
}

impl IdlModel {
    /// Parse an IDL JSON. Infallible; anything malformed degrades per-entry.
    pub(crate) fn parse(idl: &Value) -> IdlModel {
        IdlModel {
            instructions: array_of(idl, "instructions", parse_instruction),
            accounts: array_of(idl, "accounts", |a| AccountDef {
                name: str_field(a, "name"),
                discriminator: parse_discriminator(a),
            }),
            types: array_of(idl, "types", parse_type_def),
            errors: array_of(idl, "errors", |e| ErrorDef {
                code: e.get("code").and_then(Value::as_u64),
                name: str_field(e, "name").unwrap_or_default(),
                msg: str_field(e, "msg").unwrap_or_default(),
            }),
            events: array_of(idl, "events", |e| EventDef {
                name: str_field(e, "name"),
                discriminator: parse_discriminator(e),
                fields: e
                    .get("fields")
                    .and_then(Value::as_array)
                    .map(|fields| fields.iter().map(parse_field_def).collect()),
            }),
        }
    }

    /// Find an instruction by exact name.
    #[cfg(test)]
    pub(crate) fn instruction(&self, name: &str) -> Option<&IxDef> {
        self.instructions
            .iter()
            .find(|ix| ix.name.as_deref() == Some(name))
    }

    /// Find a type definition by exact name (first match).
    pub(crate) fn type_def(&self, name: &str) -> Option<&TypeDef> {
        self.types.iter().find(|t| t.name.as_deref() == Some(name))
    }

    /// A named struct's declared fields (`kind == "struct"` only).
    pub(crate) fn struct_fields(&self, name: &str) -> Option<&Vec<FieldDef>> {
        match &self.type_def(name)?.body {
            TypeBody::Struct {
                fields: Some(fields),
            } => Some(fields),
            _ => None,
        }
    }

    /// A named enum's declared variants (`kind == "enum"` only).
    pub(crate) fn enum_variants(&self, name: &str) -> Option<&Vec<VariantDef>> {
        match &self.type_def(name)?.body {
            TypeBody::Enum {
                variants: Some(variants),
            } => Some(variants),
            _ => None,
        }
    }
}

fn array_of<T>(idl: &Value, key: &str, parse: impl Fn(&Value) -> T) -> Vec<T> {
    idl.get(key)
        .and_then(Value::as_array)
        .map(|entries| entries.iter().map(parse).collect())
        .unwrap_or_default()
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(String::from)
}

fn parse_discriminator(entry: &Value) -> DiscField {
    match entry.get("discriminator") {
        None => DiscField::Absent,
        Some(d) => match d.as_array() {
            None => DiscField::NotArray,
            Some(bytes) => DiscField::Bytes(bytes.iter().map(Value::as_u64).collect()),
        },
    }
}

fn parse_instruction(ix: &Value) -> IxDef {
    IxDef {
        name: str_field(ix, "name"),
        discriminator: parse_discriminator(ix),
        accounts: ix
            .get("accounts")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|acc| AccountNode {
                        name: str_field(acc, "name"),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        args: ix
            .get("args")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|arg| ArgDef {
                        name: str_field(arg, "name"),
                        ty: arg.get("type").map(IdlType::parse),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn parse_type_def(t: &Value) -> TypeDef {
    let raw_fields = t
        .get("type")
        .and_then(|ty| ty.get("fields"))
        .and_then(Value::as_array)
        .map(|fields| fields.iter().map(parse_field_def).collect());
    let body = match t.get("type") {
        None => TypeBody::NoBody,
        Some(ty) => match ty.get("kind").and_then(Value::as_str) {
            Some("struct") => TypeBody::Struct {
                fields: raw_fields.clone(),
            },
            Some("enum") => TypeBody::Enum {
                variants: ty.get("variants").and_then(Value::as_array).map(|vs| {
                    vs.iter()
                        .map(|v| VariantDef {
                            name: str_field(v, "name"),
                            fields: v
                                .get("fields")
                                .and_then(Value::as_array)
                                .map(|fs| fs.iter().map(parse_variant_field).collect()),
                        })
                        .collect()
                }),
            },
            _ => TypeBody::Other,
        },
    };
    TypeDef {
        name: str_field(t, "name"),
        body,
        raw_fields,
    }
}

fn parse_field_def(f: &Value) -> FieldDef {
    FieldDef {
        name: str_field(f, "name"),
        ty: f.get("type").map(IdlType::parse),
    }
}

fn parse_variant_field(f: &Value) -> VariantField {
    VariantField {
        name_key: f.get("name").is_some(),
        name: str_field(f, "name"),
        ty: f.get("type").map(IdlType::parse),
        whole: IdlType::parse(f),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn parses_modern_and_legacy_spellings() {
        let idl = json!({
            "instructions": [
                {
                    "name": "modern",
                    "discriminator": [1, 2, 3, 4, 5, 6, 7, 8],
                    "accounts": [
                        { "name": "state", "writable": true, "signer": false },
                        { "name": "group", "accounts": [{ "name": "inner", "writable": true }] }
                    ],
                    "args": [{ "name": "amount", "type": "u64" }]
                },
                {
                    "name": "legacy",
                    "accounts": [{ "name": "auth", "isMut": true, "isSigner": true }],
                    "args": [
                        { "name": "key", "type": "publicKey" },
                        { "name": "cfg", "type": { "defined": "Config" } },
                        { "name": "cfg2", "type": { "defined": { "name": "Config" } } }
                    ]
                }
            ],
            "types": [{
                "name": "Config",
                "type": { "kind": "struct", "fields": [{ "name": "n", "type": "u8" }] }
            }],
            "errors": [{ "code": 6000, "name": "Nope", "msg": "no" }]
        });
        let model = IdlModel::parse(&idl);

        let modern = model.instruction("modern").unwrap();
        assert_eq!(
            modern.discriminator.lossy_bytes().unwrap(),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(modern.accounts[1].name.as_deref(), Some("group"));

        let legacy = model.instruction("legacy").unwrap();
        assert!(matches!(legacy.discriminator, DiscField::Absent));
        assert_eq!(legacy.accounts[0].name.as_deref(), Some("auth"));
        // publicKey keeps its spelling in labels.
        assert_eq!(legacy.args[0].ty.as_ref().unwrap().label(), "publicKey");
        // Both defined spellings resolve to the same name.
        for arg in &legacy.args[1..] {
            assert!(matches!(arg.ty.as_ref().unwrap(), IdlType::Defined(n) if n == "Config"));
        }
        assert!(model.struct_fields("Config").is_some());
        assert!(model.enum_variants("Config").is_none());
        assert_eq!(model.errors[0].code, Some(6000));
    }

    #[test]
    fn hostile_shapes_degrade_instead_of_failing() {
        let idl = json!({
            "instructions": [
                { "name": "noDisc" },
                { "name": "badDisc", "discriminator": "nope" },
                { "discriminator": [1, 2, 3, 4, 5, 6, 7, 8] }
            ],
            "types": [
                { "name": "Weird", "type": { "kind": "union" } },
                { "name": "NoKind", "type": { "fields": [{ "name": "x", "type": "u8" }] } },
                { "name": "NoBody" }
            ]
        });
        let model = IdlModel::parse(&idl);
        assert!(matches!(
            model.instruction("badDisc").unwrap().discriminator,
            DiscField::NotArray
        ));
        // A kind-less type with fields keeps them reachable for the account
        // decoder's loose entry lookup, but is not a struct for the walker.
        assert!(model.struct_fields("NoKind").is_none());
        assert!(model.type_def("NoKind").unwrap().raw_fields.is_some());
        assert!(matches!(
            model.type_def("NoBody").unwrap().body,
            TypeBody::NoBody
        ));
        assert!(model.type_def("Weird").is_some());
    }

    #[test]
    fn type_parse_covers_composites_and_unknowns() {
        assert!(matches!(
            IdlType::parse(&json!({ "vec": "u8" })),
            IdlType::Vec(_)
        ));
        assert!(matches!(
            IdlType::parse(&json!({ "option": "u64" })),
            IdlType::Option(_)
        ));
        assert!(matches!(
            IdlType::parse(&json!({ "array": ["u16", 4] })),
            IdlType::Array { len: 4, .. }
        ));
        assert!(matches!(
            IdlType::parse(&json!({ "array": [] })),
            IdlType::ArrayMalformed
        ));
        assert!(matches!(
            IdlType::parse(&json!({ "array": ["u16"] })),
            IdlType::ArrayMalformed
        ));
        // "array" key with a non-array value labels as "array" (old fallback).
        assert_eq!(IdlType::parse(&json!({ "array": 5 })).label(), "array");
        assert_eq!(IdlType::parse(&json!("flooble")).label(), "flooble");
        assert_eq!(IdlType::parse(&json!(null)).label(), "unknown");
    }
}

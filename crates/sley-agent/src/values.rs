//! Typed values between JSON and SSMC1 `ConstValue`.
//!
//! Inputs are read against a declared type, so `5` becomes an `i64` or a
//! `u8` as the parameter demands. Results render compactly: integers and
//! booleans bare, `{"Ok":10}`, `{"Err":"InvalidInput"}`, `"Member"` for a
//! payload-free variant case, `{"Member":payload}` otherwise.

use serde_json::{Map, Value};
use sley_id::EntityId;
use sley_mutate::value::EntityBodyValue;
use sley_ssmc::{
    BuiltinFailureKind, ConstData, ConstValue, FieldConst, MemberId, NamedType, RecordConst,
    ResultConst, TypeDefForm, TypeExpr, VariantConst,
};

use crate::error::{AgentError, AgentErrorCode, Result};
use crate::hex;
use crate::names::Names;
use crate::workspace::Program;

/// Maximum nesting depth of one input value.
const DEPTH_LIMIT: usize = 32;

fn invalid(pointer: &str, detail: impl std::fmt::Display) -> AgentError {
    AgentError::new(AgentErrorCode::InputInvalid, format!("{pointer}: {detail}"))
}

/// Substitutes a named type's arguments into a member type.
#[must_use]
pub fn substitute(ty: &TypeExpr, arguments: &[TypeExpr]) -> TypeExpr {
    match ty {
        TypeExpr::TypeParameter(index) => arguments
            .get(*index as usize)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        TypeExpr::Tuple(items) => TypeExpr::Tuple(
            items
                .iter()
                .map(|item| substitute(item, arguments))
                .collect(),
        ),
        TypeExpr::Named(named) => TypeExpr::Named(NamedType {
            definition: named.definition,
            arguments: named
                .arguments
                .iter()
                .map(|item| substitute(item, arguments))
                .collect(),
        }),
        TypeExpr::Vector(item) => TypeExpr::Vector(Box::new(substitute(item, arguments))),
        TypeExpr::Option(item) => TypeExpr::Option(Box::new(substitute(item, arguments))),
        TypeExpr::LocalCell(item) => TypeExpr::LocalCell(Box::new(substitute(item, arguments))),
        TypeExpr::OrderedMap { key, value } => TypeExpr::OrderedMap {
            key: Box::new(substitute(key, arguments)),
            value: Box::new(substitute(value, arguments)),
        },
        TypeExpr::Result { ok, error } => TypeExpr::Result {
            ok: Box::new(substitute(ok, arguments)),
            error: Box::new(substitute(error, arguments)),
        },
        other => other.clone(),
    }
}

/// Type definitions and member names, as reading a typed value needs them.
pub trait TypeDefs {
    /// The form of a type definition.
    fn form(&self, definition: &EntityId) -> Option<TypeDefForm>;
    /// A member of a definition by leaf name.
    fn member(&self, definition: &EntityId, leaf: &str) -> Option<MemberId>;
    /// The leaf name of a member.
    fn member_leaf(&self, definition: &EntityId, member: &MemberId) -> String;
    /// Renders a type (for messages).
    fn render(&self, ty: &TypeExpr) -> String;
}

/// The type definitions of one program state under its names.
pub struct ProgramTypes<'a> {
    /// The state.
    pub program: &'a Program,
    /// Its names.
    pub names: &'a Names,
}

impl TypeDefs for ProgramTypes<'_> {
    fn form(&self, definition: &EntityId) -> Option<TypeDefForm> {
        match self.program.body(definition) {
            Some(EntityBodyValue::TypeDef(typedef)) => Some(typedef.form.clone()),
            _ => None,
        }
    }

    fn member(&self, definition: &EntityId, leaf: &str) -> Option<MemberId> {
        self.names.resolve_member_leaf(definition, leaf)
    }

    fn member_leaf(&self, definition: &EntityId, member: &MemberId) -> String {
        self.names.member_leaf(definition, member)
    }

    fn render(&self, ty: &TypeExpr) -> String {
        crate::types::render(ty, self.names)
    }
}

/// Reads one JSON value against a declared type.
///
/// # Errors
///
/// `AGENT_INPUT_INVALID` naming `pointer` when the value does not fit.
pub fn read(
    value: &Value,
    ty: &TypeExpr,
    defs: &dyn TypeDefs,
    pointer: &str,
) -> Result<ConstValue> {
    read_at(value, ty, defs, pointer, 0)
}

#[allow(clippy::too_many_lines)]
fn read_at(
    value: &Value,
    ty: &TypeExpr,
    defs: &dyn TypeDefs,
    pointer: &str,
    depth: usize,
) -> Result<ConstValue> {
    if depth > DEPTH_LIMIT {
        return Err(invalid(pointer, "value nests too deeply"));
    }
    let data = match ty {
        TypeExpr::Unit => match value {
            Value::Null => ConstData::Unit,
            Value::Array(items) if items.is_empty() => ConstData::Unit,
            Value::Object(items) if items.is_empty() => ConstData::Unit,
            _ => return Err(invalid(pointer, "expected unit (null)")),
        },
        TypeExpr::Bool => ConstData::Bool(
            value
                .as_bool()
                .ok_or_else(|| invalid(pointer, "expected a boolean"))?,
        ),
        TypeExpr::SInt(width) => {
            let number = integer(value).ok_or_else(|| invalid(pointer, "expected an integer"))?;
            let bits = u32::from(width.bits());
            let (low, high) = if bits >= 128 {
                (i128::MIN, i128::MAX)
            } else {
                (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
            };
            if number < low || number > high {
                return Err(invalid(pointer, format!("{number} does not fit i{bits}")));
            }
            ConstData::SInt(number)
        }
        TypeExpr::UInt(width) => {
            let bits = u32::from(width.bits());
            // Unsigned values read as u128, so u128 values above i128::MAX
            // (which render as strings) read back.
            let number = unsigned(value).ok_or_else(|| {
                integer(value).map_or_else(
                    || invalid(pointer, "expected an integer"),
                    |number| invalid(pointer, format!("{number} does not fit u{bits}")),
                )
            })?;
            if bits < 128 && number >= (1_u128 << bits) {
                return Err(invalid(pointer, format!("{number} does not fit u{bits}")));
            }
            ConstData::UInt(number)
        }
        TypeExpr::F32 => {
            let number = float(value).ok_or_else(|| invalid(pointer, FLOAT_EXPECTED))?;
            #[allow(clippy::cast_possible_truncation)]
            ConstData::F32Bits((number as f32).to_bits())
        }
        TypeExpr::F64 => ConstData::F64Bits(
            float(value)
                .ok_or_else(|| invalid(pointer, FLOAT_EXPECTED))?
                .to_bits(),
        ),
        TypeExpr::Text => ConstData::Text(
            value
                .as_str()
                .ok_or_else(|| invalid(pointer, "expected a string"))?
                .to_owned(),
        ),
        TypeExpr::Bytes => ConstData::Bytes(
            bytes(value)
                .ok_or_else(|| invalid(pointer, "expected \"0x...\" hex or an array of bytes"))?,
        ),
        TypeExpr::Tuple(items) => {
            let values = value
                .as_array()
                .filter(|values| values.len() == items.len())
                .ok_or_else(|| invalid(pointer, format!("expected {} items", items.len())))?;
            ConstData::Sequence(
                values
                    .iter()
                    .zip(items)
                    .enumerate()
                    .map(|(index, (item, ty))| {
                        read_at(item, ty, defs, &format!("{pointer}/{index}"), depth + 1)
                    })
                    .collect::<Result<_>>()?,
            )
        }
        TypeExpr::Vector(item) => {
            let values = value
                .as_array()
                .ok_or_else(|| invalid(pointer, "expected an array"))?;
            ConstData::Sequence(
                values
                    .iter()
                    .enumerate()
                    .map(|(index, element)| {
                        read_at(
                            element,
                            item,
                            defs,
                            &format!("{pointer}/{index}"),
                            depth + 1,
                        )
                    })
                    .collect::<Result<_>>()?,
            )
        }
        TypeExpr::OrderedMap { key, value: item } => {
            // [[key, value], ...], in the kernel's order: by the keys'
            // canonical bytes (VM contract E4), each key once.
            let rows = value
                .as_array()
                .ok_or_else(|| invalid(pointer, "expected [[key, value], ...]"))?;
            let mut keyed = Vec::with_capacity(rows.len());
            for (index, row) in rows.iter().enumerate() {
                let at = format!("{pointer}/{index}");
                let pair = row
                    .as_array()
                    .filter(|pair| pair.len() == 2)
                    .ok_or_else(|| invalid(&at, "a map entry is [key, value]"))?;
                let entry_key = read_at(&pair[0], key, defs, &format!("{at}/0"), depth + 1)?;
                let entry_value = read_at(&pair[1], item, defs, &format!("{at}/1"), depth + 1)?;
                let bytes = sley_mutate::encode_const_value(&entry_key)
                    .map_err(|_| invalid(&at, "the key has no canonical encoding"))?;
                keyed.push((
                    bytes,
                    sley_ssmc::MapEntryConst {
                        key: entry_key,
                        value: entry_value,
                    },
                ));
            }
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            if keyed.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err(invalid(pointer, "a map key appears twice"));
            }
            ConstData::Map(keyed.into_iter().map(|(_, entry)| entry).collect())
        }
        TypeExpr::Option(item) => match value {
            Value::Null => ConstData::Option(None),
            Value::String(text) if text == "None" => ConstData::Option(None),
            Value::Object(object) if object.len() == 1 && object.contains_key("Some") => {
                ConstData::Option(Some(Box::new(read_at(
                    &object["Some"],
                    item,
                    defs,
                    &format!("{pointer}/Some"),
                    depth + 1,
                )?)))
            }
            other => ConstData::Option(Some(Box::new(read_at(
                other,
                item,
                defs,
                pointer,
                depth + 1,
            )?))),
        },
        TypeExpr::Result { ok, error } => {
            let object = value
                .as_object()
                .filter(|object| object.len() == 1)
                .ok_or_else(|| invalid(pointer, "expected {\"Ok\": v} or {\"Err\": e}"))?;
            if let Some(inner) = object.get("Ok") {
                ConstData::Result(ResultConst::Ok(Box::new(read_at(
                    inner,
                    ok,
                    defs,
                    &format!("{pointer}/Ok"),
                    depth + 1,
                )?)))
            } else if let Some(inner) = object.get("Err") {
                ConstData::Result(ResultConst::Err(Box::new(read_at(
                    inner,
                    error,
                    defs,
                    &format!("{pointer}/Err"),
                    depth + 1,
                )?)))
            } else {
                return Err(invalid(pointer, "expected {\"Ok\": v} or {\"Err\": e}"));
            }
        }
        TypeExpr::Named(named) => read_named(value, named, defs, pointer, depth)?,
        TypeExpr::BuiltinFailure(kind) => {
            // "Overflow", or the rendered form {"ArithmeticError": "Overflow"}.
            let named = value
                .as_object()
                .filter(|object| object.len() == 1)
                .and_then(|object| object.get(crate::types::failure_name(*kind)));
            let text = named
                .unwrap_or(value)
                .as_str()
                .ok_or_else(|| invalid(pointer, "expected a failure case name"))?;
            let code = failure_code(*kind, text)
                .ok_or_else(|| invalid(pointer, format!("unknown case `{text}`")))?;
            ConstData::BuiltinFailure(sley_ssmc::BuiltinFailureValue { kind: *kind, code })
        }
        _ => {
            return Err(invalid(
                pointer,
                format!("inputs of type {} are not supported", defs.render(ty)),
            ));
        }
    };
    Ok(ConstValue {
        value_type: ty.clone(),
        data,
    })
}

fn read_named(
    value: &Value,
    named: &NamedType,
    defs: &dyn TypeDefs,
    pointer: &str,
    depth: usize,
) -> Result<ConstData> {
    let Some(form) = defs.form(&named.definition) else {
        return Err(invalid(pointer, "unknown type definition"));
    };
    match &form {
        TypeDefForm::Variant(cases) => {
            let (case_name, payload) = match value {
                Value::String(text) => (text.as_str(), None),
                Value::Object(object) if object.len() == 1 => {
                    let (key, inner) = object.iter().next().expect("one entry");
                    (key.as_str(), Some(inner))
                }
                _ => return Err(invalid(pointer, "expected \"Case\" or {\"Case\": payload}")),
            };
            let leaf = case_name.rsplit('.').next().unwrap_or(case_name);
            let member = defs
                .member(&named.definition, leaf)
                .ok_or_else(|| invalid(pointer, format!("no case `{case_name}`")))?;
            let case = cases
                .iter()
                .find(|case| case.member_id == member)
                .ok_or_else(|| invalid(pointer, format!("no case `{case_name}`")))?;
            let payload = match (&case.payload_type, payload) {
                (None, None) => None,
                (Some(ty), Some(inner)) => Some(Box::new(read_at(
                    inner,
                    &substitute(ty, &named.arguments),
                    defs,
                    &format!("{pointer}/{case_name}"),
                    depth + 1,
                )?)),
                (None, Some(_)) => {
                    return Err(invalid(pointer, format!("`{case_name}` has no payload")));
                }
                (Some(_), None) => {
                    return Err(invalid(pointer, format!("`{case_name}` needs a payload")));
                }
            };
            Ok(ConstData::Variant(VariantConst {
                definition: named.definition,
                member_id: member,
                payload,
            }))
        }
        TypeDefForm::Record(fields) => {
            let object = value
                .as_object()
                .ok_or_else(|| invalid(pointer, "expected a record object"))?;
            if object.len() != fields.len() {
                return Err(invalid(
                    pointer,
                    format!("expected {} fields", fields.len()),
                ));
            }
            let mut out = Vec::with_capacity(fields.len());
            for field in fields {
                let leaf = defs.member_leaf(&named.definition, &field.member_id);
                let inner = object
                    .get(&leaf)
                    .ok_or_else(|| invalid(pointer, format!("missing field `{leaf}`")))?;
                out.push(FieldConst {
                    member_id: field.member_id,
                    value: read_at(
                        inner,
                        &substitute(&field.value_type, &named.arguments),
                        defs,
                        &format!("{pointer}/{leaf}"),
                        depth + 1,
                    )?,
                });
            }
            Ok(ConstData::Record(RecordConst {
                definition: named.definition,
                fields: out,
            }))
        }
    }
}

const FLOAT_EXPECTED: &str = "expected a number, or \"NaN\", \"inf\" or \"-inf\"";

/// A float: a JSON number, or the names non-finite values render as.
fn float(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => match text.as_str() {
            "NaN" => Some(f64::NAN),
            "inf" => Some(f64::INFINITY),
            "-inf" => Some(f64::NEG_INFINITY),
            _ => None,
        },
        _ => None,
    }
}

/// A float as JSON: a number when finite, else its name (JSON has no
/// non-finite numbers, and `null` is unit).
fn float_json(number: f64) -> Value {
    if number.is_nan() {
        Value::from("NaN")
    } else if number.is_infinite() {
        Value::from(if number > 0.0 { "inf" } else { "-inf" })
    } else {
        Value::from(number)
    }
}

fn unsigned(value: &Value) -> Option<u128> {
    match value {
        Value::Number(number) => number.as_u64().map(u128::from),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn integer(value: &Value) -> Option<i128> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .map(i128::from)
            .or_else(|| number.as_u64().map(i128::from)),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn bytes(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::String(text) => hex::decode(text.strip_prefix("0x")?),
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_u64().and_then(|byte| u8::try_from(byte).ok()))
            .collect(),
        _ => None,
    }
}

const fn failure_cases(kind: BuiltinFailureKind) -> &'static [&'static str] {
    match kind {
        BuiltinFailureKind::Arithmetic => &["Overflow", "DivideByZero", "InvalidShift"],
        BuiltinFailureKind::Index => &["OutOfBounds"],
        BuiltinFailureKind::DuplicateKey => &["DuplicateKey"],
        BuiltinFailureKind::ContractViolation => &["PredicateFalse"],
        BuiltinFailureKind::Capability => &["Denied", "ScopeMismatch", "Expired", "RootMismatch"],
    }
}

fn failure_code(kind: BuiltinFailureKind, name: &str) -> Option<u16> {
    failure_cases(kind)
        .iter()
        .position(|case| *case == name)
        .and_then(|index| u16::try_from(index + 1).ok())
}

fn failure_case(kind: BuiltinFailureKind, code: u16) -> String {
    failure_cases(kind)
        .get(usize::from(code).wrapping_sub(1))
        .map_or_else(|| format!("code{code}"), |name| (*name).to_owned())
}

/// Renders a value as compact JSON.
#[must_use]
pub fn to_json(value: &ConstValue, names: &Names) -> Value {
    match &value.data {
        ConstData::Unit => Value::Null,
        ConstData::Bool(flag) => Value::Bool(*flag),
        ConstData::SInt(number) => {
            i64::try_from(*number).map_or_else(|_| Value::from(number.to_string()), Value::from)
        }
        ConstData::UInt(number) => {
            u64::try_from(*number).map_or_else(|_| Value::from(number.to_string()), Value::from)
        }
        ConstData::F32Bits(bits) => float_json(f64::from(f32::from_bits(*bits))),
        ConstData::F64Bits(bits) => float_json(f64::from_bits(*bits)),
        ConstData::Bytes(bytes) => Value::from(format!("0x{}", hex::encode(bytes))),
        ConstData::Text(text) => Value::from(text.as_str()),
        ConstData::Sequence(items) => {
            Value::Array(items.iter().map(|item| to_json(item, names)).collect())
        }
        ConstData::Record(record) => {
            let mut object = Map::new();
            for field in &record.fields {
                object.insert(
                    names.member_leaf(&record.definition, &field.member_id),
                    to_json(&field.value, names),
                );
            }
            Value::Object(object)
        }
        ConstData::Variant(variant) => {
            let leaf = names.member_leaf(&variant.definition, &variant.member_id);
            match &variant.payload {
                None => Value::from(leaf),
                Some(payload) => single(&leaf, to_json(payload, names)),
            }
        }
        ConstData::Map(entries) => Value::Array(
            entries
                .iter()
                .map(|entry| {
                    Value::Array(vec![
                        to_json(&entry.key, names),
                        to_json(&entry.value, names),
                    ])
                })
                .collect(),
        ),
        ConstData::Option(None) => Value::from("None"),
        ConstData::Option(Some(inner)) => single("Some", to_json(inner, names)),
        ConstData::Result(ResultConst::Ok(inner)) => single("Ok", to_json(inner, names)),
        ConstData::Result(ResultConst::Err(inner)) => single("Err", to_json(inner, names)),
        ConstData::FunctionRef(function) => {
            Value::from(format!("fn {}", names.name(&function.function)))
        }
        ConstData::BuiltinFailure(failure) => single(
            crate::types::failure_name(failure.kind),
            Value::from(failure_case(failure.kind, failure.code)),
        ),
    }
}

fn single(key: &str, value: Value) -> Value {
    let mut object = Map::new();
    object.insert(key.to_owned(), value);
    Value::Object(object)
}

/// Renders a value as AV1 text (`Ok(10)`, `Err(CalcError.InvalidInput)`).
#[must_use]
pub fn to_text(value: &ConstValue, names: &Names) -> String {
    match &value.data {
        ConstData::Unit => "()".into(),
        ConstData::Bool(flag) => flag.to_string(),
        ConstData::SInt(number) => number.to_string(),
        ConstData::UInt(number) => number.to_string(),
        ConstData::F32Bits(bits) => format!("{:?}", f32::from_bits(*bits)),
        ConstData::F64Bits(bits) => format!("{:?}", f64::from_bits(*bits)),
        ConstData::Bytes(bytes) => format!("0x{}", hex::encode(bytes)),
        ConstData::Text(text) => serde_json::to_string(text).unwrap_or_default(),
        ConstData::Sequence(items) => {
            let inner = join(items.iter().map(|item| to_text(item, names)));
            if matches!(value.value_type, TypeExpr::Tuple(_)) {
                format!("({inner})")
            } else {
                format!("[{inner}]")
            }
        }
        ConstData::Record(record) => format!(
            "{}{{{}}}",
            names.name(&record.definition),
            join(record.fields.iter().map(|field| format!(
                "{}: {}",
                names.member_leaf(&record.definition, &field.member_id),
                to_text(&field.value, names)
            )))
        ),
        ConstData::Variant(variant) => {
            let name = names.member(&variant.definition, &variant.member_id);
            match &variant.payload {
                None => name,
                Some(payload) => format!("{name}({})", to_text(payload, names)),
            }
        }
        ConstData::Map(entries) => format!(
            "{{{}}}",
            join(entries.iter().map(|entry| format!(
                "{}: {}",
                to_text(&entry.key, names),
                to_text(&entry.value, names)
            )))
        ),
        ConstData::Option(None) => "None".into(),
        ConstData::Option(Some(inner)) => format!("Some({})", to_text(inner, names)),
        ConstData::Result(ResultConst::Ok(inner)) => format!("Ok({})", to_text(inner, names)),
        ConstData::Result(ResultConst::Err(inner)) => format!("Err({})", to_text(inner, names)),
        ConstData::FunctionRef(function) => format!("fn {}", names.name(&function.function)),
        ConstData::BuiltinFailure(failure) => format!(
            "{}.{}",
            crate::types::failure_name(failure.kind),
            failure_case(failure.kind, failure.code)
        ),
    }
}

fn join(items: impl Iterator<Item = String>) -> String {
    items.collect::<Vec<_>>().join(", ")
}

/// Returns the payload-free variant case a definition names, if any.
#[must_use]
pub fn variant_of(program: &Program, definition: &EntityId) -> bool {
    matches!(
        program.body(definition),
        Some(EntityBodyValue::TypeDef(typedef)) if matches!(typedef.form, TypeDefForm::Variant(_))
    )
}

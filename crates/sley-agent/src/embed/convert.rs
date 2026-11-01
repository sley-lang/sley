//! Conversions around [`EmbedValue`]: Rust types for generated bindings
//! ([`ToEmbed`], [`FromEmbed`]), the JSON form of [`Module::call_json`],
//! and the spellings of types and signatures that bindings check against.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Value};

use super::{EmbedError, EmbedType, EmbedValue, MAX_DEPTH, Signature, TypeDeclaration, TypeShape};
use crate::error::AgentErrorCode;

/// A Rust value with an [`EmbedValue`] form: an argument of generated
/// bindings ([`super::bindgen`]).
pub trait ToEmbed {
    /// The value's embed form.
    ///
    /// # Errors
    ///
    /// `AGENT_INPUT_INVALID` for an integer outside `i64`, the range an
    /// [`EmbedValue::Int`] holds.
    fn to_embed(&self) -> Result<EmbedValue, EmbedError>;
}

/// A Rust value read from an [`EmbedValue`]: a result of generated
/// bindings ([`super::bindgen`]).
pub trait FromEmbed: Sized {
    /// The value, when `value` has this type's form.
    fn from_embed(value: &EmbedValue) -> Option<Self>;
}

fn out_of_range(number: impl fmt::Display) -> EmbedError {
    EmbedError::new(
        AgentErrorCode::InputInvalid,
        format!("{number} is outside the i64 range of an embed integer"),
    )
}

macro_rules! integers {
    ($($ty:ty),*) => {$(
        impl ToEmbed for $ty {
            fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
                i64::try_from(*self).map(EmbedValue::Int).map_err(|_| out_of_range(self))
            }
        }

        impl FromEmbed for $ty {
            fn from_embed(value: &EmbedValue) -> Option<Self> {
                match value {
                    EmbedValue::Int(number) => <$ty>::try_from(*number).ok(),
                    _ => None,
                }
            }
        }
    )*};
}

integers!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

impl ToEmbed for () {
    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
        Ok(EmbedValue::Unit)
    }
}

impl FromEmbed for () {
    fn from_embed(value: &EmbedValue) -> Option<Self> {
        matches!(value, EmbedValue::Unit).then_some(())
    }
}

impl ToEmbed for bool {
    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
        Ok(EmbedValue::Bool(*self))
    }
}

impl FromEmbed for bool {
    fn from_embed(value: &EmbedValue) -> Option<Self> {
        match value {
            EmbedValue::Bool(flag) => Some(*flag),
            _ => None,
        }
    }
}

impl ToEmbed for String {
    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
        Ok(EmbedValue::Str(self.clone()))
    }
}

impl ToEmbed for str {
    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
        Ok(EmbedValue::Str(self.to_owned()))
    }
}

impl FromEmbed for String {
    fn from_embed(value: &EmbedValue) -> Option<Self> {
        match value {
            EmbedValue::Str(text) => Some(text.clone()),
            _ => None,
        }
    }
}

impl<T: ToEmbed> ToEmbed for [T] {
    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
        self.iter()
            .map(ToEmbed::to_embed)
            .collect::<Result<_, _>>()
            .map(EmbedValue::List)
    }
}

impl<T: ToEmbed> ToEmbed for Vec<T> {
    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
        self.as_slice().to_embed()
    }
}

impl<T: FromEmbed> FromEmbed for Vec<T> {
    fn from_embed(value: &EmbedValue) -> Option<Self> {
        match value {
            EmbedValue::List(items) => items.iter().map(T::from_embed).collect(),
            EmbedValue::IntList(items) => items
                .iter()
                .map(|number| T::from_embed(&EmbedValue::Int(*number)))
                .collect(),
            _ => None,
        }
    }
}

impl<T: ToEmbed> ToEmbed for Option<T> {
    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
        Ok(EmbedValue::Option(match self {
            Some(value) => Some(Box::new(value.to_embed()?)),
            None => None,
        }))
    }
}

impl<T: FromEmbed> FromEmbed for Option<T> {
    fn from_embed(value: &EmbedValue) -> Option<Self> {
        match value {
            EmbedValue::Option(None) => Some(None),
            EmbedValue::Option(Some(value)) => T::from_embed(value).map(Some),
            _ => None,
        }
    }
}

impl<T: ToEmbed, E: ToEmbed> ToEmbed for Result<T, E> {
    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
        Ok(EmbedValue::Result(match self {
            Ok(value) => Ok(Box::new(value.to_embed()?)),
            Err(error) => Err(Box::new(error.to_embed()?)),
        }))
    }
}

impl<T: FromEmbed, E: FromEmbed> FromEmbed for Result<T, E> {
    fn from_embed(value: &EmbedValue) -> Option<Self> {
        match value {
            EmbedValue::Result(Ok(value)) => T::from_embed(value).map(Ok),
            EmbedValue::Result(Err(error)) => E::from_embed(error).map(Err),
            _ => None,
        }
    }
}

impl<K: ToEmbed, V: ToEmbed> ToEmbed for BTreeMap<K, V> {
    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
        self.iter()
            .map(|(key, value)| Ok((key.to_embed()?, value.to_embed()?)))
            .collect::<Result<_, _>>()
            .map(EmbedValue::Map)
    }
}

impl<K: FromEmbed + Ord, V: FromEmbed> FromEmbed for BTreeMap<K, V> {
    fn from_embed(value: &EmbedValue) -> Option<Self> {
        let EmbedValue::Map(entries) = value else {
            return None;
        };
        let mut map = Self::new();
        for (key, value) in entries {
            if map
                .insert(K::from_embed(key)?, V::from_embed(value)?)
                .is_some()
            {
                return None;
            }
        }
        Some(map)
    }
}

impl<T: ToEmbed + ?Sized> ToEmbed for &T {
    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
        (**self).to_embed()
    }
}

/// The most items a Rust tuple of generated bindings has.
pub(super) const MAX_TUPLE: usize = 8;

macro_rules! tuples {
    ($(($($name:ident $index:tt),+)),*) => {$(
        impl<$($name: ToEmbed),+> ToEmbed for ($($name,)+) {
            fn to_embed(&self) -> Result<EmbedValue, EmbedError> {
                Ok(EmbedValue::List(vec![$(self.$index.to_embed()?),+]))
            }
        }

        impl<$($name: FromEmbed),+> FromEmbed for ($($name,)+) {
            fn from_embed(value: &EmbedValue) -> Option<Self> {
                let items: Vec<EmbedValue> = match value {
                    EmbedValue::List(items) => items.clone(),
                    EmbedValue::IntList(items) => {
                        items.iter().map(|number| EmbedValue::Int(*number)).collect()
                    }
                    _ => return None,
                };
                let mut items = items.iter();
                let tuple = ($($name::from_embed(items.next()?)?,)+);
                items.next().is_none().then_some(tuple)
            }
        }
    )*};
}

tuples!(
    (A 0),
    (A 0, B 1),
    (A 0, B 1, C 2),
    (A 0, B 1, C 2, D 3),
    (A 0, B 1, C 2, D 3, E 4),
    (A 0, B 1, C 2, D 3, E 4, F 5),
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6),
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7)
);

// ---------------------------------------------------------------- JSON

/// The one entry of a single-key JSON object.
fn single(value: &Value) -> Option<(&String, &Value)> {
    match value {
        Value::Object(object) if object.len() == 1 => object.iter().next(),
        _ => None,
    }
}

fn object(key: &str, value: Value) -> Value {
    let mut object = Map::new();
    object.insert(key.to_owned(), value);
    Value::Object(object)
}

/// The JSON form of a value of type `ty`, as `sley-agent call` prints it:
/// unit `null`, integers as numbers, lists and tuples as arrays, a record
/// as an object, a case as `"Case"` or `{"Case": payload}`, a built-in
/// failure as `{"ArithmeticError": "Overflow"}`, `"None"` or `{"Some": v}`,
/// `{"Ok": v}` or `{"Err": e}`, and a map as an array of `[key, value]`.
#[allow(clippy::too_many_lines)]
pub(super) fn to_json(
    value: &EmbedValue,
    ty: &EmbedType,
    declarations: &[TypeDeclaration],
) -> Value {
    let inner = |value: &EmbedValue, ty: &EmbedType| to_json(value, ty, declarations);
    let shape = |name: &str| {
        declarations
            .iter()
            .find(|declaration| declaration.name == name)
            .map(|declaration| &declaration.shape)
    };
    match (value, ty) {
        (EmbedValue::Unit, _) => Value::Null,
        (EmbedValue::Int(number), _) => Value::from(*number),
        (EmbedValue::Bool(flag), _) => Value::Bool(*flag),
        (EmbedValue::Str(text), _) => Value::from(text.as_str()),
        (EmbedValue::IntList(items), _) => {
            items.iter().map(|number| Value::from(*number)).collect()
        }
        (EmbedValue::List(items), EmbedType::Tuple(types)) if items.len() == types.len() => items
            .iter()
            .zip(types)
            .map(|(value, ty)| inner(value, ty))
            .collect(),
        (EmbedValue::List(items), _) => {
            let item = match ty {
                EmbedType::List(item) => item.as_ref(),
                _ => &EmbedType::Unit,
            };
            items.iter().map(|value| inner(value, item)).collect()
        }
        (EmbedValue::Record(fields), _) => {
            let types = match ty {
                EmbedType::Named(name) => match shape(name) {
                    Some(TypeShape::Record(types)) => types.as_slice(),
                    _ => &[],
                },
                _ => &[],
            };
            Value::Object(
                fields
                    .iter()
                    .map(|(field, value)| {
                        let ty = types
                            .iter()
                            .find(|(name, _)| name == field)
                            .map_or(&EmbedType::Unit, |(_, ty)| ty);
                        (field.clone(), inner(value, ty))
                    })
                    .collect(),
            )
        }
        (EmbedValue::Variant { case, payload }, _) => {
            let name = match ty {
                EmbedType::Named(name) => name.as_str(),
                _ => "",
            };
            if crate::types::failure_kind(name).is_some() {
                return object(name, Value::from(case.as_str()));
            }
            match payload {
                None => Value::from(case.as_str()),
                Some(payload) => {
                    let payload_type = match shape(name) {
                        Some(TypeShape::Variant(cases)) => cases
                            .iter()
                            .find(|(name, _)| name == case)
                            .and_then(|(_, ty)| ty.as_ref()),
                        _ => None,
                    };
                    object(
                        case,
                        inner(payload, payload_type.unwrap_or(&EmbedType::Unit)),
                    )
                }
            }
        }
        (EmbedValue::Option(None), _) => Value::from("None"),
        (EmbedValue::Option(Some(value)), _) => {
            let item = match ty {
                EmbedType::Option(item) => item.as_ref(),
                _ => &EmbedType::Unit,
            };
            object("Some", inner(value, item))
        }
        (EmbedValue::Result(result), _) => {
            let (ok, err) = match ty {
                EmbedType::Result { ok, err } => (ok.as_ref(), err.as_ref()),
                _ => (&EmbedType::Unit, &EmbedType::Unit),
            };
            match result {
                Ok(value) => object("Ok", inner(value, ok)),
                Err(error) => object("Err", inner(error, err)),
            }
        }
        (EmbedValue::Map(entries), _) => {
            let (key_type, value_type) = match ty {
                EmbedType::Map { key, value } => (key.as_ref(), value.as_ref()),
                _ => (&EmbedType::Unit, &EmbedType::Unit),
            };
            entries
                .iter()
                .map(|(key, value)| {
                    Value::Array(vec![inner(key, key_type), inner(value, value_type)])
                })
                .collect()
        }
    }
}

/// A value of type `ty` from its JSON form ([`to_json`]), reading every
/// form `sley-agent call` reads: an integer may also be a decimal string,
/// unit also `[]` or `{}`, an option also `null` or a bare value, a case
/// also `"Type.Case"`, and a built-in failure also its bare case name.
#[allow(clippy::too_many_lines)]
pub(super) fn from_json(
    value: &Value,
    ty: &EmbedType,
    declarations: &[TypeDeclaration],
    depth: usize,
) -> Result<EmbedValue, String> {
    if depth >= MAX_DEPTH {
        return Err(format!("the value nests more than {MAX_DEPTH} levels deep"));
    }
    let inner = |value: &Value, ty: &EmbedType| from_json(value, ty, declarations, depth + 1);
    let boxed = |value: &Value, ty: &EmbedType| inner(value, ty).map(Box::new);
    Ok(match ty {
        EmbedType::Unit
            if value.is_null()
                || value.as_array().is_some_and(Vec::is_empty)
                || value.as_object().is_some_and(Map::is_empty) =>
        {
            EmbedValue::Unit
        }
        EmbedType::Bool => EmbedValue::Bool(value.as_bool().ok_or("expected a boolean")?),
        EmbedType::Int { .. } => EmbedValue::Int(
            match value {
                Value::Number(number) => number.as_i64(),
                Value::String(text) => text.parse().ok(),
                _ => None,
            }
            .ok_or("expected an integer in the i64 range")?,
        ),
        EmbedType::Str => EmbedValue::Str(value.as_str().ok_or("expected a string")?.to_owned()),
        EmbedType::List(item) => {
            let items = value.as_array().ok_or("expected an array")?;
            EmbedValue::List(
                items
                    .iter()
                    .map(|value| inner(value, item))
                    .collect::<Result<_, _>>()?,
            )
        }
        EmbedType::Tuple(types) => {
            let items = value
                .as_array()
                .filter(|items| items.len() == types.len())
                .ok_or_else(|| format!("expected an array of {} items", types.len()))?;
            EmbedValue::List(
                items
                    .iter()
                    .zip(types)
                    .map(|(value, ty)| inner(value, ty))
                    .collect::<Result<_, _>>()?,
            )
        }
        EmbedType::Option(item) => match (value, single(value)) {
            (Value::Null, _) => EmbedValue::Option(None),
            (Value::String(text), _) if text == "None" => EmbedValue::Option(None),
            (_, Some((key, value))) if key == "Some" => {
                EmbedValue::Option(Some(boxed(value, item)?))
            }
            _ => EmbedValue::Option(Some(boxed(value, item)?)),
        },
        EmbedType::Result { ok, err } => match single(value) {
            Some((key, value)) if key == "Ok" => EmbedValue::Result(Ok(boxed(value, ok)?)),
            Some((key, value)) if key == "Err" => EmbedValue::Result(Err(boxed(value, err)?)),
            _ => return Err("expected {\"Ok\": value} or {\"Err\": value}".to_owned()),
        },
        EmbedType::Map { key, value: item } => {
            let entries = value
                .as_array()
                .ok_or("expected an array of [key, value]")?;
            EmbedValue::Map(
                entries
                    .iter()
                    .map(|entry| match entry.as_array().map(Vec::as_slice) {
                        Some([entry_key, entry_value]) => {
                            Ok((inner(entry_key, key)?, inner(entry_value, item)?))
                        }
                        _ => Err("expected a [key, value] entry".to_owned()),
                    })
                    .collect::<Result<_, _>>()?,
            )
        }
        EmbedType::Named(name) => {
            let shape = declarations
                .iter()
                .find(|declaration| declaration.name == *name)
                .map(|declaration| &declaration.shape)
                .ok_or_else(|| format!("no type named `{name}`"))?;
            match shape {
                TypeShape::Record(fields) => {
                    let object = value.as_object().ok_or("expected a record object")?;
                    let mut out = Vec::with_capacity(object.len());
                    for (field, value) in object {
                        let ty = fields
                            .iter()
                            .find(|(name, _)| name == field)
                            .map(|(_, ty)| ty)
                            .ok_or_else(|| format!("`{name}` has no field `{field}`"))?;
                        out.push((field.clone(), inner(value, ty)?));
                    }
                    EmbedValue::Record(out)
                }
                TypeShape::Variant(cases) => {
                    // A built-in failure may be named by its kind:
                    // {"ArithmeticError": "Overflow"}.
                    let value = match single(value) {
                        Some((key, inner))
                            if key == name
                                && inner.is_string()
                                && crate::types::failure_kind(name).is_some() =>
                        {
                            inner
                        }
                        _ => value,
                    };
                    let (case, payload) = match (value, single(value)) {
                        (Value::String(case), _) => (case, None),
                        (_, Some((case, payload))) => (case, Some(payload)),
                        _ => return Err("expected \"Case\" or {\"Case\": payload}".to_owned()),
                    };
                    // `"Type.Case"` names the case of its type.
                    let case = case.rsplit('.').next().unwrap_or(case);
                    let payload_type = cases
                        .iter()
                        .find(|(name, _)| name == case)
                        .map(|(_, payload)| payload)
                        .ok_or_else(|| format!("`{name}` has no case `{case}`"))?;
                    let payload = match (payload, payload_type) {
                        (None, None) => None,
                        (Some(payload), Some(ty)) => Some(boxed(payload, ty)?),
                        (None, Some(_)) => return Err(format!("case `{case}` takes a payload")),
                        (Some(_), None) => return Err(format!("case `{case}` has no payload")),
                    };
                    EmbedValue::Variant {
                        case: case.to_owned(),
                        payload,
                    }
                }
            }
        }
        EmbedType::Unit => return Err("expected null".to_owned()),
        EmbedType::Unsupported(name) => return Err(format!("{name} has no embed form")),
    })
}

// ----------------------------------------------------------- spellings

impl fmt::Display for EmbedType {
    /// The type as Sley spells it: `i64`, `list[T]`, `tuple[A, B]`,
    /// `Option<T>`, `Result<T, E>`, `Map<K, V>`, or a declared type's name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => formatter.write_str("unit"),
            Self::Bool => formatter.write_str("bool"),
            Self::Int { signed, bits } => {
                write!(formatter, "{}{bits}", if *signed { 'i' } else { 'u' })
            }
            Self::Str => formatter.write_str("str"),
            Self::List(item) => write!(formatter, "list[{item}]"),
            Self::Tuple(items) => {
                formatter.write_str("tuple[")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(", ")?;
                    }
                    write!(formatter, "{item}")?;
                }
                formatter.write_str("]")
            }
            Self::Option(item) => write!(formatter, "Option<{item}>"),
            Self::Result { ok, err } => write!(formatter, "Result<{ok}, {err}>"),
            Self::Map { key, value } => write!(formatter, "Map<{key}, {value}>"),
            Self::Named(name) | Self::Unsupported(name) => formatter.write_str(name),
        }
    }
}

impl fmt::Display for Signature {
    /// `name(a: i64, b: list[i64]) -> bool`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}(", self.name)?;
        for (index, (name, ty)) in self.parameters.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{name}: {ty}")?;
        }
        write!(formatter, ") -> {}", self.result)
    }
}

impl fmt::Display for TypeDeclaration {
    /// `record Point { x: i64, y: i64 }` or `variant Shape { Dot, Circle(i64) }`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.shape {
            TypeShape::Record(fields) => {
                write!(formatter, "record {} {{", self.name)?;
                for (index, (name, ty)) in fields.iter().enumerate() {
                    let separator = if index > 0 { "," } else { "" };
                    write!(formatter, "{separator} {name}: {ty}")?;
                }
                formatter.write_str(" }")
            }
            TypeShape::Variant(cases) => {
                write!(formatter, "variant {} {{", self.name)?;
                for (index, (name, payload)) in cases.iter().enumerate() {
                    let separator = if index > 0 { "," } else { "" };
                    match payload {
                        Some(ty) => write!(formatter, "{separator} {name}({ty})")?,
                        None => write!(formatter, "{separator} {name}")?,
                    }
                }
                formatter.write_str(" }")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int() -> EmbedType {
        EmbedType::Int {
            signed: true,
            bits: 64,
        }
    }

    fn shape() -> Vec<TypeDeclaration> {
        vec![TypeDeclaration {
            name: "Shape".into(),
            shape: TypeShape::Variant(vec![("Dot".into(), None), ("Circle".into(), Some(int()))]),
        }]
    }

    #[test]
    fn json_round_trips_every_form() {
        let declarations = shape();
        let cases = [
            (EmbedType::Unit, EmbedValue::Unit, "null"),
            (
                EmbedType::Option(Box::new(int())),
                EmbedValue::Option(Some(Box::new(EmbedValue::Int(3)))),
                r#"{"Some":3}"#,
            ),
            (
                EmbedType::Option(Box::new(int())),
                EmbedValue::Option(None),
                r#""None""#,
            ),
            (
                EmbedType::Map {
                    key: Box::new(EmbedType::Str),
                    value: Box::new(int()),
                },
                EmbedValue::Map(vec![(EmbedValue::Str("a".into()), EmbedValue::Int(1))]),
                r#"[["a",1]]"#,
            ),
            (
                EmbedType::Tuple(vec![int(), EmbedType::Bool]),
                EmbedValue::List(vec![EmbedValue::Int(1), EmbedValue::Bool(true)]),
                "[1,true]",
            ),
            (
                EmbedType::Named("Shape".into()),
                EmbedValue::Variant {
                    case: "Circle".into(),
                    payload: Some(Box::new(EmbedValue::Int(2))),
                },
                r#"{"Circle":2}"#,
            ),
            (
                EmbedType::Named("Shape".into()),
                EmbedValue::case("Dot"),
                r#""Dot""#,
            ),
        ];
        for (ty, value, json) in cases {
            assert_eq!(
                to_json(&value, &ty, &declarations).to_string(),
                json,
                "{ty}"
            );
            let parsed: Value = serde_json::from_str(json).unwrap();
            assert_eq!(from_json(&parsed, &ty, &declarations, 0), Ok(value), "{ty}");
        }
        // `null` is also an empty option; `IntList` prints as an array.
        let none = from_json(&Value::Null, &EmbedType::Option(Box::new(int())), &[], 0);
        assert_eq!(none, Ok(EmbedValue::Option(None)));
        assert_eq!(
            to_json(
                &EmbedValue::IntList(vec![1, 2]),
                &EmbedType::List(Box::new(int())),
                &[]
            )
            .to_string(),
            "[1,2]"
        );
    }

    #[test]
    fn json_reads_every_form_sley_agent_call_reads() {
        let failure = TypeDeclaration {
            name: "ArithmeticError".into(),
            shape: TypeShape::Variant(vec![("Overflow".into(), None)]),
        };
        let mut declarations = shape();
        declarations.push(failure);
        let read = |json: &str, ty: &EmbedType| {
            let value: Value = serde_json::from_str(json).unwrap();
            from_json(&value, ty, &declarations, 0)
        };
        let arithmetic = EmbedType::Named("ArithmeticError".into());
        // A built-in failure is written with its kind, and read either way.
        let overflow = EmbedValue::case("Overflow");
        assert_eq!(
            to_json(&overflow, &arithmetic, &declarations).to_string(),
            r#"{"ArithmeticError":"Overflow"}"#
        );
        assert_eq!(
            read(r#"{"ArithmeticError":"Overflow"}"#, &arithmetic),
            Ok(overflow.clone())
        );
        assert_eq!(read(r#""Overflow""#, &arithmetic), Ok(overflow));
        let shape = EmbedType::Named("Shape".into());
        assert_eq!(read(r#""Shape.Dot""#, &shape), Ok(EmbedValue::case("Dot")));
        for unit in ["null", "[]", "{}"] {
            assert_eq!(read(unit, &EmbedType::Unit), Ok(EmbedValue::Unit), "{unit}");
        }
        let option = EmbedType::Option(Box::new(int()));
        assert_eq!(
            read("4", &option),
            Ok(EmbedValue::Option(Some(Box::new(EmbedValue::Int(4)))))
        );
        // Nested: a result of a built-in failure inside a list.
        let nested = EmbedType::List(Box::new(EmbedType::Result {
            ok: Box::new(int()),
            err: Box::new(arithmetic),
        }));
        let value = EmbedValue::List(vec![EmbedValue::Result(Err(Box::new(EmbedValue::case(
            "Overflow",
        ))))]);
        let json = to_json(&value, &nested, &declarations).to_string();
        assert_eq!(json, r#"[{"Err":{"ArithmeticError":"Overflow"}}]"#);
        assert_eq!(read(&json, &nested), Ok(value));
    }

    #[test]
    fn json_refuses_cases_and_payloads_that_do_not_fit() {
        let declarations = shape();
        let named = EmbedType::Named("Shape".into());
        let refuse = |json: &str| {
            let value: Value = serde_json::from_str(json).unwrap();
            from_json(&value, &named, &declarations, 0).unwrap_err()
        };
        assert!(refuse(r#""Square""#).contains("no case `Square`"));
        assert!(refuse(r#""Circle""#).contains("takes a payload"));
        assert!(refuse(r#"{"Dot":1}"#).contains("has no payload"));
        let deep = (0..=MAX_DEPTH).fold(Value::from(1), |inner, _| Value::Array(vec![inner]));
        let mut ty = int();
        for _ in 0..=MAX_DEPTH {
            ty = EmbedType::List(Box::new(ty));
        }
        assert!(
            from_json(&deep, &ty, &[], 0)
                .unwrap_err()
                .contains("nests more than")
        );
    }

    #[test]
    fn rust_values_round_trip() {
        fn round<T: ToEmbed + FromEmbed + PartialEq + fmt::Debug>(value: &T) {
            let embedded = value.to_embed().unwrap();
            assert_eq!(
                T::from_embed(&embedded).as_ref(),
                Some(value),
                "{embedded:?}"
            );
        }
        round(&());
        round(&Some(3_i64));
        round(&None::<i64>);
        round(&Ok::<i64, String>(1));
        round(&Err::<i64, String>("no".into()));
        round(&BTreeMap::from([
            (1_u8, "a".to_owned()),
            (2, "b".to_owned()),
        ]));
        round(&(1_i64, true, "x".to_owned()));
        round(&vec![vec![1_i32, 2], vec![]]);
        // A result's integer list reads as a vector or a tuple.
        let list = EmbedValue::IntList(vec![4, 5]);
        assert_eq!(Vec::<i64>::from_embed(&list), Some(vec![4, 5]));
        assert_eq!(<(i64, i64)>::from_embed(&list), Some((4, 5)));
        assert_eq!(<(i64,)>::from_embed(&list), None);
        // A map with a repeated key is not a BTreeMap.
        let repeated = EmbedValue::Map(vec![
            (EmbedValue::Int(1), EmbedValue::Bool(true)),
            (EmbedValue::Int(1), EmbedValue::Bool(false)),
        ]);
        assert_eq!(BTreeMap::<i64, bool>::from_embed(&repeated), None);
    }
}

//! Strict request parsing for GHOSTWEAVE residual authoring.
//!
//! Requests and bindings are advisory workbench data. Construction continues
//! through the existing AF1-X, draft, and kernel paths.

pub mod binding;
pub mod choices;
pub mod dependencies;
pub mod edit;
pub mod factored;
pub mod fragments;
pub mod frontier;
pub mod interfaces;
pub mod plan;
pub(crate) mod runtime;
pub(crate) mod store;

use std::cell::Cell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

use crate::draft::DraftRef;
use crate::error::{AgentError, AgentErrorCode, Result};

/// Maximum encoded request size accepted by the residual parser.
pub const MAX_REQUEST_BYTES: usize = 1_048_576;

/// Maximum nested JSON containers accepted by the residual parser.
pub const MAX_JSON_DEPTH: usize = 32;

/// Maximum JSON values accepted in one request.
pub const MAX_JSON_VALUES: usize = 100_000;

const REQUIRED_REQUEST_MEMBERS: [&str; 6] = [
    "residual",
    "base",
    "operation",
    "fragment",
    "bindings",
    "scope",
];

/// A state reference that can be resolved against the current workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BaseRef {
    /// The accepted head observed when planning begins.
    AcceptedCurrent,
    /// An exact accepted state root supplied by the caller.
    AcceptedRoot([u8; 32]),
    /// An explicitly named draft revision; latest-only references are refused.
    Draft(DraftRef),
}

/// Whether the request creates a construction or edits inherited state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    /// Construct from the explicitly bound accepted or draft state.
    Derive,
    /// Edit a source region while preserving the explicitly listed remainder.
    Edit,
}

/// An immutable fragment identity and version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentRef {
    /// Stable, lowercase fragment identifier.
    pub id: String,
    /// Positive immutable fragment version.
    pub version: u32,
}

/// A syntactically valid, closed GHOSTWEAVE request envelope.
///
/// Fragment-specific bindings and selectors are retained as JSON values here;
/// the fragment's versioned schema must validate them before expansion.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// The explicitly selected accepted state or draft revision.
    pub base: BaseRef,
    /// Whether this is a construction or an edit.
    pub operation: Operation,
    /// The immutable fragment selected by the request.
    pub fragment: FragmentRef,
    /// Closed fragment parameters and AF1-X expressions.
    pub bindings: Map<String, Value>,
    /// Exact target names, identities, or a subsequently validated selector.
    pub scope: Value,
    /// Explicit inherited regions and policies for edit requests.
    pub preserve: Option<Value>,
    /// Optional explicitly authored closed relation of permitted completions.
    /// This is a declaration, never the output of an implicit candidate search.
    pub choices: Option<Value>,
}

impl Request {
    pub(crate) fn value(&self) -> Value {
        let base = match &self.base {
            BaseRef::AcceptedCurrent => "current".to_owned(),
            BaseRef::AcceptedRoot(root) => crate::hex::encode(root),
            BaseRef::Draft(reference) => {
                crate::draft::spell(&reference.handle, reference.revision.unwrap_or(0))
            }
        };
        let mut value = serde_json::json!({"residual":1,"base":base,
            "operation":match self.operation {Operation::Derive=>"derive",Operation::Edit=>"edit"},
            "fragment":{"id":self.fragment.id,"version":self.fragment.version},
            "bindings":self.bindings,"scope":self.scope});
        if let Some(preserve) = &self.preserve {
            value["preserve"] = preserve.clone();
        }
        if let Some(choices) = &self.choices {
            value["choices"] = choices.clone();
        }
        value
    }
}

/// Parses and validates the versioned residual request envelope.
///
/// Duplicate keys are rejected at every depth before the request can be
/// hashed or used to modify workspace state. Floating-point numbers are not
/// admitted in this protocol. Integer domain checks remain the responsibility
/// of the selected fragment schema.
///
/// # Errors
///
/// Returns a residual workbench refusal for invalid UTF-8, duplicate or
/// unknown members, unsupported versions, malformed references, invalid
/// field types, or an exceeded parser budget.
pub fn parse_request(bytes: &[u8]) -> Result<Request> {
    request_from_value(strict_json(bytes)?)
}

/// The same strict reader is used for persisted residual artifacts.
pub(crate) fn strict_json(bytes: &[u8]) -> Result<Value> {
    strict_json_with_limit(bytes, MAX_REQUEST_BYTES)
}

pub(crate) fn strict_json_with_limit(bytes: &[u8], limit: usize) -> Result<Value> {
    if bytes.len() > limit {
        return Err(parse_error(format!(
            "JSON input is {} bytes; the limit is {limit}",
            bytes.len()
        )));
    }
    std::str::from_utf8(bytes)
        .map_err(|error| parse_error(format!("request is not UTF-8: {error}")))?;

    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let budget = Rc::new(Cell::new(0));
    let value = StrictValueSeed { depth: 0, budget }
        .deserialize(&mut deserializer)
        .map_err(|error| parse_error(error.to_string()))?;
    deserializer
        .end()
        .map_err(|error| parse_error(error.to_string()))?;

    Ok(value)
}

fn request_from_value(value: Value) -> Result<Request> {
    let Value::Object(mut object) = value else {
        return Err(parse_error("request must be one JSON object"));
    };
    reject_unknown(&object, &REQUIRED_REQUEST_MEMBERS, &["preserve", "choices"])?;

    let version = object
        .remove("residual")
        .ok_or_else(|| parse_error("/residual is required"))?;
    if version.as_u64() != Some(1) {
        return Err(AgentError::new(
            AgentErrorCode::ResidualVersion,
            "/residual must be the integer 1",
        ));
    }

    let base = parse_base(
        &object
            .remove("base")
            .ok_or_else(|| parse_error("/base is required"))?,
    )?;
    let operation = match required_string(&mut object, "operation")?.as_str() {
        "derive" => Operation::Derive,
        "edit" => Operation::Edit,
        other => {
            return Err(parse_error(format!(
                "/operation has unsupported value `{other}`"
            )));
        }
    };
    let fragment = parse_fragment(
        object
            .remove("fragment")
            .ok_or_else(|| parse_error("/fragment is required"))?,
    )?;
    let bindings = object
        .remove("bindings")
        .ok_or_else(|| parse_error("/bindings is required"))?
        .as_object()
        .cloned()
        .ok_or_else(|| parse_error("/bindings must be an object"))?;
    let scope = object
        .remove("scope")
        .ok_or_else(|| parse_error("/scope is required"))?;
    if !scope.is_array() && !scope.is_object() {
        return Err(parse_error(
            "/scope must be an exact target list or typed selector object",
        ));
    }
    let preserve = object.remove("preserve");
    let choices = object.remove("choices");
    if let Some(choices) = &choices {
        self::choices::validate_contract(choices)?;
    }
    match (operation, preserve.as_ref()) {
        (Operation::Edit, None) => {
            return Err(AgentError::new(
                AgentErrorCode::ResidualPreserve,
                "edit requests require an explicit /preserve list or policy object",
            ));
        }
        (Operation::Edit, Some(Value::Array(_) | Value::Object(_))) | (Operation::Derive, None) => {
        }
        (Operation::Derive, Some(_)) => {
            return Err(AgentError::new(
                AgentErrorCode::ResidualPreserve,
                "/preserve is permitted only for an edit request",
            ));
        }
        (Operation::Edit, Some(_)) => {
            return Err(AgentError::new(
                AgentErrorCode::ResidualPreserve,
                "/preserve must be a list or policy object",
            ));
        }
    }

    Ok(Request {
        base,
        operation,
        fragment,
        bindings,
        scope,
        preserve,
        choices,
    })
}

fn parse_base(value: &Value) -> Result<BaseRef> {
    let reference = value
        .as_str()
        .ok_or_else(|| parse_error("/base must be `current`, a 64-digit root, or dN@rK"))?;
    if reference == "current" {
        return Ok(BaseRef::AcceptedCurrent);
    }
    if let Some(root) = crate::hex::decode32(reference) {
        return Ok(BaseRef::AcceptedRoot(root));
    }
    if let Some(draft) = DraftRef::parse(reference).filter(|draft| draft.revision.is_some()) {
        return Ok(BaseRef::Draft(draft));
    }
    Err(parse_error(
        "/base must be `current`, a 64-digit lowercase state root, or an explicit draft revision such as d1@r2",
    ))
}

fn parse_fragment(value: Value) -> Result<FragmentRef> {
    const FRAGMENT_MEMBERS: [&str; 2] = ["id", "version"];
    let Value::Object(mut object) = value else {
        return Err(parse_error("/fragment must be an object"));
    };
    reject_unknown(&object, &FRAGMENT_MEMBERS, &[])
        .map_err(|error| parse_error(format!("/fragment: {}", error.detail())))?;
    let id = object
        .remove("id")
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| parse_error("/fragment/id must be a string"))?;
    if id.is_empty()
        || !id.as_bytes()[0].is_ascii_lowercase()
        || !id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
    {
        return Err(parse_error(
            "/fragment/id must start with a lowercase ASCII letter and contain only lowercase letters, digits, `.`, `_`, or `-`",
        ));
    }
    let version = object
        .remove("version")
        .and_then(|value| value.as_u64())
        .and_then(|value| u32::try_from(value).ok())
        .filter(|version| *version > 0)
        .ok_or_else(|| parse_error("/fragment/version must be a positive 32-bit integer"))?;
    Ok(FragmentRef { id, version })
}

fn required_string(object: &mut Map<String, Value>, member: &str) -> Result<String> {
    object
        .remove(member)
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| parse_error(format!("/{member} must be a string")))
}

fn reject_unknown(object: &Map<String, Value>, required: &[&str], optional: &[&str]) -> Result<()> {
    for member in required {
        if !object.contains_key(*member) {
            return Err(parse_error(format!("missing required member `{member}`")));
        }
    }
    for member in object.keys() {
        if !required.contains(&member.as_str()) && !optional.contains(&member.as_str()) {
            return Err(parse_error(format!("unknown member `{member}`")));
        }
    }
    Ok(())
}

fn parse_error(detail: impl Into<String>) -> AgentError {
    AgentError::new(AgentErrorCode::ResidualParse, detail)
}

struct StrictValueSeed {
    depth: usize,
    budget: Rc<Cell<usize>>,
}

impl<'de> DeserializeSeed<'de> for StrictValueSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let count = self.budget.get();
        if count >= MAX_JSON_VALUES {
            return Err(de::Error::custom(format!(
                "request exceeds the {MAX_JSON_VALUES}-value limit"
            )));
        }
        self.budget.set(count + 1);
        if self.depth > MAX_JSON_DEPTH {
            return Err(de::Error::custom(format!(
                "request exceeds the {MAX_JSON_DEPTH}-container nesting limit"
            )));
        }
        deserializer.deserialize_any(StrictValueVisitor {
            depth: self.depth,
            budget: self.budget,
        })
    }
}

struct StrictValueVisitor {
    depth: usize,
    budget: Rc<Cell<usize>>,
}

impl<'de> Visitor<'de> for StrictValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded JSON value without duplicate keys or floating-point numbers")
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_i128<E>(self, value: i128) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_i128(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("integer exceeds the supported JSON number range"))
    }

    fn visit_u128<E>(self, value: u128) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_u128(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("integer exceeds the supported JSON number range"))
    }

    fn visit_f64<E>(self, _value: f64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom(
            "floating-point numbers are not permitted in residual requests",
        ))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let depth = self.depth + 1;
        if depth > MAX_JSON_DEPTH {
            return Err(de::Error::custom(format!(
                "request exceeds the {MAX_JSON_DEPTH}-container nesting limit"
            )));
        }
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(StrictValueSeed {
            depth,
            budget: Rc::clone(&self.budget),
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let depth = self.depth + 1;
        if depth > MAX_JSON_DEPTH {
            return Err(de::Error::custom(format!(
                "request exceeds the {MAX_JSON_DEPTH}-container nesting limit"
            )));
        }
        let mut object = Map::new();
        let mut seen = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(de::Error::custom(format!(
                    "duplicate object member `{key}`"
                )));
            }
            let value = map.next_value_seed(StrictValueSeed {
                depth,
                budget: Rc::clone(&self.budget),
            })?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

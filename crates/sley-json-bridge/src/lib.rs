//! SMP1 JSON bridge (S20-420, contract `docs/spec/SMP1_JSON_BRIDGE_V1.md`,
//! ADR-0034).
//!
//! A generated, non-canonical text representation of SMP1 frames and of the
//! records SMP1 owns. Bytes stay the only canonical form: [`frame_from_json`]
//! re-encodes through the frozen `sley-protocol` codec, and nothing here is
//! hashed, stored, or compared as identity. The bridge owns no semantics: it
//! checks text resources, object shape, and the declared encodings, and
//! every other failure keeps the codec's `PROTOCOL_*` code.

#![forbid(unsafe_code)]

use core::fmt;

use serde_json::{Map, Value};
use sley_id::{ProtocolHandshakeId, SchemaEpochId};
use sley_protocol::{
    BoundedContext, DecodedFrame, EncodedFrame, FEATURE_CANCEL, FEATURE_CHECKSUM,
    FEATURE_JSON_BRIDGE, FEATURE_NATIVE_TESTS_V1, FEATURE_STREAM, FLAG_CANCEL, FLAG_FAILED,
    FLAG_STREAM, FrameKind, Hello, LimitProfile, MAX_FRAME_BYTES, Method, PROTOCOL_VERSION,
    PROTOCOL_VERSION_V2, PROTOCOL_VERSION_V3, ProtocolError, ProtocolFailure, ProtocolFrame,
    Retryability, SelectedProfile, SessionId, StreamChunk, decode_frame, decode_frame_for_version,
    encode_frame, encode_frame_for_version, encode_hello_frame,
};

/// Largest JSON text the bridge parses: four times the absolute frame
/// ceiling, so any frame that fits on the wire fits in text with room for
/// its field names and envelope (contract section 3).
#[allow(
    clippy::cast_possible_truncation,
    reason = "the assertion below pins the frame ceiling inside a 32-bit usize with room for the factor"
)]
pub const MAX_JSON_TEXT_BYTES: usize = 4 * (MAX_FRAME_BYTES as usize);
const _: () = assert!(MAX_FRAME_BYTES <= (u32::MAX as u64) / 4);
/// Deepest object or array nesting the bridge parses (contract section 3).
pub const MAX_JSON_DEPTH: usize = 32;
/// Largest number of JSON value positions the bridge materializes from one
/// text: every object, array, string, number, boolean, and null counts.
/// Open-ended lists in bridge subjects are protocol-bounded in the dozens
/// (the frozen codec caps hello lists at 4,096), so 2^20 exceeds any
/// legitimate text by orders of magnitude while bounding the materialized
/// `Value` tree to tens of mebibytes under the text ceiling (contract
/// section 3).
pub const MAX_JSON_ELEMENTS: usize = 1_048_576;
/// The element ceiling stays below the byte ceiling: every counted value
/// position needs at least one text byte, so the count check can always
/// fire before the byte ceiling is reached.
const _: () = assert!(MAX_JSON_ELEMENTS <= MAX_JSON_TEXT_BYTES);
/// Largest integer emitted as a JSON number; larger values travel as decimal
/// strings (contract section 1).
pub const MAX_JSON_NUMBER: u64 = (1 << 53) - 1;
/// The generated method table, embedded verbatim from
/// `conformance/smp1-json-bridge/v1/methods.json`.
pub const METHOD_TABLE_JSON: &str =
    include_str!("../../../conformance/smp1-json-bridge/v1/methods.json");
/// The generated version 2 method table, embedded verbatim from
/// `conformance/smp1-json-bridge/v2/methods.json`: the version 1 table
/// plus exactly `entity.version` (306) and `entity.signature` (307).
/// Additive export for the capable CLI profile; the default table above
/// is unchanged.
pub const METHOD_TABLE_V2_JSON: &str =
    include_str!("../../../conformance/smp1-json-bridge/v2/methods.json");
/// The generated version 3 method table, embedded verbatim from
/// `conformance/smp1-json-bridge/v3/methods.json`: the frozen version 1 and
/// 2 tables plus exactly the three reserved native rows (`tests.report_read`
/// 605, `tests.replay` 606, `tests.attempt_status` 607) owned by the native
/// draft contract (`docs/spec/NATIVE_TEST_ADMISSION_V1.md` appendix D).
/// Additive export for the v3-capable CLI profile; both tables above are
/// unchanged.
pub const METHOD_TABLE_V3_JSON: &str =
    include_str!("../../../conformance/smp1-json-bridge/v3/methods.json");

// ---------------------------------------------------------------------------
// Failures
// ---------------------------------------------------------------------------

/// The bridge's own stable failures (contract section 5).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonBridgeErrorCode {
    /// `JSON_BRIDGE_SHAPE_INVALID`: unparseable text, an unknown, missing, or
    /// null field, a wrong JSON type, a wrong fixed length, or an unknown
    /// frozen name.
    ShapeInvalid,
    /// `JSON_BRIDGE_NUMBER_INVALID`: an integer outside its declared form.
    NumberInvalid,
    /// `JSON_BRIDGE_HEX_INVALID`: uppercase, odd-length, or non-hex bytes.
    HexInvalid,
    /// `JSON_BRIDGE_METHOD_UNKNOWN`: a method name or tag outside the table.
    MethodUnknown,
    /// `JSON_BRIDGE_RESOURCE_LIMIT`: text above the size, depth, or
    /// element ceiling.
    ResourceLimit,
}

impl JsonBridgeErrorCode {
    /// Every bridge code in numeric order.
    pub const ALL: [Self; 5] = [
        Self::ShapeInvalid,
        Self::NumberInvalid,
        Self::HexInvalid,
        Self::MethodUnknown,
        Self::ResourceLimit,
    ];

    /// The frozen symbol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShapeInvalid => "JSON_BRIDGE_SHAPE_INVALID",
            Self::NumberInvalid => "JSON_BRIDGE_NUMBER_INVALID",
            Self::HexInvalid => "JSON_BRIDGE_HEX_INVALID",
            Self::MethodUnknown => "JSON_BRIDGE_METHOD_UNKNOWN",
            Self::ResourceLimit => "JSON_BRIDGE_RESOURCE_LIMIT",
        }
    }

    /// The frozen numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::ShapeInvalid => 42_000,
            Self::NumberInvalid => 42_001,
            Self::HexInvalid => 42_002,
            Self::MethodUnknown => 42_003,
            Self::ResourceLimit => 42_004,
        }
    }
}

/// A bridge operation's failure: either the bridge's own code or the frozen
/// codec's code, verbatim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeError {
    /// A resource, shape, or encoding failure of the bridge.
    Bridge(JsonBridgeErrorCode),
    /// A failure of the frozen codec, carried unchanged.
    Protocol(ProtocolError),
}

impl BridgeError {
    /// The numeric code of either owner.
    #[must_use]
    pub const fn numeric(&self) -> u32 {
        match self {
            Self::Bridge(code) => code.numeric(),
            Self::Protocol(error) => error.code().numeric(),
        }
    }

    /// The symbol of either owner.
    #[must_use]
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::Bridge(code) => code.as_str(),
            Self::Protocol(error) => error.code().as_str(),
        }
    }
}

impl BridgeError {
    /// The failure envelope an endpoint answers with when a text cannot be
    /// bridged: the bridge's or the codec's code and symbol, phase zero, no
    /// retry, no incident, and no details.
    #[must_use]
    pub fn envelope(&self) -> ProtocolFailure {
        let mut failure = ProtocolFailure::protocol(match self {
            Self::Bridge(_) => sley_protocol::ProtocolErrorCode::PayloadInvalid,
            Self::Protocol(error) => error.code(),
        });
        failure.code = self.numeric();
        failure.symbol = self.symbol().to_string();
        failure
    }
}

impl From<ProtocolError> for BridgeError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.symbol())
    }
}

impl std::error::Error for BridgeError {}

/// The bridge result type.
pub type Result<T> = core::result::Result<T, BridgeError>;

const fn fail<T>(code: JsonBridgeErrorCode) -> Result<T> {
    Err(BridgeError::Bridge(code))
}

// ---------------------------------------------------------------------------
// Method table
// ---------------------------------------------------------------------------

/// One row of the generated method table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodEntry {
    /// The frozen tag.
    pub tag: u32,
    /// The frozen name.
    pub name: String,
    /// The family name (`session`, `repository`, `query`, `candidate`,
    /// `transaction`, `runtime`).
    pub family: String,
    /// Whether the method is reserved at this revision.
    pub reserved: bool,
    /// The owning work package as written in the contract table.
    pub owner: String,
}

/// Parses the embedded method table.
///
/// # Errors
///
/// Returns `JSON_BRIDGE_SHAPE_INVALID` if the embedded table is malformed.
pub fn method_table() -> Result<Vec<MethodEntry>> {
    let table: Value = serde_json::from_str(METHOD_TABLE_JSON)
        .map_err(|_| BridgeError::Bridge(JsonBridgeErrorCode::ShapeInvalid))?;
    let rows = table.get("methods").and_then(Value::as_array);
    let Some(rows) = rows else {
        return fail(JsonBridgeErrorCode::ShapeInvalid);
    };
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let fields = object(row, &["family", "name", "owner", "reserved", "tag"], &[])?;
        entries.push(MethodEntry {
            tag: u32_field(&fields["tag"])?,
            name: string_field(&fields["name"])?.to_string(),
            family: string_field(&fields["family"])?.to_string(),
            reserved: bool_field(&fields["reserved"])?,
            owner: string_field(&fields["owner"])?.to_string(),
        });
    }
    Ok(entries)
}

/// Resolves a frozen method name.
#[must_use]
pub fn method_by_name(name: &str) -> Option<Method> {
    Method::ALL
        .iter()
        .copied()
        .find(|method| method.name() == name)
}

/// The frozen "no method" tag carried by hello frames and by frame-level
/// failure responses; it renders as the empty name.
pub const NO_METHOD: u32 = 0;

fn method_name(tag: u32) -> Result<&'static str> {
    if tag == NO_METHOD {
        return Ok("");
    }
    Method::from_tag(tag)
        .map(Method::name)
        .map_err(|_| BridgeError::Bridge(JsonBridgeErrorCode::MethodUnknown))
}

fn method_tag(name: &str) -> Result<u32> {
    if name.is_empty() {
        return Ok(NO_METHOD);
    }
    method_by_name(name)
        .map(Method::tag)
        .ok_or(BridgeError::Bridge(JsonBridgeErrorCode::MethodUnknown))
}

/// Resolves a frozen method name under an explicitly selected protocol
/// version: the version 1 table plus exactly `entity.version` (306) and
/// `entity.signature` (307) under version 2, plus exactly the three reserved
/// native rows (605-607) under version 3.
fn method_tag_for_version(name: &str, version: u32) -> Result<u32> {
    if name.is_empty() {
        return Ok(NO_METHOD);
    }
    if version == PROTOCOL_VERSION_V3 {
        return Method::V3_ALL
            .iter()
            .copied()
            .find(|method| method.name() == name)
            .map(Method::tag)
            .ok_or(BridgeError::Bridge(JsonBridgeErrorCode::MethodUnknown));
    }
    if version == PROTOCOL_VERSION_V2 {
        return Method::V2_ALL
            .iter()
            .copied()
            .find(|method| method.name() == name)
            .map(Method::tag)
            .ok_or(BridgeError::Bridge(JsonBridgeErrorCode::MethodUnknown));
    }
    method_tag(name)
}

// ---------------------------------------------------------------------------
// Declared encodings
// ---------------------------------------------------------------------------

fn integer(value: u64) -> Value {
    if value <= MAX_JSON_NUMBER {
        Value::from(value)
    } else {
        Value::String(value.to_string())
    }
}

fn hex(bytes: &[u8]) -> Value {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        text.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
    }
    Value::String(text)
}

fn parse_decimal(text: &str) -> Result<u64> {
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return fail(JsonBridgeErrorCode::NumberInvalid);
    }
    text.parse()
        .map_err(|_| BridgeError::Bridge(JsonBridgeErrorCode::NumberInvalid))
}

fn u64_field(value: &Value) -> Result<u64> {
    match value {
        Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                if value <= MAX_JSON_NUMBER {
                    return Ok(value);
                }
            } else if number.as_i64() == Some(0) {
                // The text `-0`: serde_json has no negative integer zero, so
                // the parser reports it exactly as it reports `-0.0`, and the
                // reader normalizes either spelling to 0 (contract section 8).
                return Ok(0);
            } else if number
                .as_f64()
                .is_some_and(|float| float == 0.0 && float.is_sign_negative())
            {
                // The texts `-0` and `-0.0` parse as negative zero.
                return Ok(0);
            }
            Err(BridgeError::Bridge(JsonBridgeErrorCode::NumberInvalid))
        }
        Value::String(text) => parse_decimal(text),
        _ => fail(JsonBridgeErrorCode::ShapeInvalid),
    }
}

fn u32_field(value: &Value) -> Result<u32> {
    u32::try_from(u64_field(value)?)
        .map_err(|_| BridgeError::Bridge(JsonBridgeErrorCode::NumberInvalid))
}

fn bool_field(value: &Value) -> Result<bool> {
    value
        .as_bool()
        .ok_or(BridgeError::Bridge(JsonBridgeErrorCode::ShapeInvalid))
}

fn string_field(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or(BridgeError::Bridge(JsonBridgeErrorCode::ShapeInvalid))
}

fn hex_field(value: &Value) -> Result<Vec<u8>> {
    let text = string_field(value)?;
    if text.len() % 2 != 0 {
        return fail(JsonBridgeErrorCode::HexInvalid);
    }
    let nibble = |byte: u8| -> Result<u8> {
        match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => fail(JsonBridgeErrorCode::HexInvalid),
        }
    };
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        out.push((nibble(pair[0])? << 4) | nibble(pair[1])?);
    }
    Ok(out)
}

fn hex32_field(value: &Value) -> Result<[u8; 32]> {
    <[u8; 32]>::try_from(hex_field(value)?)
        .map_err(|_| BridgeError::Bridge(JsonBridgeErrorCode::ShapeInvalid))
}

fn list_field(value: &Value) -> Result<&Vec<Value>> {
    value
        .as_array()
        .ok_or(BridgeError::Bridge(JsonBridgeErrorCode::ShapeInvalid))
}

fn object<'a>(
    value: &'a Value,
    fields: &[&str],
    nullable: &[&str],
) -> Result<&'a Map<String, Value>> {
    let Some(map) = value.as_object() else {
        return fail(JsonBridgeErrorCode::ShapeInvalid);
    };
    if map.len() != fields.len() {
        return fail(JsonBridgeErrorCode::ShapeInvalid);
    }
    for field in fields {
        match map.get(*field) {
            None => return fail(JsonBridgeErrorCode::ShapeInvalid),
            Some(Value::Null) if !nullable.contains(field) => {
                return fail(JsonBridgeErrorCode::ShapeInvalid);
            }
            Some(_) => {}
        }
    }
    Ok(map)
}

fn insert(map: &mut Map<String, Value>, field: &str, value: Value) {
    map.insert(field.to_string(), value);
}

fn render(value: &Value) -> String {
    value.to_string()
}

/// Checks the text ceilings before parsing (contract section 3): byte
/// length, nesting depth, and value-position count, all in one
/// allocation-free scan. A value position is every `{`, `[`, `,`, and `:`
/// outside strings: each introduces exactly one value, so the materialized
/// tree holds at most positions + 1 values and the count check bounds
/// allocation before `serde_json` runs.
///
/// # Errors
///
/// Returns `JSON_BRIDGE_RESOURCE_LIMIT`.
pub fn check_resources(text: &str) -> Result<()> {
    if text.len() > MAX_JSON_TEXT_BYTES {
        return fail(JsonBridgeErrorCode::ResourceLimit);
    }
    let mut depth = 0usize;
    let mut positions = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in text.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_JSON_DEPTH {
                    return fail(JsonBridgeErrorCode::ResourceLimit);
                }
                positions += 1;
                if positions >= MAX_JSON_ELEMENTS {
                    return fail(JsonBridgeErrorCode::ResourceLimit);
                }
            }
            b',' | b':' => {
                positions += 1;
                if positions >= MAX_JSON_ELEMENTS {
                    return fail(JsonBridgeErrorCode::ResourceLimit);
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

fn parse(text: &str) -> Result<Value> {
    check_resources(text)?;
    serde_json::from_str(text).map_err(|_| BridgeError::Bridge(JsonBridgeErrorCode::ShapeInvalid))
}

// ---------------------------------------------------------------------------
// Limits, bounds, flags, features
// ---------------------------------------------------------------------------

const LIMIT_FIELDS: [&str; 8] = [
    "max_frame_bytes",
    "max_entities",
    "max_edges",
    "max_depth",
    "max_response_bytes",
    "max_work",
    "max_inflight",
    "max_sessions",
];
const BOUNDS_FIELDS: [&str; 8] = [
    "applied_limits",
    "returned_bytes",
    "returned_entities",
    "returned_edges",
    "reached_depth",
    "omitted",
    "truncated",
    "continuation",
];
const FLAG_FIELDS: [&str; 3] = ["cancel", "stream", "failed"];
const FEATURE_FIELDS: [&str; 4] = ["cancel", "stream", "json_bridge", "checksum"];
const FRAME_FIELDS: [&str; 8] = [
    "protocol_version",
    "session",
    "request_id",
    "kind",
    "method",
    "flags",
    "bounds",
    "body",
];
const HELLO_FIELDS: [&str; 7] = [
    "protocol_versions",
    "schema_epochs",
    "limits",
    "methods",
    "features",
    "adapters",
    "effects",
];
const SELECTED_FIELDS: [&str; 8] = [
    "protocol_version",
    "schema_epoch",
    "limits",
    "methods",
    "features",
    "adapters",
    "effects",
    "handshake_id",
];
const FAILURE_FIELDS: [&str; 6] = [
    "code",
    "symbol",
    "phase",
    "retryability",
    "incident",
    "details",
];
const CHUNK_FIELDS: [&str; 3] = ["index", "total", "bytes"];

fn limits_value(limits: &LimitProfile) -> Value {
    let mut map = Map::new();
    insert(&mut map, "max_frame_bytes", integer(limits.max_frame_bytes));
    insert(&mut map, "max_entities", integer(limits.max_entities));
    insert(&mut map, "max_edges", integer(limits.max_edges));
    insert(&mut map, "max_depth", integer(u64::from(limits.max_depth)));
    insert(
        &mut map,
        "max_response_bytes",
        integer(limits.max_response_bytes),
    );
    insert(&mut map, "max_work", integer(limits.max_work));
    insert(
        &mut map,
        "max_inflight",
        integer(u64::from(limits.max_inflight)),
    );
    insert(
        &mut map,
        "max_sessions",
        integer(u64::from(limits.max_sessions)),
    );
    Value::Object(map)
}

fn limits_from_value(value: &Value) -> Result<LimitProfile> {
    let map = object(value, &LIMIT_FIELDS, &[])?;
    Ok(LimitProfile {
        max_frame_bytes: u64_field(&map["max_frame_bytes"])?,
        max_entities: u64_field(&map["max_entities"])?,
        max_edges: u64_field(&map["max_edges"])?,
        max_depth: u32_field(&map["max_depth"])?,
        max_response_bytes: u64_field(&map["max_response_bytes"])?,
        max_work: u64_field(&map["max_work"])?,
        max_inflight: u32_field(&map["max_inflight"])?,
        max_sessions: u32_field(&map["max_sessions"])?,
    })
}

fn bounds_value(bounds: &BoundedContext) -> Value {
    let mut map = Map::new();
    insert(
        &mut map,
        "applied_limits",
        limits_value(&bounds.applied_limits),
    );
    insert(&mut map, "returned_bytes", integer(bounds.returned_bytes));
    insert(
        &mut map,
        "returned_entities",
        integer(bounds.returned_entities),
    );
    insert(&mut map, "returned_edges", integer(bounds.returned_edges));
    insert(
        &mut map,
        "reached_depth",
        integer(u64::from(bounds.reached_depth)),
    );
    insert(&mut map, "omitted", integer(bounds.omitted));
    insert(&mut map, "truncated", Value::Bool(bounds.truncated));
    insert(&mut map, "continuation", Value::Bool(bounds.continuation));
    Value::Object(map)
}

fn bounds_from_value(value: &Value) -> Result<BoundedContext> {
    let map = object(value, &BOUNDS_FIELDS, &[])?;
    Ok(BoundedContext {
        applied_limits: limits_from_value(&map["applied_limits"])?,
        returned_bytes: u64_field(&map["returned_bytes"])?,
        returned_entities: u64_field(&map["returned_entities"])?,
        returned_edges: u64_field(&map["returned_edges"])?,
        reached_depth: u32_field(&map["reached_depth"])?,
        omitted: u64_field(&map["omitted"])?,
        truncated: bool_field(&map["truncated"])?,
        continuation: bool_field(&map["continuation"])?,
    })
}

fn bits_value(bits: u32, fields: &[&str], masks: &[u32]) -> Result<Value> {
    let known = masks.iter().fold(0, |acc, mask| acc | mask);
    if bits & !known != 0 {
        return fail(JsonBridgeErrorCode::ShapeInvalid);
    }
    let mut map = Map::new();
    for (field, mask) in fields.iter().zip(masks) {
        insert(&mut map, field, Value::Bool(bits & mask != 0));
    }
    Ok(Value::Object(map))
}

fn bits_from_value(value: &Value, fields: &[&str], masks: &[u32]) -> Result<u32> {
    let map = object(value, fields, &[])?;
    let mut bits = 0;
    for (field, mask) in fields.iter().zip(masks) {
        if bool_field(&map[*field])? {
            bits |= mask;
        }
    }
    Ok(bits)
}

const FLAG_MASKS: [u32; 3] = [FLAG_CANCEL, FLAG_STREAM, FLAG_FAILED];
const FEATURE_MASKS: [u32; 4] = [
    FEATURE_CANCEL,
    FEATURE_STREAM,
    FEATURE_JSON_BRIDGE,
    FEATURE_CHECKSUM,
];

fn kind_name(kind: FrameKind) -> &'static str {
    match kind {
        FrameKind::Request => "request",
        FrameKind::Response => "response",
        FrameKind::Event => "event",
        FrameKind::Hello => "hello",
    }
}

fn kind_from_value(value: &Value) -> Result<FrameKind> {
    match string_field(value)? {
        "request" => Ok(FrameKind::Request),
        "response" => Ok(FrameKind::Response),
        "event" => Ok(FrameKind::Event),
        "hello" => Ok(FrameKind::Hello),
        _ => fail(JsonBridgeErrorCode::ShapeInvalid),
    }
}

fn retryability_name(retryability: Retryability) -> &'static str {
    match retryability {
        Retryability::Never => "never",
        Retryability::AfterRequery => "after_requery",
        Retryability::AfterCapability => "after_capability",
        Retryability::AfterLimitChange => "after_limit_change",
        Retryability::TransientHost => "transient_host",
    }
}

fn retryability_from_value(value: &Value) -> Result<Retryability> {
    match string_field(value)? {
        "never" => Ok(Retryability::Never),
        "after_requery" => Ok(Retryability::AfterRequery),
        "after_capability" => Ok(Retryability::AfterCapability),
        "after_limit_change" => Ok(Retryability::AfterLimitChange),
        "transient_host" => Ok(Retryability::TransientHost),
        _ => fail(JsonBridgeErrorCode::ShapeInvalid),
    }
}

fn hex32_list(items: &[[u8; 32]]) -> Value {
    Value::Array(items.iter().map(|item| hex(item)).collect())
}

fn hex32_list_from_value(value: &Value) -> Result<Vec<[u8; 32]>> {
    list_field(value)?.iter().map(hex32_field).collect()
}

fn method_names(tags: &[u32]) -> Result<Value> {
    tags.iter()
        .map(|tag| method_name(*tag).map(|name| Value::String(name.to_string())))
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}

/// Resolves a frozen method name under protocol version 2: the version 1
/// table plus exactly `entity.version` (306) and `entity.signature` (307).
fn method_name_versioned(tag: u32) -> Result<&'static str> {
    if tag == NO_METHOD {
        return Ok("");
    }
    Method::from_tag_versioned(tag, PROTOCOL_VERSION_V2)
        .map(Method::name)
        .map_err(|_| BridgeError::Bridge(JsonBridgeErrorCode::MethodUnknown))
}

/// Resolves a frozen method name under an explicitly selected protocol
/// version: version 2 resolves exactly as above; version 3 resolves the
/// sorted union with the three reserved native rows (605-607). Any other
/// version resolves version 1, matching the legacy default.
fn method_name_for_version(tag: u32, version: u32) -> Result<&'static str> {
    if tag == NO_METHOD {
        return Ok("");
    }
    if version == PROTOCOL_VERSION_V3 {
        return Method::from_tag_versioned(tag, PROTOCOL_VERSION_V3)
            .map(Method::name)
            .map_err(|_| BridgeError::Bridge(JsonBridgeErrorCode::MethodUnknown));
    }
    if version == PROTOCOL_VERSION_V2 {
        return method_name_versioned(tag);
    }
    method_name(tag)
}

fn method_names_versioned(tags: &[u32]) -> Result<Value> {
    tags.iter()
        .map(|tag| method_name_versioned(*tag).map(|name| Value::String(name.to_string())))
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}

fn method_names_for_version(tags: &[u32], version: u32) -> Result<Value> {
    tags.iter()
        .map(|tag| {
            method_name_for_version(*tag, version).map(|name| Value::String(name.to_string()))
        })
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}

fn method_tags_from_value(value: &Value) -> Result<Vec<u32>> {
    list_field(value)?
        .iter()
        .map(|item| method_tag(string_field(item)?))
        .collect()
}

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

/// Renders a decoded frame as the `Frame` object (contract section 2).
///
/// # Errors
///
/// Returns `JSON_BRIDGE_METHOD_UNKNOWN` for a method tag outside the table
/// and `JSON_BRIDGE_SHAPE_INVALID` for flag bits the bridge cannot name.
pub fn frame_value(frame: &ProtocolFrame) -> Result<Value> {
    frame_value_with(frame, PROTOCOL_VERSION)
}

/// Renders the `Frame` object naming methods under an explicitly selected
/// protocol version: the version 1 names, plus exactly `entity.version`
/// (306) and `entity.signature` (307) under version 2, plus exactly the
/// three reserved native rows (605-607) under version 3. Every other field
/// renders exactly as the version 1 object.
///
/// # Errors
///
/// Returns a bridge naming failure for a method tag outside the selected
/// version's table.
pub fn frame_value_for_version(frame: &ProtocolFrame, version: u32) -> Result<Value> {
    frame_value_with(frame, version)
}

fn frame_value_with(frame: &ProtocolFrame, version: u32) -> Result<Value> {
    let method = if version == PROTOCOL_VERSION_V3 {
        // Version 3 names the sorted union with the three reserved native
        // rows (605-607); every other version resolves exactly as before
        // (version 1 and below legacy, anything else version 2).
        method_name_for_version(frame.method, version)?
    } else if version <= PROTOCOL_VERSION {
        method_name(frame.method)?
    } else {
        method_name_versioned(frame.method)?
    };
    let mut map = Map::new();
    insert(
        &mut map,
        "protocol_version",
        integer(u64::from(frame.protocol_version)),
    );
    insert(
        &mut map,
        "session",
        frame
            .session
            .map_or(Value::Null, |session| hex(session.as_bytes())),
    );
    insert(&mut map, "request_id", integer(frame.request_id));
    insert(
        &mut map,
        "kind",
        Value::String(kind_name(frame.kind).to_string()),
    );
    insert(&mut map, "method", Value::String(method.to_string()));
    insert(
        &mut map,
        "flags",
        bits_value(frame.flags, &FLAG_FIELDS, &FLAG_MASKS)?,
    );
    insert(&mut map, "bounds", bounds_value(&frame.bounds));
    insert(&mut map, "body", hex(&frame.body));
    Ok(Value::Object(map))
}

/// Parses the `Frame` object into a decoded frame without encoding it.
///
/// # Errors
///
/// Returns the bridge's shape and encoding codes in contract precedence.
pub fn frame_from_value(value: &Value) -> Result<ProtocolFrame> {
    frame_from_value_with(value, PROTOCOL_VERSION)
}

/// Parses the `Frame` object resolving method names under an explicitly
/// selected protocol version. Additive export for version-aware
/// endpoints; the default above stays frozen version 1.
///
/// # Errors
///
/// Returns the bridge's shape and encoding codes in contract precedence.
pub fn frame_from_value_for_version(value: &Value, version: u32) -> Result<ProtocolFrame> {
    frame_from_value_with(value, version)
}

fn frame_from_value_with(value: &Value, version: u32) -> Result<ProtocolFrame> {
    let map = object(value, &FRAME_FIELDS, &["session"])?;
    let protocol_version = u32_field(&map["protocol_version"])?;
    let session = match &map["session"] {
        Value::Null => None,
        other => Some(SessionId::from_bytes(hex32_field(other)?)),
    };
    let request_id = u64_field(&map["request_id"])?;
    let kind = kind_from_value(&map["kind"])?;
    let name = string_field(&map["method"])?;
    let method = method_tag_for_version(name, version)?;
    let flags = bits_from_value(&map["flags"], &FLAG_FIELDS, &FLAG_MASKS)?;
    let bounds = bounds_from_value(&map["bounds"])?;
    let body = hex_field(&map["body"])?;
    let frame = ProtocolFrame {
        protocol_version,
        session,
        request_id,
        kind,
        method,
        flags,
        bounds,
        body,
    };
    if kind == FrameKind::Hello {
        // The all-zero bounds are the bridge's own rule (contract section 8);
        // the session, request id, method, and flags are the codec's hello
        // header rule (SMP1 section 2), so the codec judges them and a
        // violation keeps PROTOCOL_FRAME_INVALID instead of a bridge code.
        if frame.bounds != BoundedContext::none() {
            return fail(JsonBridgeErrorCode::ShapeInvalid);
        }
        frame.validate_header()?;
    }
    Ok(frame)
}

/// Decodes an encoded SMP1 frame with the frozen codec under the absolute
/// ceiling and renders it as the `Frame` object.
///
/// # Errors
///
/// Returns the codec's `PROTOCOL_*` code verbatim, or a bridge code when the
/// decoded frame cannot be named.
pub fn frame_to_json(bytes: &[u8]) -> Result<String> {
    let (decoded, _) = decode_frame(bytes, MAX_FRAME_BYTES)?;
    let frame = match decoded {
        DecodedFrame::Hello(hello) => ProtocolFrame {
            protocol_version: sley_protocol::PROTOCOL_VERSION,
            session: None,
            request_id: 0,
            kind: FrameKind::Hello,
            method: 0,
            flags: 0,
            bounds: BoundedContext::none(),
            body: hello.encode()?,
        },
        DecodedFrame::Request(frame) | DecodedFrame::Response(frame) => frame,
    };
    Ok(render(&frame_value(&frame)?))
}

/// Renders a frame decoded under an explicitly selected protocol version.
/// Additive export for version-aware endpoints; the default above stays
/// frozen version 1.
///
/// # Errors
///
/// Returns the codec's `PROTOCOL_*` code verbatim, or a bridge code when the
/// decoded frame cannot be named.
pub fn frame_to_json_for_version(bytes: &[u8], version: u32) -> Result<String> {
    frame_to_json_with(bytes, version)
}

fn frame_to_json_with(bytes: &[u8], version: u32) -> Result<String> {
    // Hellos travel at frame version 1 under every expectation: they
    // decode legacy, the stateless rule judges them, and their rendering
    // names no methods, so the expectation never touches them.
    if let Ok((DecodedFrame::Hello(_), _)) = decode_frame(bytes, MAX_FRAME_BYTES) {
        return frame_to_json(bytes);
    }
    let (decoded, _) = decode_frame_for_version(bytes, MAX_FRAME_BYTES, version)?;
    let frame = match decoded {
        // Unreachable: hello bytes decode legacy above. Fall back to the
        // legacy render rather than panicking if that ever changes.
        DecodedFrame::Hello(_) => return frame_to_json(bytes),
        DecodedFrame::Request(frame) | DecodedFrame::Response(frame) => frame,
    };
    Ok(render(&frame_value_for_version(&frame, version)?))
}

/// Parses the `Frame` object and encodes it with the frozen codec; the
/// returned bytes are the only canonical form.
///
/// # Errors
///
/// Returns `JSON_BRIDGE_RESOURCE_LIMIT`, then the shape and encoding codes,
/// then the codec's `PROTOCOL_*` code verbatim.
pub fn frame_from_json(text: &str) -> Result<EncodedFrame> {
    let frame = frame_from_value(&parse(text)?)?;
    if frame.kind == FrameKind::Hello {
        Ok(encode_hello_frame(&Hello::decode(&frame.body)?)?)
    } else {
        Ok(encode_frame(&frame)?)
    }
}

/// Parses the `Frame` object and encodes it under an explicitly selected
/// protocol version; a hello still encodes version-independently. Additive
/// export for version-aware endpoints; the default above stays frozen
/// version 1.
///
/// # Errors
///
/// Returns `JSON_BRIDGE_RESOURCE_LIMIT`, then the shape and encoding codes,
/// then the codec's `PROTOCOL_*` code verbatim.
pub fn frame_from_json_for_version(text: &str, version: u32) -> Result<EncodedFrame> {
    let frame = frame_from_value_for_version(&parse(text)?, version)?;
    if frame.kind == FrameKind::Hello {
        Ok(encode_hello_frame(&Hello::decode(&frame.body)?)?)
    } else {
        Ok(encode_frame_for_version(&frame, version)?)
    }
}

// ---------------------------------------------------------------------------
// Hello and selected profile
// ---------------------------------------------------------------------------

fn hello_value(hello: &Hello) -> Result<Value> {
    hello_value_with(hello, false)
}

/// Renders the `Hello` object with version 2 method naming: the version 1
/// names plus exactly `entity.version` and `entity.signature`. Every other
/// field renders exactly as the version 1 object.
fn hello_value_versioned(hello: &Hello) -> Result<Value> {
    hello_value_with(hello, true)
}

/// The version 3 feature fields: the frozen four plus exactly
/// `native_tests` (bit 5). The version 1 and 2 objects keep their four
/// fields byte-identical; only the version 3 rendering names the new bit.
const FEATURE_V3_FIELDS: [&str; 5] = [
    "cancel",
    "stream",
    "json_bridge",
    "checksum",
    "native_tests",
];
const FEATURE_V3_MASKS: [u32; 5] = [
    FEATURE_CANCEL,
    FEATURE_STREAM,
    FEATURE_JSON_BRIDGE,
    FEATURE_CHECKSUM,
    FEATURE_NATIVE_TESTS_V1,
];

/// Renders the `Hello` object with version 3 method naming (the version 2
/// names plus the three reserved native rows) and the version 3 feature
/// fields. Every other field renders exactly as the version 1 object.
fn hello_value_v3(hello: &Hello) -> Result<Value> {
    let mut map = Map::new();
    insert(
        &mut map,
        "protocol_versions",
        Value::Array(
            hello
                .protocol_versions
                .iter()
                .map(|v| integer(u64::from(*v)))
                .collect(),
        ),
    );
    insert(
        &mut map,
        "schema_epochs",
        Value::Array(
            hello
                .schema_epochs
                .iter()
                .map(|e| hex(e.as_bytes()))
                .collect(),
        ),
    );
    insert(&mut map, "limits", limits_value(&hello.limits));
    insert(
        &mut map,
        "methods",
        method_names_for_version(&hello.methods, PROTOCOL_VERSION_V3)?,
    );
    insert(
        &mut map,
        "features",
        bits_value(hello.features, &FEATURE_V3_FIELDS, &FEATURE_V3_MASKS)?,
    );
    insert(&mut map, "adapters", hex32_list(&hello.adapters));
    insert(&mut map, "effects", hex32_list(&hello.effects));
    Ok(Value::Object(map))
}

fn hello_value_with(hello: &Hello, versioned: bool) -> Result<Value> {
    let mut map = Map::new();
    insert(
        &mut map,
        "protocol_versions",
        Value::Array(
            hello
                .protocol_versions
                .iter()
                .map(|v| integer(u64::from(*v)))
                .collect(),
        ),
    );
    insert(
        &mut map,
        "schema_epochs",
        Value::Array(
            hello
                .schema_epochs
                .iter()
                .map(|e| hex(e.as_bytes()))
                .collect(),
        ),
    );
    insert(&mut map, "limits", limits_value(&hello.limits));
    if versioned {
        insert(&mut map, "methods", method_names_versioned(&hello.methods)?);
    } else {
        insert(&mut map, "methods", method_names(&hello.methods)?);
    }
    insert(
        &mut map,
        "features",
        bits_value(hello.features, &FEATURE_FIELDS, &FEATURE_MASKS)?,
    );
    insert(&mut map, "adapters", hex32_list(&hello.adapters));
    insert(&mut map, "effects", hex32_list(&hello.effects));
    Ok(Value::Object(map))
}

/// Renders a validated hello as the `Hello` object.
///
/// # Errors
///
/// Returns the codec's validation failure or a bridge naming failure.
pub fn hello_to_json(hello: &Hello) -> Result<String> {
    hello.validate()?;
    Ok(render(&hello_value(hello)?))
}

/// Renders a validated version-aware hello as the `Hello` object, naming
/// the version 2 methods the frozen version 1 rendering rejects.
///
/// # Errors
///
/// Returns the codec's validation failure or a bridge naming failure.
pub fn hello_to_json_versioned(hello: &Hello) -> Result<String> {
    hello.validate()?;
    Ok(render(&hello_value_versioned(hello)?))
}

/// Renders a validated hello as the `Hello` object under an explicitly
/// selected protocol version: version 1 renders frozen, version 2 names
/// the two entity-read methods, version 3 additionally names the three
/// reserved native rows and the `native_tests` feature bit. Additive
/// export for version-aware endpoints; both renderings above are unchanged.
///
/// # Errors
///
/// Returns the codec's validation failure, a bridge naming failure, or
/// `JSON_BRIDGE_SHAPE_INVALID` for a version with no table.
pub fn hello_to_json_for_version(hello: &Hello, version: u32) -> Result<String> {
    hello.validate()?;
    if version == PROTOCOL_VERSION_V3 {
        return Ok(render(&hello_value_v3(hello)?));
    }
    if version == PROTOCOL_VERSION_V2 {
        return Ok(render(&hello_value_versioned(hello)?));
    }
    if version == PROTOCOL_VERSION {
        return Ok(render(&hello_value(hello)?));
    }
    fail(JsonBridgeErrorCode::ShapeInvalid)
}

/// Parses the `Hello` object into a validated hello.
///
/// # Errors
///
/// Returns the bridge codes in precedence, then the codec's validation code.
pub fn hello_from_json(text: &str) -> Result<Hello> {
    let value = parse(text)?;
    let map = object(&value, &HELLO_FIELDS, &[])?;
    let hello = Hello {
        protocol_versions: list_field(&map["protocol_versions"])?
            .iter()
            .map(u32_field)
            .collect::<Result<_>>()?,
        schema_epochs: hex32_list_from_value(&map["schema_epochs"])?
            .into_iter()
            .map(SchemaEpochId::from_bytes)
            .collect(),
        limits: limits_from_value(&map["limits"])?,
        methods: method_tags_from_value(&map["methods"])?,
        features: bits_from_value(&map["features"], &FEATURE_FIELDS, &FEATURE_MASKS)?,
        adapters: hex32_list_from_value(&map["adapters"])?,
        effects: hex32_list_from_value(&map["effects"])?,
    };
    hello.validate()?;
    Ok(hello)
}

/// Renders a selected profile with its transcript-bound handshake identity.
///
/// The identity is opaque render data, never a trust root: JSON is
/// non-canonical, so identity always comes from the wire transcript
/// (contract section 2), never from this text.
///
/// # Errors
///
/// Returns the codec's failure or a bridge naming failure.
pub fn selected_to_json(
    selected: &SelectedProfile,
    handshake: &ProtocolHandshakeId,
) -> Result<String> {
    let mut map = Map::new();
    insert(
        &mut map,
        "protocol_version",
        integer(u64::from(selected.protocol_version)),
    );
    insert(
        &mut map,
        "schema_epoch",
        hex(selected.schema_epoch.as_bytes()),
    );
    insert(&mut map, "limits", limits_value(&selected.limits));
    insert(&mut map, "methods", method_names(&selected.methods)?);
    insert(
        &mut map,
        "features",
        bits_value(selected.features, &FEATURE_FIELDS, &FEATURE_MASKS)?,
    );
    insert(&mut map, "adapters", hex32_list(&selected.adapters));
    insert(&mut map, "effects", hex32_list(&selected.effects));
    insert(&mut map, "handshake_id", hex(handshake.as_bytes()));
    debug_assert_eq!(map.len(), SELECTED_FIELDS.len());
    Ok(render(&Value::Object(map)))
}

// ---------------------------------------------------------------------------
// Failure envelope and stream chunks
// ---------------------------------------------------------------------------

/// Renders a failure envelope with its code and symbol verbatim.
///
/// # Errors
///
/// Returns the codec's `PROTOCOL_PAYLOAD_INVALID` for a symbol the codec
/// would not encode.
pub fn failure_to_json(failure: &ProtocolFailure) -> Result<String> {
    failure.encode()?;
    let mut map = Map::new();
    insert(&mut map, "code", integer(u64::from(failure.code)));
    insert(&mut map, "symbol", Value::String(failure.symbol.clone()));
    insert(&mut map, "phase", integer(u64::from(failure.phase)));
    insert(
        &mut map,
        "retryability",
        Value::String(retryability_name(failure.retryability).to_string()),
    );
    insert(
        &mut map,
        "incident",
        failure.incident.map_or(Value::Null, |digest| hex(&digest)),
    );
    insert(&mut map, "details", hex(&failure.details));
    Ok(render(&Value::Object(map)))
}

/// Parses the `Failure` object.
///
/// # Errors
///
/// Returns the bridge codes in precedence, then the codec's symbol check.
pub fn failure_from_json(text: &str) -> Result<ProtocolFailure> {
    let value = parse(text)?;
    let map = object(&value, &FAILURE_FIELDS, &["incident"])?;
    let failure = ProtocolFailure {
        code: u32_field(&map["code"])?,
        symbol: string_field(&map["symbol"])?.to_string(),
        phase: u32_field(&map["phase"])?,
        retryability: retryability_from_value(&map["retryability"])?,
        incident: match &map["incident"] {
            Value::Null => None,
            other => Some(hex32_field(other)?),
        },
        details: hex_field(&map["details"])?,
    };
    failure.encode()?;
    Ok(failure)
}

/// Renders a stream chunk.
#[must_use]
pub fn chunk_to_json(chunk: &StreamChunk) -> String {
    let mut map = Map::new();
    insert(&mut map, "index", integer(chunk.index));
    insert(&mut map, "total", integer(chunk.total));
    insert(&mut map, "bytes", hex(&chunk.bytes));
    render(&Value::Object(map))
}

/// Parses the `StreamChunk` object.
///
/// # Errors
///
/// Returns the bridge codes in precedence.
pub fn chunk_from_json(text: &str) -> Result<StreamChunk> {
    let value = parse(text)?;
    let map = object(&value, &CHUNK_FIELDS, &[])?;
    Ok(StreamChunk {
        index: u64_field(&map["index"])?,
        total: u64_field(&map["total"])?,
        bytes: hex_field(&map["bytes"])?,
    })
}

#[cfg(test)]
mod tests;

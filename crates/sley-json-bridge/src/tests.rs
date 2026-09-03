use serde_json::Value;
use sley_protocol::{
    BoundedContext, DecodedFrame, FrameKind, Hello, LimitProfile, MAX_FRAME_BYTES, Method,
    ProtocolErrorCode, ProtocolFailure, ProtocolFrame, Retryability, StreamChunk, decode_frame,
    encode_frame, negotiate,
};

use super::*;

const SMP1_FIXTURE: &str = include_str!("../../../conformance/smp1/v1/accepted.json");

fn fixture() -> Value {
    serde_json::from_str(SMP1_FIXTURE).expect("fixture parses")
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
        .collect()
}

/// Every encoded frame of the SMP1 fixture: `(id, bytes)`.
fn fixture_frames() -> Vec<(String, Vec<u8>)> {
    let fixture = fixture();
    let mut frames = Vec::new();
    for frame in fixture["frames"].as_array().expect("frames") {
        frames.push((
            frame["id"].as_str().expect("id").to_string(),
            unhex(frame["frame_hex"].as_str().expect("hex")),
        ));
    }
    for peer in ["client", "server"] {
        frames.push((
            format!("hello-{peer}"),
            unhex(fixture["hellos"][peer]["frame_hex"].as_str().expect("hex")),
        ));
    }
    frames
}

fn fixture_hellos() -> (Hello, Hello) {
    let fixture = fixture();
    let hello = |peer: &str| match decode_frame(
        &unhex(fixture["hellos"][peer]["frame_hex"].as_str().expect("hex")),
        MAX_FRAME_BYTES,
    )
    .expect("decodes")
    .0
    {
        DecodedFrame::Hello(hello) => hello,
        other => panic!("not a hello: {other:?}"),
    };
    (hello("client"), hello("server"))
}

fn request_value() -> Value {
    let (id, bytes) = fixture_frames().into_iter().next().expect("a frame");
    assert_eq!(id, "request");
    serde_json::from_str(&frame_to_json(&bytes).expect("renders")).expect("parses")
}

fn bridge(code: JsonBridgeErrorCode) -> BridgeError {
    BridgeError::Bridge(code)
}

fn protocol(code: ProtocolErrorCode) -> BridgeError {
    BridgeError::Protocol(ProtocolError::new(code))
}

fn with(mut value: Value, path: &[&str], replacement: Value) -> Value {
    let mut slot = &mut value;
    for key in path {
        slot = slot.get_mut(*key).expect("path exists");
    }
    *slot = replacement;
    value
}

fn without(mut value: Value, key: &str) -> Value {
    value.as_object_mut().expect("object").remove(key);
    value
}

fn extra(mut value: Value, key: &str) -> Value {
    value
        .as_object_mut()
        .expect("object")
        .insert(key.to_string(), Value::Bool(true));
    value
}

/// The rejection matrix over the fixture's request frame (contract sections
/// 1, 2, and 5): label, text, expected failure.
#[allow(clippy::too_many_lines)] // one table row per contract case
pub(crate) fn rejections() -> Vec<(&'static str, String, BridgeError)> {
    let base = request_value();
    let hello_base = {
        let (_, bytes) = fixture_frames().into_iter().nth(3).expect("hello frame");
        serde_json::from_str::<Value>(&frame_to_json(&bytes).expect("renders")).expect("parses")
    };
    let text = |value: &Value| value.to_string();
    let shape = bridge(JsonBridgeErrorCode::ShapeInvalid);
    let number = bridge(JsonBridgeErrorCode::NumberInvalid);
    let hex = bridge(JsonBridgeErrorCode::HexInvalid);
    let method = bridge(JsonBridgeErrorCode::MethodUnknown);
    let mut cases = vec![
        ("not-json", "{".to_string(), shape.clone()),
        ("not-an-object", "[]".to_string(), shape.clone()),
        (
            "unknown-field",
            text(&extra(base.clone(), "extra")),
            shape.clone(),
        ),
        (
            "missing-field",
            text(&without(base.clone(), "body")),
            shape.clone(),
        ),
        (
            "null-field",
            text(&with(base.clone(), &["body"], Value::Null)),
            shape.clone(),
        ),
        (
            "wrong-type",
            text(&with(base.clone(), &["request_id"], Value::Bool(true))),
            shape.clone(),
        ),
        (
            "unknown-kind",
            text(&with(base.clone(), &["kind"], Value::from("frame"))),
            shape.clone(),
        ),
        (
            "nested-unknown-field",
            text(&extra(
                with(
                    base.clone(),
                    &["flags"],
                    extra(base["flags"].clone(), "trace"),
                ),
                "",
            )),
            shape.clone(),
        ),
        (
            "number-fraction",
            text(&with(base.clone(), &["request_id"], Value::from(1.5))),
            number.clone(),
        ),
        (
            "number-negative",
            text(&with(base.clone(), &["request_id"], Value::from(-1))),
            number.clone(),
        ),
        (
            "number-above-2-pow-53",
            text(&with(
                base.clone(),
                &["request_id"],
                Value::from(MAX_JSON_NUMBER + 1),
            )),
            number.clone(),
        ),
        (
            "string-leading-zero",
            text(&with(base.clone(), &["request_id"], Value::from("01"))),
            number.clone(),
        ),
        (
            "string-signed",
            text(&with(base.clone(), &["request_id"], Value::from("+1"))),
            number.clone(),
        ),
        (
            "string-empty",
            text(&with(base.clone(), &["request_id"], Value::from(""))),
            number.clone(),
        ),
        (
            "string-above-u64",
            text(&with(
                base.clone(),
                &["request_id"],
                Value::from("18446744073709551616"),
            )),
            number.clone(),
        ),
        (
            "u32-overflow",
            text(&with(
                base.clone(),
                &["protocol_version"],
                Value::from(4_294_967_296_u64),
            )),
            number.clone(),
        ),
        (
            "hex-uppercase",
            text(&with(base.clone(), &["body"], Value::from("AB"))),
            hex.clone(),
        ),
        (
            "hex-odd-length",
            text(&with(base.clone(), &["body"], Value::from("abc"))),
            hex.clone(),
        ),
        (
            "hex-non-hex",
            text(&with(base.clone(), &["body"], Value::from("zz"))),
            hex.clone(),
        ),
        (
            "hex-wrong-fixed-length",
            text(&with(base.clone(), &["session"], Value::from("abcd"))),
            shape.clone(),
        ),
        (
            "method-unknown",
            text(&with(
                base.clone(),
                &["method"],
                Value::from("query.everything"),
            )),
            method.clone(),
        ),
        (
            "method-empty",
            text(&with(base.clone(), &["method"], Value::from(""))),
            method,
        ),
        (
            "shape-before-number",
            text(&extra(
                with(base.clone(), &["request_id"], Value::from(-1)),
                "extra",
            )),
            shape.clone(),
        ),
        (
            "number-before-hex-in-field-order",
            text(&with(
                with(base.clone(), &["request_id"], Value::from(-1)),
                &["body"],
                Value::from("zz"),
            )),
            number,
        ),
        (
            "hex-before-method-in-field-order",
            text(&with(
                with(base.clone(), &["session"], Value::from("zz")),
                &["method"],
                Value::from("nope"),
            )),
            hex,
        ),
        (
            "hello-with-method",
            text(&with(
                hello_base.clone(),
                &["method"],
                Value::from("cancel"),
            )),
            shape.clone(),
        ),
        (
            "hello-with-session",
            text(&with(
                hello_base.clone(),
                &["session"],
                base["session"].clone(),
            )),
            shape.clone(),
        ),
        (
            "hello-with-request-id",
            text(&with(hello_base.clone(), &["request_id"], Value::from(1))),
            shape,
        ),
        (
            "protocol-version-unsupported",
            text(&with(base.clone(), &["protocol_version"], Value::from(2))),
            protocol(ProtocolErrorCode::VersionUnsupported),
        ),
        (
            "hello-body-invalid",
            text(&with(hello_base, &["body"], Value::from("00"))),
            protocol(ProtocolErrorCode::PayloadInvalid),
        ),
    ];
    let deep = format!(
        "{}{}",
        "[".repeat(MAX_JSON_DEPTH + 1),
        "]".repeat(MAX_JSON_DEPTH + 1)
    );
    cases.push((
        "depth-above-ceiling",
        deep,
        bridge(JsonBridgeErrorCode::ResourceLimit),
    ));
    cases
}

#[test]
fn the_embedded_method_table_equals_the_frozen_method_table() {
    let table = method_table().expect("table parses");
    assert_eq!(table.len(), Method::ALL.len());
    assert_eq!(table.len(), 41);
    let families = [
        "",
        "session",
        "repository",
        "query",
        "candidate",
        "transaction",
        "runtime",
    ];
    for (entry, method) in table.iter().zip(Method::ALL) {
        assert_eq!(entry.tag, method.tag(), "{}", entry.name);
        assert_eq!(entry.name, method.name());
        assert_eq!(entry.family, families[method.family() as usize]);
        assert_eq!(entry.reserved, method.is_reserved(), "{}", entry.name);
        assert_eq!(method_by_name(&entry.name), Some(method));
    }
    assert_eq!(table.iter().filter(|entry| entry.reserved).count(), 4);
    let header: Value = serde_json::from_str(METHOD_TABLE_JSON).expect("table");
    assert_eq!(header["method_count"], 41);
    assert_eq!(header["reserved_count"], 4);
    assert_eq!(header["source"], "docs/spec/SMP1.md");
    assert_eq!(method_by_name("query.everything"), None);
}

#[test]
fn every_smp1_fixture_frame_round_trips_through_json_to_identical_bytes() {
    let frames = fixture_frames();
    assert_eq!(frames.len(), 5);
    for (id, bytes) in &frames {
        let text = frame_to_json(bytes).expect("renders");
        let value: Value = serde_json::from_str(&text).expect("parses");
        assert_eq!(
            value.as_object().expect("object").len(),
            FRAME_FIELDS.len(),
            "{id}"
        );
        let encoded = frame_from_json(&text).expect("re-encodes");
        assert_eq!(&encoded.bytes, bytes, "{id}");
        assert_eq!(
            text,
            frame_to_json(&encoded.bytes).expect("renders again"),
            "{id}"
        );
        if id.starts_with("hello") {
            assert_eq!(value["kind"], "hello");
            assert_eq!(value["method"], "");
            assert_eq!(value["session"], Value::Null);
            assert_eq!(encoded.kind, FrameKind::Hello);
        } else {
            assert_ne!(value["method"], "");
            assert!(value["session"].is_string());
        }
    }
    // Whitespace and field order are the reader's to ignore.
    let text = frame_to_json(&frames[0].1).expect("renders");
    let value: Value = serde_json::from_str(&text).expect("parses");
    let pretty = serde_json::to_string_pretty(&value).expect("pretty");
    assert_ne!(pretty, text);
    assert_eq!(
        frame_from_json(&pretty).expect("re-encodes").bytes,
        frames[0].1
    );
}

#[test]
fn integers_follow_the_declared_encoding_on_both_sides() {
    let base = request_value();
    let at = |value: u64| with(base.clone(), &["request_id"], integer(value));
    assert_eq!(integer(MAX_JSON_NUMBER), Value::from(MAX_JSON_NUMBER));
    assert_eq!(
        integer(MAX_JSON_NUMBER + 1),
        Value::from("9007199254740992")
    );
    assert_eq!(integer(u64::MAX), Value::from("18446744073709551615"));
    for value in [0, 1, MAX_JSON_NUMBER, MAX_JSON_NUMBER + 1, u64::MAX] {
        let encoded = frame_from_json(&at(value).to_string()).expect("encodes");
        let (decoded, _) = decode_frame(&encoded.bytes, MAX_FRAME_BYTES).expect("decodes");
        let DecodedFrame::Request(frame) = decoded else {
            panic!("request expected");
        };
        assert_eq!(frame.request_id, value);
        let text = frame_to_json(&encoded.bytes).expect("renders");
        let rendered: Value = serde_json::from_str(&text).expect("parses");
        assert_eq!(rendered["request_id"], integer(value));
    }
    // A reader accepts the string form for a small value too.
    let small = with(base, &["request_id"], Value::from("7"));
    let encoded = frame_from_json(&small.to_string()).expect("encodes");
    let (decoded, _) = decode_frame(&encoded.bytes, MAX_FRAME_BYTES).expect("decodes");
    assert!(matches!(decoded, DecodedFrame::Request(frame) if frame.request_id == 7));
}

#[test]
fn the_rejection_matrix_reports_the_contract_codes_in_precedence() {
    for (label, text, expected) in rejections() {
        assert_eq!(
            frame_from_json(&text).expect_err(label),
            expected,
            "{label}"
        );
    }
    let huge = " ".repeat(MAX_JSON_TEXT_BYTES + 1);
    assert_eq!(
        frame_from_json(&huge),
        Err(bridge(JsonBridgeErrorCode::ResourceLimit))
    );
    assert_eq!(check_resources(&" ".repeat(MAX_JSON_TEXT_BYTES)), Ok(()));
    let nested = format!(
        "{}{}",
        "[".repeat(MAX_JSON_DEPTH),
        "]".repeat(MAX_JSON_DEPTH)
    );
    assert_eq!(check_resources(&nested), Ok(()));
    assert_eq!(
        frame_from_json(&nested),
        Err(bridge(JsonBridgeErrorCode::ShapeInvalid))
    );
    // Brackets inside strings do not count toward depth.
    let quoted = format!("\"{}\"", "[".repeat(MAX_JSON_DEPTH * 2));
    assert_eq!(check_resources(&quoted), Ok(()));
    for code in JsonBridgeErrorCode::ALL {
        let position = JsonBridgeErrorCode::ALL
            .iter()
            .position(|c| *c == code)
            .expect("listed");
        assert_eq!(
            code.numeric() - 42_000,
            u32::try_from(position).expect("small")
        );
        assert!(code.as_str().starts_with("JSON_BRIDGE_"));
        assert_eq!(bridge(code).symbol(), code.as_str());
        assert_eq!(bridge(code).numeric(), code.numeric());
    }
    let carried = protocol(ProtocolErrorCode::FrameInvalid);
    assert_eq!(carried.numeric(), 40_001);
    assert_eq!(carried.symbol(), "PROTOCOL_FRAME_INVALID");
    assert_eq!(carried.to_string(), "PROTOCOL_FRAME_INVALID");
}

#[test]
fn unknown_methods_and_flags_are_named_never_invented() {
    let (id, bytes) = fixture_frames().into_iter().next().expect("a frame");
    assert_eq!(id, "request");
    let DecodedFrame::Request(frame) = decode_frame(&bytes, MAX_FRAME_BYTES).expect("decodes").0
    else {
        panic!("request expected");
    };
    let foreign = ProtocolFrame {
        method: 999,
        ..frame.clone()
    };
    let encoded = encode_frame(&foreign).expect("the codec does not judge the method tag");
    assert_eq!(
        frame_to_json(&encoded.bytes),
        Err(bridge(JsonBridgeErrorCode::MethodUnknown))
    );
    assert_eq!(
        frame_value(&foreign),
        Err(bridge(JsonBridgeErrorCode::MethodUnknown))
    );
    let unnamed_flag = ProtocolFrame { flags: 4, ..frame };
    assert_eq!(
        frame_value(&unnamed_flag),
        Err(bridge(JsonBridgeErrorCode::ShapeInvalid))
    );
    assert_eq!(
        encode_frame(&unnamed_flag)
            .expect_err("codec rejects")
            .code(),
        ProtocolErrorCode::FrameInvalid
    );
    for method in Method::ALL {
        let value = with(request_value(), &["method"], Value::from(method.name()));
        let encoded = frame_from_json(&value.to_string()).expect("every frozen name encodes");
        let DecodedFrame::Request(decoded) = decode_frame(&encoded.bytes, MAX_FRAME_BYTES)
            .expect("decodes")
            .0
        else {
            panic!("request expected");
        };
        assert_eq!(decoded.method, method.tag());
    }
}

#[test]
fn equal_values_render_identical_lexicographic_text() {
    let (_, bytes) = fixture_frames().into_iter().next().expect("a frame");
    let first = frame_to_json(&bytes).expect("renders");
    for _ in 0..128 {
        assert_eq!(frame_to_json(&bytes).expect("renders"), first);
    }
    assert!(!first.contains(' '), "no insignificant whitespace");
    let value: Value = serde_json::from_str(&first).expect("parses");
    let keys: Vec<&String> = value.as_object().expect("object").keys().collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
    let positions: Vec<usize> = FRAME_FIELDS
        .iter()
        .map(|field| first.find(&format!("\"{field}\":")).expect("field present"))
        .collect();
    let mut by_name: Vec<(&str, usize)> = FRAME_FIELDS.iter().copied().zip(positions).collect();
    by_name.sort_by_key(|(name, _)| *name);
    assert!(by_name.windows(2).all(|pair| pair[0].1 < pair[1].1));
}

#[test]
fn hello_selected_failure_and_chunk_round_trip() {
    let (client, server) = fixture_hellos();
    for hello in [&client, &server] {
        let text = hello_to_json(hello).expect("renders");
        assert_eq!(&hello_from_json(&text).expect("parses"), hello);
        let value: Value = serde_json::from_str(&text).expect("parses");
        assert_eq!(value.as_object().expect("object").len(), HELLO_FIELDS.len());
        assert!(value["methods"][0].is_string());
        assert_eq!(
            value["features"].as_object().expect("features").len(),
            FEATURE_FIELDS.len()
        );
    }
    let selected = negotiate(&client, &server).expect("negotiates");
    let text = selected_to_json(&selected).expect("renders");
    let value: Value = serde_json::from_str(&text).expect("parses");
    assert_eq!(
        value.as_object().expect("object").len(),
        SELECTED_FIELDS.len()
    );
    let expected = fixture()["selected"]["handshake_id"]
        .as_str()
        .expect("id")
        .to_string();
    assert_eq!(value["handshake_id"], Value::from(expected));
    assert_eq!(
        value["protocol_version"],
        Value::from(selected.protocol_version)
    );

    let unknown_method = with(
        serde_json::from_str(&hello_to_json(&client).expect("renders")).expect("parses"),
        &["methods"],
        Value::Array(vec![Value::from("nope")]),
    );
    assert_eq!(
        hello_from_json(&unknown_method.to_string()),
        Err(bridge(JsonBridgeErrorCode::MethodUnknown))
    );
    let empty_versions = with(
        serde_json::from_str(&hello_to_json(&client).expect("renders")).expect("parses"),
        &["protocol_versions"],
        Value::Array(vec![]),
    );
    assert_eq!(
        hello_from_json(&empty_versions.to_string()),
        Err(protocol(ProtocolErrorCode::PayloadInvalid))
    );
}

#[test]
fn failure_chunk_and_bounds_round_trip() {
    let failures = [
        ProtocolFailure::protocol(ProtocolErrorCode::LimitExceeded),
        ProtocolFailure {
            code: 31_004,
            symbol: "QUERY_CURSOR_INVALID".to_string(),
            phase: 3,
            retryability: Retryability::AfterRequery,
            incident: Some([0xab; 32]),
            details: vec![1, 2, 3],
        },
    ];
    for failure in &failures {
        let text = failure_to_json(failure).expect("renders");
        assert_eq!(&failure_from_json(&text).expect("parses"), failure);
        let value: Value = serde_json::from_str(&text).expect("parses");
        assert_eq!(value["code"], Value::from(failure.code));
        assert_eq!(value["symbol"], Value::from(failure.symbol.clone()));
    }
    let lowercase = ProtocolFailure {
        symbol: "lower".to_string(),
        ..failures[0].clone()
    };
    assert_eq!(
        failure_to_json(&lowercase),
        Err(protocol(ProtocolErrorCode::PayloadInvalid))
    );
    let bad_retry = with(
        serde_json::from_str(&failure_to_json(&failures[1]).expect("renders")).expect("parses"),
        &["retryability"],
        Value::from("later"),
    );
    assert_eq!(
        failure_from_json(&bad_retry.to_string()),
        Err(bridge(JsonBridgeErrorCode::ShapeInvalid))
    );

    let chunk = StreamChunk {
        index: 3,
        total: MAX_JSON_NUMBER + 5,
        bytes: vec![0xff; 70],
    };
    let text = chunk_to_json(&chunk);
    assert_eq!(chunk_from_json(&text).expect("parses"), chunk);
    assert!(text.contains("\"total\":\"9007199254740996\""));
    assert_eq!(
        chunk_from_json("{\"index\":0,\"total\":1}"),
        Err(bridge(JsonBridgeErrorCode::ShapeInvalid))
    );

    let none = BoundedContext::none();
    let limits = LimitProfile {
        max_frame_bytes: 1,
        ..none.applied_limits
    };
    let text = bounds_value(&BoundedContext {
        applied_limits: limits,
        ..none
    })
    .to_string();
    let parsed: Value = serde_json::from_str(&text).expect("parses");
    assert_eq!(
        bounds_from_value(&parsed)
            .expect("parses")
            .applied_limits
            .max_frame_bytes,
        1
    );
}

/// Prints the fixture vectors for `scripts/generate_smp1_json_bridge_fixtures.py`.
#[test]
#[ignore = "fixture refresh emitter"]
fn emit_smp1_json_bridge_vectors_for_fixture_refresh() {
    let hexify = |text: &str| hex(text.as_bytes()).as_str().expect("string").to_string();
    for (id, bytes) in fixture_frames() {
        let text = frame_to_json(&bytes).expect("renders");
        assert_eq!(frame_from_json(&text).expect("re-encodes").bytes, bytes);
        println!(
            "SMP1_JSON_BRIDGE_VECTOR|{id}|{}|{}",
            hex(&bytes).as_str().expect("string"),
            hexify(&text)
        );
    }
    for (label, text, expected) in rejections() {
        println!(
            "SMP1_JSON_BRIDGE_REJECT|{label}|{}|{}",
            hexify(&text),
            expected.symbol()
        );
    }
}

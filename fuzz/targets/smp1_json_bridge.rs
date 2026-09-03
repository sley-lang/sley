#![allow(unsafe_code)]
#![no_main]

//! S20-700 scoped persistent surface: the S20-420 SMP1 JSON bridge
//! (`sley_json_bridge::frame_from_json`, `frame_to_json`, and the hello,
//! failure, and chunk readers).
//!
//! The first byte selects the lane; the remainder is the candidate.
//! Lane 0 reads the bytes as JSON text and parses a `Frame`; an accepted
//! text must encode to bytes that render to text which parses back to the
//! identical bytes and identity. Lane 1 reads the bytes as an encoded SMP1
//! frame and renders it; an accepted frame must parse back to the identical
//! bytes. Lane 2 reads the bytes as `Hello`, `Failure`, and `StreamChunk`
//! text and requires every accepted value to render and parse back to
//! itself. Every rejection must carry a frozen `JSON_BRIDGE_*` or
//! `PROTOCOL_*` code.

use core::slice;

use sley_json_bridge::{
    BridgeError, JsonBridgeErrorCode, chunk_from_json, chunk_to_json, failure_from_json,
    failure_to_json, frame_from_json, frame_to_json, hello_from_json, hello_to_json,
};
use sley_protocol::ProtocolErrorCode;

const SELECTOR_COUNT: u8 = 3;
const MAX_FUZZ_INPUT_BYTES: usize = 65_536;

#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn LLVMFuzzerTestOneInput(data: *const u8, len: usize) -> i32 {
    if len == 0 || len > MAX_FUZZ_INPUT_BYTES + 1 {
        return 0;
    }
    let input = unsafe { slice::from_raw_parts(data, len) };
    fuzz_one(input);
    0
}

fn fuzz_one(input: &[u8]) {
    let selector = input[0] % SELECTOR_COUNT;
    let payload = &input[1..];
    match selector {
        0 => check_text(payload),
        1 => check_frame(payload),
        _ => check_records(payload),
    }
}

fn known(error: &BridgeError) {
    match error {
        BridgeError::Bridge(code) => assert!(JsonBridgeErrorCode::ALL.contains(code)),
        BridgeError::Protocol(error) => {
            assert!(ProtocolErrorCode::ALL.contains(&error.code()));
        }
    }
}

fn check_text(candidate: &[u8]) {
    let Ok(text) = core::str::from_utf8(candidate) else {
        return;
    };
    match frame_from_json(text) {
        Ok(encoded) => {
            let rendered = frame_to_json(&encoded.bytes).expect("an encoded frame must render");
            let again = frame_from_json(&rendered).expect("rendered text must parse");
            assert_eq!(again.bytes, encoded.bytes, "text round trip drifted");
            assert_eq!(again.frame_id, encoded.frame_id, "frame identity drifted");
            assert_eq!(
                frame_to_json(&again.bytes).expect("renders again"),
                rendered,
                "rendering drifted"
            );
        }
        Err(error) => known(&error),
    }
}

fn check_frame(candidate: &[u8]) {
    match frame_to_json(candidate) {
        Ok(text) => {
            let encoded = frame_from_json(&text).expect("rendered text must parse");
            assert_eq!(encoded.bytes, candidate, "frame round trip drifted");
        }
        Err(error) => known(&error),
    }
}

fn check_records(candidate: &[u8]) {
    let Ok(text) = core::str::from_utf8(candidate) else {
        return;
    };
    match hello_from_json(text) {
        Ok(hello) => {
            let rendered = hello_to_json(&hello).expect("a parsed hello must render");
            assert_eq!(
                hello_from_json(&rendered).expect("rendered hello parses"),
                hello,
                "hello round trip drifted"
            );
        }
        Err(error) => known(&error),
    }
    match failure_from_json(text) {
        Ok(failure) => {
            let rendered = failure_to_json(&failure).expect("a parsed failure must render");
            assert_eq!(
                failure_from_json(&rendered).expect("rendered failure parses"),
                failure,
                "failure round trip drifted"
            );
        }
        Err(error) => known(&error),
    }
    match chunk_from_json(text) {
        Ok(chunk) => {
            let rendered = chunk_to_json(&chunk);
            assert_eq!(
                chunk_from_json(&rendered).expect("rendered chunk parses"),
                chunk,
                "chunk round trip drifted"
            );
        }
        Err(error) => known(&error),
    }
}

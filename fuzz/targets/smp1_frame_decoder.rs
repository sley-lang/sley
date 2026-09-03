#![allow(unsafe_code)]
#![no_main]

//! S20-700 scoped persistent surface: the S20-410 SMP1 frame decoder,
//! hello decoder, and negotiation (`sley_protocol::decode_frame`,
//! `Hello::decode`, `negotiate`).
//!
//! The first byte selects the lane; the remainder is the candidate.
//! Lane 0 decodes the bytes as a complete frame under the ceiling. Lane 1
//! rewrites only the final `ProtocolFrameId` trailer so mutations reach the
//! record rules instead of stopping at the digest. Lane 2 decodes the bytes
//! as a bare hello record and negotiates it against a fixed server hello.

use core::slice;

use sley_id::{ProtocolFrameId, SchemaEpochId};
use sley_protocol::{
    DecodedFrame, FrameKind, Hello, LimitProfile, MAX_FRAME_BYTES, Method, ProtocolErrorCode,
    decode_frame, encode_frame, encode_hello_frame, negotiate,
};

const SELECTOR_COUNT: u8 = 3;
const MAX_FUZZ_INPUT_BYTES: usize = 65_536;
const TRAILER_BYTES: usize = 32;
const LENGTH_PREFIX: usize = 8;

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

fn server_hello() -> Hello {
    Hello {
        protocol_versions: vec![1],
        schema_epochs: vec![
            SchemaEpochId::from_bytes([0x12; 32]),
            SchemaEpochId::from_bytes([0x11; 32]),
        ],
        limits: LimitProfile {
            max_frame_bytes: 4_194_304,
            max_entities: 500,
            max_edges: 20_000,
            max_depth: 8,
            max_response_bytes: 2_097_152,
            max_work: 5_000_000,
            max_inflight: 2,
        },
        methods: Method::ALL.iter().map(|method| method.tag()).collect(),
        features: 1,
        adapters: vec![[0xA2; 32]],
        effects: vec![],
    }
}

fn fuzz_one(input: &[u8]) {
    let selector = input[0] % SELECTOR_COUNT;
    let payload = &input[1..];
    match selector {
        0 => check_frame(payload),
        1 => check_frame(&with_rehashed_trailer(payload)),
        _ => check_hello(payload),
    }
}

fn check_frame(candidate: &[u8]) {
    match decode_frame(candidate, MAX_FRAME_BYTES) {
        Ok((decoded, frame_id)) => {
            let preimage_len = candidate.len() - TRAILER_BYTES;
            assert_eq!(
                frame_id,
                ProtocolFrameId::derive(&candidate[LENGTH_PREFIX..preimage_len]),
                "decoded frame identity is not the trailer's derivation"
            );
            let encoded = match &decoded {
                DecodedFrame::Hello(hello) => encode_hello_frame(hello),
                DecodedFrame::Request(frame) | DecodedFrame::Response(frame) => {
                    assert_ne!(frame.kind, FrameKind::Hello);
                    encode_frame(frame)
                }
            }
            .expect("a decoded frame must re-encode");
            assert_eq!(encoded.bytes, candidate, "re-encoding a decoded frame drifted");
            let again = decode_frame(candidate, MAX_FRAME_BYTES).expect("repeatable decode");
            assert_eq!(again.0, decoded, "decoding a frame twice drifted");
        }
        Err(error) => {
            assert!(ProtocolErrorCode::ALL.contains(&error.code()));
        }
    }
}

fn check_hello(candidate: &[u8]) {
    match Hello::decode(candidate) {
        Ok(hello) => {
            assert_eq!(hello.encode().expect("re-encode"), candidate, "hello re-encoding drifted");
            let server = server_hello();
            match negotiate(&hello, &server) {
                Ok(selected) => {
                    let id = selected.handshake_id().expect("handshake identity");
                    assert_eq!(
                        negotiate(&hello, &server).expect("repeatable").handshake_id().expect("id"),
                        id,
                        "handshake identity drifted"
                    );
                    assert!(hello.protocol_versions.contains(&selected.protocol_version));
                    assert!(selected.methods.iter().all(|m| hello.methods.binary_search(m).is_ok()));
                    assert_eq!(selected.features & !hello.features, 0);
                    assert!(selected.limits.max_inflight <= hello.limits.max_inflight);
                }
                Err(error) => assert_eq!(error.code(), ProtocolErrorCode::NoCommonProfile),
            }
        }
        Err(error) => {
            assert!(matches!(
                error.code(),
                ProtocolErrorCode::PayloadInvalid | ProtocolErrorCode::LimitExceeded
            ));
        }
    }
}

fn with_rehashed_trailer(payload: &[u8]) -> Vec<u8> {
    if payload.len() < LENGTH_PREFIX + TRAILER_BYTES {
        return payload.to_vec();
    }
    let preimage_len = payload.len() - TRAILER_BYTES;
    let mut candidate = payload[..preimage_len].to_vec();
    candidate.extend_from_slice(ProtocolFrameId::derive(&payload[LENGTH_PREFIX..preimage_len]).as_bytes());
    // Keep the length prefix consistent with the rewritten envelope.
    let envelope_len = (candidate.len() - LENGTH_PREFIX) as u64;
    candidate[..LENGTH_PREFIX].copy_from_slice(&envelope_len.to_be_bytes());
    candidate
}

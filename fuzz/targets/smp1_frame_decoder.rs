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
//! Lane 3 decodes the bytes as a stream chunk record, wraps it as an event
//! frame with a final response, and reassembles; when the bytes are not a
//! chunk they are a body split under a small ceiling and reassembled.

use core::slice;

use sley_id::{ProtocolFrameId, SchemaEpochId};
use sley_protocol::{
    BoundedContext, DecodedFrame, FLAG_STREAM, FrameKind, Hello, LimitProfile, MAX_FRAME_BYTES,
    Method, PROTOCOL_VERSION, ProtocolErrorCode, ProtocolFrame, StreamChunk, decode_frame,
    encode_frame, encode_hello_frame, negotiate_identity, reassemble_stream, stream_response,
};

const SELECTOR_COUNT: u8 = 4;
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
            max_sessions: 8,
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
        2 => check_hello(payload),
        _ => check_stream(payload),
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
                DecodedFrame::Hello(hello) => {
                    check_negotiated_hello(hello);
                    encode_hello_frame(hello)
                }
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
            check_negotiated_hello(&hello);
        }
        Err(error) => {
            assert!(matches!(
                error.code(),
                ProtocolErrorCode::PayloadInvalid | ProtocolErrorCode::LimitExceeded
            ));
        }
    }
}

/// Negotiation oracle shared by the bare-record lane (check_hello) and the
/// framed lane (check_frame): fixture hello frames decode to Hello records
/// through decode_frame, so this is where negotiation genuinely executes.
/// A decoded hello that fails validation negotiates to PayloadInvalid
/// (Hello::validate), not NoCommonProfile; the doc sentence in negotiate()
/// claiming otherwise is an owner-lane divergence (S20-410 packet), so the
/// oracle accepts both and the packet decides the contract.
fn check_negotiated_hello(hello: &Hello) {
    let server = server_hello();
    match negotiate_identity(hello, &server) {
        Ok((selected, id)) => {
            assert_eq!(
                negotiate_identity(hello, &server).expect("repeatable").1,
                id,
                "handshake identity drifted"
            );
            assert!(hello.protocol_versions.contains(&selected.protocol_version));
            assert!(selected.methods.iter().all(|m| hello.methods.binary_search(m).is_ok()));
            assert_eq!(selected.features & !hello.features, 0);
            assert!(selected.limits.max_inflight <= hello.limits.max_inflight);
        }
        Err(error) => assert!(
            matches!(
                error.code(),
                ProtocolErrorCode::NoCommonProfile | ProtocolErrorCode::PayloadInvalid
            ),
            "negotiation failed with an unexpected code"
        ),
    }
}

fn check_stream(candidate: &[u8]) {
    if let Ok(chunk) = StreamChunk::decode(candidate) {
        assert_eq!(chunk.encode().expect("re-encode"), candidate, "chunk re-encoding drifted");
        let response = ProtocolFrame {
            protocol_version: PROTOCOL_VERSION,
            session: None,
            request_id: 1,
            kind: FrameKind::Response,
            method: 502,
            flags: FLAG_STREAM,
            bounds: BoundedContext::none(),
            body: Vec::new(),
        };
        let event = ProtocolFrame {
            kind: FrameKind::Event,
            body: candidate.to_vec(),
            ..response.clone()
        };
        match reassemble_stream(&[event, response]) {
            Ok(frame) => {
                assert_eq!(chunk.index, 0);
                assert_eq!(chunk.total, 1);
                assert_eq!(frame.body, chunk.bytes);
            }
            Err(error) => assert_eq!(error.code(), ProtocolErrorCode::FrameInvalid),
        }
        return;
    }
    let response = ProtocolFrame {
        protocol_version: PROTOCOL_VERSION,
        session: None,
        request_id: 1,
        kind: FrameKind::Response,
        method: 502,
        flags: 0,
        bounds: BoundedContext::none(),
        body: candidate.to_vec(),
    };
    let ceiling = 640 + u64::from(candidate.first().copied().unwrap_or(0)) * 8;
    match stream_response(&response, ceiling, true) {
        Ok(frames) => {
            let decoded: Vec<ProtocolFrame> = frames
                .iter()
                .map(|frame| match decode_frame(&frame.bytes, ceiling).expect("stream frames decode").0 {
                    DecodedFrame::Response(frame) => frame,
                    DecodedFrame::Request(_) | DecodedFrame::Hello(_) => panic!("stream frame kind"),
                })
                .collect();
            if decoded.len() > 1 {
                let reassembled = reassemble_stream(&decoded).expect("stream reassembles");
                assert_eq!(reassembled.body, candidate, "streamed body drifted");
            }
        }
        Err(error) => assert_eq!(error.code(), ProtocolErrorCode::LimitExceeded),
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

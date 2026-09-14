#![allow(unsafe_code)]
#![no_main]

//! S20-700 adjacent persistent surface: the S20-510 semantic-delta decoder
//! (`sley_repo::decode_semantic_delta`). Lane 0 feeds raw bytes; lane 1
//! rewrites the trailing `SemanticDeltaId` so mutations reach the payload
//! rules instead of stopping at the digest.

use core::slice;

use sley_id::SemanticDeltaId;
use sley_repo::{CompareErrorCode, decode_semantic_delta, encode_semantic_delta};

const DIGEST_TRAILER_BYTES: usize = 32;
const MAX_FUZZ_INPUT_BYTES: usize = 65_536;
const SELECTOR_COUNT: u8 = 2;

#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn LLVMFuzzerTestOneInput(data: *const u8, len: usize) -> i32 {
    if len == 0 {
        return 0;
    }
    let input = unsafe { slice::from_raw_parts(data, len) };
    fuzz_one(input);
    0
}

fn fuzz_one(input: &[u8]) {
    let Some((&selector, payload)) = input.split_first() else {
        return;
    };
    if payload.len() > MAX_FUZZ_INPUT_BYTES {
        return;
    }
    let rewritten;
    let candidate = if selector % SELECTOR_COUNT == 1 && payload.len() >= DIGEST_TRAILER_BYTES {
        rewritten = with_rehashed_trailer(payload);
        rewritten.as_slice()
    } else {
        payload
    };
    match decode_semantic_delta(candidate) {
        Ok(stored) => {
            assert_eq!(
                stored.stored_bytes, candidate,
                "an accepted delta must round-trip byte for byte"
            );
            let preimage_len = candidate
                .len()
                .checked_sub(DIGEST_TRAILER_BYTES)
                .expect("an accepted delta carries its trailer");
            assert_eq!(
                stored.delta_id,
                SemanticDeltaId::derive(&candidate[..preimage_len]),
                "accepted delta identity drifted"
            );
            let again = encode_semantic_delta(&stored.delta)
                .expect("a decoded delta must re-encode");
            assert_eq!(again, stored, "re-encoding a decoded delta drifted");
        }
        Err(error) => {
            // The decoder emits only its own partition: the judgment and
            // precondition codes (workspace, epoch, root, inventory) never
            // arise from bytes alone.
            assert!(
                matches!(
                    error.code(),
                    CompareErrorCode::VersionUnsupported
                        | CompareErrorCode::DigestMismatch
                        | CompareErrorCode::CanonicalOrder
                        | CompareErrorCode::DuplicateEntry
                        | CompareErrorCode::FormatInvalid
                        | CompareErrorCode::ResourceLimit
                        | CompareErrorCode::InternalInvariant
                ),
                "decoder emitted a judgment-only failure code"
            );
        }
    }
}

fn with_rehashed_trailer(input: &[u8]) -> Vec<u8> {
    let preimage_len = input.len() - DIGEST_TRAILER_BYTES;
    let delta_id = SemanticDeltaId::derive(&input[..preimage_len]);
    let mut rewritten = input.to_vec();
    rewritten[preimage_len..].copy_from_slice(delta_id.as_bytes());
    rewritten
}

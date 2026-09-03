#![allow(unsafe_code)]
#![no_main]

//! S20-700 scoped persistent surface: the S20-300 full complete-root index
//! snapshot decoder (`sley_query::decode_complete_root_snapshot`, arm `2`).
//!
//! The first byte selects the lane; the remainder is the candidate record.
//! Lane 0 decodes the bytes as given. Lane 1 rewrites only the final
//! `IndexSnapshotId` trailer so mutations reach the structural rules instead
//! of stopping at the digest. Both lanes read the expected context from the
//! candidate's own header (schema epoch at bytes 16 through 47, the bound
//! root after option tag `2` at bytes 84 through 87), so every context
//! failure is a real mismatch and not a harness artefact.

use core::slice;

use sley_id::{IndexSnapshotId, SchemaEpochId, StateRoot};
use sley_query::{IndexCompleteness, SnapshotContext, decode_complete_root_snapshot};

const SELECTOR_COUNT: u8 = 2;
const MAX_FUZZ_INPUT_BYTES: usize = 65_536;
const TRAILER_BYTES: usize = 32;
const EPOCH_OFFSET: usize = 16;
const OPTION_OFFSET: usize = 84;
const ROOT_OFFSET: usize = 88;
const OPTION_SOME: [u8; 4] = [0, 0, 0, 2];
const MIN_NUMERIC_CODE: u32 = 30_000;
const MAX_NUMERIC_CODE: u32 = 30_010;

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
    let candidate = match selector {
        0 => payload.to_vec(),
        _ => with_rehashed_trailer(payload),
    };
    let context = context_from_header(&candidate);
    match decode_complete_root_snapshot(context, &candidate) {
        Ok(snapshot) => {
            assert_eq!(snapshot.record(), &candidate[..], "decoded record drifted from its bytes");
            assert_eq!(snapshot.completeness(), IndexCompleteness::CompleteRoot);
            assert_eq!(snapshot.context(), context);
            assert!(
                snapshot.context().claimed_root_context.is_some(),
                "an arm-2 snapshot decoded without a bound root"
            );
            let preimage_len = candidate.len() - TRAILER_BYTES;
            assert_eq!(
                snapshot.snapshot_id(),
                IndexSnapshotId::derive(&candidate[..preimage_len]),
                "decoded identity is not the trailer's derivation"
            );
            let again = decode_complete_root_snapshot(context, &candidate)
                .expect("a decodable snapshot must decode again");
            assert_eq!(again, snapshot, "decoding a snapshot twice drifted");
        }
        Err(error) => {
            let numeric = error.code().numeric();
            assert!(
                (MIN_NUMERIC_CODE..=MAX_NUMERIC_CODE).contains(&numeric),
                "unknown index snapshot failure code"
            );
        }
    }
}

fn context_from_header(candidate: &[u8]) -> SnapshotContext {
    let schema_epoch = fixed(candidate, EPOCH_OFFSET).map_or_else(
        || SchemaEpochId::from_bytes([0; 32]),
        SchemaEpochId::from_bytes,
    );
    let claimed_root_context = if candidate.get(OPTION_OFFSET..OPTION_OFFSET + 4) == Some(&OPTION_SOME) {
        fixed(candidate, ROOT_OFFSET).map(StateRoot::from_bytes)
    } else {
        None
    };
    SnapshotContext {
        schema_epoch,
        claimed_root_context,
    }
}

fn fixed(candidate: &[u8], offset: usize) -> Option<[u8; 32]> {
    candidate
        .get(offset..offset + 32)
        .and_then(|bytes| bytes.try_into().ok())
}

fn with_rehashed_trailer(payload: &[u8]) -> Vec<u8> {
    if payload.len() < TRAILER_BYTES {
        return payload.to_vec();
    }
    let preimage_len = payload.len() - TRAILER_BYTES;
    let mut candidate = payload[..preimage_len].to_vec();
    candidate.extend_from_slice(IndexSnapshotId::derive(&payload[..preimage_len]).as_bytes());
    candidate
}

#![allow(unsafe_code)]
#![no_main]

//! S20-700 Section 18.5 merge-engine surface: the S20-520 conflict decoder
//! (`sley_repo::decode_merge_conflict`) and the common-ancestor rule
//! (`sley_repo::find_common_ancestor`). Lane 0 feeds raw bytes to the
//! decoder; lane 1 rewrites the trailing `MergeConflictId`; lane 2 decodes
//! two synthetic head-first ancestries from the bytes and checks the
//! ancestor rule's invariants.

use core::slice;

use sley_id::{MergeConflictId, StateRoot, TransactionId};
use sley_repo::{
    BranchAncestryEntry, MergeErrorCode, decode_merge_conflict, encode_merge_conflict,
    find_common_ancestor,
};

const DIGEST_TRAILER_BYTES: usize = 32;
const MAX_FUZZ_INPUT_BYTES: usize = 65_536;
const SELECTOR_COUNT: u8 = 3;

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
    match selector % SELECTOR_COUNT {
        2 => fuzz_ancestor(payload),
        lane => {
            let rewritten;
            let candidate = if lane == 1 && payload.len() >= DIGEST_TRAILER_BYTES {
                rewritten = with_rehashed_trailer(payload);
                rewritten.as_slice()
            } else {
                payload
            };
            match decode_merge_conflict(candidate) {
                Ok(stored) => {
                    assert_eq!(
                        stored.stored_bytes, candidate,
                        "an accepted conflict must round-trip byte for byte"
                    );
                    let preimage_len = candidate.len() - DIGEST_TRAILER_BYTES;
                    assert_eq!(
                        stored.conflict_id,
                        MergeConflictId::derive(&candidate[..preimage_len]),
                        "accepted conflict identity drifted"
                    );
                    assert!(
                        !stored.conflict.conflicts.is_empty(),
                        "an accepted conflict carries at least one entry"
                    );
                    let again = encode_merge_conflict(&stored.conflict)
                        .expect("a decoded conflict must re-encode");
                    assert_eq!(again, stored, "re-encoding a decoded conflict drifted");
                }
                Err(error) => {
                    assert!(
                        error
                            .code()
                            .is_some_and(|code| MergeErrorCode::ALL.contains(&code)),
                        "unknown merge-conflict failure code"
                    );
                }
            }
        }
    }
}

/// Decodes two ancestries as sequences of one-byte transaction identities
/// (`0xff` separates ours from theirs) and checks the ancestor rule.
fn fuzz_ancestor(payload: &[u8]) {
    let mut halves = payload.splitn(2, |byte| *byte == 0xff);
    let ours = ancestry(halves.next().unwrap_or(&[]));
    let theirs = ancestry(halves.next().unwrap_or(&[]));
    match find_common_ancestor(&ours, &theirs) {
        Ok(found) => {
            assert!(
                theirs
                    .iter()
                    .any(|entry| entry.transaction_id == found.transaction_id),
                "ancestor must appear in theirs"
            );
            let position = ours
                .iter()
                .position(|entry| entry.transaction_id == found.transaction_id)
                .expect("ancestor must appear in ours");
            assert!(
                ours[..position].iter().all(|entry| !theirs
                    .iter()
                    .any(|other| other.transaction_id == entry.transaction_id)),
                "ancestor must be the first shared entry of ours"
            );
        }
        Err(error) => {
            assert_eq!(error.code(), Some(MergeErrorCode::NoCommonAncestor));
            assert!(ours.iter().all(|entry| !theirs
                .iter()
                .any(|other| other.transaction_id == entry.transaction_id)));
        }
    }
}

fn ancestry(bytes: &[u8]) -> Vec<BranchAncestryEntry> {
    bytes
        .iter()
        .take(64)
        .enumerate()
        .map(|(index, byte)| BranchAncestryEntry {
            transaction_id: TransactionId::from_bytes([*byte; 32]),
            state_root: StateRoot::from_bytes([*byte; 32]),
            parent_transaction_ids: bytes
                .get(index + 1)
                .map(|parent| TransactionId::from_bytes([*parent; 32]))
                .into_iter()
                .collect(),
        })
        .collect()
}

fn with_rehashed_trailer(input: &[u8]) -> Vec<u8> {
    let preimage_len = input.len() - DIGEST_TRAILER_BYTES;
    let conflict_id = MergeConflictId::derive(&input[..preimage_len]);
    let mut rewritten = input.to_vec();
    rewritten[preimage_len..].copy_from_slice(conflict_id.as_bytes());
    rewritten
}

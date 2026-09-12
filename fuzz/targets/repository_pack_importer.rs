#![allow(unsafe_code)]
#![no_main]

use core::slice;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use sley_id::{ObjectId, RepositoryPackId, StateRoot};
use sley_repo::{
    PackObjectEntry, PackRootEntry, decode_conformance_pack_entries_for_testing,
    import_conformance_pack, seal_mutated_conformance_pack_for_testing,
};
use sley_scb1::{FixtureContract, ScbError, decode_standalone_fixture};
use sley_store::ObjectStore;

const DIGEST_TRAILER_BYTES: usize = 32;
const MAX_FUZZ_INPUT_BYTES: usize = 65_536;
const SELECTOR_COUNT: u8 = 3;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Errors that prove the input reached step 6 (promotion): a store write may
/// have been attempted, so the no-store-write assertion does not apply.
/// Every other symbol is a preflight rejection (steps 1-5), which by
/// `import_conformance_pack` contract performs no object-store writes.
const PROMOTION_ERROR_SYMBOLS: [&str; 2] = ["STORE_IO", "STORE_OBJECT_SUBSTITUTION"];

/// The exact contract symbol a resealed mutation must produce.
enum ResealExpectation {
    /// Unmutated re-seal: the pack must import cleanly.
    Accept { roots: Vec<StateRoot> },
    /// Mutated re-seal: import must fail with exactly this symbol.
    Reject { symbol: &'static str },
}

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
    let resealed;
    let mut reseal_expectation: Option<ResealExpectation> = None;
    let candidate = if selector % SELECTOR_COUNT == 1 && payload.len() >= DIGEST_TRAILER_BYTES {
        rewritten = with_rehashed_pack_trailer(payload);
        rewritten.as_slice()
    } else if selector % SELECTOR_COUNT == 2 {
        let Some((bytes, expectation)) = with_resealed_content_mutation(payload) else {
            return;
        };
        resealed = bytes;
        reseal_expectation = Some(expectation);
        resealed.as_slice()
    } else {
        payload
    };

    let Some(temp) = TempRoot::new() else {
        return;
    };
    let store = ObjectStore::new(&temp.path);
    match import_conformance_pack(&store, candidate, &verify_fixture_object) {
        Ok(first) => {
            let preimage_len = candidate
                .len()
                .checked_sub(DIGEST_TRAILER_BYTES)
                .expect("an accepted pack must include its digest trailer");
            assert_eq!(
                first.pack_id,
                RepositoryPackId::derive(&candidate[..preimage_len]),
                "accepted repository pack identity drifted"
            );
            if let Some(ResealExpectation::Accept { roots }) = reseal_expectation {
                let reported: Vec<StateRoot> =
                    first.roots.iter().map(|root| root.root).collect();
                assert_eq!(
                    reported, roots,
                    "resealed pack roots drifted from the sealed claims"
                );
            } else if reseal_expectation.is_some() {
                panic!("a mutated resealed pack imported cleanly");
            }
            assert_eq!(
                first.present_objects, 0,
                "a clean import unexpectedly found existing objects"
            );

            let second = import_conformance_pack(&store, candidate, &verify_fixture_object)
                .expect("an accepted repository pack must import idempotently");
            assert_eq!(
                second.pack_id, first.pack_id,
                "repeat pack identity drifted"
            );
            assert_eq!(second.roots, first.roots, "repeat pack roots drifted");
            assert_eq!(
                second.promoted_objects, 0,
                "an idempotent import promoted an object twice"
            );
            assert_eq!(
                second.present_objects, first.promoted_objects,
                "repeat pack object accounting drifted"
            );
        }
        Err(error) => {
            if let Some(ResealExpectation::Reject { symbol }) = reseal_expectation {
                assert_eq!(
                    error.symbol(),
                    symbol,
                    "resealed component mutation escaped with the wrong failure class"
                );
            } else if reseal_expectation.is_some() {
                panic!("an unmutated resealed pack failed preflight");
            }
            if !PROMOTION_ERROR_SYMBOLS.contains(&error.symbol()) {
                assert!(
                    !store.root().join("objects").exists(),
                    "failed repository pack preflight promoted object state"
                );
            }
        }
    }
}

fn with_rehashed_pack_trailer(input: &[u8]) -> Vec<u8> {
    let preimage_len = input.len() - DIGEST_TRAILER_BYTES;
    let pack_id = RepositoryPackId::derive(&input[..preimage_len]);
    let mut rewritten = input.to_vec();
    rewritten[preimage_len..].copy_from_slice(pack_id.as_bytes());
    rewritten
}

/// Decodes the fixture pack, mutates one bound component selected by the
/// input, and re-seals so the input passes step 2 (digest tree) and reaches
/// the root/closure/object checks with attacker-controlled bytes.
/// Returns `None` when the input cannot drive this lane (undecodeable bytes,
/// too few control bytes, or an empty component family); the caller then
/// exercises the direct lane instead.
fn with_resealed_content_mutation(input: &[u8]) -> Option<(Vec<u8>, ResealExpectation)> {
    let Ok((epochs, mut roots, mut objects)) =
        decode_conformance_pack_entries_for_testing(input)
    else {
        return None;
    };
    let [class, index, bit, ..] = input else {
        return None;
    };
    let object_count = objects.len();
    let root_count = roots.len();
    let expectation = match class % 5 {
        // Unmutated re-seal: must still import cleanly with identical claims.
        0 => ResealExpectation::Accept {
            roots: roots.iter().map(|entry| entry.state_root).collect(),
        },
        // Mutated object bytes: the digest tree is recomputed over them, so
        // step 2 passes and `preflight_object` must report PACK_OBJECT_CORRUPT.
        1 => {
            let entry = objects.get_mut((*index as usize) % object_count.max(1))?;
            if entry.stored_bytes.is_empty() {
                return None;
            }
            flip_byte(&mut entry.stored_bytes, *index, *bit);
            ResealExpectation::Reject {
                symbol: "PACK_OBJECT_CORRUPT",
            }
        }
        // Mutated root bytes: step 2 passes and root admission must report
        // PACK_ROOT_INVALID.
        2 => {
            let entry = roots.get_mut((*index as usize) % root_count.max(1))?;
            if entry.stored_bytes.is_empty() {
                return None;
            }
            flip_byte(&mut entry.stored_bytes, *index, *bit);
            ResealExpectation::Reject {
                symbol: "PACK_ROOT_INVALID",
            }
        }
        // Mutated object-id claim: the closure check (step 4) runs before
        // per-object verification (step 5), so the now-unrequired id must
        // report PACK_OBJECT_MISSING (missing is checked before unexpected).
        3 => {
            let entry: &mut PackObjectEntry =
                objects.get_mut((*index as usize) % object_count.max(1))?;
            entry.object_id = mutated_id(entry.object_id.into_bytes(), *index, *bit);
            ResealExpectation::Reject {
                symbol: "PACK_OBJECT_MISSING",
            }
        }
        // Mutated state-root claim: the admitted root no longer matches, so
        // root admission must report PACK_ROOT_INVALID.
        _ => {
            let entry: &mut PackRootEntry =
                roots.get_mut((*index as usize) % root_count.max(1))?;
            entry.state_root = mutated_root(entry.state_root.into_bytes(), *index, *bit);
            ResealExpectation::Reject {
                symbol: "PACK_ROOT_INVALID",
            }
        }
    };
    // An empty component family cannot drive classes 1-4; class 0 (accept)
    // needs at least one root to bind.
    if matches!(expectation, ResealExpectation::Accept { .. }) && roots.is_empty() {
        return None;
    }
    let sealed = seal_mutated_conformance_pack_for_testing(epochs, roots, objects).ok()?;
    Some((sealed.stored_bytes.clone(), expectation))
}

fn flip_byte(bytes: &mut [u8], index: u8, bit: u8) {
    let position = (index as usize) % bytes.len();
    bytes[position] ^= 1 << (bit % 8);
}

fn mutated_id(bytes: [u8; 32], index: u8, bit: u8) -> ObjectId {
    let mut rewritten = bytes;
    rewritten[(index as usize) % rewritten.len()] ^= 1 << (bit % 8);
    ObjectId::from_bytes(rewritten)
}

fn mutated_root(bytes: [u8; 32], index: u8, bit: u8) -> StateRoot {
    let mut rewritten = bytes;
    rewritten[(index as usize) % rewritten.len()] ^= 1 << (bit % 8);
    StateRoot::from_bytes(rewritten)
}

fn verify_fixture_object(record: &[u8]) -> Result<ObjectId, ScbError> {
    decode_standalone_fixture(record, FixtureContract::EmptyObject)
        .or_else(|_| decode_standalone_fixture(record, FixtureContract::RequiredBool))
        .map(|fixture| fixture.object_id)
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new() -> Option<Self> {
        for _ in 0..1024 {
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "sley2-pack-persistent-fuzz-{}-{counter}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Some(Self { path }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(_) => return None,
            }
        }
        None
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

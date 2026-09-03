#![allow(unsafe_code)]
#![no_main]

use core::slice;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use sley_id::{ObjectId, RepositoryExchangeId};
use sley_mutate::import_entity_object;
use sley_repo::{import_repository_exchange, preflight_repository_exchange};
use sley_scb1::ScbError;
use sley_state_root::conformance_epoch_id;

const DIGEST_TRAILER_BYTES: usize = 32;
const MAX_FUZZ_INPUT_BYTES: usize = 65_536;
const SELECTOR_COUNT: u8 = 2;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
        rewritten = with_rehashed_exchange_trailer(payload);
        rewritten.as_slice()
    } else {
        payload
    };

    let Some(temp) = TempRoot::new() else {
        return;
    };
    let target = temp.path.join("clone");
    match preflight_repository_exchange(candidate, &verify_entity_object) {
        Ok(preflight) => {
            let preimage_len = candidate
                .len()
                .checked_sub(DIGEST_TRAILER_BYTES)
                .expect("an accepted exchange must include its digest trailer");
            assert_eq!(
                preflight.exchange_id,
                RepositoryExchangeId::derive(&candidate[..preimage_len]),
                "accepted repository exchange identity drifted"
            );
            assert_eq!(
                preflight.leaves,
                preflight.receipts + preflight.branches + 2,
                "accepted exchange leaf count drifted"
            );
            let first = import_repository_exchange(&target, candidate, &verify_entity_object)
                .expect("a preflighted exchange must import into a fresh target");
            assert_eq!(first.exchange_id, preflight.exchange_id);
            assert_eq!(
                first.accepted_head.transaction_id(),
                preflight.accepted_head.transaction_id,
                "clone head drifted from the exchange head"
            );
            assert_eq!(first.receipts, preflight.receipts);
            assert_eq!(first.branches, preflight.branches);
            let second = import_repository_exchange(&target, candidate, &verify_entity_object)
                .expect_err("a complete clone must reject a second import");
            assert_eq!(second.code(), "EXCHANGE_TARGET_NOT_EMPTY");
        }
        Err(_) => {
            let _ = import_repository_exchange(&target, candidate, &verify_entity_object)
                .expect_err("a failed preflight must fail the import");
            assert!(
                !target.exists(),
                "failed repository exchange preflight wrote into the target"
            );
        }
    }
}

fn with_rehashed_exchange_trailer(input: &[u8]) -> Vec<u8> {
    let preimage_len = input.len() - DIGEST_TRAILER_BYTES;
    let exchange_id = RepositoryExchangeId::derive(&input[..preimage_len]);
    let mut rewritten = input.to_vec();
    rewritten[preimage_len..].copy_from_slice(exchange_id.as_bytes());
    rewritten
}

fn verify_entity_object(record: &[u8]) -> Result<ObjectId, ScbError> {
    let epoch = conformance_epoch_id().map_err(|_| ScbError::new(sley_scb1::ScbErrorCode::ContractUnknown))?;
    import_entity_object(epoch, record).map(|object| object.object_id())
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new() -> Option<Self> {
        for _ in 0..1024 {
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "sley2-exchange-persistent-fuzz-{}-{counter}",
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

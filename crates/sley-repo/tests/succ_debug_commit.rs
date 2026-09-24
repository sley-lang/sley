//! TEMPORARY commit-debug helper (deleted after diagnosis).
use sley_policy::CandidateValidationLimits;
use sley_txn::{CommitInput, TransactionRepository};

#[test]
fn debug_commit_repro() {
    let repo_path = std::env::var("SUCC_DEBUG_REPO").unwrap();
    let candidate_hex = std::env::var("SUCC_DEBUG_CANDIDATE").unwrap();
    let principal_hex = std::env::var("SUCC_DEBUG_PRINCIPAL").unwrap_or_else(|_| "00".repeat(32));
    let repo = TransactionRepository::new(&repo_path);
    let head = repo.accepted_head().unwrap();
    let bytes = hex_decode(&candidate_hex);
    let principal = sley_id::PrincipalId::from_bytes(hex32(&principal_hex));
    let result = repo.commit(CommitInput::new(
        head.transaction_id(),
        &bytes,
        principal,
        &[],
        1_780_000_000_000,
        CandidateValidationLimits::full_v1(),
    ));
    println!("DEBUG_COMMIT: {result:?}");
    panic!("intentional debug stop");
}

fn hex_decode(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn hex32(text: &str) -> [u8; 32] {
    hex_decode(text).try_into().unwrap()
}

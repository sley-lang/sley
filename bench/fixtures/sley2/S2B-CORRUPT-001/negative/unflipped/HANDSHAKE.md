# Negative handshake: `unflipped` (S2B-CORRUPT-001, sley2)

The named Rust test is `s3_corrupt_neg_unflipped`
(`crates/sley-repo/tests/s3_g2_corrupt.rs`).

1. This is the positive-path control: with no byte flipped, the clean pack
   must import accepted (exact root, stable pack id, idempotent re-import
   promoting nothing). It proves the `PACK_DIGEST_MISMATCH` verdict is caused
   by the flip, not the harness.
2. On success-of-demonstration the test prints exactly
   `S3_NEG_RESULT ORACLE_CLEAN_ACCEPTED` to stdout, then panics
   (fails by design), so `cargo test` exits nonzero (cargo reports 101).
3. `oracle.py` runs
   `cargo test --offline -p sley-repo --test s3_g2_corrupt s3_corrupt_neg_unflipped -- --ignored --nocapture`
   (bounded timeout 240 s) and records `rejected` with code
   `ORACLE_CLEAN_ACCEPTED` only if BOTH the exit is nonzero AND the output
   contains the exact code line. A passing test or a missing code line is a
   harness error (exit 2), never a verdict.

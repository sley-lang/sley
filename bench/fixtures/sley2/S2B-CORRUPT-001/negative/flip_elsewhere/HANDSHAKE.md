# Negative handshake: `flip_elsewhere` (S2B-CORRUPT-001, sley2)

The named Rust test is `s3_corrupt_neg_flip_elsewhere`
(`crates/sley-repo/tests/s3_g2_corrupt.rs`).

1. A different canonical object byte is flipped (5/8 into the first embedded
   object entry instead of the positive's 3/8). The real
   `import_conformance_pack` must fail before ref movement with the same
   exact symbol `PACK_DIGEST_MISMATCH`, destination store unchanged.
2. On success-of-demonstration the test prints exactly
   `S3_NEG_RESULT PACK_DIGEST_MISMATCH` to stdout, then panics
   (fails by design), so `cargo test` exits nonzero (cargo reports 101).
3. `oracle.py` runs
   `cargo test --offline -p sley-repo --test s3_g2_corrupt s3_corrupt_neg_flip_elsewhere -- --ignored --nocapture`
   (bounded timeout 240 s) and records `rejected` with code
   `PACK_DIGEST_MISMATCH` only if BOTH the exit is nonzero AND the output
   contains the exact code line. A passing test or a missing code line is a
   harness error (exit 2), never a verdict.

# Negative handshake: `overlapping_change` (S2B-MERGE-001, sley2)

The named Rust test is `s3_merge_neg_overlapping_change`
(`crates/sley-repo/tests/s3_g2_merge.rs`).

1. The test judges the overlapping mutation (both branches change typedef 6
   visibility incompatibly) with the real `judge_merge` and demonstrates the
   canonical conflict object: exactly 1 `ConflictEntry`
   (`FieldEdit`, field 4), stable `conflict_id`, canonical round-trip.
2. On success-of-demonstration the test prints exactly
   `S3_NEG_RESULT ORACLE_MERGE_CONFLICT` to stdout, then panics
   (fails by design), so `cargo test` exits nonzero (cargo reports 101).
   (`ORACLE_MERGE_CONFLICT`, not an engine error symbol: a conflict is a
   successful judge outcome, never silent.)
3. `oracle.py` runs
   `cargo test --offline -p sley-repo --test s3_g2_merge s3_merge_neg_overlapping_change -- --ignored --nocapture`
   (bounded timeout 240 s) and records `rejected` with code
   `ORACLE_MERGE_CONFLICT` only if BOTH the exit is nonzero AND the output
   contains the exact code line. A passing test or a missing code line is a
   harness error (exit 2), never a verdict.

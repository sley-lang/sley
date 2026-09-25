# Negative handshake: `guard_disabled` (S2B-STALE-001, sley2)

The named Rust test is `s3_stale_neg_guard_disabled`
(`crates/sley-repo/tests/s3_g2_stale.rs`).

1. The test replays the positive A/B replacement sequence, then demonstrates
   the mutation: B's replacement re-based onto the live head (the staleness
   parent check bypassed, arm-natural form). The bypass commit visibly
   succeeds — last-write-wins, the earlier value silently lost.
2. On success-of-demonstration the test prints exactly
   `S3_NEG_RESULT ORACLE_LAST_WRITE_WINS` to stdout, then panics
   (fails by design), so `cargo test` exits nonzero (cargo reports 101).
3. `oracle.py` runs
   `cargo test --offline -p sley-repo --test s3_g2_stale s3_stale_neg_guard_disabled -- --ignored --nocapture`
   (bounded timeout 240 s) and records `rejected` with code
   `ORACLE_LAST_WRITE_WINS` only if BOTH the exit is nonzero AND the output
   contains the exact code line. A passing test or a missing code line is a
   harness error (exit 2), never a verdict.

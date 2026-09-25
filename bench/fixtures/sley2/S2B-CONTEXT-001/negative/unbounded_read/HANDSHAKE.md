# Negative handshake: `unbounded_read` (S2B-CONTEXT-001, sley2)

The named Rust test is `s3_context_neg_unbounded_read`
(`crates/sley-repo/tests/s3_g2_context.rs`).

1. The full 10,004-entity closure is demanded as one single-shot read with
   continuations disallowed (`allow_continuation = false`, page limit below
   the closure size). The real `execute_root_query` refuses with the engine
   code `QUERY_REQUIRED_FACT_OMITTED`
   (`RootQueryErrorCode::RequiredFactOmitted`,
   `crates/sley-query/src/root_query.rs`) instead of silently truncating:
   the whole-store dump has no serving path.
2. On success-of-demonstration the test prints exactly
   `S3_NEG_RESULT QUERY_REQUIRED_FACT_OMITTED` to stdout, then panics
   (fails by design), so `cargo test` exits nonzero (cargo reports 101).
3. `oracle.py` runs
   `cargo test --offline -p sley-repo --test s3_g2_context s3_context_neg_unbounded_read -- --ignored --nocapture`
   (bounded timeout 240 s) and records `rejected` with code
   `QUERY_REQUIRED_FACT_OMITTED` only if BOTH the exit is nonzero AND the
   output contains the exact code line. A passing test or a missing code
   line is a harness error (exit 2), never a verdict.

# Review queue REQ-08 — next-wave fresh and finishing reviews (staged)

Baseline: current `main` HEAD at dispatch time (verify `git rev-parse HEAD`
before starting; SMP1 scope below pins the post-`ad46a43` delta).

Each item is a first review of its lane at the stated scope. Read-only;
verdicts in the REQ format against the named field; prior verdicts and
transcripts preserved. No verdict advances without a qualifying review.

## Dispatchable now (scope unaffected by the approved redesign work)

1. `s20_700_smp1_persistent_fuzz_slice.vulcan_review` — FRESH qualifying
   review. Scope: everything since `ad46a43` (the `b64ab9e` scope),
   notably the `9df9f9e` `fixture_hellos_negotiate_ok` self-check test
   (`crates/sley-protocol/src/lib.rs`) and the S20-410 R2 doc correction
   (`negotiate` / `negotiate_identity` / `negotiate_versioned` `# Errors`
   comments). Must explicitly verify all seven: narrowed oracle arm
   (`fuzz/targets/smp1_frame_decoder.rs:148-152` NoCommonProfile-only);
   server-side gate (`:135` per-call `server.validate()`); live regression
   seed (`scripts/run_smp1_persistent_fuzz.py:674-691`,
   `seed-regression-S20-700-SMP1-001-*` executed every smoke);
   `S20-700-SMP1-001` (`fuzz/regressions/S20_700_SMP1_001.json`
   finding/input/expected/disposition); corpus execution (executed >=
   floor, monotonic inline counters); instrumentation reach (nonzero
   owner-rlib `sancov` via cargo-json linkage); absence of the previously
   observed crash (zero new artifacts AND clean retest of priors). Crash
   history must not be erased. Obligation stays REVISE until this returns
   PASS.
2. `legacy_adapter_contract_review` (lane `legacy_artifact_adapter`,
   contract `docs/spec/LEGACY_ARTIFACT_ADAPTER_V1.md` + ADR-0018) — first
   correct-lane review. Scope: `bench/legacy/runner.py`, checker
   `scripts/check_legacy_runner.py`, tests `bench/legacy/tests/test_runner.py`,
   smoke `make legacy-runner-smoke`. Must assess pinned outer identity,
   manual confined extraction, per-file rehash, single fixed argv, bounded
   child execution, create-only evidence; timeout-evidence continuity
   (retained records absent on the review host); the identified checker
   weaknesses (token-grep, hardcoded counts, no register/`FROZEN_CONTRACT`
   reconciliation, absent-artifact FAIL-vs-BLOCKED); and the 6/15
   unexercised codes. The prior fuzz REVISE
   (`evidence/review/verdicts/s20_600_frozen_legacy_adapter/vulcan_review-e464ed4.md`)
   is provenance only and scores nothing here.
3. `s20_700_vm_persistent_fuzz_slice.vulcan_review` — re-review
   (`PENDING_S20_700FUZZ_FIX_LANDED_REREVIEW`; fix landed, never reviewed).

## Held until after the approved redesign implementations land

4. `s20_700_pack_persistent_fuzz_slice.vulcan_review` — fresh review of the
   rehash-lane repair (per-component mutation-detected proof).
5. `s20_700_query_persistent_fuzz_slice` — `final`-round review folding the
   round-1 REVISE (fixture-depth repair + precedence probes).
6. `s20_700_schema_persistent_fuzz_slice` — `final`-round review folding the
   round-1 REVISE.
7. `s20_700_semantic_checkers_persistent_fuzz_slice` — fresh review of the
   Err-narrowing repair, including the wrong-code control run.

Items 4–7 dispatched only from the post-implementation SHA with regenerated
proofs; proofs bound to older source states are invalid and must not be
re-filed.

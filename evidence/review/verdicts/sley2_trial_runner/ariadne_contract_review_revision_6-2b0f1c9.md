<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: ariadne; field: ariadne_contract_review_revision_6; dispatched: 2026-09-23T11:55:14Z; duration_s: 1053; process_exit_code: 0 -->
# Ariadne Council review — sley2_trial_runner

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

What I verified myself. The tree stayed read-only: I edited, created, formatted or committed nothing tracked. Two things did change as side effects:
- `cargo test` rebuilt the debug `sley` binary under the preset `CARGO_TARGET_DIR`.
- Python wrote gitignored `__pycache__` files.

During the review, seven untracked verdict transcripts from other concurrent dispatches appeared under `evidence/review/verdicts/`, one of them `sley2_trial_runner/vulcan_surface_review_revision_7-2b0f1c9.md`. They are not mine, and I did not read them, to keep this review independent. `HEAD` was unchanged at the end.

**Scope and history**
- `git rev-parse HEAD` returned 2b0f1c9f4940c020565137891a49a3769cd02061, which matches the scope.
- `git log 5b538f36..HEAD`: 24 commits. `git diff --stat 5b538f36..HEAD`: 89 files.
- Full diffs read for the S20-620 spec, SMP1, S20-300, ADR-0036, the closeout, `tooling.py` and its tests, `sley2_tool.py`, and `server.rs`.
- `git diff f0738119..HEAD`: the only change to `SLEY2_TRIAL_RUNNER_V1.md` since the Nabu/Vulcan round-6 scope is the Status line and the revision-6 sentence at :3 and :15-20. §9 is byte-identical to what they reviewed.
- `git show 8357c243` is the revision-6 bump.
- `git log -S'unmintable snapshot'` shows that sentence dates from 05b49112.

**Checkers** (run through a python3 subprocess)
- `check_sley2_trial_runner.py`: exit 0, `"result": "PASS"`, revision 6, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, problems []. This includes its own run of `bench/sley2/tests` (23 tests).
- `check_smp1_contract.py`: exit 0, PASS, revision 14, `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING`.
- `check_complete_root_index_snapshot_profile.py`: exit 0, PASS, revision 5, `S20_300_FULL_IMPLEMENTED_REVIEW_PENDING`.
- `check_finding_register.py`: exit 0, PASS.

**In-memory checker probe** (summary `read` monkeypatched, nothing written)

| Case | Result |
|---|---|
| baseline | PASS |
| `contract_revision` 5 against spec 6 | FAIL `machine-summary:contract_revision:5!=spec:6` |
| COMPLETE without `ariadne_contract_review_revision_6` | FAIL `completion-unbound-review:ariadne_contract_review` |
| COMPLETE with a PASS `_revision_6` field | PASS |
| COMPLETE with only a PASS `_revision_5` field | FAIL |
| COMPLETE with a REVISE `_revision_6` field | FAIL |

**Tests**
- `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 4 passed. These are the v1 no-field-9 test, the v2 disclosure test, `workspace_open_v3_answers_the_version_2_open_summary`, and `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record`.
- `cargo test --locked --offline -p sley-repo --lib index_cache`: 12 passed. This includes the symlink/FIFO absence tests and the non-canonical guard test.
- `cargo test --locked --offline -p sley-cli --test cli workspace_open`: compile only, 0 matched. This built the debug `sley` binary at HEAD.
- `python3 -m unittest bench.live.tests.test_tooling bench.live.tests.test_witness_provenance bench.live.tests.test_scratch_cleanup`: 25 tests OK, 1 skipped (the real-binary scratch case, whose environment variable was unbound).
- `bench.live.tests.test_agent_access`, `test_mediated_access` and `test_mediated_gateway`, run with `SLEY2_SLEY_BINARY` set to the HEAD-built binary: 61 tests OK in 463.7 s. This includes `test_warm_up_owner_refusal_wins_and_materializes_nothing` and `test_root_query_response_records_its_continuation_binding`.
- Not run:
  - the 8 integrated `test_mediated_context` proofs;
  - the full 254-test `bench/live` discovery;
  - the witnesses;
  - `make quick` and `make lint`.

  For these I rely on gate record §12.4, and no finding below depends on them.

**Files read** (line ranges)
- Specs:
  - `docs/spec/SLEY2_TRIAL_RUNNER_V1.md` 1-367 (full)
  - `docs/spec/SMP1.md` (full diff; 716-761)
  - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` 1-30 plus the full diff
  - `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` 318-375
- Checker and bench code:
  - `scripts/check_sley2_trial_runner.py` 120-318
  - `bench/live/tooling.py` 60-209
  - `bench/live/sley2_tool.py` (diff: 71-90, 199-255, 389-399)
  - `bench/live/mediated_sley.py` 215-235
  - `bench/live/trusted_capture.py` 405-460
  - `bench/fixtures/sley2_live_judge.py` 3010-3310, 3380-3570, plus the diff since f0738119
  - `bench/live/scratch.py` 1-117
  - `bench/live/tests/test_scratch_cleanup.py` 1-153
  - `bench/live/tests/test_mediated_gateway.py` 139-184
  - `bench/live/tests/test_tooling.py` (diff)
- Rust:
  - `crates/sley-protocol/src/server.rs` 2450-2465, 2885-2954, 3280-3315
  - `crates/sley-protocol/src/server_tests.rs` 7393-7586
  - `crates/sley-repo/src/index_cache.rs` 260-300
  - `crates/sley-txn/src/repository.rs` 875-900
  - `crates/sley-txn/src/maintenance.rs` 95-175
- Records and reviews:
  - gate record 513-784
  - my revision-5 verdict (full)
  - the Nabu and Vulcan revision-6 finding lines
  - machine summary `sley2_trial_runner` (every key)
  - finding register obligations 414-421, open_reviews 36-38
  - ADR-0036 (diff), the closeout (diff), the WORK_PACKAGES S20-620 row
  - `bench/live/SUCCESSION-COVERAGE.md` 25-33, 515-560
  - `bench/live/MEDIATED-INPUT-ACCOUNTING.md` 80-115

## Evidence checked

- **Contract revision 6.**
  - The Status line at :3 says revision 6. The history sentence at :15-20 names the in-place §9 answers and keeps the revision-5 verdicts as history.
  - The Boundary at :35-37 now defers the `workspace.open` body to SMP1 and says this contract "only admits the method".
  - §9 (:316-340) cites SMP1 row 201 / `open_summary` for the request and response. It states:
    - probe failure is absence;
    - the probe never builds or writes, and the response counts one entity;
    - absence is structural, with no `omitted` or `truncated` signal;
    - a non-empty body is refused `PROTOCOL_PAYLOAD_INVALID`;
    - warm-up precedence: `QUERY_SNAPSHOT_MISMATCH` when the engine answers, otherwise the owner code, with materialization only if the cache write succeeds.
  - The continuation rule is at :341-349.
  - The frozen nineteen names, the `holds nineteen names` anchor (:303) and the `Those two entity names` terminator (:309) are verbatim, and the checker's allowlist-in-spec-order check passes. The heading at :280 is revision-neutral.
- **Code against §9.**
  - `server.rs:2904-2916`:
    - refuses a non-empty body first;
    - loads the head (blocking shared maintenance);
    - probes only when `protocol_version >= V2`;
    - uses `head_open_summary`, which appends field 9 only when the probe accepts a record (`:3292-3301`);
    - counts 1 entity.
  - `materialized_head_snapshot` (`:2925-2933`) uses the non-blocking shared acquisition and maps any error to None.
  - `cached_complete_root_snapshot_id` (`index_cache.rs:260-274`) does the following:
    - checks the guard with `covers`;
    - opens the file once with `O_NOFOLLOW|O_NONBLOCK`, requires a regular file on the open handle, and reads at most `MAX_SNAPSHOT_RECORD_BYTES + 1` bytes (`:283-300`);
    - accepts the record under `accept_cached`;
    - never writes, rebuilds or deletes.
  - The server tests pin cold = `revision.read` bytes, warm field 9 = the identity `query.root` binds, `revision.read` unchanged, and the body refusal under v1, v2 and v3.
- **Continuation rule against the judge.**
  - `_ContinuationLedger` (`sley2_live_judge.py:3025-3106`) matches §9:
    - a truncated `query.root` page (no cursor, binding present) opens (query key, next);
    - only a successful `query.continue` whose `after` equals an open next discharges it;
    - a continue with no binding or no open match rejects;
    - failed pages open nothing;
    - cursor-bearing `query.root`, `query.restricted` and `refs.list` omissions stay unbound;
    - `finish` rejects anything left open.
  - The binding is derived on the trusted side (`sley2_tool.py:228-253`). The offsets `_RQ_PREFIX` = 192 and `_RR_PREFIX` = 224 and the cursor payload sizes {1: 32, 2: 68, 3: 32} match ROOT_BACKED §5-6 (:320-373). The binding is carried unchanged into the capture on the mediated route (`mediated_sley.py:227-232`, `trusted_capture.py:418-455`). Both audit docstrings now describe the ledger.
- **TOOLING.**
  - `tooling.py:150-178` states:
    - the warm-up refusal precedence and the conditional materialization;
    - that a `QUERY_REQUIRED_FACT_OMITTED` refusal is counted, not rejected;
    - trial-wide query/cursor continuation, where only `query.continue` carries a cursor;
    - the rejection conditions, which match the ledger;
    - reply-byte budgets tied to the enforced constants.
  - The request and response layouts at :123-148 are unchanged from my revision-5 byte check. The pins are in `test_documented_layout_is_pinned` and `test_documented_budgets_are_the_enforced_constants`, and they pass.
- **Machine summary** (`sley2_trial_runner`):
  - `contract_revision` 6, status `S20_620_IMPLEMENTED_REVIEW_PENDING`, `implementation_complete` false.
  - The base fields keep their byte-exact revision-4 PASS values, with dated notes.
  - The `_revision_5` fields hold REVISE ×3. `nabu_…_revision_6` is PASS_0_P0_0_P1_0_P2_2_P3_3_P4 and `vulcan_…_revision_6` is PASS_…_3_P4_PRIOR_P3_P4_CLOSED, both scoped to f073811. No Ariadne `_revision_6` field exists.
  - The `status_note` is accurate.
  - The register rows 414-421 carry these values, and the three `_revision_5` REVISE rounds are still open reviews.
- **Other records.** The WORK_PACKAGES S20-620 row, the closeout Revision paragraph (`:7-14`) and the ADR-0036 Status (`:3-10`, revision 6, 23 tests) agree with the summary.
- **Scratch-leak fix (§12.6).**
  - `remove_scratch` restores owner permissions, retries, and raises `ScratchRemovalError` if the tree survives. `scratch_root` restores `TMPDIR` and `tempfile.tempdir` and removes the root on every path.
  - `_main` removes the judge's scratch in a `finally`, and a failed removal becomes a `LIVE_SLEY2_JUDGE_INVALID` harness error.
  - This adds no new stable code and changes no trial-runner contract text.
  - The committed witness logs predate it (source c21ff945, "clean apart from witness outputs"). §12.4 discloses the post-fix rerun at d307a22f.

### Revision-5 findings (ariadne_contract_review_revision_5-5b538f3)

- **[P1] contract-composition, SMP1 row 201 / `open_summary` ownership: CLOSED.**
  - SMP1 now defines the record. Row 201 reads "none | accepted head summary (appendix A)" (`SMP1.md:348`). Appendix A row 201 and `open_summary` are at `:682`, `:716-753`, and the owner split (fields 1-8 S20-390, field 9 S20-300) is stated.
  - The version applicability is stated: v1 answers `revision_summary` exactly; v2 and v3 answer `open_summary`.
  - The revision-12 "unchanged" statements are amended (`:55-83`, `:327-338`, appendix D `:893-896`).
  - `check_smp1_contract.py` PASSes at revision 14. The server tests pin v1, v2 and v3.
  - S20-620 §9 now cites SMP1 instead of defining the record, so the Boundary is true.
  - The owner-lane review of revision 13 ran (REVISE, on the version scope) and is answered by revision 14. The revision-14 review is pending in its own lane, and this verdict does not accept it. Stale pin text remains; see the P3 below.
- **[P2] contract-composition, S20-300 admission of the hit-path probe: CLOSED.**
  - Profile revisions 4 and 5 define `cached_complete_root_snapshot_id` in §5 (`:178-206`). The definition gives:
    - the cost class: one bounded file read, no object, no build, no write-back, no delete;
    - acceptance under rules 1-4;
    - absence semantics, including discard = absence;
    - the guard rule (`covers`, with `INDEX_SNAPSHOT_IO` the only error);
    - the sole consumer, and pointer-not-evidence.
  - The preamble (`:45-48`), the hit-reader bound (`:165-168`) and the §9 exclusion (`:274-276`) are corrected.
  - Checker PASS at revision 5. The code matches.
- **[P3] spec-precision, §9 "when and only when" and warm-up refusal: CLOSED.**
  - The text now says "only when … any probe failure … is absence", gives `QUERY_SNAPSHOT_MISMATCH` "when the engine can answer it" and the owner code otherwise, and materializes "if the cache write succeeds" (`:320-338`).
  - The owner-code case is pinned by `test_warm_up_owner_refusal_wins_and_materializes_nothing` (INDEX_SNAPSHOT_ROOT_INCOMPLETE, no snapshot ever), which I ran and which passes.
- **[P3] agent-contract, the one-shot route cannot page: CLOSED.** The alternative closure landed: the trial-wide, chain-bound continuation ledger. §9 now states it (`:341-349`), TOOLING documents it (`:162-170`), sentence pins exist, and I verified the ledger against the §9 text above. The Nabu and Vulcan round-6 reviews covered it.
- **[P3] record, SUCCESSION-COVERAGE:29 / MEDIATED-INPUT-ACCOUNTING:102-104: CLOSED.** Both records now say TOOLING documents the root-query layouts, continuation and budgets. The CONTEXT row cites 8 integrated proofs, and `test_mediated_context.py` has 8 `def test_`. Other stale passages in these files remain; see the P4 below.
- **[P4] conformance, `workspace.open` ignored a body: CLOSED.** `server.rs:2905-2907` refuses a non-empty body under every version. It is tested at `server_tests.rs:639`, `:7455-7460` and `:7529-7534`. SMP1 appendix A (`:755-760`) records the pre-existing leniency of the other empty-request rows.
- **[P4] record, ADR-0036 status and §9 heading: CLOSED.** The ADR Status (`:3-10`) names revision 6 and 23 tests. The heading is "9. Clarifications (revision 2 onward)" (`:280`). Both checker anchors are verbatim, and the checker passes.

## Findings

[P3] [contract-composition] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:37,317-318,321 - The revision-6 contract still pins superseded owner revisions. The Boundary (:37) and §9 (:318) cite "SMP1 revision 13", and §9 (:321) cites "`COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` section 5, revision 4". At HEAD, SMP1 is revision 14 (SMP1.md:3) and S20-300 is revision 5 (COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:3), and both cited revisions returned REVISE in round 6 (gate record :613-616). Commit 8357c243 bumped this contract to revision 6. The same commit re-pinned server.rs:2890, :2918 and :3288 and the bridge, CLI and session-handle consumers to SMP1 14, but not this contract. `check_sley2_trial_runner.py` has no pin check, so it still passes. There is no wire effect: the trial selects version 2 only (:60-62), and revision 14 leaves the version-2 `open_summary` unchanged. - Closure evidence: re-pin :37 and :318 to SMP1 revision 14 and :321 to S20-300 revision 5 (or make the references revision-neutral). Preferably also add a checker anchor that refuses a stale pin, following the NATIVE_TEST_ADMISSION pin check in `check_smp1_contract.py`, with a revert test.
[P4] [spec-precision] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:323-327 (with crates/sley-protocol/src/server.rs:2899-2901) - §9 lists "an absent … maintenance boundary" among the probe failures that are absence, and says only that the head load "takes the shared maintenance lock". The owner text SMP1.md:741-744 says "an absent boundary fails the method". The code agrees with SMP1: `workspace_open` loads the head first (server.rs:2908), and `accepted_head` → `acquire_shared_repository_maintenance` requires the existing lock file (sley-txn repository.rs:881-884; maintenance.rs:107-111, 153-157). The probe's absent-boundary branch is therefore reachable only if the boundary vanishes between the two acquisitions. A reader of the consumer text would expect an eight-field answer where the method actually fails. - Closure evidence: in §9 and the server docstring, either say that an absent boundary fails the method at the head load (per SMP1), or drop "absent" from the probe-failure list.
[P4] [record] bench/live/SUCCESSION-COVERAGE.md:29,540-543 - Two passages are stale. (1) The CONTEXT row's remaining column still says "SMP1 revision 13 and S20-300 revision 4 reviews pending". Those rounds ran and returned REVISE; the open items are now SMP1 r14, S20-300 r5 and this lane's r6 review. (2) The "Discovery gate retained" paragraph (pre-existing, 05b49112) still says, in the present tense, that "no permitted bounded enumeration route exists (… server queries need an unmintable snapshot)". That contradicts S20-620 §9 and the row at :29. The continuation paragraph just above it (:524-526, :534-539) was marked superseded; this one was not. - Closure evidence: refresh both passages, or mark them historical with a date.

## Assessment

The revision-6 contract, its implementation and its records conform on the substance my lane owns. I verified this against code, tests, vectors and checker output, not against the gate record:
- The accepted-head opener is now defined by SMP1, and S20-620 only admits it, so the Boundary is true. S20-300 defines and gates the identity probe it relies on.
- The server matches the text: the body refusal, the version gate, structural absence, the one-entity count, and a non-waiting probe that never writes. The tests pin v1, v2 and v3.
- The chain-bound continuation rule in §9 matches `_ContinuationLedger` exactly. The trusted-side bindings follow the ROOT_BACKED §5-6 layout byte for byte.
- TOOLING states the same rules and budgets, pinned to the enforced constants.
- The checker's revision drift gate and `_revision_6` completion gate work in both directions.
- The machine summary, register, WORK_PACKAGES, closeout and ADR agree.

All seven revision-5 findings are closed. The scratch-leak fix is sound, is disclosed, and changes no contract text.

What remains is minor:
- stale revision pins for the two owner documents in this contract's own text (P3);
- one clause restated more loosely than the owner text (P4);
- stale record passages (P4).

This PASS covers the S20-620 revision-6 lane only. It does not accept SMP1 revision 14 or S20-300 revision 5, whose own reviews are pending in their lanes and tracked in the register. If either review changes the version-2 `open_summary` or the probe, S20-620 §9 needs re-review. I make no claim about GA, release readiness or a live-model trial.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_2_P4_PRIOR_P3_P4_CLOSED
SECTION: sley2_trial_runner
FIELD: ariadne_contract_review_revision_6
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P3] [contract-composition] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:37,317-318,321 - Boundary and §9 still pin "SMP1 revision 13" and "S20-300 section 5, revision 4", while HEAD is SMP1 r14 (SMP1.md:3) and S20-300 r5 (profile :3); both cited revisions were REVISE'd in round 6; 8357c243 re-pinned server.rs docstrings and the bridge/CLI/session-handle consumers but not this contract, and the checker has no pin check; no wire effect (version 2 only) - re-pin to SMP1 14 / S20-300 5 (or revision-neutral), ideally with a checker anchor and revert test | [P4] [spec-precision] docs/spec/SLEY2_TRIAL_RUNNER_V1.md:323-327 (with server.rs:2899-2901) - §9 counts an absent maintenance boundary as probe absence, but SMP1.md:741-744 says an absent boundary fails the method, as the code does (head load first, server.rs:2908; the blocking shared acquisition requires the lock file, maintenance.rs:107-111,153-157) - state the head-load failure or drop "absent" in §9 and the docstring | [P4] [record] bench/live/SUCCESSION-COVERAGE.md:29,540-543 - CONTEXT row still lists "SMP1 revision 13 and S20-300 revision 4 reviews pending", and the unmarked "Discovery gate retained" paragraph still says no bounded enumeration route exists because server queries need an unmintable snapshot, contradicting S20-620 §9 and row :29 - refresh or mark historical
SUMMARY: S20-620 revision 6 conforms. The `workspace.open` body is now owned by SMP1 revision 14 and the identity probe by S20-300 revision 5. The server, the tests (v1, v2 and v3), the chain-bound continuation ledger, the TOOLING layouts and budgets, the checker's revision and completion gates (verified in memory in both directions), and the summary and register records all match the contract text, and all seven revision-5 findings are closed. What remains is one P3 (the contract still pins the superseded SMP1 revision 13 and S20-300 revision 4) and two P4 notes (a loose absent-boundary clause and stale coverage-record passages). This PASS does not accept the SMP1 revision 14 or S20-300 revision 5 reviews, which are pending in their own lanes.

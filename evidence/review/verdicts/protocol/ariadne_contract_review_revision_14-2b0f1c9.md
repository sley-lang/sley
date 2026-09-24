<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: ariadne; field: ariadne_contract_review_revision_14; dispatched: 2026-09-23T11:55:14Z; duration_s: 416; process_exit_code: 0 -->
# Ariadne Council review — protocol

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

**Scope.** `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, matching SCOPE_SHA. The tree was clean at the start. At the end, one untracked file I did not create was present: `evidence/review/verdicts/complete_root_index_snapshot/ariadne_contract_review_revision_5-2b0f1c9.md`, most likely from a concurrent review. I did not touch it. HEAD was unchanged.

**Git commands run:**
- `git log --oneline f0738119..HEAD`: 15 commits. The protocol delta is `8357c243` (SMP1 revision 14, NATIVE_TEST_ADMISSION revision 6, consumer re-pins) and `50fca15f` (entrypoints anchor). Fuzz proofs were refreshed in `c2664f10` at `46118639`.
- `git diff --stat` and full `git diff f0738119..HEAD` over `docs/spec`, `crates/sley-protocol` and `scripts/` (20 files, +730/−125).
- `git show 116d031e^:crates/sley-protocol/src/lib.rs` for `negotiate_versioned` before version 3 landed.
- `git log -S TESTS_REPORT_READ_TAG`: origin `116d031e`, 2026-09-16.
- `git merge-base --is-ancestor`: `dda51a16` and `8357c243` are both ancestors of `46118639`.
- `git diff --stat 46118639..HEAD` over the fuzz-lane crates and `fuzz/`: empty.

**Checkers run (exit code and result line):**
- `check_smp1_contract.py`: 0, PASS, revision 14, `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING`.
- `check_smp1_json_bridge_contract.py`: 0, PASS, revision 11.
- `check_session_handle_profile.py`: 0, PASS, revision 5, `smp1_revision` 14.
- `check_cli_contract.py`: 0, PASS, revision 9.
- `check_cli_rules.py`: 0, PASS, 46 tags audited.
- `check_required_contract_index.py`: 0, PASS.
- `check_complete_root_index_snapshot_profile.py`: 0, PASS, revision 5.
- `generate_smp1_fixtures.py --check`: 0, PASS, no drift.
- `generate_smp1_json_bridge_table.py --check`: 0, PASS.
- `generate_smp1_json_bridge_fixtures.py --check`: 0, PASS (5 vectors, 36 rejections).
- `generate_native_test_fixtures.py --check`: 0, no output.
- `check_smp1_persistent_fuzz_slice.py`: 0, PASS.
- `check_smp1_json_bridge_persistent_fuzz_slice.py`: 0, PASS.
- Vector checkers, run with `oracle/scb1/.venv/bin/python` because they need `blake3`:
  - `check_smp1_vector.py`: 0, PASS (3 frames, 4 mutations).
  - `check_smp1_json_bridge_vector.py`: 0, PASS (41 methods, 5 vectors, 36 rejections).
  - `check_entity_read_vectors.py`: 0, PASS (23 cases).
  - `check_complete_root_index_snapshot_vector.py`: 0, PASS.
  - `check_root_backed_query_vector.py`: 0, PASS (27 vectors).
  - `check_context_capsule_vector.py`: 0, PASS (24 vectors).
  - `check_native_test_vectors.py`: 0, "6 requests, 6 responses, 7 rejections OK".

**Unit tests run (Python and Rust):**
- `python3 -m unittest`:
  - `scripts/test_smp1_contract.py`: 0, 16 OK.
  - `scripts/test_cli_contract.py`: 0, 10 OK.
  - `scripts/test_session_handle_profile.py`: 0, 6 OK.
  - `scripts/test_current_contract_review.py`: 0, 6 OK.
  - `scripts/test_complete_root_index_snapshot_profile.py`: 0, 13 OK.
  - `scripts/test_required_contract_index.py`: 0, 6 OK.
  - `scripts/test_cli_rules.py`: 0, 10 OK.
  - `bench/live/tests/test_scratch_cleanup.py`: 0, 7 run, 1 skipped (binary-gated).
- `scripts/test_smp1_json_bridge_table.py` run as a script: 0, 17 cases PASS.
- Cargo:
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 0, 4 passed (v1 no field 9, v2 disclosure, v3 `open_summary`, probe non-waiting/discarded record).
  - `cargo test --locked --offline -p sley-protocol --lib version_three_negotiation_gates_native_tags_on_feature_bit`: 0, 1 passed.

**Files read (line ranges):**
- `docs/spec/SMP1.md` 1-90, 120-280, 320-349, 524-534, 638-650, 655-798, 885-903.
- `crates/sley-protocol/src/server.rs` 1236-1285, 2681-2700, 2755-2775, 2885-2933.
- `crates/sley-protocol/src/lib.rs` 775-825, 855-888, 1055-1098, 3476-3575; also `negotiate_versioned` at `116d031e^` (925-942).
- `crates/sley-protocol/src/server_tests.rs`: the diff hunks (607-640, 7383-7585).
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md` 1-10, 340-370, 540-575.
- `docs/spec/SESSION_HANDLE_PROFILE_V1.md` 1-30, 360-394.
- `docs/spec/SLEY_CLI_V1.md` 1-31, 336-362.
- `docs/spec/SMP1_JSON_BRIDGE_V1.md` 1-50.
- `docs/spec/SLEY2_TRIAL_RUNNER_V1.md` 30-42.
- `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`, `docs/spec/ENTITY_READ_PROFILE_V2.md`, `docs/spec/ERROR_CODES_V1.md`: the diffs.
- ADR-0032 1-40, ADR-0034 1-20, ADR-0035 1-30, ADR-0029 36-44.
- `docs/WORK_PACKAGES.md` rows S20-300 and S20-430.
- The machine-summary sections `protocol`, `json_bridge`, `cli`, `session_handle_profile` and `complete_root_index_snapshot`.
- `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md` 603-783.
- `evidence/review/verdicts/protocol/ariadne_contract_review_revision_13-f073811.md` (the whole file).

## Evidence checked

**Status of my lane's revision-13 findings:**

Rev13 [P1] version-gate: **CLOSED.** Closure option (b) was delivered, with the code rule unchanged:
- The gate is still `protocol_version >= PROTOCOL_VERSION_V2` (server.rs:2909). Only the docstrings and `pub(crate)` visibility changed.
- SMP1 now scopes `open_summary` to "version 2 and every later selection whose table carries row 201 (version 3)". This appears at :682 (appendix A row 201), :722-725 (scope paragraph), :61-66 (history), :331-332 (v1-table preamble) and :893-896 (appendix D).
- S20-300 revision 5 §5 names the same consumer.
- NATIVE_TEST_ADMISSION revision 6 re-pins appendix D to "`docs/spec/SMP1.md` at revision 14" and says row 201 carries `open_summary` under v3 (:3-10, :547-550).
- The new test `server_tests.rs:7468` `workspace_open_v3_answers_the_version_2_open_summary` passes. Under a v3 selection:
  - a cold open equals `revision.read`;
  - a warm open has count 9, fields 1-8 byte-equal, field 9 as tag 9 / length 32 / the materialized snapshot identity, and bounds 1/0;
  - a non-empty body gets `PROTOCOL_PAYLOAD_INVALID`.
- The consumer rationale sentences now say "version 2 and every later selection".

Rev13 [P3] spec-precision (v1 byte-for-byte overclaim): **CLOSED.** The requested restatement is present in:
- SMP1:70-75, :333-335 and :893-896;
- ADR-0032:12-18;
- `protocol.status_note`.

A separate inexactness in the new "one version 1 observable change" sentence is raised below as a new P3.

Rev13 [P3] re-pin: **CLOSED.**
- Bridge revision 11 (:3, :31-35, :44).
- CLI revision 9 (:3, :17-21), with the authority sentence at :29-31 now naming SMP1 14 and bridge 11.
- Session profile revision 5 (:3, :11-16), with :18-19 and :24 now naming SMP1 14.
- The pin-only paragraphs are removed.
- The CLI checker anchors the authority sentence (`check_cli_contract.py:151-175`). Three `CompositionSentenceCases` pass, including refusals of a stale SMP1 pin and a stale bridge pin.
- Each owner's `current_delta_review` is bound to its new revision with all lanes PENDING. Statuses are `S20_420/430/330_IMPLEMENTED_REVIEW_PENDING`.

The remaining record gaps are the new P4 below.

Rev13 [P3] evidence (stale fuzz proofs): **CLOSED.** Both SMP1-family slice checkers exit 0. The proofs are at `46118639`, which contains `dda51a16` and `8357c243`, and no lane path has changed since then.

Rev13 [P3] stale-spec (entrypoints admit 1-2 only): **CLOSED.** The named lines now admit version 3:
- SMP1:133, "1, 2, or 3".
- SMP1:221-227, `negotiate_versioned`, with version 3 defined by NATIVE_TEST_ADMISSION appendices C/D.
- SMP1:256-259, the frame entrypoints "admit only selections 1, 2, and 3". This is anchored as `entrypoints-admit-version-3`, with a revert case.

These match lib.rs:806-814, 1063-1074 and 1328-1331. A residual sentence in the same paragraph is raised below as a new P3.

Rev13 [P4] precision (non-waiting overstated): **CLOSED.**
- SMP1:739-745 drops "absent" from the probe-absence list. It states that the opener keeps the S20-390 blocking shared acquisition, that an absent boundary fails the method, and that only the probe adds no wait.
- Test `server_tests.rs:7543` passes: with an exclusive owner, the probe returns None within 5 s, and a corrupted record gives an 8-field body with its bytes unchanged.

Rev13 [P4] owner-column: **CLOSED.** SMP1:335-338 records the row 201 owner-cell exception and the frozen `methods.json` reason.

Rev13 [P4] test-coverage: **CLOSED.**
- `WORKSPACE_OPEN_ANCHORS` has 9 anchors plus the v3-owner pin (`check_smp1_contract.py:56-85`, called at :330), matched on whitespace-flattened text.
- `test_smp1_contract.py:287-318` has revert cases for 6 of the anchors and for a stale v3 owner pin.
- Three anchors (`optional-by-omission`, `open-summary-scope`, `version-1-compat`) have no dedicated revert case, but the same `text not in flat` test enforces them. This is a note only.

**Other checks on revision 14:**
- **Optional-field notation.** `[9: IndexSnapshotId]` with the optional-by-omission convention (SMP1:665-667, 716-719) matches the wire. The v2 and v3 tests show count 8 when cold and 9 when warm, with no option union.
- **Pointer, not evidence (SMP1:732-738).** `query_root` binds through `run_root_query` over the cache (server.rs:2686-2700). `capsule` always uses `run_root_query_fresh` (server.rs:2755-2772). The text is accurate.
- **Empty-body rows (SMP1:755-760).** The server ignores a body for 102 (`session_close(session)`), 103, 104, 210 `exchange_export()`, 214 `refs_recover()` and 504 `recovery()` (server.rs:1242-1275). Only 201 refuses (server.rs:2905-2907). The list is exact.
- **Determinism clause (SMP1:747-750).** It is consistent with §7 :530 and appendix B :794, where "equal repository state" is now defined to include derived cache state for this body.
- **Composition pins.** SMP1:21-23 (bridge 11, CLI 9), NATIVE_TEST_ADMISSION, ENTITY_READ_PROFILE_V2 amendment (:40-46, :76-77), ERROR_CODES (S20-330 revision 5; S20-300 probe reader) and the machine summary (`contract_revision` 14, all lanes PENDING, `contract_complete` false) all agree.

## Findings

[P3] [spec-precision] docs/spec/SMP1.md:70-75,227-230 vs crates/sley-protocol/src/lib.rs:1075-1088,3554-3563; docs/spec/NATIVE_TEST_ADMISSION_V1.md:360 - SMP1 §2 still says that under a version 1 selection the explicit path "filters exactly the two version-2 tags 306 and 307; unrelated opaque unknown numeric tags keep their legacy treatment". Revision 14's new sentence (anchored as `version-1-compat`) calls the non-empty 201 refusal "the one version 1 observable change since revision 12". Neither is exact. Since `116d031e` (2026-09-16), `negotiate_versioned` also removes the opaque tags 605/606/607 under a v1 selection, and under a v2 selection too, while keeping opaque 999. The passing test lib.rs:3554-3563 asserts `v1 drops [306, 307, 605, 606, 607]`. At `116d031e^` the function removed only 306 and 307. For hellos listing 605-607 as numeric tags, this changes the selected `methods`, and with it `selected_profile_preimage`, `ProtocolHandshakeId` and the `session.capabilities` (103) bytes. A peer that derives the selection from SMP1's text would disagree and fail `session.open` with `PROTOCOL_DOWNGRADE`. NATIVE_TEST_ADMISSION appendix C ("605–607 unknown there") read with SMP1's opaque-tag rule also implies the tags are kept. Scope is limited, because a conforming v1/v2 hello lists only dispatched tags (SMP1:167-168). - Closure evidence: either SMP1 §2 states the explicit-path filter per selection, or the code keeps 605-607 opaque under v1/v2. The per-selection statement would be:
  - v1 removes 306, 307 and 605-607;
  - v2 removes 605-607;
  - v3 without the native bit removes the native tags per NATIVE_TEST_ADMISSION appendix C;
  - the v1 compatibility sentence counts this as a second observable change since revision 12 (`116d031e`).

  In either case, add a checker anchor with a revert case.

[P4] [record] docs/adr/ADR-0034-json-bridge-boundary.md:3-4; docs/adr/ADR-0035-thin-cli-boundary.md:3-4; docs/spec/SESSION_HANDLE_PROFILE_V1.md:360-394; docs/spec/SLEY_CLI_V1.md:344-359; docs/WORK_PACKAGES.md:49 - The consumer re-pins went through revisions as ruled, but their records are incomplete:
  - ADR-0034 still says the bridge contract "is a draft at revision 10", and ADR-0035 says the CLI contract "is a draft at revision 8". Neither has a revision 11 / revision 9 record, although the WORK_PACKAGES S20-430 row attributes the revision 9 re-pin to "ADR-0035". ADR-0033 was updated.
  - The session profile's §9 "Revision history" ends at revision 4.
  - CLI §8 has no "Revision 9" record.

  The status lines carry the content, and no checker covers these places (the `ADR_MARKERS` in `check_cli_contract.py` did not move). - Closure evidence: revision 11 / revision 9 records in ADR-0034/0035 with their current-draft lines corrected, a revision 5 entry in session §9, and a CLI §8 revision 9 record. Optionally, an ADR-0035 marker in `check_cli_contract.py`.

## Assessment

SMP1 revision 14 answers every finding of my revision-13 review, and I verified each closure against code, tests, checkers and records rather than the gate record's claims:
- The version rule is now one statement across SMP1, S20-300 revision 5, NATIVE_TEST_ADMISSION revision 6 (v3 owner re-pinned to SMP1 revision 14) and the consumer contracts: `open_summary` under version 2 and every later selection whose table carries row 201.
- That statement matches the unchanged `>= PROTOCOL_VERSION_V2` gate.
- A version 3 server test checks the cold 8-field and warm 9-field bodies and the non-empty-body refusal.
- The version 1 statement is restricted to conforming requests.
- The negotiation and frame-entrypoint text names version 3.
- The optional-field notation, the pointer-not-evidence scope, the blocking head load and the determinism scope are stated.
- Checker anchors with revert tests pin the new text.
- The SMP1-family fuzz proofs are fresh.
- Every SMP1-family checker, vector checker, generator check, Python suite and the targeted Rust tests pass at HEAD.

Two residual items remain, neither blocking:
- **P3.** SMP1 §2's "filters exactly 306 and 307" sentence predates version 3, and so does revision 14's "one version 1 observable change" sentence. `negotiate_versioned` has also removed opaque 605-607 under v1 and v2 selections since `116d031e`. This is a narrow handshake-derivation disagreement that the text should state.
- **P4.** The consumer ADRs and revision-history sections do not yet record the new consumer revisions.

The consumer contracts' own new-delta reviews (bridge 11, CLI 9, session 5) and the S20-300 revision 5 reviews remain pending in their packages. This verdict does not cover them. I make no GA, release-readiness or package-completion claim.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_1_P4_PRIOR_P3_P4_CLOSED
SECTION: protocol
FIELD: ariadne_contract_review_revision_14
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P3] [spec-precision] docs/spec/SMP1.md:70-75,227-230 vs crates/sley-protocol/src/lib.rs:1075-1088,3554-3563; docs/spec/NATIVE_TEST_ADMISSION_V1.md:360 - SMP1 §2 says a v1 selection on the explicit path "filters exactly the two version-2 tags 306 and 307" and opaque unknown tags keep legacy treatment, and revision 14 calls the non-empty 201 refusal "the one version 1 observable change since revision 12"; since 116d031e (2026-09-16) negotiate_versioned also removes opaque 605/606/607 under v1 (and v2) selections while keeping 999 (asserted by lib.rs:3554-3563; 116d031e^ removed only 306/307), which changes the selected methods, the handshake identity and the session.capabilities bytes for such hellos - state the per-selection explicit-path filter (v1: 306, 307, 605-607; v2: 605-607; v3 without the bit: native tags per NATIVE_TEST_ADMISSION appendix C) and count it in the v1 compatibility sentence, or keep 605-607 opaque in code; add an anchor with a revert case | [P4] [record] docs/adr/ADR-0034-json-bridge-boundary.md:3-4; docs/adr/ADR-0035-thin-cli-boundary.md:3-4; docs/spec/SESSION_HANDLE_PROFILE_V1.md:360-394; docs/spec/SLEY_CLI_V1.md:344-359; docs/WORK_PACKAGES.md:49 - ADR-0034/0035 still say bridge "draft at revision 10" and CLI "draft at revision 8" with no revision 11/9 record (WORK_PACKAGES attributes the CLI r9 re-pin to ADR-0035); session §9 history ends at revision 4; CLI §8 has no revision 9 record; no checker covers these places - add the ADR records and corrected current-draft lines, the session §9 revision 5 entry and the CLI §8 revision 9 record
SUMMARY: SMP1 revision 14 closes every finding of my revision-13 review: open_summary is now specified for version 2 and every later selection carrying row 201 (the version 3 union), consistently across SMP1, S20-300 r5, NATIVE_TEST_ADMISSION r6 and the consumer re-pins; it matches the unchanged >= V2 gate and is proven by a new v3 server test; and the v1 statement, the entrypoint text, the notation, the probe scope and the checker anchors are corrected, with every SMP1-family checker, vector, fuzz-slice and targeted test passing at HEAD. Two non-blocking residuals remain: the negotiation filter sentence and the "one version 1 observable change" claim are inexact because negotiate_versioned has also dropped opaque 605-607 under v1/v2 since 116d031e (P3), and the consumer ADR and history records lag the new bridge/CLI/session revisions (P4). SMP1 revision 14 is accepted by the protocol contract lane.

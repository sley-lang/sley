<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: nabu; field: nabu_architecture_review_revision_14; dispatched: 2026-09-23T12:02:22Z; duration_s: 427; process_exit_code: 0 -->
# Nabu Council review — protocol

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

This is what I checked for the review. I made no edits to the tree.

**Scope and history**
- `git rev-parse HEAD` returned 2b0f1c9f4940c020565137891a49a3769cd02061, which matches the scope SHA.
- `git status --short` shows three untracked verdict files from other lanes. I did not read them, so this review is independent of them.
- `git log --oneline f0738119..HEAD` lists 14 commits. The protocol work is in 8357c243 (SMP1 revision 14), 40a84a5c (S20-300 revision 5), 50fca15f (entrypoints anchor), 46118639 / c2664f10 (fuzz lock and proofs) and d307a22f (scratch-workspace leak fix).
- `git diff --stat f0738119..HEAD -- docs/spec crates/sley-protocol scripts/` shows 20 files, +730/−125. I read the full diffs of:
  - `docs/spec/SMP1.md`
  - `crates/sley-protocol/`
  - the NATIVE_TEST_ADMISSION, CLI, bridge, session-handle, ERROR_CODES, ENTITY_READ, trial-runner and S20-300 specs
  - `scripts/check_smp1_contract.py`, `check_cli_contract.py`, `check_session_handle_profile.py`, `check_smp1_json_bridge_contract.py`, and their `test_*.py` files
- `git diff ab42a3a9..HEAD -- crates/sley-protocol/src/server.rs lib.rs`, filtered to non-doc lines, has two behavioural edits since the revision-12 base: the 201 body refusal and the ≥V2 probe.
- `git diff --name-only 46118639 HEAD -- crates/ fuzz/ Cargo.lock` is empty.

**Contract checkers (all exit 0)**

| Checker | Result |
|---|---|
| `check_smp1_contract.py` | PASS, revision 14, method_count 41, status `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING` |
| `check_smp1_json_bridge_contract.py` | PASS, revision 11 |
| `check_cli_contract.py` | PASS, revision 9 |
| `check_session_handle_profile.py` | PASS, revision 5, smp1_revision 14 |
| `check_complete_root_index_snapshot_profile.py` | PASS, revision 5 |
| `check_native_test_vectors.py` | "6 requests, 6 responses, 7 rejections OK", both bare and under uv |
| `check_smp1_persistent_fuzz_slice.py` | PASS |
| `check_smp1_json_bridge_persistent_fuzz_slice.py` | PASS |

**Vector oracles**
- Under bare python3, `check_smp1_vector.py`, `check_smp1_json_bridge_vector.py` and `check_complete_root_index_snapshot_vector.py` exit 1 with `ModuleNotFoundError: blake3`. That is an environment gap.
- Run through `uv run --offline --frozen --project oracle/scb1`, all three exit 0:
  - SMP1: PASS, frames 3, mutations 4.
  - Bridge: PASS, methods 41, rejections 36, vectors 5.
  - S20-300: PASS, vectors 1, mutations 6.
- `generate_smp1_json_bridge_table.py --check --protocol-version 1|2|3` exits 0 and PASSes for all three.

**Unit tests (all exit 0)**
- `python3 -m unittest`:
  - `test_smp1_contract`: ran 16, OK. This includes `WorkspaceOpenAnchorCases`.
  - `test_cli_contract`: ran 10, OK. This includes `CompositionSentenceCases`.
  - `test_session_handle_profile`: ran 6, OK.
  - `test_complete_root_index_snapshot_profile`: ran 13, OK.
- `test_smp1_json_bridge_table.py`: 17 cases PASS.
- `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 4 passed, 0 failed:
  - `…under_version_1_never_carries_field_9`
  - `…v2_discloses_only_the_materialized_head_snapshot`
  - `…v3_answers_the_version_2_open_summary`
  - `…probe_is_non_waiting_and_never_rewrites_a_discarded_record`
- `cargo test --locked --offline -p sley-repo --lib -- probe fifo symlink_to_a_valid non_canonical`: 6 passed. Four are the S20-300 probe, FIFO, symlink and covers tests; the other two only match the name filter.
- `python3 -m unittest bench/live/tests/test_scratch_cleanup.py`: ran 7, OK. One is skipped because `SLEY2_SLEY_BINARY` is unbound. `/tmp` inode use is at 10%.

**Files read (line ranges)**
- `docs/spec/SMP1.md`: 1-83, 214-263, 327-348, 344-377, 661-760, 890-897
- `crates/sley-protocol/src/server.rs`: 2450-2464, 2888-2942, 3280-3319, and the dispatch arms at 1242-1283
- `crates/sley-protocol/src/lib.rs`: 40-52, 1063-1100, 1328-1340
- `crates/sley-protocol/src/server_tests.rs`: 607-615, 7383-7585, 4100-4179
- `crates/sley-txn/src/maintenance.rs`: 40-199
- `crates/sley-txn/src/repository.rs`: 878-897
- `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`: 1-20, 42-46, 115-126, 175-207, 255-276
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: 1-20, 360, 540-560
- `docs/spec/ENTITY_READ_PROFILE_V2.md`: 1-12, 37-80
- `docs/spec/SLEY_CLI_V1.md`: 1-31, 296-361
- `docs/spec/SMP1_JSON_BRIDGE_V1.md`: 1-50
- `docs/spec/SESSION_HANDLE_PROFILE_V1.md`: 1-27, and the history section from 360
- `scripts/check_smp1_contract.py`: 50-86, 290-429
- `scripts/generate_smp1_json_bridge_table.py`: 20-32
- `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: 603-772
- `machineresearch/sley-2.0/machine-summary.json`: the `protocol` status, `contract_revision` = 14, `current_delta_review` = {14, PENDING×3}, and `status_note`
- `evidence/review/verdicts/protocol/nabu_architecture_review_revision_13-f073811.md`: all

## Evidence checked

**Version rule: code and spec now agree.**
- `workspace_open` (`server.rs:2904-2916`) refuses a non-empty body under every version. It then loads the head, and probes only when `protocol_version >= PROTOCOL_VERSION_V2`.
- `negotiate_versioned` (`lib.rs:1069-1074`) and `validate_for_version` (`lib.rs:1329-1334`) admit exactly versions 1, 2 and 3. So `>= V2` can only reach versions 2 and 3.
- SMP1 revision 14 states the same rule in five places:
  - the history (:57-83)
  - row 682 of appendix A
  - the `open_summary` scope paragraph (:722-727)
  - appendix D (:890-897)
  - the negotiation and entrypoint text (:219-227, :255-260)
- S20-300 section 5 (:190-200) names the consumer for version 2 and the version 3 union.
- The version 3 server test checks three things, and passes:
  - cold: the body equals `revision.read`
  - warm: fields 1-8 equal `revision.read`, `[9,32]`, and the snapshot equals the one `query.root` materialized
  - `[0]`: `PAYLOAD_INVALID`

**Version 3 owner pin.**
- NATIVE_TEST_ADMISSION revision 6 re-pins appendix D (:548-550) and its status (:3-10) to SMP1 revision 14, and states the effect on version 3.
- `workspace_open_anchor_problems` (`check_smp1_contract.py:77-85`) requires exactly one "`docs/spec/SMP1.md` at revision N" in the native spec, with N = `CONTRACT_REVISION`. `CONTRACT_REVISION` is itself checked against SMP1's Status line (:413-422).
- `test_stale_version_3_owner_pin_is_refused` shows a pin back to revision 12 is refused.

**Version 1 compatibility.**
- SMP1 :70-75 and :330-334 limit the byte-for-byte claim to conforming requests with an empty body, and name the refusal of a non-empty 201 body under every version as the only version 1 change.
- The base-to-HEAD diff confirms no other version 1 behaviour changed.
- The empty-body note (:755-760) records that rows 102, 103, 104, 210, 214 and 504 still ignore a body. The dispatch arms at `server.rs:1242-1275` confirm this.
- The bridge, CLI and session-handle re-pins and ENTITY_READ §2 (:40-46, :75-77) carry the same corrected statement.

**Composition and ownership.**
- `open_summary` is still a composition, not a new SMP1 semantics:
  - `head_open_summary` reuses `revision_summary_fields` (:3292-3301).
  - `revision.read` keeps its own eight-field encoder.
  - The field-9 source is the S20-300 probe.
- The probe fails closed:
  - It takes the boundary with non-blocking shared acquisition and never initialises it (`maintenance.rs:142-146, 153-161, 179-186`: it opens without create).
  - `.ok()?` / `.ok().flatten()` turn any failure into absence.
- The blocking head load needs an existing boundary (`maintenance.rs:107-111`; `repository.rs:883`). So SMP1's new statement (:741-744) is true: "an absent boundary fails the method and an exclusive owner makes the opener wait".
- SMP1 :732-739 now calls field 9 "a pointer, not evidence", and keeps the `QUERY_SNAPSHOT_MISMATCH` cross-check through a fresh `capsule`. That keeps authority with the build path.

**Grammar.** The appendix A preamble (:665-667) defines `[n: T]` as optional by omission, and `open_summary` uses it (:716-719). It cannot be confused with the `X[32]` fixed-length suffix, which appears elsewhere without a colon.

**Consumer re-pins.**
- The bridge (revision 11), CLI (revision 9) and session handle (revision 5) each re-pin through their own revision.
- The CLI composition sentence (:28-31) now pins SMP1 revision 14 and bridge revision 11. It is regex-anchored by `composition_pin_problems`, and negative tests cover stale SMP1 and stale bridge pins.
- The stale revision-8 history bullet was removed. The CLI history line 359 ("SMP1 revision 12 and bridge revision 10") correctly records revision 8.
- The session-handle `SMP1_PIN` is line-anchored and cross-checked against the SMP1 Status line.

**Evidence freshness.**
- Both SMP1-family fuzz slices PASS at source 46118639. No crate, fuzz or lockfile path changed after that commit.
- The frozen v1 frame and bridge vectors and the v1/v2/v3 bridge tables all still verify.

**Determinism and non-waiting.**
- SMP1 :747-751 says the body is deterministic given the accepted head plus the derived cache state, and that the same qualifier applies to appendix B and section 10.
- The new probe test (`server_tests.rs:7543-7585`) holds an exclusive guard on a separate file descriptor and gets `None` back from `materialized_head_snapshot` promptly. It then shows that a corrupted record gives an eight-field `workspace.open` with the cache bytes unchanged.
- The test calls the probe wrapper directly, which is correct: `workspace.open` itself would wait under an exclusive owner, as SMP1 now says. But see finding 2.

**Revision-13 findings from my lane:**
- [P1] version-gate/spec-code-divergence — CLOSED. The spec follows the `>= V2` gate for version 2 and the version 3 union, S20-300 §5 is amended, and there is a version 3 cold/warm/body server test.
- [P2] stale-authority-pin — CLOSED. NATIVE_TEST_ADMISSION revision 6 pins SMP1 revision 14, the checker binds the pin to the SMP1 Status line, and a refusal test exists. The generator comment is a residual, filed as finding 3.
- [P3] incomplete-repin (CLI) — CLOSED. The composition sentence pins revisions 14 and 11, the note is outside the revision-8 history, and the anchor has negative tests.
- [P3] frozen-v1-overclaim — CLOSED. SMP1 and every consumer sentence now state the conforming-request limit and the refusal under every version, and the other empty-request rows are recorded.
- [P3] grammar-convention — CLOSED. The `[n: T]` optional-by-omission convention is defined and used.
- [P3] evidence-binding/fuzz-proof-stale — CLOSED. Both slice checkers exit 0 at source 46118639, and no lane path has changed since.
- [P4] test-gap/determinism-note — CLOSED. There is an exclusive-guard probe test, a discarded-record no-write-back test, the determinism qualifier, and the statement that the opener waits.

## Findings

[P3] [spec-code-divergence/stale-exactness] docs/spec/SMP1.md:227-230; crates/sley-protocol/src/lib.rs:1075-1088; docs/spec/NATIVE_TEST_ADMISSION_V1.md:360 - Revision 14 rewrote this negotiation paragraph to admit version 3, but kept "Under selected version 1 the intersection filters exactly the two version-2 tags 306 and 307". In the code, `negotiate_versioned` also removes the native tags 605, 606 and 607 under version 1, and removes 605-607 under version 2. The native owner says 605-607 are "unknown there". Both peers must re-derive the same selection, and the handshake identity digests it. So a client built from SMP1 §2 alone that offers a native tag without version 3 would derive a different method list, and `session.open` would fail with `PROTOCOL_DOWNGRADE`. - Closure evidence: §2 names every per-version filter: 306/307 under version 1, and the version 3 native rows 605-607 under versions 1 and 2, citing NATIVE_TEST_ADMISSION_V1 appendix D. Add a `WORKSPACE_OPEN_ANCHORS`-style anchor with a revert case.

[P3] [cross-contract-contradiction/evidence-overclaim] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:259-262; docs/spec/SMP1.md:741-745; crates/sley-protocol/src/server_tests.rs:7567-7570; crates/sley-txn/src/repository.rs:883 - The S20-300 revision 5 acceptance list requires "through `workspace.open`, absence while an exclusive maintenance owner holds the boundary". That cannot be satisfied. SMP1 revision 14 says an exclusive owner makes the opener wait, and `accepted_head()` blocks on `lock_shared`. The test that closes it calls the internal `materialized_head_snapshot` directly (made `pub(crate)` for this), not `workspace.open`. No checker anchors the S20-300 sentence. - Closure evidence: reword the S20-300 acceptance item to "the `workspace.open` probe consumer (`materialized_head_snapshot`) answers absence at once while an exclusive owner holds the boundary; the opener itself waits (SMP1 appendix A)", so the two contracts and the test agree.

[P4] [stale-comment] scripts/generate_smp1_json_bridge_table.py:26-30 - The generator's authority comment still says "SMP1.md stays revision 13 while the native contract owns the five rows". This is the same stale-pin surface named in revision-13 P2; the normative pins were all moved to 14. - Closure evidence: the comment names SMP1 revision 14, or drops the revision number and points at the SMP1 Status line.

[P4] [evidence-binding/status-scope] docs/spec/ENTITY_READ_PROFILE_V2.md:3-11,40-46,75-77; docs/spec/NATIVE_TEST_ADMISSION_V1.md:3-4; docs/spec/SESSION_HANDLE_PROFILE_V1.md:360-; docs/spec/SLEY_CLI_V1.md:344-359 - ENTITY_READ's status still reads REVIEWED_IMPLEMENTATION_CONTRACT ("reviews passed on 48070373"). It says nothing about the 2026-09-23 in-place amendment that qualifies its normative rule that version 1 keeps its existing rejection behaviour. The revision 6 status of NATIVE_TEST_ADMISSION keeps an unscoped "Independent architecture review passed", which reads as covering the revision 6 re-pin. The session-handle §9 history and the CLI §8 history stop at revisions 4 and 8; revisions 5 and 9 appear only in their status paragraphs. - Closure evidence: each status line scopes its past review to the revision or commit it covered, and flags the 2026-09-23 amendment or re-pin as covered only by the pending SMP1 revision-14 round. The session and CLI history sections get revision-5 and revision-9 entries.

## Assessment

Revision 14 closes every finding my lane raised on revision 13. The version rule has one statement, in SMP1 appendix A and D and the history. It is mirrored in the S20-300 consumer clause and the version 3 owner's pin, matches the unchanged `>= V2` gate, and is exercised under version 3 by a new server test.

Authority is placed correctly. S20-390 owns fields 1-8 byte for byte, S20-300 owns the fail-closed probe behind field 9, and NATIVE_TEST_ADMISSION owns version 3 and now pins SMP1 revision 14 under a checker that follows the SMP1 Status line. SMP1 no longer implies that field 9 is evidence. Consumers re-pin through their own revisions. The CLI composition sentence is anchored. The version-1 claim is limited to conforming requests, and the deviation in the other empty-request rows is recorded rather than hidden.

Evidence is fresh: every SMP1-family checker, the oracle vectors (under uv), the bridge tables, both fuzz slices, and the four `workspace_open` tests pass at the scope head.

The two P3s are precision defects outside the new version rule:
- The pre-existing "filters exactly 306 and 307" sentence sits in the paragraph revision 14 rewrote, and it is now contradicted by the native-tag filtering.
- The S20-300 acceptance wording claims an end-to-end `workspace.open` contention test that SMP1 itself rules out.

Neither changes wire behaviour, ownership, or the fail-closed structure, so my lane accepts SMP1 revision 14. This verdict makes no claim about the trial-runner lane, the bench/live scratch fix beyond the unit run above, GA, or release readiness.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_2_P4_PRIOR_P3_P4_CLOSED
SECTION: protocol
FIELD: nabu_architecture_review_revision_14
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P3] [spec-code-divergence/stale-exactness] docs/spec/SMP1.md:227-230; crates/sley-protocol/src/lib.rs:1075-1088; docs/spec/NATIVE_TEST_ADMISSION_V1.md:360 - the rewritten negotiation paragraph still says the v1 intersection filters "exactly" 306 and 307, while negotiate_versioned also drops the native tags 605-607 under v1 (and under v2), which the native owner calls unknown there; a client re-deriving the selection from SMP1 alone would diverge and fail session.open - name every per-version filter with a NATIVE appendix D citation, plus an anchor with a revert case | [P3] [cross-contract-contradiction/evidence-overclaim] docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:259-262; docs/spec/SMP1.md:741-745; crates/sley-protocol/src/server_tests.rs:7567-7570; crates/sley-txn/src/repository.rs:883 - the S20-300 rev-5 acceptance item requires "through workspace.open, absence while an exclusive owner holds the boundary", which SMP1 rev 14 rules out (the opener waits on the blocking head load); the closing test calls materialized_head_snapshot directly - reword the acceptance item to the probe consumer and state that the opener waits | [P4] [stale-comment] scripts/generate_smp1_json_bridge_table.py:26-30 - the authority comment still says "SMP1.md stays revision 13" - name revision 14 or point at the Status line | [P4] [evidence-binding/status-scope] docs/spec/ENTITY_READ_PROFILE_V2.md:3-11,40-46,75-77; docs/spec/NATIVE_TEST_ADMISSION_V1.md:3-4; docs/spec/SESSION_HANDLE_PROFILE_V1.md:360-; docs/spec/SLEY_CLI_V1.md:344-359 - the ENTITY_READ "reviewed" status does not flag its 2026-09-23 in-place amendment, the NATIVE rev-6 status keeps an unscoped "architecture review passed", and the session and CLI history sections lack revision 5 and 9 entries - scope each status to its reviewed revision or commit, flag the amendment and re-pin as covered by the pending SMP1 rev-14 round, and add the history entries
SUMMARY: SMP1 revision 14 closes all seven Nabu revision-13 findings. Field 9 is specified for version 2 and the version 3 union, matching the `>= V2` gate. It is tested under version 3, the NATIVE_TEST_ADMISSION revision 6 pin is bound to the SMP1 Status line, the version 1 claim is exact, and the grammar, determinism and non-waiting wording are corrected; every SMP1-family checker, oracle vector, bridge table, fuzz slice and workspace_open test passes at 2b0f1c9f. Ownership is sound: S20-390 owns fields 1-8, the fail-closed S20-300 probe supplies field 9 as a pointer rather than evidence, and version 3 stays with the native owner. Two P3 precision defects remain: the §2 "filters exactly 306 and 307" sentence and the S20-300 end-to-end contention acceptance wording. There are also two P4 notes on a stale generator comment and on status/history scoping.

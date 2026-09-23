<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: vulcan; field: vulcan_surface_review_revision_14; dispatched: 2026-09-23T12:12:48Z; duration_s: 453; process_exit_code: 0 -->
# Vulcan Council review — protocol

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, which matches the scope SHA. Branch `work/succ-context-impl`. The worktree is clean apart from untracked verdict transcripts that other lanes filed during this run. I did not read those other lanes' revision-14 transcripts. The only prior transcript I read is my own lane's `evidence/review/verdicts/protocol/vulcan_surface_review_revision_13-f073811.md`.
- **Git commands run.**
  - `git log --oneline f0738119..HEAD` (13 commits).
  - `git diff --stat f0738119..HEAD -- docs/spec crates/sley-protocol scripts/` (20 files, +730/−125).
  - Full diffs of `docs/spec/SMP1.md`, `crates/sley-protocol`, NATIVE_TEST_ADMISSION_V1, ENTITY_READ_PROFILE_V2, ERROR_CODES_V1, SESSION_HANDLE_PROFILE_V1, SLEY2_TRIAL_RUNNER_V1, SLEY_CLI_V1, SMP1_JSON_BRIDGE_V1, COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1, ADR-0032, and the checker and test scripts.
  - `git show` of row 201 at `ab42a3a9`, `f0738119` and HEAD.
  - `git diff --stat ab42a3a9..HEAD -- conformance`: empty.
  - `git log -L` over the `negotiate_versioned` filter.
  - `git blame docs/spec/SMP1.md -L227,229`.
- **Checkers run** (python3 from the worktree; vector oracles via `uv run --offline --frozen --project oracle/scb1`):

  | Checker | Exit | Result |
  |---|---|---|
  | `check_smp1_contract.py` | 0 | PASS: revision 14, 41 methods, status `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING` |
  | `check_smp1_json_bridge_contract.py` | 0 | PASS: revision 11 |
  | `check_cli_contract.py` | 0 | PASS: revision 9 |
  | `check_session_handle_profile.py` | 0 | PASS: revision 5, `smp1_revision` 14 |
  | `check_complete_root_index_snapshot_profile.py` | 0 | PASS: revision 5 |
  | `check_root_backed_query_profile.py` | 0 | PASS: revision 7 |
  | `check_native_test_vectors.py` | 0 | "6 requests, 6 responses, 7 rejections OK" |
  | `check_smp1_vector.py` | 0 | PASS: frames 3, mutations 4 |
  | `check_smp1_json_bridge_vector.py` | 0 | PASS: methods 41, vectors 5, rejections 36 |
  | `check_entity_read_vectors.py` | 0 | PASS: cases 23, rejections 91 |
  | `generate_smp1_json_bridge_table.py --check --protocol-version 1` / `2` / `3` | 0 each | PASS (no drift) |

- **Tests run.**

  | Command | Result |
  |---|---|
  | `python3 -m unittest scripts.test_smp1_contract` | Ran 16, OK |
  | `scripts.test_session_handle_profile` | Ran 6, OK |
  | `scripts.test_cli_contract` | Ran 10, OK |
  | `scripts.test_current_contract_review` | Ran 6, OK |
  | `scripts.test_complete_root_index_snapshot_profile` | Ran 13, OK |
  | `python3 scripts/test_smp1_json_bridge_table.py` (script-style) | 17 cases PASS |
  | `cargo test --locked --offline -p sley-protocol --lib workspace_open` | 4 passed: v1, v2, v3, probe-obligations |
  | `cargo test --locked --offline -p sley-protocol --lib` | 121 passed, 0 failed, 4 ignored |

- **Files read.**
  - `docs/spec/SMP1.md`: 1–83, 210–275, 296–420, 524–534, 640–650, 660–760, 790–798, 885–900.
  - `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: 1–10, 355–366, 545–577.
  - `docs/spec/ENTITY_READ_PROFILE_V2.md`: 1–80.
  - `docs/spec/SLEY_CLI_V1.md`: 1–32, 325–362.
  - `docs/spec/SMP1_JSON_BRIDGE_V1.md`: 1–50.
  - `docs/spec/SESSION_HANDLE_PROFILE_V1.md`: 1–30.
  - `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`: diff hunks 1–50, 107–127, 175–210, 250–280.
  - `crates/sley-protocol/src/server.rs`: 1236–1290, 2445–2460, 2755–2795, 2880–2935.
  - `crates/sley-protocol/src/lib.rs`: 10–21, 30–52, 862–888, 1055–1100, 1325–1345, 1420–1430, 1750–1770, 3476–3575.
  - `crates/sley-protocol/src/server_tests.rs`: 608–616, 7383–7585.
  - `crates/sley-txn/src/maintenance.rs`: 95–230.
  - `crates/sley-txn/src/repository.rs`: 881–906.
  - `scripts/check_complete_root_index_snapshot_profile.py`: 89–167, 251–284.
  - `scripts/generate_smp1_json_bridge_table.py`: 295–330.
  - `docs/adr/ADR-0032-smp1-transport-boundary.md`: 1–30.
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: 603–745.
  - `machineresearch/sley-2.0/machine-summary.json`: the `protocol` section, loaded with Python.
  - `conformance/smp1-json-bridge/v{1,2,3}/methods.json`: row 201.

## Evidence checked

**Status of my lane's revision-13 findings**

- Revision-13 finding 1, [P2] version-gate, server.rs:2907 — CLOSED.
  - The code is unchanged, and the contracts now follow it: `workspace_open` still gates on `>= PROTOCOL_VERSION_V2` (server.rs:2908-2912).
  - SMP1 states the rule in four places: row 201 (:682), the `open_summary` scope paragraph (:722-726), appendix D (:893-896) and the history (:62-68).
  - NATIVE_TEST_ADMISSION revision 6 re-pins appendix D to SMP1 revision 14 (:6-10, :547-551). ENTITY_READ section 2 carries a dated amendment (:40-46, :76-77). S20-300 §5 names the consumer the same way (:189-193).
  - The bridge (:31-33), CLI (:17-19) and session-handle (:11-13) re-pin sentences now say "version 2 and every later selection".
  - The new `workspace_open_v3_answers_the_version_2_open_summary` (server_tests.rs:7463-7536) passes. It checks that the v3 selection is actually V3, that the cold body equals `revision.read`, and the warm count byte 9, the unchanged fields 1-8, the `[9, 32]` tail, the snapshot id bound by `query.root`, and the refusal of a non-empty body.
  - The checker's `v3-owner-pin` requires exactly one "`docs/spec/SMP1.md` at revision 14" in NATIVE_TEST_ADMISSION. It fails closed on an absent or stale pin, and `test_stale_version_3_owner_pin_is_refused` proves the refusal.
- Revision-13 finding 2, [P3] compat-claim, SMP1.md:62-64,315-318,845-846 — CLOSED.
  - SMP1 :70-75 and :333-336 limit byte identity to conforming (empty-body) v1 requests. They name the one observable change: a non-empty 201 body, previously ignored, is now refused under every version.
  - Appendix D :893-896 is corrected. ADR-0032:12-18 and machine-summary `protocol.status_note` match. ENTITY_READ :76-77 records the exception.
  - The v1 test at server_tests.rs:610-640 still asserts that open bytes equal `revision.read`.
- Revision-13 finding 3, [P3] evidence-integrity, SMP1.md:700-704 — CLOSED.
  - SMP1 :732-738 now says "Field 9 is a pointer, not evidence". It says `capsule` always binds a fresh snapshot, and that a client adopting field 9 keeps the `QUERY_SNAPSHOT_MISMATCH` cross-check.
  - I confirmed this at server.rs:2761-2771: `run_root_query_fresh`, then the preimage is compared with the request body.
- Revision-13 finding 4, [P3] pin, SLEY_CLI_V1.md:26 — CLOSED.
  - The composition sentence (SLEY_CLI_V1.md:29-31) pins SMP1 revision 14 and bridge revision 11.
  - `composition_pin_problems` (check_cli_contract.py:151-175, called at :257) anchors that sentence.
  - `CompositionSentenceCases` refuses both stale pins.
- Revision-13 finding 5, [P4] fail-closed, server.rs:1242-1257,1265-1266,1275 — CLOSED, via the recorded option.
  - SMP1 :755-760 states that rows 102, 103, 104, 210, 214 and 504 still ignore a non-empty body. That is the complete set of "none" request rows other than 201.
  - The dispatch at server.rs:1242-1257, 1265-1266 and 1275 still ignores the body there, as stated.
- Revision-13 finding 6, [P4] determinism, SMP1.md:693-694,746-747,624-625 — CLOSED.
  - Appendix A's conventions define optional-by-omission `[n: T]` fields (:665-668), and the grammar uses `[9: IndexSnapshotId]` (:719).
  - SMP1 :747-750 scopes "equal repository state" in appendix B (:794) and section 9 (:530), and section 10's byte-identity (:645), to include the derived cache state for this one body.

**New revision 14 text, checked against code**

- **Opener wait.** "An absent boundary fails the method and an exclusive owner makes the opener wait; only the probe adds no wait" (:741-746) is accurate:
  - `accepted_head` takes the blocking shared acquisition, and an absent lock directory or file is an error (repository.rs:881-884; maintenance.rs:148-161, 175-176).
  - The probe uses `try_lock_shared` (maintenance.rs:142-146, 177-185), and the new test shows prompt absence under an exclusive guard (server_tests.rs:7565-7568).
- **Frame entrypoints.** "Entrypoints admit only selections 1, 2, and 3" matches the code:
  - `validate_for_version` (lib.rs:1328-1333), which `encode_frame_for_version` calls (:1423-1428).
  - `negotiate_versioned` (lib.rs:1063-1073).
  - "The owner of version 3 since 2026-09-16" matches commit 116d031e (2026-09-16).
- **Owner-cell note.** The frozen bridge `methods.json` rows for 201 carry only the name, family, owner and reserved flag (v1, v2 and v3). No conformance byte changed since `ab42a3a9`, and the table drift checks pass. The owner-cell note at :337-340 is therefore accurate.
- **Checker anchors.**
  - `WORKSPACE_OPEN_ANCHORS` (check_smp1_contract.py:56-74) matches on whitespace-flattened text, and the gate fails on any missing anchor.
  - Six of the nine anchors have explicit revert cases. The other three (optional-by-omission, open-summary-scope, version-1-compat) go through the same presence loop.
  - The anchors are presence-only.

**Negotiation text (finding 1 below)**
- Revision 14 rewrote the section 2 clause about `negotiate_versioned` but left the next sentence unchanged. That sentence dates from 2026-09-08, per `git blame`.
- The code has also stripped 605–607 from version 1 and version 2 selections since 116d031e. I traced this through lib.rs:1075-1089 and the unit test at :3543-3562.

**Bounds and precedence.** The code delta does not change them:
- The body check still precedes the head load (server.rs:2905-2908).
- `counted(summary, 1)` holds either way.
- The probe's read is bounded by `MAX_SNAPSHOT_RECORD_BYTES`.

## Findings

[P3] [spec-code] docs/spec/SMP1.md:227-230 - Section 2 still says a version 1 selection "filters exactly the two version-2 tags 306 and 307" and that "unrelated opaque unknown numeric tags keep their legacy treatment" (they stay in the intersection). But `negotiate_versioned` (crates/sley-protocol/src/lib.rs:1075-1089) also removes 605, 606 and 607 from every version 1 and version 2 selection before the profile hash, and lib.rs:3543-3562 pins that ("v1 strips v2 and v3 tags alike"). A v1-only hello may list 605-607 as opaque unknown tags: `Hello::validate` (lib.rs:876-883) refuses only reserved v1 tags. NATIVE_TEST_ADMISSION appendix C (:360) calls 605-607 "unknown there", and no text says they are stripped at version 1 or version 2. So a peer that implements SMP1 revision 14 literally keeps them, derives a different `SelectedProfile` preimage and `ProtocolHandshakeId`, and fails `session.open` with `PROTOCOL_DOWNGRADE`. The failure is closed, but the contract and the code disagree on the identity preimage. The divergence predates revision 14 (text 2026-09-08, stripping added by 116d031e on 2026-09-16), yet revision 14 rewrote the clause beside it to admit version 3 and left it. - Closure evidence: SMP1 section 2 (or NATIVE appendix C, referenced from SMP1) states that the versioned intersection removes 306, 307, 605, 606 and 607 under version 1, and 605-607 under version 2, with a check_smp1_contract anchor and a revert case.
[P4] [stale-doc] crates/sley-protocol/src/server_tests.rs:610-613 - The docstring was relabelled "SMP1 revision 14" but still says "(field 9 is version 2 only)". That contradicts the revision 14 rule (version 2 and version 3) and the v3 test at :7463-7536. - Closure evidence: the docstring says field 9 applies under version 2 and every later selection carrying row 201.
[P4] [surface] crates/sley-protocol/src/server.rs:2925 - `materialized_head_snapshot` was widened from private to `pub(crate)` only so the sibling `#[cfg(test)] mod server_tests` (lib.rs:17-18) can call it. The S20-300 one-consumer gate (scripts/check_complete_root_index_snapshot_profile.py:120-165) counts only references to `cached_complete_root_snapshot_id`, so a second non-test caller of the wrapper anywhere in sley-protocol would be an unlisted field-9 consumer the gate cannot see. The wrapper keeps its non-waiting, no-write, absence semantics, so only the one-sanctioned-consumer claim is exposed. - Closure evidence: restrict the widened visibility to `cfg(test)` (or a test-only accessor), or have the gate also require exactly one non-test reference to `materialized_head_snapshot(` (inside `workspace_open`), with a negative case.

## Assessment

Revision 14 answers every finding of my lane's revision-13 round.
- **The version rule is consistent across every contract, and tested.** It holds in SMP1, the version 3 owner (NATIVE_TEST_ADMISSION revision 6), ENTITY_READ, S20-300 and the three consumer re-pins, and now has a passing version 3 server test.
- **Version 1 compatibility is stated exactly.** Field 9 is described as a pointer, not evidence, and that matches the fresh-only `capsule` code. The CLI composition pin is fixed and anchored. The other empty-body rows are recorded honestly. Optional-field notation and determinism scope are defined.
- **Verification is clean.** Every SMP1-family and native-test checker exits 0. The vector oracles, table drift checks, script suites and the full sley-protocol lib suite pass, and I found no fail-open path, precedence change or new resource exposure in the delta.

What remains:
- **Negotiation text (P3).** The section 2 sentence claims version 1 filters "exactly" 306 and 307 and leaves other unknown tags alone. The code also strips 605–607 at versions 1 and 2. This is a closed-failure divergence in the handshake-identity preimage, left beside the clause revision 14 rewrote.
- **Two notes (P4).** A stale test docstring, and a crate-wide visibility widening of the probe wrapper that the one-consumer gate cannot see.

None of these blocks acceptance of the revision 14 delta.

Out of scope, not counted (for the S20-300 owner):
- COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md:258-262 lists "through `workspace.open`, absence while an exclusive maintenance owner holds the boundary" as an acceptance test. The test (server_tests.rs:7543-7585) exercises `materialized_head_snapshot` directly, and through `workspace.open` the opener would wait on the head load, as SMP1 :744-746 correctly says.
- ADR-0029:41 still reads "SMP1 revision 13 `workspace.open` field 9 only".

This verdict makes no claim about GA or release readiness.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_2_P4_PRIOR_P2_P3_P4_CLOSED
SECTION: protocol
FIELD: vulcan_surface_review_revision_14
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P3] [spec-code] docs/spec/SMP1.md:227-230 - Section 2 still says a version 1 selection "filters exactly the two version-2 tags 306 and 307" and that "unrelated opaque unknown numeric tags keep their legacy treatment" (they stay in the intersection). But `negotiate_versioned` (crates/sley-protocol/src/lib.rs:1075-1089) also removes 605, 606 and 607 from every version 1 and version 2 selection before the profile hash, and lib.rs:3543-3562 pins that ("v1 strips v2 and v3 tags alike"). A v1-only hello may list 605-607 as opaque unknown tags: `Hello::validate` (lib.rs:876-883) refuses only reserved v1 tags. NATIVE_TEST_ADMISSION appendix C (:360) calls 605-607 "unknown there", and no text says they are stripped at version 1 or version 2. So a peer that implements SMP1 revision 14 literally keeps them, derives a different `SelectedProfile` preimage and `ProtocolHandshakeId`, and fails `session.open` with `PROTOCOL_DOWNGRADE`. The failure is closed, but the contract and the code disagree on the identity preimage. The divergence predates revision 14 (text 2026-09-08, stripping added by 116d031e on 2026-09-16), yet revision 14 rewrote the clause beside it to admit version 3 and left it. - Closure evidence: SMP1 section 2 (or NATIVE appendix C, referenced from SMP1) states that the versioned intersection removes 306, 307, 605, 606 and 607 under version 1, and 605-607 under version 2, with a check_smp1_contract anchor and a revert case. | [P4] [stale-doc] crates/sley-protocol/src/server_tests.rs:610-613 - The docstring was relabelled "SMP1 revision 14" but still says "(field 9 is version 2 only)". That contradicts the revision 14 rule (version 2 and version 3) and the v3 test at :7463-7536. - Closure evidence: the docstring says field 9 applies under version 2 and every later selection carrying row 201. | [P4] [surface] crates/sley-protocol/src/server.rs:2925 - `materialized_head_snapshot` was widened from private to `pub(crate)` only so the sibling `#[cfg(test)] mod server_tests` (lib.rs:17-18) can call it. The S20-300 one-consumer gate (scripts/check_complete_root_index_snapshot_profile.py:120-165) counts only references to `cached_complete_root_snapshot_id`, so a second non-test caller of the wrapper anywhere in sley-protocol would be an unlisted field-9 consumer the gate cannot see. The wrapper keeps its non-waiting, no-write, absence semantics, so only the one-sanctioned-consumer claim is exposed. - Closure evidence: restrict the widened visibility to `cfg(test)` (or a test-only accessor), or have the gate also require exactly one non-test reference to `materialized_head_snapshot(` (inside `workspace_open`), with a negative case.
SUMMARY: SMP1 revision 14 closes all six of my lane's revision-13 findings (1 P2, 3 P3, 2 P4). The field-9 rule now holds for version 2 and version 3 in SMP1, NATIVE_TEST_ADMISSION revision 6, ENTITY_READ, S20-300 and the consumer re-pins, and a passing version 3 server test backs it; the version 1 change, the pointer-not-evidence caveat, the CLI pin, the other empty-body rows and the determinism scope are now stated accurately. Every SMP1-family and native-test checker, vector oracle, table drift check and script suite passes, as does the sley-protocol lib suite (121 passed). What remains: SMP1 section 2 says version 1 filters "exactly" 306 and 307, while the code also strips 605-607 at versions 1 and 2, a closed-failure divergence in the handshake-identity preimage (P3), plus a stale test docstring and a crate-wide visibility widening of the probe wrapper (P4 each); PASS.

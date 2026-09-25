<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: vulcan; field: vulcan_surface_review_revision_6; dispatched: 2026-09-23T13:38:32Z; duration_s: 451; process_exit_code: 0 -->
# Vulcan Council review — session_handle_profile

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

**Scope.** `git rev-parse HEAD` printed `26d050e629b669ef4acbd54826ef4008c05a061d`, which matches the scope SHA. `git status --short` was empty. The branch is work/succ-context-impl.

**Commands and results:**
- `git log --oneline 2b0f1c9f..HEAD`: 12 commits, from 531805ae to 26d050e6.
- `git diff --stat 2b0f1c9f..HEAD` on the owned paths: 8 files, +368/−66. The files are the spec, ADR-0033, the checker and its tests, server.rs, server_tests.rs, WORK_PACKAGES and ERROR_CODES. lib.rs is unchanged.
- I read the full diffs for the spec, ADR, server.rs, server_tests.rs, the checker and its test file. I also read the SMP1 revision 15 diff and grepped it for session-owned text. It has none: rows 100, 101, 102 and 304, section 3 and `max_sessions` are untouched.
- A python3 heredoc compared the `session_handle_profile` section of `machineresearch/sley-2.0/machine-summary.json` at 2b0f1c9f and at HEAD.
  - Changed: `contract_revision` 5→6, `current_delta_review` (revision 6, all PENDING) and `status_note`.
  - Added: the `_revision_5` lane fields and dated notes. `vulcan_surface_review_revision_5` = `REVISE_0_P0_0_P1_1_P2_2_P3_2_P4` matches my transcript.
  - No field was removed. Status is still `S20_330_IMPLEMENTED_REVIEW_PENDING`.
- A python3 heredoc compared the sha256 of my revision 5 transcript with the round index `evidence/review/rounds/context-r7-2b0f1c9.json`. Both are `a97669e4…0a86e`, so the file is intact.
- `git show --stat` on bf5b7e78 (2026-09-14), e0ff1371 (2026-09-16) and a8b4cddb (2026-09-23) confirms the three unrevisioned edits that §9 now records.
- `python3 scripts/check_session_handle_profile.py`: exit 0. It printed `"result": "PASS"`, `"problems": []`, `"revision": 6`, `"smp1_revision": 15`, `"capsule_revision": 4` and `"status": "S20_330_IMPLEMENTED_REVIEW_PENDING"`. My first attempt appended `echo exit=$?`, which the sandbox refused. The plain re-run and the probe control (`main()` rc=0) confirm exit 0.
- `python3 -m unittest scripts.test_session_handle_profile -v`: 12 tests OK, including the new `VersionThreeScopeCases` (3) and `AdrPinCases` (3).
- In-memory mutation probes against the real checker `main()`, with the module's `read` patched. The table is under Evidence checked.
- Bounded cargo tests. Each ran as `cargo test --locked --offline -p sley-protocol --lib <filter>`, and all passed:

| Filter | Result |
|---|---|
| `workspace_open` | 5/5, including the new `workspace_open_answers_from_the_single_checked_head_load` |
| `head_bound` | 1/1, `entity_reads_are_head_bound_under_version_3` |
| `native_tags_below_version_3` | 1/1 |
| `session` | 23/23 |
| `binding_failures_precede_budget_exhaustion` | 1/1 |
| `entity` | 18/18 |
| `sessions_bind_workspace_root_and_epoch_and_handles_name_their_root` | 1/1 |

**Files read:**
- `evidence/review/verdicts/session_handle_profile/vulcan_surface_review_revision_5-2b0f1c9.md`: all
- `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: 784-964
- `docs/spec/SESSION_HANDLE_PROFILE_V1.md`: 150-251, plus the diffed hunks at 1-37, 131-138, 288-293 and 389-445
- `docs/adr/ADR-0033-negotiated-session-boundary.md`: 1-24 and 86-120
- `scripts/check_session_handle_profile.py`: 1-175 and 233-582
- `crates/sley-protocol/src/server.rs`: 330-359, 720-741, 1105-1330, 2170-2260, 2370-2487 and 2910-2970
- `crates/sley-protocol/src/lib.rs`: 728-825
- `crates/sley-protocol/src/server_tests.rs`: 1262-1281, 1493-1524 and 7585-7686
- `crates/sley-txn/src/repository.rs`: 870-906, 2202-2251, 2322-2368, 2420-2440, 2536-2589, 3150-3179 and 4310-4359
- `crates/sley-txn/src/maintenance.rs`: 50-75 and 102-111
- `docs/spec/SMP1.md`: 770-782, plus the revision 15 diff
- `docs/spec/ENTITY_READ_PROFILE_V2.md`: 1-30 and 265-280

## Evidence checked

**Version 3 partition (round-7 P2):**
- **The profile now states the version 3 scope.** Profile :235-248 says 306/307 are head-bound "under every version-aware selection whose method table carries them: version 2, and version 3". It gives the version 3 partition as version 2 plus 605/606/607, and those three skip check 5. The Status line (:3-24), ADR-0033 :11-17 and :100-101 and :116-117, the WORK_PACKAGES S20-330 row and ERROR_CODES ("contract draft revision 6") agree.
- **The code matches.** `from_tag_versioned` admits 306/307 at version 3 (lib.rs:806-823). `dispatch` routes every version-aware entity read to `dispatch_entity_read` (server.rs:1176-1178). That path's `session_check_retained` applies `head_bound_versioned` whenever the server is version-aware (server.rs:2210-2225).
- **The new test discriminates.** `entity_reads_are_head_bound_under_version_3` uses the same workspace with a different root, so a stale root gives `SESSION_ROOT_ADVANCED` for both tags. If the entity reads were not routed there, they would fall through to server.rs:1323-1327 and answer `InternalInvariant`, and the test would fail.
- **The checker now reads version 3.** `version_gate_problems` (checker :177-212) parses `V3_ALL` (46 entries; version 2 plus exactly `TestsReportRead`, `TestsReplay` and `TestsAttemptStatus`). It requires the `if self.version_aware { head_bound_versioned } else { head_bound }` shape and refuses any `protocol_version` token in that gate. Three revert tests cover it.

**Decode precedence of 605-607:**
- Below version 3 these tags fail decode. `from_tag` scans the 41-entry `ALL`, and `from_tag_versioned` compares against `introduced_in` = 3 (lib.rs:775-823). Decode runs at server.rs:1114-1120, before the liveness check at :1162.
- Under version 3 without the native-tests bit, the tags decode and then pass checks 1-4 and 6. `dispatch_admitted` then refuses them at :1244-1246 because the SMP1 revision 15 filter removed them from `admits`. This matches profile :220-229.
- The new test covers version 2 only (605/606/607 → `METHOD_UNSUPPORTED`, 305 → `SESSION_UNKNOWN`). The version 1 case follows from the same decode path.

**Single checked head for 201:**
- `session_check_mixed_retained` (server.rs:2410-2420) returns the `HeadRevision` whose binding it checked. `dispatch` passes it on through `dispatch_admitted` (:1183-1205, :1295), and `workspace_open` uses it directly for version 1-format heads (:2942-2943).
- Order is unchanged: liveness → admission → binding checks 3-5 → budget test and debit → body refusal. `binding_failures_precede_budget_exhaustion` (:1508-1523) and the T15 test (:1277-1278) still reach `SESSION_ROOT_ADVANCED` through the new function, and both pass.
- `head_loads` is `#[cfg(test)]` only (server.rs:730-741, :2480-2481), so the counter adds no production state.

**Other round-7 items:**
- `session.close` body: profile :134-138 records the deviation in the same terms as SMP1:776-781.
- Capsule pins: profile :28-30 and :291-293 now name capsule revision 4 and state that the arm is unchanged since revision 3. CONTEXT_CAPSULE_PROFILE_V1.md:3 reads revision 4.
- §9: it now has the unrevisioned-edits entry (:424-430) and the revision 5 and 6 entries (:431-445). The checker requires `- Revision 6 (` (`history-current-revision`).
- ADR decision 7 (:92-104) names SMP1 15 and capsule 4. The Date line (:19-21) records revisions 5 and 6. `adr_pin_problems` anchors both the current pin and decision 7.

**Checker mutation probes (real `main()`, patched `read`):**

| Mutation | Result |
|---|---|
| None (control) | rc=0 PASS |
| Spec v3 lead sentence reverted to "only in protocol version 2" | rc=1 FAIL `spec-marker` |
| §9 revision 6 entry removed | rc=1 FAIL `history-current-revision` |
| ADR decision 7 set to "revision 12 and capsule revision 3" | rc=1 FAIL `adr-smp1-pin` |
| ADR current pin set to revision 5 | rc=1 FAIL `adr-current-pin` |
| server.rs:1176 entity-read routing narrowed to `protocol_version == PROTOCOL_VERSION_V2` | rc=0 PASS |
| server.rs:1205 passes `None` instead of the retained head | rc=0 PASS |
| server.rs:2417 check uses `false` instead of `Self::head_bound(method)` | rc=0 PASS |
| Spec :242-244 version 3 partition clause deleted | rc=0 PASS |
| Spec :245-246 narrowed to "version 2 sessions" | rc=0 PASS |

**Native head path and maintenance boundary:**
- On a native head, `head_mixed` (server.rs:2391-2406) returns `HeadRevision::Native`, and `workspace_open`'s `_ => self.head()?` reloads through the version 1 loader (:2944).
- sley-txn admits a version 1 child of a native parent (repository.rs:2543-2545, verified at :2559-2581). The import installs a version 1 receipt through :4316-4328.
- `self.maintenance()` (server.rs:2469-2472) calls `initialize_repository_maintenance`. That function creates the lock directory and file when they are absent (maintenance.rs:58-75).

## Findings

[P3] [record-accuracy] crates/sley-protocol/src/server.rs:2198-2201; bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:883 - The `head_bound_versioned` doc comment still says the entity reads are "head-bound only in protocol version 2". It cites ENTITY_READ_PROFILE_V2.md §6, whose :275-277 still gives the same instruction. Both contradict profile revision 6 :235-246, which covers version 2 and version 3. The 2b0f1c9f..HEAD diff never touches these lines, yet gate record §13.2 says "the server doc comment agree[s]". My round-7 P2 closure evidence named a matching server doc comment. So a record asserts a closure that did not happen, and the code comment on the partition helper states the pre-fix scope. - Closure evidence: the doc comment reworded to "every version-aware selection carrying them (version 2 and version 3)"; the ENTITY_READ §6 sentence either amended by its owner or recorded in profile §3/§9 as superseded for version 3; the gate record claim corrected or annotated.

[P4] [toctou-residual] crates/sley-protocol/src/server.rs:2942-2945 vs docs/spec/SESSION_HANDLE_PROFILE_V1.md:166-169 - The profile says `workspace.open` is always answered from the head load that check 5 compared. When the check retained a native-format head, `workspace_open` discards it and reloads with `self.head()`. The transaction owner admits a version 1 child of a native parent (sley-txn repository.rs:2543-2545, installed via :4316-4328). So a concurrent cross-process import can advance native root R1 to a version 1 child R2 between the check and the answer. A session bound to R1 would then receive R2's summary with no `SESSION_ROOT_ADVANCED`. Without the race the method fails as before. The window is narrow and cross-process only. - Closure evidence: the Native arm refuses without reloading (or compares the reloaded root with the retained root and fails), or the spec sentence is scoped to version 1-format heads; a test with a retained native head.

[P4] [checker-coverage] scripts/check_session_handle_profile.py:122-131, :177-212 - The version-gate check binds only the helper `session_check_retained`. It does not bind the routing condition at server.rs:1176, which decides whether 306/307 reach that helper, and it does not bind the new 201 retained check at server.rs:2410-2420. Five mutations each still pass the checker (rc=0 PASS):
  - narrowing :1176 to version 2;
  - passing `None` at :1205;
  - using `false` at :2417;
  - deleting the version 3 partition clause at :242-244;
  - narrowing :245-246 to version 2.

  Cargo tests catch each code mutation: `entity_reads_are_head_bound_under_version_3`, `workspace_open_answers_from_the_single_checked_head_load`, and server_tests.rs:1277 and :1517. That is why this is only a note. - Closure evidence: shape checks or markers for the :1176 routing and the retained 201 check, with revert tests, and an anchor on the version 3 partition clause.

[P4] [doc-accuracy] crates/sley-protocol/src/server.rs:2930-2933 - The `workspace_open` docstring (edited in 004cf5be) says an absent maintenance boundary "fails the method" because the head load that precedes the probe takes the boundary shared. For 201, that preceding load is the session check's `head_mixed` (:2391-2392). It calls `self.maintenance()` (:2469-2472), which initializes the boundary and creates the lock directory and file (sley-txn maintenance.rs:58-75). An absent boundary is therefore created, not refused. The probe still never sees an absent boundary, but the stated mechanism is wrong, and no test covers it. - Closure evidence: the docstring reworded to say the session check's head load initializes the boundary (S20-390 behavior) before the probe, or a test that demonstrates the claimed failure.

## Assessment

Status of each finding from my round-7 transcript:
- Round-7 [P2] [contract-partition] SESSION_HANDLE_PROFILE_V1.md:212-220, version 3 partition of 306/307: **CLOSED**. Profile :235-248, the Status line, ADR-0033 and the summary now state the version 2 and version 3 scope. The checker parses `V3_ALL` and binds the version-aware gate shape, with revert tests. `entity_reads_are_head_bound_under_version_3` passes. The stale server doc comment named in the closure evidence is still there and is carried as the new P3.
- Round-7 [P3] [revision-record] §9 had no revision 5 entry: **CLOSED**. Entries :424-445 exist, the cited commits match, and the `history-current-revision` anchor FAILs when the entry is removed (probe).
- Round-7 [P3] [pin-drift] ADR-0033 decision 7 stale pins: **CLOSED**. Decision 7 (:95-97) names SMP1 15 and capsule 4, and the Date line (:19-21) records revisions 5 and 6. `adr_pin_problems` FAILs on either stale pin (probe).
- Round-7 [P4] [pin-consistency] capsule revision 3 vs 4: **CLOSED**. Profile :28-30 and :291-293 name revision 4 and state that the arm is unchanged since revision 3.
- Round-7 [P4] [fail-open-parse] `session.close` ignores a body: **CLOSED** (recorded). Profile :134-138 records the deviation in the same terms as SMP1:776-781. The behavior is unchanged, which the closure evidence allowed.

Revision 6 fixes the gap that made my lane REVISE revision 5. The closed head-bound partition now states version 3 explicitly, and the code enforces it: every version-aware selection routes 306/307 through `head_bound_versioned`. The server test and the checker's gate-shape rule both enforce the version 3 scope.

The new decode-precedence text matches lib.rs and the dispatch order. The single-checked-head change for 201 keeps every binding failure ahead of the budget. It also makes the answered version 1 head the same object that check 5 compared.

The remaining issues are minor:
- a stale code comment, with a gate-record claim that it was fixed (P3);
- a narrow cross-process reload on native heads that the spec's universal wording does not allow for (P4);
- checker coverage that relies on cargo tests for the routing and the 201 check (P4);
- an inaccurate mechanism in the `workspace_open` docstring (P4).

None of these reopens a fail-open path under ordinary single-writer use, so my lane accepts this package at contract revision 6.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_3_P4_PRIOR_P3_P4_CLOSED
SECTION: session_handle_profile
FIELD: vulcan_surface_review_revision_6
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P3] [record-accuracy] crates/sley-protocol/src/server.rs:2198-2201; bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:883 - head_bound_versioned doc comment still says "head-bound only in protocol version 2" (echoing ENTITY_READ_PROFILE_V2.md:275-277), contradicting profile rev 6 :235-246; the diff never touches it yet gate record 13.2 claims the server doc comment agrees - closure: reword the comment, reconcile or record ENTITY_READ section 6 as superseded for v3, correct the gate record claim | [P4] [toctou-residual] crates/sley-protocol/src/server.rs:2942-2945 vs docs/spec/SESSION_HANDLE_PROFILE_V1.md:166-169 - a retained native head is discarded and reloaded via self.head(); owner-admitted v1-child-of-native imports (sley-txn repository.rs:2543-2545, :4316-4328) let a concurrent advance be served to a session bound to the older root, contrary to the spec's universal claim - closure: refuse the Native arm without reloading or compare roots, or scope the sentence; a test | [P4] [checker-coverage] scripts/check_session_handle_profile.py:122-131, :177-212 - routing at server.rs:1176, the retained 201 check (:1205, :2417), and the v3 partition clause (spec :242-246) are unbound; five probe mutations pass the checker (cargo tests catch the code ones) - closure: shape checks or markers with revert tests and a clause anchor | [P4] [doc-accuracy] crates/sley-protocol/src/server.rs:2930-2933 - docstring says an absent maintenance boundary fails the method, but the session check's head_mixed calls self.maintenance() (:2469-2472), which creates the boundary (sley-txn maintenance.rs:58-75) - closure: reword to the actual mechanism or add a test proving the claimed failure
SUMMARY: Revision 6 closes my round-7 P2. The profile, ADR, summary and server agree that 306/307 are head-bound under versions 2 and 3; the checker now parses V3_ALL and binds the version-aware gate shape with revert tests; and the new v3 stale-root, 605-607 decode and single-head-load tests pass, as do the checker (exit 0, PASS, 12 unit tests) and 51 bounded sley-protocol tests. All four prior P3/P4 findings are verified closed. One P3 remains: the server's head_bound_versioned comment still says "only in protocol version 2" while the gate record claims it was fixed. Three P4 notes cover a narrow native-head reload race, checker blind spots that cargo tests cover, and an inaccurate maintenance-boundary docstring, so my lane PASSes revision 6.

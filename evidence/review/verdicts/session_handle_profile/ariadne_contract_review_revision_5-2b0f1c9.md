<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: ariadne; field: ariadne_contract_review_revision_5; dispatched: 2026-09-23T11:55:14Z; duration_s: 428; process_exit_code: 0 -->
# Ariadne Council review — session_handle_profile

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, the scope SHA. The working tree is clean, and `ab42a3a9` is an ancestor of HEAD, 30 commits back.
- **Git commands run:**
  - `git diff ab42a3a9..HEAD -- docs/spec/SESSION_HANDLE_PROFILE_V1.md`
  - `git log ab42a3a9..HEAD` on that file. Two commits touch it: `a8b4cddb` pinned SMP1 revision 13 in place, and `8357c243` made revision 5 with the SMP1 revision 14 pin.
  - `git show` of both commits for the profile, the checker, and the checker tests.
  - `git diff ab42a3a9..HEAD` for `docs/spec/SMP1.md`, ADR-0033, `docs/spec/ERROR_CODES_V1.md` and `docs/WORK_PACKAGES.md`.
  - A `git log -S` history search for the capsule re-pin (`bf5b7e78`, 2026-09-14), the version 3 native sentence (`e0ff1371`, 2026-09-16), the "head-bound only" wording (`d2c87b0a`), and the date version 3 landed (`116d031e`, 2026-09-16).
- **Checkers run:**
  - `python3 scripts/check_session_handle_profile.py`: exit 0, `"result": "PASS"`, `"problems": []`, `revision 5`, `smp1_revision 14`, `capsule_revision 4`, `status S20_330_IMPLEMENTED_REVIEW_PENDING`.
  - `python3 scripts/check_smp1_contract.py`: exit 0, `"result": "PASS"`, `revision 14`.
  - `python3 -m unittest scripts.test_session_handle_profile`: `Ran 6 tests`, `OK`.
- **In-memory negative probes of the checker** (by patching `read`):
  - SMP1 Status set to revision 13: FAIL `smp1-revision-pin`.
  - Spec pin set to revision 13: FAIL `spec-marker`.
  - Spec Status set to revision 4: FAIL `spec-revision`.
  - ADR-0033 line 107 changed to "pins SMP1 revision 13": **PASS**. That drift goes undetected (finding 4).
- **Cargo tests:**
  - `cargo test --locked --offline -p sley-protocol --lib workspace_open`: 4 passed, 0 failed. The four tests are `workspace_open_under_version_1_never_carries_field_9`, `workspace_open_v2_discloses_only_the_materialized_head_snapshot`, `workspace_open_v3_answers_the_version_2_open_summary` and `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record`.
  - The eight T15/T47/T56 and precedence tests: 8 passed, 0 failed. They are `handles_name_their_expected_root`, `sessions_bind_workspace_root_and_epoch_and_handles_name_their_root`, `binding_failures_precede_budget_exhaustion`, `checks_follow_contract_order`, `issuance_binds_head_and_nonce_separates_instances`, `session_repository_and_transaction_methods_answer_deterministically`, `live_sessions_are_capped_and_restarts_forget` and `identity_session_and_frame_rules_hold_at_the_server`.
- **Files read:**
  - `docs/spec/SESSION_HANDLE_PROFILE_V1.md` 1-394 (all)
  - `scripts/check_session_handle_profile.py` 1-578 (all)
  - `scripts/test_session_handle_profile.py` 1-150
  - `docs/spec/SMP1.md`: revision 14 diff hunks, plus lines 216-231 and 676-760
  - `docs/adr/ADR-0033-negotiated-session-boundary.md` 1-30 and 100-112
  - `docs/spec/ENTITY_READ_PROFILE_V2.md` 1-60, 140-159, 255-277
  - `docs/spec/NATIVE_TEST_ADMISSION_V1.md` 545-557
  - `crates/sley-protocol/src/server.rs`: 1096-1318, 1355-1401, 2159-2260, 2396-2402, 2717-2752, 2890-2933, 3284-3318
  - `crates/sley-protocol/src/lib.rs`: 560-599, 725-824
  - `crates/sley-protocol/src/server_tests.rs`: 610-640, 7385-7535
  - `evidence/security/T15|T47|T56/matrix.json`
  - `evidence/review/verdicts/session_handle_profile/delta_review-a4b6029.md` (header and verdict lines)
  - the `session_handle_profile` section of `machineresearch/sley-2.0/machine-summary.json`, compared field by field against `ab42a3a9`

## Evidence checked

**What revision 5 changes in the package's own text.**
- The Status paragraph moves to revision 5 (dated 2026-09-23) and describes the SMP1 revision 14 re-pin (SESSION_HANDLE_PROFILE_V1.md:3-15).
- The preamble now names SMP1 revision 14 (line 19), and the composition pin is updated (line 24).
- The in-place revision 13 wording that `a8b4cddb` added is gone.

**The SMP1 revision 14 claims in line 12 match SMP1:**
- `workspace.open` answers `open_summary` under version 2 and every later selection that carries row 201 (SMP1.md:682, 716-745).
- A non-empty 201 body is refused with `PROTOCOL_PAYLOAD_INVALID` under every version (SMP1.md:682, 755-760).
- Row 201 stays `workspace.open` in the SMP1 table, which is what the checker's `smp1_rows` regex reads.

**201 stays head-bound, and no session check moves.**
- `head_bound` still lists `Method::WorkspaceOpen` (server.rs:2167-2184), which matches the profile's head-bound list (profile:169-174).
- The dispatch order follows profile §3 (server.rs:1096-1194):
  1. version claim
  2. bounds
  3. method decode
  4. liveness / remembered close
  5. admission
  6. `session_check`: workspace, epoch, head-bound root
  7. budget check and one-unit charge
  8. `dispatch_admitted`
- Only after all of these does `workspace_open` refuse a non-empty body (server.rs:2904-2907). So a non-empty 201 body can never pre-empt `SESSION_UNKNOWN`, `PROTOCOL_SESSION_CLOSED`, `SESSION_WORKSPACE_MISMATCH`, `SESSION_EPOCH_MISMATCH`, `SESSION_ROOT_ADVANCED`, or the budget refusal. Profile §3 (lines 138-159) still holds.

**Bytes this package produces or consumes.**
- **`handle.expand` (304):** revision 14 does not change it. The request is still `uvar(handle) || StateRoot[32]` with an exact length check (server.rs:2717-2725). The response record has fields 1-5, with field 2 encoded as the uvar of the u32 kind (server.rs:2745-2751). That matches SMP1 appendix A row 304 (SMP1.md:698) and the profile's §4 grammar under SMP1's "uvar integers" convention.
- **Sessions:** the `session.open`/`renew`/`close` bodies are unchanged.
- **Field 9:**
  - It comes from a non-blocking probe (server.rs:2925-2933).
  - It is gated on `protocol_version >= PROTOCOL_VERSION_V2` (server.rs:2909).
  - It is added only by `head_open_summary`, so `revision.read` stays eight fields (server.rs:3284-3301).
  - Nothing in it is session-derived.
- **Profile §4 (line 243-244):** "the caller learns the new root from any head-bound read" still holds, because field 2 of `open_summary` is the `StateRoot`.

**Re-pin consistency.**
- The checker pins `CONTRACT_REVISION = 5` and `SMP1_REVISION = 14`, each tied to its authority's Status line (checker:41-47, 402-410). The negative probes confirm both pins bite.
- ADR-0033 lines 11-13 and 107-111 were updated.
- ERROR_CODES_V1.md now says "contract draft revision 5".
- The WORK_PACKAGES S20-330 row says revision 5 and `S20_330_IMPLEMENTED_REVIEW_PENDING`.
- The machine summary has `contract_revision 5`, `current_delta_review {contract_revision 5, PENDING ×3}`, `status S20_330_IMPLEMENTED_REVIEW_PENDING` (was `S20_330_COMPLETE`), `implementation_complete false`, and a dated `status_note`.
- The test updates only change their assertions to derive the revision from `CHECKER.CONTRACT_REVISION`.

**Version 3 under the pinned authority.**
- SMP1 revision 14 now admits version 3 itself, defined as the union of the version 1 and version 2 tables plus the native rows (SMP1.md:221-227). NATIVE_TEST_ADMISSION appendix D agrees (NATIVE_TEST_ADMISSION_V1.md:547-553).
- The server serves 306 and 307 as head-bound under version 3:
  - `from_tag_versioned` admits a tag whenever `version >= introduced_in` (lib.rs:815-822).
  - Every version-aware session routes 306/307 through `dispatch_entity_read` (server.rs:1176-1177).
  - That path calls `session_check_retained`, which applies `head_bound_versioned` (server.rs:2204-2205, 2231).
- The checker reads only `ALL` (41) and `V2_ALL` (43), never `V3_ALL` (checker:216-221).
- No test drives 306/307 on a version 3 session.
- Version 3 landed on 2026-09-16 (`116d031e`). This package's last delta review (`a4b6029`, 2026-09-13) and its finals (`d384f0f`, 2026-09-14) came before it, so no review of this profile has covered version 3 until now.

## Findings

[P2] [contract] docs/spec/SESSION_HANDLE_PROFILE_V1.md:212-220 (also :9-10, docs/adr/ADR-0033-negotiated-session-boundary.md:7-9) - The profile's closed classification still says `entity.version` (306) and `entity.signature` (307) are "head-bound only in protocol version 2", and it defines only a version 1 and a version 2 partition. SMP1 revision 14, which this revision pins, now defines version 3 as the union of the version 1 and version 2 tables (SMP1.md:221-227). The server treats 306 and 307 as head-bound on every version-aware selection, version 3 included (lib.rs:815-822; server.rs:1176-1177, 2204-2205, 2231). So the profile's statement is false for version 3, and its claim that every tag's head-boundness "is read from the lists" (line 161-167) has no list for version 3. Revision 5 still asserts "no session clause changes" (line 13). This is the same version-scope defect that SMP1 revision 14 was written to fix in SMP1 itself. The checker cannot see it because it reads only `ALL`/`V2_ALL`, never `V3_ALL` (checker:216-221). - Closure evidence: a dated revision that states the version 3 partition (306/307 head-bound under every selection that carries them, and where the live 601/602/605-607 sit) and replaces "only in version 2" in the Status line, §3 and ADR-0033. Checker coverage of the `V3_ALL` partition. A server test showing 306 or 307 on a stale-root version 3 session answering `SESSION_ROOT_ADVANCED` ahead of the budget failure.

[P3] [record] docs/spec/SESSION_HANDLE_PROFILE_V1.md:360-394 - §9 "Revision history" ends at revision 4. Revision 5 is described only in the Status paragraph, although every earlier revision has a §9 entry, and §9 of revision 4 is where the previous SMP1 pin move was recorded. The in-place capsule re-pin to revision 4 (`bf5b7e78`, 2026-09-14) and the version 3 native-tag sentence (`e0ff1371`, 2026-09-16) also have no history entry. - Closure evidence: a §9 "Revision 5 (2026-09-23)" entry recording the SMP1 revision 14 re-pin, and a note on the earlier in-place amendments.

[P4] [consistency] docs/spec/SESSION_HANDLE_PROFILE_V1.md:19,28,263,393 - Line 19 was edited in this revision and now pairs "SMP1 (S20-400, revision 14)" with "the master context capsule (S20-320 full, revision 3)". But the composition pin at line 28 and the checker (`CAPSULE_REVISION = 4`, checker:46) say capsule revision 4. §9 revision 4 still says "the capsule pin stays at revision 3". The preamble therefore gives two capsule revisions. - Closure evidence: line 19 either names capsule revision 4 or is clearly worded as a historical citation (as line 263 can be read).

[P4] [checker] scripts/check_session_handle_profile.py:425-426 - The ADR pin check is a substring match on "SMP1 revision 14". The new ADR status sentence (ADR-0033:12, "re-pin to SMP1 revision 14") satisfies it, so the normative sentence "This profile pins SMP1 revision 14, the current revision" (ADR-0033:107) can go stale undetected. My in-memory probe changed line 107 to 13 and the checker still passed. - Closure evidence: anchor the check to the line-107 sentence (for example the literal `pins SMP1 revision {SMP1_REVISION},`) and add a revert test.

[P4] [record] machineresearch/sley-2.0/machine-summary.json (`session_handle_profile.current_delta_review`) - The revision 4 delta verdicts (PASS ×3, transcript `evidence/review/verdicts/session_handle_profile/delta_review-a4b6029.md`) were overwritten by the revision 5 PENDING record without a machine-readable history field or note. The `json_bridge` and `cli` sections keep this in `current_delta_review_note` with transcript paths. Here it survives only in prose (the WORK_PACKAGES row, ADR-0033:11, and the tail of `status_note`). - Closure evidence: a `current_delta_review_note` (or equivalent) recording the revision 4 round, its scope `a4b6029` and its transcript path.

## Assessment

The SMP1 re-pin itself is correct and complete on its own terms:
- The status, preamble and composition pin all name SMP1 revision 14.
- The description of what revision 14 changes (`open_summary` under version 2 and later, the non-empty 201 body refusal under every version) matches SMP1.md:682-760 and server.rs:2904-2916.
- 201 stays head-bound.
- The server still applies the §3 checks, in the §3 order, before the new body refusal.
- The handle, session and capsule bytes are untouched.
- The checker, its tests, the relevant `workspace_open` and T15/T47/T56 tests, ADR-0033, ERROR_CODES, WORK_PACKAGES and the machine summary all agree with revision 5.

However, revision 14 also brings version 3 into the SMP1 authority this profile pins. The profile's version-scoped classification of 306/307 ("head-bound only in protocol version 2") is now false for a selection that authority defines, while revision 5 asserts that no session clause changes. The implementation is correct, since 306/307 are head-bound under version 3, but the contract, ADR and checker do not say or check so. Because this is a normative misstatement about the method classification, the lane cannot accept revision 5 as written.

Two observations outside this lane, not counted as findings:
- The WORK_PACKAGES S20-400 row still describes SMP1 "contract draft revision 13" (docs/WORK_PACKAGES.md:46). That is the protocol package's record.
- The live version 3 native methods run their own root check after the budget charge (server.rs:1380-1390). That behavior belongs to NATIVE_TEST_ADMISSION and predates this delta. It is relevant to finding 1's closure only in stating where those tags sit.

VERDICT: REVISE_0_P0_0_P1_1_P2_1_P3_3_P4
SECTION: session_handle_profile
FIELD: ariadne_contract_review_revision_5
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P2] [contract] docs/spec/SESSION_HANDLE_PROFILE_V1.md:212-220 (also :9-10, docs/adr/ADR-0033-negotiated-session-boundary.md:7-9) - 306/307 stated "head-bound only in protocol version 2" with only v1/v2 partitions, while pinned SMP1 revision 14 defines version 3 (union of the v1+v2 tables) and the server treats 306/307 as head-bound on v3 (lib.rs:815-822, server.rs:1176-1177/2204-2205/2231); checker never reads V3_ALL; revision 5 claims no session clause changes - needs a dated revision stating the v3 partition, aligned wording in Status/§3/ADR, V3_ALL checker coverage, and a v3 stale-root 306/307 server test; [P3] [record] docs/spec/SESSION_HANDLE_PROFILE_V1.md:360-394 - §9 revision history has no revision 5 entry and no record of the in-place 2026-09-14 capsule re-pin and 2026-09-16 v3 native sentence - needs a §9 revision 5 entry; [P4] [consistency] docs/spec/SESSION_HANDLE_PROFILE_V1.md:19,28,263,393 - line 19 (edited this revision) says capsule revision 3 while the pin and checker say 4 - needs line 19 reconciled or explicitly historical; [P4] [checker] scripts/check_session_handle_profile.py:425-426 - ADR SMP1 pin check is a substring masked by the new ADR-0033:12 status sentence; a stale ADR-0033:107 pin passes (probed) - needs an anchored check and revert test; [P4] [record] machineresearch/sley-2.0/machine-summary.json session_handle_profile.current_delta_review - revision 4 PASS ×3 (a4b6029 transcript) overwritten with no machine-readable history note - needs a current_delta_review_note recording that round
SUMMARY: Revision 5's re-pin to SMP1 revision 14 is mechanically complete and accurate for workspace.open. 201 stays head-bound, the session checks still run in §3 order before the new non-empty-body refusal, and the checker (exit 0, PASS), the unit tests (6 OK) and the cargo tests (4 + 8 passed) agree. But the pinned SMP1 revision 14 now defines version 3, and the profile still says 306/307 are head-bound "only in protocol version 2" while the server makes them head-bound under version 3. That misstatement in the method classification is a P2, so the lane returns REVISE, together with one P3 revision-history gap and three P4 record/checker notes.

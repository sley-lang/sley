<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: nabu; field: nabu_architecture_review_revision_5; dispatched: 2026-09-23T12:08:15Z; duration_s: 366; process_exit_code: 0 -->
# Nabu Council review — session_handle_profile

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `2b0f1c9f4940c020565137891a49a3769cd02061`, so the scope matches. The branch is `work/succ-context-impl`. Five untracked verdict files exist, and one of them is an Ariadne revision-5 verdict for this section. I did not read it. My memory notes from an earlier session record that an Ariadne review at this SHA raised the 306/307 version-scope point. Everything below was re-derived from code, tests and checker output. I did not take it from that note.
- **Git reads:**
  - `git diff ab42a3a9..HEAD -- docs/spec/SESSION_HANDLE_PROFILE_V1.md`
  - `git show a8b4cddb` and `git show 8357c243`, limited to the profile. These are the two commits in range: the interim unrevisioned SMP1-13 pin, then revision 5.
  - `git diff ab42a3a9..HEAD -- docs/spec/SMP1.md` (the revision 12 → 14 delta)
  - the same diff for the checker, ADR-0033, ERROR_CODES_V1 and WORK_PACKAGES
  - `git log` for the profile's history (it has a revision-less v3 edit in e0ff1371 on 2026-09-16)
- **Checker.** `python3 scripts/check_session_handle_profile.py` exited 0 with `"result": "PASS"`, `"revision": 5`, `"smp1_revision": 14`, `"status": "S20_330_IMPLEMENTED_REVIEW_PENDING"` and `problems: []`.
- **Checker unit tests.** `python3 -m unittest scripts.test_session_handle_profile` ran 6 tests: OK.
- **Cargo.** `cargo test --locked --offline -p sley-protocol --lib` with filters for workspace_open, the T15/T47/T56 matrix tests, check-order, budget-precedence, caps and entity reads: `test result: ok. 29 passed; 0 failed`. That includes:
  - `workspace_open_under_version_1_never_carries_field_9`
  - `workspace_open_v2_discloses_only_the_materialized_head_snapshot`
  - `workspace_open_v3_answers_the_version_2_open_summary`
  - `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record`
  - `binding_failures_precede_budget_exhaustion`
  - `sessions_bind_workspace_root_and_epoch_and_handles_name_their_root`
  - `checks_follow_contract_order`
  - `handles_name_their_expected_root`
  - `issuance_binds_head_and_nonce_separates_instances`
  - `live_sessions_are_capped_and_restarts_forget`
- **In-memory checker probe.** I loaded the checker module and patched `read` to apply two changes at once:
  1. server.rs `session_check_retained` gated so 306/307 are head-bound only when `protocol_version == 2`
  2. the profile truncated before the §9 "Revision 4" entry

  Result: `rc 0 result PASS problems []`.
- **Files read:**
  - `docs/spec/SESSION_HANDLE_PROFILE_V1.md`:1-394 (all of it)
  - `scripts/check_session_handle_profile.py`:1-578 (all of it)
  - the `session_handle_profile` section of `machineresearch/sley-2.0/machine-summary.json` (all of it), plus a key-by-key diff against ab42a3a9
  - `crates/sley-protocol/src/server.rs`:1096-1295, 2159-2266, 2348-2460, 2880-2933, 3280-3318
  - `crates/sley-protocol/src/lib.rs`:505-594, 733-823
  - `crates/sley-protocol/src/server_tests.rs`:620-640, 1255-1324, 1460-1523, 3290-3379, 7380-7585
  - `docs/spec/NATIVE_TEST_ADMISSION_V1.md`:1-30, 540-577
  - `docs/spec/ENTITY_READ_PROFILE_V2.md`:140-159, 266-277
  - `docs/adr/ADR-0033-negotiated-session-boundary.md`:1-16 plus its diff
  - `docs/spec/ERROR_CODES_V1.md`:318-321
  - the S20-330 row diff in `docs/WORK_PACKAGES.md`
  - the revision references in `docs/audits/S20_330_NEGOTIATED_SESSION_CLOSEOUT.md`
  - `evidence/security/{T15,T47,T56}/matrix.json`

## Evidence checked

- **The profile diff.** Only the Status paragraph (lines 3-16) and the two SMP1 pins (lines 19 and 24, now revision 14) changed. The capsule pin stays at revision 4. The §9 revision history was not touched.
- **Machine summary.** Relative to ab42a3a9, only these changed:
  - `contract_revision` 4→5
  - `current_delta_review`, from revision 4 PASS×3 to revision 5 PENDING×3
  - `status`, from `S20_330_COMPLETE` to `S20_330_IMPLEMENTED_REVIEW_PENDING`
  - `implementation_complete` true→false
  - `status_note`

  The revision-4 delta-review transcript is kept at `evidence/review/verdicts/session_handle_profile/delta_review-a4b6029.md`, so the history claim "revision 4 reviews retained" holds.
- **Checker pins.** Only `CONTRACT_REVISION = 5` and `SMP1_REVISION = 14` changed. The SMP1 pin is checked against SMP1's own `Status:` line. The ADR, WORK_PACKAGES and ERROR_CODES revision anchors all carry revision 5 and SMP1 14. The re-pin is mechanically complete across all consumer anchors the checker reads.
- **Row 201 against the profile:**
  - `workspace.open` stays in `head_bound` (server.rs:2167-2184). The profile's head-bound list (§3:169-174) still matches, and the checker compares them exactly.
  - The non-empty-body refusal (`PROTOCOL_PAYLOAD_INVALID`, server.rs:2905-2907) runs inside `dispatch_admitted`. That is after liveness (:1162), admission (:1170), `session_check` (:1179) and the budget check and debit (:1181-1193). So it ranks behind checks 1-6, and the profile's §3 precedence still holds. This is established by reading the code; no test pins the combination.
  - Field 9 is added only by `head_open_summary` (server.rs:3292-3301). Version 1 bytes are unchanged, as the test at server_tests.rs:636-637 checks. `revision.read` stays at eight fields.
  - `workspace.open` returns no handle, session identity or capsule. The profile has no clause about 201's body bytes, so "no session clause changes" is true for row 201 itself.
- **Version scope:**
  - SMP1 revision 14 now admits selection 3 on its own entrypoints (SMP1.md:219-226) and names the "wire selection 1, 2, or 3" (:70).
  - `Method::from_tag_versioned` (lib.rs:806-823) resolves 306/307 at version 3.
  - `session_check_retained` (server.rs:2204-2208) applies `head_bound_versioned`, which includes 306/307, on every `version_aware` selection, version 3 included.
  - There is no version 3 test of 306/307 session classification: the multiline search over server_tests.rs found no matches.

## Findings

[P2] [contract-scope] docs/spec/SESSION_HANDLE_PROFILE_V1.md:9-13,161-167,212-220 - Revision 5 re-pins to SMP1 revision 14, which admits version 3 as the union of the v1 and v2 tables (SMP1.md:70,219-226). Yet the profile still says 306/307 are "head-bound only in protocol version 2" and defines only the v1 and v2 partitions. The server makes 306/307 head-bound on every version-aware selection, version 3 included (server.rs:2190-2193,2204-2208; lib.rs:785-823), so the "closed" classification that the checker "fails closed" against does not describe a live selection. The checker reads only `ALL` and `V2_ALL` (checker :216-217) and never the version gate in `session_check_retained`: the in-memory probe making 306/307 v2-only still PASSes. The re-pin applies "version 2 and every later selection" to row 201 (line 12) but not to its own extension clause. The same wording is in ADR-0033:8 and ENTITY_READ_PROFILE_V2.md:274 - Closure needs: a §3 statement of the v3 partition (306/307 head-bound at v2 and every later selection carrying those rows; the classification of live 601/602/605-607 at v3 with the native-tests bit); a checker that compares against `V3_ALL` and binds the version gate in `session_check_retained` (with a revert test); and a v3 server test showing 306/307 answer `SESSION_ROOT_ADVANCED` on a stale bound root.
[P3] [contract-accuracy] docs/spec/SESSION_HANDLE_PROFILE_V1.md:203-210 - The profile says all seven reserved tags, including 605/606/607, "pass checks 1 through 4 and check 6 and are then refused" outside version 3 with the native-tests bit. Under v1 and v2, though, 605-607 are not in the table and are refused at decode, before check 1 (server.rs:1114-1120 runs before the liveness check at :1162; lib.rs:775-781,788-790,820-822; NATIVE_TEST_ADMISSION_V1.md:566-569). So an unknown session sending 605 under v2 gets `PROTOCOL_METHOD_UNSUPPORTED`, not `SESSION_UNKNOWN`. This text predates the delta (e0ff1371) but is part of the revision 5 text under review - Closure needs: reword §3 so 305/503/601/602 pass checks 1-4 and 6 and are then refused, while 605-607 below version 3 are refused at decode before check 1; add a server test for the precedence with an unknown session.
[P3] [record-completeness] docs/spec/SESSION_HANDLE_PROFILE_V1.md:360-394 - §9 has no Revision 5 entry. Its last entry (revision 4) still says the SMP1 pin "follows SMP1 to revision 12", and the interim unrevisioned SMP1-13 pin (a8b4cddb) is not recorded anywhere in the profile. ADR-0033:3-16 keeps two "Current pin" statements (2026-09-08 revision 4 and 2026-09-23 revision 5), and its Date line has no revision 5 record. The checker only needs the §9 heading (checker :89): the probe that deleted the Revision 4 entry still PASSes - Closure needs: a dated "Revision 5 (2026-09-23)" §9 entry recording the SMP1 12→13→14 pin moves; one current-pin statement plus a revision-5 date in ADR-0033; a checker assertion that §9 contains `- Revision {CONTRACT_REVISION} (`.
[P4] [editorial] docs/spec/SESSION_HANDLE_PROFILE_V1.md:11-13 - The revision 5 Status sentence is one unwrapped line of about 250 characters. It paraphrases SMP1's "every later selection whose method table includes version 2's row 201" (SMP1.md:723) as "every later selection", which drops the qualifier - Closure needs: re-wrap, and quote SMP1's qualifier exactly.
[P4] [identity-binding] crates/sley-protocol/src/server.rs:1179,2396-2397,2904-2915 - This predates the delta and is noted, not blocking. For `workspace.open`, check 5 runs on one head load (`session_check`→`head_binding_mixed`), and the body (fields 1-9) is built from a second, separate `self.head()` load. A head advanced by another process between the two loads would be answered to a session bound to the older root, without `SESSION_ROOT_ADVANCED`. The entity-read path was already repaired for exactly this (`session_check_retained`, `repair_entity_read_uses_single_admitted_head_load`). Field 9 now makes the returned identity something a client can adopt, but the later head-bound `query.root` fails closed - Closure needs: answer row 201 from the revision kept by the session check (or re-compare the root), with a single-head-load test like the entity-read repair.

## Assessment

On row 201 itself, the re-pin is sound:
- SMP1 revision 14 does not make any statement in this profile about `workspace.open` false.
- 201 stays head-bound in both the contract and the server.
- The non-empty-body refusal ranks behind every session check.
- Field 9 touches no session, handle or capsule bytes.
- Version 1 bytes are unchanged, and the tests for v1, v2, v3 and the probe pass.
- Every consumer anchor the checker reads (ADR, WORK_PACKAGES, ERROR_CODES, machine summary) consistently records revision 5 against SMP1 14.

The re-pin is not consistent as a whole, though:
- SMP1 revision 14's main change is to make version scope explicit and to recognise version 3. The profile applies that scope to 201 but leaves its own method classification at "v1 and v2 only". The implementation makes 306/307 head-bound at v3, and the checker cannot see version 3 at all (P2).
- The v3-era reserved-tag sentence misstates the precedence for 605-607 below version 3 (P3).
- The revision record for revision 5 is incomplete, and nothing in the checker enforces it (P3).

The server behaviour is fail-closed in each case: 306/307 are stricter at v3, and 605-607 are refused. So these are contract and evidence-binding defects, not safety defects. Even so, the profile's closed-partition claim, which is what the checker is supposed to enforce, does not hold under the SMP1 revision it now pins. My lane does not accept revision 5 as it stands.

VERDICT: REVISE_0_P0_0_P1_1_P2_2_P3_2_P4
SECTION: session_handle_profile
FIELD: nabu_architecture_review_revision_5
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P2] [contract-scope] docs/spec/SESSION_HANDLE_PROFILE_V1.md:9-13,161-167,212-220 - Revision 5 re-pins to SMP1 revision 14, which admits version 3 as the union of the v1 and v2 tables (SMP1.md:70,219-226). Yet the profile still says 306/307 are "head-bound only in protocol version 2" and defines only the v1 and v2 partitions. The server makes 306/307 head-bound on every version-aware selection, version 3 included (server.rs:2190-2193,2204-2208; lib.rs:785-823), so the "closed" classification that the checker "fails closed" against does not describe a live selection. The checker reads only `ALL` and `V2_ALL` (checker :216-217) and never the version gate in `session_check_retained`: the in-memory probe making 306/307 v2-only still PASSes. The re-pin applies "version 2 and every later selection" to row 201 (line 12) but not to its own extension clause. The same wording is in ADR-0033:8 and ENTITY_READ_PROFILE_V2.md:274 - Closure needs: a §3 statement of the v3 partition (306/307 head-bound at v2 and every later selection carrying those rows; the classification of live 601/602/605-607 at v3 with the native-tests bit); a checker that compares against `V3_ALL` and binds the version gate in `session_check_retained` (with a revert test); and a v3 server test showing 306/307 answer `SESSION_ROOT_ADVANCED` on a stale bound root. | [P3] [contract-accuracy] docs/spec/SESSION_HANDLE_PROFILE_V1.md:203-210 - The profile says all seven reserved tags, including 605/606/607, "pass checks 1 through 4 and check 6 and are then refused" outside version 3 with the native-tests bit. Under v1 and v2, though, 605-607 are not in the table and are refused at decode, before check 1 (server.rs:1114-1120 runs before the liveness check at :1162; lib.rs:775-781,788-790,820-822; NATIVE_TEST_ADMISSION_V1.md:566-569). So an unknown session sending 605 under v2 gets `PROTOCOL_METHOD_UNSUPPORTED`, not `SESSION_UNKNOWN`. This text predates the delta (e0ff1371) but is part of the revision 5 text under review - Closure needs: reword §3 so 305/503/601/602 pass checks 1-4 and 6 and are then refused, while 605-607 below version 3 are refused at decode before check 1; add a server test for the precedence with an unknown session. | [P3] [record-completeness] docs/spec/SESSION_HANDLE_PROFILE_V1.md:360-394 - §9 has no Revision 5 entry. Its last entry (revision 4) still says the SMP1 pin "follows SMP1 to revision 12", and the interim unrevisioned SMP1-13 pin (a8b4cddb) is not recorded anywhere in the profile. ADR-0033:3-16 keeps two "Current pin" statements (2026-09-08 revision 4 and 2026-09-23 revision 5), and its Date line has no revision 5 record. The checker only needs the §9 heading (checker :89): the probe that deleted the Revision 4 entry still PASSes - Closure needs: a dated "Revision 5 (2026-09-23)" §9 entry recording the SMP1 12→13→14 pin moves; one current-pin statement plus a revision-5 date in ADR-0033; a checker assertion that §9 contains `- Revision {CONTRACT_REVISION} (`. | [P4] [editorial] docs/spec/SESSION_HANDLE_PROFILE_V1.md:11-13 - The revision 5 Status sentence is one unwrapped line of about 250 characters. It paraphrases SMP1's "every later selection whose method table includes version 2's row 201" (SMP1.md:723) as "every later selection", which drops the qualifier - Closure needs: re-wrap, and quote SMP1's qualifier exactly. | [P4] [identity-binding] crates/sley-protocol/src/server.rs:1179,2396-2397,2904-2915 - This predates the delta and is noted, not blocking. For `workspace.open`, check 5 runs on one head load (`session_check`→`head_binding_mixed`), and the body (fields 1-9) is built from a second, separate `self.head()` load. A head advanced by another process between the two loads would be answered to a session bound to the older root, without `SESSION_ROOT_ADVANCED`. The entity-read path was already repaired for exactly this (`session_check_retained`, `repair_entity_read_uses_single_admitted_head_load`). Field 9 now makes the returned identity something a client can adopt, but the later head-bound `query.root` fails closed - Closure needs: answer row 201 from the revision kept by the session check (or re-compare the root), with a single-head-load test like the entity-read repair.
SUMMARY: The revision 5 re-pin to SMP1 revision 14 is mechanically complete: the checker passes, the tests for v1, v2, v3 and the probe pass, and no statement in this profile about workspace.open is made false (201 stays head-bound, and the non-empty-body refusal ranks after every session check). The re-pin is still inconsistent: it adopts SMP1 revision 14's "version 2 and every later selection" scope for row 201, but leaves its own method classification at v1/v2 only, while the pinned SMP1 now admits version 3 and the server makes 306/307 head-bound there. The checker cannot see version 3 (P2). Two P3s cover the precedence misstated for 605-607 below version 3 and the missing revision 5 history/ADR record; two P4s are notes. REVISE.

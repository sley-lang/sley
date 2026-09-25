<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: ariadne; field: ariadne_contract_review_revision_6; dispatched: 2026-09-23T13:38:32Z; duration_s: 407; process_exit_code: 0 -->
# Ariadne Council review — session_handle_profile

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, the scope SHA. The working tree is clean.
- **Git commands run:**
  - `git log --oneline 2b0f1c9f..HEAD`: 12 commits.
  - `git diff --stat 2b0f1c9f..HEAD`, both repository-wide and restricted to the owned paths.
  - `git diff 2b0f1c9f..HEAD` for each of:
    - `docs/spec/SESSION_HANDLE_PROFILE_V1.md`
    - `docs/adr/ADR-0033-negotiated-session-boundary.md`
    - `scripts/check_session_handle_profile.py`
    - `scripts/test_session_handle_profile.py`
    - `crates/sley-protocol/src/server.rs`
    - `crates/sley-protocol/src/server_tests.rs`
    - `docs/spec/ERROR_CODES_V1.md`
    - `docs/spec/SMP1.md`
  - A field-by-field comparison of the `session_handle_profile` section of `machineresearch/sley-2.0/machine-summary.json` at `2b0f1c9f` against HEAD.
  - `git log`/`git show --stat` for the commits the new history cites:
    - `bf5b7e78` (2026-09-14)
    - `e0ff1371` (2026-09-16)
    - `a8b4cddb` (2026-09-23)
    - `8357c243` (2026-09-23)
    - `116d031e` (2026-09-16)
    - `97c1ec30` (2026-09-13), which added `delta_review-a4b6029.md`

    Each one touches the profile, or adds that transcript, as claimed.
- **Checkers run:**
  - `python3 scripts/check_session_handle_profile.py`: exit 0, `"result": "PASS"`, `"problems": []`, `revision 6`, `smp1_revision 15`, `capsule_revision 4`, `status S20_330_IMPLEMENTED_REVIEW_PENDING`.
  - `python3 scripts/check_smp1_contract.py`: exit 0, `"result": "PASS"`, `revision 15`.
  - `python3 -m unittest -v scripts.test_session_handle_profile`: `Ran 12 tests`, `OK`. This includes the new `VersionThreeScopeCases` (3) and `AdrPinCases` (3).
- **In-memory negative probes of the checker** (by patching `read`; nothing written to the tree):

  | Mutation | Result |
  |---|---|
  | ADR decision 7 pin set to 14 | FAIL `adr-smp1-pin` |
  | Profile marker "version 2, and version 3" removed | FAIL `spec-marker` |
  | §9 `- Revision 6 (` entry renamed | FAIL `history-current-revision` |
  | Status line set to revision 5 | FAIL `spec-revision` |
  | ADR-0033:113 "This profile pins SMP1 revision 15," set to 14 | **PASS** |
  | ADR-0033:16-17 status pin set to 14 | **PASS** |
  | Profile version 3 rows "605, 606, and 607" set to "601, 602, and 605" | **PASS** |
  | Profile "item 5 on version 2 and version 3 sessions" cut to version 2 | **PASS** |
- **Cargo tests:**
  - `cargo test --locked --offline -p sley-protocol --lib -- …`: 15 passed, 0 failed. The 15 tests are:
    - `entity_reads_are_head_bound_under_version_3`
    - `native_tags_below_version_3_refuse_at_decode_before_the_session_check`
    - `workspace_open_answers_from_the_single_checked_head_load`
    - the four earlier `workspace_open_*` tests
    - the eight T15/T47/T56 and precedence tests: `handles_name_their_expected_root`, `sessions_bind_workspace_root_and_epoch_and_handles_name_their_root`, `binding_failures_precede_budget_exhaustion`, `checks_follow_contract_order`, `issuance_binds_head_and_nonce_separates_instances`, `session_repository_and_transaction_methods_answer_deterministically`, `live_sessions_are_capped_and_restarts_forget`, `identity_session_and_frame_rules_hold_at_the_server`
  - A second run: 3 passed, 0 failed. The tests are `entity_exhausted_budget_with_stale_root_answers_binding_first`, `v3_native_calls_route_live_with_bit_and_refuse_reserved_without` and `native_selection_refuses_reserved_on_v1_paths`.
- **Files read:**
  - my round-7 transcript `evidence/review/verdicts/session_handle_profile/ariadne_contract_review_revision_5-2b0f1c9.md` (all)
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md` 784-964
  - `docs/spec/SESSION_HANDLE_PROFILE_V1.md` 1-446 (all)
  - `scripts/check_session_handle_profile.py` 1-642 (all)
  - `scripts/test_session_handle_profile.py` 196-245
  - `docs/adr/ADR-0033-negotiated-session-boundary.md` 1-30 and 85-119
  - `crates/sley-protocol/src/server.rs`: 330-360, 500-560, 1090-1300, 1365-1400, 2175-2260, 2355-2500, 2913-2955
  - `crates/sley-protocol/src/lib.rs`: 560-620, 725-830
  - `crates/sley-protocol/src/server_tests.rs`: 607-614, 3480-3560, 5605-5660, 7461-7686
  - `docs/spec/SMP1.md` revision 15 diff hunks
  - `docs/spec/NATIVE_TEST_ADMISSION_V1.md` 548-575
  - `docs/spec/ENTITY_READ_PROFILE_V2.md` 1-12 and 268-285
  - `crates/sley-txn/src/repository.rs`: 881-906, 999-1011, 2420-2440, 2540-2552
  - the `docs/WORK_PACKAGES.md` S20-330 row (line 38)

## Evidence checked

**Closure of my lane's round-7 findings (transcript `ariadne_contract_review_revision_5-2b0f1c9.md`):**

- **CLOSED: [P2] [contract] version 3 partition of 306/307** (profile §3, Status, ADR-0033; checker never read `V3_ALL`).
  - Profile:235-248 now says 306/307 are head-bound "under every version-aware selection whose method table carries them: version 2, and version 3". It states the version 3 partition as the version 2 partition plus 605/606/607, which skip check 5 like 601/602.
  - Status:16-22 and ADR-0033:12-17 and :117 agree. ADR:7-9 keeps "only in version 2" inside the dated "Earlier pin (2026-09-08)" history sentence, which is acceptable.
  - The code matches:
    - `from_tag_versioned` admits 306/307 at version 3 (lib.rs:785-822).
    - A version-aware session routes them to `dispatch_entity_read` (server.rs:1176-1178).
    - `session_check_retained` applies `head_bound_versioned` whenever `version_aware`, with no version test (server.rs:2210-2224).
  - Checker coverage:
    - It reads `V3_ALL` and requires it to be `V2_ALL` plus exactly {TestsReportRead, TestsReplay, TestsAttemptStatus} (checker:177-203).
    - It requires the gate shape and refuses any `protocol_version` test inside it (checker:204-212).
    - Revert tests cover both, and both pass.
  - `entity_reads_are_head_bound_under_version_3` passes: a stale root on version 3 with the bit refuses both tags with `SESSION_ROOT_ADVANCED`, ahead of the "junk" body decode.
  - The v3 test does not exhaust the budget. Binding-before-budget is still covered: it runs on the same version-independent path (server.rs:2248-2262) as `entity_exhausted_budget_with_stale_root_answers_binding_first` (v2, passes), and the checker now binds the gate as version-independent.
- **CLOSED: [P3] [record] §9 had no revision 5 entry.**
  - Profile:424-430 records the unrevisioned edits with their commits and dates. I verified `bf5b7e78` 2026-09-14, `e0ff1371` 2026-09-16 and `a8b4cddb` 2026-09-23 with `git log`.
  - Profile:431-445 adds the revision 5 and revision 6 entries.
  - The checker requires `- Revision {CONTRACT_REVISION} (` in §9 (checker:488-489). The probe confirms this.
- **CLOSED: [P4] [consistency] line 19 named capsule revision 3.**
  - Profile:27-30 and 291-293 now name `CONTEXT_CAPSULE_PROFILE_V1.md` revision 4, "the session arm is unchanged since its revision 3". This matches the composition pin at :38 and `CAPSULE_REVISION = 4`.
  - The §9 revision 4 text (:422-423) is dated history, and :424-426 records the move to 4.
- **OPEN: [P4] [checker] ADR SMP1 pin check was a substring that could not see the "This profile pins SMP1 revision N, the current revision" sentence.**
  - The repair anchors the status "Current pin" revision and decision 7's pin pair (checker:216-230). The decision 7 probe fails correctly.
  - The old substring check was deleted rather than anchored. The sentence my finding named (now ADR-0033:113) and the status pin pair (:16-17) are unchecked: setting either to 14 still returns exit 0 PASS.
  - This is carried as finding 3 below.
- **CLOSED: [P4] [record] revision 4 PASS overwritten without a machine-readable note.**
  - `session_handle_profile.current_delta_review_note` records the revision 4 PASS x3 with transcript `evidence/review/verdicts/session_handle_profile/delta_review-a4b6029.md`, scope `a4b60294`, commit `97c1ec30` (verified), and the replacement commit `8357c243` (verified).
  - It also records the revision 5 REVISE x3.
  - My round-7 verdict is recorded unchanged as `ariadne_contract_review_revision_5 = REVISE_0_P0_0_P1_1_P2_1_P3_3_P4`, with a dated, scoped note.

**Other revision 6 claims, checked against code and records:**

- **Reserved-tag precedence (profile:220-233).**
  - Below version 3, 605-607 are outside the decode table: `from_tag` uses `ALL` (41), and `from_tag_versioned` applies the `introduced_in` gate (lib.rs:785-822). An unknown session therefore gets `PROTOCOL_METHOD_UNSUPPORTED`. `native_tags_below_version_3_refuse_at_decode_before_the_session_check` passes, with 305 giving `SESSION_UNKNOWN`.
  - At version 3 without the bit, 605-607 decode, pass the checks, and are refused in `dispatch_admitted` through `is_reserved` (server.rs:1244-1248). `v3_native_calls_route_live_with_bit_and_refuse_reserved_without` shows this.
  - This matches SMP1 revision 15 §2 (per-selection filter, SMP1 diff at 237-251) and NATIVE appendix D:554-563.
- **`workspace.open` answers from the checked head.**
  - `session_check_mixed_retained` (server.rs:2408-2420) returns the `HeadRevision` whose binding `check_session` compared.
  - `workspace_open` uses it for format-1 heads (server.rs:2942-2945). For format-1 heads, `head_mixed`'s binding is computed from the same `self.head()` result it retains (server.rs:2375-2406), so the answer and the check 5 root agree.
  - `workspace_open_answers_from_the_single_checked_head_load` passes. The old code made two `head()` calls on this path.
  - The native-head arm is not covered; see finding 1.
- **`session.close` body deviation (profile:132-138).** `Method::SessionClose => self.session_close(session)` ignores the body (server.rs:1253). The profile records this as a known deviation and does not change it, consistent with SMP1's appendix A note.
- **Records.**
  - The SMP1 pin is 15 at profile:28 and :34, and SMP1's Status line is revision 15 (the `smp1-revision-pin` check passes).
  - ERROR_CODES_V1.md:319 says "contract draft revision 6".
  - The WORK_PACKAGES S20-330 row says revision 6, `S20_330_IMPLEMENTED_REVIEW_PENDING`, and "new-delta review pending".
  - The machine summary has `contract_revision 6`, `current_delta_review {6, PENDING ×3}`, and `implementation_complete false`. The earlier PASS fields are kept byte-for-byte, with dated notes saying they predate revisions 5 and 6.
  - No status claims COMPLETE.

## Findings

[P4] [contract] docs/spec/SESSION_HANDLE_PROFILE_V1.md:164-169 (crates/sley-protocol/src/server.rs:2942-2945, 2375-2406) - Item 5 now says `workspace.open` is "answered from the very head load that check 5 compared, so a head advanced between the check and the answer is never served to a session bound to the older root". That is true only for format-1 heads. When the session check retains `HeadRevision::Native`, `workspace_open` takes `_ => self.head()?` and reads the head pointer again through the v1 loader. If the head advances between the check and that reload to a format-1 child, which the transaction layer admits for native parents (sley-txn repository.rs:2424-2428, 2544-2546), the reload can succeed and answer a root check 5 never compared. This needs an out-of-process writer; it is the same race class as the double head load repaired in this round. - Closure evidence: the native-retained arm refuses from the retained revision without a second pointer read, or the sentence is narrowed to format-1 heads; plus a test for the native-retained arm.

[P4] [consistency] crates/sley-protocol/src/server.rs:2198-2201 - The doc comment on `head_bound_versioned` still says the entity reads are "head-bound only in protocol version 2". This is the helper the checker binds as the session partition. Revision 6 §3 (profile:235-248) says version 2 and version 3, and the server applies the helper under version 3 (server.rs:2216-2220). The gate record §13.2 (GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:883) says "the server doc comment agree[s]", but no server doc comment about 306/307 changed in `git diff 2b0f1c9f..HEAD`. - Closure evidence: reword the comment to "version 2 and version 3" (or "every version-aware selection carrying them"), or correct the gate-record claim.

[P4] [checker] scripts/check_session_handle_profile.py:216-230 (docs/adr/ADR-0033-negotiated-session-boundary.md:113-115, 16-17) - This is my round-7 ADR pin P4, carried because it is only partly repaired. `adr_pin_problems` anchors the "Current pin" revision and decision 7's pin pair. But the old `SMP1 revision {N}` substring check was removed rather than anchored, so the normative sentence the finding named ("This profile pins SMP1 revision 15, the current revision", ADR-0033:113) and the status pin pair ("pins SMP1 revision 15 and capsule revision 4", :16-17) are no longer checked at all. Probe: setting either to 14 still gives exit 0, PASS. - Closure evidence: anchor `This profile pins SMP1 revision {SMP1_REVISION},` and the status pin pair, each with a revert test.

[P4] [checker] scripts/check_session_handle_profile.py:177-213, 371-394 (docs/spec/SESSION_HANDLE_PROFILE_V1.md:242-248) - The version 3 check compares the code's `V3_ALL` against a hardcoded `NATIVE_V3_ADDITIONS` and never reads the profile's stated version 3 partition. Profile:182-184 says the checker fails closed on any difference, but only the one marker sentence at :237-238 is bound. Probes: rewriting the partition rows as "601, 602, and 605" passes, and cutting "item 5 on version 2 and version 3 sessions" to version 2 passes. - Closure evidence: parse the version 3 rows (and the item 5 version scope) from the extension paragraph, compare them by tag with `V3_ALL − V2_ALL`, and add revert tests.

## Assessment

Revision 6 closes the lane's P2, its P3 and two of its three P4s:
- The profile, its Status line and ADR-0033 now state 306/307 as head-bound under version 2 and version 3. That is exactly what the server does: a version-independent `version_aware` gate over `head_bound_versioned`, reached through `from_tag_versioned`'s `introduced_in` rule.
- The checker reads `V3_ALL` and the gate shape, with revert tests.
- A version 3 stale-root server test passes.
- The §9 history records revisions 5 and 6 and the three unrevisioned edits, with commits I verified.
- The capsule revision is named consistently.
- The machine summary preserves the revision 4 round and my revision 5 verdict byte-for-byte.
- The new reserved-tag precedence text matches decode (lib.rs), dispatch (server.rs:1244-1248), SMP1 revision 15 §2 and NATIVE appendix D. It is exercised by the new below-version-3 test and the existing version-3-without-bit test.
- The single-checked-head repair makes `workspace.open` answer from the revision check 5 compared, for format-1 heads.

What remains is minor:
- the native-head reload arm (finding 1);
- a stale server doc comment the gate record says was updated (finding 2);
- the ADR pin sentence my round-7 P4 named, still unchecked, so that finding is OPEN (finding 3);
- the profile's version 3 partition text, not bound by the checker (finding 4).

None of these misstates what the implementation does in an ordinary serving path, so the lane accepts revision 6.

Observations outside this lane or not counted:
- ENTITY_READ_PROFILE_V2.md:277 still tells consumers the tags are "head-bound only in protocol version 2". That text belongs to S20-310.
- Profile:238-239 glosses version 3 as "the union of the version 1 and version 2 tables". That is the same phrasing as SMP1:64 and :745, but it omits "plus the native rows". The next sentence (:242-244) states the partition exactly.
- Profile:30 is a new unwrapped line.
- The one-load test counts only `head()` calls. `head_mixed` also loads the head through the any-format probe first. The check-versus-answer consistency the contract claims still holds.

VERDICT: PASS_0_P0_0_P1_0_P2_0_P3_4_P4
SECTION: session_handle_profile
FIELD: ariadne_contract_review_revision_6
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P4] [contract] docs/spec/SESSION_HANDLE_PROFILE_V1.md:164-169 (crates/sley-protocol/src/server.rs:2942-2945, 2375-2406) - the "answered from the very head load that check 5 compared" claim holds only for format-1 heads; a retained Native head falls to `_ => self.head()?`, a second pointer read that can succeed on a format-1 child written in between (sley-txn repository.rs:2424-2428) and serve a root check 5 never compared - needs the native arm to refuse from the retained revision without reloading, or the sentence narrowed, plus a test; [P4] [consistency] crates/sley-protocol/src/server.rs:2198-2201 - the `head_bound_versioned` doc comment still says "head-bound only in protocol version 2", contradicting revision 6 §3 (profile:235-248) and the server's version 3 use, while gate record §13.2 (line 883) says the server doc comment agrees - needs the comment reworded or the record corrected; [P4] [checker] scripts/check_session_handle_profile.py:216-230 (docs/adr/ADR-0033-negotiated-session-boundary.md:113-115, 16-17) - round-7 ADR pin P4 carried: the substring check was removed, not anchored, so "This profile pins SMP1 revision 15, the current revision" and the status pin pair are unchecked (probe: 14 still gives PASS) - needs an anchored check and revert tests; [P4] [checker] scripts/check_session_handle_profile.py:177-213, 371-394 (docs/spec/SESSION_HANDLE_PROFILE_V1.md:242-248) - the profile's stated version 3 partition rows and the item 5 version scope are not compared with `V3_ALL` (probes: "601, 602, and 605" and a version-2-only item 5 both PASS) - needs the paragraph parsed and compared by tag, with revert tests
SUMMARY: Revision 6 closes this lane's round-7 P2 (the version 3 scope of the 306/307 head-bound extension, now stated in the profile, Status and ADR-0033, bound by the checker's `V3_ALL` and gate-shape checks, and proven by a passing version 3 stale-root test), the P3 (the §9 revision 5 and 6 entries and the recorded unrevisioned edits), and two of the three P4s (capsule revision wording, `current_delta_review_note`). The ADR pin P4 stays OPEN: the sentence it named is now unchecked, as the probe shows. Checker exit 0 PASS at revision 6, SMP1 checker exit 0 PASS at revision 15, 12 checker unit tests OK, and 18 bounded sley-protocol tests passed. Four P4 notes remain (native-head reload arm, stale server doc comment, unanchored ADR pin sentences, unbound version 3 partition text), so the lane accepts the package at contract revision 6 with no P0 to P3.

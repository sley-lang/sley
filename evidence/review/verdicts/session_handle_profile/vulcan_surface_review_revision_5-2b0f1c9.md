<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 2b0f1c9f4940c020565137891a49a3769cd02061; role: vulcan; field: vulcan_surface_review_revision_5; dispatched: 2026-09-23T12:14:21Z; duration_s: 384; process_exit_code: 0 -->
# Vulcan Council review — session_handle_profile

Harness: claude-code
Reviewed checkpoint: 2b0f1c9f4940c020565137891a49a3769cd02061

I checked the scope first: `git rev-parse HEAD` printed `2b0f1c9f4940c020565137891a49a3769cd02061`, which matches the scope SHA. The branch is work/succ-context-impl. The only untracked files are other lanes' verdict files under `evidence/review/verdicts/`. I did not read the untracked Ariadne or Nabu verdicts for this section.

What I ran and what it returned:
- `git diff ab42a3a9..HEAD -- docs/spec/SESSION_HANDLE_PROFILE_V1.md` shows 8 insertions and 6 deletions. `git log ab42a3a9..HEAD` on the file lists two commits. a8b4cddb re-pinned SMP1 to revision 13 without a profile revision. 8357c243 is profile revision 5, which pins SMP1 revision 14.
- `git show a8b4cddb` and `git show 8357c243` for the profile, ADR-0033, ENTITY_READ_PROFILE_V2, the checker and its tests.
- `git diff ab42a3a9..HEAD -- docs/spec/SMP1.md` shows 101 insertions and 19 deletions. I searched the diff for every session, handle, 100/101/102/304, `max_sessions` and head-bound change.
- A python3 heredoc compared the `session_handle_profile` section of machine-summary.json at ab42a3a9, at the parent of each commit, and at HEAD.
- `python3 scripts/check_session_handle_profile.py` exited 0 with `"result": "PASS"`, `"problems": []`, `"revision": 5`, `"smp1_revision": 14`, `"capsule_revision": 4` and `"status": "S20_330_IMPLEMENTED_REVIEW_PENDING"`.
- `python3 -m unittest scripts.test_session_handle_profile` ran 6 tests, all OK.
- I ran the real checker `main()` in memory against mutated spec and ADR text, with the module's `read` patched (a table of results is under Evidence checked).
- `cargo test --locked --offline -p sley-protocol --lib workspace_open` passed 4 of 4: the v1 test (no field 9), the v2 test, the v3 test, and the probe non-waiting/no-rewrite test.
- `cargo test --locked --offline -p sley-protocol --lib session` passed 22 of 22.
- `cargo test --locked --offline -p sley-protocol --lib binding_failures_precede_budget_exhaustion` passed 1 of 1.

Files I read:
- `docs/spec/SESSION_HANDLE_PROFILE_V1.md`, all 1-395
- `docs/spec/SMP1.md` 1-100, 200-400 and 670-770
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md` 540-594
- `docs/adr/ADR-0033-negotiated-session-boundary.md`, all 1-116
- `scripts/check_session_handle_profile.py`, all 1-579
- `scripts/test_session_handle_profile.py` 1-75
- `crates/sley-protocol/src/server.rs` 1096-1379, 2155-2270, 2390-2460, 2717-2752, 2890-2933 and 3284-3318
- `crates/sley-protocol/src/lib.rs` 560-584 and 728-825
- `crates/sley-repo/src/index_cache.rs` 239-300
- `crates/sley-protocol/src/server_tests.rs` 610-640, 1493-1523 and 7385-7585
- `docs/WORK_PACKAGES.md`, the S20-330 row
- `docs/spec/ERROR_CODES_V1.md` 318-321
- `docs/audits/S20_330_NEGOTIATED_SESSION_CLOSEOUT.md` 1-20 and 95-110

## Evidence checked

**What the profile says about workspace.open (201) versus the code:**
- **201 is still head-bound.** `head_bound` lists `Method::WorkspaceOpen` (server.rs:2167-2184). The generic `session_check` uses that function for every version (server.rs:2396-2402), so 201 gets check 5 under v1, v2 and v3. This matches profile :170.
- **The status-line claims are accurate.** Profile :12 says SMP1 revision 14 answers 201 with `open_summary` under version 2 and later, and refuses a non-empty body under every version. `workspace_open` refuses a non-empty body unconditionally (server.rs:2905-2907). Field 9 is gated on `protocol_version >= PROTOCOL_VERSION_V2` (server.rs:2909). The v1, v2 and v3 tests each assert `vec![0]` is refused with `PayloadInvalid` (server_tests.rs:638, :7455, :7529), and all three pass. The profile's "every later selection" drops SMP1's qualifier "whose table carries row 201" (SMP1.md:682, :722-725). That is harmless because v3 is the only later selection and it carries row 201.
- **The new 201 refusal comes after every binding check.** `dispatch` order is:
  - liveness and remembered close (:1162-1169)
  - request-identity admission (:1170-1172)
  - `session_check`: workspace, epoch and bound root (:1179)
  - the budget test and one-unit debit (:1181-1193)
  - then `dispatch_admitted` → `profile.admits` (:1232) → `workspace_open`'s body check (:2905)
  
  So a non-empty 201 body on a stale session answers `SESSION_ROOT_ADVANCED`, and on an exhausted but valid session it answers `PROTOCOL_LIMIT_EXCEEDED`. This matches profile §3 :138-159. A refused body costs the one dispatch unit, as SMP1 section 7 requires.
- **Field 9 describes the same head as fields 1-8.** The probe takes the `head` that `workspace_open` loaded (:2908-2910). The cache path is keyed by `revision.state_root().root` and `accept_cached` validates against that revision (index_cache.rs:270-273). A session therefore never receives a snapshot identity for a different root than the one in fields 1-8. The probe is non-blocking (server.rs:2929) and read-only; the probe test passes.
- **The profile's statement that callers learn the new root from any head-bound read after renewing (:243-244) still holds.** Field 2 of `open_summary` is still the `StateRoot` (server.rs:3307).
- **No session-owned SMP1 text changed between revisions 12 and 14.** Rows 100, 101, 102 and 304, section 3, `max_sessions`, and the appendix A rows 100/101/102/304 are identical in the 12→14 diff. The only rows that changed are 201, the v3 negotiation text and the empty-body paragraph.

**Whether the re-pin is complete:**
- **Correctly updated:** profile :3, :12, :19 and :24; ADR-0033 :11-13 and :107-111; the WORK_PACKAGES S20-330 row; ERROR_CODES :319 ("contract draft revision 5"); checker :41 and :45.
- **The summary section changes only what it should.** Between ab42a3a9 and HEAD, only `contract_revision` (4→5), `current_delta_review` (rev 4 all PASS → rev 5 all PENDING), `status` (COMPLETE → IMPLEMENTED_REVIEW_PENDING), `implementation_complete` (true → false) and `status_note` change. The historical base, `_initial` and closed-list fields are preserved.
- **Checker mutation probes (real `main()`, patched `read`):**

| Mutation | Result |
|---|---|
| None (control) | PASS |
| Spec pin sentence :24 changed to revision 13 | FAIL `spec-marker` |
| Spec intro :19 changed from 14 to 12 | PASS |
| Status-line rev-14 sentence changed to rev 12 | PASS |
| §9 truncated before the revision 4 entry | PASS |
| "head-bound only in protocol version 2" changed to "versions 2 and 3" | PASS |
| "201 stays head-bound" clause deleted | PASS |
| ADR consequences sentence changed to "SMP1 revision 12" | PASS, because the status line still has 14 |
| Every "SMP1 revision 14" in the ADR changed | FAIL `adr-smp1-pin` |

**How version 3 composes with this profile:**
- SMP1 revision 12 said that the versioned negotiation supported versions "1 and 2 only" and refused any other result, "including 3". Revision 14 now admits selected version 3, defined as the union of the v1 and v2 tables plus the native rows (SMP1.md:219-227; frame entrypoints at :256-257).
- The code agrees. `from_tag_versioned` admits 306 and 307 at version 3 (lib.rs:806-823, since `introduced_in` = 2). On a version-aware server they route to `dispatch_entity_read` (server.rs:1176-1177), which applies `head_bound_versioned` with no version condition (server.rs:2190-2193, :2204-2208).

## Findings

[P2] [contract-partition] docs/spec/SESSION_HANDLE_PROFILE_V1.md:212-220 (with :11-13, :161-167) - The "Protocol version 2 extension" says 306/307 are "head-bound only in protocol version 2" and describes only v1 sessions (refused) and v2 sessions (head-bound). The re-pinned SMP1 revision 14 (SMP1.md:219-227, :256-257; NATIVE_TEST_ADMISSION_V1.md:547-551) now admits version 3 and defines it to include 306 and 307. SMP1 revision 12 had refused version 3. So under v3 the profile's closed partition leaves two live methods unclassified. That contradicts its own rules (:161-167) that "every SMP1 method tag is classified" and that head-boundness "is read from the lists and never derived". The rev-5 claim "no session clause changes" (:12-13) is therefore incomplete. The server is stricter than the text: 306/307 are head-bound on v3 (server.rs:2190-2193, :2204-2208; lib.rs:806-823), and the doc comment at server.rs:2186-2189 repeats the "only in protocol version 2" wording. The risk is a spec-level T15-class hole: an implementation that follows the text literally could serve v3 entity reads past `SESSION_ROOT_ADVANCED`. The checker cannot see this because it parses only `ALL` and `V2_ALL` (check_session_handle_profile.py:216-225), never `V3_ALL` (lib.rs:568), and the mutation probe rewording that sentence still PASSed. - Closure evidence: a profile revision that states the v3 partition (306/307 head-bound under every selection whose table carries them, with the native rows delegated as at :206-210) and a matching server doc comment; a checker gate that parses `V3_ALL` and binds the v3 partition to `head_bound_versioned`, with a revert test.

[P3] [revision-record] docs/spec/SESSION_HANDLE_PROFILE_V1.md:360-394 - §9 Revision history has entries for revisions 1-4 but none for revision 5. Nothing in §9 records the 2026-09-23 SMP1 re-pins (12→13 at a8b4cddb without a profile revision, then 13→14 as revision 5). The last pin statement in the history is revision 4's "the SMP1 pin follows SMP1 to revision 12" (:392-393), which now contradicts the header pin (:24). The checker only anchors the heading "## 9. Revision history" (check_session_handle_profile.py:89); the mutation probe that truncated §9 still PASSed. - Closure evidence: a "Revision 5 (2026-09-23)" entry in §9 that records the re-pin, and ideally a checker anchor tying the last §9 entry to `CONTRACT_REVISION`.

[P3] [pin-drift] docs/adr/ADR-0033-negotiated-session-boundary.md:87-98 (also :15-16) - Decision 7 still says the stage checker "cross-checks the SMP1 revision 12 and capsule revision 3 pins". The checker actually pins SMP1 14 and capsule 4 (check_session_handle_profile.py:45-46). The Date line lists decision records only through revision 4. The checker's `adr-smp1-pin` check (:425-426) only looks for the substring "SMP1 revision 14" anywhere in the ADR, and the status line and consequences paragraph satisfy it. That hides the stale normative sentence, as the probe confirmed: the ADR fails only when every "SMP1 revision 14" is removed. - Closure evidence: decision 7 updated to SMP1 revision 14 and capsule revision 4 (or made revision-neutral), a revision 5 record on the Date line, and optionally a checker anchor on the decision-7 sentence.

[P4] [pin-consistency] docs/spec/SESSION_HANDLE_PROFILE_V1.md:18-19, :262-263 vs :28 - The profile names the capsule profile at "S20-320 full, revision 3" on lines 19 and 263, but pins it at revision 4 on line 28. The checker enforces line 28 (CAPSULE_REVISION = 4), and CONTEXT_CAPSULE_PROFILE_V1.md:3 reads revision 4. This predates the delta (bf5b7e78, 2026-09-14), but the rev-5 edit touched line 19 and updated only the SMP1 half. - Closure evidence: lines 19 and 263 either state revision 4 or explicitly say they mean the revision that defined the arm.

[P4] [fail-open-parse] docs/spec/SMP1.md:755-760; crates/sley-protocol/src/server.rs:1242 - SMP1 revision 14 now records that `session.close` (102) ignores a non-empty request body, contrary to its empty-body grammar (SMP1.md:344, :678). SMP1 row 102 names this package as owner. The code confirms the method ignores its body: `session_close(session)` never reads it. The profile's §2 (:122-124) neither adopts the refusal nor records the deviation for its own method. The impact is low, since closing already requires the live session name. - Closure evidence: either a recorded deviation note in the profile (§2 or §8) or a later revision that refuses non-empty 102 bodies with `PROTOCOL_PAYLOAD_INVALID`, with a test.

## Assessment

The re-pin is mechanically sound. The spec, checker, summary section, work-package row, ERROR_CODES paragraph and ADR status line all move to profile revision 5 and SMP1 revision 14. The status leaves COMPLETE with the current-delta review bound to revision 5 and all three lanes PENDING, and the historical verdict fields are preserved. The checker, its 6 unit tests and 27 bounded sley-protocol tests pass.

On workspace.open, SMP1 revision 14 does not make anything false that this profile says. 201 stays head-bound under every version. The new non-empty-body refusal comes after all six ordered session checks, so binding failures and the budget failure keep their precedence. Field 9 is derived from the same head as fields 1-8.

The re-pin still does not hold up. Revision 14 is the first SMP1 revision this profile composes that admits protocol version 3. Revision 12 had explicitly refused it. Under version 3 the profile's version-scoped classification of 306/307 ("head-bound only in protocol version 2") leaves two live methods unclassified. That is a gap in the profile's central safety claim, the closed head-bound partition. The code is fail-closed, but the text and checker cannot detect the drift, and the "no session clause changes" claim is not accurate. The missing §9 entry and the stale ADR decision 7 are consistency defects that the checker cannot see either. My lane does not accept revision 5 until the P2 is answered.

VERDICT: REVISE_0_P0_0_P1_1_P2_2_P3_2_P4
SECTION: session_handle_profile
FIELD: vulcan_surface_review_revision_5
SCOPE_SHA: 2b0f1c9f4940c020565137891a49a3769cd02061
FINDINGS: [P2] [contract-partition] docs/spec/SESSION_HANDLE_PROFILE_V1.md:212-220 (with :11-13, :161-167) - the version 2 extension says 306/307 are "head-bound only in protocol version 2" but re-pinned SMP1 revision 14 admits version 3 (SMP1.md:219-227; NATIVE_TEST_ADMISSION_V1.md:547-551), which contains them; the closed partition leaves them unclassified under v3 while the server makes them head-bound there (server.rs:2190-2193, :2204-2208; lib.rs:806-823), so "no session clause changes" is incomplete, and the checker reads only ALL/V2_ALL (check_session_handle_profile.py:216-225) - closure: a profile revision stating the v3 partition, a matching server doc comment, and a V3_ALL checker gate with a revert test | [P3] [revision-record] docs/spec/SESSION_HANDLE_PROFILE_V1.md:360-394 - §9 has no revision 5 entry and its last pin statement (SMP1 revision 12, :392-393) contradicts the header pin; the checker anchors only the heading - closure: a Revision 5 §9 entry and optionally a checker anchor | [P3] [pin-drift] docs/adr/ADR-0033-negotiated-session-boundary.md:87-98 (also :15-16) - decision 7 still states "SMP1 revision 12 and capsule revision 3" pins, hidden by the substring adr-smp1-pin check (checker :425-426) - closure: decision 7 and the Date line updated to revision 5, SMP1 14, capsule 4 | [P4] [pin-consistency] docs/spec/SESSION_HANDLE_PROFILE_V1.md:18-19, :262-263 vs :28 - capsule named at revision 3 on lines 19 and 263 but pinned at revision 4 on line 28 (pre-existing since bf5b7e78) - closure: align or state the intended meaning | [P4] [fail-open-parse] docs/spec/SMP1.md:755-760; crates/sley-protocol/src/server.rs:1242 - SMP1 rev 14 records that the S20-330-owned session.close (102) ignores a non-empty body; the profile's §2 does not address it - closure: a recorded deviation note or a refusal with a test
SUMMARY: The rev-5 re-pin to SMP1 revision 14 is mechanically consistent (checker PASS, exit 0; 6 unittests and 27 bounded sley-protocol tests pass), and nothing the profile says about workspace.open is falsified: 201 stays head-bound, the new non-empty-body refusal follows all six session checks, and field 9 describes the same head as fields 1-8. SMP1 revision 14 is, however, the first composed revision to admit protocol version 3. The profile's "head-bound only in protocol version 2" classification leaves the live 306/307 methods unclassified there, a P2 gap in the closed head-bound partition that the checker cannot detect. §9 lacks a revision 5 entry, and ADR-0033 decision 7 keeps stale SMP1 12 and capsule 3 pins (P3 each), so the verdict is REVISE.

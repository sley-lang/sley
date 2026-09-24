<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: nabu; field: nabu_architecture_review_revision_6; dispatched: 2026-09-23T13:38:32Z; duration_s: 420; process_exit_code: 0 -->
# Nabu Council review — session_handle_profile

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, so the scope matches. `git status --short` was empty.
- **Git reads.**
  - `git log --oneline 2b0f1c9f..HEAD` listed 12 commits. This package's changes are in `004cf5be` (code), `60be11c8` (contracts, checker, tests), `0b21dcb0` (ADR-0033) and `1d8086fc`/`7b1e060c` (summary fields).
  - `git diff --stat 2b0f1c9f..HEAD`.
  - `git diff 2b0f1c9f..HEAD` on `docs/spec/SESSION_HANDLE_PROFILE_V1.md`, `scripts/check_session_handle_profile.py`, `scripts/test_session_handle_profile.py`, `crates/sley-protocol/src/server.rs`, `crates/sley-protocol/src/server_tests.rs`, `docs/adr/ADR-0033-negotiated-session-boundary.md`, `docs/spec/ENTITY_READ_PROFILE_V2.md` and `docs/spec/ERROR_CODES_V1.md`.
  - `git show 2b0f1c9f:crates/sley-protocol/src/server.rs` lines 2183-2200, to compare the `head_bound_versioned` doc comment.
  - `git log -1 <sha> -- docs/spec/SESSION_HANDLE_PROFILE_V1.md` for bf5b7e78 (2026-09-14), e0ff1371 (2026-09-16) and a8b4cddb (2026-09-23). All three touch the profile on the dates §9 records.
  - A key-by-key diff of `machine-summary.json` `session_handle_profile` between 2b0f1c9f and HEAD.
  - The sha256 of my revision-5 transcript (`801ee0d4…aae`) matches its entry in `evidence/review/rounds/context-r7-2b0f1c9.json`.
- **Checker.** `python3 scripts/check_session_handle_profile.py` exited 0 with `"result": "PASS"`, `"revision": 6`, `"smp1_revision": 15`, `"capsule_revision": 4`, `"status": "S20_330_IMPLEMENTED_REVIEW_PENDING"` and `"problems": []`.
- **Checker unit tests.** `python3 -m unittest scripts.test_session_handle_profile -v` exited 0: 12 tests OK. That includes the new `VersionThreeScopeCases` (3) and `AdrPinCases` (3).
- **Cargo run 1.** `cargo test --locked --offline -p sley-protocol --lib` with filters for workspace_open, the v3 head-bound test, the native-tag decode test, check order, budget precedence, binding, handles, issuance, caps and the entity-read single load: `test result: ok. 15 passed; 0 failed`. That includes:
  - `workspace_open_answers_from_the_single_checked_head_load`
  - `entity_reads_are_head_bound_under_version_3`
  - `native_tags_below_version_3_refuse_at_decode_before_the_session_check`
  - `workspace_open_under_version_1_never_carries_field_9`
  - `workspace_open_v2_discloses_only_the_materialized_head_snapshot`
  - `workspace_open_v3_answers_the_version_2_open_summary`
  - `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record`
  - `repair_entity_read_uses_single_admitted_head_load`
  - the T15, T47 and T56 matrix tests
- **Cargo run 2.** Filters `session_repository_and_transaction_methods_answer_deterministically`, `entity_read` and `head_load`: `test result: ok. 12 passed; 0 failed`.
- **In-memory checker probes.** I loaded the module and patched `read`; the tree was not touched.
  1. The server's entity-read dispatch shortcut gated to `protocol_version == PROTOCOL_VERSION_V2` → rc 0, PASS. The checker does not see this gate. The new v3 server test asserts `SESSION_ROOT_ADVANCED` under v3, so it would catch it; I established that by reasoning, not by running it.
  2. `workspace_open` ignoring `retained` while keeping the marker text → rc 0, PASS. The checker anchors only text; the single-load test covers the format-1 arm.
  3. A `- Revision 6 (` history entry emptied down to its prefix → PASS. That is the prefix anchor I asked for.
  4. `TestsReplay` added to `head_bound` → rc 1, FAIL, with `head-bound-drift … server-only=[606]`. The v3 partition claim that "605-607 skip check 5" is enforced.
- **Files read:**
  - `docs/spec/SESSION_HANDLE_PROFILE_V1.md`: full round diff, plus 160-172, 212-250 and 405-445
  - `scripts/check_session_handle_profile.py`: 1-180 and 233-642, plus the diff
  - `crates/sley-protocol/src/server.rs`: 330-360, 1090-1300, 1355-1361, 2170-2240, 2355-2500 and 2930-2955
  - `crates/sley-protocol/src/server_tests.rs`: new tests 7588-7686
  - `crates/sley-protocol/src/lib.rs`: 806-824
  - `crates/sley-txn/src/repository.rs`: 870-1040, 2202-2240, 2322-2345, 2420-2435, 2538-2600 and 23715-23770
  - `crates/sley-repo/src/native_exchange.rs`: 1270-1295 and 1615-1645
  - `docs/adr/ADR-0033-negotiated-session-boundary.md`: 1-40, plus the diff
  - `docs/spec/ENTITY_READ_PROFILE_V2.md`: 268-282, plus the diff
  - `docs/spec/SMP1.md`: Status line and the row-201 qualifier (whitespace-flattened search), plus appendix A's empty-body sentence
  - `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: 784-963
  - `evidence/security/{T15,T47,T56}/matrix.json`: test lists
  - my revision-5 transcript, in full

## Evidence checked

- **Version 3 scope (prior P2).**
  - Profile §3:235-248 now states that 306/307 are head-bound "under every version-aware selection whose method table carries them: version 2, and version 3". It defines the v3 partition as the v2 partition plus 605/606/607, which skip check 5 like 601/602.
  - The Status paragraph (:9-20) and §9 (:437-445) agree with that.
  - Server gate at server.rs:2216-2220: `if self.version_aware { head_bound_versioned } else { head_bound }`. `from_tag_versioned` (lib.rs:806-824) resolves 306/307 at v2 and v3.
  - Checker `version_gate_problems` (:180-214):
    - reads `V3_ALL` (46) and requires exactly the three native additions over `V2_ALL`
    - requires the gate's shape
    - refuses a `protocol_version` literal in it
  - Its revert tests pass.
  - Server test server_tests.rs:7611-7657 swaps in an advanced root under the same workspace. At v3 it gets `SESSION_ROOT_ADVANCED` for both tags. Because the body is `junk`, the test also shows that check 5 runs before the body decode.
- **Reserved-tag precedence (prior P3).**
  - Profile :220-233 splits the refusal point:
    - in-table tags (305/503/601/602 under every version; 605-607 at v3 only) pass checks 1-4 and 6 and are then refused (server.rs:1244-1248)
    - below v3, 605-607 are refused at decode (server.rs:1114-1120) before liveness (:1162)
  - The test at server_tests.rs:7660-7686 shows that an unknown session under v2 gets `PROTOCOL_METHOD_UNSUPPORTED` for 605-607 and `SESSION_UNKNOWN` for 305.
- **Revision record (prior P3).**
  - §9 has the "Unrevisioned edits" entry (bf5b7e78, e0ff1371 and a8b4cddb, each verified against git), plus "Revision 5 (2026-09-23)" and "Revision 6 (2026-09-23)".
  - ADR-0033:3-17 has one "Current pin (2026-09-23)" at revision 6. The 2026-09-08 pin is marked "Earlier". The Date line records revisions 5 and 6.
  - The checker asserts `- Revision 6 (` after the §9 heading (:488-489) and anchors the ADR current pin and decision-7 pins (`adr_pin_problems`, :217-230), with revert tests.
- **Editorial (prior P4).** The Status sentence is wrapped. It quotes SMP1's qualifier "under every later selection whose method table includes version 2's row 201" exactly (SMP1 Status and the §201 text match after whitespace flattening). A new 93-character line, profile :30, came in with the capsule-pin edit. It is trivial and not raised.
- **Single checked head for `workspace.open` (prior P4).**
  - Dispatch (server.rs:1179-1205) now calls `session_check_mixed_retained` (:2410-2420) for row 201 and passes the retained `HeadRevision` to `workspace_open` (:2942-2945).
  - For a format-1 head, the binding that check 5 compared and the answered revision are the same `VerifiedRevision`. That is `head_mixed`'s own `self.head()` at :2398; its preceding any-format receipt read only picks the format.
  - The new test asserts `head_load_count() == 1`. The old code path had two `head()` calls, so the test is sensitive to a revert.
  - For a native-format checked head, the `_ => self.head()?` arm re-loads the head (finding 1).
- **Consumer anchors.** ERROR_CODES (:319) names contract draft revision 6. The WORK_PACKAGES S20-330 row names revision 6 (checker :494-498).
- **Machine summary.** Changes since 2b0f1c9f:
  - `contract_revision` 5→6
  - `current_delta_review` {6, PENDING×3}
  - a `current_delta_review_note` keeping the revision-4 PASS and revision-5 REVISE history
  - three `_revision_5` REVISE fields with dated notes; my field reads `REVISE_0_P0_0_P1_1_P2_2_P3_2_P4`, which matches my transcript
  - historical PASS fields left byte-exact, with dated "does not review revision 5 or 6" notes

  Status stays `S20_330_IMPLEMENTED_REVIEW_PENDING` and `implementation_complete` stays false. No GA or complete claim is made.
- **`session.close` body note.** Profile :134-138 matches SMP1 appendix A's list of empty-request rows that still ignore a body. The code (server.rs:1355-1361) is unchanged.

## Findings

[P3] [fail-closed] crates/sley-protocol/src/server.rs:2942-2945 - Revision 6 §3 item 5 (SESSION_HANDLE_PROFILE_V1.md:166-169) states without condition that `workspace.open` "[is] answered from the very head load that check 5 compared, so a head advanced between the check and the answer is never served to a session bound to the older root". The repair keeps the checked head only when it is format-1. The `_ => self.head()?` arm makes a second, unchecked head load whenever check 5 compared a native-format head. Sessions do bind native heads: `session_open` and `session_check_mixed_retained` both go through `head_binding_mixed` (server.rs:2444, 2415). A format-1 child of a native parent is a supported shape (sley-txn repository.rs:2427-2429, 2544-2546, 2559-2581), and native exchange import accepts format-1 receipts (sley-repo native_exchange.rs:1624-1634). So if another process moves the head from a native root to a format-1 child between check 5 and the reload, the child's summary goes to a session bound to the native root without `SESSION_ROOT_ADVANCED`. The new test (server_tests.rs:7593-7605) covers only a format-1 head, and the checker anchors only marker text (probe 2 PASSes). This is my revision-5 P4, narrowed to native checked heads. It is raised to P3 because the guarantee is now normative text and gate record §13.3:909 marks it FIXED - Closure needs: answer the Native arm without a second load, either by returning the native-head refusal directly or by re-comparing the reloaded root with the checked binding and answering `SESSION_ROOT_ADVANCED`; a server test over a native accepted head showing no summary is built from a reload; or else scope §3 item 5 explicitly to format-1 heads.
[P3] [duplicated-authority] crates/sley-protocol/src/server.rs:2198-2201 - The doc comment on `head_bound_versioned`, the helper that implements the versioned partition, still says the entity reads are "head-bound only in protocol version 2". It cites `ENTITY_READ_PROFILE_V2.md` §6 as its contract, and that section (ENTITY_READ_PROFILE_V2.md:274-278) still tells the session profile and checker that "both new tags are head-bound only in protocol version 2". Both contradict profile revision 6 §3:235-248. Gate record §13.2 (GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:883) says "the Status line, section 3, ADR-0033, and the server doc comment agree". In fact this comment is byte-identical to 2b0f1c9f:server.rs:2186-2189. ENTITY_READ was amended this round (Status line and §2) without touching §6. The runtime is correct (gate at :2216-2220, and the v3 test passes), so this is an authority and record defect, not a safety defect - Closure needs: re-word the helper comment to "version 2 and version 3" and cite SESSION_HANDLE_PROFILE_V1 §3 as the classification authority; amend ENTITY_READ §6, or mark it superseded by session profile revision 6, within that document's own review scope; correct the §13.2 sentence; optionally anchor the comment in the checker.

## Assessment

Status of each finding from my revision-5 transcript (2b0f1c9f):
- [P2] contract-scope (306/307 version 3 partition; checker blind to V3_ALL and the version gate): CLOSED. All three closure items are in place: the §3 statement of the v3 partition, the `V3_ALL` and gate-shape checks with revert tests, and the v3 stale-root server test. All were verified by checker, unittest and cargo runs.
- [P3] contract-accuracy (605-607 precedence below version 3): CLOSED. §3:220-233 is reworded, the code order matches (decode :1114-1120 before liveness :1162), and the new test passes.
- [P3] record-completeness (§9 revision 5 entry, unrevisioned edits, ADR-0033 dual current pin, checker history anchor): CLOSED. The §9 revision 5 and 6 entries and the unrevisioned-edit record are present, with commits verified. ADR-0033 has a single current pin and a dated line. The checker has `history-current-revision` and `adr_pin_problems` with revert tests.
- [P4] editorial (unwrapped Status line, dropped SMP1 qualifier): CLOSED. The line is wrapped and the qualifier is quoted exactly.
- [P4] identity-binding (`workspace.open` double head load): OPEN, narrowed. It is closed for format-1 checked heads (one retained load, with a test that catches a revert). A native-format checked head still triggers an unchecked reload. Carried as finding 1 at P3.

Architecturally, revision 6 does what the round required. The version-3 scope of the session classification is now stated in the owning contract, which matches the server, and the checker binds it structurally. The v3 partition is exact (46 rows, three native additions, none head-bound), and reverting the gate fails both the checker and the server test. The reserved-tag precedence matches the decode order. The revision record chain (profile §9, ADR-0033, WORK_PACKAGES, ERROR_CODES, summary) is consistent and anchored. Historical verdicts are preserved byte-exact with dated notes, and nothing claims completion.

Two minor defects remain:
1. The new single-head guarantee in §3 item 5 is unconditional, but the implementation still reloads when the checked head is native-format. That is narrow and fail-closed in the common case, but the text is false on that arm.
2. The helper that implements the partition, and the sibling ENTITY_READ §6 it cites, still say "version 2 only", while the gate record says they agree.

Neither affects correctness of the partition itself. My lane accepts revision 6 with these two P3s open.

VERDICT: PASS_0_P0_0_P1_0_P2_2_P3_0_P4
SECTION: session_handle_profile
FIELD: nabu_architecture_review_revision_6
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P3] [fail-closed] crates/sley-protocol/src/server.rs:2942-2945 - Revision 6 §3 item 5 (SESSION_HANDLE_PROFILE_V1.md:166-169) states without condition that `workspace.open` "[is] answered from the very head load that check 5 compared, so a head advanced between the check and the answer is never served to a session bound to the older root". The repair keeps the checked head only when it is format-1. The `_ => self.head()?` arm makes a second, unchecked head load whenever check 5 compared a native-format head. Sessions do bind native heads: `session_open` and `session_check_mixed_retained` both go through `head_binding_mixed` (server.rs:2444, 2415). A format-1 child of a native parent is a supported shape (sley-txn repository.rs:2427-2429, 2544-2546, 2559-2581), and native exchange import accepts format-1 receipts (sley-repo native_exchange.rs:1624-1634). So if another process moves the head from a native root to a format-1 child between check 5 and the reload, the child's summary goes to a session bound to the native root without `SESSION_ROOT_ADVANCED`. The new test (server_tests.rs:7593-7605) covers only a format-1 head, and the checker anchors only marker text (probe 2 PASSes). This is my revision-5 P4, narrowed to native checked heads. It is raised to P3 because the guarantee is now normative text and gate record §13.3:909 marks it FIXED - Closure needs: answer the Native arm without a second load, either by returning the native-head refusal directly or by re-comparing the reloaded root with the checked binding and answering `SESSION_ROOT_ADVANCED`; a server test over a native accepted head showing no summary is built from a reload; or else scope §3 item 5 explicitly to format-1 heads. | [P3] [duplicated-authority] crates/sley-protocol/src/server.rs:2198-2201 - The doc comment on `head_bound_versioned`, the helper that implements the versioned partition, still says the entity reads are "head-bound only in protocol version 2". It cites `ENTITY_READ_PROFILE_V2.md` §6 as its contract, and that section (ENTITY_READ_PROFILE_V2.md:274-278) still tells the session profile and checker that "both new tags are head-bound only in protocol version 2". Both contradict profile revision 6 §3:235-248. Gate record §13.2 (GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md:883) says "the Status line, section 3, ADR-0033, and the server doc comment agree". In fact this comment is byte-identical to 2b0f1c9f:server.rs:2186-2189. ENTITY_READ was amended this round (Status line and §2) without touching §6. The runtime is correct (gate at :2216-2220, and the v3 test passes), so this is an authority and record defect, not a safety defect - Closure needs: re-word the helper comment to "version 2 and version 3" and cite SESSION_HANDLE_PROFILE_V1 §3 as the classification authority; amend ENTITY_READ §6, or mark it superseded by session profile revision 6, within that document's own review scope; correct the §13.2 sentence; optionally anchor the comment in the checker.
SUMMARY: Revision 6 closes my revision-5 P2, both P3s and the editorial P4. The version-3 partition is stated in the owning contract, matches the server's version-aware gate, and is enforced by the checker (V3_ALL, gate shape, revert tests) and by a v3 stale-root server test. The checker exits 0 with PASS, 12 unit tests are OK, and the bounded sley-protocol runs gave 15 and 12 passed. The workspace.open single-head repair is sound for format-1 heads, but its native-format fallback still reloads the head unchecked, which contradicts the new unconditional §3 item 5 text (P3, carrying the narrowed prior P4). The partition helper's doc comment and ENTITY_READ §6 still say "version 2 only", against the gate record's claim that they agree (P3). PASS with two P3s.

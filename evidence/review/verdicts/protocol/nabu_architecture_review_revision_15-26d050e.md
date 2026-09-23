<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: nabu; field: nabu_architecture_review_revision_15; dispatched: 2026-09-23T13:51:26Z; duration_s: 381; process_exit_code: 0 -->
# Nabu Council review — protocol

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

This is what I checked. I did not edit the tree.

**Scope and history**
- `git rev-parse HEAD` returned 26d050e629b669ef4acbd54826ef4008c05a061d, which matches the scope SHA.
- `git status --short` shows six untracked verdict files from other lanes. I did not read them.
- `git log --oneline 2b0f1c9f..HEAD` lists 12 commits. The protocol work is in:
  - 60be11c8: contracts, checkers, summary
  - 004cf5be: code
  - 2de849d7: fuzz proofs
- `git diff --stat 2b0f1c9f..HEAD` over `docs/spec`, `docs/adr`, `crates/sley-protocol`, `scripts/` and `machine-summary.json` shows 33 files, +1425/−365. I read the full diffs of:
  - `docs/spec/SMP1.md`, `NATIVE_TEST_ADMISSION_V1.md` and `ENTITY_READ_PROFILE_V2.md`
  - `scripts/check_smp1_contract.py`, `scripts/test_smp1_contract.py` and `scripts/generate_smp1_json_bridge_table.py`
  - `docs/adr/ADR-0032-smp1-transport-boundary.md`
  - `crates/sley-protocol/src/server.rs`
- `git diff --name-only 60be11c8 HEAD -- crates/ fuzz/ Cargo.lock` is empty.
- `git diff --stat 531805ae HEAD -- evidence/review/verdicts/protocol/` is empty, so the prior transcripts are unchanged.
- `git log -G "&& \*tag != TESTS_REPORT_READ_TAG" -- lib.rs` has one hit, 116d031e (2026-09-16). This confirms SMP1's claim that the code has filtered the native tags since 2026-09-16.

**Checkers (each run through `subprocess`, all exit 0)**

| Checker | Result |
|---|---|
| `check_smp1_contract.py` | PASS, revision 15, method_count 41 |
| `check_smp1_json_bridge_contract.py` | PASS, revision 12 |
| `check_cli_contract.py` | PASS, revision 10 |
| `check_session_handle_profile.py` | PASS, revision 6, smp1_revision 15 |
| `check_complete_root_index_snapshot_profile.py` | PASS, revision 6 |
| `check_native_test_vectors.py` | "6 requests, 6 responses, 7 rejections OK" |
| `check_smp1_persistent_fuzz_slice.py` | PASS |
| `check_smp1_json_bridge_persistent_fuzz_slice.py` | PASS |
| `generate_smp1_json_bridge_table.py --check --protocol-version 1`, `2`, `3` | PASS each |

**Oracle vectors under `uv run --offline --frozen --project oracle/scb1` (all exit 0)**
- `check_smp1_vector.py`: PASS, frames 3, mutations 4.
- `check_smp1_json_bridge_vector.py`: PASS, methods 41, rejections 37, vectors 5.
- `check_complete_root_index_snapshot_vector.py`: PASS, vectors 1, mutations 6.

**Unit tests (`python3 -m unittest`, all exit 0)**
- `test_smp1_contract`: 16 OK
- `test_session_handle_profile`: 12 OK
- `test_complete_root_index_snapshot_profile`: 21 OK
- `test_cli_contract`: 10 OK
- `test_smp1_json_bridge_contract`: 4 OK

**Bounded cargo test**
- `cargo test --locked --offline -p sley-protocol --lib -- negotiat workspace_open native`: 20 passed, 0 failed, 1 ignored (the fixture emitter).
- The run includes:
  - `version_three_negotiation_gates_native_tags_on_feature_bit`
  - `versioned_negotiation_filters_only_known_v2_tags_on_v1`
  - `workspace_open_answers_from_the_single_checked_head_load`
  - `workspace_open_probe_is_non_waiting_and_never_rewrites_a_discarded_record`
  - `workspace_open_v3_answers_the_version_2_open_summary`
  - `native_tags_below_version_3_refuse_at_decode_before_the_session_check`

**Files read (line ranges)**
- `docs/spec/SMP1.md`: 1-100, 222-281, 740-784
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md`: 1-20, 360-374, 550-589
- `docs/spec/ENTITY_READ_PROFILE_V2.md`: 1-8, 36-85
- `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`: 205-295
- `docs/spec/SMP1_JSON_BRIDGE_V1.md`: 1-60
- `docs/spec/SESSION_HANDLE_PROFILE_V1.md`: 425-445
- `docs/spec/SLEY_CLI_V1.md`: 412-420
- `docs/adr/ADR-0032-smp1-transport-boundary.md`: 1-40
- `crates/sley-protocol/src/lib.rs`: 28-80, 722-825, 860-888, 1035-1099, 3477-3576, 3645-3681
- `crates/sley-protocol/src/server.rs`: 2179-2193, 2375-2404, 2479-2486, plus the full round diff
- `scripts/check_smp1_contract.py`: 50-85
- `scripts/generate_smp1_json_bridge_table.py`: 26-33
- `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md`: 784-964
- `evidence/review/native-n0-review-2026-09-15.md`: verdict lines
- `machine-summary.json`: `protocol` status, `contract_revision`, `current_delta_review`, notes, and the `nabu_architecture_review_revision_14` field
- `git show 76cd3dc4:docs/spec/NATIVE_TEST_ADMISSION_V1.md`: 1-8
- `evidence/review/verdicts/protocol/nabu_architecture_review_revision_14-2b0f1c9.md`: all

## Evidence checked

**The per-selection filter in SMP1 §2 now matches the code exactly.**
- SMP1 :240-251 says the explicit path removes:
  - under version 1: 306, 307, 605, 606 and 607
  - under version 2: 605, 606 and 607
  - under version 3 without the native-tests bit: 601, 602, 605, 606 and 607
- `negotiate_versioned` (`lib.rs:1075-1097`) applies exactly these three `retain` sets. The bit test uses the negotiated `profile.features`.
- The claim that other opaque tags "stay in the intersection" holds, for two reasons:
  - `Method::from_tag` searches `Method::ALL`, the 41-row version 1 table (`lib.rs:471`, `:775-781`), so 605-607 are opaque to `Hello::validate`.
  - Tests `lib.rs:3541-3563` keep 999/100 while dropping the listed tags under versions 1, 2 and 3 without the bit.
- The 601/602 part of the version 3 no-bit filter cannot be reached with validated hellos, because `Hello::validate` (`lib.rs:876-882`) refuses 601/602 without the bit. The filter is still stated exactly as the code applies it.
- The citation is NATIVE appendix C (:367-370: "605–607 unknown there", "selected intersection lacking bit removes native methods before profile hash"). My closure evidence had asked for a citation to appendix D. Appendix C is where the rule actually lives, so I accept it.
- Anchors `version-1-native-filter-compat` and `per-selection-filter` are at `check_smp1_contract.py:73-78`, and `version-1-compat` has been rewritten. Three revert cases (`test_smp1_contract.py:301-306`) are refused.

**Version 1 compatibility and history.**
- SMP1 :76-86 records two version 1 observable changes against revision 12:
  - the refusal of a non-empty 201 body
  - the explicit-path drop of the native tags, including its effect on the selected methods, the handshake identity and the `session.capabilities` bytes
- The legacy entrypoints keep those tags; `negotiate` has no filter. The history entry for revision 15 (:69-75) and the history pointer (:92-96) are accurate.

**NATIVE revision 7.**
- The status (:3-18) scopes the architecture review to the contract as first committed (76cd3dc4). At that commit the status reads revision 1, "Independent architecture review passed", and the commit includes `evidence/review/native-n0-review-2026-09-15.md`.
- Revisions 6 and 7 are assigned to the SMP1 revision 14 and 15 rounds.
- The appendix D pin (:555) reads "`docs/spec/SMP1.md` at revision 15". It is bound to the SMP1 Status line by `workspace_open_anchor_problems`, and the checker passes.

**Row 201 code change (004cf5be).**
- `workspace.open` is now answered from the head that its session check loaded (`session_check_mixed_retained`, `server.rs:2408-2420`).
- A native-format head falls back to `self.head()` and fails as before. The version 1 path and the `>= V2` gate are unchanged.
- Field 9 still comes only from the non-waiting probe.
- SMP1 :763-766 still holds: an absent boundary fails at the blocking head load, and the opener waits on an exclusive owner. The probe doc no longer lists a "missing" boundary as absence, which is consistent with this.
- `workspace_open_answers_from_the_single_checked_head_load` passes.

**Ownership is unchanged.**
- S20-390 owns fields 1-8.
- The S20-300 revision 6 probe supplies field 9 as a pointer. It has one production caller, and its gate text is at :216-229.
- NATIVE owns version 3.
- The consumers re-pin SMP1 15 through their own revisions: bridge 12 (:41-47, :56), CLI 10 (:24, :36), session 6 (:34), S20-300 6 (:24, :206), trial runner 7 (:43).
- The machine summary binds `current_delta_review` to revision 15 with every lane PENDING, and keeps my revision 14 PASS as a dated `_revision_14` field.

**Evidence freshness.** No crate, fuzz or lockfile path has changed since the fuzz-proof source 60be11c8. Both SMP1-family slice checkers pass, and so do the vectors and the version 1/2/3 bridge tables.

**Revision-14 findings from my lane:**
- [P3] spec-code-divergence/stale-exactness (SMP1 §2 "filters exactly 306 and 307") — CLOSED. SMP1 :240-251 matches `lib.rs:1075-1097` exactly and cites NATIVE appendix C. The version 1 compatibility statement counts the drop (:81-86). There are three anchors with revert tests, and the Rust tests at `lib.rs:3538-3563` pin the version 1/2/3 behaviour. A sibling statement in ENTITY_READ §2 still carries the old rule; it is filed below as a new finding.
- [P3] cross-contract-contradiction/evidence-overclaim (S20-300 acceptance through `workspace.open`) — CLOSED. S20-300 revision 6 :284-287 now requires absence "from the consumer wrapper `materialized_head_snapshot` … `workspace.open` itself waits at the S20-390 head load". This agrees with SMP1 :763-766 and the test.
- [P4] stale-comment (`generate_smp1_json_bridge_table.py`) — CLOSED. Lines 27-32 now say "SMP1.md (whatever revision its Status line names) owns no version 3 row".
- [P4] evidence-binding/status-scope — CLOSED:
  - ENTITY_READ :3-5 flags the 2026-09-23 amendment as covered only by the SMP1 revision 14 and 15 rounds.
  - NATIVE :3-7 scopes its review to 76cd3dc4.
  - Session §9 has revision 5 and 6 entries (:431-445).
  - CLI §8 has revision 9 and 10 entries (:412, :428).

## Findings

[P3] [cross-contract-contradiction/duplicated-authority] docs/spec/ENTITY_READ_PROFILE_V2.md:59-63,78-80; crates/sley-protocol/src/lib.rs:1042-1044 - This round edited ENTITY_READ to say its amendment tracks "SMP1 revisions 13 to 15", but §2 still contradicts SMP1 revision 15 on the same rule. It still says the version-aware entrypoint "filters 306 and 307 from the intersection when the selected version is 1" and "does not filter other opaque unknown numeric tags". It also still calls the non-empty 201 refusal "the recorded exception" for version 1. SMP1 :240-251 and :81-86 and the code say otherwise: under version 1, `negotiate_versioned` also drops the opaque tags 605-607, and SMP1 records that as a second version 1 change. Two contracts therefore state different filters for one entrypoint, and `negotiate_versioned`'s doc names ENTITY_READ §2 (not SMP1 §2) as its contract. A peer that follows ENTITY_READ §2 would derive a different selection and identity. - Closure evidence: ENTITY_READ §2 either defers the explicit-path filter to SMP1 §2 or states it identically, and names both recorded version 1 exceptions (or points at SMP1's compatibility statement). The `negotiate_versioned` doc cites SMP1 §2. An anchor with a revert case binds the ENTITY_READ sentence.

[P4] [stale-comment] crates/sley-protocol/src/lib.rs:68-73,1046-1055,3645 - These doc comments describe the filter that SMP1 revision 15 now specifies, and they are stale:
- `FEATURE_NATIVE_TESTS_V1` says 605-607 "refuse as reserved until N7d".
- `negotiate_versioned` says "Reserved tags remain invalid offers, except … 601/602/605, which a hello offering version 3 may list exactly when it sets" the bit, and that "606-607 still refuse as reserved".
- The test name `versioned_negotiation_filters_only_known_v2_tags_on_v1` says only version 2 tags are filtered.

The code contradicts all three: `is_reserved`'s own doc (:727-731), 606/607 dispatching live at `server.rs:1407-1408`, NATIVE appendix D :567 ("All five native rows are live"), and test :3532-3535, where a hello without the bit lists 605-607 and validates. This predates this round but sits on the lane's code path. - Closure evidence: the comments state the revision 15 filter and the live status of all five native rows (605-607 are opaque below version 3), and the test name reflects that version 3 tags are filtered too.

[P4] [identity-discipline/pin-record] docs/adr/ADR-0032-smp1-transport-boundary.md:5-24 - SMP1's ADR now has four statements labelled "Current pin":
- 2026-09-08: revision 12
- 2026-09-23: revision 13
- 2026-09-23: revision 14
- 2026-09-23, added this round: revision 15

This round fixed exactly that defect in ADR-0033 (0b21dcb0, "one current pin statement") but did not apply the fix to ADR-0032. The checker only substring-matches "revision 15". - Closure evidence: one current pin statement (revision 15), with the earlier ones marked as earlier pins. Optionally, an anchor like `adr-current-revision` in the CLI/bridge checkers.

[P4] [pin-record-wording] docs/spec/NATIVE_TEST_ADMISSION_V1.md:555-558 - The appendix D parenthetical "revision 6 of this contract re-pinned it from revision 12 and revision 7 to revision 15" leaves out revision 6's target (14). It can be read as revision 6 re-pinning from revisions 12 and 7. The status at :7-14 states the history correctly. - Closure evidence: the parenthetical reads "revision 6 re-pinned it from revision 12 to 14 and revision 7 to revision 15", and the single `` `docs/spec/SMP1.md` at revision 15 `` anchor is kept.

## Assessment

SMP1 revision 15 and NATIVE_TEST_ADMISSION revision 7 close all four findings my lane raised on revision 14.
- The explicit negotiation filter is now stated once in SMP1 §2, identically to `negotiate_versioned`, and the version 1 compatibility statement counts it.
- The S20-300 acceptance item now names the probe wrapper and says the opener waits.
- The generator comment no longer names a revision.
- The ENTITY_READ and NATIVE statuses scope their past reviews and assign the 2026-09-23 amendments and re-pins to the SMP1 rounds.

Evidence is fresh at 26d050e6: every SMP1-family checker and its unit tests, the uv oracle vectors, the version 1/2/3 bridge tables, both fuzz slices, and 20 bounded `sley-protocol` tests all pass.

Ownership and the fail-closed structure are intact:
- S20-390 owns fields 1-8, and they are now served from the single session-checked head.
- The non-waiting S20-300 probe supplies field 9 as a pointer.
- NATIVE owns version 3, pinned to SMP1 15 under a checker bound to the Status line.
- Consumers re-pin through their own revisions.

The remaining P3 is duplicated authority. ENTITY_READ §2, which the code names as its contract, still states the old "only 306 and 307" filter and a single version 1 exception, even though this round re-dated its amendment to revision 15. Wire behaviour is not affected because the code follows SMP1. The three P4s are stale code comments and two pin-record wording defects.

My lane accepts this package at contract revision 15. I make no claim about the CLI, session, bridge, S20-620 or trial-runner lanes beyond the pins read above, nor about GA or release readiness.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_3_P4_PRIOR_P3_P4_CLOSED
SECTION: protocol
FIELD: nabu_architecture_review_revision_15
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P3] [cross-contract-contradiction/duplicated-authority] docs/spec/ENTITY_READ_PROFILE_V2.md:59-63,78-80; crates/sley-protocol/src/lib.rs:1042-1044 - ENTITY_READ §2, re-dated this round to "SMP1 revisions 13 to 15", still says the version-aware entrypoint filters only 306 and 307 at version 1, "does not filter other opaque unknown numeric tags", and has one recorded version 1 exception, contradicting SMP1 rev 15 §2 and :81-86 and the code (605-607 are dropped under v1); negotiate_versioned's doc cites ENTITY_READ §2 as its contract - defer to or restate the SMP1 §2 filter, name both v1 exceptions, cite SMP1 §2 in the doc, and anchor it with a revert case | [P4] [stale-comment] crates/sley-protocol/src/lib.rs:68-73,1046-1055,3645 - the FEATURE_NATIVE_TESTS_V1 and negotiate_versioned docs still say 605-607 or 606-607 "refuse as reserved" and that only a bit-setting v3 hello may list 605, and the test name says only v2 tags are filtered; this contradicts is_reserved (:727-731), live dispatch (server.rs:1407-1408), NATIVE appendix D :567 and test :3532-3535 - update the comments and test name to the rev-15 filter and live rows | [P4] [identity-discipline/pin-record] docs/adr/ADR-0032-smp1-transport-boundary.md:5-24 - four "Current pin" statements (revisions 12, 13, 14, 15), the defect this round fixed in ADR-0033 - keep one current pin and mark the others earlier | [P4] [pin-record-wording] docs/spec/NATIVE_TEST_ADMISSION_V1.md:555-558 - the appendix D parenthetical leaves out revision 6's target (14) and can be misread - state "12 to 14" and "revision 7 to 15"
SUMMARY: SMP1 revision 15 and NATIVE_TEST_ADMISSION revision 7 close all four Nabu revision-14 findings: the §2 per-selection filter matches negotiate_versioned exactly and is anchored with revert tests, the S20-300 acceptance item names the probe wrapper, the generator comment is revision-agnostic, and the ENTITY_READ/NATIVE statuses and CLI/session histories are scoped. Every SMP1-family checker, oracle vector, bridge table, fuzz slice and 20 bounded sley-protocol tests pass at 26d050e6, and ownership (S20-390 fields 1-8 from the single checked head, the S20-300 pointer probe, NATIVE owning version 3) is intact. One P3 remains: ENTITY_READ §2, re-dated to revision 15 this round, still states the old 306/307-only filter and a single version 1 exception, a second divergent authority for negotiate_versioned. Three P4 notes cover stale lib.rs native-tag comments, the four ADR-0032 "Current pin" statements, and the NATIVE appendix D re-pin wording.

<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 26d050e629b669ef4acbd54826ef4008c05a061d; role: ariadne; field: ariadne_contract_review_revision_15; dispatched: 2026-09-23T13:46:19Z; duration_s: 307; process_exit_code: 0 -->
# Ariadne Council review — protocol

Harness: claude-code
Reviewed checkpoint: 26d050e629b669ef4acbd54826ef4008c05a061d

**Scope.** `git rev-parse HEAD` returned `26d050e629b669ef4acbd54826ef4008c05a061d`, matching SCOPE_SHA, and HEAD had not changed when I finished. The tree was not modified. The five untracked verdict files already there at the start (the cli and session_handle_profile transcripts from concurrent reviews) are unchanged, and I added nothing.

**Git commands run:**
- `git log --oneline 2b0f1c9f..HEAD`: 12 commits. The protocol package delta is in `60be11c8` (contracts), `004cf5be` (the `workspace.open` code path) and `2de849d7` (fuzz proofs at `60be11c8`).
- `git diff --stat 2b0f1c9f..HEAD`.
- Full `git diff 2b0f1c9f..HEAD` over:
  - `docs/spec/SMP1.md` and `docs/spec/NATIVE_TEST_ADMISSION_V1.md`;
  - `scripts/check_smp1_contract.py` and `scripts/test_smp1_contract.py`;
  - `crates/sley-protocol/src/server.rs`;
  - `docs/spec/ENTITY_READ_PROFILE_V2.md`;
  - ADR-0032, ADR-0034 and ADR-0035.
- `git diff --stat 60be11c8..HEAD` over `crates/sley-protocol`, `crates/sley-json-bridge`, `fuzz` and SMP1: empty.
- `git merge-base --is-ancestor 60be11c8 2de849d7`: ancestor.
- `git show 116d031e^:crates/sley-protocol/src/lib.rs`: before the native-tag change, the v1 filter removed only 306 and 307 (lines 936-940). `116d031e` is dated 2026-09-16.
- `git log -S 'filters 306 and 307 from the intersection' -- docs/spec/ENTITY_READ_PROFILE_V2.md`: the text came from `e9792689` (2026-09-08) and has not changed since.
- `sha256sum` of my revision-14 transcript is `e2af6694…c9231`. It matches `evidence/review/rounds/context-r7-2b0f1c9.json:121`.

**Checkers run (exit code and result line):**
- `check_smp1_contract.py`: 0, PASS, revision 15, `S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING`.
- `check_smp1_json_bridge_contract.py`: 0, PASS, revision 12.
- `check_session_handle_profile.py`: 0, PASS, revision 6, `smp1_revision` 15.
- `check_cli_contract.py`: 0, PASS, revision 10.
- `check_cli_rules.py`: 0, PASS, `worker_calls` 1.
- `check_required_contract_index.py`: 0, PASS (12 contracts).
- `check_complete_root_index_snapshot_profile.py`: 0, PASS, revision 6.
- `check_sley2_trial_runner.py`: 0, PASS, revision 7.
- `check_smp1_persistent_fuzz_slice.py`: 0, PASS.
- `check_smp1_json_bridge_persistent_fuzz_slice.py`: 0, PASS.
- Generator checks:
  - `generate_smp1_fixtures.py --check`: 0, PASS (3 frames, 4 rejections, no drift).
  - `generate_smp1_json_bridge_table.py --check`: 0, PASS.
  - `generate_smp1_json_bridge_fixtures.py --check`: 0, PASS (5 vectors, 37 rejections).
  - `generate_native_test_fixtures.py --check`: 0, no output.
- Vector checkers, run with `oracle/scb1/.venv/bin/python`:
  - `check_smp1_vector.py`: 0, PASS.
  - `check_smp1_json_bridge_vector.py`: 0, PASS.
  - `check_native_test_vectors.py`: 0, "6 requests, 6 responses, 7 rejections OK".
  - `check_entity_read_vectors.py`: 0, PASS.
  - `check_complete_root_index_snapshot_vector.py`: 0, PASS.

**Unit tests run:**
- `python3 -m unittest`:
  - `test_smp1_contract.py`: 0, 16 OK.
  - `test_smp1_json_bridge_contract.py`: 0, 4 OK.
  - `test_session_handle_profile.py`: 0, 12 OK.
  - `test_cli_contract.py`: 0, 10 OK.
  - `test_cli_rules.py`: 0, 16 OK.
  - `test_current_contract_review.py`: 0, 6 OK.
  - `test_required_contract_index.py`: 0, 6 OK.
  - `test_complete_root_index_snapshot_profile.py`: 0, 21 OK.
- `scripts/test_smp1_json_bridge_table.py`: 0, 17 cases PASS.
- Cargo, `-p sley-protocol --lib`:
  - `version_three_negotiation_gates_native_tags_on_feature_bit`: 0, 1 passed.
  - `workspace_open`: 0, 5 passed (including `workspace_open_answers_from_the_single_checked_head_load` and `workspace_open_v3_answers_the_version_2_open_summary`).
  - `negotiat`: 0, 8 passed.

**Oracle probe.** I ran the independent ENTITY_READ oracle's `negotiate_versioned` in memory, calling it through the venv Python (binary-free, no writes). Both hellos listed `[100, 306, 307, 605, 606, 607, 999]`:
- client `[1,2]`, server `[1]`: selected version 1, methods `[100, 605, 606, 607, 999]`.
- client `[1,2]`, server `[1,2]`: selected version 2, methods `[100, 306, 307, 605, 606, 607, 999]`.

**Files read (line ranges):**
- `docs/spec/SMP1.md` 1-100, 150-185, 225-285.
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md` 1-20, 363-575 (appendices C and D).
- `crates/sley-protocol/src/lib.rs` 840-910, 990-1124, 3380-3590, 3645-3681.
- `crates/sley-protocol/src/server.rs` (the diff; `negotiate_identity*` call sites at 504 and 543).
- `docs/spec/ENTITY_READ_PROFILE_V2.md` 1-20, 36-80.
- `oracle/scb1/src/sley2_scb1_oracle/entity_read.py` 89-94, 971-988, 1102-1147.
- `scripts/check_smp1_vector.py` 150-185.
- `scripts/check_smp1_contract.py` 50-100.
- `scripts/test_smp1_contract.py` 275-330.
- `scripts/check_cli_contract.py` 200-220; `scripts/check_smp1_json_bridge_contract.py` 130-145; `scripts/check_session_handle_profile.py` 488.
- `docs/spec/SESSION_HANDLE_PROFILE_V1.md` 425-445; `docs/spec/SLEY_CLI_V1.md` 405-445.
- The ADR-0032, ADR-0034 and ADR-0035 diffs.
- The machine-summary `protocol` section (every key that changed since 2b0f1c9f).
- `bench/live/GATE-RECORD-20260923-CONTEXT-IMPLEMENTATION.md` 784-963.
- `evidence/review/verdicts/protocol/ariadne_contract_review_revision_14-2b0f1c9.md` (the whole file).

## Evidence checked

**Revision 14 [P3] spec-precision, per-selection negotiation filter: CLOSED.**

SMP1 §2 (SMP1.md:240-251) now states the explicit-path filter per selection:
- v1 removes 306, 307, 605, 606 and 607;
- v2 removes 605, 606 and 607;
- v3 without the native-tests bit removes 601, 602, 605, 606 and 607;
- every other opaque unknown tag stays in the intersection.

This matches `negotiate_versioned` exactly (lib.rs:1075-1097).

The claim that the filter runs "before the profile hash" is accurate. `negotiate_identity_versioned` hashes the filtered profile (lib.rs:1112-1113), and the version-aware server uses it (server.rs:543).

"The legacy entrypoints keep them" is also accurate. Legacy `negotiate` does not filter (lib.rs:998-1040), and the legacy server path uses it (server.rs:504).

The citation of NATIVE appendix C is correct. Appendix C says "605–607 unknown there" and "selected intersection lacking bit removes native methods before profile hash", so the ambiguity my finding noted at NATIVE :360 is resolved.

The version 1 compatibility statement (SMP1.md:77-89) now counts two observable changes: the non-empty 201 refusal (revision 13) and the explicit-path native-tag drop. It names the effect on the handshake identity and on the `session.capabilities` bytes, and dates the code change to 2026-09-16. I verified that date against `116d031e`.

Three new anchors pin the text (`per-selection-filter`, `version-1-native-filter-compat`, and a rewritten `version-1-compat`), in check_smp1_contract.py:63-79. Three revert cases exist (test_smp1_contract.py:301-306), and every `REVERTS` original is asserted present before it is mutated.

The passing Rust test at lib.rs:3543-3563 asserts both the v2 drop and the v1 drop of 306, 307 and 605-607, with 999 kept.

The 601/602 removal under v3 without the bit is unreachable in practice, though it is still exact. `Hello::validate` (lib.rs:876-883) refuses 601/602 in any hello that lacks both v3 and the bit, so the intersection can never contain them when the bit is absent. This is not a defect.

**Revision 14 [P4] record, consumer ADR and revision-history records: CLOSED.**
- ADR-0034:3 now reads "draft at revision 12". Revision 11 and 12 records are at :17-24, and the date line is updated.
- ADR-0035:3 now reads "draft at revision 10". Revision 9 and 10 records and decision 8 are present.
- The session profile's §9 has "- Revision 5 (2026-09-23)" at :431 and revision 6 at :437.
- CLI §8 has "### Revision 9 (2026-09-23)" at :412 and revision 10 at :428.
- Checker coverage was added:
  - `check_cli_contract.py:202-217` checks `adr-current-revision` and the `### Revision N (` record;
  - `check_smp1_json_bridge_contract.py:143` checks ADR-0034's current line;
  - `check_session_handle_profile.py:488` checks the `- Revision N (` entry.

  All three checkers and their suites pass.

**Other revision 15 checks:**
- **NATIVE revision 7.**
  - The status line (:3-18) scopes the architecture PASS to `76cd3dc4` and names both SMP1 re-pins.
  - Appendix D pins "`docs/spec/SMP1.md` at revision 15" (:555). The checker requires exactly one such pin, equal to 15 (check_smp1_contract.py:88-90), with a stale-pin refusal test.
  - The rows are unchanged. The wording "re-pinned it from revision 12 and revision 7 to revision 15" leaves out the intermediate 14, but the status line states it.
- **Row 201.**
  - The `open_summary` scope anchor is updated to "revisions 13 to 15", and the S20-300 probe pin is updated to revision 6 (SMP1.md:751).
  - server.rs now answers 201 from the head that its session check retained (server.rs:1179-1190, 2408-2420, 2939-2945). The gate is unchanged: `>= PROTOCOL_VERSION_V2` at server.rs:2946.
  - The non-empty-body refusal still runs first.
  - The docstring no longer lists an absent boundary as probe absence. This agrees with SMP1's statement that the blocking shared head load fails the method.
- **Records.** Machine-summary `protocol`:
  - `contract_revision` is 15;
  - `current_delta_review` is {15, PENDING×3};
  - the `_revision_14` lane fields and notes are retained, and the `status_note` matches the text.

  The composition pin is "bridge revision 12 / CLI revision 10" (SMP1.md:21-23), and `check_smp1_contract.py` enforces it. ADR-0032 is updated with the revision 15 record. SMP1's history paragraph (:69-75, :91-96) records the round.
- **Fuzz lanes.** Both SMP1-family slice checkers pass. The proofs are at `60be11c8`, and no lane path has changed since.

## Findings

[P3] [spec-consistency] docs/spec/ENTITY_READ_PROFILE_V2.md:43-48,58-63,78-80; oracle/scb1/src/sley2_scb1_oracle/entity_read.py:1133-1134 vs docs/spec/SMP1.md:240-251,77-89 and crates/sley-protocol/src/lib.rs:1042-1044,1075-1088 - ENTITY_READ_PROFILE_V2 §2 is the contract that `negotiate_versioned`'s docstring cites (lib.rs:1043). It still says the version-aware entrypoint "filters 306 and 307 from the intersection when the selected version is 1. It admits them for version 2 … It does not filter other opaque unknown numeric tags" (:60-62). Its amendment names "one later exception owned by SMP1" (:43), and its v1 statement names the 201 refusal as "the recorded exception" (:80). This round edited that amendment, re-pinning it to "SMP1 revisions 13 to 15", and the status line (:3-5) places it under the SMP1 revision 14 and 15 rounds. SMP1 revision 15 §2 and the code instead drop 605-607 under v1 and v2 selections, and SMP1 now counts two v1 observable changes. SMP1 also requires a re-deriving peer to apply the same filter. The repo's independent ENTITY_READ oracle follows the stale text: in-memory probe, with both hellos listing `[100,306,307,605,606,607,999]`, gives v1 `[100,605,606,607,999]` and v2 `[100,306,307,605,606,607,999]`, where Rust yields `[100,999]` and `[100,306,307,999]` (lib.rs:3550-3563). The effect is limited to hellos that list the non-dispatched 605-607, which SMP1:174-175 says a hello does not do. No current vector lists them (I checked every conformance JSON hello). - Closure evidence: ENTITY_READ §2 either states SMP1 revision 15's per-selection filter or defers the filter to it, and names the native-tag drop as a second v1 exception. The oracle's `negotiate_versioned` drops 605-607 under v1 and v2 selections (or refuses them). An oracle test or vector with a hello listing 605-607 matches the Rust selection bytes and handshake identity.

## Assessment

SMP1 revision 15 and NATIVE_TEST_ADMISSION revision 7 close both findings of my revision-14 review. I verified each closure against code, tests, checkers and records, not against the gate record's §13.3 claims:
- SMP1 §2 now states the explicit negotiation's per-selection filter exactly as `negotiate_versioned` applies it, and states that it runs before the profile hash.
- The v1 compatibility sentence counts the second observable change.
- Anchors with revert cases pin both statements.
- NATIVE appendix D is re-pinned to revision 15 under checker enforcement.
- The consumer ADR and revision-history records are complete, and checkers now cover them.

Every SMP1-family checker, generator check, vector checker, fuzz-slice checker, Python suite and targeted Rust test passes at HEAD.

One residual remains. It is the same class of defect as my closed P3, now in the composing contract. ENTITY_READ_PROFILE_V2 §2 was touched in this round and is reviewed under it, and it still says the version-aware entrypoint filters only 306 and 307 and nothing else. The independent oracle implements that stale rule, so for hellos listing 605-607 it derives a selection that differs from the Rust one. The scope is narrow and no vector exercises it, so it is P3 and non-blocking.

A stale test name, `versioned_negotiation_filters_only_known_v2_tags_on_v1` (lib.rs:3645), has assertions that agree with the spec, so it is not a finding.

This verdict covers only the SMP1 revision 15 and NATIVE revision 7 package in the protocol contract lane. It makes no GA, release-readiness or package-completion claim.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_0_P4_PRIOR_P3_P4_CLOSED
SECTION: protocol
FIELD: ariadne_contract_review_revision_15
SCOPE_SHA: 26d050e629b669ef4acbd54826ef4008c05a061d
FINDINGS: [P3] [spec-consistency] docs/spec/ENTITY_READ_PROFILE_V2.md:43-48,58-63,78-80; oracle/scb1/src/sley2_scb1_oracle/entity_read.py:1133-1134 vs docs/spec/SMP1.md:240-251,77-89 and crates/sley-protocol/src/lib.rs:1042-1044,1075-1088 - ENTITY_READ §2 (the contract cited by negotiate_versioned's docstring, edited this round, and covered by the SMP1 r14/r15 rounds) still says the version-aware entrypoint filters only 306/307 on v1 and "does not filter other opaque unknown numeric tags", and names a single later v1 exception; SMP1 r15 and the code drop 605-607 under v1 and v2 and count two v1 changes; the independent oracle implements the stale rule (probe: v1 keeps 605-607, v2 keeps 605-607; Rust drops them per lib.rs:3550-3563), so its re-derived selection and handshake identity differ for hellos listing 605-607 - align ENTITY_READ §2 with (or defer it to) SMP1 r15's per-selection filter and its second v1 exception, make the oracle drop 605-607 under v1/v2, and add an oracle test or vector over such a hello that matches the Rust bytes
SUMMARY: SMP1 revision 15 closes both of my revision-14 findings. §2 now states the explicit negotiation's per-selection filter exactly as negotiate_versioned applies it before the profile hash, the v1 compatibility statement counts the native-tag drop, and new anchors with revert cases pin both. NATIVE_TEST_ADMISSION revision 7 re-pins appendix D to SMP1 15 under checker enforcement, and the ADR-0034/0035, session §9 and CLI §8 records are complete and checker-covered, with every SMP1-family checker, vector, fuzz slice, suite and targeted Rust test passing at HEAD. One non-blocking P3 remains: ENTITY_READ_PROFILE_V2 §2, amended in this round, and the independent ENTITY_READ oracle still apply the old 306/307-only rule, so for hellos listing 605-607 they disagree with SMP1 15 and the code. The protocol contract lane accepts SMP1 revision 15.

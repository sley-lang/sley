<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 883361e3446d77174c69a8de2821247096e17574; role: ariadne; field: ariadne_contract_review; dispatched: 2026-09-23T03:32:29Z; duration_s: 2403; process_exit_code: 0 -->
# Ariadne Council review — dead_function_deletion_selection

Harness: claude-code
Reviewed checkpoint: 883361e3446d77174c69a8de2821247096e17574

What I verified myself:
- **Scope.** `git rev-parse HEAD` returned `883361e3446d77174c69a8de2821247096e17574`, so the scope matches. `git status --short` was clean. `git diff --stat acbc65f0..HEAD` touches four files: `candidate_validation.rs` (+20/−1), `native_test_plan.rs` (+486), `CANDIDATE_RESULT_V1.md` (+9/−1) and the REQ-11 packet (+85). That is the packet's three items plus the packet itself.
- **Diffs read.** Full diff of `candidate_validation.rs` and `CANDIDATE_RESULT_V1.md`, and the added `function_deletion_tests` module (`native_test_plan.rs:1789-2274`).
- **Files read, with line ranges:**
  - Packet: `evidence/review/requests/REQ-11-dead-function-deletion-selection.md:1-86`, read in full.
  - `crates/sley-policy/src/candidate_validation.rs`: 770-1090 (phases 5-13), 1541-1548 (phase-9 function loop), 1735-1751 (`live_selection_functions`), 1957-1977 (`contract_plan_failure`).
  - `crates/sley-policy/src/native_test_plan.rs`: 255-346 (`native_test_plan`), 577-605 (`affected_functions`, `live_test_map`), 1789-2274 (new module).
  - `crates/sley-check/src/contracts.rs`: 486-508 (`lookup`, `function_index`), 814-846 (`select_tests`).
  - `crates/sley-policy/src/candidate_program.rs`: 100-170 (`project`, `kinds` holds every proposed entity), 289-295 (`affected_functions`).
  - `crates/sley-mutate/src/apply.rs`: 395-445 (`ensure_body_kind`), 1030-1060 (`kind_mismatched_replacement_is_refused`).
  - Specs: `docs/spec/CONTRACT_TEST_PROFILE_V1.md:127-213` (sections 3-5), `docs/spec/NATIVE_TEST_ADMISSION_V1.md:21-89` (section 2), `docs/spec/CANDIDATE_RESULT_V1.md:30-40, 280-322`.
  - Succession records: `git show work/succession-sley20-arm:bench/live/DEAD-TOMBSTONE-PROPOSAL.md` (full) and `bench/live/SUCCESSION-COVERAGE.md:53-92` on the same branch (DEAD diagnosis).
- **Tests:**
  - `cargo test -p sley-policy --lib function_deletion_tests`: 7 passed, 0 failed.
  - `cargo test -p sley-policy`: lib 87 passed, 1 ignored; `rw080_construction_policy` 1 passed; `s3_g3_adversary` 3 passed, 1 ignored.
  - `cargo test --workspace --locked --no-fail-fast`, re-run standalone: no `FAILED`, `panicked` or `exited abnormally` lines.
- **`make quick`.** Plain `make quick` stopped with `make: *** [Makefile:45: quick] Error 1` at `check_merge_persistent_fuzz_slice.py`, which printed `proof-record-predates-lane-change`. I then ran all 130 recipe commands one at a time from a python runner: 16 exited non-zero. They are classified under Evidence checked.
- **Master-goal checkers.** With `SLEY2_MASTER_GOAL` pointed at the file outside the repository:
  - `check_candidate_result_contract.py`: exit 0, `"result": "PASS"`, `problems: []`.
  - `check_transaction_contract.py`: exit 0, `"result": "PASS"`.
- **Generators in check mode:**
  - `build_test_inventory.py --check`: exit 1, "the tracked test inventory differs from the derived inventory".
  - `generate_supply_chain_evidence.py --check`: exit 1, drift in `evidence/security/T54/secret-scan.json`.
- **Attribution.** A python trace with `fuzz_proof_record.lane_paths` found:
  - The proof `source_commit` is `69907ddf…`.
  - `git diff --name-only 69907ddf acbc65f0 -- <lane paths>` is empty.
  - `git diff --name-only acbc65f0 HEAD -- <lane paths>` lists the two `sley-policy` files.
- **Vector search.** Grep for `TEST_PLAN_SELECTION_INVALID|24016|24_016` over `conformance/` and `bench/fixtures/`. Every hit is the substring `…3124274024016483a2…` inside one hex hash. No symbol matches.

## Evidence checked

**The defect, against the DEAD diagnosis.**
- `candidate_validation.rs:802-804` builds `affected_functions` as the base ∪ proposed kind-5 union.
- Before this change, line 1008 passed that union to `validate_contract_test_program`.
- `select_tests` (`contracts.rs:821-824`) resolves each affected id through `function_index`. An id missing from the proposed index gives `UnresolvedEntity`, which is mapped to `TestSelectionInvalid`.
- `contract_plan_failure` (`candidate_validation.rs:1957-1977`) renders that at phase 11 as `TEST_PLAN_SELECTION_INVALID` / 24016.
- This matches the succession diagnosis (`SUCCESSION-COVERAGE.md:62-69`). The packet's statement that no vector pins this refusal holds.

**The fix.**
- `live_selection_functions` (`candidate_validation.rs:1745-1751`) filters the union down to ids present in `program.kinds`. `CandidateProgram::project` (`candidate_program.rs:147-154`) inserts every proposed entity of every kind into `kinds`.
- The filter keeps the order, so the input stays sorted and unique as `CONTRACT_TEST_PROFILE_V1.md:172` and `contracts.rs:361` require. It can only shrink the list, so `MAX_AFFECTED_FUNCTIONS` is unaffected.
- Every id in `proposed_functions` is bound in the proposed state. So an id is dropped exactly when it is a base Function the candidate deleted.
- Only the phase-10 call (line 1008) uses the projected list. Phase 9 (line 965) and `renderer.affected_closure` (line 798) are unchanged. Phase 9 already skips unbound Functions (`candidate_validation.rs:1541-1548`, `continue`).

**Why the projection cannot drop a test, claim by claim.**
1. *A deleted Function has no live TestCase targeting it.* Holds. `a_live_test_targeting_the_deleted_helper_still_refuses` (`native_test_plan.rs:2233-2239`) leaves the helper's test as the only dangling reference and gets phase 5 `GRAPH_UNRESOLVED_REFERENCE`.
   - `select_tests` selects only by `test.target` or by the required set (`contracts.rs:838-841`).
   - So removing an id that cannot be any live test's target gives the same selection. The contract-profile §4 output is unchanged, and §4's precondition ("every affected function … must resolve in the same closed request", `CONTRACT_TEST_PROFILE_V1.md:175-176`) is now met by construction. The checker itself is not modified.
2. *Deleting a protected required test still refuses.* Holds. `deleting_the_helper_with_its_protected_required_test_still_refuses` (2261-2273) gets phase 11 `TEST_PLAN_SELECTION_INVALID`, and the native plan returns `SelectionInvalid` through the `!output.is_valid()` gate at `native_test_plan.rs:268-270`.
   - After the fix, the refusal comes from the required-test check (`contracts.rs:828-833`). This test also passed before the fix, for a different reason, as the packet says.
   - This matches NATIVE_TEST_ADMISSION §2 ("deleting a protected required test refuses", line 36).
3. *A still-bound identity is kept and refuses inside the checker.* The code keeps such ids, but the claim is imprecise. See finding 2.
4. *Survivor selection is unchanged.* Holds.
   - `dead_code_removal_keeps_survivor_selection_and_tombstones_the_helper` (2192-2220) asserts `selected_tests == [SURVIVOR_TEST]`. It also asserts that `affected_closure` still contains both HELPER and SURVIVOR, and that the native `selected()` is `[SURVIVOR_TEST]`.
   - The baseline control (2155-2174) shows the fixture selects the survivor's test with no Function deleted.
   - The accounting part of the proposal's "no blanket omission" requirement is met: the recorded closure keeps the deleted id.
5. *Orphaned structure still refuses.* Holds. `orphaning_the_helper_block_still_refuses` (2222-2231) gets phase 5 `GRAPH_UNRESOLVED_REFERENCE`.
6. *Optional-test deletion.* NATIVE §2 says "Deleted optional tests are recorded, not run" (line 35). `deleting_the_helper_with_its_optional_test_validates_and_records_the_test` (2241-2259) gets a valid result, an empty selection, and a single `changed` entry for HELPER_TEST with `after == None`. Holds.

**Mutation evidence** (reverting the hunk makes the three positive tests fail): I did not reproduce this because the review is read-only. It follows from the trace above: each positive test deletes HELPER, which would reach `function_index` → `UnresolvedEntity` before the fix.

**Spec text.** The new phase-10 rule (`CANDIDATE_RESULT_V1.md:311-319`) accurately describes the code at `candidate_validation.rs:805/1008`. It does not conflict with §4 of the contract profile or §2 of native admission. "Selection tombstone" is qualified, so it is distinct from the context tombstone ledger (`CANDIDATE_RESULT_V1.md:37`, phase 4 at 290-291).

**Design divergence (note).** The proposal planned a tombstone index inside `contracts.rs`. The implementation instead filters the checker's input in candidate validation and leaves the checker untouched. The observable result is the same: zero tests for a deleted Function, survivors selected normally, accounting kept.

**`make quick` failures (16 commands).**
- *Caused by this diff (the records predate a change to `crates/sley-policy`):*
  - `proof-record-predates-lane-change` in 8 persistent-fuzz checks: merge, semantic_delta, smp1, smp1_json_bridge, pack, adapter_responses, exchange, merge_judgment. All proofs have `source_commit` 69907ddf and were fresh at acbc65f0.
  - `check_reproducibility_and_independent_conformance.py`: `reproducibility-report:stale:69907ddf904a:2-surface-files-changed:crates/sley-policy/src/candidate_validation.rs,crates/sley-policy/src/native_test_plan.rs`.
  - `check_decision_dossier.py`: `test-inventory:drift`. The tracked `evidence/validation/test-inventory.json` (last written at acbc65f0) has sley-policy at `tests: 86`, and this diff adds 7 tests.
  - `check_supply_chain_audit.py`: T54 `secret-scan.json` drift.
- *Not attributed to the diff (environment or untracked artifacts):*
  - `check_candidate_result_contract.py` and `check_transaction_contract.py`: master goal unset. Both pass when it is set.
  - `build_candidate_content_report.py --check`: `evidence/runtime/s20-720-release-candidate/evidence.json` is untracked (`git ls-files` empty).
  - `check_standards_sbom_and_provenance.py` (`closure:unverifiable`, `sbom:drift`, `provenance:drift`): not attributed either way.
  - `cargo test --workspace --locked` exited abnormally inside the batch runner but passed when re-run standalone.
- *Passed:* every frozen-vector and fixture check, including `check_contract_test_profile.py`, `generate_native_test_fixtures.py --check`, `check_native_test_vectors.py` and every `generate_*_fixtures.py --check` in the recipe. So there is no vector, symbol or old-profile judgment drift.

**The packet's observation question.**
- `native_test_plan.rs:302-309` tests `affected.contains(entity)`, where `entity` is a TestCase id and `affected` is the Function list from `affected_functions` (577-597). A TestCase id is never kind 5, so this clause never fires.
- NATIVE §2 item 2 (line 31) is still met, because `static_selected` equals {proposed tests whose target is in the union}. After this change that equality depends on the phase-10 projection being exact for selection, which claim 1 shows it is.
- My answer: record it separately rather than folding it into this change. It is pre-existing, outside `acbc65f0..HEAD`, changes no plan output, and adding it would break the packet's three-item limit.
- The follow-up should change it to `affected.contains(&test.target)` and add a test showing the native selection equals the static selection plus created/replaced plus required.
- The deleted helper staying in the native `affected` list would be harmless after that correction, since no live test can target it.

## Findings

[P3] [gate-staleness] evidence/review/requests/REQ-11-dead-function-deletion-selection.md:80-81 - The acceptance criterion "make quick at the reviewed head" is not met at 883361e3. The diff's changes to crates/sley-policy make stale 8 persistent-fuzz proof records (source_commit 69907ddf, fresh at acbc65f0), the S20-730 reproducibility report (which names both changed files), the test inventory (sley-policy 86 vs +7 tests, feeding decision-dossier drift) and the T54 secret scan. The packet does not disclose this. Frozen-vector checks all pass, so this is records staleness, not semantic drift - closure: a records-only rebind commit (following the 69907ddf→8966da2e pattern) with make quick exit 0 at the landing tip (with SLEY2_MASTER_GOAL set and release-candidate evidence present), or a packet amendment that limits the criterion to the vector/fixture checkers and names the rebind as a precondition for landing.

[P4] [claim-accuracy] evidence/review/requests/REQ-11-dead-function-deletion-selection.md:49-50; docs/spec/CANDIDATE_RESULT_V1.md:318-319; crates/sley-policy/src/candidate_validation.rs:1742-1744 - The "kind change keeps refusing inside the checker (WrongEntityKind)" branch cannot be reached and no test covers it. A bound identity cannot change kind: apply refuses TargetKindMismatch (crates/sley-mutate/src/apply.rs:422-430, test at 1032), and phase 4 refuses creation-ID collisions with live identities. If it were reached, select_tests would map the function_index error to TestSelectionInvalid (crates/sley-check/src/contracts.rs:822-823), rendered as phase 11 TEST_PLAN_SELECTION_INVALID (candidate_validation.rs:1957-1977), not as WrongEntityKind - closure: reword the packet and spec to say the branch is defensive (kind change is refused at apply; if it were reached the refusal would be TEST_PLAN_SELECTION_INVALID), or add a test that reaches it.

[P4] [latent-dead-clause] crates/sley-policy/src/native_test_plan.rs:302-309 - Pre-existing and outside the diff: `affected.contains(entity)` compares TestCase ids with Function ids and never fires. NATIVE_TEST_ADMISSION_V1.md:31 item 2 is met only because static_selected coincides with it. Recommend recording this separately rather than folding it into this change - closure: a separate change using `affected.contains(&test.target)` plus a test asserting the native selection = static ∪ created/replaced ∪ required, with no change to plan output.

## Assessment

The production change is small and correct.
- Projecting the phase-5 affected-Function union onto identities still bound in the proposed state removes exactly the Functions the candidate deleted.
- Phase-5 reference integrity guarantees no live TestCase targets those Functions, so the checker's selection set is unchanged. The contract-profile §4 requirement that every affected id resolve is now met without modifying the checker.
- Phase-9 grants and the recorded affected closure keep the full union. Protected required-test deletion, orphaned structure and live references to a deleted Function all still refuse, as the new tests pin through `validate_candidate_bytes` and `native_test_plan`.
- The new CANDIDATE_RESULT_V1 phase-10 rule matches the code and does not conflict with CONTRACT_TEST_PROFILE §4 or NATIVE_TEST_ADMISSION §2. It resolves the DEAD selection mechanism the succession diagnosis identified.
- No frozen vector, fixture or contract checker drifts.

What remains:
- The derived records made stale by the source and test changes need the repository's usual records-only rebind before `make quick` is green on the main line.
- The packet's kind-change statement is imprecise but has no behavioral effect.

This verdict does not cover rerunning the DEAD benchmark, the frozen S2B-DEAD-001 candidate end to end, or any release or GA status.

VERDICT: PASS_0_P0_0_P1_0_P2_1_P3_2_P4
SECTION: dead_function_deletion_selection
FIELD: ariadne_contract_review
SCOPE_SHA: 883361e3446d77174c69a8de2821247096e17574
FINDINGS: [P3] [gate-staleness] evidence/review/requests/REQ-11-dead-function-deletion-selection.md:80-81 - The acceptance criterion "make quick at the reviewed head" is not met at 883361e3; the crates/sley-policy changes make stale 8 persistent-fuzz proof records (source_commit 69907ddf, fresh at acbc65f0), the S20-730 reproducibility report, the test inventory (sley-policy 86 vs +7 tests, feeding decision-dossier drift) and the T54 secret scan; the packet does not disclose this; frozen-vector checks all pass - closure: a records-only rebind commit with make quick exit 0 at the landing tip, or a packet amendment that limits the criterion to the vector/fixture checkers and names the rebind as a landing precondition | [P4] [claim-accuracy] evidence/review/requests/REQ-11-dead-function-deletion-selection.md:49-50; docs/spec/CANDIDATE_RESULT_V1.md:318-319; crates/sley-policy/src/candidate_validation.rs:1742-1744 - The kind-change "WrongEntityKind" branch cannot be reached (apply.rs:422-430 refuses kind change; phase 4 refuses live-ID collision) and no test covers it; if reached it would render as phase 11 TEST_PLAN_SELECTION_INVALID via contracts.rs:822-823, not WrongEntityKind - closure: reword the packet and spec to describe it as defensive, or add a test that reaches it | [P4] [latent-dead-clause] crates/sley-policy/src/native_test_plan.rs:302-309 - Pre-existing: affected.contains(entity) compares TestCase ids with Function ids and never fires; NATIVE §2 item 2 is met only through static_selected; record separately - closure: a separate change using affected.contains(&test.target) plus a test showing the native selection equals static ∪ created/replaced ∪ required with plan output unchanged
SUMMARY: The phase-10 projection removes exactly the Functions the candidate deleted, which no live TestCase can target after phase 5, so test selection is unchanged. Required-test, orphan and live-reference refusals are preserved and pinned by seven passing tests, and the new spec rule matches the code and the governing contracts. The frozen vector and fixture checkers pass, but make quick at this head is red because the source and test changes leave the fuzz-proof, reproducibility, test-inventory and secret-scan records stale; a records-only rebind is needed before landing on the main line. On the observation question, the dead native-plan clause should be recorded and fixed separately.

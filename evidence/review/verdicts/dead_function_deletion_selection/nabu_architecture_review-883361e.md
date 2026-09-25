<!-- engine: claude-code; observed model: claude-opus-5-5; scope: 883361e3446d77174c69a8de2821247096e17574; role: nabu; field: nabu_architecture_review; dispatched: 2026-09-23T03:32:29Z; duration_s: 803; process_exit_code: 0 -->
# Nabu Council review — dead_function_deletion_selection

Harness: claude-code
Reviewed checkpoint: 883361e3446d77174c69a8de2821247096e17574

What I ran and read myself:
- `git rev-parse HEAD` returned `883361e3446d77174c69a8de2821247096e17574`, so the scope matches. The worktree was clean.
- `git diff --stat acbc65f0..HEAD` shows 4 files: `candidate_validation.rs` (+21/−2 counted together with the spec), `native_test_plan.rs` (+486), `docs/spec/CANDIDATE_RESULT_V1.md` (+10/−1) and `evidence/review/requests/REQ-11-dead-function-deletion-selection.md` (+85). I read the full diff for every file.
- `git diff --check acbc65f0..HEAD` exited 0 with no output.
- `git show work/succession-sley20-arm:bench/live/DEAD-TOMBSTONE-PROPOSAL.md` (read in full) and `git show work/succession-sley20-arm:bench/live/SUCCESSION-COVERAGE.md` (DEAD section, lines 53-90).
- `git show --stat acbc65f0` and `git log -- evidence/validation/test-inventory.json`: earlier fix commits update the test inventory in the same commit.
- `cargo test -p sley-policy --lib function_deletion_tests`: exit 0, `test result: ok. 7 passed; 0 failed; 0 ignored; ... 81 filtered out`.
- `cargo test -p sley-policy`: exit 0. Lib `ok. 87 passed; 0 failed; 1 ignored`. `rw080_construction_policy` 1 passed. `s3_g3_adversary` 3 passed, 1 ignored. Doc tests 0.
- Python checkers from `make quick`, each run once at HEAD:
  - `check_candidate_result_contract.py`: exit 1. Its only problem is `master:unavailable:.../Sley2.0mastergoal.md:set-SLEY2_MASTER_GOAL`. That file is outside the repo and absent at both paths I tried, so this failure comes from the environment and is not attributed to the change.
  - `generate_candidate_result_fixtures.py --check`: exit 0, `{"drift": [], "result": "PASS", "vectors": 16}`.
  - `check_candidate_contract_freeze.py`: exit 0.
  - `check_contract_test_profile.py`: exit 0.
  - `generate_native_test_fixtures.py --check`: exit 0.
  - `check_native_test_vectors.py`: exit 0, `native-test vectors: 6 requests, 6 responses, 7 rejections OK`.
  - `check_error_symbol_registration.py --check`: exit 0.
  - `check_required_contract_index.py`: exit 0.
  - `check_finding_register.py`: exit 0.
  - **`check_decision_dossier.py`: exit 1**, `"problems": ["test-inventory:drift"]`.
  - `check_s20_700_frontier.py`: exit 0.
  - `check_local_completion_frontier.py`: exit 0.
  - `build_candidate_content_report.py --check`: exit 1, because the untracked runtime file `evidence/runtime/s20-720-release-candidate/evidence.json` is missing. This is environmental and not attributed to the change.
  - `check_candidate_result_persistent_fuzz_slice.py`: exit 0.
  - `check_succession_accounting.py`: exit 0.
  - `retire_review_claims.py --check`: exit 0.
- `build_test_inventory.py --check`: exit 1, "the tracked test inventory differs from the derived inventory". I compared the derived and tracked inventories in memory without writing anything. The only differences are sley-policy `tests` tracked 86 vs derived 93, `rust_unit_tests` tracked 1958 vs derived 1965, and `inventory_digest`.
- Searched `conformance/` and `bench/fixtures/` for pins of 24016 / `TEST_SELECTION_INVALID`. Nothing in `conformance/` matches as a standalone number. The only bench matches (`S2B-DEAD-001`, `S2B-TEST-001`) are inside a 64-hex digest, so they are coincidences.
- Not run: the full `make quick`, which includes `cargo test --workspace`; cargo tests for other packages; and the packet's revert mutation, because the tree is read-only. I checked the revert claim by reasoning through the code instead (see Evidence checked).

Files read, with line ranges:
- `evidence/review/requests/REQ-11-dead-function-deletion-selection.md` (full)
- `crates/sley-policy/src/candidate_validation.rs` 760-1099, 1732-1752, 1957-1994
- `crates/sley-policy/src/candidate_program.rs` 35-66, 75-314, 667-673
- `crates/sley-policy/src/native_test_plan.rs` 240-346, 380-640, 1789-2274
- `crates/sley-check/src/contracts.rs` 260-369, 480-508, 814-847
- `crates/sley-mutate/src/apply.rs` 34-45, 68-79, 234-238, 272-277, 1017-1029
- `docs/spec/CONTRACT_TEST_PROFILE_V1.md` 167-213
- `docs/spec/CANDIDATE_RESULT_V1.md` 1-20, 300-320
- `docs/spec/NATIVE_TEST_ADMISSION_V1.md` 21-60
- `Makefile` 1-138
- `scripts/check_decision_dossier.py` 20-27, 178-297
- `scripts/build_test_inventory.py` (`rust_counts`)
- `evidence/validation/test-inventory.json` 316-318, 382

## Evidence checked

**The defect.**
- Phase 5 builds `affected_functions` as the base/proposed union of kind-5 identities (`candidate_program.rs:289-295`, `candidate_validation.rs:802-804`).
- The S20-240 checker's `select_tests` resolves each affected Function through `function_index` and maps any failure to `TestSelectionInvalid` (`contracts.rs:821-825`).
- `contract_plan_failure` reports that at phase 11 with code 24016 (`candidate_validation.rs:1962-1978`).
- A deleted base Function is in the union but not in the proposed request, so before this change every function-deleting candidate was refused. This matches the succession DEAD diagnosis (SUCCESSION-COVERAGE lines 64-70).

**The change.**
- `live_selection_functions` (`candidate_validation.rs:1745-1751`) filters the union by `program.kinds.contains_key`. `kinds` holds every proposed entity of every kind (`candidate_program.rs:153-159`).
- Only the checker input changes (`candidate_validation.rs:1008`).
- Phase 9 still receives the full `affected_functions` (`:965`), and the recorded `affected_closure` is unchanged (`:798`).
- The checker and `CONTRACT_TEST_PROFILE_V1` §4 are untouched. The caller now meets §4's precondition ("Every affected function ... must resolve in the same closed request") instead of the checker being relaxed. Dependency direction improves: the policy caller owns the projection, and the checker keeps its fail-closed resolution rule.

**Packet claim 1: a tombstone has no live TestCase targeting it.** Holds. Every reference edge resolves through `kinds.get(..).ok_or(UnresolvedReference)` (`candidate_program.rs:667-673`) during `CandidateProgram::project` at phase 5 (`candidate_validation.rs:775-778`). `a_live_test_targeting_the_deleted_helper_still_refuses` pins phase 5 `GRAPH_UNRESOLVED_REFERENCE`.

**Packet claim 2: deleting a protected required test still refuses.** Holds. The required list is passed to the checker unprojected (`:1009`). A missing required test fails at `contracts.rs:828-832`, which is rendered at phase 11. The native plan refuses `SelectionInvalid`. Both are pinned by `deleting_the_helper_with_its_protected_required_test_still_refuses`.

**Packet claim 3: a kind change still refuses inside the checker.** The fail-closed direction holds: an identity that is still bound is kept, and a non-Function kind fails `function_index`. However, this path cannot be reached. Recreating a live base identity refuses with `IdentityAlreadyLive` (`apply.rs:234-238`), and replacing it with another kind refuses with `TargetKindMismatch` (`apply.rs:45`). The error the checker would report outward is `TEST_PLAN_SELECTION_INVALID`, not `WrongEntityKind`. See P4 #1.

**Packet claim 4: survivor selection is unchanged.** Holds.
- `dead_code_removal_keeps_survivor_selection_and_tombstones_the_helper` asserts that v1 `selected_tests == [SURVIVOR_TEST]`, that the native plan selects exactly that test, and that `affected_closure` still contains both HELPER and SURVIVOR.
- `baseline_fixture_validates_with_a_selected_survivor_test` is a non-vacuous control.
- A Function still bound in the proposed state is never filtered out, so no survivor can be dropped.

**Packet claim 5: orphaned structure still refuses.** Holds. `orphaning_the_helper_block_still_refuses` pins phase 5 `GRAPH_UNRESOLVED_REFERENCE`.

**Mutation claim (checked by reasoning).** Without the projection, HELPER stays in the checker input and fails `function_index`. The three positive deletion tests would then refuse at phase 11. The baseline test and the three refusal tests do not depend on the hunk. The packet's "four refusal tests" loosely counts the baseline test among them.

**Spec consistency.**
- The `CANDIDATE_RESULT_V1` phase-10 text (lines 311-319) matches the code exactly.
- `NATIVE_TEST_ADMISSION_V1` §2 item 2 is met through item 1. `static_selected` is the checker output over the projected set. The proposed TestCases that target the full union are the same as those that target the projected set, because no live test can target a tombstone.

**Answer to the observation question.** Confirmed: `native_test_plan.rs:306` tests `affected.contains(entity)`, where `entity` is a TestCase id and `affected` holds only kind-5 ids. Identities are unique across kinds (`candidate_program.rs:153-159`), so the clause can never fire and does not change any output today.
- Record it separately; do not fold it into this change.
- Changing it to `affected.contains(&test.target)` would also produce no change in output. Item 1 already covers item 2, so no test can distinguish the two versions, and the fix would add a second derivation of the same selection rule (the local `affected_functions` at `:577-597` already copies the validator's closure code).
- The architecturally cleaner closure is to delete the dead clause and its duplicate closure computation, with a spec note that item 2 is satisfied through item 1. Correcting the clause is acceptable if the team wants a defence-in-depth restatement, but it should be documented as such.

**Vectors and symbols.** No frozen vector pins the prior refusal. The candidate-result and native-test fixture checks pass, the error-symbol registration check passes, and the candidate contract freeze passes.

**Quick gate.** The 7 new `#[test]` functions are not reflected in the tracked test inventory, so `check_decision_dossier.py` fails. See P1.

## Findings
[P1] [gate-evidence-binding] evidence/validation/test-inventory.json:316-318,382 - The diff adds 7 `#[test]` functions (crates/sley-policy/src/native_test_plan.rs:1794-2274) but does not update the tracked test inventory. `build_test_inventory.py --check` exits 1 (sley-policy tests tracked 86 vs derived 93; rust_unit_tests 1958 vs 1965), and `check_decision_dossier.py` exits 1 with `test-inventory:drift`. `make quick` is therefore red at the reviewed head, which contradicts the packet's own acceptance criterion; prior fix commits (acbc65f0) update the inventory in the same commit - closure evidence needed: the change updates test-inventory.json and any downstream dossier/GA records whose digests move, with transcripts showing `build_test_inventory.py --check`, `check_decision_dossier.py` and a full `make quick` exiting 0 at the new head.
[P3] [spec-identity] docs/spec/CANDIDATE_RESULT_V1.md:311-319 - This is a normative judgment change: function-deleting candidates that refused at phase 11 with TEST_PLAN_SELECTION_INVALID now validate under the unchanged `full_validation_profile_id`. It lands as an unmarked in-place edit, while neighbouring normative corrections in the same spec carry dated/ADR markers (lines 5-8). Nothing in the spec records when phase-10 behaviour changed or why the profile identity is unchanged - closure evidence needed: a dated amendment note (or ADR reference) in the spec naming this change, stating that no frozen vector pinned the prior refusal and that only refuse-to-valid movement occurs.
[P4] [claim-precision] crates/sley-policy/src/candidate_validation.rs:1735-1744 - The doc comment says "policy refuses deleting a protected required test", but the refusal actually comes from the checker's required-test resolution (crates/sley-check/src/contracts.rs:828-832), rendered at phase 11 by contract_plan_failure (candidate_validation.rs:1962-1978). The packet's parenthetical "(WrongEntityKind)" for a kind change is also inaccurate: that path cannot be reached (sley-mutate apply.rs:234-238, TargetKindMismatch), and any such error would surface as TEST_PLAN_SELECTION_INVALID (contracts.rs:823-824). Behaviour is correct and fail-closed; only the wording is wrong - closure evidence needed: corrected wording, or explicit acceptance as a note.
[P4] [dead-code-duplicated-authority] crates/sley-policy/src/native_test_plan.rs:286,306,577-597 - This predates the diff and is the answer to the packet's observation question. `affected.contains(entity)` compares TestCase ids against Function ids and never fires, and `affected_functions` duplicates the validator's closure derivation. Output is unaffected because item 1 (static_selected) covers NATIVE_TEST_ADMISSION §2 item 2 - closure evidence needed: a separately recorded change that either deletes the clause and its recomputation (preferred), with a spec note that item 2 is satisfied through item 1, or corrects it to use the test target with a comment documenting that it is redundant by design; not part of this change.
[P4] [traceability] evidence/review/requests/REQ-11-dead-function-deletion-selection.md:1-85 - The originating proposal (work/succession-sley20-arm:bench/live/DEAD-TOMBSTONE-PROPOSAL.md) specified a checker-side tombstone index in contracts.rs and a positive regression on the frozen DEAD candidate (90+8e+8f/91/93, commit plus judge accept). The implementation instead uses a caller-side projection, which is architecturally preferable because the checker owner and the S20-240 contract are untouched, and tests it with a synthetic analogue fixture. The packet does not state the design deviation, and the benchmark DEAD positive has not been re-run at this head - closure evidence needed: a packet or record note stating the deviation and why, plus a separate DEAD-arm rerun before anyone claims the succession DEAD positive is unblocked.

## Assessment
The code change is correct and well placed.
- The projection sits at the single caller that violated the S20-240 checker's precondition. The checker and its contract are unchanged, so the checker still refuses any unresolved affected identity.
- Grants and the recorded affected closure keep the full union, so accounting still names the deleted Function.
- A tombstone cannot select a test, because phase 5 reference integrity refuses any live reference to it.
- Survivor selection and required-test protection hold, and the tests pin all of this through `validate_candidate_bytes` and `native_test_plan`.

All five "Why the projection cannot drop a test" claims hold at the cited code and tests, except the minor wording issues in P4 #1. No frozen vector or registered symbol changes.

The change cannot land as submitted because the main-line quick gate is red: the tracked test inventory was not updated for the 7 new tests (P1). The spec amendment also lacks the change marker that the file's own convention uses (P3).

The dead `affected.contains(entity)` clause should be recorded and fixed separately, preferably by removing it rather than adding a second derivation of the same rule.

VERDICT: REVISE_0_P0_1_P1_0_P2_1_P3_3_P4
SECTION: dead_function_deletion_selection
FIELD: nabu_architecture_review
SCOPE_SHA: 883361e3446d77174c69a8de2821247096e17574
FINDINGS: [P1] [gate-evidence-binding] evidence/validation/test-inventory.json:316-318,382 - 7 new tests not reflected in the tracked test inventory; build_test_inventory.py --check exit 1 (sley-policy 86 vs 93, rust_unit_tests 1958 vs 1965), check_decision_dossier.py exit 1 test-inventory:drift, so make quick is red against the packet's own acceptance criterion - update the inventory and downstream records; show both checkers and make quick exiting 0. | [P3] [spec-identity] docs/spec/CANDIDATE_RESULT_V1.md:311-319 - normative phase-10 judgment change (refuse to valid) under the unchanged validation profile id, edited in place without the dated/ADR amendment marker the spec uses elsewhere - add an amendment note naming the change and that no frozen vector pinned the prior refusal. | [P4] [claim-precision] crates/sley-policy/src/candidate_validation.rs:1735-1744 - required-test refusal is checker resolution (contracts.rs:828-832), not "policy"; the kind-change path cannot be reached and would surface as TEST_PLAN_SELECTION_INVALID, not WrongEntityKind - correct the wording or accept as a note. | [P4] [dead-code-duplicated-authority] crates/sley-policy/src/native_test_plan.rs:286,306,577-597 - affected.contains(entity) never fires (TestCase id vs Function ids); item 2 is met through static_selected - record separately; prefer removing the clause and its duplicate closure derivation over correcting it to the test target. | [P4] [traceability] evidence/review/requests/REQ-11-dead-function-deletion-selection.md:1-85 - deviation from the proposal's checker-side tombstone design is not stated, and the frozen DEAD benchmark positive is not re-run - note the deviation; rerun the DEAD arm separately before claiming it unblocked.
SUMMARY: The caller-side projection correctly and fail-closed-ly fixes the function-deletion test-selection defect. The S20-240 checker and its contract are untouched, grants and the affected closure keep the full union, and all 7 regression tests plus the full sley-policy suite pass. Every "cannot drop a test" claim holds at the cited code, with wording-only exceptions. The change cannot land yet: the quick gate is red because the tracked test inventory was not updated for the new tests (P1), and the normative spec amendment lacks its change marker (P3). The dead native-plan clause should be recorded and fixed in a separate change.

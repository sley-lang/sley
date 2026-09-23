# Review request REQ-11 — function deletion as a test-selection tombstone

- Scope: implementation review of one production change on branch
  `work/dead-tombstone-selection` (base `acbc65f0`, the batch-14 repair tip of
  `work/finish-20260918`). Review the diff `git diff acbc65f0..HEAD`. Scope key
  `dead_function_deletion_selection`. Lanes: Ariadne (contract) and Nabu
  (architecture), independently.
- Origin: the succession DEAD task (`bench/live/DEAD-TOMBSTONE-PROPOSAL.md` and
  the DEAD section of `bench/live/SUCCESSION-COVERAGE.md` on
  `work/succession-sley20-arm`). That proposal stated the landing requires
  independent review; this packet is that review, over the actual code.

## Defect (reproduced by the new tests with the fix reverted)

Candidate validation builds the affected Function set as the base/proposed
union (`crates/sley-policy/src/candidate_validation.rs`, phase 5:
`sorted_union(base_functions, proposed_functions)`) and passed that union to
the S20-240 checker at phase 10. The checker requires every affected Function
to resolve in the closed proposed request (`docs/spec/CONTRACT_TEST_PROFILE_V1.md`
section 4; `crates/sley-check/src/contracts.rs` `select_tests` →
`function_index`). A deleted base Function is in the union but absent from the
proposed request, so EVERY candidate that deletes a Function — however
legitimate — refused at phase 11 with `TEST_PLAN_SELECTION_INVALID` (24016).
No vector or test pinned that refusal (searched: the only numeric matches in
`conformance/` and `bench/fixtures/` are hex coincidences inside one hash).

## Change

1. `candidate_validation.rs`: new `live_selection_functions(affected, program)`
   projects the union onto identities still bound in the proposed state
   (`program.kinds`, which holds every proposed entity of every kind). Only
   the phase-10 checker input uses it. Phase 9 (`validate_phase_nine`, grants)
   and the recorded `affected_closure` keep the full union.
2. `docs/spec/CANDIDATE_RESULT_V1.md` phase-10 rule states the projection.
3. Regression module `native_test_plan::function_deletion_tests` (7 tests,
   through `validate_candidate_bytes` and `native_test_plan`).

The S20-240 checker and its contract are untouched: the checker still refuses
an unresolved or wrong-kind affected identity.

## Why the projection cannot drop a test

- A tombstone has no live TestCase targeting it: phase 5 refuses any live
  reference to a deleted identity (`GRAPH_UNRESOLVED_REFERENCE`, pinned by
  `a_live_test_targeting_the_deleted_helper_still_refuses`).
- Deleting a protected required test still refuses at phase 11 with the
  preserved static symbol (`deleting_the_helper_with_its_protected_required_test_still_refuses`),
  and the native plan still refuses `SelectionInvalid`.
- An identity still bound in the proposed state is kept whatever its kind, so
  a kind change continues to refuse inside the checker (`WrongEntityKind`).
- Survivor selection is unchanged: the DEAD shape (survivor drops its
  explicitly unreachable block + helper cascade deleted) selects exactly the
  survivor's test in both v1 and native plans
  (`dead_code_removal_keeps_survivor_selection_and_tombstones_the_helper`),
  and the non-vacuity control (`baseline_fixture_validates_with_a_selected_survivor_test`)
  shows the fixture selects that test without any deletion.
- Orphaned structure still refuses (`orphaning_the_helper_block_still_refuses`,
  phase 5 `GRAPH_UNRESOLVED_REFERENCE`).

Mutation evidence: with the `candidate_validation.rs` hunk reverted, the three
positive tests fail and the four refusal tests pass; with it applied all seven
pass; the full `sley-policy` suite passes (87 lib tests).

## Observation for the reviewers (not changed here)

`native_test_plan.rs` inside `native_test_plan`: the loop over `live_tests`
tests `affected.contains(entity)` where `entity` is a TestCase id and
`affected` holds Function ids, so that clause never fires. NATIVE_TEST_ADMISSION
section 2 item 2 ("Proposed TestCases targeting the owner-derived affected
Function closure") is still met because the static v1 selection
(`static_selected`) already contains exactly those tests. Name whether this
should be corrected to `affected.contains(&test.target)` in this change or
recorded separately; it does not change any plan output today.

## Acceptance criteria

- The diff is limited to the three items above.
- Every claim in "Why the projection cannot drop a test" holds at the cited
  code and tests.
- No frozen vector, symbol, or old-profile judgment changes (`make quick` at
  the reviewed head).

Constraints: read-only review. No file writes. Verdict in the standard
Council footer (`VERDICT/SECTION/FIELD/SCOPE_SHA/FINDINGS/SUMMARY`) against
section `dead_function_deletion_selection`.

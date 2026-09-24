# DEAD tombstone-aware selection — proposal and regression spec (not landed)

> **Status 2026-09-23: SUPERSEDED by REQ-11.** The defect was fixed by a
> different, reviewed design: a caller-side projection in
> `crates/sley-policy/src/candidate_validation.rs`
> (`live_selection_functions`, commits 883361e3 + f7f9af90) projects the
> affected-Function union onto identities still bound in the proposed
> state before S20-240 selection. The checker-side tombstone index
> proposed below in `contracts.rs` was NOT built; the S20-240 checker and
> its contract are untouched. Review: `evidence/review/requests/REQ-11-dead-function-deletion-selection.md`
> (Amendment 1 records the deviation), Ariadne PASS and Nabu PASS
> (revision 2) under `evidence/review/verdicts/dead_function_deletion_selection/`.
> The regression specification below was re-run end to end on the
> succession arm with REQ-11 merged: `bench/live/succ-trials-20260923/dead/`
> (positive ACCEPTED; outcome per item in the DEAD section of
> `SUCCESSION-COVERAGE.md`). The text below is kept unchanged as the
> historical proposal.

Status: proposal only. No semantic change landed. Review gate retained.
The minimal deletion reproducer and diagnosis in `SUCCESSION-COVERAGE.md`
(DEAD section) are preserved and not reopened. This note prepares the
exact change and its regression specification within the existing
authorized scope. Landing requires independent review; unavailability
of the review provider does not authorize landing.

## Problem (preserved)

Deleting the unreachable block 90 + unused private helper 8e (+ cascade
8f/91/93) trips test-plan selection deterministically:
`affected_functions` = base ∪ proposed kind-5 entities always contains
the deleted helper 8e, while the selection index is built from
proposed-only units (`contracts.rs` select_tests → function_index →
UnresolvedEntity). Every legitimate function deletion fails the lookup
with CANDIDATE_VALIDATION_TEST_PLAN_ERROR / TEST_PLAN_SELECTION_INVALID
(24_016). Frozen S3 never validates a real DeleteEntityBinding
candidate for this shape. No contract-conforming formulation exists in
the current op set without weakening the task (forbidden).

## Proposal (exact change, review-gated)

Tombstone-aware test-plan selection: keep deleted function identities
indexable as removed while selecting tests for survivors.

- Selection index built over proposed units PLUS tombstoned base
  functions marked removed (identity, no body).
- `affected_functions` may name a tombstoned identity; the selector
  resolves it to the tombstone, selects zero tests for the removed
  function itself, and selects the normal survivor closure for all
  other affected functions.
- Survivor dependency/test accounting unchanged: required tests for
  survivors still selected; no test deleted; reference integrity still
  enforced (orphans still fail inventory/reference checks, not silently
  dropped); old-profile historical judgments untouched (no
  benchmark-specific exception, no blanket omission of deleted
  identities, no orphan workaround).
- Effect constraints and public-identity preservation still enforced by
  the existing judge paths.

Files expected to change (not changed here): `contracts.rs`
select_tests/function_index construction plus tombstone plumbing from
the candidate/base diff. Exact diff to be prepared as a separate
reviewable commit with independent review; this proposal does not
substitute for that review.

## Regression specification (to be added with the change, not here)

- Positive: the frozen DEAD deletion candidate (90 + 8e + 8f/91/93,
  reachable behavior preserved) validates and commits; judge accepts
  with absent-check, observation stability, collateral clean, and
  survivor test selection non-empty.
- Negatives (must still reject):
  - Orphaned op 93 (block 90's operation left behind) →
    CANDIDATE_VALIDATION_UNRESOLVED_REFERENCE.
  - Dropped survivor test or deleted required test → selection/test
    failure, not silent pass.
  - Helper kept → ORACLE_UNEXPECTED_ENTITY.
  - Behavior changed while keeping dead code →
    ORACLE_REACHABLE_CHANGED (observation checked before absent).
  - Blanket omission of all deleted identities from accounting →
    must not pass; survivor accounting must still reference the
    tombstone as removed.
- Historical: old-profile judgments and S3 vectors preserved byte-exact;
  no severity/status relabeling to close the gap.

## Non-goals (explicitly refused)

- No DEAD-specific production exception in the benchmark or judge.
- No weakening of removed_private_functions, test-deletion, or
  reference-integrity requirements.
- No landing without independent review.

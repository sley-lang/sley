# S20-530 v3 mechanical refreeze design

Status: ACTIVE DESIGN

Owner: Codex orchestrator

Decision date: 2026-08-29, America/New_York

## Purpose

Refreeze the S20-530 checker only for defects that are mechanical,
source-verified, and do not change the 100-row crash-recovery matrix or its
limit-event semantics. The refreeze must make the frozen Rust renderings
compilable and make the checker's own manifests satisfiable before additional
mapped recovery tests are integrated.

The v3 freeze does not claim S20-530 implementation completion. It does not
authorize publication, push, deployment, provider use, or external runtime
mutation.

## Authorities

- `docs/spec/CRASH_RECOVERY_MATRIX_V1.md`
- `docs/adr/ADR-0023-crash-recovery-boundary.md`
- `scripts/check_s20_530_crash_recovery.py`
- `scripts/run_s20_530_validation.py`
- `/home/dev/machineresearch/sley/s20-530-checkpoint-2026-08-30/README.md`
- `/home/dev/machineresearch/sley/s20-530-checkpoint-2026-08-30/s20-530-closeout-decisions.md`
- `/home/dev/machineresearch/sley/s20-530-checkpoint-2026-08-30/deviation-ruling-analysis.md`

## Tranche A: mechanical v3 amendments

The following changes are in scope:

1. Accept the exact feature-gated `recovery_ancestry_test_hook` import and bind
   it in the recovery-ancestry hook-gate inventory.
2. Admit the four exact `#[derive(Debug)]` surface tokens required by
   `RecoveryAncestryError`, `RecoveryAncestryHeadReport`, and
   `RecoveryAncestryReport`, and `RecoveryWorkUsage`. Remove the two stale
   allowed-addition entries for maintenance functions that already exist in the
   public-surface baseline anchor, plus the stale GC function token whose frozen
   signature does not exist.
3. Canonize a typed empty source-chain value so frozen empty-chain assertions
   compile on the pinned Rust toolchain.
4. Refresh the 14 exact GC recovery test-body digests after their source bodies
   and assertion spelling are settled.
5. Refreeze ancestry and snapshot source fragments to their rustfmt-stable,
   compiling forms, including ANC-08 slice-to-array comparisons with
   `.as_slice()` and its qualified `::std::vec![...]` renderings.
6. Refresh the four exact maintenance-validator body digests that intentionally
   implement the already-frozen error codes, variants, and source chains.
7. Correct the `exact_tree_delta_paths` rendered dereference.
8. Require the five fields actually emitted by
   `limit_event_owner_body_manifest`, including `attribute_chain_sha256`.
9. Render typed empty path vectors in owned-entry bodies.
10. Clone the first reused fault snapshot operands so the frozen corruption
    prelude does not move non-Copy values twice.

Items 3, 5, 7, 9, and 10 alter only generated Rust spelling needed for the
frozen assertions to compile. They do not alter the expected recovery result,
error code, durability state, row membership, or limit-event inventory.

## Deferred semantic tranche

The following findings are real closeout blockers but are not safe to fold into
the mechanical freeze without a separate semantic ruling:

- the nine receipt SCB code-plus-variant conjunctions;
- the two dead mutation-candidate error-code leaves;
- the `SCB_LABEL_NOT_NFC` route;
- owned-entry COR-01 through COR-05 semantic-binding contradictions;
- grouped multifault result-binding contradictions;
- fatal owned-entry variant selection;
- non-regular fixture construction and path-length policy;
- ANC-06 fixture feasibility;
- the COR-07 host-ref I/O injection boundary;
- LIMIT preflight-canary ownership and non-unit scanned-peak semantics.

No deferred item may be represented as passing or silently dropped from the
matrix. Runnability remains separate from plan validation.

## Invariants

- `MATRIX_ROWS_SHA256` remains unchanged.
- `LIMIT_EVENT_SPECS_SHA256` remains unchanged.
- The spec Status line remains unchanged.
- The v2 evidence remains in Git as historical audit evidence.
- `FREEZE_EVIDENCE` moves to a newly added v3 path.
- The amended checker, v3 evidence, and matching machine-summary contract set
  are committed as one freeze change set.
- The checker is not edited after that freeze commit. A later semantic checker
  change requires a new refreeze and fresh specialist reviews.
- Closeout execution evidence binds to the eventual closeout HEAD, not to the
  v3 freeze commit.

## Refreeze sequence

1. Apply and self-test the mechanical checker changes without changing either
   frozen matrix digest.
2. Fill body-digest tables last, from the settled current bodies.
3. Point the checker at
   `evidence/validation/s20-530-crash-recovery-contract-freeze-v3.json` and set
   the v3 evidence identity.
4. Recompute `CHECKER_CONTRACT_SHA256`, raw checker SHA-256, and
   `contract_set_sha256`.
5. Obtain fresh Nabu, Ariadne, and Vulcan `PASS_CONTRACT_FREEZE` reviews bound
   to both the v3 contract-set digest and evidence-payload digest.
6. Mirror the v3 evidence path, contract-set digest, and review records into
   `machine-summary.json`.
7. Run Tier 1 and targeted contract checks, then the applicable Tier 2 contract
   refreeze checks. Do not run the full release gate.
8. Commit the single freeze change set and record its commit as `freeze_commit`
   in a follow-up summary checkpoint.

## Acceptance

Tranche A is accepted only when:

- the checker compiles and its adversarial self-tests pass;
- exact frozen-contract integrity passes against v3 evidence;
- the 100 matrix rows and 30 limit events retain their existing digests;
- all three specialist freeze reviews are freshly bound to v3;
- targeted Rust tests for the touched store, transaction, and repository
  recovery owners pass;
- formatting and lint checks for the amended files pass;
- the Git worktree is clean after the freeze and summary checkpoint commits.

The full S20-530 closeout runner remains blocked until all required mapped tests
and the deferred semantic tranche are resolved.

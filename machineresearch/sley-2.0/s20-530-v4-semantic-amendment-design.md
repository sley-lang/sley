# S20-530 v4 helper-identity refreeze design

Status: ACTIVE DESIGN

Owner: Codex orchestrator

Decision date: 2026-08-30, America/New_York

## Purpose

Refreeze the S20-530 checker for one newly proven helper-identity collision
between the completed ANC-06 multifault cases and the unmerged COR-07
visible-revision cases. The amendment changes two ANC-06 test-helper names and
no recovery result, error precedence, matrix row, limit event, or production
API.

The v4 freeze does not claim S20-530 implementation completion. It does not
authorize publication, push, deployment, provider use, or external runtime
mutation.

## Proven contradiction

The v3 checker renders these calls into direct mapped tests in
`crates/sley-repo/src/refs.rs`:

- ANC-06 codec precedence:
  `import_transaction_receipt_error(&fixture, &m2_fixture)`;
- COR-07 receipt corruption:
  `import_transaction_receipt_error(&fixture, &fixture_observation,
  &fault_path)`;
- ANC-06 store precedence:
  `object_store_read_error(&fixture, &m2_fixture)`;
- COR-07 object corruption:
  `object_store_read_error(&fixture, &fixture_observation, &fault_path)`.

Stable Rust does not overload free functions by argument count. The frozen
`exact_test_definition` also requires each mapped `#[test]` function to be a
direct child of the one exact `#[cfg(test)] mod tests`, so a nested module
cannot provide a second namespace. A local integration probe confirmed both
sides of the contradiction: the first COR-07 runtime case passed in a nested
module, while the v3 checker rejected that test by construction. The probe was
removed and the repository returned to a clean tree.

Nabu returned `BLOCK v3` with high confidence and found the contradiction
unsatisfiable under the current stable-Rust and direct-test constraints.

## Design reviews

- Nabu: `BLOCK v3`; the two-helper rename is the smallest architecture-safe
  amendment.
- Vulcan: `PASS DESIGN` after the design was corrected to refresh the embedded
  specification digest and forbid stale v3 PASS evidence under v4.

Both reviews were read-only and bound to this narrow helper-identity decision.
Fresh final-contract reviews remain required after the v4 digests settle.

## Amendment

Rename only the two ANC-06 primary probe helper IDs:

| Case | v3 helper | v4 helper |
|---|---|---|
| `ANC-06/ref_nested_codec_before_claim_mismatch` | `import_transaction_receipt_error` | `probe_ref_nested_codec_origin_receipt_error` |
| `ANC-06/ref_nested_store_before_cycle` | `object_store_read_error` | `probe_ref_nested_store_cycle_object_error` |

The checker-owned `MULTIFAULT_OVERLAY_REGISTRY` is the rendering authority for
both mapped bodies. The matching private Rust helper definitions change in the
same freeze change set. COR-07 retains its generic three-argument helper IDs.

## Rejected alternatives

- Renaming the COR-07 helper IDs has a much larger rendering blast radius
  across the visible-revision cases.
- Allowing nested mapped tests weakens the global direct-owner rule and also
  conflicts with the exact snapshot-helper cardinality checks.
- Adding default arguments, variadic Rust functions, or free-function
  overloading is not available on the pinned stable toolchain.
- Leaving the collision as a known gap would make the frozen closeout
  structurally impossible even when both runtime behaviors are correct.

## Deferred findings

This refreeze does not rule on the v3 deferred semantic findings, including
receipt SCB code-plus-variant conjunctions, dead mutation-candidate leaves,
owned-entry semantic binding, grouped multifault result binding, COR-07 host
I/O injection, or limit-canary semantics. Those findings remain explicit
closeout blockers and require their own bounded decisions.

## Invariants

- `MATRIX_ROWS_SHA256` remains unchanged.
- `LIMIT_EVENT_SPECS_SHA256` remains unchanged.
- `CORRUPTION_FIXTURE_SPECS_SHA256` remains unchanged.
- The specification changes only its embedded
  `MULTIFAULT_OVERLAY_REGISTRY_SHA256` literal. `FROZEN_SPEC_SHA256` therefore
  changes with it.
- The ADR, runner, and their frozen hashes remain unchanged.
- The five ANC-04 and ANC-06 runtime precedence outcomes remain unchanged.
- v3 evidence remains in Git as historical audit evidence.
- The checker is not edited after the v4 freeze commit. Any later checker
  change requires a new refreeze and fresh specialist reviews.

## Required digest and evidence updates

The v4 change must refresh:

1. `MULTIFAULT_OVERLAY_REGISTRY_SHA256`;
2. the matching embedded registry digest in the specification and
   `FROZEN_SPEC_SHA256`;
3. the two ANC-06 rendered multifault plan digests and mapped body bindings;
4. source-bound helper, mapped-test, and source-set manifests produced during
   closeout;
5. the checker raw SHA-256 and self-masked contract SHA-256;
6. the contract-set SHA-256;
7. a new v4 freeze-evidence path, exact evidence contract identity, and
   evidence payload digest;
8. the matching machine-summary freeze path, contract-set digest, and reviews.

Implementation and closeout manifests that do not yet exist remain absent or
explicitly deferred. No v3 manifest or review may retain PASS status under v4
unless it is regenerated and rebound to the final v4 source and contract set.

## Refreeze sequence

1. Obtain Nabu architecture and Vulcan checker-safety review of this design.
2. Rename the two checker registry helper IDs and the two private Rust helpers.
3. Update the embedded specification digest, then re-render and validate both
   exact ANC-06 mapped bodies.
4. Run checker syntax, format, lint, adversarial self-tests, and targeted
   contract probes without changing frozen matrix or limit digests.
5. Run the affected `sley-repo` runtime tests and the full crate suite.
6. Create v4 freeze evidence with fresh Nabu, Ariadne, and Vulcan reviews bound
   to the final contract-set and evidence-payload digests.
7. Update machine-summary bindings, run the Tier 2 contract check, and commit
   the freeze as one coherent change set.
8. Record the freeze commit in a follow-up checkpoint commit.

The full release gate remains deferred because this is a bounded contract
refreeze, not an integration or release boundary.

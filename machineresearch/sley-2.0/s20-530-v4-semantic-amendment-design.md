# S20-530 v4 helper-identity refreeze design

Status: ACTIVE DESIGN

Owner: Codex orchestrator

Decision date: 2026-08-30, America/New_York

## Purpose

Refreeze the S20-530 checker for one newly proven helper-identity collision
between the completed ANC-06 multifault cases and the unmerged COR-07
visible-revision cases. The amendment changes two ANC-06 test-helper names and
refreshes three stale test-only anchors exposed by the authoritative checker.
It changes no recovery result, error precedence, matrix row, limit event, or
production API.

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

The first final-contract review set was invalidated before commit when the full
checker exposed three additional v3 test-only anchors. Those records are not
reused under the expanded payload.

## Amendment

Rename only the two ANC-06 primary probe helper IDs:

| Case | v3 helper | v4 helper |
|---|---|---|
| `ANC-06/ref_nested_codec_before_claim_mismatch` | `import_transaction_receipt_error` | `probe_ref_nested_codec_origin_receipt_error` |
| `ANC-06/ref_nested_store_before_cycle` | `object_store_read_error` | `probe_ref_nested_store_cycle_object_error` |

The checker-owned `MULTIFAULT_OVERLAY_REGISTRY` is the rendering authority for
both mapped bodies. The matching private Rust helper definitions change in the
same freeze change set. COR-07 retains its generic three-argument helper IDs.

## Full-checker anchor refresh

The first v4 full-checker run stopped on the transaction fixture body digest.
A complete comparison found exactly two changed CROSS-05 fixture bodies:

| Frozen test-only body | v3 digest | v4 digest |
|---|---|---|
| `crates/sley-txn/src/repository.rs:mod:tests/implFixture:new` | `991462b2f2512c8fbed633d9d80a43cfad4043f64c59340215c72545607d4675` | `d4107101b213badb80419bbe24d79d1260297dd0c19aa341a292d94eae48db0e` |
| `crates/sley-repo/src/refs.rs:mod:tests/implFixture:new_with_workspace` | `ad76afbfe9bacbc9fc672da61f1e69af17f51c0f0be547788d0013f555762091` | `f63ae230c20058f2ff971578ef1aa6dae1fd408152cd5fa86ce299bf03c408d7` |

Both body changes are the already-committed test-fixture grant of
`DeleteEntityBinding`, required to build the accepted and ref store-cycle
canaries. The public repositories and production policy paths are unchanged.

The same audit found that the frozen error-source helper contract still
required `Vec<String>` in every owner. Frozen multifault tagged assertions use
different fixed array lengths, so transaction and ref owners require the
already-implemented const-generic return type `[&'static str; N]`. v4 accepts
that exact signature and body only in `sley-txn` and `sley-repo`; store and GC
retain their v3 `Vec<String>` authority.

## Git local authority correction

The adversarial self-test then rejected the repository's exact local Git
configuration because v3 freezes the former origin URL
`https://github.com/GreyforgeLabs/sley2.git`. The current `.git/config` and
`machineresearch/sley-2.0/01-legacy-freeze-and-authority.md` both name
`https://github.com/GreyforgeLabs/sley.git`.

v4 changes only that URL literal in `GIT_LOCAL_CONFIG_BYTES`. The core, fetch,
and branch settings remain byte-identical. This correction performs no fetch,
push, publication, or other network action, and it does not rewrite the
operator's current remote. The adversarial controls must also reject the former
`sley2.git` bytes explicitly, in addition to retaining the include/helper,
exclude, attributes, permission, and stale-record rejection cases.

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
- CROSS-05 accepts only the two refreshed test-fixture body digests above.
- Error-source helper relaxation is limited to the exact const-generic
  transaction and ref implementations; store and GC remain unchanged.
- Git local authority changes only the frozen origin URL from `sley2.git` to
  the current `sley.git`; all other config bytes and all no-network gates remain
  unchanged.
- v3 evidence remains in Git as historical audit evidence.
- The checker is not edited after the v4 freeze commit. Any later checker
  change requires a new refreeze and fresh specialist reviews.

## Required digest and evidence updates

The v4 change must refresh:

1. `MULTIFAULT_OVERLAY_REGISTRY_SHA256`;
2. the matching embedded registry digest in the specification and
   `FROZEN_SPEC_SHA256`;
3. the two ANC-06 rendered multifault plan digests and mapped body bindings;
4. the two CROSS-05 test-fixture body digests and the transaction/ref exact
   error-source helper contract;
5. source-bound helper, mapped-test, and source-set manifests produced during
   closeout;
6. the exact Git local-authority bytes and their negative controls;
7. the checker raw SHA-256 and self-masked contract SHA-256;
8. the contract-set SHA-256;
9. a new v4 freeze-evidence path, exact evidence contract identity, and
   evidence payload digest;
10. the matching machine-summary freeze path, contract-set digest, and reviews.

Implementation and closeout manifests that do not yet exist remain absent or
explicitly deferred. No v3 manifest or review may retain PASS status under v4
unless it is regenerated and rebound to the final v4 source and contract set.

## Refreeze sequence

1. Obtain Nabu architecture and Vulcan checker-safety review of this design.
2. Rename the two checker registry helper IDs and the two private Rust helpers.
   Refresh the two exact fixture body digests and the transaction/ref
   const-generic error-source helper authority. Correct the one frozen Git
   origin URL literal.
3. Update the embedded specification digest, then re-render and validate both
   exact ANC-06 mapped bodies.
4. Run checker syntax, format, lint, adversarial self-tests, and targeted
   contract probes without changing frozen matrix or limit digests.
5. Run the affected `sley-repo` runtime tests and the full crate suite.
6. Create v4 freeze evidence with fresh Nabu, Ariadne, and Vulcan reviews bound
   to the final contract-set and evidence-payload digests.
7. Update machine-summary bindings, run the full Tier 2 contract check, and
   replace every pre-expansion review with a fresh final-payload review before
   committing the freeze as one coherent change set.
8. Record the freeze commit in a follow-up checkpoint commit.

The full release gate remains deferred because this is a bounded contract
refreeze, not an integration or release boundary.

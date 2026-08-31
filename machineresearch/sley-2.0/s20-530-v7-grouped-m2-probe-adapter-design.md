# S20-530 v7 grouped M2 probe-adapter amendment design

Status: V7 FREEZE CANDIDATE

Owner: Codex orchestrator

Decision date: 2026-08-30, America/New_York

## Purpose

Prepare the smallest coherent amendment that makes the six checker-rendered
grouped M2 bodies compile without changing any leaf fixture, production
recovery behavior, public error, or failure precedence. The amendment
separates each leaf's existing three-argument diagnostic probe from the
two-argument adapter used by its grouped M2 fixture.

The operator lifted the paused Sley checkpoint and authorized bounded Forge
handoffs on 2026-08-30. This design does not itself mutate the frozen v6
contract. Publication, push, deployment, spend, trading, and external runtime
mutation remain unauthorized.

## Decision

S20-530 requires a v7 contract refreeze before the six grouped M2 tests can be
integrated.

Nabu and Vulcan independently returned `BLOCKED_CONTRACT` with high
confidence. Both reviews were read-only and named the same language-level
contradiction: the frozen renderer supplies two arguments to helper identities
whose existing Rust definitions and direct leaf callers require three.

## Current evidence

The affected exact keys are:

1. `COR-06/head_checksum/head_shape_before_receipt_missing`;
2. `COR-06/head_checksum/head_checksum_before_receipt_corrupt`;
3. `COR-07/origin_format/branch_record_format_version`;
4. `COR-07/origin_digest/branch_record_digest_mismatch`;
5. `COR-07/ref_format/ref_format_version`;
6. `COR-07/ref_digest/ref_digest_mismatch`.

The v6 `multifault_probe_window` renders
`overlay.probe_class(&fixture, &m2_fixture)` for each non-cycle probe. Across
the six cases this creates 11 two-argument call sites with these eight logical
probe roles:

| Role | Frozen leaf probe |
|---|---|
| COR-06 primary pointer decode | `decode_accepted_pointer_error` |
| COR-06 secondary receipt absence | `verify_absent_revision_receipt` |
| COR-06 secondary receipt digest | `import_transaction_receipt_error` |
| COR-07 primary origin import | `import_branch_record_error` |
| COR-07 secondary origin ancestry | `probe_branch_origin_ancestry` |
| COR-07 primary ref import | `import_branch_ref_error` |
| COR-07 secondary ref binding | `probe_ref_target_binding` |
| COR-07 secondary ancestry cycle | `probe_production_ancestry_core` |

The first seven frozen leaf probes are defined as three-argument functions and
have three-argument direct callers. The cycle probe is not directly called by
the grouped M2 plan because the checker substitutes its closed cycle
descriptor window. Stable Rust has no free-function overloading, default
arguments, or user-defined multi-arity call operator that could make both call
forms legal under one name.

Targeted `cargo test --no-run` compilation produced `E0061` for every emitted
two-argument legacy call. The current ref owner already contains compile-shaped
two-argument adapters named `import_branch_record_error_m2`,
`probe_branch_origin_ancestry_m2`, `import_branch_ref_error_m2`, and
`probe_ref_target_binding_m2`. Their presence confirms the intended separation
but cannot change the frozen v6 renderer.

`COR-07/ref_digest/ref_digest_mismatch` also lacks its grouped fixture and
observation helpers. That is a private test-support implementation gap, not a
second contract contradiction. It remains part of the six-case implementation
after the adapter contract is corrected.

## Amendment A: preserve leaf probe authority

Do not rename or reinterpret any `FaultOverlay.probe_class` value. It remains
the diagnostic probe for the corresponding direct corruption leaf and stays
bound by the existing leaf fixture metadata checks.

The six grouped overlay identities, fault facts, winner and loser authorities,
repair operations, production calls, cycle epochs, and precedence order remain
unchanged.

## Amendment B: add a closed grouped M2 adapter registry

Add one checker-owned registry keyed by exact `MultifaultKey` and side. It must
cover every directly emitted probe side for the six affected grouped cases and
no unrelated case. The grouped renderer selects the adapter from this registry
without modifying the underlying `FaultOverlay`.

Freeze these adapter identities:

| Grouped role | Two-argument adapter |
|---|---|
| COR-06 pointer decode | `decode_grouped_accepted_pointer_error_m2` |
| COR-06 receipt absence | `verify_grouped_absent_revision_receipt_m2` |
| COR-06 receipt digest | `import_grouped_transaction_receipt_error_m2` |
| COR-07 origin import | `import_branch_record_error_m2` |
| COR-07 origin ancestry | `probe_branch_origin_ancestry_m2` |
| COR-07 ref import | `import_branch_ref_error_m2` |
| COR-07 ref binding | `probe_ref_target_binding_m2` |

The cycle secondary side has no adapter entry because its exact plan emits the
checker-owned closed graph descriptor rather than a function call. Its leaf
probe metadata remains unchanged.

The registry needs an independent canonical-record digest. Changing a leaf
probe must not silently change an M2 adapter, and changing an M2 adapter must
not silently change leaf authority.

## Amendment C: freeze adapter shape and hostile controls

Add a purpose-built grouped adapter validator and self-controls that prove:

- the registry has the exact required key-and-side domain;
- each rendered direct grouped probe call uses its mapped adapter and exactly
  `&fixture, &m2_fixture`;
- no grouped plan emits a two-argument call to its legacy leaf probe;
- every mapped adapter is an exact private test helper with two parameters in
  the correct owner source;
- deleting, renaming, swapping, duplicating, or changing the arity of any
  adapter entry is rejected;
- adding an adapter to the cycle secondary side is rejected;
- ordinary non-grouped multifault plans retain their current probe rendering;
- all existing exact statement-plan, ordering, binding, snapshot, repair,
  canary, cycle, and trailing-authority controls continue to pass.

The hostile corpus must include at least one legacy-name substitution, one
missing mapping, one cross-owner adapter swap, and one three-argument mapped
call.

## Amendment D: implement private test adapters and fixtures

Add only the missing private test support required by the frozen adapter map:

- three COR-06 two-argument adapters and the two grouped head fixtures;
- the grouped ref-digest cycle fixture, activation, and observation helpers;
- any narrow private traits needed to share one adapter across compatible
  grouped fixtures.

The existing COR-07 `_m2` adapters should be reused. No production helper may
be renamed or have its signature changed.

## Contract and evidence updates

The v7 execution must update every digest transitively changed by Amendments A
through D, including:

1. the grouped adapter registry and its independent digest;
2. all six exact grouped M2 bodies and plan hashes;
3. the grouped adapter validator and hostile self-contract;
4. all new private test helper bodies and source anchors;
5. the specification and ADR descriptions of leaf probes versus M2 adapters;
6. frozen checker raw, checker self-contract, specification, ADR, runner if
   changed, evidence-payload, and contract-set digests;
7. a new immutable v7 freeze-evidence artifact and matching machine-summary
   bindings;
8. fresh final Nabu, Ariadne, and Vulcan reviews bound to the settled v7
   contract set and evidence payload.

No v6 review may be reused. The v6 checker and evidence remain immutable
historical authority.

## Production boundary

v7 changes no production recovery module, public API, wire format, error enum,
storage layout, lock protocol, durability cut, limit, failure precedence, or
matrix membership. Rust changes are limited to private test helpers and the six
mapped tests. The production projection must remain byte-identical after exact
test and test-hook exclusions.

## Rejected alternatives

- Do not change a legacy three-argument helper to two arguments.
- Do not emulate overloading with macros or callable values.
- Do not rename leaf `probe_class` metadata to make grouped rendering compile.
- Do not bypass exact body validation for the six grouped cases.
- Do not drop the grouped cases or weaken their two-operation precedence proof.
- Do not treat `cargo test --no-run` as sufficient runtime evidence.
- Do not alter production error wrapping or recovery order.

## Invariants

- The 100 matrix row IDs and `MATRIX_ROWS_SHA256` remain unchanged.
- The 30 limit events and `LIMIT_EVENT_SPECS_SHA256` remain unchanged.
- The 50 owned-entry subcases and their row membership remain unchanged.
- The 73 COR-06 leaves, 159 COR-07 leaves, 232 corruption fixture plans, and
  complete 419-test map remain unchanged.
- The six grouped M2 keys and their winner and loser authorities remain
  unchanged.
- The local Git authority bytes and no-network controls remain unchanged.
- Contract refreeze and freeze-anchor checkpoint remain separate commits.
- Any post-review contract change invalidates all v7 freeze reviews.

## Prototype and refreeze sequence

1. Restore the six Rust bodies to the last compile-valid committed state.
2. Add the checker registry, renderer selection, adapter validator, and hostile
   self-controls without changing leaf metadata.
3. Add the private test adapters and missing grouped fixture support.
4. Render the six exact bodies from the unsettled checker.
5. Run targeted `cargo test --no-run`, then run all six grouped tests and the
   owning crate suites as Tier 2 evidence.
6. Run bounded checker-function probes and relevant self-controls. Do not run
   the monolithic checker while its `rust_code_mask` runtime debt remains.
7. Settle all source, body, and contract digests before requesting fresh final
   reviews.
8. Obtain final Nabu, Ariadne, and Vulcan freeze reviews, create v7 evidence,
   update machine-summary bindings, and commit one coherent freeze change set.
9. Record the exact freeze commit in a follow-up checkpoint commit.

## Acceptance

The v7 refreeze is acceptable only when:

- all 11 directly emitted grouped probe calls use their exact adapters with
  two arguments;
- no grouped plan uses a legacy leaf probe as a two-argument callee;
- all adapter domain, identity, arity, owner, deletion, mutation, and swap
  controls pass;
- all six exact bodies compile and run against production recovery entrypoints;
- every grouped winner, repair, loser, snapshot, canary, and cycle assertion
  remains exact;
- checker syntax, formatting, lint, targeted self-controls, and Tier 2 owner
  crate suites pass;
- the 100-row, 30-limit-event, 232-fixture, and 419-test counts are independently
  recomputed and unchanged;
- fresh specialist reviews bind to the final v7 payload;
- the worktree is clean after the freeze and follow-up checkpoint commits.

The full `make v1` release gate remains deferred because this refreeze is not a
release boundary.

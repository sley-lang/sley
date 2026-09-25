# S20-530 v6 owned-entry amendment design

Status: V6 FREEZE CANDIDATE

Owner: Codex orchestrator

Decision date: 2026-08-30, America/New_York

## Purpose

Prepare the smallest coherent amendment that makes the 50 COR-01 through
COR-05 owned-entry subcases satisfiable and runnable without changing
production recovery behavior. The amendment replaces contradictory shadow
semantic assertions with exact concrete evidence, repairs Linux non-regular
fixture construction, and corrects only the checker surfaces coupled to those
owned-entry bodies.

The operator lifted the paused Sley checkpoint and authorized the bounded
Forge handoffs on 2026-08-30. This design does not itself mutate the frozen v5
contract. Publication, push, deployment, spend, trading, and external runtime
mutation remain unauthorized.

## Decision

S20-530 requires a v6 contract refreeze before owned-entry closeout.

Vulcan returned `PASS_NEEDS_V6` after a read-only independent review. Nabu's
bounded provider run exhausted its timeout without a final verdict. The
decision therefore rests on Vulcan's verdict plus deterministic local source
and checker-function probes, not on an inferred Nabu result.

## Current evidence

The frozen v5 checker has five owned-entry families and 50 subcases:

| Row | Owner | Subcases |
|---|---|---:|
| COR-01 | object store | 12 |
| COR-02 | transaction receipts | 10 |
| COR-03 | accepted head | 8 |
| COR-04 | branch origins | 10 |
| COR-05 | branch refs | 10 |

Targeted probes against the current checker established:

1. `owned_entry_operation_binding_problem` accepts all 50 checker-rendered
   bodies as exact.
2. None of those 50 bodies contains any of the four identifiers required by
   `require_semantic_assertions`: `expected_result`,
   `owned_stage_removed`, `preserved`, or `no_mutation`.
3. `require_exact_result_assertion` cannot reuse the concrete fatal
   `error.code()` or `error.symbol()` assertion because it requires a
   projection from the identifier `expected_result`.
4. Adding the four shadow bindings and assertions anywhere in a body makes
   `owned_entry_operation_binding_problem` reject the body as authority before,
   between, or after its five exact statement sequences.
5. The checker's own hostile corpus rejects the same shadow semantic block.

The demand and exact-body prohibition are structurally incompatible for all
50 subcases. Missing test implementation cannot resolve that contradiction.

The current `non_regular` renderer binds a Unix datagram socket directly at
each final fixture path. COR-01's longest relative socket path is 121 bytes
before any temporary-root prefix. Vulcan measured complete current fixture
paths from 111 through 174 bytes across COR-01 through COR-05. Linux permits
107 pathname bytes in `sockaddr_un.sun_path`, with the final byte reserved for
NUL. Shortening a normal test-root label cannot make this contract portable or
runnable.

Additional current-source probes found four coupled defects:

- `COR-02/symlink` and `COR-04/symlink` are each required to satisfy both the
  exact owned-entry body and the different exact multifault statement plan.
- All 18 fatal owned-entry subcases omit all four direct assertions required by
  `require_preflight_canary_evidence`: canary snapshots, canary kinds, expected
  canary kind, and owner-tree snapshots.
- The frozen fatal variants are `CommitError::Io` for COR-02 and COR-03 and
  `BranchError::Io` for COR-04 and COR-05. Their owned-entry classification
  paths return the code through `CommitError::Transaction` and
  `BranchError::Branch`, respectively. Exact runtime prototypes must reconfirm
  this before the v6 bytes settle.
- COR-04 and COR-05 recovery create their canonical layout and `locks/refs.lock`
  before scanning. A bare baseline therefore cannot also prove an empty
  operation delta. COR-03's trusted genesis already owns `heads/accepted`, so
  its symlink and non-regular fixtures must replace that existing entry and
  record a changed path rather than an added path.

Runtime preparation found one additional M2 helper-identity collision. The
two owned-entry multifault plans render two-argument probes named
`decode_accepted_pointer_error` and `import_branch_ref_error`, while both owner
modules already define different three-argument corruption probes under those
names. v6 assigns the owned-entry probes the unique identities
`decode_accepted_pointer_error_m2` and `import_branch_ref_error_m2`.

The same runtime probes confirmed that the corrected classified variants have
empty `Error::source` chains. The v5 M2 winner authorities incorrectly paired
the classified `CommitError::Transaction` and `BranchError::Branch` variants
with `io::Error(Other)` source labels, even though those enum variants own only
stable error codes. v6 freezes empty source chains for those two winners.

## Amendment A: use concrete owned-entry semantic evidence

Stop applying the generic `require_semantic_assertions` and
`require_exact_result_assertion` rules to COR-01 through COR-05. Those generic
rules prove only assertions about checker-demanded shadow locals. They do not
strengthen the concrete recovery proof and are incompatible with the exact
body authority.

Replace them with one purpose-built owned-entry semantic validator. It must
derive the four frozen JSON facts from the existing concrete statement plan:

- `expected_result` is proven by the exact success or error outcome, the exact
  error-code projection for fatal cases, and the exact report assertions for
  success cases;
- `owned_stage_removed` is proven by the exact removed-path delta, exact
  optional-path absence, and exact removed-stage report count;
- `preserved` is proven by exact before/after snapshots for every preserved
  fixture path;
- `no_mutation` is proven by the exact added, changed, and removed operation
  deltas plus owner-tree and cleanup-canary equality where the case requires
  no mutation.

The four JSON values remain mandatory and exact. The mapped `assertions` list
must still contain every concrete assertion used by the proof. Shadow Boolean
or string locals do not become accepted authority. Existing restoration,
decoy-root, duplicate-operation, reordered-statement, and trailing-authority
hostile controls remain mandatory.

Positive and negative self-tests must cover each semantic fact independently.
Deleting or changing any concrete outcome, delta, report, path snapshot, or
canary assertion must make the owned-entry semantic validator fail.

## Amendment B: select one exact body authority for M2 subcases

Keep both existing multifault cases:

- `COR-02/symlink`;
- `COR-04/symlink`.

For those two subcases, the multifault statement plan is the sole exact body
authority. The owned-entry validator must validate the frozen owned-entry JSON
facts against the primary phase and final snapshots of that exact multifault
plan without also demanding the mutually exclusive five-sequence owned-entry
body.

All other COR-01 through COR-05 subcases retain the five-sequence owned-entry
body authority. The dispatcher must make this distinction explicit and must
have a hostile self-test proving that neither body class can bypass the other.

The two multifault winner variants change with Amendment D. Their primary
codes, secondary faults, precedence, repair operations, path-disjointness, and
M2 operation counts remain unchanged.

## Amendment C: construct non-regular fixtures at short staging paths

Add one checker-pinned, test-only helper per Rust test owner for planting a
Unix datagram socket inode:

1. allocate a unique short socket path directly under the system temporary
   directory;
2. assert that its encoded pathname fits the Linux `sockaddr_un` capacity;
3. bind the datagram socket at that short path;
4. remove the destination only for an explicitly declared replacement case;
5. rename the socket inode to the final owned-entry fixture path;
6. return the live socket handle so the mapped test keeps it open.

The checker-rendered setup must call only that exact helper. Direct socket
binds against final owned-entry paths are rejected. The helper body, source
owner, staging-name grammar, length guard, replacement flag, rename, and return
type are contract authority.

This is fixture construction only. The recovery operation still receives the
same final path, entry kind, metadata, and owner-root topology.

## Amendment D: correct fatal owned-entry variants

Keep the public error codes unchanged and correct only the exact test variants:

| Rows | v5 variant | v6 variant |
|---|---|---|
| COR-02, COR-03 | `CommitError::Io(_)` | `CommitError::Transaction(_)` |
| COR-04, COR-05 | `BranchError::Io(_)` | `BranchError::Branch(_)` |

COR-01 continues to assert `error.symbol() == "STORE_IO"` without a separate
store variant assertion.

Update the two affected owned-entry multifault winner authorities with the
same narrow variants. Do not alter host-I/O cases that legitimately use the
`Io` variants.

Before freezing, run exact runtime prototypes for every fatal subcase in each
owner row. A mismatch blocks the refreeze and returns the design to review; it
does not authorize production rewrapping.

## Amendment E: make fixture deltas match fresh production state

For COR-04 and COR-05, prepare the complete canonical refs layout and create
then release `locks/refs.lock` before the fresh baseline snapshot and before
the exclusive maintenance guard is acquired. Pin those direct setup calls and
their order. The recovery operation must then leave that baseline layout
unchanged.

Replace the single `owned_entry_expected_added_paths` assumption with an exact
fixture-delta contract containing added, changed, and removed paths. Preserve
the existing added-path calculation for ordinary cases. For COR-03 symlink and
non-regular cases:

- treat `heads/accepted` as a pre-existing trusted-genesis path;
- remove it only as an explicit fixture-replacement step;
- plant the hostile entry at the same path;
- record `heads/accepted` in the fixture changed set, not the added set.

No production layout or recovery code changes are permitted.

## Amendment F: render the required fatal canary assertions

For every fatal owned-entry body and the primary phase of each owned-entry M2
body, render and require exact assertions for:

- the canary before and after snapshots;
- the canary before and after entry kinds;
- the frozen expected canary kind;
- the owner-tree before and after snapshots.

The existing hash and kind metadata rules remain. The direct assertions must
be part of the checker-rendered body authority, not trailing plan-only text.
The two M2 cases must bind their final post-repair snapshots to the same
preflight canary facts without claiming that the intentionally executed repair
never occurred.

## Contract and evidence updates

The v6 execution must update every digest transitively changed by Amendments A
through F, including at least:

1. the owned-entry renderer and semantic-validator self-contract;
2. the three exact non-regular helper bodies and source anchors;
3. the two owned-entry multifault winner authorities and
   unique secondary-probe identities and `MULTIFAULT_OVERLAY_REGISTRY_SHA256`;
4. all affected mapped-body and helper-body manifests;
5. the specification and ADR descriptions of owned-entry and M2 evidence;
6. frozen checker raw, checker self-contract, specification, ADR, runner if
   changed, evidence-payload, and contract-set digests;
7. a new immutable v6 freeze-evidence artifact and matching machine-summary
   bindings;
8. fresh final Nabu, Ariadne, and Vulcan reviews bound to the settled v6
   contract set and evidence payload.

No v5 review may be reused. The v5 checker and evidence remain immutable
historical authority.

## Production boundary

v6 changes no production recovery module, public API, wire format, error enum,
storage layout, lock protocol, durability cut, limit, or failure precedence.
Rust changes are limited to private test helpers and mapped tests. The
production projection must remain byte-identical after exact test and
test-hook exclusions.

## Rejected alternatives

- Do not add semantic shadow locals around an otherwise exact body.
- Do not weaken the exact-body prefix, sequence, operation-count, owner-root,
  or trailing-authority gates.
- Do not accept `error.code()` as if it were a variable named
  `expected_result` under the generic validator.
- Do not depend on shorter normal test-root names for Unix socket fixtures.
- Do not use FIFO creation through shell commands or FFI in mapped tests.
- Do not drop either owned-entry M2 case merely to avoid its second exact body
  contract.
- Do not change production error wrapping to satisfy stale test variants.
- Do not treat production-created canonical layout as unexpected mutation;
  establish it before the fresh baseline instead.
- Do not overwrite `heads/accepted` without an exact replacement and
  changed-path proof.

## Invariants

- The 100 matrix row IDs and `MATRIX_ROWS_SHA256` remain unchanged.
- The 30 limit events and `LIMIT_EVENT_SPECS_SHA256` remain unchanged.
- The 50 owned-entry subcases and their row membership remain unchanged.
- The 73 COR-06 leaves, 159 COR-07 leaves, 232 corruption fixture plans, and
  complete 419-test map remain unchanged.
- Every v5 COR-06 and COR-07 runtime outcome and mapped body remains unchanged.
- The local Git authority bytes and no-network controls remain unchanged.
- Contract refreeze and mapped-test implementation remain separate commits.
- Any post-review contract change invalidates all v6 freeze reviews.

## Prototype and refreeze sequence

1. Implement isolated test-only prototypes for the short-socket helper,
   COR-03 replacement delta, ref-layout baseline, and current fatal variants.
2. Run the five owned-entry `non_regular` prototypes and every fatal variant
   prototype through the real owner recovery operation.
3. Apply Amendments A through F to the unsettled checker, specification, ADR,
   and private test support.
4. Run bounded checker-function probes across all 50 owned-entry subcases and
   both owned-entry M2 cases. Do not run the monolithic checker while its
   `rust_code_mask` runtime debt remains unresolved.
5. Run the checker's owned-entry, multifault, canary, source-helper, contract,
   and hostile self-controls individually.
6. Compile and run the affected store, transaction, and repository mapped
   tests, then run the three complete owning crate suites as Tier 2 evidence.
7. Settle all source, body, and contract digests before requesting fresh final
   reviews.
8. Obtain final Nabu, Ariadne, and Vulcan freeze reviews, create v6 evidence,
   update machine-summary bindings, and commit one coherent freeze change set.
9. Record the exact freeze commit in a follow-up checkpoint commit.

## Acceptance

The v6 refreeze is acceptable only when:

- all 50 owned-entry plan entries validate without semantic shadow bindings;
- deleting or changing any concrete semantic proof is rejected;
- the two M2 subcases validate against exactly one body authority each;
- no final owned-entry path is passed directly to `UnixDatagram::bind`;
- every short socket staging path passes an explicit capacity guard and all
  five `non_regular` runtime cases pass;
- every fatal row returns its frozen code through the corrected exact variant;
- every fatal preflight canary assertion is present in and bound to the mapped
  body;
- COR-03 replacement and COR-04/COR-05 layout deltas match actual runtime
  snapshots exactly;
- checker syntax, formatting, lint, targeted self-controls, and Tier 2 owner
  crate suites pass;
- the 100-row, 30-limit-event, 232-fixture, and 419-test counts are independently
  recomputed and unchanged;
- fresh specialist reviews bind to the final v6 payload;
- the worktree is clean after the freeze and follow-up checkpoint commits.

The full `make v1` release gate remains deferred because an owned-entry
contract refreeze is not a release boundary.

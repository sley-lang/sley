# S20-530 v5 semantic amendment design

Status: AUTHORIZED FOR LOCAL REFREEZE, EXECUTION IN PROGRESS

Owner: Codex orchestrator

Proposal date: 2026-08-30, America/New_York

## Purpose

Prepare the smallest coherent S20-530 contract amendment that removes every
currently proven v4 impossibility without changing production recovery
behavior. This proposal repairs Rust call and assertion shapes, aligns visible
revision cases with carriers the production codecs actually expose, and
removes one recovery-unreachable SCB label case.

The operator authorized the local v5 refreeze on 2026-08-30 after lifting the
paused checkpoint. That authorization covers local checker, specification,
ADR, runner, private test, and freeze-evidence work. It does not authorize
provider calls, publication, push, deployment, or external runtime mutation.

## Current evidence

The current v4 grouped corruption authority contains 235 leaves:

- COR-06: 74 leaves, with two implemented and 72 deferred;
- COR-07: 161 leaves, with 124 implemented and 37 deferred.

The 109 deferred leaves reduce to six normalized defects:

1. two ANC-04 helper names collide with 68 COR-06 three-argument probes;
2. sixteen direct empty source-chain assertions cannot infer a const-generic
   array length and element type;
3. three object-absence roles preserve `io::Error(NotFound)`, but the two
   frozen source helpers panic on that kind;
4. one multifault cycle assertion compares a slice to an owned array;
5. nine outer-receipt selectors have no matching outer receipt carrier and
   affect 27 role-instantiated leaves;
6. two candidate semantic codes are unreachable after strict decoding and
   affect six role-instantiated leaves.

The inventories above were recomputed from the current v4 checker and exact
Rust test names at `dacdc7ac513bc123cc8e199405d02d7c0a6f5918`. They do not
infer completion from the absence of checker output.

## Confidence

| Finding | Confidence | Basis |
|---|---|---|
| helper-name, empty-array, NotFound, and array-shape amendments | high | direct compiler failures and exact checker renderings |
| state-root carrier changes | high | exact contract, epoch, field-order, map-order, and duplicate-map mutations reach the proposed nested errors through direct receipt import and repository recovery |
| candidate Bool, UTF-8, and float carrier changes | high | accepted typed candidates reach the proposed nested errors through direct receipt import and repository recovery |
| removal of visible `SCB_LABEL_NOT_NFC` | high | label decoding exists only in entity-object metadata, while fixed-path object corruption loses first to digest or substitution checks |
| candidate descriptor and payload result correction | high | decoder resolves descriptors and payload ownership before semantic candidate validation |

## Amendment A: resolve the COR-06 helper collision

Rename only the two ANC-04 primary probe helpers and their checker-owned
multifault helper IDs:

| Current ANC-04 helper | Proposed v5 helper |
|---|---|
| `import_transaction_receipt_error` | `probe_anc04_nested_receipt_error` |
| `object_store_read_error` | `probe_anc04_nested_store_error` |

The receipt helper is used by two ANC-04 mapped tests. The store helper is used
by one. COR-06 keeps its existing generic three-argument probe names. This
changes three ANC-04 bodies instead of changing 68 COR-06 fixture records.

The `MULTIFAULT_OVERLAY_REGISTRY` helper IDs, the three private Rust helper
definitions, and all three rendered ANC-04 body bindings change together.

## Amendment B: freeze typed empty direct source chains

For transaction and ref owners whose helper returns `[&'static str; N]`, an
empty direct source-chain assertion must use exactly:

```rust
::core::assert_eq!(
    crate::repository::tests::exact_error_source_chain::<0>(&error),
    [] as [&'static str; 0],
);
```

The ref-owner form uses `crate::refs::tests`. Store and GC owners retain their
existing `Vec<String>` helper authority and compare empty chains with
`Vec::<String>::new()`.

The checker must require the owner-specific form rather than accepting several
interchangeable spellings. Positive and negative self-tests must cover missing
turbofish, untyped empty arrays, wrong helper owner, a vector against the
const-generic helper, and a typed array against a vector helper.

This amendment unlocks four COR-06 and twelve COR-07 direct leaves. It does not
change the four empty-chain multifault tuple assertions that already compile.

## Amendment C: preserve NotFound in exact source helpers

Add this exact arm to the transaction and ref repository source helpers and to
their checker-owned expected bodies:

```rust
::std::io::ErrorKind::NotFound => "io::Error(NotFound)",
```

The existing `Other` arm and panic fallback remain. Store and GC helper bodies
remain unchanged because no affected grouped leaf uses them.

This is test observation only. Production `StoreError::io` already preserves
the host `NotFound` source and maps it to `STORE_OBJECT_NOT_FOUND`.

## Amendment D: repair the ref-digest cycle operand

Change only the checker-rendered right operand of the
`COR-07/ref_digest/ref_digest_mismatch` secondary cycle assertion from an owned
two-element array to that array's slice:

```rust
[
    (m2_cycle_left_transaction_id, m2_cycle_right_transaction_id),
    (m2_cycle_right_transaction_id, m2_cycle_left_transaction_id),
]
.as_slice()
```

The left operand remains `m2_secondary_cycle_descriptor.as_slice()`. Cycle
topology, epochs, expected winner, expected loser, and operation ordering do
not change.

## Amendment E: align visible SCB cases with real carriers

Eight outer receipt cases retain their error code but move to a carrier that
can emit it honestly:

| v4 normalized case | v5 normalized case | v5 variant | v5 corrupter class |
|---|---|---|---|
| `receipt_scb_contract_unknown` | `state_root_scb_contract_unknown` | `commit.codec.state_root.scb` | `receipt_nested_state_root` |
| `receipt_scb_epoch_mismatch` | `state_root_scb_epoch_mismatch` | `commit.codec.state_root.scb` | `receipt_nested_state_root` |
| `receipt_scb_bool_invalid` | `candidate_scb_bool_invalid` | `commit.codec.candidate` | `receipt_nested_candidate` |
| `receipt_scb_utf8_invalid` | `candidate_scb_utf8_invalid` | `commit.codec.candidate` | `receipt_nested_candidate` |
| `receipt_scb_float_non_canonical` | `candidate_scb_float_non_canonical` | `commit.codec.candidate` | `receipt_nested_candidate` |
| `receipt_scb_field_order` | `state_root_scb_field_order` | `commit.codec.state_root.scb` | `receipt_nested_state_root` |
| `receipt_scb_map_order` | `state_root_scb_map_order` | `commit.codec.state_root.scb` | `receipt_nested_state_root` |
| `receipt_scb_map_duplicate` | `state_root_scb_map_duplicate` | `commit.codec.state_root.scb` | `receipt_nested_state_root` |

The code and role-specific source chain remain unchanged. COR-07 continues to
wrap the corrected commit variant with `branch.transaction`.

The three candidate SCB cases require accepted source revisions whose stored
candidate contains a canonical `ConstantBody` carrier for Bool, Text, or
floating-point bits. The corruption helper changes only that selected nested
value and reseals the candidate and receipt envelopes. It does not change the
transaction, pointer, other role, or production decoder.

The five state-root cases mutate the selected nested state-root envelope or
record before its own digest or transaction binding can supersede the intended
decoder error. Existing state-root record and envelope range helpers are the
starting implementation authority.

### Remove the unreachable label case

Remove `receipt_scb_label_not_nfc` from `VISIBLE_REVISION_CASES` and all three
derived role instances. The outer receipt has no normalized-label field.
Candidate, candidate-result, state-root, policy-root, and transaction records
also have no label carrier.

Entity-object metadata does have a normalized label, but the object store first
binds bytes to the path-derived `ObjectId`. Mutating that fixed-path object to a
non-NFC label therefore returns `SCB_DIGEST_MISMATCH` without resealing or
`STORE_OBJECT_SUBSTITUTION` after resealing. A single-artifact corruption
fixture cannot reach `SCB_LABEL_NOT_NFC` through recovery without weakening
content addressing or rewriting the entire visible revision graph.

The lower-level entity-object NFC unit test remains. v5 does not remove the
public error code or weaken label validation; it removes only the impossible
S20-530 recovery leaf.

## Amendment F: record decoder-before-semantic candidate precedence

Keep these two normalized candidate selectors and variants, but change their
expected result code to the production decoder result:

| Selector | v4 expected code | v5 expected code |
|---|---|---|
| `candidate_mutation_candidate_descriptor_unknown` | `MUTATION_CANDIDATE_DESCRIPTOR_UNKNOWN` | `SCB_UNION_INVALID` |
| `candidate_mutation_candidate_payload_kind` | `MUTATION_CANDIDATE_PAYLOAD_KIND` | `SCB_UNION_INVALID` |

The strict decoder resolves the operation descriptor and payload compatibility
before `CandidateRecord::validate`. v5 treats that precedence as the contract.
The semantic codes remain part of the candidate API and retain lower-level
builder or validation coverage; S20-530 no longer claims imported stored bytes
can reach them.

## Derived v5 inventories

The proposed normalized visible-revision inventory contains 71 cases:

- 12 receipt envelopes;
- 13 nested transactions;
- 14 nested candidates;
- eight nested candidate results;
- nine nested state roots;
- eight nested policy roots;
- one semantic manifest;
- one absent receipt;
- one receipt host-I/O case;
- two object-byte cases;
- one absent object;
- one object host-I/O case.

The grouped counts become:

- COR-06 groups: `2, 1, 48, 9, 8, 1, 3, 1`, for 73 leaves;
- COR-07 groups: `2, 1, 5, 1, 2, 1, 71, 76`, for 159 leaves;
- corruption fixture registry: 232 leaves, comprising 213 visible-revision
  plans, two accepted-head-pointer plans, and 17 ref-owner plans;
- complete matrix map: 419 unique mapped Rust tests.

These counts were derived mechanically from the proposed case table. The 126
currently implemented grouped tests retain their names and production outcome.
The removed label leaves and every renamed or reclassified case are currently
unimplemented under v4.

## Production boundary

v5 changes no production module, public API, wire format, error enum, recovery
operation, storage layout, lock protocol, limit, or failure precedence. The
only Rust source changes inside crates are private test helpers and mapped
tests. A dev-only `sley-ssmc` dependency lets the transaction crate build real
typed candidate fixtures without altering its production dependency graph.
The production projection must remain byte-identical after exact test and
test-hook exclusions.

## Rejected alternatives

- Do not weaken strict candidate decoding merely to expose later semantic
  errors from malformed stored bytes.
- Do not add an epoch, contract, Boolean, string, label, float, ordered record,
  or map field to the transaction receipt wire format for test coverage.
- Do not relabel unrelated outer receipt corruptions with desired error codes.
- Do not weaken object-store digest-first and substitution checks to expose a
  label error at a mismatched path.
- Do not let one corruption helper rewrite a complete transaction, state root,
  object manifest, receipt, and pointer graph while claiming one selected
  artifact fault.
- Do not change const-generic helpers to vectors, because nonempty transaction
  and ref exact assertions already rely on fixed array lengths.
- Do not rename 68 COR-06 probes when three ANC-04 bodies can resolve the same
  namespace collision.
- Do not retain 422 as a runner constant after the authoritative grouped leaf
  count becomes 232.

## Invariants

- The 100 matrix row IDs and `MATRIX_ROWS_SHA256` remain unchanged.
- The 30 limit events and `LIMIT_EVENT_SPECS_SHA256` remain unchanged.
- All durability cuts, recovery operations, guard relations, limit defaults,
  and work-accounting semantics remain unchanged.
- Existing compatible COR-06 and COR-07 runtime outcomes remain unchanged.
- `SCB_LABEL_NOT_NFC`, `MUTATION_CANDIDATE_DESCRIPTOR_UNKNOWN`, and
  `MUTATION_CANDIDATE_PAYLOAD_KIND` remain production error codes; only their
  S20-530 imported-corruption expectations change.
- The local Git authority bytes and no-network controls remain unchanged.
- v4 evidence remains immutable historical evidence.
- The v4 checker remained byte-identical until the operator authorized the v5
  refreeze on 2026-08-30.

## Required contract and evidence updates after authorization

A v5 execution must update at least:

1. `GROUPED_ERROR_CASES_SHA256` and all derived grouped case assertions;
2. `CORRUPTION_FIXTURE_SPECS_SHA256`, its specification literal, and every
   derived fixture-plan digest;
3. `MULTIFAULT_OVERLAY_REGISTRY_SHA256` for the two ANC-04 helper IDs;
4. the typed empty-chain validator and self-tests;
5. the transaction and ref exact source-helper bodies and anchors;
6. the ref-digest multifault rendering and exact mapped-body binding;
7. `docs/spec/CRASH_RECOVERY_MATRIX_V1.md` counts, carrier inventory, and
   registry digest;
8. `docs/adr/ADR-0023-crash-recovery-boundary.md` grouped counts, registry
   counts, and complete mapped-test count;
9. the runner's authoritative mapped-test count from 422 to 419;
10. frozen specification, ADR, runner, checker raw, checker self-contract, and
    contract-set digests;
11. a new v5 freeze-evidence artifact and matching machine-summary bindings;
12. fresh final Nabu, Ariadne, and Vulcan reviews bound to the settled v5
    contract-set and evidence-payload digests.

No v4 PASS review may be reused as a v5 review. Provider-backed specialist
calls require explicit operator authorization under the current Greyforge
gate.

## Settled local review target

The local v5 contract bytes settled on 2026-08-30 with these identities:

- contract set: `de921bbe2efda26d77c2d7476a8d30556b4a6ec5e077e83541990f189255c08d`;
- review payload: `4f55cd2d086b05376aaf4a22a767ab73540a3b5405d52b4c8a68122526597c62`;
- specification: `3d6c15c2b07de12fe77dd25fc342788530a98a3b5ad031d02e522362a1e8dadc`;
- ADR: `f0af44d97c523a5b3b1baa5a3bf3464f804d37310126f372479f2fe486c4a102`;
- runner: `56bcd9463781bbece8cd36dd2b23fa6868e5f1faf2ffa9210f07428ffb30a1c0`;
- checker raw: `7f4abef51e602f252bdd00847b5bcf229d12c15af3d8bc6244b08a1a970e4ee6`;
- checker self-contract: `622d1b683d318b76ae77a1eb6cc70ba8aa77d07e368089ed754ba1ea23ad4022`.

The non-authoritative review packet is
`machineresearch/sley-2.0/s20-530-v5-contract-freeze-review-packet.json`.
It is not freeze evidence and carries no review verdicts. Fresh Nabu, Ariadne,
and Vulcan calls remain pending exact operator authorization for provider-backed
specialist work.

Local validation passed the 71-case registry and all 232 fixture-plan metadata,
checker syntax and Python formatting/lint, runner self-controls, the 50-file
frozen test-authority inventory, exact source-helper bodies, typed empty-chain
positive and hostile controls, all three changed ANC-04 exact bodies, the
122-statement COR-07 ref-digest rendering, 123 active `sley-txn` tests, and 244
`sley-repo` tests. The full checker negative-control corpus was stopped after
8 minutes 27 seconds while CPU-active in an unrelated production-source parser;
the relevant bounded controls above completed successfully.

## Refreeze sequence after explicit authorization

1. Prototype the three typed-candidate carriers and five nested state-root
   corruptions in local tests. Require exact production error codes before
   editing contract authority. Completed: both direct transaction-receipt
   import and repository recovery returned the exact proposed nested errors in
   `v5_candidate_scb_carriers_reach_recovery_import` and
   `v5_state_root_scb_carriers_reach_recovery_import`.
2. Apply Amendments A through F to the checker, specification, ADR, runner, and
   private test helpers as one unsettled local change set.
3. Recompute the grouped, fixture, and multifault registries, then update
   embedded specification digests.
4. Run checker syntax, formatting, lint, adversarial self-tests, exact render
   probes, and negative controls.
5. Compile and run the affected ANC-04, source-helper, candidate-carrier,
   state-root-carrier, object-absence, empty-chain, and ref-digest tests.
6. Run the complete `sley-txn` and `sley-repo` crate suites. Run only the
   applicable Tier 2 S20-530 contract-refreeze gate; do not use the full release
   suite as an inner loop.
7. Settle all source and body digests before requesting fresh final specialist
   reviews. Any post-review contract change invalidates every review.
8. Create v5 freeze evidence, update machine-summary bindings, run the final
   contract-refreeze checks, and commit one coherent freeze change set.
9. Record the exact freeze commit in a follow-up summary checkpoint commit.

## Acceptance

The v5 refreeze is acceptable only when:

- the candidate and state-root carrier prototypes produce every proposed code
  through the same production import paths used by recovery;
- the checker compiles, formats, lints, and passes all adversarial self-tests;
- the 100 matrix rows and 30 limit events retain their v4 digests;
- the proposed 73, 159, 232, and 419 counts are independently recomputed;
- every previously implemented grouped test remains statically bound and
  passes its runtime suite;
- all newly unlocked structural cases compile with exact frozen assertions;
- fresh Nabu, Ariadne, and Vulcan reviews bind to the final v5 payload;
- supply-chain evidence passes its current local gates;
- the worktree is clean after the freeze and follow-up checkpoint commits.

The full `make v1` release gate remains deferred because a bounded S20-530
contract refreeze is not itself a release boundary.

# S20-310 Full Root-Backed Query Closeout

Status: **implemented under the draft Root-Backed Query Profile v1 contract (revision 7, 2026-09-15; revision 3 on 2026-09-03, revision 5 on 2026-09-13, revisions 6 and 7 on 2026-09-15); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus query-and-repository-focused Tier 2 handoff**

## Claim under review

The nineteen root-backed query classes the master goal requires now have
exact bounded semantics over one verified root: the pure `sley-query`
engine binds an arm-2 complete-root snapshot, the complete-root request
bodies, the record's bindings and facts, and the stored field-4
fingerprints together (`QUERY_ROOT_MISMATCH` otherwise), answers every
class completely under the work ceiling, and pages the result by canonical
key with typed `after` and `next_after` cursors and an exact `total_count`
on every page, only when the caller allows continuation. The repository
surface in `sley-repo` is the only producer of that input from persistent
state, over an S20-390 verified revision and the S20-300 cache, and is the
first and only consumer of a cache hit (edges only). The contract is
`docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` with ADR-0030. It is a draft:
every Council lane was unavailable when it was written and when the
implementation landed, so the reviews that freeze it and complete the
package are pending and must pass before the status above changes. The
class enumeration itself is the integrator's, because no authority in the
repository listed the nineteen classes.

The implementation provides:

- `sley-id`: the thirty-third domain `sley2.root-query.v1` and
  `RootQueryId` with its frozen vector;
- `sley-query`: `root_query.rs` with `RootQuery` (nineteen classes),
  `RootQueryInput` and its binding rules, `build_root_query_request`,
  `execute_root_query`, `Cursor`, exact-then-paged results, the `SLEYRQQ1`
  request preimage and `SLEYRQR1` response record, `class_applies_to`, and
  `RootQueryErrorCode` with all eleven codes 31000 through 31010; the
  restricted helpers are shared inside the crate and the restricted
  `SLEYQRY1`/`SLEYQRS1` surfaces are untouched;
- `sley-repo`: `root_query.rs` with `run_root_query` over the S20-250
  extraction adapter, `stored_fingerprints`, and the S20-300 cache, with
  extraction, cache, and query failure namespaces preserved.

## Evidence

- Contract draft revision 7 (2026-09-15) and ADR-0030. Revision 3
  (2026-09-03) was revision 2 plus the section 1 root-bound input binding
  (`verify()` recomputes the `StateRoot` digest from the nine
  `STATE_ROOT_V1` fields with `interpretation_flags` in the input, so every
  caller-declared answer-bearing fact is committed); revision 4 composed
  the entity-read surface (section 11); revision 5 (2026-09-13) repaired
  the review-round P1/P2 text items; revision 6 (2026-09-15) closed the
  round-7 wording packet (sections 3, 7, 9, 10) under the 2026-09-15
  operator authorization; revision 7 (2026-09-15) repairs the a809906
  council round's section 7 audit attribution, class names, and class-1
  direct-edge count, the section 3 duplicate sentence, and the section 9
  evidence rule (the unit walk `single_item_classes_walk_two_items_at_limit_one`
  now covers the `Roots`, `EntryRows`, `DependencyRows`, and
  `InventoryEntries` arms). Implementation, corpus, oracle, and fuzz slice
  refreshed accordingly.
- Conformance corpus: `conformance/root-backed-query/v1/accepted.json`
  (twenty-three vectors: all nineteen classes over the frozen S20-250
  fixture with the S20-300 snapshot, the honestly recomputed root, epoch,
  workspace, synthetic bindings, fingerprints, and roots, plus two-page
  continuation walks over entities and edges) and `rejected.json` (eight failures: truncation
  without continuation, wrong cursor type, cursor on a single-key class,
  class not applicable, unresolved entity, noncanonical filter, depth cut,
  work exhausted), drift-gated by
  `scripts/generate_root_backed_query_fixtures.py --check` in `make quick`;
  `scripts/check_root_backed_query_vector.py` recomputes every
  `RootQueryId` and response record from the S20-250 fixture bodies, its
  frozen edge set, and the fixture context with its own encoder, work
  accounting, paging, and precedence, registered in `make conformance`
  under the frozen oracle environment.
- Native tests: `sley-query` root-query tests (every class over the
  fixture with 128-run determinism; continuation walks over entities and
  edges that union to the complete result with exact totals, plus the
  omitted, invalid-cursor, depth-cut, and single-key failures; the
  applicability, resolution, shape, limit, binding, arm-1, foreign-request,
  and drift matrices with the eleven-code table; the section 4 schedule pin
  over all nineteen traversal charges including the namespace-subject
  zero-scan case; the seven-fact binding-tamper matrix where every
  substituted answer-bearing fact is `QUERY_ROOT_MISMATCH`), one
  `sley-repo` test (rebuild then cache hit answering byte-identical
  records, a by-kind and entity walk against the verified objects, and the
  cached edges served with the object store removed); `sley-query` 65
  tests, `sley-repo` 346 tests, `sley-id` 7 tests pass.
- Persistent fuzz: `fuzz/targets/root_query_engine.rs` (structured root
  plus typed query, limits, flag, and cursor; determinism, record length,
  count, truncation, and continuation-walk invariants; the target claims
  the honestly recomputed digest so mutated roots keep reaching the engine
  instead of dying at binding), smoke `PASS` over a
  971-seed corpus (`docs/audits/S20_700_ROOT_QUERY_PERSISTENT_SLICE.md`);
  seventeen scoped targets and sixteen smoke gates now stand; the S20-700
  finding register and independent review remain deferred.
- Tier 1: `make quick` green at every commit.
- Tier 2: see the validation record below.

## Findings closed in flight

- Section 1 input binding checked only shape agreement and left seven
  root-committed answer-bearing facts caller-declared (the bound
  `ObjectId` values, the entry points, the dependency roots, and the three
  roots, none in the `RootQueryId` preimage), reproduced by a tampered
  binding verifying clean (Nabu P0). `verify()` now recomputes the
  `StateRoot` digest from the nine `STATE_ROOT_V1` fields exactly as given
  (adding `interpretation_flags` to the input, via a new pure
  `sley-state-root` helper with no registry and no new authority), and the
  seven-fact tamper matrix pins the mismatch; the honest `sley-repo`
  extraction verifies unchanged, confirming the revision layer already
  binds extraction to the record and the hole was exactly at query
  binding.
- Section 4 work charging could not derive the implemented per-class
  numbers while `charged_work` stayed a frozen `SLEYRQR1` field (Nabu P0,
  same defect as the Ariadne schedule finding). Closed by the frozen
  section 4 schedule: GetEntity flat 3, GetSemanticFingerprint flat 2,
  class 9 zero-scan for a namespace subject, and classes 12/13 over every
  snapshot edge are all pinned, with `charged_work` kept in the record.
- The `option(cursor)` encoding carries an option tag and a cursor tag
  before the payload; the response length accounting was corrected to the
  contract's grammar and the oracle reproduces it independently.
- The fixture's contract targets the type definition and its function
  declares no effects, so the frozen class-16 and class-18 vectors name
  the type definition and the adapter import; the applicability rule for
  `ListDeclaredEffects` is exercised by the rejection corpus.

## Explicitly open and deferred

- **Council reviews.** Ariadne, Nabu, and Vulcan reviews are queued behind
  the S20-250, S20-510, S20-520, and S20-300 reviews and land as contract
  revisions; the campaign record lists the open questions, above all
  whether the nineteen classes are the right enumeration.
- The S20-320 capsule consumes the root-backed response: a context capsule
  builds only from a `RootQueryRequest` and the `RootQueryResponse`
  produced for it (`crates/sley-query/src/context_capsule.rs`).
- Strict pedantic clippy debt in older `sley-repo` exchange and GC test
  modules is pre-existing; the new paths lint clean under `--no-deps`.

## Validation record

Tier 1 `make quick` passed at every commit of the slice. Tier 2 ran on
2026-09-03 at `76cf2a2`: `make core` (945 tests), `make conformance`
(including the root-backed query oracle line), `make adversarial`,
`make fuzz-smoke`, and `make root-query-persistent-fuzz-smoke` all exited 0
in 38 seconds; the per-gate record is in
`machineresearch/sley-2.0/s20-310-full-root-backed-query-campaign-2026-09-03.md`.
Tier 2 re-ran for contract revision 3 on 2026-09-05: `make quick`,
`make lint`, `make core`, `make conformance` (the snapshot, query, and
capsule oracles recompute the refreshed fixtures independently),
`make adversarial`, `make fuzz-smoke`,
`make root-query-persistent-fuzz-smoke`, and
`make context-capsule-persistent-fuzz-smoke` all exited 0.
The full `make v1` gate was skipped
because this is a subsystem handoff, not a release boundary; `make v2` and
`make release-check` remain intentionally fail closed.

## Independent review

Three Council rounds have landed on the nineteen-class contract (FAIL
rounds with P0s recorded in the finding register under
`root_backed_query_profile`; several P1s remain open and unrepaired).
The AT-MW-02 entity-read surface (`crates/sley-query/src/entity_read.rs`,
`crates/sley-repo/src/entity_read.rs`, `conformance/entity-read/v2`) is
reviewed under REQ-04/REQ-05 against `docs/spec/ENTITY_READ_PROFILE_V2.md`;
its verdicts are recorded on entity-read-scoped fields and never supersede
the root-query lane dispositions. Sessions and verdicts are recorded here
when they land.

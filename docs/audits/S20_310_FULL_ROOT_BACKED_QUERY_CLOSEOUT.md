# S20-310 Full Root-Backed Query Closeout

Status: **implemented under the draft Root-Backed Query Profile v1 contract (revision 1); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

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

- Contract draft revision 1 and ADR-0030 at `d7065ee`; implementation,
  corpus, oracle, and fuzz slice at `76cf2a2`.
- Conformance corpus: `conformance/root-backed-query/v1/accepted.json`
  (twenty-three vectors: all nineteen classes over the frozen S20-250
  fixture with the S20-300 snapshot, root, epoch, workspace, synthetic
  bindings, fingerprints, and roots, plus two-page continuation walks over
  entities and edges) and `rejected.json` (eight failures: truncation
  without continuation, wrong cursor type, cursor on a single-key class,
  class not applicable, unresolved entity, noncanonical filter, depth cut,
  work exhausted), drift-gated by
  `scripts/generate_root_backed_query_fixtures.py --check` in `make quick`;
  `scripts/check_root_backed_query_vector.py` recomputes every
  `RootQueryId` and response record from the S20-250 fixture bodies, its
  frozen edge set, and the fixture context with its own encoder, work
  accounting, paging, and precedence, registered in `make conformance`
  under the frozen oracle environment.
- Native tests: three `sley-query` root-query tests (every class over the
  fixture with 128-run determinism; continuation walks over entities and
  edges that union to the complete result with exact totals, plus the
  omitted, invalid-cursor, depth-cut, and single-key failures; the
  applicability, resolution, shape, limit, binding, arm-1, foreign-request,
  and drift matrices with the eleven-code table), one `sley-repo` test
  (rebuild then cache hit answering byte-identical records, a by-kind and
  entity walk against the verified objects, and the cached edges served
  with the object store removed); `sley-query` 57 tests, `sley-repo` 344
  tests, `sley-id` 7 tests pass.
- Persistent fuzz: `fuzz/targets/root_query_engine.rs` (structured root
  plus typed query, limits, flag, and cursor; determinism, record length,
  count, truncation, and continuation-walk invariants), smoke `PASS` over a
  971-seed corpus (`docs/audits/S20_700_ROOT_QUERY_PERSISTENT_SLICE.md`);
  seventeen scoped targets and sixteen smoke gates now stand; the S20-700
  finding register and independent review remain deferred.
- Tier 1: `make quick` green at every commit.
- Tier 2: see the validation record below.

## Findings closed in flight

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
- Work accounting per class is the implementation's interpretation of the
  contract's charging rule and is part of the frozen record; a review may
  tighten the rule and the vectors then refresh.
- No consumer of a root-backed response exists yet; the full S20-320
  capsule wraps it next.
- Strict pedantic clippy debt in older `sley-repo` exchange and GC test
  modules is pre-existing; the new paths lint clean under `--no-deps`.

## Validation record

Tier 1 `make quick` passed at every commit of the slice. Tier 2 ran on
2026-09-03 at `76cf2a2`: `make core` (945 tests), `make conformance`
(including the root-backed query oracle line), `make adversarial`,
`make fuzz-smoke`, and `make root-query-persistent-fuzz-smoke` all exited 0
in 38 seconds; the per-gate record is in
`machineresearch/sley-2.0/s20-310-full-root-backed-query-campaign-2026-09-03.md`.
The full `make v1` gate was skipped
because this is a subsystem handoff, not a release boundary; `make v2` and
`make release-check` remain intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

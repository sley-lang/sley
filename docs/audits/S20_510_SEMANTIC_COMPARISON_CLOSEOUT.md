# S20-510 Semantic Comparison Closeout

Status: **implemented under the draft Semantic Comparison v1 contract (revision 1); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus repository-focused Tier 2 handoff**

## Claim under review

Two complete roots of one workspace and schema epoch compare to one
canonical semantic-delta record whose five sections realize every delta
class the Repository Model names: entity deltas (five change classes), field
deltas per changed SSMC1 field, body deltas from the restricted `Function`
fingerprint, relation deltas as the symmetric difference of the two
complete-root indexes, and root-set plus collateral deltas from the record
facts and the restricted transitive impact. The contract is
`docs/spec/SEMANTIC_COMPARISON_V1.md` with ADR-0027. It is a draft: every
Council lane was unavailable when it was written and when the implementation
landed (see the S20-250 and S20-510 campaign records), so the reviews that
freeze it and complete the package are pending and must pass before the
status above changes.

The implementation provides:

- `sley-id`: the thirty-first domain `sley2.semantic-delta.v1` and
  `SemanticDeltaId` with its frozen vector;
- `sley-repo`: `compare.rs` with `compare_complete_roots` over two
  `CompleteRootRequest` values (both judged by the S20-250 full closure
  first), `encode_semantic_delta`, `decode_semantic_delta`, the delta epoch
  record (tag 510, digest domain 20), and the eleven `COMPARE_*` codes
  51000 through 51010 with wrapped `SCB_*`, `IMPACT_*`, and `FINGERPRINT_*`
  codes preserved; `CompleteRootRequest::from_parts` for callers that hold a
  complete root outside a repository.

`sley-query`, `sley-ssmc`, and the S20-250 profiles are unchanged; the
dependency direction `sley-repo -> sley-query -> sley-check -> sley-ssmc`
holds, and `sley-ssmc` is now a production dependency of `sley-repo`.

## Evidence

- Contract draft revision 1 at `fee3bd2` (checker registered at `df4e555`);
  implementation at `6ecfe89`.
- Conformance corpus: `conformance/semantic-comparison/v1/accepted.json`
  (nine root pairs: identical, every change class, type members added,
  removed, changed, and reordered, signature, body-only through an owned
  block, call/effect/capability/contract/test relations, entry-point and
  dependency-root sets, and a collateral case a naive entity comparison
  misses) and `rejected.json` (five stored-byte mutations with their
  codes), drift-gated by
  `scripts/generate_semantic_comparison_fixtures.py --check` in
  `make quick`; `scripts/check_semantic_comparison_vector.py` re-derives
  every section, re-encodes the canonical bytes, and re-derives
  `SemanticDeltaId` under the frozen oracle environment, registered in
  `make conformance`.
- Native tests: twelve `sley-repo` comparison tests (empty delta and stable
  identity over 128 runs, every change class, type members, signature plus
  body, body-only, relation kinds, root sets, collateral, swapped
  direction, preconditions with wrapped codes, the decoder rejection matrix
  reaching every frozen code, and the repository-backed path through the
  extraction adapter); `sley-repo` 332 tests and `sley-id` 7 tests pass.
- Persistent fuzz: `fuzz/targets/semantic_delta_decoder.rs`, smoke `PASS`
  over a 322-seed two-lane corpus
  (`docs/audits/S20_700_SEMANTIC_DELTA_PERSISTENT_SLICE.md`); S20-700
  scoped counts are fourteen targets, thirteen smoke gates, fifteen landed
  surfaces.
- Tier 1: `make quick` green at every commit.
- Tier 2: see the validation record below.

## Findings closed in flight

- The Python oracle initially sliced the delta epoch at a one-byte contract
  tag offset; the tag `510` is a two-byte varint, so the epoch starts at
  byte 11. The oracle now documents the offset.
- Retargeting a contract onto its own predicate adds no relation, because
  the `(contract, predicate, Contract)` edge already exists; the corpus
  retargets onto a capability requirement instead.

## Explicitly open and deferred

- **Council reviews.** Ariadne contract review, Nabu architecture review,
  and Vulcan surface review are queued behind the S20-250 reviews in the
  session retry loop and land as contract revisions.
- The oracle decides body-delta membership from a slot-normalized inventory
  projection and takes the restricted fingerprint bytes from the corpus; a
  Python reproduction of the S20-250 `Function` fingerprint remains outside
  this package.
- Block-level CFG alignment, rename and move detection, and cross-epoch
  comparison are excluded by the contract.
- Strict pedantic clippy debt in older `sley-repo` test modules is
  pre-existing; the new paths lint clean under `--no-deps`.

## Validation record

Tier 1 `make quick` passed at `fee3bd2`, `df4e555`, and `6ecfe89`. Tier 2
ran on 2026-09-03 at `6ecfe89`: `make core` (926 tests),
`make conformance` (including the new oracle line), `make adversarial`,
`make fuzz-smoke`, and `make semantic-delta-persistent-fuzz-smoke` all
exited 0 in 32 seconds. The full `make v1` gate was skipped because this is
a subsystem handoff, not a release boundary; `make v2` and
`make release-check` remain intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

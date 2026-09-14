# S20-510 Semantic Comparison Closeout

Status: **implemented under the draft Semantic Comparison v1 contract (revision 3); the three Council review rounds landed 2026-09-04 with four freeze-blocking findings, all closed by revision 2; revision 3 closes every remaining report-grade finding below; the package awaits re-review, so it is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03; revised 2026-09-14 (revision 3)

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

## Revision 2 (2026-09-05)

The three Council review rounds landed 2026-09-04 with four freeze-blocking
findings, all closed by contract revision 2 with a small implementation
change; the corpus bytes are unchanged (nine pairs, five mutations):

- the owned inventory is exactly the forward closure the frozen S20-250
  fingerprint walks, and the body-delta counts are the lengths of the
  fingerprint input vectors (`crates/sley-repo/src/compare.rs`,
  `owned_inventory`); disagreement fails `COMPARE_INVENTORY_INVALID` with
  the exact `FINGERPRINT_*` code preserved (Ariadne P0);
- the collateral seed rule seeds every entity carrying a body delta into both
  seed sets, and collateral is the closure members bound by both roots that
  carry no entity delta (Nabu P0-1);
- the derivation body (`## Change classes` through `## Stable failures`) is
  pinned by `derivation_semantics_hash`, recomputed by the stage checker
  (Nabu P0-2);
- the delta schema epoch is pinned as `delta_schema_epoch`, asserted by the
  stage checker and the fixture oracle and by a native test (Nabu P0-3).

Lower-severity findings — including the dependency-direction sentence, the
per-kind field-tag domain, the precondition-4 reach, and the work-charging
rule — remain open and are tracked in the finding register.

## Findings closed in revision 3 (2026-09-14)

- **Closed with code.** The closed section-2 field grammar is enforced by
  the decoder (`valid_field_grammar`: per-kind rows, TypeDef-2/Function-2
  flag bits, presence bit) with oracle parity and three new rejection
  mutations (off-table field, bad flags, equal-roots-nonempty; matrix now
  8); equal roots admit only the empty delta; field and root-set
  added/removed pairs must be disjoint; the public encoder honors the
  per-section counts; two dead markers removed; the fuzz target asserts
  the decoder/comparer code partition instead of the closed-enum
  tautology.
- **Closed with verified restraint.** Precondition-4 presence over all four
  linkage classes (function blocks/parameters, block parameters/operations)
  was probed and is enforced by the frozen complete-root judgment before
  any delta (`COMPARE_ROOT_INCOMPLETE` with exact `IMPACT_*` codes),
  including `Added`/`Removed` functions as judged root members — a
  compare-layer duplicate would be unreachable dead code, so none was
  shipped; the contract now states the verified two-layer enforcement.
  Post-judgment non-resource impact failures keep their
  `COMPARE_ROOT_INCOMPLETE` wrap with exact source instead of
  re-labeling: re-labeling as `INTERNAL_INVARIANT` would drop the source
  the wrap preserves.
- **Closed with text.** The sley-ssmc dependency sentence corrected
  (contract, ADR-0027, checker marker); `from_parts` authority stated;
  resource-tier collapse rule and outer-code naming rule stated;
  well-formedness rules moved from decoder-only into the contract;
  flat-8 charging stated as the per-kind-maximum bound; oracle
  scope disclosed; CanonicalSet byte-order backstop stated; TypeDef bit
  suppression, MemberId-as-EntityId, Retyped evidence, section-2
  non-self-sufficiency, zero32, presence-bit, and optional-field rules
  stated; `INTERNAL_INVARIANT` defense-only; field-5 enforcement noted;
  edge-slice reliance named with its frozen owner; member-granularity
  exclusion recorded (campaign question 2 answered: keep v1 granularity);
  campaign questions 1 (MetadataOnly seeds: no, with reasons in contract
  section 5) and 3 (roots only, ancestry out per the Repository Model)
  recorded answered.
- **Checker.** Revision anchored to the Status header and pinned
  (`SPEC_REVISION = 3`) with the summary cross-check; completeness now
  verifies the oracle, rejected matrix, and fuzz target exist.
- **Overclaim corrected.** The decoder rejection matrix reaches the
  decoder-emittable codes; judgment and precondition codes are covered by
  native tests, and the closeout no longer claims otherwise.

## Explicitly open and deferred

- **Council reviews.** Ariadne contract review, Nabu architecture review,
  and Vulcan surface review landed 2026-09-04 as `FAIL` rounds; the four P0s
  are closed by revision 2 above, and the remaining P1/P2/P3 findings land as
  later contract revisions.
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

Landed 2026-09-04: Ariadne contract review (`FAIL`, 1 P0), Nabu architecture
review (`FAIL`, 3 P0), Vulcan surface review (`FAIL`, 0 P0). Full logs in
`machineresearch/sley-2.0/reviews/s20-510-*-2026-09-04.log`; dispositions in
the machine summary and the finding register.

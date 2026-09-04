# S20-250 Full Entity Bodies Closeout

Status: **implemented under the draft Complete Entity Model and Impact Profile v1 (revision 2); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus semantics-focused Tier 2 handoff**

## Claim under review

The six SSMC1 entity bodies that the restricted S20-250 profile left outside
the semantic core now exist in one normative model, the impact request covers
all eighteen kinds, and one strictly decoded `StateRoot` record plus the
objects it binds yield an exact complete-root impact index after eleven
fail-closed closure rules. The contract is
`docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md` with ADR-0026. The contract
is a draft: every Council lane was unavailable when it was written and when
the implementation landed (see the campaign record), so the reviews that
freeze it and complete the package are recorded here as pending and must
pass before the status above changes.

The implementation provides:

- `sley-ssmc`: `WorkspaceDefinition`, `PackageDefinition`,
  `NamespaceDefinition`, `EntryPointDefinition`, `PolicyBindingDefinition`,
  `DependencyBindingDefinition`, and `EntryExposure`, which `sley-mutate`
  re-exports so the generated proposal codec is byte-identical;
- `sley-policy`: the S20-360 projection maps all eighteen bodies, and the
  public `complete_entities::project_complete_entities` exposes the owned
  definitions plus the validator's private reference graph as evidence;
- `sley-query`: `ModeledEntityKind` over tags 1 through 18 with the
  restricted-arm predicate, the six-body edge rows over the twelve frozen
  kinds, `judge_complete_root` with `CompleteRootFacts`, `CompleteRootIndex`,
  and codes 25013 through 25024, and the restricted snapshot builder failing
  closed with `INDEX_SNAPSHOT_COMPLETENESS_UNSUPPORTED` on the six kinds;
- `sley-repo`: `CompleteRootRequest::extract` and
  `judge_complete_root_revision` over a verified revision, re-checking every
  object against its binding and copying the record's `entry_points` and
  `dependency_roots` as the only root facts.

Fingerprints, `value_hash`, and the restricted profile's bytes are unchanged.
The dependency direction `sley-query -> sley-check -> sley-ssmc` is unchanged;
`sley-repo` now depends on `sley-policy` and `sley-query` in production.

## Evidence

- Contract draft revision 1 at `4701733`; revision 2 names the adapter
  crates. ADR-0026 and `scripts/check_complete_entity_impact_profile.py`
  (in `make quick`) bind the contract, ADR, work-package row, summary
  section, and code registry, and stage the implementation.
- Implementation commit `e78a1ab`.
- Conformance: `conformance/complete-entity-impact/v1/accepted.json` (one
  nineteen-entity eighteen-kind complete root with its exact direct edges,
  workspace, counts, and the transitive impact of its function) and
  `rejected.json` (nineteen mutations, each with the first-failure code and
  numeric), drift-gated by
  `scripts/generate_complete_entity_impact_fixtures.py --check` in
  `make quick`; `scripts/check_complete_entity_impact_vector.py`
  independently re-derives the edge set and every closure code in plain
  Python and is registered in `make conformance`.
- Native tests: seventeen `sley-query` complete-root tests (passing judgment
  with the six-body edges and reverse impact, 128 repeated judgments
  identical, tags and codes, canonicality, C1 through C11 as first failures,
  zero-package root, restricted snapshot fail-closed on each of the six
  kinds), four `sley-repo` adapter tests (verified-revision judgment,
  validator-graph-versus-index edge agreement, missing workspace, dependency
  root mismatch); `sley-query` 51 tests, `sley-repo` 320 tests, `sley-policy`
  and `sley-mutate` and `sley-ssmc` unchanged counts pass.
- Persistent fuzz: `fuzz/targets/complete_root_judgment.rs`, smoke `PASS`
  over a 191-seed corpus in four flag lanes
  (`docs/audits/S20_700_COMPLETE_ROOT_JUDGMENT_PERSISTENT_SLICE.md`); the
  S20-700 scoped counts are thirteen targets, twelve smoke gates, fourteen
  landed surfaces.
- Tier 1: `make quick` green at every commit.
- Tier 2: see the validation record below.

## Findings closed in flight

- The design brief proposed six new fingerprint projections; SSMC1 section 8
  requires field 4 only on GA-valid `TypeDef` and `Function`, so the six
  kinds carry no fingerprint and no preimage changed.
- The brief assumed `Package.dependencies` name packages; the frozen S20-360
  reference graph resolves them to `DependencyBinding` entities, which the
  contract adopts.
- The edge-agreement test proved the validator's private graph and the
  impact index produce the same ten edges over the repository fixture, so no
  second edge vocabulary exists.

## Explicitly open and deferred

- **Council reviews.** Ariadne contract review, Nabu architecture review,
  and Vulcan surface review are queued (`council_retry.sh` in the session
  evidence) and land as contract revisions; until all three pass the
  contract is not frozen and the package is not complete.
- The Python oracle covered the constructs the first fixture exercises
  (identity-valued fields, `Bool`/`Unit` types, a parameter-returning
  block); type-expression and constant recursion rested on the restricted
  profile's own evidence. Closed on 2026-09-03: the fixture's compact JSON
  now carries whole type expressions and whole constants, a second accepted
  vector (`recursive-type-and-constant-bodies`, 20 entities, 52 edges)
  populates every section 7.2 rule that can name an entity, and the oracle
  applies that recursion itself. Dropping one nested rule from the oracle
  makes it disagree with the implementation on both the edge set and the
  transitive impact, so the coverage is load bearing.
- The fuzz target's set grammar carried at most four members per set, and
  the corpus seeded the first four members of larger fixture namespaces.
  Closed on 2026-09-03: a set's length is now one byte below four and two
  bytes for four through twenty-four, the widest set that can name every
  entity a request carries, so every seed encodes its whole fixture set.
  `make complete-root-persistent-fuzz-smoke` passed over 191 seeds with no
  artifact.
- Strict pedantic clippy debt in older `sley-repo` test modules was
  pre-existing. Closed on 2026-09-03: the workspace configures
  `clippy::all` and `clippy::pedantic` as warnings but nothing enforced
  them, so thirty-eight had accumulated. All are fixed at the source, the
  four long functions and the one cfg-gated `self` carry a reason with
  their allow, and `make lint` now denies every warning and checks
  formatting, so the debt cannot silently return.
- Full S20-300 (complete-root snapshot), root-backed S20-310 queries, and
  S20-510 comparison remain separate packages.

## Validation record

Tier 1 `make quick` passed at `4701733` and `e78a1ab`. Tier 2 ran on
2026-09-03 at `e78a1ab`: `make core` (914 tests), `make conformance`
(including the new oracle line), `make adversarial`, `make fuzz-smoke`, and
`make complete-root-persistent-fuzz-smoke` all exited 0 in 30 seconds. The
full `make v1` gate was skipped because this is a subsystem handoff, not a
release boundary; `make v2` and `make release-check` remain intentionally
fail closed.

## Independent review

Pending. Dispatch attempts and their exact errors are recorded in
`machineresearch/sley-2.0/s20-250-full-entity-bodies-campaign-2026-09-03.md`.
Sessions and verdicts are recorded here when they land.

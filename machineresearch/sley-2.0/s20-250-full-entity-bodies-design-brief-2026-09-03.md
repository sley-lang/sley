# Full S20-250 entity bodies: design brief for the next package (2026-09-03)

Status: DESIGN BRIEF prepared by the Claude orchestrator after S20-540
closed; no contract amendment, code, or Council consult exists yet. Opening
this lane is an operator roadmap decision (see
`s20-530-aging-and-resume-2026-09-03.md`).

## Why this is the next dependency-complete work

- The local completion frontier names it: S20-510 semantic comparison and
  S20-520 merge, the two remaining M4 packages, are blocked by "the six
  absent full S20-250 entity bodies and complete-root impact semantics"
  (`docs/audits/S20_LOCAL_COMPLETION_FRONTIER.md`). S20-300's full
  complete-root index carries the same blocker.
- Every other lane's first unavailable boundary is either a later
  restricted-profile completion (S20-240, S20-360, S20-390 full), a
  protocol contract that itself waits for the root-backed S20-310, or a
  benchmark or release gate that is intentionally fail closed.

## What exists (verified 2026-09-03)

| Layer | State |
|---|---|
| Frozen schema | `docs/spec/SSMC1_EPOCH1_SCHEMA.txt` defines all 18 bodies: `WorkspaceBody(packages, root_namespace, capability_requirements, contracts, tests)`, `PackageBody(workspace, root_namespace, dependencies, exports)`, `NamespaceBody(parent, members)`, `EntryPointBody(function, exposure)`, `PolicyBindingBody(subject, requirements)`, `DependencyBindingBody(dependency_root: StateRoot, external_package, local_namespace)` |
| Mutation value model | `crates/sley-mutate/src/value_generated.rs` already carries `EntityBodyValue` for all 18 kinds with generated codecs (S20-340/S20-350), so candidates can construct the six bodies today |
| Semantic core | `crates/sley-ssmc` models kinds 4 through 15 for judgment; `crates/sley-query` `ImpactEntity` and `ImpactIndex` derive edges for those twelve kinds; kinds 1, 2, 3, 16, 17, 18 are unmodeled and `field 4` (fingerprint) must be absent on them |
| Contract | `docs/spec/FINGERPRINT_IMPACT_PROFILE_V1.md` is the restricted epoch-1 profile; it states that "a later package must add the six missing bodies and extend this profile before S20-300, S20-510, or GA may claim a complete-root impact index" |
| Frontier checker | `scripts/check_local_completion_frontier.py` fails closed if `pub struct WorkspaceDefinition` (and the other five) appear in `sley-ssmc` before a re-audit |

## Proposed shape

1. **One normative model.** Extend `sley-ssmc` with the six definitions
   under the same ownership rules as kinds 4 through 15: exact typed fields
   from the frozen schema, closed decoding, no second host model and no
   locally invented edge kinds. The mutation value model stays the proposal
   codec; the semantic core owns judgment.
2. **Relationship judgments (fail closed).** Workspace membership
   (`packages`, `root_namespace`, `capability_requirements`, `contracts`,
   `tests` all resolve to entities of the right kind and every package
   points back to its workspace); package dependency and export closure
   (dependencies are packages, exports are members of the package's
   namespace tree); namespace parentage (a single rooted tree per package,
   no cycle, members typed); entry-point exposure (`function` is a
   `Function`, `exposure` an epoch-1 `EntryExposure`); policy-subject
   binding (subject exists, requirements are capability requirements); and
   the external-root relationship of `DependencyBinding` (an edge to a
   `StateRoot` outside the current root, carried as an identity, never
   dereferenced by the semantic core).
3. **Fingerprints.** Canonical projections for the six kinds under the
   existing `sley2.semantic-fingerprint.v1` domain, excluding own identity
   and presentation order exactly as the profile's section 1 does; the
   profile's "field 4 must be absent" rule flips to "field 4 required" for
   these kinds in a new profile revision, which is a schema-epoch-visible
   change and therefore needs its own frozen vectors.
4. **Complete-root impact.** Extend `ImpactKind` and `ImpactIndex` with the
   new edge kinds (workspace-package, package-namespace, namespace-member,
   entry-function, policy-subject, dependency-external-root) and define
   complete-root extraction: every entity bound by a `StateRoot` is
   inventoried, unmodeled kinds no longer exist, and the index is exact over
   the whole root. Cross-root edges from `DependencyBinding` are recorded as
   external and never followed.
5. **Contracts and checkers.** A `FINGERPRINT_IMPACT_PROFILE_V2.md` (or a
   versioned amendment of v1), an ADR for the entity-body ownership and the
   external-root rule, new stable codes appended after `25012`, an updated
   `check_fingerprint_impact_profile.py`, frozen fixtures with an
   independent Python reproduction of the six fingerprints, and the frontier
   re-audit that removes the `pub struct WorkspaceDefinition` fail-closed
   marker.
6. **Consumers unblocked.** S20-300 (complete-root index) and S20-510
   (semantic comparison over all canonical entity classes) become
   dependency-complete; S20-520 follows S20-510.

## Risks

- Ambiguous relationship semantics (for example whether `exports` must be
  transitive members) must be frozen in the contract before code; Ariadne's
  exactness standard applies to prose.
- Fingerprint projections for container kinds must exclude member order
  unless the schema freezes it as semantic (sets are canonical, so order
  is not semantic; parent links are).
- `DependencyBinding.dependency_root` is the first cross-root reference in
  the semantic core; the contract must say it is an identity, that impact
  treats it as an external leaf, and that no root is loaded to judge it.

## Questions for a Nabu consult before freezing

1. Whether the six bodies land in `sley-ssmc` (single normative model, as
   the frontier requires) with the generated mutation value codec kept as
   the proposal-side mirror, or whether the generated model becomes the
   single source with the semantic core importing it.
2. The exact relationship judgments that are S20-250 (structural, per root)
   versus S20-510 (comparison across roots) versus policy (S20-370).
3. Whether the external-root edge belongs in the impact index at all or in
   a separate cross-root ledger owned by the repository lane.
4. The profile versioning: amend v1 in place with a new epoch-visible
   revision, or freeze v2 alongside v1 for the restricted consumers that
   already bind v1's digests.

## Estimated shape of the campaign

Contract freeze with Ariadne and Nabu (two to four review passes, as
S20-540 needed), implementation in `sley-ssmc` and `sley-query` with
fixtures and oracle, S20-700 fuzz coverage for the new decoders, Tier 2,
and three implementation reviews. Comparable to S20-540 in size.

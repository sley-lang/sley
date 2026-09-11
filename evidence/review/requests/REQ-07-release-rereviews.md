# Review queue REQ-07 — release-area re-reviews after evidence repairs (staged)

Baseline: current `main` HEAD at dispatch time (verify `git rev-parse HEAD`
before starting).

These sections carry prior-round FAILs whose repairs have landed since,
so re-review is meaningful (dependencies satisfied):

1. `release_candidate_packaging` (ariadne/nabu/vulcan `_contract_review`):
   candidate rebuilt REPRODUCIBLE through the smoke mechanics, SBOM and
   provenance regenerated over it, T52/T54 refreshed. Review the current
   packaging surface against `RELEASE_CANDIDATE_PACKAGING_V1.md`.
2. `standards_sbom_and_provenance` (ariadne/nabu/vulcan `_contract_review`):
   SBOM pair plus unsigned provenance regenerated; provenance predicate now
   carries the truthful build invocation and cleanliness flag, checker
   cross-checks subject against the attested artifact (fail-closed).
   Review against `STANDARDS_SBOM_AND_PROVENANCE_V1.md`.
3. `mutation_schema` (`nabu_architecture_review` REVISE) and
   `protected_policy_root` (`nabu_architecture_review` REVISE) and
   `s20_610_offline_raw_runner` (`nabu_review` REVISE): re-review ONLY if
   the prescribed repairs have landed; otherwise leave pending.

Sections with FAIL rounds and no intervening repair (complete_root_index_snapshot,
context_capsule, decision_dossier, epoch_migration_policy, json_bridge, merge,
required_contract_index, semantic_comparison, succession_accounting,
s20_700_remaining_surface_audit, root_backed_query_profile root-query lanes,
finding_register ariadne/nabu lanes) are NOT yet dispatchable: re-review
without repair would return the same verdicts. They await owner-lane repair
cycles.

Read-only; verdicts in the REQ format; prior verdicts preserved (revision
fields for new rounds on FAIL bases, never overwrites).

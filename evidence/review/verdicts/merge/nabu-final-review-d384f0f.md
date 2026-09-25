# Final review — S20-520 merge (Nabu architecture role)

Date: 2026-09-14. Scope: P-C wave through d384f0f; repair 9806747 (rev-5).
Prior: 09-04 FAIL (3 P0: AddEntryPoint-as-create, fingerprint None, label asymmetry; 5 P1).
Reviewed: merge.rs judgment/plan/commit paths, rev-5 contract, corpus, fuzz lanes.

Findings (all prior verified closed):
- P0 AddEntryPoint-as-create / fingerprint None / label asymmetry: CLOSED (fail-closed entry-point bound; plan objects carry A's label+fingerprint; ours-label + metadata_overridden; symmetry section scoped per-side).
- P1 precondition/theirs_object/collateral-scope/MergeSide-leak (bounded: struct still pub, production path forced through judge_merge_verified + trust note)/retyped-dependent: CLOSED/BOUNDED.
- P2/P3: CLOSED/BOUNDED (grants bound; typed CommitFailure + numerics preserved; derived-collision fail-closed).
New: NONE. Collateral kind stays base-only without conflict (collateral sources always base-present; tiebreak is no-base-case only); decoder kind 1..=18 accepts tiebreak output.
Verdict: PASS — PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4.
Role: harness final (council lane down).

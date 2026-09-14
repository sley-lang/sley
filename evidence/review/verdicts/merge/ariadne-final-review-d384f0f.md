# Final review — S20-520 merge (Ariadne contract role)

Date: 2026-09-14. Scope: P-C wave through d384f0f; repair 9806747 (rev-5: kind-divergent AddAdd lesser-tag tiebreak + swap test + allocation row).
Prior: 09-04 FAIL (3 P0: kind-16 unexecutable, ordinal contradiction, theirs-collateral; 5 P1; rev-4 closed most).
Reviewed: MERGE_V1.md rev-5, crates/sley-repo/src/merge.rs, corpus, oracle, checker.

Findings (all prior verified closed):
- P0 kind-16 unexecutable: CLOSED (fail-closed PlanUnsupported + recovery).
- P0 ordinal contradiction: CLOSED (derivation in judged raw-ID order; creates-first in derivation order).
- P0 theirs-collateral theirs_object: CLOSED (reads A/B directly, no other_side).
- P1 both-Removed/52001/collateral-on-conflicted/anchor-closure/corpus: CLOSED/BOUNDED (closeout disclosures stand).
- P2/P3: CLOSED/BOUNDED (AddAdd kind pinned rev-5; allocation row named; dedup key symmetric).
New: NONE. Tiebreak verified strictly swap-symmetric (.min() over present side-kinds); rev-5 text matches code exactly; swap test asserts kind=3 both directions.
Verdict: PASS — PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4.
Role: harness final (council lane down).

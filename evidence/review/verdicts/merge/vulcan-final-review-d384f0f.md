# Final review — S20-520 merge (Vulcan surface role)

Date: 2026-09-14. Scope: P-C wave through d384f0f; repair 9806747 (rev-5).
Prior: 09-04 FAIL (1 P0 caller-chosen O; 5 P1; fuzz-evidence bounded).
Reviewed: server merge paths, judgment guards, decoder pins, corpus.

Findings (all prior verified closed):
- P0 caller-chosen O: CLOSED (verified path; bound + charged; caps).
- P1 theirs-collateral/partial-remap/commit-before-CAS/52001/fuzz-evidence: CLOSED/BOUNDED (judgment fuzz lane via natives/vectors, closeout-disclosed).
- P2/P3: CLOSED/BOUNDED (decoder detail/kind/field pins; empty-plan pairing; ancestry/work bounds; Changed/Changed-no-fields invariant).
New: NONE.
Verdict: PASS — PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4.
Role: harness final (council lane down).

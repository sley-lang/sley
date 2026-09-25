# Final review — S20-750 decision dossier (Nabu architecture role)

Date: 2026-09-14. Scope: P-C wave through d384f0f; repair 3001447 (rev-6).
Prior: 09-04 FAIL (2 P0 + P1s).
Reviewed: derive_decision entries-first, required(), all-null rule, S3.2/S3.4 ordering, revision binding, evidence lists, summary cycle.

Findings (all prior verified closed):
- P0 entries-ignored/shadowing: CLOSED (missing()/evidenced_value() + gated-contributes-BLOCKED; separate inventories).
- P1 missing-key/nested-null/S3.2/S3.4/revision/evidence-lists/summary-cycle: CLOSED/BOUNDED (required() 76001; all-null→GATED-no-value + test; FAILED→FAIL vs non-OPEN→BLOCKED + test; ordered P0/P1→P2-approval→gate-FAIL→thresholds→CONDITIONAL + tests; SPEC_REVISION anchor; existence+divergence+drift bounds; entries-digest + non-input declaration, write-back retained).
New: NONE. open_p2_rows matches register shape; live declared p2=0 so path dormant; read_gate_status fail-closed on exception.
Verdict: PASS — PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4.
Role: harness final (council lane down).

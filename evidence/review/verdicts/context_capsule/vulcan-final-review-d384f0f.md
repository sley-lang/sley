# Final review — S20-320 context capsule (Vulcan surface role)

Date: 2026-09-14. Scope: P-C wave through d384f0f; repair 1edb5e7 (rev-4).
Prior: 09-04 FAIL (P0 session-mintable, P0 preimage, P1s 32009/32010/32011/matrix/Negotiated).
Reviewed: contract rev-4, code failure paths, rejection matrix, tests.

Findings (all prior verified closed):
- P0 session-mintable: CLOSED (authority path takes no provenance).
- P0 preimage: CLOSED.
- P1 32011: CLOSED (kinds!=ids → InternalInvariant; spec table).
- P1 From<QueryError> 32010: BOUNDED (fires only on builder-internal encode/work; engine failures keep QUERY_* pre-construction; matches preflight bucket).
- P1 32009: BOUNDED (defense-in-depth + inspection coverage).
- P1 matrix/Negotiated: CLOSED/BOUNDED (swap executed, drift at authority, restricted type-excluded, Negotiated covered).
- P2 (Complete-builder, table ceilings, work-before-alloc, kind-0/marker) + P3 (rev drift, SOURCE_INVALID magic/length): CLOSED/BOUNDED.
New: NONE (single pre-existing cosmetic nit, no severity).
Verdict: PASS — PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4.
Role: harness final (council lane down).

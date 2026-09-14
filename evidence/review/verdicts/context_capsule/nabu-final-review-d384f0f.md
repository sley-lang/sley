# Final review — S20-320 context capsule (Nabu architecture role)

Date: 2026-09-14. Scope: P-C wave through d384f0f; repair 1edb5e7 (rev-4).
Prior: 09-04 FAIL (P0 preimage, P0 rev-2-vs-authority drift, P0 Negotiated evidence).
Reviewed: contract rev-4, implementation, checker (rev cross-check), oracle, root_query accessor.

Findings (all prior verified closed):
- P0 preimage: CLOSED (spec framing + oracle u64 length prefix).
- P0 rev-2-vs-authority drift: CLOSED (ADR/closeout/spec rev-4 + checker binding).
- P0 Negotiated evidence: CLOSED (fixture-session vector + oracle binding).
- P1 caller-supplied session: CLOSED (authority-only mint; no-provenance path).
- P1 §2/§11 + walk predicate + entities-membership: CLOSED (spec tuple/chaining/sum rule, non-claim).
- P1 kind-0 + Complete/Page window + restricted row + oracle-from-§6 + subject accessor + checker gaps: CLOSED.
- P2/P3: CLOSED (returned-counts, rev-1 lines, truncated-from-next_after bounded).
New: NONE. Riskiest re-checked: work-order prose vs gates-after-derivation (bounded by source ceiling, inspection-only).
Verdict: PASS — PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4.
Role: harness final (council lane down).

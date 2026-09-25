# Final review — S20-420 JSON bridge (Vulcan surface role)

Date: 2026-09-14. Scope: P-C wave through d384f0f; repairs 7718391 + d384f0f (hello-version declaration + test).
Prior: 09-04 FAIL; re-review FLAGGED hello-protocol_version declaration/vector gap (behavior fixed).
Reviewed: hello header rule, decoder, oracle, tests, fuzz lanes.

Findings:
- P0 precedence/widths: CLOSED.
- P1 hello-version: now CLOSED (behavior was codec-rejected; declaration + PROTOCOL-only test added in d384f0f).
- P1 dup-keys/allocation/fuzz-split: CLOSED (declared + tested; Frame-level dup via object collapse).
- P2/P3: CLOSED/BOUNDED (4× exact; emitter rule; canonical render).
New: NONE.
Verdict: PASS — PASS_PRIOR_P0_P1_P2_P3_CLOSED_NO_NEW_P0_P1_P2_P3_P4.
Role: harness final (council lane down).

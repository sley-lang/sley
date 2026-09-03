# S20-740 finding register campaign (2026-09-03)

Package: the finding register half of S20-740 (independent semantic and
security review), dependency S20-730, phase M6. Owner of record Vulcan;
the register mechanics were executed by the integrator because every Council
lane was unavailable (ADR-0026). The independent review itself is not done and
is not claimed: the register reads `FINDING_REGISTER_OPEN`, and
`make release-check` and `make v2` stay fail-closed.

## Contract

- `docs/spec/FINDING_REGISTER_V1.md`, draft revision 1, with
  `docs/adr/ADR-0042-finding-register-derived-from-recorded-dispositions.md`.
- Staged checker `scripts/check_finding_register.py` in `make quick`; summary
  section `finding_register`; work package row updated; error codes 75000
  through 75003 reserved in `docs/spec/ERROR_CODES_V1.md`.

## Mechanics

| Surface | File | What it does |
|---|---|---|
| Register | `scripts/build_finding_register.py` | derives `evidence/review/finding-register.json` from the machine summary: every review obligation, its state, the severities its disposition names, the completed packages, and the completion invariant |
| Tests | `bench/review/tests/test_finding_register.py` | 13 offline tests: exact state classification, obligation filtering, list agreement, the completion invariant and its historical-round exception, fixed-point stability under counter updates, purity, and the three fail-closed inputs |

## Result at this commit

- 181 review obligations across the summary: 84 `PASS`, 60 `PENDING`, 12
  `DEFERRED` (the Forge OAuth 401 lane outage), 24 `HISTORICAL_ROUND`
  (superseded rounds of packages that later passed), 1 `OTHER`
  (`mutation_value_profile.merlin_review` recorded a timed-out handoff that
  Codex integrated).
- Severity mentions across dispositions: 29 P0, 40 P1, 42 P2, 19 P3, 15 P4,
  all of them inside dispositions that also record closure or supersession.
- 24 packages carry a `COMPLETE` status, and none of them carries an open
  review: the completion invariant the release gates assume is now enforced by
  the builder rather than by hand.
- Result `FINDING_REGISTER_OPEN`, which is the honest state while the Council
  lanes are down.

## Design note

The register digest covers the derived obligation list rather than the summary
bytes. The summary records this register's own counts, so a byte digest would
never reach a fixed point: writing the counts would change the digest, which
would change the register, which would change the counts. A test pins that
stability directly.

## Open questions for the Council

1. Ariadne: should `HISTORICAL_ROUND` obligations be allowed to remain in a
   completed package's record indefinitely, or should they move to a
   superseded-rounds list once the package passes?
2. Nabu: is the machine summary the right single source, or should campaign
   records also be parsed for findings that never reached the summary?
3. Vulcan: does the register carry enough for an independent review to start,
   or does S20-740 need per-finding text (title, severity, disposition,
   closing commit) rather than per-obligation dispositions?

## Validation

Recorded below after Tier 1 and Tier 2.

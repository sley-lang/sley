# S20-630 Succession Accounting Closeout

Status: **implemented under the draft Succession Accounting v1 contract (revision 2); Council reviews pending, so the package is not complete; no trial exists and no accounting number is evidence; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus benchmark-focused Tier 2 handoff**

## Claim under review

Succession accounting is an exact derivation from immutable evidence. The
report reads one run's create-once manifest and each arm's claim chain,
verified by that arm's own runner, and nothing else. Every claim is an
attempt whatever its status, so timeouts and harness failures stay in every
denominator; every quantity is an integer or a reduced ratio, medians are
exact, and a zero denominator is a named null. Accepted Change Tokens are
total observable model tokens over accepted correct changes, exactly as the
master goal defines them and never as a ranking on their own. The plan's
section 22 thresholds are evaluated only between complete arms and are
otherwise `UNDETERMINED` with the arms' statuses as the reason. The report
inherits the claims' unverified status, so the dossier's succession fields
stay null. The contract is `docs/spec/SUCCESSION_ACCOUNTING_V1.md` with
ADR-0037; it is a draft written and implemented while every Council lane
was unavailable, so the Ariadne, Nabu, and Vulcan reviews that freeze it
and complete the package are pending and must pass before the status above
changes.

The implementation provides:

- `bench/accounting/report.py`: `AccountingErrorCode` with the eight codes
  63000 through 63007, `ratio`, `median`, `arm_accounting`,
  `evaluate_thresholds` over the plan's ten threshold names,
  `derive_report`, `report_digest`, `verify_report`, `write_report`, and
  the `derive` and `smoke` commands;
- `bench/accounting/tests/test_report.py`: four offline tests;
- `make accounting-smoke`: the report over the S20-620 scripted smoke run,
  retained under `evidence/runtime/s20-630-accounting-smoke/`;
- `scripts/check_succession_accounting.py` in `make quick`: contract
  markers, module markers and forbidden surfaces (no float construction,
  no subprocess, no clock), the offline tests, and the dossier's succession
  fields still null.

## Evidence

- Contract draft revision 1, ADR-0037, and the stage checker at
  `539c0f9`; revision 2 and the implementation in the commit recorded in
  the campaign record.
- Offline tests (four, all pass): ratios and medians are exact over odd
  and even counts with zero denominators as nulls and a float refused;
  every attempt stays in the denominator across accepted, rejected,
  timeout, and harness-failure claims with exact sums, medians, per-class
  correctness, and the ACT null reason; thresholds pass on a better
  synthetic arm, fail on a worse one, pass the equal-correctness ACT
  clause, and read `UNDETERMINED` on partial and absent arms; a report over
  real raw and Sley 2 chains built through the S20-610 and S20-620 append
  functions is exact, digested, repeatable, refuses a complete-report
  demand, and fails closed on a tampered report, a tampered chain, and a
  missing run.
- Accounting smoke over the S20-620 run: status `PARTIAL`, the Sley 2 arm
  with two attempts (one rejected, one harness failure), no accepted change
  and a null ACT with reason, the raw and legacy arms `NO_CLAIM_CHAIN`, and
  every threshold `UNDETERMINED`; evidence PASS.
- `make quick` green at the commit. Tier 2: see the validation record
  below.

## Findings closed in flight

- The first accepted-change increase used a sign trick through the
  reduction helper and read an 8-versus-10 decrease as an increase; the
  offline threshold test caught it and the increase is now computed
  directly.
- Revision 2 of the contract records the clarifications the
  implementation forced: the ACT null reason, the smoke reading `PARTIAL`,
  the accepted-change increase definition, the regression-cap result, the
  per-arm chain loaders, and exact rational arithmetic.

## Explicitly open and deferred

- **Council reviews.** Ariadne, Nabu, and Vulcan reviews land as contract
  revisions; the campaign record lists the open questions (critical
  classes, the accepted-change denominator, per-seed accounting).
- The legacy arm has no claim chain producer (S20-600 stops at the version
  smoke), so no threshold can leave `UNDETERMINED` until it does.
- No trial has run; the dossier's succession fields and every number in
  the smoke report are mechanics, not evidence.

## Validation record

Tier 1 `make quick` passed at the commit. Tier 2 is recorded in
`machineresearch/sley-2.0/s20-630-succession-accounting-campaign-2026-09-03.md`.
The full `make v1` gate was skipped because this is a subsystem handoff,
not a release boundary; `make v2` and `make release-check` remain
intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

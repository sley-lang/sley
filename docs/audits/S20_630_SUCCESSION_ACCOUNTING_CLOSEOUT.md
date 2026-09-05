# S20-630 Succession Accounting Closeout

Status: **implemented under the draft Succession Accounting v1 contract (revision 3); the Ariadne, Nabu, and Vulcan reviews returned FAIL against revision 2 and every P0 and every P1 lands in this revision; the package is not complete until those lanes re-review; no trial exists and no accounting number is evidence; the Sley 2 goal remains incomplete**

Date: 2026-09-05

Validation tier: **Tier 1 plus benchmark-focused Tier 2 handoff**

## Claim under review

Succession accounting is an exact derivation from immutable evidence. The
report reads one run's create-once manifest and each required arm's claim
chain, verified by that arm's own runner through one registry, and
nothing else. Every claim is an attempt whatever its status, so timeouts
and harness failures stay in every denominator; every quantity is an
integer or a reduced ratio, medians are exact, and a zero denominator is
a named null. Accepted Change Tokens are total observable model tokens
over accepted correct changes, exactly as the master goal defines them
and never as a ranking on their own. The plan's section 22 thresholds are
evaluated only between complete arms and are otherwise `UNDETERMINED`
with the arms' statuses as the reason; the three section 22 conditions
the plan does not encode travel as `NOT_EVALUATED` rows with their
owners, so no all-`PASS` report can be read as section 22 satisfied. The
report derives its evidence status from the claims' own statuses and
re-derives it on verify. The dossier's succession fields stay null. The
contract is `docs/spec/SUCCESSION_ACCOUNTING_V1.md` with ADR-0037.

The implementation provides:

- `bench/accounting/report.py`: `AccountingErrorCode` with the eight codes
  63000 through 63007, `ratio`, `median`, `derive_evidence_status`,
  `arm_accounting` (all 25 plan metrics, median companions, per-seed and
  per-class groupings, claim statuses, derived every-attempt flag,
  duplicate-slot refusal), `evaluate_thresholds` over the plan's ten
  threshold names plus three `NOT_EVALUATED` rows with structural
  coverage, `derive_report`, `report_digest`, `verify_report`,
  `write_report`, and the `derive` and `smoke` commands;
- `bench/accounting/tests/test_report.py`: eight offline tests;
- `make accounting-smoke`: the report over the S20-620 scripted smoke run,
  regenerable runtime evidence (digest below);
- `scripts/check_succession_accounting.py` in `make quick`: contract
  markers, module markers and forbidden surfaces (no float construction,
  no subprocess, no clock), the offline tests, and the dossier's succession
  fields still null.

## Evidence

- Contract draft revision 1, ADR-0037, and the stage checker at
  `539c0f9`; revision 2 and the implementation at `33d90af`; revision 3
  (this slice) closing every review finding.
- Offline tests (eight, all pass): ratios and medians are exact over odd
  and even counts with zero denominators as nulls and a float refused;
  every attempt stays in the denominator across accepted, rejected,
  timeout, and harness-failure claims with exact sums, medians, the
  non-harness-failure companions, per-class correctness with collateral
  sums, per-seed grouping, the ACT null reason, duplicate-slot and
  pre-derived-ACT refusal; thresholds pass on a better real arm output,
  fail on a worse one, pass the equal-correctness ACT clause at equal
  medians, and read `UNDETERMINED` on partial and absent arms, on
  undefined regressions, and on empty denominators; the cap row measures
  both regressions and passes beside a failed threshold when nothing
  regressed; coverage, criticality-flag, per-arm-budget, legacy
  reserved-path, and evidence-mismatch closures all fail closed; a
  complete report through `derive_report` over two full chains verifies
  with `COMPLETE`, a `PASS` context row, and per-row evidence status; a
  report over real raw and Sley 2 chains built through the S20-610 and
  S20-620 append functions is exact, digested, repeatable, refuses a
  complete-report demand, and fails closed on a tampered report, a
  tampered chain, and a missing run.
- Accounting smoke over the S20-620 run: status `PARTIAL`, the Sley 2 arm
  with two attempts (one rejected, one harness failure), no accepted change
  and a null ACT with reason, the raw and legacy arms `NO_CLAIM_CHAIN`, and
  every evaluated threshold `UNDETERMINED` beside three `NOT_EVALUATED`
  rows; scope
  `REPORT_OVER_THE_S20_620_SCRIPTED_SMOKE_RUN_PARTIAL_2_SCRIPTED_ATTEMPTS`
  derived from the report itself; report digest
  `614212d22568a68c178da81b6bb70df15ba07cb3a9f19d27bea443b5ca9b6b67`;
  evidence PASS.
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
- Revision 3 closes the Council reviews. Vulcan's P0 (the report claimed
  section 22 while three conditions went unevaluated) is now three
  `NOT_EVALUATED` rows with owners and a narrowed boundary sentence.
  Ariadne's P0s (the cap row mirrored the threshold into a false `FAIL`;
  the legacy arm frozen as absent) are now measured regressions with
  named-null undefined cases, and a verifier registry with a reserved
  path that fails a premature chain closed. Nabu's P0s (seven dropped
  metrics; a constant evidence status; class shape that cannot express
  section 22.4; no per-seed accounting) are now full metric coverage
  with latency and memory medians, a status derived from the claims'
  own statuses and re-derived on verify, per-class collateral sums with
  the deferred conditions named, and per-seed grouping for S20-640.
  Every P1 lands with them: measured threshold legs, exact percent rows,
  every-attempt medians stated with companions, structural threshold
  coverage, per-arm fixture and per-row evidence status, a regenerable
  smoke with derived scope, `DERIVED` derivation output, the shared
  action budget read and recorded, and threshold evaluation over real
  arm outputs end to end.
- Two review prescriptions were narrowed in flight with reasons in the
  contract: corpus-declared critical classes would edit S20-610's frozen
  input, so the plan flag is the control accounting reads and
  all-classes is recorded; the section 22.2 documented-reason override
  has no input to carry it, so a reasoned exception needs a plan
  revision.

## Explicitly open and deferred

- **Council re-reviews.** Ariadne, Nabu, and Vulcan re-review revision 3;
  the FAIL verdicts above stand until those lanes pass.
- The legacy arm has no claim chain producer (S20-600 stops at the version
  smoke), so no threshold can leave `UNDETERMINED` except over synthetic
  arms, and the contract states that the PASS/FAIL paths are exercised by
  synthetic tests only; `S20_630_COMPLETE` must not claim
  threshold-evaluation capability until a real legacy chain exists.
- No trial has run; the dossier's succession fields and every number in
  the smoke report are mechanics, not evidence.

## Validation record

Tier 1 `make quick` passed at the commit. Tier 2 ran on 2026-09-05 at the
commit below (`make core`, `make conformance`, `make adversarial`,
`make fuzz-smoke`, and `make accounting-smoke`, all exit 0) and is
recorded in
`machineresearch/sley-2.0/s20-630-succession-accounting-campaign-2026-09-03.md`.
`make sley2-runner-smoke` FAILS identically with and without this slice
(bisected: PASS at `9164ba3`, FAIL at `ed7fe87`): S20-330's per-instance
server nonce makes pre-restart handshake names unknown by design, while
the S20-620 smoke reuses a probe `handshake_id` in a later `serve`
invocation, so `session.open` is refused and the scripted trial dies
with `SLEY2_TRIAL_HANDSHAKE_FAILED`. That integration break belongs to
S20-330/S20-620 and is left untouched here; it is recorded, not repaired,
because a session-semantics fix from the accounting slice would be
unreviewed scope. The full `make v1` gate was skipped because this is a
subsystem handoff, not a release boundary; `make v2` and `make
release-check` remain intentionally fail closed.

## Independent review

Ariadne contract review, Nabu architecture review, and Vulcan surface
review against revision 2 (2026-09-04): FAIL with 7 P0 and 20 P1
findings, all closed in revision 3 above. Re-review of revision 3 is
pending. Sessions and verdicts are recorded here when they land.

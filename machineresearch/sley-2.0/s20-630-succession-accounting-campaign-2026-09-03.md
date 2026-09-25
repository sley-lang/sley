# S20-630 Succession Accounting Campaign (2026-09-03)

Status: contract draft revision 4 committed (revision 3 closed every
Council finding; revision 4 adds the FAIL-path end-to-end test and the
residual P2/P3 precision);
Ariadne contract review, Nabu architecture review, and Vulcan surface
review returned FAIL against revision 2 (7 P0, 20 P1) and re-review of
revision 4 is queued.

## Frontier at start

- S20-610 and S20-620 store per-arm digest-chained trial claims with the
  plan's 25 integer metrics under one create-once run manifest; floats are
  forbidden so that accounting derives ratios later.
- No trial has run: the dossier's succession fields are null and every
  arm's chain is empty or absent.
- The local completion frontier names S20-630 as the next authority-safe
  package and its guard blocks `bench/accounting` until the summary allows
  it.

## Design brief

Contract: `docs/spec/SUCCESSION_ACCOUNTING_V1.md`, ADR-0037, stage checker
`scripts/check_succession_accounting.py`.

- Inputs are the verified manifest and each arm's chain verified by that
  arm's own runner; nothing else is read.
- Exact integers and reduced ratios; medians as exact ratios; zero
  denominators are named nulls.
- Every claim is an attempt; timeouts and harness failures stay in every
  denominator; ACT is total observable tokens over accepted changes.
- Section 22 thresholds evaluate only between complete arms and are
  otherwise `UNDETERMINED`; the report inherits the claims' unverified
  status and the dossier stays null.
- Codes 63000 through 63007.

## Open questions for the reviews

- Whether every task class should count as critical for the correctness
  threshold or the corpus should name the critical classes.
- Whether "accepted correct changes per fixed action budget" should divide
  by the manifest's action budget or compare accepted counts directly under
  the shared budget.
- Whether the report should also carry per-seed accounting for S20-640.

## Records

| Stage | Commit | Tier 1 | Notes |
|---|---|---|---|
| Contract draft revision 1 | `539c0f9` | green | ADR-0037, stage checker |
| Implementation, revision 2 | `33d90af` | green | `bench/accounting`; 4 offline tests; smoke over the S20-620 run: PARTIAL, 2 attempts, no accepted change, thresholds UNDETERMINED; closeout `docs/audits/S20_630_SUCCESSION_ACCOUNTING_CLOSEOUT.md` |
| Reviews against revision 2 | n/a | n/a | Ariadne FAIL (2 P0, 8 P1, 9 P2, 8 P3), Nabu FAIL (4 P0, 7 P1, 8 P2, 4 P3), Vulcan FAIL (1 P0, 5 P1, 7 P2, 2 P3); logs under `machineresearch/sley-2.0/reviews/s20-630-*.log` |
| Contract revision 3 | this slice, see closeout | green | every P0 and every P1 closed: NOT_EVALUATED section 22 rows, measured regression cap with named-null legs, verifier registry with legacy reserved path, all 25 plan metrics, derived evidence status, per-class collateral sums, per-seed grouping, exact percent rows, stated median basis with companions, structural coverage, per-arm fixture and per-row evidence status, regenerable smoke with derived scope, DERIVED output, shared action budget read and recorded, end-to-end threshold tests on real arm outputs; 8 offline tests; closeout revision 3 |

## Tier 2 handoff record (2026-09-05, revision 3 slice)

| Gate | Result | Evidence |
|---|---|---|
| `make core` | exit 0 | 0 failures |
| `make conformance` | exit 0 | oracle results PASS |
| `make adversarial` | exit 0 | 0 failures |
| `make fuzz-smoke` | exit 0 | bounded smoke tests passed |
| `make accounting-smoke` | exit 0 | report PARTIAL over the S20-620 run; evidence PASS; stage checker PASS |
| `make sley2-runner-smoke` | FAIL, pre-existing | identical FAIL with the slice stashed; bisected to `ed7fe87` (S20-330 nonce vs smoke handshake reuse); owned by S20-330/S20-620, see closeout |

Logs were captured under the session scratchpad; `make v1` was skipped because this is a subsystem handoff, not a release boundary.

## Tier 2 handoff record (2026-09-03, at `33d90af`)

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 13 s | 982 tests passed, 0 failed across 39 test binaries |
| `make conformance` | exit 0 | 11 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 8 s | 596 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | under 1 s | 5 bounded smoke tests passed |
| `make accounting-smoke` | exit 0 | 1 s | report PARTIAL over the S20-620 run; evidence PASS; stage checker PASS |
| `make sley2-runner-smoke` | exit 0 | under 1 s (cached build) | scripted and intruding trials; evidence PASS |

Logs were captured under the session scratchpad; `make v1` was skipped because this is a subsystem handoff, not a release boundary.

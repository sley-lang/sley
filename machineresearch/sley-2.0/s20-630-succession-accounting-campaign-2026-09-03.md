# S20-630 Succession Accounting Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Ariadne contract review, Nabu architecture
review, and Vulcan surface review queued.

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
| Implementation, revision 2 | pending | pending | `bench/accounting`; 4 offline tests; smoke over the S20-620 run: PARTIAL, 2 attempts, no accepted change, thresholds UNDETERMINED |

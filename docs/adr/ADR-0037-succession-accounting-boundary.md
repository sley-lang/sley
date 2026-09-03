# ADR-0037: accounting as exact derivation from immutable claims

Status: proposed; the S20-630 contract is a draft at revision 1 with
Council review pending; implementation pending

Date: 2026-09-03

## Context

The master goal defines Accepted Change Tokens as total observable model
tokens over accepted correct changes, requires every attempted trial to
stay in the denominator, and sets section 22 thresholds that the plan
freezes. S20-610 and S20-620 store per-arm digest-chained claims with
integer metrics and forbid floats so that accounting can derive ratios
later. Nothing has run a trial; the dossier's succession fields are null.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **Claims are the only input.** The report reads the manifest and each
   arm's verified chain; it never reads a trace body, a model, or a clock.
2. **Exact arithmetic.** Integers and reduced ratios only; a zero
   denominator is a named null, never a substitute.
3. **Every attempt counts.** `attempted` is the claim count whatever the
   status; timeouts and harness failures stay in every denominator.
4. **Thresholds need complete arms.** A threshold is `UNDETERMINED` unless
   both compared arms cover the full task and seed product.
5. **Inherited evidence status.** The report is derived from unverified
   claims and says so; the dossier's succession fields stay null until a
   complete report over verified claims exists.
6. **Codes.** Eight `ACCOUNTING_*` codes 63000 through 63007.
7. **Staging.** `scripts/check_succession_accounting.py` binds the
   contract, ADR, work-package row, and summary section, and fails closed
   if the accounting module appears before the summary allows it.

## Consequences

- When real trials are approved, the same module produces the succession
  numbers with no new arithmetic.
- No number in the report can be misread as a result: the status and
  evidence status travel with it.

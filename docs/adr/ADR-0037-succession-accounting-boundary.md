# ADR-0037: accounting as exact derivation from immutable claims

Status: accepted at contract revision 3 (2026-09-05), closing the
Ariadne, Nabu, and Vulcan review findings; implemented at
`bench/accounting/report.py` with eight offline tests and a smoke over
the S20-620 run

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
5. **Inherited evidence status.** The report derives its evidence status
   from the claims' own statuses and records them, and says so; the
   dossier's succession fields stay null until a complete report over
   verified claims exists.
6. **Codes.** Eight `ACCOUNTING_*` codes 63000 through 63007.
7. **Staging.** `scripts/check_succession_accounting.py` binds the
   contract, ADR, work-package row, and summary section, and fails closed
   if the accounting module appears before the summary allows it.
8. **Named omissions.** The section 22 conditions the plan does not
   encode travel as `NOT_EVALUATED` rows with their owners; the boundary
   sentence claims only the conditions the plan encodes.
9. **Registry, not branches.** Arms load through one verifier registry;
   the legacy arm's missing producer is registry data, and a premature
   chain at its reserved path fails closed.

## Consequences

- When real trials are approved, the same module produces the succession
  numbers with no new arithmetic.
- Every number in the report travels with its threshold verdict (or its
  explicit non-verdict), its evidence status, and its arm's fixture
  status, so no number is lifted out of the report as a bare result: a
  threshold row IS a verdict (`PASS`/`FAIL`), and what travels with it
  is the provenance that keeps the verdict attributable. The guarantee
  is carriage, not verdictlessness. The `derive` command prints
  `DERIVED`, never `PASS`.

# ADR-0042: the finding register is derived from recorded dispositions

Status: proposed; the S20-740 contract is a draft at revision 5 with Council
review pending; register mechanics implemented (2026-09-05, revised
2026-09-11 with enforced round ordering, lane-core compatibility, and
lane-leaf collection) with the
independent review still pending and `release-check` fail-closed

Date: 2026-09-03

## Context

S20-740 owes an independent review dossier whose named input is a finding
register. Every package already records its review obligations and their
dispositions in the machine summary, in the exact strings the reviewers
returned, and the pattern that a package reaches `COMPLETE` only with three
`PASS` reviews has been followed by hand. Nothing verified that pattern, and
nothing answered "how many reviews are open, and at what severity" without
reading the whole summary.

A second, hand-maintained register would drift from the summary and would
invite editing a disposition after the fact. The Council lanes are still
unavailable (ADR-0026), so most obligations are legitimately open.

## Decision

1. **One source.** The register is derived from the machine summary alone and
   is drift-checked in `make quick`; there is no second place to record a
   review outcome.
2. **States, not judgments.** Dispositions are classified into `PASS`,
   `PENDING`, `DEFERRED`, `HISTORICAL_ROUND`, and `OTHER` by first
   underscore-delimited token over a closed head set plus a declared alias
   table, and the register reports the severity tokens a disposition names
   outside negations. It never rewrites, grades, or accepts a finding; an
   unclassifiable disposition is surfaced in `unclassified` rather than
   normalized away, and a `PASS` that still claims an open finding
   contradicts itself into `OTHER`.
3. **Completion implies closure.** A section whose status ends in `COMPLETE`
   (but not `INCOMPLETE` or `NOT_COMPLETE`) may not carry a `PENDING`,
   `DEFERRED`, or `OTHER` obligation; that is a hard failure of the
   builder, so the completion pattern is now machine-enforced. A failed
   round is historical only with a same-reviewer superseding `PASS` in the
   same section; an unre-reviewed failure stays open even when it names
   itself `FAIL`.
4. **Open is the honest state.** `FINDING_REGISTER_OPEN` is expected while the
   lanes are down and is not a gate failure; S20-740 completes only when the
   register is clear and an independent reviewer records a PASS.
5. **Codes and staging.** 75000 through 75003 name the exact failures, and a
   staged checker carries the contract from draft to complete.

## Consequences

- The independent reviewer gets one file naming every obligation, its state,
  and its severities, with a digest tying it to the summary it came from.
- Marking a package `COMPLETE` while a review is pending now fails `make
  quick`, which is the invariant the release gates depend on.
- A `FAIL`/`REVISE` round stays open until the same lane records `PASS`, so
  closing a review round's findings eventually requires re-reviews of the
  failed lanes, not just fixes: the register cannot be cleared by repair
  alone while the failing verdict stands unrevisited.
- Retiring a Council lane or renaming a disposition changes the register, so
  the drift check forces the change to be deliberate.

Note (revision 5, 2026-09-18): round folding additionally requires that a closing `PASS` is not dated before the round it closes (dates read from the `_note` fields); token order alone no longer suffices when both notes are dated.

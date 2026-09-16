# ADR-0043: the decision dossier is derived, and the decision is not

Status: proposed; the S20-750 contract is a draft at revision 8 with Council
review pending; dossier mechanics implemented (2026-09-03, revised
2026-09-14) with the derived state `BLOCKED` and `release-check` and `v2`
still fail-closed

Note (2026-09-15): Revision 8 requires valid binding digests, complete zero-count PASS forms, and GATED packaging facts when no candidate is selected. Regression fixtures construct review states explicitly; the dossier still derives a decision without authorizing release.

Date: 2026-09-03

## Context

The master goal ends with a completion report of thirty-four items and a
release decision state, and forbids vague conclusions. Every item's evidence
now exists somewhere: the machine summary, the S20-710 full SBOM and provenance
documents, the S20-720 candidate evidence, the S20-730 reproducibility and
conformance reports, and the S20-740 finding register. Nothing assembled them,
so "what is actually evidenced" could only be answered by reading everything,
and the decision state was an opinion rather than a derivation.

Several items cannot be evidenced today: no succession trial has run, no
independent review exists, the Council lanes are down, the root license is
unapproved, and the second host is gated.

## Decision

1. **One entry per required item.** The dossier carries the thirty-four items
   verbatim, in the master goal's order, and the contract lists them so the
   checker can verify coverage without reading a document outside the
   repository.
2. **Gated is a state, not a gap to be filled by prose.** An item with no
   tracked evidence is `GATED` with a note naming the missing authority,
   execution, or decision, and carries no value. Nothing is estimated,
   inferred, or asserted from a campaign record.
3. **The state is derived.** Section 27's five states are computed in
   precedence from the entries and sources, with the exact reasons listed.
   Today that yields `BLOCKED`: open reviews, deferred lanes, an unapproved
   root license, no executed trial, fail-closed gates, and a single attesting
   host. A `PASS` while a product gate is fail-closed is a hard failure.
   (2026-09-05: `derive_decision` now reads the entries the contract's
   section 3 mapping names, with a missing or gated decision-input entry
   failing closed; the release-check gate, succession thresholds, and
   approved conditional items still read the tracked sources because no
   section 30 item carries them.)
4. **No mechanism to publish.** The dossier records that no push, tag, upload,
   deployment, or announcement occurred, and this package adds no code that
   could perform one. `decision_authority` always reads
   `OPERATOR_DECISION_NOT_DELEGATED`.
5. **Codes and staging.** 76000 through 76003 name the exact failures, and a
   staged checker carries the contract from draft to complete; completion needs
   every entry evidenced and an operator decision record.

## Consequences

- Progress toward GA is now measurable: the gated-item count and the decision
  reasons shrink as real work lands, and both are checked on every commit.
- The dossier cannot drift into optimism: an unevidenced item stays gated, and
  the decision state follows the evidence rather than the narrative.
- When the succession arms run and the reviews land, the same builder produces
  the completion report without rewriting it.

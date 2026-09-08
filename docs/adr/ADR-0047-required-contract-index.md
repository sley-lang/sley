# ADR-0047: the required contracts are indexed, not restated

Status: proposed; the S20-770 index is a draft at revision 2 with Council
review pending; it defines no contract and changes no identity. Revision
2 record (2026-09-08): row 17.12 gains the accepted
`ENTITY_READ_PROFILE_V2.md` document and the additive
`conformance/smp1-json-bridge/v2` metadata directory; twelve rows and all
digest domains unchanged.

Date: 2026-09-03; revision 2 record 2026-09-08

## Context

Master goal section 17 names twelve contracts Sley 2.0 GA must define and
validate. All twelve exist in this repository, but under the names the
implementation gave them: `sley-scb-object-v1` is SCB1 section 2 plus the
object store record plus the entity body kinds, `sley-protocol-handshake-v1`
is SMP1 sections 2 and 3, and so on. Nothing connected the required name to
the defining document, the digest domain, the checker, or the corpus, so
answering "is 17.10 satisfied?" meant reading the tree.

Two designs were possible: restate each required contract in its own document,
or index the existing ones. Restating would duplicate frozen rules and create a
second source that can drift from the first.

## Decision

1. **Index, never restate.** One table maps each required name to its defining
   documents, digest domains, checkers, and corpora. Where the table and a
   document disagree, the document wins.
2. **The index is machine-checked.** The checker parses the table and verifies
   every named document, checker, corpus, and crate exists, that every domain
   is frozen in `IDENTIFIERS_V1.md`, and that every named checker is actually
   invoked by the Makefile, so the index cannot rot into a list of wishes.
3. **Absence is stated, not hidden.** A required contract with no corpus says
   so with its reason: `sley-test-report-v1` has no corpus of produced reports
   because no test has been executed under any authority.
4. **No claim of completeness.** Several of the twelve are implemented under
   draft revisions with reviews pending; the index says where they are defined,
   not that they are accepted.

## Consequences

- An independent reviewer can start from the master goal's list and reach the
  exact artifacts without searching.
- Renaming a document, dropping a checker from `make quick`, or deleting a
  corpus now fails a gate instead of silently orphaning a required contract.
- The index adds no runtime surface: it is documentation plus a checker.

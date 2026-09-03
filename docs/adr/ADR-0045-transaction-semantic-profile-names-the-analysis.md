# ADR-0045: the transaction semantic profile names the analysis that validated the program

Status: proposed; the transaction model gains a second semantic profile value
as draft revision 2 with Council review pending; implemented 2026-09-03; no
new authority, test execution, or publication

Date: 2026-09-03

## Context

S20-390 froze commit metadata as one triple: commit profile 1, semantic
profile 1 ("executable-program-operation-free"), durability profile 1, and the
transaction decoder accepted exactly that triple. The S20-360 full operation
analysis (ADR-0044) then made candidates whose programs carry semantic
operations validate, which would have produced receipts asserting a profile the
transaction did not run under. The commit path refused such candidates so the frozen claim stayed true
(`3a56342`), and this decision lifts that refusal honestly. Writing the
refusal also exposed that the operation-free wording never described a trusted
genesis, whose object sets already carried operations: the revision states the
value's real meaning instead of preserving a phrase that was never accurate for
every transaction kind.

## Decision

1. **A second value, not a redefinition.** Semantic profile 2 names the
   extended operation analysis. Value 1 keeps its exact meaning, which the
   revision states precisely: no operation analysis ran in the transaction.
   Every existing receipt and vector still says what it always said.
2. **The receipt states what ran.** `commit` selects the profile from the
   validated program: a proposed state carrying an `Operation` entity commits
   under 2, and one without commits under 1.
3. **The decoder accepts exactly two triples.** Anything else is
   `TXN_FIELD_SHAPE`; no other field of the metadata moves.
4. **A trusted genesis records profile 1.** Genesis installs a trusted object
   set instead of validating it, so it performs no analysis whatever the set
   contains, which is exactly what profile 1 states. Genesis receipts are
   therefore unchanged, including the existing fixtures whose object sets carry
   operations.
5. **The corpus covers both.** The accepted receipt corpus gains an
   `ORDINARY_EXTENDED` vector, and the independent Python oracle derives the
   vector label from the decoded semantic profile, so profile 2 is verified by
   an implementation that shares no code with the Rust one.

## Consequences

- An operation-carrying candidate can commit, and its receipt says which
  analysis validated it, which is the S20-390 property that mattered.
- Receipt and exchange identities of operation-free commits are unchanged;
  only the new vector is new.
- Selected-test evidence, policy and epoch transitions, and full crash recovery
  remain out of scope, and this revision is a draft until the Council reviews
  it.

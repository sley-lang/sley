# ADR-0045: the transaction semantic profile names the analysis that judged the program

Status: accepted in draft; the transaction model carries the second semantic
profile value at draft revision 3 with Council review pending; implemented
2026-09-03 with the revision 3 correction 2026-09-05; no new authority, test
execution, or publication

Date: 2026-09-03; corrected 2026-09-05

## Context

S20-390 froze commit metadata as one triple: commit profile 1, semantic
profile 1 ("executable-program-operation-free"), durability profile 1, and the
transaction decoder accepted exactly that triple. The S20-360 full operation
analysis (ADR-0044) then made candidates whose programs carry semantic
operations validate, which would have produced receipts asserting a profile the
transaction did not run under. The commit path refused such candidates so the
frozen claim stayed true (`3a56342`), and this decision lifts that refusal
honestly. Writing the refusal also exposed that the operation-free wording
never described a trusted genesis, whose object sets already carried
operations: the revision states the value's real meaning instead of preserving
a phrase that was never accurate for every transaction kind. The 2026-09-05
correction (revision 3) further repairs decision 1 below: value 1 never meant
"no analysis ran", because phase 7 judges every function unit of an ordinary
commit including operation-free ones. Value 1 means the transaction judged no
semantic operation, which is true of genesis and ordinary operation-free
commits for different reasons.

## Decision

1. **A second value, not a redefinition.** Semantic profile 2 names the
   extended operation analysis: the transaction judged at least one semantic
   operation under it. Value 1 states the transaction judged no semantic
   operation, which covers the trusted genesis (it installs its object set
   without validating it) and the ordinary commit of a program without
   operations (validation ran and judged none). Both values are claims about
   judgment drawn from one basis, the validated program. Receipt bytes are
   unchanged; consumers holding the earlier prose must re-read earlier
   receipts under this correction.
2. **The receipt states what was judged.** `commit` selects the profile from
   the whole proposed entity state, inherited entities included: a proposed
   state carrying an `Operation` entity commits under 2, and one without
   commits under 1. Value 2 may only be emitted under a validation profile
   that performs the S20-360 E1 through E6 analysis.
3. **The decoder accepts exactly two triples, conditioned on kind.** Anything
   else is `TXN_FIELD_SHAPE`; no other field of the metadata moves. Genesis
   accepts only `[1, 1, 1]`; an ordinary commit accepts `[1, 1, 1]` or
   `[1, 2, 1]`.
4. **A trusted genesis records profile 1, as a wire rule.** Genesis installs a
   trusted object set instead of validating it, so it judged no operation
   whatever the set contains, which is exactly what profile 1 states. Both
   decoders enforce this: a genesis asserting profile 2 fails as
   `TXN_FIELD_SHAPE`. Genesis receipts are therefore unchanged, including the
   existing fixtures whose object sets carry operations.
5. **The corpus covers both values and the kind rule.** The accepted receipt
   corpus gains an `ORDINARY_EXTENDED` vector, and the independent Python
   oracle derives the vector label from the decoded semantic profile, so
   profile 2 is verified by an implementation that shares no code with the
   Rust one. The rejected corpus gains a genesis-asserting-profile-2 vector,
   so both implementations prove the kind-conditioned refusal with
   `TXN_FIELD_SHAPE`.

## Consequences

- An operation-carrying candidate can commit, and its receipt says which
  analysis judged it, which is the S20-390 property that mattered.
- Receipt and exchange identities of operation-free commits are unchanged;
  only the new vector is new.
- Selected-test evidence, policy and epoch transitions, and full crash recovery
  remain out of scope, and this revision is a draft until the Council reviews
  it.

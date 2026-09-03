# S20-510 semantic comparison: campaign record (opened 2026-09-03)

Status: contract draft revision 1 committed; Council review pending.

## Why now

Full S20-250 is implemented (reviews pending), so S20-510 is the next
dependency-complete package on the M4 critical path (S20-510, then S20-520
merge, which is also the one absent Section 18.5 fuzz surface). The operator
directed the work to continue to the end of the Sley 2 goal.

## Council availability

Unchanged from the S20-250 record: every lane was still down when this draft
was written (the retry loop in the session scratchpad probes both lanes every
15 minutes and dispatches the queued S20-250 reviews first). The S20-510
Ariadne, Nabu, and Vulcan requests are queued behind them when a lane
returns. The design below is the integrator's, from the Repository Model
sentence on comparison and the frozen S20-250 inputs.

## Design decisions taken by the integrator (pending review)

1. One canonical SCB1 delta record (`sley2.semantic-delta.v1`, tag 510,
   digest domain 20) with five sections: entities (five change classes),
   fields (per changed schema field of every Changed entity, with identity
   add/remove sets and small flag bits), bodies (restricted Function
   fingerprint pairs), relations (symmetric difference of the two
   complete-root indexes' direct edges), and root sets plus collateral
   (transitive impact of the changed seeds over each root's own index).
2. The eleven Repository Model delta classes are exact views over those
   sections, so no second encoding or matching heuristic exists.
3. `sley-repo` owns it over two verified revisions; `sley-query` unchanged.
4. Preconditions fail closed before any delta: same workspace, same epoch,
   both roots complete (IMPACT code preserved), complete function
   inventories (FINGERPRINT code preserved). Identical roots give an empty
   valid delta.

## Open questions for the reviews

- Whether `MetadataOnly` (same body, different object) should count as a
  seed for collateral when no body delta exists (the draft says no).
- Whether the per-field flag bits for TypeDef form and Function parameters
  are the right granularity for S20-520 disjointness, or whether member-level
  entries are needed.
- Whether the delta record needs the base and target transaction identities
  (the draft binds only roots, keeping ancestry out per the Repository Model).

## Commits

- contract draft revision 1: the commit that adds this file.

# S20-320 Full Context Capsule Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Ariadne, Nabu, and Vulcan reviews queued behind the
S20-250, S20-510, S20-520, S20-300, and S20-310 reviews.

## Frontier at start

- Full S20-310 root-backed queries implemented at `76cf2a2` under their
  draft: nineteen classes, exact `total_count`, typed continuation cursors.
- Restricted S20-320 complete: `SLEYRQC1` evidence capsule over complete
  restricted responses only, fixed no-omission, no-truncation,
  no-continuation status, distinct identity domain.
- The master domain `sley2.context-capsule.v1 -> ContextCapsuleId` is
  registered (ADR-0013) and unused.
- No negotiated session exists (S20-330 deferred); S20-400 SMP1 waits for
  the query and capsule surfaces.

## Design brief

Contract: `docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md`, ADR-0031, stage
checker `scripts/check_context_capsule_profile.py`.

- The capsule restates the question (class, body, limits, continuation
  flag, cursor), copies the engine-bound provenance (workspace, epoch,
  root, snapshot, query identity), states completeness, truncation, exact
  `total_count`, `returned`, `omitted`, and `next_after`, copies the exact
  `SLEYRQR1` record, and reorganizes its facts into raw-identity
  dictionaries (entities with kinds, relationships, roots, objects,
  fingerprints).
- The constructor accepts one `RootQueryRequest` and its `RootQueryResponse`
  bound by query identity; nothing else constructs a capsule.
- Session binding is the fixed arm `None`; `Negotiated` is reserved for
  S20-330.
- `SLEYCCP1` record under the master domain; codes 32008 through 32011.
- Repository surface `run_context_capsule` over the S20-310 full surface.

## Open questions for the reviews

- Whether a page capsule should also carry the identity of the previous
  page's capsule (a chained lineage) or whether exact `total_count` plus
  canonical key order suffices for a reader to prove a walk complete.
- Whether the question's named entities belong in the entity dictionary
  (they do at this revision, so class-3 fingerprints have an index).
- Whether `kinds` should be a parallel list with `0` for unknown or a
  sparse `(index, kind)` table.

## Commits

- contract draft revision 1: the commit that adds this file.

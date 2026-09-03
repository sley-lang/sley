# S20-310 Full Root-Backed Query Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Ariadne, Nabu, and Vulcan reviews queued behind the
S20-250, S20-510, S20-520, and S20-300 reviews.

## Frontier at start

- Full S20-300 complete-root snapshot implemented at `094a7bf` under its
  draft (arm-2 snapshot bound to the exact root, repository cache whose hits
  may serve read-only derived query surfaces only).
- Restricted S20-310 complete: four typed queries over arm 1, hard failure
  on any omitted fact, `sley2.query.v1` identities.
- The nineteen root-backed classes the master goal requires are counted in
  the work-package row, the error-code registry, and the machine summary
  (`full_query_classes_required: 19`) but enumerated nowhere; the legacy
  Sley exposed six structural query kinds (`all`, `modules`, `tasks`,
  `types`, `effects`, `calls`).

## Design brief

Contract: `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md`, ADR-0030, stage
checker `scripts/check_root_backed_query_profile.py`.

- Nineteen classes over one verified root: summary, entity, fingerprint,
  by-kind, workspace packages, package exports and dependencies, namespace
  members, owning-namespace chain, entry points, dependency roots, direct
  dependencies and dependents, reverse and forward closures, contracts and
  tests for a target, declared effects, capability requirements for a
  subject.
- Exact-then-paged results: complete computation under the work ceiling,
  canonical key order, typed `after`/`next_after` cursors, exact
  `total_count` on every page, paging only under `allow_continuation`;
  closure depth and single-key classes never page.
- Input binding (`QUERY_ROOT_MISMATCH`) ties the arm-2 snapshot, bodies,
  bindings, fingerprints, and root facts together before any class runs.
- One repository surface over an S20-390 verified revision and the S20-300
  cache: the first consumer of a cache hit, edges only, read-only evidence.
- Thirty-third identifier domain `sley2.root-query.v1`; `SLEYRQQ1` request
  and `SLEYRQR1` response records; codes 31008 through 31010.

## Open questions for the reviews

- Whether the nineteen classes are the right enumeration of the master
  goal's requirement, and whether any class should be split (for example
  `ListDeclaredEffects` per kind) or merged.
- Whether `ListOwningNamespaces` and the closure classes should page rather
  than fail when they exceed the applied limits.
- Whether a continuation cursor needs to bind the previous page's response
  (a chained identity) or the exact `total_count` plus canonical key order
  is sufficient evidence that no page hides a fact.

## Commits

- contract draft revision 1: the commit that adds this file.

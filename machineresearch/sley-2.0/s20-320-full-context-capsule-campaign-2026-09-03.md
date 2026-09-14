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
- Session binding is the arm `None` outside a session; `Negotiated` is
  minted only by `SessionAuthority::bind_context_capsule` over a live
  session (contract revision 3, review slice 2026-09-05).
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

## Implementation under the draft (2026-09-03)

The Council lanes were still unavailable, so the integrator implemented the
full profile under contract draft revision 1 with the reviews queued:

- `crates/sley-query/src/context_capsule.rs`: `build_context_capsule` over a
  bound request and response, question, provenance, status, copied record,
  fact dictionaries, `SLEYCCP1` record under `sley2.context-capsule.v1`,
  codes 32008 through 32011; the S20-310 question encoder is shared inside
  the crate.
- `crates/sley-repo/src/root_query.rs`: `run_context_capsule`; the
  repository query outcome now carries the request.
- Fixture `conformance/context-capsule/v1` (23 vectors bound to the S20-310
  fixture by query identity); independent oracle
  `scripts/check_context_capsule_vector.py` PASS from the S20-310 records.
- Persistent fuzz `fuzz/targets/context_capsule_builder.rs`, smoke PASS
  over 971 seeds.
- Closeout `docs/audits/S20_320_FULL_CONTEXT_CAPSULE_CLOSEOUT.md`; summary
  status `S20_320_FULL_IMPLEMENTED_REVIEW_PENDING`; frontier re-anchored to
  the S20-400 SMP1 protocol contract.

Implementation commit: `e695c7d`.

## Tier 2 handoff gate (2026-09-03, at `e695c7d`)

| Gate | Result |
|---|---|
| `make core` | PASS (949 tests) |
| `make conformance` | PASS (context capsule oracle line included) |
| `make adversarial` | PASS |
| `make fuzz-smoke` | PASS |
| `make context-capsule-persistent-fuzz-smoke` | PASS (971 seeds) |

Total 31 seconds. `make v1` skipped: subsystem handoff, not a release
boundary. Council reviews remain pending; the package is not complete.

## Revision note (2026-09-14)

This brief stays as the revision-1-start campaign record above. The
contract has since moved to revision 4 (Status lines in the contract,
ADR-0031, and the closeout name revision 4; the checker cross-checks all
three): revision 3 added the `u64be(response_bytes)` prefix and the
authority-minted `Negotiated` arm, and revision 4 binds `Complete` to the
whole result, enforces the table ceilings at encoding, wires the
internal-invariant code, and restates the evidence set. The open
questions above are answered: walks prove complete by exact `total_count`
plus canonical key order (no chained lineage), named entities stay in the
dictionary as non-existence claims, and `kinds` stays a parallel list
with reserved `0`.

# S20-320 Full Context Capsule Closeout

Status: **implemented under the draft Context Capsule Profile v1 contract (revision 4); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus query-and-repository-focused Tier 2 handoff**

## Claim under review

The master context capsule under the registered `sley2.context-capsule.v1`
domain now exists: over one `RootQueryRequest` and the `RootQueryResponse`
the S20-310 full engine produced for it (bound by query identity), it
restates the question, copies the engine-bound provenance (workspace,
epoch, root, snapshot, query identity), states completeness, truncation,
exact `total_count`, `returned`, `omitted`, and `next_after`, copies the
exact `SLEYRQR1` record, and reorganizes its facts into raw-identity
dictionaries with kinds, relationships, roots, objects, and fingerprints.
The session binding is the arm `None` outside a session; the `Negotiated`
arm is minted only by `SessionAuthority::bind_context_capsule` over a
live session whose authority-held binding equals the response provenance,
so no capsule is a handle and no caller-declared provenance enters the
identity. The
contract is `docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md` with ADR-0031. It is
a draft: every Council lane was unavailable when it was written and when
the implementation landed, so the reviews that freeze it and complete the
package are pending and must pass before the status above changes.

The implementation provides:

- `sley-query`: `context_capsule.rs` with `build_context_capsule`,
  the authority-delegated `build_context_capsule_session` primitive,
  `ContextCapsule` and its getters, `CapsuleCompleteness`,
  `ContextRelationship`, the `SLEYCCP1` record encoder, and
  `ContextCapsuleErrorCode` with the four codes 32008 through 32011; the
  S20-310 question encoder is shared inside the crate and the restricted
  `SLEYRQC1` capsule is untouched;
- `sley-protocol`: `SessionAuthority::bind_context_capsule`, the only
  supported path to the `Negotiated` arm, verifying session liveness and
  the authority-held binding before delegating the encoding;
- `sley-repo`: `run_context_capsule` over the S20-310 full surface, with
  the request now carried in the repository query outcome and the capsule
  failure namespace preserved.

## Evidence

- Contract draft revision 3 and ADR-0031 at this commit; implementation,
  corpus, oracle, and fuzz slice below.
- Conformance corpus: `conformance/context-capsule/v1/accepted.json`
  (twenty-four vectors: all nineteen classes, one `Negotiated`-arm vector
  over the class-01 question under a fixed fixture session, and both pages of the
  entity and edge continuation walks, each bound to its S20-310 fixture
  vector by query identity), drift-gated by
  `scripts/generate_context_capsule_fixtures.py --check` in `make quick`;
  `scripts/check_context_capsule_vector.py` rebuilds every record and
  `ContextCapsuleId` from the S20-310 fixture alone by decoding each
  `SLEYRQR1` payload by class, including the arm bytes and session
  identity of the bound vector, registered in `make conformance` under the
  frozen oracle environment.
- Native tests: four `sley-query` capsule tests (every class yields a
  `Complete` capsule with exact facts and 128-run determinism; page
  capsules state `omitted` and cover the walk; foreign and drifted sources
  fail `CONTEXT_CAPSULE_SOURCE_INVALID` and the code table is exact; the
  session arm carries the session identity and changes the digest) and
  one `sley-repo` test (capsules from a rebuild and a cache hit are
  identical and carry the verified root and workspace); one
  `sley-protocol` test pins the authority mint (the live session mints,
  unknown and closed sessions refuse `SESSION_UNKNOWN`, foreign
  provenance refuses `CONTEXT_CAPSULE_SOURCE_INVALID`); `sley-query` 60
  tests and `sley-repo` 345 tests pass.
- Persistent fuzz: `fuzz/targets/context_capsule_builder.rs` (engine-answered
  queries must capsule or fail only on a resource ceiling; dictionary
  order, index ranges, omission arithmetic, completeness, determinism, and
  the page-capsule walk are asserted; the `Negotiated` arm encoding is
  covered through the authority-delegated primitive), smoke `PASS` over a 971-seed corpus
  (`docs/audits/S20_700_CONTEXT_CAPSULE_PERSISTENT_SLICE.md`); eighteen
  scoped targets and seventeen smoke gates now stand.
- Tier 1: `make quick` green at every commit.
- Tier 2: see the validation record below.

## Findings closed in flight

- The question's named entities join the entity dictionary so class-3
  fingerprints and class-2 objects have an index; a namespace named by
  class 8 therefore carries kind `0` in the dictionary while every member
  carries its stated kind, and the oracle reproduces that rule.
- Review slice 2026-09-05 (contract revision 3, closing the seven S20-320
  P0s): section 6 spells the `u64be(response_bytes)` record prefix both
  encoders emit and defines `bytes(x)`; the `Negotiated` arm is kept and
  minted only by `SessionAuthority::bind_context_capsule` over a live
  session, replacing the caller-declared constructor, with section 11,
  ADR-0031, this closeout, and the campaign record reconciled to the kept
  arm and the stage checker binding the authority markers plus a
  contract/ADR/closeout revision cross-check; the corpus gains the bound
  class-01 vector with an oracle arm path, and the fuzz target covers the
  arm encoding.

## Explicitly open and deferred

- **Council reviews.** Ariadne, Nabu, and Vulcan reviews are queued behind
  the earlier draft packages and land as contract revisions; the campaign
  record lists the open questions (chained page lineage, question entities
  in the dictionary, sparse versus parallel kinds).
- No consumer of a capsule exists yet: S20-400 SMP1 transports it and
  S20-330 mints a negotiated session binding through the session
  authority into its `Negotiated` arm.
- Strict pedantic clippy debt in older `sley-repo` exchange and GC test
  modules is pre-existing; the new paths lint clean under `--no-deps`.

## Validation record

Tier 1 `make quick` passed at every commit of the slice. Tier 2 ran on
2026-09-03 at `e695c7d`: `make core` (949 tests), `make conformance`
(including the context capsule oracle line), `make adversarial`,
`make fuzz-smoke`, and `make context-capsule-persistent-fuzz-smoke` all
exited 0 in 31 seconds; the per-gate record is in
`machineresearch/sley-2.0/s20-320-full-context-capsule-campaign-2026-09-03.md`.
The full `make v1` gate was skipped
because this is a subsystem handoff, not a release boundary; `make v2` and
`make release-check` remain intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

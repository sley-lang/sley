# S20-410 SMP1 Frame Campaign (2026-09-03)

Status: slice A implemented under the S20-400 contract draft (revision 2);
the deterministic server dispatch is slice B; Council reviews of S20-400
pending.

## Slice A (frame, negotiation, identity, envelopes)

- `crates/sley-protocol` (new workspace member): `ProtocolFrame`,
  `encode_frame`, `encode_hello_frame`, `decode_frame` with the length
  prefix checked against the negotiated ceiling before allocation and the
  `ProtocolFrameId` trailer verified before any field is read; `Hello`,
  `negotiate`, `SelectedProfile::handshake_id` under
  `sley2.protocol-handshake.v1`, `check_claim` (`PROTOCOL_DOWNGRADE`);
  `RequestRegistry` (session-scoped strictly increasing identifiers,
  inflight ceiling, closed sessions); `Method` (forty-one frozen tags, five
  reserved); `BoundedContext`; `ProtocolFailure`; the twelve codes 40000
  through 40011; the protocol schema epoch record (one contract 400 under
  digest domain tag 22).
- `sley-id`: thirty-fourth domain `sley2.protocol-frame.v1`,
  `ProtocolFrameId`, frozen vector.
- Contract revision 2: one contract tag 400; the hello is frame kind 4
  (the epoch registry keys digest domains uniquely per contract, so the
  three-tag draft could not be registered).
- Fixture `conformance/smp1/v1` (client and server hello frames, the
  selected profile and handshake identity, request, response, and failure
  frames, four rejections); independent oracle `scripts/check_smp1_vector.py`
  PASS with its own SCB1 encoders, envelope, digests, and negotiation.
- Persistent fuzz `fuzz/targets/smp1_frame_decoder.rs` (direct, rehashed,
  and hello-negotiation lanes), smoke PASS over 489 seeds
  (`docs/audits/S20_700_SMP1_PERSISTENT_SLICE.md`); nineteen scoped targets
  and eighteen smoke gates now stand.
- Native: four `sley-protocol` tests (frame round trip and defect matrix,
  derived negotiation with downgrade detection, request identity matrix,
  method table and failure envelope), 128-run determinism.

## Slice B (deterministic server, read-only and reference families)

- `crates/sley-protocol/src/server.rs`: `Server::answer` decodes a request
  frame under the negotiated ceiling, enforces version, session, request
  identity, and method rules in contract precedence, dispatches the body to
  the owning engine, and answers with a response frame whose bounded
  context is copied from the owner's response; twenty-two methods dispatch
  (session 100 through 104, refs 202/203/205/206/214, revision 204,
  compare 207, merge.judge 208, exchange.export 210, query 300 through
  303, receipt 501, checkout 502, recovery 504, cancel 603); fourteen
  non-reserved methods answer `PROTOCOL_METHOD_UNSUPPORTED` with the
  versioned detail `S20-410-SLICE-C-DEFERRED`; five reserved methods answer
  `SMP1-RESERVED-METHOD`.
- Contract revision 3 adds appendix A with the exact body records of the
  dispatched methods.
- `sley-repo` gains the `test-support` feature (a public mirror of the
  trusted-genesis fixtures) so downstream crates can build repositories in
  tests.
- Native: four server tests (session, repository, and transaction methods
  over a trusted genesis with byte-identical answers from a second server
  and 128 repeated reads; the query family transporting the exact S20-310,
  S20-320, and restricted records with paging and owner mismatch codes;
  identity, session, downgrade, and frame rules at the server; a
  dependency-free root exported as a pack byte for byte).
- Known owner behaviours surfaced and preserved: a root binding a
  dependency root no packed root provides is refused by the pack owner
  (`PACK_ROOT_INVALID`), and an exchange over a repository with no named
  branch is `REF_IO`; pack failures carry numeric 0 until S20-560 exposes
  its registry.

## Slice C (mutation-side families)

- Nine more methods dispatch: `workspace.create` (trusted genesis from the
  state root, policy root, object, and tombstone bytes; identical inputs
  yield the identical `TransactionId`), `workspace.open`, `candidate.create`,
  `candidate.inspect`, `candidate.discard` (the server holds no candidate
  state), `candidate.validate` (the S20-360 validator renders every outcome
  as a result record), `commit` (S20-390 with the caller-supplied
  principal and clock), `merge.commit` (judge, plan, commit through
  S20-520), and `exchange.import` (S20-540 into the server's repository);
  thirty-one methods now dispatch and five stay deferred
  (`candidate.append`, `gc.dry_run`, `gc.collect`, `execute`, `report`).
- Contract revision 3 appendix A carries the body records of every
  dispatched method.
- Native: a fifth server test (workspace.create reproducing the genesis
  identity in a fresh repository, owner codes preserved through the
  candidate and commit paths over malformed bytes, merge.commit over equal
  roots reaching the merge owner, an export/import round trip whose
  accepted head is the source genesis, and the deferred reason on
  `execute`).
- Numeric exposure follow-up: validation, candidate, state-root, and
  policy-root failures carry numeric 0 at this slice because their crates
  expose symbols only; commit, branch, exchange, query, and capsule
  failures carry their exact numerics.

## Slice D (candidate.append and the remaining blockers)

- `candidate.append` dispatches by composing two candidate records through
  the owner's codec (the operations and preconditions of a canonical
  one-or-more-operation record are appended in order and the candidate is
  rebuilt); thirty-two methods now dispatch.
- Blocked, and answered with the versioned reason until their owners
  expose the surfaces: `gc.dry_run` and `gc.collect` need a production
  S20-560 `GcObjectVerifier` (the crate has test implementations only);
  `execute` and `report` need a public S20-380 `ConstValue` bytes codec and
  a stored-bytes form of the S20-290 execution report envelope. These are
  owner gaps, recorded in the machine summary as
  `protocol.deferred_method_blockers`, not transport gaps.
- With every unblocked method dispatched, S20-410 is implemented under the
  S20-400 draft with the Council reviews pending.

## Tier 2 handoff gate for slice A (2026-09-03, at `d4ff651`)

| Gate | Result |
|---|---|
| `make core` | PASS (953 tests) |
| `make conformance` | PASS (SMP1 oracle line included) |
| `make adversarial` | PASS |
| `make fuzz-smoke` | PASS |
| `make smp1-persistent-fuzz-smoke` | PASS (489 seeds) |

Total 31 seconds. `make v1` skipped: subsystem handoff, not a release
boundary.

## Tier 2 handoff gate for slice B (2026-09-03, at `bb67ab9`)

| Gate | Result |
|---|---|
| `make core` | PASS (957 tests) |
| `make conformance` | PASS |
| `make adversarial` | PASS |
| `make fuzz-smoke` | PASS |
| `make smp1-persistent-fuzz-smoke` | PASS (489 seeds) |

Total 38 seconds. `make v1` skipped: subsystem handoff, not a release
boundary.

## Tier 2 handoff gate for slice C (2026-09-03, at `8cd39b3`)

| Gate | Result |
|---|---|
| `make core` | PASS (958 tests) |
| `make conformance` | PASS |
| `make adversarial` | PASS |
| `make fuzz-smoke` | PASS |
| `make smp1-persistent-fuzz-smoke` | PASS (489 seeds) |

Total 32 seconds. `make v1` skipped: subsystem handoff, not a release
boundary.

## Tier 2 handoff gate for slice D (2026-09-03, at `6dac878`)

| Gate | Result |
|---|---|
| `make core` | PASS (958 tests) |
| `make conformance` | PASS |
| `make adversarial` | PASS |
| `make fuzz-smoke` | PASS |
| `make smp1-persistent-fuzz-smoke` | PASS (489 seeds) |

Total 31 seconds. `make v1` skipped: subsystem handoff, not a release
boundary. Council reviews remain pending; S20-400 and S20-410 are not
complete.

## Commits

- slice D: `6dac878`.
- slice C: `8cd39b3`.
- slice B: `bb67ab9`.
- slice A: `d4ff651`.

## Revision 6 (2026-09-03, S20-620 finding)

The S20-620 runner derives per-response failure counts from frames it cannot decode, and no wire signal distinguished a failure envelope from an owner body. SMP1 revision 6 sets response flag bit 2 (`failed`) on every failure envelope; requests and hellos carrying it are `PROTOCOL_FRAME_INVALID`. The codec, server, SMP1 fixture (failure vector), Python oracle, and the S20-420 bridge (revision 4, `flags.failed`) moved together; the S20-410 persistent slice and every oracle pass unchanged.

## Slice C (2026-09-03, SMP1 revision 7)

Appendix C defines the bodies of `gc.dry_run`, `gc.collect`, `execute`, and `report`. The server derives the GC retention snapshot itself (clients may only add session pins) and verifies objects through the new S20-560 `RepositoryObjectVerifier`; `execute` is head-bound, runs the named Function of the bound root through the S20-250 full projection, the restricted VM, and the S20-290 report builder, and stores the report preimage create-once (S20-560 report store, codes 56000 through 56002); `report` answers the stored record. `sley-mutate` exposes `encode_const_value` and `decode_const_value`. The offered hello now names thirty-seven methods. Commit and Tier 2 are recorded in the table below when they land.

| Stage | Commit | Tier 1 | Notes |
|---|---|---|---|
| Slice C implementation, SMP1 revision 7 | `49c06f7` | green | 18 protocol tests (two new), report store test, executable test genesis |

### Slice C Tier 2 handoff record (2026-09-03, at `49c06f7`)

| Gate | Result | Wall time | Evidence |
|---|---|---:|---|
| `make core` | exit 0 | 13 s | 985 tests passed, 0 failed across 39 test binaries |
| `make conformance` | exit 0 | 12 s | 19 oracle results PASS |
| `make adversarial` | exit 0 | 11 s | 597 tests passed, 0 failed |
| `make fuzz-smoke` | exit 0 | under 1 s | 5 bounded smoke tests passed |
| `make smp1-json-bridge-persistent-fuzz-smoke` | exit 0 | 9 s | 633 runs, PASS |
| `make smp1-persistent-fuzz-smoke` | exit 0 | 8 s | 653 runs, PASS |
| `make sley2-runner-smoke` | exit 0 | 1 s | scripted and intruding trials over the endpoint now offering 37 methods; evidence PASS |
| `make accounting-smoke` | exit 0 | 1 s | report PARTIAL over the S20-620 run; evidence PASS |

Logs were captured under the session scratchpad; `make v1` was skipped because this is a subsystem handoff, not a release boundary.

## Revision 8: execute profile selector (2026-09-03)

The frontier package `S20-410-EXECUTE-PROFILE-SELECTOR` lands SMP1 revision 8:
`limits` field 6 selects the cache profile for `execute` (1 restricted, 2
extended), decoded in `crates/sley-protocol/src/server.rs`
(`decode_execution_limits`), covered by
`execute_selects_the_cache_profile_from_limits_field_six`, and carried by the
release demo fixture (`conformance/release-demo/v1/demo.json`, regenerated).
Tier 2 gates and the release candidate smoke are recorded below once run.

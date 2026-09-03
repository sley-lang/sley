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

## Slice C (mutation-side families) — pending

`workspace.create/open`, `merge.commit`, `exchange.import`, `gc.*`, the
candidate family, `commit`, `execute`, and `report` dispatch, plus the
server request/response conformance corpus. Until slice C lands the
protocol summary status stays `S20_400_CONTRACT_DRAFT_S20_410_IN_PROGRESS`.

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

## Commits

- slice A: `d4ff651`.

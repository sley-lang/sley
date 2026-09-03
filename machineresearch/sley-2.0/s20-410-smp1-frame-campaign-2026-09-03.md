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

## Slice B (deterministic server) — pending

Method dispatch over the frozen engines for every non-reserved method,
byte-identical responses across runs, and the request/response conformance
corpus. Until slice B lands the protocol summary status stays
`S20_400_CONTRACT_DRAFT_S20_410_IN_PROGRESS`.

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

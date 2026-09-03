# S20-440 SMP1 Cancellation, Streaming, and Budgets Campaign (2026-09-03)

Status: implemented under the S20-400 contract draft (revision 4); the
S20-400 Council reviews are pending.

## Design brief

- Batch admission: decode and admit every frame of a batch before any
  engine runs; cancel by flag bit 0 or method 603 names a same-session
  request; a cancelled request keeps its identifier and answers
  `PROTOCOL_CANCELLED`; completed requests answer normally; the latency
  bound is one request execution.
- Streaming: `stream_chunk = record(index, total, bytes)` in event frames
  under the negotiated feature, final flagged response with the bounded
  context; overhead 512 bytes, minimum chunk 64 bytes; fail closed with
  `PROTOCOL_LIMIT_EXCEEDED` otherwise; reassembly rejects any disorder.
- Budgets: per-session `max_work` minus one plus returned bytes per
  success; exhausted budgets fail closed before dispatch.
- Fuzz: a fourth lane over chunk records and split-and-reassemble bodies.

## Open questions for the reviews

- Whether budgets should charge the owner's `charged_work` where an owner
  reports it (S20-310) instead of returned bytes.
- Whether a cancel should be allowed to name a request of a later batch
  (a pre-cancel) or only of the same batch.

## Tier 2 handoff gate (2026-09-03, at `1c8d149`)

| Gate | Result |
|---|---|
| `make core` | PASS (960 tests) |
| `make conformance` | PASS |
| `make adversarial` | PASS |
| `make fuzz-smoke` | PASS |
| `make smp1-persistent-fuzz-smoke` | PASS (652 seeds, four lanes) |

Total 30 seconds. `make v1` skipped: subsystem handoff, not a release
boundary. Council reviews remain pending.

## Commits

- implementation and contract revision 4: `1c8d149`.

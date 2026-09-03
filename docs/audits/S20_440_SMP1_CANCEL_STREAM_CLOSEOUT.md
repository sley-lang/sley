# S20-440 SMP1 Cancellation, Streaming, and Budgets Closeout

Status: **implemented under the draft SMP1 contract (S20-400, revision 4); Council reviews pending, so S20-400 and S20-440 are not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus protocol-focused Tier 2 handoff**

## Claim under review

Over the S20-410 server, cancellation is bounded and never partial: a
batch of frames is decoded and admitted in order before any engine runs, a
cancel (flag bit 0 or method 603) naming a request of the same session that
has not started executing makes it answer `PROTOCOL_CANCELLED` without
running while keeping its identifier in the sequence, and a completed
request answers normally with the cancel acknowledged; the latency bound
is one request execution. Streaming is lawful only under the negotiated
`stream` feature: a response frame that would exceed the ceiling travels
as ordered event frames carrying `stream_chunk` records followed by the
flagged response with the bounded context and an empty body, every frame
fits the ceiling, reassembly rejects reordering, gaps, and foreign frames,
and without the feature the response is `PROTOCOL_LIMIT_EXCEEDED` with no
partial body. Budgets are hard: each session starts with the negotiated
`max_work`, every successful response charges one plus its returned bytes,
`session.budgets` reports the remainder, and an exhausted budget fails a
request closed before any engine runs. The rules are `docs/spec/SMP1.md`
section 7 and appendix B (revision 4). They are a draft: the S20-400
reviews are pending.

The implementation provides, in `crates/sley-protocol`: `StreamChunk`,
`stream_response`, `reassemble_stream`, `STREAM_FRAME_OVERHEAD`,
`MIN_STREAM_CHUNK_BYTES`; `Server::answer_batch` (batch admission and
cancellation), the streaming path of every response, per-session budgets
with `Server::remaining_budget`, and `session.budgets` reporting the
remainder.

## Evidence

- Contract revision 4 and the implementation at `1c8d149`; Tier 2 recorded in
  `machineresearch/sley-2.0/s20-440-smp1-cancel-stream-campaign-2026-09-03.md`.
- Native tests: the streaming codec test (split, every frame within the
  ceiling, reassembly to the exact body and bounded context, single frame
  when it fits, fail closed without the feature or under an uncarriable
  ceiling, reordering, gap, and foreign-frame rejection, 128-run
  determinism) and the server test (cancel in the same batch before
  execution by method and by flag, acknowledgement after completion,
  identifiers strictly increasing across cancelled requests, a streamed
  checkout under a 640-byte ceiling reassembling to the unstreamed body,
  the same body refused without the feature, and a 64-unit budget
  exhausted after two renewals); `sley-protocol` 11 tests pass.
- Persistent fuzz: the SMP1 target gains a fourth lane over chunk records
  and split-and-reassemble bodies, smoke `PASS` over a 652-seed corpus.
- Tier 1: `make quick` green at every commit.

## Explicitly open and deferred

- **Council reviews** of S20-400 (Ariadne owner, Nabu, Vulcan) land as
  contract revisions.
- Engine calls are not interruptible; the bound is therefore one request
  execution. The S20-380 VM's own deterministic `cancel_at_fuel` point
  (threat T31) becomes reachable through SMP1 only when `execute`
  dispatches, which waits on the S20-380 codec gap recorded in S20-410.
- Budgets charge returned bytes, not engine work; an owner that exposes
  its charged work (the S20-310 engine does) may refine the charge in a
  later revision.
- Frames of one batch are answered in order; concurrent transports are a
  later profile.

## Validation record

Tier 1 `make quick` passed at the commit. Tier 2 ran on 2026-09-03 at
`1c8d149` (`make core` 960 tests, `make conformance`, `make adversarial`,
`make fuzz-smoke`, `make smp1-persistent-fuzz-smoke`, all exit 0 in 30
seconds) and is recorded in the campaign record. The
full `make v1` gate was skipped because this is a subsystem handoff, not a
release boundary; `make v2` and `make release-check` remain intentionally
fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

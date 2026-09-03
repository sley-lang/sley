# S20-620 Sley 2 Trial Runner Closeout

Status: **implemented under the draft Sley 2 Trial Runner v1 contract (revision 2); Council reviews pending, so the package is not complete; no model ran and no real trial exists; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus benchmark-focused Tier 2 handoff**

## Claim under review

The Sley 2 arm's runner is endpoint-only and trace-complete. One trial is
one `sley serve --json` process over a disposable repository the runner
seeds through `exchange.import`; the runner reads the deterministic
handshake identity from a hello-only probe invocation's report, opens the
session, and hands the agent adapter a handle whose only operations are
`exchange` and `affordances`. Every frame in either direction is chained
into the trial's trace before the next request is written, so the recorded
responses are exactly the context the agent had; ten metrics derive only
from frame records, and the remaining frozen metrics are injected,
explicitly unverified adapter claims appended under the `sley_2_0` arm with
the S20-610 manifest and chain primitives. The exact endpoint binary is
part of the record as `endpoint_sha256`. The contract is
`docs/spec/SLEY2_TRIAL_RUNNER_V1.md` with ADR-0036; it is a draft written
and implemented while every Council lane was unavailable, so the Ariadne,
Nabu, and Vulcan reviews that freeze it and complete the package are
pending and must pass before the status above changes.

The implementation provides:

- `bench/sley2/runner.py`: `Sley2ErrorCode` with the ten codes 62000
  through 62009, the `AgentAdapter`, `OracleAdapter`, and `AccountingClock`
  Protocols, `EndpointHandle` and `handle_surface`, the `Endpoint` process
  wrapper (the runner's only external command), `endpoint_offer`,
  `probe_handshake`, the create-once `Trace` with `append_trace_record` and
  `verify_trace`, `derive_trace_metrics`, `append_trial_claim` and
  `verify_trial_claims`, `run_scripted_trial`, the scripted smoke agent
  and oracle, and the `smoke` and `verify` commands;
- `bench/sley2/tests/test_runner.py`: five offline tests over a fake
  endpoint;
- `make sley2-runner-smoke`: builds `sley`, runs the scripted trial and the
  intruding trial over the frozen S20-540 exchange fixture, and writes
  `evidence/runtime/s20-620-sley2-smoke/evidence.json`;
- `scripts/check_sley2_trial_runner.py` in `make quick`: contract markers,
  runner markers and forbidden surfaces, and the offline tests.

## Evidence

- Contract draft revision 1, ADR-0036, and the stage checker at
  `51a8e22`; SMP1 revision 6 (`failed` response flag, an S20-620 finding)
  at `aea2d11`; revision 2 and the implementation in the commit recorded in
  the campaign record.
- Offline tests (five, all pass): the trace chain is complete and tamper
  evident (duplicate trace, deleted record, edited frame, missing footer);
  metrics derive only from frame records (tool calls, context bytes,
  entities and relationships, check attempts, repair loops); the handle
  exposes exactly two operations and an agent naming a runner field or an
  unlisted method is refused with its code; a scripted trial over a fake
  endpoint traces eight request frames, claims, verifies, refuses a
  duplicate, and reports an incomplete run; claim validation fails closed
  on arm, status, timeout, record count, digests, unknown tasks, and a
  truncated chain.
- Smoke over the real endpoint: the scripted trial completes with the
  oracle's `rejected` claim (no task attempted), 18 trace records, four
  tool calls, the endpoint's report showing 8 frames read and written, 7
  answers, 0 failed answers, and exit status 0; the intruding trial is
  refused as `SLEY2_TRIAL_PRIVILEGED_CONTEXT` with its trace and claim
  written; both claims verify; the handle surface is exactly
  `affordances` and `exchange`, and a leaky handle subclass is refused.
- `make quick` green at the commit. Tier 2: see the validation record
  below.

## Findings closed in flight

- SMP1 had no wire signal distinguishing a failure envelope from an owner
  body, so a client could not count failed answers without decoding
  bodies; SMP1 revision 6 sets response flag bit 2 (`failed`), the bridge
  names it (revision 4), and the runner derives repair loops from it.
- Revision 2 of the contract records the clarifications the
  implementation forced: the runner opens and closes the session, the
  agent's request shape, `endpoint_sha256` in the header and claim,
  `tool_calls` excluding the runner's close, the scripted smoke's rejected
  claim, nullable claim digests, and adapter exceptions as harness
  failures.

## Explicitly open and deferred

- **Council reviews.** Ariadne, Nabu, and Vulcan reviews land as contract
  revisions; the campaign record lists the open questions.
- No model, provider, or oracle implementation exists; no real trial has
  run; `actual_trials` stays zero and nothing here is benchmark evidence.
- The scripted smoke exercises no capsule or candidate round trip because
  no request builder for those records is reachable through the endpoint;
  the trace and metric derivation for them are covered by the offline test
  over the fake endpoint.
- Wall time in the smoke claim is derived from the injected second clock
  and reads zero for a sub-second trial.

## Validation record

Tier 1 `make quick` passed at the commit. Tier 2 is recorded in
`machineresearch/sley-2.0/s20-620-sley2-trial-runner-campaign-2026-09-03.md`.
The full `make v1` gate was skipped because this is a subsystem handoff,
not a release boundary; `make v2` and `make release-check` remain
intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

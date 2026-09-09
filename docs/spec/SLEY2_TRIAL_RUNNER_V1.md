# Sley 2 Trial Runner v1

Status: S20-620 contract draft, revision 4 (2026-09-09); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 records the clarifications found while implementing
revision 1 (section 9). Revision 3 replaces the section 2 capability claim
with the cooperative-adapter trust boundary the round showed it to be.
Revision 4 admits `entity.version` and `entity.signature` to
`ARM_AFFORDANCES` (eighteen names) and binds the claim digest to the
per-trial immutable snapshot; the two names require a version 2 offer.
The implementation is `bench/sley2/runner.py` and `bench/sley2/handle.py`;
implementation state is tracked in the machine summary.

## Boundary

S20-620 freezes the runner mechanics of the mandatory `sley_2_0` comparison
arm: how one trial drives the S20-430 endpoint over one disposable
repository, what the agent arm may see, what a complete trace is, and how
trial claims chain. It executes no model, provider, shell, or oracle
(every such interface is an injected Protocol, as in S20-610), runs no
real trial, derives no ratio, and makes no succession claim. It composes
the S20-610 run manifest (`sley2.raw-run-manifest.v1`, one manifest per
run naming all three arms), the S20-610 canonical JSON and digest-chain
mechanics, the S20-430 endpoint (`docs/spec/SLEY_CLI_V1.md`), the S20-420
JSON form, and the S20-540 exchange fixture; it alters none of them. The
master goal requires an agent that receives only an SMP1 context capsule
and mutation affordances and completes its task without raw repository
files, source syntax, an entire-store dump, or human intervention (master
goal sections 20.10, 21.3, 21.4, 21.6, 21.7).

## 1. Endpoint driving

- One trial is one `sley serve --repository <disposable> --json --report
  <path>` process in per-frame mode. The runner writes one `Frame` line per
  request and reads event and response lines until the response naming
  that request identifier arrives. The runner never speaks bytes, never
  builds a frame from anything but the endpoint's own outputs and the
  fixture, and computes no protocol digest.
- The trial's handshake identity is read from the report of a preceding
  probe invocation of the same endpoint that receives only the client
  hello; the identity is deterministic, so the trial invocation opens its
  session with it.
- The client hello is the endpoint's own offer (`sley hello`, decoded with
  `sley frame decode`), so the negotiated profile is the full offer.
- The disposable repository is seeded through the endpoint by
  `exchange.import` of the arm fixture's exchange bytes; the runner writes
  no repository file itself and reads none. The runner then opens the
  session with the handshake identity and closes it after the agent
  returns; the agent never sees an identity.
- Every frame in either direction is recorded in order (section 2) before
  the next request is written.

## 2. Privileged-context guard

The agent adapter receives exactly one object, the endpoint handle, whose
only operations are:

```text
exchange(frame_object) -> [frame_object]   // events then the response
affordances() -> [method_name]             // the negotiated method names
```

The agent's `frame_object` names only `method`, `body` (hex), and an
optional `cancel` flag; the runner issues the session and request
identifiers and builds the frame. It receives no repository path, file
handle, environment, clock, random source, task fixture bytes, or trace.
The runner owns all of those. A handle that exposes anything else, or an
agent request that names any other field (a session, an identifier, a
kind), is `SLEY2_TRIAL_PRIVILEGED_CONTEXT`, and the trial is recorded as a
harness failure; a request naming a method outside the affordances or a
body that is not hex is `SLEY2_TRIAL_FRAME_INVALID`.

The context the agent was **offered** is exactly the recorded responses. This
is a cooperative-adapter trust boundary, not a capability boundary, and the
earlier claim that "nothing else existed" was false: one Python process cannot
withhold itself from code running inside it. A bound method carries its
defining module's globals, and a closure carries its cells, so an adapter that
reflects reaches both. Two things follow, and both are contract:

1. `EndpointHandle` is defined in its own module that imports nothing
   privileged, so `reflected_privileged_names` is empty and stays empty: the
   runner's endpoint, subprocess, filesystem, and trace names are not reachable
   that way. A handle whose class can see them is
   `SLEY2_TRIAL_PRIVILEGED_CONTEXT`, checked per trial rather than asserted.
2. The exchange closure's own cells remain reachable. That residual is
   irreducible in one process, because the callable must hold what it needs to
   answer. It is stated here rather than denied, and an arm whose adapter is
   not trusted to decline it must run the adapter in its own process.

An arm's result therefore rests on the adapter being cooperative, and the trace
is what makes a breach visible after the fact rather than impossible.

The guard raises into the agent's frame, so an adapter can catch it. It
therefore records a `guard_refusal` record **before** raising and remembers that
it fired: a trial whose guard fired is a harness failure even if the adapter
returned normally. Swallowing the exception changes nothing except that the
adapter also wasted its turn.

`context_bytes` counts every body the agent received. Counting only `capsule`
and `query.*` understated the arm under test, because `refs.list`,
`handle.expand`, `candidate.inspect`, `compare`, `revision.read`, `execute`,
`report` and `diagnostics` bodies are context the agent read exactly as a
capsule is, and master goal 21.6 forbids unreported extra context.

`derive_context_breakdown` records how that total divides (`capsule_bytes` and
`non_capsule_context_bytes`) in this work package's own evidence. It is
deliberately **not** added to the benchmark plan's metric field set: that set is
an S20-610 cross-arm fairness control, and one arm widening it is the shared
control drift S20-610 rejects. An arm that needs the split as a reported metric
must have S20-610 amend the plan.

`invalid_candidates` and `repair_loops` are S20-360 candidate verdicts, and the
trace cannot see them. They used to be derived from SMP1's `failed` flag, which
marks a ProtocolFailure envelope: a request that did not complete, not a
candidate found invalid. A candidate that validates as invalid returns a
successful response whose body carries the verdict, so the derivation read zero
invalid candidates whenever candidates were genuinely invalid, always in the
arm's favour. Both are oracle-owned. The flag is still counted, as
`protocol_failures` in this package's context breakdown, which is what it
measures.

Every injected metric has exactly one source, and the two sets do not overlap.
`model_input_tokens` and `model_output_tokens` come from the adapter's
observation, because only the adapter knows its own usage. The correctness and
resource metrics (`stale_candidates`, `stale_candidates_incorrectly_accepted`,
`collateral_semantic_changes`, `invalid_committed_states`, `peak_memory`,
`canonical_storage_bytes`, `pack_bytes`, `execution_latency`) come from the
oracle's judgement only. An arm never reports its own correctness or cost: the
observation used to take precedence over the judgement, which let the arm under
test grade itself and report zero against its own interest. A value that is not
a non-negative integer is zero, so a hostile self-report cannot become a score.

## 3. Complete trace

Each trial writes one append-only file `<run>/sley2/<trial_id>.trace.jsonl`
under contract `sley2.sley2-trial-trace.v1`. Records are canonical JSON
(S20-610 rules) with `previous_record_digest` and `record_digest =
SHA256("sley2.sley2-trial-trace.v1\0" || canonical_json(record without
record_digest))`; the first record's previous digest is the run manifest
digest.

```text
header  { kind: "header", trial_id, run_manifest_digest, task_id, seed,
          fixture_digest, endpoint_sha256, endpoint_version, handshake_id }
guard_refusal { kind: "guard_refusal", seq, code, detail }
frame   { kind: "frame", seq, direction: "request"|"response"|"event",
          frame: Frame, frame_sha256 }
footer  { kind: "footer", frames_recorded, report: Report (the endpoint's
          report copied verbatim), outcome: "completed"|"harness_failure",
          failure_code: integer|null }
```

The trace is complete: no frame is omitted, reordered, or rewritten; a
runner failure appends the footer with its code and closes the trace.
Trace-derived quantities are computed only from frame records:

| Metric | Derivation |
|---|---|
| `tool_calls` | session-scoped request frames other than the runner's `session.close` |
| `context_bytes` | body bytes of every response and event the agent received, whatever the method |
| `entities_inspected` | sum of `bounds.returned_entities` over responses |
| `relationships_inspected` | sum of `bounds.returned_edges` over responses |
| `compile_or_check_attempts` | `candidate.validate` requests |
| `repair_loops` | failed `candidate.validate` responses |
| `invalid_candidates` | failed `candidate.validate` responses |
| `attempted_tasks` | 1 |
| `files_inspected` | 0 (no file exists in this arm) |
| `human_interventions` | 0 |

Every other frozen metric of `bench/benchmark-plan.json` (tokens,
strict correctness, accepted changes, stale outcomes, collateral changes,
wall time, peak memory, storage and pack bytes, execution latency) is an
injected adapter claim with status `UNVERIFIED_ADAPTER_CLAIM`;
`accepted_change_tokens` is `null` for S20-630.

## 4. Trial claims

One claim per trial is appended to `<run>/sley2/claims.jsonl` under
contract `sley2.sley2-trial-digest-claim.v1` with the S20-610 mechanics
(create-once run manifest, exclusive lock, full-chain verification,
`O_APPEND`, fsync, no rewrite or deletion). A claim binds run, trial, arm
`sley_2_0`, task, seed, start and end UTC seconds from the injected clock,
outcome, the trace head digest, the report digest, the fixture and
exchange digests, and all 25 frozen metrics. Duplicate trial identifiers or
task/seed pairs fail closed; complete verification requires exactly the
manifest's task/seed product. Every claim carries status
`UNVERIFIED_INJECTED_DIGEST_CLAIMS`: the chain proves bytes and ordering,
not that a model ran or an oracle judged.

## 5. Smoke and control audit

`make sley2-runner-smoke` builds the `sley` binary, runs one scripted
trial (a scripted agent that asks `session.capabilities`, `refs.list`,
`handle.expand`, and `session.budgets`; no model, no task) and one
intruding trial (an agent naming a session field, refused as
`SLEY2_TRIAL_PRIVILEGED_CONTEXT`) over the frozen S20-540 exchange fixture
in a private run directory, verifies both trace chains, the handle
surface, and both claims, and writes evidence under
`evidence/runtime/s20-620-sley2-smoke/`. `scripts/check_sley2_trial_runner.py`
in `make quick` audits the contract markers, the runner's dependency
surface (the `sley` binary and the S20-610 module only; no kernel crate,
no repository file access, no provider client), the exact two-operation
endpoint handle, and the offline tests.

## 6. Stable failures

| Numeric | Symbolic code |
|---:|---|
| 62000 | `SLEY2_TRIAL_MANIFEST_INVALID` |
| 62001 | `SLEY2_TRIAL_ENDPOINT_UNAVAILABLE` |
| 62002 | `SLEY2_TRIAL_HANDSHAKE_FAILED` |
| 62003 | `SLEY2_TRIAL_FRAME_INVALID` |
| 62004 | `SLEY2_TRIAL_TRACE_INVALID` |
| 62005 | `SLEY2_TRIAL_CLAIM_INVALID` |
| 62006 | `SLEY2_TRIAL_PRIVILEGED_CONTEXT` |
| 62007 | `SLEY2_TRIAL_DUPLICATE` |
| 62008 | `SLEY2_TRIAL_TIMEOUT` |
| 62009 | `SLEY2_TRIAL_INTERNAL_INVARIANT` |

## 7. Required evidence

- offline tests: trace chain construction and verification, the guard
  refusing a foreign session or identifier and a handle with extra
  operations, metric derivation from a recorded trace, claim validation and
  duplicate refusal, and a scripted trial over a fake endpoint;
- the smoke evidence over the real `sley` binary and the exchange fixture;
- `scripts/check_sley2_trial_runner.py` in `make quick`;
- Tier 1 plus Tier 2 validation, and the Ariadne, Nabu, and Vulcan reviews
  with every report-grade finding closed.

## 8. Explicit exclusions

This contract does not claim: model or provider execution; a real trial;
the raw and legacy arms; Accepted Change Tokens and accounting (S20-630);
statistics and trial sets (S20-640); succession thresholds; artifact
provenance; publication; runtime, packaging, release, or GA.

## 9. Revision 2 clarifications

- The shared S20-610 manifest's `execution_mode: offline_injected` and
  `external_command_policy: forbidden` are **S20-610 shared run-level
  controls** and are defined there, not here. This work package does not
  narrow, restate, or except them: an earlier revision did, which is
  shared-control drift whether or not the words agree. Under the S20-610
  definition, this arm's endpoint binary is its measured artifact rather than
  an external command, and the arm satisfies the conditions S20-610 attaches:
  the endpoint is frozen by digest in the run manifest, its SHA-256 is in the
  trace header and every claim as `endpoint_sha256`, no other command is
  invoked for any purpose, and the endpoint is this arm's declared subject.
- The client hello is `sley hello --protocol-profile v2-capable` decoded
  with `sley frame decode`. The
  affordances are **not** that hello's `methods`: the endpoint offers all 41
  SMP1 methods, `exchange.export` among them, and an arm holding an
  entire-store dump is what master goal 20.10 forbids. The arm's affordances
  are the frozen `ARM_AFFORDANCES` allowlist, and `arm_affordances_digest` is a
  run control carried in every claim, so a run that widened the arm's reach is
  visible in the record rather than inferred from the binary. The allowlist
  holds eighteen names: the sixteen version 1 methods plus `entity.version`
  and `entity.signature`. Those two require a version 2 endpoint offer
  (SLEY_CLI_V1 section 9 profile): under a version 1 offer the handshake
  fails exactly as for any unoffered name. A name the
  allowlist claims that the endpoint does not offer is
  `SLEY2_TRIAL_HANDSHAKE_FAILED`, so drift in either direction stops the run.
- `tool_calls` counts the agent's session-scoped requests and excludes the
  runner's `session.close`; seeding and opening carry no session and are
  not counted either.
- The scripted smoke attempts no task, so its oracle claim is `rejected`
  with `SLEY2_SMOKE_NO_TASK_ATTEMPTED`; a capsule round trip joins the
  script once a request builder is reachable through the endpoint.
- A claim carries `arm_affordances_digest`, the canonical digest of the
  allowlist the trial ran under: the trial snapshots its affordance list
  once into an immutable tuple before execution, and the guard, the
  handle, and the claim digest all use that snapshot, so a mutated input
  list cannot split exercised admission from the claimed digest.
- A claim's `handshake_id`, `report_digest`, `model_output_digest`, and
  `oracle_report_digest` may be null only for timeouts and harness
  failures; `trace_record_count` is at least two (header and footer).
- Unexpected adapter exceptions are recorded as
  `SLEY2_TRIAL_INTERNAL_INVARIANT` harness failures; the trace footer and
  the claim are still written.

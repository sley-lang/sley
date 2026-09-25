# ADR-0036: the Sley 2 arm as an endpoint-only, trace-complete runner

Status: proposed; the S20-620 contract is a draft at revision 8
(2026-09-23: `workspace.open` admitted as the accepted-head opener at
revision 5, nineteen-name allowlist; revision 6 names the section 9 text
that answered the revision 5 REVISE round and passed in all three lanes;
revision 7 re-pinned SMP1 revision 15 and S20-300 revision 6 (Vulcan PASS,
Ariadne and Nabu REVISE at 26d050e); revision 8 corrects the
absent-boundary sentence, pins SMP1's errata-only revision 16, and binds
completion to verdicts scoped to the reviewed text, with its new-delta
review pending);
implemented at
`bench/sley2/runner.py` with 23 offline tests and a smoke over the real
endpoint. (Revision 2 record: implemented 2026-09-03 with five offline
tests.)

Date: 2026-09-03

## Context

The master goal's benchmark requires a Sley 2.0 arm in which an agent
receives only an SMP1 context capsule and mutation affordances, with every
prompt, protocol definition, output, tool call, candidate, root, receipt,
and failure preserved. S20-610 froze the raw arm's offline mechanics (a
create-once manifest and an append-only digest chain of unverified claims)
after Nabu's review. S20-430 now provides the endpoint through which the
Sley 2.0 arm can be driven without a second code path.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **Endpoint only.** The runner drives the `sley` binary in JSON mode
   and computes no protocol digest; the handshake identity comes from a
   probe invocation's report.
2. **Two-operation handle.** The agent sees `exchange` and `affordances`
   and nothing else; any other access is a recorded harness failure.
3. **Trace before progress.** Every frame is chained into the trace before
   the next request is written, so the trace is the only context the agent
   could have had.
4. **Trace-derived versus injected.** Ten metrics are computed only from
   frame records; the rest are unverified adapter claims, as in S20-610.
5. **Same chain mechanics.** Claims reuse the S20-610 module's manifest,
   canonical JSON, locking, and chain verification rather than a second
   implementation.
6. **Codes.** Ten `SLEY2_TRIAL_*` codes 62000 through 62009.
7. **Staging.** `scripts/check_sley2_trial_runner.py` binds the contract,
   ADR, work-package row, and summary section, and fails closed if the
   runner appears before the summary allows it.

## Consequences

- A real trial needs only injected adapters and operator approval; no
  runner code changes.
- The privileged-context risk named by the work package is mechanically
  audited where a mechanism exists, and stated where none does. The declared
  handle surface and the reflected privileged-name set are both checked per
  trial; the exchange closure's cells are reachable by any adapter that
  reflects, which one process cannot prevent. Calling the whole thing
  "mechanically audited" was wrong: three reviewers reached the live session
  and the runner's own module through `__closure__` and `__globals__`. An arm
  that cannot trust its adapter must isolate it in a process, and the trace
  exists so that a breach is visible afterwards.

# S20-620 Sley 2 Trial Runner Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Ariadne contract review, Nabu architecture
review, and Vulcan surface review queued.

## Frontier at start

- S20-430 provides the `sley` endpoint (byte and JSON modes, per-frame or
  batch, counting report); S20-420 the JSON form; S20-320 the master
  capsule; S20-540 a frozen exchange fixture that seeds a repository
  through `exchange.import`.
- S20-610 froze the raw arm's offline mechanics: one create-once run
  manifest naming all three arms, canonical JSON, and an append-only digest
  chain of explicitly unverified claims with injected adapters.
- The local completion frontier names S20-620 as the next authority-safe
  package and its guard blocks `bench/sley2` until the summary allows it.

## Design brief

Contract: `docs/spec/SLEY2_TRIAL_RUNNER_V1.md`, ADR-0036, stage checker
`scripts/check_sley2_trial_runner.py`.

- One trial is one `sley serve --json` process in per-frame mode; the
  handshake identity comes from a probe invocation's report; the
  repository is seeded through the endpoint.
- The agent adapter receives a two-operation handle (`exchange`,
  `affordances`) and nothing else; any other access is a recorded harness
  failure.
- A complete, digest-chained trace of every frame precedes every next
  request; ten metrics derive only from frame records, the rest are
  injected unverified claims.
- Trial claims reuse the S20-610 chain mechanics under the `sley_2_0` arm.
- Codes 62000 through 62009.

## Open questions for the reviews

- Whether the probe invocation for the handshake identity should be
  replaced by an endpoint command that prints it, given SMP1 section 2's
  wording that the selection travels in the server hello.
- Whether `context_bytes` should count every response body or only the
  capsule and query bodies.
- Whether the scripted smoke agent should also exercise a candidate
  round trip once a public candidate builder exists.

## Records

| Stage | Commit | Tier 1 | Notes |
|---|---|---|---|
| Contract draft revision 1 | pending | pending | ADR-0036, stage checker |

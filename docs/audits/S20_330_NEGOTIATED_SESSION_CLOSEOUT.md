# S20-330 Negotiated Session and Handle Closeout

Status: **implemented under the draft Negotiated Session and Handle Profile v1 contract (revision 3); Council re-reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03; revision 2 implemented 2026-09-05; revision 3 2026-09-05

Validation tier: **Tier 1 plus protocol-focused Tier 2 handoff**

## Claim under review

A session is a server-issued binding of one negotiated handshake to one
workspace, one verified root, and one schema epoch, identified under the
thirty-fifth domain `sley2.session.v1` over a per-instance server nonce,
the handshake identity, the workspace, the accepted head root, the epoch,
and the issuance ordinal, so equal inputs under one live instance issue
equal identities while twin instances and restarts never share an
identity space and every pre-restart name is unknown. Every request is
checked against its session in true contract order (remembered close,
live existence, admission, workspace, epoch, budget, and for the closed
head-bound set the accepted head root) before any engine runs; renewal
rebinds explicitly within one epoch and never implicitly; a handle is the
pair of a binding position and its expected root and resolves only under
that session and root, staying stale after renewal while naming the old
root; a capsule built under a session carries the `Negotiated` arm with
the session identity and refuses foreign provenance; live sessions are
capped at the negotiated `max_sessions` and remembered closes at the
same cap, so no session state grows without bound. The contract is
`docs/spec/SESSION_HANDLE_PROFILE_V1.md` with ADR-0033. It is a draft:
the 2026-09-04 Nabu, Ariadne, and Vulcan reviews landed six P0s and 42
P1, P2, and P3 items; revision 2 closed the P0s and the freeze-blocking
P1s, revision 3 answers every remaining item, and the re-reviews that
freeze it and complete the package are pending and must pass before the
status above changes.

The implementation provides:

- `sley-id`: the thirty-fifth domain `sley2.session.v1`, `SessionId`, and
  its frozen vector (unchanged: the domain string never moved);
- `sley-protocol`: `session.rs` with `SessionAuthority`
  (`fresh_server_nonce`, `open_session`, `renew_session`,
  `close_session`, `check_session`, `expand_handle` with the expected
  root, `bind_context_capsule`), `SessionRecord`, `HeadBinding`,
  `HandleFacts`, `derive_session_id` over the nonce preimage,
  `MAX_SESSION_RENEWALS`, and `SessionErrorCode` with the eight codes
  33000 through 33007; `RequestRegistry` with the remembered-close cap
  and `is_closed`; `LimitProfile.max_sessions` as the eighth negotiated
  limit field; the server issues sessions through the authority in true
  precedence, gates the sessionless genesis exemption on the absence of
  a head, checks renewal and close like every other method, verifies
  the renew body names the frame session, dispatches the closed
  head-bound set, and expands handles with the expected root;
- `sley-json-bridge`: the eighth limit field in both directions;
- conformance: SMP1 hello/selection vectors with eight-field limits,
  bridge round-trip vectors with eight-key limits, oracles reproducing
  both.

## Evidence

- Contract draft revision 2 and ADR-0033 revision 2; SMP1 revision 10
  (eighth limit field, expected-root `handle.expand` request owned by
  the S20-330 freeze); bridge revision 5 (eighth limit key);
  `ERROR_CODES_V1.md` freezing 33000 through 33007 per row;
  `THREAT_REGISTER.md` with the T15 owner corrected to `sley-protocol`
  and the T56 row for cross-caller live-name use.
- Native tests: the authority matrix (issuance binding head and nonce
  separating instances, the live-session cap, ordered checks, handles
  naming their root across renewal, exact retention, exact renewal
  limit), the registry matrix (admission, remembered closes, the
  close-cap forgetting the oldest name), and the server matrix (twin
  inequality with cross-instance refusal, the T15 lifecycle with the
  old-root handle staying stale after renewal, the closed head-bound
  set refusing branch reads under a stale session, the renew-body
  check, the unknown/closed precedence pair, the sessionless genesis
  gate, the capsule arm, the cap with slot freeing, restart forgetting,
  and binding-before-budget precedence).
- Threat matrices recorded at `evidence/security/T15/`, `T47/`, and
  `T56/`.
- Tier 1: `make quick` green at the commit.
- Tier 2: see the validation record below.

## Findings closed in flight

- Revision 1 reached for cross-server determinism and could not hold
  its authority rule: the handshake carries no randomness, so a
  deterministic identity is computable from public head state. Revision
  2 mints a per-instance nonce and narrows the rule to what the
  mechanism provides; peer isolation is a recorded transport
  obligation, not a silent gap.
- `SESSION_UNKNOWN` was unreachable because admission ran first;
  liveness now precedes admission and the budget follows the binding
  checks, so the documented order is the implemented order.
- The sessionless exemption covered the head-advancing
  `exchange.import` on headed repositories; it now ends at the first
  head.
- Handles resolved across renewal because only the bound root was
  compared; the expected root travels in the request now.
- The contract floated over SMP1 revision 8 and capsule revision 2
  while both moved; revision 2 pinned SMP1 revision 10 and capsule
  revision 3, and SMP1 then moved to 11 under it. Revision 3 pins SMP1
  revision 11 and the stage checker reads both authorities' own status
  lines, so the pin fails the moment either moves.
- The revision 2 head-bound enumeration named four methods under wrong
  tags and called the non-mutating `candidate.*` methods mutating.
  Revision 3 classifies every frozen tag into one list and the checker
  compares the lists against the server dispatch table and the SMP1
  method table.
- `renewals` is a `u16`, the range it always had; the checker binds
  the capsule module and the threat-matrix test names it never read.

## Explicitly open and deferred

- **Council re-reviews.** Nabu, Ariadne, and Vulcan re-reviews land as
  the freeze; the campaign record lists the answered design questions
  (explicit renewal only, no cursor handles, no request-count expiry
  with the session-count cap instead).
- The root-advance matrix replaces the repository on disk with another
  root of the same workspace, because no public candidate builder exists to
  commit through the server in tests; the check is exact regardless of how
  the head moved.
- Epoch mismatch is exercised at the authority level only: every trusted
  genesis carries the one frozen conformance epoch.

## Validation record

Tier 1 `make quick` passed at the commit. Tier 2 ran at the commit
(`make core`, `make conformance`, `make adversarial`, `make fuzz-smoke`,
`make smp1-persistent-fuzz-smoke`, `make context-capsule-persistent-fuzz-smoke`)
and is recorded in
`machineresearch/sley-2.0/s20-330-negotiated-session-campaign-2026-09-03.md`.
The full `make v1` gate was skipped because this is a subsystem handoff,
not a release boundary; `make v2` and `make release-check` remain
intentionally fail closed.

## Independent review

Nabu architecture review, Ariadne contract review, and Vulcan surface
review of 2026-09-04: FAIL with six P0s, closed by revision 2.
Re-reviews pending. Sessions and verdicts are recorded here when they
land.

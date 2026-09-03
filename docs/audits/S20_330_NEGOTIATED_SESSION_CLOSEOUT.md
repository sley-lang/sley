# S20-330 Negotiated Session and Handle Closeout

Status: **implemented under the draft Negotiated Session and Handle Profile v1 contract (revision 1); Council reviews pending, so the package is not complete; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus protocol-focused Tier 2 handoff**

## Claim under review

A session is a server-issued binding of one negotiated handshake to one
workspace, one verified root, and one schema epoch, identified under the
thirty-fifth domain `sley2.session.v1` over the handshake identity, the
workspace, the accepted head root, the epoch, and the issuance ordinal, so
equal servers over equal repository state issue equal identities. Every
request is checked against its session in contract order (existence,
workspace, epoch, and for head-bound methods the accepted head root)
before any engine runs; renewal rebinds explicitly; a handle is the
binding position of an entity in the session's bound root and resolves
only under that session and root; a capsule built under a session carries
the `Negotiated` arm with the session identity and refuses foreign
provenance. The contract is `docs/spec/SESSION_HANDLE_PROFILE_V1.md` with
ADR-0033. It is a draft: every Council lane was unavailable when it was
written and when the implementation landed, so the Nabu, Ariadne, and
Vulcan reviews that freeze it and complete the package are pending and
must pass before the status above changes.

The implementation provides:

- `sley-id`: the thirty-fifth domain `sley2.session.v1`, `SessionId`, and
  its frozen vector;
- `sley-protocol`: `session.rs` with `SessionAuthority` (`open_session`,
  `renew_session`, `close_session`, `check_session`, `expand_handle`),
  `SessionRecord`, `HeadBinding`, `HandleFacts`, `derive_session_id`,
  `MAX_SESSION_RENEWALS`, and `SessionErrorCode` with the eight codes
  33000 through 33007; the server issues sessions through the authority,
  checks every request in contract order, dispatches `handle.expand`
  (304), and lets `workspace.create` and `exchange.import` travel without a
  session because a headless repository cannot bind one (SMP1 revision 5);
- `sley-query`: `build_context_capsule_bound` and the `Negotiated` arm of
  the capsule record (S20-320 full revision 2), with `ContextCapsule::session`.

## Evidence

- Contract draft revision 1 and ADR-0033 at `c018cc8` (guard fix
  `3f598b4`); implementation at `a0c9a70`.
- Native tests: three authority tests (deterministic issuance binding the
  head, the ordered check matrix with T47 first and the T15 handle
  lifecycle across a root advance and renewal, the exact renewal limit),
  one capsule test (the negotiated arm carries the session and foreign
  provenance is refused), and one server test (equal issuance from a twin
  server, a positional handle expanded under the bound root and unknown
  past the inventory, `SESSION_STALE_HANDLE` and `SESSION_ROOT_ADVANCED`
  after the repository's head is replaced by another root of the same
  workspace, explicit-transaction methods unaffected, renewal rebinding
  and handles resolving again, `SESSION_WORKSPACE_MISMATCH` before any
  other check when a repository of another workspace stands at the path,
  the capsule carrying the session, and an unknown session refused);
  `sley-protocol` 15 tests, `sley-query` 61 tests, `sley-id` 7 tests pass.
- The SMP1 method table now dispatches 33 methods; four stay reserved
  (305, 503, 601, 602) and four deferred on owner gaps.
- Tier 1: `make quick` green at the commit.
- Tier 2: see the validation record below.

## Findings closed in flight

- A repository without an accepted head cannot bind a session, so the two
  methods that create a head (`workspace.create`, `exchange.import`) travel
  without a session; the first draft required a session for every method
  and could not create a workspace at all.
- `handle.expand` reports `SESSION_STALE_HANDLE` rather than
  `SESSION_ROOT_ADVANCED` when the root moved, as threat T15 names it; the
  head-bound check list excludes it for that reason.

## Explicitly open and deferred

- **Council reviews.** Nabu (who deferred the package until authority
  existed), Ariadne, and Vulcan reviews land as contract revisions; the
  campaign record lists the open questions (implicit rebinding after a
  session's own commit, cursor handles, request-count expiry).
- The root-advance matrix replaces the repository on disk with another
  root of the same workspace, because no public candidate builder exists to
  commit through the server in tests; the check is exact regardless of how
  the head moved.
- Epoch mismatch is exercised at the authority level only: every trusted
  genesis carries the one frozen conformance epoch.

## Validation record

Tier 1 `make quick` passed at the commit. Tier 2 ran on 2026-09-03 at
`a0c9a70` (`make core` 965 tests, all gates exit 0 in 41 seconds) and is
recorded in
`machineresearch/sley-2.0/s20-330-negotiated-session-campaign-2026-09-03.md`.
The full `make v1` gate was skipped because this is a subsystem handoff,
not a release boundary; `make v2` and `make release-check` remain
intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

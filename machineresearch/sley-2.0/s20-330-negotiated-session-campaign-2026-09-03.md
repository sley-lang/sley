# S20-330 Negotiated Session Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Nabu architecture review (the package was
deferred by Nabu's earlier review until authority existed), Ariadne
contract review, and Vulcan surface review queued.

## Frontier at start

- SMP1 negotiates a digested profile and scopes request identity per
  session (S20-400 draft revision 4, S20-410 and S20-440 implemented).
- The S20-410 server issues provisional session identities under the
  handshake domain; the S20-320 full capsule reserved `Negotiated(2)`.
- The threat register names T15 (`SESSION_STALE_HANDLE`) and T47
  (`SESSION_WORKSPACE_MISMATCH`) for this package; the summary records the
  required binding (negotiated session, workspace, verified root, epoch)
  and the forbidden substitutes.

## Design brief

Contract: `docs/spec/SESSION_HANDLE_PROFILE_V1.md`, ADR-0033, stage
checker `scripts/check_session_handle_profile.py`.

- Thirty-fifth domain `sley2.session.v1`; the identity digests the
  handshake, workspace, accepted head root, epoch, and issuance ordinal.
- Request checks in order: session exists, workspace matches (T47), epoch
  matches, and for head-bound methods the accepted head equals the bound
  root; renewal rebinds explicitly.
- Handles are the binding positions of the bound root: exact, stateless,
  stale after any root advance (T15), unknown past the inventory.
- The capsule's `Negotiated` arm carries the session identity and the
  builder refuses foreign provenance.
- Codes 33000 through 33007.

## Open questions for the reviews

- Whether mutation methods should rebind the session implicitly after a
  successful commit instead of requiring an explicit renewal.
- Whether handles should also name query cursors or only entities.
- Whether a session should expire by request count in addition to the
  renewal limit and the S20-440 budget.

## Implementation under the draft (2026-09-03)

- `sley-id` domain 35 `sley2.session.v1`; `crates/sley-protocol/src/session.rs`
  (`SessionAuthority`, positional handles, eight codes); the server issues,
  checks, renews, closes, and expands handles; `workspace.create` and
  `exchange.import` travel without a session (SMP1 revision 5); the capsule
  gains `build_context_capsule_bound` (S20-320 revision 2).
- Closeout `docs/audits/S20_330_NEGOTIATED_SESSION_CLOSEOUT.md`; summary
  status `S20_330_IMPLEMENTED_REVIEW_PENDING`; frontier re-anchored to the
  S20-420 JSON bridge.

Implementation commit: `a0c9a70`.

## Tier 2 handoff gate (2026-09-03, at `a0c9a70`)

| Gate | Result |
|---|---|
| `make core` | PASS (965 tests) |
| `make conformance` | PASS |
| `make adversarial` | PASS |
| `make fuzz-smoke` | PASS |
| `make smp1-persistent-fuzz-smoke` | PASS |

Total 41 seconds. `make v1` skipped: subsystem handoff, not a release
boundary. Council reviews remain pending; the package is not complete.

## Commits

- contract draft revision 1: `c018cc8` (guard fix `3f598b4`).

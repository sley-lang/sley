# S20-330 Negotiated Session Campaign (2026-09-03)

Status: contract draft revision 2 written and implemented 2026-09-05;
2026-09-04 Nabu, Ariadne, and Vulcan reviews landed six P0s and the
freeze-blocking P1s; Council re-reviews queued.

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

## Review answers recorded (2026-09-05, contract revision 2)

- **Implicit rebinding after commit: no.** All three reviewers agree
  explicit renewal is the correct model; silent rebinding would
  revalidate handles and capsules against a root the caller never
  accepted. Recorded in contract section 2 with the rationale.
- **Cursor handles: no.** A cursor is continuation state and would
  destroy the statelessness that makes T15 structural; cursors belong in
  `query.continue`'s `after`. Recorded in contract section 4 and
  section 8.
- **Request-count expiry: not needed.** The S20-440 budget bounds a
  session's work; the missing bound was the concurrent session count,
  closed by the negotiated `max_sessions` cap plus the remembered-close
  cap. Recorded in contract section 8.
- **Identity-domain choice (Ariadne P0-3 routed to Nabu): unpredictability
  within per-instance determinism.** The handshake carries no randomness,
  so twin handshakes are byte-identical and a deterministic identity is
  computable from public head state; determinism across servers was
  consumed by nothing but the twin test. The authority mints a
  per-instance nonce into the preimage; peer isolation is a transport
  obligation on the S20-420/430 boundary, tracked as threat T56.
  Recorded in ADR-0033 and contract sections 1 and 8.

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

## Revision 2 (2026-09-05): the review round closed

- Per-instance server nonce in the identity preimage; twin inequality
  and restart forgetting replace cross-server determinism.
- True dispatch precedence (remembered close, live existence, admission,
  workspace, epoch, budget, bound root); `SESSION_UNKNOWN` reachable on
  the wire; budget follows binding.
- Sessionless genesis only: the exemption ends at the first head.
- `handle.expand` carries the expected root; old-root handles stay stale
  after renewal; the head-bound set is closed over fourteen methods.
- Negotiated `max_sessions` (eighth limit field, SMP1 revision 10,
  bridge revision 5) caps live sessions; remembered closes capped FIFO
  at the same number.
- `SESSION_BINDING_INVALID` enumerated; headless open travels under the
  S20-390 loader owner code; `ERROR_CODES_V1.md` freezes 33000-33007 per
  row; T15 owner corrected; T56 recorded with evidence.
- Contract pins SMP1 revision 10, capsule revision 3, STATE_ROOT S20-160
  normative; the stage checker binds server and registry markers,
  revision pins, and per-code numerics, and fails closed on drift.

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
- contract draft revision 2: the S20-330 revision 2 commit on `main`
  (see `git log --oneline`, closeout revision 2 for the file set).

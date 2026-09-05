# ADR-0033: Negotiated session, binding checks, and positional handles

Status: proposed; the S20-330 contract is a draft at revision 2 with
Council re-reviews pending; implemented under the draft
(`docs/audits/S20_330_NEGOTIATED_SESSION_CLOSEOUT.md`)

Date: 2026-09-03; revision 2 decision record 2026-09-05

## Context

S20-330 was deferred until a negotiated session, a verified workspace and
root, and a live epoch binding existed, because a claimed root or a
restricted capsule digest must never substitute for that authority (ADR-0013,
the T15 gate). SMP1 (S20-400) now negotiates a digested profile and scopes
request identity per session, the S20-410 server issues provisional
session identities, and the S20-320 full capsule fills its `Negotiated`
arm through the session authority.

The 2026-09-04 review round (Nabu architecture, Ariadne contract, Vulcan
surface) returned six P0s: two `SESSION_*` codes did not mean what the
contract said, the contract floated over moving drafts, the authority
rule was false as written for a deterministic identity, the sessionless
exemption covered a mutation unconditionally, positional handles survived
renewal, and the identity was a public-state digest. The Council lanes
were available for the reviews; the revision below is the integrator's
answer, submitted for re-review.

## Decision

1. **A session binds a handshake to one workspace, root, and epoch,
   under a per-instance nonce.** The thirty-fifth domain
   `sley2.session.v1` digests the server nonce, the handshake identity,
   the workspace, the accepted head root, the epoch, and the issuance
   ordinal. Equal inputs under one live instance issue equal identities;
   twin instances and restarts never share an identity space, and a
   restart answers `SESSION_UNKNOWN` to every pre-restart name.
2. **The identity-domain choice is unpredictability, keeping
   determinism only within one instance.** Ariadne routed this choice to
   architecture. Verification showed the attack is real: the handshake
   carries no randomness, so twin handshakes are byte-identical and a
   deterministic identity is computable from public head state by any
   caller. Determinism across servers was consumed by nothing but the
   twin-server test: no fixture bakes a session identity, GC pins are
   in-memory, and the capsule arm carries the identity opaquely. The
   renewal keeps the issuance identity (a renewal counter in the digest
   would invalidate the name every client, registry entry, and budget
   holds, for no security gain: binding checks are live-state). The
   residual is recorded, not hidden: a live name is usable by any caller
   that learns it, peer isolation is a transport obligation on the
   S20-420/430 boundary, and threat T56 tracks it.
3. **Every request is checked against the binding in true precedence.**
   Frame validity, remembered close, live existence, request-identity
   admission, workspace, epoch, budget, then the bound root for the
   closed head-bound set. `SESSION_UNKNOWN` is reachable on the wire;
   the budget never masks a binding failure.
4. **Handles are positions under an expected root, not bare numerals.**
   `handle.expand` carries `uvar(handle) || StateRoot[32]`; the server
   refuses any expected root but the session's bound root and any bound
   root but the head. Renewal leaves every old-root handle stale by
   construction, which is what makes T15 structural rather than
   enforced.
5. **Genesis is the only sessionless state.** The create/import
   exemption holds only while no accepted head exists; once a head
   exists both methods require a session. Live sessions are capped at
   the negotiated `max_sessions`, remembered closes at the same cap in
   close order: no session state grows without bound, with no clock.
6. **Codes.** Eight `SESSION_*` codes 33000 through 33007 travel in the
   SMP1 failure envelope, frozen by `ERROR_CODES_V1.md` with per-row
   meanings; `SESSION_BINDING_INVALID` is the enumerated
   authority-cannot-bind bucket.
7. **Staging.** `scripts/check_session_handle_profile.py` binds the
   contract, ADR, work-package row, summary section, ERROR_CODES rows,
   and the server implementation markers, cross-checks the SMP1
   revision 10 and capsule revision 3 pins, and fails closed on drift.

## Consequences

- S20-420 and S20-430 transport and display a real session identity;
  the bridge renders the eighth limit field, and peer isolation is
  their transport obligation (threat T56).
- SMP1 revision 10 records the eighth limit field and the expected-root
  `handle.expand` request the S20-330 revision 2 freeze owns; no other
  SMP1 body changes.
- The T15, T47, and T56 matrices are exact server tests with recorded
  evidence artifacts, not narrative gates.

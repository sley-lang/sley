# ADR-0033: Negotiated session, binding checks, and positional handles

Status: proposed; the S20-330 contract is a draft at revision 1 with
Council review pending; implemented under the draft
(`docs/audits/S20_330_NEGOTIATED_SESSION_CLOSEOUT.md`)

Date: 2026-09-03

## Context

S20-330 was deferred until a negotiated session, a verified workspace and
root, and a live epoch binding existed, because a claimed root or a
restricted capsule digest must never substitute for that authority (ADR-0013,
the T15 gate). SMP1 (S20-400) now negotiates a digested profile and scopes
request identity per session, the S20-410 server issues provisional
session identities, and the S20-320 full capsule reserved its `Negotiated`
arm. The threat register names T15 (handle reuse across roots) and T47
(cross-workspace leakage) for this package.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Nabu, Ariadne, and Vulcan as
soon as a lane returns.

## Decision

1. **A session binds a handshake to one workspace, root, and epoch.** The
   thirty-fifth domain `sley2.session.v1` digests the handshake identity,
   the workspace, the accepted head root, the epoch, and the issuance
   ordinal, so equal servers over equal state issue equal identities.
2. **Every request is checked against the binding.** Existence, workspace,
   epoch, and (for head-bound methods) the accepted head root are checked
   in that order before any engine runs; a mutation leaves the session
   bound to the previous root until an explicit renewal.
3. **Handles are positions, not allocations.** A handle is the zero-based
   binding position of an entity in the session's bound root; it resolves
   only under that session and root, so a root advance makes every handle
   stale without any per-handle state.
4. **Capsules carry the session.** A capsule built under a session carries
   the `Negotiated` arm and must match the session's provenance.
5. **Codes.** Eight `SESSION_*` codes 33000 through 33007 travel in the
   SMP1 failure envelope.
6. **Staging.** `scripts/check_session_handle_profile.py` binds the
   contract, ADR, work-package row, and summary section, and fails closed
   if the session module appears before the summary allows it.

## Consequences

- S20-420 and S20-430 transport and display a real session identity; the
  S20-410 provisional issuance is replaced, not layered.
- The T15 and T47 matrices become exact server tests rather than
  narrative gates.

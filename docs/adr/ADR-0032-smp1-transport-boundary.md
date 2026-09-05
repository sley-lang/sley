# ADR-0032: SMP1 transport, negotiation, and identity-scoping boundary

Status: accepted; the S20-400 contract is at revision 11 with the
Ariadne contract, Nabu architecture, and Vulcan surface re-reviews PASS
and no new findings; implementation is S20-410

Date: 2026-09-03

## Context

The M0 `SMP1.md` was a constitutional draft written before any query,
mutation, or transaction schema existed. Its dependencies are now
implemented: root-backed queries and master capsules (S20-310 and S20-320
full, drafts under review), candidate construction and validation (S20-350
and S20-360), atomic commit and recovery (S20-390 and S20-530), refs,
comparison, merge, exchange, and GC. The threat register names three
protocol gates: downgrade (T45), request-identity confusion (T46), and
cross-workspace leakage (T47, owned with S20-330).

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **Transport owns no semantics.** Every SMP1 body is a frozen record of
   an existing contract and every failure keeps its owner's code; SMP1
   freezes framing, negotiation, identity scoping, bounded context, and the
   failure envelope only.
2. **Derived negotiation.** The selected profile is a function of the two
   hellos (greatest common version, first common epoch, minimum limits,
   intersected methods and features) and is digested under the registered
   `sley2.protocol-handshake.v1` domain, so a downgrade is detectable and
   `PROTOCOL_DOWNGRADE` is exact.
3. **Session-scoped strictly increasing request identifiers.** Reuse,
   decrease, cross-session use, and use after close fail before execution.
4. **Frozen method table.** Six families with frozen numeric tags; reserved
   methods fail `PROTOCOL_METHOD_UNSUPPORTED` with a versioned reason and
   never succeed generically.
5. **Bounded context on every response.** Counts and omission state are
   copied from the owning contract; a body that does not fit fails with no
   partial body.
6. **Frames are SCB1 envelopes.** Length-prefixed standalone SCB1 envelopes
   under a single contract tag 400 and the `sley2.protocol-frame.v1`
   domain, added to the identifier registry by S20-410.
7. **Staging.** `scripts/check_smp1_contract.py` binds the contract, ADR,
   work-package row, and summary section, and fails closed if a
   `sley-protocol` crate appears before the summary allows S20-410.
8. **Batch admission with cancellation before execution.** One batch is
   one explicit frame list; a cancel naming an admitted, not-yet-executed
   request of the same session answers `PROTOCOL_CANCELLED` without
   running anything, under single-frame `answer` (acknowledge) and
   multi-frame `answer_batch` alike. Streaming chunks a fitting body
   under the negotiated feature; budgets charge one unit at dispatch
   plus bytes on success.
9. **Transcript-bound identity.** The selection is the canonical
   `SelectedProfile` record digested with both hello bodies; each peer
   re-derives from the hellos as observed, and `session.open` compares
   against that derivation, so tamper with either hello is
   `PROTOCOL_DOWNGRADE` at open.

## Consequences

- S20-410 can implement the frame codec, handshake, and deterministic
  server without inventing semantics; S20-440, S20-420, and S20-430 have a
  frozen surface to extend, generate, and wrap.
- S20-330 gains its exact seams: the session methods, the reserved handle
  method, and the `SESSION_*` code family.

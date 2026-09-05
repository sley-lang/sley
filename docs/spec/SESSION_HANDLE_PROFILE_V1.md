# Negotiated Session and Handle Profile v1

Status: S20-330 contract draft, revision 2 (2026-09-05); implemented under
this draft with Council re-reviews pending (Nabu architecture review,
Ariadne contract review, Vulcan surface review), so the contract is not
frozen and the package is not complete. Revision 2 closes the six P0s and
the freeze-blocking P1s of the 2026-09-04 review round. Implementation
state is tracked in the machine summary.

This profile defines the negotiated session authority that SMP1 (S20-400,
revision 10) and the master context capsule (S20-320 full, revision 3)
reserved: what a session binds, how it is issued and renewed, how every
request is checked against its binding, what a session-local handle is,
and the `SESSION_*` codes. It composes, and never alters:

- `docs/spec/SMP1.md` at revision 10: the handshake, `session.open`
  (100), `session.renew` (101), `session.close` (102), the request-identity
  rules, and the reserved `handle.expand` (304) method whose bodies this
  profile freezes;
- `docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md` at revision 3: the
  `SessionBinding` field, whose `Negotiated(2)` arm this profile fills;
- `docs/spec/STATE_ROOT_V1.md` (S20-160 normative contract, unversioned),
  and the S20-390 verified revision, which supply the workspace, root,
  and epoch a session binds.

The authority rule is:

> A session is a server-issued binding of one negotiated handshake to one
> workspace, one verified root, and one schema epoch. The session identity
> names the binding; only the issuing authority instance mints names, and
> only for bindings it holds. Every request is checked against the live
> binding in contract order, and a handle is a session-local name that
> dies with the binding it was made under.
>
> The identity is not a secret and not peer ownership: anyone who learns
> a live name can use it while it is live. Peer isolation is a transport
> obligation outside this profile (section 8, threat T56).

## 1. Identity

The thirty-fifth identifier domain is `sley2.session.v1 -> SessionId`:

```text
session_preimage =
  ServerNonce[32] || ProtocolHandshakeId[32] || WorkspaceId[32] ||
  StateRoot[32] || SchemaEpochId[32] || u64be(issue_ordinal)
SessionId = BLAKE3-256("sley2.session.v1" || session_preimage)
```

`ServerNonce[32]` is minted once per authority instance from the
platform's documented-random keys and never persisted, transmitted, or
reused. `issue_ordinal` is the authority's count of sessions issued under
this handshake, starting at 1.

Consequences, all load-bearing:

- Equal inputs under one live instance issue equal identities, and the
  ordinal separates successive opens; equal servers over equal
  repository state issue different identities, and no caller derives
  another instance's live names from public head state (threat T56).
- A restart mints a fresh nonce, so every pre-restart name is unknown
  rather than silently re-minted: there is no cross-restart confusion.
- The identity names the session, not the root. Renewal rebinds the
  session to a new root while the identity still digests the issuance
  root, so after the first renewal the identity is verifiable only
  against the issuing authority's live record. Verifiers read the root
  from the capsule's own provenance (section 5), never by re-deriving
  the session identity.

## 2. Session record and issuance

```text
SessionRecord {
  session_id:    SessionId,
  handshake_id:  ProtocolHandshakeId,
  workspace_id:  WorkspaceId,
  bound_root:    StateRoot,        // the accepted head at open or last renew
  schema_epoch:  SchemaEpochId,
  issue_ordinal: u64,
  renewals:      u32               // at most 65,535
}
```

`session.open` (100) carries the negotiated `ProtocolHandshakeId`; a claim
that is not the negotiated identity is `PROTOCOL_DOWNGRADE`. The server
reads its repository's accepted head through the S20-390 loader, binds
its workspace, root, and epoch, issues the `SessionId`, and answers the
identity. A repository with no accepted head cannot open a session: the
S20-390 loader failure travels under its owner code, never under a
`SESSION_*` code.

Open is refused with `SESSION_BINDING_INVALID` when the live session
count already reaches the negotiated `max_sessions` (section 8), when the
issue ordinal space is exhausted, or as a defensive refusal when the
derived identity is already issued or the retained record is not the
bound root's own record.

`session.renew` (101) carries the `SessionId` in both the frame's session
field and the request body; a body that does not name the frame's
session is `PROTOCOL_FRAME_INVALID`. The server rebinds the session to
the current accepted head within the same epoch, increments `renewals`
(`SESSION_RENEWAL_LIMIT` past 65,535), and answers the same `SessionId`.
Renewal across an epoch change is refused with `SESSION_EPOCH_MISMATCH`
and is terminal: only a fresh `session.open` binds the new epoch. Every
handle naming the previous root is stale afterwards (section 4).
Implicit rebinding is rejected: a mutation that advances the head never
rebinds the session, because silent rebinding would revalidate handles
and capsules against a root the caller never accepted; the cost is one
explicit `session.renew` per commit.

`session.close` (102) ends the session. It is checked like every other
method (existence, workspace, epoch); every later frame naming a
remembered close is `PROTOCOL_SESSION_CLOSED` (section 8).

The two methods that create a head travel without a session only while
no accepted head exists: on a headless repository `workspace.create`
(200) and `exchange.import` (211) run sessionless, unadmitted and
uncharged, because no session can exist to charge. Once an accepted head
exists both methods require a session and are admitted, checked, and
charged like every other method; a sessionless frame then answers
`SESSION_BINDING_INVALID`. `exchange.import` advances the head, so the
unconditional exemption would let an unbound caller invalidate every
live binding.

## 3. Request checks

Before any method other than `session.open` runs, the server applies
these checks in this order; the first failure answers:

1. the frame names a session the authority holds live, else
   `SESSION_UNKNOWN`; a remembered close answers
   `PROTOCOL_SESSION_CLOSED` first (section 8);
2. request-identity admission for the live session
   (`PROTOCOL_REQUEST_ID_CONFLICT`, SMP1 section 3);
3. the repository's workspace equals the session's workspace
   (`SESSION_WORKSPACE_MISMATCH`, threat T47);
4. the repository's accepted schema epoch equals the session's epoch
   (`SESSION_EPOCH_MISMATCH`);
5. the session's work budget is not exhausted
   (`PROTOCOL_LIMIT_EXCEEDED`, S20-440);
6. for head-bound methods only, the accepted head root equals the
   session's `bound_root` (`SESSION_ROOT_ADVANCED`); the caller renews
   and retries.

The head-bound set is closed. A method is head-bound exactly when it
answers over current repository state without naming the state it
answers over:

`workspace.open` (201), `query.root` (300), `query.continue` (301),
`capsule` (302), `query.restricted` (303), `handle.expand` (304, which
performs the same root comparison itself and reports
`SESSION_STALE_HANDLE` instead of `SESSION_ROOT_ADVANCED`), `execute`
(600), `refs.list` (202), `refs.resolve` (203), `refs.recover` (209),
`recovery` (603), `exchange.export` (207), `gc.dry_run` (601), and
`report` (604).

Methods naming an explicit `TransactionId` (`revision.read`,
`checkout`), receipt, candidate bytes, or merge inputs answer over
caller-named state and are not head-bound. Methods that mutate
(`branch.create`, `branch.advance`, `commit`, `merge.commit`,
`candidate.*`, `workspace.create`, `exchange.import`, `gc.collect`)
advance the head instead of answering over it: they are checked for
workspace and epoch only, and leave the session bound to the previous
root until an explicit renewal, which is the explicit signal that
earlier handles and capsules describe an older root.

## 4. Handles

A handle is the session-local name of one entity of the session's bound
root: the pair of its zero-based position in the root's
`entity_bindings` in canonical ascending `EntityId` order and the
`StateRoot` it is expected under. Handles are never allocated or
stored; position-in-bound-root is a total function of the root, so a
root advance kills every handle with no per-handle state.

```text
handle.expand (304)
  request  = uvar(handle) || StateRoot[32] expected_root
  response = record(1: EntityId, 2: u32 kind, 3: ObjectId,
                    4: StateRoot bound_root, 5: SessionId)
```

`handle.expand` fails `SESSION_STALE_HANDLE` (threat T15) when the
expected root differs from the session's `bound_root` or the accepted
head root differs from the `bound_root`: a handle never resolves across
roots, sessions, or epochs, and a renewal leaves every handle naming
the old root stale instead of silently resolving to another entity.
Only a handle naming the new root resolves after renewal; the caller
learns the new root from any head-bound read after renewing. Positions
at or past the bound root's inventory fail `SESSION_HANDLE_UNKNOWN`;
the inventory extract fails closed above 65,535 bindings while
`uvar(handle)` accepts u64, so far positions are unknown, never
wrapped.

Response field 2 `u32 kind` is the entity's kind tag from the S20-250
projection of the bound root. A bindings/kinds length misalignment
answers `SESSION_BINDING_INVALID`: that is the internal invariant of a
well-formed root surfacing as a caller failure, never a binding the
caller could repair. A handle from another session is meaningless by
construction: positions are interpreted only under the named session's
binding. Handles never name query cursors: a cursor is continuation
state, and statelessness is exactly what makes positional handles safe;
cursors belong in `query.continue`'s `after`.

## 5. Capsule binding

The master context capsule built under a session carries
`SessionBinding = Negotiated(2) || SessionId[32]` (S20-320 full, revision
3). The capsule's provenance must equal the session's binding: the
authority fails `CONTEXT_CAPSULE_SOURCE_INVALID` when the response's
workspace, root, or epoch differs from the session's. The arm names the
session, not the root: a verifier reads the root from the capsule's own
provenance. A capsule built outside a session keeps arm `None(1)`. On
the server path the refusal is reachable only through the authority
directly, because `capsule` is head-bound over the server's own head;
the wire-observable refusal is the authority-level and unit-level one.

## 6. Stable failures

| Numeric | Symbolic code |
|---:|---|
| 33000 | `SESSION_UNKNOWN` |
| 33001 | `SESSION_WORKSPACE_MISMATCH` |
| 33002 | `SESSION_ROOT_ADVANCED` |
| 33003 | `SESSION_EPOCH_MISMATCH` |
| 33004 | `SESSION_STALE_HANDLE` |
| 33005 | `SESSION_HANDLE_UNKNOWN` |
| 33006 | `SESSION_RENEWAL_LIMIT` |
| 33007 | `SESSION_BINDING_INVALID` |

`SESSION_UNKNOWN` answers for a name no live session holds: a name the
authority never issued, a name forgotten after the remembered-close cap
(section 8), or a pre-restart name. A remembered close answers
`PROTOCOL_SESSION_CLOSED`, never `SESSION_UNKNOWN`.

`SESSION_BINDING_INVALID` is the authority-cannot-bind bucket: opening
past the negotiated `max_sessions`; sessionless `workspace.create` or
`exchange.import` while an accepted head exists; issue-ordinal
exhaustion; the defensive duplicate-identity and retained-record
refusals at bind time; and the bindings/kinds misalignment inside
`handle.expand`. A headless `session.open` is not in this bucket: the
S20-390 loader failure travels under its owner code.

`PROTOCOL_*` codes keep their SMP1 meanings; `SESSION_*` failures travel
in the SMP1 failure envelope with their exact numerics. The eight codes
33000 through 33007 are frozen by `docs/spec/ERROR_CODES_V1.md`.

## 7. Required evidence

Implementation acceptance requires at least:

- the T15 matrix (`evidence/security/T15/`): a handle expanded under
  its session and root, stale after the head advances, still stale
  after renewal while naming the old root, resolving only when naming
  the new root, unknown past the inventory, and stale when naming a
  root the session never bound;
- the T47 matrix (`evidence/security/T47/`): one server whose
  repository workspace changes under a live session, refused with
  `SESSION_WORKSPACE_MISMATCH` before any other check, with the epoch
  refusal exercised at the authority level (every trusted genesis
  carries the one frozen conformance epoch, so no server-level epoch
  swap exists);
- the T56 matrix (`evidence/security/T56/`): twin instances over equal
  state issuing different identities for the same ordinal, and a
  restarted instance answering `SESSION_UNKNOWN` to every pre-restart
  name;
- the precedence matrix: an unknown name answering `SESSION_UNKNOWN`
  (never `PROTOCOL_REQUEST_ID_CONFLICT`), a remembered close answering
  `PROTOCOL_SESSION_CLOSED`, an exhausted budget answering the binding
  failure ahead of `PROTOCOL_LIMIT_EXCEEDED` and the budget failure
  under a valid binding, a mismatched renew body answering
  `PROTOCOL_FRAME_INVALID`, and the sessionless genesis exemption
  ending at the first head;
- the cap matrix: opening past `max_sessions` refused with
  `SESSION_BINDING_INVALID`, closing freeing the slot;
- a capsule built under a session carrying arm `Negotiated(2)` and the
  session identity, and refusing a foreign provenance;
- Tier 1 plus Tier 2 validation, and the Nabu, Ariadne, and Vulcan
  re-reviews with every report-grade finding closed.

## 8. Explicit exclusions

This contract does not claim: session expiry by wall-clock time (the
server reads no clock; renewal counts and budgets bound a session's
work, and `max_sessions` bounds the live count); peer ownership of a
session name (a live name is usable by any caller that learns it; peer
isolation is a transport obligation on the S20-420/430 boundary,
threat T56); multi-repository or cross-workspace sessions;
authentication or transport security; the JSON bridge and CLI
representations (S20-420, S20-430); runtime, benchmark, packaging,
release, or GA.

Reclamation is exact and bounded without a clock. The registry holds
exactly the live sessions plus at most `max_sessions` remembered
closes in close order; closing past the cap forgets the oldest closed
name, which then answers `SESSION_UNKNOWN`. The authority holds exactly
the live sessions. No session state grows without bound for the process
lifetime: live sessions are capped at open, remembered closes are
capped at close.

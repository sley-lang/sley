# Negotiated Session and Handle Profile v1

Status: S20-330 contract draft, revision 1 (2026-09-03); implemented under
this draft with Council review pending (Nabu architecture review, Ariadne
contract review, Vulcan surface review), so the contract is not frozen and
the package is not complete. Implementation state is tracked in the machine
summary.

This profile defines the negotiated session authority that SMP1 (S20-400)
and the master context capsule (S20-320 full) reserved: what a session
binds, how it is issued and renewed, how every request is checked against
its binding, what a session-local handle is, and the `SESSION_*` codes.
It composes, and never alters:

- `docs/spec/SMP1.md`: the handshake, `session.open` (100),
  `session.renew` (101), `session.close` (102), the request-identity rules,
  and the reserved `handle.expand` (304) method whose bodies this profile
  freezes;
- `docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md`: the `SessionBinding` field,
  whose `Negotiated(2)` arm this profile fills;
- `docs/spec/STATE_ROOT_V1.md` and the S20-390 verified revision, which
  supply the workspace, root, and epoch a session binds.

The authority rule is:

> A session is a server-issued binding of one negotiated handshake to one
> workspace, one verified root, and one schema epoch. Nothing a caller
> declares can substitute for it, every request is checked against it, and
> a handle is a session-local name that dies with the binding it was made
> under.

## 1. Identity

The thirty-fifth identifier domain is `sley2.session.v1 -> SessionId`:

```text
session_preimage =
  ProtocolHandshakeId[32] || WorkspaceId[32] || StateRoot[32] ||
  SchemaEpochId[32] || u64be(issue_ordinal)
SessionId = BLAKE3-256("sley2.session.v1" || session_preimage)
```

`issue_ordinal` is the server's count of sessions issued under this
handshake, starting at 1. The identity therefore binds the negotiated
profile, the workspace, the accepted head root and epoch at issuance, and
the issuance order; equal servers over equal repository state issue equal
identities.

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
identity. A repository with no accepted head cannot open a session
(`SESSION_BINDING_INVALID`).

`session.renew` (101) carries the `SessionId`; the server rebinds the
session to the current accepted head, increments `renewals`
(`SESSION_RENEWAL_LIMIT` past 65,535), and answers the same `SessionId`.
Every handle issued before the renewal is stale afterwards (section 4).

`session.close` (102) ends the session; every later frame naming it is
`PROTOCOL_SESSION_CLOSED` (SMP1 section 3).

## 3. Request checks

Before any method other than `session.open` runs, the server checks the
named session in this order:

1. the session exists (`SESSION_UNKNOWN`);
2. the repository's workspace equals the session's workspace
   (`SESSION_WORKSPACE_MISMATCH`, threat T47);
3. the repository's accepted schema epoch equals the session's epoch
   (`SESSION_EPOCH_MISMATCH`);
4. for head-bound methods only, the accepted head root equals the
   session's `bound_root` (`SESSION_ROOT_ADVANCED`); the caller renews and
   retries.

Head-bound methods answer over "the accepted head" without naming it:
`workspace.open` (201), `query.root` (300), `query.continue` (301),
`capsule` (302), and `query.restricted` (303). `handle.expand` (304)
performs the same root comparison itself and reports
`SESSION_STALE_HANDLE` (section 4) instead of `SESSION_ROOT_ADVANCED`.
`workspace.create` (200) and `exchange.import` (211) may travel without a
session, because a repository without an accepted head cannot bind one;
under a session they are checked like every other method.
Methods that name an explicit `TransactionId` or mutate the repository are
not head-bound; a mutation that advances the head leaves the session bound
to the previous root until renewal, which is the explicit signal that
earlier handles and capsules describe an older root.

SMP1 revision 7 adds `execute` (600) to the head-bound set: it runs over
the session's bound root, so a root advance answers `SESSION_ROOT_ADVANCED`
until the session renews.

## 4. Handles

A handle is the session-local name of one entity of the session's bound
root: its zero-based position in the root's `entity_bindings`. Handles are
never allocated or stored; they are the binding order itself, so they are
exact, deterministic, and free of allocation state.

```text
handle.expand (304)
  request  = uvar(handle)
  response = record(1: EntityId, 2: u32 kind, 3: ObjectId,
                    4: StateRoot bound_root, 5: SessionId)
```

`handle.expand` fails `SESSION_STALE_HANDLE` (threat T15) when the
accepted head root differs from the session's `bound_root` (a handle
never resolves across roots, sessions, or epochs), and
`SESSION_HANDLE_UNKNOWN` when the position is outside the bound root's
inventory. A handle from another session is meaningless by construction:
positions are interpreted only under the named session's binding.

## 5. Capsule binding

The master context capsule built under a session carries
`SessionBinding = Negotiated(2) || SessionId[32]` (S20-320 full, revision
2). The capsule's provenance must equal the session's binding: the
builder fails `CONTEXT_CAPSULE_SOURCE_INVALID` when the response's
workspace, root, or epoch differs from the session's. A capsule built
outside a session keeps arm `None(1)`.

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

`PROTOCOL_*` codes keep their SMP1 meanings; `SESSION_*` failures travel
in the SMP1 failure envelope with their exact numerics.

## 7. Required evidence

Implementation acceptance requires at least:

- the T15 matrix: a handle expanded under its session and root, then stale
  after a commit advances the head, resolving again after renewal to the
  new root's binding order, and unknown past the inventory;
- the T47 matrix: two repositories with different workspaces under one
  server profile, a session of one refused by the other with
  `SESSION_WORKSPACE_MISMATCH`, and the same session refused after the
  epoch changes;
- deterministic issuance (equal servers over equal state issue equal
  identities) and the renewal limit;
- a capsule built under a session carrying arm `Negotiated(2)` and the
  session identity, and refusing a foreign provenance;
- Tier 1 plus Tier 2 validation, and the Nabu, Ariadne, and Vulcan reviews
  with every report-grade finding closed.

## 8. Explicit exclusions

This contract does not claim: session expiry by wall-clock time (the
server reads no clock; renewal counts and budgets bound a session);
multi-repository or cross-workspace sessions; authentication or transport
security; the JSON bridge and CLI representations (S20-420, S20-430);
runtime, benchmark, packaging, release, or GA.

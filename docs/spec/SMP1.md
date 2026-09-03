# Sley Machine Protocol v1 (SMP1)

Status: S20-400 contract draft, revision 1 (2026-09-03); Council review
pending (Ariadne contract review as the package owner, Nabu architecture
review, Vulcan surface review). This revision supersedes the M0
constitutional draft of the same file; the M0 text's commitments (bounded,
version-negotiated, request/response, cancellation-aware, machine-code
first, transport-neutral above framing, prose only as debug metadata) are
carried forward and made exact. No implementation exists at this revision:
S20-410 implements the frame and the deterministic server, S20-440 freezes
cancellation and streaming semantics, S20-420 generates the JSON bridge, and
S20-430 wraps the CLI. Implementation state is tracked in the machine
summary.

SMP1 is the primary programming interface of Sley 2. It transports the
frozen records of the repository, query, capsule, candidate, transaction,
and execution surfaces and their stable codes. The authority rule is:

> SMP1 owns framing, negotiation, identity scoping, and bounded transport.
> It owns no semantics: every payload is a frozen record of an existing
> contract, every judgment is made by that contract's engine, and every
> failure keeps that contract's code. A transport that could add, omit, or
> reinterpret a fact is not SMP1.

## 1. Framing

The reference transport is stdio; any byte stream that delivers frames in
order may carry SMP1. A frame is:

```text
frame = u64be(payload_length) || protocol_envelope
protocol_envelope = SCB1 standalone envelope with
  contract_tag   = 400 (request) | 401 (response) | 402 (hello)
  schema_epoch   = the negotiated protocol schema epoch
  payload        = ProtocolFrame (below), a canonical SCB1 record
  digest domain  = sley2.protocol-frame.v1 -> ProtocolFrameId
```

`payload_length` must equal the envelope's exact stored length and must not
exceed the negotiated `max_frame_bytes` (never above 67,108,864); the check
happens before allocation. The envelope digest is verified before any field
is read. A frame that fails length, magic, tag, epoch, or digest rules is
`PROTOCOL_FRAME_INVALID` or `PROTOCOL_FRAME_TOO_LARGE`, the connection
state does not change, and no partial frame is ever acted on. An optional
checksum profile never changes payload semantics.

```text
ProtocolFrame {
  protocol_version: u32,                    // negotiated, 1 at this revision
  session:          option(SessionId[32]),  // None only for hello and session.open
  request_id:       u64,                    // scoped to the session, strictly increasing
  kind:             u32 (1 request | 2 response | 3 event),
  method:           u32,                    // section 4
  flags:            u32,                    // bit 0 cancel, bit 1 stream, others reserved
  bounds:           BoundedContext,         // section 5, zero on requests
  body:             bytes                   // the method's frozen record, opaque here
}
```

## 2. Handshake

Both peers send one `hello` frame (tag 402, kind request from the client,
kind response from the server) whose body is:

```text
Hello {
  protocol_versions: list(u32),        // strictly increasing
  schema_epochs:     list(SchemaEpochId), // raw order
  limits:            LimitProfile,
  methods:           list(u32),        // strictly increasing method tags
  features:          u32,              // bit 0 cancel, bit 1 stream, bit 2 json_bridge, bit 3 checksum
  adapters:          list(AdapterId[32]),
  effects:           list(EntityId[32])
}
LimitProfile {
  max_frame_bytes: u64, max_entities: u64, max_edges: u64, max_depth: u32,
  max_response_bytes: u64, max_work: u64, max_inflight: u32
}
```

The selected profile is derived, never chosen freely:

- `protocol_version` is the greatest version present in both lists;
- `schema_epoch` is the first epoch of the server's list that the client
  also lists;
- each limit is the minimum of the two declared limits;
- `methods`, `features`, `adapters`, and `effects` are the intersections;
- the selection is encoded as `SelectedProfile` in the server hello and
  identified by `ProtocolHandshakeId =
  BLAKE3-256("sley2.protocol-handshake.v1" || selected_profile_preimage)`,
  which both peers must compute identically.

If no common version, epoch, or method family exists the server answers
`PROTOCOL_NO_COMMON_PROFILE` and closes. A client that, after receiving the
server hello, opens a session naming a version or epoch lower than the
selected one, or a server hello that names a version lower than the
greatest common one, is `PROTOCOL_DOWNGRADE` (threat T45). No silent
downgrade exists: the selected profile is explicit and digested, and every
later frame names `protocol_version` and is checked against it.

## 3. Sessions and request identity

`session.open` (method 100) is the only request frame with `session = None`
after the handshake; it carries the `ProtocolHandshakeId` and returns the
`SessionId` under which every later frame is scoped. The negotiated session
authority, its workspace, verified root, and epoch binding, handle
issuance, renewal, and the `SESSION_*` codes (including
`SESSION_WORKSPACE_MISMATCH` for threat T47 and `SESSION_STALE_HANDLE` for
threat T15) belong to S20-330, which fills the reserved arms of this
contract and of the S20-320 capsule. SMP1 freezes only the scoping rules:

- request identifiers are `u64` values strictly increasing within one
  session; a reused, decreasing, or cross-session identifier fails
  `PROTOCOL_REQUEST_ID_CONFLICT` (threat T46) and the request is not
  executed;
- a response carries the request's identifier and session; an event
  (kind 3) carries the identifier of the request it belongs to;
- after `session.close` (method 102) every frame naming that session is
  `PROTOCOL_SESSION_CLOSED`;
- at most `max_inflight` requests may be outstanding; beyond that
  `PROTOCOL_LIMIT_EXCEEDED`.

## 4. Method families and tags

Method tags are frozen numbers. Each method names the frozen contract that
owns its request and response bodies; SMP1 adds nothing to them.

| Tag | Method | Request body | Response body | Owner |
|---:|---|---|---|---|
| 100 | `session.open` | `ProtocolHandshakeId` | `SessionId` | S20-330 |
| 101 | `session.renew` | `SessionId` | `SessionId` | S20-330 |
| 102 | `session.close` | none | none | S20-330 |
| 103 | `session.capabilities` | none | `SelectedProfile` | S20-400 |
| 104 | `session.budgets` | none | `LimitProfile` remaining | S20-400 |
| 200 | `workspace.create` | trusted genesis input | `TransactionId` | S20-390 |
| 201 | `workspace.open` | repository path digest | accepted head | S20-390 |
| 202 | `refs.list` | limit | `list(ResolvedBranch)` | S20-500 |
| 203 | `refs.resolve` | branch name | `ResolvedBranch` | S20-500 |
| 204 | `revision.read` | `TransactionId` | verified revision summary | S20-390 |
| 205 | `branch.create` | `BranchRecord` | `ImportedBranchRecord` | S20-500 |
| 206 | `branch.advance` | branch advance input | `ImportedBranchRef` | S20-500 |
| 207 | `compare` | two `StateRoot` | `StoredSemanticDelta` | S20-510 |
| 208 | `merge.judge` | ours, theirs | `MergeOutcome` record | S20-520 |
| 209 | `merge.commit` | merge plan input | commit output | S20-520 |
| 210 | `exchange.export` | none | exchange stored bytes | S20-540 |
| 211 | `exchange.import` | exchange stored bytes | `ExchangeImportReport` | S20-540 |
| 212 | `gc.dry_run` | retention snapshot | `GcReport` | S20-560 |
| 213 | `gc.collect` | retention snapshot | `GcReport` | S20-560 |
| 214 | `refs.recover` | none | `RefRecoveryReport` | S20-500 |
| 300 | `query.root` | `SLEYRQQ1` request | `SLEYRQR1` response | S20-310 full |
| 301 | `query.continue` | request with `after` | `SLEYRQR1` response | S20-310 full |
| 302 | `capsule` | `SLEYRQQ1` request | `SLEYCCP1` capsule | S20-320 full |
| 303 | `query.restricted` | `SLEYQRY1` request | `SLEYQRS1` response | S20-310 restricted |
| 304 | `handle.expand` | reserved | reserved | S20-330 |
| 305 | `diagnostics` | reserved | reserved | S20-620 |
| 400 | `candidate.create` | candidate record | stored candidate | S20-350 |
| 401 | `candidate.append` | candidate plus operation | stored candidate | S20-350 |
| 402 | `candidate.validate` | stored candidate | candidate result | S20-360 |
| 403 | `candidate.inspect` | stored candidate | imported candidate summary | S20-350 |
| 404 | `candidate.discard` | candidate attempt digest | none | S20-350 |
| 500 | `commit` | `CommitInput` | `CommitOutput` | S20-390 |
| 501 | `receipt.read` | `ReceiptId` | imported receipt | S20-390 |
| 502 | `checkout` | `TransactionId` | verified revision objects | S20-390 |
| 503 | `ref.move.protected` | reserved | reserved | S20-370 |
| 504 | `recovery` | none | recovery report | S20-530 |
| 600 | `execute` | restricted VM input | execution report | S20-380 |
| 601 | `tests.selected` | reserved | reserved | S20-620 |
| 602 | `tests.affected` | reserved | reserved | S20-620 |
| 603 | `cancel` | `request_id` | none | S20-440 |
| 604 | `report` | report identity | report record | S20-290 |

A reserved method, or any tag outside this table, fails
`PROTOCOL_METHOD_UNSUPPORTED` with the versioned reason; it never succeeds
generically. A body that does not decode as the owner's frozen record fails
`PROTOCOL_PAYLOAD_INVALID` before the owner's engine runs; a body that
decodes but fails the owner's rules keeps the owner's exact code.

## 5. Bounded context

Every response frame carries:

```text
BoundedContext {
  applied_limits:   LimitProfile,
  returned_bytes:   u64, returned_entities: u64, returned_edges: u64,
  reached_depth:    u32,
  omitted:          u64,
  truncated:        u32 (1 false | 2 true),
  continuation:     u32 (1 none | 2 cursor in body)
}
```

The counts are copied from the owning contract's response (a `SLEYRQR1`
response supplies its `total_count`, `returned`, and cursor; a capsule its
status) and are zero for bodies that carry none. A response can therefore
never hide an omission the owning contract stated, and a transport-level
truncation of a body is impossible: a body that does not fit the negotiated
limits fails `PROTOCOL_LIMIT_EXCEEDED` with no partial body.

## 6. Failure envelope

A failed request is answered by a response frame whose body is:

```text
ProtocolFailure {
  code:          u32,                  // the exact stable numeric code
  symbol:        text,                 // its symbolic name
  phase:         u32,                  // owner phase or 0
  retryability:  u32 (1 NEVER | 2 AFTER_REQUERY | 3 AFTER_CAPABILITY |
                      4 AFTER_LIMIT_CHANGE | 5 TRANSIENT_HOST),
  incident:      option(digest[32]),   // for INTERNAL_ERROR only
  details:       bytes                 // the owner's frozen failure record, may be empty
}
```

Owner codes are never collapsed or renumbered; `INTERNAL_ERROR` is
fail-closed, non-committable, and non-retryable unless the typed details
establish `TRANSIENT_HOST`.

## 7. Cancellation and streaming

Flag bit 0 (`cancel`) on a request frame, or method 603 naming an
outstanding request, asks the server to stop that request; the request is
answered with `PROTOCOL_CANCELLED` or with its normal response if it had
already completed, never with a partial body. Flag bit 1 (`stream`) marks
event frames of a streaming response. S20-440 freezes the cancel latency
bound, the streaming continuation rules, and the hard limits; this
contract freezes only the flags, the method, and the no-partial-body rule.

## 8. JSON bridge

S20-420 generates the JSON bridge from this contract. Byte strings are
lowercase hex, integers are decimal strings when above 2^53, codes and
symbols are preserved verbatim, unknown and omission states are explicit,
and the bridge performs no semantic validation. JSON is non-canonical and
cannot participate in any program identity.

## 9. Stable failures

| Numeric | Symbolic code |
|---:|---|
| 40000 | `PROTOCOL_VERSION_UNSUPPORTED` |
| 40001 | `PROTOCOL_FRAME_INVALID` |
| 40002 | `PROTOCOL_FRAME_TOO_LARGE` |
| 40003 | `PROTOCOL_NO_COMMON_PROFILE` |
| 40004 | `PROTOCOL_DOWNGRADE` |
| 40005 | `PROTOCOL_REQUEST_ID_CONFLICT` |
| 40006 | `PROTOCOL_SESSION_CLOSED` |
| 40007 | `PROTOCOL_METHOD_UNSUPPORTED` |
| 40008 | `PROTOCOL_PAYLOAD_INVALID` |
| 40009 | `PROTOCOL_LIMIT_EXCEEDED` |
| 40010 | `PROTOCOL_CANCELLED` |
| 40011 | `PROTOCOL_INTERNAL_INVARIANT` |

Precedence: frame length and envelope (`PROTOCOL_FRAME_TOO_LARGE`,
`PROTOCOL_FRAME_INVALID`); version (`PROTOCOL_VERSION_UNSUPPORTED`,
`PROTOCOL_DOWNGRADE`); session and identity (`PROTOCOL_SESSION_CLOSED`,
`PROTOCOL_REQUEST_ID_CONFLICT`, `PROTOCOL_LIMIT_EXCEEDED` for inflight);
method (`PROTOCOL_METHOD_UNSUPPORTED`); body (`PROTOCOL_PAYLOAD_INVALID`);
owner engine (owner codes); response fit (`PROTOCOL_LIMIT_EXCEEDED`);
cancellation (`PROTOCOL_CANCELLED`); impossible construction
(`PROTOCOL_INTERNAL_INVARIANT`). `SESSION_*` codes are S20-330's.

## 10. Required evidence

Contract acceptance (S20-400) requires the Ariadne contract review, Nabu
architecture review, and Vulcan surface review of this document with every
report-grade finding closed. Implementation acceptance (S20-410) requires
at least: fixed frame and hello vectors with an independent reproduction;
the handshake matrix (no common profile, each downgrade shape, identical
`ProtocolHandshakeId` on both peers); the request-identity matrix
(duplicate, decreasing, cross-session, after close, inflight ceiling); a
method-table freeze test; a deterministic server answering every
non-reserved method over the frozen engines with byte-identical responses
across runs; an S20-700 persistent target over the frame decoder and
handshake; and Tier 1 plus Tier 2 validation.

## 11. Explicit exclusions

This contract does not claim: transport security, authentication,
multi-tenant isolation, or network behaviour; negotiated session semantics
and handles (S20-330); cancellation latency and streaming rules (S20-440);
the generated JSON bridge (S20-420) and CLI (S20-430); diagnostics and test
selection (S20-620); runtime, benchmark, packaging, release, or GA.

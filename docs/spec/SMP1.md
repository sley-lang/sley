# Sley Machine Protocol v1 (SMP1)

Status: S20-400 contract draft, revision 12 (2026-09-08; the revision
history is listed below after the authority rule); Council review pending
(Ariadne contract review as the package owner, Nabu architecture review,
Vulcan surface review). This revision supersedes the M0 constitutional
draft of the same file; the M0 text's commitments (bounded,
version-negotiated, request/response, cancellation-aware, machine-code
first, transport-neutral above framing, prose only as debug metadata) are
carried forward and made exact. Implementation state: S20-410 implements
the frame and the deterministic server under this draft (thirty-seven
non-reserved methods dispatched, closeout
`docs/audits/S20_410_SMP1_FRAME_CLOSEOUT.md`), S20-440 freezes the
cancellation, streaming, and budget rules carried in section 7 and
appendix B (closeout
`docs/audits/S20_440_SMP1_CANCEL_STREAM_CLOSEOUT.md`), S20-420 generates
the JSON bridge from this contract (revision 7, closeout
`docs/audits/S20_420_JSON_BRIDGE_CLOSEOUT.md`), and S20-430 wraps the CLI
(revision 4, closeout `docs/audits/S20_430_THIN_CLI_CLOSEOUT.md`).
Current composition (revision 12): the S20-420 bridge contract
`docs/spec/SMP1_JSON_BRIDGE_V1.md` revision 8 and the S20-430 CLI contract
`docs/spec/SLEY_CLI_V1.md` revision 5.
Further implementation state is tracked in the machine summary.

SMP1 is the primary programming interface of Sley 2. It transports the
frozen records of the repository, query, capsule, candidate, transaction,
and execution surfaces and their stable codes. The authority rule is:

> SMP1 owns framing, negotiation, identity scoping, and bounded transport.
> It owns no semantics: every payload is a frozen record of an existing
> contract, every judgment is made by that contract's engine, and every
> failure keeps that contract's code. A transport that could add, omit, or
> reinterpret a fact is not SMP1.

**Revision history.** 2 folds the hello into frame kind 4 under one
contract tag; 3 adds appendix A, the exact body records of the methods
S20-410 dispatches; 4 freezes the S20-440 cancellation, streaming, and
budget rules of section 7 and appendix B; 5 hands `handle.expand` and
session issuance to the S20-330 profile; 6 marks failure envelopes with
response flag bit 2; 7 adds appendix C, the body records of the four
methods S20-410 slice C dispatches; 8 adds the `execute` cache profile
selector, `limits` field 6; 9 binds both hello bodies into the handshake
identity with per-peer re-derivation, values the envelope epoch, scopes
cancel to the answering path, completes the zero-code table, and pins
every live session in gc; 10 adds `limits` field 8 `max_sessions` and
binds the S20-330 revision 2 `handle.expand` expected-root request field,
with no other body changes; 11 answers the three Council review rounds in
full (explicit retryability enumeration, enforced response ceilings,
dispatch-cost budgets, identifier floor, negotiation floor, version-claim
rule, failed-stream flag, per-seam reserved reasons, gated extended
profile, stated details mapping, ordered appendix A, this history list);
12 adds the protocol version 2 static successor metadata (the two
S20-310 entity-read methods of `docs/spec/ENTITY_READ_PROFILE_V2.md`)
without changing any version 1 row, byte, or helper. Document revision
(a draft number of this file) and negotiated protocol version (the wire
selection 1 or 2) are distinct: revision 12 still serves version 1
exactly as before. Revision 12 is a static delta only: the version 1
implementations remain in place, capable bridge/CLI runtime is phase 3,
and the revision 11 review history is retained as history. Those reviews
do not review revision 12; its new-delta review is pending.

## 1. Framing

The reference transport is stdio; any byte stream that delivers frames in
order may carry SMP1. A frame is:

```text
frame = u64be(frame_length) || protocol_envelope
protocol_envelope = SCB1 standalone envelope with
  contract_tag   = 400 (every frame kind)
  schema_epoch   = the protocol schema epoch of section 1
  payload        = ProtocolFrame (below), a canonical SCB1 record
  digest domain  = sley2.protocol-frame.v1 -> ProtocolFrameId
```

`frame_length` is the envelope's exact stored length and must not exceed
the negotiated `max_frame_bytes` (never above 67,108,864); the check
happens before allocation. It is not SCB1 section 2's `payload_length`:
that inner `uvar` covers only the envelope's inner payload, while the
outer `u64be` covers the whole envelope. The envelope digest is verified
before any field is read. A frame that fails length, magic, tag, or
digest rules, or that carries bytes after the `digest` trailer, is
`PROTOCOL_FRAME_INVALID` (or `PROTOCOL_FRAME_TOO_LARGE` for the
ceiling); a format-version or envelope-epoch mismatch is
`PROTOCOL_VERSION_UNSUPPORTED`, never a frame defect: version mismatch
is a version failure, and the two codes never overlap. The
connection state does not change, and no partial frame is ever acted on.
An optional checksum profile never changes payload semantics. A hello is
decoded under the absolute 67,108,864-byte ceiling and the hello list caps
before any negotiation exists: there is no negotiated ceiling yet, so the
absolute one plus the caps are the pre-negotiation rule.

The envelope `schema_epoch` is the protocol schema epoch, constant at this
revision: epoch number 1, the `SchemaEpochRecordV1` built by
`sley-protocol::protocol_epoch_record` (SCB1 format 1, hash tag 1,
epoch-1 Unicode and limits, one contract descriptor for tag 400 carrying
the frame field schema, no extensions, no predecessor), identified by
`sley-protocol::protocol_epoch_id()`. It is not the negotiated content
epoch: the envelope epoch versions the framing itself and is the same on
every frame, while the section 2 selection versions the bodies the frames
carry and differs per negotiation. A body naming a version or epoch
outside the selection fails at the method layer; a frame naming an
envelope epoch outside the protocol fails at the envelope layer.

```text
ProtocolFrame {
  protocol_version: u32,                    // negotiated, 1 at this revision
  session:          option(SessionId[32]),  // None only for hello and session.open
  request_id:       u64,                    // scoped to the session, strictly increasing
  kind:             u32 (1 request | 2 response | 3 event | 4 hello),
  method:           u32,                    // section 4; 0 for hello
  flags:            u32,                    // bit 0 cancel, bit 1 stream, bit 2 failed (responses only), others reserved
  bounds:           BoundedContext,         // section 5, zero on requests: a request carrying nonzero bounds is PROTOCOL_FRAME_INVALID
  body:             bytes                   // the method's frozen record, opaque here
}
```

## 2. Handshake

Both peers send one hello frame (kind 4, `session = None`, `request_id = 0`,
`method = 0`, `flags = 0`), the client first, whose body is:

```text
Hello {
  protocol_versions: list(u32),        // strictly increasing
  schema_epochs:     list(SchemaEpochId), // raw order
  limits:            LimitProfile,
  methods:           list(u32),        // strictly increasing method tags
  features:          u32,              // bit 0 cancel, bit 1 stream, bit 2 json_bridge, bit 3 checksum, bit 4 extended_execute
  adapters:          list(AdapterId[32]),
  effects:           list(EntityId[32])
}
LimitProfile {
  max_frame_bytes: u64, max_entities: u64, max_edges: u64, max_depth: u32,
  max_response_bytes: u64, max_work: u64, max_inflight: u32,
  max_sessions: u32
}
```

A method family is the hundred-group of a tag (1 session, 2
repository, 3 query, 4 candidate, 5 commit, 6 execution). A hello lists
only dispatched, non-reserved tags: a hello naming a reserved tag
(305, 503, 601, 602) is `PROTOCOL_PAYLOAD_INVALID`, because a method
that always fails is never common. The selected profile is derived, never
chosen freely:

- `protocol_version` is the greatest version present in both lists;
- `schema_epoch` is the first epoch of the server's list that the client
  also lists: the epoch choice is the server's preference among the
  common epochs, tamper-evident like everything else in the transcript
  but not derived against the server's own ordering;
- each limit is the minimum of the two declared limits;
- `methods`, `adapters`, and `effects` are the client-ordered
  intersections, `features` the intersected bits;
- the intersection must contain `session.open` (100): without it no
  session can ever bind, so an intersection without the session family
  is `PROTOCOL_NO_COMMON_PROFILE`, not a successful handshake.

The selection is exactly the `SelectedProfile` record, the only canonical
definition (the S20-420 bridge renders it as JSON and carries the identity
as opaque data; it defines nothing):

```text
SelectedProfile {
  protocol_version: u32,                  // the derived greatest common version
  schema_epoch:     SchemaEpochId[32],    // the derived content epoch
  limits:           LimitProfile,         // the pairwise minima
  methods:          list(u32),            // the intersection, client order
  features:         u32,                  // the intersection bits
  adapters:         list(AdapterId[32]),  // the intersection, client order
  effects:          list(EntityId[32])    // the intersection, client order
}
selected_profile_preimage = the canonical SCB1 bytes of that record, in
  field order 1 through 7
handshake_transcript = client hello body || server hello body ||
  selected_profile_preimage, where each hello body is the exact
  `Hello::encode` bytes of the hello as observed, the client first
  because it speaks first
ProtocolHandshakeId =
  BLAKE3-256("sley2.protocol-handshake.v1" || handshake_transcript)
```

Both peers re-derive per peer: each runs this section's derivation on the
hellos as observed (the own hello as sent, the peer hello as received)
and uses the derived selection and identity. A server never accepts an
asserted selection for identity: it negotiates from the observed hellos
and `session.open` compares the presented identity against that
derivation, failing `PROTOCOL_DOWNGRADE` on any difference. The server
hello is a `Hello` offer like the client's, not a carrier for the
selection; `session.capabilities` exposes the server's derived selection,
and it equals the re-derived one.

If no common version, epoch, or method family exists the server answers
`PROTOCOL_NO_COMMON_PROFILE` and closes. The explicit version-aware
negotiation entrypoint (`negotiate_versioned`, revision 12) supports
selected protocol versions 1 and 2 only: an unsupported greatest-common
result, including 3, is refused with `PROTOCOL_VERSION_UNSUPPORTED`
before establishment, without selecting a lesser common version. Under
selected version 1 the intersection filters exactly the two version-2
tags 306 and 307; unrelated opaque unknown numeric tags keep their
legacy treatment. Every hello frame travels as frame version 1, so a
version-1 peer can read a version-2 offer. The legacy numeric hello and
negotiation helpers retain their existing behavior, including retaining
unknown numeric tags such as 306 and 307 in a version-1-only offer
intersection without thereby dispatching them. Old method, codec, and
server entrypoints stay version-1-only; version-aware entrypoints select
the expected version explicitly. A `session.open` whose identity
is not the re-derived one is `PROTOCOL_DOWNGRADE` (threat T45).
Tampering with either hello is the same failure: each side binds its true
hello, so any tamper makes the two transcripts differ and the two
identities differ, and the session never opens. A server that offers only
versions it supports is exercising preference, not tamper: both peers
re-derive the same greatest common version from the observed hellos, and
the client refuses any selection that is not that derivation. Every later
frame names `protocol_version` and is checked against the selection: a
frame below it is a downgrade attempt and answers `PROTOCOL_DOWNGRADE`, a
frame above it names a version the selection does not know and answers
`PROTOCOL_VERSION_UNSUPPORTED`. The codec applies the same split at
decode against the implementation version, so the rule holds on wire
input before dispatch ever runs; the selection-level split in
`SelectedProfile::check_claim` agrees with it. A `SchemaEpochId` is a 32-byte identity
with no order, so there is no "lower epoch": a body naming any epoch
outside the selection fails at the method layer. No silent downgrade
exists: the selection is explicit and digested over both hellos, and the
session binds the handshake at `session.open`, so the negotiated
`ProtocolHandshakeId` is never repeated on later frames.

## 3. Sessions and request identity

`session.open` (method 100) is the request frame with `session = None`
that binds a session: it carries the `ProtocolHandshakeId` and returns the
`SessionId` under which every later frame is scoped. A session-less frame
carries identifier 0, no other: hello, `session.open`, and the genesis
path share no counter with any session, and a session-less frame naming
any nonzero identifier is `PROTOCOL_FRAME_INVALID`. Opens are not
idempotent: replaying an open creates another session (capped by the
negotiated `max_sessions`). Replaying a genesis write once a head exists
needs a session like any other write and answers the owner's
`TXN_ALREADY_INITIALIZED`; while no head exists the session-less replay
answers the owner failure directly. The only other
session-less requests are the genesis path: `workspace.create` and
`exchange.import` may travel without a session only while the repository
has no accepted head, because no session can bind before there is a head
to bind it to; once a head exists they require a session like every other
method. The negotiated session authority, its workspace, verified root,
and epoch binding, handle issuance, renewal, and the `SESSION_*` codes
(including `SESSION_WORKSPACE_MISMATCH` for threat T47 and
`SESSION_STALE_HANDLE` for threat T15) belong to S20-330, which fills the
reserved arms of this contract and of the S20-320 capsule. A session
acquires its workspace at open under the S20-330 binding; the workspace
bodies stay S20-390's. SMP1 freezes only the scoping rules:

- request identifiers are `u64` values strictly increasing within one
  session, starting at 1: identifier 0 never enters a session. It is the
  pre-session sentinel, carried by hello frames (kind 4, no session),
  answered by frame-level failures (no session), and allowed on
  `session.open` without consuming the sequence, so the three are told
  apart by frame kind, never by identifier. A reused, decreasing, zero, or cross-session identifier fails `PROTOCOL_REQUEST_ID_CONFLICT` (threat T46) and the request is not executed; a cross-session
  identifier is one admitted under another session, because every
  session carries its own counter and counters never merge. After
  `u64::MAX` every further identifier conflicts: exhaustion closes the
  sequence, it never wraps;
- a response carries the request's identifier and session; an event
  (kind 3) carries the identifier of the request it belongs to;
- after `session.close` (method 102) every frame naming that session is
  `PROTOCOL_SESSION_CLOSED`; at most the most recently closed names are
  remembered, so a forgotten name answers `SESSION_UNKNOWN` at the
  session layer instead, which bounds registry memory;
- at most `max_inflight` requests may be outstanding; beyond that
  `PROTOCOL_LIMIT_EXCEEDED`;
- `session.open` always creates a new session: a repeated open is a new
  session, never a failure, capped only by the negotiated
  `max_sessions`.

## 4. Method families and tags

Method tags are frozen numbers. Each method names the frozen contract that
owns its request and response bodies; SMP1 adds nothing to them. The
owner is the body-record owner: failure codes keep their own owners
under section 6, and where dispatch, storage, or reporting cross an
owner boundary the row says so. A tag added after freeze takes a new
`protocol_version`: the frozen table never grows under a shipped version,
and a reserved tag goes live the same way, never by appearing in a
negotiated `methods` intersection.

### Protocol version 1

The version 1 table below is frozen: 41 rows total, 37 dispatched
(non-reserved) methods. Its rows, bytes, and legacy helpers are unchanged
by revision 12.

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
| 212 | `gc.dry_run` | retention snapshot | `GcReport` | S20-560 (GC failures are S20-180) |
| 213 | `gc.collect` | retention snapshot | `GcReport` | S20-560 (GC failures are S20-180) |
| 214 | `refs.recover` | none | `RefRecoveryReport` | S20-500 |
| 300 | `query.root` | `SLEYRQQ1` request | `SLEYRQR1` response | S20-310 full |
| 301 | `query.continue` | request with `after` | `SLEYRQR1` response | S20-310 full |
| 302 | `capsule` | `SLEYRQQ1` request | `SLEYCCP1` capsule | S20-320 full |
| 303 | `query.restricted` | `SLEYQRY1` request | `SLEYQRS1` response | S20-310 restricted |
| 304 | `handle.expand` | `uvar(handle) \|\| StateRoot[32]`, the binding position in the session's bound root with the root it is expected under | handle facts record | S20-330 |
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
| 600 | `execute` | restricted VM input | execution report | S20-380 (execution report is S20-290) |
| 601 | `tests.selected` | reserved | reserved | S20-620 |
| 602 | `tests.affected` | reserved | reserved | S20-620 |
| 603 | `cancel` | `request_id` | none | S20-440 |
| 604 | `report` | report identity | report record | S20-290 (report store is S20-560) |

### Protocol version 2 additions

Protocol version 2 is the sorted union of the version 1 table above and
exactly the two rows below: 43 rows total, 39 dispatched methods. No
second independently maintained 43-row table exists; consumers union the
two tables in tag order. Reserved tags remain 305, 503, 601, 602 in both
versions. Both additions are owned by S20-310; their request and response
records are defined by `docs/spec/ENTITY_READ_PROFILE_V2.md` sections 3
through 5, which this contract links to instead of restating.

| Tag | Method | Request body | Response body | Owner |
|---:|---|---|---|---|
| 306 | `entity.version` | entity version request (ENTITY_READ section 3) | entity version response (ENTITY_READ sections 3-5) | S20-310 |
| 307 | `entity.signature` | entity signature request (ENTITY_READ section 3) | entity signature response (ENTITY_READ sections 3-5) | S20-310 |

A reserved method, or any tag outside this table, fails
`PROTOCOL_METHOD_UNSUPPORTED` with the versioned reason; it never succeeds
generically. The reason names the seam that owns the tag:
`SMP1-RESERVED-S20-370` for the protected-move seam,
`SMP1-RESERVED-S20-620` for the diagnostics and test-selection seam, so a
client can tell the two apart. A reserved-method failure is retryable
`AFTER_CAPABILITY`: it lifts when the capability appears. A body that
does not decode as the owner's frozen record fails
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
status) and are zero for bodies that carry none. `applied_limits` is the
negotiated profile's limits, echoed unchanged: it is the ceiling the
response was fitted against, never a per-request effective set. A
response can therefore never hide an omission the owning contract stated,
and a body that does not fit the negotiated limits fails
`PROTOCOL_LIMIT_EXCEEDED` with no partial body: the body bytes against
`max_response_bytes` and the copied counts against `max_entities`,
`max_edges`, and `max_depth`, on the single-frame and streaming paths
alike (section 7 streams a body that fits; it never ships one that does
not). Zero counts on a failure response or an event frame mean
not-counted-by-the-transport, never owner-stated zero: only a successful
kind-2 response carries counted facts.

## 6. Failure envelope

A failed request is answered by a response frame whose body is:

```text
ProtocolFailure {
  code:          u32,                  // the exact stable numeric code
  symbol:        text,                 // its symbolic name
  phase:         u32,                  // owner phase or 0
  retryability:  u32 (1 NEVER | 2 AFTER_REQUERY | 3 AFTER_CAPABILITY |
                      4 AFTER_LIMIT_CHANGE | 5 TRANSIENT_HOST),
  incident:      option(digest[32]),   // none at this revision (below)
  details:       bytes                 // transport-supplied reason bytes from the set below, else empty
}
```

Owner codes are never collapsed or renumbered; `INTERNAL_ERROR` is
fail-closed, non-committable, and non-retryable unless the typed details
establish `TRANSIENT_HOST`. The five owner classes whose crates expose
symbols only travel with numeric `0` until their owners expose numerics;
they are listed with their owning packages at the end of appendix A and
are the only exception to this section's exact-code rule. Two zeros meet
in one envelope and must not be confused: numeric `0` marks the five
symbol-only classes and never equals a frozen code, while `phase: 0`
marks a transport-raised failure with no owner phase. The envelope's `incident` is none at this revision: the transport mints no digests, and an owner's incident digest travels inside the owner's own failure record, where the registry's incident rule governs it.

Retryability is an explicit mapping from the owner's symbol, not a pattern
over its text. `AFTER_REQUERY` names exactly `REF_CAS_STALE`,
`REF_NAMED_CAS_STALE`, `SESSION_ROOT_ADVANCED`, `SESSION_STALE_HANDLE`,
and `STALE_ROOT`: the last is the emitted S20-390 repository symbol,
numeric 36002 through the S20-360 decision mapping under its registry
name). `AFTER_LIMIT_CHANGE` names exactly
`BRANCH_RESOURCE_LIMIT`, `CANDIDATE_TEST_RESOURCE_LIMIT`,
`CANDIDATE_VALIDATION_RESOURCE_LIMIT`, `CFG_RESOURCE_LIMIT`,
`COMPARE_RESOURCE_LIMIT`, `CONTEXT_CAPSULE_RESOURCE_LIMIT`,
`CONTRACT_TEST_PLAN_RESOURCE_LIMIT`, `EFFECT_RESOURCE_LIMIT`,
`EXCHANGE_RESOURCE_LIMIT`, `FINGERPRINT_RESOURCE_LIMIT`,
`GC_RESOURCE_LIMIT`, `IMPACT_RESOURCE_LIMIT`,
`INDEX_SNAPSHOT_RESOURCE_LIMIT`, `JSON_BRIDGE_RESOURCE_LIMIT`,
`MERGE_RESOURCE_LIMIT`, `PACK_RESOURCE_LIMIT`, `POLICY_ROOT_RESOURCE_LIMIT`,
`QUERY_REQUIRED_FACT_OMITTED`, `QUERY_RESOURCE_LIMIT`,
`REPORT_RESOURCE_LIMIT`, `RESTRICTED_CAPSULE_RESOURCE_LIMIT`,
`SCB_RESOURCE_LIMIT`, `SSMC_RESOURCE_LIMIT`, `TXN_RESOURCE_LIMIT`,
`TYPE_RESOURCE_LIMIT`, `VM_EXEC_RESOURCE_LIMIT`, `VM_LOWER_RESOURCE_LIMIT`,
and `PROTOCOL_LIMIT_EXCEEDED`. `AFTER_CAPABILITY` names exactly
`PROTOCOL_METHOD_UNSUPPORTED` on a reserved method. Every other symbol is
`NEVER`, which is the fail-closed direction: a client retries less than
it might, never more than it should. A new limit symbol answers `NEVER`
until it is listed here and in the implementation, which keeps additions
fail closed. A suffix rule decided this until 2026-09-03 and answered
`NEVER` for `SESSION_STALE_HANDLE` and `STALE_ROOT` while answering
`AFTER_REQUERY` for `REF_CAS_STALE`, because only the last ends in the
word.

`details` are transport-supplied reason bytes, never owner records: an
owner's failure record stays with its owner, and the envelope carries at
most a UTF-8 reason from this set: `SMP1-RESERVED-S20-370`,
`SMP1-RESERVED-S20-620`, `SMP1-METHOD-NOT-NEGOTIATED`,
`FUNCTION-UNKNOWN`, `REPORT-UNKNOWN`. Any other failure carries empty
details.

The response frame carrying a failure envelope sets flag bit 2 (`failed`;
revision 6), so a client distinguishes a failure envelope from an owner
body without decoding either. A request or hello frame carrying bit 2 is
`PROTOCOL_FRAME_INVALID`; an event frame of a streamed failed response
carries it, and the terminal frame of a failed stream carries it with an
empty body (section 7).

## 7. Cancellation and streaming

S20-440 freezes these rules (appendix B carries the exact records).
The two protocol version 2 methods are the stated exception to the
generic charging and streaming rules below: every successful
`entity.version` (306) or `entity.signature` (307) request charges
exactly the ENTITY_READ work bound and emits only a complete single-frame
response, per `docs/spec/ENTITY_READ_PROFILE_V2.md` section 5. The generic
per-byte dispatch-plus-body charge and the section 7 streaming path do not
override that profile; all version 1 accounting is unchanged. The frozen
version 1 rules are:

- **Batch admission.** A batch is an explicit frame list: the canonical
  server answers one frame at a time, and `answer_batch` answers a list
  read together. The server decodes and admits every request identifier
  in order, then executes the surviving requests in order. Admission
  never runs an engine. Admission precedes the cancel below: a cancel
  naming an unadmitted identifier answers the admission failure, and
  equal frame lists over equal repository state produce equal answer
  sequences, whatever transport chunking delivered the list.
- **Cancellation.** Flag bit 0 (`cancel`) on a request frame, or method 603
  whose body names a request identifier, cancels that request when it
  belongs to the same session, is admitted, and has not started
  executing. A cancelled request keeps its place in the identifier
  sequence and is answered `PROTOCOL_CANCELLED` with no body; it never
  runs, and it skips the binding and budget checks below. A request that
  has already completed is answered normally and the cancel
  acknowledges. The cancel latency bound is therefore exactly one request execution: an engine call is never interrupted and never
  yields a partial body. Under single-frame `answer` every request has
  already completed when the next frame is read, so the cancel
  acknowledges; under `answer_batch` a not-yet-executed same-session
  request is answered `PROTOCOL_CANCELLED`.
- **Streaming.** A response whose frame would exceed the negotiated
  `max_frame_bytes` is delivered, only when feature bit 1 (`stream`) was
  negotiated, as ordered event frames (kind 3, flag bit 1) each carrying
  one chunk record, followed by the response frame with flag bit 1, the
  bounded context, and an empty body. `chunk_bytes` bounds the
  `bytes(chunk)` field of each chunk record, never the whole encoded
  frame; `stream_frame_overhead` budgets 512 bytes as the upper bound of
  every non-body frame cost, and `min_stream_chunk` 64 bytes is the
  smallest useful chunk, so a ceiling that cannot carry it is not
  streamable and the response fails `PROTOCOL_LIMIT_EXCEEDED` even with
  the feature. Streaming chunks a body that fits the section 5 ceilings;
  it never ships one that does not. Chunks are exactly `0..total` under
  one session, request identifier, and method, every frame fits the
  ceiling, and a failed response keeps flag bit 2 on every event frame
  and the terminal frame. Without the feature the response is
  `PROTOCOL_LIMIT_EXCEEDED` with no partial body. A reader reassembles
  by concatenating the chunks in order and rejects any reordering, gap,
  foreign frame, or disagreed failure bit as `PROTOCOL_FRAME_INVALID`.
  Chunk event frames carry zero bounds (not counted); the terminal frame
  carries the response's bounded context, which on a failed stream is
  the failure envelope's uncounted zeros.
- **Budgets.** Each session starts with the negotiated `max_work` as its
  budget; dispatch costs one unit up front, and every successful
  response charges one further unit per returned body byte, saturating
  at zero. A request that reaches an engine and fails still costs its
  dispatch unit, so failure loops exhaust the budget instead of burning
  free work. `session.budgets` reports the remaining budget in the
  `max_work` field of the negotiated `LimitProfile`: the field carries
  remaining, not ceiling, and the response is a budget reading, not a
  profile. `session.renew` never resets the budget. A request admitted
  with an exhausted budget is `PROTOCOL_LIMIT_EXCEEDED` before any
  engine runs. `max_inflight` bounds admitted-but-unanswered requests as
  section 3 states. The VM instruction, fuel, value, and output bounds
  of an `execute` request are request-scoped and independent of the
  limit profile; the profile binds the transport outcome (response
  ceilings above, budget here).

## 8. JSON bridge

S20-420 generates the JSON bridge from this contract: the version 1
method table above generates the frozen version 1 metadata, and the
version 2 additions above generate the additive version 2 metadata (the
S20-420 contract names both tables). Byte strings are
lowercase hex, integers are decimal strings when above 2^53, codes and
symbols are preserved verbatim, unknown and omission states are explicit,
and the bridge performs no semantic validation. The integer rule stays
value-conditional on purpose: small integers render as JSON numbers so
the frozen S20-420 vectors and the bridge's field decoders keep one
agreed shape, while large integers never lose precision; JSON is
non-canonical debug rendering and cannot participate in any program
identity.

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
`PROTOCOL_FRAME_INVALID`); version below the selection
(`PROTOCOL_DOWNGRADE`) or above it (`PROTOCOL_VERSION_UNSUPPORTED`);
method-tag validity (`PROTOCOL_METHOD_UNSUPPORTED`); session routing and
liveness (`PROTOCOL_SESSION_CLOSED`, `SESSION_UNKNOWN`); admission
(`PROTOCOL_REQUEST_ID_CONFLICT` for unknown, zero, reused, decreasing,
or cross-session identifiers, `PROTOCOL_SESSION_CLOSED` for a
registry-closed session, `PROTOCOL_LIMIT_EXCEEDED` for the inflight
ceiling); cancellation (`PROTOCOL_CANCELLED`, skipping binding and budget); session binding; budget exhaustion
(`PROTOCOL_LIMIT_EXCEEDED`); method negotiation and reservation
(`PROTOCOL_METHOD_UNSUPPORTED`); body (`PROTOCOL_PAYLOAD_INVALID`);
owner engine (owner codes); response fit (`PROTOCOL_LIMIT_EXCEEDED`);
impossible construction (`PROTOCOL_INTERNAL_INVARIANT`). `SESSION_*`
codes are S20-330's.

## 10. Required evidence

Contract acceptance (S20-400) requires the Ariadne contract review, Nabu
architecture review, and Vulcan surface review of this document with every
report-grade finding closed; the freeze status is the acceptance, so no
status at or past freeze passes the stage gate without all three reviews
`PASS`. Before that, the implemented status means exactly what it says:
implemented under a draft whose review rounds are either superseded by
same-lane `PASS` obligations or itemized as same-lane open findings in
the machine summary and the finding register, never silently pending:
one lane's items never satisfy another lane. The stage gate pins that
invariant, and an empty list set with no same-lane `PASS` fails it. Implementation acceptance (S20-410) requires
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
and handles (S20-330); the generated JSON bridge (S20-420) and CLI
(S20-430); diagnostics and test selection (S20-620); runtime, benchmark,
packaging, release, or GA. Cancellation, streaming, and budgets are not
excluded: section 7 and appendix B freeze the S20-440 rules here, once,
with S20-440 owning their future revisions.

## Appendix A. Body records of the dispatched methods (S20-410)

Bodies are canonical SCB1 values (`uvar` integers, `record` as
`uvar(count) || (uvar(tag) || uvar(len) || bytes)...`, `list` as
`uvar(count) || (uvar(len) || bytes)...`, `union` as
`uvar(tag) || uvar(len) || bytes`, options as the SSMC1 generic union
`0:None | 1:Some`). Fixed identities are raw 32-byte strings. The methods
below are dispatched by the S20-410 deterministic server; the four methods
of appendix C (revision 7) are dispatched as well, so no non-reserved method
answers a deferred detail, and reserved methods answer with the per-seam
reason of section 4.

| Method | Request body | Response body |
|---|---|---|
| 100 `session.open` | `ProtocolHandshakeId[32]`; a claim that is not the negotiated identity is `PROTOCOL_DOWNGRADE` | `SessionId[32]` issued by the S20-330 authority (`sley2.session.v1` over the per-instance server nonce, handshake, workspace, accepted head root, epoch, and issuance ordinal; provisional until S20-330 freezes it) |
| 101 `session.renew` | `SessionId[32]` | the same `SessionId[32]`, rebound to the current accepted head (S20-330; provisional until S20-330 freezes it) |
| 102 `session.close` | empty | empty |
| 103 `session.capabilities` | empty | the `SelectedProfile` record (section 2) |
| 104 `session.budgets` | empty | the `LimitProfile` record |
| 200 `workspace.create` | `record(1: state root stored bytes, 2: policy root stored bytes, 3: list(bytes(object stored bytes)), 4: list(EntityId))` | the genesis `TransactionId[32]`; bodies are S20-390's, the session-to-workspace binding is S20-330's (section 3) |
| 201 `workspace.open` | empty | `revision_summary` of the accepted head; bodies are S20-390's, the session-to-workspace binding is S20-330's (section 3) |
| 202 `refs.list` | `uvar(limit)`, 1 through 4,096 | `list(branch_summary)` |
| 203 `refs.resolve` | branch name bytes | `branch_summary` |
| 204 `revision.read` | `TransactionId[32]` | `revision_summary` |
| 205 `branch.create` | `record(1: name bytes, 2: origin TransactionId)` | `uvar(status)`: 1 created, 2 advanced, 3 present |
| 206 `branch.advance` | `record(1: name bytes, 2: expected head, 3: new head)` | `uvar(status)` |
| 207 `compare` | `record(1: base TransactionId, 2: target TransactionId)` | the S20-510 stored delta bytes |
| 208 `merge.judge` | `record(1: ancestor, 2: ours, 3: theirs)` | `union(1: record(1: merged StateRoot, 2: uvar(objects), 3: bytes(state root stored bytes)) \| 2: the S20-520 stored conflict bytes)` |
| 209 `merge.commit` | `record(1: ancestor, 2: ours, 3: theirs, 4: PrincipalId, 5: uvar(now millis), 6: uvar(expiry millis), 7: branch name bytes)` | `union(1: TransactionId \| 2: the S20-520 stored conflict bytes)` |
| 210 `exchange.export` | empty | the S20-540 exchange stored bytes |
| 211 `exchange.import` | the S20-540 exchange stored bytes | `record(1: RepositoryExchangeId, 2: accepted head TransactionId, 3: uvar(receipts), 4: uvar(branches))` |
| 214 `refs.recover` | empty | `record(1: removed branch stages, 2: removed ref stages, 3: visible branches)` |
| 300 `query.root` | the exact `SLEYRQQ1` request preimage over the accepted head's snapshot; a preimage bound to another snapshot is the owner's `QUERY_SNAPSHOT_MISMATCH` | the `SLEYRQR1` record |
| 301 `query.continue` | as 300 with `after` present (`PROTOCOL_PAYLOAD_INVALID` otherwise) | the `SLEYRQR1` record |
| 302 `capsule` | as 300 | the `SLEYCCP1` record |
| 303 `query.restricted` | the exact `SLEYQRY1` request preimage over the arm-1 snapshot of the accepted head's kinds 4 through 15 | the `SLEYQRS1` record |
| 304 `handle.expand` | `uvar(handle) \|\| StateRoot[32] expected_root`, the binding position with the root it is expected under; any expected root but the session's bound root is `SESSION_STALE_HANDLE` | `record(1: EntityId, 2: uvar(kind), 3: ObjectId, 4: bound StateRoot, 5: SessionId)`; stale after a root advance and after a renewal naming the old root (S20-330; provisional until S20-330 freezes it) |
| 400 `candidate.create` | the canonical S20-350 candidate record payload | the stored candidate bytes |
| 401 `candidate.append` | `record(1: stored candidate bytes, 2: a canonical candidate record payload whose operations and preconditions are appended in order)` | the stored candidate bytes |
| 402 `candidate.validate` | `record(1: base TransactionId, 2: PrincipalId, 3: uvar(now millis), 4: stored candidate bytes)` | the S20-360 candidate result stored bytes |
| 403 `candidate.inspect` | stored candidate bytes | `record(1: CandidateId, 2: WorkspaceId, 3: base TransactionId, 4: base StateRoot, 5: PrincipalId, 6: uvar(operations), 7: uvar(preconditions))` |
| 404 `candidate.discard` | stored candidate bytes | empty; the server holds no candidate state, so a discard verifies the bytes and acknowledges |
| 500 `commit` | `record(1: expected parent TransactionId, 2: PrincipalId, 3: uvar(now millis), 4: stored candidate bytes)` | `record(1: TransactionId, 2: ReceiptId, 3: StateRoot, 4: bytes(candidate result stored bytes))` |
| 501 `receipt.read` | `TransactionId[32]` | the receipt stored bytes |
| 502 `checkout` | `TransactionId[32]` | `record(1: StateRoot, 2: list(bytes(object stored bytes)))` |
| 504 `recovery` | empty | `record(1: removed object stages, 2: removed receipt stages, 3: removed head stages, 4: option(accepted TransactionId), 5: verified ancestry transactions)` |
| 603 `cancel` | `uvar(request_id)` | empty; under single-frame `answer` the named request has already completed, so the cancel acknowledges; under `answer_batch` section 7 applies and a not-yet-executed same-session request is answered `PROTOCOL_CANCELLED` |

```text
branch_summary   = record(1: name bytes, 2: origin TransactionId,
                          3: head TransactionId, 4: head StateRoot)
revision_summary = record(1: TransactionId, 2: StateRoot, 3: PolicyRootId,
                          4: WorkspaceId, 5: SchemaEpochId, 6: uvar(objects),
                          7: uvar(tombstones), 8: ReceiptId)
```

The bounded context of a response copies the owning record's counts: a
`SLEYRQR1` response supplies `returned`, `total_count - returned`, its
truncation flag, and whether a cursor follows; a capsule supplies its
dictionary sizes and status; list responses count their items; every
other body counts one item and its bytes. Owner failures keep their symbol
and numeric code, except the five classes whose crates expose symbols
only and therefore carry numeric `0` at this revision: candidate failures
(S20-350), validation failures (S20-360), state-root and policy-root
failures (S20-110), and pack failures (S20-560). Each owning package
exposes the numerics and closes its row; S20-400 tracks the table but
does not close other packages' codes.

## Appendix B. Cancellation, streaming, and budget records (S20-440)

```text
stream_chunk = record(1: uvar(index), 2: uvar(total), 3: bytes(chunk))
stream_frame_overhead = 512 bytes   // the largest non-body cost of a frame
min_stream_chunk = 64 bytes         // a ceiling that cannot carry it is not streamable
chunk_bytes = max_frame_bytes - stream_frame_overhead
```

Batch order for `n` frames: decode all; for each decoded request frame with
a session, record `(session, request_id)` as cancelled when flag bit 0 is
set, and `(session, target)` when the method is 603 and the body is
`uvar(target)`; then answer frame by frame in order, where a request whose
`(session, request_id)` is cancelled and whose method is not 603 is
admitted, released, and answered `PROTOCOL_CANCELLED`. Admission precedes
the cancel: a cancel naming an unadmitted identifier answers the admission
failure. Frame-level failures are answered without a session and with
identifier 0 in their batch position; 0 is reserved there, never a
session's identifier (section 3). A session-less request refused as
malformed is answered the same way: the forbidden identifier is never
echoed. Equal frame lists over equal repository state produce equal
answer sequences.

Budget accounting: `remaining := max_work` at `session.open`;
`remaining := remaining - 1` at dispatch, then `remaining := remaining -
returned_bytes` after each successful response, saturating at zero; a
request that finds `remaining = 0` after admission is released and
answered `PROTOCOL_LIMIT_EXCEEDED`. `session.renew` leaves `remaining`
untouched.

## Appendix C. Body records of the slice C methods (S20-410 slice C, revision 7; profile selector revision 8)

Revision 7 defines the bodies of the four methods that earlier revisions
left to owner gaps, so the server dispatches every non-reserved method and
no non-reserved method answers a deferred detail, and reserved methods
answer with the per-seam reason of section 4. Bodies follow the
appendix A conventions; identities are raw 32-byte values.

| Method | Request body | Response body |
|---|---|---|
| 212 `gc.dry_run` | `record(1: list(pin))` where `pin = union(1: StateRoot[32], 2: ObjectId[32])` | `gc_report` |
| 213 `gc.collect` | the same | `gc_report`; the server holds the exclusive GC guard for the request |
| 600 `execute` | `record(1: function EntityId[32], 2: list(const_value), 3: limits)` over the session's bound root | `execution_report` |
| 604 `report` | `ExecutionReportId[32]` | the stored `execution_report` |

```text
gc_report        = record(1: list(anchor_key), 2: list(StateRoot[32]) retained roots,
                          3: list(ObjectId[32]) reachable, 4: list(ObjectId[32]) inventory,
                          5: list(ObjectId[32]) deletion candidates,
                          6: uvar(inventory_bytes), 7: uvar(candidate_bytes),
                          8: uvar(decision: 1 dry_run | 2 collected | 3 partial_delete_failure),
                          9: list(ObjectId[32]) deleted, 10: option(ObjectId[32]) failed object)
anchor_key       = record(1: uvar(retention kind 1..7), 2: anchor id[32])
const_value      = the S20-350 mutation value codec's `ConstValue` bytes
                   (`sley_mutate::encode_const_value` / `decode_const_value`)
limits           = record(1: uvar(max_instructions), 2: uvar(max_fuel),
                          3: uvar(max_value_units), 4: uvar(max_output_units),
                          5: option(uvar(cancel_at_fuel)),
                          6: uvar(profile: 1 restricted_v1 | 2 extended_v1))
execution_report = record(1: ExecutionReportId[32], 2: bytes(execution report preimage, `SLEYEXR1`))
```

Rules:

- **The server owns the retention snapshot.** A client can only add
  retention: the request's pins join one `SessionPin` anchor under the
  calling session's identity, and every live session contributes one
  `SessionPin` anchor targeting its bound root, so one session never
  collects another session's bound root (threat T15). Every other anchor
  and every root come from the repository itself: one `Ref` anchor per
  named branch targeting its head root, one `Transaction` anchor for the
  accepted head, the accepted state roots of those revisions, and the
  full root records bound by live sessions (retained at open and renew,
  pruned from retention when no live session binds them). `Tag`, `Lease`,
  `ProtectedRoot`, and `PackManifest` anchors have no repository source
  at this revision, so they enter only through request pins: a client
  that holds such an anchor pins its targets explicitly. A dependency
  root that is not among them fails closed with the S20-180
  `GC_DEPENDENCY_MISSING` code; nothing is deleted. `gc.dry_run` never
  mutates; `gc.collect` acquires the S20-180 exclusive guard for the
  duration of the request and answers its report verbatim, including a
  partial-delete failure.
- **Objects are verified by the production verifier.** Reachability reads
  every object through the S20-560 `RepositoryObjectVerifier`: an object
  is an S20-340 entity object under the conformance schema epoch and
  references no other object (entity references are binding positions of
  the state root, which the planner already traverses), so any other
  record in the inventory fails closed with `GC_OBJECT_REFERENCE_MALFORMED`.
- **Execute is head-bound.** `execute` runs over the session's bound root
  under the S20-330 root check, projects the root's entities with the
  S20-250 full projection, lowers and executes the named Function under
  the cache profile the request selects (`limits` field 6, revision 8: 1 is
  `RESTRICTED_V1`, 2 is `EXTENDED_V1`, the S20-260/S20-270 full profile of
  `docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`; any other value is
  `PROTOCOL_PAYLOAD_INVALID`), and builds the S20-290 execution report,
  whose cache key names the selected profile. Selecting `EXTENDED_V1`
  needs the negotiated `extended_execute` feature bit (bit 4): without
  the intersected bit the selection is `PROTOCOL_PAYLOAD_INVALID`, so no
  session escalates to a profile it never negotiated. The request's VM
  instruction, fuel, value, and output bounds are request-scoped; the
  negotiated profile binds the transport outcome, never the VM run
  (section 7). Every VM, lowering, projection, and report failure keeps
  its owner code. Before answering, the server stores the report preimage
  under `reports/execution/<id hex>` create-once with fsync (S20-560
  report store); an identical re-execution finds the same identity already
  stored and answers it.
- **Report reads the store.** `report` answers the stored record for the
  identity, verifying that the identity re-derives from the stored
  preimage; an unknown identity answers `PROTOCOL_PAYLOAD_INVALID` with
  the detail `REPORT-UNKNOWN`, and a corrupt store entry the S20-560 store
  code. The identity derives from the exact report preimage, so it is a
  bearer capability for a requester that already computed it: store
  dedup confirms only what the requester knew, and answers nothing to a
   guesser. S20-330 may bind report reads to the session workspace in a
   later revision; at this one the store is identity-keyed.

## Appendix D. Body references for the protocol version 2 additions (S20-310, revision 12)

Appendices A and C above carry the 37 legacy body rows and are unchanged
by revision 12. The two version-2 methods define no new serializer: their
request and response records are the ENTITY_READ canonical records,
referenced here and owned by S20-310.

| Method | Request body | Response body |
|---|---|---|
| 306 `entity.version` | `docs/spec/ENTITY_READ_PROFILE_V2.md` section 3 request record (expected root, entity, positive `max_objects`/`max_response_bytes`/`max_work` ceilings) | `docs/spec/ENTITY_READ_PROFILE_V2.md` sections 3-5 response record (exactly the requested object; exact work bound; complete single frame) |
| 307 `entity.signature` | `docs/spec/ENTITY_READ_PROFILE_V2.md` section 3 request record (same shape as 306) | `docs/spec/ENTITY_READ_PROFILE_V2.md` sections 3-5 response record (Function first, then exactly its ordered parameter objects; exact work bound; complete single frame) |

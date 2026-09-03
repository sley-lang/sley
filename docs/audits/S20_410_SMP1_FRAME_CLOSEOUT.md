# S20-410 SMP1 Frame and Deterministic Server Closeout

Status: **implemented under the draft SMP1 contract (S20-400, revision 3); Council reviews pending, so S20-400 and S20-410 are not complete; four methods stay deferred on owner gaps; the Sley 2 goal remains incomplete**

Date: 2026-09-03

Validation tier: **Tier 1 plus protocol-focused Tier 2 handoff**

## Claim under review

SMP1 frames are length-prefixed SCB1 standalone envelopes under the
thirty-fourth identifier domain `sley2.protocol-frame.v1`, whose length is
checked against the negotiated ceiling before allocation and whose digest
is verified before any field is read; the hello record and the derived
selected profile are digested as `ProtocolHandshakeId` with downgrade
detection; request identifiers are session-scoped and strictly increasing;
the method table is the contract's forty-one frozen tags; and a
deterministic server over one repository answers every request frame with
one response frame, dispatching the body to the owning engine as an opaque
frozen record, copying the owner's counts into the bounded context, and
preserving the owner's failure code. The contract is `docs/spec/SMP1.md`
with ADR-0032. It is a draft: every Council lane was unavailable when it
was written and when the implementation landed, so the Ariadne, Nabu, and
Vulcan reviews that freeze it and complete S20-400 and S20-410 are pending
and must pass before the status above changes.

The implementation provides, in `crates/sley-protocol`:

- `lib.rs`: `ProtocolFrame`, `encode_frame`, `encode_hello_frame`,
  `frame_length`, `decode_frame`, `Hello`, `negotiate`, `SelectedProfile`
  (`preimage`, `handshake_id`, `check_claim`, `admits`), `RequestRegistry`
  (`open`, `admit`, `complete`, `close`), `Method` (forty-one tags, five
  reserved), `LimitProfile`, `BoundedContext`, `ProtocolFailure`,
  `Retryability`, `ProtocolErrorCode` with the twelve codes 40000 through
  40011, and the protocol schema epoch record;
- `server.rs`: `Server::answer` with thirty-two dispatched methods (session
  100 through 104; workspace 200 and 201; refs 202, 203, 205, 206, 214;
  revision 204; compare 207; merge 208 and 209; exchange 210 and 211; query
  300 through 303; candidate 400 through 404; commit 500; receipt 501;
  checkout 502; recovery 504; cancel 603), the `SLEYRQQ1` and `SLEYQRY1`
  request decoders, and the body records of appendix A;
- `sley-id`: `ProtocolFrameId` and the frozen registry vector;
- `sley-repo`: the `test-support` feature exposing the trusted-genesis
  fixtures to downstream tests.

## Evidence

- Contract revision 1 at `3a38db0` (fix `bfe96fb`), revision 2 with slice A
  at `d4ff651`, slice B at `bb67ab9`, revision 3 appendix A with slice C at
  `8cd39b3`, slice D (`candidate.append` and the blocker record) in the
  commit after this file.
- Conformance corpus: `conformance/smp1/v1/accepted.json` (client and
  server hello frames, the selected profile and its handshake identity,
  request, response, and failure frames) and `rejected.json` (length above
  the ceiling, trailer bit, truncated envelope, magic bit), drift-gated by
  `scripts/generate_smp1_fixtures.py --check` in `make quick`;
  `scripts/check_smp1_vector.py` rebuilds every frame, the selected profile,
  and both identities with its own SCB1 encoders under the frozen protocol
  epoch identity and classifies every rejection, registered in
  `make conformance`.
- Native tests: four codec tests (frame round trip and defect matrix with
  length-before-allocation, derived negotiation with the downgrade matrix,
  the request-identity matrix, the method table and failure envelope) and
  five server tests (session, repository, and transaction methods with
  byte-identical answers from a second server and 128 repeated reads; the
  query family transporting the exact S20-310, S20-320, and restricted
  records with paging and the owner's mismatch code; identity, session,
  downgrade, and frame rules at the server; a dependency-free root exported
  as a pack byte for byte; the mutation-side family with the genesis
  identity reproduced by `workspace.create`, owner codes preserved through
  candidate, validate, append, and commit over malformed bytes, `merge.commit`
  reaching the merge owner, and an export/import round trip whose accepted
  head is the source genesis); `sley-protocol` 9 tests pass.
- Persistent fuzz: `fuzz/targets/smp1_frame_decoder.rs` (direct, rehashed,
  and hello-negotiation lanes), smoke `PASS` over a 489-seed corpus
  (`docs/audits/S20_700_SMP1_PERSISTENT_SLICE.md`); nineteen scoped targets
  and eighteen smoke gates stand.
- Tier 1: `make quick` green at every commit.
- Tier 2: see the validation record below and the per-slice tables in
  `machineresearch/sley-2.0/s20-410-smp1-frame-campaign-2026-09-03.md`.

## Findings closed in flight

- The epoch registry keys digest domains uniquely per contract, so the
  three-tag framing of revision 1 could not be registered; revision 2 uses
  one contract tag with the hello as frame kind 4.
- Record fields are already length-delimited, so branch names travel as
  raw bytes inside records; the first server test encoded them twice.
- The S20-360 validator renders every outcome as a result record, so
  `candidate.validate` over malformed bytes succeeds with an invalid
  result rather than failing; the test and appendix state this.

## Explicitly open and deferred

- **Council reviews.** Ariadne (owner), Nabu, and Vulcan reviews of
  S20-400 are queued and land as contract revisions.
- **Owner gaps that block four methods.** `gc.dry_run` and `gc.collect`
  wait for a production S20-560 `GcObjectVerifier`; `execute` and `report`
  wait for a public S20-380 `ConstValue` bytes codec and a stored-bytes
  form of the S20-290 execution report envelope. They answer
  `PROTOCOL_METHOD_UNSUPPORTED` with the versioned detail
  `S20-410-SLICE-C-DEFERRED` and are recorded in the machine summary as
  `protocol.deferred_method_blockers`.
- **Numeric exposure.** Validation, candidate, state-root, policy-root, and
  pack failures carry numeric 0 on the wire because their crates expose
  symbols only; commit, branch, exchange, query, and capsule failures carry
  their exact numerics.
- **Session issuance** is provisional (derived from the handshake identity
  and an issue counter) until S20-330 owns it; the server holds no
  candidate state, so `candidate.discard` verifies and acknowledges.
- A server request/response conformance corpus with an independent oracle
  is not attempted: the responses are the owners' frozen records, each
  already covered by its own corpus and oracle.
- S20-440 freezes cancellation latency and streaming; the server answers
  each request before reading the next frame, so `cancel` acknowledges.

## Validation record

Tier 1 `make quick` passed at every commit of the slices. Tier 2 ran on
2026-09-03 after each slice (`make core`, `make conformance`,
`make adversarial`, `make fuzz-smoke`, `make smp1-persistent-fuzz-smoke`)
and again after slice D; the per-slice tables are in the campaign record.
The full `make v1` gate was skipped because this is a subsystem handoff,
not a release boundary; `make v2` and `make release-check` remain
intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

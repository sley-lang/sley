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
  `8cd39b3`, slice D (`candidate.append` and the blocker record) at `6dac878`.
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
- **Owner gaps that blocked four methods (closed by slice C, 2026-09-03).**
  `gc.dry_run` and `gc.collect` waited for a production S20-560
  `GcObjectVerifier`; `execute` and `report` waited for a public S20-380
  `ConstValue` bytes codec and a stored-bytes form of the S20-290 execution
  report envelope. SMP1 revision 7 (appendix C) defines their bodies, the
  S20-560 `RepositoryObjectVerifier` and execution report store, the public
  `sley_mutate::{encode_const_value, decode_const_value}`, and the server
  dispatches all thirty-seven non-reserved methods; see the slice C addendum
  below.
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
and again after slice D at `6dac878` (`make core` 958 tests, all gates exit 0
in 31 seconds); the per-slice tables are in the campaign record.
The full `make v1` gate was skipped because this is a subsystem handoff,
not a release boundary; `make v2` and `make release-check` remain
intentionally fail closed.

## Independent review

Pending. Sessions and verdicts are recorded here when they land.

## Slice C addendum (2026-09-03)

SMP1 revision 7 adds appendix C and the server dispatches `gc.dry_run`
(212), `gc.collect` (213), `execute` (600), and `report` (604):

- the server derives the retention snapshot from its accepted head and
  named branches and lets a request only add session pins, verifies every
  object through the S20-560 `RepositoryObjectVerifier` (entity objects
  under the conformance epoch, no object references), and answers the
  S20-180 report verbatim; `gc.collect` holds the exclusive guard for the
  request;
- `execute` is head-bound: it projects the bound root with the S20-250 full
  projection, lowers and executes the named Function under the restricted
  profile, builds the S20-290 report, and stores its preimage create-once
  under `reports/execution/<id hex>` (S20-560 report store, codes 56000
  through 56002) before answering `record(id, preimage)`; a rejected
  execution is still a report; an unknown Function is
  `PROTOCOL_PAYLOAD_INVALID` with detail `FUNCTION-UNKNOWN`;
- `report` answers the stored record after re-deriving its identity; an
  unknown identity is `PROTOCOL_PAYLOAD_INVALID` with detail
  `REPORT-UNKNOWN`.

Evidence: two server tests (a dry run and a collection over a
dependency-free genesis with a named branch, an unknown pin refused inside
the owner, and reads after collection; an execution of the BoolAnd Function
of the executable test genesis with the identity re-deriving from the
preimage, equal executions answering equal bytes, `report` answering the
stored record, an input-count mismatch answered as a rejected report, and
unknown-function and malformed-value payload failures), the report store
unit test, and the offered-hello test now asserting thirty-seven methods.
The genesis of the main harness names a dependency root the repository
does not hold, so its dry run fails closed with `GC_DEPENDENCY_MISSING`.

Slice C landed at `49c06f7` with `make quick` green; Tier 2 at that commit
(`make core` 985 tests, `make conformance` 19 oracles, `make adversarial`
597 tests, `make fuzz-smoke`, both SMP1 persistent smoke gates, and the
S20-620 and S20-630 smokes, all exit 0) is recorded in
`machineresearch/sley-2.0/s20-410-smp1-frame-campaign-2026-09-03.md`.

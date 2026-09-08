# Bounded entity and signature reads

Status: DRAFT, 2026-09-08; contract review required before implementation.
Owner: S20-310 query semantics, S20-410 protocol integration.
Authority: retained Machine Genesis section 8.2; REWEAVE sections 6–9;
architecture finding AT-MW-02. This draft neither freezes a contract nor
closes the finding.

## 1. Required behavior

`GetEntityVersion` obtains one entity's exact canonical stored object under
one verified root. `GetSignature` obtains a Function object and its ordered
function-parameter objects under that same root. Both are complete, bounded
reads. They grant no mutation, validation, commit, or execution authority.

The signature consists of the Function's declaration type parameters,
ordered function parameters and their exact value types, result type,
declared effect identities, contract identities, and visibility. It is
represented by existing canonical objects, not by a new semantic serializer.
The Function object's block identities are incidental existing fields; block
and operation objects are not returned. Parameter owner, role, and ordinal
must agree with the Function's ordered parameter list.

Nominal type definitions, effect definitions, contract bodies, inferred
transitive effects, and implementation blocks are not expanded by
`GetSignature`. Their identities remain available in the exact objects;
the caller can request a named definition with `GetEntityVersion` under
the same expected root. This is declaration-signature retrieval, not a
replacement for the separately required type/effect/contract queries.

## 2. Version boundary

Add protocol version 2 with method 306 `entity.version` and method 307
`entity.signature`. Tag 305 remains reserved. All existing tags, owner
payloads, numeric errors, query-profile v1 classes, canonical object bytes,
schema epochs, and digest preimages retain their version-1 definitions.

Version 2 uses the existing frame envelope, tag 400, frame epoch, field
schema and digest domain. Its frame's `protocol_version` field is 2.
Existing frame and handshake identities continue to hash the exact bytes,
including the version; there is no alias between v1 and v2 transcripts.
No additional persistent query identity or object format is introduced.

Hello transport uses a version-1 hello frame so a v1 peer can read an offer.
A v2 implementation may offer `[1, 2]`; version negotiation remains the
greatest common offered version. The version-aware negotiation entrypoint
filters 306 and 307 from the intersection when the selected version is 1.
It admits them for version 2 only when both hellos offer that version.
It does not filter other opaque unknown numeric tags. Reserved tags remain
invalid offers in both versions.

Legacy Hello and negotiation entrypoints retain their existing treatment of
unknown numeric tags, including 306 and 307 in a v1-only offer: they accept
the shape and retain the numeric intersection. They do not thereby dispatch
those methods. Preserve the legacy v1 method decoder and reject either tag
on every v1 serving path, including an opaque negotiated intersection that
contains it. The version-aware entrypoint's known-v2-tag filtering is a
separately selected negotiation rule, not a change to the legacy helper.
The caller must choose the same negotiation profile at both endpoints;
the exact selected method set remains bound in the handshake transcript.

An old implementation's inability to consume a v2 offer is not silent
downgrade authority. A caller may explicitly initiate a fresh v1-only
negotiation; no failed or uncertain request is replayed automatically.
Version-1-only offers, negotiation, frames and methods must retain their
existing byte vectors and rejection behavior. Existing v1 codec entrypoints
remain v1-only; version-aware entrypoints select the expected version
explicitly. Post-negotiation framing and `check_claim` use the selected
version, preserving the existing lower-version downgrade and higher-version
unsupported distinction. A v1 session cannot invoke 306 or 307.

## 3. Body records

Bodies use ordinary canonical SCB1 records inside the existing authenticated
SMP frame. All IDs below use their existing exact 32-byte representation.
Unknown, repeated, missing, or out-of-order fields and trailing bytes are
invalid. There is no embedded standalone SCB envelope around these records.

Both request bodies are:

```text
record(
  1: StateRoot expected_root,
  2: EntityId entity,
  3: uvar(max_objects),
  4: uvar(max_response_bytes),
  5: uvar(max_work)
)
```

All three limits are positive and cannot exceed the corresponding selected
`max_entities`, `max_response_bytes`, and `max_work`. They apply to a complete
response. No request-side session or handle can override the frame session.
Only stable EntityIds are accepted. A caller possessing a positional handle
must first use existing `handle.expand`, then retain its root with the
returned EntityId. There is no implicit resolution of a stale handle.

Both response bodies are:

```text
record(
  1: uvar(2),
  2: WorkspaceId workspace,
  3: StateRoot root,
  4: SchemaEpochId epoch,
  5: SessionId session,
  6: EntityId requested_entity,
  7: list(object),
  8: uvar(work_units)
)

object = record(
  1: EntityId entity,
  2: uvar(kind),
  3: ObjectId object_id,
  4: bytes(exact_stored_object)
)
```

For `entity.version`, the list contains exactly the requested object. For
`entity.signature`, it contains the Function first, then exactly its
function-parameter objects in declaration order. It contains no other
objects, sorted substitute ordering, omission marker, cursor, or continuation.
The caller verifies each existing object envelope and ObjectId independently.
Root binding is asserted by the authenticated verified-revision server;
this response is not a standalone Merkle membership proof.

The bytes are copied from `EntityObject` storage in `VerifiedRevision`.
Do not re-encode the decoded Function or Parameter body for the response.
Object identity, record identity/kind, epoch and state-root binding must
agree; disagreement is an invariant failure, never a best-effort answer.

## 4. Authority and failure order

Existing framing, negotiation, request-id, selected-method, cancellation
and session admission order remains unchanged. Both new methods are
head-bound and invoke the existing session authority. Unknown, closed,
wrong-workspace, wrong-epoch, or root-advanced sessions keep the existing
session/protocol codes and precedences.

After that common admission, the owner performs the following ordered steps:

1. Decode the exact request record and validate positive limit ranges;
   malformed records are `PROTOCOL_PAYLOAD_INVALID`, out-of-range positive
   ceilings are `PROTOCOL_LIMIT_EXCEEDED`.
2. Require `expected_root` to equal the live session-bound root and the
   single `VerifiedRevision` used for this response; otherwise
   `QUERY_ROOT_MISMATCH` (31008). Do not search another root.
3. Resolve the target against that root's exact bindings; absent or
   tombstoned entities are `QUERY_UNRESOLVED_ENTITY` (31004).
4. For `entity.signature`, require the target to be a Function;
   otherwise `QUERY_CLASS_NOT_APPLICABLE` (31010).
5. Determine exact result membership and charge the resource checks in
   section 5 before decoding or copying source bytes as specified there.
6. Verify signature relationships and exact borrowed object bindings. A
   missing required Parameter or inconsistent verified body is
   `QUERY_INTERNAL_INVARIANT` (31007), preserving any more specific existing
   storage-verification error encountered before owner entry.
7. Encode the complete response and existing bounded frame. Emit nothing
   until all body and frame limits pass.

Errors from the query owner keep their stable owner numeric codes in the
existing failure envelope. All new-method budget exhaustion and checked
arithmetic overflow return `PROTOCOL_LIMIT_EXCEEDED`. No new error numbers
are allocated. No successful result may hide a missing fact or partial object.

## 5. Resource accounting

The query engine receives an already verified immutable revision. It must
not extract every semantic body, build a whole-root graph/index, hydrate a
cache, checkout the revision, or perform further filesystem reads. Repository
verification and common session admission remain existing prerequisite work;
this profile does not claim to bound that inherited work by query-local
limits. Their separate storage/transaction ceilings remain mandatory.

Let N be the number of root bindings and L be `bit_length(N) + 1`, including
N=0. Use binary lookup on the existing binding order without a new index.
Let K be the exact returned object count and B the sum of their stored byte
lengths. Charge a conservative deterministic work bound:

```text
work_units = 1 + K * L + 2 * B + max_response_bytes
```

Here `max_response_bytes` is the request ceiling, deliberately charged as
the maximum encoding work, not the actual result length. All arithmetic is
checked. A caller choosing an unnecessarily large output ceiling can
therefore exhaust its work limit; this is specified behavior, not an
implementation-dependent cost.

The revision already contains typed `EntityObjectRecord::body` values.
Borrow the Function and Parameter variants directly; do not decode their
stored bytes a second time. Require the K=1 bound for the target's bytes to
fit before traversing its ordered parameter list. If that list's count plus
one exceeds `max_objects`, refuse before allocating the parameter result
list. Resolve each selected Parameter by borrowed lookup; check its length
and the growing K/B bound before traversing its fields or copying bytes.
The final work bound must fit both the request and the session budget as it
stood immediately before this request's existing dispatch charge.

An admitted failed request retains the existing one-unit dispatch charge and
returns no object bytes. A successful new-method request charges exactly
`work_units` in total: credit its already charged dispatch unit and bypass
the existing generic additional body-byte charge. Reserve the remaining
`work_units - 1` before output allocation. Unused reserved work after an
encoding/frame failure is not refunded. These rules apply only to 306/307;
all v1-method accounting remains unchanged.

Compute exact SCB body length from borrowed fields with checked arithmetic
before allocation. Require it not to exceed the request or negotiated
response ceiling. Then compute the complete frame length, including the
existing envelope and prefix accounting, and require the selected frame
ceiling. No object-sized output allocation occurs before its byte ceiling
is established; no partial response is streamed. The inherited failure
envelope floor must still fit when the successful body does not.

BoundedContext reports `applied_limits` as the selected LimitProfile,
`returned_bytes` as the exact response-body size, `returned_entities = K`,
`returned_edges = 0`, `reached_depth = 0`, `omitted = 0`, `truncated = false`,
and `continuation = false`. These are direct reads, not graph traversal;
there is no depth omission. Work is carried only in response field 8 and
the session budget, because BoundedContext has no work field.

## 6. Ownership and integration

S20-310 owns membership, signature selection and query-owner failures.
S20-390's `VerifiedRevision` remains the only persistent-state authority.
S20-410 owns session/root admission, version negotiation, bounded transport
and protocol failures. A thin repository adapter may expose borrowed objects
to the query engine; it may not supply unverified caller-owned bindings as
production truth. The bridge and CLI delegate without semantic interpretation.

Before freeze, synchronize SMP1's versioned method table and history, the
bridge and CLI version/revision pins, REQUIRED_CONTRACT_INDEX_V1, and
SLEY2_TRIAL_RUNNER_V1's `ARM_AFFORDANCES` allowlist. Generate bridge metadata
from the versioned contract; never add a bridge-private method. Existing root
query v1 and capsule formats remain byte-identical.

## 7. Acceptance

- Fixed accepted/rejected request and response vectors are reproduced by an
  independent Python encoder/decoder using the existing canonical object
  oracle. Compare raw bytes, ObjectIds, frame identities and stable failures.
- All eighteen supported entity kinds return their exact stored object.
  Signature cases include zero/multiple ordered parameters, nontrivial type
  expressions, generics, declared effects/contracts and a wrong-kind target.
- Wrong root/session/workspace/epoch, closed and renewed sessions, head
  advancement, absent/tombstoned entities, malformed/trailing payloads,
  reordered fields and conflicting request identities reject in stated order.
- Exact and one-below object, response, frame, and work limits are tested;
  oversized Function/Parameter input and checked overflow refuse before
  result allocation. Instrument the owner boundary to prove no whole-root
  semantic extraction, cache construction, checkout or unbounded body copy.
- v1 negotiation, old frames, method offers and all existing byte vectors are
  unchanged; v2 mixed negotiation filters new methods on a v1 selection,
  and a v1 session rejects both new tags. Include legacy unknown-tag
  intersections containing 306, 307 and an unrelated unknown number.
- A real runner/stdio scripted EC1a edit performs body query, candidate
  creation and validation through ARM_AFFORDANCES without checkout,
  exchange export or fixture-side knowledge of the pre-edit body. Add a
  signature edit using retrieved parameter types to prove usable context.
- Ariadne reviews the contract, Nabu reviews bounded-context architecture,
  and Vulcan reviews the serving and negative-test surface on pinned commits.

AT-MW-02 remains open until implementation, independent vectors, consumer
integration and the bounded edit demonstrations pass. A contract-only
checkpoint is not a runtime or full-query completion claim.

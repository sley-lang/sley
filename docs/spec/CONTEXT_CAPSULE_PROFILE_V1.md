# Context Capsule Profile v1

Status: S20-320 full contract draft, revision 4 (2026-09-14; revision 4
binds `Complete` to the whole result in the builder, enforces the table
ceilings at their encoding point, wires the internal-invariant code to the
dictionary structure check, enumerates the exact source-checked set,
states the kind-0 and walk-coverage rules normatively, and restates the
evidence set to what the builder, tests, fuzz target, and independent
reproduction actually prove); implemented
under this draft with Council review pending (Ariadne contract review, Nabu
architecture review, Vulcan surface review), so the contract is not frozen
and the package is not complete. Implementation state is tracked in the
machine summary.

This profile completes S20-320. It defines the master context capsule: a
deterministic evidence envelope over one root-backed query, carrying the
question that was asked, the verified provenance it was answered under, its
exact omission and continuation status, the exact response record, and the
facts of that response reorganized into raw-identity dictionaries. It
composes, and never alters:

- the restricted profile `docs/spec/RESTRICTED_QUERY_CAPSULE_PROFILE_V1.md`:
  its `SLEYRQC1` record, `sley2.restricted-query-capsule.v1` identities,
  codes 32000 through 32007, and its restricted-response source stay
  normative and byte-identical;
- the S20-310 full profile `docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md`:
  the request, the response record, the typed cursors, and the exact
  `total_count` that make omission lawful;
- the S20-300 full profile and the S20-390 verified revision, through which
  every provenance fact of a root-backed response was verified.

The authority rule extends the restricted one:

> A context capsule can restate the question, the verified provenance, the
> omission status, and the facts of one root-backed response; it cannot
> add, omit, resume, import, or authorize facts, and it is never a session.

## 1. Identity and source

ADR-0013 already registers the master domain:

```text
sley2.context-capsule.v1 -> ContextCapsuleId
ContextCapsuleId = BLAKE3-256("sley2.context-capsule.v1" || capsule_preimage)
```

The two public construction paths are
`build_context_capsule(request, response)` over one `RootQueryRequest` and
the `RootQueryResponse` the engine produced for it, which carries arm
`None`; and `SessionAuthority::bind_context_capsule(session, request,
response)` (S20-330 session authority), which carries arm `Negotiated`.
The two are bound by `request.query_id == response.query_id`, and both
can only be constructed by the S20-310 full engine, whose input binding
verified the root, epoch, workspace, snapshot, bodies, and bindings.
Query failures, partial bytes, raw `SLEYRQR1` records, restricted
responses, caller-provided dictionaries, and caller-declared provenance
cannot construct a capsule: the session arm takes no provenance arguments,
and whether a response's provenance equals a live session's binding is
known only to the authority, which refuses unknown and closed sessions.

## 2. Provenance and session binding

```text
Provenance {
  workspace_id: WorkspaceId,
  schema_epoch: SchemaEpochId,
  root:         StateRoot,
  snapshot_id:  IndexSnapshotId,
  query_id:     RootQueryId
}
SessionBinding = None(1) | Negotiated(2); when the arm is
Negotiated(2), SessionId[32] follows the arm tag.   // S20-330
```

Provenance is copied from the response, whose fields the engine bound to a
verified root; the capsule adds no claim of its own. A capsule built
outside a session carries arm `None`. A capsule built under a negotiated
session (S20-330, `SessionAuthority::bind_context_capsule`) carries arm
`Negotiated` with the `SessionId`, and minting fails `SESSION_UNKNOWN`
when the session is not live under the authority, and
`CONTEXT_CAPSULE_SOURCE_INVALID` when the response's workspace, root, or
epoch differs from the session's authority-held binding. The capsule is
root-, epoch-, and workspace-bound evidence, and never a handle: no
consumer may read `capsule.session()` as session authority, and the arm
carries the binding the authority verified, not a right to act.

## 3. Question

The capsule restates the exact request so the evidence is self-describing:

```text
Question {
  class_tag:          u32,
  class_body:         (the S20-310 class_body grammar),
  limits:             query_limits,
  allow_continuation: u32 (1 false | 2 true),
  after:              option(cursor)
}
```

## 4. Omission and continuation status

```text
Completeness = Complete(1) | Page(2)
Truncation   = False(1) | True(2)
Status {
  completeness, truncation,
  total_count: u64, returned: u64, omitted: u64,   // omitted = total_count - returned
  next_after:  option(cursor)
}
```

`Complete` holds exactly when the response is not truncated and the
question had no `after` cursor; every other response is a `Page`. The
builder refuses a `Complete` whose `returned` differs from `total_count`
or that carries a `next_after`, with `CONTEXT_CAPSULE_SOURCE_INVALID`, so
a dropped fact can never present as complete; `omitted` is then zero by
construction (`total_count - returned`). A `Page` carries no completeness
promise: a remainder page (no truncation, fewer than the total) is a
`Page` with `omitted > 0`, while a degenerate page can carry `omitted =
0`, so `Page` never implies a missing fact. `omitted` counts every fact
of the complete result that this capsule does not carry, before and after
the window. `returned` counts response items, not dictionary entries: a
class whose payload carries no identities (class 1, or a `Fingerprint`
over nothing queried) still returns its item. Because `total_count` is
exact and the key order is canonical, a reader holding the capsules of a
full continuation walk can prove it holds every fact, and a single capsule
can never present a page as complete. A walk covers the result exactly
when its capsules share one `(workspace, epoch, root, snapshot, class
tag, class body, limits, allow_continuation)` tuple, the first carries no
`after` cursor, each next `after` equals the previous `next_after`, the
last carries no `next_after`, all `total_count` values agree, and the
`returned` values sum to `total_count`; the join key is that tuple, not
the `query_id`.

## 5. Facts

The facts are derived only from the typed response result and the question:

- `entities`: strict raw-`EntityId`-sorted unique list of every identity in
  the payload (entities, rows, inventory entries, edge endpoints) plus the
  question's named entities and seeds. This list is not an existence,
  presence, or membership claim: it names the identities the question
  asked about or the payload carried, whether or not any fact about them
  follows;
- `kinds`: one `u32` per dictionary entry, the SSMC1 kind when the payload
  states it (class 2 and class 8 entries) and `0` otherwise. Kind `0`
  means "kind not stated by this payload", never "this entity has no
  kind": the SSMC1 kind tags start at 1, so `0` is reserved by this
  profile and collides with no stated kind. A namespace the question
  names but the payload never states (class 8) carries kind `0`;
- `relationships`: for edge results, `(dependent_index, dependency_index,
  impact_kind)` in source edge order, indexes into `entities`;
- `roots`: strict raw-sorted unique `StateRoot` list from class 7 rows and
  class 11 results;
- `objects`: `(entity_index, ObjectId)` for class 2;
- `fingerprints`: `(entity_index, Fingerprint)` for class 2 and class 3
  results that carry one.

No label, type expression, path, source position, ranking, summary,
diagnostic, mutation affordance, or caller-supplied identity enters the
facts. Single-entity classes attribute their payload to the question's
subject: the first named entity, read through the query's declared
subject accessor, never a positional call-site convention. Class 1 (root
summary) yields empty dictionaries; its facts are the
copied record.

## 6. Canonical record

All integers are fixed-width big endian; lists, options, and cursors use
the S20-310 encodings. `bytes(x)` is the raw bytes of `x` with no length
prefix; every other variable-length field in this preimage is
length-prefixed as written.

```text
capsule_preimage =
  "SLEYCCP1" || u32be(format_version=1) || u32be(profile_version=1) ||
  WorkspaceId[32] || SchemaEpochId[32] || StateRoot[32] ||
  IndexSnapshotId[32] || RootQueryId[32] ||
  u32be(session_binding) || (SessionId[32] when session_binding = 2) ||
  u32be(class_tag) || class_body || query_limits ||
  u32be(allow_continuation) || option(cursor, after) ||
  u32be(completeness) || u32be(truncation) ||
  u64be(total_count) || u64be(returned) || u64be(omitted) ||
  option(cursor, next_after) ||
  u64be(response_bytes) || bytes(exact_SLEYRQR1_response_record) ||
  list(EntityId[32], entities) || list(u32be(kind), kinds) ||
  list(u32be(dependent_index) || u32be(dependency_index) ||
       u32be(impact_kind), relationships) ||
  list(StateRoot[32], roots) ||
  list(u32be(entity_index) || ObjectId[32], objects) ||
  list(u32be(entity_index) || Fingerprint[32], fingerprints)

capsule_record = capsule_preimage || ContextCapsuleId[32]
```

Every repeated field must equal the trusted request and response getters,
and the builder checks exactly this set, in order: the `query_id`,
`snapshot_id`, root, and class tag bind the request to the response; the
copied record must begin `SLEYRQR1` and have the response's exact length;
`returned` must equal the response item count and must not exceed
`total_count`; truncation must agree with carrying a `next_after`; and a
response counted `Complete` (not truncated, no `after` cursor) must
return exactly `total_count` with no `next_after`. The workspace, epoch,
snapshot, and root provenance is copied from the response, whose fields
the engine bound to a verified root; the builder cross-checks provenance
against nothing of its own, because only the session authority holds an
independent binding to compare against (section 2). `u64be(response_bytes)`
is the byte length of the copied record
(the builder refuses a record whose length differs with
`CONTEXT_CAPSULE_SOURCE_INVALID`), and `kinds` must have the length of
`entities` (a mismatch is a builder defect and carries
`CONTEXT_CAPSULE_INTERNAL_INVARIANT`). The trailer is
outside its own preimage. There is no public record decoder, importer,
hydrator, continuation expander, or constructor from components.

## 7. Limits and work

| Limit | Maximum |
|---|---:|
| source response bytes | 33,554,432 |
| entity dictionary entries | 65,535 |
| relationships | 400,000 |
| roots, objects, fingerprints (each) | 65,535 |
| complete capsule record | 67,108,864 bytes |
| charged derivation and encoding work | 100,000,000 |

Work charges one unit per inspected result item, dictionary insertion or
lookup, relationship projection, copied source byte, and emitted capsule
byte. Charging happens during derivation, item by item, so every
allocation the builder performs is already charged; the table-length
gates (dictionaries, relationships, roots, objects, fingerprints) run
after derivation and before encoding, and the transient derivation state
is bounded by the source ceiling (a 33,554,432-byte response bounds every
dictionary the derivation can build). A response larger than the
source ceiling cannot be capsuled; the failure returns no capsule.

## 8. Repository surface

`sley-repo` exposes `run_context_capsule(repository, revision, query,
limits, allow_continuation, after)`, which runs the S20-310 full surface
and builds the capsule from its request and response. It is read-only
derived evidence and grants no authority.

## 9. Stable failures and precedence

Codes 32000 through 32007 are unchanged. This profile appends:

| Numeric | Symbolic code |
|---:|---|
| 32008 | `CONTEXT_CAPSULE_SOURCE_INVALID` |
| 32009 | `CONTEXT_CAPSULE_DICTIONARY_INVALID` |
| 32010 | `CONTEXT_CAPSULE_RESOURCE_LIMIT` |
| 32011 | `CONTEXT_CAPSULE_INTERNAL_INVARIANT` |

Precedence: request and response identity or record disagreement
(`CONTEXT_CAPSULE_SOURCE_INVALID`); count, byte, and work preflight
(`CONTEXT_CAPSULE_RESOURCE_LIMIT`); dictionary or index canonicality
(`CONTEXT_CAPSULE_DICTIONARY_INVALID`); checked encoding and identity
derivation (`CONTEXT_CAPSULE_INTERNAL_INVARIANT`). The dictionary code
fires on index misses, which cannot occur for engine-produced pairs
(every attributed identity is collected before indexing) and stands as
defense in depth; the invariant code fires on the dictionary structure
itself (exactly one kind per entry), which is a builder defect, never a
source defect. S20-310 failures occur
before construction and keep their `QUERY_*` codes; session failures occur
before construction and keep their `SESSION_*` codes (`SESSION_UNKNOWN`
for a session that is not live).

## 10. Required evidence

Implementation acceptance requires at least:

- fixed capsule record and identity vectors for all nineteen classes and
  for both pages of a continuation walk over the frozen S20-310 fixture,
  with an independent Python reproduction from the fixture records;
  one vector carries the `Negotiated` arm under a fixed fixture session,
  reproduced by the same independent path;
- 128 equal derivations producing byte-identical records and identities;
- proof that a page capsule is never `Complete`, that `omitted` equals
  `total_count - returned`, that a `Complete` returns exactly the total
  with no `next_after`, and that the capsules of a walk cover the
  complete result under the section 4 predicate;
- the source binding matrix: swapped request/response pairs fail without
  a capsule at the builder; a response whose provenance drifts from the
  session's authority-held binding fails at the session authority; a
  restricted response cannot reach the builder at all, because it is a
  distinct type the builder never accepts (type-excluded, not
  runtime-checked);
- the dictionary, index, count, size, and work failure paths: the builder
  enforces every ceiling before encoding and refuses inconsistent
  sources, but inconsistent engine pairs are unconstructible (responses
  originate only in the engine, whose outputs satisfy the checks by
  construction), so these paths are defense in depth, unreachable
  through any public input, and covered by inspection rather than by a
  failing execution;
- a repository test producing the same capsule from a cache hit and a
  rebuild;
- an S20-700 persistent libFuzzer target over the capsule builder,
  covering the `Negotiated` arm encoding;
- Tier 1 plus semantics-focused Tier 2 validation;
- Ariadne contract review, Nabu architecture review, and Vulcan surface
  review with every report-grade finding closed.

## 11. Explicit exclusions

This contract does not claim:

- session handles: a `Negotiated` capsule carries the binding the session
  authority verified and grants no right to act (S20-330 owns the
  authority; section 2 names the only minting path);
- SMP1 transport (S20-400) or any protocol framing;
- diagnostics, mutation affordances, type, effect, contract, or test facts
  beyond what the nineteen classes return;
- cross-root, cross-repository, or imported capsules;
- runtime, benchmark, packaging, release, or GA.

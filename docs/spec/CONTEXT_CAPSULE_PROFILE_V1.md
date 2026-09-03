# Context Capsule Profile v1

Status: S20-320 full contract draft, revision 1 (2026-09-03); implemented
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

The only public constructor is
`build_context_capsule(request, response)` over one `RootQueryRequest` and
the `RootQueryResponse` the engine produced for it. The two are bound by
`request.query_id == response.query_id`, and both can only be constructed
by the S20-310 full engine, whose input binding verified the root, epoch,
workspace, snapshot, bodies, and bindings. Query failures, partial bytes,
raw `SLEYRQR1` records, restricted responses, caller-provided
dictionaries, and caller-declared provenance cannot construct a capsule.

## 2. Provenance and session binding

```text
Provenance {
  workspace_id: WorkspaceId,
  schema_epoch: SchemaEpochId,
  root:         StateRoot,
  snapshot_id:  IndexSnapshotId,
  query_id:     RootQueryId
}
SessionBinding = None(1)          // Negotiated(2) reserved for S20-330
```

Provenance is copied from the response, whose fields the engine bound to a
verified root; the capsule adds no claim of its own. The session binding
is the fixed arm `None` at this revision: S20-330 owns negotiated session
authority and will add the `Negotiated` arm as a contract revision. The
capsule is therefore root-, epoch-, and workspace-bound evidence, and never
a handle.

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
question had no `after` cursor; every other response is a `Page`.
`omitted` counts every fact of the complete result that this capsule does
not carry, before and after the window. Because `total_count` is exact and
the key order is canonical, a reader holding the capsules of a full
continuation walk can prove it holds every fact, and a single capsule can
never present a page as complete.

## 5. Facts

The facts are derived only from the typed response result and the question:

- `entities`: strict raw-`EntityId`-sorted unique list of every identity in
  the payload (entities, rows, inventory entries, edge endpoints) plus the
  question's named entities and seeds;
- `kinds`: one `u32` per dictionary entry, the SSMC1 kind when the payload
  states it (class 2 and class 8 entries) and `0` otherwise;
- `relationships`: for edge results, `(dependent_index, dependency_index,
  impact_kind)` in source edge order, indexes into `entities`;
- `roots`: strict raw-sorted unique `StateRoot` list from class 7 rows and
  class 11 results;
- `objects`: `(entity_index, ObjectId)` for class 2;
- `fingerprints`: `(entity_index, Fingerprint)` for class 2 and class 3
  results that carry one.

No label, type expression, path, source position, ranking, summary,
diagnostic, mutation affordance, or caller-supplied identity enters the
facts. Class 1 (root summary) yields empty dictionaries; its facts are the
copied record.

## 6. Canonical record

All integers are fixed-width big endian; lists, options, and cursors use
the S20-310 encodings.

```text
capsule_preimage =
  "SLEYCCP1" || u32be(format_version=1) || u32be(profile_version=1) ||
  WorkspaceId[32] || SchemaEpochId[32] || StateRoot[32] ||
  IndexSnapshotId[32] || RootQueryId[32] || u32be(session_binding=1) ||
  u32be(class_tag) || class_body || query_limits ||
  u32be(allow_continuation) || option(cursor, after) ||
  u32be(completeness) || u32be(truncation) ||
  u64be(total_count) || u64be(returned) || u64be(omitted) ||
  option(cursor, next_after) ||
  bytes(exact_SLEYRQR1_response_record) ||
  list(EntityId[32], entities) || list(u32be(kind), kinds) ||
  list(u32be(dependent_index) || u32be(dependency_index) ||
       u32be(impact_kind), relationships) ||
  list(StateRoot[32], roots) ||
  list(u32be(entity_index) || ObjectId[32], objects) ||
  list(u32be(entity_index) || Fingerprint[32], fingerprints)

capsule_record = capsule_preimage || ContextCapsuleId[32]
```

Every repeated field must equal the trusted request and response getters,
the copied record must begin `SLEYRQR1` and have the response's exact
length, and `kinds` must have the length of `entities`. The trailer is
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
byte, checked before allocation or append. A response larger than the
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
derivation (`CONTEXT_CAPSULE_INTERNAL_INVARIANT`). S20-310 failures occur
before construction and keep their `QUERY_*` codes.

## 10. Required evidence

Implementation acceptance requires at least:

- fixed capsule record and identity vectors for all nineteen classes and
  for both pages of a continuation walk over the frozen S20-310 fixture,
  with an independent Python reproduction from the fixture records;
- 128 equal derivations producing byte-identical records and identities;
- proof that a page capsule is never `Complete`, that `omitted` equals
  `total_count - returned`, and that the capsules of a walk cover the
  complete result;
- the source binding matrix (foreign request, drifted response, restricted
  response) and the dictionary, index, count, size, and work perturbations
  failing without a capsule;
- a repository test producing the same capsule from a cache hit and a
  rebuild;
- an S20-700 persistent libFuzzer target over the capsule builder;
- Tier 1 plus semantics-focused Tier 2 validation;
- Ariadne contract review, Nabu architecture review, and Vulcan surface
  review with every report-grade finding closed.

## 11. Explicit exclusions

This contract does not claim:

- negotiated sessions or handles (S20-330), whose `Negotiated` arm is
  reserved and not constructible;
- SMP1 transport (S20-400) or any protocol framing;
- diagnostics, mutation affordances, type, effect, contract, or test facts
  beyond what the nineteen classes return;
- cross-root, cross-repository, or imported capsules;
- runtime, benchmark, packaging, release, or GA.

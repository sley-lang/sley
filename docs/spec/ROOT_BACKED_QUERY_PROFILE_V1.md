# Root-Backed Query Profile v1

Status: S20-310 full contract draft, revision 1 (2026-09-03); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). No implementation exists at this revision. Implementation state is
tracked in the machine summary.

This profile completes S20-310. It defines the nineteen root-backed query
classes the master goal requires, exact bounded semantics for each, lawful
truncation with explicit continuation, and the one repository surface that
answers them over a verified revision through the S20-300 full complete-root
snapshot. It composes, and never alters:

- the restricted profile `docs/spec/RESTRICTED_QUERY_PROFILE_V1.md`: its
  four queries, `sley2.query.v1` identities, `SLEYQRY1`/`SLEYQRS1` records,
  codes 31000 through 31007, and arm-1 binding stay normative and
  byte-identical; the restricted S20-320 capsule keeps wrapping them;
- the S20-300 full profile `docs/spec/COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md`:
  the arm-2 snapshot bound to the exact root and the repository index cache,
  whose hits this profile is the first and only consumer of;
- the S20-250 full profile: entity bodies, the twelve impact kinds, and the
  complete-root closure judgment;
- `docs/spec/STATE_ROOT_V1.md` and the S20-390 verified revision, which
  supply every root fact, binding, and object.

The authority rule extends the restricted one:

> A root-backed query may inspect only facts already verified for one exact
> root: the revision's record, its inventory-checked objects, and an arm-2
> snapshot bound to that root. No candidate bytes, claimed root, label, path,
> clock, or caller-declared fact can supply an answer, and no answer can omit
> a fact silently.

## 1. Input binding

```text
RootQueryInput {
  snapshot:     IndexSnapshot            // arm 2, context (epoch, Some(root))
  entities:     [ImpactEntity]           // the complete-root request bodies
  facts:        CompleteRootFacts        // bound_entities, entry_points, dependency_roots
  bindings:     [(EntityId, ObjectId)]   // the record's entity_bindings, in order
  fingerprints: [(EntityId, Fingerprint)]// field-4 fingerprints, raw EntityId order
  root:         StateRoot
  workspace_id: WorkspaceId
  schema_epoch: SchemaEpochId
  contract_root, test_root: ObjectId
  policy_root:  PolicyRootId
}
```

The input is accepted only when all of the following hold, else the query
fails `QUERY_ROOT_MISMATCH` before any class runs:

1. `snapshot.completeness = CompleteRoot(2)` and
   `snapshot.context = (schema_epoch, Some(root))`;
2. the snapshot inventory identities equal `bindings` identities and
   `facts.bound_entities`, in order, and every inventory kind equals the
   entity's SSMC1 kind;
3. `entities` are in raw `EntityId` order with no duplicates and their
   identities equal the inventory;
4. `fingerprints` identities are a subset of the inventory in raw order, and
   every carried fingerprint belongs to a `TypeDef` or `Function`.

The pure engine lives in `sley-query` and performs no I/O. It does not
re-judge closure rules C1 through C11: the arm-2 snapshot exists only for a
request that passed them, and the binding above ties the bodies to that
snapshot. The repository surface of section 7 is the only producer of a
`RootQueryInput` from persistent state.

## 2. Query classes

Tags are the exact `u32be` class tags. Kinds are SSMC1 tags 1 through 18;
edge kinds are the S20-250 impact tags 1 through 12.

| Tag | Class | Body | Result | Key |
|---:|---|---|---|---|
| 1 | `GetRootSummary` | none | root summary | single |
| 2 | `GetEntity { entity }` | entity | `(kind, object_id, fingerprint?)` | single |
| 3 | `GetSemanticFingerprint { entity }` | entity | `fingerprint?` | single |
| 4 | `ListEntitiesByKind { kind }` | kind | `list(entity)` | entity |
| 5 | `ListWorkspacePackages` | none | `list(entity)` | entity |
| 6 | `ListPackageExports { package }` | entity | `list(entity)` | entity |
| 7 | `ListPackageDependencies { package }` | entity | `list(dependency_row)` | entity |
| 8 | `ListNamespaceMembers { namespace }` | entity | `list(inventory_entry)` | entity |
| 9 | `ListOwningNamespaces { entity }` | entity | `list(entity)` | single |
| 10 | `ListEntryPoints` | none | `list(entry_row)` | entity |
| 11 | `ListDependencyRoots` | none | `list(StateRoot)` | root |
| 12 | `ListDirectDependencies { entity, kinds }` | entity, filter | `list(edge)` | edge |
| 13 | `ListDirectDependents { entity, kinds }` | entity, filter | `list(edge)` | edge |
| 14 | `ReverseImpactClosure { seeds }` | seeds | `list(entity)` | entity |
| 15 | `ForwardDependencyClosure { seeds }` | seeds | `list(entity)` | entity |
| 16 | `ListContractsFor { target }` | entity | `list(entity)` | entity |
| 17 | `ListTestsFor { target }` | entity | `list(entity)` | entity |
| 18 | `ListDeclaredEffects { entity }` | entity | `list(entity)` | entity |
| 19 | `ListCapabilityRequirementsFor { subject }` | entity | `list(entity)` | entity |

Exact semantics:

- `GetRootSummary` returns `workspace_id`, `schema_epoch`, `root`,
  `contract_root`, `test_root`, `policy_root`, the eighteen per-kind entity
  counts in kind order, the entry-point count, the dependency-root count,
  and the snapshot's direct-edge count.
- `GetEntity` returns the inventory kind, the bound `ObjectId`, and the
  field-4 fingerprint when the object carries one.
- `GetSemanticFingerprint` returns the field-4 fingerprint when present. An
  absent fingerprint is the exact fact `None`; the class applies to every
  kind.
- `ListEntitiesByKind` returns every inventory identity of the kind.
- `ListWorkspacePackages` returns the workspace body's `packages` set.
- `ListPackageExports` returns the package body's `exports` set;
  `ListPackageDependencies` returns one row `(binding, dependency_root,
  external_package, local_namespace)` per identity in the package body's
  `dependencies` set, keyed by the binding identity.
- `ListNamespaceMembers` returns one inventory entry per identity in the
  namespace body's `members` set.
- `ListOwningNamespaces` returns the owning namespace of the entity followed
  by each parent up to the root namespace (inner to outer). For a namespace
  the chain starts at its parent. Kinds 1, 2, 6, 7, 8, and 18 never have an
  owner (closure rule C7), so their chain is exactly empty. The chain is a
  single fact: it is never paged.
- `ListEntryPoints` returns one row `(entry_point, function, exposure)` per
  entry point in `facts.entry_points`; `ListDependencyRoots` returns
  `facts.dependency_roots`.
- `ListDirectDependencies`, `ListDirectDependents`, and
  `ReverseImpactClosure` keep the restricted profile's exact semantics over
  the arm-2 snapshot (every kind, every edge). `ForwardDependencyClosure`
  is the mirror over direct edges: the seeds at depth zero plus every entity
  reachable along `dependent -> dependency` edges, strict raw order.
- `ListContractsFor` returns every `Contract` whose body `target` is the
  entity; `ListTestsFor` every `TestCase` whose body `target` is the entity.
- `ListDeclaredEffects` applies to `Function` and `AdapterImport` (the
  `effects` set) and to `CapabilityRequirement` (its single `effect`);
  every other kind fails `QUERY_CLASS_NOT_APPLICABLE`.
- `ListCapabilityRequirementsFor` returns the union, in raw order without
  duplicates, of the `requirements` of every `PolicyBinding` whose `subject`
  is the entity, plus the workspace body's `capability_requirements` when
  the entity is the workspace.

Filters, seeds, and named entities keep the restricted rules: nonempty
strictly increasing filter tags resolving to kinds 1 through 12, nonempty
strict raw-order seed lists, and every named identity present in the
inventory. A `kind` body must resolve to an SSMC1 tag 1 through 18, else
`QUERY_UNSUPPORTED`. A class whose body names an entity of a kind the class
does not apply to (`ListPackageExports` on a non-package, and so on) fails
`QUERY_CLASS_NOT_APPLICABLE`.

## 3. Exact results, paging, and continuation

Every list class is computed completely under the applied `max_work` and
the profile ceilings (65,535 inventory entries, 400,000 snapshot edges,
65,535 seeds and depth). The exact result is then ordered by its key (raw
`EntityId`, canonical `(dependent, dependency, kind)` edge order, or raw
`StateRoot` order) and paged:

```text
page = items whose key > after (all items when after is None),
       taking at most the applied entity or edge limit
total_count = exact complete count
returned    = items in the page
truncated   = returned < items after the cursor
next_after  = key of the last returned item when truncated, else None
```

`allow_continuation = false` keeps the restricted rule: a truncated page is
`QUERY_REQUIRED_FACT_OMITTED` and nothing is returned. With
`allow_continuation = true` the page is returned with `truncated = true`
and `next_after`, and the caller continues with `after = next_after`. The
`after` cursor must be the class's key type (`QUERY_CONTINUATION_INVALID`
otherwise); it need not name an item of the result. Because `total_count`
is exact on every page and the key order is canonical, the union of the
pages is the complete result and no page can hide a fact. Closure depth
keeps the restricted rule: a `max_depth` that cuts a closure short is
`QUERY_REQUIRED_FACT_OMITTED`, never a page. Single-key classes never page:
their whole result must fit the applied limits or the query fails
`QUERY_REQUIRED_FACT_OMITTED`.

## 4. Limits

`QueryLimits` and its ceilings are the restricted profile's, unchanged. The
request preimage ceiling is 4,194,304 bytes, the response record ceiling
67,108,864 bytes, and charged work 100,000,000. Work charges one unit per
inventory lookup, body field visited, expanded entity, examined edge, and
emitted response byte.

## 5. Request identity

All integers are fixed-width big endian; lists and options use the
restricted encodings. The thirty-third identifier domain is
`sley2.root-query.v1 -> RootQueryId`.

```text
root_query_preimage =
  "SLEYRQQ1" || u32be(format_version=1) || u32be(profile_version=1) ||
  IndexSnapshotId[32] || SchemaEpochId[32] || StateRoot[32] ||
  WorkspaceId[32] || u32be(completeness=2) || u32be(limits_profile=1) ||
  query_limits || u32be(allow_continuation: 1 false | 2 true) ||
  option(cursor, after) || u32be(class_tag) || class_body

cursor = u32be(1) || EntityId[32]
       | u32be(2) || dependent[32] || dependency[32] || u32be(impact_kind)
       | u32be(3) || StateRoot[32]

class_body =
  (none)                                  // 1, 5, 10, 11
  entity[32]                              // 2, 3, 6, 7, 8, 9, 16, 17, 18, 19
  u32be(kind)                             // 4
  entity[32] || list(u32be(impact_kind))  // 12, 13
  list(EntityId[32])                      // 14, 15

RootQueryId = BLAKE3-256("sley2.root-query.v1" || root_query_preimage)
```

The identity binds the exact snapshot, root, epoch, workspace, arm, limits,
continuation flag, cursor, class, and body. It is not a result digest, a
proof of root provenance, or a session.

## 6. Response record

```text
root_query_response =
  "SLEYRQR1" || u32be(format_version=1) || u32be(profile_version=1) ||
  RootQueryId[32] || IndexSnapshotId[32] || SchemaEpochId[32] ||
  StateRoot[32] || WorkspaceId[32] || u32be(completeness=2) ||
  u32be(limits_profile=1) || query_limits ||
  u32be(allow_continuation) || option(cursor, after) || u32be(class_tag) ||
  u64be(total_count) || u64be(returned) || u32be(truncated: 1 false | 2 true) ||
  option(cursor, next_after) || u32be(reached_depth) ||
  u64be(charged_work) || u64be(response_bytes) ||
  u32be(result_tag) || result_payload

result_payload by result_tag (= class_tag):
  1  WorkspaceId[32] || SchemaEpochId[32] || StateRoot[32] ||
     ObjectId[32] || ObjectId[32] || PolicyRootId[32] ||
     18 x u64be(kind_count) || u64be(entry_points) ||
     u64be(dependency_roots) || u64be(direct_edges)
  2  u32be(kind) || ObjectId[32] || option(Fingerprint[32])
  3  option(Fingerprint[32])
  4,5,6,9,14,15,16,17,18,19  list(EntityId[32])
  7  list(binding[32] || StateRoot[32] || external_package[32] || local_namespace[32])
  8  list(EntityId[32] || u32be(kind))
  10 list(entry_point[32] || function[32] || u32be(exposure))
  11 list(StateRoot[32])
  12,13 list(dependent[32] || dependency[32] || u32be(impact_kind))
```

`returned` counts payload items (one for tags 1 through 3);
`total_count` is the exact complete count (one for tags 1 through 3);
`reached_depth` is nonzero only for the closure classes. `response_bytes`
is the complete record length computed with checked arithmetic before
encoding. The record has no digest trailer and no public decoder; the full
S20-320 capsule wraps it.

## 7. Repository surface

`sley-repo` owns the only producer of a `RootQueryInput` from persistent
state:

```text
run_root_query(repository, revision, query, limits, allow_continuation, after)
  = input  <- extract(revision)                   // S20-250 full adapter
    snap   <- complete_root_snapshot(repository, revision)   // S20-300 cache
    bind(snap, input) then execute
```

The revision is an S20-390 verified revision. The snapshot comes from the
S20-300 cache (`Hit` or `Rebuilt`); a hit supplies edges only, while every
body, binding, fingerprint, and root fact comes from the verified objects
and record, and the section 1 binding is re-checked so a cached inventory
can never disagree with the record. The surface is read-only derived query
evidence: it grants no root, commit, policy, mutation, session, or
protocol authority, and nothing it returns is an input to validation,
comparison, merge, commit, exchange, GC, or recovery.

## 8. Stable failures and precedence

Codes 31000 through 31007 are unchanged. This profile appends:

| Numeric | Symbolic code |
|---:|---|
| 31008 | `QUERY_ROOT_MISMATCH` |
| 31009 | `QUERY_CONTINUATION_INVALID` |
| 31010 | `QUERY_CLASS_NOT_APPLICABLE` |

Precedence:

1. invalid limit profile or ceiling (`QUERY_RESOURCE_LIMIT`);
2. unsupported format, profile, or arm (`QUERY_PROFILE_UNSUPPORTED`), then
   unsupported class, kind, or filter tag (`QUERY_UNSUPPORTED`);
3. noncanonical filters, seeds, or request-identity drift
   (`QUERY_REQUEST_NOT_CANONICAL`);
4. cursor of the wrong key type (`QUERY_CONTINUATION_INVALID`);
5. input binding failure (`QUERY_ROOT_MISMATCH`);
6. request and snapshot binding mismatch (`QUERY_SNAPSHOT_MISMATCH`);
7. absent entity or seed (`QUERY_UNRESOLVED_ENTITY`);
8. class not applicable to the resolved kind (`QUERY_CLASS_NOT_APPLICABLE`);
9. traversal or work ceiling (`QUERY_RESOURCE_LIMIT`);
10. exact result cannot fit the applied limits without continuation
    (`QUERY_REQUIRED_FACT_OMITTED`);
11. impossible trusted-construction invariant (`QUERY_INTERNAL_INVARIANT`).

`IMPACT_*`, `INDEX_SNAPSHOT_*`, `STORE_*`, `TXN_*`, and `SCB_*` failures
from extraction, the cache, and revision loading are preserved by the
repository surface and never remapped.

## 9. Required evidence

Implementation acceptance requires at least:

- fixed request-identity and response-record vectors for all nineteen
  classes over the frozen S20-250 fixture with a fixed root, epoch,
  workspace, bindings, and fingerprints, and an independent Python
  reproduction of every identity and record from the fixture bodies and
  frozen edges;
- a continuation walk for every paged class whose pages union to the
  complete result with exact `total_count` on every page, plus the
  truncated-without-continuation, invalid-cursor, and depth-cut failures;
- 128 equal queries producing byte-identical identities and records;
- the class-kind applicability matrix, the binding failure matrix
  (`QUERY_ROOT_MISMATCH`), and the restricted arm-1 snapshot rejected with
  `QUERY_PROFILE_UNSUPPORTED`;
- a repository test where a cache hit and a rebuild answer the same query
  with byte-identical records, and a forged cache inventory cannot reach
  the engine;
- an S20-700 persistent libFuzzer target over the typed class constructor,
  continuation, and binding rules;
- Tier 1 plus semantics-focused Tier 2 validation;
- Ariadne contract review, Nabu architecture review, and Vulcan surface
  review with every report-grade finding closed.

## 10. Explicit exclusions

This contract does not claim:

- the full S20-320 context capsule, S20-330 session handles, or the SMP1
  transport of S20-400, which consume this profile;
- cross-root, cross-repository, label, path, text, or ranked queries;
- fingerprint recomputation (only the stored field-4 claim is returned);
- any authority beyond read-only derived query evidence;
- runtime, benchmark, packaging, release, or GA.

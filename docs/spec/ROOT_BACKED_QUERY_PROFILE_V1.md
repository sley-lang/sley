# Root-Backed Query Profile v1

Status: S20-310 full contract draft, revision 7 (2026-09-15); implemented
under this draft with Council review pending (Ariadne contract review, Nabu
architecture review, Vulcan surface review), so the contract is not frozen
and the package is not complete. Revision 5 repairs the review-round P1/P2
text items without touching the nineteen-class contract: the arm rule is
split (arm-1 snapshots fail `QUERY_PROFILE_UNSUPPORTED`, section 8 item 2),
the precedence list states the engine's cursor-before-drift order for the
drift subcase (section 8 items 3-4), paging keys are named with the
page-union consumer rule (section 3), and the exclusion list names the
non-enumerating lookup classes (section 10). Revision 6 closes the round-7
wording packet (operator authorization 2026-09-15, consistent with the
reviewed semantics): section 3 states the page-set acceptance rule a
consumer applies, section 7 names which answer facts a cache hit supplies
and which the verified record supplies, section 9 states the one-walk-per-
cursor-key-type evidence rule with the unit walks that pin the truncated
arms of the single-item classes, and section 10 states that the
nineteen-class enumeration is chosen, not derived. Revision 7 repairs the
a809906 council round's items: the section 7 audit attribution (the
hit-path decode audit is separated from the off-path
`verify_cached_snapshot` byte-rebuild audit), the section 7 class names
and class-1 direct-edge count, the section 3 duplicate sentence, and the
section 9 evidence rule (the unit walks now cover `Roots`, `EntryRows`,
`DependencyRows`, and `InventoryEntries`). Revision 4 composes the entity-read
surface (section 11): the S20-310 methods 306/307 stay governed by
`docs/spec/ENTITY_READ_PROFILE_V2.md`, whose owner, adapter, corpus, and
vector line are listed as profile surface without changing the
nineteen-class contract. Revision 3 binds input binding to the
committed root: `verify()` recomputes the `StateRoot` digest from the nine
`STATE_ROOT_V1` fields (adding `interpretation_flags` to the input), so no
caller-declared answer-bearing fact survives a mismatch. Revision 2 adds
the section 2 class-kind applicability table and the exact section 4
per-class work schedule the Council round required; no query semantics,
record layout, or charge value changed. Implementation state is tracked in
the machine summary.

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
  interpretation_flags: [u32]       // the record's flags, raw order
}
```

The input is accepted only when all of the following hold. A snapshot
whose completeness is not `CompleteRoot(2)` is not a binding failure: the
arm selects the profile before binding runs, so an arm-1 snapshot fails
`QUERY_PROFILE_UNSUPPORTED` (precedence item 2), never
`QUERY_ROOT_MISMATCH`. Every other rule below fails `QUERY_ROOT_MISMATCH`
(precedence item 5) before any class runs:

1. `snapshot.completeness = CompleteRoot(2)` and
   `snapshot.context = (schema_epoch, Some(root))`;
2. the snapshot inventory identities equal `bindings` identities and
   `facts.bound_entities`, in order, and every inventory kind equals the
   entity's SSMC1 kind;
3. `entities` are in raw `EntityId` order with no duplicates and their
   identities equal the inventory;
4. `fingerprints` identities are a subset of the inventory in raw order, and
   every carried fingerprint belongs to a `TypeDef` or `Function`;
5. the nine `STATE_ROOT_V1` fields the input carries (`workspace_id`,
   `schema_epoch`, `bindings`, `facts.entry_points`,
   `facts.dependency_roots`, `contract_root`, `test_root`, `policy_root`,
   `interpretation_flags`) recompute to `root` exactly as given, with no
   reordering.

Rule 5 is the whole binding, not a supplement: the shape rules above admit
any caller that copies an inventory correctly, while the seven
answer-bearing facts they carry (the bound `ObjectId` values class 2
answers, the entry points class 10 answers, the dependency roots class 11
answers, the three roots class 1 answers, and the flags) are committed by
the root digest and absent from the `RootQueryId` preimage. Only the
canonical commitment reproduces the digest, so a substituted, reordered, or
extended fact fails binding rather than entering an answer. The digest
recompute needs no registry and grants no authorization; the request
preimage of section 5 is unchanged.

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
strictly increasing seed lists in raw `EntityId` order, and every named
identity present in the inventory. A `kind` body must resolve to an SSMC1
tag 1 through 18, else `QUERY_UNSUPPORTED`.

### Class-kind applicability

The table below is normative and exhaustive: it decides, for every class,
which subject kinds the class accepts. A class whose body names an entity
of a kind the table does not admit for that class fails
`QUERY_CLASS_NOT_APPLICABLE`. The engine checks the table for every named
entity and seed after input binding and before running the class, so the
applicability failure (precedence item 8) can only follow items 1
through 7.

| Class (tag) | Admitted subject kinds | Note |
|---|---|---|
| GetRootSummary (1) | no subject | n/a |
| GetEntity (2) | every kind | kind, binding, and fingerprint are reported for any inventory entity |
| GetSemanticFingerprint (3) | every kind | an absent fingerprint is the exact fact `None` |
| ListEntitiesByKind (4) | no subject; the body is a kind tag 1 through 18, not an entity | n/a |
| ListWorkspacePackages (5) | no subject | n/a |
| ListPackageExports (6) | Package only | else `QUERY_CLASS_NOT_APPLICABLE` |
| ListPackageDependencies (7) | Package only | else `QUERY_CLASS_NOT_APPLICABLE` |
| ListNamespaceMembers (8) | Namespace only | else `QUERY_CLASS_NOT_APPLICABLE` |
| ListOwningNamespaces (9) | every kind | ownerless kinds 1, 2, 6, 7, 8, and 18 yield the exact empty chain |
| ListEntryPoints (10) | no subject | n/a |
| ListDependencyRoots (11) | no subject | n/a |
| ListDirectDependencies (12) | every kind | edges may touch any entity |
| ListDirectDependents (13) | every kind | edges may touch any entity |
| ReverseImpactClosure (14) | every seed kind | seeds keep the restricted shape rule |
| ForwardDependencyClosure (15) | every seed kind | seeds keep the restricted shape rule |
| ListContractsFor (16) | every kind | any entity may be a contract target; the class reports referencing Contracts and never re-judges target validity |
| ListTestsFor (17) | every kind | any entity may be a test target; the class reports referencing TestCases and never re-judges target validity |
| ListDeclaredEffects (18) | Function, AdapterImport, and CapabilityRequirement only | else `QUERY_CLASS_NOT_APPLICABLE` |
| ListCapabilityRequirementsFor (19) | every kind | any entity may be a policy subject; the workspace additionally contributes its workspace capability requirements when it is the subject |

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
otherwise); it need not name an item of the result. The entity key is the
named identity the class answers about: for `ListNamespaceMembers` the
member identity, for `ListEntryPoints` the entry-point identity, for edge
classes the canonical `(dependent, dependency, kind)` triple, for
`ListDependencyRoots` the `StateRoot`. Keys are unique identities, so pages
compose only when successive requests are identical except for `after`
(same snapshot, root, epoch, workspace, limits, class, and body): then,
because `total_count` is exact on every page and the key order is
canonical, keys strictly increase across the walk, the union of the pages
is the complete result, and no page can hide a fact.

A consumer accepts a response as the complete result under exactly one of
two rules. A standalone response is complete iff `after = None` and
`truncated = false`. A page set is complete iff every page was produced by
requests identical except for `after`; the first page has `after = None`;
each later page's `after` equals the previous page's `next_after`; every
page but the last has `truncated = true`; the last page has `truncated =
false` and `next_after = None`; every page carries the same `total_count`;
and the sum of `returned` over the pages equals that `total_count`. A
response set meeting neither rule is not a complete result, whatever its
pages contain; a consumer that needs completeness and lacks continuation
authority must use `allow_continuation = false` and treat
`QUERY_REQUIRED_FACT_OMITTED` as the answer. Closure depth
keeps the restricted rule: a `max_depth` that cuts a closure short is
`QUERY_REQUIRED_FACT_OMITTED`, never a page. Single-key classes never page:
their whole result must fit the applied limits or the query fails
`QUERY_REQUIRED_FACT_OMITTED`.

## 4. Limits

`QueryLimits` and its ceilings are the restricted profile's, unchanged. The
request preimage ceiling is 4,194,304 bytes, the response record ceiling
67,108,864 bytes, and charged work 100,000,000. Charged work is traversal
work plus the exact response record bytes, each addition with checked
arithmetic against the applied `max_work` and the profile ceiling; either
ceiling breached is `QUERY_RESOURCE_LIMIT`. Traversal work is exactly the
per-class schedule below, one unit per item scanned, examined, followed,
or counted as stated, plus the two fixed single-key constants. The
independent oracle reproduces this table item for item, so an
implementation charge outside it fails the vector check rather than
entering a record.

| Class (tag) | Traversal work |
|---|---|
| GetRootSummary (1) | one unit per inventory entry; the three summary counts are derived without further charge |
| GetEntity (2) | flat 3: one inventory lookup, one binding read, one fingerprint lookup |
| GetSemanticFingerprint (3) | flat 2 |
| ListEntitiesByKind (4) | one unit per inventory entry |
| ListWorkspacePackages (5) | one unit per entity body scanned |
| ListPackageExports (6) | 1 plus one unit per export |
| ListPackageDependencies (7) | 1 plus one unit per dependency |
| ListNamespaceMembers (8) | 1 plus one unit per member |
| ListOwningNamespaces (9) | one unit per scanned candidate body, skipped when the subject is a namespace, plus one unit per chain link followed |
| ListEntryPoints (10) | one unit per entry point |
| ListDependencyRoots (11) | one unit per dependency root |
| ListDirectDependencies (12) | one unit per snapshot edge examined |
| ListDirectDependents (13) | one unit per snapshot edge examined |
| ReverseImpactClosure (14) | one unit per dequeued entity plus one unit per examined reverse edge |
| ForwardDependencyClosure (15) | one unit per dequeued entity plus one unit per examined direct edge |
| ListContractsFor (16) | one unit per entity body scanned |
| ListTestsFor (17) | one unit per entity body scanned |
| ListDeclaredEffects (18) | 1 plus one unit per effect |
| ListCapabilityRequirementsFor (19) | one unit per entity body scanned |

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
S20-300 cache (`Hit` or `Rebuilt`). A hit supplies the derived edge facts,
`direct_edges` and their inverse, and those facts answer exactly the edge
classes 12 through 15 (`ListDirectDependencies`, `ListDirectDependents`,
`ReverseImpactClosure`, `ForwardDependencyClosure`) and the direct-edge
count that `GetRootSummary` (class 1) reports; no other answer fact comes
from a hit. Every other answer fact is record-derived: bodies, bindings,
fingerprints, kinds, namespace membership, entry points, dependency
roots, and the section 1 binding come from the verified objects and the
verified `StateRootRecord`, never from the cache (the inventory a hit
carries is read for identities and kinds, but the section 1 binding,
re-checked on every request, forces each of them equal to the verified
objects', so those answers are record-determined). Two audits apply to a
cached record, and they are distinct. The hit-path admission audit is the
decode-time audit inside `decode_complete_root_snapshot`: the hit path
(`complete_root_snapshot` -> `accept_cached` -> decode, then alignment of
the cached inventory identities with the record's `entity_bindings`, in
`crates/sley-repo/src/index_cache.rs`) checks the format, context, and
arm, authenticates the record's self-digest trailer, rebuilds the inverse
edge groups from the direct edges, and refuses a snapshot whose groups
disagree. That audit proves the internal consistency of the cached edge
set, not its agreement with the objects, and this profile claims no more:
under the S20-300 hit-authority rule a self-consistent cached edge set is
authoritative on a hit, accepted only under the repository authority that
wrote it and only for this read-only derived query surface. The
byte-rebuild audit `verify_cached_snapshot` (S20-300) rebuilds the
snapshot from the revision's objects and compares the cached record byte
for byte; it is a separate off-path audit for Tier 2 evidence that the
hit path never calls. Exported evidence never rests on a bare hit:
`run_root_query_fresh` (`crates/sley-repo/src/root_query.rs`) answers
from a fresh rebuild and is the exported-evidence path. The surface is
read-only derived query evidence: it grants no root, commit, policy,
mutation, session, or protocol authority, and nothing it returns is an
input to validation, comparison, merge, commit, exchange, GC, or
recovery.

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
3. noncanonical filters or seeds (`QUERY_REQUEST_NOT_CANONICAL`); the
   shape check runs before the cursor check. Request-identity drift (a
   preimage or `RootQueryId` that does not recompute) is checked after
   the cursor (see item 4), because the cursor type gates before the
   identity comparison runs;
4. cursor of the wrong key type (`QUERY_CONTINUATION_INVALID`), checked
   before request-identity drift;
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
- one continuation walk per cursor key type over the frozen fixture
  (entity, edge-triple, and root cursors), each walk's pages unioning to
  the complete result with exact `total_count` on every page, plus the
  truncated-without-continuation, invalid-cursor, and depth-cut failures;
  where the frozen fixture carries a single item for a class (one
  dependency root for `ListDependencyRoots`, one entry point for
  `ListEntryPoints`, one package dependency for
  `ListPackageDependencies`), a fixture walk is degenerate at best (page
  one complete, page two empty past the end), so the truncated-emission
  arms of the paging layer are pinned by unit walks at limit 1 over a
  two-item complete result in `crates/sley-query/src/root_query.rs`
  (`single_item_classes_walk_two_items_at_limit_one`, covering the
  `Roots`, `EntryRows`, `DependencyRows`, and `InventoryEntries` arms,
  the last for `ListNamespaceMembers`, whose fixture result no vector
  walks), which is the evidence rule rather than a fixture with invented
  items;
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
- enumeration by the lookup classes: `GetEntity`, `GetSemanticFingerprint`,
  `ListOwningNamespaces`, and `ListDeclaredEffects` answer only their named
  subject and never list a class population (`ListEntitiesByKind` is the
  enumerating class);
- any authority beyond read-only derived query evidence;
- runtime, benchmark, packaging, release, or GA.

The nineteen query classes of section 2 are a chosen closed enumeration,
not a derivation from the entity or edge model: a later profile may add a
class 20 only additively, with its own key type or an existing one, and
no existing class number, cursor tag, or key type is reallocated (tag 3
stays the root cursor and class 18 stays what section 2 names). Empty
results for classes 16, 17, and 19 are lawful complete answers
(`total_count = 0`), not failures.

## 11. Entity-read composition

The S20-310 entity-read methods (`entity.version` 306,
`entity.signature` 307) are governed by
`docs/spec/ENTITY_READ_PROFILE_V2.md`, composed here rather than
re-specified: the owner (`crates/sley-query/src/entity_read.rs`), the
verified-repository adapter (`crates/sley-repo/src/entity_read.rs`), the
corpus (`conformance/entity-read/v2` with its `SHA256SUMS`), and the
`make conformance` entity-read vector line belong to this profile's
S20-310 surface. The owner reuses this profile's stable codes
(`QUERY_UNRESOLVED_ENTITY` 31004, `QUERY_INTERNAL_INVARIANT` 31007,
`QUERY_ROOT_MISMATCH` 31008, `QUERY_CLASS_NOT_APPLICABLE` 31010) with
owner-numeric identity, so no new error numbers are allocated. The
nineteen-class contract, paging rules, and exclusion list above are
unchanged; entity-read verdicts are recorded on entity-read-scoped review
fields and never supersede the root-query lane dispositions of section 9.

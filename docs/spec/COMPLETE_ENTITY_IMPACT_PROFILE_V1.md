# Complete Entity Model and Impact Profile v1

Status: S20-250 full contract draft, revision 2 (2026-09-03); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). Revision 2 names the extraction and projection crates decided by
the implementation, which landed against the draft while every Council lane
was unavailable (ADR-0026 context). Implementation state is tracked in the
machine summary and in `docs/audits/S20_250_FULL_ENTITY_BODIES_CLOSEOUT.md`.

This contract completes S20-250. It adds the six SSMC1 entity bodies that the
restricted profile left outside the semantic core, freezes their exact
relationships, extends the closed impact request to all eighteen entity
kinds, and defines the complete-root request and its closure judgment. It
composes, and never alters, three frozen contracts:

- the S20-200 SSMC1 epoch-1 schema (`docs/spec/SSMC1_EPOCH1_SCHEMA.txt`),
  whose six bodies `WorkspaceBody`, `PackageBody`, `NamespaceBody`,
  `EntryPointBody`, `PolicyBindingBody`, and `DependencyBindingBody` are
  taken field for field;
- the S20-250 restricted profile (`docs/spec/FINGERPRINT_IMPACT_PROFILE_V1.md`),
  whose fingerprint preimages, `value_hash`, twelve edge kinds, edge rows for
  kinds 4 through 15, reverse and transitive rules, limits, and codes 25000
  through 25012 stay normative and byte-identical;
- the S20-150 state root (`docs/spec/STATE_ROOT_V1.md`), whose record fields
  `entity_bindings`, `entry_points`, and `dependency_roots` are the only root
  facts a complete-root request consumes.

Fingerprints, impact indexes, and root closure judgments are derived evidence.
They never replace an `EntityId`, `ObjectId`, `StateRoot`, schema judgment,
candidate validation, or repository authority.

## 1. Scope

The restricted profile states that "a later package must add the six missing
bodies and extend this profile before S20-300, S20-510, or GA may claim a
complete-root impact index". This document is that extension. It claims:

- one normative Rust model for all eighteen SSMC1 bodies in `sley-ssmc`;
- exact direct impact edges for the six bodies, using only the twelve frozen
  edge kinds;
- a closed impact request over any subset of the eighteen kinds;
- a complete-root request derived from one strictly decoded `StateRoot`
  record and the exact objects it binds, with a fail-closed closure judgment.

It does not claim semantic comparison (S20-510), merge (S20-520), the full
S20-300 complete-root index snapshot, root-backed S20-310 queries, sessions,
protocol, policy transitions, or GA.

## 2. Fingerprints are unchanged

SSMC1 section 8 freezes the rule: "Field 4 must be absent until S20-250
provides the exact verifier; GA-valid `TypeDef` and `Function` entities
require it." The six bodies therefore carry no semantic fingerprint. A present
field-4 claim on any kind other than 4 or 5 still fails with
`FINGERPRINT_ENTITY_UNSUPPORTED`, exactly as the restricted profile's section
5 states. No preimage, domain, or profile version changes. Equivalence of two
entities of the six kinds is byte equality of their canonical SSMC1 bodies.

## 3. Normative model

`sley-ssmc` gains six definitions. Every field is the exact schema field;
every `Set<EntityId>` is a raw-ID-sorted, duplicate-free list, as the existing
`invariants` and `effects` fields already are; `Option<EntityId>` is the
schema's generic `Option`.

```text
WorkspaceDefinition(entity_id, packages: [EntityId], root_namespace: EntityId,
                    capability_requirements: [EntityId], contracts: [EntityId],
                    tests: [EntityId])
PackageDefinition(entity_id, workspace: EntityId, root_namespace: EntityId,
                  dependencies: [EntityId], exports: [EntityId])
NamespaceDefinition(entity_id, parent: Option<EntityId>, members: [EntityId])
EntryPointDefinition(entity_id, function: EntityId, exposure: EntryExposure)
PolicyBindingDefinition(entity_id, subject: EntityId, requirements: [EntityId])
DependencyBindingDefinition(entity_id, dependency_root: StateRoot,
                            external_package: EntityId, local_namespace: EntityId)
```

`EntryExposure` (`1 Local`, `2 Protocol`) moves into `sley-ssmc` beside
`Visibility`; `sley-mutate` re-exports it so the generated proposal codec is
byte-identical. The generated `sley-mutate` bodies remain the proposal-side
codec, and the S20-360 validator projection maps them onto these definitions
exactly as it maps the twelve existing kinds. No second host model exists, and
the S20-360 private reference graph is not changed by this contract.

`DependencyBindingDefinition.dependency_root` is an identity. No API in this
profile loads, resolves, dereferences, or compares the content of an external
root. `external_package` names an entity of the external root and is never
resolved locally.

## 4. Closed request over eighteen kinds

The closed impact request keeps the restricted profile's rules: raw-ID-sorted
unique entities, at most `65,535`, every referenced identity resolving inside
the request. `ModeledEntityKind` now covers SSMC1 tags 1 through 18;
`IMPACT_ENTITY_UNSUPPORTED` remains the failure for any tag outside that
closed set. A request may contain any subset of kinds; completeness is a
property of the complete-root request in section 6, never of an arbitrary
request.

## 5. Direct impact edges for the six bodies

The edge kinds are the twelve frozen tags of the restricted profile. Each row
below assigns the same kind that the frozen S20-360 candidate reference graph
assigns to the same field, so the validator and the impact index agree on
every relationship over the same closed request. `Any modeled` means one of
kinds 1 through 18 and performs existence but not a narrower kind check.

| Body and field | Edge kind | Required dependency kind |
|---|---|---|
| Workspace `packages[]` | Ownership | Package |
| Workspace `root_namespace` | Ownership | Namespace |
| Workspace `capability_requirements[]` | Capability | CapabilityRequirement |
| Workspace `contracts[]` | Contract | Contract |
| Workspace `tests[]` | TestTarget | TestCase |
| Package `workspace` | Ownership | Workspace |
| Package `root_namespace` | Ownership | Namespace |
| Package `dependencies[]` | Ownership | DependencyBinding |
| Package `exports[]` | Ownership | Any modeled |
| Namespace `parent` (when present) | Ownership | Namespace |
| Namespace `members[]` | Ownership | Any modeled |
| EntryPoint `function` | Ownership | Function |
| PolicyBinding `subject` | Ownership | Any modeled |
| PolicyBinding `requirements[]` | Capability | CapabilityRequirement |
| DependencyBinding `local_namespace` | Ownership | Namespace |
| DependencyBinding `external_package` | no edge | never resolved locally |
| DependencyBinding `dependency_root` | no edge | identity only |

`EntryExposure` contains no `EntityId` and creates no edge. Edges are
canonical, deduplicated, and sorted exactly as the restricted profile's
section 7 requires. `Contract.target` and `PolicyBinding.subject` may name an
entity of any of the eighteen kinds.

## 6. Complete-root request

A complete-root request is derived from one strictly decoded `StateRoot`
record `R` and the exact objects it binds:

1. If `R.entity_bindings` has more than `65,535` entries, the request fails
   with `IMPACT_RESOURCE_LIMIT` before any object is read.
2. Every `(EntityId, ObjectId)` binding is read from the object store and
   decoded as one strict SSMC1 `EntityObject`. Store, SCB1, and SSMC failures
   preserve their exact `STORE_*`, `SCB_*`, and `SSMC_*` codes.
3. The decoded object's `entity_id` must equal the binding key; otherwise
   `IMPACT_ROOT_BINDING_MISMATCH`.
4. Each decoded body is projected onto its normative definition. The request
   is the raw-ID-sorted inventory of those definitions together with two root
   facts copied from `R`: `entry_points` and `dependency_roots`.

The pure closure judgment consumes borrowed definitions and the two root
facts and performs no I/O. The extraction adapter is `sley-repo`
(`CompleteRootRequest::extract` over a verified revision, whose objects the
transaction owner has already loaded and inventory-checked); it projects
through the validator's public `sley-policy` projection
(`complete_entities::project_complete_entities`), which is the single
mapping from proposal bodies to definitions; the judgment and the index live
in `sley-query`. The dependency
direction `sley-query -> sley-check -> sley-ssmc` is unchanged, and
`sley-query` gains no dependency on `sley-store`, `sley-mutate`, or
`sley-policy`.

### 6.1 Closure rules

Let `W` be the Workspace entities, `P` the Package entities, `N` the Namespace
entities, `E` the EntryPoint entities, and `D` the DependencyBinding entities
of the request. Rules are checked in order; the first failure is returned and
no partial index is produced.

| Rule | Requirement | Failure |
|---|---|---|
| C1 | The request's entity set equals the keys of `R.entity_bindings`. | `IMPACT_ROOT_INVENTORY_MISMATCH` |
| C2 | Every edge of section 5 and of the restricted profile resolves with the required kind. | restricted-profile codes |
| C3 | `W` contains exactly one entity `w`. | `IMPACT_ROOT_WORKSPACE_MISSING` when empty, `IMPACT_ROOT_WORKSPACE_AMBIGUOUS` otherwise |
| C4 | `w.packages` equals the raw-ID-sorted identities of `P`, and every `p.workspace` equals `w`. | `IMPACT_ROOT_PACKAGE_MEMBERSHIP` |
| C5 | `w.root_namespace` and every `p.root_namespace` have `parent = None`, are pairwise distinct, and are exactly the parentless namespaces of `N`. | `IMPACT_ROOT_NAMESPACE_ROOT` |
| C6 | For every namespace `n` with `parent = Some(q)`, `n` is a member of `q`; for every namespace member `m` of `n` that is a Namespace, `m.parent = Some(n)`; following `parent` from any namespace reaches a parentless namespace within `|N|` steps. | `IMPACT_ROOT_NAMESPACE_TREE` |
| C7 | Every namespace member is listed by exactly one namespace, and its kind is not Workspace, Package, Parameter, Block, Operation, or DependencyBinding. | `IMPACT_ROOT_MEMBER_OWNERSHIP` |
| C8 | Every `p.exports` element is a member of a namespace in `tree(p)`, the namespaces reachable from `p.root_namespace` through namespace members. | `IMPACT_ROOT_EXPORT_UNSCOPED` |
| C9 | `R.entry_points` equals the raw-ID-sorted identities of `E`. | `IMPACT_ROOT_ENTRY_POINTS_MISMATCH` |
| C10 | `R.dependency_roots` equals the sorted distinct `d.dependency_root` values of `D`. | `IMPACT_ROOT_DEPENDENCY_ROOTS_MISMATCH` |
| C11 | Every `d` in `D` is listed by exactly one `p.dependencies`, and `d.local_namespace` is in `tree(p)`. | `IMPACT_ROOT_DEPENDENCY_BINDING_UNOWNED` |

C9 matches the frozen S20-360 judgment that every root entry point is an
EntryPoint entity (`STATE_ROOT_ENTRY_POINT_KIND_INVALID`), and C10 matches
the validator's `dependency_roots` derivation. A root with zero packages is
valid when `w.packages` is empty and no Package, DependencyBinding, or
package-rooted namespace exists. Parameter, Block, and Operation entities are
Function-owned and are never namespace members; a request need not place
every eligible entity in a namespace, but every listed membership must be
exact.

### 6.2 Result

A successful complete-root judgment yields the exact `ImpactIndex` over the
eighteen-kind request, whose direct edges, reverse edges, and transitive
closure follow the restricted profile's sections 7 and 8 unchanged, plus a
completeness fact stating that the index covers every entity bound by `R`.
Nothing in this profile caches, persists, or signs that fact; S20-300 full
may later carry it.

## 7. Restricted consumers stay restricted

The S20-300 restricted snapshot, the S20-310 restricted queries, and the
S20-320 capsule bind "modeled SSMC1 kinds 4 through 15" and their fixed
vectors. They are not extended by this contract. `build_index_snapshot` and
`admit_index_snapshot` fail closed with
`INDEX_SNAPSHOT_COMPLETENESS_UNSUPPORTED` when a request contains an entity
of kind 1 through 3 or 16 through 18, before any record is derived. Their
contracts, checkers, fixed identities, and error tables are unchanged.

## 8. Limits

All restricted-profile limits apply unchanged. In addition, the complete-root
request charges one work unit per binding, per decoded object, per projected
definition, per closure-rule visit of an entity, member, export, or
dependency, and per namespace parent step, against the same `100,000,000`
ceiling; exhaustion is `IMPACT_RESOURCE_LIMIT` with no partial judgment.

## 9. Stable failures

Codes 25000 through 25012 are unchanged. This profile appends:

| Numeric | Symbolic code |
|---:|---|
| 25013 | `IMPACT_ROOT_BINDING_MISMATCH` |
| 25014 | `IMPACT_ROOT_INVENTORY_MISMATCH` |
| 25015 | `IMPACT_ROOT_WORKSPACE_MISSING` |
| 25016 | `IMPACT_ROOT_WORKSPACE_AMBIGUOUS` |
| 25017 | `IMPACT_ROOT_PACKAGE_MEMBERSHIP` |
| 25018 | `IMPACT_ROOT_NAMESPACE_ROOT` |
| 25019 | `IMPACT_ROOT_NAMESPACE_TREE` |
| 25020 | `IMPACT_ROOT_MEMBER_OWNERSHIP` |
| 25021 | `IMPACT_ROOT_EXPORT_UNSCOPED` |
| 25022 | `IMPACT_ROOT_ENTRY_POINTS_MISMATCH` |
| 25023 | `IMPACT_ROOT_DEPENDENCY_ROOTS_MISMATCH` |
| 25024 | `IMPACT_ROOT_DEPENDENCY_BINDING_UNOWNED` |

Exact earlier `STORE_*`, `SCB_*`, `SSMC_*`, `FINGERPRINT_*`, and `IMPACT_*`
failures are preserved and never converted into success.

## 10. Required evidence

Implementation acceptance requires at least:

- the six definitions in `sley-ssmc` with exact tags, and the `EntryExposure`
  move with a byte-identical generated proposal codec;
- the S20-360 projection mapping all eighteen bodies with no change to its
  private graph, proven by its existing tests;
- an edge-agreement test that builds the S20-360 candidate reference graph
  and the `ImpactIndex` over the same eighteen-kind request and proves the
  edge sets equal pair for pair and kind for kind;
- exact direct, reverse, and transitive tests over a request containing all
  eighteen kinds, with at least 128 shuffled-input seeds byte-identical;
- a rejection matrix reaching every code in section 9 and every restricted
  code reachable from a six-body field, each as the first failure;
- a complete-root extraction test over a real object store and accepted
  root, including a binding whose object names another entity, a store miss,
  and a corrupt object, each preserving its source code;
- a restricted-consumer test proving `build_index_snapshot` and
  `admit_index_snapshot` fail closed on each of the six kinds;
- an independent Python reproduction of the direct edge set and the closure
  judgment over a frozen eighteen-kind fixture under
  `conformance/complete-entity-impact/v1/`;
- an S20-700 persistent libFuzzer target over the closure judgment input;
- Tier 1 plus semantics-focused Tier 2 validation;
- Ariadne contract review, Nabu architecture review, and Vulcan surface
  review with every report-grade finding closed.

## 11. Explicit exclusions

This contract does not claim:

- fingerprints, `value_hash`, or a profile version change for any kind;
- semantic comparison, typed deltas, merge, or conflict objects;
- the full S20-300 snapshot, root-backed S20-310 queries, or any cache of the
  completeness fact;
- loading, verifying, or comparing external dependency roots;
- namespace labels, paths, name resolution, or visibility judgment;
- a rule that every eligible entity belongs to a namespace;
- roots above `65,535` bindings;
- SMP1, JSON bridge, CLI, runtime, benchmark, packaging, release, or GA.

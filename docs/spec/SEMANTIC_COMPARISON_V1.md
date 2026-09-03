# Semantic Comparison v1

Status: S20-510 contract draft, revision 1 (2026-09-03); Council review
pending (Ariadne contract review, Nabu architecture review, Vulcan surface
review). No implementation exists at this revision. Implementation state is
tracked in the machine summary.

## Notation

This document uses the SCB1 notation: `||` is byte concatenation, `uvar(x)`
is the unsigned varint of `x`, `len(x)` is `uvar(byte_length(x))`, and
`BLAKE3-256` is the 32-byte BLAKE3 digest. All byte comparisons are unsigned
lexicographic comparisons. `zero32` is thirty-two zero bytes.

## Scope

Semantic Comparison v1 is the exact, deterministic, typed comparison of two
complete semantic roots of one workspace under one schema epoch. It composes,
and never alters, the frozen S20-250 profiles: the restricted profile's
`Function` and `TypeDef` fingerprints, the twelve edge kinds, the
eighteen-kind closed request, and the complete-root judgment of
`docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md`. The Repository Model
requires that "root comparison emits typed entity, type, signature, CFG,
call, effect, capability, contract, test, entry-point, and dependency
deltas"; this contract realizes every one of those classes through five
sections of one canonical delta record and defines exactly how each class is
derived.

A semantic delta is derived evidence. It never replaces a `StateRoot`, an
`ObjectId`, a receipt, a candidate, or repository authority, and it contains
no labels, source text, paths, timestamps, ref names, ancestry, or Git facts.
Merge (S20-520) consumes deltas; this contract defines no merge, conflict,
or composition rule.

`sley-repo` owns comparison. The dependency direction
`sley-repo -> sley-query -> sley-check -> sley-ssmc` is unchanged, and
`sley-query` gains nothing.

## Inputs

A comparison takes two complete-root requests, `base` and `target`, each
consisting of the projected eighteen-kind definitions, the record facts
`entity_bindings`, `entry_points`, and `dependency_roots`, the record's
`workspace_id` and `schema_epoch_id`, and the root identity `StateRoot`.
In v1 both come from verified revisions through the S20-250 full extraction
adapter. Before any delta is derived:

1. `base` and `target` MUST pass the complete-root judgment; a failure is
   `COMPARE_ROOT_INCOMPLETE` with the exact `IMPACT_*` code preserved;
2. their `workspace_id` MUST be equal, else `COMPARE_WORKSPACE_MISMATCH`;
3. their `schema_epoch_id` MUST be equal, else `COMPARE_EPOCH_MISMATCH`;
4. every `Function` of either root MUST have a complete owned inventory
   (its Parameter, Block, and Operation entities are in the same root), else
   `COMPARE_INVENTORY_INVALID` with the exact `FINGERPRINT_*` code preserved.

Two identical roots compare to an empty delta, which is valid.

## Change classes

For every `EntityId` bound by `base` or `target` (their union, raw-ID
order), exactly one class applies:

| Tag | Class | Condition |
|---:|---|---|
| 1 | `Added` | bound only by `target` |
| 2 | `Removed` | bound only by `base` |
| 3 | `Changed` | bound by both, same kind, canonical definitions differ |
| 4 | `Retyped` | bound by both, different kind |
| 5 | `MetadataOnly` | bound by both, same kind, `ObjectId` differs, canonical definitions equal (label or reserved fields differ) |

An entity bound by both with the same `ObjectId` is unchanged and produces
no entity delta. The canonical definition of an entity is its projected
normative body; equality is structural equality of that body, exactly as the
S20-250 full profile defines equivalence of the six kinds and as the frozen
bodies of the twelve kinds define theirs.

## Sections

The delta record carries five sections. Every section is a canonical set in
the order defined below; a comparison never emits a duplicate.

### 1. Entities

One entity delta per classified identity:

```text
entity_delta = (entity_id, change, base_kind, target_kind, base_object, target_object)
```

`base_kind`/`base_object` are `0`/`zero32` for `Added`; `target_kind`/
`target_object` are `0`/`zero32` for `Removed`. Kinds are SSMC1 tags.

### 2. Fields

One field delta per changed schema field of every `Changed` entity, keyed by
`(entity_id, kind, field)` where `field` is the exact SSMC1 body field tag:

```text
field_delta = (entity_id, kind, field, flags, added, removed)
```

`flags` bit 0 (`1`) is always set. `added` and `removed` hold identities that
entered or left a set-valued or identity-list field. Other bits and the
identity sets are defined per field:

| Kind and field | Bits beyond bit 0 | `added`/`removed` |
|---|---|---|
| TypeDef 1 `type_parameters` | none | none |
| TypeDef 2 `form` | bit 1 form kind changed (record/variant); bit 2 an existing member's type, payload, or visibility changed; bit 3 member order changed | `MemberId` values added/removed |
| TypeDef 3 `invariants` | none | contract identities |
| TypeDef 4 `visibility` | none | none |
| Function 1 `type_parameters` | none | none |
| Function 2 `parameters` | bit 1 the positional parameter types differ | parameter identities |
| Function 3 `result_type` | none | none |
| Function 4 `effects` | none | effect identities |
| Function 5 `entry_block` | none | none |
| Function 6 `blocks` | none | block identities |
| Function 7 `contracts` | none | contract identities |
| Function 8 `visibility` | none | none |
| Parameter 1 `owner`, 2 `role`, 3 `ordinal`, 4 `value_type` | none | none |
| Block 1 `function`, 4 `terminator`, 5 `reachability` | none | none |
| Block 2 `parameters`, 3 `operations` | none | identities |
| Operation 1 `block`, 2 `ordinal`, 3 `opcode`, 4 `operands`, 5 `result_types`, 6 `immediate` | none | none |
| Constant 1 `value` | none | none |
| GlobalValue 1 `value_type`, 3 `visibility` | none | none |
| GlobalValue 2 `initializer` | none | none |
| EffectDef 1 through 6 | none | none |
| CapabilityRequirement 1 `effect`, 2 `allowed_scopes` | none | none |
| CapabilityRequirement 3 `constraint_contracts` | none | contract identities |
| Contract 1 `target`, 2 `contract_kind`, 3 `predicate`, 4 `bindings`, 5 `resource_limits` | none | none |
| TestCase 1 through 6 | none | none |
| AdapterImport 1 through 5 | none | none |
| AdapterImport 6 `effects` | none | effect identities |
| Workspace 1 `packages`, 3 `capability_requirements`, 4 `contracts`, 5 `tests` | none | identities |
| Workspace 2 `root_namespace` | none | none |
| Package 1 `workspace`, 2 `root_namespace` | none | none |
| Package 3 `dependencies`, 4 `exports` | none | identities |
| Namespace 1 `parent` | none | none |
| Namespace 2 `members` | none | identities |
| EntryPoint 1 `function`, 2 `exposure` | none | none |
| PolicyBinding 1 `subject` | none | none |
| PolicyBinding 2 `requirements` | none | identities |
| DependencyBinding 1 `dependency_root`, 2 `external_package`, 3 `local_namespace` | none | none |

A field delta exists exactly when the two canonical field values differ. For
identity lists (`parameters`, `blocks`, `operations`) the value differs when
the ordered list differs; the sets carry the identities present in only one
side, which may both be empty for a pure reorder.

The classes named by the Repository Model derive as: type deltas are the
field deltas of kind 4; signature deltas are the field deltas of kind 5 for
fields 1, 2, 3, 4, 7, and 8; entry-point deltas are the field deltas of kind
16 plus the root entry-point sets; dependency deltas are the field deltas of
kind 18, of Package field 3, and the root dependency-root sets; effect,
capability, contract, and test deltas are the field deltas of kinds 11, 12
and 17, 13, and 14 together with the relation deltas of the same kinds.

### 3. Bodies

One body delta per `Function` bound by both roots, same kind, whose S20-250
restricted `Function` fingerprint differs between the roots:

```text
body_delta = (entity_id, base_fingerprint, target_fingerprint,
              base_blocks, target_blocks, base_operations, target_operations)
```

The fingerprints are computed by `fingerprint_function` over each root's
exact inventory and schema epoch. A function whose fingerprint differs is
`Changed` in section 1 whenever its own body differs; a function whose own
body is byte-identical but whose owned Parameter, Block, or Operation
entities changed is `MetadataOnly` or unchanged in section 1 and still
appears here. This is the CFG delta; block-level and operation-level
alignment is outside v1.

### 4. Relations

One relation delta per direct impact edge present in exactly one root's
index:

```text
relation_delta = (dependent, dependency, kind, change)
```

`kind` is the frozen S20-250 edge kind tag (1 through 12) and `change` is
`1` (`Added`, only in target) or `2` (`Removed`, only in base). The set is
the symmetric difference of the two complete-root indexes' direct edge sets,
so call deltas are the relation deltas of kind 5, effect deltas kind 6,
capability deltas kind 7, contract deltas kind 8, test deltas kind 10, and
so on; no relation is derived by any other rule.

### 5. Root sets and collateral

`dependency_roots_added`/`dependency_roots_removed` and
`entry_points_added`/`entry_points_removed` are the set differences of the
two records' facts (which C9 and C10 tie to the entity inventories).

`collateral` is the raw-ID-sorted set of entities that are bound by both
roots and unchanged, and that transitively depend on a changed entity:

```text
seeds_base   = Removed ∪ Changed ∪ Retyped ∪ MetadataOnly-with-body-delta
seeds_target = Added ∪ Changed ∪ Retyped ∪ MetadataOnly-with-body-delta
collateral   = (transitive_impact(base_index, seeds_base)
               ∪ transitive_impact(target_index, seeds_target))
               \ (seeds_base ∪ seeds_target) restricted to unchanged entities
```

`transitive_impact` is the restricted profile's bounded reverse reachability
over each root's own complete-root index. A missed collateral change is the
primary risk this package answers, so `collateral` is normative output, not
advice.

## Envelope and identity

```text
format_version    = 1
contract_tag      = 510
contract_domain   = "sley2.semantic-delta.v1"
digest_domain_tag = 20
kind_tag          = 510
```

```text
delta_preimage = "SLEYSCB1" || uvar(1) || uvar(510) ||
                 DeltaSchemaEpochId[32] || len(payload) || payload
SemanticDeltaId = BLAKE3-256("sley2.semantic-delta.v1" || delta_preimage)
stored_delta = delta_preimage || SemanticDeltaId[32]
```

`sley2.semantic-delta.v1` is a new `sley-id` domain; adding its row to
`IDENTIFIERS_V1.md`, the `Domain` enumeration, and the frozen vectors is
identifier-owner work inside the S20-510 slice.

The delta schema epoch is a standalone epoch-1 record with exactly one
contract descriptor: `contract_tag = 510`, `digest_domain_tag = 20`,
`kind_tag = 510`, `required_fields = {1, ..., 14}`, `optional_fields = {}`,
`variant_tags = {}`, and the two frozen hashes below, raw BLAKE3-256 hashes
of these exact ASCII texts:

```text
field schema preimage = sley2.semantic-delta.v1.schema:required(1:delta_version u32,2:workspace_id fixed32,3:root_schema_epoch fixed32,4:base_root fixed32,5:target_root fixed32,6:entities set entity_delta,7:fields set field_delta,8:bodies set body_delta,9:relations set relation_delta,10:dependency_roots_added set fixed32,11:dependency_roots_removed set fixed32,12:entry_points_added set fixed32,13:entry_points_removed set fixed32,14:collateral set fixed32);entity_delta=record(1:entity_id fixed32,2:change u32,3:base_kind u32,4:target_kind u32,5:base_object fixed32,6:target_object fixed32);field_delta=record(1:entity_id fixed32,2:kind u32,3:field u32,4:flags u32,5:added set fixed32,6:removed set fixed32);body_delta=record(1:entity_id fixed32,2:base_fingerprint fixed32,3:target_fingerprint fixed32,4:base_blocks u32,5:target_blocks u32,6:base_operations u32,7:target_operations u32);relation_delta=record(1:dependent fixed32,2:dependency fixed32,3:kind u32,4:change u32);epoch=1
field_schema_hash = 5e58f98ecf6d7a501fc49011aa585e85abef9396ff389b5b6bb7c796f118739c

decoder limits preimage = sley2.semantic-delta.v1.decoder-limits:stored=67108864,entities=131070,fields=1048560,bodies=65535,relations=8000000,identities=131070,allocation=134217728,work=100000000
decoder_limits_hash = d25baa2eb5fcb394fb7fcfdca09326eb4373a1cc6e79139548d1d9d0fb37f370
```

## Payload

The payload is a closed SCB1 Record with all fields required:

| Tag | Field | Type and rule |
|---:|---|---|
| 1 | `delta_version` | `UInt<32>`; exactly `1` |
| 2 | `workspace_id` | `FixedBytes<32>`; the shared workspace |
| 3 | `root_schema_epoch` | `FixedBytes<32>`; the shared schema epoch of both roots (not the delta envelope epoch) |
| 4 | `base_root` | `FixedBytes<32>` |
| 5 | `target_root` | `FixedBytes<32>`; MAY equal field 4 only for the empty delta |
| 6 | `entities` | `CanonicalSet` of entity deltas; order is raw `entity_id` order because every element begins with `entity_id fixed32` |
| 7 | `fields` | `CanonicalSet` of field deltas; raw `entity_id`, then `kind`, then `field` order |
| 8 | `bodies` | `CanonicalSet` of body deltas; raw `entity_id` order |
| 9 | `relations` | `CanonicalSet` of relation deltas; raw `dependent`, then `dependency`, then `kind` order |
| 10 | `dependency_roots_added` | `CanonicalSet<FixedBytes<32>>` |
| 11 | `dependency_roots_removed` | `CanonicalSet<FixedBytes<32>>` |
| 12 | `entry_points_added` | `CanonicalSet<FixedBytes<32>>` |
| 13 | `entry_points_removed` | `CanonicalSet<FixedBytes<32>>` |
| 14 | `collateral` | `CanonicalSet<FixedBytes<32>>` |

Element records use the tags of the preimage text. `u32` values are SCB1
`UInt<32>`. Within every record every field is required. A `Bytes`-typed
field is framed exactly once, as the frozen S20-540 realization states.
Because every set element begins with a `fixed32` identity and continues
with small `u32` tags, SCB1 canonical-set order coincides with the stated
raw orders; a decoder never sorts input and fails `COMPARE_CANONICAL_ORDER`
or `COMPARE_DUPLICATE_ENTRY` on any deviation.

## Completeness invariants

An implementation MUST satisfy, and the fixture oracle MUST check:

- I1: section 1 contains exactly the identities whose bindings differ, each
  once, with the class table's unique class;
- I2: section 2 contains exactly one entry per changed field of every
  `Changed` entity and no entry for any other entity;
- I3: section 3 contains exactly the functions whose fingerprints differ;
- I4: section 4 equals the symmetric difference of the two direct edge sets;
- I5: sections 5's root sets equal the record fact differences;
- I6: `collateral` equals the definition above;
- I7: the record re-encodes byte-identically from its decoded form, and
  comparing the same two roots twice yields the same bytes and identity.

## Resource limits

| Limit | Maximum |
|---:|---|
| stored delta bytes | `67,108,864` |
| entity deltas | `131,070` |
| field deltas | `1,048,560` |
| body deltas | `65,535` |
| relation deltas | `8,000,000` |
| identities per root set or collateral | `131,070` |
| top-down allocation budget | `134,217,728` bytes |
| charged comparison work | `100,000,000` |

Comparison charges one work unit per classified identity, per compared
field, per fingerprint inventory entity, per edge of either index, and per
transitive-impact step; exhaustion is `COMPARE_RESOURCE_LIMIT` with no
partial delta. Decoding enforces the same counts before allocation.

## Stable failures

Numeric codes `51000` through `51010` are exact:

| Numeric | Symbolic |
|---:|---|
| 51000 | `COMPARE_VERSION_UNSUPPORTED` |
| 51001 | `COMPARE_DIGEST_MISMATCH` |
| 51002 | `COMPARE_CANONICAL_ORDER` |
| 51003 | `COMPARE_DUPLICATE_ENTRY` |
| 51004 | `COMPARE_FORMAT_INVALID` |
| 51005 | `COMPARE_WORKSPACE_MISMATCH` |
| 51006 | `COMPARE_EPOCH_MISMATCH` |
| 51007 | `COMPARE_ROOT_INCOMPLETE` |
| 51008 | `COMPARE_INVENTORY_INVALID` |
| 51009 | `COMPARE_RESOURCE_LIMIT` |
| 51010 | `COMPARE_INTERNAL_INVARIANT` |

`SCB_*`, `IMPACT_*`, and `FINGERPRINT_*` failures are preserved with their
exact codes inside `COMPARE_FORMAT_INVALID`, `COMPARE_ROOT_INCOMPLETE`, and
`COMPARE_INVENTORY_INVALID` respectively; they are never remapped.
`COMPARE_INTERNAL_INVARIANT` is reserved.

## Required evidence

Implementation acceptance requires at least:

- the frozen comparison corpus under `conformance/semantic-comparison/v1/`
  with at least these root pairs: identical roots (empty delta); every
  change class; a type delta with members added, removed, changed, and
  reordered; a signature delta; a body delta with an unchanged signature; a
  body delta caused only by owned-entity changes (`MetadataOnly`); call,
  effect, capability, contract, and test relation deltas; entry-point and
  dependency-root set deltas; and a collateral set that a naive
  entity-only comparison would miss;
- an independent Python reproduction of the classification, the field
  deltas, the relation deltas, the collateral set, the canonical bytes, and
  `SemanticDeltaId` over the corpus, plus a checker that recomputes both
  frozen hashes from the preimage texts;
- exact encode, decode, and re-encode round trips and a rejection matrix
  reaching every code in the table;
- workspace-mismatch, epoch-mismatch, incomplete-root, and
  inventory-invalid tests preserving the wrapped codes;
- a determinism test comparing the same pair 128 times and both directions
  (`base`/`target` swapped yields mirrored classes);
- an S20-700 persistent libFuzzer target over delta decoding;
- Tier 1 plus repository-focused Tier 2 validation;
- Ariadne contract review, Nabu architecture review, and Vulcan surface
  review with every report-grade finding closed.

## Explicit exclusions

This contract does not claim:

- merge, three-way composition, conflict objects, or disjointness judgment
  (S20-520);
- block-level or operation-level CFG alignment, renaming detection, or
  moved-entity detection;
- comparison across workspaces or schema epochs;
- label, source, or presentation differences beyond the `MetadataOnly`
  class;
- persistence of deltas in the object store, transactions, or refs;
- SMP1, JSON bridge, CLI, runtime, benchmark, packaging, release, or GA.

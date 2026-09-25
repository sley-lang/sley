# Semantic Comparison v1

Status: S20-510 contract draft, revision 3 (2026-09-14); the three Council
review rounds landed 2026-09-04 (Ariadne contract review, Nabu architecture
review, Vulcan surface review) with four freeze-blocking findings, all closed
by revision 2; revision 3 closes the remaining report-grade findings with a
closed field grammar the decoder enforces, equal-roots and disjointness
shape rules, encoder count caps, corrected error-tier and inventory-order
statements, and the residual precision notes. The implementation
landed against revision 1 at `6ecfe89` while every Council lane was
unavailable (ADR-0026 context); state is tracked in the machine summary and
`docs/audits/S20_510_SEMANTIC_COMPARISON_CLOSEOUT.md`.

## Notation

This document uses the SCB1 notation: `||` is byte concatenation, `uvar(x)`
is the unsigned varint of `x`, `len(x)` is `uvar(byte_length(x))`, and
`BLAKE3-256` is the 32-byte BLAKE3 digest. All byte comparisons are unsigned
lexicographic comparisons. `zero32` is thirty-two zero bytes: the
absent-value sentinel for optional objects and identity-free entries,
never the name of a real identity.

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
A stored delta authenticates nothing by itself: decode checks canonical
bytes, digest, record shape, and self-consistency (including
equal-roots-admit-only-empty), but `base_root`, `target_root`, and
`workspace_id` carry no proof of the roots they name. Authority comes only
from re-derivation — merge re-derives every delta it consumes — so a forged
or stale delta that decodes is still never authority.
Merge (S20-520) consumes deltas; this contract defines no merge, conflict,
or composition rule.

`sley-repo` owns comparison. Comparison reads the frozen S20-250
fingerprint through a direct production dependency `sley-repo -> sley-ssmc`
(added at `6ecfe89` for `fingerprint_function` and the fingerprint failure
codes); the fallback chain `sley-repo -> sley-query -> sley-check ->
sley-ssmc` is unchanged, and `sley-query` gains nothing.

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
   in the same root (its `parameters` and `blocks`, and those blocks'
   `parameters` and `operations`).

Precondition 4 is enforced in two layers, both before any delta is
emitted. Linkage — every forward-list reference resolving in-root — is
enforced by the frozen complete-root judgment as precondition 1, which
runs over every function of both roots, so `Added` and `Removed`
functions are covered exactly like shared ones; a linkage gap fails
`COMPARE_ROOT_INCOMPLETE` with the exact `IMPACT_*` code preserved.
Exact closure validity — ownership, roles, ordinals, back-references,
and the entry block — is enforced by the frozen fingerprint per compared
function pair in section 3; a validity gap fails
`COMPARE_INVENTORY_INVALID` with the exact `FINGERPRINT_*` code
preserved. Comparison is all-or-nothing: any failure emits no delta.

`CompleteRootRequest::from_parts` assembles a request from caller-held
projections and verifies nothing itself: production callers pass judged
roots (merge's verified extraction and builder-verified synthesis), and
comparison re-judges both sides as precondition 1 before deriving
anything, so unverified bindings never reach identity.

The frozen derivation pins below are part of the contract text. They bind the
normative derivation body, which is exactly the text from the
`## Change classes` line through the line before `## Required evidence`:
any edit inside that span changes the digest and fails the stage checker
until the pins are deliberately re-recorded in this document, the checker,
and the machine summary.

```text
derivation_semantics_hash = 0717d420a7234b0faef04d5c23a3533578f068f155fbb570a762213f70ab9314
delta_schema_epoch = 25b186d5ec4238f3f05e8af05454f62bac649143ebddf37c01c1786180b6dee4
```

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
`Changed` and `MetadataOnly` carry equal nonzero kinds with unequal
objects; `Retyped` carries two unequal nonzero kinds; kind tags are 1
through 18; body deltas carry unequal fingerprints. The decoder enforces
every rule in this section (`COMPARE_FORMAT_INVALID`); the judgment emits
only conforming deltas.

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

The table above is closed: a field delta whose `(kind, field)` is not a
row here, whose `flags` carry bits beyond bit 0 outside TypeDef field 2
(bits 1, 2, and 3) and Function field 2 (bit 1), or with bit 0 unset, is
`COMPARE_FORMAT_INVALID`. Flags bit 0 is a presence marker: always set,
carrying no information beyond "this entry exists". On TypeDef field 2,
bit 2 is suppressed when bit 1 is set and no shared member changed across
the record/variant change, so the reachable combinations are a subset of
the independent bits. TypeDef field 2 `added`/`removed` carry `MemberId`
bytes in `EntityId` clothing: they name type members, never entities, and
consumers (notably S20-520) MUST NOT resolve them as entities. The
judgment emits only rows of this table; the decoder rejects everything
else, so forged evidence cannot smuggle arbitrary kind, field, or flag
codes through a round-trip.

Retyped entities produce no field deltas and no body delta: the kinds
differ, so no field row applies, and section 3 compares same-kind
functions only. A retyped Function's body evidence is the two sides'
kinds and objects in section 1. Section 2 is not self-sufficient for
disjointness judgment: reference-bearing scalar fields (operands,
immediates, predicates, environments) carry no identity sets, so
consumers cannot infer disjointness from absence here. An optional field
compares by its exact optional value: absent versus present is a
difference like any other.

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

The owned inventory of a function is exactly the forward closure the frozen
S20-250 fingerprint walks: the function's `parameters` list (each a
Function-role parameter owned by the function, ordinal its position), its
`blocks` list (each with `block.function` the function), and the
`parameters` and `operations` lists of those blocks (each a Block-role
parameter owned by its block with ordinal its position; each an operation
owned by its block with ordinal its position). `fingerprint_function`
verifies every back-reference and rejects any missing or extra inventory
entity, so an implementation collects exactly this closure and nothing else.

`base_blocks` and `target_blocks` are the lengths of the two block vectors
passed to `fingerprint_function`; `base_operations` and `target_operations`
are the lengths of the two operation vectors. The counts and the fingerprints
derive from the same vectors, so they cannot disagree with each other. When
the forward lists and the back-references disagree — a listed block owned
elsewhere, an unlisted block claiming the function, a dangling `entry_block`
— there is no second reading of the counts: the derivation fails
`COMPARE_INVENTORY_INVALID` with the exact `FINGERPRINT_*` code preserved,
and no divergent `SemanticDeltaId` can result.

The fingerprints are computed by `fingerprint_function` over each root's
owned inventory and schema epoch. A function whose fingerprint differs is
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
so on; no relation is derived by any other rule. The symmetric difference
merge-joins the two direct-edge slices, which MUST be canonically ordered
and duplicate-free as the frozen index provides them; slice order is the
frozen index's own invariant, relied upon here, not re-checked.

### 5. Root sets and collateral

`dependency_roots_added`/`dependency_roots_removed` and
`entry_points_added`/`entry_points_removed` are the set differences of the
two records' facts (which C9 and C10 tie to the entity inventories).

`collateral` is the raw-ID-sorted set of entities bound by both roots that
carry no entity delta, and that transitively depend on a changed entity.
"Unchanged" in this section always means exactly that: bound by both roots
with the same `ObjectId`, hence carrying no entity delta in section 1. A
`MetadataOnly` entity carries a delta and is never collateral.

```text
body_seeds   = every entity carrying a body delta (section 3)
seeds_base   = Removed ∪ Changed ∪ Retyped ∪ body_seeds
seeds_target = Added ∪ Changed ∪ Retyped ∪ body_seeds
collateral   = { e ∈ transitive_impact(base_index, seeds_base)
                   ∪ transitive_impact(target_index, seeds_target)
               : e bound by both roots and e carries no entity delta }
```

`body_seeds` joins both seed sets because a body delta exists only for
functions bound by both roots — including a function unchanged in section 1
whose owned entities changed, which section 3 explicitly allows. A body-delta
seed with no entity delta stays in `collateral` when reached: the exclusion
above removes entities carrying an entity delta, not seeds as such. A
`MetadataOnly` function without a body delta seeds nothing: its normative
body is equal, hence every impact edge and every fingerprint is equal, and no
dependent's semantics can shift.

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

The delta schema epoch identity is frozen as `delta_schema_epoch` in
`## Inputs` above. It is the `schema_epoch_id()` of exactly the record stated
here: epoch 1, SCB format version 1, hash algorithm tag 1, the epoch-1
Unicode version and limits, one contract descriptor, no extensions, no
predecessor, no migration contracts. If any shared epoch-1 constant moves,
the recomputed identity will not match the pin and every gate fails. The
epoch bytes carried in stored deltas (`stored[11:43]`) MUST equal the pin,
and the fixture oracle compares against the pin rather than trusting the
artifact under test.

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
| 5 | `target_root` | `FixedBytes<32>`; equal to field 4 only for the empty delta, enforced by the decoder (`COMPARE_FORMAT_INVALID`) |
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
or `COMPARE_DUPLICATE_ENTRY` on any deviation. Non-canonical encoding
reaches the decoder only through the same backstop: any bytes that do not
re-encode identically fail `COMPARE_CANONICAL_ORDER`, which therefore
covers both order deviations and non-canonical encodings. Added and
removed identity sets are disjoint by construction on both the field
level and the root-set level; a shared member is `COMPARE_FORMAT_INVALID`.

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

Comparison charges one work unit per classified identity, eight units per
`Changed` entity for field comparison (the per-kind maximum field count, a
conservative bound — no kind row carries more than eight fields), per
fingerprint inventory entity plus two per compared function pair, per edge
of either index, and per collateral seed and reached identity; exhaustion
is `COMPARE_RESOURCE_LIMIT` with no partial delta. Decoding enforces the
same counts before allocation, and the public encoder refuses to mint
beyond them, so no delta the encoder produces fails the decoder's counts.
Relation comparison walks the two judged indexes' edge slices and caps the
emitted deltas at 8,000,000; comparison never holds more than the judged
inputs plus the capped outputs. The 134,217,728-byte top-down budget is
the shared SCB1 budget the limits preimage pins, enforced through the
per-section pre-allocation counts.

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
`COMPARE_INVENTORY_INVALID` respectively: every non-resource failure wraps
with its exact source code preserved, never remapped. Resource exhaustion
is the single exception: work-budget exhaustion at any layer, including
`IMPACT_RESOURCE_LIMIT` from bounded reachability, surfaces as the
comparing layer's own `COMPARE_RESOURCE_LIMIT` — the budget is one budget
and the charging rule above owns exhaustion. The outer code names the
layer that detected the failure, not the stage: a post-judgment impact
failure still reads `COMPARE_ROOT_INCOMPLETE` with its exact source,
because re-labeling it would drop the source it preserves.
`COMPARE_INTERNAL_INVARIANT` is defense-only, never a reachable verdict on
valid inputs: the union arm emits it on the unreachable both-absent case,
and `delta_epoch_id` emits it if the shared epoch-1 constants drift from
the pin.

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
  frozen hashes from the preimage texts. The oracle re-derives from the
  corpus root pairs with its own reader and encoders, including the closed
  field grammar and the strict-decode shape rules; body-delta fingerprints
  and membership are cross-checked against the recorded values rather than
  re-fingerprinted (the oracle projects inventories instead of calling
  `fingerprint_function`), so fingerprint fidelity rests on the native
  tests and the strict-decoder matrix, not on the oracle;
- exact encode, decode, and re-encode round trips and a rejection matrix
  reaching every code in the table;
- workspace-mismatch, epoch-mismatch, incomplete-root, and
  inventory-invalid tests preserving the wrapped codes;
- a determinism test comparing the same pair 128 times and both directions
  (`base`/`target` swapped yields mirrored classes);
- an S20-700 persistent libFuzzer target over delta decoding, asserting
  the decoder/comparer code partition (decoding never emits the judgment
  and precondition codes); comparison itself is covered by the corpus,
  the determinism test, and the native precondition tests, not by fuzz;
- Tier 1 plus repository-focused Tier 2 validation;
- Ariadne contract review, Nabu architecture review, and Vulcan surface
  review with every report-grade finding closed.

## Explicit exclusions

This contract does not claim:

- merge, three-way composition, conflict objects, or disjointness judgment
  (S20-520);
- block-level or operation-level CFG alignment, renaming detection, or
  moved-entity detection;
- member-level disjointness within TypeDef field 2 and Function field 2:
  concurrent edits to different members are indistinguishable beyond the
  flag bits (a deliberate v1 granularity choice, answering campaign
  question 2), so consumers treat same-field multi-member touches as
  conflicts;
- comparison across workspaces or schema epochs;
- label, source, or presentation differences beyond the `MetadataOnly`
  class;
- persistence of deltas in the object store, transactions, or refs;
- SMP1, JSON bridge, CLI, runtime, benchmark, packaging, release, or GA.

# Merge v1

Status: S20-520 contract draft, revision 4 (2026-09-05); implementation
revised against the three Council reviews (Ariadne contract review, Nabu
architecture review, Vulcan surface review, all 2026-09-04, all FAIL with
seven P0s between them). Every P0 and every P1 is closed below; the P2/P3
notes are closed or explicitly bounded. State is tracked in the machine
summary and `docs/audits/S20_520_MERGE_CLOSEOUT.md`.

## Notation

This document uses the SCB1 notation: `||` is byte concatenation, `uvar(x)`
is the unsigned varint of `x`, `len(x)` is `uvar(byte_length(x))`, and
`BLAKE3-256` is the 32-byte BLAKE3 digest. `zero32` is thirty-two zero
bytes. `O`, `A`, and `B` name the ancestor, ours, and theirs roots; `dA` and
`dB` name the S20-510 deltas `compare(O, A)` and `compare(O, B)`.

## Scope

Merge v1 is the exact, deterministic three-way composition of two complete
roots against their exact common ancestor. The Repository Model requires
that "three-way merge uses an exact common ancestor and accepts automatic
composition only when disjointness or deterministic composition is proven"
and that "ambiguity yields a canonical conflict object; text conflict
markers do not exist". This contract realizes that rule over frozen inputs
only: the S20-500 verified ancestry, the S20-510 semantic delta, the S20-250
full complete-root judgment, the S20-350 candidate record, and the S20-390
fixed-head commit. It defines:

- how the exact common ancestor of two verified heads is found;
- the merge judgment: which pairs of deltas are disjoint, which compose
  deterministically field by field, and which conflict;
- the merged root: its objects, its record facts, and its judgment;
- the merge plan: the S20-350 candidate that moves `A` to the merged root
  through the frozen S20-390 commit on `A`'s repository;
- the canonical conflict object emitted whenever composition is not proven.

`sley-repo` owns merge. The dependency direction
`sley-repo -> sley-txn -> sley-store` and
`sley-repo -> sley-query -> sley-check -> sley-ssmc` is unchanged. The
S20-390 commit path, the S20-360 validator, the S20-500 refs, and the
S20-510 delta are consumed exactly as frozen and never bypassed: a merge
never writes a root, object, receipt, or ref directly.

## Inputs and the common ancestor

A merge takes three verified revisions `O`, `A`, and `B` with their
complete-root requests (S20-250 full extraction), and the repository whose
fixed accepted head is `A`. The ancestor precondition is proven, not
asserted: the caller supplies the two bounded S20-500 ancestries (head
first) and `verify_merge_ancestor` checks that each ancestry is headed by
its side's transaction, charges one work unit per ancestry entry, and runs
`find_common_ancestor(ours_ancestry, theirs_ancestry)`, which returns the
first entry of `ours_ancestry` whose transaction identity also appears in
`theirs_ancestry`. On the protocol surface both ancestries are walked
server-side from the supplied revisions through `transaction_ancestry`, so
a caller-chosen `O` never becomes authority: supplying `O = A` would empty
`dA` and take all of `B` silently, and instead fails closed. Preconditions,
checked in order before any composition:

1. `A`, `B`, and `O` share one `workspace_id` and one `schema_epoch_id`,
   else `MERGE_WORKSPACE_MISMATCH` or `MERGE_EPOCH_MISMATCH`;
2. the supplied ancestries verify and the supplied `O` is their computed
   common entry, else `MERGE_NO_COMMON_ANCESTOR` (no shared entry within
   the bounds) or `MERGE_ANCESTOR_MISMATCH` (the supplied `O` differs from
   the computed entry); ancestry slices longer than the per-side bound fail
   `MERGE_RESOURCE_LIMIT` before any search;
3. `dA = compare(O, A)` and `dB = compare(O, B)` succeed and both sides
   extract, else `MERGE_COMPARE_FAILED` with the exact `COMPARE_*` code
   preserved, or the exact input-side extraction code preserved inside
   `MERGE_COMPARE_FAILED` when a side fails its own S20-250 judgment;
4. neither delta changes `dependency_roots`, else
   `MERGE_DEPENDENCY_ROOT_CHANGE_UNSUPPORTED` (the frozen S20-360 profile
   rejects any dependency-root change in a candidate);
5. `A` and `B` carry the same `contract_root`, `test_root`, and
   `policy_root` as `O`, else the conflict reasons `RootAnchor` or
   `PolicyRoot` below (these roots move only through their owning packages).

Because S20-500 ancestry is linear and head first, the common ancestor is
exact and unique. `A` and `B` may live in different repositories (for
example one imported through S20-540 exchange); only `A` must be the fixed
accepted head of the repository that receives the merge commit.

A `MergeSide` is either copied from a verified revision (every production
input) or built synthetically from owned objects (tests and the corpus
only). Roots, anchors, and record facts of a side are caller-asserted
inside the judgment: the production boundary is that sides come from
verified revisions and the ancestor is proven by `verify_merge_ancestor`.
Bare `judge_merge` takes the ancestor as proven and is the synthetic-input
composition; `judge_merge_verified` proves then judges and is the
production entry point both protocol handlers use.

When `dA` is empty the merge is `B`'s changes applied to `A`
(fast-forward-equivalent) and when `dB` is empty the merged root is `A` and
the plan is empty; both are valid merges and produce no conflict.

## Merge judgment

Let `E` be the union of the entity identities that appear in `dA.entities`
or `dB.entities`. For each identity, exactly one rule applies, in this
order:

| Rule | Situation | Outcome |
|---|---|---|
| J1 | present in only one delta | that side's outcome (its object, or its removal) |
| J2 | both `Added` or both `Changed`/`MetadataOnly`/`Retyped` with the same target object, or both `Removed` | convergent: that object, or convergent removal |
| J3 | `Removed` on one side and `Added`, `Changed`, `MetadataOnly`, or `Retyped` on the other | conflict `DeleteEdit` |
| J4 | both `Added` with different objects | conflict `AddAdd` |
| J5 | `Retyped` on either side and the other side not identical | conflict `KindEdit` |
| J6 | `MetadataOnly` on one side and `Changed` on the other | the `Changed` object; the identity is listed in the plan's `metadata_overridden` set |
| J7 | both `Changed` (same kind) | field composition (below) or conflict `FieldEdit` |
| J8 | both `MetadataOnly` with different objects | conflict `MetadataEdit` |

Field composition for J7 considers every `(kind, field)` in
`fields(dA, id) ∪ fields(dB, id)`:

- a field changed on one side only takes that side's field value;
- a set-valued identity field (Workspace 1, 3, 4, 5; Package 3, 4;
  Namespace 2; TypeDef 3; Function 4, 7; CapabilityRequirement 3;
  AdapterImport 6; PolicyBinding 2) changed on both sides composes to
  `(O.value ∪ dA.added ∪ dB.added) \ (dA.removed ∪ dB.removed)` when
  `dA.added ∩ dB.removed = ∅` and `dA.removed ∩ dB.added = ∅`; otherwise
  conflict `FieldEdit`;
- any other field changed on both sides is convergent when both sides hold
  the same value, otherwise conflict `FieldEdit`.

Both sides `Changed` with no field delta on either side is
`MERGE_INTERNAL_INVARIANT`, never a silent composition of `O`'s body: the
S20-510 delta names every changed field, so an empty field set means the
judgment cannot see what changed.

The composed body is `O`'s canonical body with each changed field replaced
as above. Its object carries `A`'s label and `A`'s fingerprint claim, which
is the only choice the frozen S20-390 commit reproduces: frozen
`replace_body` preserves exactly the current side's label and fingerprint,
so any other choice makes the precomputed root uncommittable
(`MERGE_RESULT_MISMATCH` on every fingerprint-bearing or labeled
composition). A label theirs changed is therefore reported, never dropped:
any composed identity whose two sides carry different labels joins the
plan's `metadata_overridden` set. This keeps J6's answer (a semantic change
overrides a metadata-only change, reported) consistent: metadata is
overridden only ever with a report.

Collateral rule, checked after J1 through J8 for every identity that
survived without conflict (an identity that took any conflict branch above
is excluded, so one identity never carries both a judgment conflict and
`Collateral`):

- an identity that `dA` classifies `Changed`, `Retyped`, or `Removed` and
  that any `dB`-added, `dB`-changed, or `dB`-retyped entity reaches through
  one or more relations of kind other than `Ownership` in `B`'s
  complete-root index (the entity's non-ownership dependents, transitively)
  is conflict `Collateral`; and symmetrically for `dB` against `dA`.
  `MetadataOnly` touches never trigger collateral on either side, and a
  removed dependent resolves by its own removal: the dependent side counts
  only `Added`, `Changed`, and `Retyped`, the last because a retyped
  dependent is a strictly stronger edit than a changed one.
- `ours_object` always comes from `A` and `theirs_object` always from `B`,
  whichever direction the check runs; a removed side contributes `zero32`.

`Ownership` relations (containment, membership, exposure, and subject
binding) are excluded because containment composes by the set rules above;
every other relation (type, value, control-flow, call, effect, capability,
contract, initializer, test-target, adapter, and definition-member) is a
semantic dependency whose compatibility with a dependent the other side
touched is not proven by structure alone, so it is never composed
automatically. The S20-510 `collateral` set, which spans all relations,
remains the reported advisory for the resulting delta; the merge rule is
the narrower non-ownership closure.

## Merged root

When no rule yields a conflict, the merged entity set is `O`'s bound
entities with every J1, J2, J6, and J7 outcome applied (removals drop the
binding). Every reused object keeps its exact `ObjectId`; only J7-composed
entities get new objects. The merged record facts are `O`'s `workspace_id`,
`schema_epoch_id`, `contract_root`, `test_root`, `policy_root`, and
`dependency_roots`, and `entry_points` equal to the merged EntryPoint
entities. The merged request MUST pass the S20-250 full complete-root
judgment; a failure is conflict `Closure` with the `IMPACT_*` numeric code
as its detail, never a merged root. The merged `StateRoot` is derived by the
frozen S20-160 builder from those facts.

## Merge plan

The plan is the S20-350 candidate that moves `A` to the merged root.
Created entities are emitted first, in derivation order (raw-ID order of
the judged identities, below), and every other operation follows in raw-ID
order of its plan target:

| Situation | Class | Precondition |
|---|---|---|
| bound in merged, not in `A`, other kinds | `CreateEntity` | `ExpectedIdentityAbsent` |
| bound in merged, not in `A`, kind 16 | `MERGE_PLAN_UNSUPPORTED` (below) | none: no plan exists |
| bound in `A`, not in merged, kind 16 | `RemoveEntryPoint` then `DeleteEntityBinding` | `ExactEntityVersion` on `A`'s object |
| bound in `A`, not in merged, other kinds | `DeleteEntityBinding` | `ExactEntityVersion` on `A`'s object |
| bound in both with different objects | `ReplaceEntityVersion` with the merged body | `ExactEntityVersion` on `A`'s object |

Identities are re-derived before the operations are formed. The frozen
S20-345 identity rule makes every `CreateEntity` target
`EntityId::derive(workspace, candidate_nonce, kind, creation_ordinal)`,
where the validator numbers creation ordinals by `CreateEntity` operation
position, so an entity that the merged root binds and `A` does not (an
entity `B` added) cannot keep `B`'s identity in `A`'s repository. The plan
therefore fixes `candidate_nonce =
BLAKE3-256("sley2.merge-plan-nonce.v1" || A.root || judged_merged.root)`
over the judged (pre-remap) merged root, assigns each such entity the
derived identity in raw-ID order of the judged identities, and emits the
creations in that same order, so derivation order and operation order
agree. (`sley2.merge-plan-nonce.v1` is a preimage separator registered in
`IDENTIFIERS_V1.md` (ADR-0048): it names no identifier, is not a `sley-id`
derived identity, and is pinned by the stage checker as raw domain bytes.) A derived identity that already names an
unrelated live entity in `A` fails `MERGE_PLAN_UNSUPPORTED` instead of
silently replacing it. Every local reference to a re-identified entity is
rewritten in every merged body (identity fields, identity sets
and lists, nested type expressions, constants, value references, immediates,
terminators, contract bindings, and test environments; `external_package`
is never local), rebuilt objects are exactly what the frozen commit path
produces for their operation (no label and no fingerprint claim for
created entities, `A`'s label and fingerprint for replaced ones), a
reference rewrite that collapses or duplicates an identity set fails
`MERGE_INTERNAL_INVARIANT` instead of emitting a partially-remapped body,
and the `(judged, plan)` pairs are recorded as the plan's `identity_map`.
The plan's merged root is the judged merged root with those bindings and
entry points remapped; it is derived by the frozen S20-160 builder from
the same anchors and dependency roots.

A theirs-added entry point (kind 16) is the one merge the single-candidate
plan cannot express: the frozen S20-350 descriptor binds `AddEntryPoint`
to `ExactEntityVersion`, and frozen S20-360 phase 3 checks every such
precondition against the base state, so no candidate can bind an entry
point it also creates, and a lone `AddEntryPoint` needs a live target the
frozen apply path rejects as missing. The plan reports
`MERGE_PLAN_UNSUPPORTED` rather than emitting that shape or keeping
theirs' identity. The recovery is to bind the entry point on `A` first
(create, then add in a second candidate) and merge again.

The plan carries `base_transaction_id = A`'s transaction, `base_root = A`'s
root, the precomputed merged root, the operations with one precondition
each, the `metadata_overridden` set, the nonce above, the `identity_map`,
and the remapped objects. The committing principal, the capability summary
of the frozen S20-370 projection, the expiry, the validation limits, and
the branch arrive with the plan at `commit_merge` through
`MergeCommitInput`. Committing the plan first proves the named branch still
resolves to the plan base (a moved branch fails with the exact frozen
`REF_NAMED_CAS_STALE` before anything is durable), then runs the frozen
S20-390 commit against `A` as the expected parent, proves the new head's
root equals the plan's merged root computed before the commit, else
`MERGE_RESULT_MISMATCH`, and only then advances the named branch with the
S20-500 direct-parent CAS, which remains the authority against a
concurrent advance: a failure after the commit leaves the merge
transaction durable but unnamed, and the merge never repairs a mismatch.
An empty plan pairs only with `A`'s own root, else `MERGE_RESULT_MISMATCH`;
it commits nothing and advances nothing. Merge commits carry no capability
grants: the projection and the commit run over an empty grant slice, so a
merge whose operations need a grant is not committable. A plan that
would need a class or transition the frozen candidate profile cannot express
is `MERGE_PLAN_UNSUPPORTED`. The restricted S20-360 success subset
(executable programs without semantic operation entities, no selected
tests) bounds which merged roots can be committed today; the judgment,
merged root, and plan are still exact for every root.

## Conflict object

Every conflict from the judgment yields one canonical record and no merged
root or plan:

```text
format_version    = 1
contract_tag      = 520
contract_domain   = "sley2.merge-conflict.v1"
digest_domain_tag = 21
kind_tag          = 520
```

```text
conflict_preimage = "SLEYSCB1" || uvar(1) || uvar(520) ||
                    ConflictSchemaEpochId[32] || len(payload) || payload
MergeConflictId = BLAKE3-256("sley2.merge-conflict.v1" || conflict_preimage)
stored_conflict = conflict_preimage || MergeConflictId[32]
```

`sley2.merge-conflict.v1` is a new `sley-id` domain (identifier-owner work
inside the S20-520 slice). The conflict schema epoch is a standalone
epoch-1 record with exactly one contract descriptor: `contract_tag = 520`,
`digest_domain_tag = 21`, `kind_tag = 520`, `required_fields = {1, ..., 8}`,
`optional_fields = {}`, `variant_tags = {}`, and the two frozen hashes below,
raw BLAKE3-256 hashes of these exact ASCII texts:

```text
field schema preimage = sley2.merge-conflict.v1.schema:required(1:conflict_version u32,2:workspace_id fixed32,3:ancestor_root fixed32,4:ours_root fixed32,5:theirs_root fixed32,6:ours_delta fixed32,7:theirs_delta fixed32,8:conflicts set conflict_entry);conflict_entry=record(1:entity_id fixed32,2:reason u32,3:kind u32,4:field u32,5:ours_object fixed32,6:theirs_object fixed32,7:detail u32);epoch=1
field_schema_hash = 008e0676f911dc82b605286624c855a21b2e9e9d397358bf0ce7f196f55c2c8b

decoder limits preimage = sley2.merge-conflict.v1.decoder-limits:stored=67108864,conflicts=131070,allocation=134217728,work=100000000
decoder_limits_hash = c6c071df3c80f18702cb0cb41298d56524e2cdba4bbe77d8a9755cfcd95d20c5
```

| Tag | Field | Type and rule |
|---:|---|---|
| 1 | `conflict_version` | `UInt<32>`; exactly `1` |
| 2 | `workspace_id` | `FixedBytes<32>` |
| 3 | `ancestor_root` | `FixedBytes<32>` |
| 4 | `ours_root` | `FixedBytes<32>` |
| 5 | `theirs_root` | `FixedBytes<32>` |
| 6 | `ours_delta` | `FixedBytes<32>`; `SemanticDeltaId` of `dA` |
| 7 | `theirs_delta` | `FixedBytes<32>`; `SemanticDeltaId` of `dB` |
| 8 | `conflicts` | `CanonicalSet` of conflict entries in raw `entity_id`, then `reason`, then `field` order; at least one entry |

A conflict entry carries the identity, the reason tag, the SSMC1 kind of
the entity in `O` (or of the added entity; `0` for the identity-free
reasons `Closure`, `RootAnchor`, and `PolicyRoot`, whose entity field is
`zero32`), the field tag for `FieldEdit` (`0` otherwise), `A`'s and `B`'s
objects (`zero32` when absent), and `detail`: the nonzero `IMPACT_*`
numeric code for `Closure`, the moved anchor class for `RootAnchor`
(`1` contract root, `2` test root, `3` both), and `0` otherwise. Reason tags:

| Tag | Reason |
|---:|---|
| 1 | `AddAdd` |
| 2 | `DeleteEdit` |
| 3 | `FieldEdit` |
| 4 | `KindEdit` |
| 5 | `Collateral` |
| 6 | `Closure` |
| 7 | `RootAnchor` |
| 8 | `PolicyRoot` |
| 9 | `MetadataEdit` |

A `Closure` conflict always carries `zero32`: the judgment names no failing
identity (projection and judgment failures surface only their code), and
carrying one would invent attribution the judgment did not compute. The
conflict set is deduplicated on the whole entry tuple. A decoder never
sorts or repairs input; the stored bytes re-encode byte-identically. The
strict decoder additionally rejects what the judgment never emits: a
nonzero kind on an identity-free reason (or zero, or above 18, elsewhere),
a nonzero detail outside `Closure` and `RootAnchor` (or outside 1-3 on
`RootAnchor`, or zero on `Closure`), and a field tag outside 1-8 on
`FieldEdit` (or nonzero elsewhere), all as
`MERGE_CONFLICT_FORMAT_INVALID`, so forged evidence cannot smuggle
arbitrary kind, field, or detail codes through a round-trip.

## Determinism and symmetry

Merging the same `(O, A, B)` twice yields the same judged merged root, the
same plan (identity map, operations, and merged root), or the same conflict
bytes. Merging `(O, B, A)` yields the same judged merged entity set and the
same composed bodies (composition is commutative) and the same conflict set
with `ours` and `theirs` swapped. Composed-object metadata follows each
merge's own ours side (`A`'s label and fingerprint one way around, `B`'s
the other) and is always reported in `metadata_overridden`, so the two
plans differ exactly by the identity remap plus the reported ours-side
metadata: each plan re-identifies the entities its own side lacks, and the
two committed roots are equal up to the derived identities of added
entities and the reported labels.

## Resource limits

| Limit | Maximum |
|---:|---|
| ancestry nodes per side | `65,536` (S20-500) |
| entities per root | `65,535` (S20-250) |
| conflict entries | `131,070` |
| plan operations | `131,070` |
| stored conflict bytes | `67,108,864` |
| charged merge work | `100,000,000` |

Merge charges one work unit per ancestry entry visited, per delta entry
read, per composed field, per collateral check, per reverse edge walked in
the collateral closure, and per plan operation; exhaustion is
`MERGE_RESOURCE_LIMIT` with no partial result. Ancestry slices longer than
the per-side bound fail before any search.

## Stable failures

Numeric codes `52000` through `52013` are exact:

| Numeric | Symbolic |
|---:|---|
| 52000 | `MERGE_NO_COMMON_ANCESTOR` |
| 52001 | `MERGE_ANCESTOR_MISMATCH` |
| 52002 | `MERGE_WORKSPACE_MISMATCH` |
| 52003 | `MERGE_EPOCH_MISMATCH` |
| 52004 | `MERGE_DEPENDENCY_ROOT_CHANGE_UNSUPPORTED` |
| 52005 | `MERGE_COMPARE_FAILED` |
| 52006 | `MERGE_RESOURCE_LIMIT` |
| 52007 | `MERGE_CONFLICT_FORMAT_INVALID` |
| 52008 | `MERGE_CONFLICT_DIGEST_MISMATCH` |
| 52009 | `MERGE_CONFLICT_CANONICAL_ORDER` |
| 52010 | `MERGE_CONFLICT_VERSION_UNSUPPORTED` |
| 52011 | `MERGE_PLAN_UNSUPPORTED` |
| 52012 | `MERGE_RESULT_MISMATCH` |
| 52013 | `MERGE_INTERNAL_INVARIANT` |

A conflict is a successful judgment, not a failure code. `COMPARE_*`,
`IMPACT_*`, commit-path (`TXN_*`, `REF_*`, `CAP_*`, `SCB_*`, and
branch-layer), and `SCB_*` failures are preserved with
their exact codes and owning numerics inside `MERGE_COMPARE_FAILED`, the commit path, and
`MERGE_CONFLICT_FORMAT_INVALID`, never remapped. `MERGE_ANCESTOR_MISMATCH`
covers both the supplied ancestor differing from the verified common entry
and the plan base differing from ours' transaction at commit.
`MERGE_COMPARE_FAILED` covers both delta-comparison failures and an input
side failing its own extraction, with the exact source code preserved
either way.

## Required evidence

Implementation acceptance requires at least:

- a frozen merge corpus under `conformance/merge/v1/` with at least:
  disjoint entity changes; disjoint field changes on one entity; composed
  set-valued fields on one entity; convergent identical changes; every
  conflict reason except the defensive `Closure` (below); a collateral
  conflict a naive entity comparison would accept, in both directions; a
  conflicted identity whose dependent the other side touched, proving the
  survivor scope; a both-sides removal proving convergent deletion; a
  fast-forward-equivalent merge; an empty merge; and the symmetric pair of
  one merged and one conflicting case;
- an independent Python reproduction of the judgment outcome, the merged
  entity set, the conflict entries, the canonical conflict bytes, and
  `MergeConflictId` over the corpus, plus a checker that recomputes both
  frozen hashes;
- a repository-backed test that commits a merge plan through the frozen
  S20-390 commit and S20-500 advance, proves the new head's root equals the
  plan's merged root, proves an added entity is re-identified with every
  reference rewritten, proves two created entities derive in operation
  order, proves a theirs-added entry point is `MERGE_PLAN_UNSUPPORTED`
  with a working bind-on-ours-first recovery, proves a composed object
  carries ours-side metadata with divergence reported, proves a conflict
  commits nothing, and proves the stale-branch and foreign-root pairings
  fail closed;
- an ancestor test over two S20-500 ancestries with the no-ancestor and
  mismatch failures, proving a caller-chosen `O = A` fails verification
  while bare judgment would take it silently;
- determinism over 128 repeated merges and the symmetry properties above;
- two S20-700 persistent libFuzzer targets: the conflict-decoder lane
  (decoder plus ancestor rule) and the judgment lane (byte-decoded
  mutation scripts over a fixed valid base root, asserting deterministic
  well-formed outcomes or frozen-coded failures);
- the `Closure` path is defensive and has no corpus case: over valid
  complete roots set composition preserved every closure rule in every
  constructed case, so no `Closure` vector could be built; its coverage is
  the strict-decoder rejection matrix plus the pinned nonzero-detail rule;
- Tier 1 plus repository-focused Tier 2 validation;
- Ariadne contract review, Nabu architecture review, and Vulcan surface
  review with every report-grade finding closed.

## Explicit exclusions

This contract does not claim:

- rename or move detection, block-level CFG merging, or semantic
  compatibility judgment beyond structural collateral;
- merges across workspaces or schema epochs, or with dependency-root,
  contract-root, test-root, or policy-root changes;
- multi-parent transactions: the merge commit is an ordinary S20-390
  candidate on `A` whose ancestry records `A` as its parent, with `O`, `B`,
  and both deltas identified only inside the plan and conflict evidence;
- octopus merges, rebase, cherry-pick, or history rewriting;
- SMP1, JSON bridge, CLI, runtime, benchmark, packaging, release, or GA.

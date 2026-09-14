# Epoch Migration Policy v1

Status: S20-760 contract draft, revision 3 (2026-09-14); Council review
pending (Ariadne contract review as the schema owner, Nabu architecture
review, Vulcan surface review). This is the master goal's M6 deliverable
"migration policy for Sley 2.x epochs" (section 16.7), which had no document.
Revision 2 answers, per item, the question section 1 requires to be answered
before an epoch is proposed: does the change alter encoded bytes or
acceptance, or can it be a profile? Revision 3 (2026-09-14) is a
curative-notes amendment against the three 2026-09-04 Council reviews (all
FAIL, no P0): it corrects the section 1 epoch test, the section 6 citations,
the section 7 agenda record, the section 8 checker claim, and the section 9
history, and adds section 10 curative notes prescribing what the FINAL draft
must contain per finding. The six determinations are unchanged: the
schema-owner rulings in the Ariadne review confirm every classification.

## Boundary

`SCHEMA_EPOCH_V1.md` freezes the epoch *record*: its fields, its identity, its
predecessor rule, and its migration descriptors. Nothing said when a new epoch
is required, what a migration must prove, or who decides. This contract freezes
that policy.

It creates no epoch, performs no migration, and decides nothing: the epoch-2
agenda of section 6 is a list of candidates for the schema owner, not an
approval. `make release-check` and `make v2` stay fail-closed, and no accepted
root, ref, or artifact changes because this document exists.

## 1. What forces a new epoch

An epoch freezes exactly the nine facts of master goal section 6.4: object
kinds, field numbers, canonical field order, required and optional fields, the
opcode table, type rules that affect encoding, hash algorithm identifiers, the
extension policy, and decoder limits that affect validity.

A change to any of them requires a new epoch. Concretely, a change requires a
new epoch when it alters any of the nine facts for an existing identity:
when an artifact that a prior epoch's decoder produced or accepted under
that identity would change bytes or acceptance. Additive changes count: a
new optional field, a new entity kind, a new type tag, or a raised decoder
limit still alters the frozen facts even when every prior artifact still
decodes. A profile is permitted only when the change carries a new disjoint
identity (profile tag, bytecode magic, cache preimage, or metadata value)
under which no prior-identity artifact changes bytes or acceptance, and no
new-identity artifact is ever presented to the old decoder as the old
identity. Disjoint identity is what licenses the two precedents below
(`SLEYBC02` with `lowering_profile = 2`; `semantic_profile` value 2 under
its validation profile); unaffected prior artifacts alone do not.

- adding, removing, or renumbering an entity kind, a record field, or a type
  tag;
- adding an opcode, changing an opcode's signature rule, or changing which
  opcodes an execution profile may run, when the change alters lowered bytes
  or a cache key preimage;
- changing a canonical encoding rule, a normalization rule, or a hash domain;
- raising or lowering a decoder limit that decides validity;
- changing the extension policy or the set of declared contracts.

A change does not require a new epoch when it is invisible to encoded bytes and
to acceptance: adding a checker, a fuzz lane, an oracle, a test, an evidence
document, or a diagnostic; deriving a new report from existing artifacts; or
adding a *profile* that carries its own identity, as the S20-260/S20-270
extended opcode profile does with `lowering_profile = 2` and bytecode
`SLEYBC02`. Profile separation is the preferred alternative to an epoch bump
and must be considered first.

## 2. What a migration must prove

Master goal section 6.4 requires seven steps. This policy states them as
obligations with their evidence:

| # | Obligation | Evidence |
|---:|---|---|
| 1 | preserve the old decoder | the old epoch's decoder remains selectable and its conformance corpus still passes |
| 2 | decode under the old epoch | every migrated artifact is decoded by the old decoder before any new construction |
| 3 | construct new canonical state under the new epoch | the new state root is built by the new epoch's canonical construction, not by rewriting bytes |
| 4 | produce a migration transaction | one transaction whose receipt names both epochs and both roots, with `plan_id` equal to an externally approved plan identifier rather than candidate-supplied authority (`SCHEMA_EPOCH_V1.md` section 6) |
| 5 | record old and new roots | both roots appear in the receipt and in the machine summary |
| 6 | verify semantic equivalence under declared migration contracts | every `MigrationContractDescriptor` of the new epoch record is discharged, each naming its `contract_id`, verifier identity, and scope hash, with the reproduced evidence digest equal to the draft before validation succeeds (`SCHEMA_EPOCH_V1.md` section 6) |
| 7 | never overwrite the old root | the old root remains readable and referenced; the migration is additive |

A migration that cannot discharge every obligation is refused. Partial
migration, in-place rewriting, and "the old decoder was removed" are all
failures, not trade-offs.

## 3. Who decides

- The **schema owner** (Ariadne) decides whether a change requires an epoch and
  approves the new epoch record's contents.
- The **architecture owner** (Nabu) reviews whether profile separation was
  correctly preferred over an epoch bump.
- The **surface owner** (Vulcan) reviews the migration's failure closure and
  the preservation of the old decoder.
- The **operator** authorizes the migration transaction itself, because it
  writes accepted state.

No implementation may create an epoch record without those four, and this
document does not stand in for any of them.

## 4. Ordering

Exactly one epoch is active for a workspace at a time. Epoch numbers are
strictly increasing, and every epoch after the first names exactly one
predecessor (`SCHEMA_EPOCH_V1.md` section 3). A migration moves one workspace
from its current epoch to the immediate successor: skipping an epoch is
refused, and two migrations may not interleave on one workspace.

## 5. What stays true across an epoch

- Identities of artifacts already accepted under an earlier epoch do not
  change; a migration adds new identities.
- The frozen conformance corpora of earlier epochs stay in the tree and stay
  green, so a decoder regression is caught by the old vectors.
- The independent oracle keeps a decoder for every epoch it has checked.
- Evidence documents that name an epoch (the SBOM, the provenance, the
  dossier) name the epoch of the artifact they describe.

## 6. Epoch 2 candidate agenda, with determinations

These are the changes that currently want an epoch. Section 1 requires the
profile question to be answered before an epoch is proposed, so each item
carries its determination and the mechanical fact the determination rests on.
A determination is not an approval: an item marked `EPOCH REQUIRED` still
needs the four approvals of section 3, and an item marked `PROFILE` still
needs its own package, contract revision, and evidence.

| # | Candidate | Determination | Mechanical basis |
|---:|---|---|---|
| 1 | The four unsupported contract kinds (`Invariant`, `EffectBound`, `CapabilityBound`, `ResourceCeiling`) | **EPOCH REQUIRED** | `ContractSource` is a closed four-variant union (`Parameter`, `Result`, `Error`, `Global`) with no value, effect, capability, or resource evidence source. `CONTRACT_TEST_PROFILE_V1.md` section 1.1 rejects all four ContractKind tags with a preamble forbidding any future implementation from reinterpreting that rejection as acceptance, and section 1.3 admits only the four source forms. A new variant changes the frozen SSMC1 field schema whose hash `1983bc8d…` is fixed in the epoch descriptor of `SSMC1.md` and in every cache-key preimage. |
| 2 | The production-epoch semantic fingerprint requirement | **PROFILE** | `CANDIDATE_RESULT_V1.md` phases 7 and 8 state the rule: the restricted conformance epoch allows absent fingerprint claims while complete production-epoch assembly must require every master-goal-mandatory claim before GA. The requiring rule already carries its own identity through `validation_profile_id` field 4 (the exact full-v1 `ValidationProfileId`), so the decoder does not change and every epoch-1 artifact still decodes. A successor profile requires the claim; `full_v1` keeps its meaning. |
| 3a | `contract_assert` (144) execution | **PROFILE** | The opcode is in the frozen epoch-1 opcode table, `CONTRACT_TEST_PROFILE_V1.md` section 2 already accepts it statically under epoch 1, and that section assigns predicate execution to S20-270 and report evidence to S20-290. A lowering profile carries its own identity in `lowering_profile`, so no existing lowered bytes or cache key can change. |
| 3b | `test_observe` (145) execution | **EPOCH REQUIRED** | `CONTRACT_TEST_PROFILE_V1.md` section 3.4 rejects the opcode in every supplied function under epoch 1, because epoch 1 defines no execution multiplicity, path ordering, or report matching. The preamble forbids any future implementation from reinterpreting that rejection as acceptance. A profile cannot accept what the epoch's own checker refuses. |
| 3c | `effect_request` (160) and `capability_narrow` (162) execution | **PROFILE, OWNER BLOCKED** | Both opcodes are in the frozen table and neither needs a schema field that epoch 1 lacks: their runtime values are the execution-local forms of section 2 of the extended profile, which slice E5 already realized for local cells without touching `ConstData`. Re-verified 2026-09-04: no narrowing function exists in `sley-policy`, and no host services an effect, so the blocker is an owning package's semantics. |
| 3d | `adapter_invoke` (161) execution | **SPLIT, DESIGN OWED** | Invoking an existing `AdapterImport` is profile-shaped: its operands and result are ordinary values, so no adapter-handle runtime form is even needed, and `CONTRACT_TEST_PROFILE_V1.md` section 3.2 states that epoch 1 has no replay scope or cursor semantics and `AdapterImport` carries no configuration type, and that S20-280 or S20-290 *or* a later epoch must define replay scope and cursor semantics before effectful tests can pass, so an epoch is not forced for invocation. What is missing is a design nobody has written: epoch 1's `AdapterImport` carries no configuration type, so the binding from an import to a fixture would have to enter through the execution request, and no contract says what that record is, how a replay cursor advances, or how a fixture failure maps. That design belongs to S20-280 full, and this table records the question rather than answering it. Adapter replay and configuration *as schema* stay **EPOCH REQUIRED**. |

Two consequences follow, and both are recorded rather than acted on here:

- The epoch-2 agenda that genuinely needs an epoch is items 1, 3b, and the
  replay half of 3d. Items 2, 3a, and 3c are profile work under epoch 1.
- Item 3a carries no epoch blocker: determination 3a (PROFILE) answers the
  section 1 question only; the independent basis is
  `CONTRACT_TEST_PROFILE_V1.md` section 2, which already accepts
  `contract_assert` statically under epoch 1 and assigns predicate execution
  to S20-270. Slice E7a has since landed as an extended-profile slice on
  that basis (see section 7 record correction).

## 7. Explicit exclusions

- No epoch record is created, registered, or activated here.
- No migration transaction, root, or receipt is produced.
- No epoch is proposed, and no decision is made on the section 6 agenda. A
  determination answers section 1's profile question; it does not approve a
  change, schedule one, or authorize an implementation.
- Record correction (2026-09-14): determination 3a was consumed as authority
  before schema-owner review. `VM_EXTENDED_OPCODE_PROFILE_V1.md` revision 9
  landed slice E7a (`contract_assert` execution) citing this policy's section
  6, and the machine summary records `e7a_contract_assertion_landed: true`.
  Independent authority exists in `CONTRACT_TEST_PROFILE_V1.md` section 2,
  so this is a citation and sequencing defect, not a missing basis; the FINAL
  draft must cite that basis and place architecture review ahead of any slice
  landing (see section 10, N-P2-3).
- No change to the epoch-1 record, its identity, or its corpora.
- No GA claim, release decision, or publication.

## 8. Staging

`scripts/check_epoch_migration_policy.py` runs under `make quick`. Statuses:
`S20_760_CONTRACT_DRAFT_REVIEW_PENDING`, `S20_760_POLICY_ACCEPTED` (after the
three reviews read `PASS`), and `S20_760_SUPERSEDED` (when a later revision
replaces it). The checker verifies this document's sections, the ADR, the work
package row, the machine summary section, the epoch-1 bootstrap record (not a
tree-wide scan for further epoch records), and that `release-check` and `v2`
stay `NOT_IMPLEMENTED`.

It also verifies the facts the section 6 determinations rest on, so a
determination cannot rot silently: `ContractSource` still has exactly four
variants, the SSMC1 field-schema hash in the source still equals the epoch
descriptor's, the five E7 opcode tags are still in the frozen opcode table,
`CONTRACT_TEST_PROFILE_V1.md` still accepts `contract_assert` statically and
still rejects `test_observe`, the candidate-result fingerprint rule and the
E7 effect/capability ownership still read as the determinations state, and
the cache-key preimage still carries the lowering profile. A drift in any of
them fails the checker rather than leaving a stale determination standing.
The checker asserts the epoch-1 bootstrap record (epoch number 1, null
predecessor), the schema-epoch summary status, and that `release-check` and
`v2` stay `NOT_IMPLEMENTED`; it does not scan the tree for further epoch
records (see section 10, N-P1-5).

## 9. Revision history

- Revision 1 (2026-09-03): initial draft freezing the nine facts, the seven
  obligations, the four approvals, ordering, epoch-1 invariants, and the
  epoch-2 candidate agenda.
- Revision 2: the agenda carries the profile-versus-epoch determination per
  item with its mechanical basis and drift checker (ADR-0046 decision 6).
- Revision 3 (2026-09-14): curative-notes amendment against the three
  2026-09-04 Council reviews (all FAIL, no P0). Corrects the section 1 epoch
  test (disjoint identity; additive changes count), the section 6 citations
  (items 1, 2, 3b, 3d), the section 7 agenda record (E7a sequencing
  correction), the section 8 checker claim (bootstrap record, not a tree
  scan), and this history; adds section 10 curative notes. The six
  determinations are unchanged.

## 10. Curative notes (revision 3, 2026-09-14)

These notes answer every finding of the three 2026-09-04 Council reviews
(Ariadne contract, Nabu architecture, Vulcan surface; all FAIL, no P0) with
implementable directives for the FINAL draft. Items marked DONE IN REV 3 are
closed by the revision 3 text or checker above; all other items are owned
work the FINAL draft must land, with the owner named. This draft creates no
epoch, performs no migration, and decides nothing, so every directive below
is contract or checker text, never an implementation claim.

### Ariadne contract review

- A-P1-1 (section 1 test weaker than the nine facts): DONE IN REV 3. The
  test now scopes to the nine facts for an existing identity, counts
  additive changes, and permits a profile only under a new disjoint
  identity that is never presented to the old decoder as the old identity.
  FINAL must keep both halves; dropping either reopens the finding.
- A-P1-2 (section 7 denied the E7a landing): DONE IN REV 3. Section 7 now
  records that determination 3a was consumed as authority before
  schema-owner review, names the independent basis
  (`CONTRACT_TEST_PROFILE_V1.md` section 2), and points at N-P2-3 for the
  sequencing fix.
- A-P1-3 (checker covered only items 1, 3a, 3b): DONE IN REV 3. The checker
  now also pins the item 2 fingerprint rule
  (`CANDIDATE_RESULT_V1.md` restricted-epoch-absent-claims text and the
  value-2 validation-profile condition in `TRANSACTION_MODEL_V1.md`) and the
  item 3c ownership block (the E7 exclusion pending S20-240/280/380
  ownership in `VM_EXTENDED_OPCODE_PROFILE_V1.md`).
- A-P2-4 (obligations 4 and 6 omit `plan_id` / reproduction /
  `contract_id`): DONE IN REV 3 for the transcription half. Section 2 now
  requires an externally approved `plan_id`, per-descriptor `contract_id`
  with verifier identity and scope hash, and reproduced-digest equality
  with the draft. The multi-descriptor shape defect stays open under
  N-P1-1 below.
- A-P2-5 (section 6 citation errors): DONE IN REV 3. Item 1 cites sections
  1.1 and 1.3 with the preamble; item 3d cites section 3.2 with its MUST;
  item 3b cites section 3.4 with the preamble.
- A-P2-6 (item 2 had no contract citation): DONE IN REV 3. The basis now
  cites `CANDIDATE_RESULT_V1.md` phases 7-8 and `validation_profile_id`
  field 4 (exact full-v1 identity), with the checker pin in A-P1-3.
- A-P2-7 (checker pins the draft status string; ADR decision 6 unpinned):
  DONE IN REV 3. The checker accepts the draft, accepted, and superseded
  statuses by constant (not by a pinned revision-2 literal) and pins ADR
  decision 6; any revision 4 must extend the status set the same way.
- A-P3-a (revision strings: header rev 2 vs section 9 "Revision 1", ADR
  status "revision 1" vs its own decision 6): DONE IN REV 3. Sections 9-10
  and ADR-0046 carry the revision 3 record; FINAL must bump all three
  together (spec Status, section 9, ADR Status) or the checker fails.
- A-P3-b (section 6 "unblocked today" stale after E7a landed): DONE IN REV
  3. The consequence now reads as a no-epoch-blocker observation with the
  independent basis named and the landing disclosed.
- A-P3-c (checker docstring "five facts" vs six checked): DONE IN REV 3.
  The docstring names all six determination groups including item 2 and 3c.
- A-P3-d (obligation 7 evidence names no failure code or rule): OPEN.
  FINAL must add a failure-code column to the section 2 table binding every
  obligation to the frozen `SCHEMA_EPOCH_V1.md` section 7 codes (see V-P2-3;
  obligations 1, 5, 7 need codes assigned, not prose).
- A-P3-e (`ContractSource` check counts variants, not names): OPEN. FINAL
  must extend the checker to assert the exact four variant names
  (`Parameter`, `Result`, `Error`, `Global`), so a rename to a value or
  effect source fails instead of passing at four.

### Nabu architecture review

- N-P1-1 (obligations 4 and 6 jointly unimplementable for multi-descriptor
  epochs): OPEN, schema owner decides. FINAL must either widen
  `MigrationPlan` / `MigrationTransactionDraft` to carry a descriptor set
  (owning crates `sley-schema`; `SCHEMA_EPOCH_V1.md` section 6 amendment),
  or scope obligation 6 to the in-plan descriptors and name who checks
  set-completion. One transaction carrying exactly one `contract_id` cannot
  discharge a `CanonicalSet` of two.
- N-P1-2 (section 4 scopes ordering to the workspace; epoch binds per root
  and per ref): OPEN, architecture owner decides. FINAL must re-scope the
  invariant to the ref, or require all refs of a workspace to migrate as one
  unit, and must state the fate of unmigrated sibling branches and of
  cross-epoch merge (`MERGE_V1.md` refuses `MERGE_EPOCH_MISMATCH` 52003).
  Migrating one ref today falsifies "exactly one epoch active per
  workspace" and strands siblings unmergeable.
- N-P1-3 (two incompatible section 1 scopes; opposite verdicts on
  `ContractSource` widening vs `semantic_profile` widening): HALF DONE IN
  REV 3. The disjoint-identity restatement is in section 1. OPEN half:
  FINAL must scope the epoch test to encodings committed by an epoch
  `ContractDescriptor` (`field_schema_hash` / `decoder_limits_hash`,
  `SCHEMA_EPOCH_V1.md` section 4) with other envelopes governed by their
  owning contract's revision, and must add the named third rule for
  in-place widening of a closed value set (neither identity separation nor
  epoch bump; breaks forward compatibility).
- N-P1-4 (the preferred profile path has no gate): OPEN. FINAL must add a
  profile gate to section 3 naming who approves creating a profile, who
  allocates profile identity values, the single registry of those values,
  and which profile tuples stay conformance-tested. Sixteen profile specs
  exist and `lowering_profile` already mirrors into
  `REPORT_ENVELOPE_PROFILE_V1.md` as `execution_profile`.
- N-P1-5 (section 8 overstates the checker; no tree scan): DONE IN REV 3 as
  a narrowed claim. Section 8 now states exactly what the checker asserts
  (bootstrap record, summary status, closed gates, determination facts).
  FINAL must either implement a real tree scan for epoch records beyond
  epoch 1 or retain the narrowed claim; re-widening the prose without the
  scan reopens the finding.
- N-P2-1 (four approvals have no artifact hook): OPEN. FINAL must bind the
  section 3 approvals to the migration `plan_id` (externally approved plan
  identifier, `SCHEMA_EPOCH_V1.md` section 6, carried at
  `crates/sley-schema/src/lib.rs:797`) and to the epoch record's
  `SchemaEpochId`, so "approved" is a checkable identifier, not narrative.
- N-P2-2 (ADR status stale at revision 1): DONE IN REV 3. ADR-0046 carries
  the revision 3 record; the checker pins its decision markers including
  decision 6.
- N-P2-3 (E7a landed citing an unaccepted draft): OPEN, sequencing fix.
  FINAL (with the VM profile owner) must cite the independent basis
  (`CONTRACT_TEST_PROFILE_V1.md` section 2) as the slice authority in
  `VM_EXTENDED_OPCODE_PROFILE_V1.md`, or record in section 7 that a PROFILE
  determination is what unblocks a slice and place architecture review ahead
  of the slice. Implementation must never again land on a determination this
  review has not yet reviewed.
- N-P2-4 (checker pins decisions 1-5 and determinations 3a/3b only): DONE IN
  REV 3. ADR markers include decision 6 and determination facts cover items
  1, 2, 3a, 3b, 3c-discriminator, and the cache-key binding.
- N-P3-1 (section 9 "Revision 1 carries none" in a rev-2 document): DONE IN
  REV 3 (section 9 is now a real history).
- N-P3-2 (section 4 cites `SCHEMA_EPOCH_V1.md` section 3 for no-skip;
  enforcement is section 6): OPEN, one-line FINAL fix: cite section 6 so
  the rule has an enforcement point.
- N-P3-3 (`determination_facts` counts indented lines to the first `}`):
  OPEN. FINAL must parse the `ContractSource` enum by variant declaration
  (with A-P3-e exact-name assertion) so attribute lines or brace-bearing
  payloads cannot silently pass or fail the count.
- N-P3-4 (ADR consequences "three of the six determinations"; row 3d is
  SPLIT): DONE IN REV 3. ADR-0046 phrases it as three of six rows.

### Vulcan surface review

- V-P1-1 (obligation 7 closes overwrite, not erasure by GC): OPEN, with
  Nabu on retention-anchor coordination (S20-180 / S20-500 ownership).
  FINAL must state that the old root and its reachable state stay durably
  retained after migration under a named retention anchor, and must name
  which `GARBAGE_COLLECTION_V1.md` anchor kind carries it or owe a new
  migration-predecessor kind. A caller-owned retention snapshot that anchors
  neither the old root nor the migration transaction can otherwise collect
  the old root with every obligation discharged and no failure code fired.
- V-P1-2 (old-decoder preservation is migration-time only; "supported" is an
  exit; corpus evidence does not require the ID-selected decoder): OPEN.
  FINAL must promote preservation to a section 5 standing invariant, state
  that migrating off an epoch never renders it unsupported, and require the
  frozen corpus to run under the decoder selected by exact `SchemaEpochId`
  (the standing rule is `SCHEMA_EPOCH_V1.md:126`; the exit is
  `REPOSITORY_PACK_V1.md:154` rejecting "unsupported epochs").
- V-P1-3 (partially discharged migration wedges on the occupied target
  slot): OPEN. FINAL must state that the active epoch changes only on atomic
  acceptance of the migration transaction, that a refused migration leaves
  the workspace at the old epoch with the old root as head, that an
  undischarged new root is not accepted state and is not selectable, and
  must give the discard path that clears the target slot (else retry is
  refused forever under `SCHEMA_ROOT_OVERWRITE_FORBIDDEN`).
- V-P2-1 (section 6 consequence read as scheduling permission): DONE IN REV
  3. The consequence is an observation naming the independent basis, and the
  preamble plus section 7 still require the slice's own package, contract
  revision, and evidence.
- V-P2-2 (ADR revision drift unguarded): DONE IN REV 3 (ADR-0046 revision 3
  record; checker pins decision 6).
- V-P2-3 (obligations not bound to frozen failure codes): OPEN, same work as
  A-P3-d. FINAL section 2 gains the code column against the eight frozen
  `SCHEMA_EPOCH_V1.md` section 7 codes so refusal is mechanically checkable.
- V-P3-1 (section 9 revision miss): DONE IN REV 3.
- V-P3-2 (ADR dated 2026-09-03 summarizes the 2026-09-04 3c
  re-verification): DONE IN REV 3. ADR-0046 carries the 2026-09-14 revision
  3 record covering the re-verified ownership block.

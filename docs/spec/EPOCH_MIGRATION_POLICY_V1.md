# Epoch Migration Policy v1

Status: S20-760 contract draft, revision 2 (2026-09-03); Council review
pending (Ariadne contract review as the schema owner, Nabu architecture
review, Vulcan surface review). This is the master goal's M6 deliverable
"migration policy for Sley 2.x epochs" (section 16.7), which had no document.
Revision 2 answers, per item, the question section 1 requires to be answered
before an epoch is proposed: does the change alter encoded bytes or
acceptance, or can it be a profile?

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
new epoch when it would alter the bytes or the acceptance of an artifact that a
prior epoch's decoder produced or accepted:

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
| 4 | produce a migration transaction | one transaction whose receipt names both epochs and both roots |
| 5 | record old and new roots | both roots appear in the receipt and in the machine summary |
| 6 | verify semantic equivalence under declared migration contracts | every `MigrationContractDescriptor` of the new epoch record is discharged, with its verifier identity and scope hash |
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
| 1 | The four unsupported contract kinds (`Invariant`, `EffectBound`, `CapabilityBound`, `ResourceCeiling`) | **EPOCH REQUIRED** | `ContractSource` is a closed four-variant union (`Parameter`, `Result`, `Error`, `Global`) with no value, effect, capability, or resource evidence source, and `CONTRACT_TEST_PROFILE_V1.md` sections 1.5 and 1.6 reject the four kinds for exactly that reason. A new variant changes the frozen SSMC1 field schema whose hash `1983bc8d…` is fixed in the epoch descriptor of `SSMC1.md` and in every cache-key preimage. |
| 2 | The production-epoch semantic fingerprint requirement | **PROFILE** | The claim is already optional field 4 of the frozen epoch-1 descriptor, so the decoder does not change and every epoch-1 artifact still decodes. Requiring the claim is a candidate-validation rule, and that rule already carries its own identity through the validation profile record, exactly as `full_v1` does today. A successor profile requires the claim; `full_v1` keeps its meaning. |
| 3a | `contract_assert` (144) execution | **PROFILE** | The opcode is in the frozen epoch-1 opcode table, `CONTRACT_TEST_PROFILE_V1.md` section 2 already accepts it statically under epoch 1, and that section assigns predicate execution to S20-270 and report evidence to S20-290. A lowering profile carries its own identity in `lowering_profile`, so no existing lowered bytes or cache key can change. |
| 3b | `test_observe` (145) execution | **EPOCH REQUIRED** | `CONTRACT_TEST_PROFILE_V1.md` section 3.4 rejects the opcode in every supplied function under epoch 1, because epoch 1 defines no execution multiplicity, path ordering, or report matching, and section 1.1 forbids any future implementation from reinterpreting that rejection as acceptance. A profile cannot accept what the epoch's own checker refuses. |
| 3c | `effect_request` (160) and `capability_narrow` (162) execution | **PROFILE, OWNER BLOCKED** | Both opcodes are in the frozen table and neither needs a schema field that epoch 1 lacks: their runtime values are the execution-local forms of section 2 of the extended profile, which slice E5 already realized for local cells without touching `ConstData`. Re-verified 2026-09-04: no narrowing function exists in `sley-policy`, and no host services an effect, so the blocker is an owning package's semantics. |
| 3d | `adapter_invoke` (161) execution | **SPLIT** | Invoking an existing `AdapterImport` is profile-shaped for the same reason as 3c. Adapter replay and configuration are **EPOCH REQUIRED**, because `AdapterImport` carries no configuration type and epoch 1 has no replay scope or cursor semantics. |

Two consequences follow, and both are recorded rather than acted on here:

- The epoch-2 agenda that genuinely needs an epoch is items 1, 3b, and the
  replay half of 3d. Items 2, 3a, and 3c are profile work under epoch 1.
- Item 3a is unblocked today: it needs no epoch, no new owner, and no schema
  change, so it is available as an extended-profile slice.

## 7. Explicit exclusions

- No epoch record is created, registered, or activated here.
- No migration transaction, root, or receipt is produced.
- No epoch is proposed, and no decision is made on the section 6 agenda. A
  determination answers section 1's profile question; it does not approve a
  change, schedule one, or authorize an implementation.
- No change to the epoch-1 record, its identity, or its corpora.
- No GA claim, release decision, or publication.

## 8. Staging

`scripts/check_epoch_migration_policy.py` runs under `make quick`. Statuses:
`S20_760_CONTRACT_DRAFT_REVIEW_PENDING`, `S20_760_POLICY_ACCEPTED` (after the
three reviews read `PASS`), and `S20_760_SUPERSEDED` (when a later revision
replaces it). The checker verifies this document's sections, the ADR, the work
package row, the machine summary section, that no epoch record beyond epoch 1
exists in the tree, and that `release-check` and `v2` stay `NOT_IMPLEMENTED`.

It also verifies the facts the section 6 determinations rest on, so a
determination cannot rot silently: `ContractSource` still has exactly four
variants, the SSMC1 field-schema hash in the source still equals the epoch
descriptor's, the five E7 opcode tags are still in the frozen opcode table,
`CONTRACT_TEST_PROFILE_V1.md` still accepts `contract_assert` statically and
still rejects `test_observe`, and the cache-key preimage still carries the
lowering profile. A drift in any of them fails the checker rather than leaving
a stale determination standing.

## 9. Clarifications

Revision 1 carries none.

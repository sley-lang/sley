# Epoch Migration Policy v1

Status: S20-760 contract draft, revision 1 (2026-09-03); Council review
pending (Ariadne contract review as the schema owner, Nabu architecture
review, Vulcan surface review). This is the master goal's M6 deliverable
"migration policy for Sley 2.x epochs" (section 16.7), which had no document.

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

## 6. Epoch 2 candidate agenda (proposals, not decisions)

These are the changes that currently want an epoch. Each is a candidate for the
schema owner, with the reason it cannot be a profile:

1. **The four unsupported contract kinds** (`Invariant`, `EffectBound`,
   `CapabilityBound`, `ResourceCeiling`) of `CONTRACT_TEST_PROFILE_V1.md`.
   They add contract descriptors the epoch record's contract set must carry,
   so they change what a conforming epoch declares.
2. **The production-epoch semantic fingerprint requirement** recorded by
   S20-360 as `full_ga_fingerprint_requirement_complete: false`, which changes
   what a candidate must present, and therefore acceptance.
3. **The five E7 opcodes** (144 contract assertion, 145 test observation, 160
   effect request, 161 adapter invocation, 162 capability narrowing). Their
   semantics need owners first; whether they need an epoch or a further profile
   depends on whether their judgment changes existing lowered bytes, which
   section 1 says must be answered before an epoch is proposed.

The agenda is deliberately short: everything else the goal names is
implemented under epoch 1 with profile separation.

## 7. Explicit exclusions

- No epoch record is created, registered, or activated here.
- No migration transaction, root, or receipt is produced.
- No decision is made on the section 6 agenda.
- No change to the epoch-1 record, its identity, or its corpora.
- No GA claim, release decision, or publication.

## 8. Staging

`scripts/check_epoch_migration_policy.py` runs under `make quick`. Statuses:
`S20_760_CONTRACT_DRAFT_REVIEW_PENDING`, `S20_760_POLICY_ACCEPTED` (after the
three reviews read `PASS`), and `S20_760_SUPERSEDED` (when a later revision
replaces it). The checker verifies this document's sections, the ADR, the work
package row, the machine summary section, that no epoch record beyond epoch 1
exists in the tree, and that `release-check` and `v2` stay `NOT_IMPLEMENTED`.

## 9. Clarifications

Revision 1 carries none.

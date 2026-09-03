# ADR-0046: profile separation is preferred to an epoch bump, and migrations are additive

Status: proposed; the S20-760 policy is a draft at revision 1 with Council
review pending; no epoch is created and no migration is performed

Date: 2026-09-03

## Context

The master goal lists "migration policy for Sley 2.x epochs" among the M6
deliverables and states the seven obligations of an epoch migration, but no
document in the repository said when an epoch is required, what a migration
must prove, or who decides. `SCHEMA_EPOCH_V1.md` freezes the epoch record and
its migration descriptors only.

Meanwhile the work of this phase repeatedly faced the question in practice. The
extended opcode profile added fifty-two opcodes without an epoch, by carrying
its own `lowering_profile` and bytecode magic. The transaction semantic profile
added a second value rather than redefining the first. Both preserved every
earlier identity. By contrast the four unsupported contract kinds and the
production-epoch fingerprint requirement change what a conforming epoch
declares and what a candidate must present, so they cannot hide inside a
profile.

## Decision

1. **Profile separation first.** A change that can carry its own identity
   (profile tag, bytecode magic, cache preimage, or metadata value) does so,
   and no epoch is bumped. An epoch is required only when the change alters the
   bytes or the acceptance of artifacts an earlier decoder produced or
   accepted.
2. **Migrations are additive.** The old decoder stays selectable, the old
   corpora stay green, the old root stays readable, and the migration produces
   a new root through canonical construction rather than rewriting bytes. Any
   obligation that cannot be discharged refuses the migration.
3. **One successor at a time.** Epoch numbers are strictly increasing, each
   epoch names one predecessor, migrations do not skip and do not interleave on
   one workspace.
4. **Four approvals.** The schema owner decides that an epoch is required and
   approves its record; architecture reviews the profile-versus-epoch call;
   surface reviews failure closure and old-decoder preservation; the operator
   authorizes the transaction, because it writes accepted state.
5. **An agenda, not a decision.** The document lists the three candidates that
   currently want an epoch and explicitly does not decide them.

## Consequences

- The schema epoch gate now has a concrete object to approve or reject instead
  of an unstated question.
- Future work has a test for "does this need an epoch?" that does not depend on
  recalling the master goal's prose.
- Recording the profile-first preference makes the extended opcode profile and
  the semantic profile value defensible rather than improvised.

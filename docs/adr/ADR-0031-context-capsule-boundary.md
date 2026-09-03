# ADR-0031: Context capsule provenance and omission boundary

Status: proposed; the S20-320 full contract is a draft at revision 1 with
Council review pending; implementation pending

Date: 2026-09-03

## Context

The restricted S20-320 capsule wraps only complete restricted responses
with fixed no-omission, no-truncation, no-continuation status and a
distinct identity domain, because the restricted queries could not page
lawfully and no verified workspace, root, or session existed (ADR-0013).
The full S20-310 engine now answers nineteen classes over a verified root
with exact `total_count` and typed continuation cursors, so a capsule can
carry lawful omission status and verified provenance without inventing
either. S20-330 still owns negotiated session authority.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **Question, provenance, status, facts.** The master capsule restates
   the exact request, copies the provenance the engine bound (workspace,
   epoch, root, snapshot, query identity), states completeness, truncation,
   exact `total_count`, `returned`, `omitted`, and `next_after`, copies the
   exact response record, and reorganizes its facts into raw-identity
   dictionaries with kinds, relationships, roots, objects, and fingerprints.
2. **Bound source only.** The constructor accepts one `RootQueryRequest`
   and the `RootQueryResponse` produced for it, bound by query identity;
   nothing else can construct a capsule.
3. **Session reserved, never implied.** The session binding is the fixed
   arm `None`; the `Negotiated` arm is reserved for S20-330 and not
   constructible, so no capsule is a handle.
4. **Master identity.** The registered `sley2.context-capsule.v1` domain
   names the capsule; the restricted `SLEYRQC1` capsule is untouched.
5. **Codes.** Four codes 32008 through 32011 are appended to the frozen
   32000 range; `QUERY_*` failures stay upstream.
6. **Staging.** `scripts/check_context_capsule_profile.py` binds the
   contract, ADR, work-package row, and summary section, and fails closed
   if the capsule module appears before the summary allows implementation.

## Consequences

- SMP1 (S20-400) gains a self-describing evidence envelope to transport,
  and S20-330 gains the exact field to bind a negotiated session into.
- A reader can prove a continuation walk complete from its capsules alone,
  which is what makes paged omission lawful under the T14 gate.

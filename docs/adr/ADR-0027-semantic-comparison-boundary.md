# ADR-0027: Semantic comparison delta boundary

Status: proposed; the S20-510 contract is a draft at revision 1 with Council
review pending; implementation pending

Date: 2026-09-03

## Context

The Repository Model requires root comparison to emit typed entity, type,
signature, CFG, call, effect, capability, contract, test, entry-point, and
dependency deltas, and S20-520 merge must accept automatic composition only
when disjointness is proven. The full S20-250 profile now supplies a
complete-root request over all eighteen kinds, an exact impact index, and
restricted-profile fingerprints for functions and type definitions, so every
delta class can be derived from frozen inputs without a second semantic
model. The work-package row names a missed collateral change as the primary
risk.

The Council lanes were still unavailable at this draft (see ADR-0026 and
the S20-250 campaign record); the design is the integrator's and is
submitted to Ariadne, Nabu, and Vulcan as soon as a lane returns.

## Decision

1. **One record, five sections.** Semantic Comparison v1
   (`docs/spec/SEMANTIC_COMPARISON_V1.md`, contract tag 510, domain
   `sley2.semantic-delta.v1`, `digest_domain_tag` 20) emits one canonical
   SCB1 record with entity, field, body, relation, and root-set/collateral
   sections. The Repository Model's eleven delta classes are defined as
   exact views over those sections, not as separate encodings.
2. **Derivation only from frozen inputs.** Change classes compare bindings
   and canonical definitions; field deltas compare schema fields of the
   normative bodies; body deltas compare the restricted `Function`
   fingerprints over each root's inventory; relation deltas are the
   symmetric difference of the two complete-root indexes' direct edges;
   collateral is the restricted transitive impact over each root's own
   index. No new semantic judgment, matching heuristic, or edge kind exists.
3. **Ownership.** `sley-repo` owns comparison over two verified revisions'
   complete-root requests; `sley-query` and `sley-ssmc` are unchanged.
   `sley-id` gains the thirty-first domain inside the slice.
4. **Fail closed.** Different workspaces or epochs, an incomplete root, or an
   incomplete function inventory fail before any delta with the wrapped
   `IMPACT_*` or `FINGERPRINT_*` code preserved; identical roots yield an
   empty, valid delta.
5. **Staging.** `scripts/check_semantic_comparison_spec.py` binds the
   contract, ADR, work-package row, summary section, and both frozen
   hashes, and fails closed if `crates/sley-repo/src/compare.rs` or the
   corpus appears before the summary allows implementation.

## Consequences

- S20-520 merge gains a deterministic, typed, digest-identified input and a
  collateral set to test disjointness against.
- The S20-700 merge-adjacent surface gains a delta decoder fuzz target; the
  merge engine itself remains the one absent Section 18.5 surface.
- Block-level CFG alignment and move or rename detection stay outside the
  frozen boundary; a later contract may add them without changing this
  record's identity domain.

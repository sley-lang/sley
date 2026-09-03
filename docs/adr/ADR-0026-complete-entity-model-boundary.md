# ADR-0026: Complete entity model and complete-root impact boundary

Status: proposed; the S20-250 full contract is a draft at revision 2 with
Council review pending; implementation landed against the draft at `e78a1ab`
(closeout `docs/audits/S20_250_FULL_ENTITY_BODIES_CLOSEOUT.md`)

Date: 2026-09-03

## Context

The S20-200 schema freezes eighteen entity bodies, but the restricted S20-250
profile models only kinds 4 through 15, requires field 4 to be absent
elsewhere, and states that a later package must add the six missing bodies
before S20-300, S20-510, or GA may claim a complete-root impact index. The
local completion frontier names those bodies as the next dependency-complete
work and forbids a second host model or locally invented edge kinds.

Two frozen facts constrain the design. SSMC1 section 8 requires a semantic
fingerprint only on GA-valid `TypeDef` and `Function` entities. The S20-360
validator already extracts a private reference graph over all eighteen kinds
(`crates/sley-policy/src/candidate_program.rs`) with the same relationship
tags the impact profile uses, treats every root entry point as an EntryPoint
entity, derives root `dependency_roots` from the DependencyBinding bodies, and
never resolves `external_package` locally.

At the time of this draft every Council lane was unavailable: the gateway's
Claude CLI provider returned `OAuth access token has been revoked`, the
`openai/gpt-5.6-sol` provider returned `API rate limit reached`, and the
`anthropic/claude-opus-5` model is not registered for the specialist harness.
The design below was therefore made by the front-line integrator from the
frozen constraints and is submitted to Ariadne, Nabu, and Vulcan as soon as a
lane returns; their findings are applied as contract revisions before the
freeze.

## Decision

1. **One normative model.** `sley-ssmc` owns `WorkspaceDefinition`,
   `PackageDefinition`, `NamespaceDefinition`, `EntryPointDefinition`,
   `PolicyBindingDefinition`, and `DependencyBindingDefinition` with the exact
   schema fields. `EntryExposure` moves into `sley-ssmc` and `sley-mutate`
   re-exports it so the generated proposal codec is byte-identical. The
   generated bodies stay the proposal codec and the S20-360 projection maps
   them onto the definitions.
2. **No new fingerprints.** The six kinds carry no field 4; equivalence is
   canonical body equality. No preimage, domain, or profile version changes.
3. **Edge vocabulary mirrors S20-360.** The six bodies' edges use only the
   twelve frozen kinds, assigned exactly as the validator's reference graph
   assigns them: Ownership for containment, membership, exposure, and subject
   binding; Capability, Contract, and TestTarget for the workspace and policy
   sets. `external_package` and `dependency_root` create no edge. An
   edge-agreement test between the validator graph and the impact index is
   required evidence.
4. **Complete-root request and closure.** The complete-root request is
   derived from one strictly decoded `StateRoot` record and the exact objects
   it binds, carries the record's `entry_points` and `dependency_roots` as
   its only root facts, and is judged by the eleven closure rules of the
   contract's section 6.1 with twelve new stable codes 25013 through 25024.
   The pure judgment lives in `sley-query`; the extraction adapter lives in
   `sley-repo` over a verified revision and projects through the public
   `sley-policy` projection, so `sley-query` gains no dependency on
   `sley-store`, `sley-mutate`, or `sley-policy`.
5. **Restricted consumers stay restricted.** The S20-300, S20-310, and
   S20-320 profiles keep "kinds 4 through 15" and their fixed vectors; the
   snapshot builders fail closed with
   `INDEX_SNAPSHOT_COMPLETENESS_UNSUPPORTED` on any other kind. The full
   S20-300 snapshot is a later package.
6. **Contract form.** The full profile is a new document,
   `docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md`, composed over the
   unchanged restricted profile, because `scripts/check_fingerprint_impact_profile.py`
   binds the restricted status line and the restricted consumers bind its
   digests. `scripts/check_complete_entity_impact_profile.py` binds this
   contract, the ADR, the work-package row, and the machine-summary section,
   and fails closed if the six definitions appear before the summary says the
   contract is frozen.

## Consequences

- The frontier's fail-closed marker on `pub struct WorkspaceDefinition` is
  replaced by the staged checker: the definitions may appear only after the
  freeze, and the frontier re-audit records the change.
- S20-510 semantic comparison and the full S20-300 snapshot become
  dependency-complete once the implementation and its reviews land.
- No cross-root loading exists anywhere in the semantic core; a later
  package that needs external-root content must add its own contract.

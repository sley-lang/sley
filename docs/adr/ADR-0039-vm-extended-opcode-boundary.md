# ADR-0039: the extended opcode profile as a second, explicit VM profile

Status: accepted architecture; current contract revision 17 (2026-09-17).
Slices E1–E6, E7a and E8 are implemented. Revision 16 specified the E7
capability-handle host-binding boundary. Revision 17 implements its recursive
result-escape guard for `AdapterHandle` and `CapabilityToken`; 160/162
execution remains deferred to S20-280/S20-380 owners.
Historical review records remain
preserved; independent item-level review accepted the revision 15 corrections
in `evidence/review/vm-nabu-correction-review-2026-09-15.md`.
Revision 12 acceptance and the revision 13 E8 implementation are historical
milestones, not the current revision number.

Date: 2026-09-03

## Context

The restricted S20-260 and S20-270 profiles lower and execute three Boolean
opcodes and all five terminators, and their vectors, cache keys, and
observation digests are frozen. Full GA is blocked on the other fifty-two
opcodes, whose shapes and failure values the epoch-1 manifest and the S20-210
type checker already fix. The candidate demo now executes through SMP1, so
every new family becomes reachable end to end.

The Council lanes were still unavailable at this draft (see ADR-0026); the
design is the integrator's and is submitted to Ariadne, Nabu, and Vulcan as
soon as a lane returns.

## Decision

1. **A second profile, not a mutation.** `EXTENDED_V1` with its own
   lowering profile tag and bytecode magic leaves every restricted byte
   and identity untouched.
2. **The manifest and type checker are the authority.** Operand counts,
   immediates, result shapes, and builtin failure codes come from the
   epoch-1 manifest and S20-210; the profile adds rules only where the
   manifest leaves runtime meaning open (order, overflow, NaN, map order).
   Where the profile depends on a property S20-210 deliberately does not
   establish, it must verify that property rather than assume it. Ordered-map
   entry order is the case: `TYPE_SYSTEM_V1.md` section 5 reserves the byte
   ordering to the SCB codec, while `equal` and `value_hash` read it
   structurally, so the profile asks the codec about every map it did not
   build itself and refuses one with no canonical form. It does not sort the
   value, which would put a second ordering authority in the VM.
3. **Immediates enter the bytecode explicitly** so the derived stream
   still carries no local EntityId beyond the immediates the SSMC1 model
   already names.
4. **Family slices.** Data, checked integers, floats, aggregates and maps,
   cells and references, then calls; each lands with vectors, a rejection
   matrix, repeat determinism, and a fuzz lane.
5. **Execution-local values never persist.** Cells and E7 handles are
   rejected as results and never hashed.
6. **E7 waits for its owners, except where an owner already spoke.** Tests,
   effects, adapters, and capability narrowing stay unsupported until their
   runtime owners exist. Contract assertions do not wait: S20-240 already
   accepts `contract_assert` statically under epoch 1 and assigns predicate
   execution to S20-270, so slice E7a takes that assignment rather than
   leaving an owned operation unimplemented. The static typing stays with
   S20-240 at candidate phase 10; the VM owns only the execution.
7. **Staging.** `scripts/check_vm_extended_opcode_profile.py` binds the
   contract, ADR, work-package rows, and the per-slice summary, and fails
   closed if `EXTENDED_V1` appears in the crate before a slice is allowed.

## Consequences

- Programs written against the manifest's full opcode set can be lowered,
  executed, and reported through the endpoint as slices land.
- The restricted profile remains the frozen conformance floor.

# ADR-0039: the extended opcode profile as a second, explicit VM profile

Status: proposed; the S20-260/S20-270 full-profile contract is a draft at
revision 7 with Council review pending; slices E1 through E6 implemented
(2026-09-03), E7 excluded

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

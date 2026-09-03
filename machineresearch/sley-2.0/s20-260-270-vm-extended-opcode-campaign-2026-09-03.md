# S20-260/S20-270 VM Extended Opcode Campaign (2026-09-03)

Status: contract draft revision 1 written by the integrator with every
Council lane unavailable; Ariadne contract review, Nabu architecture
review, and Vulcan surface review queued. Slices E1 through E6 land under
it in order; E7 is excluded until its owners exist.

## Frontier at start

- The restricted profiles lower and execute three Boolean opcodes and all
  five terminators with frozen vectors, cache keys, and observation
  digests; every other opcode fails closed with `VM_LOWER_OPCODE_UNSUPPORTED`.
- The epoch-1 manifest fixes every opcode's operand count, immediate kind,
  result shape, and failure value; S20-210 fixes the closed builtin failure
  codes; S20-250 fixes the value hash.
- SMP1 `execute` reaches the VM from the endpoint, so each family becomes
  demonstrable end to end.

## Design brief

Contract: `docs/spec/VM_EXTENDED_OPCODE_PROFILE_V1.md`, ADR-0039, stage
checker `scripts/check_vm_extended_opcode_profile.py`.

- A second cache profile (`EXTENDED_V1`, bytecode `SLEYBC02` with explicit
  immediates); the restricted bytes and identities are untouched.
- Family slices: E1 data, E2 checked integers, E3 floats, E4 aggregates and
  maps, E5 cells and references, E6 direct calls; E7 excluded.
- No new numeric codes; the seven lowering and six execution codes cover
  every failure; builtin failure values use the S20-210 closed codes.

## Open questions for the reviews

- Whether `vector_new` with zero operands should take its element type
  from the declared result or from an immediate.
- Whether float order predicates should follow IEEE unordered semantics or
  a total order.
- Whether the call-depth ceiling of 256 belongs in the request limits.
- Whether map order by S20-350 canonical key bytes is the right frozen
  rule for runtime maps.

## Records

| Stage | Commit | Tier 1 | Notes |
|---|---|---|---|
| Contract draft revision 1 | pending | pending | ADR-0039, stage checker |

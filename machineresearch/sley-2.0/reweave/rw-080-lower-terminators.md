# RW-080 §1.3 lowerer slice 3: Sley-owned simple terminators

Status: PROVISIONAL C0 CONSTRUCTION. This is an executable Sley lowering slice
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

## Scope and behavior

`simple_terminator_lowerer` lowers the four non-switch native terminator forms
from runtime dense facts:

- return tag 1 with one value register;
- branch tag 2 with one block slot and an arbitrary argument-register vector;
- conditional-branch tag 3 with a condition register and two complete edges;
- trap tag 5 with a frozen trap code and optional payload register.

It returns a normalized typed model containing the exact tag, primary value,
two block slots, both argument vectors, and optional payload. The tests compare
that model field for field with `sley_vm::lower_function` output for all four
forms. The normalized shape is internal construction data and does not change
the frozen `SLEYBC02` encoding.

The image also contains a reusable Sley helper
`validate_register_vector(values, register_count)`. It traverses a runtime
vector with `VectorLen`, `VectorGet`, and a CFG backedge. Branch and conditional
branch invoke it through `CallDirect`, so every edge argument is checked. The
negative corpus places an invalid register at the end of a vector to prove the
algorithm does not inspect only the first argument.

Dense value registers and block slots must be below their supplied inventory
counts. Trap tags must be in the frozen range 1 through 4, and an optional trap
payload must name an allocated register. These failures return
`VM_LOWER_LOCAL_REFERENCE_INVALID` (`26004`). The not-yet-implemented variant
switch tag 4 returns `VM_LOWER_OPCODE_UNSUPPORTED` (`26001`) rather than being
accepted partially. Arithmetic overflow inside the helper returns
`VM_LOWER_RESOURCE_LIMIT` (`26006`). The in-bounds `VectorGet` absence path
traps `InternalInvariant` and is unreachable for a conforming VM.

## Gate

`lower_simple_terminators_match_native_models` covers all four successful
forms and repeats every execution for determinism.
`lower_simple_terminators_reject_invalid_dense_references` covers invalid
return registers, targets, late edge arguments, trap codes, trap payloads, and
the explicitly unsupported variant-switch form. All eleven tests in
`rw080_lower_scaffold` pass; focused Clippy, formatting, and diff checks are
clean; the anti-goal gate remains PASS.

## Explicit remainder

Variant-switch cases, case keys, and case-payload arguments remain to be
lowered. This slice consumes admitted compact facts rather than decoded
checked SSMC closure objects. Block/function inventory traversal, complete
register-type construction, callee-table construction, `SLEYBC02` emission,
`EXEC_PACKAGE_V2` assembly, and `BuildError` remain RW-110 work. RW-080 and R2
statuses stay provisional pending the recorded independent acceptance debt.

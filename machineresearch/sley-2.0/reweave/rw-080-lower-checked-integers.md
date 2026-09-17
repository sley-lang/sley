# RW-080 §1.3 lowerer slice 7: Sley-owned checked-integer family

Status: PROVISIONAL C0 CONSTRUCTION. This is a real Sley lowering algorithm
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

## Scope and behavior

`ordered_scalar_inventory_lowerer` now dispatches the complete checked-integer
opcode family in addition to its prior Boolean and comparison operations:

- binary `IntAddChecked`, `IntSubChecked`, `IntMulChecked`, `IntDivChecked`,
  and `IntRemChecked`;
- unary `IntNegChecked`; and
- binary `IntShlChecked` and `IntShrChecked`.

The runtime Sley loop routes negation through the unary signature and every
other new opcode through the binary signature. It checks all referenced dense
registers, derives one result register from the current frontier, appends the
exact instruction model through `PSH1`, and advances both loop and register
state with checked additions. The loop now covers seventeen scalar opcodes.

## Native parity

`native_single_scalar` builds well-typed native fixtures for every added row:
unsigned `UInt(32)` arithmetic and shifts return the frozen arithmetic
`Result`; signed `SInt(32)` negation returns the corresponding signed result.
The reference passes through `sley_vm::lower_function`, while Sley receives
only the compact checked row after admission. The differential test compares
each opcode tag, ordered operands, dense result register, processed count, and
final frontier. Unary negation also proves the one-parameter frontier remains
dense rather than assuming the binary fixture's starting register.

All thirteen tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

Rows still omit operand/result type objects and rely on prior checker evidence.
Aggregate and container operations, floating operations, direct calls,
variant/result constructors, contracts/effects, cells, value hashes, globals,
function references, immediate-bearing instructions, arbitrary decoded SSMC
closures, SLEYBC02 emission, package assembly, and the build driver remain
RW-110 work. RW-080 and R2 stay provisional pending the recorded independent
acceptance debt.

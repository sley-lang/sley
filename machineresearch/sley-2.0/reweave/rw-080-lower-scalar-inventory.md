# RW-080 §1.3 lowerer slice 6: Sley-owned scalar operation families

Status: PROVISIONAL C0 CONSTRUCTION. This is a real Sley lowering algorithm
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

Successor note: `rw-080-lower-checked-integers.md` extends the same scalar
inventory loop with the complete checked-integer family; this record preserves
the equality/ordering increment and its evidence.

## Scope and behavior

`ordered_scalar_inventory_lowerer` extends the prior ordered Boolean inventory
loop with every immediate-free equality and ordering opcode:

- `Equal` and `NotEqual`;
- `LessThan` and `LessEqual`; and
- `GreaterThan` and `GreaterEqual`.

The same admitted Sley CFG now dispatches nine scalar opcodes in total. Every
new opcode takes two already-lowered operand registers and produces one dense
result register. Sley preserves the frozen decision order—supported opcode,
arity, first local reference, second local reference—then emits the exact
`(opcode, operands, results)` model, appends it through the frozen `PSH1` row,
and advances the register frontier with checked arithmetic.

## Native parity and negative corpus

The native reference fixture constructs well-typed Boolean equality values
and `UInt(32)` ordering values, invokes `sley_vm::lower_function`, and exposes
the resulting `Instruction`. The Sley test compares opcode tags, both operand
registers, the dense result register, processed count, and final frontier for
all six added opcodes. The original two-operation Boolean composition, empty
inventory, determinism, late-row arity/reference failures, and frontier
overflow remain covered. An unsupported `TupleNew` row now pins the opcode
error after `Equal` becomes supported.

All thirteen tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

The rows are checked compact facts and carry no result or operand types; native
type/CFG judgment remains the oracle that constructs valid rows. Aggregate,
container, integer, call, variant/result, contract/effect, cell, hash, global,
function-reference, and immediate-bearing operations; arbitrary decoded SSMC
closures; SLEYBC02 emission; package assembly; and the build driver remain
RW-110 work. RW-080 and R2 stay provisional pending the recorded independent
acceptance debt.

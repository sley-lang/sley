# RW-080 §1.3 lowerer slice 8: Sley-owned unary/binary floating family

Status: PROVISIONAL C0 CONSTRUCTION. This is a real Sley lowering algorithm
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

Successor note: `rw-080-lower-values-cells.md` extends the same runtime loop
with value constructors, local cells, and hashing; this record preserves the
floating-family increment and its evidence.

## Scope and behavior

`ordered_scalar_inventory_lowerer` now accepts `FloatAdd`, `FloatSub`,
`FloatMul`, `FloatDiv`, and `FloatNeg`. The four arithmetic operations follow
the binary path; negation follows the unary path. They share the same complete
runtime-vector traversal, dense-reference checks, typed instruction-model
emission, `PSH1` append, and checked frontier advancement used by the Boolean,
comparison, and integer families. The loop covers twenty-two opcodes.

`FloatFma` remains excluded because the admitted row carries two operand
register fields. Supporting it requires a deliberate row-schema extension and
a third-reference traversal; it is not collapsed into a binary form.

## Native parity

`native_single_scalar` builds a valid `F64` function for each added opcode and
obtains the reference instruction through `sley_vm::lower_function`. The Sley
differential corpus compares every opcode tag, ordered operand list, dense
result register, operation count, and final frontier. Unary `FloatNeg` begins
at a one-register frontier, independently exercising the unary emission path.

All thirteen tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

`FloatFma`, aggregates and containers, direct calls, variant/result
constructors, contracts/effects, cells, value hashes, globals, function
references, immediate-bearing instructions, arbitrary decoded SSMC closures,
SLEYBC02 emission, package assembly, and the build driver remain RW-110 work.
RW-080 and R2 stay provisional pending the recorded independent acceptance
debt.

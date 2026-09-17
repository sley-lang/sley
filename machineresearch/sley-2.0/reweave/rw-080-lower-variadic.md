# RW-080 §1.3 lowerer slice 10: variadic and container operation models

Status: PROVISIONAL C0 CONSTRUCTION. This is a real Sley lowering algorithm
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

Successor note: `rw-080-lower-immediate-free.md` consolidates the scalar and
variadic paths behind one complete operand-vector entry point.

Successor note: `rw-080-lower-map-construction.md` adds `MapNew` with a
Sley-owned even-arity judgment; this record preserves the initial variadic
family and its evidence.

## Scope and behavior

`variadic_operation_lowerer` accepts an opcode tag, the complete runtime
operand-register vector, and the dense result frontier. It supports:

- ternary `FloatFma`;
- variadic `TupleNew` and `VectorNew`;
- `VectorLen`, `VectorGet`, and `VectorSet`; and
- `MapGet`, `MapContains`, `MapInsert`, and `MapRemove`.

Sley dispatches the opcode and enforces the frozen structural arity where one
exists. A separate Sley helper function walks every operand with `VectorGet`
and rejects any register outside the current frontier; the main image invokes
that helper through `CallDirect` and forwards its typed failure. On success it
emits the exact `(opcode, operands, [frontier])` model and advances the frontier
with checked arithmetic. Together with the scalar inventory, the admitted
lowering paths now model forty distinct opcode tags.

## Native parity and negative corpus

`native_variadic_instruction` builds well-typed native fixtures for all ten
opcodes, including heterogeneous tuples, vector index/update results, and
ordered-map access/update types, then invokes `sley_vm::lower_function`. The
Sley differential test compares the complete operand vector, opcode, dense
result, next frontier, and deterministic repeat execution. Negative cases pin
the exact order for unsupported opcode, arity mismatch, a late invalid third
operand, and checked frontier overflow after an empty variadic tuple.

All fifteen tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

The compact input contains checked register facts rather than operand and
result type objects. `MapNew`, projections and named record/variant immediates,
constant/global/function references, direct calls, contracts/effects/adapters/
capabilities, arbitrary decoded SSMC closures, block/function assembly,
SLEYBC02 emission, package assembly, and the build driver remain RW-110 work.
RW-080 and R2 stay provisional pending the recorded independent acceptance
debt.

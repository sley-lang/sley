# RW-080 §1.3 lowerer slice 1: Sley-owned single Boolean operation lowering

Status: PROVISIONAL C0 CONSTRUCTION. This advances the retained scaffold with
one real lowering algorithm under the existing operator development override.
It is not RW-110 completion, C1, a self-hosting claim, or runtime authority.

## Scope and behavior

`single_bool_lowerer(opcode: UInt32, parameter_count: UInt32)` executes as a
Sley program admitted through the frozen v2 boundary. It accepts the frozen
`BoolNot`, `BoolAnd`, and `BoolOr` opcode tags. It derives the dense register
model used by the native lowerer:

- `BoolNot`: operands `[0]`, results `[1]`;
- `BoolAnd` and `BoolOr`: operands `[0, 1]`, results `[2]`.

The result is the typed value
`Result<Tuple<UInt32, Vector<UInt32>, Vector<UInt32>>, UInt32>` containing the
opcode, operand registers, and result registers. A wrong arity returns the
frozen `VM_LOWER_SIGNATURE_MISMATCH` numeric `26002`; any other opcode returns
`VM_LOWER_OPCODE_UNSUPPORTED` numeric `26001`. These are value returns, never
traps. No native adapter import is used.

The selected opcode and arity are runtime inputs supplied after the image is
admitted. The image therefore cannot contain the selected answer. Sley owns
opcode dispatch, arity judgment, vector construction, dense result assignment,
and error selection.

## Reference comparison

The same test builds minimal checked native `FunctionGraph` inputs for all
three supported opcodes and runs `sley_vm::lower_function`. It compares the
Sley tuple with the native instruction's exact opcode tag, operand registers,
and result registers. Every accepted case runs twice and must be deterministic.
Four wrong-arity inputs and three unsupported-opcode inputs pin the exact
frozen errors.

Source and gate: `crates/sley-vm/tests/rw080_lower_scaffold.rs`, tests
`lower_single_boolean_operations_match_native_dense_registers` and
`lower_single_boolean_operations_return_frozen_errors`. The retained four
scaffold tests remain green.

## Explicit remainder

This slice does not parse a closure, allocate registers across multiple
operations or blocks, lower terminators, build a callee table, emit SLEYBC02,
assemble an execution package, or define `BuildError`. The next lowerer slice
is ordered multi-operation register allocation with local-reference refusal;
byte emission follows only after the typed model matches the native reference.
RW-080 and R2 statuses remain unchanged pending the recorded independent
acceptance debt.

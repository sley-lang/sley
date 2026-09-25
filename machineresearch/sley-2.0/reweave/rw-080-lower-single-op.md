# RW-080 §1.3 lowerer slice 1: Sley-owned single Boolean operation lowering

Status: PROVISIONAL C0 CONSTRUCTION. This advances the retained scaffold with
one real lowering algorithm under the existing operator development override.
It is not RW-110 completion, C1, a self-hosting claim, or runtime authority.

## Scope and behavior

`single_bool_lowerer(opcode: UInt32, parameter_count: UInt32,
operand_zero: UInt32, operand_one: UInt32, next_register: UInt32)` executes as
a Sley program admitted through the frozen v2 boundary. It accepts the frozen
`BoolNot`, `BoolAnd`, and `BoolOr` opcode tags. It validates that every used
operand is less than `next_register`, then derives the typed instruction model
used by the native lowerer:

- `BoolNot`: operands `[operand_zero]`, results `[next_register]`;
- `BoolAnd` and `BoolOr`: operands `[operand_zero, operand_one]`, results
  `[next_register]`.

The result is the typed value
`Result<Tuple<UInt32, Vector<UInt32>, Vector<UInt32>>, UInt32>` containing the
opcode, operand registers, and result registers. A wrong arity returns the
frozen `VM_LOWER_SIGNATURE_MISMATCH` numeric `26002`; any other opcode returns
`VM_LOWER_OPCODE_UNSUPPORTED` numeric `26001`; a forward, equal, or otherwise
unallocated register reference returns `VM_LOWER_LOCAL_REFERENCE_INVALID`
numeric `26004`. These are value returns, never traps. No native adapter import
is used.

The selected opcode, arity, operands, and result register are runtime inputs
supplied after the image is admitted. The image therefore cannot contain the
selected answer. Sley owns opcode dispatch, arity judgment, local-reference
validation, vector construction, result assignment, and error selection.

## Reference comparison

The same test builds minimal checked native `FunctionGraph` inputs for all
three supported opcodes and runs `sley_vm::lower_function`. It compares the
Sley tuple with the native instruction's exact opcode tag, operand registers,
and result registers. A second native graph contains `BoolAnd` followed by
`BoolNot` over the first operation's result; two Sley invocations reproduce
its `[0,1] -> [2]` then `[2] -> [3]` register sequence. Every basic accepted
case runs twice and must be deterministic. Four wrong-arity inputs, three
unsupported-opcode inputs, and three invalid local-reference inputs pin the
exact frozen errors.

Source and gate: `crates/sley-vm/tests/rw080_lower_scaffold.rs`, tests
`lower_single_boolean_operations_match_native_dense_registers`,
`lower_single_boolean_operations_return_frozen_errors`, and
`lower_composes_over_a_prior_operation_result`. The retained four scaffold
tests remain green.

## Explicit remainder

This slice does not parse a closure, internally iterate across an arbitrary
operation inventory or block graph, lower terminators, build a callee table,
emit SLEYBC02, assemble an execution package, or define `BuildError`. The next
lowerer slice is an internally iterated ordered operation inventory; byte
emission follows only after the complete typed model matches the native
reference.
RW-080 and R2 statuses remain unchanged pending the recorded independent
acceptance debt.

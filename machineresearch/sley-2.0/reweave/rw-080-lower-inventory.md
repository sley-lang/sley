# RW-080 §1.3 lowerer slice 2: Sley-owned ordered operation inventory

Status: PROVISIONAL C0 CONSTRUCTION. This is a real Sley lowering algorithm
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

Successor note: the implementation was renamed to
`ordered_scalar_inventory_lowerer` and extended by
`rw-080-lower-scalar-inventory.md`; this record preserves the original
Boolean-only slice and its evidence.

## Scope and behavior

`ordered_bool_inventory_lowerer(inventory, first_register)` accepts a runtime
`Vector<Tuple<UInt32, UInt32, UInt32, UInt32>>`. Each row is an opcode tag,
declared arity, first operand register, and second operand register. The second
operand is ignored for unary rows. `first_register` is the dense frontier after
the already checked function parameters.

The Sley program computes the vector length, enters a CFG backedge, obtains
each row with `VectorGet`, projects all four fields with `TupleGet`, dispatches
`BoolNot`, `BoolAnd`, and `BoolOr`, checks arity, and checks every consumed
register against the current frontier. It constructs the exact typed
`(opcode, operand_registers, result_registers)` instruction and appends it to
a Sley-owned model vector through the frozen `PSH1` vector-push primitive. A
successful row then advances both the inventory index and dense-register
frontier with checked integer addition. The second row can therefore consume
the result register derived for the first row. Completion returns
`Result<Tuple<Vector<Instruction>, UInt64, UInt32>, UInt32>` containing the
ordered model, processed operation count, and final register frontier.

All rows arrive after image admission. The image contains neither their count
nor their values. The program handles an empty inventory, two-operation
composition, and an arbitrary runtime length subject to the admitted VM
resource limits.

## Error parity

Each row preserves the native decision order already established by slice 1:
unsupported opcode, signature mismatch, then local-reference validity. A bad
second row proves traversal does not validate only the head. Exact frozen
values are returned for:

- `VM_LOWER_OPCODE_UNSUPPORTED` (`26001`),
- `VM_LOWER_SIGNATURE_MISMATCH` (`26002`),
- `VM_LOWER_LOCAL_REFERENCE_INVALID` (`26004`), and
- `VM_LOWER_RESOURCE_LIMIT` (`26006`) when the register frontier cannot
  advance.

The in-bounds `VectorGet` absence path traps `InternalInvariant`; the loop
condition proves it unreachable for a conforming VM. A `PSH1` capacity
failure returns the same resource-limit value as checked frontier overflow.

## Native comparison and gate

`lower_ordered_boolean_inventory_matches_native_model_and_frontier` compares
the complete ordered Sley instruction model for
`BoolAnd(0,1); BoolNot(2)` with `sley_vm::lower_function`, including opcode,
operand registers, result registers, instruction count two, and final register
frontier four. It also runs the same request twice for determinism and proves
the empty inventory returns an empty model and preserves frontier two.
`lower_ordered_boolean_inventory_checks_every_row_in_order` pins the exact
late-row opcode, arity, and reference failures plus checked frontier overflow.
All nine tests in `rw080_lower_scaffold` pass, focused Clippy is warning-free,
and the anti-goal checker remains PASS.

## Explicit remainder

The inventory rows are admitted compact facts rather than decoded checked
SSMC closure objects. This slice does not yet traverse block or function
inventories, lower terminators, build a callee table, emit `SLEYBC02`,
assemble `EXEC_PACKAGE_V2`, or define a `BuildError` vocabulary.
Those remain RW-110 work. RW-080 and R2 statuses remain provisional pending
the recorded independent acceptance debt.

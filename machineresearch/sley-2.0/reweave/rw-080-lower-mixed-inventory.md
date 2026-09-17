# RW-080 §1.3 lowerer slice 15: ordered mixed-operation inventory

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

`mixed_operation_inventory_lowerer` is the first single Sley entry that walks
an ordered inventory containing both immediate-free and immediate-bearing
target operations. Its admitted transitive closure contains:

- the ordered inventory walker;
- an opcode-family dispatcher;
- the 41-tag immediate-free operation lowerer and register validator;
- the eight-family bootstrap immediate lowerer and register validator.

The dispatcher recognizes all immediate-free opcode tags before inspecting
the supplied immediate. It requires the exact `Immediate::None` tag and zero
payload slots, then calls the immediate-free lowerer and expands its compact
three-field instruction to the common six-field model. All remaining opcodes
go to the immediate-bearing lowerer, which performs its own opcode,
immediate-kind, arity, and reference ordering. This keeps the native
opcode-before-immediate-before-signature precedence across the family split.

The fixture assembler rebases independently built parameter, block,
operation, and constant identities into a disjoint namespace before joining
the functions. Function identities and frozen external bridge identities stay
unchanged. This rebasing is construction plumbing in the seed harness; it is
not a host service visible to the Sley program.

## Native parity and negative corpus

The mixed sequence uses native-derived models for `ConstantRef`, `BoolNot`,
`TupleNew`, `CallDirect`, and B2V1 `AdapterInvoke`. Sley preserves their order,
threads prior-result references across the family boundary, allocates results
5 through 9, and returns frontier 10. Repeated executions are identical.

Negative cases prove that a wrong or nonzero `None` immediate refuses before
an immediate-free arity failure, a late direct-call reference refuses after
earlier immediate-free rows succeed, and an unsupported target opcode remains
`VM_LOWER_OPCODE_UNSUPPORTED`. All twenty-one tests in
`rw080_lower_scaffold` pass; focused Clippy with warnings denied is clean.

## Explicit remainder

Rows still carry already-decoded opcode, register, and bounded immediate
facts. The model does not allocate register types, divide operations into
semantic blocks, attach terminators, build the transitive bytecode function
table, emit SLEYBC02, or assemble an execution package. Those are the next
RW-110 construction layers. The driver remains RW-120 work. RW-080 and R2
stay provisional pending the recorded independent acceptance debt.

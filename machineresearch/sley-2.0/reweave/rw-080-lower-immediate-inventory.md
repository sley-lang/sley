# RW-080 §1.3 lowerer slice 13: ordered immediate inventory

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

Successor note: `rw-080-lower-mixed-inventory.md` adds opcode-first dispatch
between this immediate-bearing path and the unified immediate-free path.

## Scope and behavior

`ordered_immediate_inventory_lowerer` composes the prior
`bootstrap_immediate_lowerer` over an execution-time vector. Each row carries
an opcode, complete ordered operand-register vector, immediate tag, and two
bounded immediate payload fields. The Sley caller:

1. derives the runtime inventory length and empty output model;
2. walks rows in order through a CFG backedge;
3. calls the Sley per-operation lowerer with the current dense frontier;
4. forwards its first typed rejection unchanged;
5. appends each accepted instruction through the frozen PSH1 value mechanic;
6. advances the row index with checked arithmetic and threads the frontier
   returned by the callee.

The success value contains the complete ordered instruction vector, processed
row count, and final register frontier. Empty inventories preserve the input
frontier. The function, its per-operation callee, and the callee's register
vector validator are all present in one admitted transitive Sley closure.

## Native parity and negative corpus

The native reference image supplies an eight-instruction dense sequence:
constant reference, tuple projection, record construction and projection,
variant construction and projection, direct call, and B2V1 adapter invocation.
The Sley inventory result matches every ordered native instruction field and
advances registers 5 through 12 to frontier 13. Repeated executions and the
empty-inventory case are deterministic.

A wrong immediate on row six returns `VM_LOWER_IMMEDIATE_MISMATCH`; an invalid
reference on row seven returns `VM_LOWER_LOCAL_REFERENCE_INVALID`; a one-row
constant inventory beginning at `UInt32::MAX` returns
`VM_LOWER_RESOURCE_LIMIT`. These cases prove successful earlier rows cannot
hide or reorder a later rejection. All nineteen tests in
`rw080_lower_scaffold` pass; focused Clippy with warnings denied is clean.

## Explicit remainder

This slice composes the immediate-bearing family only. Scalar and variadic
models still have separate inventory entry points, and a complete function
assembler must unify all operation families with register types, blocks,
terminators, and the callee table. Full identity payloads, SLEYBC02 emission,
package assembly, and the driver remain RW-110/RW-120 work. RW-080 and R2 stay
provisional pending the recorded independent acceptance debt.

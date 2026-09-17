# RW-080 §1.3 lowerer slice 12: bootstrap immediates

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

`bootstrap_immediate_lowerer` covers every immediate-bearing operation in the
frozen bootstrap opcode table: `ConstantRef`, `TupleGet`, `RecordNew`,
`RecordGet`, `VariantNew`, `VariantGet`, and `CallDirect`. It also covers the
separately admitted `AdapterInvoke` bridge operation. The existing scalar and
variadic slices cover every immediate-free bootstrap opcode.

For each input row, Sley owns and executes this exact decision order:

1. recognize the opcode;
2. require its frozen immediate kind;
3. enforce the structural operand arity, including zero-or-one for
   `VariantNew` and arbitrary vectors for record construction and calls;
4. walk every operand and require a prior dense register;
5. derive the one-result register and advance the frontier with checked
   arithmetic.

The result is a compact typed instruction tuple containing opcode, ordered
operands, result register, immediate tag, and two bounded `UInt(64)` identity
fields. Entity and member fixtures project the final eight identity bytes;
the native oracle retains the full identities. This projection is a testable
construction boundary and is not SLEYBC02 encoding.

## Native parity and negative corpus

One native lowering image exercises all eight families in a single function.
It includes a constant, tuple projection, named record construction and field
projection, named variant construction and projection, a direct call, and the
frozen B2V1 adapter import. The eight native instructions form one dense
register sequence with prior-result references. The Sley result matches every
native opcode, ordered operand vector, result register, immediate tag,
projected payload, and next frontier. Repeated executions are identical.

The negative corpus pins native error ordering and numerics for an unsupported
opcode, wrong immediate kind, fixed-arity and variant-arity violations, a
late invalid register reference, and frontier overflow. All seventeen tests in
`rw080_lower_scaffold` pass; focused Clippy with warnings denied is clean.

## Explicit remainder

The compact model does not decode full SSMC type or immediate payloads, judge
their semantic targets, assemble functions and blocks into complete bytecode,
emit exact SLEYBC02 bytes, or build an execution package. Full closure
hydration, checked-function integration, cache metadata, package assembly, and
the build driver remain RW-110/RW-120 work. RW-080 and R2 stay provisional
pending the recorded independent acceptance debt.

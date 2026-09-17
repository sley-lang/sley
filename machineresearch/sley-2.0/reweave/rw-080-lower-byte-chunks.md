# RW-080 §1.3 lowerer slice 23: exact byte-chunk append

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley encoding
primitive under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

A private Sley function now appends an arbitrary exact `Bytes` chunk to a
nonempty `Vector<UInt8>` accumulator. B2V1 exposes the chunk, a CFG backedge
walks every octet by checked `VectorGet`, PSH1 appends it, and checked UInt64
advance returns to the loop guard. V2B1 is used only by the focused public test
entry to close the final vector.

This is the raw-copy path required for function identities, canonical type
encodings, and exact immediate encodings already retained by the lowerer.
The host does not concatenate or interpret any payload. An impossible missing
element after the length guard traps as an internal invariant; conversion,
push, and index-capacity failures return the frozen
`VM_LOWER_RESOURCE_LIMIT` code.

## Native parity and boundary corpus

The admitted two-function closure preserves empty prefix/chunk cases, the
SLEYBC02 header, mixed high/low octets, and a complete `0..=255` chunk after a
nonempty prefix. Each result is compared byte for byte with native slice
concatenation.

All thirty-one tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

The lowerer now has both exact chunk append and fixed-width big-endian numeric
append primitives. The next layer must compose them over the complete function
model, beginning with identity, parameter registers, canonical register/result
types, entry slot, and block count. Ordered blocks, instructions, terminators,
SLEYBC02 header/callee framing, and execution-package assembly remain. The
driver remains RW-120 work. RW-080 and R2 stay provisional pending the recorded
independent acceptance debt.

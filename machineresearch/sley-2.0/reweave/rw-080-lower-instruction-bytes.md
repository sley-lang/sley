# RW-080 §1.3 lowerer slice 25: exact instruction bytes

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley encoding
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

One lossless lowered instruction model now serializes entirely in Sley using
the exact extended SLEYBC02 layout:

1. UInt32 opcode;
2. UInt64 operand count plus every UInt32 operand register;
3. UInt64 result count plus every UInt32 result register; and
4. the complete canonical immediate byte chunk.

The encoder composes the fixed-width UInt32/UInt64 appenders, the checked
register-vector walker, and exact byte-chunk append. Compact immediate fields
remain available for semantic validation but are never substituted for the
lossless canonical payload. Helper failures propagate their frozen UInt32
lowering code unchanged.

The focused closure exposes the Sley-owned octet vector directly. It imports
only B2V1 and the single UInt8 PSH1 schema; no V2B1 conversion is needed to
judge its contents.

## Native parity corpus

All eight native bootstrap-immediate instructions are encoded after a unique
nonempty prefix and compared byte for byte with the native extended layout.
The corpus covers `None`, entity, index, field, variant, function, and adapter
entity immediates, along with empty/nonempty operand vectors and dense result
vectors. Prefix preservation also proves that the function is a true appender.

All thirty-three tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

Instruction bytes are not yet traversed from the ordered instruction map.
The next block layer must emit the instruction count and walk ordinal keys,
then serialize the terminator and reachability. Function-body block traversal,
SLEYBC02 image header/callee framing, and execution-package assembly remain.
The driver remains RW-120 work. RW-080 and R2 stay provisional pending the
recorded independent acceptance debt.

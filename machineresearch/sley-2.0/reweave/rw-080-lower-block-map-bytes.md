# RW-080 §1.3 lowerer slice 31: ordered block-map bytes

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley encoding
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

The serializer now walks every function block in canonical slot order by
aligning two Sley-owned structures:

- the validated semantic block-fact vector supplies the UInt64 block count,
  each row's raw instruction inventory, and therefore its instruction count;
- the lowered ordered block map supplies the lossless complete block model at
  the corresponding UInt32 slot.

The walker keeps checked UInt64 fact index and UInt32 slot counters, requires
both the fact and map entry at each position, derives the instruction count by
`VectorLen`, and calls the complete block appender. Missing aligned rows are
internal invariants because both structures were created and validated by the
composed lowerer. Arithmetic exhaustion returns the frozen resource-limit
code, and block-encoder errors propagate unchanged.

This entry extends the complete block closure instead of rebuilding its helper
inventory. It therefore retains exactly B2V1 and the single UInt8 PSH1 schema.

## Native parity corpus

The walker encodes all three blocks of the native Option-switch function after
a nonempty prefix. The result matches the concatenation of the native block
records byte for byte, proving aligned order and preservation across switch,
branch, and return blocks.

All thirty-nine tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

The existing function-header encoder and this block-map encoder must now be
composed behind one complete function-body entry and closed to `Bytes` through
V2B1. SLEYBC02 image header/callee framing and execution-package assembly
remain. The driver remains RW-120 work. RW-080 and R2 stay provisional pending
the recorded independent acceptance debt.

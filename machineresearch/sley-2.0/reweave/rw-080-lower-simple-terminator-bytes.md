# RW-080 §1.3 lowerer slice 27: simple terminator bytes

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley encoding
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

The four non-switch terminator families now serialize from their validated
normalized Sley models:

- return: tag plus result register;
- branch: tag plus target edge;
- conditional branch: tag, condition register, true edge, then false edge;
- trap: tag, trap code, and the canonical option tag plus optional payload
  register.

A private target-edge helper emits the UInt32 target followed by the
length-prefixed UInt32 argument vector. The terminator encoder emits the kind
first, dispatches within Sley, and composes fixed-width and vector helpers for
the remaining payload. Helper errors propagate unchanged. The eventual caller
is the complete terminator encoder, whose model has already passed semantic
kind and reference validation.

The focused closure needs only the single UInt8 PSH1 adapter schema. It returns
the Sley-owned octet vector directly for inspection.

## Native parity corpus

Return, branch, conditional, and trap fixtures from the native lowerer are
each encoded after a distinct nonempty prefix and compared byte for byte with
the native terminator layout. The cases cover one and two target edges,
nonempty argument vectors, and a present trap payload.

All thirty-five tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

Built-in variant-switch terminators remain. Their case and switch-argument
vectors must be emitted before the simple and switch paths can be unified into
the complete block serializer. Function-body block traversal, SLEYBC02 image
header/callee framing, and execution-package assembly remain. The driver
remains RW-120 work. RW-080 and R2 stay provisional pending the recorded
independent acceptance debt.

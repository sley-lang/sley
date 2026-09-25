# RW-080 §1.3 lowerer slice 30: complete block bytes

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley encoding
composition under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

A complete lowered block model now serializes in the exact SLEYBC02 order:

1. UInt32 block slot;
2. UInt64 parameter-register count plus each UInt32 register;
3. UInt64 instruction count plus every instruction at dense ordinal map keys;
4. the complete terminator bytes; and
5. UInt32 reachability tag.

The block appender composes the existing fixed-width, register-vector,
instruction-map, and complete-terminator functions. The instruction count is
passed separately from the already validated semantic fact inventory because
the frozen map profile has no length operation. The map itself remains the
lossless lowered authority for opcode, registers, and exact immediate bytes.
Every helper error propagates unchanged.

The closure imports B2V1 for immediate byte chunks and the single UInt8 PSH1
schema for all emitted octets. It exposes the accumulator vector directly.

## Native parity corpus

All three blocks of the native Option-switch function are encoded after unique
nonempty prefixes and compared byte for byte with the native block layout.
Together they cover a built-in switch, branch, return, empty and nonempty block
parameter vectors, multiple instruction counts, exact immediates, and required
reachability.

All thirty-eight tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

The complete function encoder must now walk the aligned semantic block facts
and ordered block map, derive each instruction count, and append these block
records after the existing function header. SLEYBC02 image header/callee
framing and execution-package assembly remain. The driver remains RW-120 work.
RW-080 and R2 stay provisional pending the recorded independent acceptance
debt.

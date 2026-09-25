# RW-080 §1.3 lowerer slice 22: fixed-width byte emission

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley encoding
primitive under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

The lowerer construction now includes a Sley-owned primitive that preserves
an arbitrary byte prefix and appends one `UInt32` plus one `UInt64` in the
canonical big-endian representation used by SLEYBC02. It uses the frozen
bridges only for representation boundaries: B2V1 opens the prefix, the one
permitted `Vector<UInt8> × UInt8 -> Vector<UInt8>` PSH1 row appends octets,
and V2B1 closes the result.

The frozen bootstrap profile has no integer-cast opcode. Private Sley
functions therefore narrow a proven `0..=255` quotient by reconstructing its
eight bits with comparisons and checked subtraction/addition. Separate
UInt32 and UInt64 appenders divide and take remainders at fixed powers of 256,
call that ladder, and emit most-significant octets first. Impossible checked
arithmetic failures trap as internal invariants. Bridge capacity failures
return the frozen `VM_LOWER_RESOURCE_LIMIT` code.

This is the same semantic workaround established by the landed Sley SCB
codec's `encode_uvar` implementation. It introduces no native integer
conversion or byte-writing service.

## Native parity and negative boundaries

The focused test admits and executes the complete five-function closure under
bootstrap profile V2. It compares Sley output with Rust `to_be_bytes()` for
zero, mixed high/low octets, the SLEYBC02 header/version prefix shape, and the
maximum UInt32/UInt64 values. It also proves that the admitted import inventory
contains exactly the three required bridge identities.

All thirty tests in `rw080_lower_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

This slice supplies the numeric and raw-prefix primitive; it does not yet
serialize the complete lowered-function model. The next construction layer
must walk function parameters, canonical type byte chunks, ordered block and
instruction maps, and terminator payloads through these appenders. SLEYBC02
header/callee framing and execution-package assembly remain. The driver
remains RW-120 work. RW-080 and R2 stay provisional pending the recorded
independent acceptance debt.

# RW-080 §1.3 lowerer slice 29: complete terminator bytes

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley encoding
composition under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

The normalized complete terminator model now has one byte-encoding entry.
Sley reads its outer kind and dispatches kind `4` to the built-in switch
encoder; return, branch, conditional, and trap models go to the simple
terminator encoder. Both paths preserve the incoming octet accumulator and
return the same frozen `Result<Vector<UInt8>, UInt32>` shape.

This dispatcher does not duplicate serialization rules. The simple model owns
its kind field and the switch encoder owns the constant switch kind, so exactly
one UInt32 terminator discriminator is emitted on either path. The complete
model is produced by the semantic lowerer, which has already validated the
kind, references, targets, and switch inventory.

The complete closure imports only the single UInt8 PSH1 schema and exposes the
Sley-owned octet vector directly.

## Native parity corpus

All five native terminator families are passed through the complete model and
encoded after unique nonempty prefixes. Every result matches the native
terminator layout byte for byte, including both conditional edges, trap option
payload, and nested switch case/argument inventories.

All thirty-seven tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

Complete terminator bytes must now be composed with block slot, parameter
registers, instruction inventory, and reachability. Then the complete function
encoder must walk its ordered block map. SLEYBC02 image header/callee framing
and execution-package assembly remain. The driver remains RW-120 work. RW-080
and R2 stay provisional pending the recorded independent acceptance debt.

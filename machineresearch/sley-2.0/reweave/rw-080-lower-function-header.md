# RW-080 §1.3 lowerer slice 24: exact function-body header

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering and
encoding composition under the operator development override. It is not
RW-110 completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

The complete semantic function lowerer now composes with Sley-owned byte
appenders to emit the exact SLEYBC02 function-body prefix through the block
count. The emitted fields are, in canonical order:

1. raw 32-byte function identity;
2. UInt64 parameter-register count plus every UInt32 register;
3. UInt64 register-type count plus every already canonical type byte chunk;
4. the canonical result-type byte chunk;
5. UInt32 entry-block slot; and
6. UInt64 block count.

Two concrete vector walkers share one construction routine. Each emits its
own UInt64 length, performs checked indexed traversal, and delegates items to
the existing UInt32 or exact-byte appender. The wrapper first calls the
complete lowerer, so identity width, dense registers, block order, instruction
semantics, entry range, and register-type cardinality are all accepted before
any successful byte result. Frozen lowering errors pass through unchanged.

The closure imports exactly B2V1, the single UInt8 PSH1 schema, and V2B1.
There is no native serialization or integer-cast service.

## Native parity and negative corpus

The admitted closure lowers the native three-block Option fixture and compares
its complete emitted prefix byte for byte with the native Encoder layout. The
fixture exercises multiple function parameters, eight heterogeneous register
types, a canonical result type, nonempty blocks, and entry slot zero. A
31-byte function identity still returns `VM_LOWER_LOCAL_REFERENCE_INVALID`,
showing the encoder wrapper preserves semantic failure behavior.

All thirty-two tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

The emitted prefix stops after the block count. The next layers must traverse
the ordered block map and serialize each block slot, parameter registers,
instruction map, exact immediates, terminator, and reachability. SLEYBC02
image header/callee framing and execution-package assembly remain. The driver
remains RW-120 work. RW-080 and R2 stay provisional pending the recorded
independent acceptance debt.

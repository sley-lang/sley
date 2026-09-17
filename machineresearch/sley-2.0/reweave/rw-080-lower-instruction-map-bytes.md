# RW-080 §1.3 lowerer slice 26: ordered instruction-map bytes

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley encoding
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

The serializer now emits a block's UInt64 instruction count and walks every
dense ordinal key `0..count` in the Sley-owned ordered instruction map. Each
successful `MapGet` feeds the exact instruction encoder from slice 25, and a
checked UInt64 increment returns to the loop guard. The count is supplied from
the already validated semantic instruction inventory because the frozen
bootstrap map profile deliberately has no map-length opcode.

A missing key below the validated count traps as an internal invariant. This
is appropriate at this boundary: the instruction map is constructed only by
the lowerer's sequential `MapInsert` loop, not accepted as untrusted external
state. Encoder failures propagate their frozen lowering code, and arithmetic
resource failure returns `VM_LOWER_RESOURCE_LIMIT`.

## Native parity and boundaries

The admitted closure is tested with empty, one-row, and complete eight-row
inventories after distinct prefixes. Every result is compared with the native
UInt64 count followed by each instruction's extended SLEYBC02 bytes in ordinal
order. The complete case covers all bootstrap immediate forms.

The growing composed graph made the prior one-million semantic-value-unit test
budget smaller than the admitted image's initial live-value charge. The shared
test budget is now ten million units, still one tenth of the existing
100-million-unit bootstrap-closure budget. Instruction, fuel, and output
budgets are unchanged; this is a test-fixture admission limit, not a product
resource-policy change.

All thirty-four tests in `rw080_lower_scaffold` pass; focused Clippy with
warnings denied is clean.

## Explicit remainder

The instruction inventory is not yet embedded in a complete block record.
The next layer must emit block slot and parameters, call this map walker,
serialize the terminator and reachability, then traverse all function blocks.
SLEYBC02 image header/callee framing and execution-package assembly remain.
The driver remains RW-120 work. RW-080 and R2 stay provisional pending the
recorded independent acceptance debt.

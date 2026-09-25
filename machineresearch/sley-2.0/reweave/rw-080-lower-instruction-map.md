# RW-080 §1.3 lowerer slice 21: canonical instruction map

Status: PROVISIONAL C0 CONSTRUCTION. This is a real bounded Sley lowering
algorithm under the operator development override. It is not RW-110
completion, C1, self-hosting evidence, or runtime authority.

## Scope and behavior

The ordered operation walker now accumulates lowered instructions in an
`OrderedMap<UInt64, InstructionModel>` keyed by the exact runtime row ordinal.
It begins with the infallible empty `MapNew` result, inserts each accepted
instruction at the current checked loop index, and returns the map with the
same instruction count and dense-register frontier as before. Sequential
ordinal keys make canonical map order identical to semantic operation order.

This representation change resolves a frozen bootstrap-ABI constraint. V2
admission permits one monomorphized PSH1 import row per closure identity. The
former instruction-vector accumulator used PSH1 with `InstructionModel` and
would have required an illegal second PSH1 schema for byte emission. Map
insertion is a Sley-owned value operation already in the frozen profile, so
the complete function-lowering closure now carries no PSH1 row. The one future
row can therefore be `Vector<UInt8> × UInt8 → Vector<UInt8>` for exact image
assembly.

No validation order changes. Per-operation lowering finishes before map
insertion, checked index advance remains after acceptance, and all opcode,
immediate, arity, reference, and frontier failures retain their codes and
precedence. Empty-map construction failure is unreachable and traps as an
internal invariant.

## Native parity and negative corpus

All instruction assertions now compare ordinal keys and full instruction
values against native semantic order. Simple blocks, complete blocks, and the
three-block function retain exact native parity through the map-backed model.
The function test also inspects the assembled closure and proves it contains
no PSH1 adapter row. All twenty-nine tests in `rw080_lower_scaffold` pass;
focused Clippy with warnings denied is clean.

## Explicit remainder

The map is an internal construction representation; SLEYBC02 still requires
length-prefixed sequential instruction bytes. The next encoder must traverse
keys `0..count`, emit the exact instruction layout, serialize terminators and
blocks, and use the single PSH1 octet row before V2B1 conversion. Callee-table,
image-header, and execution-package work remain. The driver remains RW-120
work. RW-080 and R2 stay provisional pending the recorded independent
acceptance debt.

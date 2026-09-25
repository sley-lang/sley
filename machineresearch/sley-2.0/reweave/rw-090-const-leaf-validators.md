# RW-090 constant leaf validators

Date: 2026-09-18

Status: implemented primitive slice; RW-090 remains open

## Result

The Sley codec now validates every primitive `ConstData` payload family that
the recursive `ConstValue` decoder will need, each as a standalone unit-result
function over one canonical payload:

- strict booleans (`0` or `1`, one byte);
- canonical 128-bit unsigned varints, which also cover ZigZag signed values,
  with the native strict-reader precedence: integer overflow (a twentieth
  byte with payload, a nineteenth byte above `3`, or a twentieth
  continuation), then non-minimal encoding, then trailing bytes;
- canonical `f32` and `f64` bit patterns (exact width, negative zero refused,
  every NaN other than the canonical quiet NaN refused);
- length-prefixed bytes bounded by the 16 MiB payload cap; and
- length-prefixed UTF-8 text, validated by a byte-tier state machine that
  refuses stray continuations, overlong forms (`C0`/`C1`, `E0 80..9F`,
  `F0 80..8F`), surrogates (`ED A0..BF`), code points above `U+10FFFF`
  (`F4 90..`, `F5..FF` leads), truncated sequences, and bad continuations.

Each validator converts the payload through the frozen `B2V1` bridge and
walks it with checked 64-bit indices; no float opcode, no wide integer type,
and no native text service is used.

## Validation

Eighty-four cases (thirty accepted, fifty-four refused) are checked twice:
against the refusal code the case names, and against the native
`ScbValueCursor` reader running the same read followed by `check_finished`,
so every Sley verdict is pinned to native precedence rather than to the
author's reading of it. Standalone images admit each validator alone under
the bootstrap gate (only the reached function and the referenced bridge row
ride the admission).

```text
cargo test -p sley-vm --test rw120_toolchain_integration const_leaf_validators
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
```

## Next

The shallow `ConstValue` child projector composes these with the existing
fixed-identity, bounded-enum, exact-varint, and recursive `TypeExpr`
validators, and the existing worklist driver then closes the recursion for
Constant (kind 9) and CapabilityRequirement (kind 12).

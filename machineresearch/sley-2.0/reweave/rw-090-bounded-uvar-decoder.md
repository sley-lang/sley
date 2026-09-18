# RW-090 bounded uvar decoder

Date: 2026-09-18

Status: implemented scalar slice; RW-090 remains open

## Result

The Sley codec now has an exact canonical uvar wrapper and a runtime bounded
uvar decoder. The exact wrapper invokes the shared Sley uvar decoder at offset
zero, compares its returned offset with the runtime byte length, and rejects
trailing bytes. The bounded entry fixes the width to 32 bits and enforces
caller-supplied inclusive `UInt64` minimum and maximum values.

Malformed inputs preserve the underlying `SCB_LENGTH_OVERFLOW`,
`SCB_VARINT_NON_MINIMAL`, and `SCB_TRAILING_BYTES` decisions. Values outside
the declared closed enum range return `SCB_UNION_INVALID`.

The standalone reachable closure contains three functions: bounded uvar,
exact uvar, and canonical uvar. It contains 336 parameters, 63 blocks, 133
operations, and 34 constants. Its image is 17,820 bytes. The approved package
digest is
`9a75acf11cf9f12ed48f02414bd592e1b8959305b95dbc139761ee8881597219`.

## Validation

```text
cargo test -p sley-vm --test rw120_toolchain_integration bounded_uvar_decoder -- --nocapture
```

The positive corpus covers every visibility tag, 1 through 4. Negative cases
cover empty input, non-minimal input, trailing bytes, zero, and five. The
bounded decoder is composed into the Function schema image for visibility.

# RW-090 Parameter schema projection

Date: 2026-09-18

Status: implemented bootstrap slice; RW-090 remains open

## Result

The Sley codec now validates an arbitrary canonical `ParameterBody` and
projects its four ordered field payloads. The executable entry function owns
these structural decisions:

- the outer union tag is exactly entity kind 6;
- record fields 1 through 4 are present and no unknown field remains;
- `owner` is exactly 32 bytes;
- `role` is an exact canonical uvar in the closed range 1 through 2;
- `ordinal` is an exact canonical 32-bit uvar; and
- `value_type` is a complete depth-bounded recursive `TypeExpr`.

The result type is
`Result<Tuple<Bytes, Bytes, Bytes, Bytes>, Bytes>`. Successful tuple members
preserve the exact canonical bytes from the source body. Dense or owner-local
ordinal rules remain semantic checker work rather than codec structure.

## Executable surface

The image contains 15 reachable functions, 1,318 parameters, 319 blocks, 582
operations, and 185 constants. Its encoded image is 77,884 bytes. The approved
package digest is
`69ba030358db0833224d149e2ce0995b8deacccfb243b8fad8cf1a2856731ed9`
under the declared codec-profile execution limits.

## Validation

The positive fixture uses the maximum `u32` ordinal, the `Block` role, and a
nested `Option<Tuple<Bool, Named<Bytes>>>` value type. Negative cases cover a
wrong entity kind, missing and unknown fields, a short owner identity, an
out-of-range role, a 33-bit ordinal, and an unknown nested `TypeExpr` tag.

```text
cargo test -p sley-vm --test rw120_toolchain_integration parameter_schema_decoder -- --nocapture
```

This slice is not yet wired into the all-kind program dispatch. Kinds 4 and
7 through 15 still require arbitrary schema decoders, and RW-090 remains open.

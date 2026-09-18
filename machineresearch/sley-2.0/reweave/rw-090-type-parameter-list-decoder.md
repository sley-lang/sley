# RW-090 type-parameter list decoder

Date: 2026-09-18

Status: implemented structural slice; RW-090 remains open

## Result

The Sley codec now validates `List<TypeParameterDef>` bytes. It composes the
generic list and record decoders with the exact-uvar wrapper. Every element
must be a canonical record containing exactly field 1, whose payload must be
one exact canonical 32-bit uvar. Unknown fields take precedence over a missing
ordinal, matching the native record decoder.

The structural decoder deliberately accepts any `u32` ordinal. Dense,
zero-based declaration order is a semantic rule owned by the checker rather
than the canonical byte codec.

The standalone reachable closure contains five functions and has 803
parameters, 134 blocks, 241 operations, and 64 constants. Its image is 36,602
bytes. The approved package digest is
`610c4a95492a5bd0cd7d0d9802db2dbf853395869490917c7648817b73c6ff1e`.

## Function integration

Function field 1 now invokes this validator. The positive fixture contains a
real ordinal-zero declaration. Negative cases cover a missing ordinal and an
unknown record field. After recursive TypeExpr composition, the Function
schema image contains 15 reachable functions, 1,427 parameters, 334 blocks,
601 operations, and 191 constants. Its image is 82,222 bytes and its package
digest is
`b5ad5270511b9ee6f2e11a7247cb24564553a982f84b7988f3b2d8ae4b61963a`
under the codec-profile execution limits.

## Validation

```text
cargo test -p sley-vm --test rw120_toolchain_integration type_parameter_list_decoder -- --nocapture
cargo test -p sley-vm --test rw120_toolchain_integration function_schema_decoder -- --nocapture
```

The complete recursive `TypeExpr` language and semantic dense-ordinal check
remain open.

# RW-090 TypeExpr leaf decoder

Date: 2026-09-18

Status: implemented leaf slice; RW-090 remains open

## Result

The Sley codec now validates every non-recursive `TypeExpr` family from its
canonical union bytes. The decoder composes the generic union, fixed-32,
exact-uvar, and bounded-uvar decoders and accepts these tags:

- `Unit`, `Bool`, `F32`, `F64`, `Bytes`, and `Text` with empty payloads;
- `SInt` and `UInt` with exact canonical 16-bit uvar payloads;
- `AdapterHandle` and `CapabilityToken` with exact 32-byte payloads;
- `TypeParameter` with an exact canonical 32-bit uvar payload; and
- `BuiltinFailure` with a canonical tag in the closed range 1 through 5.

Malformed leaf payloads retain their canonical `SCB_*` failures. Unknown
union tags return `SCB_UNION_INVALID`. Recursive tags 9 through 15 and 18
return `SSMC_RESERVED_FIELD_PRESENT` until the recursive validator replaces
this bootstrap boundary.

The standalone reachable closure contains six functions and has 516
parameters, 122 blocks, 236 operations, and 75 constants. Its image is 30,214
bytes. The approved package digest is
`7b627cdd034a922bee52723eefc96a7c3943f9ef2b78bf801632aab1f1b46ced`.

## Function integration

The Function schema decoder invokes the leaf validator on field 3 after its
collection, identity, and visibility checks. Its composite image contains 10
reachable functions, 1,124 parameters, 224 blocks, 406 operations, and 125
constants. The image is 57,396 bytes and has package digest
`fb18f26295a18c6022a310b409ee7a39940d7549bad8a4644654e59def82d046`
under the codec-profile execution limits.

The Function corpus accepts its native `Bool` result type, rejects a nonempty
`Unit` payload, and preserves the explicit recursive scope refusal for an
`Option<Bool>` result.

## Validation

```text
cargo test -p sley-vm --test rw120_toolchain_integration type_expr_leaf_decoder -- --nocapture
cargo test -p sley-vm --test rw120_toolchain_integration function_schema_decoder -- --nocapture
```

The complete recursive `TypeExpr` language and Function type-parameter
records remain open.

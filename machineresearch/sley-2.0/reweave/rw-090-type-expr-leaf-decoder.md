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
union tags return `SCB_UNION_INVALID`. This leaf component still refuses
recursive tags 9 through 15 and 18 when invoked directly; the recursive
decoder dispatches those tags through its child projector and uses this
component only for leaf nodes.

The standalone reachable closure contains six functions and has 516
parameters, 122 blocks, 236 operations, and 75 constants. Its image is 30,214
bytes. The approved package digest is
`7b627cdd034a922bee52723eefc96a7c3943f9ef2b78bf801632aab1f1b46ced`.

## Function integration

The Function schema decoder invokes the complete recursive validator on field
3 after its collection, identity, visibility, and type-parameter structural
checks. Its current composite image contains 15 reachable functions, 1,427
parameters, 334 blocks, 601 operations, and 191 constants. The image is 82,222
bytes and has package digest
`b5ad5270511b9ee6f2e11a7247cb24564553a982f84b7988f3b2d8ae4b61963a`
under the codec-profile execution limits.

The Function corpus accepts its native `Bool` result type, rejects a nonempty
`Unit` payload, and preserves the explicit recursive scope refusal for an
`Option<Bool>` result.

## Validation

```text
cargo test -p sley-vm --test rw120_toolchain_integration type_expr_leaf_decoder -- --nocapture
cargo test -p sley-vm --test rw120_toolchain_integration function_schema_decoder -- --nocapture
```

The standalone leaf boundary remains useful for focused scalar parity. The
complete recursive language is covered by the recursive decoder; broader
all-entity codec work remains open.

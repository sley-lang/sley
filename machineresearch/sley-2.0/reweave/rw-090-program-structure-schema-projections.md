# RW-090 program-structure schema projections

Date: 2026-09-18

Status: implemented bootstrap slice; RW-090 remains open

## Result

The Sley codec now validates the two program-structure entity schemas, both
composed entirely from existing validators over a shared closure of value
references and target edges:

- `Block` (kind 7): an exact function identity, parameter and operation
  identity sequences, the closed five-arm `Terminator` union, and
  reachability `1..=2`; and
- `Operation` (kind 8): an exact block identity, 32-bit ordinal and opcode,
  an ordered `ValueRef` operand list, a recursively validated `TypeExpr`
  result-type list, and the closed seven-arm `Immediate` union.

The terminator closure validates `Return`, `Branch`, `CondBranch`,
`VariantSwitch` (case keys as member identities or built-in cases `1..=4`,
switch edges with `Value` or empty `CasePayload` arguments), and `Trap` (codes
`1..=4` with an optional payload reference). The immediate closure validates
`None` (empty), `Entity`, 32-bit `Index`, `Field`, `Variant`, `Observation`,
and `Function` with recursive type arguments. `ValueRef` covers parameter
references and operation results with 32-bit result indices.

Two small wrappers let a bare closed enum or exact-width integer stand alone
as a union arm, and the closed-union builder now accepts arms of any result
type. Composed images prune functions the entry does not reach (and the
inventories they own) before admission, so one shared closure can serve
schemas that use different subsets of it.

## Executable surfaces

| Schema | Functions | Parameters | Blocks | Operations | Constants | Image bytes | Package digest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Block | 32 | 1,226 | 366 | 607 | 159 | 81,298 | `20e2cc9489f3f80ed2c3dcd9d11043f82f9bbb7071833e4a23ae7d04cdf0f7d2` |
| Operation | 24 | 1,439 | 391 | 681 | 202 | 90,990 | `74429c9fceb76375f9a05fbcdb8cb54c912f78e4c62c0741670a81d6e0624d21` |

Both are approved under the declared codec-profile execution limits.

## Validation

Positive fixtures cover all five terminator families (including a switch
with both case-key kinds and both argument kinds, and traps with and without
a payload) and all seven immediate families, with maximum 32-bit ordinals and
result indices. Negative coverage includes wrong entity kinds, missing and
unknown fields, short identities at several depths, an unknown terminator or
immediate tag, an unknown value-reference tag, a 33-bit result index, a
branch edge missing its arguments, a conditional edge carrying a malformed
operation result, an out-of-range built-in case, an unknown switch-argument
tag, a non-empty case payload, an out-of-range trap code, an invalid trap
payload option, invalid reachability, a 33-bit ordinal, a non-minimal opcode,
a malformed result type, a non-empty `None` immediate, a 33-bit index, a
short variant member, and a malformed function type argument.

```text
cargo test -p sley-vm --test rw120_toolchain_integration block_schema_decoder
cargo test -p sley-vm --test rw120_toolchain_integration operation_schema_decoder
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
```

These decoders are not yet wired into the all-kind program dispatch. Only
kind 14 (TestCase) still requires an arbitrary schema decoder before RW-090
can close its per-kind coverage.

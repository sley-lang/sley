# RW-090 TypeDef schema projection

Date: 2026-09-18

Status: implemented bootstrap slice; RW-090 remains open

## Result

The Sley codec now validates an arbitrary canonical `TypeDef` body and
projects its four canonical field payloads. The slice adds one reusable
closed-union validator that dispatches tags `1..=n` to per-arm unit
validators, and generalizes the optional-payload validator so its `Some` arm
can call a decoder with any result type. Everything else is composed from the
validators already built for the Parameter, Function, simple-entity, and
Contract slices.

The executable closure validates:

- entity kind 4 and exactly fields 1 through 4;
- every ordered `TypeParameterDef`, including its 32-bit ordinal;
- the closed `TypeDefForm` union: tag 1 record fields, tag 2 variant cases;
- every `RecordField`: exact 32-byte member identity, recursive `TypeExpr`
  value type, and visibility 1 through 4;
- every `VariantCase`: exact 32-byte member identity and an optional payload
  type (`None` must carry an empty payload, `Some` a recursive `TypeExpr`);
- the raw-ID-sorted invariant contract set; and
- definition visibility 1 through 4.

Empty record and variant forms are accepted; the projection returns the
original canonical field payloads unchanged.

## Executable surface

The image contains 22 reachable functions, 1,408 parameters, 379 blocks, 664
operations, and 205 constants. Its encoded image is 88,508 bytes. The approved
package digest is
`7e0a3f3205acb4e73971227f92cc4b5077b0f26320bd4573d84af34b8e510bc9`
under the declared codec-profile execution limits.

## Validation

Positive fixtures cover a two-parameter declaration with the maximum 32-bit
ordinal, a record form with nested composite field types and mixed field
visibility, a variant form with both present and absent payload types, an
empty record form, and a two-element ordered invariant set. Negative cases
cover wrong entity kind, missing and unknown fields, a 33-bit type-parameter
ordinal, an unknown form tag, a short member identity, a malformed field type,
an invalid field visibility, a variant case missing its payload field, an
invalid option tag, a non-empty `None` payload, a malformed `Some` payload
type, unordered invariants, and an invalid definition visibility.

```text
cargo test -p sley-vm --test rw120_toolchain_integration type_def_schema -- --nocapture
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
```

The arbitrary TypeDef decoder is not yet wired into the all-kind program
dispatch. Kinds 7 through 9, 12, and 14 still require arbitrary schema
decoders, so RW-090 remains open.

# RW-090 recursive ConstValue decoder

Date: 2026-09-18

Status: implemented recursive structural slice; RW-090 remains open

## Result

The Sley codec now validates complete recursive `ConstValue` trees (SMP1
appendix C `const_value`) without a recursive function-call cycle. A shallow
child projector validates one node — its two-field record, its recursive
`value_type` through the existing `TypeExpr` decoder, and its sixteen-arm
`data` union — and returns the node's direct and list children in the shape
the existing worklist driver already consumes, so the `TypeExpr` driver is
reused unchanged.

The projector covers every `ConstData` family:

- leaves through the constant leaf validators (unit, bool, signed and
  unsigned 128-bit varints, canonical `f32`/`f64` bits, bytes, UTF-8 text);
- `Sequence` as list children;
- `Record` as an exact definition identity plus an ordered `FieldConst` list
  whose member identities are validated and whose values become children;
- `Variant` as two exact identities plus an optional payload child;
- `Map` as an ordered `MapEntryConst` list whose raw key bytes must be
  strictly increasing (`SCB_MAP_DUPLICATE` / `SCB_MAP_ORDER`) and whose keys
  and values both become children;
- `Option` (`0` with an empty payload, or `1`) and `Result` (`1` or `2`) as
  optional single children;
- `FunctionRef` as an exact identity plus recursively validated type
  arguments; and
- `BuiltinFailure` as a failure kind `1..=5` and a 16-bit code.

New reusable builders: a closed tagged-child projection, a two-field entry
list walker (record-field and ordered-map modes), a prefixed-record child
projection, and the sixteen-arm projector itself; the unit list validator now
takes its element decoder's result type so it can walk `TypeExpr` lists.

## Executable surface

The image contains 31 reachable functions, 1,825 parameters, 538 blocks,
1,056 operations, and 348 constants. Its encoded image is 128,014 bytes. The
approved package digest is
`236e1492b4b5e409ef35d59a6fe67024cf87a9470049a0b949388e3e8c3d4e09`
under the declared codec-profile execution limits.

## Validation

The positive fixture is one 936-byte constant containing every family
(including a record holding an optional text, a variant with and without a
payload, a two-entry map, both result arms, and a function reference with two
type arguments); it decodes in 83,782 instructions under the 100,000
instruction codec profile. A 20-deep option chain also decodes. Thirty-seven
refusal cases cover the node record, an unknown data tag, every leaf family,
and every composite boundary (short identities, bad nested leaves at several
depths, missing and unknown fields, option and result tags, duplicate and
unordered map keys, out-of-range failure kinds, wide failure codes, trailing
bytes). Every case, accepted or refused, is compared with
`sley_mutate::decode_const_value` on the same bytes.

```text
cargo test -p sley-vm --test rw120_toolchain_integration const_value_decoder -- --nocapture
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
```

## Known deviations

- Depth accounting follows the `TypeExpr` driver precedent: one unit per
  `ConstValue` node with the 63-level bound, whereas the native codec charges
  each intermediate container (record, list, union) separately. Accepted
  inputs are identical for practical depths; the refusal boundary for
  pathological nesting differs and is not yet pinned.
- Map key order is checked on raw entry bytes before the key nodes are
  themselves validated; the native decoder validates each entry first. The
  accepted set is identical (canonical keys re-encode to themselves); only
  the refusal code for an entry that is both malformed and misordered can
  differ.
- Instruction cost is roughly 90 instructions per input byte, so constants
  above roughly 1.1 KiB exceed the current codec profile budget.

The decoder is not yet wired into an entity schema. Constant (kind 9) and
CapabilityRequirement (kind 12) compose it next.

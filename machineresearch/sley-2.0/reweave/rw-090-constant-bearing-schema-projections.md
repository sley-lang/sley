# RW-090 constant-bearing schema projections

Date: 2026-09-18

Status: implemented bootstrap slice; RW-090 remains open

## Result

The Sley codec now validates the two entity schemas whose fields carry
recursive constants, composing the recursive `ConstValue` decoder with the
simple-record schema path:

- `Constant` (kind 9): one recursive `ConstValue` field; and
- `CapabilityRequirement` (kind 12): an exact effect identity, an ordered
  list of recursive `ConstValue` scope constants, and a raw-ID-sorted
  constraint contract set.

The `ConstValue` closure is now a reusable builder that any schema image can
compose with, and the simple field-validator table gained a bytes-result
variant so a field can be validated by a decoder that returns the accepted
bytes (the recursive `ConstValue` and `TypeExpr` drivers). Both entries
return `Result<Tuple<Bytes...>, Bytes>` with one member per manifest field,
preserving the original canonical field payloads.

## Executable surfaces

| Schema | Functions | Parameters | Blocks | Operations | Constants | Image bytes | Package digest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Constant | 33 | 1,846 | 558 | 1,091 | 358 | 131,828 | `faac35ee29a55402945af77f7c630298b265b3c4e732b47a51af2f7fc8a6499d` |
| CapabilityRequirement | 34 | 1,874 | 570 | 1,112 | 365 | 134,608 | `7ad59c6bc4b28d893fa7adcf49c3f0f132e48af8d0a81ee145f5afd7c910935d` |

Both are approved under the declared codec-profile execution limits.

## Validation

Positive fixtures use a nested scope constant (a named record holding an
ordered map of text and an optional boolean), a unit constant, a two-scope
list, and an empty scope list. Negative coverage includes wrong entity kinds,
missing and unknown fields, an invalid nested boolean leaf, unordered map
keys inside a constant, a short effect identity, an unknown constant data tag
inside a scope, a non-minimal scope count, and unordered and duplicate
constraint contracts.

```text
cargo test -p sley-vm --test rw120_toolchain_integration constant_schema_decoder
cargo test -p sley-vm --test rw120_toolchain_integration capability_requirement_schema_decoder
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
```

These decoders are not yet wired into the all-kind program dispatch. Kinds 7
(Block), 8 (Operation), and 14 (TestCase) still require arbitrary schema
decoders, so RW-090 remains open.

# RW-090 simple entity schema projections

Date: 2026-09-18

Status: implemented bootstrap slice; RW-090 remains open

## Result

The Sley codec now has a reusable simple-record schema decoder. Given a
closed entity kind and an ordered field-validator table, it validates the
outer union, projects an exact record, validates every field in manifest
order, and returns the original canonical field payloads. The current field
validators cover exact 32-byte identities, exact-width uvars, bounded closed
enums, recursive `TypeExpr` values, and ordered or sequence entity-ID
collections.

Three entity schemas now use this path:

- `GlobalValue` (kind 10): recursive value type, exact initializer identity,
  and visibility 1 through 4;
- `EffectDef` (kind 11): effect kind 1 through 8, four recursive type fields,
  and visibility 1 through 4; and
- `AdapterImport` (kind 15): exact adapter identity, 32-bit ABI version, three
  recursive type fields, and an ordered effect-ID set.

Each entry returns `Result<Tuple<Bytes...>, Bytes>` with one tuple member per
manifest field. The bytes are preserved from the input after validation.

## Executable surfaces

| Schema | Functions | Parameters | Blocks | Operations | Constants | Image bytes | Package digest |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| GlobalValue | 14 | 1,285 | 303 | 545 | 170 | 73,854 | `b7853a97f19b9ba5872e4d435470545b16eaadede908dea01a069ebd075af8aa` |
| EffectDef | 15 | 1,353 | 323 | 595 | 190 | 79,702 | `97f6615d6c022d393c7e2070a032f01966c9b2d3c24be1f78d5851065fad9dfa` |
| AdapterImport | 15 | 1,353 | 323 | 593 | 188 | 79,550 | `f67002b5b9684860e49c80115c22f7bf267e23f10910513229bebb0b8b6ac3b3` |

## Validation

Positive fixtures use nested composite types, non-minimal profiles such as
the maximum 32-bit ABI version, and nonempty ordered effect sets. Negative
coverage includes wrong entity kinds, missing and unknown fields, invalid
closed enums, malformed recursive types, short identities, a 33-bit ABI
version, and unordered effect identities.

```text
cargo test -p sley-vm --test rw120_toolchain_integration schema_decoder
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
```

These decoders are not yet wired into the all-kind program dispatch. Kinds 4,
7 through 9, and 12 through 14 still require arbitrary schema decoders, so
RW-090 remains open.

# RW-090 Function schema projection

Date: 2026-09-18

Status: implemented bootstrap slice; RW-090 remains open

## Result

The Sley codec now composes the generic canonical union, record, and list
decoders into one executable image and projects a runtime `FunctionBody` into
its eight ordered field payloads. The entry function owns these decisions in
Sley:

- the outer union tag is exactly entity kind 5;
- record fields 1 through 8 are all present;
- no unknown record field remains after the eight required fields are removed;
- fields 1, 2, 4, 6, and 7 are exact canonical list containers; and
- structural decoder failures retain their canonical `SCB_*` error codes.

The projection returns
`Result<Tuple<Bytes, Bytes, Bytes, Bytes, Bytes, Bytes, Bytes, Bytes>, Bytes>`.
Each successful tuple member is the exact canonical payload for its schema
field. This provides the next Sley-owned boundary for decoding type
parameters, entity identities, `TypeExpr`, set ordering, and visibility.

## Executable surface

The composite image contains five reachable functions: the Function schema
entry, generic record decoder, generic union decoder, generic list decoder,
and shared canonical uvar decoder. It contains 1,003 parameters, 160 blocks,
289 operations, and 77 constants. Its encoded image is 44,746 bytes. The
approved package digest is
`a7dcf6d9a4279389b700bb37067ed47933fbb3c4da10df508ddac8fcbad1cc80`.

## Validation

Focused tests compare all eight projected payloads with the native SCB cursor
over a non-empty Function body. Negative cases cover a wrong entity kind, a
missing field, an unknown field, and a non-minimal list count inside the
Function record.

```text
cargo test -p sley-vm --test rw120_toolchain_integration function_schema_decoder -- --nocapture
```

The slice does not yet claim a complete Function decoder. Fixed-width entity
IDs, set ordering, recursive `TypeExpr`, type-parameter records, and visibility
are still raw canonical payloads. The fixed representative Function codec and
the canonical toolchain root therefore remain unchanged until those semantic
decoders are composed and tested.

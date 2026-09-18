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
- field 1 is an exact canonical list container;
- fields 2 and 6 are lists of exact 32-byte entity identities;
- fields 4 and 7 are strictly increasing sets of exact 32-byte entity
  identities;
- field 5 is an exact 32-byte entity identity; and
- field 8 is an exact canonical uvar in the visibility range 1 through 4; and
- structural decoder failures retain their canonical `SCB_*` error codes.

The projection returns
`Result<Tuple<Bytes, Bytes, Bytes, Bytes, Bytes, Bytes, Bytes, Bytes>, Bytes>`.
Each successful tuple member is the exact canonical payload for its schema
field. This provides the next Sley-owned boundary for decoding type
parameters, entity identities, `TypeExpr`, set ordering, and visibility.

## Executable surface

The composite image contains nine reachable functions: the Function schema
entry, entity-identity collection validator, fixed-32 validator, generic
record decoder, generic union decoder, generic list decoder, and shared
canonical uvar decoder, plus exact and bounded uvar wrappers. It contains
1,067 parameters, 192 blocks, 346 operations, and 98 constants. Its encoded
image is 51,112 bytes. The
approved package digest is
`66fe5448510a8df7cded714a8ea3c7713168c0aaaf93a6f426033666fdac3dc8`
under the declared codec-profile execution limits.

## Validation

Focused tests compare all eight projected payloads with the native SCB cursor
over a non-empty Function body. Negative cases cover a wrong entity kind, a
missing field, an unknown field, a non-minimal list count, a short parameter
identity, an unordered effect set, and a short entry-block identity.
Visibility cases also cover an out-of-range tag and a non-minimal encoding.

```text
cargo test -p sley-vm --test rw120_toolchain_integration function_schema_decoder -- --nocapture
```

The slice does not yet claim a complete Function decoder. Fixed-width entity
Recursive `TypeExpr` and type-parameter records are still raw canonical
payloads. The fixed representative Function codec and the canonical toolchain
root therefore remain unchanged until those semantic decoders are composed
and tested.

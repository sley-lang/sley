# RW-090 Contract schema projection

Date: 2026-09-18

Status: implemented bootstrap slice; RW-090 remains open

## Result

The Sley codec now validates an arbitrary canonical `ContractBody` and
projects a normalized five-field tuple. It composes new reusable validators
for lists of nested records, exact projected records, closed payload unions,
and optional trailing record fields.

The executable closure validates:

- entity kind 13 and required fields 1 through 4;
- the optional field 5, accepting both canonical record counts 4 and 5;
- exact target and predicate identities;
- contract kinds 1 through 7;
- every ordered `ContractBinding`, including its 32-bit predicate ordinal;
- all four `ContractSource` arms and their payload rules; and
- all six exact 64-bit `ResourceLimits` fields when limits are present.

When resource limits are absent, the fifth projected `Bytes` value is empty.
An encoded limits record is never empty, so this is an unambiguous normalized
absence marker within this bootstrap projection.

## Executable surface

The image contains 18 reachable functions, 1,142 parameters, 271 blocks, 510
operations, and 153 constants. Its encoded image is 68,088 bytes. The approved
package digest is
`4059d026826f8c979e58be21128d475c33e1ac01034a1895059fe793bda4b6cf`
under the declared codec-profile execution limits.

## Validation

The positive fixture covers parameter, result, and global binding sources,
the maximum 32-bit predicate ordinal, all six resource fields, and the absent
optional-limits form. Negative cases cover wrong entity kind, missing and
unknown fields, short identities, an invalid contract kind, an unknown source
arm, a 33-bit binding ordinal, and an incomplete limits record.

```text
cargo test -p sley-vm --test rw120_toolchain_integration contract_schema_decoder -- --nocapture
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
```

The arbitrary Contract decoder is not yet wired into the all-kind program
dispatch. Kinds 4, 7 through 9, 12, and 14 still require arbitrary schema
decoders, so RW-090 remains open.

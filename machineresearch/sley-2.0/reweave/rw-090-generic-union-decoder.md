# RW-090 canonical generic union decoder

Status: PROVISIONAL CODEC CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This slice adds the
second schema-independent runtime parser needed by the SSMC1 codec. It does
not complete any entity-body schema or establish official C1.

The retained Sley graph accepts an arbitrary canonical SCB1 union and returns
its runtime `UInt64` tag plus exact `Bytes` payload. The tag and payload length
are decoded by the Sley uvar implementation. The payload is bounded, copied,
and converted by the admitted byte bridges, and the graph requires exact input
consumption. Empty, trailing, truncated, non-minimal-tag, and over-limit
payload cases return the frozen structural error bytes.

The graph contains two functions, 449 parameters, 74 blocks, 146 operations,
and 40 constants. Its admitted image is 21,210 bytes and its package digest is
`94db891d5b7fbedc3a9b4785bbe97df5b5a23a679de36117b9cd2f8780f72fd6`.
The tests pin those facts and the full CFG surface, then decode a nontrivial
runtime tag and payload and exercise all boundary refusals.

Together with the generic ordered-record decoder, this removes fixed digest
profiles from the two outer structural forms used throughout entity bodies.
The remaining RW-090 work is generic list decoding and schema-specific nested
decoding for types, constants, references, immediates, terminators, and every
mandatory entity kind.

Validation:

- `cargo test -p sley-vm --test rw120_toolchain_integration generic_union_decoder -- --nocapture`
- `cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings`
- `cargo fmt --all -- --check`

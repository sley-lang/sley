# RW-090 entity-identity collection decoder

Date: 2026-09-18

Status: implemented structural/identity slice; RW-090 remains open

## Result

The Sley codec now validates arbitrary canonical `List<EntityId>` and
`Set<EntityId>` payloads without a native semantic service. The entry accepts
the canonical bytes, an `ordered` Boolean, and Unit. It calls the Sley list
decoder, walks every runtime element by ordinal, calls a Sley fixed-32
validator for each identity, and optionally enforces strict byte ordering.

List mode preserves order and permits repeated identities. Set mode returns
`SCB_MAP_DUPLICATE` for equal adjacent identities and `SCB_MAP_ORDER` for a
descending pair. Wrong identity width returns `SCB_LENGTH_OVERFLOW`.

The standalone reachable closure contains four functions: identity collection,
fixed-32, generic list, and canonical uvar decoding. It contains 538
parameters, 96 blocks, 180 operations, and 50 constants. Its image is 26,064
bytes. Under the standard focused codec limits, the approved package digest is
`cf1b0657353d4ade9f480217fda3aee3dd2cfd127f67b0f3300a96567b8233b5`.

## Validation

```text
cargo test -p sley-vm --test rw120_toolchain_integration entity_id_collection_decoder -- --nocapture
```

The positive cases cover a non-sorted repeated list and a strictly increasing
set. Negative cases cover a 31-byte identity, duplicate set identities, and a
descending set. The validator is also composed into the Function schema image
for parameters, effects, blocks, contracts, and the entry-block identity.

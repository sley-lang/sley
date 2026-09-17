# RW-080 §1.3 package-builder slice 37: empty inventories and dependency bytes

Status: PROVISIONAL C0 CONSTRUCTION. This is Sley-owned byte construction
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

## Scope and behavior

A Sley function now emits the canonical constants, layouts, imports, and
dependency section bytes for the empty-inventory boundary. Constants, layouts,
and imports each emit the exact big-endian `u64(0)` counted section. The
dependency encoder constructs every field in format order from typed runtime
inputs: entry, schema epoch, state root, gate operation and bridge counts,
arbitrary ordered closure fingerprints, and the four admitted execution
limits.

The fixed `EXTENDED_V1` VM/lowering/lowerer versions, zero profile counts,
`cancel_at_fuel = None`, and empty global/contract counts are Sley constants.
The encoder owns every byte append. It converts identities and fingerprints
through B2V1, emits big-endian integers through the existing checked Sley
fixed-width helpers, mutates only Sley-owned octet vectors through PSH1, and
returns section `Bytes` through V2B1. The generalized vector byte appender now
supports both counted and uncounted sequences; all previous counted users keep
their original behavior.

## Native parity

The parity fixture supplies two distinct 32-byte semantic fingerprints,
nonzero gate counts, all three identities, and nonzero limits after the Sley
program has been admitted. Its four output sections equal
`encode_constants_section`, `encode_layouts_section`,
`encode_imports_section`, and `encode_dependency_section` byte-for-byte. The
native dependency decoder then recovers the original fingerprints, limits,
and identities from the Sley bytes.

## Explicit remainder

Slices 38 and 39 supersede this boundary: Sley now composes the fixed header
and five frames, and emits nonempty constants, layouts, imports, globals, and
contracts. The remaining construction work is one canonical `build_package`
entry over those admitted closures. SHA-256 stays host artifact mechanics as
frozen by `rw-075-hash-inventory.md`; the builder consumes its five results in
fixed-width fields. Contract/surface review remains mandatory. R2 stays
provisional pending that implementation and the independent acceptance debt.

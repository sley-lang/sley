# RW-080 §1.3 package-builder slice 39: nonempty inventory rows

Status: PROVISIONAL C0 CONSTRUCTION. This is Sley-owned byte construction
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

## Scope and ownership

A Sley function now emits canonical counted constants, layouts, and imports
sections from ordered canonical row bytes. It derives and emits the section
count. Constant rows already contain the entity identity and constant-value
length frame required by `EXEC_PACKAGE_V2`; layout and import rows are exact
canonical bodies, so Sley derives and emits each `u64` row length before
appending the body. The same checked loop handles empty and nonempty vectors.

The dependency encoder now derives the global and contract counts and appends
every ordered canonical row. Its admitted-limit cancellation field covers
both `None` and `Some(fuel)` at runtime. The codec/checker remains responsible
for the canonical meaning of each row body. The builder owns section counts,
row frames, ordering, and byte composition; it receives no prebuilt section.

## Executable evidence

`rw080_lower_scaffold.rs` constructs two constants, two type layouts, two
exact adapter imports, one global, and one contract. For each inventory, the
test obtains independently canonical single-row bodies from the native codec,
executes the admitted Sley section builder, and compares the complete result
byte for byte with the corresponding native section encoder. The strict
decoders recover the same inventories.

The end-to-end composer test uses those populated Sley-emitted sections, a
real Sley-emitted `SLEYBC02` image, and the five fixed-arity host-mechanic
SHA-256 results. The Sley envelope equals `encode_package_envelope_v2`
byte-for-byte and strict hydration reconstructs the exact populated
`ExecutionPackage`, including `cancel_at_fuel = Some(1500)`.

## SHA-256 boundary and remainder

The five SHA-256 values are execution results supplied to the Sley builder,
consistent with `rw-075-hash-inventory.md`: executable/package SHA-256 is
host artifact mechanics, outside `RAW_BLAKE3_V1`, while Sley constructs every
hashed section byte and places each result in its fixed header field. Adding a
second raw primitive or hand-rolling SHA-256 in Sley would contradict that
frozen boundary.

The lowerer, inventory builders, dependency builder, and envelope composer
remain separate admitted closures. The next construction slice must compose
them behind the canonical `build_package` entry and preserve the mechanical
digest handoff. Contract/surface review remains mandatory, and R2 remains
provisional pending the independent acceptance debt.

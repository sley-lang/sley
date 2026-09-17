# RW-080 §1.3 package-builder slice 38: v2 envelope composition

Status: PROVISIONAL C0 CONSTRUCTION. This is Sley-owned byte construction
under the operator development override. It is not RW-110 completion, C1,
self-hosting evidence, or runtime authority.

## Scope and behavior

A Sley function now composes the complete serialized `EXEC_PACKAGE_V2`
envelope from five exact sections, five exact SHA-256 digest inputs, and the
entry/epoch/root identities. The function emits the fixed `SLEYPKG1` magic,
version 2, frozen profile digest, host ABI 2, VM `[1,0,0]`, the five digest
slots in canonical order, and the three repeated identities. It then derives
each section's byte length inside Sley and appends all five `u64`-framed
sections in image/constants/layouts/imports/dependency order.

The digest positions are five distinct function parameters, so a caller
cannot alter the header width by supplying a variable-length digest vector.
All fixed-width integers use the existing checked Sley appenders. Exact byte
chunks use the same B2V1/PSH1 traversal as image and dependency emission, and
V2B1 returns the final envelope bytes. A new framed-section helper derives
length from the actual section octets before appending the unchanged section.

## End-to-end parity

The parity test first executes the prior Sley section encoder with two runtime
closure fingerprints and nonzero limits. It computes the currently native
SHA-256 section digests, executes the Sley envelope composer, and compares the
result byte-for-byte with `encode_package_envelope_v2`. The strict native
hydrator then decodes the Sley-composed bytes back to the exact original
`ExecutionPackage` and digest set.

The test uses a real `SLEYBC02` image and raises only the fixture's monotonic
value-unit budget to accommodate repeated immutable vector construction; the
instruction, fuel, and output ceilings remain bounded.

## Explicit remainder

The Sley builder receives the five SHA-256 section digests in this slice; it
does not yet produce them. Nonempty constants/layouts/imports/globals/contracts
also remain. The compiler-side functions are still separate admitted closures
rather than one final `build_package` entry. Contract/surface review remains
mandatory, and R2 remains provisional pending the independent acceptance
debt.

# RW-080 aggregate slice 87: provisional toolchain component graph

Status: PROVISIONAL C0 CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This slice binds
the five RW-080 toolchain surfaces into one admitted Sley closure. It does not
construct the canonical state root `S`, establish C1, complete RW-080, or
change R2 from NOT_READY.

## Constructed closure

`crates/sley-vm/tests/rw080_toolchain_graph.rs` constructs a five-function
graph with a driver entry, codec, checker, lowerer, and package-builder
surface. The driver accepts a named `BuildManifest` record containing the
object-closure bytes, entry points, expected artifact digests, source epoch,
state root, and the four frozen profile/dependency pins. It extracts the
object closure, directly calls all four leaves, and returns the typed
`BuildError::Incomplete` arm after dispatch. The package-builder surface
returns the extracted closure bytes, the codec surface returns its scaffold
VERSION code (`6`), and the checker and lowerer surfaces return their scaffold
success codes (`0`).

This leaf behavior is intentionally small. It proves that the module
identities, types, call edges, closure traversal, lowering, admission,
approval, execution, and package framing coexist in one graph. It does not
replace the mature standalone codec/checker/lowerer/package-builder algorithms
already exercised by their separate construction units, and it does not claim
that those algorithms have been rebased into this graph. The aggregate has no
source DSL and uses no host imports.

Three retained named layouts define `BuildManifest`, `BuiltToolchain`, and
`BuildError`. `BuildError` has typed codec, checker, lowerer, package,
fixed-point, and incomplete arms. The current driver never constructs
`BuiltToolchain`; returning the incomplete arm keeps successful assembly,
per-artifact digest production, and fixed-point equality open for RW-120.

## Pinned component identity

Every language-owned entity now uses the contract derivation rule
`EntityId::derive(workspace, candidate_nonce, kind, ordinal)`. The exact
genesis seed, candidate nonce, derived workspace, 36 entity/object bindings,
five explicit entry-point records, their five target functions, empty
partial-component contract/test anchors, object bytes, and root bytes are retained in
`rw-080-toolchain-component-manifest.json`. The object bundle and state-root
bytes use deterministic gzip/base64 storage with lengths and SHA-256 digests;
each object also records its bundle offset, exact length, owner, identity
origin and creation provenance.

The closure now includes workspace, package, namespace, and entry-point
metadata rather than treating function records themselves as the root entry
points. The namespace also retains the three driver type definitions. All 36
objects round-trip through `import_entity_object`. Their exact bindings
produce registered schema epoch
`a7fcf97a85d41ef9b1c89394a324f2dc7ec875b9ded48a783104314857dc870e`
and partial component root:

`9b8a13eee185d9452a3d19dbdc727015f2536226ec4fa06ab2f7a096e9fdad3f`

The 2,803-byte root record round-trips through `import_state_root`; its exact
stored bytes have SHA-256
`25a36144a42f01fc7808bcbefb1f090cbcef2e91ef32aa6bbe975ef9a0a57d80`.
Its policy binding is no longer an opaque placeholder: an empty policy built
through `PolicyRootBuilder` and the preserved policy registry round-trips
through `import_policy_root`, with root
`3b8eab80acdc874bd3f3958981d0da81d2ce2314073fc9773d75ed901f0dc89c`
and exact retained bytes in the component manifest.
Changing the codec constant from `6` to `7` changes exactly that constant's
object binding and changes the derived root, providing a bounded anti-copy
check.

The graph admits under `BOOTSTRAP_PROFILE_2`, lowers to a 1,530-byte
`SLEYBC02` image, and produces a 3,393-byte `EXEC_PACKAGE_V2` envelope. The
closure has ten operations, zero bridge uses, and five semantic
fingerprints. Its exact package digest is:

`6e53babe851b2e9911852402cddad16cb6c80b611503416a7cc01c20ee98b72a`

The test pins that digest and strictly decodes the envelope back to the driver
entry, registered schema epoch, partial component root, and section digests.
This digest identifies the provisional component package only. It is not a
semantic fingerprint, acceptance receipt, or release identity.

Two runtime manifests exercise the admitted driver. Both use 19 fuel and ten
instructions; their peak value units are 2,782 and 2,790 respectively. The
typed incomplete result is checked exactly. The retained operation graph
binds the manifest projection to the builder argument, and the runtime executes
the full ten-instruction path without exposing a false success value.

## Explicit construction boundary

The root above is an actual canonical `StateRoot` for this partial scaffold
component. It is not complete `S`: the leaf functions do not contain the
mature algorithms, the driver cannot construct `BuiltToolchain`, and the
contract/test anchors are explicitly construction-only empty
partial-component anchors. The accepted empty policy authorizes no principal
or operation. `bootstrap-manifest.json` therefore remains unchanged with `S`
absent and C0 without a preserved image. Neither the root nor package digest
above may be copied into those stage fields.

The next aggregate step is to replace each leaf surface with the already
constructed algorithm graph behind the same call boundary, implement typed
error propagation and successful artifact/fixed-point assembly, replace the
partial anchors with their accepted complete records, and retain the expanded
canonical object closure. Only that
complete closure can become candidate `S`, after the contract's review and
acceptance gates.

Validation for this slice includes the focused six-test target, Clippy with
warnings denied, workspace formatting,
`scripts/check_rw080_toolchain_component.py`, the derived-evidence checks, the
anti-goal checker, and `git diff --check`. This is local construction evidence
only; independent acceptance remains pending.

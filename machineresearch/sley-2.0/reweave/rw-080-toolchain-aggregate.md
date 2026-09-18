# RW-080 aggregate slice 87: provisional toolchain component graph

Status: PROVISIONAL C0 CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This slice binds
the five RW-080 toolchain surfaces into one admitted Sley closure. It does not
construct the canonical state root `S`, establish C1, complete RW-080, or
change R2 from NOT_READY.

## Constructed closure

`crates/sley-vm/tests/rw080_toolchain_graph.rs` constructs a five-function
graph with a driver entry, codec, checker, lowerer, and package-builder
surface. The driver directly calls all four leaves and returns their results
as one tuple. The package-builder surface returns the runtime manifest bytes,
the codec surface returns its scaffold VERSION code (`6`), and the checker
and lowerer surfaces return their scaffold success codes (`0`).

This leaf behavior is intentionally small. It proves that the module
identities, types, call edges, closure traversal, lowering, admission,
approval, execution, and package framing coexist in one graph. It does not
replace the mature standalone codec/checker/lowerer/package-builder algorithms
already exercised by their separate construction units, and it does not claim
that those algorithms have been rebased into this graph. The aggregate has no
source DSL and uses no host imports.

The success-only driver surface also does not invent a `BuildError`
vocabulary. Error propagation and the complete typed build manifest remain
part of the real driver construction required by contract section 1.4.

## Pinned component identity

The graph admits under `BOOTSTRAP_PROFILE_2`, lowers to a 1,154-byte
`SLEYBC02` image, and produces a 2,063-byte `EXEC_PACKAGE_V2` envelope. The
closure has eight operations, zero bridge uses, and five semantic
fingerprints. Its exact package digest is:

`25cf82a3fbf5c07b4926cb03e1acbb0d6a12e52bb53fd7d67abfa5af03d05cac`

The test pins that digest and strictly decodes the envelope back to the driver
entry, schema epoch, fixture root, and section digests. This digest identifies
the provisional component package only. It is not a `StateRoot`, semantic
fingerprint, acceptance receipt, or release identity.

Two runtime manifests exercise the admitted driver. Both use 17 fuel and
eight instructions; their peak value units are 1,422 and 1,430 respectively.
The returned manifest bytes are checked exactly, so the builder edge is
runtime-data-dependent rather than a cached result.

## Explicit construction boundary

The package still carries historical fixture root `[9; 32]` and schema epoch
`[8; 32]`. No canonical object closure has been persisted, no root has been
computed from that closure, and `bootstrap-manifest.json` therefore remains
unchanged with `S` absent and C0 without a preserved image. The digest above
must not be copied into those fields.

The next aggregate step is to replace each leaf surface with the already
constructed algorithm graph behind the same call boundary, define the typed
manifest and error values, persist the complete canonical object closure, and
compute its actual state root. Only that complete closure can become candidate
`S`, after the contract's review and acceptance gates.

Validation for this slice includes the focused two-test target, Clippy with
warnings denied, workspace formatting, JSON validation, the derived-evidence
checks, the anti-goal checker, and `git diff --check`. This is local
construction evidence only; independent acceptance remains pending.

# RW-090 canonical codec object construction — provisional record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice converts the
executable four-leg codec fixture into canonical SSMC1 entity objects with
contract-derived construction identities. It does not yet bind those objects
into canonical state root `S`, complete RW-080/RW-090, or change R2 from
NOT_READY.

## Identity construction

`crates/sley-vm/tests/rw080_codec_program/canonical_codec.rs` rewrites every
language-owned identity and every internal reference in the composed codec
image. It uses the retained RW-080 construction seeds:

- genesis seed: byte `80` repeated 32 times;
- candidate nonce: byte `87` repeated 32 times;
- workspace: `WorkspaceId::derive(genesis)`;
- functions, parameters, blocks, operations, and constants:
  `EntityId::derive(workspace, candidate, kind, 1000 + sorted_index)` for
  kinds 5 through 9;
- codec entry point: kind 16, ordinal 5.

The ordinal ranges are disjoint from the existing 45-object aggregate
component. Sorting the original construction identities before allocation
makes the retained mapping deterministic and independent of vector insertion
order. Every reference in function graphs, types, constants, value operands,
terminators, immediates, and ownership fields is rewritten through the same
closed map.

The four bridge entities are not reallocated. B2V1, V2B1, PSH1, and RHW1 keep
their frozen profile identities and are materialized as inherited
`AdapterImport` objects with exact request, response, failure, ABI, and effect
fields. A test proves the inherited IDs are disjoint from every derived
language-owned identity.

## Canonical object evidence

The remapped graph contains 6,221 canonical objects across its functions,
parameters, blocks, operations, constants, four adapter imports, and one local
entry point. Every object is built under the registered source epoch
`sley_state_root::conformance_epoch_id()` and reimports byte-for-byte through
`sley_mutate::import_entity_object`.

Measured construction facts:

- objects: 6,221;
- total stored object bytes: 1,571,749;
- SHA-256 of stored bytes concatenated in entity-ID order:
  `981f68e6114b556e9f20f09d8e4f5393a039cb6b7678774328b2b2f62705fcdb`.

This digest is a construction-bundle check, not an object identity, semantic
fingerprint, state root, package digest, or acceptance verdict. The objects are
currently reproduced from the exact typed construction source during the test;
the bundle is not yet claimed as complete `S` and is not written into
`bootstrap-manifest.json`.

## Behavioral and anti-copy checks

After remapping, the complete image still lowers, admits under Bootstrap
Profile V2, approves, and decodes a native canonical Workspace object through
the real `codec_main` entry. The materialization test verifies a one-to-one
object inventory, unique identities, all function and adapter bindings, and
exact import equality.

Changing one Boolean constant changes exactly its canonical Constant object's
`ObjectId`. This pins an early anti-copy property: object identity follows the
semantic input instead of a cached bundle result.

Validation commands:

- `cargo test -p sley-vm --test rw080_codec_program_outer canonical_codec -- --nocapture`
- `cargo test -p sley-vm --test rw080_codec_program_outer` (102 tests)
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo fmt --all -- --check`

The next construction step is to merge this object inventory with the retained
toolchain metadata, replace the aggregate codec leaf and its entry point, and
derive a new partial component root. Checker, lowerer, package builder, driver
success assembly, semantic contract/test coverage, independent review, and
formal acceptance remain open.

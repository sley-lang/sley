# RW-090 canonical codec object construction — provisional record

Status: PROVISIONAL (2026-09-18, operator development override
`rw075_correction.operator_override_2026_09_07`). This slice converts the
executable four-leg codec fixture into canonical SSMC1 entity objects with
contract-derived construction identities. The slice now binds that inventory
into a reproducible codec component root. This component is not canonical
state root `S`, does not complete RW-080/RW-090, and does not change R2 from
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
reproduced from the exact typed construction source during the test. The
bundle is not claimed as complete `S` and is not written into
`bootstrap-manifest.json`.

## Codec component root

The component adds twelve retained objects to the codec bundle: workspace,
package, and namespace metadata; two Boolean witness functions with their two
blocks, two operations, and shared constant; one precondition Contract; and
one executable TestCase. The existing local codec EntryPoint remains part of
the original 6,221-object bundle. All 6,233 bindings are committed in one
registered-epoch `StateRoot` with the retained empty policy root.

Pinned construction facts:

- entity bindings: 6,233;
- component root:
  `87afab53ad6634ae0e169cbe767e641c292a363e7ba435808ce9be64ee36e555`;
- stored root bytes: 411,675;
- stored-root SHA-256:
  `ea8ed0afe3a82d47dc91058196665be766b16733fbde4ea1fd94edb505292a2f`.

The test root targets the canonical frozen-schema decode child. Its exact
preimage inputs and exact `(epoch, record)` Result value pass the S20-240
contract/test validator and execute from a package bound to this component
root. The exported four-leg `codec_main` also admits, approves, and executes
the same schema request from a package bound to this root.

The full four-leg graph is not yet an S20-230 effect-validation claim. Its
pinned RHW1 raw-hash bridge has the frozen `Bytes -> Bytes` row, while the
current pure-adapter checker accepts conversion and push rows only. RW-100
must resolve that checker/profile boundary before whole-closure semantic
validation can be claimed.

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
- `cargo test -p sley-vm --test rw080_codec_program_outer` (103 tests)
- `cargo clippy -p sley-vm --test rw080_codec_program_outer -- -D warnings`
- `cargo fmt --all -- --check`

The next construction step is the composed checker. Later RW-080 integration
must merge this root-backed codec inventory with the retained checker,
lowerer, package-builder, and driver inventories to derive `S`. Whole-closure
checker coverage, lowerer and package-builder composition, driver success
assembly, independent review, and formal acceptance remain open.

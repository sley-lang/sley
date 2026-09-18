# RW-090 arbitrary codec canonical object construction

Date: 2026-09-18

Status: implemented parallel derivation; evidence for the next component re-mint. The retained codec component (`rw-090-canonical-codec-objects.md`), its manifest, canonical `S`, and the preserved C0/C1 candidate are unchanged.

## Construction

The arbitrary four-leg composition (`rw-090-arbitrary-codec-main.md`) is
rewritten into canonical SSMC1 entity objects by the same construction the
retained component uses: genesis seed `80`, workspace
`WorkspaceId::derive(genesis)`, functions/parameters/blocks/operations/
constants at `EntityId::derive(workspace, candidate, kind, 1000 + sorted
index)`, the codec entry point at kind 16 ordinal 5, the retained schema
decode contract and test witnesses at their ordinals, and the inherited
bridge imports. The only difference is the candidate nonce: byte `88`
repeated, one apart from the retained component's `87`, so the two
components share ordinals but never an identity (a test asserts the
function identity sets are disjoint).

## Evidence

- 13,214 canonical objects (119 functions, 6,776 parameters, 1,972 blocks,
  4,185 operations, 145 constants, four inherited adapter imports, one
  entry point, the contract, the test, and the workspace/package/namespace
  spine), 3,350,895 stored object bytes, every one reimporting
  byte-for-byte through `sley_mutate::import_entity_object` under the
  registered source epoch;
- an accepted state root
  `468dbbc72a8dd4b1fede9020294e56d65713fdfcd879b222ce534f24926e77c7`
  (872,421 stored bytes, SHA-256
  `ca74e6b75678a74565b600eee9720c26be7d848ea574ea4b166389f98c501aa0`)
  binding every object, reimporting through the frozen state-root
  registry;
- the retained schema-decode contract and test validate against the
  rewritten graph, and the test executes to its exact expected value from
  a package admitted with the component root's bindings;
- the integration codec execution over the rewritten image matches the
  retained expectation.

```text
cargo test -p sley-vm --test rw120_toolchain_integration arbitrary_canonical_codec -- --nocapture
cargo test -p sley-vm --test rw120_toolchain_integration
cargo test -p sley-vm --test rw080_codec_program_outer
```

This root is a component root, not canonical `S`, and is not written into
`bootstrap-manifest.json` or any RW-120 manifest. Replacing the retained
codec component with this one (re-deriving the merged canonical `S`, the
integrated driver fixtures, and a C0-built C1 candidate) is the deliberate
operator-scheduled step that closes RW-090's bounded-codec gap.

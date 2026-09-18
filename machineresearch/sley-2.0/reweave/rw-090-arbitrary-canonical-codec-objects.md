# RW-090 arbitrary codec canonical object construction

Date: 2026-09-18

Status: CANONICAL CODEC COMPONENT (codec re-mint 2026-09-18). This component replaced the bounded generation of `rw-090-canonical-codec-objects.md` as the codec member of canonical `S`; see `rw-120-c0-c1-candidate.md` for the re-minted candidate.

## Construction

The arbitrary four-leg composition (`rw-090-arbitrary-codec-main.md`) is
rewritten into canonical SSMC1 entity objects by the same construction the
retained component uses: genesis seed `80`, workspace
`WorkspaceId::derive(genesis)`, functions/parameters/blocks/operations/
constants at `EntityId::derive(workspace, candidate, kind, 1000 + sorted
index)`, the codec entry point at kind 16 ordinal 5, the retained schema
decode contract and test witnesses at their ordinals, and the inherited
bridge imports, all under the retained candidate nonce (byte `87`). The
component therefore occupies exactly the identities the retained codec
component occupies today: it is the substitute a codec re-mint installs,
not a sibling. (A first derivation of this slice used a distinct nonce
`88` and root `468dbbc7…`; it is superseded by the replacement-semantics
derivation below so that the RW-120 dry run reproduces the exact `S` a
re-mint would produce.)

## Evidence

- 14,739 canonical objects (133 functions, 7,789 parameters, 2,131 blocks,
  4,531 operations, 138 constants, four inherited adapter imports, one
  entry point, the contract, the test, and the workspace/package/namespace
  spine), 14,727 codec objects in 3,731,690 stored bytes with ordered-bundle
  SHA-256 `e6633dcc886d970c823cd95d7c22a63a076066ce87b26b3188cd8799a1add291`
  (asserted by `canonical_codec_objects_round_trip_and_bind_the_complete_graph`),
  every one reimporting byte-for-byte through
  `sley_mutate::import_entity_object` under the registered source epoch
  (figures after the 178873d7 Council repairs; the pre-review arbitrary
  generation had 13,214 objects, 119 functions and root `8c933cca…`);
- an accepted state root
  `2958619ac0b70cbf4a33e14f26df7fdfe306d527c6f6adf31962bf75e31b7af8`
  (973,071 stored bytes, SHA-256
  `e39142274572c908ae5c7eb05f98dea31c8285f26c0ceb56e5f9bcfa4a65fbe1`)
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

This root is a component root, not canonical `S`; canonical `S` binds this
component together with the retained checker, lowerer, builder, and handoff
driver (`canonical-s-manifest.json`).

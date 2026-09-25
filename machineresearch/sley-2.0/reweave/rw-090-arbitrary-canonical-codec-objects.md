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

- 14,746 canonical objects (132 functions, 7,797 parameters, 2,131 blocks,
  4,531 operations, 138 constants, four inherited adapter imports, one
  entry point, the contract, the test, and the workspace/package/namespace
  spine), 14,734 codec objects in 3,733,462 stored bytes with ordered-bundle
  SHA-256
  `c100ef39b2afb8343517989a7d681075cfdc95912f94b30af96ef087a3b07785`
  (asserted by `canonical_codec_objects_round_trip_and_bind_the_complete_graph`
  and bound to `rw-090-codec-component-manifest.json` by
  `scripts/check_reweave_codec_component.py`), every one reimporting
  byte-for-byte through `sley_mutate::import_entity_object` under the
  registered source epoch (figures of the fifth generation, after the
  178873d7, 92fa6646, c04539b9 and db53894e Council rounds; the superseded
  generations are `superseded_*_component` in the manifest: pre-review
  arbitrary 13,214 objects / root `8c933cca…`, first repair 14,739 / root
  `2958619a…` (7426bc0b), third repair 14,740 / root `3398fa0d…`
  (0b7a1482), fourth repair 14,740 / root `d35a5d6d…` (867009de));
- an accepted state root
  `8833b4e7491d86ce39856e91baba4e2f53428e88d90aa8431c66a67c81788189`
  (973,533 stored bytes, SHA-256
  `80d0e1876c80eb6ad028ffc1d9b2c03e38cc73b5ef512f2432491a37754a1787`)
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

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

- 13,214 canonical objects (119 functions, 6,776 parameters, 1,972 blocks,
  4,185 operations, 145 constants, four inherited adapter imports, one
  entry point, the contract, the test, and the workspace/package/namespace
  spine), 3,350,895 stored object bytes, every one reimporting
  byte-for-byte through `sley_mutate::import_entity_object` under the
  registered source epoch;
- an accepted state root
  `8c933ccab89b6e150e1070ad534b498dad736bd0ae689a5d30fc600084b2d78a`
  (872,421 stored bytes, SHA-256
  `5a0d017c85ea2846bdaece500b9963ca5f761a50ed7d73bb291ec2a970a1e289`)
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

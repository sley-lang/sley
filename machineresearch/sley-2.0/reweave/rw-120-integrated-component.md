# RW-120 integrated component root — provisional record

Status: PROVISIONAL C0 CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This slice retains
the four constructed toolchain programs in one canonical object root and
executes each program against that same root. It is not the completed build
driver, whole-toolchain reconstruction, a fixed point, canonical state root
`S`, or an RW-120 pass.

## Shared construction boundary

`crates/sley-vm/tests/rw120_toolchain_integration.rs` includes the existing
canonical constructors rather than copying their graphs or reading cached
final artifacts. Narrow `pub(crate)` construction adapters expose each graph
to that integration crate while leaving the production API unchanged.

The four graphs merge by derived entity identity. A duplicate identity is
accepted only when the complete semantic value compares equal; this dedupes
the shared host-adapter rows and rejects any other collision. The resulting
union contains:

| Category | Count |
|---|---:|
| Functions | 100 |
| Parameters | 5,072 |
| Blocks | 1,846 |
| Operations | 4,049 |
| Constants | 699 |
| Adapter imports | 4 |

The root adds workspace, package, namespace, and four local EntryPoint
objects, plus real Contract and TestCase anchors. The retained exact TestCase
targets the bounded checker and selects its unary type-chain projection; it
expects normalized metrics `(4, 0, 0, 0)`. The checker subclosure plus the
contract witnesses passes `validate_contract_test_program`.

## Same-root execution

Each package below is independently lowered and admitted under C0, but every
package binds the same integrated state root:

- the exported four-leg codec performs its frozen-schema decode and returns
  the exact retained codec value;
- the seven-slice checker returns the exact retained normalized result;
- the complete fact-fed image lowerer emits the exact native-oracle
  `SLEYBC02` direct-call image; and
- the populated package builder emits the exact native-oracle
  `EXEC_PACKAGE_V2` envelope.

All 11,786 entity objects and the state-root record round-trip through their
frozen importers. Pinned facts:

- objects/entity bindings: 11,786;
- stored object bytes: 2,949,384;
- object-bundle SHA-256:
  `a9e6d19aa46527453219627e33c0694db1b540525a4b83f5b705f6a304422b97`;
- integrated component root:
  `ca4287c72bf8ec1a5c0405554603cd0e4c6afe3ed7434b59236026b1c2b13df9`;
- stored root bytes: 778,273;
- stored-root SHA-256:
  `b28ab1c1d237d3f4cf6694cba67e23c07101065d01828ffc7e28060a63daba9c`.

The executable test pins every count and digest above.

Validation commands:

- `cargo test -p sley-vm --test rw120_toolchain_integration canonical_component_programs_are_available_to_one_integration_crate`
- `cargo test -p sley-vm --test rw120_toolchain_integration canonical_programs_merge_without_semantic_identity_collisions`
- `cargo test -p sley-vm --test rw120_toolchain_integration merged_component_root_executes_all_four_canonical_programs`
- `cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings`
- `cargo fmt --all -- --check`

## Open integration surface

The packages still run as four separate entry executions. The bounded RW-120
driver does not yet call these integrated entry functions, resolve arbitrary
object-closure bytes into their typed inventories, propagate their typed
errors, or assemble all toolchain images in one invocation. The codec remains
profile-bounded and its complete graph still has the recorded RHW1
`Bytes -> Bytes` effect-validation boundary. The checker remains seven fact
projections rather than the complete arbitrary program checker and test
planner. The lowerer still consumes prepared, canonically ordered facts.

For those reasons the integrated root is a construction component, not `S`.
It provides the single identity namespace and common root needed for the next
driver integration step without asserting whole-toolchain reconstruction or
self-hosting.

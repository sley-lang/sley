# RW-120 bounded build driver — provisional record

Status: PROVISIONAL C0 CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This slice replaces
the aggregate driver's unconditional `BuildError::Incomplete` behavior with a
real bounded orchestration path. It does not integrate the mature codec,
checker, lowerer, or package-builder graphs, reconstruct the complete
toolchain, establish a fixed point, define canonical state root `S`, or satisfy
RW-120.

## Executable behavior

`crates/sley-vm/tests/rw080_toolchain_graph/rw120_driver.rs` derives a
successor of the preserved RW-080 aggregate without changing that historical
component. The successor driver:

1. projects the object closure, expected artifact digests, and five frozen
   dependency pins from the typed `BuildManifest`;
2. compares the profile, host ABI, package format, raw-hash primitive, and
   schema epoch with their exact retained values;
3. returns the typed package/dependency `BuildError` arm with the presented
   profile bytes when any comparison fails;
4. invokes the aggregate codec, checker, lowerer, and package-builder leaf
   surfaces on the valid path;
5. assembles a typed `BuiltToolchain` record containing the builder output,
   the presented expected-digest vector, and `fixed_point = false`; and
6. returns that record through the typed success arm.

Two distinct closure byte strings produce distinct exact build results. This
is a bounded anti-copy fact: the driver does not return one cached artifact.
Changing any one of the five checked pins returns an exact typed error and
does not reach the assembly block. The current error payload carries the
presented profile field as bounded dependency context; it does not yet name
which non-profile pin failed.

## Retained component evidence

The successor graph retains an exact TestCase whose static manifest uses all
five correct dependency pins and expects the precise non-fixed-point build
record. The complete function inventory, contract, test, affected driver, and
required test pass `validate_contract_test_program`. The TestCase then
executes through an approved package bound to the same component root.

Pinned facts:

- graph operations: 34;
- `SLEYBC02` image bytes: 3,138;
- objects/entity bindings: 80;
- stored object bytes: 20,789;
- object-bundle SHA-256:
  `530a6df328131c4f6cc6179af84335a1c61fbcb2b93a980ef0ea2d41d70bd3f3`;
- component root:
  `e4ad2b891c0ccf72c64aceda5af2cb7bac55f2201a268776006083bcae61b4d9`;
- stored root bytes: 5,707;
- stored-root SHA-256:
  `4c5dffd8215b09bc2af5d3319f36172e75637484a36c1be7ff985aacf77d427b`;
- package digest:
  `49cf29999395d4950a09c484aea9afbd94d73ccb5e6823f7dcbb5a0056b5ddfe`.

The executable test pins every value above.

Validation commands:

- `cargo test -p sley-vm --test rw080_toolchain_graph`
- `cargo clippy -p sley-vm --test rw080_toolchain_graph -- -D warnings`
- `cargo fmt --all -- --check`

## Open RW-120 surface

The leaf functions remain the small RW-080 aggregate scaffolds. The builder
echoes the supplied closure bytes; the codec, checker, and lowerer return their
scaffold constants. The driver does not parse the object closure, resolve its
inventory, prove entry-point membership, compare the state-root field with its
executing root, compute or verify the presented artifact digests, propagate
the mature modules' typed errors, or rebuild its complete executable closure.

The expected-digest vector is carried into the result rather than proved, and
the fixed-point field is always false. Replacing the four leaf surfaces with
the retained RW-090/RW-100/RW-110 algorithms, resolving the whole dependency
closure, producing all toolchain packages, and comparing a second build remain
mandatory before RW-120 can pass. This component root is not `S`.

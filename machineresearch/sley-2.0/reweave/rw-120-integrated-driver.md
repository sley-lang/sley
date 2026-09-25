# RW-120 integrated driver invocation — provisional record

Status: PROVISIONAL C0 CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This successor to
the four-entry integrated component adds one Sley wrapper function that calls
all four constructed entry graphs in one admitted execution. It is not
canonical state root `S`, a fixed point, whole-toolchain reconstruction, or an
RW-120 pass.

## One reachable execution closure

`component::driver_fixture` starts from the retained codec, checker, lowerer,
and package-builder union. It appends a typed wrapper with 75 parameters. Four
`CallDirect` operations pass the exact existing test facts to the four real
entry functions, and one `TupleNew` returns their heterogeneous typed results.
The wrapper introduces no host callback and the frozen successor admission
gate reaches every function and every adapter import in the merged inventory.

The successor graph contains:

| Category | Count |
|---|---:|
| Functions | 101 |
| Parameters | 5,147 |
| Blocks | 1,847 |
| Operations | 4,054 |
| Constants | 699 |
| Adapter imports | 4 |
| Local entry points | 5 |

One `execute_approved_package_v2` invocation returns a four-field tuple equal
to the independently frozen expectations for:

- the exported four-leg codec schema decode;
- the seven-slice checker projection;
- the complete fact-fed function-image lowerer; and
- the populated `EXEC_PACKAGE_V2` builder.

The admitted executable image is 490,920 bytes. Its package digest is
`4516219fab750bef20d30714a6c033318379af534dba55b297210ad27793ea4f`.
The gate records 4,054 operations and 147 bridge uses.

## Pinned successor root

The wrapper is retained in a new component root with its fifth local entry
point. The pre-existing checker Contract and exact TestCase remain the root
anchors; wrapper behavior is pinned by the executable integration test.

- objects/entity bindings: 11,869;
- stored object bytes: 2,969,080;
- object-bundle SHA-256:
  `a9f4aaa72edbb0382df8dd134e1495599bc20d1c1052f842db0bf230978d07a4`;
- successor component root:
  `65b567be0c60ab007d57e4fd990f5c83d5f4c6a13aea6ce5eed03d20da4362f7`;
- stored root bytes: 783,784;
- stored-root SHA-256:
  `396cb4bf7ddb1911f2c3f49a68c38735156f02714ae8bde0b04209ebf3ac5c45`.

Validation commands:

- `cargo test -p sley-vm --test rw120_toolchain_integration integrated_driver_calls_all_four_real_programs_in_one_execution`
- `cargo test -p sley-vm --test rw120_toolchain_integration`
- `cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings`
- `cargo fmt --all -- --check`

## Remaining semantic boundary

The wrapper orchestrates four real entry graphs in one VM invocation, but its
inputs are prepared test facts and its outputs are collected into a tuple.
The codec output does not feed the checker, checker output does not drive the
lowerer, and lowerer output does not become the package-builder inventory.
The codec retains its RHW1 effect-validation boundary; the checker retains its
bounded fact projections; and the lowerer still consumes prepared canonical
facts. This evidence establishes direct-call integration and a closed admitted
execution closure. It does not establish the semantic pipeline, arbitrary
object-closure reconstruction, or self-hosting required by later RW-120 work.

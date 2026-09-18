# RW-110 bounded lowerer component — provisional record

Status: PROVISIONAL C0 CONSTRUCTION (2026-09-18, operator development
override `rw075_correction.operator_override_2026_09_07`). This component
retains the complete fact-fed image lowerer and populated execution-package
builder as canonical Sley object graphs. It is not arbitrary checked-closure
lowering, the RW-120 build driver, canonical state root `S`, self-hosting
evidence, or an R2 readiness claim.

## Retained programs

`complete_function_image_encoder` accepts the complete checked facts for a
root function and its canonically ordered transitive callees. Its Sley graph
performs semantic lowering, constructs the complete `SLEYBC02` root and callee
bodies, frames the image, and returns exact bytes. The retained test uses the
native direct-call corpus solely as an oracle: Sley receives facts for the
root and its callee and independently returns the exact reference image.

`package_builder` accepts a lowered image, canonical inventory rows, identities,
gate evidence, limits, cancellation data, and five host-mechanic digests. One
Sley invocation builds the constant, layout, import, dependency, global, and
contract sections and returns the complete `EXEC_PACKAGE_V2` envelope. The
root-bound execution test compares its result with the native encoder and
strictly hydrates the result back into the expected populated package.

The retained programs have these graph sizes:

| Program | Functions | Parameters | Blocks | Operations | Constants |
|---|---:|---:|---:|---:|---:|
| Complete image lowerer | 35 | 860 | 514 | 945 | 267 |
| Package builder | 26 | 827 | 407 | 661 | 179 |

Both use the same three frozen pure byte adapters. Those imports are retained
once in the component root.

## Canonical component root

Fixture identities are rewritten into contract-derived construction
identities, using separate category ordinal ranges beginning at 40,000 and
50,000. Every ownership edge, graph reference, type, constant, operand,
terminator, function immediate, and adapter type follows the same rewrite.
The component adds workspace/package/namespace metadata, two local entry
points, two Boolean contract-witness functions, one retained precondition, and
one exact lowerer TestCase.

The entire merged lowerer, builder, and witness closure passes the native
contract/test validator. The exact lowerer TestCase and the package builder
then execute through packages bound to the component's actual root. Every
entity object and the state-root record round-trips through its frozen decoder.

Pinned facts:

- objects/entity bindings: 4,738;
- stored object bytes: 1,177,725;
- object-bundle SHA-256:
  `8e4dfdd341d813db468fc0bb09d6b228ca07137dd9e34e7e524d6f183219ccc4`;
- component root:
  `b743296c43cf350b155c4aec445bf861ed6d81a09e23085764034df7c1aa952f`;
- stored root bytes: 313,038;
- stored-root SHA-256:
  `ddcd54f60a4fb9f96dc2d6173d6df23dc23e186217bf51ec140322729a51f741`.

The executable test pins all of these facts, including both SHA-256 digests
and the state-root identity.

Validation commands:

- `cargo test -p sley-vm --test rw080_lower_scaffold`
- `cargo clippy -p sley-vm --test rw080_lower_scaffold -- -D warnings`
- `cargo fmt --all -- --check`

## Open RW-110/RW-120 surface

The lowerer consumes already checked `CompleteFunctionFact` values. It does
not discover the transitive call graph from an arbitrary canonical program
closure, prove that the supplied callees are complete and canonically ordered,
or construct those facts from decoded state objects. Those closure-resolution
responsibilities belong to the RW-120 build driver and must compose with the
RW-090 codec and RW-100 checker rather than call a native compiler service.

The package builder likewise consumes already prepared canonical inventory
rows and mechanical digest results. A canonical driver-facing `BuildError`
vocabulary is not yet frozen. Exact profile conformance over the complete
bootstrap corpus and independent byte/error comparison remain required before
RW-110 can pass. This component root retains the bounded programs only; it is
not `S`.

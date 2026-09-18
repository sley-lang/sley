# RW-090 arbitrary all-kind body validation

Date: 2026-09-18

Status: implemented integration slice; RW-090 remains open on codec_main and component re-derivation

## Result

The all-kind program decoder now has an arbitrary-schema variant. Where the
bounded aggregate (`rw-080-codec-all-kind-digest-dispatch.md`) judged a
decoded body for kinds 1 through 17 by comparing its digest to one pinned
representative, the new image routes the declared kind to that kind's
arbitrary schema decoder and forwards the decoder's own refusal code. The
envelope validation, outer record projection, declared-kind range check,
DependencyBinding route (kind 18), and six-field result contract are
unchanged; the bounded image and its retained evidence are untouched.

Every schema decoder is built once over one shared primitive closure (uvar,
record, union, list, fixed identity, identity collections, exact and bounded
varints, one- to six-field record projections, the recursive `TypeExpr`
chain, type-parameter lists, and the empty-payload check) under one
sequential namespace allocator that skips the host image's reserved
identities. Kinds 1 (Workspace), 2 (Package), 3 (Namespace, with its optional
parent), 16 (EntryPoint), and 17 (PolicyBinding) gained arbitrary
simple-record schemas in this slice; the remaining kinds compose the recipes
recorded in today's per-kind slices.

## Executable surface

The image contains 108 reachable functions, 5,790 parameters, 1,701 blocks,
3,034 operations, and 137 deduplicated constants. Its encoded image is
397,266 bytes. The approved package digest is
`89eb112f7c574f1fbd6b18f34dbf4c15849ca40ac91537c811c327aa9544af61`
under the declared codec-profile execution limits (100,000 instructions,
10,000,000 fuel, 100,000,000 value units, 10,000,000 output units).

## Measured evidence

All 18 representative native objects of the bounded aggregate decode through
the arbitrary image with byte-identical results, and 32 further rich objects
decode that the bounded aggregate refuses: nonempty Workspace, Package,
Namespace, and PolicyBinding sets, a Protocol entry point, both TypeDef
forms, a full Function, a maximal Parameter, all five Block terminators, all
seven Operation immediates, nested constants in Constant and
CapabilityRequirement, a Contract with and without limits, both TestCase
environments, and a full AdapterImport. Peak cost is the 862-byte replay
TestCase at 656,148 fuel, 72,981 instructions, and 35,625,029 value units.
Representative-object decodes cost between 21,530 and 63,378 fuel.

Refusals forward unchanged: unordered Workspace packages (`SCB_MAP_ORDER`),
an unknown TypeDef form and a declared-kind mismatch (`SCB_UNION_INVALID`),
an invalid nested constant boolean (`SCB_BOOL_INVALID`), a missing Workspace
field (`SCB_FIELD_MISSING`); declared kinds 0 and 19 return
`SSMC_ENTITY_KIND_UNKNOWN` before any body work.

```text
cargo test -p sley-vm --test rw120_toolchain_integration arbitrary_dispatch -- --nocapture
cargo test -p sley-vm --test rw120_toolchain_integration
cargo test -p sley-vm --test rw080_codec_program_outer
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
```

## Remaining for RW-090

- Route `codec_main` selector 0 to the arbitrary image and re-derive the
  canonical codec component (`rw-090-canonical-codec-objects.md`) and its
  manifest over the larger closure.
- The program encode leg (selector 1) remains the bounded witness emitter.
- Instruction cost near 90 per body byte bounds a single decode to roughly
  1.1 KiB of body under the current profile; larger objects need either a
  profile decision or cheaper byte handling.
- Independent review and acceptance are unchanged.

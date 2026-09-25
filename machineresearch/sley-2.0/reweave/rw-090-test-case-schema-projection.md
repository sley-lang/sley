# RW-090 TestCase schema projection

Date: 2026-09-18

Status: implemented bootstrap slice; every entity kind now has an arbitrary schema decoder; RW-090 remains open on dispatch integration

## Result

The Sley codec now validates an arbitrary canonical `TestCase` body (kind
14), the last entity kind without an arbitrary schema decoder. It composes
the recursive `ConstValue` closure with the existing record, list, closed
union, and exact-width validators:

- an exact target identity;
- an ordered list of recursive input constants;
- the closed `EffectEnvironment` union: replay bindings (adapter identity,
  request constants, and a two-arm `ResultConst` response) or deterministic
  adapter configurations (adapter identity and a configuration constant);
- the closed `ExpectedOutcome` union: a recursive value or a 32-bit failure
  code;
- an ordered list of expected observations (32-byte observation identity and
  a constant); and
- all six exact 64-bit `ResourceLimits` fields.

With this slice, arbitrary schema decoders exist for all eighteen entity
kinds: Workspace, Package, Namespace, Function, Parameter, EntryPoint,
PolicyBinding, and DependencyBinding from the earlier RW-080/RW-090 program
work; TypeDef, Block, Operation, Constant, GlobalValue, EffectDef,
CapabilityRequirement, Contract, TestCase, and AdapterImport from today's
schema-projection slices.

## Executable surface

The image contains 46 reachable functions, 2,095 parameters, 676 blocks,
1,282 operations, and 410 constants. Its encoded image is 156,448 bytes. The
approved package digest is
`c0e925c0a8799feb6e3585083c851442512be4c616dcb979ecc5d32bdff7f61a`
under the declared codec-profile execution limits.

## Validation

Positive fixtures cover a replay environment with both response arms and a
value outcome, and an adapter environment with a maximum failure code, both
with nested composite input constants, an observation, and a maximum fuel
limit. Negative coverage includes wrong entity kind, missing and unknown
fields, a short target, a malformed nested input leaf, an unknown environment
tag, an unknown response tag, a malformed replay request constant, an adapter
configuration missing its constant, an unknown outcome tag, a 33-bit failure
code, a malformed outcome value, a short observation identity, and an
incomplete limits record.

```text
cargo test -p sley-vm --test rw120_toolchain_integration test_case_schema_decoder
cargo clippy -p sley-vm --test rw120_toolchain_integration -- -D warnings
```

## Remaining for RW-090

The per-kind decoders built today (kinds 4, 7, 8, 9, 10, 11, 12, 13, 14, 15)
are standalone images. Wiring them into the all-kind program dispatch, and
re-deriving the codec component manifest over the integrated closure, is the
remaining RW-090 work before independent review.

# RW-080 §1.2 checker slice 5: Sley-owned single-effect closure

Status: PROVISIONAL C0 CONSTRUCTION. This is an executable Sley effect-checker
slice under the operator development override. It is not RW-100 completion,
C1, self-hosting evidence, or runtime authority.

## Scope and behavior

`single_effect_closure_checker` accepts runtime presence and identity facts for
one optional effect definition, one optional declared effect, and one optional
effect request. Its admitted projection resolves the declared identity first,
then the requested identity, and finally compares the declared set with the
least local closure. Success returns the closure size, direct-call edge count,
closure rounds, and charged closure work.

The checker returns the frozen S20-230 codes used by the native phase:

- `EFFECT_UNRESOLVED_ENTITY` (`23000`) when a declared or requested identity
  does not resolve to the single definition; and
- `EFFECT_CLOSURE_MISMATCH` (`23003`) when the resolved declaration and local
  request differ.

This slice uses actual Sley branches, equality operations, tuple construction,
and typed `Result` construction through the admitted v2 package path. It does
not call a native semantic service.

## Native parity and negative corpus

`native_single_effect_closure` independently constructs the corresponding
`FunctionUnit`, optional `EffectDefinition`, and optional `EffectRequest`, then
invokes `sley_check::effects::validate_effect_program`. Tests compare the empty
and one-effect success reports and repeat each Sley execution for determinism.
The negative corpus covers absent definitions, mismatched declaration and
request identities, both closure-mismatch directions, and a multi-fault case
that pins declaration-resolution precedence over request resolution.

All twelve tests in `rw080_checker_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

This slice covers at most one definition and one function with no calls,
adapters, capabilities, or contracts. Arbitrary sorted effect inventories,
duplicate and wrong-kind detection, call-graph closure propagation, request
type judgments, adapter and capability validation, resource ceilings,
arbitrary decoded program values, contracts, and mandatory test planning
remain RW-100 work. RW-080 and R2 stay provisional pending the recorded
independent acceptance debt.

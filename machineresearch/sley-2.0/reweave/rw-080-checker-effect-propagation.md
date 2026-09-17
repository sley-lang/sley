# RW-080 §1.2 checker slice 7: Sley-owned direct-call effect propagation

Status: PROVISIONAL C0 CONSTRUCTION. This is an executable Sley effect-checker
slice under the operator development override. It is not RW-100 completion,
C1, self-hosting evidence, or runtime authority.

## Scope and behavior

`two_function_effect_closure_checker` models two functions and an optional
direct call from the first to the second. Runtime facts select each function's
local request and declared closure plus the call edge. Sley computes the least
callee-to-caller propagation with `BoolOr`, compares the caller declaration
first and the callee declaration second, and returns the total closure
memberships, direct-call edge count, convergence round, and charged work.

The summary reproduces the native fixed-point accounting for every bounded
state: initial local memberships, the call-edge and copied-member charges, and
the additional fixed-point confirmation charges when propagation changes the
caller. Any declaration mismatch returns `EFFECT_CLOSURE_MISMATCH` (`23003`).
All judgments execute in the admitted Sley image; the native validator is used
only by the test oracle.

## Native parity and exhaustive corpus

`native_two_function_effect_closure` independently constructs two complete
`FunctionUnit` values, optional `EffectRequest` operations, the optional
`CallDirect` edge, and one effect definition before invoking
`validate_effect_program`. The differential test enumerates all 32 states of
the five Boolean inputs. It compares every accepted report field or exact
rejection code and repeats every Sley execution to prove determinism.

All fifteen tests in `rw080_checker_scaffold` pass; focused Clippy with warnings
denied is clean.

## Explicit remainder

This slice has one possible effect identity, two functions, and one acyclic
edge. Arbitrary call-graph inventories, cycles and multi-round convergence,
multiple effect identities, direct-call type failures, adapters, capabilities,
contracts, resource maxima, arbitrary decoded program values, and mandatory
test planning remain RW-100 work. RW-080 and R2 stay provisional pending the
recorded independent acceptance debt.

# Optional residual authoring

Full planning, fill, provenance and fragment contracts: `help residual-reference`.

Use ordinary AF1-X or these fragments as appropriate. `residual try request.json`
expands explicit decisions and uses the same compiler/kernel as ordinary `try`.
It returns a candidate handle or obligations, never commits. `--no-test` is supported.
A request has `residual:1`, `base:"current"` (or exact `dN@rK`), `operation`,
`fragment:{"id":NAME,"version":1}`, `scope`, and `bindings`. Named types must
already exist in the accepted program or selected draft. Preserve the task's
API, scope and behavior; fragment availability does not authorize extra changes.

## Construct or replace one function

`operation:"derive"` and `scope:[FUNCTION]`. Bindings always include `params`
and `returns`. These are complete authoring choices, not inferred intent.

- `checked_pipeline`: ordered `[name,expression]` steps; explicit
  `arithmetic_failure` (an error case name or `{"propagate":true}`),
  `rounding:"toward_zero"`, and final `result`. Each arithmetic operation is
  checked in order; the result is wrapped in Ok. Example:

```json
{"residual":1,"base":"current","operation":"derive","fragment":{"id":"checked_pipeline","version":1},"scope":["scaled_half"],"bindings":{"params":[["x","i64"]],"returns":"Result<i64,ArithmeticError>","steps":[["scaled",["mul","x",3]],["answer",["div","scaled",2]]],"arithmetic_failure":{"propagate":true},"rounding":"toward_zero","result":"answer"}}
```

- `ordered_guard_chain`: `guards:[{"when":EXPRESSION,"fail":["fail","Case"]},...]`
  in precedence order; the first true guard exits. `success` is either
  `{"ops":[...],"term":["ok",VALUE]}` or a nested
  `{"fragment":{"id":"checked_pipeline","version":1},"bindings":{...}}`.
  Nested fragments inherit the enclosing signature; state their remaining decisions.
- `typed_branch_result`: `input`, exact variant `cases`, and `join`. Each case
  has `case`, `payload` (null or `[name,type]`), `ops`, and `values` passed to
  the join. Join has `params`, `ops`, `term`. See `help residual-reference` for details.

## Change selected arithmetic literals

`operation:"edit"` with `checked_pipeline@1` can change exact integer constant
loads feeding checked arithmetic. It inherits their widths and preserves other
graph entities. Get exact operation names from a focused view; scope is not a
function wildcard. A generic example is:

```json
{"residual":1,"base":"current","operation":"edit","fragment":{"id":"checked_pipeline","version":1},"scope":["adjust.entry.amount"],"bindings":{"lens":"integer_literal","value":7,"overrides":{}},"preserve":{"outside_scope":true,"boundaries":true}}
```

The value is a default across selected loads; `overrides` maps exact selected
names to exceptions. Every use of a selected load sees its changed value.
Other edit shapes, arbitrary constant returns and loops use ordinary AF1-X.
Small edits can be shorter in ordinary AF1-X. Batched edits share the explicit
default; compare full interaction cost. Replies preview 16 target names/values
with counts. `residual show dN@rK --targets` lists every saved target's name,
width, previous and requested value without full provenance. The view is historical,
not a new verification. `--provenance` retrieves full source evidence.
`residual plan`/`fill` support explicit missing decisions and finite choices;
`help residual-reference` describes their contracts. No missing policy is guessed.

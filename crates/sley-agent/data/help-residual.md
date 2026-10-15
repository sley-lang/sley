# Experimental residual authoring (GHOSTWEAVE)

`residual try request.json` expands explicit decisions into AF1-X/AF1 and uses
the ordinary compiler, kernel, draft and test path. Options: `--no-test`,
`--all-tests`, `--public cases.json`, `--raw`, `--verbose`. It never commits.
The workspace must already have an accepted head.

Use `base: "current"`, an exact accepted state root, or an exact draft
revision such as `d1@r2`. A draft base layers onto that frame and creates
its next revision. Changed heads, names, policies or base revisions refuse.
There is no implicit rebase.

Example request (save as request.json):

```json
{"residual":1,"base":"current","operation":"derive","fragment":{"id":"checked_pipeline","version":1},"scope":["scaled_half"],"bindings":{"params":[["x","i64"]],"returns":"Result<i64,ArithmeticError>","steps":[["scaled",["mul","x",3]],["answer",["div","scaled",2]]],"arithmetic_failure":{"propagate":true},"rounding":"toward_zero","result":"answer"}}
```

This formula retains intermediate overflow and truncates signed division
toward zero. The author explicitly propagates native ArithmeticError payloads.
It is not an algebraic simplification of another formula.

All three fragment families have version 1. Derive bindings require
`params` and `returns`; `scope` names exactly one function. Named types
must already exist at the head or in the selected base draft.
Declared interfaces are checked before expansion; conflicts name the authored
bindings. `--provenance` retains the interface report and its deferred checks.
Known expression connections, local binding order, branch payloads/join values,
failure routes and callee effects are checked before fragment expansion. Missing
ordinary literal contexts or unresolved fragment expressions require explicit
typed authoring or ordinary AF1-X escape. Full program validity and ordinary
source-body obligations still use the compiler/kernel path.

- `ordered_guard_chain`: ordered `guards` with `when` and `fail`
  terminators, then `success`. The first true predicate fails.
  Success is a nested `{"fragment":...,"bindings":...}` application
  or an explicit `{"ops":[...],"term":[...]}` terminal region.
  Terminals are return/ok/fail/trap, with no named block targets. Trap accepts
  an optional code (default unreachable) and optional persistable payload.
- `checked_pipeline`: ordered `[name, expression]` steps, explicit
  `arithmetic_failure` (error variant case name or `{"propagate":true}`),
  `rounding: "toward_zero"`, and final `result`.
  Every arithmetic node is checked; the final result is packaged with ok.
  Integer opcode spellings follow AF1-X: short mnemonic, long SSMC1 name or
  decimal tag (64–71). Write unmarked operations; the explicit failure policy
  supplies their checked route.
- `typed_branch_result`: `input`, `cases`, `join`. A case supplies
  `case`, `payload` (null or [name,type]), `ops`, and join `values`.
  The join supplies `params`, `ops`, and a return/ok/fail `term`.
  Directly named inputs require exact case coverage and matching payloads before
  expansion. Computed inputs stay compiler obligations. No fallback is invented.

Sparse integer edits use checked_pipeline@1 against the accepted graph or an
explicit latest kernel-valid draft revision:

```json
{"residual":1,"base":"current","operation":"edit","fragment":{"id":"checked_pipeline","version":1},"scope":["adjust.entry.amount"],"bindings":{"lens":"integer_literal","value":7,"overrides":{}},"preserve":{"outside_scope":true,"boundaries":true}}
```

Scope names 1–64 exact integer constant loads that directly feed checked
arithmetic. Integer width is inherited. The default value applies to each
load; optional overrides map exact scope names to exceptional values.
Alternatively use `{"lens":"integer_literal","values":{"adjust.entry.amount":7}}`:
one value per scope member, without `value` or `overrides`. Missing `values`
entries become separate decision paths; use `values:{}` to request every site.
Separate paths do not establish independence for factoring.
A changed load affects every use of its value. Shared old constants remain
unchanged; only selected loads and newly needed constants may change.
Function signatures, effects, control flow and all other existing entities
must remain byte-identical. Static callers may behave differently even though
their bytes remain unchanged; the result reports this boundary impact.
For a draft edit, set base to `dN@rK`. The source candidate is validated,
edited with its original identities/create order, and independently validated
again. A changed edit records the next revision through the ordinary test and
publication path. Retained source syntax, tests, tables and imported-test
provenance survive; tests may fail when their expected outputs no longer match.
Frame statements and a replay against the accepted head must agree with the
validated graph. Changed edits make leftover constants explicit as inherited
declarations with source-object provenance, then verify complete retained-frame
coverage. Original-source reports retain any omissions; historical authorship
is not inferred. No-op edits leave the original frame untouched.
No-op edits run requested checks on the source graph without a new candidate or
draft revision. Complete draft plans report their actual validation; closed
relations validate every row without pruning. Incomplete/refused source drafts,
some Ripple-generated sites and other edit shapes remain
unsupported.

For finite per-site choices, `values:{}` may use this explicit contract:

```json
{"choices":{"version":2,"contract":"author_closed_constraints","constraints":[{"fields":["/bindings/values/adjust.entry.amount"],"rows":[{"/bindings/values/adjust.entry.amount":3},{"/bindings/values/adjust.entry.amount":7}]}],"dependencies":[]}}
```

Tables define the complete allowed conjunction; use exact missing-site paths.
The planner adds mandatory typed graph dependencies and checks every integer
width. Independent components avoid a global product; coupled components over
256 rows refuse. Question `declared_values` remain subject to all tables.
No test prunes alternatives. With an explicit valid draft base, the stored source
candidate is validated before dependency analysis; its created entities are part
of the graph. Source validation is reported separately. Only the selected
completion compiles and runs the kernel. Use `show rN@1 --decisions` for the original constraint contract
and `--provenance` for dependency/reconstruction evidence. Counts and JSON byte
costs are not provider tokens or measured savings.

Equal typed values produce no candidate or draft. A local rN@1 record keeps
the request and preservation evidence. Accepted-base kernel remains not_run;
draft no-ops report their source/composed validation and select inherited draft
tests. Public cases run if supplied, and --all-tests selects all tests in the
source graph. Accepted bases select no checks without either option;
--no-test skips execution. These records support the
same residual show views as draft revisions.

Inspect a returned revision:

```text
sley-agent residual show d1@r1
sley-agent residual show d1@r1 --expanded
sley-agent residual show d1@r1 --decisions
sley-agent residual show d1@r1 --provenance
```

For a saved integer-literal edit, `residual show <reference> --targets` lists
all target names, widths, previous/requested values and changed flags without
full provenance. It is a historical projection, not a new verification; plans
and non-literal edits refuse this view.

Results separate construction, kernel validity, public checks and native
admission. Zero checks say `zero_ran`; skipped checks say `not_run`.
External check success never supplies native admission evidence. Full
request, binding, expansion and source maps stay with the atomic draft.
Ordinary `draft`, `fill`, `explain`, `call`, and `submit` still work.
No-op default summaries mark omitted check arrays and keep failures/counts.
`residual show <rN@1> --provenance` also retrieves the saved full checks without
rerunning them; `--verbose` on the original try includes all check details.

Missing fragment fields produce a bound plan without creating a candidate.
Without a choices contract, the result lists every unresolved path and the
relevant fragment contract. Even the sole supported rounding policy requires
an explicit author choice. Closed relations are described below.
To preview a request, use plan and inspect the saved plan with show. These
steps do not publish a candidate or draft revision. Planning may validate
source and composed candidates through the kernel:

```text
sley-agent residual plan request.json
sley-agent residual show r1@1 --decisions
```

Then fill the plan to run the ordinary trial; it may create a candidate or
draft revision:

```text
sley-agent residual fill r1@1 decisions.json
```

Requests without `choices` share a 250 ms planning deadline through preparation;
finite choices use the aggregate two-second frontier budget. Startup hashing,
initial envelope parsing and ordinary trials are outside that planning clock.
`--provenance` shows the route and actual ceiling. On exhaustion use ordinary
AF1-X; no partial residual plan or candidate is published by the planner.

For a plan missing only rounding, decisions.json contains:

```json
{"residual":1,"plan":"r1@1","choose":{"/bindings/rounding":"toward_zero"}}
```

Answer exactly the listed missing fields in one fill. Existing authored
values cannot be overwritten. A complete preview accepts an empty choose
object. The filled request uses ordinary expansion, validation and drafts;
try options that led to the plan carry forward. A changed source requires
explicit replanning. A plan permits one trial attempt, including simultaneous
fills; after an attempted or interrupted fill, inspect its drafts or replan.
Malformed answers do not consume the plan. No automatic second question round
is started. Expanded output is unavailable while required decisions remain.

Optional `choices` declares the complete allowed relation over missing fields:

```json
{"choices":{"version":1,"contract":"author_closed_relation","rows":[{"/bindings/params":[["x","i8"]],"/bindings/returns":"Result<i8,ArithmeticError>","/bindings/rounding":"toward_zero"},{"/bindings/params":[["x","i64"]],"/bindings/returns":"Result<i64,ArithmeticError>","/bindings/rounding":"toward_zero"}]}}
```

Add this member to the example request after removing params, returns and
rounding from its bindings. Every row must supply exactly all missing fields.
This is an author constraint defining the permitted programs. Sampled
candidates, duplicate/partial rows and authored-field overwrites refuse.
Plans compile every row in the bound context; one invalid row refuses the
entire declaration. Tests never filter it. Compilation does not imply kernel
validity or correctness for your task.

A deterministic greedy frontier asks only distinguishing field questions and
discloses their full value domains. In this example, one signature answer
selects a row; rounding follows from the authored relation. Answer exactly
the plan's listed questions. A literal singleton needs no question round.
Without that declaration, missing policies still require explicit answers.
Selection uses descriptor JSON bytes as an additive proxy, not billed tokens.

The full relation is omitted from the compact report. Read `show --decisions`
to recover it before answering in a new context. `show --provenance` exposes
reconstruction and links derived fields to their actual contract and answers.

Alternatively, `choices` version 2 uses `contract:"author_closed_constraints"`,
a `constraints` array of `{fields:[paths],rows:[objects]}` tables and a required
`dependencies` array (empty is allowed). Tables are conjoined exactly. It supports
per-target integer edits and all three derive fragments. A derived function's
missing fields stay in one conservative component, bounded to 256 joined rows.
Each joined completion passes the existing partial interface preflight; one
conflict refuses the family. Compiler/kernel checks still apply to the selected
completion. Draft bases require an exact valid source revision. No supplied
dependency list proves independence or authorizes row filtering.

Planning supports up to 64 missing fields and 256 literal rows. One shared
wall/work/CPU budget covers analysis through preparation; exhaustion refuses
before saving a plan or consuming a fill. Resource observations are available
through `show --provenance`. Checks occur between bounded steps, so a step
may cross the deadline before refusal. Startup executable hashing and the
ordinary candidate trial are outside that planning budget. The instrumented binary
checks an additional working-memory ceiling across allocations, allocator
cache, stacks and mappings during preparation. Its conservative upper bound may
refuse before the actual resident increase reaches 256 MiB. A step or allocation
can exceed the ceiling before a checkpoint: this is not an instantaneous OS
memory cap. An unavailable observation refuses with ordinary AF1-X escape;
library callers without the CLI observer remain explicitly unobserved. Startup,
persistence and ordinary candidate trial remain outside this planner bound.
Further lenses remain future work. No measured cost-reduction or release claim
is made.

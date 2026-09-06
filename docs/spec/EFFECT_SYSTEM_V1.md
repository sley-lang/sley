# Effect System v1

Status: S20-230 normative specification.

This contract freezes deterministic static effect closure and scope typing for
the SSMC1 epoch-1 entities and opcodes frozen by S20-200. It consumes the
S20-210 type judgment and S20-220 CFG/value-use judgment. It never converts an
earlier failure into success.

This phase proves that effectful operations are explicit, exactly declared,
and statically scoped. It does not issue or authenticate capability tokens,
read a protected policy root, authorize an execution, invoke an adapter, judge
contract predicates, lower bytecode, execute a VM, or mutate repository state.
Those remain S20-240, S20-280, S20-370, S20-380, and later packages.

## 1. Closed model

### 1.1 Effect kinds

| Tag | Kind |
|---:|---|
| 1 | `StdoutWrite` |
| 2 | `StderrWrite` |
| 3 | `FileRead` |
| 4 | `FileWrite` |
| 5 | `ClockRead` |
| 6 | `RandomRead` |
| 7 | `EnvironmentRead` |
| 8 | `AdapterCall` |

There is no network, process, shell, secret, deployment, spend, or ambient
effect in epoch 1.

### 1.2 Effect definitions

An `EffectDef` contains its `EntityId`, one closed `EffectKind`, exact closed
`scope_type`, `request_type`, `response_type`, and `failure_type`, and its
visibility. All four types must pass S20-210 with zero free type parameters.
The scope type must be persistable because capability allowlists contain
canonical constants of that exact type. Request, response, and failure types
need not be persistable; opaque adapter handles remain permitted where later
runtime rules support them.

### 1.3 Capability requirements

A `CapabilityRequirement` contains its `EntityId`, one exact `EffectDef`
identity, an ordered canonical allowlist of scope constants, and a raw-ID
sorted set of constraint-contract identities.

Every allowed scope:

- passes S20-210 constant judgment, preserving its exact `TYPE_*` failure;
- has a declared type exactly equal to the linked effect's `scope_type`;
- is strictly ordered by the complete structural constant order in Appendix A;
- is unequal to every earlier scope.

The contract identities must resolve to entities decoded as SSMC1 kind 13 and
be raw-ID sorted and unique. S20-230 establishes that boundary only. S20-240
owns contract shape, binding, and predicate judgment.

An empty allowed-scope list is valid static data and grants nothing at runtime.

### 1.4 Adapter imports

An `AdapterImport` contains its `EntityId`, stable 32-byte adapter identity,
ABI version, exact closed request/response/failure types, and a raw-ID sorted
effect set. S20-230 does not interpret the adapter identity or ABI version.

An import carrying effects requires the effect set to contain
exactly one `EffectDef`, and that definition's kind must be `AdapterCall`.
The adapter scope type is that definition's `scope_type`.

This cardinality rule resolves an otherwise unsafe schema ambiguity: the
frozen `adapter_invoke` opcode has one scope operand, while `AdapterImport`
stores a set without per-effect scope bindings. An effect set that does not
contain exactly one `EffectDef` is unsafe in general: multiple effects could
not be scoped unambiguously, and an empty set could hide authority. Section
1.5 admits exactly one narrow exception — registered pure deterministic host
primitives with frozen value-mechanic shapes — and every other empty-set use
fails at invocation (§3.3) or at the positive-registration gate downstream.
A future schema epoch may add explicit per-effect scope bindings; epoch 1
does not infer or merge them.

### 1.5 Registered pure deterministic host primitives (owner amendment A1)

Authority: operator / S20-230-owner decision 2026-09-06, RW-050 §8 option
(i), narrowly tied to G-10 bootstrap byte transport and construction. This
section does not authorize arbitrary effectless imports, a general FFI,
native compiler services, or any weakening of the effect system. It does not
authorize full S20-230 effectful lowering; that remains its separately owned
full-Machine-Genesis obligation. Extension to any further primitive needs a
new owner amendment, not an analogy to this one.

Background: the RW-030 G-10 admission selected `adapter_invoke` entries with
empty effect sets, which the §1.4 rule above failed closed with
`ADAPTER_EFFECT_CARDINALITY` (verified: Ariadne round 3, RW-050 slice 1
negative result, preserved). The zero-effects rationale — an empty set could
hide authority — attaches to imports that could carry authority. A pure value
function over caller-owned values carries none: no scope to bind, no host
state to touch, no capability to confine. This section distinguishes that
case by frozen shape, keeping fail-closed behavior for everything outside
the three shapes below. No `AdapterCall` effect is manufactured to satisfy
the old rule: pure primitives are not mislabeled as external effects.

An `AdapterImport` with an empty effect set is a pure-primitive candidate.
S20-230 judges the candidate's form at two points. At declaration (step 4)
the request/response/failure triple must equal one frozen row shape —
`P-BYTES-FROM` / `P-BYTES-TO` triples, or the `P-PUSH` relationship
(response exactly `Vector` of the request type) — with failure exactly
`BuiltinFailure(Index)`; any other triple fails with
`ADAPTER_INVOKE_TYPE`, invoked or not. At invocation (step 6) the
scope/operand/result binding must match §3.3. The candidate becomes
callable only through positive registration: its exact identity and version
must be registered in the approved host-boundary/import profile (RW-050
permitted-import entries, RW-070 import manifest). A missing, unknown, or
unregistered zero-effect import fails closed at the registration gate
(lowering refuses the immediate); S20-230 acceptance of its form never
executes it.

The closed pure shapes (frozen; the only G-10-admitted value mechanics):

- `P-BYTES-FROM`: request `Bytes`, response `Vector<UInt(8)>`. Invocation
  scope operand: `Unit`.
- `P-BYTES-TO`: request `Vector<UInt(8)>`, response `Bytes`. Invocation
  scope operand: `Unit`.
- `P-PUSH`: request `E`, response `Vector<E>`, for any closed `E`
  (genericity by monomorphization: each concrete use declares its own
  closed row; rows with free type parameters fail closed type judgment).
  Invocation scope operand: `Vector<E>`, exactly equal to the response
  type. The response is always exactly `Vector` of the request type; no
  other request/response relationship is pure.

`UInt(8)` is `UInt` with 8-bit width. In every pure shape the failure type
is exactly `BuiltinFailure(Index)` (capacity refusal lives in the
collection-bounds family; kinds are epoch-closed). The invocation scope
operand binds no authority: `Unit` for the conversions (stateless), the
acted-upon vector itself for push.

A zero-effect row satisfying the owner's validity conditions only through
these shapes: exact registered identity and version; frozen typed input and
output schemas; deterministic behavior on identical canonical inputs; no
filesystem, network, clock, randomness, environment, mutable-host-state,
secret, capability, policy, process, or other external effect; no ability
to call or select arbitrary native functions; bounded allocation, input
size, output size, recursion, and work; fuel/resource charge before
externally observable resource consumption per the accepted accounting
contract; typed deterministic failures; fixture/conformance-tested
semantics. Registration, bounds, charges, and tests live in the owning
profile/lane artifacts (RW-050 freeze, RW-070 manifest, conformance
vectors); S20-230 owns the shape rule stated here.

Semantic authority boundary: pure primitives provide primitive value
mechanics only. They must not perform or delegate program/schema-aware SCB
validation or canonical judgment, SSMC formation or validation, reference
or identity judgment, type or CFG checking, effect or contract judgment,
mandatory test selection, Witness integrity/discharge judgment, semantic
lowering, compiler-image assembly, candidate validation, or policy
judgment. Those responsibilities remain Sley-owned under REWEAVE SH2; a
primitive doing so invalidates SH2, not just its use.

## 2. Validation request boundary

One effect-program request supplies:

- a raw-ID-sorted complete set of function units, each containing one
  `FunctionGraph` and its complete parameter, block, and operation inventory;
- raw-ID-sorted complete sets of `EffectDef`, `CapabilityRequirement`, and
  `AdapterImport` entities;
- the raw-ID-sorted set of identities already decoded as Contract entities;
- the S20-210 `TypeEnvironment` selected for the same root and schema epoch.

All entity identities in the request are globally distinct. Every direct-call
target must be present in the function set. The request is closed: lookup never
falls back to another root, repository, cache, label, path, host registry, or
latest version.

S20-230 model records belong to `sley-ssmc`; their deterministic judgment
belongs to `sley-check`. `sley-policy` remains the owner of protected policy
roots and runtime capability validation and is not created or populated by
this package.

## 3. Relevant operation rules

S20-230 scans operations in function-ID, function block-list, and operation
ordinal order. It interprets only four frozen opcodes after the complete input
has passed S20-210/S20-220:

### 3.1 `call_direct`

- the immediate is exactly `Function(target, type_arguments)`;
- `target` resolves to one function in this request;
- type-argument count equals the callee type-parameter count;
- every type argument is well formed in the caller's parameter scope;
- operands exactly match the instantiated callee parameter types;
- there is exactly one result and its type equals the instantiated callee
  result type.

The call graph contains one edge from caller to target. There are no indirect,
label, address, host-symbol, or fallback calls.

### 3.2 `effect_request`

- the immediate is exactly `Entity(effect)` and resolves to one `EffectDef`;
- there are exactly two operands and one result;
- operand 0 has exactly the effect scope type;
- operand 1 has exactly the effect request type;
- the result type is exactly
  `Result<effect.response_type,effect.failure_type>`.

The referenced effect is a local direct effect of the enclosing function.

### 3.3 `adapter_invoke`

An effectful `adapter_invoke` names an import carrying effects:

- the immediate is exactly `Entity(adapter)` and resolves to one
  `AdapterImport` with exactly one effect;
- there are exactly two operands and one result;
- operand 0 has exactly the sole `AdapterCall` effect's scope type;
- operand 1 has exactly the adapter request type;
- the result type is exactly
  `Result<adapter.response_type,adapter.failure_type>`.

The adapter's sole effect is a local direct effect of the enclosing function.

A pure `adapter_invoke` names a pure-primitive candidate (§1.5) whose
declared triple already satisfies one frozen row shape:

- the immediate is exactly `Entity(adapter)` and resolves to one
  `AdapterImport` with an empty effect set;
- there are exactly two operands and one result;
- the adapter failure type is exactly `BuiltinFailure(Index)`;
- operand, scope, request, and response types match exactly one frozen
  pure shape: `P-BYTES-FROM` (scope `Unit`, request `Bytes`, response
  `Vector<UInt(8)>`), `P-BYTES-TO` (scope `Unit`, request
  `Vector<UInt(8)>`, response `Bytes`), or `P-PUSH` (scope exactly the
  response type, response exactly `Vector` of the request type);
- the result type is exactly
  `Result<adapter.response_type,adapter.failure_type>`.

A pure invocation contributes no local effect to the enclosing function.
Every other empty-effect invocation fails with `ADAPTER_INVOKE_TYPE`.

No adapter is invoked during static judgment.

### 3.4 `capability_narrow`

- the immediate is exactly `Entity(requirement)` and resolves to one
  `CapabilityRequirement`;
- there are exactly two operands and one result;
- operand 0 is exactly `CapabilityToken(requirement EntityId)`;
- operand 1 has exactly the linked effect scope type;
- the result is exactly
  `Result<CapabilityToken(requirement EntityId),BuiltinFailure(CapabilityFailure)>`.

Narrowing is a local authority-shaping operation and adds no external effect to
the function closure. This phase does not prove the requested runtime value is
within an allowlist and does not validate issuer, principal, root, nonce,
expiry, signature/MAC, adapter identity, budget, replay, or current policy.
S20-370/S20-380 own those fail-closed checks.

Every other opcode remains outside S20-230 semantic-signature judgment. Its
presence neither creates an effect nor causes this phase to claim the opcode
is otherwise executable.

## 4. Exact least effect closure

For each function, `local_effects` is the raw-ID set referenced by its valid
`effect_request` and effectful `adapter_invoke` operations. A pure
`adapter_invoke` (§3.3) contributes no local effect, so `Function.effects`
needs no declaration for it; declaring an effect for a pure invocation is
compared exactly like any other declaration (an unjustified declaration
fails closure comparison).

The computed closure is the least fixed point:

```text
closure(f) = local_effects(f) union union(closure(g) for each direct call f -> g)
```

Construction begins with only `local_effects`, then visits functions and call
edges in raw-ID order until no set grows. Legal recursive/self-recursive call
cycles therefore terminate. An effect that is merely declared around a call
cycle but has no reachable local request is not in the least fixed point and
cannot justify itself.

`Function.effects` must be raw-ID sorted and exactly equal to the computed
closure. Missing effects and unused over-declarations both fail. The checker
never inserts, removes, sorts, or repairs a declaration.

Raw-ID ordering and uniqueness are part of the consumed S20-220 graph
judgment. A duplicate or unsorted `Function.effects` list therefore preserves
that earlier exact `GRAPH_INVENTORY_MISMATCH`; it never reaches S20-230 closure
comparison. `EFFECT_CLOSURE_MISMATCH` applies only after that earlier
canonical-set judgment passes.

## 5. Determinism and limits

| Limit | Maximum |
|---|---:|
| functions per request | 4,096 |
| effect definitions per request | 4,096 |
| capability requirements per request | 4,096 |
| adapter imports per request | 4,096 |
| known contract identities | 65,535 |
| total function-owned graph entities | 2,000,000 |
| total operations across functions | 1,000,000 |
| total CFG value uses across functions | 2,000,000 |
| total CFG edges across functions | 65,535 |
| total prior-phase dominator word operations | 50,000,000 |
| direct-call edges | 16,384 |
| effect identities in one declared set | 4,096 |
| allowed scopes in one requirement | 65,535 |
| total allowed scopes | 1,000,000 |
| constraint contracts in one requirement | 65,535 |
| total constraint-contract memberships | 1,000,000 |
| closure memberships across all functions | 1,000,000 |
| closure convergence rounds | 4,096 |
| charged closure set/edge operations | 50,000,000 |

Counts and checked arithmetic are enforced before or during allocation/work.
Exhaustion, overflow, non-convergence, missing input, or internal ambiguity is
`EFFECT_RESOURCE_LIMIT` or a more specific failure, never success. Inputs are
immutable and repeated validation is independent of insertion order outside
the declared semantic lists.

## 6. Stable failures

| Numeric | Symbolic code |
|---:|---|
| 23000 | `EFFECT_UNRESOLVED_ENTITY` |
| 23001 | `EFFECT_WRONG_ENTITY_KIND` |
| 23002 | `EFFECT_SET_NOT_CANONICAL` |
| 23003 | `EFFECT_CLOSURE_MISMATCH` |
| 23004 | `EFFECT_CALL_TYPE` |
| 23005 | `EFFECT_REQUEST_TYPE` |
| 23006 | `ADAPTER_EFFECT_CARDINALITY` |
| 23007 | `ADAPTER_EFFECT_KIND` |
| 23008 | `ADAPTER_INVOKE_TYPE` |
| 23009 | `CAPABILITY_REQUIREMENT_TYPE` |
| 23010 | `CAPABILITY_SCOPE_CONST_TYPE` |
| 23011 | `CAPABILITY_SCOPE_CONST_CANONICAL` |
| 23012 | `CONSTRAINT_CONTRACT_BOUNDARY` |
| 23013 | `EFFECT_RESOURCE_LIMIT` |

An entity immediate that resolves to the wrong supplied SSMC1 kind uses
`EFFECT_WRONG_ENTITY_KIND`; an absent identity uses
`EFFECT_UNRESOLVED_ENTITY`. Shape, immediate, arity, operand-type, and
result-type failures use the owning operation code above.

Earlier `SSMC_*`, `TYPE_*`, and `GRAPH_*`/`CFG_*` failures retain their phase
and exact code. In particular, an ill-formed EffectDef type, nonpersistable
EffectDef scope type, ill-formed AdapterImport type, ill-formed
CapabilityRequirement scope constant, or earlier constant-shape/range failure
returns the exact S20-210 `TYPE_*` result. A pure-primitive candidate row
with a non-closed schema (including free type parameters) fails here, before
any shape rule applies. After a constant passes S20-210,
an exact declared-type mismatch uses `CAPABILITY_SCOPE_CONST_TYPE`, and an
ordering/duplicate failure uses `CAPABILITY_SCOPE_CONST_CANONICAL`. S20-230
does not collapse earlier errors into an effect error.

Pure-shape violations — at declaration (row triple) or at invocation
(scope/operand/result binding) — use `ADAPTER_INVOKE_TYPE`; an effect set
with more than one effect uses `ADAPTER_EFFECT_CARDINALITY`; a single
non-`AdapterCall` effect uses `ADAPTER_EFFECT_KIND`. The shared code keeps
first-failure deterministic: a row violating both points fails at step 4.

## 7. Deterministic validation order

1. closed request counts, global identities, and raw-ID input order;
2. S20-210/S20-220 judgment for every function in function-ID order;
3. effect-definition type and scope-persistability judgment;
4. adapter set/cardinality/kind/pure-row-shape and type judgment;
5. capability-requirement set, constant, and contract-boundary judgment;
6. relevant operation shape/type judgment (including pure invocation
   scope/operand/result binding) and call-edge construction;
7. bounded least-fixed-point closure construction;
8. exact function declaration comparison in function-ID order.

The first failure in this order is returned. A later phase never replaces an
earlier failure.

## 8. S20-230 acceptance

- positive fixtures cover direct request, adapter invoke, capability narrow,
  direct calls, empty effects, transitive calls, recursion, and mutual
  recursion;
- positive pure fixtures cover the three §1.5 shapes, including push
  monomorphizations over distinct element types, with empty function effect
  declarations and mixed effectful-plus-pure functions;
- negative fixtures cover every stable code, wrong entity kinds, operation
  shape/type mismatches, extra/missing declarations, ambiguous adapters,
  malformed scope constants, unresolved contracts, and hostile limits;
- negative pure fixtures cover every pure-shape violation (scope, request,
  response, and failure-type mismatches), effect-carrying rows presented
  for pure treatment, and multi-effect sets;
- pipeline negatives prove unregistered zero-effect imports fail closed at
  the lowering/registration gate even where their static form is valid;
- a recursive call-only SCC cannot self-justify an unused declared effect;
- function/input insertion-order perturbation produces the same judgment;
- a seeded hostile-call/closure smoke corpus terminates without panic;
- independent Vulcan review has no open P0, P1, or P2 finding;
- no policy root, capability authenticator, adapter execution, contract
  predicate, general opcode, VM, repository, provider, or Sley 1.x path is
  touched.

## Appendix A. Structural constant order

Allowed scopes use one complete host-independent order over already valid,
persistable `ConstValue` records. `compare_value(a,b)` first applies
`compare_type(a.value_type,b.value_type)`. Only when the types compare equal
does it apply `compare_data(a.data,b.data)`. There is no second type comparison.

All lists use the same lexicographic rule: compare corresponding elements
until one differs; if every shared element is equal, the shorter list sorts
first. `EntityId` and `MemberId` compare their complete 32 raw bytes
lexicographically. Optional values use `None < Some`, followed by the contained
comparison. Enum/union variants compare frozen numeric tag before payload.

`compare_type` first compares the frozen `TypeExpr` tag. Equal tags compare as
follows:

- `Unit`, `Bool`, `F32`, `F64`, `Bytes`, and `Text` have no payload;
- `SInt` and `UInt` compare raw width bits numerically;
- `Tuple` compares child types as a list;
- `Named` compares definition `EntityId`, then type arguments as a list;
- `Vector`, `Option`, and `LocalCell` compare their child type;
- `OrderedMap` compares key type, then value type;
- `Result` compares success type, then error type;
- `FunctionRef` compares parameter types as a list, result type, then the
  raw-ID effect list;
- `AdapterHandle` and `CapabilityToken` compare their `EntityId`;
- `TypeParameter` compares its ordinal numerically;
- `BuiltinFailure` compares the frozen failure-kind tag.

`compare_data` first compares the frozen `ConstData` tag. Equal tags compare as
follows:

- `Unit` has no payload and `Bool` uses `false < true`;
- `SInt` and `UInt` compare their mathematical integer values;
- `F32Bits` and `F64Bits` compare their canonical raw bits as unsigned
  integers;
- `Bytes` compares raw bytes and `Text` compares exact UTF-8 bytes;
- `Sequence` compares child `ConstValue` records as a list;
- `Record` compares definition `EntityId`, then fields as a list where each
  field compares `MemberId` followed by its value;
- `Variant` compares definition `EntityId`, case `MemberId`, then its optional
  payload value;
- `Map` compares entries as a list where each entry compares key value then
  mapped value;
- `Option` compares optional child value using `None < Some`;
- `Result` compares frozen arm tag (`Ok < Err`), then the arm value;
- `FunctionRef` compares target `EntityId`, then type arguments with
  `compare_type` as a list;
- `BuiltinFailure` compares frozen failure-kind tag, then numeric code.

No locale, label, Unicode normalization, address, host hash, filesystem fact,
or debug representation enters this order. Comparison equality is a duplicate
and is rejected. The checker never reorders input.

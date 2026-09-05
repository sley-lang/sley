# VM Extended Opcode Profile v1

Status: S20-260/S20-270 full-profile contract draft, revision 11 (2026-09-05);
Council review pending (Ariadne contract review, Nabu architecture review,
Vulcan surface review). Revisions 2 through 7 record the clarifications of
slices E1 through E6 (section 7); every slice is implemented. Revision 8 adds
the judgment-only entry external owners use (section 3.1). Revision 9 lands
slice E7a, `contract_assert` execution, which the S20-760 revision 2
determination showed needs no schema epoch. Revision 11 makes the family fuzz
lanes reach execution (section 5): per-fixture requests, a completion
assertion, a pinned refusal code, and position-stable seed selection.
Implementation lands in family slices E1 through E6 plus E7a, tracked in the machine summary; the rest of E7
stays excluded until its owners exist.

## Boundary

This profile extends the restricted S20-260 lowering and S20-270 execution
profiles to the SSMC1 epoch-1 opcodes beyond the three Boolean operations.
It composes, and never alters, `docs/spec/VM_LOWERING_PROFILE_V1.md`,
`docs/spec/VM_EXECUTION_PROFILE_V1.md`, `docs/spec/SSMC1.md` section 7,
the epoch-1 manifest (`docs/spec/SSMC1_EPOCH1_SCHEMA.txt`, the `op` table),
and the S20-210 type judgment. The restricted profile
(`CacheProfile::RESTRICTED_V1`, bytecode `SLEYBC01`) stays byte-identical
and its vectors stay frozen; the extended profile is a second cache profile,
`CacheProfile::EXTENDED_V1` (`lowering_profile = 2`, bytecode `SLEYBC02`),
selected explicitly by the caller. It reuses the seven `VM_LOWER_*` and six
`VM_EXEC_*` codes and adds no numeric range; the manifest's shapes and the
type checker's closed builtin-failure codes are the authority for every
operand, result, immediate, and failure value.

## 1. Profile and bytecode

- `CacheProfile::EXTENDED_V1 = { vm_version [1,0,0], lowering_profile 2,
  lowerer_version [1,0,0], entry_type_arguments 0, adapter_abi_entries 0,
  execution_abi_flags 0 }`; the cache key preimage is the S20-260 preimage
  with this profile, so restricted and extended keys never collide.
- Bytecode `SLEYBC02` is `SLEYBC01` with one `immediate` per instruction
  after its results: `u32 tag` (1 none, 2 entity, 3 index, 4 field, 5
  variant, 6 observation, 7 function) followed by the SSMC1 immediate
  payload (`EntityId[32]`, `u32`, `MemberId[32]`, `EntityId[32] ||
  MemberId[32]`, `[u8;32]`, or `EntityId[32] || list(type_expr)`).
- Registers, block slots, terminators, reachability, deterministic order,
  and the lowering limits are those of the restricted profile; the
  register type of every result is the exact type the signature judgment
  assigns (section 3), never the declared `result_types` alone.
- The lowering input gains the root's `constants`, `globals`, and
  `functions` inventories; the restricted profile ignores them.

## 2. Runtime values

Runtime values remain immutable views of validated `ConstValue`, extended
by two execution-local forms that never persist, never enter an observation,
and are rejected as a Function result type at lowering
(`VM_LOWER_SIGNATURE_MISMATCH`): local cells (a slot in the execution's
cell table) and adapter handles or capability tokens (E7, excluded). Every
constructed value satisfies `check_constant` of its register type; a value
that would not is `VM_EXEC_INTERNAL_INVARIANT`. Value units are charged for
every constructed value as in S20-270.

## 3. Signature judgment and semantics by family

Every operation is judged at lowering in Function block order: the opcode
must belong to a landed slice, the operand count and immediate kind must
match the manifest, every operand register type must match the rule, and
the declared `result_types` must equal the derived result exactly
(`VM_LOWER_OPCODE_UNSUPPORTED`, `VM_LOWER_IMMEDIATE_MISMATCH`, or
`VM_LOWER_SIGNATURE_MISMATCH`). `T` is any epoch-1 type unless the family
narrows it; integer widths are 8, 16, 32, 64, or 128.

### E1 data (1, 16, 17, 32, 33, 34, 35, 96 to 101, 128 to 131)

| Opcode | Operands | Immediate | Result | Runtime |
|---|---|---|---|---|
| 1 `constant_ref` | none | Entity: a `Constant` of the root | the constant's exact type | the constant's value |
| 16 `tuple_new` | `T1..Tn`, n at most 64 | none | `Tuple([T1..Tn])` | the tuple |
| 17 `tuple_get` | `Tuple(ts)` | Index i, i < len(ts) | `ts[i]` | the element |
| 32 `vector_new` | n of `T` | none | `Vector<T>`; with n = 0 the declared result names `T` | the vector |
| 33 `vector_len` | `Vector<T>` | none | `UInt(64)` | the length |
| 34 `vector_get` | `Vector<T>`, `UInt(64)` | none | `Option<T>` | `Some` in range, `None` otherwise |
| 35 `vector_set` | `Vector<T>`, `UInt(64)`, `T` | none | `Result<Vector<T>, BuiltinFailure(Index)>` | a new vector, or `Err(Index, 1)` out of range |
| 96 `equal`, 97 `not_equal` | `T`, `T`, `T` hashable and float-free | none | `Bool` | canonical value equality |
| 98 to 101 order predicates | `T`, `T`, `T` in `Bool`, `SInt`, `UInt`, `Bytes`, `Text` | none | `Bool` | `false < true`; numeric order; `Bytes` and `Text` by byte then length; floats in E3 |
| 128 `option_some` | `T` | none | `Option<T>` | `Some` |
| 129 `option_none` | none | none | the declared `Option<T>` | `None` |
| 130 `result_ok` | `T` | none | the declared `Result<T, E>` | `Ok` |
| 131 `result_err` | `E` | none | the declared `Result<T, E>` | `Err` |

### E2 checked integers (64 to 71)

Operands are two integers of one type `T` (`SInt(w)` or `UInt(w)`), except
`int_neg_checked` (one `SInt(w)`) and the shifts (`T`, `UInt(32)`). The
result is `Result<T, BuiltinFailure(Arithmetic)>` with the S20-210 closed
codes: overflow 1, divide by zero 2, invalid shift 3. Rules: add, sub, mul
overflow the width to code 1; div and rem by zero are code 2 and signed
minimum divided by minus one is code 1; neg of the signed minimum is code
1; a shift amount at or above the width is code 3; `int_shl_checked` whose
shifted-out bits are not all zero (unsigned) or not all the sign bit
(signed) is code 1; `int_shr_checked` is arithmetic for `SInt` and logical
for `UInt`. Results are exact in the width; no widening, narrowing, or
wrapping exists.

### E3 deterministic floats (80 to 85) and float order

Operands are `F32` or `F64` (three for `float_fma`); the result is the
same type. Semantics are IEEE-754 binary32 or binary64,
round-to-nearest-ties-to-even, subnormals preserved, `float_fma` a single
rounding; every result that is a NaN is canonicalized to the quiet NaN with
a zero sign and zero payload (`0x7fc00000`, `0x7ff8000000000000`), and the
result bits are stored exactly. The order predicates over floats follow
IEEE: an unordered pair makes `equal`, `less_than`, `less_equal`,
`greater_than`, and `greater_equal` false and `not_equal` true; negative
zero cannot occur (section 7).

### E4 aggregates and maps (18 to 21, 36 to 40)

`record_new` takes Entity: a non-generic `Record` type definition of the
environment, one operand per field in definition order with the field's
type, and yields `Named { definition, arguments: [] }`; `record_get` takes
Field: a member of the operand's record definition and yields the field's
type; `variant_new` takes Variant: a case of a non-generic `Variant`
definition, one operand of the payload type when the case has one and
none otherwise, and yields the named type; `variant_get` takes Variant: a
case with a payload and yields `Option<payload>` (`Some` when the value is
that case). `map_new` takes `2n` operands (`K, V` pairs, `K` hashable)
and yields `Result<OrderedMap<K, V>, BuiltinFailure(DuplicateKey)>` with
code 1 on a repeated key; `map_get` yields `Option<V>`; `map_contains`
`Bool`; `map_insert` and `map_remove` yield the new map. Map order is the
order of the keys' S20-350 canonical bytes, exactly as constants require.
Maps the profile builds are sorted on construction; maps supplied from outside
are required to arrive in that order, because `equal` and `value_hash` read it
structurally.

### E5 cells, hashing, globals, references (176 to 178, 192 to 194)

`cell_new` takes `T` (persistable) and yields `LocalCell<T>`; `cell_get`
yields `T`; `cell_set` takes the cell and a `T` and yields `Unit`; cells
are per execution and their contents count as live value units. `cell_new`
and `cell_set` clone their value into the cell table, so each charges the
stored value's units in addition to its result: the table is a second place
the value lives, and a budget that did not count it would bound nothing. At
most 1,048,576 cells exist in one execution, which holds when a request
declares a value-unit budget large enough to make the charge no bound at
all; exceeding either is `VM_EXEC_RESOURCE_LIMIT` with `ResourceKind` value
units.
`value_hash` takes a hashable `T` and yields `Bytes` of exactly 32 bytes,
the S20-250 `hash_validated_value` of the operand under the schema epoch.
`global_get` takes Entity: a `GlobalValue` of the root whose initializer
is a `Constant` and yields the global's type with the constant's value.
`function_ref` takes Function `{ function, type_arguments: [] }` naming a
zero-parameter-type Function of the root and yields
`FunctionRef(FunctionType)` equal to that Function's signature.

### E6 direct calls (112)

`call_direct` takes Function `{ function, type_arguments: [] }`, one
operand per callee parameter with its type, and yields the callee's result
type. Lowering lowers every Function reachable through direct calls under
the same profile, cache key, and limits; execution keeps an explicit call
stack with a depth ceiling of 256 frames (`ResourceKind::CallDepth`, tag
5, a resource-limit termination) and charges one fuel and one instruction
per call. Recursion within the ceiling is allowed; a callee's trap or
resource termination terminates the whole execution.

### E7a contract assertions (144)

`CONTRACT_TEST_PROFILE_V1.md` section 2 accepts `contract_assert` statically
under epoch 1 and assigns predicate execution to S20-270 and report evidence
to S20-290. This slice takes that assignment. It needs no schema epoch,
because the opcode is already in the frozen epoch-1 table and this profile
carries its own `lowering_profile` identity; `EPOCH_MIGRATION_POLICY_V1.md`
section 6 records the determination.

Judgment re-derives every rule rather than trusting the checker that already
passed:

- the immediate is `Entity(contract)` and resolves in the request's Contract
  inventory, else `VM_LOWER_IMMEDIATE_MISMATCH`;
- the contract kind is `Precondition`, `Postcondition`, or `ResultPredicate`
  (the three epoch-1 supported kinds) and carries no resource ceiling;
- the contract's `target` is the enclosing function and its `predicate` is a
  different function that declares no effects and no contracts;
- the predicate resolves with zero type parameters and returns exactly `Bool`;
- the operands equal the predicate's parameter types in exact order, and the
  binding count equals the parameter count, else
  `VM_LOWER_SIGNATURE_MISMATCH`;
- the binding `source` is uninterpreted under `EXTENDED_V1`: predicate
  arguments come from operand order alone, so the S20-240 binding declarations
  and the VM operand order are not two agreeing authorities;
- the single declared result is exactly
  `Result<Unit, BuiltinFailure(ContractViolation)>`.

Execution enters the predicate as an ordinary E6 frame: the same call-depth
ceiling, the same shared fuel, instruction, value-unit, and cell budgets, and
the same unwinding on a callee trap or resource termination. The caller wraps
the answer rather than returning it: `true` yields `Ok(Unit)` and `false`
yields `Err(BuiltinFailure(ContractViolation, 1))`, the only code the S20-210
checker admits for that family. A violated contract is a value, never a trap:
the caller decides what a failed assertion means.

The predicate's frame is charged like any call, so one assertion over a
one-operation predicate costs exactly five fuel.

`contract_assert` stays outside S20-360 phase 7 operation analysis. Its static
typing belongs to the S20-240 checker at phase 10, which reports the exact
`CONTRACT_ASSERT_TYPE` diagnosis; phase 7 does not preempt an owner with a
lowering code. The VM judges the operation again when it lowers, after
validation has passed.

### E7 tests, effects, adapters, capabilities (145, 160 to 162)

Excluded from this revision: they answer `VM_LOWER_OPCODE_UNSUPPORTED`
until S20-240 full, S20-280 full, and S20-380 full own their runtime.
`test_observe` additionally needs a schema epoch, because epoch 1 rejects it
outright rather than leaving its semantics open.

### 3.1 Judgment without lowering (revision 8)

`judge_function_operations(input)` judges every operation of one Function
under `EXTENDED_V1` and returns the operation count and the judgment work,
without emitting bytecode, deriving a cache key, lowering callees, or
executing anything. It is the surface external owners use: S20-360 candidate
validation calls it once per function unit after the S20-220 graph report.

Unlike `lower_function` it does not refuse a Function that declares type
parameters, effects, or contracts, because those belong to the S20-210,
S20-230, and S20-240 owners; a caller that needs bytecode still uses
`lower_function` and gets the frozen refusals. Failures keep their exact
`VM_LOWER_*` codes, so a caller can map them onto its own decisions without
inventing a code. A restricted-profile request is
`VM_LOWER_PROFILE_UNSUPPORTED`.

## 4. Observation and reports

The observation preimage of S20-270 is unchanged; the extended profile
enters it through the cache key and the new resource kind, and every
`Success` value is hashed with `hash_validated_value` as before. The
preimage's own `execution_profile` field stays at its frozen value because
the cache key it already carries binds the lowering profile, so no two
profiles can share an observation identity.

S20-290 report building accepts `EXTENDED_V1` beside `RESTRICTED_V1`, which
`REPORT_ENVELOPE_PROFILE_V1.md` revision 2 (2026-09-03) records. That
envelope needed the opposite treatment from the observation: a rejected
report carries no cache key, only a phase and a numeric code, so it binds
the profile itself in `execution_profile`. Revision 1's constant let one
request rejected under both profiles derive one identity. SMP1 `execute` selects the profile
named by the request's limits record field 6 (`uvar(profile: 1 restricted
| 2 extended)`, appendix C revision 8), defaulting to restricted when the
field is absent.

## 5. Required evidence

Per slice: fixed vectors under `conformance/vm-extended/v1/` (function
graphs, inputs, expected termination, and observation identity) emitted
from the crate and drift-gated in `make quick`; a rejection matrix for each
signature rule; 128 repeated vector executions producing equal outcomes (the
count belongs to the vectors: the fuzz lane asserts determinism twice per
draw instead); the `vm_canonical_inputs` persistent slice extended with one
lane per landed family; and the restricted vectors unchanged. Each family lane
must reach a completed termination, success or the family's value failure, on
the pinned corpus: the lane builds its request from the fixture's own
parameter types under generous limits and asserts the first execution
completes, while the restricted-profile refusal pins
`VM_LOWER_OPCODE_UNSUPPORTED` for the single-graph families. A lane that stops
reaching execution fails loudly instead of comparing two input errors, and the
runner requires the executed run count to cover the corpus. Lanes carry
per-family reachability; vectors plus the rejection matrix carry per-opcode
duty. For the profile: Tier 1 plus
Tier 2 validation, and the Ariadne, Nabu, and Vulcan reviews with every
report-grade finding closed.

## 6. Explicit exclusions

This contract does not claim: E7 beyond slice E7a; generic specialization or type
arguments; an optimizer; effects, adapters, capabilities, replay, or live
cancellation beyond S20-270's rules; a second host or byte-memory budget;
S20-360 full operation analysis; or GA.

## 7. Revisions 2 through 7 clarifications (slices E1 through E6)

- E1 equality excludes any type containing `F32` or `F64` (S20-210 counts
  floats as hashable); E3 defines float equality and order under IEEE.
- The E1 vectors live in `conformance/vm-extended/v1/accepted.json`
  (bytecode bytes, cache key, success value hash, observation identity,
  instruction count) under the fixed epoch `08`\*32 and root `09`\*32,
  emitted by the crate and drift-gated in `make quick`.
- The E1 fuzz lane is the profile toggle of `vm_canonical_inputs`: every
  fixed fixture also lowers and executes under `EXTENDED_V1`, and the
  terminations must equal the restricted profile's while the cache keys
  differ; a generated data-family lane joins with E2.
- `constant_ref` reads the root's Constant inventory carried by the
  lowering input; the restricted profile ignores the new inventories.
- The S20-290 report builder accepts `EXTENDED_V1` beside `RESTRICTED_V1`.
- E2: `int_div_checked` truncates toward zero and `int_rem_checked` takes
  the dividend's sign (the truncated remainder), so `rem` overflows exactly
  when `div` overflows (signed minimum by minus one); every checked
  operation computes in 128 bits and then checks the operand width, so a
  narrower width overflows at its own bounds and width 128 at the native
  ones; the shift amount is `UInt(32)` and a shift of `width` or more is
  the invalid-shift code before any overflow check.
- E3: `equal` and `not_equal` admit bare `F32` and `F64` operands with IEEE
  meaning (an unordered pair is unequal, the zeros are equal) while a
  float nested in an aggregate stays excluded from equality; every input
  float is an S20-210 canonical constant (one quiet NaN, no negative zero),
  a NaN result becomes that canonical NaN, and a negative-zero result
  becomes positive zero, so `-0` never appears in a value; the subnormal
  and rounding rules are the host's IEEE-754 binary32 and binary64
  arithmetic under round-to-nearest-ties-to-even, with `float_fma` a single
  fused rounding.
- E4: record and variant immediates must name non-generic definitions of
  the lowering environment (a generic definition, an unknown member, a
  record named as a variant, or `variant_get` on a payload-less case is
  `VM_LOWER_IMMEDIATE_MISMATCH`); a map key type needs the S20-210 total
  order and no float (a float key already fails the S20-220 graph check
  with `TYPE_NOT_ORDERABLE`, so the lowering guard is defense in depth); `map_new` with no operands takes its key and value
  types from the declared result; the runtime map order is the
  lexicographic order of the keys' S20-350 canonical bytes, obtained
  through `sley_mutate::encode_const_value` (the VM crate now depends on
  the mutation crate, which the dependency direction allows), and
  `map_insert` of an existing key replaces its value in place. That order
  is a precondition on every value the profile does not build itself, and
  S20-210 does not establish it (`TYPE_SYSTEM_V1.md` section 5), so the VM
  verifies it against the same encoder where such a value is supplied: an
  execution input with no canonical form is
  `VM_EXEC_INPUT_NOT_CANONICAL` and a constant that `constant_ref` or
  `global_get` names is `VM_LOWER_IMMEDIATE_MISMATCH`, checked after the
  operation judgment so the frozen S20-260 failure order is unchanged. The
  VM refuses such a value; it does not sort it.
- E5: a `LocalCell` value exists only inside one execution: it may be an
  operand of `cell_get` and `cell_set` only, no other operation may take a
  cell or a type containing one, and a Function whose result type contains
  a cell fails lowering with `VM_LOWER_SIGNATURE_MISMATCH`; a cell handle
  is register-only and has no wire form; `cell_new` and `cell_set` each
  charge the stored value's units on top of their result, because the cell
  table holds a clone that outlives the instruction and is live independently
  of the register file (charging only the result charged the handle, so a
  loop could hold unbounded host memory with the budget intact);
  `value_hash` is the S20-250 `hash_validated_value` under the execution's
  schema epoch; `global_get` resolves the global's initializer Constant in
  the lowering inventory and the constant's type must equal the global's
  type (`VM_LOWER_IMMEDIATE_MISMATCH` otherwise); `function_ref` requires an
  empty type-argument list and a zero-type-parameter Function of the
  inventory, and the derived `FunctionRef` carries the callee's parameter
  types in ordinal order, its result type, and its effects.
- E6: lowering collects the transitive `call_direct` closure of the entry
  through the Function inventory (an unknown callee is
  `VM_LOWER_IMMEDIATE_MISMATCH`, a generic, effectful, or contracted callee
  is `VM_LOWER_PROFILE_UNSUPPORTED`, and a callee's own lowering failure is
  the entry's failure), lowers each callee under the entry's profile, cache
  key, and work budget, and appends a callee table to `SLEYBC02` after the
  entry body: a u64 count followed by each callee body in ascending
  function-id order (the entry never repeats itself in the table; a
  self-call resolves to the entry body). Execution opens one register file
  per frame, copies the arguments into the callee's parameter registers
  (charging their value units), shares the instruction, fuel, value-unit,
  and cell state across frames, charges one fuel per call up front, counts
  the call instruction only when the callee returns (a call chain cut by a
  termination counts no call instructions), and refuses a frame beyond the
  256-frame ceiling (entry frame included) with `ResourceLimit(CallDepth)`,
  tag 5, which occurs only under `EXTENDED_V1`; the restricted profile's
  closed `ResourceKind` set is unchanged. The call stack is explicit
  (suspended caller frames in a list), so the ceiling never depends on the
  host stack.

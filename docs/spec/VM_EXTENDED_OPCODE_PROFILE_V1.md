# VM Extended Opcode Profile v1

Status: S20-260/S20-270 full-profile contract draft, revision 4 (2026-09-03);
Council review pending (Ariadne contract review, Nabu architecture review,
Vulcan surface review). Revision 2 records the clarifications of slice E1,
revision 3 those of slice E2, and revision 4 those of slice E3 (section 7). Implementation lands in family slices E1 through E6 tracked in
the machine summary; E7 is explicitly excluded until its owners exist.

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

### E5 cells, hashing, globals, references (176 to 178, 192 to 194)

`cell_new` takes `T` (persistable) and yields `LocalCell<T>`; `cell_get`
yields `T`; `cell_set` takes the cell and a `T` and yields `Unit`; cells
are per execution and their contents count as live value units.
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

### E7 contracts, tests, effects, adapters, capabilities (144, 145, 160 to 162)

Excluded from this revision: they answer `VM_LOWER_OPCODE_UNSUPPORTED`
until S20-240 full, S20-280 full, and S20-380 full own their runtime.

## 4. Observation and reports

The observation preimage of S20-270 is unchanged; the extended profile
enters it through the cache key and the new resource kind, and every
`Success` value is hashed with `hash_validated_value` as before. S20-290
report building accepts `EXTENDED_V1` beside `RESTRICTED_V1` (a revision
of that contract's profile check). SMP1 `execute` selects the profile
named by the request's limits record field 6 (`uvar(profile: 1 restricted
| 2 extended)`, appendix C revision 8), defaulting to restricted when the
field is absent.

## 5. Required evidence

Per slice: fixed vectors under `conformance/vm-extended/v1/` (function
graphs, inputs, expected termination, and observation identity) emitted
from the crate and drift-gated in `make quick`; a rejection matrix for each
signature rule; 128 repeated executions producing equal outcomes; the
`vm_canonical_inputs` persistent slice extended with one lane per landed
family; and the restricted vectors unchanged. For the profile: Tier 1 plus
Tier 2 validation, and the Ariadne, Nabu, and Vulcan reviews with every
report-grade finding closed.

## 6. Explicit exclusions

This contract does not claim: E7; generic specialization or type
arguments; an optimizer; effects, adapters, capabilities, replay, or live
cancellation beyond S20-270's rules; a second host or byte-memory budget;
S20-360 full operation analysis; or GA.

## 7. Revision 2 clarifications (slice E1)

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

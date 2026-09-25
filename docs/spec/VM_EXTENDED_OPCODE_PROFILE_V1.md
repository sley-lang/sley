# VM Extended Opcode Profile v1

Status: S20-260/S20-270 full-profile contract draft, revision 17 (2026-09-17).
This revision implements the E7 result-escape guard specified by revision 16:
an extended-profile Function result containing an `AdapterHandle` or
`CapabilityToken`, at any structural depth, fails lowering with
`VM_LOWER_SIGNATURE_MISMATCH`. It does not authorize E7 execution: every
landed slice keeps its bytes, vectors, and cache keys, and opcodes 145, 160,
and 162 keep their exact refusals. Revision 15 closed the historical Nabu
documentation and corpus gaps; independent item-level review accepted these
corrections (see
`evidence/review/vm-nabu-correction-review-2026-09-15.md`). Existing epoch-1
execution semantics and earlier vector identities are preserved. Revision
14 recorded the E8 review currency; later E8 review evidence remains in the
machine summary and is not replaced by this correction campaign.

Revisions 2 through 7 record the clarifications of
slices E1 through E6 (section 7); every slice is implemented. Revision 8 adds
the judgment-only entry external owners use (section 3.1). Revision 9 lands
slice E7a, `contract_assert` execution, which the S20-760 revision 2
determination showed needs no schema epoch. Revision 11 makes the family fuzz
lanes reach execution (section 5): per-fixture requests, a completion
assertion, a pinned refusal code, and position-stable seed selection.
Revision 12 answers the Ariadne contract review and Vulcan surface review
freeze findings: the `lowerer_version` bump with its rule and the full
`SLEYBC02` layout in section 1; the stated post-construction invariant and
the liveness-not-peak charging bound in section 2; the exact equality rows,
the named negative-zero deviation with its consequence, the pinned float
environment, the encoding-order decision with byte key identity, the absent-key
and operand-arity gaps, the failure-name mapping, and the five-fuel
derivation in section 3; the preimage field name in section 4; and the
same-toolchain evidence scope in section 5. Revision 13 lands slice E8, the
host bridge imports admitted by the REWEAVE RW-030 charter (section E8):
three versioned `adapter_invoke` entries over the already-frozen opcode 161,
which the same S20-760 determination class shows needs no schema epoch. The
rest of E7 stays excluded until its owners exist. Revision 16 specifies the
E7 capability-handle host-binding design (new section below): it answers the
S20-760 item 3c owner blocker with a boundary a future owner slice must
satisfy, and authorizes no execution, no bytecode or cache-key change, and
no new code. Revision 17 lands only the design's result-escape guard, with a
direct regression test over both handle forms and nested result/function
shapes. Handle construction, binding, narrowing, effect servicing, and host
minting remain owner-blocked and unsupported.

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
operand, result, immediate, and failure value. The manifest op table spells
the failure kinds `ArithmeticError`, `IndexError`, `DuplicateKeyError`;
those are the wire spellings of `BuiltinFailureKind::Arithmetic`, `::Index`,
`::DuplicateKey`, and this contract uses the kind names (`ContractViolation`
is spelled alike in both).

## 1. Profile and bytecode

- `CacheProfile::EXTENDED_V1 = { vm_version [1,0,0], lowering_profile 2,
  lowerer_version [2,0,0], entry_type_arguments 0, adapter_abi_entries 0,
  execution_abi_flags 0 }`; the cache key preimage is the S20-260 preimage
  with this profile, so restricted and extended keys never collide. The
  lowerer version separates bytecode layouts in cache identity: slice E6
  appended the callee table below to the `SLEYBC02` body under `[1, 0, 0]`
  without a bump, so two incompatible layouts shared one cache key;
  revision 12 bumps to `[2, 0, 0]`, and any future `SLEYBC02` layout change
  bumps `lowerer_version` again or the change does not land. Pre-freeze
  caches under `[1, 0, 0]` are discarded, not migrated.
- Bytecode `SLEYBC02` is a header, one entry body, then a callee table:
  the magic, a `u32` format version, the entry body, then a `u64` callee
  count followed by each callee body in ascending function-id order (the
  entry never repeats itself in the table; a self-call resolves to the
  entry body). Each body is `SLEYBC01` with one `immediate` per instruction
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
cell table) and adapter handles or capability tokens (E7 execution remains
excluded; revision 17 enforces the result-escape portion of the handle model
specified in the E7 design section below). Every
constructed value carries exactly its register type; a value whose
`value_type` differs from the result register's type is
`VM_EXEC_INTERNAL_INVARIANT`. That identity check is the whole
post-construction check: `check_constant` runs on execution inputs at the
boundary (with the canonical-form refusal), never on constructed values,
because the E5 cell handle is register-only and has no constant form to
check against. Value units are charged for every constructed value as in
S20-270; `max_value_units` bounds charged liveness, not transient peak
allocation: the whole result is built before it is charged, so one
`tuple_new` (arity at most 64) or `map_insert` can transiently allocate
past the budget before the limit fires.

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
| 96 `equal`, 97 `not_equal` | two `T` operands, `T` hashable and containing no float (bare `F32`/`F64` admitted with IEEE meaning, E3) | none | `Bool` | canonical value equality; structural equality would make `NaN == NaN` true, so aggregates holding floats stay excluded |
| 98 to 101 order predicates | two `T` operands in `Bool`, `SInt`, `UInt`, `Bytes`, `Text`, `F32`, `F64` | none | `Bool` | `false < true`; numeric order; `Bytes` and `Text` by byte then length; bare floats by IEEE (E3), unordered pairs all false |
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

Operands are `F32` or `F64`: one for `float_neg`, three for `float_fma`,
two for the rest; the result is the
same type. Each operation is correctly rounded in its declared binary32 or
binary64 format. Excess-precision evaluation, implicit FMA contraction of
separately represented operations, flush-to-zero (FTZ), and denormals-are-zero
(DAZ) are forbidden. A host/environment violating these requirements is
nonconforming. Semantics are IEEE-754 binary32 or binary64,
round-to-nearest-ties-to-even, subnormals preserved, `float_fma` a single
rounding; every result that is a NaN is canonicalized to the quiet NaN with
a zero sign and zero payload (`0x7fc00000`, `0x7ff8000000000000`), and every
other result bit is stored exactly. Named deviation: a negative-zero result
becomes positive zero, so `-0` never appears in a value, which composes
observably (`float_div(1.0, float_mul(-1.0, 0.0))` is `+inf` where IEEE gives
`-inf`). The order predicates over floats follow
IEEE: an unordered pair makes `equal`, `less_than`, `less_equal`,
`greater_than`, and `greater_equal` false and `not_equal` true; negative
zero cannot occur (section 7). The determinism claim is pinned to the
toolchain in `rust-toolchain.toml` on one target triple: the evidence is
same-toolchain repeatability, and cross-host identity additionally requires
that floating-point environment (binary32/64, round-to-nearest-ties-to-even,
subnormals preserved, no flush-to-zero), which is the default build with no
fast-math anywhere in this repo.

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
code 1 on a repeated key; `map_get` takes the map and a key and yields
`Option<V>`; `map_contains` takes the map and a key and yields `Bool`;
`map_insert` takes map, key, and value and `map_remove` takes map and key,
both yielding the new map; removing an absent key returns the map unchanged.
Map order is the lexicographic order of the keys' S20-350 canonical bytes:
the encoding order, named as such. It is deterministic and canonical, but
it is not the numeric key order (`UInt(255)` encodes as `FF 01` and sorts
after `UInt(256)` as `80 02`), and the S20-210 total order stays only the
admissibility precondition for key types. Key identity is the encoding too:
duplicate detection and every probe compare canonical key bytes, so the map
`map_new` accepts is exactly the map the codec accepts.
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
most 1,048,576 cells exist in one execution; exceeding the count or the
value-unit budget is `VM_EXEC_RESOURCE_LIMIT` with `ResourceKind` value
units.
`value_hash` takes a hashable `T` and yields `Bytes` of exactly 32 bytes,
the S20-250 `hash_validated_value` of the operand under the schema epoch.
For admitted canonical non-float values, equality implies identical value
hashes. The converse is not a proof of equality: hashes can collide.
Canonical NaNs are a concrete named exception to the converse even without
a cryptographic collision: identical canonical NaNs hash identically but
IEEE `equal` returns false. This explains the exclusion of float-containing
aggregates from structural equality. The native regression
`equal_values_share_hashes_and_canonical_nan_keeps_ieee_inequality` pins the
NaN exception and a non-float example.
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
stack with a depth ceiling of 256 live frames, entry frame included
(`ResourceKind::CallDepth`, tag 5, a resource-limit termination): the frame
that would make 257 live is refused, so 256 live is the deepest reachable
stack, pinned by `e6_call_depth_ceiling_is_256_live_frames_with_the_entry_included`
and the `call-direct-depth-ceiling` vector. The ceiling is fixed and ignores
the manifest `ResourceLimits` call-depth field (field 5): no request path
delivers that field to execution (the SMP1 limits record carries no depth),
so honoring a caller-supplied depth needs a protocol change and a new
`lowering_profile`; until then a declared depth binds nothing. The existing
fixed 256 bound belongs to the frozen VM/profile identity. Any future
request-supplied depth must also be encoded in the observation-bound request
limits: a profile tag alone cannot distinguish two depths in one profile.
The prospective-frame depth check precedes call-entry fuel/cancellation
charging. A depth refusal charges no call-entry action; any previously
completed charges remain counted. `call_depth_refusal_precedes_call_entry_fuel_and_cancellation`
pins the simultaneous boundary and accounting. Execution charges one fuel
per admitted call; its instruction is counted only on successful return. Recursion within the ceiling is allowed; a callee's trap or
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
one-operation predicate costs exactly five fuel: 1 for the assertion's
dispatch, 1 for the frame, 1 for the predicate's operation, and 2 for the
two terminators.

`contract_assert` stays outside S20-360 phase 7 operation analysis. Its static
typing belongs to the S20-240 checker at phase 10, which reports the exact
`CONTRACT_ASSERT_TYPE` diagnosis; phase 7 does not preempt an owner with a
lowering code. The VM judges the operation again when it lowers, after
validation has passed.

### E7 tests, effects, adapters, capabilities (145, 160 to 162)

Excluded from this revision, except the three slice-E8 bridge entries over
op 161 named in section E8: every other use answers
`VM_LOWER_OPCODE_UNSUPPORTED` until S20-240 full, S20-280 full, and S20-380
full own their runtime.
`test_observe` additionally needs a schema epoch, because epoch 1 rejects it
outright rather than leaving its semantics open (`CONTRACT_TEST_PROFILE_V1.md`
section 3.4 carries the rejection; op 145 is in the epoch-1 table, so the
manifest alone does not).

### E7 handle-model design (160, 162; execution deferred to owners)

This section specifies the boundary a future owner slice must satisfy before
opcodes 160 (`effect_request`) and 162 (`capability_narrow`) can execute. It
authorizes no execution: every use of 160 and 162 still answers
`VM_LOWER_OPCODE_UNSUPPORTED`, and no owner may cite this section as
execution authority. It answers the S20-760 section 6 item 3c owner blocker
with a design rather than an implementation.

Authority facts the design rests on:

- `CAPABILITY_TOKEN_V1.md` section 1: only the host issues or verifies
  tokens, because the keyed-BLAKE3 secret and the current time are explicit
  host inputs that are never serialized and never read from ambient process
  state.
- `CAPABILITY_TOKEN_V1.md` section 7: the token profile does not complete
  VM effect opcodes.
- Execution has no host-secret or wall-clock channel: the execution context
  carries cells, fuel, budgets, and call-stack state, but no host secret or
  wall-clock channel. Carrying the secret or the clock
  into execution would breach the token authority boundary above: the secret
  never enters execution, so in-execution verification and in-execution
  minting are both refused by this design, not merely unimplemented.
- `EPOCH_MIGRATION_POLICY_V1.md` section 6 item 3c (re-verified 2026-09-04):
  no narrowing function exists in `sley-policy`, and no host services an
  effect.

Handle form. A capability handle is an opaque execution-local reference to
a host-owned table entry, in the same family as the E5 cell handle: it never
persists, never enters an observation, has no wire or constant form, and a
Function whose result type contains one fails lowering with
`VM_LOWER_SIGNATURE_MISMATCH`. Execution receives handles only: token bytes
and the host secret never cross the execution boundary in either direction.
Handles originate only from verified-token binding; an unbound handle is
refused by the owner slice's code, the host table is dropped with the
execution and never reused across executions, and handles cross the boundary
as references only — they never serialize into observations or token bytes.

Landed boundary enforcement (revision 17). The result-escape half of the
handle form is active even while handle-producing opcodes remain refused:
`check_result_type` recursively rejects `AdapterHandle` and
`CapabilityToken` through tuples, named arguments, vectors, options, maps,
results, function-reference signatures, and local-cell element types. The
existing E5 `LocalCell` result guard remains separate because it also governs
which operations may consume cells. This correction changes no bytecode
layout and creates no handle value, host call, token verification, or minting
path.

Verification boundary. Presented tokens are verified by the host before
execution, in the token profile section 5 order (version, issuer, key, MAC,
policy root, workspace, principal, state root, effect, scope, adapter, time,
grant, budget). Execution never re-verifies and never trusts an unverified
handle: a handle resolves only through the host table that verified-token
binding populates.

Narrowing decomposition. A future 162 slice must split narrowing into three
parts and may land only all three together:

- structural scope subset against the presented token's requirement and its
  frozen allowlist
  (`EFFECT_SYSTEM_V1.md` section 1.3 with its Appendix A order), which needs
  no authority because the allowlist is static requirement data;
- constraint-contract predicate execution, for which slice E7a is the
  precedent (predicates run as ordinary frames under the same budgets);
- host mint of the narrowed token at the observation boundary, because only
  the host holds the secret.

The handoff record between execution and host mint — the narrowed-body
bytes, the proof inputs, the fresh-nonce discipline that keeps the token
profile replay ledger sound, the grant, policy-root, and time recheck at
mint, the narrowed-budget subset rule, and the budget charge for the mint —
is owed by S20-380 full. Until that owner specifies it, 162 execution cannot land,
because landing structural narrowing without authenticated mint would hand
out authority from unverified bytes.

Effect servicing. Opcode 160 stays excluded until S20-280 full owns
fixtures and host servicing: no host services an effect today, and a request
with nowhere to be serviced is not executable. The fixture protocol
(identity, budgets, replay order, atomicity) is that owner's acceptance, not
this profile's.

What a future owner slice must still bring. The E7a/E8 evidence pattern
applies unchanged: exact signature judgments with the frozen `VM_LOWER_*`
mapping and no new numeric range, the acceptance-invariant differential
test of section 3.1, conformance vectors with a pinned refusal code for
every remaining refusal, fuzz-lane reach-execution per section 5, an S20-360
phase-7 analyzability addendum (phase 7 is owned by `sley-policy` and does
not widen by implication from VM execution), and S20-280/S20-380 owner
review. Opcode 145 and the item 1 contract kinds are outside this design:
they need a schema epoch with the four approvals of the epoch policy
section 3, which no profile revision grants.

### E8 host bridge imports (161)

Slice E8 implements the byte-access/construction remedy admitted by the
REWEAVE RW-030 charter (`host-boundary.json` `bridge_g10`,
`machineresearch/sley-2.0/reweave/rw-030-g10-admission.md`): a bounded
lossless representation bridge plus one minimal bounded generic growth
operation, carried as three versioned host-import entries over the
already-frozen `adapter_invoke` opcode (tag 161), in its frozen invocation
shape: two operands (`scope`, `request`) and an `Entity` immediate naming
an import with declared request/response/failure types. No new opcode, no
new failure kind, no `SLEYBC02` layout change (161 with an `Entity`
immediate already encodes), so `lowerer_version` stays `[2, 0, 0]` and no
schema epoch is required: the opcode is already in the frozen epoch-1
table and this profile carries its own `lowering_profile` identity, the
same determination class as slice E7a (`EPOCH_MIGRATION_POLICY_V1.md`
section 6). `adapter_abi_entries` stays 0: the entries are profile-pinned
and take no adapter configuration.

Entry identities are `Entity` immediates with twelve ASCII bytes
`SLY1/BRIDGE/`, a four-byte entry code, and zero padding to 32 bytes.
These are REWEAVE host-ABI import identities (the RW-070 freeze records
them), not reference-adapter identities: bridge entries are pure value
functions over caller-owned values, so no `AdapterCall` effect, no fixture
state, and no reference-registry kind applies to them. Each entry is a
genuine epoch-1 `AdapterImport` value — identity, adapter identity, ABI
version 1, exact request/response types, `Index` failure type, empty effect
list — and resolution is genuine: the immediate must name a carried import
of the lowering/execution inventory, and every frozen field of that row
must equal the frozen bridge values. A frozen identity alone, with no
carried row, stays `VM_LOWER_OPCODE_UNSUPPORTED`; so does a carried row
with a foreign adapter identity, ABI version, failure type, effect list,
or (for the conversions) request/response types. Scope mirrors the
reference-adapter convention (the state acted upon; `Unit` where there is
none); epoch-1 stores scope on the effect, which pure imports do not have,
so the profile freezes each entry's scope type here instead:

| Entry | Code | Scope | Request | Response | Declared failure |
|---|---|---|---|---|---|
| `host-bytes-to-u8vector` | `B2V1` | `Unit` | `Bytes` | `Vector<UInt(8)>` | `BuiltinFailure(Index)` |
| `host-u8vector-to-bytes` | `V2B1` | `Unit` | `Vector<UInt(8)>` | `Bytes` | `BuiltinFailure(Index)` |
| `vector-push` | `PSH1` | `Vector<T>` | `T`, any `T` | `Vector<T>` | `BuiltinFailure(Index)` |

Judgment (lowering and the section 3.1 entry alike): the immediate must be
`Entity` naming a carried import that resolves to exactly one frozen
entry, else `VM_LOWER_OPCODE_UNSUPPORTED` (an unapproved adapter stays an
unsupported operation however well formed its operands are, so the rest of
E7 remains closed by default; resolution precedes arity, so a mistyped
unapproved call is still unsupported, not mismatched); the two operand
types must equal the entry's scope and request types exactly — in
particular `V2B1` accepts only `Vector<UInt(8)>`, never any other width —
and the single declared result must equal `Result<response,
BuiltinFailure(Index)>` exactly, else `VM_LOWER_SIGNATURE_MISMATCH`.
Push instantiates generically by monomorphization: each concrete element
type `E` declares its own closed row with request `E` and response
`Vector<E>` (the row pins identity, purity, and the relationship, not just
identity and purity); per-use operand types must equal the carried row —
scope exactly the row response, request exactly the row request — and
execution revalidates the same binding against the row, which is what keeps
the rule byte-unaware and unbypassable through the public execution
helper. The top-of-judgment cell rule applies unchanged: no
entry takes a type containing `LocalCell`.

Execution is total and program-unaware. `B2V1` maps each request byte to
one `UInt(8)` element in order; `V2B1` maps each `UInt(8)` request element
back to its byte (an element value above 255 is an internal invariant
violation, never a silent wrap); `PSH1` appends one cloned request element
to the scope vector. The unit scopes carry no data and are rechecked, not
trusted. No entry parses tags, dispatches on schema, enforces canonical
form, assembles images, judges trust, or returns verdicts: over program
bytes the output is a `u8` vector, and the negative corpus pins exactly
that. Every strict-rejection decision stays in Sley code over these
vectors.

Capacity: no bridge byte string or octet vector exceeds `BRIDGE_MAX_ITEMS`
= 1,048,576 bytes/elements (2^20, symmetric with the `MAX_EXECUTION_CELLS`
cap). An input or result past the cap is `Err(BuiltinFailure(Index, 2))`,
the bridge-capacity code, distinct from the index-out-of-range code 1 the
`VectorSet` precedent pins. Single values past 1 MiB cannot cross in one
call; that is a profile bound, and raising it needs a profile revision with
contract review, not a quiet implementation change. The RW-030 admission
record's "typed Limit failure" is realized as this `Index` code 2: the
`BuiltinFailureKind` set is epoch-closed (manifest kinds 1–5), while codes
are per-kind values, and capacity refusal belongs to the collection-bounds
family the `Index` kind already owns.

Fuel: the existing per-instruction and per-terminator charges cover
dispatch, and bridge fuel is charged up front, before the arm allocates or
converts: every request element of a conversion and the single pushed
element charges one fuel through `charge_action`
(`BRIDGE_ELEMENT_FUEL` = 1), so a starved budget terminates without the
work being performed (the E6 call-fuel precedent). The call-site value-unit
charge covers the result as for every operation. The exact cap and
per-element charge frozen here are the RW-050 profile-freeze values the
admission record requires; the RW-070 host-ABI freeze records the entry
identities, schemas, and denial tests.

Threat posture: T25 (adapter impersonation) cannot arise — the immediate
must name a carried import whose every frozen field equals the frozen
bridge values, never caller-selected adapter resolution, and anything
else is refused at judgment; T26 (adapter response injection) cannot
arise — scope/request/response schemas are pinned at judgment, execution
re-resolves from the same inventory and rechecks data shapes, and a
mismatch is an internal fault, never a mistyped value.

Permission note (owner amendment A1, S20-230 §1.5 — supersedes the slice-1
profile-local note): pure, effectless imports are permitted exactly these
three frozen rows, with generic instantiation for push. This is not a
general adapter permission: S20-230 §1.4 requires one `AdapterCall` effect
because zero effects on a host-state import could hide authority — a pure
value function has no host-state authority to hide, so the rationale does
not attach, and the S20-230 validator now judges the frozen pure-row shapes
at declaration plus scope/operand/result binding at invocation (the E7a
effect-free predicate precedent). Effectful adapters keep their owners
(S20-240 full, S20-280 full, S20-380 full); the profile's refusal of
effectful Functions is untouched, so bridge callers stay effect-free and no
`AdapterCall` judgment is delegated anywhere. Positive registration (exact
bridge identity/version) is enforced at lowering from the supplied import
inventory: unregistered zero-effect imports fail closed here even where
their static form is valid.

### 3.1 Judgment without lowering

`judge_function_operations(input)` judges every operation of one Function
under `EXTENDED_V1` and returns the operation count and the judgment work,
without emitting bytecode, deriving a cache key, lowering callees, or
executing anything. It is the surface external owners use: S20-360 candidate
validation calls it once per function unit after the S20-220 graph report.
Callers must apply their owner's capability prefilter before judging:
S20-360 uses `CandidateProgram::operation_analysis_supported()` and skips
phase-7 operation judgment when it is false, preserving the later owning
phase's diagnosis/refusal. Its excluded-opcode policy is owned by
`sley-policy/src/candidate_program.rs`; E7a/E8 executable support does not
by itself widen phase-7 analyzability. `VM_LOWER_OPCODE_UNSUPPORTED` is a
capability result, not a general verdict that the program is invalid.
The stage checker pins this consumer guard before its judgment call.
The entry first landed in revision 8 and is unchanged in substance since;
the campaign record cites revision 8 section 3.1 for that landing.

Unlike `lower_function` it does not refuse a Function that declares type
parameters, effects, or contracts, because those belong to the S20-210,
S20-230, and S20-240 owners; a caller that needs bytecode still uses
`lower_function` and gets the frozen refusals. Failures keep their exact
`VM_LOWER_*` codes, so a caller can map them onto its own decisions without
inventing a code. A restricted-profile request is
`VM_LOWER_PROFILE_UNSUPPORTED`.

Normative acceptance invariant: judgment accepts exactly the Functions
`lower_function` accepts under `EXTENDED_V1`, minus the type-parameter /
effect / contract refusal, the graph validation, the cache key, the
bytecode, and callee lowering. In particular the judgment runs
`require_canonical_referenced_constants` in the same position as lowering
(after the judgment, so the frozen S20-260 failure order is unchanged), so a
judged Function with no type parameters, effects, or contracts is lowerable.
The differential test
`judgment_acceptance_matches_lowering_acceptance` pins both directions of
the invariant: lowering accepted implies judgment accepted, and judgment
refused implies lowering refused.

The judgment ignores `LoweringInput.state_root` and
`LoweringInput.schema_epoch`: both feed only `derive_cache_key`, which the
judgment never calls. No judgment code may start reading them without a
contract change. The caller supplies the same root-wide inventories and the
same per-unit Function that lowering takes; the judgment re-narrows them to
the unit exactly as lowering does, which is a no-op when the caller already
narrowed per unit.

## 4. Observation and reports

The admitted epoch-1 profiles share the frozen `vm_version = [1,0,0]`.
The existing observation encoder may therefore use that common version;
`epoch_one_profiles_share_the_frozen_observation_vm_version` and the stage
checker enforce equality for the two admitted profiles. A future differing
version must not enter that encoder without a versioned contract change.
Observation/report decoders share `ResourceKind` tags 1 through 5; tag 5
means CallDepth and is valid decoding vocabulary. Restricted execution
cannot produce it; reachability differs from the shared wire vocabulary.


The observation preimage of S20-270 is unchanged; the extended profile
enters it through the cache key and the new resource kind, and every
`Success` value is hashed with `hash_validated_value` as before. The
preimage's own `execution_profile` field stays at its frozen value because
the cache key it already carries binds `lowering_profile` in its preimage
(`cache_key_preimage` hashes the field), so no two
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
signature rule; 128 repeated vector executions producing equal outcomes under
the pinned toolchain (the count belongs to the vectors: the fuzz lane asserts determinism twice per
draw instead, and cross-host identity needs the section E3 floating-point environment); the `vm_canonical_inputs` persistent slice extended with one
lane per landed family; and the restricted vectors unchanged. Each family lane
must reach a completed termination, success or the family's value failure, on
the pinned corpus: the lane builds its request from the fixture's own
parameter types under generous limits and asserts the first execution
completes, while the restricted-profile refusal pins
`VM_LOWER_OPCODE_UNSUPPORTED` for the single-graph families. A lane that stops
reaching execution fails loudly instead of comparing two input errors, and the
runner requires the executed run count to cover the corpus. Lanes carry
per-family reachability; vectors plus the rejection matrix carry per-opcode
duty. For the judgment entry: the differential test
`judgment_acceptance_matches_lowering_acceptance` runs on every change to
either entry and asserts the section 3.1 acceptance invariant in both
directions, so the two paths' static acceptance sets cannot silently diverge
again. For the profile: Tier 1 plus
Tier 2 validation, and the Ariadne, Nabu, and Vulcan reviews with every
report-grade finding closed.

The corpus retains `call-direct-depth-ceiling` as a successful 256-frame
boundary and adds `call-direct-depth-exceeded` for the refused 257th frame.
The latter records `termination = {kind: ResourceLimit, resource: CallDepth,
tag: 5}`, observation identity, instruction count and fuel used, and has no
success-value hash. `map-new-duplicate-key` remains successful VM execution
whose value is exactly `Err(BuiltinFailure(DuplicateKey,1))`; its metadata,
value hash and observation identity are frozen separately. The native
emitter asserts both outcomes. The independent Python oracle checks bytecode
container/cache identities and record shape, not execution semantics.

## 6. Explicit exclusions

This contract does not claim: E7 execution beyond slices E7a and E8; the E7
handle-model subsection authorizes no execution, and revision 17 implements
only its result-escape guard; generic specialization or type
arguments; an optimizer; effects, adapters, capabilities, replay, or live
cancellation beyond S20-270's rules; a second host or byte-memory budget;
S20-360 full operation analysis; or GA.

## 7. Revisions 2 through 7 clarifications (slices E1 through E6)

- E1 equality excludes any type containing `F32` or `F64`: structural
  `ConstValue` equality would make `NaN == NaN` true, contradicting the IEEE
  rule E3 fixes (S20-210 counts floats as hashable, which is why the
  exclusion needs stating); E3 defines float equality and order under IEEE.
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
  and rounding rules are the normative correctly-rounded environment in
  section E3, not an implementation-defined host choice; `float_fma` has
  one fused rounding.
- E4: record and variant immediates must name non-generic definitions of
  the lowering environment (a generic definition, an unknown member, a
  record named as a variant, or `variant_get` on a payload-less case is
  `VM_LOWER_IMMEDIATE_MISMATCH`); a map key type needs the S20-210 total
  order and no float (a float key already fails the S20-220 graph check
  with `TYPE_NOT_ORDERABLE`, so the lowering guard is defense in depth); `map_new` with no operands takes its key and value
  types from the declared result; the runtime map order is the
  lexicographic order of the keys' S20-350 canonical bytes, obtained
  through `sley_mutate::encode_const_value` (the VM crate now depends on
  the mutation crate, which the dependency direction allows), and duplicate
  detection and every probe compare those bytes, so key identity is the
  encoding and the map `map_new` accepts is exactly the map the codec
  accepts; `map_insert` of an existing key replaces its value with the
  order recomputed, and `map_remove` of an absent key returns the map
  unchanged. That order
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
  loop could hold unbounded host memory with the budget intact); at most
  1,048,576 cells exist in one execution and the count check fires at the
  cap;
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
  termination counts no call instructions), and refuses the frame that would
  make 257 live, so 256 live frames is the deepest reachable stack (entry frame included) with `ResourceLimit(CallDepth)`,
  tag 5, which occurs only under `EXTENDED_V1`; the restricted profile's
  execution still reaches only tags 1 through 4, while shared decoders
  accept the five-tag vocabulary described in section 4. The call stack is explicit
  (suspended caller frames in a list), so the ceiling never depends on the
  host stack.

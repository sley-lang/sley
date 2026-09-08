# Candidate Result and Validation Pipeline v1

Status: S20-360 normative contract implemented for the restricted conformance
epoch. Supported success subset: the restricted subset plus programs carrying semantic `Operation`
entities of the extended families E1 through E6, judged by the S20-360 full
operation analysis (revision 2, ADR-0045); E7 opcodes are refused at phase 12
(section 9; the earlier "executable-program-operation-free" status described
the pre-ADR-0045 subset and was corrected 2026-09-08). Full-GA operation
coverage remains incomplete; restricted S20-390 fixed-head commit is
implemented as a separate layer.

## 1. Boundary and authority

Candidate validation is a deterministic, read-only judgment over one stored
candidate and one explicit trusted context. Candidate fields are comparison
targets, not authority. Validation never mutates the accepted root, object
store, repository, ref, transaction graph, capability ledger, policy root,
candidate bytes, or host state.

A `CandidateResult` is immutable evidence. Its digest proves exact result
bytes, not authorization to commit. Only a `VALID` result is eligible for the
separate S20-390 commit recheck, and no S20-360 API performs that recheck or a
durable action.

The implementation may validate the explicitly supported conformance-epoch
semantic subset. Any unimplemented semantic, capability, resource, object, or
root judgment fails closed; it cannot produce `VALID`.

## 2. Trusted validation context

The host supplies one closed validation context containing:

- exact accepted base `TransactionId` and registry-authorized
  `AcceptedStateRoot`;
- the complete strictly decoded SSMC1 entity-object inventory whose IDs equal
  every base-root binding, with no missing or surplus object;
- a canonical set of tombstoned `EntityId` values;
- the exact accepted schema epoch and decoder set;
- the registry-authorized `AcceptedPolicyRoot`;
- the authenticated `PrincipalId`;
- capability tokens plus explicit trusted issuer/key/secret facts sufficient
  to rebuild the proposal's capability summary without serializing secrets;
- explicit trusted `now_unix_millis` and local ceilings no looser than the
  schema, profile, or policy.

The context contains no caller-supplied phase outcome, semantic-pass claim,
candidate-root claim, test-plan claim, or diagnostic suppression. A
`ValidationContextDigest` is validator-owned evidence over the canonical
public projection: accepted base transaction/root, schema, policy, principal,
rebuilt capability-summary digest, trusted time, object-inventory digest,
tombstone digest, and effective ceilings. Host secrets, token authenticators,
raw token bytes, ledger memory, paths, and handles are excluded.

The canonical public projection is one exact eleven-field SCB1 Record:

| Tag | Field | Encoding |
|---:|---|---|
| 1 | context format | `UInt32`, exactly `1` |
| 2 | accepted base transaction | exact `TransactionId` bytes |
| 3 | accepted base state | exact `StateRoot` bytes |
| 4 | accepted schema epoch | exact `SchemaEpochId` bytes |
| 5 | accepted policy | exact `PolicyRootId` bytes |
| 6 | principal | exact `PrincipalId` bytes |
| 7 | rebuilt capability summary | exact `CapabilitySummaryDigest` bytes |
| 8 | trusted validation time | `UInt64` milliseconds |
| 9 | object inventory digest | exact component digest |
| 10 | tombstone digest | exact component digest |
| 11 | effective ceilings | exact nine-field limits Record |

The inventory component is a canonical List of two-field Records containing
`EntityId` and `ObjectId`, sorted by their encoded record bytes. The tombstone
component is a canonical raw-ID-sorted List of `EntityId` bytes. Their exact
digests are:

```text
inventory_digest = BLAKE3-256(
  "sley2.validation-context-inventory.v1" ||
  u64be(byte_length(canonical_inventory_list)) || canonical_inventory_list
)
tombstone_digest = BLAKE3-256(
  "sley2.validation-context-tombstones.v1" ||
  u64be(byte_length(canonical_tombstone_list)) || canonical_tombstone_list
)
```

The limits Record contains, in order, maximum operations, preconditions,
candidate bytes, decoded/change bytes, graph work, selected tests, entities,
test call depth, and test wall timeout. Values are the effective minima after
clamping requested local limits to the full-v1 profile, SCB1, policy, and
implementation hard ceilings. A looser requested limit therefore cannot alter
authority or produce a different effective context.

The exact digest wrapper is:

```text
ValidationContextDigest = BLAKE3-256(
  "sley2.validation-context.v1" ||
  u64be(byte_length(canonical_public_projection)) ||
  canonical_public_projection
)
```

This formula alone grants no context authority. The in-process validator owns
construction of the closed public projection; a caller-supplied byte string or
digest is never accepted as a phase result.

## 3. Candidate attempt and result identity

Every byte attempt has a causal digest, including malformed input:

```text
attempt_preimage = "SLEYATT1" || u64be(len(stored_candidate_bytes)) ||
                   stored_candidate_bytes
CandidateAttemptDigest =
  BLAKE3-256("sley2.candidate-attempt.v1" || attempt_preimage)
```

This digest is not a `CandidateId` and cannot stand in for one. The result's
`candidate_id` is `Some` only after exact `SLEYCAN1` envelope, trailer, record,
and structural verification. Invalid encoding therefore never fabricates a
candidate identity.

The canonical result envelope is:

```text
result_preimage = "SLEYCRS1" || uvar(1) ||
                  len(candidate_result_record) || candidate_result_record
CandidateResultId =
  BLAKE3-256("sley2.candidate-result.v1" || result_preimage)
stored_result = result_preimage || CandidateResultId[32]
```

The digest trailer is outside its own preimage and no bytes follow it.

## 4. Candidate result record

All thirteen fields are required. `Option<T>` uses SCB1 tags `0=None` and
`1=Some<T>`.

| Tag | Field | Type |
|---:|---|---|
| 1 | format_version | `UInt32`, exactly `1` |
| 2 | candidate_attempt_digest | `FixedBytes<32>` |
| 3 | candidate_id | `Option<CandidateId>` |
| 4 | validation_profile_id | exact full-v1 `ValidationProfileId` |
| 5 | validation_context_digest | `FixedBytes<32>` |
| 6 | decision | closed `CandidateDecision` |
| 7 | phase_results | exactly fourteen ordered `PhaseResult` records |
| 8 | diagnostics | ordered `List<CandidateDiagnostic>` |
| 9 | affected_closure | raw-ID-sorted `Set<EntityId>` |
| 10 | required_capabilities | raw-ID-sorted `Set<EntityId>` |
| 11 | selected_tests | raw-ID-sorted `Set<EntityId>` |
| 12 | candidate_root | `Option<StateRoot>`; `Some` iff `VALID` |
| 13 | validated_at_unix_millis | trusted explicit `UInt64` |

A candidate root is present exactly for `VALID`; every other decision carries
`None`.

For invalid encoding, fields derived from decoded candidate semantics are
empty, `candidate_id=None`, and phase 1 alone is failed. For every other
decision, `candidate_id=Some(exact verified CandidateId)`.

Each `PhaseResult` is the exact required four-field Record:

| Tag | Field | Type |
|---:|---|---|
| 1 | phase_tag | `UInt32`, exact ordinal `1..=14` |
| 2 | outcome | `PhaseOutcome` |
| 3 | evidence_digest | `Option<FixedBytes<32>>` |
| 4 | terminal_decision | `Option<CandidateDecision>` |

Each `CandidateDiagnostic` is the exact required six-field Record:

| Tag | Field | Type |
|---:|---|---|
| 1 | phase_tag | `UInt32` |
| 2 | result_code | frozen S20-360 `UInt32` |
| 3 | source_numeric_code | `Option<UInt32>` |
| 4 | source_symbol | bounded ASCII `Text` |
| 5 | retryability | `DiagnosticRetryability` |
| 6 | causal_digest | `Option<FixedBytes<32>>` |

Diagnostic retryability tags are `1=PERMANENT`, `2=FRESH_BASE`,
`3=FRESH_AUTHORITY`, `4=HIGHER_CEILINGS`, and `5=INTERNAL_REPAIR`. Source
symbols are 1 through 96 ASCII bytes, begin with `A..Z`, and contain only
`A..Z`, `0..9`, or `_`. Diagnostics are strictly ordered by their complete
typed record, contain no duplicates, and all bind the one failed phase and its
outer result code. `VALID` carries no diagnostics; every invalid decision
carries at least one primary diagnostic. The primary diagnostic is exactly
list element zero after that strict typed ordering; it has no redundant marker
field. Import derives and validates this element explicitly, so an empty list
is an omitted-primary failure.

The three entity-ID sets are strictly raw-ID ascending and duplicate-free,
with at most 65,535 members each. Invalid encoding carries empty sets. The
diagnostic list is bounded to 1,024 records.

## 5. Decision tags

Tags are closed and preserve the master-goal states:

| Tag | Decision | Failed phase |
|---:|---|---:|
| 1 | `VALID` | none |
| 2 | `INVALID_ENCODING` | 1 |
| 3 | `INVALID_SCHEMA` | 2 |
| 4 | `STALE_ROOT` | 3 |
| 5 | `STALE_ENTITY` | 3 |
| 6 | `INVALID_IDENTITY` | 4 |
| 7 | `INVALID_GRAPH` | 5 |
| 8 | `UNRESOLVED_REFERENCE` | 5 |
| 9 | `TYPE_ERROR` | 6 |
| 10 | `CONTROL_FLOW_ERROR` | 7 |
| 11 | `EFFECT_ERROR` | 8 |
| 12 | `CAPABILITY_DENIED` | 9 |
| 13 | `CONTRACT_ERROR` | 10 |
| 14 | `RESOURCE_LIMIT` | the first bounded phase that exhausted its ceiling |
| 15 | `TEST_PLAN_ERROR` | 11 |
| 16 | `INTERNAL_ERROR` | the first phase whose invariant failed closed |

`RESOURCE_LIMIT` records the actual first failed phase; it never permits a
later phase to run. Unknown decisions are invalid result bytes.

Tag 10 `CONTROL_FLOW_ERROR` at phase 7 carries two owner classes separable
only by `source_symbol`: S20-220 structure failures (the `CFG_*` family) and
S20-260 signature failures (the `VM_LOWER_*` family, section 8.2). Both
report `Permanent` retryability, so section 8.1's one-retryability-per-symbol
rule is not violated. No seventeenth tag is added: the closed sixteen-tag
enum, the 36000–36014 range, and the fixed vectors stay frozen. The GA
revision that lands E7 runtime ownership must split tag 10 by owner class.

## 6. Phase records and monotonicity

The exact phase order is:

1. canonical frame;
2. schema and limits;
3. stale base and preimages;
4. identity;
5. graph and references;
6. type;
7. CFG;
8. effects;
9. protected capability and policy;
10. contracts;
11. test planning;
12. supported resource analysis;
13. candidate-root construction;
14. final candidate/result digest generation.

Each `PhaseResult` is a four-field record: `phase_tag`, `outcome`, optional
`evidence_digest`, and optional `terminal_decision`. Outcomes are `1=PASSED`,
`2=FAILED`, and `3=NOT_RUN`. Passed and failed phases carry validator-derived
evidence; not-run phases carry neither evidence nor a decision. A valid result
has fourteen passed phases. An invalid result has a passed prefix, exactly one
failed phase whose decision equals the outer decision, and a not-run suffix.

Phase evidence uses:

```text
PhaseEvidenceDigest = BLAKE3-256(
  "sley2.candidate-phase-evidence.v1" ||
  u32be(phase_tag) || uvar(byte_length(canonical_phase_input_output)) ||
  canonical_phase_input_output
)
```

Phase evidence cannot contain a caller-provided pass flag. Import validates
the monotonic shape but does not rerun or authenticate the underlying
judgment; only the in-process validator creates authoritative result objects.

For a successful phase 14, `canonical_phase_input_output` is the
validator-owned final-result core: result fields 1 through 6 and 8 through 13,
phase records 1 through 13, and phase-14 tag/outcome with its evidence and
terminal fields omitted. It excludes `CandidateResultId`, the stored result
bytes, and its own `PhaseEvidenceDigest`, preventing a self-hash cycle. The
canonical result envelope then binds that phase-14 evidence and every other
record byte. A terminal result whose earlier phase failed may still receive
its envelope integrity digest from the failure renderer; that does not mark
phase 14 as executed or passed.

## 7. Required phase judgments

- Phase 1 strictly imports candidate bytes and verifies `CandidateId`.
- Phase 2 binds the exact full-v1 profile, schema epoch, candidate and decoded
  value limits, context completeness, and effective ceilings.
- Phase 3 compares accepted transaction/root/epoch/policy/capability bindings,
  checks explicit time against candidate and policy expiry, and verifies every
  exact entity/container preimage against the base binding.
- Phase 4 rechecks deterministic creation IDs and rejects collision with live
  or tombstoned identities.
- Phase 5 applies operations only to an in-memory clone, checks operation-local
  invariants, derives the exact complete reference graph, and distinguishes
  malformed graph structure from a missing referenced identity.
- Phases 6 through 8 invoke the owning S20-210/S20-220/S20-230 checkers and
  preserve their exact source code in diagnostics.
- Phase 7 judges every operation of every function unit through the S20-260
  judgment owner after the same unit's S20-220 graph report; a program
  containing an excluded E7 opcode skips judgment program-wide and keeps its
  frozen phase 12 refusal (section 9).
- Present semantic-fingerprint claims on supported TypeDef and Function
  entities are recomputed only after their owning semantic checker passes.
  A mismatch is reported at phase 6 for TypeDef or phase 8 for Function while
  preserving `FINGERPRINT_MISMATCH` as the source symbol. Present claims on an
  unsupported entity kind fail closed. The restricted conformance epoch allows
  absent claims; complete production-epoch assembly must require every
  master-goal-mandatory claim before GA.
- Phase 9 independently rebuilds and compares the capability summary, verifies
  authenticated token bindings where present, enforces policy mutation-class
  grants and ceilings, and runs protected ordinary-program isolation.
- Phase 10 invokes the owning contract checker.
- Phase 11 finalizes the checker-produced plan against protected mandatory
  tests/contracts; caller-selected tests are forbidden.
- Phase 12 charges deterministic operation, decoded-value, graph, checker,
  selected-test, and candidate-object/root construction work against the
  narrowest applicable ceiling. Unsupported analysis fails closed.
- Phase 13 canonically builds every changed SSMC1 object and the candidate
  `StateRoot` in memory. The policy, schema, contract root, and test root stay
  unchanged for an ordinary candidate.
- Phase 14 derives the result from validator-owned phase evidence. It performs
  no commit, I/O, ledger charge, ref change, or accepted-state mutation.

## 8. Diagnostics and stable result codes

Diagnostics are deterministic, bounded records containing phase tag, one
S20-360 numeric/result symbol, optional preserved source numeric code, exact
source symbol, retryability tag, and optional safe causal digest. They contain
no free-form host error, secret, path, source text, model text, or unbounded
payload. Unknown or omitted primary diagnostics invalidate imported result
bytes.

Numeric codes 36000 through 36014 correspond in order to every non-`VALID`
decision listed in Section 5. Their symbols are the decision prefixed with
`CANDIDATE_VALIDATION_`, for example
`CANDIDATE_VALIDATION_INVALID_ENCODING`. Exact underlying `SCB_*`, `TYPE_*`,
`CFG_*`, `EFFECT_*`, `POLICY_*`, `CAP_*`, and `CONTRACT_*` symbols remain in
the source-code field and are never collapsed into success.

### 8.1 Source symbols the validator originates

Most source symbols are preserved from the owning checker that produced the
failure. Twenty-six are the validator's own, because the check belongs to no
other owner: it is the validator that compares the bound context, re-derives
identity, rebuilds the capability summary, and rebuilds the root. They are
enumerated here so a consumer reading `source_symbol` can resolve every value
the validator can emit, and `scripts/check_candidate_result_contract.py`
verifies that this table and the implementation name the same set.

| Phase | Decision | Source symbol |
|---:|---|---|
| 2 | `INVALID_SCHEMA` | `CANDIDATE_CONTEXT_INVENTORY_MISMATCH` |
| 2 | `INVALID_SCHEMA` | `CANDIDATE_CONTEXT_POLICY_BINDING_INVALID` |
| 2 | `INVALID_SCHEMA` | `CANDIDATE_CONTEXT_STATE_ROOT_MISMATCH` |
| 2 | `INVALID_SCHEMA` | `CANDIDATE_CONTEXT_TOMBSTONE_SET_INVALID` |
| 2 | `INTERNAL_ERROR` | `CANDIDATE_CONTEXT_STATE_REGISTRY_INVALID` |
| 3 | `STALE_ENTITY` | `CANDIDATE_APPLY_EXACT_PREIMAGE_MISMATCH` |
| 3 | `STALE_ROOT` | `CANDIDATE_EXPIRY_EXPIRED` |
| 3 | `STALE_ROOT` | `POLICY_ROOT_EXPIRED` |
| 4 | `INVALID_IDENTITY` | `CANDIDATE_IDENTITY_COLLISION` |
| 4 | `INVALID_IDENTITY` | `MUTATION_CANDIDATE_PRECONDITION_MISMATCH` |
| 4 | `INVALID_IDENTITY` | `MUTATION_CANDIDATE_TARGET_ENTITY` |
| 4 | `RESOURCE_LIMIT` | `SSMC_RESOURCE_LIMIT` |
| 5 | `INVALID_GRAPH` | `CANDIDATE_DEPENDENCY_ROOT_CHANGE_UNSUPPORTED` |
| 5 | `INVALID_GRAPH` | `STATE_ROOT_ENTRY_POINT_KIND_INVALID` |
| 9 | `CAPABILITY_DENIED` | `CAPABILITY_SUMMARY_MISMATCH` |
| 9 | `CAPABILITY_DENIED` | `CAPABILITY_REQUIREMENT_MISSING` |
| 9 | `CAPABILITY_DENIED` | `CAPABILITY_REQUIREMENT_UNRESOLVED` |
| 9 | `CAPABILITY_DENIED` | `CAPABILITY_REQUIREMENT_EFFECT_UNRESOLVED` |
| 9 | `CAPABILITY_DENIED` | `CAP_BUDGET_EXCEEDED` |
| 9 | `CAPABILITY_DENIED` | `CAP_EFFECT_MISMATCH` |
| 9 | `CAPABILITY_DENIED` | `CAP_PRINCIPAL_MISMATCH` |
| 9 | `CAPABILITY_DENIED` | `CAP_SCOPE_MISMATCH` |
| 9 | `CAPABILITY_DENIED` | `POLICY_GRANT_DENIED` |
| 12 | `RESOURCE_LIMIT` | `CANDIDATE_OPERATION_ANALYSIS_UNSUPPORTED` |
| 12 | `INTERNAL_ERROR` | `CANDIDATE_SELECTED_TEST_UNRESOLVED` |
| 13 | `INTERNAL_ERROR` | `CANDIDATE_ROOT_REBUILD_MISMATCH` |

The four phase-9 `CAPABILITY_*` symbols name the validator's independent
rebuild of the capability summary, which is why they are not `CAP_*`: the
`CAP_*` family belongs to the S20-380 token authority, and these failures
occur before any token is verified.
`CAPABILITY_REQUIREMENT_EFFECT_UNRESOLVED` is the clearest case: the
program's own requirement names an effect that does not resolve, so no token
is involved at all. It previously reported the token authority's
`CAP_EFFECT_MISMATCH`, which both misattributed the failure and gave that
one symbol two retryability answers.

A symbol reports one retryability. `scripts/check_candidate_result_contract.py`
verifies that, because a consumer that reads `(code, symbol)` and gets
contradictory retry guidance cannot act on either answer. A phase pass record may also carry the
marker `CONTRACT_PREFIX_PASSED`, which records that the contract checker
accepted every contract before a later phase failed; it is evidence, not a
failure.

The result symbol is the unique mapping of `result_code` and is therefore not
duplicated as a second text field. Result-integrity shape failures use numeric
codes 36100 through 36107 under `CANDIDATE_RESULT_*`; strict SCB1 syntax,
canonicality, resource, envelope, and digest failures retain their exact
`SCB_*` source code.

### 8.2 Preserved VM lowering symbols at phase 7

Phase 7 preserves the exact `VM_LOWER_*` symbol and numeric code of a
judgment failure in the diagnostic source-code field; the validator never
renames them. The set is closed — it is exactly the `LowerErrorCode`
enumeration in `crates/sley-vm/src/lib.rs` — and
`scripts/check_candidate_result_contract.py` verifies this table names the
same set. The phase 7 decision mapping is: a signature or immediate mismatch
is `CONTROL_FLOW_ERROR` (carrying the two owner classes of section 5), a
lowering resource ceiling is `RESOURCE_LIMIT`, and any other lowering failure
is `INTERNAL_ERROR`.

| Source symbol | Numeric code | Phase 7 decision |
|---|---|---|
| `VM_LOWER_PROFILE_UNSUPPORTED` | 26000 | `INTERNAL_ERROR` |
| `VM_LOWER_OPCODE_UNSUPPORTED` | 26001 | `INTERNAL_ERROR` |
| `VM_LOWER_SIGNATURE_MISMATCH` | 26002 | `CONTROL_FLOW_ERROR` |
| `VM_LOWER_IMMEDIATE_MISMATCH` | 26003 | `CONTROL_FLOW_ERROR` |
| `VM_LOWER_LOCAL_REFERENCE_INVALID` | 26004 | `INTERNAL_ERROR` |
| `VM_LOWER_CACHE_KEY_UNSUPPORTED` | 26005 | `INTERNAL_ERROR` |
| `VM_LOWER_RESOURCE_LIMIT` | 26006 | `RESOURCE_LIMIT` |

## 9. Acceptance and explicit gaps

Acceptance requires exact result round trips and fixed vectors; all sixteen
decisions; every phase as the first failure; stale root/entity/preimage tests;
identity collision and tombstone tests; graph/reference distinction; exact
type/CFG/effect/contract source-code preservation; capability-summary,
expiry, mutation-grant, policy-isolation, and mandatory-test failures; resource
ceilings; byte-identical repeated valid results; invalid-state immutability;
and persistent fuzzing of result import and monotonic phase shape.

The landed slice judges every operation whose opcode belongs to the
S20-260/S20-270 extended families E1 through E6: for a program with no
excluded opcode, phase 7 calls the VM owner's judgment entry once per
function unit after the S20-220 graph report, charges its work, and records
the judged-operation count, the judgment work, and the analyzability flag in
the phase evidence. The flag distinguishes "judged, zero operations" from
"judgment skipped for an excluded opcode" — both otherwise read as
`judged_operations=0` — and phase evidence is part of result identity. A
program containing any excluded E7 opcode skips phase 7 judgment for every
unit and keeps its frozen phase 12 refusal instead of failing here. A
judgment failure keeps its exact `VM_LOWER_*` symbol and numeric code
(section 8.2): a signature or immediate mismatch is a phase 7
`CONTROL_FLOW_ERROR`, a lowering resource ceiling is a phase 7
`RESOURCE_LIMIT`, and any other lowering failure is a phase 7
`INTERNAL_ERROR`.

Excluded-opcode invariant: every excluded opcode is refused before phase 12
— unconditionally by its owner, or by shape validation at its owner's phase
— and the phase 12 guard that answers `RESOURCE_LIMIT` with source symbol
`CANDIDATE_OPERATION_ANALYSIS_UNSUPPORTED` is the live residual refusal path
for the well-formed E7 programs that clear their owners, not defense in
depth. The five excluded E7 opcodes are contract assertion 144, test
observation 145, effect request 160, adapter invocation 161, and capability
narrowing 162. The per-opcode owner phases, refusal symbols, and proving
tests live in the closeout addendum, which is evidence: when an E7 owner
lands, its opcode becomes analyzable by removing it from
`EXCLUDED_OPERATION_OPCODES` and revising the addendum, without revising
this contract. Opcode 144 is owned since profile revision 9 slice E7a, which
lowers and executes `contract_assert`; it stays excluded from phase 7
because its static typing belongs to the S20-240 checker at phase 10. The
validator still exercises all fourteen phases, all sixteen terminal
decision encodings, complete all-18-kind reference extraction, native
type/CFG/effect/contract owners, capability and policy checks, mandatory test
planning, in-memory root reconstruction, and byte-identical result
generation.

S20-360 does not authorize policy transitions, mutate accepted state, consume
runtime capability budget, execute tests or effects, write objects, commit,
create receipts, move refs, perform CAS, access a repository, open a session,
invoke a provider, deploy, publish, or complete M3/M4/GA. S20-390 remains the
first package allowed to perform durable commit after an exact recheck.

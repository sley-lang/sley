# Error Codes v1

Status: M0 namespace draft with package-frozen sections. Numeric assignments
are not globally frozen except where an owning work package says so.

Every failure response binds protocol/schema version, phase, stable symbolic
and numeric code, typed details, safe causal IDs, retryability, mechanically
established repair affordances, and truncation. Prose is optional.

Three properties hold over the whole tree and
`scripts/check_error_symbol_registration.py` enforces them: every symbol a
crate emits is assigned by a contract under `docs/spec/`, not merely mentioned
somewhere; one numeric code carries one symbol; and every symbol is reached by
a test, corpus, fuzz target, or oracle, by its string or by its enum variant.
A document outside `docs/spec/` cannot register a symbol, because prose can
name a code that no owner ever assigned.

## Candidate terminal states

`VALID`, `INVALID_ENCODING`, `INVALID_SCHEMA`, `STALE_ROOT`, `STALE_ENTITY`,
`INVALID_IDENTITY`, `INVALID_GRAPH`, `UNRESOLVED_REFERENCE`, `TYPE_ERROR`,
`CONTROL_FLOW_ERROR`, `EFFECT_ERROR`, `CAPABILITY_DENIED`, `CONTRACT_ERROR`,
`RESOURCE_LIMIT`, `TEST_PLAN_ERROR`, and `INTERNAL_ERROR`.

Only `VALID` permits commit. `INTERNAL_ERROR`, unknown, incomparable, missing,
and ambiguity are failures, never success.

## Namespaces

- `SCB_*`: framing, canonical encoding, schema, epoch, digest, and limits.
- `ID_*`: identity derivation, collision, reuse, and workspace mismatch.
- `STORE_*`: immutable-object lookup, substitution, persistence, and local I/O.
- `STATE_ROOT_*`: root construction, duplicate input, and excluded-fact checks.
- `SSMC_*`: semantic-entity structure, closed tags, signatures, and limits.
- `GRAPH_*`, `TYPE_*`, `CFG_*`, `EFFECT_*`, `CONTRACT_*`: kernel judgment.
- `STALE_*`, `TXN_*`, `REF_*`, `RECOVERY_*`: transaction and durability.
- `POLICY_*`, `CAP_*`, `ADAPTER_*`: authority boundary.
- `QUERY_*`: typed request identity, snapshot binding, bounded traversal, and
  required-fact completeness.
- `SESSION_*`, `PROTOCOL_*`: bounded interface and negotiation.
- `FINGERPRINT_*`, `VALUE_HASH_*`, `IMPACT_*`: semantic projection and
  derived relationships.
- `INDEX_SNAPSHOT_*`: restricted derived-index record construction and
  bounded candidate inspection.
- `RESTRICTED_CAPSULE_*`: derived complete-query evidence projection and
  resource/invariant checks.
- `VM_LOWER_*`: validated deterministic derived-bytecode lowering.
- `TEST_PLAN_*`: canonical test-entity validation and provisional selection.
- `VM_*`, `TEST_*`: execution, cancellation, determinism, and oracle.
- `PACK_*`, `GC_*`, `MERGE_*`: repository operations.
- `MUTATION_CANDIDATE_*`: proposal-record structure and descriptor binding.
- `CANDIDATE_VALIDATION_*`: S20-360 terminal judgment and result integrity.

S20-170 freezes these repository-pack codes:

- `PACK_VERSION_UNSUPPORTED`
- `PACK_DIGEST_MISMATCH`
- `PACK_DIGEST_TREE_MISMATCH`
- `PACK_CANONICAL_ORDER`
- `PACK_DUPLICATE_ENTRY`
- `PACK_SCHEMA_UNSUPPORTED`
- `PACK_ROOT_INVALID`
- `PACK_OBJECT_MISSING`
- `PACK_OBJECT_UNEXPECTED`
- `PACK_OBJECT_CORRUPT`
- `PACK_RESOURCE_LIMIT`
- `PACK_COMPRESSION_UNSUPPORTED`
- `PACK_DECOMPRESSION_LIMIT` (reserved until a compressed profile exists)
- `PACK_PROFILE_UNSUPPORTED`

S20-180 freezes these garbage-collection codes:

- `GC_RESOURCE_LIMIT`
- `GC_ANCHOR_MALFORMED`
- `GC_ANCHOR_UNRESOLVED`
- `GC_ROOT_MISSING`
- `GC_ROOT_INVALID`
- `GC_DEPENDENCY_MISSING`
- `GC_OBJECT_REFERENCE_MALFORMED`
- `GC_OBJECT_MISSING`
- `GC_INVENTORY_INVALID`
- `GC_DRY_RUN_REQUIRED`
- `GC_EXCLUSIVE_LOCK_REQUIRED`
- `GC_DELETE_IO`
- `GC_REACHABILITY_VIOLATION`
- `GC_INTERNAL_INVARIANT`

Numeric ranges and exact detail schemas are frozen with their owning contract,
generated into all transports, and checked for drift. Bridges may not invent or
collapse codes.

S20-200 freezes numeric codes 20000 through 20015 for the exact `SSMC_*`
failures listed in `SSMC1.md`. Those codes cover structural schema judgment
only and never substitute for later `TYPE_*`, `CFG_*`, `EFFECT_*`,
`CONTRACT_*`, or `VM_*` results.

S20-210 freezes numeric codes 21000 through 21020 for the exact `TYPE_*`
failures listed in `TYPE_SYSTEM_V1.md`. They cover type well-formedness,
definition cycles, trait requirements, substitution, and constant/type
agreement only; they do not claim CFG, effect, contract, lowering, or runtime
judgment.

S20-220 freezes numeric codes 22000 through 22020 for the exact `GRAPH_*` and
`CFG_*` failures listed in `CFG_VALIDATION_V1.md`. They cover graph inventory,
reachability, dominance, value uses, target arguments, switch payloads, traps,
and bounded CFG work only; they preserve earlier `SSMC_*` and `TYPE_*` failures
and do not claim opcode semantics, effects, contracts, lowering, or runtime
judgment.

S20-230 freezes numeric codes 23000 through 23013 for the exact `EFFECT_*`,
`ADAPTER_*`, `CAPABILITY_*`, and `CONSTRAINT_*` failures listed in
`EFFECT_SYSTEM_V1.md`. They cover closed entity lookup, exact least effect
closure, direct-call/effect-operation typing, epoch-1 adapter effect
cardinality, static capability scope constants, contract-identity boundaries,
and bounded closure work only. Earlier type/CFG failures are preserved; these
codes do not claim protected-policy, runtime-token, adapter-execution,
contract-predicate, lowering, or VM judgment.

S20-240 freezes numeric codes 24000 through 24017 for the exact `CONTRACT_*`,
`TEST_PLAN_*`, and `CONTRACT_TEST_PLAN_*` failures listed in
`CONTRACT_TEST_PROFILE_V1.md`. They cover the restricted epoch-1 pure-function
contract/test profile and policy-incomplete deterministic selection only.
Unsupported invariants, effect/capability/resource bounds, effectful tests,
adapter replay/configuration, and observations fail closed. Runtime `TEST_*`,
protected-policy finality, predicate/test execution, resource evidence, and
reports remain later namespaces/packages.

S20-250 freezes numeric codes 25000 through 25012 for the exact
`FINGERPRINT_*`, `VALUE_HASH_*`, and `IMPACT_*` failures listed in
`FINGERPRINT_IMPACT_PROFILE_V1.md`. They cover the restricted epoch-1
TypeDef/Function projection, canonical value hashing, and exact impact edges
for modeled SSMC1 kinds 4 through 15. They do not claim a complete-root index;
kinds 1 through 3 and 16 through 18 remain unsupported until their semantic
bodies enter the Rust model.

S20-250 full reserves numeric codes 25013 through 25024 for the exact
`IMPACT_ROOT_*` closure failures listed in
`COMPLETE_ENTITY_IMPACT_PROFILE_V1.md` (contract draft; frozen with that
contract). They cover the complete-root request over all eighteen SSMC1
kinds: binding and inventory equality, the single workspace, package
membership, namespace roots and tree, member ownership, export scoping,
entry-point and dependency-root equality, and dependency-binding ownership.
They do not claim semantic comparison, merge, or the full S20-300 snapshot.

S20-510 reserves numeric codes 51000 through 51010 for the exact `COMPARE_*`
failures listed in `SEMANTIC_COMPARISON_V1.md` (contract draft; frozen with
that contract). They cover the canonical semantic-delta record (version,
digest, canonical order, duplicates, format), the comparison preconditions
(shared workspace and epoch, complete roots, complete function inventories),
and resource ceilings; wrapped `SCB_*`, `IMPACT_*`, and `FINGERPRINT_*` codes
are preserved. They do not claim merge, conflict objects, or S20-520.

S20-520 reserves numeric codes 52000 through 52013 for the exact `MERGE_*`
failures listed in `MERGE_V1.md` (contract draft revision 4; frozen with
that contract).
They cover the verified common-ancestor preconditions, shared workspace and epoch,
unsupported dependency-root changes, wrapped comparison and extraction failures, resource
ceilings, the canonical conflict record with its strict decoder, unsupported plans
(including the single-candidate entry-point bound), the branch pre-check, and the
post-commit result check. Commit-path failures keep their exact `TXN_*`, `REF_*`, `CAP_*`, and `SCB_*`
symbols with owning numerics, including the exact frozen `REF_NAMED_CAS_STALE`
from the branch pre-check. A conflict is a successful judgment carrying a
conflict object, not a failure code.

S20-260 freezes numeric codes 26000 through 26006 for the exact `VM_LOWER_*`
failures listed in `VM_LOWERING_PROFILE_V1.md`. They cover only the restricted
epoch-1 O0 lowering profile for all five terminators and the three validated
Boolean opcodes, exact cache-profile binding, local rewrite invariants, and
resource ceilings. They do not claim semantic judgment for the other 52
opcodes, generic specialization, adapters, bytecode decoding, or execution.

S20-270 freezes numeric codes 27000 through 27005 for the exact `VM_EXEC_*`
failures listed in `VM_EXECUTION_PROFILE_V1.md`. They cover only integrated
restricted execution of S20-260 Boolean bytecode, all five terminators,
deterministic input/fuel/value/output/cancellation limits, traps, invariant
failures, and the observation digest. They do not claim the other 52 opcodes,
adapters/effects, live cancellation, or persistent S20-290 reports.

S20-280 freezes numeric codes 28000 through 28011 for the exact `ADAPTER_*`
failures listed in `REFERENCE_ADAPTER_PROFILE_V1.md`. They cover only the
restricted request-owned fixture profile: identity/ABI/effect/type boundaries,
canonical state and virtual paths, replay order, resource limits,
cancellation, and atomic in-memory mutation. Exact earlier `TYPE_*` and
`FINGERPRINT_*` failures are preserved. These codes do not claim VM adapter
opcode execution, protected capability judgment, or confined live host access.

S20-290 freezes numeric codes 29000 through 29007 for the exact `REPORT_*` and
`TEST_REPORT_*` failures listed in `REPORT_ENVELOPE_PROFILE_V1.md`. They cover
only deterministic derived-envelope profile/context/cache/observation/plan/
execution/resource consistency. Exact earlier `TYPE_*`, `FINGERPRINT_*`, and
`VM_LOWER_*` failures are preserved. These codes do not claim canonical report
entity validity, persistence, protected-policy finality, complete resource
evidence, a passed TestCase, or the M2 exit.

S20-300 restricted freezes numeric codes 30000 through 30007 for the exact
`INDEX_SNAPSHOT_*` failures listed in `INDEX_SNAPSHOT_PROFILE_V1.md`. They
cover bounded construction and private inspection of disposable `SLEYIDX1`
records for explicit modeled SSMC1 kinds 4 through 15. Candidate admission
always performs a fresh S20-250 rebuild before candidate inspection and
requires exact byte comparison before a hit; these codes do not establish root
provenance, authorize decoded cache edges, model
the six missing entity bodies, provide a useful performance cache, complete
full S20-300, or unblock root-backed S20-310.

S20-300 full reserves numeric codes 30008 through 30010 for the exact
`INDEX_SNAPSHOT_*` failures listed in
`COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` (contract draft; frozen with
that contract): an incomplete root (wrapping the exact `IMPACT_*` code), a
cached inventory that differs from the root's bindings, and cache I/O. They
cover the complete-root arm and the repository-owned index cache whose hits
serve read-only derived query surfaces only.

S20-310 restricted freezes numeric codes 31000 through 31007 for the exact
`QUERY_*` failures listed in `RESTRICTED_QUERY_PROFILE_V1.md`. They cover four
typed modeled-snapshot queries, exact `QueryId`/context/limit binding, hard
traversal and response ceilings, and explicit failure when an applied limit
would omit a required fact. They return no partial payload and do not implement
the nineteen root-backed query classes, truncation, continuation, capsules,
SMP1, full S20-310, the M3 blocker, or GA.

S20-310 full reserves numeric codes 31008 through 31010 for the exact
`QUERY_ROOT_MISMATCH`, `QUERY_CONTINUATION_INVALID`, and
`QUERY_CLASS_NOT_APPLICABLE` failures of `ROOT_BACKED_QUERY_PROFILE_V1.md`.
They cover the input binding between an arm-2 snapshot, verified bodies,
bindings, and root facts, typed continuation cursors, and the class-kind
applicability table of the nineteen root-backed classes. The entity-read
owner reuses 31004 (`QUERY_UNRESOLVED_ENTITY`), 31007
(`QUERY_INTERNAL_INVARIANT`), 31008 (`QUERY_ROOT_MISMATCH`), and 31010
(`QUERY_CLASS_NOT_APPLICABLE`) with owner-numeric identity under
`ENTITY_READ_PROFILE_V2.md`, composed into the S20-310 surface by
`ROOT_BACKED_QUERY_PROFILE_V1.md` section 11; no new error numbers are
allocated. Codes 31000 through
31007 keep their restricted meanings unchanged. The contract is a draft with
Council review pending and reserves, rather than freezes, these codes.

S20-320 restricted freezes numeric codes 32000 through 32007 for the exact
`RESTRICTED_CAPSULE_*` failures listed in
`RESTRICTED_QUERY_CAPSULE_PROFILE_V1.md`. They cover derived dictionaries,
direct-edge indexes, source-response binding, fixed complete/nontruncated/
noncontinuable status, and bounded record construction only. They do not
implement the master context capsule, use `ContextCapsuleId`, establish
workspace/root/session provenance, authorize continuation/import, or unblock
S20-330, S20-400, S20-620, M3, M5, or GA.

S20-320 full reserves numeric codes 32008 through 32011 for the exact
`CONTEXT_CAPSULE_SOURCE_INVALID`, `CONTEXT_CAPSULE_DICTIONARY_INVALID`,
`CONTEXT_CAPSULE_RESOURCE_LIMIT`, and `CONTEXT_CAPSULE_INTERNAL_INVARIANT`
failures of `CONTEXT_CAPSULE_PROFILE_V1.md`. They cover the binding between a
root-backed request and its response, the fact dictionaries, and the bounded
construction of the master capsule under `sley2.context-capsule.v1`. Codes
32000 through 32007 keep their restricted meanings unchanged. The contract is
a draft with Council review pending and reserves, rather than freezes, these
codes.

S20-330 freezes numeric codes 33000 through 33007 for the exact `SESSION_*`
failures of `SESSION_HANDLE_PROFILE_V1.md` (contract draft revision 4,
implemented under the draft with Council re-reviews pending). They travel
in the SMP1 failure envelope:

| Numeric | Symbolic |
|---:|---|
| 33000 | `SESSION_UNKNOWN` |
| 33001 | `SESSION_WORKSPACE_MISMATCH` |
| 33002 | `SESSION_ROOT_ADVANCED` |
| 33003 | `SESSION_EPOCH_MISMATCH` |
| 33004 | `SESSION_STALE_HANDLE` |
| 33005 | `SESSION_HANDLE_UNKNOWN` |
| 33006 | `SESSION_RENEWAL_LIMIT` |
| 33007 | `SESSION_BINDING_INVALID` |

`SESSION_UNKNOWN` answers for a name no live session holds; a remembered
close answers `PROTOCOL_SESSION_CLOSED`. `SESSION_BINDING_INVALID` is the
enumerated authority-cannot-bind bucket named by the owning contract.

S20-350 freezes numeric codes 35000 through 35010 for candidate-specific
proposal-construction failures:

| Numeric | Symbolic |
|---:|---|
| 35000 | `MUTATION_CANDIDATE_FORMAT_VERSION` |
| 35001 | `MUTATION_CANDIDATE_EXPIRY_INVALID` |
| 35002 | `MUTATION_CANDIDATE_EMPTY_OPERATIONS` |
| 35003 | `MUTATION_CANDIDATE_OPERATION_ORDINAL` |
| 35004 | `MUTATION_CANDIDATE_OPERATION_PRECONDITION_ORDINAL` |
| 35005 | `MUTATION_CANDIDATE_PRECONDITION_COUNT` |
| 35006 | `MUTATION_CANDIDATE_PRECONDITION_MISMATCH` |
| 35007 | `MUTATION_CANDIDATE_DESCRIPTOR_UNKNOWN` |
| 35008 | `MUTATION_CANDIDATE_PAYLOAD_KIND` |
| 35009 | `MUTATION_CANDIDATE_TARGET_ENTITY` |
| 35010 | `MUTATION_CANDIDATE_VALIDATION_PROFILE` |

These codes cover canonical proposal structure only. Strict encoding,
canonicality, envelope, digest, and resource failures preserve their exact
`SCB_*` code instead of being collapsed into this range. Neither namespace
claims semantic validity, authority, freshness against trusted host time, or
permission to mutate accepted state.

S20-360 freezes numeric codes 36000 through 36014 for every non-`VALID`
candidate decision, in the order listed by the master goal and
`CANDIDATE_RESULT_V1.md`:

| Numeric | Symbolic |
|---:|---|
| 36000 | `CANDIDATE_VALIDATION_INVALID_ENCODING` |
| 36001 | `CANDIDATE_VALIDATION_INVALID_SCHEMA` |
| 36002 | `CANDIDATE_VALIDATION_STALE_ROOT` |
| 36003 | `CANDIDATE_VALIDATION_STALE_ENTITY` |
| 36004 | `CANDIDATE_VALIDATION_INVALID_IDENTITY` |
| 36005 | `CANDIDATE_VALIDATION_INVALID_GRAPH` |
| 36006 | `CANDIDATE_VALIDATION_UNRESOLVED_REFERENCE` |
| 36007 | `CANDIDATE_VALIDATION_TYPE_ERROR` |
| 36008 | `CANDIDATE_VALIDATION_CONTROL_FLOW_ERROR` |
| 36009 | `CANDIDATE_VALIDATION_EFFECT_ERROR` |
| 36010 | `CANDIDATE_VALIDATION_CAPABILITY_DENIED` |
| 36011 | `CANDIDATE_VALIDATION_CONTRACT_ERROR` |
| 36012 | `CANDIDATE_VALIDATION_RESOURCE_LIMIT` |
| 36013 | `CANDIDATE_VALIDATION_TEST_PLAN_ERROR` |
| 36014 | `CANDIDATE_VALIDATION_INTERNAL_ERROR` |

These are stable outer result codes. Diagnostics also preserve the exact
owning `SCB_*`, `TYPE_*`, `CFG_*`, `EFFECT_*`, `POLICY_*`, `CAP_*`, or
`CONTRACT_*` source symbol and optional source numeric code. A wrapper code
never converts a source failure into success. Result byte canonicality and
digest failures preserve their exact `SCB_*` code.

S20-360 also freezes result-integrity codes 36100 through 36107:

| Numeric | Symbolic |
|---:|---|
| 36100 | `CANDIDATE_RESULT_FORMAT_VERSION` |
| 36101 | `CANDIDATE_RESULT_PROFILE_INVALID` |
| 36102 | `CANDIDATE_RESULT_PHASE_SHAPE` |
| 36103 | `CANDIDATE_RESULT_DECISION_PHASE_MISMATCH` |
| 36104 | `CANDIDATE_RESULT_DIAGNOSTIC_INVALID` |
| 36105 | `CANDIDATE_RESULT_SET_INVALID` |
| 36106 | `CANDIDATE_RESULT_CANDIDATE_ID_SHAPE` |
| 36107 | `CANDIDATE_RESULT_ROOT_SHAPE` |

These codes validate internal result shape only. Import never authenticates
phase evidence or grants candidate/commit authority. Strict value/envelope
failures remain in the independent `SCB_*` namespace.

S20-370 freezes numeric codes 37000 through 37018 for the exact `POLICY_*`
failures listed in `POLICY_ROOT_V1.md`. They cover protected policy-record
construction/import, closed principal grant data, ordinary-program isolation,
and policy-required test/contract finalization. Exact `SCB_*` and `SCHEMA_*`
failures are preserved. These codes do not authenticate policy transitions,
issue capability tokens, establish live scope/expiry/replay/budget authority,
construct candidates, commit state, or complete M3/M4/GA.

S20-390 freezes numeric codes 39000 through 39022 for transaction-core,
receipt, fixed accepted-head, recovery, and incomplete-clone guard failures
(`39022` was appended by S20-540 under ADR-0025):

| Numeric | Symbolic |
|---:|---|
| 39000 | `TXN_FORMAT_VERSION` |
| 39001 | `TXN_KIND_INVALID` |
| 39002 | `TXN_PARENT_SHAPE` |
| 39003 | `TXN_FIELD_SHAPE` |
| 39004 | `TXN_CHANGED_BINDING_INVALID` |
| 39005 | `TXN_TOMBSTONE_INVALID` |
| 39006 | `TXN_RESULT_NOT_VALID` |
| 39007 | `TXN_RESULT_BINDING_MISMATCH` |
| 39008 | `TXN_TEST_EVIDENCE_UNSUPPORTED` |
| 39009 | `TXN_OBJECT_INVENTORY_MISMATCH` |
| 39010 | `TXN_RECEIPT_BINDING_MISMATCH` |
| 39011 | `TXN_RECEIPT_CONFLICT` |
| 39012 | `TXN_GENESIS_INVALID` |
| 39013 | `TXN_ALREADY_INITIALIZED` |
| 39014 | `REF_HEAD_MISSING` |
| 39015 | `REF_HEAD_CORRUPT` |
| 39016 | `REF_CAS_STALE` |
| 39017 | `RECOVERY_RECEIPT_INCOMPLETE` |
| 39018 | `RECOVERY_REF_CAS_INCOMPLETE` |
| 39019 | `TXN_IO` |
| 39020 | `TXN_INTERNAL_INVARIANT` |
| 39021 | `TXN_RESOURCE_LIMIT` |
| 39022 | `TXN_INCOMPLETE_CLONE` |

A live-head mismatch exposed by ordinary commit is the existing `STALE_ROOT`
terminal decision, not last-write-wins. Exact S20-360 `STALE_ENTITY` and all
earlier source codes are preserved when fresh commit-time validation rejects a
candidate. `REF_CAS_STALE` is the lower fixed-head primitive failure and cannot
advance the head. S20-390 codes do not claim S20-500 named refs, S20-530 full
cross-process recovery, selected-test execution, policy transitions, M3/M4, or
GA.

S20-400 reserves numeric codes 40000 through 40011 for the exact `PROTOCOL_*`
failures of `SMP1.md`: frame length and envelope, version and downgrade,
session closure and request-identity conflict, unsupported methods, payload
decoding, negotiated limits, cancellation, and the internal invariant. Every
body keeps its owning contract's code; `SESSION_*` codes belong to S20-330.
The contract is a draft with Council review pending and reserves, rather than
freezes, these codes; the wording flips to frozen when the contract does:

| Numeric | Symbolic |
|---:|---|
| 40000 | `PROTOCOL_VERSION_UNSUPPORTED` |
| 40001 | `PROTOCOL_FRAME_INVALID` |
| 40002 | `PROTOCOL_FRAME_TOO_LARGE` |
| 40003 | `PROTOCOL_NO_COMMON_PROFILE` |
| 40004 | `PROTOCOL_DOWNGRADE` |
| 40005 | `PROTOCOL_REQUEST_ID_CONFLICT` |
| 40006 | `PROTOCOL_SESSION_CLOSED` |
| 40007 | `PROTOCOL_METHOD_UNSUPPORTED` |
| 40008 | `PROTOCOL_PAYLOAD_INVALID` |
| 40009 | `PROTOCOL_LIMIT_EXCEEDED` |
| 40010 | `PROTOCOL_CANCELLED` |
| 40011 | `PROTOCOL_INTERNAL_INVARIANT` |

The emitted repository symbol `STALE_ROOT` carries numeric 36002 through the
S20-360 decision mapping (registry name `CANDIDATE_VALIDATION_STALE_ROOT`);
SMP1 section 6 names the emitted symbol, and the numeric rides the mapping,
not a second row.

S20-560 reserves numeric codes 56000 through 56002 for the exact
`REPORT_STORE_*` failures of the execution report store composed by SMP1
appendix C (`SMP1.md` revision 7): `REPORT_STORE_UNKNOWN` (56000) for an
unknown identity, `REPORT_STORE_INVALID` (56001) for stored bytes that do not
re-derive their identity or exceed the bound, and `REPORT_STORE_IO` (56002)
for a host failure or a differing existing entry. The production object verifier reports the
S20-180 `GC_*` codes of the planner it serves. These codes are reserved with
the S20-400 draft and frozen with it.

S20-420 reserves numeric codes 42000 through 42004 for the exact `JSON_BRIDGE_*`
failures of `SMP1_JSON_BRIDGE_V1.md`: object shape, integer encoding, hex
encoding, unknown method name, and the text resource ceiling. They precede
the `PROTOCOL_*` codes only for resource, shape, and encoding failures; a frame
that is invalid on the wire keeps its `PROTOCOL_*` code. The contract is a
draft with Council review pending and reserves, rather than freezes, these
codes.

S20-430 reserves numeric codes 43000 through 43003 for the exact `CLI_*`
failures of `SLEY_CLI_V1.md`: usage, unreadable input, input or output
failure, and a missing handshake. They name only the endpoint's own
failures and map to exit statuses 2 through 5; a failed answer keeps its
owning code inside the response frame and is never a CLI failure. The
contract is a draft with Council review pending and reserves, rather than
freezes, these codes.

S20-620 reserves numeric codes 62000 through 62009 for the exact `SLEY2_TRIAL_*`
failures of `SLEY2_TRIAL_RUNNER_V1.md`: the run manifest, endpoint
availability, handshake, frame, trace, and claim validity, the
privileged-context guard, duplicate trials, timeouts, and the internal
invariant. They name only the runner's own failures; every frame keeps its
owning code inside the trace. The contract is a draft with Council review
pending and reserves, rather than freezes, these codes.

S20-630 reserves numeric codes 63000 through 63007 for the exact `ACCOUNTING_*`
failures of `SUCCESSION_ACCOUNTING_V1.md`: an invalid run directory, an
arm chain its own runner rejects, an unknown arm, an invalid metric, a
float anywhere, a complete report demanded over partial chains, an invalid
report, and the internal invariant. A zero denominator is a named null, not
a failure. The contract is a draft with Council review pending and reserves,
rather than freezes, these codes.

S20-720 reserves numeric codes 72000 through 72007 for the exact `PACKAGE_*`
failures of `RELEASE_CANDIDATE_PACKAGING_V1.md`: a failed clean build, an
invalid manifest, forbidden content (local paths or secrets), a failed
conformance subset or demo from the unpacked artifact, a non-reproducible
second build, a dirty working tree, and the internal invariant. They name
the candidate mechanics only; `release-check` stays fail-closed. The contract
is a draft with Council review pending and reserves, rather than freezes,
these codes.

S20-730 reserves numeric codes 73000 through 73007 for the exact failures of
`REPRODUCIBILITY_AND_INDEPENDENT_CONFORMANCE_V1.md`: a missing
(`REPRO_EVIDENCE_MISSING`) or unusable (`REPRO_EVIDENCE_INVALID`) S20-720
evidence record, an attestation that fails the frozen shape
(`REPRO_ATTESTATION_INVALID`), two hosts attesting one commit with different
artifact digests (`REPRO_ATTESTATION_CONFLICT`), an unreadable fixture
(`CONFORMANCE_FIXTURE_UNREADABLE`), a fixture digest list that disagrees with
its fixtures (`CONFORMANCE_SUMS_MISMATCH`), an undeclared fixture family, a
declared oracle command absent from the `make conformance` recipe or a Rust
dependency in the independent oracle (`CONFORMANCE_ORACLE_DRIFT`), and a
tracked conformance report that differs from the derived one
(`CONFORMANCE_REPORT_DRIFT`). They name evidence derivation only; no GA,
release, or publication claim follows from them. The contract is a draft with
Council review pending and reserves, rather than freezes, these codes.

S20-710 full reserves numeric codes 74000 through 74007 for the exact failures
of `STANDARDS_SBOM_AND_PROVENANCE_V1.md`: a missing
(`SBOM_INVENTORY_MISSING`) or malformed (`SBOM_INVENTORY_INVALID`) T52
inventory, a component without a purl, name, version, ecosystem, or usable
license expression (`SBOM_COMPONENT_INCOMPLETE`, which also covers a license
declaration that does not parse under the contract section 2 SPDX grammar), a tracked CycloneDX or SPDX document
that differs from the derived one (`SBOM_DOCUMENT_DRIFT`), a missing
(`PROVENANCE_EVIDENCE_MISSING`) or non-reproducible
(`PROVENANCE_EVIDENCE_INVALID`) candidate evidence record, a provenance subject
that disagrees with the candidate or the SBOM root
(`PROVENANCE_SUBJECT_MISMATCH`), and a tracked provenance that differs from the
derived statement (`PROVENANCE_DOCUMENT_DRIFT`). Nothing here signs, publishes,
or completes the S20-710 audit. The contract is a draft with Council review
pending and reserves, rather than freezes, these codes.

S20-740 reserves numeric codes 75000 through 75003 for the exact failures of
`FINDING_REGISTER_V1.md`: a missing (`REGISTER_SUMMARY_MISSING`) or unusable
(`REGISTER_SUMMARY_INVALID`) machine summary, a package whose status claims
completion while a review obligation is pending, deferred, or unclassified
(`REGISTER_COMPLETION_VIOLATION`), and a tracked register that differs from the
derived one (`REGISTER_DRIFT`). The register reports recorded dispositions; it
issues no finding and completes no review. The contract is a draft with Council
review pending and reserves, rather than freezes, these codes.

S20-780 reserves numeric codes 78000 and 78001 for the exact failures of
`CLEAN_ROOM_DISPOSITION_REGISTER_V1.md`: legacy source, a legacy dependency, or
an in-process legacy touchpoint in the tree (`CLEAN_ROOM_BOUNDARY_VIOLATION`),
and a register entry missing one of the seven ADR-0002 disposition fields
(`DISPOSITION_INCOMPLETE`). The register reuses no legacy source and reserves,
rather than freezes, these codes while its review is pending.

S20-770 reserves numeric codes 77000 and 77001 for the exact failures of
`REQUIRED_CONTRACT_INDEX_V1.md`: a master-goal section 17 contract whose
defining document, checker, or frozen digest domain is missing
(`CONTRACT_INDEX_UNSATISFIED`), and an index that names an artifact which does
not exist (`CONTRACT_INDEX_DRIFT`). The index defines no contract and reserves,
rather than freezes, these codes while its review is pending.

S20-750 reserves numeric codes 76000 through 76003 for the exact failures of
`DECISION_DOSSIER_V1.md`: a missing (`DOSSIER_SOURCE_MISSING`) or malformed
(`DOSSIER_SOURCE_INVALID`) evidence source, including a dossier that does not
cover the thirty-four required items in order; a derived decision that claims
`PASS` while a product gate is fail-closed (`DOSSIER_DECISION_INVALID`); and a
tracked dossier that differs from the derived one (`DOSSIER_DRIFT`). The
dossier reports; the release decision remains the operator's. The contract is a
draft with Council review pending and reserves, rather than freezes, these
codes.

S20-500 freezes numeric codes 50000 through 50020 for strict native branch and
named-ref metadata. ADR-0022 and `NATIVE_REFS_BRANCHES_V1.md` passed Nabu,
Ariadne, and Vulcan review before implementation:

| Numeric | Symbolic |
|---:|---|
| 50000 | `REF_FORMAT_VERSION` |
| 50001 | `REF_NAME_INVALID` |
| 50002 | `REF_NAME_RESERVED` |
| 50003 | `REF_DIGEST_MISMATCH` |
| 50004 | `REF_FIELD_SHAPE` |
| 50005 | `REF_BRANCH_BINDING_MISMATCH` |
| 50006 | `REF_NOT_FOUND` |
| 50007 | `REF_ALREADY_EXISTS` |
| 50008 | `REF_NAME_COLLISION` |
| 50009 | `REF_TARGET_MISMATCH` |
| 50010 | `REF_NAMED_CAS_STALE` |
| 50011 | `BRANCH_RECORD_FORMAT_VERSION` |
| 50012 | `BRANCH_RECORD_DIGEST_MISMATCH` |
| 50013 | `BRANCH_RECORD_FIELD_SHAPE` |
| 50014 | `BRANCH_ORIGIN_MISMATCH` |
| 50015 | `BRANCH_NOT_FAST_FORWARD` |
| 50016 | `BRANCH_ANCESTRY_CYCLE` |
| 50017 | `BRANCH_RESOURCE_LIMIT` |
| 50018 | `RECOVERY_NAMED_REF_INCOMPLETE` |
| 50019 | `REF_IO` |
| 50020 | `REF_INTERNAL_INVARIANT` |

Exact upstream `SCB_*`, `TXN_*`, `STATE_ROOT_*`, `POLICY_*`, and `STORE_*`
failures are preserved. This range does not authorize deletion, force movement,
symbolic refs, tags, named-branch candidate commit, merge, or full recovery.

Every validation phase has one declared default terminal state and a finite
set of more specific codes in that namespace. Retryability is an enum
(`NEVER`, `AFTER_REQUERY`, `AFTER_CAPABILITY`, `AFTER_LIMIT_CHANGE`,
`TRANSIENT_HOST`) rather than inferred prose. Truncation is explicit and never
removes the terminal code or phase. Unsupported methods, effects, epochs, or
features use versioned `*_UNSUPPORTED` failures, never generic success.

`INTERNAL_ERROR` is fail-closed, non-committable, and non-retryable unless the
typed details establish `TRANSIENT_HOST`. It carries an incident digest, not a
substitute program result. A ref comparison after any internal failure must
prove that accepted state did not advance.

# Native test profile reservations v1

Status: N0 **reserved, unimplemented**, revision 1 (2026-09-15). This is an
additive reservation ledger, not a list of currently admitted contracts. The
live `IDENTIFIERS_V1.md` registry intentionally continues to match code exactly.
Promote a reservation into that registry only in the same reviewed change as
its implementation and positive/negative independent vectors. No existing
identifier/domain/tag is renamed or renumbered.

## 1. Collision survey and profile boundary

Inspected the live identifier domains, digest-domain tag table, transaction
magics/metadata, VM/cache profiles, protocol `ALL`/`V2_ALL` methods/feature mask,
SCB/SSMC descriptors, error-owner documents and ADR index before reservation.
Existing descriptor tags end at22; these reservations start at23. Existing
SMP execution tags end604;605–607 are reserved below. Existing feature bits
are1,2,4,8,16; native uses32. Existing ADRs end0049.

Native custom envelopes do not allocate new SSMC object kinds, opcodes or
semantic epoch tags. Their exact P/Stored grammar is specified in the execution
owner contract. Descriptor tag reservations below are ledger ownership only;
custom envelope encoding contains its magic, not the numeric domain tag.
The live identifier registry explicitly permits reviewed, magic-disjoint
versions within the same identity family while preserving all old IDs and
decoder behavior; it forbids cross-purpose reuse and reinterpretation. A
future assembled SCB descriptor may consume the reserved domain tag after
review, without ever presenting a custom envelope as an old SSMC object.

## 2. Identifier domains, magics and type names

All domains are exact non-NUL-terminated ASCII. IDs are32-byte BLAKE3 over
domain||P(magic,canonical record). Exact ID means that function, not a host-
chosen opaque value; profile constructors must derive, not accept arbitrary IDs.

| Reserved descriptor tag | Type | Exact domain | Magic | Owner |
|---:|---|---|---|---|
| 23 | NativeExecutionProfileId | `sley2.native-test-execution-profile.v1` | `SLEYNXP1` | execution §2 |
| 24 | NativeTestPlanId | `sley2.native-test-plan.v1` | `SLEYTPL1` | admission §3 |
| 25 | NativeResourcePolicyId | `sley2.native-test-resource-policy.v1` | `SLEYNRP1` | admission §2 |
| 26 | NativeObservationId | `sley2.native-test-observation.v1` | `SLEYNOB1` | execution §5 |
| 27 | MeasuredTestAttestationId | `sley2.native-test-measurement.v1` | `SLEYMTA1` | execution §7 |
| 28 | NativeTestApprovalId | `sley2.native-test-approval.v1` | `SLEYNAP1` | admission §3 |
| 29 | NativeEvidenceBundleId | `sley2.native-test-evidence-bundle.v1` | `SLEYNBU1` | admission §5 |
| 30 | HistoricalAdmissionContextId | `sley2.native-test-historical-context.v1` | `SLEYNCT1` | admission §4 |
| 31 | CommitAdmissionStatementId | `sley2.native-test-admission-statement.v1` | `SLEYNSA1` | admission §5 |
| 32 | HistoricalTrustPolicyId | `sley2.native-test-trust-policy.v1` | `SLEYNTR1` | admission §4 |
| 33 | SupervisorConfigId | `sley2.native-test-supervisor-config.v1` | `SLEYNHC1` | execution §7 |
| 34 | NativeAdmissionProfileId | `sley2.native-test-admission-profile.v1` | `SLEYNAD1` | admission §4 |
| 35 | NativeExchangeProfileId | `sley2.native-test-exchange-profile.v1` | `SLEYNTP1` | admission appendix B |

Disjoint formats reusing existing identity families (no new domain/tag):

| Existing type/domain | New reserved magic | Preservation |
|---|---|---|
| ExecutionReportId / `sley2.execution-report.v1` | `SLEYNEX1` | old SLEYEXR1 unchanged |
| TestReportId / `sley2.test-report.v1` | `SLEYNTS1` | old SLEYTSR1 incomplete forever |
| TransactionId / `sley2.transaction.v1` | `SLEYTXN2` | old SLEYTXN1 test refusal unchanged |
| ReceiptId / `sley2.transaction-receipt.v1` | `SLEYRCP2` | old nine-field SLEYRCP1 unchanged |
| RepositoryExchangeId / `sley2.repository-exchange.v1` | `SLEYXCH2` | old contract540 schema/decoder unchanged |

These are disjoint preimage variants within their correctly typed identity
families, not domain aliases or reinterpretation of any previous bytes.
Acceptance always dispatches by exact magic/profile. New builders must not
silently feed new records to old variant validators.

Existing SHA256 worker/supervisor binary digests are raw binary measurements,
not new `sley2.*` hash domains. Raw Ed25519 public keys, attempt nonce32,
client attempt_id16 and report token32 are not content IDs. Possession conveys
no authority outside their explicit trust/session scopes.

Ed25519 signing contexts, **not BLAKE3 identifier domains**:

* `sley2.native-test-measurement-signature.v1` — measurement unsigned envelope.
* `sley2.native-test-admission-signature.v1` — acceptance unsigned envelope.

When implemented, registry tooling must classify signing contexts separately
from actual BLAKE3 derives rather than invent phantom identifier domains.

## 3. Profiles and protocol

* Native execution descriptor: exact record in execution §2. Cache/lowering
  profile remains existing2; native execution is distinguished by profile ID
  and observation magic, not a new SLEYBC version.
* Native admission descriptor: exact record in admission §4. Old full-v1
  candidate profile ID remains unchanged and is bound inside native descriptor.
* Native exchange descriptor: exact record in admission appendix B.
* Transaction record format2: metadata triple `[2,3,1]`, ordinary kind2 only.
  Old metadata `[1,1,1]`/`[1,2,1]` and genesis rules remain.
* SMP protocol table/profile version3, native feature32. Reserved native methods:
  601 tests.selected,602 tests.affected activated only for v3+feature32;
  605 tests.report_read,606 tests.replay,607 tests.attempt_status new in that scope.
  V1/v2 retain old unknown/reserved refusals, old feature masks and identities.

## 4. Reserved native failure assignments

Numbers below are reserved exclusively for this owner; none is currently
emitted by code. Positive/negative tests and symbol registration must accompany
implementation. Exact earlier owner failures remain their original codes.
Native public symbols map one-to-one; enum union tags in owner records are
local wire tags, not these global numeric error codes.

| Numeric | Symbol |
|---:|---|
| 29200 | `NATIVE_TEST_PROFILE_UNSUPPORTED` |
| 29201 | `NATIVE_TEST_ENCODING_INVALID` |
| 29202 | `NATIVE_TEST_SELECTION_INVALID` |
| 29203 | `NATIVE_TEST_SELECTED_COUNT_EXCEEDED` |
| 29204 | `NATIVE_TEST_DECLARED_LIMIT_EXCEEDS_POLICY` |
| 29205 | `NATIVE_TEST_AGGREGATE_OVERFLOW` |
| 29206 | `NATIVE_TEST_AGGREGATE_LIMIT_EXCEEDED` |
| 29207 | `NATIVE_TEST_EVIDENCE_LIMIT_EXCEEDED` |
| 29208 | `NATIVE_TEST_CONTEXT_MISMATCH` |
| 29209 | `NATIVE_TEST_OBSERVATION_MISMATCH` |
| 29210 | `NATIVE_TEST_OUTCOME_MISMATCH` |
| 29211 | `NATIVE_TEST_EXECUTION_REJECTED` |
| 29212 | `NATIVE_TEST_ENFORCER_UNAVAILABLE` |
| 29213 | `NATIVE_TEST_MEASUREMENT_REJECTED` |
| 29214 | `NATIVE_TEST_SIGNATURE_INVALID` |
| 29215 | `NATIVE_TEST_HISTORICAL_TRUST_UNAVAILABLE` |
| 29216 | `NATIVE_TEST_HISTORICAL_TRUST_REJECTED` |
| 29217 | `NATIVE_TEST_ADMISSION_BINDING_MISMATCH` |
| 29218 | `NATIVE_COMMIT_BUSY_RETRY_SAFE` |
| 29219 | `NATIVE_COMMIT_ABORTED_RETRY_SAFE` |
| 29220 | `NATIVE_COMMIT_OUTCOME_UNKNOWN` |
| 29221 | `NATIVE_TEST_ATTEMPT_BINDING_MISMATCH` |
| 29222 | `NATIVE_TEST_REPORT_TOKEN_INVALID` |
| 29223 | `NATIVE_TEST_REPLAY_INCONCLUSIVE` |
| 29224 | `NATIVE_TEST_RESOURCE_LIMIT` |
| 29225 | `NATIVE_TEST_INTERNAL_INVARIANT` |

Unknown attempt0 is an attempt-status result, not a retry-safe error. Native
resource limit is a failure status, never expected TrapCode. Use strict
execution-contract error details; do not invent arbitrary causal text.

## 5. Promotion checklist and historical compatibility

Implement an owner reservation only with: exact canonical fixtures and IDs;
independent decoder/derive agreement; old-profile bytes and refusal preservation;
malformed/bounds/substitution fixtures; reserved-to-live registry/drift-check
update; required owner review. No script can clear product/release gates merely
because this ledger exists. New routes remain unnegotiable until their entire
authority/evidence path is implemented.

# Native Test Admission v1

Status: N0 owner-contract proposal, revision 1 (2026-09-15). Independent
architecture review passed; this precise contract still awaits owner review,
vectors and implementation. Reserved wire formats are not currently admitted.
No product completion, test execution or release claim follows from this file.

## 1. Authority and encoding

Use `NATIVE_TEST_EXECUTION_V1.md` §2 canonical grammar, P/Stored envelopes,
strict bounds and type notation. Exact magics/domains/profile reservations are
in `NATIVE_TEST_RESERVATIONS_V1.md`. Every record below has required fields in
written order unless explicit numeric tags appear. No metadata map/extensions.

Policy selects under the protected parent policy; VM executes; tests verifies
evidence; supervisor attests measured resources; transaction alone authorizes
and durably accepts. A parsed result, signature, report ID or trusted principal
name alone is never commit authority. Keep old full-v1 validation pure and
unchanged; native approval is an additional disjoint gate.

## 2. Native selection and exact resource policy

Native plan derives from fresh validator-owned full-v1 output, exact parent
transaction/root, proposed inventory/root, protected policy, authenticated
principal/capability projection and effective local limits. It cannot be
constructed from an imported static `Valid` record as authority.

Selected set is the raw-ID-sorted union of:

1. Existing full-v1 selected TestCases.
2. Proposed TestCases targeting the owner-derived affected Function closure.
3. Every created/replaced live TestCase, even if target Function is unchanged.
4. Protected required TestCases, which must resolve in proposed state.

Derive affected closure through existing base/proposed reverse-dependency owner
code, not labels or protocol heuristics. Deleted optional tests are recorded,
not run; deleting a protected required test refuses. Bind exact TestCase and
target ObjectIds in proposed state. Native selected set may be stronger than
the old result; never rewrite the old result to claim it selected more tests.

Final stronger set MUST be checked anew against policy, including tests absent
from old phase12. Compare each test's literal fuel/memory/output/effects with
the authenticated principal grant's corresponding ceilings. Compare depth/wall
with effective CandidateValidationLimits and native caps. Over-policy declared
budgets refuse rather than silently clamp. Effective implementation safety
limits may separately be stricter and later cause execution refusal.

`ValidationLimits` is the exact nine-field context record:
`{max_operations:UInt32,max_preconditions:UInt32,max_candidate_bytes,
max_decoded_value_bytes,max_graph_work,max_selected_tests:UInt32,
max_entities:UInt32,max_test_call_depth,max_test_wall_timeout_millis}`.
`GrantCeilings` preserves the full six-field grant record:
`{max_fuel,max_memory_bytes,max_output_bytes,max_effect_count,
max_mutation_count,max_adapter_calls}`.
`NativeAggregateLimits` is
`{max_selected_tests:UInt32,max_fuel,max_memory_sum,max_output_sum,
max_effect_sum,max_depth_sum,max_wall_millis_sum,max_evidence_bytes}`.

Native aggregate hard maxima, in that order: 256; 1,000,000,000;
17,179,869,184; 16,777,216; 1,000,000; 65,536; 30,000; 50,331,648.
Actual selected count ≤min(effective static max_selected_tests, native count).
Every sum is checked u64, over exactly the final selected set. Serial execution
does not waive aggregate limits. Native per-test wall ≤30,000ms; execution depth
≤256; grant/local depth admission can be higher only when the tighter runtime
cap is explicitly recorded and may refuse. Local configuration may tighten all
native maxima but cannot enlarge them.

`NativeResourcePolicyV1` (`SLEYNRP1`) fields:
`{version:UInt32=1,policy_root:Id,principal:Id,capability_summary:Id,
grant:GrantCeilings,validation:ValidationLimits,
implementation:ImplementationLimits,aggregate:NativeAggregateLimits,
native_wall_cap:UInt64=30000,admission_profile:Id}`.
Exact effective policy bytes enter the plan and approval, not just prose.

Refusal order: existing static failure; selection resolution/order; final
count; each selected test in raw-ID order with fields fuel,memory,output,
effects,depth,wall; checked aggregates in that order; evidence-byte admission.
NativePlanError union tags: SelectionInvalid1, SelectedCountExceeded2,
DeclaredLimitExceedsPolicy3, AggregateOverflow4, AggregateLimitExceeded5,
EvidenceLimitExceeded6. Details follow execution contract §6. Existing result
decision/error tags are not extended.

## 3. Plan and test approval

`NativeTestPlanV1` (`SLEYTPL1`) exact fields:

| Tag | Field | Type |
|---:|---|---|
| 1 | version | UInt32=1 |
| 2 | selection_mode | UInt32 explicit-root1, candidate-affected2 |
| 3 | workspace | Id |
| 4 | semantic_epoch | Id |
| 5 | parent_transaction | Id |
| 6 | parent_root | Id |
| 7 | proposed_root | Id |
| 8 | policy_root | Id |
| 9 | candidate_id | Option<Id> |
| 10 | static_result_id | Option<Id> |
| 11 | protected_required_ids | Set<EntityId> |
| 12 | selected | List<SelectedEntry>, raw test-ID sorted, max256 |
| 13 | changed_tests | List<ChangedTest>, raw test-ID sorted |
| 14 | execution_profile | Id |
| 15 | implementation_limits | ImplementationLimits |
| 16 | static_selected_ids | Set<EntityId> |
| 17 | resource_policy | NativeResourcePolicyV1 direct record |

SelectedEntry is `{test_entity:Id,test_object:Id,target_function:Id,
target_object:Id,declared_limits:DeclaredLimits}`. ChangedTest is
`{test_entity:Id,before:Option<ObjectId>,after:Option<ObjectId>}`; at least one
exists and equal pairs refuse. It is the complete changed TestCase inventory.
Explicit-root mode requires parent/proposed root equality, absent candidate/
static result, empty static-selected/changed-test lists. Candidate mode requires
both IDs, exact old selected set, complete change list and fresh bindings.
Diagnostic explicit-root plans cannot authorize a candidate commit.

`NativeTestApprovalV1` (`SLEYNAP1`) fields:
`{version:UInt32=1,candidate:Id,static_result:Id,validation_context:Id,
parent_transaction:Id,parent_root:Id,proposed_root:Id,policy_root:Id,
plan_id:Id,test_report_id:Id,attestations:List<AttestationBinding>,
measurement_trust_policy:Id,resource_policy_id:Id,historical_context_id:Id,
decision:ApprovalDecision}`.
AttestationBinding is `{test_entity:Id,attestation_id:Id}`, strictly test-ID
sorted. Decision union: Accepted1(empty), Rejected2(NativeFailureRecord).
Accepted requires exact one-to-one report/measurement/plan selection, every
comparison Match, valid authorized measurement signature and all limits true.
Rejected artifacts cannot populate accepted receipts. Empty selection requires
an explicit empty test report and zero measurement bindings; it still requires
historical admission context and acceptance signature below.

Strict parsing yields `ParsedNativeTestApproval`. Only verification against a
fresh policy owner plan and configured measured authority constructs
`VerifiedNativeTestApproval`; callers cannot fill public fields to mint it.

## 4. Historical context and trust

`HistoricalAdmissionContextV1` (`SLEYNCT1`) fields:
`{version:UInt32=1,static_context_projection:Bytes,
capability_summary_projection:Bytes,resource_policy:NativeResourcePolicyV1,
authorization_profile:Id}`. Each Bytes field ≤16,777,216. Its ID binds all data.

Preserve exact eleven-field bytes already hashed by
`candidate_validation.rs::encode_context_projection`: format, base transaction,
base root, semantic epoch, protected policy, principal, capability-summary
digest, historical now_unix_millis, sorted base inventory digest, tombstone
digest and effective ValidationLimits. Obtain them from an owner accessor,
not a duplicate transaction encoder. Include the exact nonsecret capability
summary projection so its digest can be recomputed. Never export MAC keys,
private signing keys or capability secret authenticators.

`HistoricalTrustPolicyV1` (`SLEYNTR1`) fields:
`{version:UInt32=1,policy_nonce:FixedBytes32,entries:List<TrustEntry>}`.
Entry is `{key_id:FixedBytes32,role:UInt32,workspaces:Set<Id>,
profiles:Set<Id>,valid_from_unix_millis,valid_until_unix_millis}`.
Roles measurement1, acceptance2. Entries strictly key_id then role sorted;
max256; workspace/profile sets each nonempty≤256; validity interval is
half-open [from,until), from<until. For measurement role, profiles contains explicitly allowed SupervisorConfigIds;
for acceptance role, profiles contains allowed NativeAdmissionProfileIds. The
measurement execution profile is independently checked against the closed plan.
Same key may have separately provisioned roles but roles never imply each other. Key ID is exact Ed25519 public key;
manifest is an immutable receiver-provisioned trust anchor, not self-signed
authority installed from an exchange. Nonce distinguishes deliberate policy
versions; possession of its ID conveys no trust.

Store original trust manifests through key rotation; verify historical time
against their intervals, not today's capability expiry. Missing policy/key
refuses native promotion. An explicit receiver quarantine/revocation can render
history untrusted while preserving bytes; never silently rewrite head/history.
Manifest bytes may travel as diagnostic material but are not automatically
trusted. Accepted imports need receiver-established exact manifest IDs.

`NativeAdmissionProfileV1` (`SLEYNAD1`) exact descriptor:
`{version:UInt32=1,static_validation_profile:Id(full_v1),
execution_profile:Id(NativeExecutionProfileV1),selection_rule:UInt32=1,
measurement_profile:UInt32=1,acceptance_signature_profile:UInt32=1,
aggregate_hard_limits:NativeAggregateLimits(maxima),
lock_wait_millis:2000,prepromotion_watchdog_millis:35000,
cleanup_millis:2000,cancel_profile:UInt32=1}`.
Cancel1 is between-requests frame cancellation with independent disconnect
watcher. This descriptor is also the historical authorization_profile; no
separate unspecified authorization algorithm ID is accepted.

## 5. Evidence bundle and signed acceptance

`NativeEvidenceBundleV1` (`SLEYNBU1`) record:
`{version:UInt32=1,plan:Embedded,approval:Embedded,test_report:Embedded,
executions:List<TestEmbedded>,measurements:List<TestEmbedded>,
supervisor_configs:List<Embedded>}`.
TestEmbedded is `{test_entity:Id,stored:Embedded}`, strictly test-ID sorted,
max256. Lists exactly match selected plan; no duplicates/surplus. Supervisor
configs are unique/raw config-ID sorted and exactly those referenced by
measurements; no self-selected configuration grants trust. Every referenced
object is available as exact bytes. Plan and approval have their declared
magic, execution/test reports their exact native magic. Complete bundle record
encoded size≤50,331,648 bytes. Each embedded item remains within SCB1 Bytes
16MiB bound; execution item≤262,144 bytes.

The receipt carries the bundle as a **direct record**, not one oversized Bytes
field. Its BundleId is still derived from P(`SLEYNBU1`,record), and independent
bundle storage uses the normal trailer. No byte limit is raised by nesting.

`CommitAdmissionStatementV1` (`SLEYNSA1`) exact fields:
`{version:UInt32=1,admission_profile:Id,key_id:FixedBytes32,
workspace:Id,principal:Id,parent_transaction:Id,parent_root:Id,
candidate:Id,static_result:Id,historical_context_id:Id,
static_context_digest:Id,resource_policy_id:Id,native_approval_id:Id,
committed_root:Id,bundle_id:Id,transaction_id:Id,
historical_validation_time,acceptance_trust_policy_id:Id,
signature:FixedBytes64}`.
Sign exact ASCII `sley2.native-test-admission-signature.v1` followed by
P(`SLEYNSA1`,fields1..18), using strict Ed25519 pure mode as execution §7.
Final envelope includes field19 signature. Signature context is not a BLAKE3
domain. Statement is in receipt only; its transaction_id is computed first,
so there is no transaction/receipt/signature cycle.

Acceptance signer attests that the configured transaction authority checked
original issuer-secret capability authentication, expiry at preserved time,
protected policy, fresh static validation, selected test execution and native
approval before authorizing this transaction. Measurement signer is not
implicitly authorized to make this statement. Required even for empty tests.

Import/load/recovery independently verify canonical bytes/IDs, ancestry and
objects, context/capability projection digests, historical time versus static
result, resource ceilings, exact protected selection, every evidence binding
and authorized signatures. Pure graph/type/effect/contract checks can be
repeated. They do **not** independently reconstruct issuer-secret MAC checks,
actual past clocks or historical wall/memory measurements. Those are explicit
trusted signer judgments under preserved receiver-established roles/scopes.
Existing v1 verification remains its historical structural contract; do not
retroactively label old receipts native-authenticated.

## 6. Native commit, recovery and attempt lifecycle

Native commit takes exact candidate bytes, expected parent, client attempt_id
(FixedBytes16), admission profile and authenticated server context. Caller
reports/keys/executor objects are not authority. Initial implementation is
serial execution under the existing writer lock.

1. Bound maintenance/writer acquisition to2,000ms. No unbounded queue; timeout
   yields BusyRetrySafe without worker or accepted-state write.
2. Reload exact accepted parent and all context; stale parent refuses before
   execution. Run old full-v1 static validation and preserve exact failure.
3. Derive native plan and complete stronger-set policy/aggregate checks.
4. Record admitted attempt; execute each test in raw-ID order through qualified
   supervisor. Independently verify exact result, comparison and measurement.
   Failure preserves attempts and leaves accepted head unchanged.
5. Construct approval/context, transaction-v2 core, separate acceptance
   signature and complete receipt. Reverify through shared load/import logic.
6. Persist objects, complete receipt, sync and accepted-head CAS in the existing
   receipt-before-head order; return success only after head-directory sync.

Attempt journal is not semantic state. Bind attempt_id to workspace/principal,
candidate/expected parent. Duplicate with different bindings refuses. States:
Admitted1, Running2, AbortedBeforePromotion3, PromotionStarted4,
Committed5({transaction,receipt}), OutcomeUnknown6. Journal states are hints
until reconciled with actual receipt/head. Recovery never promotes an orphan
receipt merely to complete an attempt. Diagnostic storage failure before launch
refuses; record preservation failures after launch yield unknown, not success.

Synchronous handler cannot receive a mid-request cancel frame. Advertise
between_requests_only; a separate disconnect watcher sets a stop flag. Total
declared worker wall budgets≤30s, prep/evidence watchdog35s, global teardown
reserve2s. At safe points after validation, before each worker, after each
worker and before signature/staging, expiry/disconnect aborts before promotion.
After final checkpoint defer cancellation through atomic durability.

No universal bound is promised for kernel/fsync latency. Watchdog bounds abort
request and qualified cleanup, not arbitrary I/O. Unknown response or failed
reap yields OutcomeUnknown, never RetrySafe. Query attempt/accepted history
before retry; then rebase if parent changed. Caller/daemon crash must kill all
workers independently and preserve old/new-head atomicity. Load/recovery never
reruns programs or checks historical token expiry against current time.

## Appendix A. Transaction/receipt v2

Transaction preimage P(`SLEYTXN2`,record), ID under existing transaction domain;
receipt P(`SLEYRCP2`,record), ID under existing receipt domain. Envelope version
is1; record format field is2. Do not change v1 builders/decoders/guards.

Transaction record retains v1 tags1..19, with tag1=2, kind2 only (ordinary),
exactly one parent, old optional authority fields required Some, and tag20
native_approval_id:Id. Metadata is exactly `[commit_profile=2,
semantic_profile=3,durability_profile=1]`, identifying native additional
admission atop fresh full-v1 judgment. Native selected tests equal the native
plan; old static selected IDs equal its field16 and are a subset. test_result_refs
is exactly the singleton native TestReportId, including empty report. Genesis
remains v1; no v2 genesis/profile fallback.

Receipt retains v1 fields1..9 with format2 and ordinary nested bytes, and adds
10 bundle:NativeEvidenceBundleV1 direct record, 11 historical_context:Embedded,
12 commit_admission:Embedded. Complete receipt≤67,108,864 bytes; nested Bytes
retain16MiB limits; all total bounds checked before accepted persistence.
Candidate/result/root/policy and exact changed-binding/object manifest invariants
remain. Cross-check transaction20, approval, bundleID, statement transactionID,
historical context and selected/test-result sets. Any mismatch refuses.

Introduce additive Rust native types and explicit imported version enum.
Old import APIs retain v1-only acceptance; new dispatch selects exact magic.
Mixed ancestry uses shared relationship accessors. Older binaries must refuse
v2 rather than partially interpret it as v1. GC retains reachable complete
receipts; optional report cache loss cannot invalidate history.

## Appendix B. Native repository exchange profile

Use disjoint custom envelope P(`SLEYXCH2`,record) under existing repository
exchange domain. It is a transport profile, not an epoch-1 SSMC envelope or a
new program semantic epoch. Old `SLEYSCB1` contract540 exchange registry and
decoder stay unchanged. Transport descriptor `NativeExchangeProfileV1`
(`SLEYNTP1`) is `{version:UInt32=1,exchange_record_version:UInt32=2,
chunk_bytes:65536,max_exchange_bytes:67108864,max_receipts:4096,
max_branches:4096,max_signatures:1052672,max_test_visits:1048576}`.
All fields except first two UInt64. Profile ID binds exact transport rules.

Exchange record tags1..8 retain v1 logical fields but tag1=2 and receipt
elements use `{transaction_id:Id,receipt_id:Id,receipt_chunks:List<Bytes>}`.
Chunking is canonical: every nonfinal chunk exactly65,536 bytes; final1..65,536;
empty receipts disallowed; at most1024 chunks; concatenation is exact stored
receipt. No alternative partitions or extra empty chunks. This carries up to
64MiB receipts without raising any SCB1 Bytes ceiling. All other unchanged
nested pack, branch, head, absent-signature and compression records preserve
their v1 encoding/closure meaning. Append field9 required_trust_policy_ids:Set<Id>
and field10 transport_profile_id:Id. Trust IDs are exact union referenced by
all native measurement/acceptance statements. They never install receiver trust.

Tree sections1..4 and ordering use old exchange leaf/node construction over
the exact reconstructed receipt bytes. Add final section5 leaf with identifier
transport_profile_id and stored bytes C(required_trust_policy_ids). Leaf count
is3+receipt_count+branch_count, max8195. Outer v2 envelope binds all record bytes.
Per-receipt native signatures/context/evidence verified before any writes.
V1 ancestry is accepted under unchanged v1 verifier; v2 under native verifier.

Keep v1 total/allocation/pack/object/binding work maxima; additionally bound
signature verifications≤1,052,672 (4096*(256+1)), native test visits≤1,048,576,
and decoded evidence bytes≤1,073,741,824 across preflight. Each nested parse
shares the top-down allocation budget. Fail before exceeding any counter.
No execution during structural import; require structurally verified plus
trusted native attestations before clone promotion. Replay is explicit.

Import stages objects/complete mixed receipts and writes head last using
existing maintenance/incomplete-clone discipline under a separate native
exchange marker identity. Corrupt/missing/surplus reports, signatures or trust
must fail preflight before promotion. Retain exact bytes on retry. New transport
format acceptance does not require migrating semantic program roots.

## Appendix C. SMP v3 native surfaces

Preserve old protocol method tables/feature masks/frame/hello behavior. Add
v3 table and bit32 native_tests_v1; activation requires negotiated v3 and bit.
V1/v2 still refuse reserved601/602 byte-for-byte;605–607 unknown there. Update
hello selection, method admission, `is_reserved_for(profile)` and frame checks,
not just dispatch. A v3 hello may list native methods only when bit32 is set;
selected intersection lacking bit removes native methods before profile hash.
Old report604 and entity-handle304 payloads/meanings remain exact.

Native requests are SCB1 direct records, fields in listed order:

*601 tests.selected:* `{root:Id,selected_ids:Set<Id>,execution_profile:Id,
attempt_id:FixedBytes16}`. Root equals session root; owner adds required tests.
*602 tests.affected:* `{candidate_bytes:Bytes,execution_profile:Id,
attempt_id:FixedBytes16}`. Base binds session parent. Responses both:
`{plan_id:Id,report_id:Id,diagnostic_status:UInt32,selected_count:UInt32,
report_token:FixedBytes32,total_bytes:UInt64}`. Status comparison-complete1,
mismatch2, execution-rejected3, measured-resource-refusal4. None grants commit.

*605 tests.report_read:* `{token:FixedBytes32,offset:UInt64,max_bytes:UInt32}`.
Response `{report_id:Id,bound_root:Id,offset,total_bytes,bytes:Bytes,
next_offset:Option<UInt64>}`. Page max65,536. Token random32 server capability
binds session/generation, accepted parent, optional candidate/proposed root,
exact immutable evidence digest/size, creation and expiry. Bytes served are the
complete Stored native test report identified by report_id; execution/measurement
bundle bytes are obtained through receipt read/export, not an overloaded report
page. Diagnostic detailed evidence remains in the local attempt journal. Max32 tokens and64MiB
cached evidence per session. TTL5min or session lifetime, shorter wins.
Renew/close/checkout/commit invalidate tokens; no implicit root rebinding.
Offsets≤total; max_bytes>0; page length=min(max_bytes,total-offset); final page
next=None, otherwise Some(offset+length). Each call charges decoded fields,
returned bytes and owner work to negotiated session limits. Wrong session,
expired token or missing bytes refuses, never a semantic entity-handle lookup.

*606 tests.replay:* `{transaction_id:Id,expected_root:Id,execution_profile:Id,
replay_resource_policy_id:Id,attempt_id:FixedBytes16}`. Require transaction in
authorized session history/ref scope and exact receipt root; server policy
controls replay ceilings. Response `{transaction_id:Id,root:Id,status:UInt32,
original_report_id:Id,replay_report_id:Option<Id>,report_token:Option<FixedBytes32>}`.
Status Matched1, Mismatch2, InconclusiveResource3, UntrustedHistory4. Pin historical
root under read/maintenance ownership. Recompute deterministic native plan and
observation under recorded execution profile; compare exact bytes/counters.
New host timeout is inconclusive, not historical failure/success. No accepted
transaction or replacement historical attestation is created.

*607 tests.attempt_status:* `{attempt_id:FixedBytes16,candidate_id:Option<Id>}`.
Response `{state:UInt32,transaction_id:Option<Id>,receipt_id:Option<Id>,
report_token:Option<FixedBytes32>}`. State uses §6 journal tags; UnknownAttempt0.
Same authenticated workspace/principal required; usable after session renewal.
Committed identities come from verified history, not a journal assertion.
Unknown never implies retry-safe. Fresh token only for verified accepted evidence.

V3 native commit payload is `{candidate_bytes:Bytes,expected_parent:Id,
attempt_id:FixedBytes16,admission_profile:Id}` under the existing commit method
and v3-specific negotiated native admission route. Old commit payload/route
remain unchanged outside v3+native feature. Successful response is the v3-only record `{transaction_id:Id,receipt_id:Id,
state_root:Id,candidate_result_id:Id,native_approval_id:Id,attempt_id:FixedBytes16}`.
Native refusal uses the existing protocol failure envelope with reserved native
numeric/symbol/details and explicit retryability; only BusyRetrySafe and
AbortedRetrySafe assert pre-promotion retry safety. Transaction
owner derives all evidence. The protocol cannot accept caller signing keys,
grant bypasses or arbitrary supervisor configs.

JSON bridge maps these exact typed records; CLI is thin SMP routing. Typed
record allocation, exact response fields and profile identity require N7
independent vectors; no current bridge method is implicitly activated by N0.

## 7. Required implementation evidence

N1 codecs: canonical vectors plus strict invalid/missing/surplus/reordered,
signature malleability, profile/root/object substitution and bounds cases.
N4: TestCase-only added tests at equality/one-over each six policy ceilings,
final count and every aggregate; static full-v1 bytes unchanged.
N5: fresh locked execution, exact context, empty/nonempty acceptance signatures,
staleness and full crash cuts; old selected-test guard still refuses v1.
N6: mixed-history chunked exchange, all preflight counters, missing/untrusted
roles/context, replay and source-free recovery without secret export.
N7: profile/frame negotiation, old handles/refusals, paging lifetime/budgets,
disconnect/unknown-response reconciliation and actual packaged lifecycle.
Only after real N3 supervision and all owner verification may selected-test
native commits accept. Full successor epoch/effect/corpus/GA tasks remain open.

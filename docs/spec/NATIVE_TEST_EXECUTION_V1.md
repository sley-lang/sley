# Native Test Execution v1

Status: N0 owner-contract proposal, revision 3 (2026-09-15). The architecture
passed independent design review with zero open issues. This wire-contract
proposal still requires review, vectors and implementation before accepting
paths are enabled. No native TestCase execution or PASS is claimed here.

## 1. Ownership and preservation

Support exactly the epoch-1 TestCase forms admitted by
`CONTRACT_TEST_PROFILE_V1.md`: monomorphic pure target, `Replay([])`, empty
observations, exact persistable expected value or frozen TrapCode 1–4.
Unsupported contracts, effectful TestCases and `test_observe` remain refused.
Preserve full-v1 candidate/result bytes, `SLEYEXR1`, `SLEYTSR1`, old VM
observations/requests, lowering/cache keys and `SLEYBC01`/`SLEYBC02`. Old report
`Match` retains incomplete finality. No v1 artifact gains authority.

`sley-vm::native_execution` owns native request, limits, termination and
observation types. It depends on existing pure check/id/mutate/SSMC/codec
owners, never tests/conformance/host services. `sley-tests` owns plans,
execution/test report wrappers, comparison and approval verification and
depends one-way on VM/conformance. `sley-test-runner` owns host supervision and
signing, depending on tests/VM, not policy/transaction/repository/protocol.
Policy depends on tests' primitive plan types; transaction depends on
policy/tests/runner; CLI invokes the worker entry.

CI MUST check this acyclic graph and exclude host I/O from pure owner APIs.
Pure signature verification is permitted; spawning, clocks, filesystem,
randomness and provisioning are runner/repository responsibilities. N2 VM
types/implementation precedes N1 execution-dependent test records.

The first native commit retains full-v1 static validation. E7-bearing
candidates currently refused at phase 12 remain refused even if a VM entry
can execute such an opcode. Broader static admission needs a new profile.

## 2. Canonical grammar and limits

SCB1 primitive/value encoding is normative. Record notation `{a:T,b:U}` assigns
required tags 1,2 in written order. Every listed field is required once; extra,
unknown, duplicate and trailing fields refuse. Integers default to UInt64;
identifiers/digests are FixedBytes<32>. `Set<Id>` means a strictly raw-ID-sorted
list. `Embedded` is SCB1 Bytes containing a complete verified stored envelope.
Options use SCB1 union 0/empty or 1/value. Direct nested records are not Bytes.

All new custom envelopes use this exact grammar:

```
P(magic, record) = magic[8] || uvar(1) || uvar(len(C(record))) || C(record)
Id = BLAKE3-256(exact_domain || P)
Stored = P || Id[32]
```

`C` is canonical SCB1. Magics/domains are reserved in
`NATIVE_TEST_RESERVATIONS_V1.md`. These derived profiles are not SSMC entity
envelopes and cannot enter the old decoder as epoch-1 semantic objects.
Stored standalone size ≤67,108,864 bytes including trailer; nested record depth
≤64; list count ≤65,535 unless stricter below; aggregate decoded allocations
≤134,217,728 bytes; decoding visits ≤4,194,304. Charge repeated nested bytes
independently; never normalize. Check length/arithmetic before allocation.

`DeclaredLimits` is the six-field record, all UInt64, in this exact order:
`{fuel,memory_bytes,output_bytes,effect_count,call_depth,wall_timeout_millis}`.
Copy exact canonical TestCase limits without clamping.

`ImplementationLimits` is the five-field UInt64 record:
`{max_instructions,max_value_units,max_output_units,max_call_depth,max_report_bytes}`.
Native hard maxima respectively are 10,000,000; 67,108,864; 67,108,864; 256;
262,144. Local effective values may tighten, never increase them. Zero is literal.

`NativeExecutionProfileV1` (`SLEYNXP1`) is the exact record:
`{version:UInt32=1,lowering_profile:UInt32=2,vm_major:UInt32=1,
vm_minor:UInt32=0,vm_patch:UInt32=0,entry_depth:UInt32=1,
output_encoding:UInt32=1,pure_only:Bool=true,
hard_limits:ImplementationLimits(maxima),cancellation_profile:UInt32=0}`.
Its ID names immutable execution rules, not evidence. Output encoding 1 is §4;
cancellation 0 prohibits caller-programmed cancellation fuel points. Existing
lowering profile 2 and its bytecode remain unchanged.

## 3. Native VM entry and failure ordering

The N2 Rust API lives entirely in `sley-vm::native_execution`:

```rust
pub struct NativeExecutionInput<'a> {
    pub lowering: LoweringInput<'a>,
    pub effects: &'a [sley_ssmc::EffectDefinition],
    pub requirements: &'a [sley_ssmc::CapabilityRequirement],
}
pub struct NativeExecutionRequestV1 {
    pub inputs: Vec<sley_ssmc::ConstValue>,
    pub declared_limits: NativeDeclaredLimits,
    pub implementation_limits: NativeImplementationLimits,
    pub profile_id: NativeExecutionProfileId,
}
pub fn execute_native_function(
    input: NativeExecutionInput<'_>,
    request: NativeExecutionRequestV1,
) -> Result<NativeExecutionOutcome, NativeExecutionError>;
```

`NativeDeclaredLimits` and `NativeImplementationLimits` have exactly the named
u64 fields of DeclaredLimits/ImplementationLimits in §2. NativeExecutionProfileId
is the sley-id fixed32 identity reserved by N0. NativeExecutionOutcome owns
`termination: NativeExecutionTermination` and
`observation: NativeExecutionObservationV1`. The in-memory termination variants
are `Success(ConstValue)`, `ResourceLimit(NativeResourceKind)`,
`Trap { trap_tag:u32,payload:Option<ConstValue> }`, and `InternalInvariant`.
NativeResourceKind is the closed seven-tag enum in §4. Observation contains the
exact typed fields1..18 of §5 and their canonical record/ID; its Success/Trap
projection uses VM-validated value hashes, not duplicate unvalidated values.
Only VM execution builds an observed outcome. Accessors expose observation
facts/bytes; parsed reports do not construct this runtime outcome.

`NativeExecutionError` is `Preserved(ExecutionError) |
Effect(sley_check::effects::EffectValidationError) |
Profile(NativeExecutionProfileError)`. Existing nested numeric/symbol errors
remain exact. NativeExecutionProfileError has Unsupported (29200) and
InvalidContext (29208); unsupported profile/non-pure closure uses Unsupported,
inconsistent supplied function inventories use InvalidContext. These errors
are pre-execution failures; observed resource exhaustion is an outcome.

LoweringInput supplies complete graph/parameter/block/operation/constant/global/
function/contract/adapter inventories. VM constructs `FunctionUnit` views itself
by the existing function/block ownership relationships; no independently
caller-supplied units can diverge from lowering inventories. Require the target
FunctionGraph to equal the uniquely identified graph in functions. Epoch/root
come only from LoweringInput; no duplicate request context is reconciled by
convention. Limits/inputs remain exact caller data checked below.

**TestCase admission is the test owner's obligation, not the N2 VM's.**
`sley-tests` resolves the bound canonical TestCase ObjectId and invokes the
existing contract/test checker on complete source inventories. That checker
validates target/input/expected value, supported contracts, exact Replay([])
and empty expected observations. It then constructs the VM request from the
verified target, inputs and declared limits. N2 has no TestCase input and never
claims to validate absent replay/observation fields. No reverse dependency on
tests or conformance is introduced.

VM's independently checkable pure obligations and refusal order are:

1. Admit native profile, native hard input/count/limit representation and
   `lowering.profile == CacheProfile::EXTENDED_V1`. These new outer profile
   checks precede existing execution, without altering old APIs.
2. Preserve the existing entry path's exact integrated lowering/CFG and input
   judgment order: lower first, input count/cap next, then each complete
   constant/hashability/type/value-unit/hash check in declaration order.
3. Verify target/inventory equality; derive FunctionUnits and call
   `sley_check::effects::validate_effect_program` using types, effects,
   requirements, adapters and sorted contract identities. Preserve its exact
   errors. Require the target's computed least effect closure empty. Require
   every supplied contract predicate to resolve and have empty computed closure;
   broader contract attachment/binding/kind checking remains the test owner's
   preceding canonical admission. Unknown predicate/context is InvalidContext.
4. Use the unchanged extended lowering/cache identity produced in step2.
5. Before executing or admitting a frame, reserve observation capacity. The VM
   computes `observation_capacity_required(input_count, declared_limits,
   implementation_limits) -> Result<u64, ScbError>` using the exact stored
   NativeObservation envelope grammar, exact supplied limits, fixed context/ID
   widths, the actual ordered input-hash count, the longest permitted termination
   (Trap with present ValueHash), and u64::MAX encodings for all five dynamic
   counters (instruction, fuel, peak value units, output bytes, peak depth).
   Version/profile/effect_count remain their fixed encodings. This is a
   conservative reservation, not an observed byte count. If it exceeds literal
   max_report_bytes, including zero, return
   `Preserved(ExecutionError::Status(ExecutionStatusCode::ResourceLimit))`
   before execution, preserving the exact owner numeric/symbol and phase5.
   Equality passes. This reserves only stored NativeObservation bytes; it does
   not promise that an outer execution report fits. Before invoking N2, N1 must
   separately reserve the complete NativeExecutionReport using this observation
   capacity as its Embedded payload, including the Bytes/union/record lengths,
   all wrapper fields and the report envelope/trailer. Above the report's
   262,144-byte hard cap, N1 refuses NATIVE_TEST_EVIDENCE_LIMIT_EXCEEDED (29207)
   before execution. No wrapper truncation or post-run capacity discovery is
   permitted. The final observation builder independently enforces its
   exact stored size (including trailer), but a size failure after successful
   preflight is an internal implementation defect, never an expected trap or
   normal post-execution resource rejection.
   Admit initial frame: depth zero produces native CallDepth resource outcome;
   then preserve initial value-unit checks.
6. Execute shared opcode implementation under native limits/observation.
   No host callback or effect dispatcher is available. Test-owner admission
   plus VM checks are both required before native acceptance.

Entry depth is 1. Prospective direct and contract-predicate call depth is
checked before frame allocation/call-entry fuel charge, retaining extended
ordering. Effective depth is min(declared, implementation limit). Every call
route updates shared current/peak counts; tail calls have no exemption.
Preserve existing instruction/fuel/value-unit charging and failure order.
Host interruption affects measured evidence, not deterministic VM bytes.

## 4. Resource semantics

| Field | Exact enforcement |
|---|---|
| fuel | Existing charged actions; stop before charge exceeds literal cap |
| memory_bytes | Fresh worker cgroup including worker bootstrap/IPC decode; §7 |
| output_bytes | Canonical SSMC ConstValue bytes of returned value/present trap payload; absent trap payload costs zero |
| effect_count | Actual zero, proved by pure closure and no effect dispatcher |
| call_depth | Entry counts one; check every prospective call before allocation |
| wall_timeout_millis | Supervisor monotonic interval from launch to complete output and confirmed exit |

Output bytes are exactly **`sley_mutate::encode_const_value`** output
(`crates/sley-mutate/src/codec.rs`, public S20-350 mutation-value codec),
including its canonical type/value structure. This is not the private
fingerprint encoding or a report/object envelope. The current public function
allocates a Vec; N2 must first add a bounded counting path in that same pure
codec owner, sharing its canonical traversal/field structure with encoding.
Tests/conformance must not implement a second production encoder.

Freeze the counting API as
`measure_const_value_bounded(value: &ConstValue, cap: u64) ->
Result<ConstValueByteMeasure, ScbError>`, owned/re-exported by sley-mutate.
`ConstValueByteMeasure` is `Exact(u64) | OverLimit`; it does not expose a
traversal-dependent counter. For **every** cap, first run the complete canonical
codec validation/counting traversal within the codec's hard work bounds. A
canonical or hard-limit failure returns the same ScbError as that shared owner
traversal before any declared-cap classification. A small cap never skips later
map ordering, duplicate, type/value structure, depth, collection, byte or checked
arithmetic validation. Do not return OverLimit merely because a partial count
crossed cap. Only a completely valid encoding produces Exact(length) when
length <= cap or OverLimit when length > cap. Thus a value exceeding both the
declared byte cap and a codec hard bound produces the codec refusal, for all
caps including zero and u64::MAX.

The traversal checks existing depth/collection bounds before descent/iteration,
uses checked lengths, and terminates with ResourceLimit at the existing hard
node/byte/work ceiling. It need not continue beyond a hard refusal to discover
later invalidity; preserve the shared codec owner's canonical traversal and
first-error order. This is bounded full validation, not unlimited traversal:
no size-based fast path may bypass those checks or materialize an oversized
buffer. Within-cap value encoding occurs only after Exact. Counting and encoding
share the owner traversal, including ordering and hard-refusal decisions.

`output_bytes_counted` has exactly one deterministic rule:

* Before output sizing is entered (including fuel/instruction/value-unit/depth
  refusal and InternalInvariant), the counter is0.
* Absent trap payload has counter0 and needs no value encoding.
* Successful complete sizing within cap reports its exact encoded length.
* A completely valid encoded size above declared output cap reports **cap+1 exactly**,
  regardless of traversal chunk sizes or the moment overflow became known,
  and terminates with native ResourceLimit(OutputBytes).
* At cap=u64::MAX there is no representable cap+1; codec/hard checked limits
  apply first. This cap cannot produce an OutputBytes refusal: an impossible
  size/codec resource overrun produces native ResourceLimit(OutputEncoding) with
  counter0. SCB malformed/impossible postvalidated value maps to InternalInvariant
  with counter0. Exact encodable values retain their exact length.

Validate/hashability-check output as existing VM does, check existing output
value-unit ceiling, then bounded sizing, then byte ceiling, then materialize
and derive value hash. Earlier refusal wins and leaves counter0. Counting uses
existing codec hard node/depth/byte bounds plus the already charged semantic
output units; it adds **no** instruction/fuel/value-unit charge or observation
counter. The implementation must stop before exceeding codec hard work bounds;
codec ResourceLimit maps to native OutputEncoding/counter0, other impossible codec
failure to InternalInvariant/counter0. Thus no traversal-dependent extra fuel
or work measurement enters deterministic observations. Golden vectors pin
zero/exact/one-over and u64::MAX behavior to one expected observation ID.

Fuel/byte/depth equality passes; one over refuses. Zero fuel follows existing
charged-action semantics. Zero output permits absent trap payload but not
encoded Unit. Zero memory/wall refuses before worker launch. Never convert
bytes to value units or milliseconds to fuel. Implementation safety ceilings
are separately bound and can refuse without satisfying an expected trap.

```
NativeTermination =
  1 Success(ValueHash) |
  2 ResourceLimit(UInt32 resource_tag) |
  3 Trap({trap_tag:UInt32,payload:Option<ValueHash>}) |
  4 InternalInvariant(empty)
```

Native resource tags: instruction=1, fuel=2, value units=3, output units=4,
call depth=5, output bytes=6, canonical output-encoding safety ceiling=7. Trap tag is exactly 1..4. There is no deterministic
cancellation arm in native v1. Memory/time/enforcer failure never becomes a VM
trap or a matching expected failure.

## 5. Observation and report bytes

`NativeObservationV1` (`SLEYNOB1`) exact record:

| Tag | Field | Type |
|---:|---|---|
| 1 | version | UInt32=1 |
| 2 | schema_epoch | Id |
| 3 | field_schema_hash | Id, exact epoch hash |
| 4 | decoder_limits_hash | Id, exact epoch hash |
| 5 | state_root | Id |
| 6 | function | EntityId |
| 7 | cache_key | BytecodeCacheKey |
| 8 | native_execution_profile | Id |
| 9 | input_hashes | ordered List<ValueHash>, max 65,535 |
| 10 | declared_limits | DeclaredLimits |
| 11 | implementation_limits | ImplementationLimits |
| 12 | termination | NativeTermination |
| 13 | instruction_count | UInt64 |
| 14 | fuel_used | UInt64 |
| 15 | peak_value_units | UInt64 |
| 16 | output_bytes_counted | UInt64 |
| 17 | peak_call_depth | UInt64 |
| 18 | effect_count | UInt64=0 |

VM derives observation internally. No wall time, host peak memory, paths,
process identity, nonce or signatures occur here. Test owner rederives
context/cache/input hashes and exact observation bytes. Parsed caller counters
are not proof that the VM ran.

`NativeExecutionReportV1` (`SLEYNEX1`):
`{version:UInt32=1,plan_id:Id,test_entity:Id,test_object:Id,
target_object:Id,result:NativeExecutionEvidence}`. Evidence union:

* 1 Observed(Embedded NativeObservationV1).
* 2 Rejected({phase:UInt32,numeric_code:UInt32,symbol:Text}).

Rejected phase tags: type=1, CFG=2, lowering=3, fingerprint=4, execution=5,
native profile=6, effect=7. Tags1..6 retain their meanings. The complete conversion
from NativeExecutionError uses the leaf owner below; wrapper origin does not
change the phase or code:

| NativeExecutionError path | Phase | Numeric code and symbol owner |
|---|---|---|
| Preserved(Type(e)) | 1 | e's TypeError code |
| Preserved(Lowering(Cfg(Type(e)))) | 1 | e's TypeError code |
| Preserved(Lowering(Cfg(Cfg(e)))) | 2 | e's CfgError code |
| Preserved(Lowering(Lower(e))) | 3 | e's LowerError code |
| Preserved(Fingerprint(e)) | 4 | e's FingerprintError code |
| Preserved(Status(e)) | 5 | exact ExecutionStatusCode e |
| Preserved(Exec(e)) | 5 | exact ExecutionErrorCode e |
| Effect(Type(e)) | 1 | e's TypeError code |
| Effect(Cfg(Type(e))) | 1 | e's TypeError code |
| Effect(Cfg(Cfg(e))) | 2 | e's CfgError code |
| Effect(Effect(e)) | 7 | e's S20-230 EffectError code |
| Profile(Unsupported) | 6 | 29200 / NATIVE_TEST_PROFILE_UNSUPPORTED |
| Profile(InvalidContext) | 6 | 29208 / NATIVE_TEST_CONTEXT_MISMATCH |

Use each existing owner's numeric and symbol accessors, never Display text,
remapping or a generic wrapper code. Native profile symbols are the exact
reservation-ledger symbols. Symbol is ASCII 1..96 bytes.
Rejected reports are diagnostic, never proof of successful execution. Report
stored bytes ≤262,144. Existing ExecutionReportId domain is retained over the
disjoint magic; old preimages/IDs remain unchanged.

`NativeTestReportV1` (`SLEYNTS1`):
`{version:UInt32=1,plan_id:Id,schema_epoch:Id,proposed_root:Id,
entries:List<NativeTestEntry>,match_count,mismatch_count,rejected_count}`.
Entry is `{test_entity:Id,test_object:Id,execution_report_id:Id,
expected:Expected,comparison:UInt32}`. Expected union: Value(ValueHash)=1,
FailureCode(UInt32 1..4)=2. Comparison: Match=1, Mismatch=2,
ExecutionRejected=3. Entries strictly raw test-ID sorted, exactly one per
selected plan entry, max256. Totals are checked exact sums. Empty plan has an
explicit empty report with zero counts. Existing TestReportId domain covers
the disjoint new magic; it does not promote old reports.

Value comparison checks exact validated ValueHash; failure comparison matches
only explicit observed trap tag. Resource failure, wrong result, cancelled
host, internal failure or rejected execution never matches. Validate exact
test-object/root/epoch/function/input bindings against canonical bodies.
No expected-value override or omitted selected entry is allowed. Reports carry
comparison, not Passed; admission requires protected plan and measurements.

## 6. Refusals and decoder order

Preserve earlier owner numeric/symbol errors. Native failures use
`{numeric:UInt32,symbol:Text,details:Option<NativeDetails>}`. Details union:
1 `{test:Option<Id>,resource:UInt32,actual:UInt64,limit:UInt64}`;
2 `{expected:Id,actual:Id}`; 3 `{phase:UInt32}`. Resource detail tags for literal
policy fields are fuel1, memory2, output3, effects4, depth5, wall6, count7,
evidence8. These are a different named enum from NativeTermination resources.
Do not emit paths/secrets/arbitrary worker stderr as causal details.

Decode in order: total bound, magic/version, payload length, digest, strict
record shape, field ranges/counts, nested fields in tag order, cross-bindings.
No decoder fallback after magic/digest/profile failure. A registry reservation
does not make an unsupported profile accepted.

## 7. Measured supervisor and authenticated telemetry

Initial enforcer: root-owned `sley-test-supervisor.service`, authenticated local
Unix socket, one systemd **system** transient service per test. No
same-credential worker, arbitrary command/property interface or spawn-then-move
fallback. Authenticate peer UID and configured workspace/principal mapping.
The system manager installs group/limits before pinned worker exec. Trusted
manager setup precedes counted worker scope; dynamic loading and IPC decode
are inside it.

Unit configuration requires DynamicUser, private network/temp, no new
privileges, empty capability bound, protected system/home, read-only binary/
libraries/input, private bounded output, no writable cgroup or supervisor
socket, `MemoryMax=floor(request/page_size)*page_size`, zero swap budget,
process cap, control-group kill and manager runtime backstop. Worker has no
repository or signing/issuer keys. Missing controllers/properties/isolation
refuses. Zero installed memory cap refuses. Successful memory evidence requires
peak ≤ installed cap ≤ request and all limit/OOM/kill events zero.

Daemon monotonic elapsed is strictly less than wall_ms*1,000,000, checked for
overflow. Interval includes launch through full output and confirmed exit.
Deadline kills every descendant; reap confirmation budget2,000ms. Missing
confirmation degrades supervisor and prevents successful signatures/new runs.
Manager deadline is no later than daemon deadline plus teardown reserve.
Peer death/socket loss stops unit. Binding to daemon service lifetime plus
manager timeout survive caller/daemon failure. Restart kills/reconciles orphans;
it cannot sign an orphan as a previously successful run.

`SupervisorConfigV1` (`SLEYNHC1`):
`{version:UInt32=1,worker_sha256:Id,supervisor_sha256:Id,
unit_properties:List<Property>,allowed_callers:List<Caller>,page_size,
cleanup_millis:2000,launch_profile:UInt32=1}`.
Property is `{name:Text,value:Text}`, unique/sorted raw name UTF-8, ASCII names
≤128 and values≤4096 bytes, max64. Caller is `{uid:UInt32,workspace:Id,
principal:Id}`, sorted uid then IDs, max256. Config reflects explicit normalized required unit settings; paths/PIDs/runtime
unit names are excluded. Required property projection (exact names/values) is:
`DynamicUser=yes`, `PrivateNetwork=yes`, `PrivateTmp=yes`,
`NoNewPrivileges=yes`, `CapabilityBoundingSet=empty`, `ProtectSystem=strict`,
`ProtectHome=yes`, `ProtectControlGroups=yes`, `MemoryAccounting=yes`,
`MemorySwapMax=0`, `TasksMax=1`, `KillMode=control-group`,
`SendSIGKILL=yes`, `TimeoutStopUSec=2000000`. Resource-instantiated properties
use normalized values `MemoryMax=declared-page-floor` and
`RuntimeMaxUSec=declared-wall-plus-cleanup`; runner verifies actual installed
numeric values and binds them through attestation limits/config. Binary/input/
output mapping is fixed launch_profile1 and cannot be supplied by caller. N3
must prove all properties installed; unknown/missing/extra properties refuse
under this profile rather than relying on manager defaults. Services must bind
to the supervisor unit lifetime; that ownership is launch_profile1, not a
caller-chosen property. Configuration digest binds each run.

`MeasuredTestAttestationV1` (`SLEYMTA1`) exact record:

| Tag | Field | Type |
|---:|---|---|
| 1 | version | UInt32=1 |
| 2 | key_id | raw Ed25519 public key32 |
| 3 | trust_policy_id | Id |
| 4 | supervisor_config_id | Id |
| 5 | plan_id | Id |
| 6 | test_object | Id |
| 7 | execution_report_id | Option<Id> |
| 8 | attempt_nonce | host-random FixedBytes32 |
| 9 | workspace | Id |
| 10 | principal | Id |
| 11 | caller_uid | UInt32 |
| 12 | declared_limits | DeclaredLimits |
| 13 | installed_memory_cap | UInt64 |
| 14 | elapsed_ns | UInt64 |
| 15 | measured_memory_peak | UInt64 |
| 16 | memory_events | record {max,oom,oom_kill}, UInt64 |
| 17 | termination | UInt32 complete1, prelaunch-refused2, timeout3, killed4, crash5, enforcer-error6 |
| 18 | complete_output | Bool |
| 19 | empty_cgroup_confirmed | Bool |
| 20 | recorded_unix_millis | supervisor UInt64 historical trust time |
| 21 | signature | FixedBytes64 |

Ed25519 pure mode signs exact ASCII
`sley2.native-test-measurement-signature.v1` followed by
P(`SLEYMTA1`, record fields1..20). Full envelope ID includes signature. Require
strict RFC8032 canonical point/scalar encoding and non-small-order public/R
points; no alternate prehash modes. Signature prefix is a signing context, not
a BLAKE3 identifier domain. Key ID is raw public key. Provisioned trust grants
measurement role/scopes; bundled keys never authorize themselves.

Successful measured result requires Some execution ID, Complete termination,
complete output, confirmed empty cgroup, zero memory events, bounded peak and
strict wall deadline. VM mismatch can have valid measurement but cannot pass
admission. Missing/failed telemetry remains diagnostic. Only daemon holds the
root-only measurement key; separate acceptance key is unavailable to worker.

Both intended hosts must prove actual placement/controller availability,
distinct UID/control/key isolation, private IPC/network, exact deadlines,
peer SIGKILL cleanup, daemon SIGKILL cleanup and manager backstop. Preserve
probe/configuration receipts. Mock success cannot qualify an enforcer.

## 8. N2 and N1 acceptance requirements

N2 implements native types, VM entry, call/depth/output/fuel ordering and
observations; preserve every old vector/ID. Add zero/equality/one-over,
direct/predicate recursion, malformed input, output/trap and repeated-run
vectors; independent oracle reproduces canonical observation bytes/IDs.

N1 implements strict plan/report/approval codecs using N2 types, with empty
plan, exact comparisons, root/ObjectId/order substitutions, nested allocation
limits and canonical signatures. Parsed data never constructs protected
authority. N3 real supervisor, N4 protected selection, N5 receipt admission and
N6/N7 export/replay/transport are still required before selected-test commits.

## 9. Revision 2 contract-review responses

N0-01: fixed exact VM input/request/outcome/error ownership and API; TestCase
replay/observation/expected-body validation remains at the test owner. VM checks
only supplied graph/contract/effect obligations, with explicit new-versus-old
failure ordering. No tests/conformance-to-VM reverse edge.

N0-02: named `sley_mutate::encode_const_value` as byte owner and specified its
shared bounded sizing API before allocation. Output overflow counter is always
cap+1; all prior refusals/absent payload/u64::MAX and sizing charge/precedence
are explicit. Independent vectors must pin the resulting unique observations.


## 10. Revision 3 refusal mapping and measurement precedence

N0-04: §5 exhaustively maps every NativeExecutionError leaf, including nested
effect Type/Cfg failures. Only true S20-230 errors use new phase7; tags1..6 and
all existing numeric/symbol identities remain unchanged. N1 vectors must cover
every row, including both routes to nested Type/Cfg failures.

N0-05: §4 applies codec validation/hard-refusal precedence to every declared
cap. Only fully valid encodings yield Exact/OverLimit; cap overflow never waives
later canonical or hard-bound failures. N2 must test valid exact/over-limit,
small-cap plus invalid map/depth/collection overlaps, and u64::MAX, verifying
shared-codec error identity and deterministic OutputEncoding/0 versus
OutputBytes/cap+1 observations where the preceding VM checks permit sizing.

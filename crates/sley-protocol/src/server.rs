//! Deterministic SMP1 server (S20-410 slice B, contract `docs/spec/SMP1.md`
//! sections 3 through 7 and appendix A).
//!
//! The server owns one repository path and one negotiated profile. It
//! decodes request frames, enforces version, session, request-identity, and
//! method rules in contract precedence, dispatches the body to the owning
//! engine as an opaque frozen record, and answers with a response frame
//! whose bounded context is copied from the owner's response. It reads no
//! clock and no randomness, so equal requests over equal repository state
//! produce byte-identical responses.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use sley_check::TypeEnvironment;
use sley_conformance::{build_execution_report, execution_report_preimage};
use sley_id::{
    CandidateId, EntityId, ExecutionReportId, NativeAdmissionProfileId, NativeExecutionProfileId,
    ObjectId, ProtocolHandshakeId, SchemaEpochId, StateRoot, TransactionId,
};
use sley_id::{PrincipalId, ReceiptId};
use sley_mutate::{
    EntityObject, build_candidate, decode_candidate_record, decode_const_value, import_candidate,
    import_entity_object,
    value::{EntityBodyValue, TestCaseBody},
};
use sley_policy::{
    CandidateValidationContext, CandidateValidationLimits, NativeExplicitRootInputs,
    NativePlanErrorV1, NativePlanInputs, conformance_registry as policy_registry,
    import_policy_root, native_test_plan, native_test_plan_explicit_root, validate_candidate_bytes,
};
use sley_query::{
    Cursor, EntityReadCeilings, EntityReadError, EntityReadMethod, ImpactEdge, ImpactKind,
    IndexCompleteness, ModeledEntityKind, QueryLimits, RestrictedQuery, RootQuery, SnapshotContext,
    build_index_snapshot, build_restricted_query_request, execute_restricted_query,
};
use sley_repo::{
    BranchName, BranchRepository, BranchUpdateStatus, CompleteRootRequest, GcDecision, GcReport,
    IndexCacheError, MAX_ANCESTRY_NODES, MergeCommitInput, MergeOutcome, MergeSide,
    NativeExchangeTrust, NativeReplayRequest, NativeReplayStatus, ReportStoreErrorCode,
    RepositoryObjectVerifier, RepositoryQueryError, RetentionAnchor, RetentionKind,
    RetentionSnapshot, RetentionTarget, acquire_exclusive_gc, build_merge_plan,
    cached_complete_root_snapshot_id, commit_merge, compare_complete_roots,
    encode_verified_entity_read_response, export_repository_exchange, gc_collect, gc_dry_run,
    import_repository_exchange, judge_merge_verified, prepare_verified_entity_read,
    read_execution_report, replay_native_commit, run_root_query, run_root_query_fresh,
    store_execution_report, transaction_ancestry,
};
use sley_scb1::{
    encode_bytes, encode_list, encode_option_uvar, encode_record, encode_union, encode_uvar,
};
use sley_state_root::conformance_epoch_id as state_epoch_id;
use sley_state_root::{
    AcceptedStateRoot, conformance_registry as state_registry, import_state_root,
};
use sley_store::ObjectStore;
use sley_tests::{
    HistoricalTrustPolicyV1, NativeAggregateLimits, NativeTestPlanV1, NativeTestReportV1,
};
use sley_txn::{
    AttemptStatus, CommitInput, ImportedReceipt, NativeAcceptanceSigner, NativeAttemptId,
    NativeCommitInput, NativeDiagnosticAssembly, NativeTestExecutor, NativeVerifiedRevision,
    RepositoryMaintenanceGuard, TransactionRepository, TrustedGenesisInput, VerifiedRevision,
    acquire_shared_repository_maintenance, acquire_shared_repository_maintenance_nonblocking,
    assemble_diagnostic_report, initialize_repository_maintenance,
};
use sley_vm::native_execution::{
    NativeImplementationLimits, profile_id as native_execution_profile,
};
use sley_vm::{CacheProfile, ExecutionLimits, ExecutionRequest, LoweringInput, execute_function};

use crate::session::{
    CapsuleBindError, HeadBinding, SessionAuthority, SessionError, SessionErrorCode,
    fresh_server_nonce,
};
use crate::{
    BoundedContext, DecodedFrame, EncodedFrame, FEATURE_CANCEL, FEATURE_EXTENDED_EXECUTE,
    FEATURE_NATIVE_TESTS_V1, FEATURE_STREAM, FLAG_CANCEL, FLAG_FAILED, FrameKind, Hello,
    LimitProfile, Method, PROTOCOL_VERSION, PROTOCOL_VERSION_V2, PROTOCOL_VERSION_V3,
    ProtocolError, ProtocolErrorCode, ProtocolFailure, ProtocolFrame, RequestRegistry,
    Retryability, SelectedProfile, SessionId, decode_frame, decode_frame_for_version,
    encode_frame_for_version, negotiate_identity, negotiate_identity_versioned,
    stream_response_for_version,
};

/// Detail carried by `PROTOCOL_PAYLOAD_INVALID` when `report` names no
/// stored report (SMP1 appendix C).
pub const REPORT_UNKNOWN_DETAIL: &[u8] = b"REPORT-UNKNOWN";
/// Detail carried by `PROTOCOL_PAYLOAD_INVALID` when `execute` names no
/// Function of the bound root (SMP1 appendix C).
pub const FUNCTION_UNKNOWN_DETAIL: &[u8] = b"FUNCTION-UNKNOWN";
/// Reason carried for a reserved method on the S20-370 seam.
pub const RESERVED_SEAM_370_DETAIL: &[u8] = b"SMP1-RESERVED-S20-370";
/// Reason carried for a reserved method on the S20-620 seam.
pub const RESERVED_SEAM_620_DETAIL: &[u8] = b"SMP1-RESERVED-S20-620";

/// The versioned reason for a reserved tag: the seam that owns it
/// (contract section 4).
fn reserved_detail(method: Method) -> &'static [u8] {
    match method {
        Method::RefMoveProtected => RESERVED_SEAM_370_DETAIL,
        _ => RESERVED_SEAM_620_DETAIL,
    }
}

const ROOT_QUERY_MAGIC: &[u8; 8] = b"SLEYRQQ1";
const RESTRICTED_QUERY_MAGIC: &[u8; 8] = b"SLEYQRY1";
const OPTION_NONE: u32 = 1;
const OPTION_SOME: u32 = 2;
const MAX_BRANCH_LIST: u64 = 4_096;

type Result<T> = core::result::Result<T, ProtocolFailure>;

fn protocol_failure<T>(code: ProtocolErrorCode) -> Result<T> {
    Err(ProtocolFailure::protocol(code))
}

fn owner_failure<T>(symbol: &str, numeric: u32) -> Result<T> {
    Err(owner(symbol, numeric))
}

/// The exact symbols a client can retry after requerying the current head.
///
/// A suffix rule decided this until 2026-09-03 and answered `Never` for
/// `SESSION_STALE_HANDLE` and `STALE_ROOT` while answering `AfterRequery`
/// for `REF_CAS_STALE`, because only the latter ends in the word. The set is
/// explicit so word order cannot change a client's retry decision, and an
/// unlisted symbol stays `Never`, which is the fail-closed direction: a
/// client retries less than it could, never more than it should.
const RETRY_AFTER_REQUERY: [&str; 5] = [
    "REF_CAS_STALE",
    "REF_NAMED_CAS_STALE",
    "SESSION_ROOT_ADVANCED",
    "SESSION_STALE_HANDLE",
    "STALE_ROOT",
];

/// The exact symbols a client can retry after changing its declared limits
/// (contract section 6). This is an explicit list, never a suffix rule: a
/// new `*_RESOURCE_LIMIT` or `*_REQUIRED_FACT_OMITTED` symbol answers
/// `Never` until it is listed here and in the contract, which is the
/// fail-closed direction.
const RETRY_AFTER_LIMIT_CHANGE: [&str; 28] = [
    "BRANCH_RESOURCE_LIMIT",
    "CANDIDATE_TEST_RESOURCE_LIMIT",
    "CANDIDATE_VALIDATION_RESOURCE_LIMIT",
    "CFG_RESOURCE_LIMIT",
    "COMPARE_RESOURCE_LIMIT",
    "CONTEXT_CAPSULE_RESOURCE_LIMIT",
    "CONTRACT_TEST_PLAN_RESOURCE_LIMIT",
    "EFFECT_RESOURCE_LIMIT",
    "EXCHANGE_RESOURCE_LIMIT",
    "FINGERPRINT_RESOURCE_LIMIT",
    "GC_RESOURCE_LIMIT",
    "IMPACT_RESOURCE_LIMIT",
    "INDEX_SNAPSHOT_RESOURCE_LIMIT",
    "JSON_BRIDGE_RESOURCE_LIMIT",
    "MERGE_RESOURCE_LIMIT",
    "PACK_RESOURCE_LIMIT",
    "POLICY_ROOT_RESOURCE_LIMIT",
    "QUERY_REQUIRED_FACT_OMITTED",
    "QUERY_RESOURCE_LIMIT",
    "REPORT_RESOURCE_LIMIT",
    "RESTRICTED_CAPSULE_RESOURCE_LIMIT",
    "SCB_RESOURCE_LIMIT",
    "SSMC_RESOURCE_LIMIT",
    "TXN_RESOURCE_LIMIT",
    "TYPE_RESOURCE_LIMIT",
    "VM_EXEC_RESOURCE_LIMIT",
    "VM_LOWER_RESOURCE_LIMIT",
    "PROTOCOL_LIMIT_EXCEEDED",
];

/// Maps one owner symbol to the retryability SMP1 section 6 carries.
pub(crate) fn owner_retryability(symbol: &str) -> Retryability {
    if RETRY_AFTER_REQUERY.contains(&symbol) {
        return Retryability::AfterRequery;
    }
    if RETRY_AFTER_LIMIT_CHANGE.contains(&symbol) {
        return Retryability::AfterLimitChange;
    }
    Retryability::Never
}

fn owner(symbol: &str, numeric: u32) -> ProtocolFailure {
    let retryability = owner_retryability(symbol);
    ProtocolFailure {
        code: numeric,
        symbol: symbol.to_string(),
        phase: 0,
        retryability,
        incident: None,
        details: Vec::new(),
    }
}

fn unsupported(reason: &[u8]) -> ProtocolFailure {
    ProtocolFailure {
        details: reason.to_vec(),
        ..ProtocolFailure::protocol(ProtocolErrorCode::MethodUnsupported)
    }
}

/// A reserved tag names a real seam whose owner has not claimed it yet
/// (contract section 4): the detail names the seam, and the failure is
/// retryable after the capability appears.
fn reserved_refusal(method: Method) -> ProtocolFailure {
    let mut failure = unsupported(reserved_detail(method));
    failure.retryability = Retryability::AfterCapability;
    failure
}

/// Report-token time to live in milliseconds: five minutes, after which a
/// minted token refuses even inside its session (contract appendix C).
const DIAGNOSTIC_TOKEN_TTL_MILLIS: u64 = 300_000;
/// Maximum live report tokens per session.
const MAX_DIAGNOSTIC_TOKENS_PER_SESSION: usize = 32;
/// Maximum cached diagnostic report bytes per session (64 MiB).
const MAX_DIAGNOSTIC_BYTES_PER_SESSION: u64 = 67_108_864;
/// Maximum bytes served on one 605 report page (contract appendix C).
const MAX_DIAGNOSTIC_PAGE_BYTES: u64 = 65_536;
/// Maximum cached diagnostic attempts per server; the oldest goes on
/// overflow, and a re-submitted attempt simply re-executes.
const MAX_DIAGNOSTIC_ATTEMPTS: usize = 256;
/// Maximum cached replay queries per server; the oldest goes on
/// overflow, and a re-submitted attempt simply re-executes.
const MAX_REPLAY_ATTEMPTS: usize = 256;
/// Token derivation domain: the per-instance server nonce, session, and a
/// server counter make each token unpredictable to callers while keeping
/// one instance deterministic.
const DIAGNOSTIC_TOKEN_DOMAIN: &[u8] = b"sley2.diagnostic-report-token.v1";

/// One minted diagnostic report token with its cached report bytes.
///
/// The bound report identity and root are part of the minted capability:
/// 605 answers them back verbatim, so a token can never be rebound to a
/// different report or root.
struct DiagnosticToken {
    token: [u8; 32],
    session: SessionId,
    report_id: [u8; 32],
    bound_root: [u8; 32],
    report: Vec<u8>,
    created_millis: u64,
}

/// One completed diagnostic attempt with the response it produced.
///
/// Resubmission with identical bindings replays the cached response
/// without re-executing; any binding divergence refuses as an attempt
/// conflict. The attempt dies with its token: renewal, close, checkout,
/// and commit invalidate tokens, and an attempt whose token is gone can
/// never replay a dead capability.
struct DiagnosticAttempt {
    session: SessionId,
    bindings: Vec<u8>,
    response: Vec<u8>,
    selected_count: u64,
    token: [u8; 32],
    created_millis: u64,
}

/// One completed replay query with the response it produced.
///
/// Resubmission with identical bindings replays the cached response
/// without re-running the engine; any binding divergence refuses as an
/// attempt conflict. Like diagnostics, the attempt dies with its token:
/// a cached response whose token is gone re-executes fresh instead of
/// serving a dead capability. Replay attempts are never journaled, so
/// 607 always answers unknown for them.
struct ReplayAttempt {
    session: SessionId,
    bindings: Vec<u8>,
    response: Vec<u8>,
    compared_count: u64,
    token: Option<[u8; 32]>,
    created_millis: u64,
}

/// Operator-provisioned native commit authority, installed as one unit.
///
/// The v3 commit route cannot accept caller signing keys, grant bypasses,
/// or supervisor configuration: the executor runs the tests, the signer
/// claims the acceptance statement with the server's key, and the two
/// receiver manifests resolve every trust reference the receipt names. A
/// server without this authority refuses native commits before any
/// journal or accepted-state write.
pub struct NativeAuthority {
    /// Qualified test-execution dispatch for commit and replay.
    executor: Box<dyn NativeTestExecutor>,
    /// Configured acceptance signer claiming statements.
    signer: Box<dyn NativeAcceptanceSigner>,
    /// Receiver-provisioned trust manifests: measurement first, then
    /// acceptance. The array carries both sides together because the
    /// replay engine resolves every trust reference the receipt names
    /// against the supplied set.
    trust_manifests: [HistoricalTrustPolicyV1; 2],
}

impl NativeAuthority {
    /// Provisions the commit authority as one unit: test-execution
    /// dispatch, acceptance signer, measurement trust manifest, and
    /// acceptance trust manifest, in that argument order.
    #[must_use]
    pub fn provision(
        executor: Box<dyn NativeTestExecutor>,
        signer: Box<dyn NativeAcceptanceSigner>,
        measurement_trust: HistoricalTrustPolicyV1,
        acceptance_trust: HistoricalTrustPolicyV1,
    ) -> Self {
        Self {
            executor,
            signer,
            trust_manifests: [measurement_trust, acceptance_trust],
        }
    }

    /// Returns the receiver-provisioned measurement trust manifest.
    #[must_use]
    pub fn measurement_trust(&self) -> &HistoricalTrustPolicyV1 {
        &self.trust_manifests[0]
    }

    /// Returns the receiver-provisioned acceptance trust manifest.
    #[must_use]
    pub fn acceptance_trust(&self) -> &HistoricalTrustPolicyV1 {
        &self.trust_manifests[1]
    }
}

/// Accepted head of either receipt format for the session layer and the
/// native surfaces.
///
/// Legacy data paths keep the v1-only loader; session binding, the native
/// reads, and the v3 commit route serve both formats through the shared
/// state, object, and policy types. The format dispatches on the stored
/// bytes through the transaction owner's any-format loader, never on
/// caller assertion.
enum HeadRevision {
    V1(Box<VerifiedRevision>),
    Native(Box<NativeVerifiedRevision>),
}

impl HeadRevision {
    /// Returns the exact accepted revision identity.
    fn transaction_id(&self) -> TransactionId {
        match self {
            Self::V1(revision) => revision.transaction_id(),
            Self::Native(revision) => revision.transaction_id(),
        }
    }

    /// Returns the registry-authorized accepted semantic root.
    fn state_root(&self) -> &AcceptedStateRoot {
        match self {
            Self::V1(revision) => revision.state_root(),
            Self::Native(revision) => revision.state_root(),
        }
    }

    /// Returns the registry-authorized protected policy root.
    fn policy_root(&self) -> &sley_policy::AcceptedPolicyRoot {
        match self {
            Self::V1(revision) => revision.policy_root(),
            Self::Native(revision) => revision.policy_root(),
        }
    }

    /// Returns every exact live entity object in state-root binding order.
    fn objects(&self) -> &[EntityObject] {
        match self {
            Self::V1(revision) => revision.objects(),
            Self::Native(revision) => revision.objects(),
        }
    }

    /// Returns the complete sorted non-reusable identity ledger.
    fn tombstoned_entities(&self) -> &[EntityId] {
        match self {
            Self::V1(revision) => revision.tombstoned_entities(),
            Self::Native(revision) => revision.tombstoned_entities(),
        }
    }
}

/// Reads wall-clock milliseconds for diagnostic token expiry. Response
/// bytes stay a pure function of requests plus server state: the clock
/// only expires cached tokens, never enters a response.
fn system_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

/// The deterministic server over one repository.
pub struct Server {
    repository: PathBuf,
    profile: SelectedProfile,
    handshake_id: ProtocolHandshakeId,
    registry: RequestRegistry,
    authority: SessionAuthority,
    /// Bytes still available to each session under the negotiated
    /// `max_work` budget (contract appendix B).
    budgets: BTreeMap<SessionId, u64>,
    /// Explicit version-aware serving mode, carried from the constructor
    /// through decoding, dispatch, and response framing. Legacy servers
    /// retain v1-only behavior even when legacy negotiation selected
    /// version 2 from arbitrary hello offers.
    version_aware: bool,
    /// Test-only fault forcing an unexpected post-reservation encoding
    /// failure on the entity-read path, proving the debit is retained.
    #[cfg(test)]
    entity_encode_fault: bool,
    /// Test-only count of accepted-head loads through [`Server::head`],
    /// the single surface where the server loads a revision. A serving
    /// test resets it after setup and asserts per-answer loads, proving
    /// the retained admitted revision supplies the entire answer.
    #[cfg(test)]
    head_loads: core::cell::Cell<u64>,
    /// Configured diagnostic test-execution dispatch for the 601/602
    /// selection reads: server-operator configuration, never caller
    /// authority. `None` refuses diagnostics before any owner work.
    executor: Option<Box<dyn NativeTestExecutor>>,
    /// Provisioned native commit authority for the v3 commit route and
    /// replay trust resolution. `None` refuses native commits before any
    /// journal or accepted-state write; replay without it runs the engine
    /// with no executor and no trust, so verifiable histories answer
    /// untrusted.
    native_authority: Option<NativeAuthority>,
    /// Completed replay queries keyed by client attempt identity.
    replay_attempts: BTreeMap<[u8; 16], ReplayAttempt>,
    /// Wall-clock source for diagnostic token expiry; tests inject a
    /// manual clock, production uses system time.
    now_millis: fn() -> u64,
    /// Server counter distinguishing minted diagnostic tokens.
    diagnostic_token_counter: u64,
    /// Live diagnostic report tokens with their cached report bytes.
    diagnostic_tokens: Vec<DiagnosticToken>,
    /// Completed diagnostic attempts keyed by client attempt identity.
    diagnostic_attempts: BTreeMap<[u8; 16], DiagnosticAttempt>,
}

impl core::fmt::Debug for Server {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Server")
            .field("repository", &self.repository)
            .field("profile", &self.profile)
            .field("handshake_id", &self.handshake_id)
            .field("registry", &self.registry)
            .field("authority", &self.authority)
            .field("budgets", &self.budgets)
            .field("version_aware", &self.version_aware)
            .field("executor_configured", &self.executor.is_some())
            .field(
                "native_authority_configured",
                &self.native_authority.is_some(),
            )
            .field("diagnostic_token_counter", &self.diagnostic_token_counter)
            .field("diagnostic_tokens", &self.diagnostic_tokens.len())
            .field("diagnostic_attempts", &self.diagnostic_attempts.len())
            .finish_non_exhaustive()
    }
}

impl DiagnosticToken {
    fn expired(&self, now: u64) -> bool {
        now.saturating_sub(self.created_millis) > DIAGNOSTIC_TOKEN_TTL_MILLIS
    }
}

/// One answered frame and the request identity it belongs to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Answer {
    pub session: Option<SessionId>,
    pub request_id: u64,
    pub method: u32,
    pub failed: bool,
    /// The response frame (the last frame of a streamed answer).
    pub frame: EncodedFrame,
    /// Event frames preceding the response when the body was streamed.
    pub events: Vec<EncodedFrame>,
}

impl Server {
    /// Creates a server over a repository, re-deriving the negotiated profile
    /// and the transcript-bound handshake identity from the hellos as
    /// observed (contract section 2, threat T45). An asserted selection is
    /// never accepted: identity always comes from per-peer re-derivation,
    /// so any tamper of either hello makes `session.open` fail
    /// `PROTOCOL_DOWNGRADE`.
    ///
    /// # Errors
    ///
    /// Returns the negotiation failure, or `PROTOCOL_INTERNAL_INVARIANT`
    /// when the transcript cannot be digested.
    pub fn new(
        repository: impl Into<PathBuf>,
        client_hello: &Hello,
        server_hello: &Hello,
    ) -> core::result::Result<Self, ProtocolError> {
        let (profile, handshake_id) = negotiate_identity(client_hello, server_hello)?;
        Ok(Self {
            repository: repository.into(),
            profile,
            handshake_id,
            registry: RequestRegistry::new(),
            authority: SessionAuthority::new(handshake_id, fresh_server_nonce()),
            budgets: BTreeMap::new(),
            version_aware: false,
            #[cfg(test)]
            entity_encode_fault: false,
            #[cfg(test)]
            head_loads: core::cell::Cell::new(0),
            executor: None,
            native_authority: None,
            now_millis: system_millis,
            diagnostic_token_counter: 0,
            diagnostic_tokens: Vec::new(),
            diagnostic_attempts: BTreeMap::new(),
            replay_attempts: BTreeMap::new(),
        })
    }

    /// Creates an explicitly version-aware server, deriving the selection
    /// with version-aware negotiation (contract
    /// `docs/spec/ENTITY_READ_PROFILE_V2.md` section 2). Opt-in is the
    /// constructor, never the negotiated version alone: a legacy server
    /// keeps legacy decoding, dispatch, and framing even when its
    /// selection names version 2.
    ///
    /// # Errors
    ///
    /// Returns the negotiation failure, or `PROTOCOL_INTERNAL_INVARIANT`
    /// when the transcript cannot be digested.
    pub fn new_versioned(
        repository: impl Into<PathBuf>,
        client_hello: &Hello,
        server_hello: &Hello,
    ) -> core::result::Result<Self, ProtocolError> {
        let (profile, handshake_id) = negotiate_identity_versioned(client_hello, server_hello)?;
        Ok(Self {
            repository: repository.into(),
            profile,
            handshake_id,
            registry: RequestRegistry::new(),
            authority: SessionAuthority::new(handshake_id, fresh_server_nonce()),
            budgets: BTreeMap::new(),
            version_aware: true,
            #[cfg(test)]
            entity_encode_fault: false,
            #[cfg(test)]
            head_loads: core::cell::Cell::new(0),
            executor: None,
            native_authority: None,
            now_millis: system_millis,
            diagnostic_token_counter: 0,
            diagnostic_tokens: Vec::new(),
            diagnostic_attempts: BTreeMap::new(),
            replay_attempts: BTreeMap::new(),
        })
    }

    /// The hello this server offers to an endpoint (S20-430): protocol
    /// version 1, the frozen conformance schema epoch, the limit ceilings,
    /// every method the server dispatches (reserved methods are not
    /// offered), the cancel and stream features, and no adapters
    /// or effects.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_INTERNAL_INVARIANT` when the conformance epoch
    /// cannot be derived.
    pub fn offered_hello() -> core::result::Result<Hello, ProtocolError> {
        let epoch = state_epoch_id()
            .map_err(|_| ProtocolError::new(ProtocolErrorCode::InternalInvariant))?;
        let hello = Hello {
            protocol_versions: vec![PROTOCOL_VERSION],
            schema_epochs: vec![epoch],
            limits: LimitProfile::maximum(),
            methods: Method::ALL
                .iter()
                .copied()
                .filter(|method| !method.is_reserved())
                .map(Method::tag)
                .collect(),
            features: FEATURE_CANCEL | FEATURE_STREAM,
            adapters: Vec::new(),
            effects: Vec::new(),
        };
        hello.validate()?;
        Ok(hello)
    }

    /// The hello an explicitly version-aware server offers: versions 1
    /// and 2 with the v2 method table. The hello frame itself still
    /// travels at frame version 1 so a v1 peer can read the offer.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_INTERNAL_INVARIANT` when the conformance epoch
    /// cannot be derived.
    pub fn offered_hello_versioned() -> core::result::Result<Hello, ProtocolError> {
        let epoch = state_epoch_id()
            .map_err(|_| ProtocolError::new(ProtocolErrorCode::InternalInvariant))?;
        let hello = Hello {
            protocol_versions: vec![PROTOCOL_VERSION, PROTOCOL_VERSION_V2],
            schema_epochs: vec![epoch],
            limits: LimitProfile::maximum(),
            methods: Method::V2_ALL
                .iter()
                .copied()
                .filter(|method| !method.is_reserved())
                .map(Method::tag)
                .collect(),
            features: FEATURE_CANCEL | FEATURE_STREAM,
            adapters: Vec::new(),
            effects: Vec::new(),
        };
        hello.validate()?;
        Ok(hello)
    }

    /// The hello a native-capable server offers: versions 1 through 3 with
    /// the v3 method table and the native-tests feature bit. The hello
    /// frame itself still travels at frame version 1 so older peers can
    /// read the offer and negotiate down. The live native reads 601/602
    /// (since N7c), 605 (since N7d-1) and 606/607 (since N7d-2) are
    /// offered (listable only with version 3 and the bit); older
    /// negotiations still refuse 601/602 as reserved byte-for-byte, while
    /// 605–607 never existed in the frozen v1/v2 tables and refuse at
    /// decode as unsupported. Without the bit, negotiation strips native
    /// tags and they refuse as not-negotiated.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_INTERNAL_INVARIANT` when the conformance epoch
    /// cannot be derived.
    pub fn offered_hello_v3() -> core::result::Result<Hello, ProtocolError> {
        let epoch = state_epoch_id()
            .map_err(|_| ProtocolError::new(ProtocolErrorCode::InternalInvariant))?;
        let hello = Hello {
            protocol_versions: vec![PROTOCOL_VERSION, PROTOCOL_VERSION_V2, PROTOCOL_VERSION_V3],
            schema_epochs: vec![epoch],
            limits: LimitProfile::maximum(),
            methods: Method::V3_ALL
                .iter()
                .copied()
                .filter(|method| !method.is_reserved() || method.is_native_test())
                .map(Method::tag)
                .collect(),
            features: FEATURE_CANCEL | FEATURE_STREAM | FEATURE_NATIVE_TESTS_V1,
            adapters: Vec::new(),
            effects: Vec::new(),
        };
        hello.validate()?;
        Ok(hello)
    }

    /// The frame version this server answers with: the selected version on
    /// the explicit path, always 1 on the legacy path.
    fn response_version(&self) -> u32 {
        if self.version_aware {
            self.profile.protocol_version
        } else {
            PROTOCOL_VERSION
        }
    }

    #[must_use]
    pub fn repository(&self) -> &Path {
        &self.repository
    }

    /// Installs the configured diagnostic test-execution dispatch serving
    /// the 601/602 selection reads. The executor is operator configuration;
    /// without one every selection read refuses before any owner work.
    pub fn set_executor(&mut self, executor: Box<dyn NativeTestExecutor>) {
        self.executor = Some(executor);
    }

    /// Installs the provisioned native commit authority serving the v3
    /// commit route and replay trust resolution. Signer and manifests
    /// travel as one unit because a commit needs all three: without the
    /// authority every native commit refuses before any journal or
    /// accepted-state write.
    pub fn set_native_authority(&mut self, authority: NativeAuthority) {
        self.native_authority = Some(authority);
    }

    /// Overrides the wall-clock source for diagnostic token expiry. Tests
    /// inject a manual clock to prove TTL behavior deterministically.
    pub fn set_clock(&mut self, now: fn() -> u64) {
        self.now_millis = now;
    }

    #[must_use]
    pub const fn profile(&self) -> &SelectedProfile {
        &self.profile
    }

    #[must_use]
    pub const fn handshake_id(&self) -> ProtocolHandshakeId {
        self.handshake_id
    }

    /// Crate-internal access to the session authority for the retention
    /// tests: production code reaches the authority only through the
    /// session methods above, never directly.
    #[cfg(test)]
    pub(crate) fn authority_mut(&mut self) -> &mut SessionAuthority {
        &mut self.authority
    }

    /// Arms the test-only post-reservation encoding fault on the
    /// entity-read path.
    #[cfg(test)]
    pub(crate) fn set_entity_encode_fault(&mut self, fault: bool) {
        self.entity_encode_fault = fault;
    }

    /// Test-only wall-clock override for diagnostic token expiry.
    #[cfg(test)]
    pub(crate) fn set_clock_millis(&mut self, now: fn() -> u64) {
        self.now_millis = now;
    }

    /// Test-only accepted-head load count through [`Server::head`].
    #[cfg(test)]
    pub(crate) fn head_load_count(&self) -> u64 {
        self.head_loads.get()
    }

    /// Test-only reset of the accepted-head load count.
    #[cfg(test)]
    pub(crate) fn reset_head_load_count(&self) {
        self.head_loads.set(0);
    }

    /// Answers one complete request frame with one response frame.
    ///
    /// Frame-level failures are answered without a session and with request
    /// identifier zero. Every other failure is answered under the request's
    /// session and identifier with a `ProtocolFailure` body.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_INTERNAL_INVARIANT` only when the response frame
    /// itself cannot be encoded.
    pub fn answer(&mut self, request_bytes: &[u8]) -> core::result::Result<Answer, ProtocolError> {
        let mut answers = self.answer_batch(&[request_bytes])?;
        answers
            .pop()
            .ok_or(ProtocolError::new(ProtocolErrorCode::InternalInvariant))
    }

    /// Answers a batch of request frames read together (contract appendix
    /// B): every frame is decoded and admitted in order first, cancellations
    /// (flag bit 0 or method 603) naming a request of the same batch that
    /// has not yet executed take effect, then the surviving requests execute
    /// in order. A cancelled request is answered `PROTOCOL_CANCELLED` and
    /// never runs.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_INTERNAL_INVARIANT` only when a response frame
    /// cannot be encoded.
    pub fn answer_batch(
        &mut self,
        requests: &[&[u8]],
    ) -> core::result::Result<Vec<Answer>, ProtocolError> {
        let mut decoded: Vec<Option<ProtocolFrame>> = Vec::with_capacity(requests.len());
        let mut early: Vec<Option<Answer>> = Vec::with_capacity(requests.len());
        for request_bytes in requests {
            let decoded_frame = if self.version_aware {
                decode_frame_for_version(
                    request_bytes,
                    self.profile.limits.max_frame_bytes,
                    self.profile.protocol_version,
                )
            } else {
                decode_frame(request_bytes, self.profile.limits.max_frame_bytes)
            };
            match decoded_frame {
                Ok((DecodedFrame::Request(frame), _)) => {
                    decoded.push(Some(frame));
                    early.push(None);
                }
                Ok(_) => {
                    decoded.push(None);
                    early.push(Some(self.respond(
                        None,
                        0,
                        0,
                        Err(ProtocolFailure::protocol(ProtocolErrorCode::FrameInvalid)),
                    )?));
                }
                Err(error) => {
                    decoded.push(None);
                    early.push(Some(self.respond(
                        None,
                        0,
                        0,
                        Err(ProtocolFailure::protocol(error.code())),
                    )?));
                }
            }
        }
        let mut cancelled: BTreeSet<(SessionId, u64)> = BTreeSet::new();
        for frame in decoded.iter().flatten() {
            let Some(session) = frame.session else {
                continue;
            };
            if frame.flags & FLAG_CANCEL != 0 {
                cancelled.insert((session, frame.request_id));
            }
            if frame.method == Method::Cancel.tag()
                && let Ok(target) = single_uvar(&frame.body)
            {
                cancelled.insert((session, target));
            }
        }
        let mut answers = Vec::with_capacity(requests.len());
        for (slot, frame) in decoded.into_iter().enumerate() {
            if let Some(answer) = early[slot].take() {
                answers.push(answer);
                continue;
            }
            let Some(frame) = frame else {
                return Err(ProtocolError::new(ProtocolErrorCode::InternalInvariant));
            };
            let is_cancel_method = frame.method == Method::Cancel.tag();
            let outcome = match frame.session {
                Some(session)
                    if !is_cancel_method && cancelled.contains(&(session, frame.request_id)) =>
                {
                    self.admit_only(session, frame.request_id)
                        .and_then(|()| protocol_failure(ProtocolErrorCode::Cancelled))
                }
                _ => self.dispatch(&frame),
            };
            // Session-less answers carry identifier 0 (contract section 3
            // and appendix B): a malformed pre-session identifier is
            // answered, never echoed.
            let answer_id = if frame.session.is_none() {
                0
            } else {
                frame.request_id
            };
            answers.push(self.respond(frame.session, answer_id, frame.method, outcome)?);
        }
        Ok(answers)
    }

    /// Admits a cancelled request's identifier so the sequence stays exact,
    /// then releases its slot without executing anything.
    fn admit_only(&mut self, session: SessionId, request_id: u64) -> Result<()> {
        self.registry
            .admit(session, request_id, self.profile.limits.max_inflight)
            .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        self.registry
            .complete(session)
            .map_err(|error| ProtocolFailure::protocol(error.code()))
    }

    /// Remaining budget of an open session, or `None` when unknown.
    #[must_use]
    pub fn remaining_budget(&self, session: SessionId) -> Option<u64> {
        self.budgets.get(&session).copied()
    }

    #[cfg(test)]
    pub(crate) fn profile_limits_for_test(&self) -> LimitProfile {
        self.profile.limits
    }

    #[cfg(test)]
    pub(crate) fn respond_for_test(
        &self,
        session: Option<SessionId>,
        request_id: u64,
        method: u32,
        outcome: Result<(Vec<u8>, BoundedContext)>,
    ) -> core::result::Result<Answer, ProtocolError> {
        self.respond(session, request_id, method, outcome)
    }

    fn respond(
        &self,
        session: Option<SessionId>,
        request_id: u64,
        method: u32,
        outcome: Result<(Vec<u8>, BoundedContext)>,
    ) -> core::result::Result<Answer, ProtocolError> {
        let version = self.response_version();
        let (body, bounds, failed) = match outcome {
            Ok((body, bounds)) => (body, bounds, false),
            Err(failure) => (
                failure.encode()?,
                BoundedContext {
                    applied_limits: self.profile.limits,
                    ..BoundedContext::none()
                },
                true,
            ),
        };
        // Entity reads on the explicit path never stream: one direct frame
        // or a limit failure with no partial body and no events.
        if !failed && self.version_aware && is_entity_read_method(method) {
            return self.respond_entity_direct(session, request_id, method, body, bounds);
        }
        // The negotiated limits bind the transport outcome (contract
        // section 5): a successful body that does not fit fails with no
        // partial body, on the single-frame and streaming paths alike.
        if !failed {
            let limits = self.profile.limits;
            let body_len = u64::try_from(body.len())
                .map_err(|_| ProtocolError::new(ProtocolErrorCode::InternalInvariant))?;
            if body_len > limits.max_response_bytes
                || bounds.returned_bytes > limits.max_response_bytes
                || bounds.returned_entities > limits.max_entities
                || bounds.returned_edges > limits.max_edges
                || bounds.reached_depth > limits.max_depth
            {
                return self.refuse_transport(session, request_id, method);
            }
        }
        let frame = ProtocolFrame {
            protocol_version: version,
            session,
            request_id,
            kind: FrameKind::Response,
            method,
            flags: if failed { FLAG_FAILED } else { 0 },
            bounds,
            body,
        };
        let stream_negotiated = self.profile.features & FEATURE_STREAM != 0;
        let mut frames = match stream_response_for_version(
            &frame,
            self.profile.limits.max_frame_bytes,
            stream_negotiated,
            version,
        ) {
            Ok(frames) => frames,
            Err(error) if error.code() == ProtocolErrorCode::LimitExceeded && !failed => {
                // The body cannot travel: answer the limit failure instead,
                // with no partial body.
                let failure =
                    ProtocolFailure::protocol(ProtocolErrorCode::LimitExceeded).encode()?;
                let refused = ProtocolFrame {
                    flags: FLAG_FAILED,
                    bounds: BoundedContext {
                        applied_limits: self.profile.limits,
                        ..BoundedContext::none()
                    },
                    body: failure,
                    ..frame
                };
                return Ok(Answer {
                    session,
                    request_id,
                    method,
                    failed: true,
                    frame: encode_frame_for_version(&refused, version)?,
                    events: Vec::new(),
                });
            }
            Err(error) => return Err(error),
        };
        let last = frames
            .pop()
            .ok_or(ProtocolError::new(ProtocolErrorCode::InternalInvariant))?;
        Ok(Answer {
            session,
            request_id,
            method,
            failed,
            frame: last,
            events: frames,
        })
    }

    /// Answers a transport-level limit refusal at the selected version with
    /// no partial body and no events.
    fn refuse_transport(
        &self,
        session: Option<SessionId>,
        request_id: u64,
        method: u32,
    ) -> core::result::Result<Answer, ProtocolError> {
        let version = self.response_version();
        let failure = ProtocolFailure::protocol(ProtocolErrorCode::LimitExceeded).encode()?;
        let refused = ProtocolFrame {
            protocol_version: version,
            session,
            request_id,
            kind: FrameKind::Response,
            method,
            flags: FLAG_FAILED,
            bounds: BoundedContext {
                applied_limits: self.profile.limits,
                ..BoundedContext::none()
            },
            body: failure,
        };
        Ok(Answer {
            session,
            request_id,
            method,
            failed: true,
            frame: encode_frame_for_version(&refused, version)?,
            events: Vec::new(),
        })
    }
    /// the selected version: no streaming, no events, no partial body.
    fn respond_entity_direct(
        &self,
        session: Option<SessionId>,
        request_id: u64,
        method: u32,
        body: Vec<u8>,
        bounds: BoundedContext,
    ) -> core::result::Result<Answer, ProtocolError> {
        let version = self.response_version();
        let limits = self.profile.limits;
        let body_len = u64::try_from(body.len())
            .map_err(|_| ProtocolError::new(ProtocolErrorCode::InternalInvariant))?;
        if body_len > limits.max_response_bytes
            || bounds.returned_bytes > limits.max_response_bytes
            || bounds.returned_entities > limits.max_entities
            || bounds.returned_edges > limits.max_edges
            || bounds.reached_depth > limits.max_depth
        {
            return self.refuse_entity_direct(session, request_id, method);
        }
        let frame = ProtocolFrame {
            protocol_version: version,
            session,
            request_id,
            kind: FrameKind::Response,
            method,
            flags: 0,
            bounds,
            body,
        };
        match crate::encode_single_frame_direct(&frame, limits.max_frame_bytes) {
            Ok(encoded) => Ok(Answer {
                session,
                request_id,
                method,
                failed: false,
                frame: encoded,
                events: Vec::new(),
            }),
            Err(error) if error.code() == ProtocolErrorCode::FrameTooLarge => {
                self.refuse_entity_direct(session, request_id, method)
            }
            Err(error) => Err(error),
        }
    }

    fn refuse_entity_direct(
        &self,
        session: Option<SessionId>,
        request_id: u64,
        method: u32,
    ) -> core::result::Result<Answer, ProtocolError> {
        let version = self.response_version();
        let failure = ProtocolFailure::protocol(ProtocolErrorCode::LimitExceeded).encode()?;
        let refused = ProtocolFrame {
            protocol_version: version,
            session,
            request_id,
            kind: FrameKind::Response,
            method,
            flags: FLAG_FAILED,
            bounds: BoundedContext {
                applied_limits: self.profile.limits,
                ..BoundedContext::none()
            },
            body: failure,
        };
        Ok(Answer {
            session,
            request_id,
            method,
            failed: true,
            frame: encode_frame_for_version(&refused, version)?,
            events: Vec::new(),
        })
    }

    fn dispatch(&mut self, frame: &ProtocolFrame) -> Result<(Vec<u8>, BoundedContext)> {
        // A frame below the selection is a downgrade attempt; a frame
        // above it names a version the selection does not know (contract
        // section 2, `SelectedProfile::check_claim`).
        if let Err(error) = self
            .profile
            .check_claim(frame.protocol_version, self.profile.schema_epoch)
        {
            return Err(ProtocolFailure::protocol(error.code()));
        }
        // Bounds ride responses only (contract section 5): a request
        // carrying nonzero bounds is malformed.
        if frame.bounds != BoundedContext::none() {
            return protocol_failure(ProtocolErrorCode::FrameInvalid);
        }
        // The legacy decoder stays v1-only: version-2 tags refuse on every
        // v1 serving path, including an opaque negotiated intersection
        // that retained their numerics.
        let method = if self.version_aware {
            Method::from_tag_versioned(frame.method, self.profile.protocol_version)
                .map_err(|error| ProtocolFailure::protocol(error.code()))?
        } else {
            Method::from_tag(frame.method)
                .map_err(|error| ProtocolFailure::protocol(error.code()))?
        };
        // The pre-session space carries identifier 0 only (contract
        // section 3): hello, `session.open`, and the genesis path share
        // no counter with any session.
        if frame.session.is_none() && frame.request_id != 0 {
            return protocol_failure(ProtocolErrorCode::FrameInvalid);
        }
        if method == Method::SessionOpen {
            if frame.session.is_some() {
                return protocol_failure(ProtocolErrorCode::FrameInvalid);
            }
            return self.session_open(&frame.body);
        }
        // A repository without an accepted head cannot bind a session, so
        // the two methods that create one may travel without a session
        // only while no head exists (contract section 2): once a head
        // exists they require a session like every other method, because
        // `exchange.import` advances the head and would otherwise
        // invalidate every live binding from an unbound caller.
        if frame.session.is_none()
            && matches!(method, Method::WorkspaceCreate | Method::ExchangeImport)
        {
            if !self.profile.admits(method) {
                return Err(unsupported(b"SMP1-METHOD-NOT-NEGOTIATED"));
            }
            if self.head_binding().is_ok() {
                return Err(session_failure(SessionError::new(
                    crate::session::SessionErrorCode::BindingInvalid,
                )));
            }
            return match method {
                Method::WorkspaceCreate => self.workspace_create(&frame.body),
                _ => self.exchange_import(&frame.body),
            };
        }
        let Some(session) = frame.session else {
            return protocol_failure(ProtocolErrorCode::SessionClosed);
        };
        // Liveness precedes admission (contract section 3): a name no
        // live session holds answers `SESSION_UNKNOWN`, a remembered
        // close answers `PROTOCOL_SESSION_CLOSED`, and only a live name
        // reaches request-identity admission.
        if self.authority.record(session).is_none() {
            if self.registry.is_closed(session) {
                return protocol_failure(ProtocolErrorCode::SessionClosed);
            }
            return Err(session_failure(SessionError::new(
                crate::session::SessionErrorCode::Unknown,
            )));
        }
        self.registry
            .admit(session, frame.request_id, self.profile.limits.max_inflight)
            .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        // Entity reads on the explicit path retain the admitted revision
        // through preparation with no second head load; every other method
        // keeps the legacy admission order and accounting.
        if self.version_aware && matches!(method, Method::EntityVersion | Method::EntitySignature) {
            return self.dispatch_entity_read(session, method, frame);
        }
        let outcome = match self.session_check(session, method) {
            Ok(()) => {
                if self.budgets.get(&session).copied().unwrap_or(0) == 0 {
                    self.registry
                        .complete(session)
                        .map_err(|error| ProtocolFailure::protocol(error.code()))?;
                    return protocol_failure(ProtocolErrorCode::LimitExceeded);
                }
                // Dispatch costs one unit up front, so a request that
                // reaches an engine and fails still costs work and cannot
                // repeat forever (contract section 7). A successful
                // response additionally charges its returned bytes below.
                if let Some(remaining) = self.budgets.get_mut(&session) {
                    *remaining = remaining.saturating_sub(1);
                }
                self.dispatch_admitted(session, method, frame)
            }
            Err(failure) => Err(failure),
        };
        // Synchronous server: the request completes before the next frame.
        if self.registry.is_open(session) {
            self.registry
                .complete(session)
                .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        }
        if let Ok((body, _)) = &outcome {
            // One unit per returned byte on top of the dispatch unit,
            // never below zero.
            let charge = to_u64(body.len())?;
            if let Some(remaining) = self.budgets.get_mut(&session) {
                *remaining = remaining.saturating_sub(charge);
            }
        }
        outcome
    }

    fn dispatch_admitted(
        &mut self,
        session: SessionId,
        method: Method,
        frame: &ProtocolFrame,
    ) -> Result<(Vec<u8>, BoundedContext)> {
        if method.is_native_test() && self.native_tests_live() && self.profile.admits(method) {
            return self.tests_native(session, method, &frame.body);
        }
        // The v3 native commit route owns the existing commit method under
        // negotiated version 3 with the native-tests bit (contract
        // appendix C, revision 5): the four-field native payload replaces
        // the legacy payload there, routed by negotiation and never by
        // sniffing. Every other version keeps the legacy commit below.
        if method == Method::Commit && self.native_tests_live() && self.profile.admits(method) {
            return self.commit_native_route(session, &frame.body);
        }
        if !self.profile.admits(method) || method.is_reserved() {
            if method.is_reserved() {
                return Err(reserved_refusal(method));
            }
            return Err(unsupported(b"SMP1-METHOD-NOT-NEGOTIATED"));
        }
        let body = frame.body.as_slice();
        match method {
            Method::SessionOpen => protocol_failure(ProtocolErrorCode::InternalInvariant),
            Method::SessionRenew => self.session_renew(session, body),
            Method::SessionClose => self.session_close(session),
            Method::SessionCapabilities => {
                let preimage = self
                    .profile
                    .preimage()
                    .map_err(|error| ProtocolFailure::protocol(error.code()))?;
                self.plain(preimage)
            }
            Method::SessionBudgets => {
                let remaining = LimitProfile {
                    max_work: self.budgets.get(&session).copied().unwrap_or(0),
                    ..self.profile.limits
                };
                let limits = encode_limits(&remaining)?;
                self.plain(limits)
            }
            Method::RefsList => self.refs_list(body),
            Method::RefsResolve => self.refs_resolve(body),
            Method::RevisionRead => self.revision_read(body),
            Method::BranchCreate => self.branch_create(body),
            Method::BranchAdvance => self.branch_advance(body),
            Method::Compare => self.compare(body),
            Method::MergeJudge => self.merge_judge(body),
            Method::ExchangeExport => self.exchange_export(),
            Method::RefsRecover => self.refs_recover(),
            Method::QueryRoot | Method::QueryContinue => {
                self.query_root(body, method == Method::QueryContinue)
            }
            Method::Capsule => self.capsule(body, session),
            Method::HandleExpand => self.handle_expand(body, session),
            Method::QueryRestricted => self.query_restricted(body),
            Method::ReceiptRead => self.receipt_read(body),
            Method::Checkout => self.checkout(session, body),
            Method::Recovery => self.recovery(),
            Method::Cancel => {
                // Every request completes before the next frame is read, so
                // the named request has already been answered.
                let _ = single_uvar(body)?;
                self.plain(Vec::new())
            }
            Method::WorkspaceCreate => self.workspace_create(body),
            Method::WorkspaceOpen => self.workspace_open(body),
            Method::MergeCommit => self.merge_commit(body),
            Method::ExchangeImport => self.exchange_import(body),
            Method::CandidateCreate => self.candidate_create(body),
            Method::CandidateInspect => self.candidate_inspect(body),
            Method::CandidateDiscard => {
                // The server holds no candidate state: candidates are
                // caller-held bytes, so a discard only verifies the bytes.
                import_candidate(body).map_err(|error| owner(error.code(), 0))?;
                self.plain(Vec::new())
            }
            Method::CandidateValidate => self.candidate_validate(body),
            Method::CandidateAppend => self.candidate_append(body),
            Method::Commit => self.commit(body),
            Method::GcDryRun => self.gc(body, session, false),
            Method::GcCollect => self.gc(body, session, true),
            Method::Execute => self.execute(body),
            Method::Report => self.report(body),
            Method::Diagnostics
            | Method::RefMoveProtected
            | Method::TestsReplay
            | Method::TestsAttemptStatus => Err(reserved_refusal(method)),
            // Live native reads never reach the generic dispatch: the
            // gate above routes them to `tests_native`, and every other
            // version refuses them as reserved before this arm.
            Method::TestsSelected | Method::TestsAffected | Method::TestsReportRead => {
                protocol_failure(ProtocolErrorCode::InternalInvariant)
            }
            Method::EntityVersion | Method::EntitySignature => {
                // Served only through the explicit entity-read path with
                // its retained revision and debit table; reaching the
                // generic owner dispatch is an internal invariant breach.
                protocol_failure(ProtocolErrorCode::InternalInvariant)
            }
        }
    }

    /// Renews one session binding (contract section 2): the renew body
    /// names the session being renewed and must agree with the frame's
    /// session scope. Renewal rebinds the root the diagnostic tokens were
    /// minted under, so the session's tokens and attempts die here.
    fn session_renew(
        &mut self,
        session: SessionId,
        body: &[u8],
    ) -> Result<(Vec<u8>, BoundedContext)> {
        if fixed32(body)? != *session.as_bytes() {
            return protocol_failure(ProtocolErrorCode::FrameInvalid);
        }
        let (head, binding) = self.head_binding_mixed()?;
        let record = self
            .authority
            .renew_session(session, &binding, head.state_root())
            .map_err(session_failure)?;
        self.invalidate_diagnostic_session(session);
        self.plain(record.session_id.as_bytes().to_vec())
    }

    /// Closes one session, releasing its budgets, bindings, and diagnostic
    /// tokens and attempts with it.
    fn session_close(&mut self, session: SessionId) -> Result<(Vec<u8>, BoundedContext)> {
        self.registry
            .close(session, self.profile.limits.max_sessions)
            .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        self.authority
            .close_session(session)
            .map_err(session_failure)?;
        self.budgets.remove(&session);
        self.invalidate_diagnostic_session(session);
        self.plain(Vec::new())
    }

    /// Whether the live native reads dispatch on this server: explicit
    /// version-aware serving with a selected version 3 and the negotiated
    /// native-tests bit. Legacy servers keep refusing them as reserved even
    /// when a negotiated intersection retained their numerics.
    fn native_tests_live(&self) -> bool {
        self.version_aware
            && self.profile.protocol_version == PROTOCOL_VERSION_V3
            && self.profile.features & FEATURE_NATIVE_TESTS_V1 != 0
    }

    /// Routes a live native read after the admission gate proved
    /// version 3, the bit, and negotiation. The session root must still be
    /// the accepted head: diagnostics answer over the live binding, replay
    /// and status resolve over the bound head's history, and a session left
    /// behind by a head advance fails stale instead of answering over a
    /// root the head no longer names. Report paging serves immutable
    /// cached bytes rather than head state, but the same staleness bar
    /// keeps one session view coherent: a token minted under a moved-past
    /// root is already dead by invalidation.
    fn tests_native(
        &mut self,
        session: SessionId,
        method: Method,
        body: &[u8],
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let (session_root, workspace) = self
            .authority
            .record(session)
            .map(|record| (record.bound_root, record.workspace_id))
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        let (head, _) = self.head_binding_mixed()?;
        if session_root != head.state_root().root {
            return Err(session_failure(SessionError::new(
                SessionErrorCode::RootAdvanced,
            )));
        }
        match method {
            Method::TestsSelected => self.tests_selected(session, workspace, &head, body),
            Method::TestsAffected => self.tests_affected(session, workspace, &head, body),
            Method::TestsReportRead => self.tests_report_read(session, body),
            Method::TestsReplay => self.tests_replay(session, workspace, &head, body),
            Method::TestsAttemptStatus => self.tests_attempt_status(session, workspace, body),
            // Every live native method routes above; reaching here with any
            // other tag is an internal invariant breach.
            _ => protocol_failure(ProtocolErrorCode::InternalInvariant),
        }
    }

    /// 601 `tests.selected`: runs the caller selection plus the owner-added
    /// required tests over the session root state without committing
    /// anything, stores the diagnostic report, and mints its 605 token.
    fn tests_selected(
        &mut self,
        session: SessionId,
        workspace: sley_id::WorkspaceId,
        head: &HeadRevision,
        body: &[u8],
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 4)?;
        let root = StateRoot::from_bytes(fixed32(fields[0])?);
        let selected = parse_id_set(fields[1])?;
        let profile = NativeExecutionProfileId::from_bytes(fixed32(fields[2])?);
        let attempt = fixed16(fields[3])?;
        if profile != native_execution_profile() {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        if root != head.state_root().root {
            return Err(session_failure(SessionError::new(
                SessionErrorCode::StaleHandle,
            )));
        }
        let mut bindings = Vec::with_capacity(128 + selected.len() * 32);
        bindings.extend_from_slice(&Method::TestsSelected.tag().to_be_bytes());
        bindings.extend_from_slice(workspace.as_bytes());
        bindings.extend_from_slice(root.as_bytes());
        bindings.extend_from_slice(profile.as_bytes());
        for identity in &selected {
            bindings.extend_from_slice(identity.as_bytes());
        }
        if let Some(cached) = self.replay_diagnostic(session, attempt, &bindings)? {
            return Ok(cached);
        }
        let executor = self
            .executor
            .as_ref()
            .ok_or_else(|| owner("NATIVE_EXECUTOR_UNAVAILABLE", 0))?;
        let plan = native_test_plan_explicit_root(&NativeExplicitRootInputs {
            head_transaction_id: head.transaction_id(),
            state: head.state_root(),
            objects: head.objects(),
            policy: head.policy_root(),
            caller_selected: &selected,
            limits: CandidateValidationLimits::full_v1(),
            implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
            aggregate: NativeAggregateLimits::HARD_MAXIMA,
        })
        .map_err(native_plan_failure)?;
        let (object_bytes, test_cases) = diagnostic_test_cases(head);
        let executions = executor
            .execute_diagnostic(&plan, &object_bytes)
            .map_err(|error| owner(error.symbol(), 0))?;
        let assembly = assemble_diagnostic_report(
            &plan,
            head.state_root().record.schema_epoch_id,
            workspace,
            &test_cases,
            &executions,
        )
        .map_err(|error| owner(error.symbol(), error.numeric()))?;
        self.store_diagnostic(
            session,
            attempt,
            bindings,
            &plan,
            &assembly,
            head.state_root().root,
        )
    }

    /// 602 `tests.affected`: validates the candidate against the session
    /// parent preserving the exact static failure, derives the ordinary
    /// candidate-affected plan, and runs it diagnostically like 601.
    fn tests_affected(
        &mut self,
        session: SessionId,
        workspace: sley_id::WorkspaceId,
        head: &HeadRevision,
        body: &[u8],
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 3)?;
        let profile = NativeExecutionProfileId::from_bytes(fixed32(fields[1])?);
        let attempt = fixed16(fields[2])?;
        if profile != native_execution_profile() {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        let candidate = import_candidate(fields[0]).map_err(|error| owner(error.code(), 0))?;
        let base_id = head.transaction_id();
        let mut bindings = Vec::with_capacity(160);
        bindings.extend_from_slice(&Method::TestsAffected.tag().to_be_bytes());
        bindings.extend_from_slice(workspace.as_bytes());
        bindings.extend_from_slice(base_id.as_bytes());
        bindings.extend_from_slice(candidate.candidate_id.as_bytes());
        bindings.extend_from_slice(profile.as_bytes());
        if let Some(cached) = self.replay_diagnostic(session, attempt, &bindings)? {
            return Ok(cached);
        }
        let executor = self
            .executor
            .as_ref()
            .ok_or_else(|| owner("NATIVE_EXECUTOR_UNAVAILABLE", 0))?;
        let context = CandidateValidationContext::new(
            base_id,
            head.state_root(),
            head.objects(),
            head.tombstoned_entities(),
            head.policy_root(),
            candidate.record.principal_id,
            &[],
            (self.now_millis)(),
            CandidateValidationLimits::full_v1(),
        )
        .map_err(|error| owner(&error.to_string(), 0))?;
        let output = validate_candidate_bytes(&context, fields[0])
            .map_err(|error| owner(&error.to_string(), 0))?;
        if !output.is_valid() {
            return Err(output.result().record.diagnostics.first().map_or_else(
                || owner("NATIVE_TEST_SELECTION_INVALID", 0),
                |diagnostic| {
                    owner(
                        &diagnostic.source_symbol,
                        diagnostic.source_numeric_code.unwrap_or(0),
                    )
                },
            ));
        }
        let validated = output
            .validated_plan()
            .ok_or_else(|| owner("NATIVE_TEST_SELECTION_INVALID", 0))?;
        let plan = native_test_plan(
            &output,
            &NativePlanInputs {
                base_transaction_id: base_id,
                base_state: head.state_root(),
                base_objects: head.objects(),
                policy: head.policy_root(),
                capability_summary: context.capability_summary_digest(),
                limits: CandidateValidationLimits::full_v1(),
                implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
                aggregate: NativeAggregateLimits::HARD_MAXIMA,
            },
        )
        .map_err(native_plan_failure)?;
        let executions = executor
            .execute(&plan, validated)
            .map_err(|error| owner(error.symbol(), 0))?;
        let proposed = validated.proposed_state();
        let mut test_cases = BTreeMap::new();
        for object in proposed.entities() {
            if let EntityBodyValue::TestCase(body) = &object.record().body {
                test_cases.insert(object.record().entity_id, body);
            }
        }
        let assembly = assemble_diagnostic_report(
            &plan,
            validated.candidate_root().record.schema_epoch_id,
            workspace,
            &test_cases,
            &executions,
        )
        .map_err(|error| owner(error.symbol(), error.numeric()))?;
        self.store_diagnostic(
            session,
            attempt,
            bindings,
            &plan,
            &assembly,
            validated.candidate_root().root,
        )
    }

    /// 605 `tests.report_read`: serves one page of the Stored native
    /// test report a live token names (contract appendix C, revision 4).
    ///
    /// Page length is `min(max_bytes, 65536, total - offset)`; the final
    /// page answers next `None`, any earlier page `Some(offset + length)`.
    /// A token minted for another session, an expired token, or an
    /// unknown token refuses as `NATIVE_TOKEN_INVALID`: paging never
    /// falls back to a semantic entity-handle lookup. Request shape
    /// (arity, nonzero `max_bytes` fitting `UInt32`) is checked before
    /// token lookup; the offset bound needs the bound total, so it is
    /// checked after.
    fn tests_report_read(
        &self,
        session: SessionId,
        body: &[u8],
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 3)?;
        let token = fixed32(fields[0])?;
        let offset = single_uvar(fields[1])?;
        let max_bytes = single_uvar(fields[2])?;
        if max_bytes == 0 || u32::try_from(max_bytes).is_err() {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        let now = (self.now_millis)();
        let Some(entry) = self.diagnostic_tokens.iter().find(|candidate| {
            candidate.token == token && candidate.session == session && !candidate.expired(now)
        }) else {
            return Err(owner("NATIVE_TOKEN_INVALID", 0));
        };
        let total = to_u64(entry.report.len())?;
        if offset > total {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        let length = max_bytes.min(MAX_DIAGNOSTIC_PAGE_BYTES).min(total - offset);
        let end = offset + length;
        let start = usize::try_from(offset)
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?;
        let stop = usize::try_from(end)
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?;
        let page = entry.report[start..stop].to_vec();
        let next = (end < total).then_some(end);
        let body = scb(encode_record(&[
            (1, entry.report_id.to_vec()),
            (2, entry.bound_root.to_vec()),
            (3, encode_uvar(offset)),
            (4, encode_uvar(total)),
            (5, scb(encode_bytes(&page))?),
            (6, scb(encode_option_uvar(next))?),
        ]))?;
        self.counted(body, 0)
    }

    /// 606 `tests.replay`: replays one in-scope native commit through the
    /// shared native engine and answers the Appendix C response record
    /// (contract appendix C, revision 5).
    ///
    /// The claimed execution profile and replay resource policy must equal
    /// the stored plan's own bindings or the request is malformed; ceilings
    /// are always the server-enforced stored ones. Scope precedes replay:
    /// the transaction must sit in the session head's ancestry or name a
    /// visible branch head, else `NATIVE_REPLAY_SCOPE_REFUSED`. Engine
    /// verdicts map one-to-one (matched 1, mismatch 2,
    /// inconclusive-resource 3, untrusted-history 4); engine host errors
    /// refuse with their preserved symbols. Only a match mints a fresh
    /// 605 token over the verified original report bytes; every other
    /// status carries no token, and `replay_report_id` stays `None`
    /// locally. `attempt_id` binds the server-side replay cache with the
    /// diagnostic conflict rule.
    #[allow(clippy::too_many_lines)]
    fn tests_replay(
        &mut self,
        session: SessionId,
        workspace: sley_id::WorkspaceId,
        head: &HeadRevision,
        body: &[u8],
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 5)?;
        let transaction_id = TransactionId::from_bytes(fixed32(fields[0])?);
        let expected_root = StateRoot::from_bytes(fixed32(fields[1])?);
        let profile = NativeExecutionProfileId::from_bytes(fixed32(fields[2])?);
        let policy = sley_id::NativeResourcePolicyId::from_bytes(fixed32(fields[3])?);
        let attempt = fixed16(fields[4])?;
        if profile != native_execution_profile() {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        let mut bindings = Vec::with_capacity(192);
        bindings.extend_from_slice(&Method::TestsReplay.tag().to_be_bytes());
        bindings.extend_from_slice(workspace.as_bytes());
        bindings.extend_from_slice(transaction_id.as_bytes());
        bindings.extend_from_slice(expected_root.as_bytes());
        bindings.extend_from_slice(profile.as_bytes());
        bindings.extend_from_slice(policy.as_bytes());
        if let Some(cached) = self.replay_cached(session, attempt, &bindings)? {
            return Ok(cached);
        }
        self.require_replay_scope(head, transaction_id)?;
        // The stored plan binds the profile and policy claims: the caller
        // names them, the server enforces the stored values. A stored plan
        // that no longer parses is malformed history, so the claim checks
        // step aside and the engine errors honestly below.
        let stored = self
            .transactions()
            .verified_native_revision(transaction_id)
            .ok();
        let (compared, report_stored) = match stored.as_ref() {
            Some(revision) => {
                match sley_tests::NativeTestPlanV1::parse(revision.receipt().bundle.plan_stored()) {
                    Ok(plan) => {
                        if plan.execution_profile() != profile
                            || plan.resource_policy().policy_id() != policy
                        {
                            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
                        }
                        (
                            to_u64(plan.selected().len())?,
                            Some(revision.receipt().bundle.test_report_stored().to_vec()),
                        )
                    }
                    Err(_) => (0, None),
                }
            }
            None => (0, None),
        };
        let epoch = state_epoch_id()
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        let verifier =
            move |bytes: &[u8]| import_entity_object(epoch, bytes).map(|object| object.object_id());
        let authority = self.native_authority.as_ref();
        let request = NativeReplayRequest {
            transaction_id,
            expected_root,
            executor: authority
                .map(|authority| &*authority.executor as &dyn sley_txn::NativeTestExecutor),
            trust: NativeExchangeTrust {
                manifests: authority.map_or(&[], |authority| &authority.trust_manifests[..]),
            },
        };
        let report = replay_native_commit(&self.repository, &request, &verifier)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        let status = replay_status_number(report.status);
        // Only a match mints a token: the replay confirmed the history,
        // so the verified original report pages like accepted evidence.
        // Any other status carries no token.
        let token = if report.status == NativeReplayStatus::Matched {
            let stored: Vec<u8> = report_stored
                .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
            let parsed = NativeTestReportV1::parse(&stored)
                .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
            Some(self.cache_verified_report(
                session,
                *parsed.report_id().as_bytes(),
                *report.root.as_bytes(),
                stored,
            )?)
        } else {
            None
        };
        let body = scb(encode_record(&[
            (1, transaction_id.as_bytes().to_vec()),
            (2, report.root.as_bytes().to_vec()),
            (3, encode_uvar(status)),
            (4, report.original_report_id.as_bytes().to_vec()),
            (5, encode_option_id(None)),
            (6, encode_option_id(token)),
        ]))?;
        self.store_replay(session, attempt, bindings, body, compared, token)
    }

    /// Replays a cached replay answer without re-running the engine.
    ///
    /// Identical bindings return the cached response bytes; any binding
    /// divergence refuses as an attempt conflict. A cached answer whose
    /// token is gone (invalidated, expired, or never minted for a
    /// non-match) is not replayed: the caller re-executes fresh.
    fn replay_cached(
        &self,
        session: SessionId,
        attempt: [u8; 16],
        bindings: &[u8],
    ) -> Result<Option<(Vec<u8>, BoundedContext)>> {
        let Some(cached) = self.replay_attempts.get(&attempt) else {
            return Ok(None);
        };
        if cached.session != session || cached.bindings != bindings {
            return Err(owner("NATIVE_ATTEMPT_CONFLICT", 0));
        }
        let now = (self.now_millis)();
        let live = cached.token.is_some_and(|token| {
            self.diagnostic_tokens.iter().any(|entry| {
                entry.token == token && entry.session == session && !entry.expired(now)
            })
        });
        if !live {
            return Ok(None);
        }
        Ok(Some(
            self.counted(cached.response.clone(), cached.compared_count)?,
        ))
    }

    /// Stores a completed replay answer, evicting the oldest entry past
    /// its bound, and answers the Appendix C response record.
    fn store_replay(
        &mut self,
        session: SessionId,
        attempt: [u8; 16],
        bindings: Vec<u8>,
        body: Vec<u8>,
        compared: u64,
        token: Option<[u8; 32]>,
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let now = (self.now_millis)();
        if self.replay_attempts.len() >= MAX_REPLAY_ATTEMPTS {
            let oldest = self
                .replay_attempts
                .iter()
                .min_by_key(|(_, attempt)| attempt.created_millis)
                .map(|(identity, _)| *identity);
            if let Some(identity) = oldest {
                self.replay_attempts.remove(&identity);
            }
        }
        self.replay_attempts.insert(
            attempt,
            ReplayAttempt {
                session,
                bindings,
                response: body.clone(),
                compared_count: compared,
                token,
                created_millis: now,
            },
        );
        self.counted(body, compared)
    }

    /// 607 `tests.attempt_status`: resolves one journaled attempt against
    /// journal, receipt, and head bytes and answers the Appendix C
    /// response record (contract appendix C, revision 5).
    ///
    /// No journal record means `UnknownAttempt0` with every optional field
    /// absent: diagnostic and replay attempts are never journaled, so they
    /// always answer unknown here, and unknown never implies retry-safe. A
    /// record bound to another workspace, or to another candidate when the
    /// request names one, refuses as `NATIVE_ATTEMPT_CONFLICT` before any
    /// status work. States reuse the §6 journal tags; committed answers
    /// both identities plus a fresh 605 token minted over the verified
    /// accepted report bytes when they load (identities without a token
    /// when they do not), every other known state answers neither
    /// identities nor token. Binding is by workspace rather than session
    /// identity, so a renewed session keeps answering for its workspace's
    /// attempts.
    fn tests_attempt_status(
        &mut self,
        session: SessionId,
        workspace: sley_id::WorkspaceId,
        body: &[u8],
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 2)?;
        let attempt = NativeAttemptId(fixed16(fields[0])?);
        let candidate = option_id(fields[1])?.map(CandidateId::from_bytes);
        let commit_failure =
            |error: sley_txn::CommitError| owner(error.code(), error.numeric_code().unwrap_or(0));
        let scope = self
            .transactions()
            .native_attempt_scope(attempt)
            .map_err(commit_failure)?;
        let Some(scope) = scope else {
            return self.unknown_attempt();
        };
        if scope.workspace != workspace
            || candidate.is_some_and(|claimed| claimed != scope.candidate_id)
        {
            return Err(owner("NATIVE_ATTEMPT_CONFLICT", 0));
        }
        let status = self
            .transactions()
            .native_attempt_status(attempt)
            .map_err(commit_failure)?;
        let state = attempt_state_number(&status);
        let (transaction_id, receipt_id, token) = match status {
            AttemptStatus::Committed {
                transaction_id,
                receipt_id,
                ..
            } => (
                Some(*transaction_id.as_bytes()),
                Some(*receipt_id.as_bytes()),
                self.committed_report_token(session, transaction_id)?,
            ),
            _ => (None, None, None),
        };
        let body = scb(encode_record(&[
            (1, encode_uvar(state)),
            (2, encode_option_id(transaction_id)),
            (3, encode_option_id(receipt_id)),
            (4, encode_option_id(token)),
        ]))?;
        self.counted(body, 0)
    }

    /// Answers `UnknownAttempt0` with every optional field absent.
    fn unknown_attempt(&self) -> Result<(Vec<u8>, BoundedContext)> {
        let body = scb(encode_record(&[
            (1, encode_uvar(0)),
            (2, encode_option_id(None)),
            (3, encode_option_id(None)),
            (4, encode_option_id(None)),
        ]))?;
        self.counted(body, 0)
    }

    /// Mints a fresh 605 token over one committed transaction's verified
    /// test report bytes, or carries no token when the bytes do not load.
    ///
    /// The receipt was already reconciled against history by the status
    /// owner; this reloads it read-only and caches its test report under
    /// the session caps. Caps failures refuse like any mint; unloadable
    /// bytes degrade to identities without a token, never a status
    /// refusal.
    fn committed_report_token(
        &mut self,
        session: SessionId,
        transaction_id: TransactionId,
    ) -> Result<Option<[u8; 32]>> {
        let revision = self
            .transactions()
            .verified_native_revision(transaction_id)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        let stored = revision.receipt().bundle.test_report_stored();
        let Ok(report) = NativeTestReportV1::parse(stored) else {
            return Ok(None);
        };
        Ok(Some(
            self.cache_verified_report(
                session,
                *report.report_id().as_bytes(),
                *revision
                    .receipt()
                    .transaction
                    .record
                    .committed_root
                    .as_bytes(),
                stored.to_vec(),
            )?,
        ))
    }

    /// Requires the named transaction in the session's authorized scope:
    /// an ancestor of the session-bound head or the head of a visible
    /// branch ref (contract appendix C, revision 5). Anything else refuses
    /// as `NATIVE_REPLAY_SCOPE_REFUSED` before any replay work. The walk
    /// covers mixed v1/native ancestry through the transaction owner's
    /// any-format loader; a corrupt chain refuses with its preserved
    /// failure, never as out-of-scope.
    fn require_replay_scope(
        &self,
        head: &HeadRevision,
        transaction_id: TransactionId,
    ) -> Result<()> {
        if self.history_contains(head.transaction_id(), transaction_id)? {
            return Ok(());
        }
        let limit = usize::try_from(MAX_BRANCH_LIST).unwrap_or(usize::MAX);
        let branches = self
            .branches()
            .list_branches(limit)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        if branches
            .iter()
            .any(|branch| branch.revision.transaction_id() == transaction_id)
        {
            return Ok(());
        }
        Err(owner("NATIVE_REPLAY_SCOPE_REFUSED", 0))
    }

    /// Returns whether the target sits in the head-first ancestry of one
    /// transaction over mixed-format history: cycle-checked, bounded, and
    /// verified per node through the any-format loader.
    fn history_contains(&self, head: TransactionId, target: TransactionId) -> Result<bool> {
        let maintenance = self.maintenance()?;
        let transactions = self.transactions();
        let commit_failure =
            |error: sley_txn::CommitError| owner(error.code(), error.numeric_code().unwrap_or(0));
        let mut seen = BTreeSet::new();
        let mut stack = vec![head];
        while let Some(transaction_id) = stack.pop() {
            if transaction_id == target {
                return Ok(true);
            }
            if !seen.insert(transaction_id) {
                continue;
            }
            if seen.len() > MAX_ANCESTRY_NODES {
                return Err(owner("MERGE_RESOURCE_LIMIT", 52_006));
            }
            let receipt = transactions
                .imported_receipt_any_with_maintenance(&maintenance, transaction_id)
                .map_err(commit_failure)?;
            stack.extend(receipt.parent_transaction_ids().iter().copied());
        }
        Ok(false)
    }

    /// Replays a completed diagnostic attempt without re-executing.
    ///
    /// Identical bindings return the cached response bytes; any binding
    /// divergence refuses as an attempt conflict. A completed attempt
    /// whose token is gone (invalidated, expired) is not replayed: the
    /// caller re-executes fresh instead of receiving a dead capability.
    fn replay_diagnostic(
        &self,
        session: SessionId,
        attempt: [u8; 16],
        bindings: &[u8],
    ) -> Result<Option<(Vec<u8>, BoundedContext)>> {
        let Some(cached) = self.diagnostic_attempts.get(&attempt) else {
            return Ok(None);
        };
        if cached.session != session || cached.bindings != bindings {
            return Err(owner("NATIVE_ATTEMPT_CONFLICT", 0));
        }
        let now = (self.now_millis)();
        let live = self.diagnostic_tokens.iter().any(|token| {
            token.token == cached.token && token.session == session && !token.expired(now)
        });
        if !live {
            return Ok(None);
        }
        Ok(Some(
            self.counted(cached.response.clone(), cached.selected_count)?,
        ))
    }

    /// Stores a completed diagnostic report, mints its 605 token, and
    /// answers the Appendix C response record.
    ///
    /// The attempt cache evicts its oldest entry past its bound, and a
    /// re-submitted attempt then simply re-executes; token caps live in
    /// the shared mint below.
    fn store_diagnostic(
        &mut self,
        session: SessionId,
        attempt: [u8; 16],
        bindings: Vec<u8>,
        plan: &NativeTestPlanV1,
        assembly: &NativeDiagnosticAssembly,
        bound_root: StateRoot,
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let report_bytes = assembly.report.stored_bytes().to_vec();
        let total = to_u64(report_bytes.len())?;
        let count = to_u64(plan.selected().len())?;
        let now = (self.now_millis)();
        let token = self.cache_verified_report(
            session,
            *assembly.report.report_id().as_bytes(),
            *bound_root.as_bytes(),
            report_bytes,
        )?;
        let body = scb(encode_record(&[
            (1, plan.plan_id().as_bytes().to_vec()),
            (2, assembly.report.report_id().as_bytes().to_vec()),
            (3, encode_uvar(u64::from(assembly.status.tag()))),
            (4, encode_uvar(count)),
            (5, token.to_vec()),
            (6, encode_uvar(total)),
        ]))?;
        if self.diagnostic_attempts.len() >= MAX_DIAGNOSTIC_ATTEMPTS {
            let oldest = self
                .diagnostic_attempts
                .iter()
                .min_by_key(|(_, attempt)| attempt.created_millis)
                .map(|(identity, _)| *identity);
            if let Some(identity) = oldest {
                self.diagnostic_attempts.remove(&identity);
            }
        }
        self.diagnostic_attempts.insert(
            attempt,
            DiagnosticAttempt {
                session,
                bindings,
                response: body.clone(),
                selected_count: count,
                token,
                created_millis: now,
            },
        );
        self.counted(body, count)
    }

    /// Mints one 605 token over verified report bytes for this session.
    ///
    /// Diagnostics, replay matches, and committed attempt answers share
    /// this capability: every token binds session, report identity, and
    /// root, and the per-session token count and cached-evidence bytes are
    /// enforced before minting. Bytes must be verified accepted evidence
    /// (a diagnostic assembly, an engine-confirmed history, or a
    /// reconciled committed receipt); the mint itself checks caps, never
    /// provenance.
    fn cache_verified_report(
        &mut self,
        session: SessionId,
        report_id: [u8; 32],
        bound_root: [u8; 32],
        report: Vec<u8>,
    ) -> Result<[u8; 32]> {
        let total = to_u64(report.len())?;
        let now = (self.now_millis)();
        self.prune_diagnostic_tokens(session, now);
        let held_count = self
            .diagnostic_tokens
            .iter()
            .filter(|token| token.session == session)
            .count();
        if held_count >= MAX_DIAGNOSTIC_TOKENS_PER_SESSION {
            return Err(owner("NATIVE_DIAGNOSTIC_TOKEN_LIMIT", 0));
        }
        let mut held_bytes = 0_u64;
        for token in &self.diagnostic_tokens {
            if token.session == session {
                held_bytes = held_bytes
                    .checked_add(token.report.len() as u64)
                    .ok_or_else(|| owner("NATIVE_DIAGNOSTIC_TOKEN_LIMIT", 0))?;
            }
        }
        if held_bytes.saturating_add(total) > MAX_DIAGNOSTIC_BYTES_PER_SESSION {
            return Err(owner("NATIVE_DIAGNOSTIC_TOKEN_LIMIT", 0));
        }
        let token = self.mint_diagnostic_token(session)?;
        self.diagnostic_tokens.push(DiagnosticToken {
            token,
            session,
            report_id,
            bound_root,
            report,
            created_millis: now,
        });
        Ok(token)
    }

    /// Mints one diagnostic report token, unpredictable to callers and
    /// deterministic for one server instance.
    fn mint_diagnostic_token(&mut self, session: SessionId) -> Result<[u8; 32]> {
        let counter = self
            .diagnostic_token_counter
            .checked_add(1)
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        self.diagnostic_token_counter = counter;
        let mut preimage =
            Vec::with_capacity(DIAGNOSTIC_TOKEN_DOMAIN.len() + 32 + session.as_bytes().len() + 8);
        preimage.extend_from_slice(DIAGNOSTIC_TOKEN_DOMAIN);
        preimage.extend_from_slice(self.authority.server_nonce());
        preimage.extend_from_slice(session.as_bytes());
        preimage.extend_from_slice(&counter.to_be_bytes());
        Ok(*blake3::hash(&preimage).as_bytes())
    }

    /// Drops one session's expired diagnostic tokens.
    fn prune_diagnostic_tokens(&mut self, session: SessionId, now: u64) {
        self.diagnostic_tokens
            .retain(|token| token.session != session || !token.expired(now));
    }

    /// Invalidates one session's diagnostic tokens and the attempts bound
    /// to them. Renewal, close, checkout, commit, merge-commit, and
    /// exchange import all rebind or advance the head the tokens were
    /// minted under, so nothing they name survives.
    fn invalidate_diagnostic_session(&mut self, session: SessionId) {
        self.diagnostic_tokens
            .retain(|token| token.session != session);
        self.diagnostic_attempts
            .retain(|_, attempt| attempt.session != session);
    }

    /// Invalidates every diagnostic token and attempt. Head-advancing
    /// calls (commit, merge-commit, exchange import) move the binding for
    /// all sessions at once, so per-session pruning cannot suffice.
    fn invalidate_all_diagnostics(&mut self) {
        self.diagnostic_tokens.clear();
        self.diagnostic_attempts.clear();
    }

    /// Head-bound methods answer over the accepted head without naming it
    /// (contract section 3). The set is closed: a method is head-bound
    /// exactly when it answers over current repository state without
    /// naming the state it answers over. Methods naming an explicit
    /// `TransactionId`, receipt, candidate bytes, or merge inputs answer
    /// over caller-named state; methods that mutate advance the head and
    /// leave rebinding to explicit renewal. `handle.expand` performs the
    /// same root check itself and reports `SESSION_STALE_HANDLE`.
    const fn head_bound(method: Method) -> bool {
        matches!(
            method,
            Method::WorkspaceOpen
                | Method::QueryRoot
                | Method::QueryContinue
                | Method::Capsule
                | Method::QueryRestricted
                | Method::Execute
                | Method::RefsList
                | Method::RefsResolve
                | Method::RefsRecover
                | Method::Recovery
                | Method::ExchangeExport
                | Method::GcDryRun
                | Method::Report
        )
    }

    /// Version-aware head-bound partition: the legacy set plus the two
    /// entity-read methods, head-bound only in protocol version 2
    /// (contract `docs/spec/ENTITY_READ_PROFILE_V2.md` section 6). The
    /// legacy partition is unchanged.
    const fn head_bound_versioned(method: Method) -> bool {
        Self::head_bound(method)
            || matches!(method, Method::EntityVersion | Method::EntitySignature)
    }

    /// Admits a session and retains the owned verified revision whose
    /// binding was checked, with no second head load. The legacy
    /// unit-returning helper is unchanged.
    fn session_check_retained(
        &self,
        session: SessionId,
        method: Method,
    ) -> Result<VerifiedRevision> {
        let (revision, binding) = self.head_binding()?;
        let head_bound = if self.version_aware {
            Self::head_bound_versioned(method)
        } else {
            Self::head_bound(method)
        };
        self.authority
            .check_session(session, &binding, head_bound)
            .map_err(session_failure)?;
        Ok(revision)
    }

    /// Serves an entity read on the explicit path with the ordered debit
    /// table (contract `docs/spec/ENTITY_READ_PROFILE_V2.md` section 5):
    /// admission failures precede the dispatch charge; every
    /// pre-reservation failure costs exactly the dispatch unit; success
    /// costs the full prepared work with no generic body-byte charge and
    /// no refund after reservation.
    fn dispatch_entity_read(
        &mut self,
        session: SessionId,
        method: Method,
        frame: &ProtocolFrame,
    ) -> Result<(Vec<u8>, BoundedContext)> {
        // Inherited order: session binding, then budget exhaustion, then
        // method negotiation. Binding failures keep their no-debit
        // semantics but must still release the admitted slot, exactly as
        // the generic path does.
        let revision = match self.session_check_retained(session, method) {
            Ok(revision) => revision,
            Err(failure) => {
                if self.registry.is_open(session) {
                    self.registry
                        .complete(session)
                        .map_err(|error| ProtocolFailure::protocol(error.code()))?;
                }
                return Err(failure);
            }
        };
        if self.budgets.get(&session).copied().unwrap_or(0) == 0 {
            self.registry
                .complete(session)
                .map_err(|error| ProtocolFailure::protocol(error.code()))?;
            return protocol_failure(ProtocolErrorCode::LimitExceeded);
        }
        let budget_before_dispatch = self.budgets.get(&session).copied().unwrap_or(0);
        if let Some(remaining) = self.budgets.get_mut(&session) {
            *remaining = remaining.saturating_sub(1);
        }
        // Method negotiation runs inside the outcome path whose cleanup
        // cannot be skipped: a live funded request for an unoffered method
        // still costs the one dispatch unit charged above.
        let outcome = if self.profile.admits(method) {
            self.entity_read(session, method, frame, budget_before_dispatch, &revision)
        } else {
            Err(unsupported(b"SMP1-METHOD-NOT-NEGOTIATED"))
        };
        if self.registry.is_open(session) {
            self.registry
                .complete(session)
                .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        }
        outcome
    }

    /// Prepares, preflights, reserves, captures, and encodes one entity read
    /// over the retained revision. Preparation runs only through the trusted
    /// repository adapter and allocates no object-sized output; the full
    /// frame size is established next, then the work reservation, and only
    /// then are the selected bytes captured and encoded.
    fn entity_read(
        &mut self,
        session: SessionId,
        method: Method,
        frame: &ProtocolFrame,
        budget_before_dispatch: u64,
        revision: &VerifiedRevision,
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let read = match method {
            Method::EntityVersion => EntityReadMethod::Version,
            Method::EntitySignature => EntityReadMethod::Signature,
            _ => return protocol_failure(ProtocolErrorCode::InternalInvariant),
        };
        let selected = EntityReadCeilings {
            max_entities: self.profile.limits.max_entities,
            max_response_bytes: self.profile.limits.max_response_bytes,
            max_work: self.profile.limits.max_work,
            budget_before_dispatch,
        };
        let (_, selection) = prepare_verified_entity_read(revision, read, &frame.body, &selected)
            .map_err(entity_read_failure)?;
        let bounds = BoundedContext {
            applied_limits: self.profile.limits,
            returned_bytes: selection.body_len(),
            returned_entities: selection.object_count(),
            ..BoundedContext::none()
        };
        let frame_len = crate::frame_total_len(&crate::FrameSize {
            version: self.response_version(),
            session: frame.session,
            request_id: frame.request_id,
            kind_tag: FrameKind::Response.tag(),
            method: frame.method,
            flags: 0,
            bounds,
            body_len: selection.body_len(),
        })
        .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        // The outgoing fit compares the complete wire bytes INCLUDING the
        // 8-byte length prefix against the negotiated ceiling, while the
        // prefix value stays the envelope length. This preflight runs
        // before any work reservation or output allocation: preparation
        // above copies no object bytes.
        let wire_len = frame_len
            .checked_add(8)
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::LimitExceeded))?;
        if wire_len > self.profile.limits.max_frame_bytes {
            return protocol_failure(ProtocolErrorCode::LimitExceeded);
        }
        let reserve = selection
            .work_units()
            .checked_sub(1)
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        let remaining = self
            .budgets
            .get_mut(&session)
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        *remaining = remaining
            .checked_sub(reserve)
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        // Capture runs only after the frame preflight and the reservation,
        // so no object-sized output allocation precedes either gate. A
        // capture failure here is an unexpected post-reservation failure:
        // the complete debit is retained and no object bytes are returned.
        let plan = sley_repo::capture_entity_read_selection(selection)
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        #[cfg(test)]
        if self.entity_encode_fault {
            return protocol_failure(ProtocolErrorCode::InternalInvariant);
        }
        let outcome = encode_verified_entity_read_response(plan, session)
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        Ok((outcome.body, bounds))
    }

    fn head_binding(&self) -> Result<(VerifiedRevision, HeadBinding)> {
        let head = self.head()?;
        let record = &head.state_root().record;
        let binding = HeadBinding {
            workspace_id: record.workspace_id,
            root: head.state_root().root,
            schema_epoch: record.schema_epoch_id,
        };
        Ok((head, binding))
    }

    /// Loads the accepted head of either receipt format with its session
    /// binding. The session layer and the native surfaces work over both
    /// formats; legacy data paths keep the v1-only loader above and fail
    /// loudly on native heads.
    fn head_binding_mixed(&self) -> Result<(HeadRevision, HeadBinding)> {
        let head = self.head_mixed()?;
        let record = &head.state_root().record;
        let binding = HeadBinding {
            workspace_id: record.workspace_id,
            root: head.state_root().root,
            schema_epoch: record.schema_epoch_id,
        };
        Ok((head, binding))
    }

    /// Loads the accepted head of either receipt format, dispatching on
    /// the stored bytes through the transaction owner's any-format
    /// loader. Format-1 heads verify through the frozen v1 loader;
    /// native heads verify through the native loader with its evidence,
    /// relationship, object, inventory, and pin checks.
    fn head_mixed(&self) -> Result<HeadRevision> {
        let maintenance = self.maintenance()?;
        let transactions = self.transactions();
        let receipt = transactions
            .accepted_head_any_with_maintenance(&maintenance)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        match receipt {
            ImportedReceipt::V1(_) => Ok(HeadRevision::V1(Box::new(self.head()?))),
            ImportedReceipt::V2(native) => {
                let revision = transactions
                    .verified_native_revision(native.transaction.transaction_id)
                    .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
                Ok(HeadRevision::Native(Box::new(revision)))
            }
        }
    }

    fn session_check(&self, session: SessionId, method: Method) -> Result<()> {
        let (_, binding) = self.head_binding_mixed()?;
        self.authority
            .check_session(session, &binding, Self::head_bound(method))
            .map(|_| ())
            .map_err(session_failure)
    }

    fn plain(&self, body: Vec<u8>) -> Result<(Vec<u8>, BoundedContext)> {
        let bounds = BoundedContext {
            applied_limits: self.profile.limits,
            returned_bytes: to_u64(body.len())?,
            ..BoundedContext::none()
        };
        Ok((body, bounds))
    }

    fn session_open(&mut self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let claimed = fixed32(body)?;
        if claimed != *self.handshake_id.as_bytes() {
            return protocol_failure(ProtocolErrorCode::Downgrade);
        }
        let (head, binding) = self.head_binding_mixed()?;
        let record = self
            .authority
            .open_session(
                &binding,
                head.state_root(),
                self.profile.limits.max_sessions,
            )
            .map_err(session_failure)?;
        let session = record.session_id;
        self.registry
            .open(session)
            .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        self.budgets.insert(session, self.profile.limits.max_work);
        self.plain(session.as_bytes().to_vec())
    }

    fn transactions(&self) -> TransactionRepository {
        TransactionRepository::new(&self.repository)
    }

    /// Shared maintenance over this server's repository, held for one
    /// cache-touching call: the guard pins the lock boundary against a
    /// concurrent import purge or exclusive owner while the index cache
    /// is read and written.
    fn maintenance(&self) -> core::result::Result<RepositoryMaintenanceGuard, ProtocolFailure> {
        initialize_repository_maintenance(&self.repository)
            .and_then(|()| acquire_shared_repository_maintenance(&self.repository))
            .map_err(|_| owner("TXN_IO", 39_019))
    }

    fn branches(&self) -> BranchRepository {
        BranchRepository::new(&self.repository)
    }

    fn head(&self) -> Result<VerifiedRevision> {
        #[cfg(test)]
        self.head_loads.set(self.head_loads.get().saturating_add(1));
        self.transactions()
            .accepted_head()
            .map(sley_txn::AcceptedHead::into_verified_revision)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))
    }

    fn revision(&self, id: TransactionId) -> Result<VerifiedRevision> {
        self.transactions()
            .verified_revision(id)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))
    }

    fn refs_list(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let limit = single_uvar(body)?;
        if limit == 0 || limit > MAX_BRANCH_LIST {
            return protocol_failure(ProtocolErrorCode::LimitExceeded);
        }
        let branches = self
            .branches()
            .list_branches(usize::try_from(limit).unwrap_or(usize::MAX))
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        let summaries: Vec<Vec<u8>> = branches.iter().map(branch_summary).collect::<Result<_>>()?;
        let count = to_u64(summaries.len())?;
        let body = scb(encode_list(&summaries))?;
        let bounds = BoundedContext {
            applied_limits: self.profile.limits,
            returned_bytes: to_u64(body.len())?,
            returned_entities: count,
            ..BoundedContext::none()
        };
        Ok((body, bounds))
    }

    fn refs_resolve(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let name = BranchName::parse(body)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        let resolved = self
            .branches()
            .resolve_branch(name.as_bytes())
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        let summary = branch_summary(&resolved)?;
        self.counted(summary, 1)
    }

    fn revision_read(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let id = TransactionId::from_bytes(fixed32(body)?);
        let revision = self.revision(id)?;
        let summary = revision_summary(&revision)?;
        self.counted(summary, 1)
    }

    fn branch_create(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 2)?;
        let origin = TransactionId::from_bytes(fixed32(fields[1])?);
        let status = self
            .branches()
            .create_branch(fields[0], origin)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        self.counted(encode_uvar(u64::from(status_tag(status))), 1)
    }

    fn branch_advance(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 3)?;
        let expected = TransactionId::from_bytes(fixed32(fields[1])?);
        let new_head = TransactionId::from_bytes(fixed32(fields[2])?);
        let status = self
            .branches()
            .advance_branch(fields[0], expected, new_head)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        self.counted(encode_uvar(u64::from(status_tag(status))), 1)
    }

    fn compare(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 2)?;
        let base = self.revision(TransactionId::from_bytes(fixed32(fields[0])?))?;
        let target = self.revision(TransactionId::from_bytes(fixed32(fields[1])?))?;
        let base_request = CompleteRootRequest::extract(&base)
            .map_err(|error| owner(error.code(), error.numeric()))?;
        let target_request = CompleteRootRequest::extract(&target)
            .map_err(|error| owner(error.code(), error.numeric()))?;
        let delta = compare_complete_roots(&base_request, &target_request)
            .map_err(|error| owner(error.code().as_str(), error.code().numeric()))?;
        let entities = to_u64(delta.delta.entities.len())?;
        let edges = to_u64(delta.delta.relations.len())?;
        let body = delta.stored_bytes;
        let bounds = BoundedContext {
            applied_limits: self.profile.limits,
            returned_bytes: to_u64(body.len())?,
            returned_entities: entities,
            returned_edges: edges,
            ..BoundedContext::none()
        };
        Ok((body, bounds))
    }

    /// Judges a merge with the ancestor precondition proven server-side: the
    /// ancestries are walked from the supplied revisions (never caller
    /// chains), so a caller-chosen `O` that is not the exact common ancestor
    /// fails `MERGE_ANCESTOR_MISMATCH` before any composition.
    fn verified_merge(
        &self,
        ancestor: &MergeSide,
        ours: &MergeSide,
        theirs: &MergeSide,
    ) -> Result<MergeOutcome> {
        let failure = |error: sley_repo::MergeError| {
            owner(
                &error.symbol(),
                error.code().map_or_else(
                    || error.commit_numeric().unwrap_or(0),
                    sley_repo::MergeErrorCode::numeric,
                ),
            )
        };
        let (Some(ours_id), Some(theirs_id)) = (ours.transaction_id, theirs.transaction_id) else {
            return Err(failure(sley_repo::MergeError::Merge(
                sley_repo::MergeErrorCode::PlanUnsupported,
            )));
        };
        let transactions = self.transactions();
        let ours_ancestry =
            transaction_ancestry(&transactions, ours_id, sley_repo::MAX_ANCESTRY_NODES)
                .map_err(failure)?;
        let theirs_ancestry =
            transaction_ancestry(&transactions, theirs_id, sley_repo::MAX_ANCESTRY_NODES)
                .map_err(failure)?;
        judge_merge_verified(ancestor, ours, theirs, &ours_ancestry, &theirs_ancestry)
            .map_err(failure)
    }

    fn merge_judge(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 3)?;
        let ancestor = MergeSide::from_revision(
            &self.revision(TransactionId::from_bytes(fixed32(fields[0])?))?,
        );
        let ours = MergeSide::from_revision(
            &self.revision(TransactionId::from_bytes(fixed32(fields[1])?))?,
        );
        let theirs = MergeSide::from_revision(
            &self.revision(TransactionId::from_bytes(fixed32(fields[2])?))?,
        );
        let outcome = self.verified_merge(&ancestor, &ours, &theirs)?;
        let (payload, count) = match outcome {
            MergeOutcome::Merged(merged) => {
                let inner = scb(encode_record(&[
                    (1, merged.state_root.root.as_bytes().to_vec()),
                    (2, encode_uvar(to_u64(merged.objects.len())?)),
                    (3, scb(encode_bytes(&merged.state_root.stored_bytes))?),
                ]))?;
                (scb(encode_union(1, &inner))?, to_u64(merged.objects.len())?)
            }
            MergeOutcome::Conflict(conflict) => {
                let count = to_u64(conflict.conflict.conflicts.len())?;
                (scb(encode_union(2, &conflict.stored_bytes))?, count)
            }
        };
        self.counted(payload, count)
    }

    fn exchange_export(&self) -> Result<(Vec<u8>, BoundedContext)> {
        let epoch = state_epoch_id()
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        let verifier =
            move |bytes: &[u8]| import_entity_object(epoch, bytes).map(|object| object.object_id());
        let exchange = export_repository_exchange(&self.repository, &verifier)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        self.counted(exchange.stored_bytes, 1)
    }

    fn refs_recover(&self) -> Result<(Vec<u8>, BoundedContext)> {
        let report = self
            .branches()
            .recover_refs()
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        let body = scb(encode_record(&[
            (1, encode_uvar(report.removed_branch_stages)),
            (2, encode_uvar(report.removed_ref_stages)),
            (3, encode_uvar(report.visible_branches)),
        ]))?;
        self.counted(body, report.visible_branches)
    }

    fn receipt_read(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let revision = self.revision(TransactionId::from_bytes(fixed32(body)?))?;
        self.counted(revision.receipt().stored_bytes.clone(), 1)
    }

    fn checkout(&mut self, session: SessionId, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let revision = self.revision(TransactionId::from_bytes(fixed32(body)?))?;
        let objects: Vec<Vec<u8>> = revision
            .objects()
            .iter()
            .map(|object| encode_bytes(object.stored_bytes()))
            .collect::<core::result::Result<_, _>>()
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::LimitExceeded))?;
        let count = to_u64(objects.len())?;
        let payload = scb(encode_record(&[
            (1, revision.state_root().root.as_bytes().to_vec()),
            (2, scb(encode_list(&objects))?),
        ]))?;
        // Checkout refocuses the caller: its diagnostic tokens and
        // attempts die even though the head itself did not move.
        self.invalidate_diagnostic_session(session);
        self.counted(payload, count)
    }

    fn recovery(&self) -> Result<(Vec<u8>, BoundedContext)> {
        let report = self
            .transactions()
            .recover()
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        let accepted = match report.accepted_transaction_id {
            None => scb(encode_union(0, &[]))?,
            Some(id) => scb(encode_union(1, id.as_bytes()))?,
        };
        let body = scb(encode_record(&[
            (1, encode_uvar(report.removed_object_stages)),
            (2, encode_uvar(report.removed_receipt_stages)),
            (3, encode_uvar(report.removed_head_stages)),
            (4, accepted),
            (5, encode_uvar(report.verified_ancestry_transactions)),
        ]))?;
        self.counted(body, report.verified_ancestry_transactions)
    }

    fn query_root(&self, body: &[u8], continuation: bool) -> Result<(Vec<u8>, BoundedContext)> {
        let decoded = decode_root_query(body)?;
        if continuation && decoded.after.is_none() {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        let revision = self.head()?;
        let guard = self.maintenance()?;
        let outcome = run_root_query(
            &self.repository,
            &revision,
            &guard,
            decoded.query,
            decoded.limits,
            decoded.allow_continuation,
            decoded.after,
        )
        .map_err(|error| repository_query_failure(&error))?;
        if outcome.request.preimage() != body {
            return owner_failure("QUERY_SNAPSHOT_MISMATCH", 31_003);
        }
        let response = outcome.response;
        let edge_class = matches!(response.class_tag(), 12 | 13);
        let body = response.record().to_vec();
        let bounds = BoundedContext {
            applied_limits: self.profile.limits,
            returned_bytes: to_u64(body.len())?,
            returned_entities: if edge_class { 0 } else { response.returned() },
            returned_edges: if edge_class { response.returned() } else { 0 },
            reached_depth: response.reached_depth(),
            omitted: response.total_count().saturating_sub(response.returned()),
            truncated: response.truncated(),
            continuation: response.next_after().is_some(),
        };
        Ok((body, bounds))
    }

    fn handle_expand(&self, body: &[u8], session: SessionId) -> Result<(Vec<u8>, BoundedContext)> {
        // The request names the root it expects alongside the position
        // (contract section 4): `uvar(handle) || StateRoot[32]`.
        let mut offset = 0_usize;
        let handle = uvar_at(body, &mut offset)?;
        let expected_root = StateRoot::from_bytes(fixed32(body.get(offset..).unwrap_or(&[]))?);
        if body.len() != offset + 32 {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        let (head, binding) = self.head_binding()?;
        let request = CompleteRootRequest::extract(&head)
            .map_err(|error| owner(error.code(), error.numeric()))?;
        let kinds: Vec<u32> = request
            .borrowed()
            .iter()
            .map(|entity| entity.kind().tag())
            .collect();
        let facts = self
            .authority
            .expand_handle(
                session,
                &binding,
                request.bound_objects(),
                &kinds,
                handle,
                expected_root,
            )
            .map_err(session_failure)?;
        let record = scb(encode_record(&[
            (1, facts.entity.as_bytes().to_vec()),
            (2, encode_uvar(u64::from(facts.kind))),
            (3, facts.object_id.as_bytes().to_vec()),
            (4, facts.bound_root.as_bytes().to_vec()),
            (5, facts.session_id.as_bytes().to_vec()),
        ]))?;
        self.counted(record, 1)
    }

    fn capsule(&self, body: &[u8], session: SessionId) -> Result<(Vec<u8>, BoundedContext)> {
        let decoded = decode_root_query(body)?;
        let revision = self.head()?;
        // Exported capsules build from a fresh snapshot, never a bare
        // cache hit: a hit is accepted without re-deriving edges, so only
        // a fresh build may underwrite evidence that leaves the process.
        let outcome = run_root_query_fresh(
            &revision,
            decoded.query.clone(),
            decoded.limits,
            decoded.allow_continuation,
            decoded.after,
        )
        .map_err(|error| repository_query_failure(&error))?;
        if outcome.request.preimage() != body {
            return owner_failure("QUERY_SNAPSHOT_MISMATCH", 31_003);
        }
        // The bound capsule is minted by the session authority, never
        // from caller-declared provenance: the authority verifies the
        // session is live and its binding equals the response provenance
        // (capsule contract section 2).
        let capsule =
            match self
                .authority
                .bind_context_capsule(session, &outcome.request, &outcome.response)
            {
                Ok(capsule) => capsule,
                Err(CapsuleBindError::UnknownSession) => {
                    return Err(session_failure(SessionError::new(
                        crate::session::SessionErrorCode::Unknown,
                    )));
                }
                Err(CapsuleBindError::Capsule(error)) => {
                    return Err(owner(error.code().as_str(), error.code().numeric()));
                }
            };
        let body = capsule.record().to_vec();
        let bounds = BoundedContext {
            applied_limits: self.profile.limits,
            returned_bytes: to_u64(body.len())?,
            returned_entities: to_u64(capsule.entities().len())?,
            returned_edges: to_u64(capsule.relationships().len())?,
            reached_depth: 0,
            omitted: capsule.omitted(),
            truncated: capsule.is_truncated(),
            continuation: capsule.next_after().is_some(),
        };
        Ok((body, bounds))
    }

    fn query_restricted(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let decoded = decode_restricted_query(body)?;
        let revision = self.head()?;
        let request = CompleteRootRequest::extract(&revision)
            .map_err(|error| owner(error.code(), error.numeric()))?;
        let borrowed = request.borrowed();
        let restricted: Vec<_> = borrowed
            .iter()
            .filter(|entity| entity.kind().restricted_kind())
            .copied()
            .collect();
        let context = SnapshotContext {
            schema_epoch: request.schema_epoch_id(),
            claimed_root_context: Some(request.root()),
        };
        let snapshot = build_index_snapshot(context, &restricted).map_err(|error| match error {
            sley_query::IndexSnapshotBuildError::Impact(impact) => {
                owner(impact.code().as_str(), impact.code().numeric())
            }
            sley_query::IndexSnapshotBuildError::Snapshot(snapshot) => {
                owner(snapshot.code().as_str(), snapshot.code().numeric())
            }
        })?;
        if snapshot.snapshot_id() != decoded.snapshot_id
            || snapshot.context() != decoded.context
            || snapshot.completeness() != IndexCompleteness::RestrictedModeledKinds4To15Only
        {
            return owner_failure("QUERY_SNAPSHOT_MISMATCH", 31_003);
        }
        let request = build_restricted_query_request(&snapshot, decoded.query, decoded.limits)
            .map_err(|error| owner(error.code().as_str(), error.code().numeric()))?;
        if request.preimage() != body {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        let response = execute_restricted_query(&snapshot, &request)
            .map_err(|error| owner(error.code().as_str(), error.code().numeric()))?;
        let body = response.record().to_vec();
        let bounds = BoundedContext {
            applied_limits: self.profile.limits,
            returned_bytes: to_u64(body.len())?,
            returned_entities: response.returned_entities(),
            returned_edges: response.returned_edges(),
            reached_depth: response.reached_depth(),
            ..BoundedContext::none()
        };
        Ok((body, bounds))
    }

    fn workspace_create(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 4)?;
        let state_registry = state_registry()
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        let policy_registry = policy_registry()
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        let state = import_state_root(&state_registry, fields[0])
            .map_err(|error| owner(error.code_str(), 0))?;
        let policy = import_policy_root(&policy_registry, fields[1])
            .map_err(|error| owner(error.code_str(), 0))?;
        let epoch = state_epoch_id()
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        let objects = list(fields[2])?
            .into_iter()
            .map(|item| {
                import_entity_object(epoch, item).map_err(|error| owner(error.code().as_str(), 0))
            })
            .collect::<Result<Vec<_>>>()?;
        let tombstones = list(fields[3])?
            .into_iter()
            .map(|item| fixed32(item).map(EntityId::from_bytes))
            .collect::<Result<Vec<_>>>()?;
        let head = self
            .transactions()
            .initialize_trusted_genesis(TrustedGenesisInput::new(
                &state,
                &policy,
                &objects,
                &tombstones,
            ))
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        self.counted(
            head.transaction_id().as_bytes().to_vec(),
            to_u64(objects.len())?,
        )
    }

    /// The head-pinned opener (SMP1 revision 13, appendix A row 201): an
    /// empty request answered with the accepted head's summary. Under a
    /// version 1 selection the body is exactly `revision_summary`. Under a
    /// version 2 (or later) selection it is `open_summary`: the same eight
    /// fields plus field 9, the accepted head's complete-root index snapshot
    /// identity, present only when the S20-300 read-only probe finds an
    /// accepted cache record for that root. The probe never builds or writes
    /// the cache; any probe condition (no record, a discarded record, a
    /// missing or contended maintenance boundary) is structural absence of
    /// field 9, never an `omitted` or `truncated` signal, and the response
    /// counts one entity either way. `revision.read` keeps the eight-field
    /// encoding byte for byte.
    fn workspace_open(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        if !body.is_empty() {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        let head = self.head()?;
        let snapshot = if self.profile.protocol_version >= PROTOCOL_VERSION_V2 {
            self.materialized_head_snapshot(&head)
        } else {
            None
        };
        let summary = head_open_summary(&head, snapshot)?;
        self.counted(summary, 1)
    }

    /// The accepted head's cached snapshot identity (S20-300 revision 4
    /// probe reader). The probe adds no wait and no write: the maintenance
    /// boundary is taken shared without waiting and never initialized here,
    /// so an absent or contended boundary, or any probe failure, is
    /// absence. (Loading the accepted head itself already takes the shared
    /// maintenance lock, blocking, as every head-bound read does; that is
    /// S20-390 behavior this probe does not change.)
    fn materialized_head_snapshot(
        &self,
        head: &VerifiedRevision,
    ) -> Option<sley_id::IndexSnapshotId> {
        let guard = acquire_shared_repository_maintenance_nonblocking(&self.repository).ok()?;
        cached_complete_root_snapshot_id(&self.repository, head, &guard)
            .ok()
            .flatten()
    }

    fn candidate_create(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let record = decode_candidate_record(body).map_err(|error| owner(error.code(), 0))?;
        let candidate = build_candidate(&record).map_err(|error| owner(error.code(), 0))?;
        let count = to_u64(candidate.record.operations.len())?;
        self.counted(candidate.stored_bytes, count)
    }

    fn candidate_append(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 2)?;
        let base = import_candidate(fields[0]).map_err(|error| owner(error.code(), 0))?;
        let addition =
            decode_candidate_record(fields[1]).map_err(|error| owner(error.code(), 0))?;
        let mut record = base.record;
        record.operations.extend(addition.operations);
        record.preconditions.extend(addition.preconditions);
        let candidate = build_candidate(&record).map_err(|error| owner(error.code(), 0))?;
        let count = to_u64(candidate.record.operations.len())?;
        self.counted(candidate.stored_bytes, count)
    }

    fn candidate_inspect(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let candidate = import_candidate(body).map_err(|error| owner(error.code(), 0))?;
        let record = &candidate.record;
        let summary = scb(encode_record(&[
            (1, candidate.candidate_id.as_bytes().to_vec()),
            (2, record.workspace_id.as_bytes().to_vec()),
            (3, record.base_transaction_id.as_bytes().to_vec()),
            (4, record.base_root.as_bytes().to_vec()),
            (5, record.principal_id.as_bytes().to_vec()),
            (6, encode_uvar(to_u64(record.operations.len())?)),
            (7, encode_uvar(to_u64(record.preconditions.len())?)),
        ]))?;
        self.counted(summary, to_u64(record.operations.len())?)
    }

    fn candidate_validate(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 4)?;
        let base_id = TransactionId::from_bytes(fixed32(fields[0])?);
        let principal = PrincipalId::from_bytes(fixed32(fields[1])?);
        let now = single_uvar(fields[2])?;
        let base = self.revision(base_id)?;
        let context = CandidateValidationContext::new(
            base_id,
            base.state_root(),
            base.objects(),
            base.tombstoned_entities(),
            base.policy_root(),
            principal,
            &[],
            now,
            CandidateValidationLimits::full_v1(),
        )
        .map_err(|error| owner(&error.to_string(), 0))?;
        let output = validate_candidate_bytes(&context, fields[3])
            .map_err(|error| owner(&error.to_string(), 0))?;
        self.counted(output.result().stored_bytes.clone(), 1)
    }

    /// Native commit over the v3 route: takes the four-field native
    /// payload under the existing commit method when version 3 with the
    /// native-tests bit negotiated (contract appendix C, revision 5).
    ///
    /// The session must still be bound to the accepted head. The principal
    /// comes from the validated candidate itself, never the caller; no
    /// caller capabilities are honored and the server clock stamps
    /// validation. Without provisioned authority the call refuses as
    /// `NATIVE_SIGNER_UNAVAILABLE` before any journal or accepted-state
    /// write; trust and admission failures then surface with their
    /// preserved symbols through the commit owner. Success answers the
    /// v3-only record and advances the head, invalidating diagnostic
    /// tokens like any commit. The journaled attempt is what 607 later
    /// resolves and 606 replays against.
    fn commit_native_route(
        &mut self,
        session: SessionId,
        body: &[u8],
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let session_root = self
            .authority
            .record(session)
            .map(|record| record.bound_root)
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        let (head, _) = self.head_binding_mixed()?;
        if session_root != head.state_root().root {
            return Err(session_failure(SessionError::new(
                SessionErrorCode::RootAdvanced,
            )));
        }
        let fields = record(body, 4)?;
        let candidate = import_candidate(fields[0]).map_err(|error| owner(error.code(), 0))?;
        let expected_parent = TransactionId::from_bytes(fixed32(fields[1])?);
        let attempt = NativeAttemptId(fixed16(fields[2])?);
        let admission_profile = NativeAdmissionProfileId::from_bytes(fixed32(fields[3])?);
        let authority = self
            .native_authority
            .as_ref()
            .ok_or_else(|| owner("NATIVE_SIGNER_UNAVAILABLE", 0))?;
        let input = NativeCommitInput {
            expected_parent,
            stored_candidate: fields[0],
            principal_id: candidate.record.principal_id,
            capabilities: &[],
            now_unix_millis: (self.now_millis)(),
            limits: CandidateValidationLimits::full_v1(),
            attempt_id: attempt,
            admission_profile_id: admission_profile,
            implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
            aggregate: NativeAggregateLimits::HARD_MAXIMA,
            executor: Some(&*authority.executor),
            acceptance_signer: &*authority.signer,
            measurement_trust: authority.measurement_trust(),
            acceptance_trust: authority.acceptance_trust(),
        };
        let outcome = self
            .transactions()
            .commit_native(&input)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        // Rejected tests are a refusal with the preserved failure symbol,
        // never a success: the journal keeps the aborted attempt for 607
        // while the head stays unchanged.
        let output = match outcome {
            sley_txn::NativeCommitOutcome::Committed(output) => output,
            sley_txn::NativeCommitOutcome::Rejected(rejection) => {
                match rejection.approval.decision() {
                    sley_tests::ApprovalDecision::Rejected(record) => {
                        return Err(owner(&record.symbol, record.numeric_code));
                    }
                    sley_tests::ApprovalDecision::Accepted => {
                        return Err(ProtocolFailure::protocol(
                            ProtocolErrorCode::InternalInvariant,
                        ));
                    }
                }
            }
        };
        let body = scb(encode_record(&[
            (1, output.transaction_id().as_bytes().to_vec()),
            (2, output.receipt_id().as_bytes().to_vec()),
            (3, output.state_root().root.as_bytes().to_vec()),
            (
                4,
                scb(encode_bytes(&output.candidate_result().stored_bytes))?,
            ),
            (5, output.approval_id().as_bytes().to_vec()),
            (6, output.attempt_id().0.to_vec()),
        ]))?;
        // The commit advances the head every token was minted under.
        self.invalidate_all_diagnostics();
        self.counted(body, 1)
    }

    fn commit(&mut self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 4)?;
        let parent = TransactionId::from_bytes(fixed32(fields[0])?);
        let principal = PrincipalId::from_bytes(fixed32(fields[1])?);
        let now = single_uvar(fields[2])?;
        let input = CommitInput::new(
            parent,
            fields[3],
            principal,
            &[],
            now,
            CandidateValidationLimits::full_v1(),
        );
        let output = self
            .transactions()
            .commit(input)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        let record = scb(encode_record(&[
            (1, output.transaction_id().as_bytes().to_vec()),
            (2, output.receipt_id().as_bytes().to_vec()),
            (3, output.state_root().root.as_bytes().to_vec()),
            (
                4,
                scb(encode_bytes(&output.candidate_result().stored_bytes))?,
            ),
        ]))?;
        let _: ReceiptId = output.receipt_id();
        // The commit advances the head every token was minted under.
        self.invalidate_all_diagnostics();
        self.counted(record, 1)
    }

    fn merge_commit(&mut self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 7)?;
        let ancestor = MergeSide::from_revision(
            &self.revision(TransactionId::from_bytes(fixed32(fields[0])?))?,
        );
        let ours = MergeSide::from_revision(
            &self.revision(TransactionId::from_bytes(fixed32(fields[1])?))?,
        );
        let theirs = MergeSide::from_revision(
            &self.revision(TransactionId::from_bytes(fixed32(fields[2])?))?,
        );
        let principal = PrincipalId::from_bytes(fixed32(fields[3])?);
        let now = single_uvar(fields[4])?;
        let expiry = single_uvar(fields[5])?;
        let branch = fields[6];
        let merge_failure = |error: sley_repo::MergeError| {
            owner(
                &error.symbol(),
                error.code().map_or_else(
                    || error.commit_numeric().unwrap_or(0),
                    sley_repo::MergeErrorCode::numeric,
                ),
            )
        };
        let outcome = self.verified_merge(&ancestor, &ours, &theirs)?;
        match outcome {
            MergeOutcome::Conflict(conflict) => {
                let count = to_u64(conflict.conflict.conflicts.len())?;
                self.counted(scb(encode_union(2, &conflict.stored_bytes))?, count)
            }
            MergeOutcome::Merged(merged) => {
                let plan = build_merge_plan(&ours, &merged).map_err(merge_failure)?;
                let transaction_id = commit_merge(
                    &self.transactions(),
                    &self.branches(),
                    &ours,
                    &plan,
                    MergeCommitInput {
                        principal_id: principal,
                        now_unix_millis: now,
                        expiry_unix_millis: expiry,
                        limits: CandidateValidationLimits::full_v1(),
                        branch,
                    },
                )
                .map_err(merge_failure)?;
                // A merged commit advances the head like any commit.
                self.invalidate_all_diagnostics();
                self.counted(scb(encode_union(1, transaction_id.as_bytes()))?, 1)
            }
        }
    }

    fn exchange_import(&mut self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let epoch = state_epoch_id()
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        let verifier =
            move |bytes: &[u8]| import_entity_object(epoch, bytes).map(|object| object.object_id());
        let report = import_repository_exchange(&self.repository, body, &verifier)
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        let record = scb(encode_record(&[
            (1, report.exchange_id.as_bytes().to_vec()),
            (2, report.accepted_head.transaction_id().as_bytes().to_vec()),
            (3, encode_uvar(to_u64(report.receipts)?)),
            (4, encode_uvar(to_u64(report.branches)?)),
        ]))?;
        // An import advances the accepted head past every minted token.
        self.invalidate_all_diagnostics();
        self.counted(record, to_u64(report.receipts)?)
    }

    fn counted(&self, body: Vec<u8>, entities: u64) -> Result<(Vec<u8>, BoundedContext)> {
        let bounds = BoundedContext {
            applied_limits: self.profile.limits,
            returned_bytes: to_u64(body.len())?,
            returned_entities: entities,
            ..BoundedContext::none()
        };
        Ok((body, bounds))
    }
}

fn session_failure(error: SessionError) -> ProtocolFailure {
    let mut failure = owner(error.code().as_str(), error.code().numeric());
    failure.retryability = match error.code() {
        crate::session::SessionErrorCode::RootAdvanced
        | crate::session::SessionErrorCode::StaleHandle => Retryability::AfterRequery,
        _ => Retryability::Never,
    };
    failure
}

/// Whether a numeric method tag names an entity read (306 or 307).
fn is_entity_read_method(tag: u32) -> bool {
    tag == Method::EntityVersion.tag() || tag == Method::EntitySignature.tag()
}

/// Maps a transport-neutral entity-read failure to its wire failure:
/// shape defects to `PROTOCOL_PAYLOAD_INVALID`, budget exhaustion and
/// checked overflow to `PROTOCOL_LIMIT_EXCEEDED`, owner failures keeping
/// their stable codes.
fn entity_read_failure(error: EntityReadError) -> ProtocolFailure {
    match error {
        EntityReadError::NotCanonical => {
            ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid)
        }
        EntityReadError::BudgetExceeded => {
            ProtocolFailure::protocol(ProtocolErrorCode::LimitExceeded)
        }
        _ => owner(
            error.owner_symbol().unwrap_or("QUERY_INTERNAL_INVARIANT"),
            error.owner_numeric().unwrap_or(31_007),
        ),
    }
}

fn repository_query_failure(error: &RepositoryQueryError) -> ProtocolFailure {
    match error {
        RepositoryQueryError::Extraction(inner) => owner(inner.code(), inner.numeric()),
        RepositoryQueryError::Cache(inner) => owner(&inner.code(), cache_numeric(inner)),
        RepositoryQueryError::Query(inner) => owner(inner.code().as_str(), inner.code().numeric()),
        RepositoryQueryError::Capsule(inner) => {
            owner(inner.code().as_str(), inner.code().numeric())
        }
    }
}

fn cache_numeric(error: &IndexCacheError) -> u32 {
    match error {
        IndexCacheError::Snapshot(build) => match build {
            sley_query::IndexSnapshotBuildError::Impact(impact) => impact.code().numeric(),
            sley_query::IndexSnapshotBuildError::Snapshot(snapshot) => snapshot.code().numeric(),
        },
        IndexCacheError::Extraction(extraction) => extraction.numeric(),
        IndexCacheError::Io(_) => 30_010,
    }
}

const fn status_tag(status: BranchUpdateStatus) -> u32 {
    match status {
        BranchUpdateStatus::Created => 1,
        BranchUpdateStatus::Advanced => 2,
        BranchUpdateStatus::Present => 3,
    }
}

fn branch_summary(branch: &sley_repo::ResolvedBranch) -> Result<Vec<u8>> {
    scb(encode_record(&[
        (
            1,
            scb(encode_bytes(branch.origin.record.branch_name.as_bytes()))?,
        ),
        (
            2,
            branch
                .origin
                .record
                .origin_transaction_id
                .as_bytes()
                .to_vec(),
        ),
        (3, branch.revision.transaction_id().as_bytes().to_vec()),
        (4, branch.revision.state_root().root.as_bytes().to_vec()),
    ]))
}

fn revision_summary(revision: &VerifiedRevision) -> Result<Vec<u8>> {
    scb(encode_record(&revision_summary_fields(revision)?))
}

/// `workspace.open` only (SMP1 revision 13 `open_summary`): the head
/// summary plus field 9 when the caller passes the head's materialized
/// snapshot. Kept apart from `revision_summary`, which has no knowledge of
/// headness and serves arbitrary caller-named revisions.
fn head_open_summary(
    head: &VerifiedRevision,
    snapshot: Option<sley_id::IndexSnapshotId>,
) -> Result<Vec<u8>> {
    let mut fields = revision_summary_fields(head)?;
    if let Some(snapshot) = snapshot {
        fields.push((9, snapshot.as_bytes().to_vec()));
    }
    scb(encode_record(&fields))
}

fn revision_summary_fields(revision: &VerifiedRevision) -> Result<Vec<(u32, Vec<u8>)>> {
    let record = &revision.state_root().record;
    Ok(vec![
        (1, revision.transaction_id().as_bytes().to_vec()),
        (2, revision.state_root().root.as_bytes().to_vec()),
        (3, revision.policy_root().root().as_bytes().to_vec()),
        (4, record.workspace_id.as_bytes().to_vec()),
        (5, record.schema_epoch_id.as_bytes().to_vec()),
        (6, encode_uvar(to_u64(revision.objects().len())?)),
        (
            7,
            encode_uvar(to_u64(revision.tombstoned_entities().len())?),
        ),
        (8, revision.receipt().receipt_id.as_bytes().to_vec()),
    ])
}

fn encode_limits(limits: &LimitProfile) -> Result<Vec<u8>> {
    scb(encode_record(&[
        (1, encode_uvar(limits.max_frame_bytes)),
        (2, encode_uvar(limits.max_entities)),
        (3, encode_uvar(limits.max_edges)),
        (4, encode_uvar(u64::from(limits.max_depth))),
        (5, encode_uvar(limits.max_response_bytes)),
        (6, encode_uvar(limits.max_work)),
        (7, encode_uvar(u64::from(limits.max_inflight))),
    ]))
}

// ---------------------------------------------------------------------------
// Request preimage decoders (S20-310 full and restricted grammars)
// ---------------------------------------------------------------------------

struct DecodedRootQuery {
    query: RootQuery,
    limits: QueryLimits,
    allow_continuation: bool,
    after: Option<Cursor>,
}

struct DecodedRestrictedQuery {
    snapshot_id: sley_id::IndexSnapshotId,
    context: SnapshotContext,
    query: RestrictedQuery,
    limits: QueryLimits,
}

struct Wire<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Wire<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?;
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(
            |_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid),
        )?))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(
            |_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid),
        )?))
    }

    fn id(&mut self) -> Result<[u8; 32]> {
        fixed32(self.take(32)?)
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn limits(&mut self) -> Result<QueryLimits> {
        Ok(QueryLimits {
            max_returned_entities: self.u64()?,
            max_returned_edges: self.u64()?,
            max_depth: self.u32()?,
            max_response_bytes: self.u64()?,
            max_work: self.u64()?,
        })
    }

    fn kinds(&mut self) -> Result<Vec<ImpactKind>> {
        let count = self.u64()?;
        if count > 12 {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        (0..count).map(|_| impact_kind(self.u32()?)).collect()
    }

    fn ids(&mut self) -> Result<Vec<EntityId>> {
        let count = self.u64()?;
        if count > 65_535 {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        (0..count)
            .map(|_| self.id().map(EntityId::from_bytes))
            .collect()
    }

    fn cursor(&mut self) -> Result<Option<Cursor>> {
        match self.u32()? {
            OPTION_NONE => Ok(None),
            OPTION_SOME => match self.u32()? {
                1 => Ok(Some(Cursor::Entity(EntityId::from_bytes(self.id()?)))),
                2 => Ok(Some(Cursor::Edge(ImpactEdge {
                    dependent: EntityId::from_bytes(self.id()?),
                    dependency: EntityId::from_bytes(self.id()?),
                    kind: impact_kind(self.u32()?)?,
                }))),
                3 => Ok(Some(Cursor::Root(StateRoot::from_bytes(self.id()?)))),
                _ => protocol_failure(ProtocolErrorCode::PayloadInvalid),
            },
            _ => protocol_failure(ProtocolErrorCode::PayloadInvalid),
        }
    }
}

fn impact_kind(tag: u32) -> Result<ImpactKind> {
    const KINDS: [ImpactKind; 12] = [
        ImpactKind::Ownership,
        ImpactKind::TypeReference,
        ImpactKind::ValueReference,
        ImpactKind::ControlFlow,
        ImpactKind::Call,
        ImpactKind::Effect,
        ImpactKind::Capability,
        ImpactKind::Contract,
        ImpactKind::Initializer,
        ImpactKind::TestTarget,
        ImpactKind::Adapter,
        ImpactKind::DefinitionMember,
    ];
    usize::try_from(tag)
        .ok()
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| KINDS.get(index).copied())
        .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))
}

fn decode_root_query(body: &[u8]) -> Result<DecodedRootQuery> {
    let mut wire = Wire::new(body);
    if wire.take(8)? != ROOT_QUERY_MAGIC {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    if wire.u32()? != 1 || wire.u32()? != 1 {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    let _snapshot_id = wire.id()?;
    let _schema_epoch = wire.id()?;
    let _root = wire.id()?;
    let _workspace = wire.id()?;
    if wire.u32()? != 2 || wire.u32()? != 1 {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    let limits = wire.limits()?;
    let allow_continuation = match wire.u32()? {
        1 => false,
        2 => true,
        _ => return protocol_failure(ProtocolErrorCode::PayloadInvalid),
    };
    let after = wire.cursor()?;
    let class = wire.u32()?;
    let entity = |wire: &mut Wire<'_>| wire.id().map(EntityId::from_bytes);
    let query = match class {
        1 => RootQuery::GetRootSummary,
        2 => RootQuery::GetEntity {
            entity: entity(&mut wire)?,
        },
        3 => RootQuery::GetSemanticFingerprint {
            entity: entity(&mut wire)?,
        },
        4 => RootQuery::ListEntitiesByKind {
            kind: ModeledEntityKind::from_ssmc_tag(wire.u32()?)
                .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?,
        },
        5 => RootQuery::ListWorkspacePackages,
        6 => RootQuery::ListPackageExports {
            package: entity(&mut wire)?,
        },
        7 => RootQuery::ListPackageDependencies {
            package: entity(&mut wire)?,
        },
        8 => RootQuery::ListNamespaceMembers {
            namespace: entity(&mut wire)?,
        },
        9 => RootQuery::ListOwningNamespaces {
            entity: entity(&mut wire)?,
        },
        10 => RootQuery::ListEntryPoints,
        11 => RootQuery::ListDependencyRoots,
        12 => {
            let entity = entity(&mut wire)?;
            RootQuery::ListDirectDependencies {
                entity,
                kinds: wire.kinds()?,
            }
        }
        13 => {
            let entity = entity(&mut wire)?;
            RootQuery::ListDirectDependents {
                entity,
                kinds: wire.kinds()?,
            }
        }
        14 => RootQuery::ReverseImpactClosure { seeds: wire.ids()? },
        15 => RootQuery::ForwardDependencyClosure { seeds: wire.ids()? },
        16 => RootQuery::ListContractsFor {
            target: entity(&mut wire)?,
        },
        17 => RootQuery::ListTestsFor {
            target: entity(&mut wire)?,
        },
        18 => RootQuery::ListDeclaredEffects {
            entity: entity(&mut wire)?,
        },
        19 => RootQuery::ListCapabilityRequirementsFor {
            subject: entity(&mut wire)?,
        },
        _ => return protocol_failure(ProtocolErrorCode::PayloadInvalid),
    };
    if !wire.finished() {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    Ok(DecodedRootQuery {
        query,
        limits,
        allow_continuation,
        after,
    })
}

fn decode_restricted_query(body: &[u8]) -> Result<DecodedRestrictedQuery> {
    let mut wire = Wire::new(body);
    if wire.take(8)? != RESTRICTED_QUERY_MAGIC {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    if wire.u32()? != 1 || wire.u32()? != 1 {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    let snapshot_id = sley_id::IndexSnapshotId::from_bytes(wire.id()?);
    let schema_epoch = SchemaEpochId::from_bytes(wire.id()?);
    let claimed_root_context = match wire.u32()? {
        OPTION_NONE => None,
        OPTION_SOME => Some(StateRoot::from_bytes(wire.id()?)),
        _ => return protocol_failure(ProtocolErrorCode::PayloadInvalid),
    };
    if wire.u32()? != 1 || wire.u32()? != 1 {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    let limits = wire.limits()?;
    let query = match wire.u32()? {
        1 => RestrictedQuery::GetModeledEntityKind {
            entity: EntityId::from_bytes(wire.id()?),
        },
        2 => {
            let entity = EntityId::from_bytes(wire.id()?);
            RestrictedQuery::ListDirectDependencies {
                entity,
                kinds: wire.kinds()?,
            }
        }
        3 => {
            let entity = EntityId::from_bytes(wire.id()?);
            RestrictedQuery::ListDirectDependents {
                entity,
                kinds: wire.kinds()?,
            }
        }
        4 => RestrictedQuery::ReverseImpactClosure { seeds: wire.ids()? },
        _ => return protocol_failure(ProtocolErrorCode::PayloadInvalid),
    };
    if !wire.finished() {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    Ok(DecodedRestrictedQuery {
        snapshot_id,
        context: SnapshotContext {
            schema_epoch,
            claimed_root_context,
        },
        query,
        limits,
    })
}

// ---------------------------------------------------------------------------
// Small SCB1 helpers shared with the frame codec
// ---------------------------------------------------------------------------

fn scb<T>(result: core::result::Result<T, sley_scb1::ScbError>) -> Result<T> {
    result.map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))
}

fn to_u64(value: usize) -> Result<u64> {
    u64::try_from(value).map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::LimitExceeded))
}

fn fixed32(input: &[u8]) -> Result<[u8; 32]> {
    input
        .try_into()
        .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))
}

fn fixed16(input: &[u8]) -> Result<[u8; 16]> {
    input
        .try_into()
        .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))
}

/// Encodes an optional 32-byte identity for 606/607 records: exactly 32
/// bytes when present, empty when absent (contract appendix C, revision
/// 5). The shape is strict so a truncated identity can never decode as
/// absent.
fn encode_option_id(value: Option<[u8; 32]>) -> Vec<u8> {
    value.map_or_else(Vec::new, |identity| identity.to_vec())
}

/// Decodes an optional 32-byte identity: empty means absent, exactly 32
/// bytes means present, any other length is malformed.
fn option_id(input: &[u8]) -> Result<Option<[u8; 32]>> {
    if input.is_empty() {
        return Ok(None);
    }
    Ok(Some(fixed32(input)?))
}

/// Maps a replay verdict to its Appendix C response status number:
/// matched 1, mismatch 2, inconclusive-resource 3, untrusted-history 4.
pub(crate) fn replay_status_number(status: NativeReplayStatus) -> u64 {
    match status {
        NativeReplayStatus::Matched => 1,
        NativeReplayStatus::Mismatch => 2,
        NativeReplayStatus::InconclusiveResource => 3,
        NativeReplayStatus::UntrustedHistory => 4,
    }
}

/// Maps a reconciled attempt status to its Appendix C state number:
/// `UnknownAttempt0` plus the §6 journal tags 1..=6.
pub(crate) fn attempt_state_number(status: &AttemptStatus) -> u64 {
    match status {
        AttemptStatus::Unknown => 0,
        AttemptStatus::Admitted => 1,
        AttemptStatus::Running => 2,
        AttemptStatus::AbortedBeforePromotion => 3,
        AttemptStatus::PromotionStarted => 4,
        AttemptStatus::Committed { .. } => 5,
        AttemptStatus::OutcomeUnknown { .. } => 6,
    }
}

/// Parses a strictly increasing set of 32-byte identities from one SCB1
/// list field. Wire order is part of the contract: an unsorted or
/// duplicated set refuses rather than being silently canonicalized.
fn parse_id_set(field: &[u8]) -> Result<Vec<EntityId>> {
    let items = list(field)?;
    let mut identities = Vec::with_capacity(items.len().min(257));
    for item in items {
        identities.push(EntityId::from_bytes(fixed32(item)?));
    }
    if identities.windows(2).any(|pair| pair[0] >= pair[1]) {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    Ok(identities)
}

/// Maps a native plan derivation refusal to its preserved owner failure.
fn native_plan_failure(error: NativePlanErrorV1) -> ProtocolFailure {
    owner(error.symbol(), 0)
}

/// Splits one revision's objects into executor bytes keyed by object
/// identity and canonical test-case bodies keyed by test entity, both
/// owner-loaded from the same revision the plan derived from.
fn diagnostic_test_cases(
    head: &HeadRevision,
) -> (BTreeMap<ObjectId, &[u8]>, BTreeMap<EntityId, &TestCaseBody>) {
    let mut object_bytes = BTreeMap::new();
    let mut test_cases = BTreeMap::new();
    for object in head.objects() {
        object_bytes.insert(object.object_id(), object.stored_bytes());
        if let EntityBodyValue::TestCase(body) = &object.record().body {
            test_cases.insert(object.record().entity_id, body);
        }
    }
    (object_bytes, test_cases)
}

fn single_uvar(input: &[u8]) -> Result<u64> {
    let mut value = 0_u64;
    let mut shift = 0_u32;
    for (index, byte) in input.iter().enumerate() {
        if shift >= 64 {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            if index + 1 != input.len() || (*byte == 0 && index != 0) {
                return protocol_failure(ProtocolErrorCode::PayloadInvalid);
            }
            return Ok(value);
        }
        shift += 7;
    }
    protocol_failure(ProtocolErrorCode::PayloadInvalid)
}

fn uvar_at(input: &[u8], offset: &mut usize) -> Result<u64> {
    let mut value = 0_u64;
    let mut shift = 0_u32;
    loop {
        let byte = *input
            .get(*offset)
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?;
        *offset += 1;
        if shift >= 64 {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
    }
}

/// Reads a list of length-delimited items.
fn list(input: &[u8]) -> Result<Vec<&[u8]>> {
    let mut offset = 0_usize;
    let count = uvar_at(input, &mut offset)?;
    if count > 1_000_000 {
        return protocol_failure(ProtocolErrorCode::LimitExceeded);
    }
    let mut items = Vec::with_capacity(usize::try_from(count).unwrap_or(0).min(4_096));
    for _ in 0..count {
        let len = usize::try_from(uvar_at(input, &mut offset)?)
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?;
        let end = offset
            .checked_add(len)
            .filter(|end| *end <= input.len())
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?;
        items.push(&input[offset..end]);
        offset = end;
    }
    if offset != input.len() {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    Ok(items)
}

/// Reads a record with exactly the fields 1 through `expected`.
fn record(input: &[u8], expected: u64) -> Result<Vec<&[u8]>> {
    let mut offset = 0_usize;
    if uvar_at(input, &mut offset)? != expected {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    let mut fields = Vec::with_capacity(usize::try_from(expected).unwrap_or(0));
    for index in 1..=expected {
        if uvar_at(input, &mut offset)? != index {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        let len = usize::try_from(uvar_at(input, &mut offset)?)
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?;
        let end = offset
            .checked_add(len)
            .filter(|end| *end <= input.len())
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?;
        fields.push(&input[offset..end]);
        offset = end;
    }
    if offset != input.len() {
        return protocol_failure(ProtocolErrorCode::PayloadInvalid);
    }
    Ok(fields)
}

// ---------------------------------------------------------------------------
// Slice C: garbage collection, execution, and reports (SMP1 appendix C)
// ---------------------------------------------------------------------------

const PIN_STATE_ROOT: u64 = 1;
const PIN_OBJECT: u64 = 2;
const MAX_GC_PINS: usize = 4_096;

impl Server {
    /// `gc.dry_run` (212) and `gc.collect` (213): the server derives the
    /// retention snapshot from its refs, its accepted head, and every live
    /// session's bound root; the request may only add session pins
    /// (contract appendix C).
    fn gc(
        &self,
        body: &[u8],
        session: SessionId,
        collect: bool,
    ) -> Result<(Vec<u8>, BoundedContext)> {
        let pins = decode_pins(body)?;
        let epoch = state_epoch_id()
            .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))?;
        let head = self.head()?;
        let mut roots: BTreeMap<StateRoot, AcceptedStateRoot> = BTreeMap::new();
        let mut anchors = Vec::new();
        let head_root = head.state_root().root;
        anchors.push(RetentionAnchor::new(
            RetentionKind::Transaction,
            *head.transaction_id().as_bytes(),
            vec![RetentionTarget::StateRoot(head_root)],
        ));
        roots.insert(head_root, head.state_root().clone());
        let branches = self
            .branches()
            .list_branches(usize::try_from(MAX_BRANCH_LIST).unwrap_or(usize::MAX))
            .map_err(|error| owner(error.code(), error.numeric_code().unwrap_or(0)))?;
        let mut seen: BTreeSet<TransactionId> = BTreeSet::new();
        for branch in &branches {
            let revision = &branch.revision;
            if !seen.insert(revision.transaction_id()) {
                continue;
            }
            let root = revision.state_root().root;
            anchors.push(RetentionAnchor::new(
                RetentionKind::Ref,
                *revision.transaction_id().as_bytes(),
                vec![RetentionTarget::StateRoot(root)],
            ));
            roots
                .entry(root)
                .or_insert_with(|| revision.state_root().clone());
        }
        // Catalog every live session's bound root alongside the head and
        // branch revisions: a session bound to a root no branch targets
        // stays importable while the session is live, and the engine
        // retains exactly the catalogued closure (threat T15).
        for retained in self.authority.retained_roots() {
            roots.entry(retained.root).or_insert(retained);
        }
        // One SessionPin anchor per live session: a session's bound root
        // stays retained while the session is live, so one session never
        // collects another session's bound root (threat T15). Targets are
        // deduplicated within one anchor because the snapshot fails
        // duplicate targets closed.
        let mut pin_targets: BTreeMap<SessionId, Vec<RetentionTarget>> = BTreeMap::new();
        for (live, root) in self.authority.live_pins() {
            pin_targets.insert(live, vec![RetentionTarget::StateRoot(root)]);
        }
        if !pins.is_empty() {
            pin_targets.entry(session).or_default().extend(pins);
        }
        for (owner, targets) in pin_targets {
            let mut unique: Vec<RetentionTarget> = Vec::with_capacity(targets.len());
            for target in targets {
                if !unique.contains(&target) {
                    unique.push(target);
                }
            }
            anchors.push(RetentionAnchor::new(
                RetentionKind::SessionPin,
                *owner.as_bytes(),
                unique,
            ));
        }
        let snapshot = RetentionSnapshot::new(anchors, roots.into_values().collect())
            .map_err(|error| owner(error.symbol(), 0))?;
        let store = ObjectStore::new(&self.repository);
        let verifier = RepositoryObjectVerifier::new(epoch);
        let report = if collect {
            let guard = acquire_exclusive_gc(&store).map_err(|error| owner(error.symbol(), 0))?;
            gc_collect(&store, &snapshot, &verifier, &guard)
        } else {
            gc_dry_run(&store, &snapshot, &verifier)
        }
        .map_err(|error| owner(error.symbol(), 0))?;
        let objects = to_u64(report.inventory_objects.len())?;
        self.counted(encode_gc_report(&report)?, objects)
    }

    /// `execute` (600): runs one Function of the bound root under the
    /// restricted profile, builds the S20-290 report, and stores it.
    fn execute(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let fields = record(body, 3)?;
        let function = EntityId::from_bytes(fixed32(fields[0])?);
        let inputs = list(fields[1])?
            .into_iter()
            .map(|bytes| {
                decode_const_value(bytes)
                    .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))
            })
            .collect::<Result<Vec<_>>>()?;
        let (limits, profile) = decode_execution_limits(fields[2])?;
        // The extended profile is a negotiated capability (contract
        // appendix C): without the intersected `extended_execute`
        // feature bit, selecting it is a payload failure.
        if profile == CacheProfile::EXTENDED_V1
            && self.profile.features & FEATURE_EXTENDED_EXECUTE == 0
        {
            return protocol_failure(ProtocolErrorCode::PayloadInvalid);
        }
        let revision = self.head()?;
        let request = CompleteRootRequest::extract(&revision)
            .map_err(|error| owner(error.code(), error.numeric()))?;
        let entities = request.entities();
        let types = TypeEnvironment::new(entities.type_definitions.clone())
            .map_err(|error| owner(error.code().as_str(), error.code().numeric()))?;
        let Some(graph) = entities
            .functions
            .iter()
            .find(|graph| graph.entity_id == function)
        else {
            return Err(ProtocolFailure {
                details: FUNCTION_UNKNOWN_DETAIL.to_vec(),
                ..ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid)
            });
        };
        let input = || LoweringInput {
            types: &types,
            function: graph,
            parameters: &entities.parameters,
            blocks: &entities.blocks,
            operations: &entities.operations,
            schema_epoch: request.schema_epoch_id(),
            state_root: request.root(),
            profile,
            constants: &entities.constants,
            globals: &entities.globals,
            functions: &entities.functions,
            contracts: &entities.contracts,
            // The SMP1 execute path carries no adapter inventory: bridge
            // entries stay unreachable here until an owner wires one.
            adapters: &[],
        };
        let execution_request = ExecutionRequest { inputs, limits };
        let execution = execute_function(input(), execution_request.clone());
        let report = build_execution_report(input(), &execution_request, &execution)
            .map_err(|error| report_validation_owner(&error))?;
        let preimage = execution_report_preimage(&report)
            .map_err(|error| owner(error.code().as_str(), error.code().numeric()))?;
        let id = report.report_id();
        store_execution_report(&self.repository, id, &preimage)
            .map_err(|error| owner(error.code().as_str(), error.code().numeric()))?;
        self.counted(encode_execution_report(id, &preimage)?, 1)
    }

    /// `report` (604): answers the stored execution report for an identity.
    fn report(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
        let id = ExecutionReportId::from_bytes(fixed32(body)?);
        let preimage = read_execution_report(&self.repository, id).map_err(|error| {
            if error.code() == ReportStoreErrorCode::Unknown {
                ProtocolFailure {
                    details: REPORT_UNKNOWN_DETAIL.to_vec(),
                    ..ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid)
                }
            } else {
                owner(error.code().as_str(), error.code().numeric())
            }
        })?;
        self.counted(encode_execution_report(id, &preimage)?, 1)
    }
}

/// Maps an S20-290 report validation failure to its owner's code and symbol.
fn report_validation_owner(error: &sley_conformance::ReportValidationError) -> ProtocolFailure {
    use sley_conformance::ReportValidationError as Validation;
    use sley_vm::ExecutionError as Exec;
    let symbol = error.to_string();
    let numeric = match error {
        Validation::Type(inner) | Validation::Execution(Exec::Type(inner)) => {
            inner.code().numeric()
        }
        Validation::Fingerprint(inner) | Validation::Execution(Exec::Fingerprint(inner)) => {
            inner.code().numeric()
        }
        Validation::Lower(_) | Validation::Execution(Exec::Lowering(_)) => 0,
        Validation::Execution(Exec::Status(inner)) => inner.numeric(),
        Validation::Execution(Exec::Exec(inner)) => inner.numeric(),
        Validation::Report(inner) => inner.code().numeric(),
    };
    owner(&symbol, numeric)
}

fn decode_pins(body: &[u8]) -> Result<Vec<RetentionTarget>> {
    let fields = record(body, 1)?;
    let items = list(fields[0])?;
    if items.len() > MAX_GC_PINS {
        return protocol_failure(ProtocolErrorCode::LimitExceeded);
    }
    items
        .into_iter()
        .map(|item| {
            let mut offset = 0;
            let tag = uvar_at(item, &mut offset)?;
            let length = usize::try_from(uvar_at(item, &mut offset)?)
                .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?;
            let payload = item
                .get(offset..offset + length)
                .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::PayloadInvalid))?;
            if offset + length != item.len() {
                return protocol_failure(ProtocolErrorCode::PayloadInvalid);
            }
            let bytes = fixed32(payload)?;
            match tag {
                PIN_STATE_ROOT => Ok(RetentionTarget::StateRoot(StateRoot::from_bytes(bytes))),
                PIN_OBJECT => Ok(RetentionTarget::Object(ObjectId::from_bytes(bytes))),
                _ => protocol_failure(ProtocolErrorCode::PayloadInvalid),
            }
        })
        .collect()
}

/// Decodes the appendix C `limits` record, including the revision 8 cache
/// profile selector in field 6.
fn decode_execution_limits(body: &[u8]) -> Result<(ExecutionLimits, CacheProfile)> {
    let fields = record(body, 6)?;
    let mut offset = 0;
    let tag = uvar_at(fields[4], &mut offset)?;
    let length = uvar_at(fields[4], &mut offset)?;
    let cancel_at_fuel = match (tag, length) {
        (0, 0) if offset == fields[4].len() => None,
        (1, _) => Some(single_uvar(&fields[4][offset..])?),
        _ => return protocol_failure(ProtocolErrorCode::PayloadInvalid),
    };
    let profile = match single_uvar(fields[5])? {
        1 => CacheProfile::RESTRICTED_V1,
        2 => CacheProfile::EXTENDED_V1,
        _ => return protocol_failure(ProtocolErrorCode::PayloadInvalid),
    };
    Ok((
        ExecutionLimits {
            max_instructions: single_uvar(fields[0])?,
            max_fuel: single_uvar(fields[1])?,
            max_value_units: single_uvar(fields[2])?,
            max_output_units: single_uvar(fields[3])?,
            cancel_at_fuel,
        },
        profile,
    ))
}

fn id_list<T: AsRef<[u8]>>(items: &[T]) -> Result<Vec<u8>> {
    let encoded: Vec<Vec<u8>> = items.iter().map(|item| item.as_ref().to_vec()).collect();
    encode_list(&encoded)
        .map_err(|_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant))
}

fn encode_gc_report(report: &GcReport) -> Result<Vec<u8>> {
    let invariant = |_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant);
    let anchors: Vec<Vec<u8>> = report
        .examined_anchors
        .iter()
        .map(|key| {
            encode_record(&[
                (1, encode_uvar(key.kind as u64)),
                (2, key.anchor_id.to_vec()),
            ])
            .map_err(invariant)
        })
        .collect::<Result<_>>()?;
    let decision = match report.decision {
        GcDecision::DryRun => 1,
        GcDecision::Collected => 2,
        GcDecision::PartialDeleteFailure => 3,
    };
    let failed = match report.failed_object {
        None => encode_union(0, &[]),
        Some(object) => encode_union(1, object.as_bytes()),
    }
    .map_err(invariant)?;
    encode_record(&[
        (1, encode_list(&anchors).map_err(invariant)?),
        (
            2,
            id_list(
                &report
                    .retained_roots
                    .iter()
                    .map(|root| *root.as_bytes())
                    .collect::<Vec<_>>(),
            )?,
        ),
        (
            3,
            id_list(
                &report
                    .reachable_objects
                    .iter()
                    .map(|id| *id.as_bytes())
                    .collect::<Vec<_>>(),
            )?,
        ),
        (
            4,
            id_list(
                &report
                    .inventory_objects
                    .iter()
                    .map(|id| *id.as_bytes())
                    .collect::<Vec<_>>(),
            )?,
        ),
        (
            5,
            id_list(
                &report
                    .deletion_candidates
                    .iter()
                    .map(|id| *id.as_bytes())
                    .collect::<Vec<_>>(),
            )?,
        ),
        (6, encode_uvar(report.inventory_bytes)),
        (7, encode_uvar(report.candidate_bytes)),
        (8, encode_uvar(decision)),
        (
            9,
            id_list(
                &report
                    .deleted_objects
                    .iter()
                    .map(|id| *id.as_bytes())
                    .collect::<Vec<_>>(),
            )?,
        ),
        (10, failed),
    ])
    .map_err(invariant)
}

fn encode_execution_report(id: ExecutionReportId, preimage: &[u8]) -> Result<Vec<u8>> {
    let invariant = |_| ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant);
    encode_record(&[
        (1, id.as_bytes().to_vec()),
        (2, encode_bytes(preimage).map_err(invariant)?),
    ])
    .map_err(invariant)
}

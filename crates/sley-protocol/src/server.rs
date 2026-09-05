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
    EntityId, ExecutionReportId, ObjectId, ProtocolHandshakeId, SchemaEpochId, StateRoot,
    TransactionId,
};
use sley_id::{PrincipalId, ReceiptId};
use sley_mutate::{
    build_candidate, decode_candidate_record, decode_const_value, import_candidate,
    import_entity_object,
};
use sley_policy::{
    CandidateValidationContext, CandidateValidationLimits, conformance_registry as policy_registry,
    import_policy_root, validate_candidate_bytes,
};
use sley_query::{
    Cursor, ImpactEdge, ImpactKind, IndexCompleteness, ModeledEntityKind, QueryLimits,
    RestrictedQuery, RootQuery, SnapshotContext, build_index_snapshot,
    build_restricted_query_request, execute_restricted_query,
};
use sley_repo::{
    BranchName, BranchRepository, BranchUpdateStatus, CompleteRootRequest, GcDecision, GcReport,
    IndexCacheError, MergeCommitInput, MergeOutcome, MergeSide, ReportStoreErrorCode,
    RepositoryObjectVerifier, RepositoryQueryError, RetentionAnchor, RetentionKind,
    RetentionSnapshot, RetentionTarget, acquire_exclusive_gc, build_merge_plan, commit_merge,
    compare_complete_roots, export_repository_exchange, gc_collect, gc_dry_run,
    import_repository_exchange, judge_merge_verified, read_execution_report, run_root_query,
    store_execution_report, transaction_ancestry,
};
use sley_scb1::{encode_bytes, encode_list, encode_record, encode_union, encode_uvar};
use sley_state_root::conformance_epoch_id as state_epoch_id;
use sley_state_root::{
    AcceptedStateRoot, conformance_registry as state_registry, import_state_root,
};
use sley_store::ObjectStore;
use sley_txn::{CommitInput, TransactionRepository, TrustedGenesisInput, VerifiedRevision};
use sley_vm::{CacheProfile, ExecutionLimits, ExecutionRequest, LoweringInput, execute_function};

use crate::session::{
    CapsuleBindError, HeadBinding, SessionAuthority, SessionError, fresh_server_nonce,
};
use crate::{
    BoundedContext, DecodedFrame, EncodedFrame, FEATURE_CANCEL, FEATURE_EXTENDED_EXECUTE,
    FEATURE_STREAM, FLAG_CANCEL, FLAG_FAILED, FrameKind, Hello, LimitProfile, Method,
    PROTOCOL_VERSION, ProtocolError, ProtocolErrorCode, ProtocolFailure, ProtocolFrame,
    RequestRegistry, Retryability, SelectedProfile, SessionId, decode_frame, encode_frame,
    negotiate_identity, stream_response,
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

/// The deterministic server over one repository.
#[derive(Debug)]
pub struct Server {
    repository: PathBuf,
    profile: SelectedProfile,
    handshake_id: ProtocolHandshakeId,
    registry: RequestRegistry,
    authority: SessionAuthority,
    /// Bytes still available to each session under the negotiated
    /// `max_work` budget (contract appendix B).
    budgets: BTreeMap<SessionId, u64>,
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

    #[must_use]
    pub fn repository(&self) -> &Path {
        &self.repository
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
            match decode_frame(request_bytes, self.profile.limits.max_frame_bytes) {
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
                let failure =
                    ProtocolFailure::protocol(ProtocolErrorCode::LimitExceeded).encode()?;
                let refused = ProtocolFrame {
                    protocol_version: PROTOCOL_VERSION,
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
                return Ok(Answer {
                    session,
                    request_id,
                    method,
                    failed: true,
                    frame: encode_frame(&refused)?,
                    events: Vec::new(),
                });
            }
        }
        let frame = ProtocolFrame {
            protocol_version: PROTOCOL_VERSION,
            session,
            request_id,
            kind: FrameKind::Response,
            method,
            flags: if failed { FLAG_FAILED } else { 0 },
            bounds,
            body,
        };
        let stream_negotiated = self.profile.features & FEATURE_STREAM != 0;
        let mut frames = match stream_response(
            &frame,
            self.profile.limits.max_frame_bytes,
            stream_negotiated,
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
                    frame: encode_frame(&refused)?,
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
        let method = Method::from_tag(frame.method)
            .map_err(|error| ProtocolFailure::protocol(error.code()))?;
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
        if !self.profile.admits(method) || method.is_reserved() {
            // A reserved tag names a real seam whose owner has not claimed
            // it yet (contract section 4): the detail names the seam, and
            // the failure is retryable after the capability appears.
            if method.is_reserved() {
                let mut failure = unsupported(reserved_detail(method));
                failure.retryability = Retryability::AfterCapability;
                return Err(failure);
            }
            return Err(unsupported(b"SMP1-METHOD-NOT-NEGOTIATED"));
        }
        let body = frame.body.as_slice();
        match method {
            Method::SessionOpen => protocol_failure(ProtocolErrorCode::InternalInvariant),
            Method::SessionRenew => {
                // The renew body names the session being renewed; the
                // frame's session scopes the request. They must agree
                // (contract section 2).
                if fixed32(body)? != *session.as_bytes() {
                    return protocol_failure(ProtocolErrorCode::FrameInvalid);
                }
                let (head, binding) = self.head_binding()?;
                let record = self
                    .authority
                    .renew_session(session, &binding, head.state_root())
                    .map_err(session_failure)?;
                self.plain(record.session_id.as_bytes().to_vec())
            }
            Method::SessionClose => {
                self.registry
                    .close(session, self.profile.limits.max_sessions)
                    .map_err(|error| ProtocolFailure::protocol(error.code()))?;
                self.authority
                    .close_session(session)
                    .map_err(session_failure)?;
                self.budgets.remove(&session);
                self.plain(Vec::new())
            }
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
            Method::Checkout => self.checkout(body),
            Method::Recovery => self.recovery(),
            Method::Cancel => {
                // Every request completes before the next frame is read, so
                // the named request has already been answered.
                let _ = single_uvar(body)?;
                self.plain(Vec::new())
            }
            Method::WorkspaceCreate => self.workspace_create(body),
            Method::WorkspaceOpen => self.workspace_open(),
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
            | Method::TestsSelected
            | Method::TestsAffected => {
                let mut failure = unsupported(reserved_detail(method));
                failure.retryability = Retryability::AfterCapability;
                Err(failure)
            }
        }
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

    fn session_check(&self, session: SessionId, method: Method) -> Result<()> {
        let (_, binding) = self.head_binding()?;
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
        let (head, binding) = self.head_binding()?;
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

    fn branches(&self) -> BranchRepository {
        BranchRepository::new(&self.repository)
    }

    fn head(&self) -> Result<VerifiedRevision> {
        self.transactions()
            .accepted_head()
            .map(|head| head.verified_revision().clone())
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

    fn checkout(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
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
        let outcome = run_root_query(
            &self.repository,
            &revision,
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
        let outcome = run_root_query(
            &self.repository,
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

    fn workspace_open(&self) -> Result<(Vec<u8>, BoundedContext)> {
        let head = self.head()?;
        let summary = revision_summary(&head)?;
        self.counted(summary, 1)
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

    fn commit(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
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
        self.counted(record, 1)
    }

    fn merge_commit(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
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
                self.counted(scb(encode_union(1, transaction_id.as_bytes()))?, 1)
            }
        }
    }

    fn exchange_import(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
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
    let record = &revision.state_root().record;
    scb(encode_record(&[
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
    ]))
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

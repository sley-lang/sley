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

use sley_id::{EntityId, ProtocolHandshakeId, SchemaEpochId, StateRoot, TransactionId};
use sley_id::{PrincipalId, ReceiptId};
use sley_mutate::{
    build_candidate, decode_candidate_record, import_candidate, import_entity_object,
};
use sley_policy::{
    CandidateValidationContext, CandidateValidationLimits, conformance_registry as policy_registry,
    import_policy_root, validate_candidate_bytes,
};
use sley_query::{
    Cursor, ImpactEdge, ImpactKind, IndexCompleteness, ModeledEntityKind, QueryLimits,
    RestrictedQuery, RootQuery, SnapshotContext, build_context_capsule_bound, build_index_snapshot,
    build_restricted_query_request, execute_restricted_query,
};
use sley_repo::{
    BranchName, BranchRepository, BranchUpdateStatus, CompleteRootRequest, IndexCacheError,
    MergeCommitInput, MergeOutcome, MergeSide, RepositoryQueryError, build_merge_plan,
    commit_merge, compare_complete_roots, export_repository_exchange, import_repository_exchange,
    judge_merge, run_root_query,
};
use sley_scb1::{encode_bytes, encode_list, encode_record, encode_union, encode_uvar};
use sley_state_root::conformance_epoch_id as state_epoch_id;
use sley_state_root::{conformance_registry as state_registry, import_state_root};
use sley_txn::{CommitInput, TransactionRepository, TrustedGenesisInput, VerifiedRevision};

use crate::session::{HeadBinding, SessionAuthority, SessionError};
use crate::{
    BoundedContext, DecodedFrame, EncodedFrame, FEATURE_CANCEL, FEATURE_STREAM, FLAG_CANCEL,
    FLAG_FAILED, FrameKind, Hello, LimitProfile, Method, PROTOCOL_VERSION, ProtocolError,
    ProtocolErrorCode, ProtocolFailure, ProtocolFrame, RequestRegistry, Retryability,
    SelectedProfile, SessionId, decode_frame, encode_frame, stream_response,
};

/// Versioned reason carried by `PROTOCOL_METHOD_UNSUPPORTED` for methods
/// whose dispatch is a later slice.
pub const DEFERRED_DISPATCH_REASON: &[u8] = b"S20-410-SLICE-C-DEFERRED";
/// Reason carried for reserved methods.
pub const RESERVED_METHOD_REASON: &[u8] = b"SMP1-RESERVED-METHOD";

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

fn owner(symbol: &str, numeric: u32) -> ProtocolFailure {
    let retryability = match symbol {
        s if s.ends_with("RESOURCE_LIMIT") || s.ends_with("REQUIRED_FACT_OMITTED") => {
            Retryability::AfterLimitChange
        }
        s if s.ends_with("STALE") || s.ends_with("NOT_FAST_FORWARD") => Retryability::AfterRequery,
        _ => Retryability::Never,
    };
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
    /// Creates a server over a repository under a negotiated profile.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_INTERNAL_INVARIANT` when the profile cannot be digested.
    pub fn new(
        repository: impl Into<PathBuf>,
        profile: SelectedProfile,
    ) -> core::result::Result<Self, ProtocolError> {
        let handshake_id = profile.handshake_id()?;
        Ok(Self {
            repository: repository.into(),
            profile,
            handshake_id,
            registry: RequestRegistry::new(),
            authority: SessionAuthority::new(handshake_id),
            budgets: BTreeMap::new(),
        })
    }

    /// The hello this server offers to an endpoint (S20-430): protocol
    /// version 1, the frozen conformance schema epoch, the limit ceilings,
    /// every method the server dispatches (reserved and deferred methods
    /// are not offered), the cancel and stream features, and no adapters
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
                .filter(|method| !method.is_reserved() && !Self::is_deferred(*method))
                .map(Method::tag)
                .collect(),
            features: FEATURE_CANCEL | FEATURE_STREAM,
            adapters: Vec::new(),
            effects: Vec::new(),
        };
        hello.validate()?;
        Ok(hello)
    }

    /// Methods whose dispatch is a later slice (`DEFERRED_DISPATCH_REASON`).
    #[must_use]
    pub const fn is_deferred(method: Method) -> bool {
        matches!(
            method,
            Method::GcDryRun | Method::GcCollect | Method::Execute | Method::Report
        )
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
            answers.push(self.respond(frame.session, frame.request_id, frame.method, outcome)?);
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
        if frame.protocol_version != self.profile.protocol_version {
            return protocol_failure(ProtocolErrorCode::VersionUnsupported);
        }
        let method = Method::from_tag(frame.method)
            .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        if method == Method::SessionOpen {
            if frame.session.is_some() {
                return protocol_failure(ProtocolErrorCode::FrameInvalid);
            }
            return self.session_open(&frame.body);
        }
        // A repository without an accepted head cannot bind a session, so
        // the two methods that create one may travel without a session
        // (contract section 3); under a session they are checked normally.
        if frame.session.is_none()
            && matches!(method, Method::WorkspaceCreate | Method::ExchangeImport)
        {
            if !self.profile.admits(method) {
                return Err(unsupported(b"SMP1-METHOD-NOT-NEGOTIATED"));
            }
            return match method {
                Method::WorkspaceCreate => self.workspace_create(&frame.body),
                _ => self.exchange_import(&frame.body),
            };
        }
        let Some(session) = frame.session else {
            return protocol_failure(ProtocolErrorCode::SessionClosed);
        };
        self.registry
            .admit(session, frame.request_id, self.profile.limits.max_inflight)
            .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        if self.budgets.get(&session).copied().unwrap_or(0) == 0 {
            self.registry
                .complete(session)
                .map_err(|error| ProtocolFailure::protocol(error.code()))?;
            return protocol_failure(ProtocolErrorCode::LimitExceeded);
        }
        let outcome = match self.session_check(session, method) {
            Ok(()) => self.dispatch_admitted(session, method, frame),
            Err(failure) => Err(failure),
        };
        // Synchronous server: the request completes before the next frame.
        if self.registry.is_open(session) {
            self.registry
                .complete(session)
                .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        }
        if let Ok((body, _)) = &outcome {
            // One unit per request plus one per returned byte, never below zero.
            let charge = to_u64(body.len())?.saturating_add(1);
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
            return Err(unsupported(if method.is_reserved() {
                RESERVED_METHOD_REASON
            } else {
                b"SMP1-METHOD-NOT-NEGOTIATED"
            }));
        }
        let body = frame.body.as_slice();
        match method {
            Method::SessionOpen => protocol_failure(ProtocolErrorCode::InternalInvariant),
            Method::SessionRenew => {
                let (_, binding) = self.head_binding()?;
                let record = self
                    .authority
                    .renew_session(session, &binding)
                    .map_err(session_failure)?;
                self.plain(record.session_id.as_bytes().to_vec())
            }
            Method::SessionClose => {
                self.registry
                    .close(session)
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
            Method::GcDryRun | Method::GcCollect | Method::Execute | Method::Report => {
                Err(unsupported(DEFERRED_DISPATCH_REASON))
            }
            Method::Diagnostics
            | Method::RefMoveProtected
            | Method::TestsSelected
            | Method::TestsAffected => Err(unsupported(RESERVED_METHOD_REASON)),
        }
    }

    /// Head-bound methods answer over the accepted head without naming it
    /// (contract section 3). `handle.expand` performs the same root check
    /// itself and reports `SESSION_STALE_HANDLE`.
    const fn head_bound(method: Method) -> bool {
        matches!(
            method,
            Method::WorkspaceOpen
                | Method::QueryRoot
                | Method::QueryContinue
                | Method::Capsule
                | Method::QueryRestricted
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
        if matches!(method, Method::SessionRenew | Method::SessionClose) {
            return Ok(());
        }
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
        let (_, binding) = self.head_binding()?;
        let record = self
            .authority
            .open_session(&binding)
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
        let outcome = judge_merge(&ancestor, &ours, &theirs).map_err(|error| {
            owner(
                &error.symbol(),
                error.code().map_or(0, sley_repo::MergeErrorCode::numeric),
            )
        })?;
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
        let handle = single_uvar(body)?;
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
            .expand_handle(session, &binding, request.bound_objects(), &kinds, handle)
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
        let record = self.authority.record(session).copied().ok_or_else(|| {
            session_failure(SessionError::new(crate::session::SessionErrorCode::Unknown))
        })?;
        let capsule = build_context_capsule_bound(
            &outcome.request,
            &outcome.response,
            session,
            record.workspace_id,
            record.bound_root,
            record.schema_epoch,
        )
        .map_err(|error| owner(error.code().as_str(), error.code().numeric()))?;
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
                error.code().map_or(0, sley_repo::MergeErrorCode::numeric),
            )
        };
        let outcome = judge_merge(&ancestor, &ours, &theirs).map_err(merge_failure)?;
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

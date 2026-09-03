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

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sley_id::{EntityId, ProtocolHandshakeId, SchemaEpochId, StateRoot, TransactionId};
use sley_mutate::import_entity_object;
use sley_query::{
    Cursor, ImpactEdge, ImpactKind, IndexCompleteness, ModeledEntityKind, QueryLimits,
    RestrictedQuery, RootQuery, SnapshotContext, build_index_snapshot,
    build_restricted_query_request, execute_restricted_query,
};
use sley_repo::{
    BranchName, BranchRepository, BranchUpdateStatus, CompleteRootRequest, IndexCacheError,
    MergeOutcome, MergeSide, RepositoryQueryError, compare_complete_roots,
    export_repository_exchange, judge_merge, run_context_capsule, run_root_query,
};
use sley_scb1::{encode_bytes, encode_list, encode_record, encode_union, encode_uvar};
use sley_state_root::conformance_epoch_id as state_epoch_id;
use sley_txn::{TransactionRepository, VerifiedRevision};

use crate::{
    BoundedContext, DecodedFrame, EncodedFrame, FrameKind, LimitProfile, Method, PROTOCOL_VERSION,
    ProtocolError, ProtocolErrorCode, ProtocolFailure, ProtocolFrame, RequestRegistry,
    Retryability, SelectedProfile, SessionId, decode_frame, encode_frame,
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
    sessions_issued: u64,
    open_sessions: BTreeSet<SessionId>,
}

/// One answered frame and the request identity it belongs to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Answer {
    pub session: Option<SessionId>,
    pub request_id: u64,
    pub method: u32,
    pub failed: bool,
    pub frame: EncodedFrame,
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
            sessions_issued: 0,
            open_sessions: BTreeSet::new(),
        })
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
        let frame = match decode_frame(request_bytes, self.profile.limits.max_frame_bytes) {
            Ok((DecodedFrame::Request(frame), _)) => frame,
            Ok(_) => {
                return self.respond(
                    None,
                    0,
                    0,
                    Err(ProtocolFailure::protocol(ProtocolErrorCode::FrameInvalid)),
                );
            }
            Err(error) => {
                return self.respond(None, 0, 0, Err(ProtocolFailure::protocol(error.code())));
            }
        };
        let outcome = self.dispatch(&frame);
        self.respond(frame.session, frame.request_id, frame.method, outcome)
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
            flags: 0,
            bounds,
            body,
        };
        let encoded = encode_frame(&frame)?;
        Ok(Answer {
            session,
            request_id,
            method,
            failed,
            frame: encoded,
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
        let Some(session) = frame.session else {
            return protocol_failure(ProtocolErrorCode::SessionClosed);
        };
        self.registry
            .admit(session, frame.request_id, self.profile.limits.max_inflight)
            .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        let outcome = self.dispatch_admitted(session, method, frame);
        // Synchronous server: the request completes before the next frame.
        if self.registry.is_open(session) {
            self.registry
                .complete(session)
                .map_err(|error| ProtocolFailure::protocol(error.code()))?;
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
            Method::SessionRenew => self.plain(session.as_bytes().to_vec()),
            Method::SessionClose => {
                self.registry
                    .close(session)
                    .map_err(|error| ProtocolFailure::protocol(error.code()))?;
                self.open_sessions.remove(&session);
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
                let limits = encode_limits(&self.profile.limits)?;
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
            Method::Capsule => self.capsule(body),
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
            Method::WorkspaceCreate
            | Method::WorkspaceOpen
            | Method::MergeCommit
            | Method::ExchangeImport
            | Method::GcDryRun
            | Method::GcCollect
            | Method::CandidateCreate
            | Method::CandidateAppend
            | Method::CandidateValidate
            | Method::CandidateInspect
            | Method::CandidateDiscard
            | Method::Commit
            | Method::Execute
            | Method::Report => Err(unsupported(DEFERRED_DISPATCH_REASON)),
            Method::HandleExpand
            | Method::Diagnostics
            | Method::RefMoveProtected
            | Method::TestsSelected
            | Method::TestsAffected => Err(unsupported(RESERVED_METHOD_REASON)),
        }
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
        self.sessions_issued = self
            .sessions_issued
            .checked_add(1)
            .ok_or_else(|| ProtocolFailure::protocol(ProtocolErrorCode::LimitExceeded))?;
        let mut preimage = Vec::with_capacity(48);
        preimage.extend_from_slice(b"session:");
        preimage.extend_from_slice(self.handshake_id.as_bytes());
        preimage.extend_from_slice(&self.sessions_issued.to_be_bytes());
        let session = SessionId::from_bytes(*ProtocolHandshakeId::derive(&preimage).as_bytes());
        self.registry
            .open(session)
            .map_err(|error| ProtocolFailure::protocol(error.code()))?;
        self.open_sessions.insert(session);
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

    fn capsule(&self, body: &[u8]) -> Result<(Vec<u8>, BoundedContext)> {
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
        let (capsule, _) = run_context_capsule(
            &self.repository,
            &revision,
            decoded.query,
            decoded.limits,
            decoded.allow_continuation,
            decoded.after,
        )
        .map_err(|error| repository_query_failure(&error))?;
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

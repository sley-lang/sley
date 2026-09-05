//! Negotiated session authority and positional handles (S20-330, contract
//! `docs/spec/SESSION_HANDLE_PROFILE_V1.md`, ADR-0033).
//!
//! A session binds one negotiated handshake to one workspace, one verified
//! root, and one schema epoch. Every request is checked against that
//! binding in contract order, and a handle is the binding position of an
//! entity in the session's bound root, so it resolves only under that
//! session and root.

// Variant, field, and getter names are the contract's own names; the
// contract text is their documentation.
#![allow(missing_docs)]

use core::fmt;
use std::collections::BTreeMap;

use sley_id::{
    EntityId, ObjectId, ProtocolHandshakeId, SchemaEpochId, SessionId, StateRoot, WorkspaceId,
};
use sley_query::{
    ContextCapsule, ContextCapsuleError, RootQueryRequest, RootQueryResponse,
    build_context_capsule_session,
};
use sley_state_root::AcceptedStateRoot;

/// Renewals a session may perform (contract section 2).
pub const MAX_SESSION_RENEWALS: u32 = 65_535;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionErrorCode {
    Unknown,
    WorkspaceMismatch,
    RootAdvanced,
    EpochMismatch,
    StaleHandle,
    HandleUnknown,
    RenewalLimit,
    BindingInvalid,
}

impl SessionErrorCode {
    pub const ALL: [Self; 8] = [
        Self::Unknown,
        Self::WorkspaceMismatch,
        Self::RootAdvanced,
        Self::EpochMismatch,
        Self::StaleHandle,
        Self::HandleUnknown,
        Self::RenewalLimit,
        Self::BindingInvalid,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "SESSION_UNKNOWN",
            Self::WorkspaceMismatch => "SESSION_WORKSPACE_MISMATCH",
            Self::RootAdvanced => "SESSION_ROOT_ADVANCED",
            Self::EpochMismatch => "SESSION_EPOCH_MISMATCH",
            Self::StaleHandle => "SESSION_STALE_HANDLE",
            Self::HandleUnknown => "SESSION_HANDLE_UNKNOWN",
            Self::RenewalLimit => "SESSION_RENEWAL_LIMIT",
            Self::BindingInvalid => "SESSION_BINDING_INVALID",
        }
    }

    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::Unknown => 33_000,
            Self::WorkspaceMismatch => 33_001,
            Self::RootAdvanced => 33_002,
            Self::EpochMismatch => 33_003,
            Self::StaleHandle => 33_004,
            Self::HandleUnknown => 33_005,
            Self::RenewalLimit => 33_006,
            Self::BindingInvalid => 33_007,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionError(SessionErrorCode);

impl SessionError {
    #[must_use]
    pub const fn new(code: SessionErrorCode) -> Self {
        Self(code)
    }

    #[must_use]
    pub const fn code(&self) -> SessionErrorCode {
        self.0
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for SessionError {}

/// Failure to mint a session-bound context capsule (capsule contract
/// section 2): the session is not live under this authority, or the
/// capsule builder refused the bound source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapsuleBindError {
    /// The session was never issued or is already closed. Liveness is
    /// checked against the authority's live map, so a copied record of a
    /// dead session cannot mint a capsule.
    UnknownSession,
    /// The session is live but the capsule builder refused the source:
    /// response provenance differs from the session's authority-held
    /// binding, or the request and response disagree.
    Capsule(ContextCapsuleError),
}

impl fmt::Display for CapsuleBindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSession => formatter.write_str("SESSION_UNKNOWN"),
            Self::Capsule(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for CapsuleBindError {}

fn fail<T>(code: SessionErrorCode) -> Result<T, SessionError> {
    Err(SessionError(code))
}

/// The facts of a repository's accepted head that a session binds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeadBinding {
    pub workspace_id: WorkspaceId,
    pub root: StateRoot,
    pub schema_epoch: SchemaEpochId,
}

/// One issued session (contract section 2).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionRecord {
    pub session_id: SessionId,
    pub handshake_id: ProtocolHandshakeId,
    pub workspace_id: WorkspaceId,
    pub bound_root: StateRoot,
    pub schema_epoch: SchemaEpochId,
    pub issue_ordinal: u64,
    pub renewals: u32,
}

/// The expanded fact of a handle (contract section 4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandleFacts {
    pub entity: EntityId,
    pub kind: u32,
    pub object_id: ObjectId,
    pub bound_root: StateRoot,
    pub session_id: SessionId,
}

/// Derives the session identity (contract section 1).
#[must_use]
pub fn derive_session_id(
    handshake_id: ProtocolHandshakeId,
    head: &HeadBinding,
    issue_ordinal: u64,
) -> SessionId {
    let mut preimage = Vec::with_capacity(136);
    preimage.extend_from_slice(handshake_id.as_bytes());
    preimage.extend_from_slice(head.workspace_id.as_bytes());
    preimage.extend_from_slice(head.root.as_bytes());
    preimage.extend_from_slice(head.schema_epoch.as_bytes());
    preimage.extend_from_slice(&issue_ordinal.to_be_bytes());
    SessionId::derive(&preimage)
}

/// The session authority of one server over one repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionAuthority {
    handshake_id: ProtocolHandshakeId,
    issued: u64,
    sessions: BTreeMap<SessionId, SessionRecord>,
    /// Full root records bound by live sessions. `gc` catalogs exactly
    /// these alongside the head and branch revisions, so a session bound
    /// to a root no branch targets stays retained while the session is
    /// live (contract appendix C, threat T15). Entries no live session
    /// binds are pruned on renew and close; over-retention is impossible
    /// by construction, under-retention by the prune rule below.
    retained: BTreeMap<StateRoot, AcceptedStateRoot>,
}

impl SessionAuthority {
    #[must_use]
    pub const fn new(handshake_id: ProtocolHandshakeId) -> Self {
        Self {
            handshake_id,
            issued: 0,
            sessions: BTreeMap::new(),
            retained: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn handshake_id(&self) -> ProtocolHandshakeId {
        self.handshake_id
    }

    #[must_use]
    pub fn record(&self, session: SessionId) -> Option<&SessionRecord> {
        self.sessions.get(&session)
    }

    /// Mints the session-bound context capsule (capsule contract section
    /// 2): the only supported path to the `Negotiated` arm. The session
    /// must be live under this authority, and the response's workspace,
    /// root, and epoch must equal the session's authority-held binding;
    /// both facts come from authority state, never from caller arguments,
    /// so no caller can declare a binding into the capsule identity.
    ///
    /// # Errors
    ///
    /// Returns `UnknownSession` for a session this authority never issued
    /// or already closed, and the builder's `CONTEXT_CAPSULE_*` failure
    /// otherwise.
    pub fn bind_context_capsule(
        &self,
        session: SessionId,
        request: &RootQueryRequest,
        response: &RootQueryResponse,
    ) -> Result<ContextCapsule, CapsuleBindError> {
        let record = self
            .sessions
            .get(&session)
            .ok_or(CapsuleBindError::UnknownSession)?;
        if response.workspace_id() != record.workspace_id
            || response.root() != record.bound_root
            || response.schema_epoch() != record.schema_epoch
        {
            return Err(CapsuleBindError::Capsule(ContextCapsuleError::new(
                sley_query::ContextCapsuleErrorCode::SourceInvalid,
            )));
        }
        build_context_capsule_session(request, response, record.session_id)
            .map_err(CapsuleBindError::Capsule)
    }

    /// Every live session with its bound root, in session order. `gc`
    /// derives one `SessionPin` anchor per entry so one session never
    /// collects another session's bound root (contract appendix C,
    /// threat T15).
    #[must_use]
    pub fn live_pins(&self) -> Vec<(SessionId, StateRoot)> {
        self.sessions
            .values()
            .map(|record| (record.session_id, record.bound_root))
            .collect()
    }

    /// Full root records bound by live sessions, for the `gc` root
    /// catalog (contract appendix C). A session bound to a root no
    /// branch targets stays importable while the session is live.
    #[must_use]
    pub fn retained_roots(&self) -> Vec<AcceptedStateRoot> {
        self.retained.values().cloned().collect()
    }

    /// Issues a session bound to the accepted head (contract section 2).
    ///
    /// # Errors
    ///
    /// Returns `SESSION_BINDING_INVALID` when the ordinal space is exhausted,
    /// the identity is already issued, or the retained record is not the
    /// bound root's own record.
    pub fn open_session(
        &mut self,
        head: &HeadBinding,
        root_record: &AcceptedStateRoot,
    ) -> Result<SessionRecord, SessionError> {
        if root_record.root != head.root {
            return fail(SessionErrorCode::BindingInvalid);
        }
        let issue_ordinal = self
            .issued
            .checked_add(1)
            .ok_or(SessionError(SessionErrorCode::BindingInvalid))?;
        let session_id = derive_session_id(self.handshake_id, head, issue_ordinal);
        if self.sessions.contains_key(&session_id) {
            return fail(SessionErrorCode::BindingInvalid);
        }
        let record = SessionRecord {
            session_id,
            handshake_id: self.handshake_id,
            workspace_id: head.workspace_id,
            bound_root: head.root,
            schema_epoch: head.schema_epoch,
            issue_ordinal,
            renewals: 0,
        };
        self.issued = issue_ordinal;
        self.sessions.insert(session_id, record);
        self.retained.insert(head.root, root_record.clone());
        Ok(record)
    }

    /// Rebinds a session to the current accepted head (contract section 2).
    ///
    /// # Errors
    ///
    /// Returns `SESSION_UNKNOWN`, `SESSION_WORKSPACE_MISMATCH`,
    /// `SESSION_EPOCH_MISMATCH`, `SESSION_RENEWAL_LIMIT`, or
    /// `SESSION_BINDING_INVALID` when the retained record is not the new
    /// bound root's own record.
    pub fn renew_session(
        &mut self,
        session: SessionId,
        head: &HeadBinding,
        root_record: &AcceptedStateRoot,
    ) -> Result<SessionRecord, SessionError> {
        if root_record.root != head.root {
            return fail(SessionErrorCode::BindingInvalid);
        }
        let bound = {
            let record = self
                .sessions
                .get_mut(&session)
                .ok_or(SessionError(SessionErrorCode::Unknown))?;
            if record.workspace_id != head.workspace_id {
                return fail(SessionErrorCode::WorkspaceMismatch);
            }
            if record.schema_epoch != head.schema_epoch {
                return fail(SessionErrorCode::EpochMismatch);
            }
            if record.renewals >= MAX_SESSION_RENEWALS {
                return fail(SessionErrorCode::RenewalLimit);
            }
            record.renewals += 1;
            record.bound_root = head.root;
            *record
        };
        self.retained.insert(head.root, root_record.clone());
        self.prune_retained();
        Ok(bound)
    }

    /// Closes a session.
    ///
    /// # Errors
    ///
    /// Returns `SESSION_UNKNOWN`.
    pub fn close_session(&mut self, session: SessionId) -> Result<(), SessionError> {
        self.sessions
            .remove(&session)
            .map(|_| ())
            .ok_or(SessionError(SessionErrorCode::Unknown))?;
        self.prune_retained();
        Ok(())
    }

    /// Drops retained roots no live session binds. Sessions only ever add
    /// bindings, so pruning exactly the unbound keeps retention exact:
    /// every live session's root is catalogued, nothing else is.
    fn prune_retained(&mut self) {
        self.retained.retain(|root, _| {
            self.sessions
                .values()
                .any(|record| record.bound_root == *root)
        });
    }

    /// Checks a request against its session's binding in contract order
    /// (section 3).
    ///
    /// # Errors
    ///
    /// Returns the first failing check.
    pub fn check_session(
        &self,
        session: SessionId,
        head: &HeadBinding,
        head_bound: bool,
    ) -> Result<&SessionRecord, SessionError> {
        let record = self
            .sessions
            .get(&session)
            .ok_or(SessionError(SessionErrorCode::Unknown))?;
        if record.workspace_id != head.workspace_id {
            return fail(SessionErrorCode::WorkspaceMismatch);
        }
        if record.schema_epoch != head.schema_epoch {
            return fail(SessionErrorCode::EpochMismatch);
        }
        if head_bound && record.bound_root != head.root {
            return fail(SessionErrorCode::RootAdvanced);
        }
        Ok(record)
    }

    /// Expands a positional handle under its session and root (section 4).
    ///
    /// # Errors
    ///
    /// Returns the binding failures, `SESSION_STALE_HANDLE` when the head
    /// root differs from the bound root, or `SESSION_HANDLE_UNKNOWN`.
    pub fn expand_handle(
        &self,
        session: SessionId,
        head: &HeadBinding,
        bindings: &[(EntityId, ObjectId)],
        kinds: &[u32],
        handle: u64,
    ) -> Result<HandleFacts, SessionError> {
        let record = self.check_session(session, head, false)?;
        if record.bound_root != head.root {
            return fail(SessionErrorCode::StaleHandle);
        }
        let index =
            usize::try_from(handle).map_err(|_| SessionError(SessionErrorCode::HandleUnknown))?;
        let (entity, object_id) = bindings
            .get(index)
            .copied()
            .ok_or(SessionError(SessionErrorCode::HandleUnknown))?;
        let kind = kinds
            .get(index)
            .copied()
            .ok_or(SessionError(SessionErrorCode::BindingInvalid))?;
        Ok(HandleFacts {
            entity,
            kind,
            object_id,
            bound_root: record.bound_root,
            session_id: record.session_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sley_id::PolicyRootId;
    use sley_state_root::StateRootRecord;

    fn head(workspace: u8, root: u8, epoch: u8) -> HeadBinding {
        HeadBinding {
            workspace_id: WorkspaceId::from_bytes([workspace; 32]),
            root: StateRoot::from_bytes([root; 32]),
            schema_epoch: SchemaEpochId::from_bytes([epoch; 32]),
        }
    }

    /// A retained root record for `root`: opaque storage for the
    /// authority (never inspected here); the server retains real
    /// importable records.
    fn retained(root: u8) -> AcceptedStateRoot {
        AcceptedStateRoot {
            root: StateRoot::from_bytes([root; 32]),
            stored_bytes: Vec::new(),
            record: StateRootRecord {
                workspace_id: WorkspaceId::from_bytes([1; 32]),
                schema_epoch_id: SchemaEpochId::from_bytes([0x11; 32]),
                entity_bindings: Vec::new(),
                entry_points: Vec::new(),
                dependency_roots: Vec::new(),
                contract_root: ObjectId::from_bytes([0xA0; 32]),
                test_root: ObjectId::from_bytes([0xB0; 32]),
                policy_root: PolicyRootId::from_bytes([0xC0; 32]),
                interpretation_flags: Vec::new(),
            },
        }
    }

    fn handshake() -> ProtocolHandshakeId {
        ProtocolHandshakeId::from_bytes([0x44; 32])
    }

    #[test]
    fn issuance_is_deterministic_and_binds_the_head() {
        let mut a = SessionAuthority::new(handshake());
        let mut b = SessionAuthority::new(handshake());
        let first = a
            .open_session(&head(1, 0x10, 0x11), &retained(0x10))
            .unwrap();
        let again = b
            .open_session(&head(1, 0x10, 0x11), &retained(0x10))
            .unwrap();
        assert_eq!(first, again);
        assert_eq!(first.issue_ordinal, 1);
        assert_eq!(first.bound_root, StateRoot::from_bytes([0x10; 32]));
        let second = a
            .open_session(&head(1, 0x10, 0x11), &retained(0x10))
            .unwrap();
        assert_ne!(second.session_id, first.session_id);
        assert_eq!(second.issue_ordinal, 2);
        assert_ne!(
            derive_session_id(handshake(), &head(1, 0x10, 0x11), 1),
            derive_session_id(handshake(), &head(1, 0x20, 0x11), 1)
        );
        assert_eq!(a.record(first.session_id).map(|r| r.renewals), Some(0));
    }

    #[test]
    fn checks_follow_contract_order_and_handles_die_with_the_root() {
        let mut authority = SessionAuthority::new(handshake());
        let session = authority
            .open_session(&head(1, 0x10, 0x11), &retained(0x10))
            .unwrap()
            .session_id;
        let bindings = [
            (
                EntityId::from_bytes([1; 32]),
                ObjectId::from_bytes([0x81; 32]),
            ),
            (
                EntityId::from_bytes([2; 32]),
                ObjectId::from_bytes([0x82; 32]),
            ),
        ];
        let kinds = [1, 3];
        // T47: another workspace is refused before anything else.
        assert_eq!(
            authority
                .check_session(session, &head(2, 0x10, 0x11), true)
                .unwrap_err()
                .code(),
            SessionErrorCode::WorkspaceMismatch
        );
        assert_eq!(
            authority
                .check_session(session, &head(1, 0x10, 0x12), true)
                .unwrap_err()
                .code(),
            SessionErrorCode::EpochMismatch
        );
        assert_eq!(
            authority
                .check_session(session, &head(1, 0x20, 0x11), true)
                .unwrap_err()
                .code(),
            SessionErrorCode::RootAdvanced
        );
        assert!(
            authority
                .check_session(session, &head(1, 0x20, 0x11), false)
                .is_ok()
        );
        assert_eq!(
            authority
                .check_session(SessionId::from_bytes([9; 32]), &head(1, 0x10, 0x11), true)
                .unwrap_err()
                .code(),
            SessionErrorCode::Unknown
        );
        // T15: the handle resolves under the bound root, is stale after the
        // root advances, and resolves again after renewal.
        let facts = authority
            .expand_handle(session, &head(1, 0x10, 0x11), &bindings, &kinds, 1)
            .unwrap();
        assert_eq!(facts.entity, EntityId::from_bytes([2; 32]));
        assert_eq!(facts.kind, 3);
        assert_eq!(facts.session_id, session);
        assert_eq!(
            authority
                .expand_handle(session, &head(1, 0x20, 0x11), &bindings, &kinds, 1)
                .unwrap_err()
                .code(),
            SessionErrorCode::StaleHandle
        );
        assert_eq!(
            authority
                .expand_handle(session, &head(1, 0x10, 0x11), &bindings, &kinds, 2)
                .unwrap_err()
                .code(),
            SessionErrorCode::HandleUnknown
        );
        let renewed = authority
            .renew_session(session, &head(1, 0x20, 0x11), &retained(0x20))
            .unwrap();
        assert_eq!(renewed.renewals, 1);
        assert_eq!(renewed.bound_root, StateRoot::from_bytes([0x20; 32]));
        assert!(
            authority
                .expand_handle(session, &head(1, 0x20, 0x11), &bindings, &kinds, 0)
                .is_ok()
        );
        assert_eq!(
            authority
                .renew_session(session, &head(2, 0x20, 0x11), &retained(0x20))
                .unwrap_err()
                .code(),
            SessionErrorCode::WorkspaceMismatch
        );
        authority.close_session(session).unwrap();
        assert_eq!(
            authority.close_session(session).unwrap_err().code(),
            SessionErrorCode::Unknown
        );
        for pair in SessionErrorCode::ALL.windows(2) {
            assert!(pair[0].numeric() < pair[1].numeric());
        }
        assert_eq!(SessionErrorCode::ALL[7].numeric(), 33_007);
    }

    #[test]
    fn retention_pins_exactly_the_live_sessions_roots() {
        let mut authority = SessionAuthority::new(handshake());
        let first = authority
            .open_session(&head(1, 0x10, 0x11), &retained(0x10))
            .unwrap()
            .session_id;
        let second = authority
            .open_session(&head(1, 0x20, 0x11), &retained(0x20))
            .unwrap()
            .session_id;
        let mut pins = authority.live_pins();
        pins.sort();
        let mut expected = vec![
            (first, StateRoot::from_bytes([0x10; 32])),
            (second, StateRoot::from_bytes([0x20; 32])),
        ];
        expected.sort();
        assert_eq!(pins, expected);
        assert_eq!(authority.retained_roots().len(), 2);
        // Renewing the first session onto the second root unbinds 0x10,
        // so it is pruned: retention is exact, never over- or under-held.
        authority
            .renew_session(first, &head(1, 0x20, 0x11), &retained(0x20))
            .unwrap();
        assert_eq!(authority.retained_roots().len(), 1);
        assert_eq!(
            authority.retained_roots()[0].root,
            StateRoot::from_bytes([0x20; 32])
        );
        // Closing the second session keeps 0x20: the first still binds it.
        authority.close_session(second).unwrap();
        assert_eq!(authority.retained_roots().len(), 1);
        // Closing the last session releases everything.
        authority.close_session(first).unwrap();
        assert!(authority.retained_roots().is_empty());
        assert!(authority.live_pins().is_empty());
        // A retained record that is not the bound root's own record is
        // refused at bind time, so retention can never silently cover
        // the wrong root.
        assert_eq!(
            authority
                .open_session(&head(1, 0x10, 0x11), &retained(0x20))
                .unwrap_err()
                .code(),
            SessionErrorCode::BindingInvalid
        );
        let session = authority
            .open_session(&head(1, 0x10, 0x11), &retained(0x10))
            .unwrap()
            .session_id;
        assert_eq!(
            authority
                .renew_session(session, &head(1, 0x10, 0x11), &retained(0x20))
                .unwrap_err()
                .code(),
            SessionErrorCode::BindingInvalid
        );
    }

    #[test]
    fn renewal_limit_is_exact() {
        let mut authority = SessionAuthority::new(handshake());
        let session = authority
            .open_session(&head(1, 0x10, 0x11), &retained(0x10))
            .unwrap()
            .session_id;
        for _ in 0..MAX_SESSION_RENEWALS {
            authority
                .renew_session(session, &head(1, 0x10, 0x11), &retained(0x10))
                .unwrap();
        }
        assert_eq!(
            authority
                .renew_session(session, &head(1, 0x10, 0x11), &retained(0x10))
                .unwrap_err()
                .code(),
            SessionErrorCode::RenewalLimit
        );
    }
}

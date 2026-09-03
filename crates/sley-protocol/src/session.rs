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
}

impl SessionAuthority {
    #[must_use]
    pub const fn new(handshake_id: ProtocolHandshakeId) -> Self {
        Self {
            handshake_id,
            issued: 0,
            sessions: BTreeMap::new(),
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

    /// Issues a session bound to the accepted head (contract section 2).
    ///
    /// # Errors
    ///
    /// Returns `SESSION_BINDING_INVALID` when the ordinal space is exhausted
    /// or the identity is already issued.
    pub fn open_session(&mut self, head: &HeadBinding) -> Result<SessionRecord, SessionError> {
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
        Ok(record)
    }

    /// Rebinds a session to the current accepted head (contract section 2).
    ///
    /// # Errors
    ///
    /// Returns `SESSION_UNKNOWN`, `SESSION_WORKSPACE_MISMATCH`,
    /// `SESSION_EPOCH_MISMATCH`, or `SESSION_RENEWAL_LIMIT`.
    pub fn renew_session(
        &mut self,
        session: SessionId,
        head: &HeadBinding,
    ) -> Result<SessionRecord, SessionError> {
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
        Ok(*record)
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
            .ok_or(SessionError(SessionErrorCode::Unknown))
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

    fn head(workspace: u8, root: u8, epoch: u8) -> HeadBinding {
        HeadBinding {
            workspace_id: WorkspaceId::from_bytes([workspace; 32]),
            root: StateRoot::from_bytes([root; 32]),
            schema_epoch: SchemaEpochId::from_bytes([epoch; 32]),
        }
    }

    fn handshake() -> ProtocolHandshakeId {
        ProtocolHandshakeId::from_bytes([0x44; 32])
    }

    #[test]
    fn issuance_is_deterministic_and_binds_the_head() {
        let mut a = SessionAuthority::new(handshake());
        let mut b = SessionAuthority::new(handshake());
        let first = a.open_session(&head(1, 0x10, 0x11)).unwrap();
        let again = b.open_session(&head(1, 0x10, 0x11)).unwrap();
        assert_eq!(first, again);
        assert_eq!(first.issue_ordinal, 1);
        assert_eq!(first.bound_root, StateRoot::from_bytes([0x10; 32]));
        let second = a.open_session(&head(1, 0x10, 0x11)).unwrap();
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
            .open_session(&head(1, 0x10, 0x11))
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
            .renew_session(session, &head(1, 0x20, 0x11))
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
                .renew_session(session, &head(2, 0x20, 0x11))
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
    fn renewal_limit_is_exact() {
        let mut authority = SessionAuthority::new(handshake());
        let session = authority
            .open_session(&head(1, 0x10, 0x11))
            .unwrap()
            .session_id;
        for _ in 0..MAX_SESSION_RENEWALS {
            authority
                .renew_session(session, &head(1, 0x10, 0x11))
                .unwrap();
        }
        assert_eq!(
            authority
                .renew_session(session, &head(1, 0x10, 0x11))
                .unwrap_err()
                .code(),
            SessionErrorCode::RenewalLimit
        );
    }
}

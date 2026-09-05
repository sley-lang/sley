//! Sley Machine Protocol v1 frames, negotiation, and identity scoping
//! (S20-410, contract `docs/spec/SMP1.md`, ADR-0032).
//!
//! This crate owns framing, the derived handshake, session-scoped request
//! identity, the bounded-context and failure envelopes, and the frozen
//! method table. It owns no semantics: every body is an opaque frozen
//! record of another contract, and nothing here judges it.

// Variant, field, and getter names are the contract's own names; the
// contract text is their documentation.
#![allow(missing_docs)]

pub mod server;
pub mod session;
pub use server::*;
pub use session::*;
#[cfg(test)]
mod server_tests;

use core::fmt;
use std::collections::BTreeMap;

use sley_id::{ProtocolFrameId, ProtocolHandshakeId, SchemaEpochId};
use sley_scb1::{encode_bytes, encode_list, encode_record, encode_union, encode_uvar};
use sley_schema::{ContractDescriptor, EpochLimits, SchemaEpochRecordV1, UnicodeVersion};

/// Frozen protocol version of this contract revision.
pub const PROTOCOL_VERSION: u32 = 1;
/// Absolute frame ceiling; negotiated `max_frame_bytes` never exceeds it.
pub const MAX_FRAME_BYTES: u64 = 67_108_864;
/// Protocol frame contract tag (every frame kind shares it).
pub const FRAME_CONTRACT_TAG: u32 = 400;
/// Digest domain tag of `sley2.protocol-frame.v1` in the epoch registry.
pub const DIGEST_DOMAIN_TAG: u32 = 22;
/// Feature bits of the hello and selected profile.
pub const FEATURE_CANCEL: u32 = 1;
pub const FEATURE_STREAM: u32 = 2;
pub const FEATURE_JSON_BRIDGE: u32 = 4;
pub const FEATURE_CHECKSUM: u32 = 8;
/// Feature bit 4: the session may select the extended execute profile
/// (`limits` field 6 value 2, contract appendix C). Without the negotiated
/// bit, selecting it is `PROTOCOL_PAYLOAD_INVALID`.
pub const FEATURE_EXTENDED_EXECUTE: u32 = 16;
const FEATURE_MASK: u32 = FEATURE_CANCEL
    | FEATURE_STREAM
    | FEATURE_JSON_BRIDGE
    | FEATURE_CHECKSUM
    | FEATURE_EXTENDED_EXECUTE;
/// Flag bits of a frame.
pub const FLAG_CANCEL: u32 = 1;
pub const FLAG_STREAM: u32 = 2;
/// Response flag bit 2: the body is a failure envelope (contract section 6).
pub const FLAG_FAILED: u32 = 4;
const FLAG_MASK: u32 = FLAG_CANCEL | FLAG_STREAM | FLAG_FAILED;
/// Ceilings of the negotiable limits.
pub const MAX_LIMIT_ENTITIES: u64 = 65_535;
pub const MAX_LIMIT_EDGES: u64 = 400_000;
pub const MAX_LIMIT_DEPTH: u32 = 65_535;
pub const MAX_LIMIT_RESPONSE_BYTES: u64 = 67_108_864;
pub const MAX_LIMIT_WORK: u64 = 100_000_000;
pub const MAX_LIMIT_INFLIGHT: u32 = 1_024;
/// Ceiling of negotiable live sessions (contract section 3): sessions pin
/// bound roots, so the live count and the remembered closed names are both
/// capped by the selected `max_sessions`.
pub const MAX_LIMIT_SESSIONS: u32 = 256;
/// List ceilings inside hello records.
pub const MAX_HELLO_LIST: usize = 4_096;

const MAGIC: &[u8; 8] = b"SLEYSCB1";
const FORMAT_VERSION: u64 = 1;
const ID_LEN: usize = 32;
const LENGTH_PREFIX: usize = 8;
const FRAME_FIELD_SCHEMA_HASH: [u8; 32] = [
    0xd3, 0x6c, 0xce, 0x86, 0x1a, 0xb7, 0x0c, 0xbb, 0xd0, 0x21, 0xff, 0x3d, 0x80, 0x20, 0xe7, 0x1e,
    0xb9, 0xe9, 0x68, 0x47, 0x73, 0xf8, 0x27, 0x34, 0xb6, 0xf2, 0xaa, 0x48, 0x49, 0x37, 0xdf, 0x0e,
];
const DECODER_LIMITS_HASH: [u8; 32] = [
    0x21, 0x2b, 0x29, 0x3d, 0x90, 0x64, 0x66, 0x21, 0xc2, 0xe8, 0x3c, 0xaf, 0x09, 0x5c, 0x2a, 0x21,
    0xb6, 0x86, 0x48, 0x88, 0xa3, 0xcc, 0x98, 0x87, 0x8b, 0x7f, 0x6a, 0xb8, 0x1a, 0xb9, 0x8f, 0xe7,
];

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolErrorCode {
    VersionUnsupported,
    FrameInvalid,
    FrameTooLarge,
    NoCommonProfile,
    Downgrade,
    RequestIdConflict,
    SessionClosed,
    MethodUnsupported,
    PayloadInvalid,
    LimitExceeded,
    Cancelled,
    InternalInvariant,
}

impl ProtocolErrorCode {
    pub const ALL: [Self; 12] = [
        Self::VersionUnsupported,
        Self::FrameInvalid,
        Self::FrameTooLarge,
        Self::NoCommonProfile,
        Self::Downgrade,
        Self::RequestIdConflict,
        Self::SessionClosed,
        Self::MethodUnsupported,
        Self::PayloadInvalid,
        Self::LimitExceeded,
        Self::Cancelled,
        Self::InternalInvariant,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VersionUnsupported => "PROTOCOL_VERSION_UNSUPPORTED",
            Self::FrameInvalid => "PROTOCOL_FRAME_INVALID",
            Self::FrameTooLarge => "PROTOCOL_FRAME_TOO_LARGE",
            Self::NoCommonProfile => "PROTOCOL_NO_COMMON_PROFILE",
            Self::Downgrade => "PROTOCOL_DOWNGRADE",
            Self::RequestIdConflict => "PROTOCOL_REQUEST_ID_CONFLICT",
            Self::SessionClosed => "PROTOCOL_SESSION_CLOSED",
            Self::MethodUnsupported => "PROTOCOL_METHOD_UNSUPPORTED",
            Self::PayloadInvalid => "PROTOCOL_PAYLOAD_INVALID",
            Self::LimitExceeded => "PROTOCOL_LIMIT_EXCEEDED",
            Self::Cancelled => "PROTOCOL_CANCELLED",
            Self::InternalInvariant => "PROTOCOL_INTERNAL_INVARIANT",
        }
    }

    #[must_use]
    pub const fn numeric(self) -> u32 {
        match self {
            Self::VersionUnsupported => 40_000,
            Self::FrameInvalid => 40_001,
            Self::FrameTooLarge => 40_002,
            Self::NoCommonProfile => 40_003,
            Self::Downgrade => 40_004,
            Self::RequestIdConflict => 40_005,
            Self::SessionClosed => 40_006,
            Self::MethodUnsupported => 40_007,
            Self::PayloadInvalid => 40_008,
            Self::LimitExceeded => 40_009,
            Self::Cancelled => 40_010,
            Self::InternalInvariant => 40_011,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolError(ProtocolErrorCode);

impl ProtocolError {
    #[must_use]
    pub const fn new(code: ProtocolErrorCode) -> Self {
        Self(code)
    }

    #[must_use]
    pub const fn code(&self) -> ProtocolErrorCode {
        self.0
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for ProtocolError {}

type Result<T> = core::result::Result<T, ProtocolError>;

fn fail<T>(code: ProtocolErrorCode) -> Result<T> {
    Err(ProtocolError(code))
}

fn scb<T>(result: core::result::Result<T, sley_scb1::ScbError>) -> Result<T> {
    result.map_err(|_| ProtocolError(ProtocolErrorCode::InternalInvariant))
}

// ---------------------------------------------------------------------------
// Identities and profiles
// ---------------------------------------------------------------------------

/// The negotiated session identity (S20-330, `sley2.session.v1`).
pub use sley_id::SessionId;

/// Negotiable limits (contract section 2).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LimitProfile {
    pub max_frame_bytes: u64,
    pub max_entities: u64,
    pub max_edges: u64,
    pub max_depth: u32,
    pub max_response_bytes: u64,
    pub max_work: u64,
    pub max_inflight: u32,
    pub max_sessions: u32,
}

impl LimitProfile {
    /// The profile ceilings.
    #[must_use]
    pub const fn maximum() -> Self {
        Self {
            max_frame_bytes: MAX_FRAME_BYTES,
            max_entities: MAX_LIMIT_ENTITIES,
            max_edges: MAX_LIMIT_EDGES,
            max_depth: MAX_LIMIT_DEPTH,
            max_response_bytes: MAX_LIMIT_RESPONSE_BYTES,
            max_work: MAX_LIMIT_WORK,
            max_inflight: MAX_LIMIT_INFLIGHT,
            max_sessions: MAX_LIMIT_SESSIONS,
        }
    }

    /// All-zero profile carried by request frames.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            max_frame_bytes: 0,
            max_entities: 0,
            max_edges: 0,
            max_depth: 0,
            max_response_bytes: 0,
            max_work: 0,
            max_inflight: 0,
            max_sessions: 0,
        }
    }

    /// Validates a declared profile: every limit nonzero and at most its ceiling.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_LIMIT_EXCEEDED`.
    pub fn validate(&self) -> Result<()> {
        let ceiling = Self::maximum();
        if self.max_frame_bytes == 0
            || self.max_frame_bytes > ceiling.max_frame_bytes
            || self.max_entities == 0
            || self.max_entities > ceiling.max_entities
            || self.max_edges == 0
            || self.max_edges > ceiling.max_edges
            || self.max_depth > ceiling.max_depth
            || self.max_response_bytes == 0
            || self.max_response_bytes > ceiling.max_response_bytes
            || self.max_work == 0
            || self.max_work > ceiling.max_work
            || self.max_inflight == 0
            || self.max_inflight > ceiling.max_inflight
            || self.max_sessions == 0
            || self.max_sessions > ceiling.max_sessions
        {
            return fail(ProtocolErrorCode::LimitExceeded);
        }
        Ok(())
    }

    /// The field-wise minimum of two profiles.
    #[must_use]
    pub fn minimum(&self, other: &Self) -> Self {
        Self {
            max_frame_bytes: self.max_frame_bytes.min(other.max_frame_bytes),
            max_entities: self.max_entities.min(other.max_entities),
            max_edges: self.max_edges.min(other.max_edges),
            max_depth: self.max_depth.min(other.max_depth),
            max_response_bytes: self.max_response_bytes.min(other.max_response_bytes),
            max_work: self.max_work.min(other.max_work),
            max_inflight: self.max_inflight.min(other.max_inflight),
            max_sessions: self.max_sessions.min(other.max_sessions),
        }
    }

    fn encode(&self) -> Result<Vec<u8>> {
        scb(encode_record(&[
            (1, encode_uvar(self.max_frame_bytes)),
            (2, encode_uvar(self.max_entities)),
            (3, encode_uvar(self.max_edges)),
            (4, encode_uvar(u64::from(self.max_depth))),
            (5, encode_uvar(self.max_response_bytes)),
            (6, encode_uvar(self.max_work)),
            (7, encode_uvar(u64::from(self.max_inflight))),
            (8, encode_uvar(u64::from(self.max_sessions))),
        ]))
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let fields = Reader::new(input).record(8)?;
        Ok(Self {
            max_frame_bytes: single_uvar(fields[0])?,
            max_entities: single_uvar(fields[1])?,
            max_edges: single_uvar(fields[2])?,
            max_depth: single_u32(fields[3])?,
            max_response_bytes: single_uvar(fields[4])?,
            max_work: single_uvar(fields[5])?,
            max_inflight: single_u32(fields[6])?,
            max_sessions: single_u32(fields[7])?,
        })
    }
}

/// Bounded context carried by every response frame (contract section 5).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundedContext {
    pub applied_limits: LimitProfile,
    pub returned_bytes: u64,
    pub returned_entities: u64,
    pub returned_edges: u64,
    pub reached_depth: u32,
    pub omitted: u64,
    pub truncated: bool,
    pub continuation: bool,
}

impl BoundedContext {
    /// The all-zero context of a request frame.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            applied_limits: LimitProfile::zero(),
            returned_bytes: 0,
            returned_entities: 0,
            returned_edges: 0,
            reached_depth: 0,
            omitted: 0,
            truncated: false,
            continuation: false,
        }
    }

    fn encode(&self) -> Result<Vec<u8>> {
        scb(encode_record(&[
            (1, self.applied_limits.encode()?),
            (2, encode_uvar(self.returned_bytes)),
            (3, encode_uvar(self.returned_entities)),
            (4, encode_uvar(self.returned_edges)),
            (5, encode_uvar(u64::from(self.reached_depth))),
            (6, encode_uvar(self.omitted)),
            (7, encode_uvar(flag(self.truncated))),
            (8, encode_uvar(flag(self.continuation))),
        ]))
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let fields = Reader::new(input).record(8)?;
        Ok(Self {
            applied_limits: LimitProfile::decode(fields[0])?,
            returned_bytes: single_uvar(fields[1])?,
            returned_entities: single_uvar(fields[2])?,
            returned_edges: single_uvar(fields[3])?,
            reached_depth: single_u32(fields[4])?,
            omitted: single_uvar(fields[5])?,
            truncated: decode_flag(single_uvar(fields[6])?)?,
            continuation: decode_flag(single_uvar(fields[7])?)?,
        })
    }
}

const fn flag(value: bool) -> u64 {
    if value { 2 } else { 1 }
}

fn decode_flag(value: u64) -> Result<bool> {
    match value {
        1 => Ok(false),
        2 => Ok(true),
        _ => fail(ProtocolErrorCode::FrameInvalid),
    }
}

// ---------------------------------------------------------------------------
// Method table
// ---------------------------------------------------------------------------

/// The frozen method table (contract section 4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Method {
    SessionOpen,
    SessionRenew,
    SessionClose,
    SessionCapabilities,
    SessionBudgets,
    WorkspaceCreate,
    WorkspaceOpen,
    RefsList,
    RefsResolve,
    RevisionRead,
    BranchCreate,
    BranchAdvance,
    Compare,
    MergeJudge,
    MergeCommit,
    ExchangeExport,
    ExchangeImport,
    GcDryRun,
    GcCollect,
    RefsRecover,
    QueryRoot,
    QueryContinue,
    Capsule,
    QueryRestricted,
    HandleExpand,
    Diagnostics,
    CandidateCreate,
    CandidateAppend,
    CandidateValidate,
    CandidateInspect,
    CandidateDiscard,
    Commit,
    ReceiptRead,
    Checkout,
    RefMoveProtected,
    Recovery,
    Execute,
    TestsSelected,
    TestsAffected,
    Cancel,
    Report,
}

impl Method {
    pub const ALL: [Self; 41] = [
        Self::SessionOpen,
        Self::SessionRenew,
        Self::SessionClose,
        Self::SessionCapabilities,
        Self::SessionBudgets,
        Self::WorkspaceCreate,
        Self::WorkspaceOpen,
        Self::RefsList,
        Self::RefsResolve,
        Self::RevisionRead,
        Self::BranchCreate,
        Self::BranchAdvance,
        Self::Compare,
        Self::MergeJudge,
        Self::MergeCommit,
        Self::ExchangeExport,
        Self::ExchangeImport,
        Self::GcDryRun,
        Self::GcCollect,
        Self::RefsRecover,
        Self::QueryRoot,
        Self::QueryContinue,
        Self::Capsule,
        Self::QueryRestricted,
        Self::HandleExpand,
        Self::Diagnostics,
        Self::CandidateCreate,
        Self::CandidateAppend,
        Self::CandidateValidate,
        Self::CandidateInspect,
        Self::CandidateDiscard,
        Self::Commit,
        Self::ReceiptRead,
        Self::Checkout,
        Self::RefMoveProtected,
        Self::Recovery,
        Self::Execute,
        Self::TestsSelected,
        Self::TestsAffected,
        Self::Cancel,
        Self::Report,
    ];

    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::SessionOpen => 100,
            Self::SessionRenew => 101,
            Self::SessionClose => 102,
            Self::SessionCapabilities => 103,
            Self::SessionBudgets => 104,
            Self::WorkspaceCreate => 200,
            Self::WorkspaceOpen => 201,
            Self::RefsList => 202,
            Self::RefsResolve => 203,
            Self::RevisionRead => 204,
            Self::BranchCreate => 205,
            Self::BranchAdvance => 206,
            Self::Compare => 207,
            Self::MergeJudge => 208,
            Self::MergeCommit => 209,
            Self::ExchangeExport => 210,
            Self::ExchangeImport => 211,
            Self::GcDryRun => 212,
            Self::GcCollect => 213,
            Self::RefsRecover => 214,
            Self::QueryRoot => 300,
            Self::QueryContinue => 301,
            Self::Capsule => 302,
            Self::QueryRestricted => 303,
            Self::HandleExpand => 304,
            Self::Diagnostics => 305,
            Self::CandidateCreate => 400,
            Self::CandidateAppend => 401,
            Self::CandidateValidate => 402,
            Self::CandidateInspect => 403,
            Self::CandidateDiscard => 404,
            Self::Commit => 500,
            Self::ReceiptRead => 501,
            Self::Checkout => 502,
            Self::RefMoveProtected => 503,
            Self::Recovery => 504,
            Self::Execute => 600,
            Self::TestsSelected => 601,
            Self::TestsAffected => 602,
            Self::Cancel => 603,
            Self::Report => 604,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::SessionOpen => "session.open",
            Self::SessionRenew => "session.renew",
            Self::SessionClose => "session.close",
            Self::SessionCapabilities => "session.capabilities",
            Self::SessionBudgets => "session.budgets",
            Self::WorkspaceCreate => "workspace.create",
            Self::WorkspaceOpen => "workspace.open",
            Self::RefsList => "refs.list",
            Self::RefsResolve => "refs.resolve",
            Self::RevisionRead => "revision.read",
            Self::BranchCreate => "branch.create",
            Self::BranchAdvance => "branch.advance",
            Self::Compare => "compare",
            Self::MergeJudge => "merge.judge",
            Self::MergeCommit => "merge.commit",
            Self::ExchangeExport => "exchange.export",
            Self::ExchangeImport => "exchange.import",
            Self::GcDryRun => "gc.dry_run",
            Self::GcCollect => "gc.collect",
            Self::RefsRecover => "refs.recover",
            Self::QueryRoot => "query.root",
            Self::QueryContinue => "query.continue",
            Self::Capsule => "capsule",
            Self::QueryRestricted => "query.restricted",
            Self::HandleExpand => "handle.expand",
            Self::Diagnostics => "diagnostics",
            Self::CandidateCreate => "candidate.create",
            Self::CandidateAppend => "candidate.append",
            Self::CandidateValidate => "candidate.validate",
            Self::CandidateInspect => "candidate.inspect",
            Self::CandidateDiscard => "candidate.discard",
            Self::Commit => "commit",
            Self::ReceiptRead => "receipt.read",
            Self::Checkout => "checkout",
            Self::RefMoveProtected => "ref.move.protected",
            Self::Recovery => "recovery",
            Self::Execute => "execute",
            Self::TestsSelected => "tests.selected",
            Self::TestsAffected => "tests.affected",
            Self::Cancel => "cancel",
            Self::Report => "report",
        }
    }

    /// Family tag: the hundreds digit of the method tag.
    #[must_use]
    pub const fn family(self) -> u32 {
        self.tag() / 100
    }

    /// Reserved methods fail `PROTOCOL_METHOD_UNSUPPORTED` at this revision.
    #[must_use]
    pub const fn is_reserved(self) -> bool {
        matches!(
            self,
            Self::Diagnostics | Self::RefMoveProtected | Self::TestsSelected | Self::TestsAffected
        )
    }

    /// Resolves a frozen tag.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_METHOD_UNSUPPORTED` for any tag outside the table.
    pub fn from_tag(tag: u32) -> Result<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|method| method.tag() == tag)
            .ok_or(ProtocolError(ProtocolErrorCode::MethodUnsupported))
    }
}

// ---------------------------------------------------------------------------
// Handshake
// ---------------------------------------------------------------------------

/// A peer's hello (contract section 2).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hello {
    pub protocol_versions: Vec<u32>,
    pub schema_epochs: Vec<SchemaEpochId>,
    pub limits: LimitProfile,
    pub methods: Vec<u32>,
    pub features: u32,
    pub adapters: Vec<[u8; 32]>,
    pub effects: Vec<[u8; 32]>,
}

/// The derived selection (contract section 2).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedProfile {
    pub protocol_version: u32,
    pub schema_epoch: SchemaEpochId,
    pub limits: LimitProfile,
    pub methods: Vec<u32>,
    pub features: u32,
    pub adapters: Vec<[u8; 32]>,
    pub effects: Vec<[u8; 32]>,
}

impl Hello {
    /// Validates the declared shape: strictly increasing sets, known
    /// features, valid limits, bounded lists.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_PAYLOAD_INVALID` or `PROTOCOL_LIMIT_EXCEEDED`.
    pub fn validate(&self) -> Result<()> {
        if self.protocol_versions.is_empty()
            || self.protocol_versions.len() > MAX_HELLO_LIST
            || !strictly_increasing(&self.protocol_versions)
            || self.schema_epochs.is_empty()
            || self.schema_epochs.len() > MAX_HELLO_LIST
            || self.methods.is_empty()
            || self.methods.len() > MAX_HELLO_LIST
            || !strictly_increasing(&self.methods)
            || self.adapters.len() > MAX_HELLO_LIST
            || !strictly_increasing(&self.adapters)
            || self.effects.len() > MAX_HELLO_LIST
            || !strictly_increasing(&self.effects)
            || self.features & !FEATURE_MASK != 0
            || self
                .methods
                .iter()
                .any(|tag| Method::from_tag(*tag).is_ok_and(Method::is_reserved))
        {
            return fail(ProtocolErrorCode::PayloadInvalid);
        }
        self.limits.validate()
    }

    /// Encodes the hello record.
    ///
    /// # Errors
    ///
    /// Returns the validation failure.
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        scb(encode_record(&[
            (1, uvar_list(&self.protocol_versions)?),
            (
                2,
                fixed_list(
                    &self
                        .schema_epochs
                        .iter()
                        .map(|e| *e.as_bytes())
                        .collect::<Vec<_>>(),
                )?,
            ),
            (3, self.limits.encode()?),
            (4, uvar_list(&self.methods)?),
            (5, encode_uvar(u64::from(self.features))),
            (6, fixed_list(&self.adapters)?),
            (7, fixed_list(&self.effects)?),
        ]))
    }

    /// Decodes and validates a hello record.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_PAYLOAD_INVALID` on any shape defect.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let fields = Reader::new(input)
            .record(7)
            .map_err(|_| ProtocolError(ProtocolErrorCode::PayloadInvalid))?;
        let hello = Self {
            protocol_versions: u32_list(fields[0])?,
            schema_epochs: fixed32_list(fields[1])?
                .into_iter()
                .map(SchemaEpochId::from_bytes)
                .collect(),
            limits: LimitProfile::decode(fields[2])
                .map_err(|_| ProtocolError(ProtocolErrorCode::PayloadInvalid))?,
            methods: u32_list(fields[3])?,
            features: single_u32(fields[4])
                .map_err(|_| ProtocolError(ProtocolErrorCode::PayloadInvalid))?,
            adapters: fixed32_list(fields[5])?,
            effects: fixed32_list(fields[6])?,
        };
        hello.validate()?;
        Ok(hello)
    }
}

/// The exact bytes the handshake identity digests: the client hello body,
/// then the server hello body, then the selection preimage (contract
/// section 2). The client speaks first, so it comes first.
#[must_use]
pub fn handshake_transcript(
    client_hello_body: &[u8],
    server_hello_body: &[u8],
    selection_preimage: &[u8],
) -> Vec<u8> {
    let mut transcript = Vec::with_capacity(
        client_hello_body.len() + server_hello_body.len() + selection_preimage.len(),
    );
    transcript.extend_from_slice(client_hello_body);
    transcript.extend_from_slice(server_hello_body);
    transcript.extend_from_slice(selection_preimage);
    transcript
}

/// Derives the selection and binds the observed hello transcript.
///
/// Both peers must call this on the hellos as observed (the own hello as
/// sent, the peer hello as received) and use the returned selection and
/// identity; an asserted selection is never trustworthy for identity.
/// The identity binds both hello bodies, so any tamper of either hello
/// changes at least one peer's transcript and the two identities differ
/// (contract section 2, threat T45).
///
/// # Errors
///
/// Returns `PROTOCOL_NO_COMMON_PROFILE` when no common version, epoch, or
/// method exists, the hellos fail validation, or the transcript cannot be
/// digested.
pub fn negotiate_identity(
    client: &Hello,
    server: &Hello,
) -> Result<(SelectedProfile, ProtocolHandshakeId)> {
    let profile = negotiate(client, server)?;
    let identity = profile.handshake_id_bound(&client.encode()?, &server.encode()?)?;
    Ok((profile, identity))
}

/// Derives the selected profile from two hellos (contract section 2).
///
/// Prefer [`negotiate_identity`]: identity must bind the observed hello
/// transcript, never the selection alone.
///
/// # Errors
///
/// Returns `PROTOCOL_NO_COMMON_PROFILE` when no common version, epoch, or
/// method exists, or the hellos fail validation.
pub fn negotiate(client: &Hello, server: &Hello) -> Result<SelectedProfile> {
    client.validate()?;
    server.validate()?;
    let protocol_version = client
        .protocol_versions
        .iter()
        .rev()
        .copied()
        .find(|version| server.protocol_versions.binary_search(version).is_ok())
        .ok_or(ProtocolError(ProtocolErrorCode::NoCommonProfile))?;
    let schema_epoch = server
        .schema_epochs
        .iter()
        .copied()
        .find(|epoch| client.schema_epochs.contains(epoch))
        .ok_or(ProtocolError(ProtocolErrorCode::NoCommonProfile))?;
    let methods: Vec<u32> = client
        .methods
        .iter()
        .copied()
        .filter(|method| server.methods.binary_search(method).is_ok())
        .collect();
    if methods.is_empty() {
        return fail(ProtocolErrorCode::NoCommonProfile);
    }
    // The session family is the mandatory floor (contract section 2): a
    // selection without `session.open` can never bind a session, so it is
    // no common profile rather than a successful handshake.
    if !methods.contains(&Method::SessionOpen.tag()) {
        return fail(ProtocolErrorCode::NoCommonProfile);
    }
    let adapters = intersect(&client.adapters, &server.adapters);
    let effects = intersect(&client.effects, &server.effects);
    Ok(SelectedProfile {
        protocol_version,
        schema_epoch,
        limits: client.limits.minimum(&server.limits),
        methods,
        features: client.features & server.features,
        adapters,
        effects,
    })
}

fn intersect(left: &[[u8; 32]], right: &[[u8; 32]]) -> Vec<[u8; 32]> {
    left.iter()
        .copied()
        .filter(|item| right.binary_search(item).is_ok())
        .collect()
}

impl SelectedProfile {
    /// Canonical preimage of the selection (contract section 2): the SCB1
    /// record of the seven derived fields (1 `protocol_version`, 2
    /// `schema_epoch`, 3 `limits`, 4 `methods`, 5 `features`, 6 `adapters`,
    /// 7 `effects`).
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_INTERNAL_INVARIANT` on an encoding defect.
    pub fn preimage(&self) -> Result<Vec<u8>> {
        scb(encode_record(&[
            (1, encode_uvar(u64::from(self.protocol_version))),
            (2, self.schema_epoch.as_bytes().to_vec()),
            (3, self.limits.encode()?),
            (4, uvar_list(&self.methods)?),
            (5, encode_uvar(u64::from(self.features))),
            (6, fixed_list(&self.adapters)?),
            (7, fixed_list(&self.effects)?),
        ]))
    }

    /// The transcript-bound `ProtocolHandshakeId` both peers must compute
    /// identically (contract section 2):
    /// `BLAKE3-256("sley2.protocol-handshake.v1" || client hello body ||
    /// server hello body || selected_profile_preimage)`, the client first
    /// because it speaks first. Each body is the exact `Hello::encode`
    /// bytes of the hello as observed, so a tampered hello changes at
    /// least one peer's transcript and `session.open` fails
    /// `PROTOCOL_DOWNGRADE` (threat T45). The selection-only digest is
    /// gone: it attested the server's asserted selection only.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_INTERNAL_INVARIANT` on an encoding defect.
    pub fn handshake_id_bound(
        &self,
        client_hello_body: &[u8],
        server_hello_body: &[u8],
    ) -> Result<ProtocolHandshakeId> {
        let preimage = self.preimage()?;
        Ok(ProtocolHandshakeId::derive(handshake_transcript(
            client_hello_body,
            server_hello_body,
            &preimage,
        )))
    }

    /// Detects a downgrade: a claimed version or epoch below the selection.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_DOWNGRADE`, or `PROTOCOL_VERSION_UNSUPPORTED` for a
    /// version above the selection.
    pub fn check_claim(&self, claimed_version: u32, claimed_epoch: SchemaEpochId) -> Result<()> {
        if claimed_version < self.protocol_version || claimed_epoch != self.schema_epoch {
            return fail(ProtocolErrorCode::Downgrade);
        }
        if claimed_version > self.protocol_version {
            return fail(ProtocolErrorCode::VersionUnsupported);
        }
        Ok(())
    }

    /// Whether the selection admits a method.
    #[must_use]
    pub fn admits(&self, method: Method) -> bool {
        self.methods.binary_search(&method.tag()).is_ok()
    }
}

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameKind {
    Request,
    Response,
    Event,
    Hello,
}

impl FrameKind {
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Request => 1,
            Self::Response => 2,
            Self::Event => 3,
            Self::Hello => 4,
        }
    }

    fn from_tag(tag: u64) -> Result<Self> {
        match tag {
            1 => Ok(Self::Request),
            2 => Ok(Self::Response),
            3 => Ok(Self::Event),
            4 => Ok(Self::Hello),
            _ => fail(ProtocolErrorCode::FrameInvalid),
        }
    }
}

/// A decoded protocol frame (contract section 1).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolFrame {
    pub protocol_version: u32,
    pub session: Option<SessionId>,
    pub request_id: u64,
    pub kind: FrameKind,
    pub method: u32,
    pub flags: u32,
    pub bounds: BoundedContext,
    pub body: Vec<u8>,
}

/// One encoded frame: the length prefix, the envelope, and its digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedFrame {
    pub kind: FrameKind,
    pub frame_id: ProtocolFrameId,
    pub bytes: Vec<u8>,
}

/// Returns the protocol schema epoch record.
#[must_use]
pub fn protocol_epoch_record() -> SchemaEpochRecordV1 {
    let descriptor =
        |contract_tag: u32, field_schema_hash: [u8; 32], required: Vec<u32>| ContractDescriptor {
            contract_tag,
            digest_domain_tag: DIGEST_DOMAIN_TAG,
            kind_tag: contract_tag,
            field_schema_hash,
            required_fields: required,
            optional_fields: Vec::new(),
            variant_tags: Vec::new(),
            decoder_limits_hash: DECODER_LIMITS_HASH,
        };
    SchemaEpochRecordV1 {
        epoch_number: 1,
        scb_format_version: 1,
        hash_algorithm_tag: 1,
        unicode_nfc_version: UnicodeVersion::EPOCH_1,
        limits: EpochLimits::EPOCH_1,
        contracts: vec![descriptor(
            FRAME_CONTRACT_TAG,
            FRAME_FIELD_SCHEMA_HASH,
            (1..=8).collect(),
        )],
        extensions: Vec::new(),
        predecessor: None,
        migration_contracts: Vec::new(),
    }
}

/// Returns the exact protocol schema epoch identity.
///
/// # Errors
///
/// Returns `PROTOCOL_INTERNAL_INVARIANT` if the descriptor record drifts.
pub fn protocol_epoch_id() -> Result<SchemaEpochId> {
    protocol_epoch_record()
        .schema_epoch_id()
        .map_err(|_| ProtocolError(ProtocolErrorCode::InternalInvariant))
}

impl ProtocolFrame {
    /// Validates a decoded frame's header with the frozen codec, including
    /// the hello header rule of SMP1 section 2 (`session = None`,
    /// `request_id = 0`, `method = 0`, `flags = 0` on kind 4).
    ///
    /// Readers that build a frame without decoding wire bytes (notably the
    /// S20-420 JSON bridge) call this instead of re-stating the rule, so a
    /// header the codec would not encode keeps the codec's
    /// `PROTOCOL_FRAME_INVALID` rather than a reader-owned code.
    ///
    /// # Errors
    ///
    /// Returns the exact validation failure; never a partial judgment.
    pub fn validate_header(&self) -> Result<()> {
        self.validate()
    }

    fn validate(&self) -> Result<()> {
        // Below the implementation version is a downgrade attempt;
        // above it names a version this code does not know (contract
        // section 2; the selection-level split lives in
        // `SelectedProfile::check_claim`).
        if self.protocol_version < PROTOCOL_VERSION {
            return fail(ProtocolErrorCode::Downgrade);
        }
        if self.protocol_version > PROTOCOL_VERSION {
            return fail(ProtocolErrorCode::VersionUnsupported);
        }
        if self.flags & !FLAG_MASK != 0 {
            return fail(ProtocolErrorCode::FrameInvalid);
        }
        if self.flags & FLAG_FAILED != 0
            && !matches!(self.kind, FrameKind::Response | FrameKind::Event)
        {
            return fail(ProtocolErrorCode::FrameInvalid);
        }
        if self.kind == FrameKind::Hello
            && (self.session.is_some()
                || self.request_id != 0
                || self.method != 0
                || self.flags != 0)
        {
            return fail(ProtocolErrorCode::FrameInvalid);
        }
        Ok(())
    }

    fn payload(&self) -> Result<Vec<u8>> {
        let session = match self.session {
            None => scb(encode_union(0, &[]))?,
            Some(session) => scb(encode_union(1, session.as_bytes()))?,
        };
        scb(encode_record(&[
            (1, encode_uvar(u64::from(self.protocol_version))),
            (2, session),
            (3, encode_uvar(self.request_id)),
            (4, encode_uvar(u64::from(self.kind.tag()))),
            (5, encode_uvar(u64::from(self.method))),
            (6, encode_uvar(u64::from(self.flags))),
            (7, self.bounds.encode()?),
            (8, scb(encode_bytes(&self.body))?),
        ]))
    }

    fn from_payload(payload: &[u8]) -> Result<Self> {
        let fields = Reader::new(payload).record(8)?;
        let (session_tag, session_payload) = Reader::new(fields[1]).union()?;
        let session = match (session_tag, session_payload.len()) {
            (0, 0) => None,
            (1, ID_LEN) => Some(SessionId::from_bytes(fixed32(session_payload)?)),
            _ => return fail(ProtocolErrorCode::FrameInvalid),
        };
        let frame = Self {
            protocol_version: single_u32(fields[0])?,
            session,
            request_id: single_uvar(fields[2])?,
            kind: FrameKind::from_tag(single_uvar(fields[3])?)?,
            method: single_u32(fields[4])?,
            flags: single_u32(fields[5])?,
            bounds: BoundedContext::decode(fields[6])?,
            body: Reader::new(fields[7]).bytes()?.to_vec(),
        };
        frame.validate()?;
        Ok(frame)
    }
}

/// Encodes a request, response, or event frame.
///
/// # Errors
///
/// Returns the validation failure or `PROTOCOL_FRAME_TOO_LARGE`.
pub fn encode_frame(frame: &ProtocolFrame) -> Result<EncodedFrame> {
    if frame.kind == FrameKind::Hello {
        return fail(ProtocolErrorCode::FrameInvalid);
    }
    frame.validate()?;
    encode_envelope(frame.kind, &frame.payload()?)
}

/// Encodes a hello frame (frame kind 4 carrying the hello record).
///
/// # Errors
///
/// Returns the validation failure or `PROTOCOL_FRAME_TOO_LARGE`.
pub fn encode_hello_frame(hello: &Hello) -> Result<EncodedFrame> {
    let frame = ProtocolFrame {
        protocol_version: PROTOCOL_VERSION,
        session: None,
        request_id: 0,
        kind: FrameKind::Hello,
        method: 0,
        flags: 0,
        bounds: BoundedContext::none(),
        body: hello.encode()?,
    };
    encode_envelope(FrameKind::Hello, &frame.payload()?)
}

fn encode_envelope(kind: FrameKind, payload: &[u8]) -> Result<EncodedFrame> {
    let contract_tag = FRAME_CONTRACT_TAG;
    let epoch = protocol_epoch_id()?;
    let mut preimage = Vec::with_capacity(payload.len() + 64);
    preimage.extend_from_slice(MAGIC);
    preimage.extend_from_slice(&encode_uvar(FORMAT_VERSION));
    preimage.extend_from_slice(&encode_uvar(u64::from(contract_tag)));
    preimage.extend_from_slice(epoch.as_bytes());
    preimage.extend_from_slice(&encode_uvar(payload.len() as u64));
    preimage.extend_from_slice(payload);
    let envelope_len = preimage.len() + ID_LEN;
    if u64::try_from(envelope_len).map_or(true, |len| len > MAX_FRAME_BYTES) {
        return fail(ProtocolErrorCode::FrameTooLarge);
    }
    let frame_id = ProtocolFrameId::derive(&preimage);
    let mut bytes = Vec::with_capacity(LENGTH_PREFIX + envelope_len);
    bytes.extend_from_slice(&(envelope_len as u64).to_be_bytes());
    bytes.extend_from_slice(&preimage);
    bytes.extend_from_slice(frame_id.as_bytes());
    Ok(EncodedFrame {
        kind,
        frame_id,
        bytes,
    })
}

/// A decoded envelope before its body is interpreted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedFrame {
    Hello(Hello),
    Request(ProtocolFrame),
    Response(ProtocolFrame),
}

/// Reads the length prefix and checks it against the negotiated ceiling
/// before any allocation.
///
/// # Errors
///
/// Returns `PROTOCOL_FRAME_INVALID` for a short prefix and
/// `PROTOCOL_FRAME_TOO_LARGE` for a length above the ceiling.
pub fn frame_length(prefix: &[u8], max_frame_bytes: u64) -> Result<usize> {
    let bytes: [u8; LENGTH_PREFIX] = prefix
        .get(..LENGTH_PREFIX)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(ProtocolError(ProtocolErrorCode::FrameInvalid))?;
    let length = u64::from_be_bytes(bytes);
    if length > max_frame_bytes.min(MAX_FRAME_BYTES) {
        return fail(ProtocolErrorCode::FrameTooLarge);
    }
    usize::try_from(length).map_err(|_| ProtocolError(ProtocolErrorCode::FrameTooLarge))
}

/// Decodes one complete frame (prefix plus envelope) under the negotiated
/// ceiling, verifying the digest before any field is read.
///
/// # Errors
///
/// Returns the exact frame, version, or payload failure; never a partial
/// frame.
pub fn decode_frame(input: &[u8], max_frame_bytes: u64) -> Result<(DecodedFrame, ProtocolFrameId)> {
    let length = frame_length(input, max_frame_bytes)?;
    let envelope = input
        .get(LENGTH_PREFIX..)
        .ok_or(ProtocolError(ProtocolErrorCode::FrameInvalid))?;
    if envelope.len() != length || length < ID_LEN + MAGIC.len() {
        return fail(ProtocolErrorCode::FrameInvalid);
    }
    let (preimage, trailer) = envelope.split_at(length - ID_LEN);
    let frame_id = ProtocolFrameId::derive(preimage);
    if trailer != frame_id.as_bytes() {
        return fail(ProtocolErrorCode::FrameInvalid);
    }
    let mut reader = Reader::new(preimage);
    if reader.take(MAGIC.len())? != MAGIC {
        return fail(ProtocolErrorCode::FrameInvalid);
    }
    if reader.uvar()? != FORMAT_VERSION {
        return fail(ProtocolErrorCode::VersionUnsupported);
    }
    let contract_tag = reader.uvar()?;
    let epoch = SchemaEpochId::from_bytes(reader.fixed32()?);
    if epoch != protocol_epoch_id()? {
        return fail(ProtocolErrorCode::VersionUnsupported);
    }
    let payload_len = usize::try_from(reader.uvar()?)
        .map_err(|_| ProtocolError(ProtocolErrorCode::FrameInvalid))?;
    let payload = reader.take(payload_len)?;
    if !reader.finished() {
        return fail(ProtocolErrorCode::FrameInvalid);
    }
    if contract_tag != u64::from(FRAME_CONTRACT_TAG) {
        return fail(ProtocolErrorCode::FrameInvalid);
    }
    let frame = ProtocolFrame::from_payload(payload)?;
    let decoded = match frame.kind {
        FrameKind::Hello => DecodedFrame::Hello(Hello::decode(&frame.body)?),
        FrameKind::Request => DecodedFrame::Request(frame),
        FrameKind::Response | FrameKind::Event => DecodedFrame::Response(frame),
    };
    Ok((decoded, frame_id))
}

// ---------------------------------------------------------------------------
// Failure envelope
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Retryability {
    Never,
    AfterRequery,
    AfterCapability,
    AfterLimitChange,
    TransientHost,
}

impl Retryability {
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Never => 1,
            Self::AfterRequery => 2,
            Self::AfterCapability => 3,
            Self::AfterLimitChange => 4,
            Self::TransientHost => 5,
        }
    }

    fn from_tag(tag: u64) -> Result<Self> {
        match tag {
            1 => Ok(Self::Never),
            2 => Ok(Self::AfterRequery),
            3 => Ok(Self::AfterCapability),
            4 => Ok(Self::AfterLimitChange),
            5 => Ok(Self::TransientHost),
            _ => fail(ProtocolErrorCode::PayloadInvalid),
        }
    }
}

/// The failure body of a response frame (contract section 6).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolFailure {
    pub code: u32,
    pub symbol: String,
    pub phase: u32,
    pub retryability: Retryability,
    pub incident: Option<[u8; 32]>,
    pub details: Vec<u8>,
}

impl ProtocolFailure {
    /// A protocol-owned failure with no owner details.
    #[must_use]
    pub fn protocol(code: ProtocolErrorCode) -> Self {
        Self {
            code: code.numeric(),
            symbol: code.as_str().to_string(),
            phase: 0,
            retryability: Retryability::Never,
            incident: None,
            details: Vec::new(),
        }
    }

    /// Encodes the failure record.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_PAYLOAD_INVALID` for a non-ASCII or empty symbol.
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.symbol.is_empty()
            || !self
                .symbol
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte == b'_' || byte.is_ascii_digit())
        {
            return fail(ProtocolErrorCode::PayloadInvalid);
        }
        let incident = match self.incident {
            None => scb(encode_union(0, &[]))?,
            Some(digest) => scb(encode_union(1, &digest))?,
        };
        scb(encode_record(&[
            (1, encode_uvar(u64::from(self.code))),
            (2, scb(encode_bytes(self.symbol.as_bytes()))?),
            (3, encode_uvar(u64::from(self.phase))),
            (4, encode_uvar(u64::from(self.retryability.tag()))),
            (5, incident),
            (6, scb(encode_bytes(&self.details))?),
        ]))
    }

    /// Decodes a failure record.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_PAYLOAD_INVALID`.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let invalid = |_| ProtocolError(ProtocolErrorCode::PayloadInvalid);
        let fields = Reader::new(input).record(6).map_err(invalid)?;
        let (incident_tag, incident_payload) = Reader::new(fields[4]).union().map_err(invalid)?;
        let incident = match (incident_tag, incident_payload.len()) {
            (0, 0) => None,
            (1, ID_LEN) => Some(fixed32(incident_payload).map_err(invalid)?),
            _ => return fail(ProtocolErrorCode::PayloadInvalid),
        };
        let symbol = String::from_utf8(Reader::new(fields[1]).bytes().map_err(invalid)?.to_vec())
            .map_err(|_| ProtocolError(ProtocolErrorCode::PayloadInvalid))?;
        let failure = Self {
            code: single_u32(fields[0]).map_err(invalid)?,
            symbol,
            phase: single_u32(fields[2]).map_err(invalid)?,
            retryability: Retryability::from_tag(single_uvar(fields[3]).map_err(invalid)?)?,
            incident,
            details: Reader::new(fields[5]).bytes().map_err(invalid)?.to_vec(),
        };
        failure.encode()?;
        Ok(failure)
    }
}

// ---------------------------------------------------------------------------
// Session-scoped request identity
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
struct SessionState {
    last_request_id: Option<u64>,
    inflight: u32,
    closed: bool,
}

/// Tracks request identity per session (contract section 3).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RequestRegistry {
    sessions: BTreeMap<SessionId, SessionState>,
    /// Close order of remembered sessions. A closed session keeps its
    /// entry so later frames answer `PROTOCOL_SESSION_CLOSED`, but only
    /// the most recently closed names are remembered: closing past the
    /// cap forgets the oldest closed name (which then answers
    /// `SESSION_UNKNOWN` at the session layer), so registry memory is
    /// bounded by live sessions plus remembered closes (contract
    /// section 3).
    close_order: std::collections::VecDeque<SessionId>,
    closed_names: u32,
}

impl RequestRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a session issued by the session authority.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_REQUEST_ID_CONFLICT` when the session already exists.
    pub fn open(&mut self, session: SessionId) -> Result<()> {
        if self.sessions.contains_key(&session) {
            return fail(ProtocolErrorCode::RequestIdConflict);
        }
        self.sessions.insert(
            session,
            SessionState {
                last_request_id: None,
                inflight: 0,
                closed: false,
            },
        );
        Ok(())
    }

    /// Admits a request identifier before execution.
    ///
    /// Identifier 0 is the pre-session sentinel (hello, `session.open`,
    /// and frame-level failure answers) and never enters a session: the
    /// first legal in-session identifier is 1 (contract section 3).
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_SESSION_CLOSED`, `PROTOCOL_REQUEST_ID_CONFLICT`, or
    /// `PROTOCOL_LIMIT_EXCEEDED` in contract precedence.
    pub fn admit(&mut self, session: SessionId, request_id: u64, max_inflight: u32) -> Result<()> {
        let state = self
            .sessions
            .get_mut(&session)
            .ok_or(ProtocolError(ProtocolErrorCode::RequestIdConflict))?;
        if state.closed {
            return fail(ProtocolErrorCode::SessionClosed);
        }
        if request_id == 0 || state.last_request_id.is_some_and(|last| request_id <= last) {
            return fail(ProtocolErrorCode::RequestIdConflict);
        }
        if state.inflight >= max_inflight {
            return fail(ProtocolErrorCode::LimitExceeded);
        }
        state.last_request_id = Some(request_id);
        state.inflight += 1;
        Ok(())
    }

    /// Marks an admitted request complete.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_INTERNAL_INVARIANT` when nothing is in flight.
    pub fn complete(&mut self, session: SessionId) -> Result<()> {
        let state = self
            .sessions
            .get_mut(&session)
            .ok_or(ProtocolError(ProtocolErrorCode::InternalInvariant))?;
        state.inflight = state
            .inflight
            .checked_sub(1)
            .ok_or(ProtocolError(ProtocolErrorCode::InternalInvariant))?;
        Ok(())
    }

    /// Closes a session; every later frame naming it fails. At most
    /// `max_remembered` closed names are remembered: forgetting the
    /// oldest closed name bounds registry memory, and a forgotten name
    /// answers `SESSION_UNKNOWN` instead of `PROTOCOL_SESSION_CLOSED`.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_SESSION_CLOSED` when already closed or unknown.
    pub fn close(&mut self, session: SessionId, max_remembered: u32) -> Result<()> {
        let state = self
            .sessions
            .get_mut(&session)
            .ok_or(ProtocolError(ProtocolErrorCode::SessionClosed))?;
        if state.closed {
            return fail(ProtocolErrorCode::SessionClosed);
        }
        state.closed = true;
        self.close_order.push_back(session);
        self.closed_names = self.closed_names.saturating_add(1);
        // Every close pushes exactly one entry and a name is closed at
        // most once (re-close fails above), so one pop retires exactly
        // one remembered close and the loop always terminates.
        while self.closed_names > max_remembered {
            let Some(oldest) = self.close_order.pop_front() else {
                self.closed_names = self.closed_names.min(max_remembered);
                break;
            };
            self.closed_names = self.closed_names.saturating_sub(1);
            if self.sessions.get(&oldest).is_some_and(|state| state.closed) {
                self.sessions.remove(&oldest);
            }
        }
        Ok(())
    }

    /// Whether the registry remembers this session as closed (a later
    /// frame answers `PROTOCOL_SESSION_CLOSED`). A name the registry
    /// never saw, or forgot after the remembered-close cap, is not
    /// closed here: the session layer answers `SESSION_UNKNOWN`.
    #[must_use]
    pub fn is_closed(&self, session: SessionId) -> bool {
        self.sessions
            .get(&session)
            .is_some_and(|state| state.closed)
    }

    #[must_use]
    pub fn is_open(&self, session: SessionId) -> bool {
        self.sessions
            .get(&session)
            .is_some_and(|state| !state.closed)
    }
}

// ---------------------------------------------------------------------------
// Minimal SCB1 reader
// ---------------------------------------------------------------------------

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(ProtocolError(ProtocolErrorCode::FrameInvalid))?;
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn uvar(&mut self) -> Result<u64> {
        let mut value = 0_u64;
        let mut shift = 0_u32;
        loop {
            let byte = *self
                .bytes
                .get(self.offset)
                .ok_or(ProtocolError(ProtocolErrorCode::FrameInvalid))?;
            self.offset += 1;
            if shift >= 64 || (shift == 63 && byte & 0x7e != 0) {
                return fail(ProtocolErrorCode::FrameInvalid);
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                if byte == 0 && shift != 0 {
                    // Non-minimal encoding.
                    return fail(ProtocolErrorCode::FrameInvalid);
                }
                return Ok(value);
            }
            shift += 7;
        }
    }

    fn fixed32(&mut self) -> Result<[u8; 32]> {
        fixed32(self.take(ID_LEN)?)
    }

    fn bytes(&mut self) -> Result<&'a [u8]> {
        let len = usize::try_from(self.uvar()?)
            .map_err(|_| ProtocolError(ProtocolErrorCode::FrameInvalid))?;
        let out = self.take(len)?;
        if !self.finished() {
            return fail(ProtocolErrorCode::FrameInvalid);
        }
        Ok(out)
    }

    fn union(&mut self) -> Result<(u64, &'a [u8])> {
        let tag = self.uvar()?;
        let len = usize::try_from(self.uvar()?)
            .map_err(|_| ProtocolError(ProtocolErrorCode::FrameInvalid))?;
        let payload = self.take(len)?;
        if !self.finished() {
            return fail(ProtocolErrorCode::FrameInvalid);
        }
        Ok((tag, payload))
    }

    /// Reads a record with exactly the fields 1 through `expected`.
    fn record(&mut self, expected: u64) -> Result<Vec<&'a [u8]>> {
        let count = self.uvar()?;
        if count != expected {
            return fail(ProtocolErrorCode::FrameInvalid);
        }
        let mut fields = Vec::with_capacity(usize::try_from(expected).unwrap_or(0));
        for index in 1..=expected {
            if self.uvar()? != index {
                return fail(ProtocolErrorCode::FrameInvalid);
            }
            let len = usize::try_from(self.uvar()?)
                .map_err(|_| ProtocolError(ProtocolErrorCode::FrameInvalid))?;
            fields.push(self.take(len)?);
        }
        if !self.finished() {
            return fail(ProtocolErrorCode::FrameInvalid);
        }
        Ok(fields)
    }

    fn list(&mut self, maximum: usize) -> Result<Vec<&'a [u8]>> {
        let count = usize::try_from(self.uvar()?)
            .map_err(|_| ProtocolError(ProtocolErrorCode::FrameInvalid))?;
        if count > maximum {
            return fail(ProtocolErrorCode::LimitExceeded);
        }
        let mut items = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            let len = usize::try_from(self.uvar()?)
                .map_err(|_| ProtocolError(ProtocolErrorCode::FrameInvalid))?;
            items.push(self.take(len)?);
        }
        if !self.finished() {
            return fail(ProtocolErrorCode::FrameInvalid);
        }
        Ok(items)
    }
}

fn fixed32(input: &[u8]) -> Result<[u8; 32]> {
    input
        .try_into()
        .map_err(|_| ProtocolError(ProtocolErrorCode::FrameInvalid))
}

fn single_uvar(input: &[u8]) -> Result<u64> {
    let mut reader = Reader::new(input);
    let value = reader.uvar()?;
    if !reader.finished() {
        return fail(ProtocolErrorCode::FrameInvalid);
    }
    Ok(value)
}

fn single_u32(input: &[u8]) -> Result<u32> {
    u32::try_from(single_uvar(input)?).map_err(|_| ProtocolError(ProtocolErrorCode::FrameInvalid))
}

fn strictly_increasing<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn uvar_list(values: &[u32]) -> Result<Vec<u8>> {
    let elements: Vec<Vec<u8>> = values
        .iter()
        .map(|value| encode_uvar(u64::from(*value)))
        .collect();
    scb(encode_list(&elements))
}

fn fixed_list(values: &[[u8; 32]]) -> Result<Vec<u8>> {
    let elements: Vec<Vec<u8>> = values.iter().map(|value| value.to_vec()).collect();
    scb(encode_list(&elements))
}

fn u32_list(input: &[u8]) -> Result<Vec<u32>> {
    Reader::new(input)
        .list(MAX_HELLO_LIST)
        .map_err(|_| ProtocolError(ProtocolErrorCode::PayloadInvalid))?
        .into_iter()
        .map(|item| single_u32(item).map_err(|_| ProtocolError(ProtocolErrorCode::PayloadInvalid)))
        .collect()
}

fn fixed32_list(input: &[u8]) -> Result<Vec<[u8; 32]>> {
    Reader::new(input)
        .list(MAX_HELLO_LIST)
        .map_err(|_| ProtocolError(ProtocolErrorCode::PayloadInvalid))?
        .into_iter()
        .map(|item| fixed32(item).map_err(|_| ProtocolError(ProtocolErrorCode::PayloadInvalid)))
        .collect()
}

// ---------------------------------------------------------------------------
// Streaming (S20-440)
// ---------------------------------------------------------------------------

/// Bytes of a frame that are not body: length prefix, envelope header with
/// the longest uvars, the frame record with a session, a full bounded
/// context, and the trailer. Chunk sizing subtracts it from the ceiling.
pub const STREAM_FRAME_OVERHEAD: u64 = 512;
/// Smallest useful chunk; a ceiling that cannot carry it is not streamable.
pub const MIN_STREAM_CHUNK_BYTES: u64 = 64;

/// One event frame's chunk record (contract appendix B).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamChunk {
    pub index: u64,
    pub total: u64,
    pub bytes: Vec<u8>,
}

impl StreamChunk {
    /// Encodes the chunk record.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_INTERNAL_INVARIANT` on an encoding defect.
    pub fn encode(&self) -> Result<Vec<u8>> {
        scb(encode_record(&[
            (1, encode_uvar(self.index)),
            (2, encode_uvar(self.total)),
            (3, scb(encode_bytes(&self.bytes))?),
        ]))
    }

    /// Decodes a chunk record.
    ///
    /// # Errors
    ///
    /// Returns `PROTOCOL_FRAME_INVALID` on any shape defect.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let fields = Reader::new(input).record(3)?;
        Ok(Self {
            index: single_uvar(fields[0])?,
            total: single_uvar(fields[1])?,
            bytes: Reader::new(fields[2]).bytes()?.to_vec(),
        })
    }
}

/// Splits one response into event frames plus a final response frame when
/// its body does not fit the ceiling.
///
/// The final response carries the bounded context and an empty body; every
/// event frame carries one chunk record under the `stream` flag. A body
/// that fits is returned as the single response frame unchanged.
///
/// # Errors
///
/// Returns `PROTOCOL_LIMIT_EXCEEDED` when streaming is not negotiated or
/// the ceiling cannot carry a chunk, and encoding failures otherwise.
pub fn stream_response(
    response: &ProtocolFrame,
    max_frame_bytes: u64,
    stream_negotiated: bool,
) -> Result<Vec<EncodedFrame>> {
    if response.kind != FrameKind::Response {
        return fail(ProtocolErrorCode::FrameInvalid);
    }
    let ceiling = max_frame_bytes.min(MAX_FRAME_BYTES);
    let single = encode_frame(response)?;
    if u64::try_from(single.bytes.len()).is_ok_and(|len| len <= ceiling) {
        return Ok(vec![single]);
    }
    if !stream_negotiated {
        return fail(ProtocolErrorCode::LimitExceeded);
    }
    let chunk_bytes = ceiling
        .checked_sub(STREAM_FRAME_OVERHEAD)
        .filter(|bytes| *bytes >= MIN_STREAM_CHUNK_BYTES)
        .ok_or(ProtocolError(ProtocolErrorCode::LimitExceeded))?;
    let chunk_len = usize::try_from(chunk_bytes)
        .map_err(|_| ProtocolError(ProtocolErrorCode::LimitExceeded))?;
    let chunks: Vec<&[u8]> = response.body.chunks(chunk_len).collect();
    let total =
        u64::try_from(chunks.len()).map_err(|_| ProtocolError(ProtocolErrorCode::LimitExceeded))?;
    let mut frames = Vec::with_capacity(chunks.len() + 1);
    for (index, chunk) in chunks.iter().enumerate() {
        let record = StreamChunk {
            index: u64::try_from(index)
                .map_err(|_| ProtocolError(ProtocolErrorCode::LimitExceeded))?,
            total,
            bytes: chunk.to_vec(),
        }
        .encode()?;
        let event = ProtocolFrame {
            kind: FrameKind::Event,
            // A streamed failed response keeps its failure bit on every
            // event frame (contract section 6): the bit is part of the
            // response, not of the chunking.
            flags: response.flags | FLAG_STREAM,
            bounds: BoundedContext::none(),
            body: record,
            ..response.clone()
        };
        let encoded = encode_frame(&event)?;
        if u64::try_from(encoded.bytes.len()).map_or(true, |len| len > ceiling) {
            return fail(ProtocolErrorCode::LimitExceeded);
        }
        frames.push(encoded);
    }
    let last = ProtocolFrame {
        flags: response.flags | FLAG_STREAM,
        body: Vec::new(),
        ..response.clone()
    };
    frames.push(encode_frame(&last)?);
    Ok(frames)
}

/// Reassembles a streamed response from its event frames and final frame.
///
/// # Errors
///
/// Returns `PROTOCOL_FRAME_INVALID` when the chunks are not exactly
/// `0..total` in order under one session, request, and method, or the
/// final frame is not the stream's response.
pub fn reassemble_stream(frames: &[ProtocolFrame]) -> Result<ProtocolFrame> {
    let Some((last, events)) = frames.split_last() else {
        return fail(ProtocolErrorCode::FrameInvalid);
    };
    if last.kind != FrameKind::Response || last.flags & FLAG_STREAM == 0 || !last.body.is_empty() {
        return fail(ProtocolErrorCode::FrameInvalid);
    }
    // The failure bit is part of the streamed response: every event and
    // the terminal frame carry the same bit (contract section 6).
    let failed = last.flags & FLAG_FAILED;
    let mut body = Vec::new();
    let mut expected_total = None;
    for (index, event) in events.iter().enumerate() {
        if event.kind != FrameKind::Event
            || event.flags & FLAG_STREAM == 0
            || event.flags & FLAG_FAILED != failed
            || event.session != last.session
            || event.request_id != last.request_id
            || event.method != last.method
        {
            return fail(ProtocolErrorCode::FrameInvalid);
        }
        let chunk = StreamChunk::decode(&event.body)?;
        let position =
            u64::try_from(index).map_err(|_| ProtocolError(ProtocolErrorCode::FrameInvalid))?;
        if chunk.index != position || expected_total.is_some_and(|total| total != chunk.total) {
            return fail(ProtocolErrorCode::FrameInvalid);
        }
        expected_total = Some(chunk.total);
        body.extend_from_slice(&chunk.bytes);
    }
    let events_len =
        u64::try_from(events.len()).map_err(|_| ProtocolError(ProtocolErrorCode::FrameInvalid))?;
    if expected_total.is_none_or(|total| total != events_len) || events_len == 0 {
        return fail(ProtocolErrorCode::FrameInvalid);
    }
    Ok(ProtocolFrame {
        flags: last.flags,
        body,
        ..last.clone()
    })
}

#[cfg(test)]
#[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
mod tests {
    use super::*;

    fn epoch(byte: u8) -> SchemaEpochId {
        SchemaEpochId::from_bytes([byte; 32])
    }

    fn session(byte: u8) -> SessionId {
        SessionId::from_bytes([byte; 32])
    }

    fn client_hello() -> Hello {
        Hello {
            protocol_versions: vec![1, 2],
            schema_epochs: vec![epoch(0x11), epoch(0x12)],
            limits: LimitProfile {
                max_frame_bytes: 1_048_576,
                max_entities: 1_000,
                max_edges: 10_000,
                max_depth: 16,
                max_response_bytes: 1_048_576,
                max_work: 1_000_000,
                max_inflight: 4,
                max_sessions: 16,
            },
            methods: Method::ALL
                .iter()
                .filter(|method| !method.is_reserved())
                .map(|method| method.tag())
                .collect(),
            features: FEATURE_CANCEL | FEATURE_STREAM | FEATURE_JSON_BRIDGE,
            adapters: vec![[0xA1; 32], [0xA2; 32]],
            effects: vec![[0xE1; 32]],
        }
    }

    fn server_hello() -> Hello {
        Hello {
            protocol_versions: vec![1],
            schema_epochs: vec![epoch(0x12), epoch(0x11)],
            limits: LimitProfile {
                max_frame_bytes: 4_194_304,
                max_entities: 500,
                max_edges: 20_000,
                max_depth: 8,
                max_response_bytes: 2_097_152,
                max_work: 5_000_000,
                max_inflight: 2,
                max_sessions: 8,
            },
            methods: vec![100, 102, 103, 300, 301, 302, 303, 603],
            features: FEATURE_CANCEL | FEATURE_CHECKSUM,
            adapters: vec![[0xA2; 32]],
            effects: vec![],
        }
    }

    fn request(body: &[u8]) -> ProtocolFrame {
        ProtocolFrame {
            protocol_version: PROTOCOL_VERSION,
            session: Some(session(0x51)),
            request_id: 7,
            kind: FrameKind::Request,
            method: Method::QueryRoot.tag(),
            flags: 0,
            bounds: BoundedContext::none(),
            body: body.to_vec(),
        }
    }

    #[test]
    fn frames_round_trip_and_are_verified_before_any_field_is_read() {
        let frame = request(b"SLEYRQQ1-body");
        let encoded = encode_frame(&frame).unwrap();
        assert_eq!(&encoded.bytes[8..16], b"SLEYSCB1");
        let (decoded, frame_id) = decode_frame(&encoded.bytes, MAX_FRAME_BYTES).unwrap();
        assert_eq!(decoded, DecodedFrame::Request(frame.clone()));
        assert_eq!(frame_id, encoded.frame_id);
        for _ in 0..128 {
            assert_eq!(encode_frame(&frame).unwrap(), encoded);
        }
        // A response with bounded context.
        let response = ProtocolFrame {
            kind: FrameKind::Response,
            bounds: BoundedContext {
                applied_limits: client_hello().limits,
                returned_bytes: 300,
                returned_entities: 3,
                returned_edges: 0,
                reached_depth: 0,
                omitted: 1,
                truncated: true,
                continuation: true,
            },
            ..frame.clone()
        };
        let encoded_response = encode_frame(&response).unwrap();
        let (decoded, _) = decode_frame(&encoded_response.bytes, MAX_FRAME_BYTES).unwrap();
        assert_eq!(decoded, DecodedFrame::Response(response));
        // Length is checked before allocation: a prefix above the ceiling is
        // refused without reading the envelope.
        let mut huge = encoded.bytes.clone();
        huge[..8].copy_from_slice(&(MAX_FRAME_BYTES + 1).to_be_bytes());
        assert_eq!(
            decode_frame(&huge, MAX_FRAME_BYTES).unwrap_err().code(),
            ProtocolErrorCode::FrameTooLarge
        );
        assert_eq!(
            frame_length(&encoded.bytes, 16).unwrap_err().code(),
            ProtocolErrorCode::FrameTooLarge
        );
        // Digest, magic, version, epoch, tag, kind, and flag defects.
        let mut flipped = encoded.bytes.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 1;
        assert_eq!(
            decode_frame(&flipped, MAX_FRAME_BYTES).unwrap_err().code(),
            ProtocolErrorCode::FrameInvalid
        );
        let mut short = encoded.bytes.clone();
        short.pop();
        assert_eq!(
            decode_frame(&short, MAX_FRAME_BYTES).unwrap_err().code(),
            ProtocolErrorCode::FrameInvalid
        );
        // A hello-kind frame cannot be encoded as an ordinary frame, and a
        // hello frame with a non-hello body is rejected.
        let mut wrong_tag = request(b"x");
        wrong_tag.kind = FrameKind::Hello;
        assert_eq!(
            encode_frame(&wrong_tag).unwrap_err().code(),
            ProtocolErrorCode::FrameInvalid
        );
        let bad_hello = ProtocolFrame {
            session: None,
            request_id: 0,
            method: 0,
            ..wrong_tag
        };
        let bad_hello_bytes =
            encode_envelope(FrameKind::Hello, &bad_hello.payload().unwrap()).unwrap();
        assert_eq!(
            decode_frame(&bad_hello_bytes.bytes, MAX_FRAME_BYTES)
                .unwrap_err()
                .code(),
            ProtocolErrorCode::PayloadInvalid
        );
        let mut bad_flags = request(b"x");
        bad_flags.flags = 0x10;
        assert_eq!(
            encode_frame(&bad_flags).unwrap_err().code(),
            ProtocolErrorCode::FrameInvalid
        );
        let mut bad_version = request(b"x");
        bad_version.protocol_version = 2;
        assert_eq!(
            encode_frame(&bad_version).unwrap_err().code(),
            ProtocolErrorCode::VersionUnsupported
        );
    }

    #[test]
    fn negotiation_is_derived_digested_and_downgrade_is_detected() {
        let client = client_hello();
        let server = server_hello();
        let selected = negotiate(&client, &server).unwrap();
        assert_eq!(selected.protocol_version, 1);
        assert_eq!(
            selected.schema_epoch,
            epoch(0x12),
            "first server epoch the client lists"
        );
        assert_eq!(selected.limits, client.limits.minimum(&server.limits));
        assert_eq!(selected.limits.max_inflight, 2);
        assert_eq!(
            selected.methods,
            vec![100, 102, 103, 300, 301, 302, 303, 603]
        );
        assert_eq!(selected.features, FEATURE_CANCEL);
        assert_eq!(selected.adapters, vec![[0xA2; 32]]);
        assert!(selected.effects.is_empty());
        let id = negotiate_identity(&client, &server).unwrap().1;
        // Both peers derive the same selection and transcript-bound identity.
        assert_eq!(negotiate_identity(&client, &server).unwrap().1, id);
        assert_eq!(
            selected
                .handshake_id_bound(&client.encode().unwrap(), &server.encode().unwrap())
                .unwrap(),
            id
        );
        // Threat T45: a client hello stripped of version 2 on the wire
        // negotiates the same selection (version 1 either way) but a
        // different transcript, so the server's identity differs from the
        // honest client's and `session.open` fails closed. A
        // selection-only digest would call these identical.
        let mut stripped = client.clone();
        stripped.protocol_versions = vec![1];
        let (stripped_selected, stripped_id) = negotiate_identity(&stripped, &server).unwrap();
        assert_eq!(stripped_selected, selected);
        assert_ne!(stripped_id, id);
        assert!(selected.admits(Method::QueryRoot));
        assert!(!selected.admits(Method::Commit));
        // Hello frames round trip.
        let encoded = encode_hello_frame(&client).unwrap();
        let (decoded, _) = decode_frame(&encoded.bytes, MAX_FRAME_BYTES).unwrap();
        assert_eq!(decoded, DecodedFrame::Hello(client.clone()));
        // No common profile.
        let mut other_version = server.clone();
        other_version.protocol_versions = vec![3];
        assert_eq!(
            negotiate(&client, &other_version).unwrap_err().code(),
            ProtocolErrorCode::NoCommonProfile
        );
        let mut other_epoch = server.clone();
        other_epoch.schema_epochs = vec![epoch(0x99)];
        assert_eq!(
            negotiate(&client, &other_epoch).unwrap_err().code(),
            ProtocolErrorCode::NoCommonProfile
        );
        let mut other_methods = server.clone();
        other_methods.methods = vec![604];
        let mut client_no_report = client.clone();
        client_no_report.methods = vec![100, 300];
        assert_eq!(
            negotiate(&client_no_report, &other_methods)
                .unwrap_err()
                .code(),
            ProtocolErrorCode::NoCommonProfile
        );
        // Downgrade claims.
        assert_eq!(selected.check_claim(1, epoch(0x12)), Ok(()));
        assert_eq!(
            selected.check_claim(0, epoch(0x12)).unwrap_err().code(),
            ProtocolErrorCode::Downgrade
        );
        assert_eq!(
            selected.check_claim(1, epoch(0x11)).unwrap_err().code(),
            ProtocolErrorCode::Downgrade
        );
        assert_eq!(
            selected.check_claim(2, epoch(0x12)).unwrap_err().code(),
            ProtocolErrorCode::VersionUnsupported
        );
        // Shape defects.
        let mut unsorted = client.clone();
        unsorted.methods = vec![300, 100];
        assert_eq!(
            unsorted.validate().unwrap_err().code(),
            ProtocolErrorCode::PayloadInvalid
        );
        let mut bad_feature = client.clone();
        bad_feature.features = 0x100;
        assert_eq!(
            bad_feature.validate().unwrap_err().code(),
            ProtocolErrorCode::PayloadInvalid
        );
        let mut bad_limits = client.clone();
        bad_limits.limits.max_work = 0;
        assert_eq!(
            bad_limits.validate().unwrap_err().code(),
            ProtocolErrorCode::LimitExceeded
        );
        let mut too_big = client.clone();
        too_big.limits.max_frame_bytes = MAX_FRAME_BYTES + 1;
        assert_eq!(
            too_big.validate().unwrap_err().code(),
            ProtocolErrorCode::LimitExceeded
        );
    }

    #[test]
    fn request_identity_is_session_scoped_and_strictly_increasing() {
        let mut registry = RequestRegistry::new();
        let a = session(0xA0);
        let b = session(0xB0);
        registry.open(a).unwrap();
        registry.open(b).unwrap();
        assert_eq!(
            registry.open(a).unwrap_err().code(),
            ProtocolErrorCode::RequestIdConflict
        );
        registry.admit(a, 1, 2).unwrap();
        registry.admit(a, 5, 2).unwrap();
        assert_eq!(
            registry.admit(a, 5, 2).unwrap_err().code(),
            ProtocolErrorCode::RequestIdConflict
        );
        assert_eq!(
            registry.admit(a, 3, 2).unwrap_err().code(),
            ProtocolErrorCode::RequestIdConflict
        );
        // Inflight ceiling, then completion frees a slot.
        assert_eq!(
            registry.admit(a, 6, 2).unwrap_err().code(),
            ProtocolErrorCode::LimitExceeded
        );
        registry.complete(a).unwrap();
        registry.admit(a, 6, 2).unwrap();
        // Identifiers are scoped: session b starts its own sequence.
        registry.admit(b, 1, 2).unwrap();
        assert_eq!(
            registry.admit(session(0xC0), 1, 2).unwrap_err().code(),
            ProtocolErrorCode::RequestIdConflict
        );
        // Closed sessions refuse everything.
        registry.close(a, 8).unwrap();
        assert!(!registry.is_open(a));
        assert!(registry.is_closed(a));
        assert_eq!(
            registry.admit(a, 9, 2).unwrap_err().code(),
            ProtocolErrorCode::SessionClosed
        );
        assert_eq!(
            registry.close(a, 8).unwrap_err().code(),
            ProtocolErrorCode::SessionClosed
        );
        assert!(registry.is_open(b));
        assert!(!registry.is_closed(b));
        assert!(!registry.is_closed(session(0xC0)));
        // Remembered closes are capped: closing past the cap forgets the
        // oldest closed name, which is then unknown rather than closed.
        registry.close(b, 1).unwrap();
        assert!(!registry.is_closed(a));
        assert!(registry.is_closed(b));
        assert_eq!(
            registry.admit(a, 10, 2).unwrap_err().code(),
            ProtocolErrorCode::RequestIdConflict
        );
    }

    #[test]
    fn method_table_and_failure_envelope_are_frozen() {
        assert_eq!(Method::ALL.len(), 41);
        let tags: Vec<u32> = Method::ALL.iter().map(|method| method.tag()).collect();
        assert!(strictly_increasing(&tags));
        assert_eq!(tags[0], 100);
        assert_eq!(tags[40], 604);
        for method in Method::ALL {
            assert_eq!(Method::from_tag(method.tag()).unwrap(), method);
            assert!(
                method
                    .name()
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'_')
            );
        }
        assert_eq!(
            Method::from_tag(999).unwrap_err().code(),
            ProtocolErrorCode::MethodUnsupported
        );
        assert_eq!(
            Method::ALL
                .iter()
                .filter(|method| method.is_reserved())
                .count(),
            4
        );
        assert_eq!(Method::GcDryRun.name(), "gc.dry_run");
        assert_eq!(Method::Report.family(), 6);
        assert_eq!(ProtocolErrorCode::ALL.len(), 12);
        for pair in ProtocolErrorCode::ALL.windows(2) {
            assert!(pair[0].numeric() < pair[1].numeric());
        }
        assert_eq!(ProtocolErrorCode::ALL[0].numeric(), 40_000);
        assert_eq!(ProtocolErrorCode::ALL[11].numeric(), 40_011);
        let failure = ProtocolFailure {
            code: 31_006,
            symbol: "QUERY_REQUIRED_FACT_OMITTED".to_string(),
            phase: 7,
            retryability: Retryability::AfterLimitChange,
            incident: None,
            details: vec![1, 2, 3],
        };
        let encoded = failure.encode().unwrap();
        assert_eq!(ProtocolFailure::decode(&encoded).unwrap(), failure);
        let internal = ProtocolFailure {
            incident: Some([0x77; 32]),
            ..ProtocolFailure::protocol(ProtocolErrorCode::InternalInvariant)
        };
        assert_eq!(
            ProtocolFailure::decode(&internal.encode().unwrap()).unwrap(),
            internal
        );
        let mut bad_symbol = failure.clone();
        bad_symbol.symbol = "lower case".to_string();
        assert_eq!(
            bad_symbol.encode().unwrap_err().code(),
            ProtocolErrorCode::PayloadInvalid
        );
        assert_eq!(
            ProtocolFailure::decode(b"garbage").unwrap_err().code(),
            ProtocolErrorCode::PayloadInvalid
        );
        // The epoch record is stable.
        let epoch_id = protocol_epoch_id().unwrap();
        assert_eq!(protocol_epoch_id().unwrap(), epoch_id);
        assert_eq!(protocol_epoch_record().contracts.len(), 1);
    }

    #[test]
    fn streaming_splits_reassembles_and_fails_closed_without_the_feature() {
        let body = vec![0xAB; 5_000];
        let response = ProtocolFrame {
            kind: FrameKind::Response,
            body: body.clone(),
            bounds: BoundedContext {
                applied_limits: client_hello().limits,
                returned_bytes: 5_000,
                ..BoundedContext::none()
            },
            ..request(b"")
        };
        let ceiling = 2_048;
        let frames = stream_response(&response, ceiling, true).unwrap();
        assert!(frames.len() >= 4, "{} frames", frames.len());
        for frame in &frames {
            assert!(frame.bytes.len() as u64 <= ceiling);
        }
        let decoded: Vec<ProtocolFrame> = frames
            .iter()
            .map(
                |frame| match decode_frame(&frame.bytes, ceiling).unwrap().0 {
                    DecodedFrame::Response(frame) => frame,
                    DecodedFrame::Request(_) | DecodedFrame::Hello(_) => {
                        panic!("event or response")
                    }
                },
            )
            .collect();
        assert!(
            decoded[..decoded.len() - 1]
                .iter()
                .all(|frame| frame.kind == FrameKind::Event)
        );
        let reassembled = reassemble_stream(&decoded).unwrap();
        assert_eq!(reassembled.body, body);
        assert_eq!(reassembled.bounds, response.bounds);
        assert_eq!(reassembled.request_id, response.request_id);
        // A body that fits is one frame; without the feature a large body fails closed.
        let small = ProtocolFrame {
            body: vec![1, 2, 3],
            ..response.clone()
        };
        assert_eq!(stream_response(&small, ceiling, false).unwrap().len(), 1);
        assert_eq!(
            stream_response(&response, ceiling, false)
                .unwrap_err()
                .code(),
            ProtocolErrorCode::LimitExceeded
        );
        assert_eq!(
            stream_response(&response, 100, true).unwrap_err().code(),
            ProtocolErrorCode::LimitExceeded
        );
        // Reassembly rejects reordering, missing chunks, and foreign frames.
        let mut swapped = decoded.clone();
        swapped.swap(0, 1);
        assert_eq!(
            reassemble_stream(&swapped).unwrap_err().code(),
            ProtocolErrorCode::FrameInvalid
        );
        let mut missing = decoded.clone();
        missing.remove(1);
        assert_eq!(
            reassemble_stream(&missing).unwrap_err().code(),
            ProtocolErrorCode::FrameInvalid
        );
        let mut foreign = decoded.clone();
        foreign[0].request_id += 1;
        assert_eq!(
            reassemble_stream(&foreign).unwrap_err().code(),
            ProtocolErrorCode::FrameInvalid
        );
        assert_eq!(
            reassemble_stream(&[]).unwrap_err().code(),
            ProtocolErrorCode::FrameInvalid
        );
        for _ in 0..128 {
            assert_eq!(stream_response(&response, ceiling, true).unwrap(), frames);
        }
    }

    #[test]
    fn decode_splits_unknown_versions_below_and_above() {
        // Below the implementation version is a downgrade attempt;
        // above it names a version this code does not know (contract
        // section 2). The codec classifies; the selection-level split in
        // `check_claim` agrees.
        let frame = request(b"");
        for (claimed, code) in [
            (0, ProtocolErrorCode::Downgrade),
            (PROTOCOL_VERSION + 1, ProtocolErrorCode::VersionUnsupported),
        ] {
            let mut wire = frame.clone();
            wire.protocol_version = claimed;
            let bytes = encode_envelope(wire.kind, &wire.payload().unwrap())
                .unwrap()
                .bytes;
            assert_eq!(
                decode_frame(&bytes, 1_048_576).unwrap_err().code(),
                code,
                "claimed version {claimed}"
            );
        }
    }

    #[test]
    fn envelope_format_version_mismatch_is_unsupported() {
        // The SCB1 envelope format version is a version failure, not a
        // frame defect (contract section 1): a foreign format version
        // answers PROTOCOL_VERSION_UNSUPPORTED.
        let frame = request(b"");
        let payload = frame.payload().unwrap();
        let mut preimage = Vec::new();
        preimage.extend_from_slice(MAGIC);
        preimage.extend_from_slice(&encode_uvar(FORMAT_VERSION + 1));
        preimage.extend_from_slice(&encode_uvar(u64::from(FRAME_CONTRACT_TAG)));
        preimage.extend_from_slice(protocol_epoch_id().unwrap().as_bytes());
        preimage.extend_from_slice(&encode_uvar(payload.len() as u64));
        preimage.extend_from_slice(&payload);
        let frame_id = ProtocolFrameId::derive(&preimage);
        let envelope_len = (preimage.len() + ID_LEN) as u64;
        let mut bytes = envelope_len.to_be_bytes().to_vec();
        bytes.extend_from_slice(&preimage);
        bytes.extend_from_slice(frame_id.as_bytes());
        assert_eq!(
            decode_frame(&bytes, 1_048_576).unwrap_err().code(),
            ProtocolErrorCode::VersionUnsupported
        );
    }

    #[test]
    fn streamed_failure_keeps_the_failed_bit_on_every_frame() {
        // A streamed failed response keeps its failure bit on every event
        // frame and the terminal frame (contract section 6); reassembly
        // requires the bit to agree everywhere.
        let body = vec![0xCD; 5_000];
        let failed = ProtocolFrame {
            kind: FrameKind::Response,
            flags: FLAG_FAILED,
            body: body.clone(),
            bounds: BoundedContext {
                applied_limits: client_hello().limits,
                ..BoundedContext::none()
            },
            ..request(b"")
        };
        let ceiling = 2_048;
        let frames = stream_response(&failed, ceiling, true).unwrap();
        assert!(frames.len() >= 4, "{} frames", frames.len());
        let decoded: Vec<ProtocolFrame> = frames
            .iter()
            .map(
                |frame| match decode_frame(&frame.bytes, ceiling).unwrap().0 {
                    DecodedFrame::Response(frame) => frame,
                    DecodedFrame::Request(_) | DecodedFrame::Hello(_) => {
                        panic!("event or response")
                    }
                },
            )
            .collect();
        assert!(
            decoded
                .iter()
                .all(|frame| frame.flags == FLAG_STREAM | FLAG_FAILED)
        );
        let terminal = decoded.last().unwrap();
        assert_eq!(terminal.kind, FrameKind::Response);
        assert!(terminal.body.is_empty());
        let reassembled = reassemble_stream(&decoded).unwrap();
        assert_eq!(reassembled.body, body);
        assert_eq!(reassembled.flags, FLAG_STREAM | FLAG_FAILED);
        // A cleared bit anywhere fails reassembly.
        let mut cleared = decoded.clone();
        cleared[0].flags = FLAG_STREAM;
        assert_eq!(
            reassemble_stream(&cleared).unwrap_err().code(),
            ProtocolErrorCode::FrameInvalid
        );
    }

    fn hex(bytes: &[u8]) -> String {
        use core::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
    }

    /// Emits the frozen frame, hello, and failure vectors for
    /// `scripts/generate_smp1_fixtures.py`.
    #[test]
    #[ignore = "fixture refresh emitter; run through the generator script"]
    fn emit_smp1_vectors_for_fixture_refresh() {
        println!(
            "SMP1_EPOCH|{}",
            hex(protocol_epoch_id().unwrap().as_bytes())
        );
        let client = client_hello();
        let server = server_hello();
        let selected = negotiate(&client, &server).unwrap();
        let (bound_selected, bound_id) = negotiate_identity(&client, &server).unwrap();
        assert_eq!(bound_selected, selected);
        let client_frame = encode_hello_frame(&client).unwrap();
        let server_frame = encode_hello_frame(&server).unwrap();
        println!(
            "SMP1_HELLO|client|{}|{}",
            hex(&client_frame.bytes),
            hex(client_frame.frame_id.as_bytes())
        );
        println!(
            "SMP1_HELLO|server|{}|{}",
            hex(&server_frame.bytes),
            hex(server_frame.frame_id.as_bytes())
        );
        println!(
            "SMP1_SELECTED|{}|{}|{}|{}|{}|{}|{}",
            selected.protocol_version,
            hex(selected.schema_epoch.as_bytes()),
            hex(&selected.preimage().unwrap()),
            hex(bound_id.as_bytes()),
            selected.features,
            selected
                .methods
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(","),
            hex(&handshake_transcript(
                &client.encode().unwrap(),
                &server.encode().unwrap(),
                &selected.preimage().unwrap()
            )),
        );
        let req = request(b"SLEYRQQ1-body");
        let encoded = encode_frame(&req).unwrap();
        println!(
            "SMP1_FRAME|request|{}|{}",
            hex(&encoded.bytes),
            hex(encoded.frame_id.as_bytes())
        );
        let response = ProtocolFrame {
            kind: FrameKind::Response,
            bounds: BoundedContext {
                applied_limits: selected.limits,
                returned_bytes: 300,
                returned_entities: 3,
                returned_edges: 0,
                reached_depth: 0,
                omitted: 1,
                truncated: true,
                continuation: true,
            },
            body: b"SLEYRQR1-body".to_vec(),
            ..req.clone()
        };
        let encoded = encode_frame(&response).unwrap();
        println!(
            "SMP1_FRAME|response|{}|{}",
            hex(&encoded.bytes),
            hex(encoded.frame_id.as_bytes())
        );
        let failure = ProtocolFailure {
            code: 31_006,
            symbol: "QUERY_REQUIRED_FACT_OMITTED".to_string(),
            phase: 7,
            retryability: Retryability::AfterLimitChange,
            incident: None,
            details: vec![1, 2, 3],
        };
        let failure_frame = ProtocolFrame {
            kind: FrameKind::Response,
            flags: FLAG_FAILED,
            body: failure.encode().unwrap(),
            ..req.clone()
        };
        let encoded = encode_frame(&failure_frame).unwrap();
        println!(
            "SMP1_FRAME|failure|{}|{}",
            hex(&encoded.bytes),
            hex(encoded.frame_id.as_bytes())
        );
        let rejects: Vec<(&str, Vec<u8>, ProtocolErrorCode)> = {
            let base = encode_frame(&req).unwrap().bytes;
            let mut huge = base.clone();
            huge[..8].copy_from_slice(&(MAX_FRAME_BYTES + 1).to_be_bytes());
            let mut flipped = base.clone();
            let last = flipped.len() - 1;
            flipped[last] ^= 1;
            let mut short = base.clone();
            short.pop();
            let mut magic = base.clone();
            magic[8] ^= 1;
            vec![
                (
                    "length-above-ceiling",
                    huge,
                    ProtocolErrorCode::FrameTooLarge,
                ),
                (
                    "digest-trailer-bit",
                    flipped,
                    ProtocolErrorCode::FrameInvalid,
                ),
                ("truncated-envelope", short, ProtocolErrorCode::FrameInvalid),
                ("magic-bit", magic, ProtocolErrorCode::FrameInvalid),
            ]
        };
        for (label, bytes, expected) in rejects {
            let code = decode_frame(&bytes, MAX_FRAME_BYTES).unwrap_err().code();
            assert_eq!(code, expected);
            println!(
                "SMP1_REJECT|{label}|{}|{}|{}",
                hex(&bytes),
                code.as_str(),
                code.numeric()
            );
        }
    }
}

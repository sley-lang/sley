//! The workbench's own refusals: the `AGENT_*` namespace of
//! `docs/spec/SLEY_AGENT_V1.md`. They name tool-side failures only. A
//! candidate's validity is never an `AgentError`; it is the kernel's
//! candidate result, reported as data.

use core::fmt;

/// Closed workbench refusal codes (symbol-only, `SLEY_AGENT_V1.md` section 9).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentErrorCode {
    /// The command line does not name a known command or its arguments.
    Usage,
    /// The proposal was authored against a different accepted state root.
    ProposalStale,
    /// The proposal exceeds its explicit function scope.
    ProposalScope,
    /// No workspace directory (one holding `repo/` or `base.pack`) was found.
    WorkspaceNotFound,
    /// The workspace repository or seed pack could not be loaded.
    WorkspaceInvalid,
    /// A local name or id does not resolve in the selected state.
    NameUnknown,
    /// An AF1 frame or raw operation list is malformed.
    FrameInvalid,
    /// A candidate handle does not exist in this workspace.
    HandleUnknown,
    /// The kernel refused to construct the candidate record.
    CandidateInvalid,
    /// A call or test input does not fit the declared parameter type.
    InputInvalid,
    /// The function cannot be executed by the dev loop (profile refusal).
    ExecutionRefused,
    /// The submission was refused (the candidate is not Valid).
    SubmissionRefused,
    /// A workspace file could not be read or written.
    Io,
    /// An AF1-X checked propagation has no single, type-correct failure route.
    XPropagation,
    /// An AF1-X name is ambiguous, unavailable on a path, or cannot be threaded.
    XScope,
    /// An AF1-X expansion or traversal bound was reached.
    XLimit,
    /// Reserved and never emitted: AF1-X expansion never reorders
    /// evaluation, and a form that would run an operation on a path not
    /// taken is an `AGENT_FRAME_INVALID` grammar refusal
    /// (`SLEY_AGENT_V1.md` section 9).
    XEffectOrder,
    /// The given revision is not the draft's latest revision.
    DraftStale,
    /// The accepted head changed since the draft revision; rebase explicitly.
    DraftHeadChanged,
    /// The draft revision has no complete, Valid candidate for the request.
    DraftIncomplete,
    /// A delta target is invalid, missing, or overlaps another.
    DeltaInvalid,
    /// A test table or case row is malformed or duplicated.
    TestTableInvalid,
    /// The ripple intent is unknown or not enabled.
    RippleIntentUnknown,
    /// The ripple target is not an entity of the kind the intent needs.
    RippleTargetKind,
    /// A ripple derivation left a decision for the author.
    RippleHoleUnfilled,
    /// A ripple derivation reached an exported boundary.
    RippleExportedBoundary,
    /// A ripple derivation reached its resource bound.
    RippleLimit,
    /// The guard's type shape is not supported.
    RippleGuardShape,
    /// The guard placement or equivalence cannot be established.
    RippleGuardOrder,
    /// A residual envelope is malformed or contains a duplicate JSON member.
    ResidualParse,
    /// A residual envelope declares an unsupported version.
    ResidualVersion,
    /// The accepted head, policy, draft, or other bound dependency changed.
    ResidualBindingStale,
    /// The selected fragment identity or version is unavailable.
    ResidualFragmentUnknown,
    /// The selected fragment does not match the bound graph or input shape.
    ResidualFragmentShape,
    /// A required author decision was not supplied.
    ResidualChoiceMissing,
    /// An author decision is not a member of its declared domain.
    ResidualChoiceUnknown,
    /// A semantic decision has no adequate authored, inherited, fragment, or rule provenance.
    ResidualSemanticUnresolved,
    /// The declared construction constraints admit no completion.
    ResidualFamilyEmpty,
    /// The declared construction family cannot be shown complete.
    ResidualFamilyIncomplete,
    /// The decision vocabulary cannot distinguish required completions.
    ResidualVocabularyIncomplete,
    /// Explicit residual bindings or policies contradict each other.
    ResidualConstraintConflict,
    /// A residual planner or expander reached a declared resource limit.
    ResidualLimit,
    /// A bounded oracle could not establish the requested result.
    ResidualInconclusive,
    /// The edit scope is invalid or not exactly identified.
    ResidualScope,
    /// A residual edit omitted or violated its explicit preservation request.
    ResidualPreserve,
    /// No permitted, independent public cases are available to search against.
    SearchNoOracle,
    /// The search seed is not a usable candidate or complete draft.
    SearchSeedInvalid,
}

impl AgentErrorCode {
    /// Returns the exact registered symbol.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Usage => "AGENT_USAGE_INVALID",
            Self::ProposalStale => "AGENT_PROPOSAL_STALE",
            Self::ProposalScope => "AGENT_PROPOSAL_SCOPE",
            Self::WorkspaceNotFound => "AGENT_WORKSPACE_NOT_FOUND",
            Self::WorkspaceInvalid => "AGENT_WORKSPACE_INVALID",
            Self::NameUnknown => "AGENT_NAME_UNKNOWN",
            Self::FrameInvalid => "AGENT_FRAME_INVALID",
            Self::HandleUnknown => "AGENT_HANDLE_UNKNOWN",
            Self::CandidateInvalid => "AGENT_CANDIDATE_INVALID",
            Self::InputInvalid => "AGENT_INPUT_INVALID",
            Self::ExecutionRefused => "AGENT_EXECUTION_REFUSED",
            Self::SubmissionRefused => "AGENT_SUBMISSION_REFUSED",
            Self::Io => "AGENT_IO_FAILED",
            Self::XPropagation => "AGENT_X_PROPAGATION",
            Self::XScope => "AGENT_X_SCOPE",
            Self::XLimit => "AGENT_X_LIMIT",
            Self::XEffectOrder => "AGENT_X_EFFECT_ORDER",
            Self::DraftStale => "AGENT_DRAFT_STALE",
            Self::DraftHeadChanged => "AGENT_DRAFT_HEAD_CHANGED",
            Self::DraftIncomplete => "AGENT_DRAFT_INCOMPLETE",
            Self::DeltaInvalid => "AGENT_DELTA_INVALID",
            Self::TestTableInvalid => "AGENT_TEST_TABLE_INVALID",
            Self::RippleIntentUnknown => "AGENT_RIPPLE_INTENT_UNKNOWN",
            Self::RippleTargetKind => "AGENT_RIPPLE_TARGET_KIND",
            Self::RippleHoleUnfilled => "AGENT_RIPPLE_HOLE_UNFILLED",
            Self::RippleExportedBoundary => "AGENT_RIPPLE_EXPORTED_BOUNDARY",
            Self::RippleLimit => "AGENT_RIPPLE_LIMIT",
            Self::RippleGuardShape => "AGENT_RIPPLE_GUARD_SHAPE",
            Self::RippleGuardOrder => "AGENT_RIPPLE_GUARD_ORDER",
            Self::ResidualParse => "AGENT_RESIDUAL_PARSE",
            Self::ResidualVersion => "AGENT_RESIDUAL_VERSION",
            Self::ResidualBindingStale => "AGENT_RESIDUAL_BINDING_STALE",
            Self::ResidualFragmentUnknown => "AGENT_RESIDUAL_FRAGMENT_UNKNOWN",
            Self::ResidualFragmentShape => "AGENT_RESIDUAL_FRAGMENT_SHAPE",
            Self::ResidualChoiceMissing => "AGENT_RESIDUAL_CHOICE_MISSING",
            Self::ResidualChoiceUnknown => "AGENT_RESIDUAL_CHOICE_UNKNOWN",
            Self::ResidualSemanticUnresolved => "AGENT_RESIDUAL_SEMANTIC_UNRESOLVED",
            Self::ResidualFamilyEmpty => "AGENT_RESIDUAL_FAMILY_EMPTY",
            Self::ResidualFamilyIncomplete => "AGENT_RESIDUAL_FAMILY_INCOMPLETE",
            Self::ResidualVocabularyIncomplete => "AGENT_RESIDUAL_VOCABULARY_INCOMPLETE",
            Self::ResidualConstraintConflict => "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            Self::ResidualLimit => "AGENT_RESIDUAL_LIMIT",
            Self::ResidualInconclusive => "AGENT_RESIDUAL_INCONCLUSIVE",
            Self::ResidualScope => "AGENT_RESIDUAL_SCOPE",
            Self::ResidualPreserve => "AGENT_RESIDUAL_PRESERVE",
            Self::SearchNoOracle => "AGENT_SEARCH_NO_ORACLE",
            Self::SearchSeedInvalid => "AGENT_SEARCH_SEED_INVALID",
        }
    }
}

/// One workbench refusal with a short, actionable detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentError {
    code: AgentErrorCode,
    detail: String,
}

impl AgentError {
    /// Constructs a refusal.
    #[must_use]
    pub fn new(code: AgentErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    /// Returns the closed code.
    #[must_use]
    pub const fn code(&self) -> AgentErrorCode {
        self.code
    }

    /// Returns the human- and agent-readable detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for AgentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code.symbol(), self.detail)
    }
}

impl std::error::Error for AgentError {}

/// Workbench result.
pub type Result<T> = core::result::Result<T, AgentError>;

pub(crate) fn usage(detail: impl Into<String>) -> AgentError {
    AgentError::new(AgentErrorCode::Usage, detail)
}

pub(crate) fn frame(pointer: &str, detail: impl fmt::Display) -> AgentError {
    AgentError::new(AgentErrorCode::FrameInvalid, format!("{pointer}: {detail}"))
}

pub(crate) fn io(path: &std::path::Path, error: &std::io::Error) -> AgentError {
    AgentError::new(AgentErrorCode::Io, format!("{}: {error}", path.display()))
}

pub(crate) fn unknown_name(name: &str) -> AgentError {
    AgentError::new(
        AgentErrorCode::NameUnknown,
        format!("no entity named `{name}`"),
    )
}

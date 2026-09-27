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
}

impl AgentErrorCode {
    /// Returns the exact registered symbol.
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Usage => "AGENT_USAGE_INVALID",
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

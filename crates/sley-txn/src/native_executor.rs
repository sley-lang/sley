//! Candidate-commit dispatch to the authenticated native supervisor.
//!
//! This adapter derives each request from the protected plan and validated
//! proposed Sley state, uses a fresh kernel-random nonce, and preserves only
//! request-bound, receiver-trusted response evidence. Its socket peer must be
//! root. The supervisor service is still refusal-only, so provisioning this
//! adapter does not by itself enable native test admission.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::Component;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use sley_policy::ValidatedCandidatePlan;
use sley_test_runner::{
    client::{ClientError, run_native_test},
    config::SOCKET_NAME,
    protocol::RunStatus,
};
use sley_tests::{HistoricalTrustPolicyV1, NativeTestPlanV1};

use crate::native_commit::{
    ExecutedNativeTest, NativeCommitError, NativeTestExecutor, SupervisorEvidenceError,
    build_candidate_supervisor_request, verified_supervisor_execution,
};

/// Whole-batch preparation, execution, and evidence watchdog.
pub const NATIVE_EXECUTOR_WATCHDOG: Duration = Duration::from_secs(35);

/// Production candidate-commit adapter for the root-owned local supervisor.
///
/// An operator must provision this together with the matching receiver trust
/// manifest and acceptance authority. Replay and explicit-root diagnostics
/// remain unsupported by this candidate-only adapter.
#[derive(Clone, Debug)]
pub struct SocketNativeCommitExecutor {
    socket_path: PathBuf,
    caller_uid: u32,
    measurement_trust: HistoricalTrustPolicyV1,
}

impl SocketNativeCommitExecutor {
    /// Installs the fixed local endpoint, expected caller UID, and receiver
    /// measurement trust for one server instance.
    ///
    /// # Errors
    ///
    /// Refuses paths outside the fixed supervisor socket shape before any
    /// worker can start.
    pub fn new(
        socket_path: PathBuf,
        caller_uid: u32,
        measurement_trust: HistoricalTrustPolicyV1,
    ) -> Result<Self, NativeCommitError> {
        if !socket_path.is_absolute()
            || socket_path.file_name() != Some(std::ffi::OsStr::new(SOCKET_NAME))
            || socket_path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        {
            return Err(NativeCommitError::ExecutorUnavailable);
        }
        Ok(Self {
            socket_path,
            caller_uid,
            measurement_trust,
        })
    }

    /// Root supervisor endpoint this adapter will contact.
    #[must_use]
    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }
}

fn new_nonce(
    source: &mut File,
    used: &mut BTreeSet<[u8; 32]>,
) -> Result<[u8; 32], NativeCommitError> {
    let mut nonce = [0_u8; 32];
    source
        .read_exact(&mut nonce)
        .map_err(|_| NativeCommitError::ExecutorUnavailable)?;
    if nonce == [0; 32] || !used.insert(nonce) {
        return Err(NativeCommitError::ExecutorUnavailable);
    }
    Ok(nonce)
}

fn client_failure(error: ClientError) -> NativeCommitError {
    match error {
        ClientError::InvalidDeadline
        | ClientError::InvalidRequest(_)
        | ClientError::ConnectFailure
        | ClientError::UnauthenticatedServer => NativeCommitError::ExecutorUnavailable,
        ClientError::DeadlineReached
        | ClientError::WriteFailure
        | ClientError::ReadFailure
        | ClientError::FrameTooLarge
        | ClientError::TrailingResponse
        | ClientError::MalformedResponse(_) => NativeCommitError::OutcomeUnknown,
    }
}

fn evidence_failure(error: SupervisorEvidenceError) -> NativeCommitError {
    match error {
        SupervisorEvidenceError::NoEvidence {
            status: RunStatus::Refused,
            ..
        } => NativeCommitError::ExecutorUnavailable,
        SupervisorEvidenceError::NoEvidence {
            status: RunStatus::Failed | RunStatus::Complete,
            ..
        }
        | SupervisorEvidenceError::InvalidResponse(_) => NativeCommitError::OutcomeUnknown,
        SupervisorEvidenceError::MeasurementTrust(error) => error,
    }
}

impl NativeTestExecutor for SocketNativeCommitExecutor {
    fn execute(
        &self,
        plan: &NativeTestPlanV1,
        validated: &ValidatedCandidatePlan,
    ) -> Result<Vec<ExecutedNativeTest>, NativeCommitError> {
        if plan.selected().is_empty() {
            return Ok(Vec::new());
        }
        let deadline = Instant::now()
            .checked_add(NATIVE_EXECUTOR_WATCHDOG)
            .ok_or(NativeCommitError::ExecutorUnavailable)?;
        let mut entropy =
            File::open("/dev/urandom").map_err(|_| NativeCommitError::ExecutorUnavailable)?;
        let mut used = BTreeSet::new();
        let mut executions = Vec::with_capacity(plan.selected().len());
        for selected in plan.selected() {
            let nonce = new_nonce(&mut entropy, &mut used)?;
            let request = build_candidate_supervisor_request(
                plan,
                validated,
                selected.test_entity,
                selected.declared_limits.wall_timeout_millis,
                nonce,
            )
            .map_err(|_| NativeCommitError::ExecutorUnavailable)?;
            let timeout = deadline
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
                .ok_or(NativeCommitError::OutcomeUnknown)?;
            let frame =
                run_native_test(&self.socket_path, &request, timeout).map_err(client_failure)?;
            let execution = verified_supervisor_execution(
                &request,
                &frame,
                self.caller_uid,
                &self.measurement_trust,
            )
            .map_err(evidence_failure)?;
            executions.push(execution);
        }
        Ok(executions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncertain_transport_and_evidence_remain_unknown() {
        assert_eq!(
            client_failure(ClientError::ConnectFailure),
            NativeCommitError::ExecutorUnavailable
        );
        assert_eq!(
            client_failure(ClientError::DeadlineReached),
            NativeCommitError::OutcomeUnknown
        );
        assert_eq!(
            evidence_failure(SupervisorEvidenceError::NoEvidence {
                status: RunStatus::Refused,
                code: 1,
            }),
            NativeCommitError::ExecutorUnavailable
        );
        assert_eq!(
            evidence_failure(SupervisorEvidenceError::NoEvidence {
                status: RunStatus::Failed,
                code: 1,
            }),
            NativeCommitError::OutcomeUnknown
        );
    }
}

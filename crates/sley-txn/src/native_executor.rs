//! Native commit and diagnostic dispatch to the authenticated supervisor.
//!
//! This adapter derives each request from the protected plan and the
//! validator-owned proposed or owner-loaded accepted state, uses a fresh
//! kernel-random nonce, and preserves only
//! request-bound, receiver-trusted response evidence. Its socket peer must be
//! root. Provisioning this adapter does not by itself qualify the supervisor
//! or enable native test admission on an untested host.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Component;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use sley_id::{EntityId, ObjectId};
use sley_policy::ValidatedCandidatePlan;
use sley_state_root::AcceptedStateRoot;
use sley_test_runner::{
    client::{ClientError, run_native_test},
    config::SOCKET_NAME,
    nonce::random_attempt_nonce,
    protocol::{RunRequest, RunStatus},
    service::RUN_REFUSAL_WALL_UNSUPPORTED,
};
use sley_tests::{
    HistoricalTrustPolicyV1, NativeTestPlanV1, TERMINATION_PRELAUNCH_REFUSED, TERMINATION_TIMEOUT,
};

use crate::native_commit::{
    ExecutedNativeTest, NativeCommitError, NativeTestExecutor, SupervisorEvidenceError,
    build_candidate_supervisor_request, build_explicit_supervisor_request,
    verified_supervisor_execution,
};

/// Whole-batch preparation, execution, and evidence watchdog.
pub const NATIVE_EXECUTOR_WATCHDOG: Duration = Duration::from_secs(35);

/// Production candidate and explicit-root adapter for the root-owned local supervisor.
///
/// An operator must provision this together with the matching receiver trust
/// manifest and acceptance authority. Replay remains unsupported.
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

    fn run_selected(
        &self,
        plan: &NativeTestPlanV1,
        mut build: impl FnMut(EntityId, u64, [u8; 32]) -> Result<RunRequest, NativeCommitError>,
    ) -> Result<Vec<ExecutedNativeTest>, NativeCommitError> {
        if plan.selected().is_empty() {
            return Ok(Vec::new());
        }
        let deadline = Instant::now()
            .checked_add(NATIVE_EXECUTOR_WATCHDOG)
            .ok_or(NativeCommitError::ExecutorUnavailable)?;
        let mut used = BTreeSet::new();
        let mut executions = Vec::with_capacity(plan.selected().len());
        for selected in plan.selected() {
            let nonce = new_nonce(&mut used, || {
                random_attempt_nonce().map_err(|_| NativeCommitError::ExecutorUnavailable)
            })?;
            let request = build(
                selected.test_entity,
                selected.declared_limits.wall_timeout_millis,
                nonce,
            )?;
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

fn new_nonce(
    used: &mut BTreeSet<[u8; 32]>,
    source: impl FnOnce() -> Result<[u8; 32], NativeCommitError>,
) -> Result<[u8; 32], NativeCommitError> {
    let nonce = source()?;
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
        SupervisorEvidenceError::SignedManagerOom
        | SupervisorEvidenceError::SignedNoResult {
            status: RunStatus::Refused,
            termination: TERMINATION_PRELAUNCH_REFUSED,
            code: RUN_REFUSAL_WALL_UNSUPPORTED,
        }
        | SupervisorEvidenceError::SignedNoResult {
            status: RunStatus::Failed,
            termination: TERMINATION_TIMEOUT,
            ..
        } => NativeCommitError::ResourceRefused,
        SupervisorEvidenceError::NoEvidence {
            status: RunStatus::Refused,
            ..
        }
        | SupervisorEvidenceError::SignedNoResult {
            status: RunStatus::Refused,
            ..
        } => NativeCommitError::ExecutorUnavailable,
        SupervisorEvidenceError::NoEvidence {
            status: RunStatus::Failed | RunStatus::Complete,
            ..
        }
        | SupervisorEvidenceError::SignedNoResult {
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
        self.run_selected(plan, |test_entity, wall_ms, nonce| {
            build_candidate_supervisor_request(plan, validated, test_entity, wall_ms, nonce)
                .map_err(|_| NativeCommitError::ExecutorUnavailable)
        })
    }

    fn execute_diagnostic(
        &self,
        plan: &NativeTestPlanV1,
        root: &AcceptedStateRoot,
        objects: &BTreeMap<ObjectId, &[u8]>,
    ) -> Result<Vec<ExecutedNativeTest>, NativeCommitError> {
        self.run_selected(plan, |test_entity, wall_ms, nonce| {
            build_explicit_supervisor_request(plan, root, objects, test_entity, wall_ms, nonce)
                .map_err(|_| NativeCommitError::ExecutorUnavailable)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_source_failure_and_zero_do_not_reserve_an_attempt() {
        let mut used = BTreeSet::new();
        assert_eq!(
            new_nonce(&mut used, || Err(NativeCommitError::ExecutorUnavailable)),
            Err(NativeCommitError::ExecutorUnavailable)
        );
        assert!(used.is_empty());
        assert_eq!(
            new_nonce(&mut used, || Ok([0; 32])),
            Err(NativeCommitError::ExecutorUnavailable)
        );
        assert!(used.is_empty());
    }

    #[test]
    fn nonce_is_preserved_and_reuse_is_refused() {
        let mut used = BTreeSet::new();
        let expected = random_attempt_nonce().expect("OS entropy");
        let first = new_nonce(&mut used, || Ok(expected)).expect("first attempt");
        assert_eq!(first, expected);
        assert_ne!(first, [0; 32]);
        assert_eq!(used, BTreeSet::from([first]));
        assert_eq!(
            new_nonce(&mut used, || Err(NativeCommitError::ExecutorUnavailable)),
            Err(NativeCommitError::ExecutorUnavailable)
        );
        assert_eq!(used, BTreeSet::from([first]));
        assert_eq!(
            new_nonce(&mut used, || Ok(first)),
            Err(NativeCommitError::ExecutorUnavailable)
        );
        assert_eq!(used, BTreeSet::from([first]));
        let second = new_nonce(&mut used, || {
            random_attempt_nonce().map_err(|_| NativeCommitError::ExecutorUnavailable)
        })
        .expect("fresh OS entropy");
        assert_ne!(first, second);
        assert_eq!(used, BTreeSet::from([first, second]));
    }

    #[test]
    fn uncertain_transport_and_evidence_remain_unknown() {
        assert_eq!(
            evidence_failure(SupervisorEvidenceError::SignedManagerOom),
            NativeCommitError::ResourceRefused
        );
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
                status: RunStatus::Refused,
                code: RUN_REFUSAL_WALL_UNSUPPORTED,
            }),
            NativeCommitError::ExecutorUnavailable
        );
        assert_eq!(
            evidence_failure(SupervisorEvidenceError::SignedNoResult {
                status: RunStatus::Refused,
                termination: TERMINATION_PRELAUNCH_REFUSED,
                code: RUN_REFUSAL_WALL_UNSUPPORTED,
            }),
            NativeCommitError::ResourceRefused
        );
        assert_eq!(
            evidence_failure(SupervisorEvidenceError::SignedNoResult {
                status: RunStatus::Refused,
                termination: TERMINATION_PRELAUNCH_REFUSED,
                code: sley_test_runner::service::RUN_REFUSAL_STAGE_FAILED,
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
        assert_eq!(
            evidence_failure(SupervisorEvidenceError::SignedNoResult {
                status: RunStatus::Failed,
                termination: TERMINATION_TIMEOUT,
                code: 1,
            }),
            NativeCommitError::ResourceRefused
        );
        assert_eq!(
            evidence_failure(SupervisorEvidenceError::SignedNoResult {
                status: RunStatus::Failed,
                termination: sley_tests::TERMINATION_KILLED,
                code: 1,
            }),
            NativeCommitError::OutcomeUnknown
        );
    }
}

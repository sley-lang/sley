//! Signed completion of one already owned native worker attempt.
//!
//! The root service supplies a kernel-authenticated request, the unit owner's
//! launch-through-reap result, and an administrator-provisioned trust policy.
//! This module rechecks report bindings and success facts before it asks the
//! root-only signer to sign. It neither loads trust nor launches a worker.

use std::time::{SystemTime, UNIX_EPOCH};

use sley_scb1::ScbErrorCode;
use sley_tests::{
    HistoricalTrustPolicyV1, MeasuredTestAttestationParts, MeasuredTestAttestationV1, MemoryEvents,
    NativeExecutionEvidence, ROLE_MEASUREMENT, TERMINATION_PRELAUNCH_REFUSED, TERMINATION_TIMEOUT,
    measurement_signature_preimage, unsigned_record_prefix,
};

use crate::config::RunnerConfig;
use crate::config::sha256_bytes;
use crate::enforce::{deadline_ns, floor_page_cap};
use crate::ingress::AuthenticatedRunRequest;
use crate::manager_oom::{ManagerOomClaimV1, RunManagerOomEvidence};
use crate::outcome::{
    AdmissionRefusal, AttemptOutcome, ObservedTermination, Signer, SignerError, admit_for_signature,
};
use crate::owner::{OwnedOomFacts, OwnedTimeoutFacts, OwnedWorkerResult};
use crate::protocol::{RunRequest, RunResponse, RunStatus};
use crate::response::{RunEvidence, RunNoResultEvidence};
use crate::unit::expected_supervisor_config;

/// Refusal before a successful host measurement can be signed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttestationError {
    /// Request, worker report, or response binding is inconsistent.
    Binding(ScbErrorCode),
    /// Cgroup group-kill telemetry is nonzero.
    GroupKill,
    /// The host attempt fails a success admission predicate.
    Admission(AdmissionRefusal),
    /// Provisioned trust does not grant this key, workspace, and supervisor.
    TrustRejected,
    /// Trusted historical time is unavailable or cannot fit the wire type.
    ClockUnavailable,
    /// Root-only signing failed.
    Signing(SignerError),
}

impl core::fmt::Display for AttestationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Binding(_) => "NATIVE_ATTESTATION_BINDING_REFUSED",
            Self::GroupKill => "NATIVE_ATTESTATION_GROUP_KILL",
            Self::Admission(_) => "NATIVE_ATTESTATION_ADMISSION_REFUSED",
            Self::TrustRejected => "NATIVE_ATTESTATION_TRUST_REJECTED",
            Self::ClockUnavailable => "NATIVE_ATTESTATION_CLOCK_UNAVAILABLE",
            Self::Signing(_) => "NATIVE_ATTESTATION_SIGNING_FAILED",
        })
    }
}

impl std::error::Error for AttestationError {}

fn bind_worker_report(
    request: &RunRequest,
    result: &OwnedWorkerResult,
) -> Result<(), AttestationError> {
    let program = request
        .verified_program()
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let report = &result.gated.report;
    let selected = program.selected();
    if report.plan_id() != request.plan_id
        || report.test_entity() != selected.test_entity
        || report.test_object() != selected.test_object
        || report.target_object() != selected.target_object
    {
        return Err(AttestationError::Binding(ScbErrorCode::ContractUnknown));
    }
    if matches!(report.evidence(), NativeExecutionEvidence::Observed { .. }) {
        request
            .verified_observed_worker_report(report.stored_bytes())
            .map_err(|error| AttestationError::Binding(error.code()))?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct NoResultFacts {
    status: RunStatus,
    termination: u32,
    installed_cap: u64,
    elapsed_ns: u64,
    memory_peak: u64,
    memory_events: MemoryEvents,
}

fn sign_no_result(
    config: &RunnerConfig,
    authenticated: &AuthenticatedRunRequest,
    trust: &HistoricalTrustPolicyV1,
    signer: &dyn Signer,
    code: u32,
    facts: NoResultFacts,
) -> Result<RunResponse, AttestationError> {
    if code == 0 {
        return Err(AttestationError::Binding(ScbErrorCode::ContractUnknown));
    }
    let request = authenticated.request();
    request
        .verified_program()
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let caller_uid = authenticated.caller_uid();
    let supervisor_config = expected_supervisor_config(config, request, caller_uid)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let recorded_unix_millis = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AttestationError::ClockUnavailable)?
            .as_millis(),
    )
    .map_err(|_| AttestationError::ClockUnavailable)?;
    let key_id = signer.public_key();
    if !trust.grants(
        &key_id,
        ROLE_MEASUREMENT,
        request.workspace.as_bytes(),
        supervisor_config.id().as_bytes(),
        recorded_unix_millis,
    ) {
        return Err(AttestationError::TrustRejected);
    }
    let mut parts = MeasuredTestAttestationParts {
        key_id,
        trust_policy_id: *trust.id().as_bytes(),
        supervisor_config_id: *supervisor_config.id().as_bytes(),
        plan_id: request.plan_id,
        test_object: request.test_object,
        execution_report_id: None,
        attempt_nonce: request.nonce,
        workspace: request.workspace,
        principal: request.principal,
        caller_uid,
        declared_limits: request.declared_limits,
        installed_memory_cap: facts.installed_cap,
        elapsed_ns: facts.elapsed_ns,
        measured_memory_peak: facts.memory_peak,
        memory_events: facts.memory_events,
        termination: facts.termination,
        complete_output: false,
        empty_cgroup_confirmed: true,
        recorded_unix_millis,
        signature: [0; 64],
    };
    let unsigned =
        unsigned_record_prefix(&parts).map_err(|error| AttestationError::Binding(error.code()))?;
    let preimage = measurement_signature_preimage(&unsigned)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    parts.signature = signer.sign(&preimage).map_err(AttestationError::Signing)?;
    let attestation = MeasuredTestAttestationV1::build(parts)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let no_result = RunNoResultEvidence::build(attestation, supervisor_config)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let response = RunResponse {
        status: facts.status,
        code,
        evidence: None,
        no_result: Some(no_result),
        manager_oom: None,
    };
    request
        .verified_no_result_evidence(&response, caller_uid)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    Ok(response)
}

/// Signs a refusal after authenticated admission but before any worker unit
/// is launched. The zero host facts are valid only for that boundary.
///
/// # Errors
/// Refuses invalid request bindings, absent measurement trust, clock failure,
/// or signing failure. The caller must establish that launch did not occur.
pub fn sign_prelaunch_refusal(
    config: &RunnerConfig,
    authenticated: &AuthenticatedRunRequest,
    trust: &HistoricalTrustPolicyV1,
    signer: &dyn Signer,
    code: u32,
) -> Result<RunResponse, AttestationError> {
    sign_no_result(
        config,
        authenticated,
        trust,
        signer,
        code,
        NoResultFacts {
            status: RunStatus::Refused,
            termination: TERMINATION_PRELAUNCH_REFUSED,
            installed_cap: 0,
            elapsed_ns: 0,
            memory_peak: 0,
            memory_events: MemoryEvents {
                max: 0,
                oom: 0,
                oom_kill: 0,
            },
        },
    )
}

/// Signs a deadline observed with a clean live cgroup snapshot, after the
/// owner has killed the launched unit and confirmed the group empty.
///
/// # Errors
/// Refuses dirty or inconsistent sampled facts, a deadline that was not
/// actually reached, absent trust, or signing failure. No VM report is
/// claimed and this response cannot become native receipt evidence.
pub fn sign_measured_timeout(
    config: &RunnerConfig,
    authenticated: &AuthenticatedRunRequest,
    facts: OwnedTimeoutFacts,
    trust: &HistoricalTrustPolicyV1,
    signer: &dyn Signer,
    code: u32,
) -> Result<RunResponse, AttestationError> {
    let (_, installed_cap) = floor_page_cap(
        authenticated.request().declared_limits.memory_bytes,
        config.page_size,
    )
    .map_err(|_| AttestationError::Binding(ScbErrorCode::ResourceLimit))?;
    let elapsed_floor = deadline_ns(authenticated.request().wall_ms)
        .map_err(|_| AttestationError::Binding(ScbErrorCode::ResourceLimit))?;
    if !facts.telemetry.events_clean()
        || facts.telemetry.main_pid == 0
        || facts.telemetry.memory_peak > installed_cap
        || facts.elapsed_ns < elapsed_floor
    {
        return Err(AttestationError::Binding(ScbErrorCode::ContractUnknown));
    }
    sign_no_result(
        config,
        authenticated,
        trust,
        signer,
        code,
        NoResultFacts {
            status: RunStatus::Failed,
            termination: TERMINATION_TIMEOUT,
            installed_cap,
            elapsed_ns: facts.elapsed_ns,
            memory_peak: facts.telemetry.memory_peak,
            memory_events: facts.telemetry.memory_events,
        },
    )
}

/// Signs a separately typed, manager-observed OOM after the owner verified
/// exact unit settings, retained OOM result, SIGKILL status, and empty cgroup.
/// No cgroup event counters are asserted by this diagnostic.
///
/// # Errors
/// Refuses a mismatched cap or peak, invalid request, absent trust, clock
/// failure, or signer failure.
pub fn sign_manager_oom(
    config: &RunnerConfig,
    authenticated: &AuthenticatedRunRequest,
    facts: OwnedOomFacts,
    trust: &HistoricalTrustPolicyV1,
    signer: &dyn Signer,
    code: u32,
) -> Result<RunResponse, AttestationError> {
    if code != crate::service::RUN_FAILURE_MANAGER_OOM {
        return Err(AttestationError::Binding(ScbErrorCode::ContractUnknown));
    }
    let request = authenticated.request();
    request
        .verified_program()
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let (_, installed_cap) = floor_page_cap(request.declared_limits.memory_bytes, config.page_size)
        .map_err(|_| AttestationError::Binding(ScbErrorCode::ResourceLimit))?;
    if facts.manager.memory_peak == 0 || facts.manager.memory_peak > installed_cap {
        return Err(AttestationError::Binding(ScbErrorCode::ContractUnknown));
    }
    let supervisor_config = expected_supervisor_config(config, request, authenticated.caller_uid())
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let recorded_unix_millis = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AttestationError::ClockUnavailable)?
            .as_millis(),
    )
    .map_err(|_| AttestationError::ClockUnavailable)?;
    let key_id = signer.public_key();
    if !trust.grants(
        &key_id,
        ROLE_MEASUREMENT,
        request.workspace.as_bytes(),
        supervisor_config.id().as_bytes(),
        recorded_unix_millis,
    ) {
        return Err(AttestationError::TrustRejected);
    }
    let request_frame = request
        .encode_frame()
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let mut claim = ManagerOomClaimV1 {
        request_sha256: sha256_bytes(&request_frame),
        caller_uid: authenticated.caller_uid(),
        key_id,
        trust_policy_id: *trust.id().as_bytes(),
        supervisor_config_id: *supervisor_config.id().as_bytes(),
        installed_memory_cap: installed_cap,
        memory_peak: facts.manager.memory_peak,
        exec_main_pid: facts.manager.exec_main_pid,
        elapsed_ns: facts.elapsed_ns,
        recorded_unix_millis,
        signature: [0; 64],
    };
    let preimage = claim
        .signature_preimage()
        .map_err(|error| AttestationError::Binding(error.code()))?;
    claim.signature = signer.sign(&preimage).map_err(AttestationError::Signing)?;
    let evidence = RunManagerOomEvidence::build(claim, supervisor_config)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let response = RunResponse {
        status: RunStatus::Failed,
        code,
        evidence: None,
        no_result: None,
        manager_oom: Some(evidence),
    };
    request
        .verified_manager_oom_evidence(&response, authenticated.caller_uid())
        .map_err(|error| AttestationError::Binding(error.code()))?;
    Ok(response)
}

/// Signs a completed, already reaped worker attempt under provisioned trust.
///
/// The caller must be the root service after kernel peer authentication and
/// `run_owned_system_unit`; `result` alone does not prove those prerequisites.
/// `trust` must have been loaded from administrator-controlled storage, never
/// from the request. The recorded time comes from this host's clock.
///
/// # Errors
///
/// Refuses mismatched source or report, dirty telemetry, non-admissible host
/// facts, missing trust, clock failure, or signer failure. Report bindings,
/// host facts, and trust are checked before signing.
pub fn sign_complete_attempt(
    config: &RunnerConfig,
    authenticated: &AuthenticatedRunRequest,
    result: OwnedWorkerResult,
    trust: &HistoricalTrustPolicyV1,
    signer: &dyn Signer,
) -> Result<RunResponse, AttestationError> {
    let request = authenticated.request();
    let caller_uid = authenticated.caller_uid();
    bind_worker_report(request, &result)?;
    let supervisor_config = expected_supervisor_config(config, request, caller_uid)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let (_, installed_cap) = floor_page_cap(request.declared_limits.memory_bytes, config.page_size)
        .map_err(|_| AttestationError::Binding(ScbErrorCode::ResourceLimit))?;
    let telemetry = result.gated.telemetry;
    if telemetry.oom_group_kill != 0 {
        return Err(AttestationError::GroupKill);
    }
    admit_for_signature(&AttemptOutcome {
        has_execution_report: true,
        termination: ObservedTermination::Complete,
        complete_output: true,
        empty_cgroup_confirmed: true,
        measured_peak: telemetry.memory_peak,
        installed_cap,
        requested_cap: request.declared_limits.memory_bytes,
        max_events: telemetry.memory_events.max,
        oom_events: telemetry.memory_events.oom,
        oom_kill_events: telemetry.memory_events.oom_kill,
        elapsed_ns: result.elapsed_ns,
        wall_ms: request.wall_ms,
    })
    .map_err(AttestationError::Admission)?;
    let recorded_unix_millis = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AttestationError::ClockUnavailable)?
            .as_millis(),
    )
    .map_err(|_| AttestationError::ClockUnavailable)?;
    let key_id = signer.public_key();
    if !trust.grants(
        &key_id,
        ROLE_MEASUREMENT,
        request.workspace.as_bytes(),
        supervisor_config.id().as_bytes(),
        recorded_unix_millis,
    ) {
        return Err(AttestationError::TrustRejected);
    }
    let report = result.gated.report;
    let mut parts = MeasuredTestAttestationParts {
        key_id,
        trust_policy_id: *trust.id().as_bytes(),
        supervisor_config_id: *supervisor_config.id().as_bytes(),
        plan_id: request.plan_id,
        test_object: request.test_object,
        execution_report_id: Some(report.report_id()),
        attempt_nonce: request.nonce,
        workspace: request.workspace,
        principal: request.principal,
        caller_uid,
        declared_limits: request.declared_limits,
        installed_memory_cap: installed_cap,
        elapsed_ns: result.elapsed_ns,
        measured_memory_peak: telemetry.memory_peak,
        memory_events: telemetry.memory_events,
        termination: ObservedTermination::Complete.tag(),
        complete_output: true,
        empty_cgroup_confirmed: true,
        recorded_unix_millis,
        signature: [0; 64],
    };
    let unsigned =
        unsigned_record_prefix(&parts).map_err(|error| AttestationError::Binding(error.code()))?;
    let preimage = measurement_signature_preimage(&unsigned)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    parts.signature = signer.sign(&preimage).map_err(AttestationError::Signing)?;
    let attestation = MeasuredTestAttestationV1::build(parts)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let evidence = RunEvidence::build(report, attestation, supervisor_config)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    let response = RunResponse {
        status: RunStatus::Complete,
        code: 0,
        evidence: Some(evidence),
        no_result: None,
        manager_oom: None,
    };
    request
        .verified_response_evidence(&response, caller_uid)
        .map_err(|error| AttestationError::Binding(error.code()))?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    use ed25519_dalek::{Signature, VerifyingKey};
    use sley_id::ObjectId;
    use sley_tests::{
        HistoricalTrustPolicyParts, MemoryEvents, NativeExecutionReportParts,
        NativeExecutionReportV1, REJECT_PHASE_EXECUTION, ROLE_ACCEPTANCE, RejectedEvidence,
        TrustEntry,
    };

    use super::*;
    use crate::config::{AllowedCaller, default_config};
    use crate::ingress::authenticate_request;
    use crate::manager::ReapedOomWorker;
    use crate::outcome::Ed25519MeasurementSigner;
    use crate::phase::GatedWorkerResult;
    use crate::program::PortableTestProgram;
    use crate::telemetry::LiveTelemetrySample;
    use crate::worker::WorkerRequest;

    fn fixture() -> (
        RunnerConfig,
        AuthenticatedRunRequest,
        OwnedWorkerResult,
        Ed25519MeasurementSigner,
    ) {
        let worker = WorkerRequest::decode_frame(include_bytes!(
            "../../../conformance/native-worker/v1/observed-input.bin"
        ))
        .expect("canonical worker input");
        let program = PortableTestProgram::parse(&worker.program_bytes).expect("portable program");
        let request = RunRequest::from_portable_program(&program, 1_000, [9; 32]).expect("request");
        let (mut server, mut client) = UnixStream::pair().expect("socket pair");
        let uid = nix::unistd::getuid().as_raw();
        let config = default_config(
            "/run/sley-test-supervisor",
            "/usr/lib/sley/sley-native-test-worker",
            [7; 32],
            [8; 32],
            vec![AllowedCaller {
                uid,
                workspace: request.workspace,
                principal: request.principal,
            }],
            "/etc/sley-test-supervisor/measurement.key",
            "/etc/sley-test-supervisor/trust",
        )
        .expect("config");
        client
            .write_all(&request.encode_frame().expect("request frame"))
            .expect("send request");
        let authenticated = authenticate_request(&mut server, &config, Duration::from_secs(1))
            .expect("kernel-authenticated request");
        let report = NativeExecutionReportV1::parse(include_bytes!(
            "../../../conformance/native-worker/v1/observed-report.bin"
        ))
        .expect("canonical worker report");
        let result = OwnedWorkerResult {
            gated: GatedWorkerResult {
                report,
                telemetry: LiveTelemetrySample {
                    memory_peak: 1,
                    memory_events: MemoryEvents {
                        max: 0,
                        oom: 0,
                        oom_kill: 0,
                    },
                    oom_group_kill: 0,
                    main_pid: 123,
                },
            },
            elapsed_ns: 1,
        };
        let signer = Ed25519MeasurementSigner::from_secret_bytes([3; 32]);
        (config, authenticated, result, signer)
    }

    fn trust(
        request: &RunRequest,
        signer: &dyn Signer,
        role: u32,
        profile: [u8; 32],
    ) -> HistoricalTrustPolicyV1 {
        HistoricalTrustPolicyV1::build(HistoricalTrustPolicyParts {
            policy_nonce: [5; 32],
            entries: vec![TrustEntry {
                key_id: signer.public_key(),
                role,
                workspaces: vec![*request.workspace.as_bytes()],
                profiles: vec![profile],
                valid_from_unix_millis: 0,
                valid_until_unix_millis: u64::MAX,
            }],
        })
        .expect("trust manifest")
    }

    #[test]
    fn complete_run_is_signed_for_exact_supervisor_scope() {
        let (config, authenticated, result, signer) = fixture();
        let request = authenticated.request();
        let profile = *expected_supervisor_config(&config, request, authenticated.caller_uid())
            .expect("config profile")
            .id()
            .as_bytes();
        let trust = trust(request, &signer, ROLE_MEASUREMENT, profile);
        let response = sign_complete_attempt(&config, &authenticated, result, &trust, &signer)
            .expect("signed complete run");
        let evidence = request
            .verified_response_evidence(&response, authenticated.caller_uid())
            .expect("bound response");
        assert_eq!(
            evidence.attestation().trust_policy_id(),
            *trust.id().as_bytes()
        );
        assert_eq!(evidence.attestation().parts().key_id, signer.public_key());
        assert!(evidence.attestation().claims_success());
        let parts = evidence.attestation().parts();
        let unsigned = unsigned_record_prefix(parts).expect("canonical unsigned measurement");
        let preimage = measurement_signature_preimage(&unsigned).expect("signature preimage");
        VerifyingKey::from_bytes(&parts.key_id)
            .expect("public key")
            .verify_strict(&preimage, &Signature::from_bytes(&parts.signature))
            .expect("signed measurement verifies");
    }

    #[test]
    fn prelaunch_refusal_is_signed_without_a_worker_report() {
        let (config, authenticated, _, signer) = fixture();
        let request = authenticated.request();
        let profile = *expected_supervisor_config(&config, request, authenticated.caller_uid())
            .expect("config profile")
            .id()
            .as_bytes();
        let trusted = trust(request, &signer, ROLE_MEASUREMENT, profile);
        let response = sign_prelaunch_refusal(&config, &authenticated, &trusted, &signer, 4)
            .expect("signed refusal");
        assert_eq!(response.status, RunStatus::Refused);
        assert!(response.evidence.is_none());
        let frame = response.encode_frame().expect("canonical response");
        let parsed = RunResponse::decode_frame(&frame).expect("parsed response");
        let evidence = request
            .verified_no_result_evidence(&parsed, authenticated.caller_uid())
            .expect("request-bound refusal");
        let parts = evidence.attestation().parts();
        assert_eq!(parts.execution_report_id, None);
        assert_eq!(parts.installed_memory_cap, 0);
        assert_eq!(parts.termination, TERMINATION_PRELAUNCH_REFUSED);
        let unsigned = unsigned_record_prefix(parts).expect("canonical unsigned measurement");
        let preimage = measurement_signature_preimage(&unsigned).expect("signature preimage");
        VerifyingKey::from_bytes(&parts.key_id)
            .expect("public key")
            .verify_strict(&preimage, &Signature::from_bytes(&parts.signature))
            .expect("signed refusal verifies");
        assert_eq!(
            sign_prelaunch_refusal(&config, &authenticated, &trusted, &signer, 0),
            Err(AttestationError::Binding(ScbErrorCode::ContractUnknown))
        );
        let wrong_trust = trust(request, &signer, ROLE_MEASUREMENT, [4; 32]);
        assert_eq!(
            sign_prelaunch_refusal(&config, &authenticated, &wrong_trust, &signer, 4),
            Err(AttestationError::TrustRejected)
        );
    }

    #[test]
    fn measured_timeout_is_signed_only_after_deadline_with_clean_counters() {
        let (config, authenticated, result, signer) = fixture();
        let request = authenticated.request();
        let profile = *expected_supervisor_config(&config, request, authenticated.caller_uid())
            .expect("config profile")
            .id()
            .as_bytes();
        let trust = trust(request, &signer, ROLE_MEASUREMENT, profile);
        let mut facts = OwnedTimeoutFacts {
            telemetry: result.gated.telemetry,
            elapsed_ns: request.wall_ms * 1_000_000,
        };
        let response = sign_measured_timeout(&config, &authenticated, facts, &trust, &signer, 1)
            .expect("signed timeout");
        assert_eq!(response.status, RunStatus::Failed);
        assert!(response.evidence.is_none());
        let evidence = request
            .verified_no_result_evidence(&response, authenticated.caller_uid())
            .expect("bound timeout");
        let parts = evidence.attestation().parts();
        assert_eq!(parts.termination, TERMINATION_TIMEOUT);
        assert_eq!(parts.execution_report_id, None);
        let unsigned = unsigned_record_prefix(parts).expect("unsigned timeout");
        let preimage = measurement_signature_preimage(&unsigned).expect("preimage");
        VerifyingKey::from_bytes(&parts.key_id)
            .expect("public key")
            .verify_strict(&preimage, &Signature::from_bytes(&parts.signature))
            .expect("signed timeout verifies");
        facts.elapsed_ns -= 1;
        assert_eq!(
            sign_measured_timeout(&config, &authenticated, facts, &trust, &signer, 1),
            Err(AttestationError::Binding(ScbErrorCode::ContractUnknown))
        );
        facts.elapsed_ns += 1;
        facts.telemetry.memory_events.oom_kill = 1;
        assert_eq!(
            sign_measured_timeout(&config, &authenticated, facts, &trust, &signer, 1),
            Err(AttestationError::Binding(ScbErrorCode::ContractUnknown))
        );
    }

    #[test]
    fn manager_oom_is_signed_without_claiming_cgroup_counters_or_a_report() {
        let (config, authenticated, _, signer) = fixture();
        let request = authenticated.request();
        let profile = *expected_supervisor_config(&config, request, authenticated.caller_uid())
            .expect("config profile")
            .id()
            .as_bytes();
        let trust = trust(request, &signer, ROLE_MEASUREMENT, profile);
        let facts = OwnedOomFacts {
            manager: ReapedOomWorker {
                exec_main_pid: 123,
                memory_peak: 4_096,
            },
            elapsed_ns: 1,
        };
        let response = sign_manager_oom(
            &config,
            &authenticated,
            facts,
            &trust,
            &signer,
            crate::service::RUN_FAILURE_MANAGER_OOM,
        )
        .expect("signed manager OOM");
        assert!(response.evidence.is_none());
        assert!(response.no_result.is_none());
        let frame = response.encode_frame().expect("bounded response");
        let parsed = RunResponse::decode_frame(&frame).expect("canonical response");
        let evidence = request
            .verified_manager_oom_evidence(&parsed, authenticated.caller_uid())
            .expect("exact request binding");
        let claim = evidence.claim();
        assert_eq!(claim.memory_peak, 4_096);
        assert_eq!(claim.exec_main_pid, 123);
        let preimage = claim.signature_preimage().expect("preimage");
        VerifyingKey::from_bytes(&claim.key_id)
            .expect("public key")
            .verify_strict(&preimage, &Signature::from_bytes(&claim.signature))
            .expect("signature verifies");
        let mut wrong_nonce = request.clone();
        wrong_nonce.nonce[0] ^= 1;
        assert!(
            wrong_nonce
                .verified_manager_oom_evidence(&parsed, authenticated.caller_uid())
                .is_err()
        );
        assert!(
            request
                .verified_manager_oom_evidence(&parsed, authenticated.caller_uid() + 1)
                .is_err()
        );
        let dirty = OwnedOomFacts {
            manager: ReapedOomWorker {
                memory_peak: 8_192,
                ..facts.manager
            },
            ..facts
        };
        assert!(
            sign_manager_oom(
                &config,
                &authenticated,
                dirty,
                &trust,
                &signer,
                crate::service::RUN_FAILURE_MANAGER_OOM,
            )
            .is_err()
        );
    }

    #[test]
    fn completed_worker_rejection_is_signed_as_a_complete_host_attempt() {
        let (config, authenticated, mut result, signer) = fixture();
        let request = authenticated.request();
        let report = &result.gated.report;
        result.gated.report = NativeExecutionReportV1::build(NativeExecutionReportParts {
            plan_id: report.plan_id(),
            test_entity: report.test_entity(),
            test_object: report.test_object(),
            target_object: report.target_object(),
            evidence: NativeExecutionEvidence::Rejected(
                RejectedEvidence::from_parts(
                    REJECT_PHASE_EXECUTION,
                    29_211,
                    "NATIVE_TEST_EXECUTION_REJECTED",
                )
                .expect("rejected evidence"),
            ),
        })
        .expect("well-formed worker rejection");
        let profile = *expected_supervisor_config(&config, request, authenticated.caller_uid())
            .expect("config profile")
            .id()
            .as_bytes();
        let trust = trust(request, &signer, ROLE_MEASUREMENT, profile);
        let response = sign_complete_attempt(&config, &authenticated, result, &trust, &signer)
            .expect("complete host attempt");
        assert_eq!(response.status, RunStatus::Complete);
        assert!(matches!(
            response
                .evidence
                .as_ref()
                .expect("evidence")
                .report()
                .evidence(),
            NativeExecutionEvidence::Rejected(_)
        ));
    }

    #[test]
    fn refuses_a_role_or_profile_without_signing() {
        let (config, authenticated, result, signer) = fixture();
        let request = authenticated.request();
        let profile = *expected_supervisor_config(&config, request, authenticated.caller_uid())
            .expect("config profile")
            .id()
            .as_bytes();
        let wrong_role = trust(request, &signer, ROLE_ACCEPTANCE, profile);
        assert_eq!(
            sign_complete_attempt(&config, &authenticated, result, &wrong_role, &signer),
            Err(AttestationError::TrustRejected)
        );
        let (_, _, result, _) = fixture();
        let wrong_profile = trust(request, &signer, ROLE_MEASUREMENT, [4; 32]);
        assert_eq!(
            sign_complete_attempt(&config, &authenticated, result, &wrong_profile, &signer),
            Err(AttestationError::TrustRejected)
        );
    }

    #[test]
    fn refuses_dirty_group_kill_and_wrong_report() {
        let (config, authenticated, mut result, signer) = fixture();
        let request = authenticated.request();
        let profile = *expected_supervisor_config(&config, request, authenticated.caller_uid())
            .expect("config profile")
            .id()
            .as_bytes();
        let trust = trust(request, &signer, ROLE_MEASUREMENT, profile);
        result.gated.telemetry.oom_group_kill = 1;
        assert_eq!(
            sign_complete_attempt(&config, &authenticated, result, &trust, &signer),
            Err(AttestationError::GroupKill)
        );
        let (_, _, mut result, _) = fixture();
        let report = &result.gated.report;
        result.gated.report = NativeExecutionReportV1::build(NativeExecutionReportParts {
            plan_id: report.plan_id(),
            test_entity: report.test_entity(),
            test_object: report.test_object(),
            target_object: ObjectId::from_bytes([6; 32]),
            evidence: report.evidence().clone(),
        })
        .expect("well-formed substituted report");
        assert_eq!(
            sign_complete_attempt(&config, &authenticated, result, &trust, &signer),
            Err(AttestationError::Binding(ScbErrorCode::ContractUnknown))
        );
    }
}

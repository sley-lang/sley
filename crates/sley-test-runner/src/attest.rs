//! Signed completion of one already owned native worker attempt.
//!
//! The root service supplies a kernel-authenticated request, the unit owner's
//! launch-through-reap result, and an administrator-provisioned trust policy.
//! This module rechecks report bindings and success facts before it asks the
//! root-only signer to sign. It neither loads trust nor launches a worker.

use std::time::{SystemTime, UNIX_EPOCH};

use sley_scb1::ScbErrorCode;
use sley_tests::{
    HistoricalTrustPolicyV1, MeasuredTestAttestationParts, MeasuredTestAttestationV1,
    NativeExecutionEvidence, ROLE_MEASUREMENT, measurement_signature_preimage,
    unsigned_record_prefix,
};

use crate::config::RunnerConfig;
use crate::enforce::floor_page_cap;
use crate::ingress::AuthenticatedRunRequest;
use crate::outcome::{
    AdmissionRefusal, AttemptOutcome, ObservedTermination, Signer, SignerError, admit_for_signature,
};
use crate::owner::OwnedWorkerResult;
use crate::protocol::{RunRequest, RunResponse, RunStatus};
use crate::response::RunEvidence;
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

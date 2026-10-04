//! Typed evidence carried by the private supervisor response.
//!
//! A report is deterministic VM evidence, an attestation is a signed claim
//! about the host attempt, and a supervisor configuration describes the
//! enforced profile. This transport joins their canonical identities and
//! bounds their bytes. It does not verify the measurement signature or grant
//! native test admission; those checks belong to the transaction owner.

use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_record};
use sley_tests::{
    MeasuredTestAttestationV1, NativeExecutionEvidence, NativeExecutionReportV1, SupervisorConfigV1,
};

use crate::config::MAX_WORKER_OUTPUT_BYTES;
use crate::enforce::{check_elapsed, check_memory_evidence, floor_page_cap, runtime_max_usec};
use crate::protocol::{RunRequest, RunResponse, RunStatus};

/// Maximum response-embedded measurement attestation bytes.
pub const MAX_RESPONSE_ATTESTATION_BYTES: usize = 4_096;
/// Maximum response-embedded supervisor configuration bytes.
pub const MAX_RESPONSE_CONFIG_BYTES: usize = 65_536;
/// Maximum evidence record bytes, including its three SCB1 field wrappers.
pub const MAX_RESPONSE_EVIDENCE_BYTES: usize =
    MAX_WORKER_OUTPUT_BYTES + MAX_RESPONSE_ATTESTATION_BYTES + MAX_RESPONSE_CONFIG_BYTES + 512;
/// Maximum complete supervisor response frame bytes.
pub const MAX_RESPONSE_FRAME_BYTES: usize = MAX_RESPONSE_EVIDENCE_BYTES + 64;

fn mismatch() -> ScbError {
    ScbError::new(ScbErrorCode::ContractUnknown)
}

/// Exact canonical report, signed measurement claim, and configuration for
/// one native test attempt. Constructing or parsing this grants no trust.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunEvidence {
    report: NativeExecutionReportV1,
    attestation: MeasuredTestAttestationV1,
    supervisor_config: SupervisorConfigV1,
}

impl RunEvidence {
    /// Joins three canonical artifacts by their exact identities and bounds.
    ///
    /// # Errors
    ///
    /// Refuses oversized artifacts or mismatched report, attestation, and
    /// configuration identities. Signature trust is checked elsewhere.
    pub fn build(
        report: NativeExecutionReportV1,
        attestation: MeasuredTestAttestationV1,
        supervisor_config: SupervisorConfigV1,
    ) -> Result<Self, ScbError> {
        if report.stored_bytes().len() > MAX_WORKER_OUTPUT_BYTES
            || attestation.stored_bytes().len() > MAX_RESPONSE_ATTESTATION_BYTES
            || supervisor_config.stored_bytes().len() > MAX_RESPONSE_CONFIG_BYTES
        {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        if report.plan_id() != attestation.plan_id()
            || report.test_object() != attestation.test_object()
            || attestation.supervisor_config_id() != *supervisor_config.id().as_bytes()
        {
            return Err(mismatch());
        }
        match attestation.execution_report_id() {
            Some(id) if id == report.report_id() => {}
            None if matches!(report.evidence(), NativeExecutionEvidence::Rejected(_)) => {}
            _ => return Err(mismatch()),
        }
        Ok(Self {
            report,
            attestation,
            supervisor_config,
        })
    }

    /// Canonical deterministic report, not a measured pass by itself.
    #[must_use]
    pub const fn report(&self) -> &NativeExecutionReportV1 {
        &self.report
    }

    /// Parsed measurement claim; signature verification remains separate.
    #[must_use]
    pub const fn attestation(&self) -> &MeasuredTestAttestationV1 {
        &self.attestation
    }

    /// Parsed exact supervisor configuration referenced by the attestation.
    #[must_use]
    pub const fn supervisor_config(&self) -> &SupervisorConfigV1 {
        &self.supervisor_config
    }

    /// Encodes the three complete artifacts as one internal SCB1 record.
    ///
    /// # Errors
    ///
    /// Returns `SCB_RESOURCE_LIMIT` if the combined record exceeds its cap.
    pub fn encode_record(&self) -> Result<Vec<u8>, ScbError> {
        let record = encode_record(&[
            (1, self.report.stored_bytes().to_vec()),
            (2, self.attestation.stored_bytes().to_vec()),
            (3, self.supervisor_config.stored_bytes().to_vec()),
        ])?;
        if record.len() > MAX_RESPONSE_EVIDENCE_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(record)
    }

    /// Strictly parses the bounded evidence record and all three envelopes.
    ///
    /// # Errors
    ///
    /// Refuses oversized, malformed, noncanonical, or mismatched artifacts.
    pub fn parse_record(record: &[u8]) -> Result<Self, ScbError> {
        if record.len() > MAX_RESPONSE_EVIDENCE_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let mut cursor = ScbValueCursor::new(record)?;
        if cursor.read_record_field_count()? != 3 {
            return Err(ScbError::new(ScbErrorCode::FieldMissing));
        }
        let mut values = Vec::with_capacity(3);
        for expected_tag in 1..=3 {
            if cursor.read_uvar(32)? != expected_tag {
                return Err(ScbError::new(ScbErrorCode::FieldOrder));
            }
            values.push(cursor.read_sized_payload()?);
        }
        cursor.check_finished()?;
        if values[0].len() > MAX_WORKER_OUTPUT_BYTES
            || values[1].len() > MAX_RESPONSE_ATTESTATION_BYTES
            || values[2].len() > MAX_RESPONSE_CONFIG_BYTES
        {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let report = NativeExecutionReportV1::parse(values[0])?;
        let attestation = MeasuredTestAttestationV1::parse(values[1])?;
        let config = SupervisorConfigV1::parse(values[2])?;
        let evidence = Self::build(report, attestation, config)?;
        if evidence.encode_record()? != record {
            return Err(mismatch());
        }
        Ok(evidence)
    }
}

impl RunRequest {
    /// Checks a typed response against the exact requested test and socket UID.
    ///
    /// This is a data-consistency check for the client. It does not verify the
    /// daemon's signature, the trust manifest, or the actual host telemetry.
    ///
    /// # Errors
    ///
    /// Refuses absent or cross-boundary substituted evidence. Successful-run
    /// cap and deadline checks apply only to `Complete`; a signed diagnostic
    /// refusal may have no installed cap or worker output.
    pub fn verified_response_evidence<'a>(
        &self,
        response: &'a RunResponse,
        caller_uid: u32,
    ) -> Result<&'a RunEvidence, ScbError> {
        let evidence = response.evidence.as_ref().ok_or_else(mismatch)?;
        let program = self.verified_program()?;
        let report = evidence.report();
        let attestation = evidence.attestation();
        let config = evidence.supervisor_config();
        if report.plan_id() != self.plan_id
            || report.test_entity() != self.test_entity
            || report.test_object() != self.test_object
            || report.target_object() != program.selected().target_object
            || attestation.plan_id() != self.plan_id
            || attestation.test_object() != self.test_object
            || attestation.parts().attempt_nonce != self.nonce
            || attestation.workspace() != self.workspace
            || attestation.principal() != self.principal
            || attestation.parts().caller_uid != caller_uid
            || attestation.declared_limits() != self.declared_limits
            || !config.callers().iter().any(|caller| {
                caller.uid == caller_uid
                    && caller.workspace == self.workspace
                    && caller.principal == self.principal
            })
        {
            return Err(mismatch());
        }
        if response.status == RunStatus::Complete {
            if !attestation.claims_success() || attestation.execution_report_id().is_none() {
                return Err(mismatch());
            }
            let config_parts = config.parts();
            if !config_parts.page_size.is_power_of_two() {
                return Err(mismatch());
            }
            let (_, expected_cap) =
                floor_page_cap(self.declared_limits.memory_bytes, config_parts.page_size)
                    .map_err(|_| mismatch())?;
            let expected_runtime = runtime_max_usec(self.wall_ms).map_err(|_| mismatch())?;
            let expected_cap_text = expected_cap.to_string();
            let expected_runtime_text = expected_runtime.to_string();
            let property = |name: &str| {
                config
                    .properties()
                    .iter()
                    .find(|property| property.name == name)
                    .map(|property| property.value.as_str())
            };
            if attestation.installed_memory_cap() != expected_cap
                || property("MemoryMax") != Some(expected_cap_text.as_str())
                || property("RuntimeMaxUSec") != Some(expected_runtime_text.as_str())
            {
                return Err(mismatch());
            }
            let events = attestation.memory_events();
            check_memory_evidence(
                attestation.measured_memory_peak(),
                attestation.installed_memory_cap(),
                self.declared_limits.memory_bytes,
                events.max,
                events.oom,
                events.oom_kill,
            )
            .map_err(|_| mismatch())?;
            check_elapsed(attestation.elapsed_ns(), self.wall_ms).map_err(|_| mismatch())?;
            if matches!(report.evidence(), NativeExecutionEvidence::Observed { .. }) {
                self.verified_observed_worker_report(report.stored_bytes())?;
            }
        }
        Ok(evidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::outcome::{Ed25519MeasurementSigner, Signer};
    use crate::program::PortableTestProgram;
    use crate::unit::REQUIRED_PROPERTIES;
    use crate::worker::WorkerRequest;
    use sley_tests::{
        Caller, MeasuredTestAttestationParts, MemoryEvents, NativeExecutionReportParts, Property,
        REJECT_PHASE_EXECUTION, RejectedEvidence, SupervisorConfigParts, TERMINATION_COMPLETE,
        TERMINATION_PRELAUNCH_REFUSED, measurement_signature_preimage, unsigned_record_prefix,
    };

    fn complete_fixture() -> (RunRequest, RunResponse) {
        let worker = WorkerRequest::decode_frame(include_bytes!(
            "../../../conformance/native-worker/v1/observed-input.bin"
        ))
        .expect("canonical worker vector");
        let program = PortableTestProgram::parse(&worker.program_bytes).expect("portable program");
        let request = RunRequest::from_portable_program(
            &program,
            1_000,
            crate::nonce::random_attempt_nonce().expect("OS entropy"),
        )
        .expect("supervisor request");
        let report = NativeExecutionReportV1::parse(include_bytes!(
            "../../../conformance/native-worker/v1/observed-report.bin"
        ))
        .expect("canonical worker report");
        let mut properties = REQUIRED_PROPERTIES
            .iter()
            .map(|(name, value)| Property {
                name: (*name).to_owned(),
                value: (*value).to_owned(),
            })
            .collect::<Vec<_>>();
        properties.push(Property {
            name: "MemoryMax".to_owned(),
            value: "4096".to_owned(),
        });
        properties.push(Property {
            name: "RuntimeMaxUSec".to_owned(),
            value: "3000000".to_owned(),
        });
        properties.sort_by(|left, right| left.name.cmp(&right.name));
        let config = SupervisorConfigV1::build(SupervisorConfigParts {
            worker_digest: [7; 32],
            supervisor_digest: [8; 32],
            properties,
            callers: vec![Caller {
                uid: 1_000,
                workspace: request.workspace,
                principal: request.principal,
            }],
            page_size: 4_096,
            cleanup_millis: 2_000,
            launch_profile: 1,
        })
        .expect("supervisor config");
        let signer = Ed25519MeasurementSigner::from_secret_bytes([3; 32]);
        let mut parts = MeasuredTestAttestationParts {
            key_id: signer.public_key(),
            trust_policy_id: [5; 32],
            supervisor_config_id: *config.id().as_bytes(),
            plan_id: request.plan_id,
            test_object: request.test_object,
            execution_report_id: Some(report.report_id()),
            attempt_nonce: request.nonce,
            workspace: request.workspace,
            principal: request.principal,
            caller_uid: 1_000,
            declared_limits: request.declared_limits,
            installed_memory_cap: 4_096,
            elapsed_ns: 1,
            measured_memory_peak: 1,
            memory_events: MemoryEvents {
                max: 0,
                oom: 0,
                oom_kill: 0,
            },
            termination: TERMINATION_COMPLETE,
            complete_output: true,
            empty_cgroup_confirmed: true,
            recorded_unix_millis: 1_000,
            signature: [0; 64],
        };
        let unsigned = unsigned_record_prefix(&parts).expect("unsigned attestation");
        let preimage = measurement_signature_preimage(&unsigned).expect("signing preimage");
        parts.signature = signer.sign(&preimage).expect("test signature");
        let attestation = MeasuredTestAttestationV1::build(parts).expect("attestation");
        let evidence = RunEvidence::build(report, attestation, config).expect("bound evidence");
        (
            request,
            RunResponse {
                status: RunStatus::Complete,
                code: 0,
                evidence: Some(evidence),
            },
        )
    }

    fn replace_measured_facts(
        response: &mut RunResponse,
        change: impl FnOnce(&mut MeasuredTestAttestationParts, &mut SupervisorConfigParts),
    ) {
        let old = response.evidence.as_ref().expect("fixture evidence");
        let report = old.report().clone();
        let mut config_parts = old.supervisor_config().parts().clone();
        let mut attestation_parts = old.attestation().parts().clone();
        change(&mut attestation_parts, &mut config_parts);
        let config = SupervisorConfigV1::build(config_parts).expect("modified config");
        attestation_parts.supervisor_config_id = *config.id().as_bytes();
        attestation_parts.signature = [0; 64];
        let unsigned = unsigned_record_prefix(&attestation_parts).expect("unsigned attestation");
        let preimage = measurement_signature_preimage(&unsigned).expect("signing preimage");
        attestation_parts.signature = Ed25519MeasurementSigner::from_secret_bytes([3; 32])
            .sign(&preimage)
            .expect("test signature");
        let attestation = MeasuredTestAttestationV1::build(attestation_parts).expect("attestation");
        response.evidence =
            Some(RunEvidence::build(report, attestation, config).expect("evidence"));
    }

    fn diagnostic_fixture() -> (RunRequest, RunResponse) {
        let (request, complete) = complete_fixture();
        let old = complete.evidence.expect("complete evidence");
        let selected = request.verified_program().expect("program").selected();
        let report = NativeExecutionReportV1::build(NativeExecutionReportParts {
            plan_id: request.plan_id,
            test_entity: request.test_entity,
            test_object: request.test_object,
            target_object: selected.target_object,
            evidence: NativeExecutionEvidence::Rejected(
                RejectedEvidence::from_parts(
                    REJECT_PHASE_EXECUTION,
                    29_211,
                    "NATIVE_TEST_EXECUTION_REJECTED",
                )
                .expect("rejection"),
            ),
        })
        .expect("rejected report");
        let mut config_parts = old.supervisor_config().parts().clone();
        config_parts
            .properties
            .iter_mut()
            .find(|property| property.name == "MemoryMax")
            .expect("memory property")
            .value = "8192".to_owned();
        let config = SupervisorConfigV1::build(config_parts).expect("diagnostic config");
        let mut parts = old.attestation().parts().clone();
        parts.supervisor_config_id = *config.id().as_bytes();
        parts.execution_report_id = None;
        parts.installed_memory_cap = 0;
        parts.elapsed_ns = 0;
        parts.measured_memory_peak = 0;
        parts.termination = TERMINATION_PRELAUNCH_REFUSED;
        parts.complete_output = false;
        parts.signature = [0; 64];
        let unsigned = unsigned_record_prefix(&parts).expect("unsigned attestation");
        let preimage = measurement_signature_preimage(&unsigned).expect("signing preimage");
        parts.signature = Ed25519MeasurementSigner::from_secret_bytes([3; 32])
            .sign(&preimage)
            .expect("test signature");
        let attestation = MeasuredTestAttestationV1::build(parts).expect("attestation");
        let evidence =
            RunEvidence::build(report, attestation, config).expect("diagnostic evidence");
        (
            request,
            RunResponse {
                status: RunStatus::Refused,
                code: 7,
                evidence: Some(evidence),
            },
        )
    }

    #[test]
    fn complete_response_carries_three_bound_artifacts() {
        let (request, response) = complete_fixture();
        let encoded = response.encode_frame().expect("response frame");
        let parsed = RunResponse::decode_frame(&encoded).expect("response parse");
        assert_eq!(parsed, response);
        let evidence = request
            .verified_response_evidence(&parsed, 1_000)
            .expect("request-bound response");
        assert_eq!(evidence.report().plan_id(), request.plan_id);
        assert!(evidence.attestation().claims_success());
        assert_eq!(
            evidence.supervisor_config().id().as_bytes(),
            &evidence.attestation().supervisor_config_id()
        );
    }

    #[test]
    fn complete_worker_owned_rejection_remains_bound_diagnostic_output() {
        let (request, mut response) = complete_fixture();
        let old = response.evidence.take().expect("complete evidence");
        let report = NativeExecutionReportV1::build(NativeExecutionReportParts {
            plan_id: request.plan_id,
            test_entity: request.test_entity,
            test_object: request.test_object,
            target_object: request
                .verified_program()
                .expect("program")
                .selected()
                .target_object,
            evidence: NativeExecutionEvidence::Rejected(
                RejectedEvidence::from_parts(
                    REJECT_PHASE_EXECUTION,
                    29_211,
                    "NATIVE_TEST_EXECUTION_REJECTED",
                )
                .expect("worker rejection"),
            ),
        })
        .expect("rejected report");
        let mut parts = old.attestation().parts().clone();
        parts.execution_report_id = Some(report.report_id());
        parts.signature = [0; 64];
        let unsigned = unsigned_record_prefix(&parts).expect("unsigned attestation");
        let preimage = measurement_signature_preimage(&unsigned).expect("signature preimage");
        parts.signature = Ed25519MeasurementSigner::from_secret_bytes([3; 32])
            .sign(&preimage)
            .expect("test signature");
        let attestation = MeasuredTestAttestationV1::build(parts).expect("attestation");
        response.evidence = Some(
            RunEvidence::build(report, attestation, old.supervisor_config().clone())
                .expect("bound rejection"),
        );
        let frame = response.encode_frame().expect("complete rejected frame");
        let parsed = RunResponse::decode_frame(&frame).expect("canonical frame");
        let evidence = request
            .verified_response_evidence(&parsed, 1_000)
            .expect("request-bound complete rejection");
        assert!(evidence.attestation().claims_success());
        assert!(matches!(
            evidence.report().evidence(),
            NativeExecutionEvidence::Rejected(_)
        ));
    }

    #[test]
    fn complete_response_rejects_cross_attempt_and_missing_measurement() {
        let (request, mut response) = complete_fixture();
        assert_eq!(
            request
                .verified_response_evidence(&response, 1_001)
                .expect_err("wrong socket UID")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut other_attempt = request.clone();
        other_attempt.nonce = [8; 32];
        assert_eq!(
            other_attempt
                .verified_response_evidence(&response, 1_000)
                .expect_err("wrong attempt nonce")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        response.code = 7;
        assert_eq!(
            response
                .encode_frame()
                .expect_err("complete failure code")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        response.code = 0;
        response.evidence = None;
        assert_eq!(
            response
                .encode_frame()
                .expect_err("missing measurement")
                .code(),
            ScbErrorCode::ContractUnknown
        );
    }

    #[test]
    fn complete_response_refuses_measurement_and_installed_limit_substitution() {
        let (request, mut response) = complete_fixture();
        replace_measured_facts(&mut response, |attestation, _| {
            attestation.measured_memory_peak = 4_097;
        });
        assert_eq!(
            request
                .verified_response_evidence(&response, 1_000)
                .expect_err("peak exceeds installed cap")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let (request, mut response) = complete_fixture();
        replace_measured_facts(&mut response, |attestation, _| {
            attestation.elapsed_ns = 1_000_000_000;
        });
        assert_eq!(
            request
                .verified_response_evidence(&response, 1_000)
                .expect_err("elapsed meets wall deadline")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let (request, mut response) = complete_fixture();
        replace_measured_facts(&mut response, |_, config| {
            config
                .properties
                .iter_mut()
                .find(|property| property.name == "MemoryMax")
                .expect("memory property")
                .value = "8192".to_owned();
        });
        assert_eq!(
            request
                .verified_response_evidence(&response, 1_000)
                .expect_err("installed cap property substituted")
                .code(),
            ScbErrorCode::ContractUnknown
        );
    }

    #[test]
    fn signed_prelaunch_refusal_preserves_diagnostics_without_an_installed_cap() {
        let (request, response) = diagnostic_fixture();
        let frame = response.encode_frame().expect("diagnostic response");
        let parsed = RunResponse::decode_frame(&frame).expect("parsed diagnostics");
        let evidence = request
            .verified_response_evidence(&parsed, 1_000)
            .expect("request-bound diagnostic evidence");
        assert!(!evidence.attestation().claims_success());
        assert_eq!(evidence.attestation().execution_report_id(), None);
        assert!(matches!(
            evidence.report().evidence(),
            NativeExecutionEvidence::Rejected(_)
        ));
    }
}

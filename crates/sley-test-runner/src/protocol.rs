//! Closed `RunNativeTest` IPC protocol.
//!
//! The daemon exposes exactly one request shape. There is no command
//! string, executable path, unit-property map, credential selection, or
//! signing-key field anywhere in these types: the type system, not a
//! runtime check, excludes caller-chosen authority. Requests carry an
//! 8-byte magic followed by a 4-byte big-endian length and are refused past
//! [`MAX_REQUEST_BYTES`](crate::config::MAX_REQUEST_BYTES).
//! Field 4 is an optional candidate identity because explicit-root plans
//! have no candidate. Field 13 carries the exact bounded worker frame so
//! the daemon can stage it read-only after authenticating the outer scope.
//! Response version 2 carries a bounded optional triple: canonical worker
//! report, measured attestation, and exact supervisor configuration. Parsing
//! the triple never grants signature trust or native test admission. This
//! internal format has not shipped with a daemon.

use sley_id::{CandidateId, EntityId, ObjectId, PolicyRootId, PrincipalId, WorkspaceId};
use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_record, encode_union, encode_uvar};
use sley_tests::{NativeExecutionEvidence, NativeExecutionReportV1};
use sley_vm::{
    SSMC1_DECODER_LIMITS_HASH, SSMC1_FIELD_SCHEMA_HASH,
    native_execution::{NativeDeclaredLimits, NativeExecutionObservationV1},
};

use crate::config::{MAX_REQUEST_BYTES, MAX_WORKER_OUTPUT_BYTES};
use crate::program::PortableTestProgram;
use crate::response::{MAX_RESPONSE_FRAME_BYTES, RunEvidence};
use crate::worker::WorkerRequest;

/// IPC magic for the closed run protocol.
pub const RUN_MAGIC: &[u8; 8] = b"SLEYRUN1";
/// IPC record version.
pub const RUN_VERSION: u64 = 1;
/// Internal measured-response record version; request framing stays v1.
pub const RUN_RESPONSE_VERSION: u64 = 2;

/// Daemon-owned closed run request. The worker executable, unit properties,
/// and signing keys come from administrator configuration, never from these
/// bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunRequest {
    /// Workspace under test; must match the caller's authorized workspace.
    pub workspace: WorkspaceId,
    /// Principal the caller runs as; must match the authorized principal.
    pub principal: PrincipalId,
    /// Candidate the plan was derived from, absent for explicit-root tests.
    /// The plan ID still binds the owner-derived selection in both modes.
    pub candidate_id: Option<CandidateId>,
    /// Native plan under execution.
    pub plan_id: sley_id::NativeTestPlanId,
    /// Test object under execution.
    pub test_object: ObjectId,
    /// Test entity under execution.
    pub test_entity: EntityId,
    /// Target function entity.
    pub target_function: EntityId,
    /// Protected policy root the plan binds.
    pub policy_root: PolicyRootId,
    /// Literal `TestCase` limits, without policy clamping.
    pub declared_limits: NativeDeclaredLimits,
    /// Supervisor wall budget in milliseconds; zero refuses.
    pub wall_ms: u64,
    /// Host-random attempt nonce bound into the attestation.
    pub nonce: [u8; 32],
    /// Exact bounded worker input frame. The daemon owns its read-only
    /// staging and checks the embedded limits before any worker launch.
    /// Program semantics remain the N5 owner and worker's responsibility.
    pub worker_frame: Vec<u8>,
}

/// Daemon-owned run response status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    /// Worker completed with bounded output attached.
    Complete,
    /// Pre-launch refusal; no worker spawned.
    Refused,
    /// Deadline or worker failure; diagnostic only.
    Failed,
}

impl RunStatus {
    const fn tag(self) -> u32 {
        match self {
            Self::Complete => 1,
            Self::Refused => 2,
            Self::Failed => 3,
        }
    }

    const fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            1 => Some(Self::Complete),
            2 => Some(Self::Refused),
            3 => Some(Self::Failed),
            _ => None,
        }
    }
}

/// Daemon-owned closed run response with bounded canonical evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunResponse {
    /// Terminal status of the run.
    pub status: RunStatus,
    /// Stable refusal/failure code; zero for `Complete`.
    pub code: u32,
    /// Canonical report, measurement claim, and configuration when available.
    /// A `Complete` response requires this triple; diagnostic refusals may
    /// have none. Parsing it never verifies measurement trust.
    pub evidence: Option<RunEvidence>,
}

fn declared_record(limits: NativeDeclaredLimits) -> Result<Vec<u8>, ScbError> {
    encode_record(&[
        (1, encode_uvar(limits.fuel)),
        (2, encode_uvar(limits.memory_bytes)),
        (3, encode_uvar(limits.output_bytes)),
        (4, encode_uvar(limits.effect_count)),
        (5, encode_uvar(limits.call_depth)),
        (6, encode_uvar(limits.wall_timeout_millis)),
    ])
}

fn parse_declared(value: &[u8]) -> Result<NativeDeclaredLimits, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_record_field_count()?;
    if count != 6 {
        return Err(ScbError::new(ScbErrorCode::FieldMissing));
    }
    let mut fields = [0_u64; 6];
    for (index, slot) in fields.iter_mut().enumerate() {
        let tag = cursor.read_uvar(32)?;
        if tag != index as u64 + 1 {
            return Err(ScbError::new(ScbErrorCode::FieldOrder));
        }
        let bytes = cursor.read_sized_payload()?;
        let mut inner = ScbValueCursor::new(bytes)?;
        *slot = inner.read_uvar(64)?;
        inner.check_finished()?;
    }
    cursor.check_finished()?;
    Ok(NativeDeclaredLimits {
        fuel: fields[0],
        memory_bytes: fields[1],
        output_bytes: fields[2],
        effect_count: fields[3],
        call_depth: fields[4],
        wall_timeout_millis: fields[5],
    })
}

fn read_id(value: &[u8]) -> Result<[u8; 32], ScbError> {
    <[u8; 32]>::try_from(value).map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))
}

fn read_uvar(value: &[u8]) -> Result<u64, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let parsed = cursor.read_uvar(64)?;
    cursor.check_finished()?;
    Ok(parsed)
}

fn read_nonce(value: &[u8]) -> Result<[u8; 32], ScbError> {
    read_id(value)
}

fn option_candidate(value: Option<CandidateId>) -> Result<Vec<u8>, ScbError> {
    match value {
        None => encode_union(0, &[]),
        Some(id) => encode_union(1, id.as_bytes()),
    }
}

fn read_option_candidate(value: &[u8]) -> Result<Option<CandidateId>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let (tag, payload) = cursor.read_union()?;
    cursor.check_finished()?;
    match tag {
        0 if payload.is_empty() => Ok(None),
        1 => Ok(Some(CandidateId::from_bytes(read_id(payload)?))),
        _ => Err(ScbError::new(ScbErrorCode::UnionInvalid)),
    }
}

impl RunRequest {
    /// Constructs one complete supervisor request from a selected portable
    /// Sley program. All semantic identities, limits, and ordered input hashes
    /// come from the plan and statically validated `TestCase`; only the host
    /// attempt nonce and a no-greater-than-declared wall budget are supplied.
    ///
    /// This does not authenticate the caller or authorize the plan. The
    /// supervisor must still bind the request to kernel peer credentials.
    ///
    /// # Errors
    ///
    /// Refuses invalid source, zero or expanded wall budget, or oversized
    /// worker and outer frames.
    pub fn from_portable_program(
        program: &PortableTestProgram,
        wall_ms: u64,
        nonce: [u8; 32],
    ) -> Result<Self, ScbError> {
        let selected = program.selected();
        let plan = program.plan();
        let worker_frame = program.derive_worker_request()?.encode_frame()?;
        let request = Self {
            workspace: plan.workspace(),
            principal: plan.resource_policy().principal(),
            candidate_id: plan.candidate_id(),
            plan_id: plan.plan_id(),
            test_object: selected.test_object,
            test_entity: selected.test_entity,
            target_function: selected.target_function,
            policy_root: plan.policy_root(),
            declared_limits: selected.declared_limits,
            wall_ms,
            nonce,
            worker_frame,
        };
        request.encode_frame()?;
        Ok(request)
    }

    /// Verifies a worker's canonical observed report against authenticated
    /// request scope and the exact embedded worker input.
    ///
    /// This checks report/observation identity, root, epoch, function, input
    /// hashes, execution profile, and limits. It does not prove that the VM
    /// ran, measure the host, or admit the test; those are later owners.
    ///
    /// # Errors
    ///
    /// Refuses malformed, oversized, rejected, or cross-boundary substituted
    /// report bytes.
    pub fn verified_observed_worker_report(
        &self,
        output: &[u8],
    ) -> Result<NativeExecutionReportV1, ScbError> {
        if output.len() > MAX_WORKER_OUTPUT_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let program = self.verified_program()?;
        let worker = self.worker_request()?;
        let report = NativeExecutionReportV1::parse(output)?;
        let selected = program.selected();
        if report.plan_id() != self.plan_id
            || report.test_entity() != selected.test_entity
            || report.test_object() != selected.test_object
            || report.target_object() != selected.target_object
        {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        let NativeExecutionEvidence::Observed { stored, .. } = report.evidence() else {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        };
        let observation = NativeExecutionObservationV1::parse_stored(stored)?;
        if observation.schema_epoch() != program.plan().semantic_epoch()
            || observation.field_schema_hash() != &SSMC1_FIELD_SCHEMA_HASH
            || observation.decoder_limits_hash() != &SSMC1_DECODER_LIMITS_HASH
            || observation.state_root() != program.plan().proposed_root()
            || observation.function() != selected.target_function
            || observation.declared_limits() != selected.declared_limits
            || observation.implementation_limits() != worker.implementation_limits
            || observation.input_hashes().len() != worker.input_hashes.len()
            || observation
                .input_hashes()
                .iter()
                .zip(&worker.input_hashes)
                .any(|(actual, declared)| actual.as_bytes() != declared)
        {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        Ok(report)
    }

    /// Parses the portable program and binds its owner plan, test, policy,
    /// and limits to this authenticated request and the worker envelope.
    ///
    /// # Errors
    ///
    /// Refuses malformed program bytes or a mismatch across any boundary.
    /// This verifies data consistency; it does not authorize the plan.
    pub fn verified_program(&self) -> Result<PortableTestProgram, ScbError> {
        let worker = self.worker_request()?;
        let program = PortableTestProgram::parse(&worker.program_bytes)?;
        let plan = program.plan();
        let selected = program.selected();
        if plan.workspace() != self.workspace
            || plan.resource_policy().principal() != self.principal
            || plan.candidate_id() != self.candidate_id
            || plan.plan_id() != self.plan_id
            || selected.test_object != self.test_object
            || selected.test_entity != self.test_entity
            || selected.target_function != self.target_function
            || plan.policy_root() != self.policy_root
            || selected.declared_limits != self.declared_limits
            || plan.implementation_limits() != worker.implementation_limits
            || program.derive_worker_request()?.input_hashes != worker.input_hashes
        {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        Ok(program)
    }

    /// Parses the embedded worker frame and checks its declared limits
    /// against this authenticated outer request.
    ///
    /// # Errors
    ///
    /// Returns the worker-frame refusal or `SCB_CONTRACT_UNKNOWN` when its
    /// limits differ; zero or expanded wall budgets refuse separately.
    pub fn worker_request(&self) -> Result<WorkerRequest, ScbError> {
        let worker = WorkerRequest::decode_frame(&self.worker_frame)?;
        if worker.declared_limits != self.declared_limits {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        if self.wall_ms == 0 || self.wall_ms > self.declared_limits.wall_timeout_millis {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(worker)
    }

    /// Encodes one length-delimited request frame for the socket.
    ///
    /// # Errors
    ///
    /// Returns `SCB_RESOURCE_LIMIT` when the frame exceeds the socket bound.
    pub fn encode_frame(&self) -> Result<Vec<u8>, ScbError> {
        self.worker_request()?;
        let record = encode_record(&[
            (1, encode_uvar(RUN_VERSION)),
            (2, self.workspace.as_bytes().to_vec()),
            (3, self.principal.as_bytes().to_vec()),
            (4, option_candidate(self.candidate_id)?),
            (5, self.plan_id.as_bytes().to_vec()),
            (6, self.test_object.as_bytes().to_vec()),
            (7, self.test_entity.as_bytes().to_vec()),
            (8, self.target_function.as_bytes().to_vec()),
            (9, self.policy_root.as_bytes().to_vec()),
            (10, declared_record(self.declared_limits)?),
            (11, encode_uvar(self.wall_ms)),
            (12, self.nonce.to_vec()),
            (13, self.worker_frame.clone()),
        ])?;
        let mut frame = RUN_MAGIC.to_vec();
        let len =
            u32::try_from(record.len()).map_err(|_| ScbError::new(ScbErrorCode::ResourceLimit))?;
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(&record);
        if frame.len() > MAX_REQUEST_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(frame)
    }

    /// Strictly decodes one length-delimited request frame.
    ///
    /// # Errors
    ///
    /// Returns the first stable SCB1 refusal for magic, length, shape, or
    /// version violations.
    pub fn decode_frame(frame: &[u8]) -> Result<Self, ScbError> {
        if frame.len() < 12 || frame.len() > MAX_REQUEST_BYTES {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        if frame[..8] != *RUN_MAGIC {
            return Err(ScbError::new(ScbErrorCode::MagicInvalid));
        }
        let len = u32::from_be_bytes(
            frame[8..12]
                .try_into()
                .map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))?,
        );
        if len as usize != frame.len() - 12 {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        let mut cursor = ScbValueCursor::new(&frame[12..])?;
        let count = cursor.read_record_field_count()?;
        if count != 13 {
            return Err(ScbError::new(ScbErrorCode::FieldMissing));
        }
        let mut tags = Vec::with_capacity(13);
        let mut values = Vec::with_capacity(13);
        for _ in 0..13 {
            tags.push(cursor.read_uvar(32)?);
            values.push(cursor.read_sized_payload()?.to_vec());
        }
        cursor.check_finished()?;
        for (index, tag) in tags.iter().enumerate() {
            if *tag != index as u64 + 1 {
                return Err(ScbError::new(ScbErrorCode::FieldOrder));
            }
        }
        if read_uvar(&values[0])? != RUN_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        let request = Self {
            workspace: WorkspaceId::from_bytes(read_id(&values[1])?),
            principal: PrincipalId::from_bytes(read_id(&values[2])?),
            candidate_id: read_option_candidate(&values[3])?,
            plan_id: sley_id::NativeTestPlanId::from_bytes(read_id(&values[4])?),
            test_object: ObjectId::from_bytes(read_id(&values[5])?),
            test_entity: EntityId::from_bytes(read_id(&values[6])?),
            target_function: EntityId::from_bytes(read_id(&values[7])?),
            policy_root: PolicyRootId::from_bytes(read_id(&values[8])?),
            declared_limits: parse_declared(&values[9])?,
            wall_ms: read_uvar(&values[10])?,
            nonce: read_nonce(&values[11])?,
            worker_frame: values[12].clone(),
        };
        request.worker_request()?;
        Ok(request)
    }
}

impl RunResponse {
    fn validate_shape(&self) -> Result<(), ScbError> {
        match self.status {
            RunStatus::Complete
                if self.code == 0
                    && self
                        .evidence
                        .as_ref()
                        .is_some_and(|evidence| evidence.attestation().claims_success()) =>
            {
                Ok(())
            }
            RunStatus::Refused | RunStatus::Failed if self.code != 0 => Ok(()),
            _ => Err(ScbError::new(ScbErrorCode::ContractUnknown)),
        }
    }

    /// Encodes one length-delimited response frame.
    ///
    /// # Errors
    ///
    /// Refuses missing complete evidence, invalid status/code combinations,
    /// or an oversized response frame.
    pub fn encode_frame(&self) -> Result<Vec<u8>, ScbError> {
        self.validate_shape()?;
        let evidence = match &self.evidence {
            None => encode_union(0, &[])?,
            Some(value) => encode_union(1, &value.encode_record()?)?,
        };
        let record = encode_record(&[
            (1, encode_uvar(RUN_RESPONSE_VERSION)),
            (2, encode_uvar(u64::from(self.status.tag()))),
            (3, encode_uvar(u64::from(self.code))),
            (4, evidence),
        ])?;
        let mut frame = RUN_MAGIC.to_vec();
        let len =
            u32::try_from(record.len()).map_err(|_| ScbError::new(ScbErrorCode::ResourceLimit))?;
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(&record);
        if frame.len() > MAX_RESPONSE_FRAME_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(frame)
    }

    /// Strictly decodes one length-delimited response frame.
    ///
    /// # Errors
    ///
    /// Returns the first stable SCB1 refusal for frame bounds, magic, length,
    /// shape, version, status, or evidence violations.
    pub fn decode_frame(frame: &[u8]) -> Result<Self, ScbError> {
        if frame.len() < 12 || frame.len() > MAX_RESPONSE_FRAME_BYTES {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        if frame[..8] != *RUN_MAGIC {
            return Err(ScbError::new(ScbErrorCode::MagicInvalid));
        }
        let len = u32::from_be_bytes(
            frame[8..12]
                .try_into()
                .map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))?,
        );
        if len as usize != frame.len() - 12 {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        let mut cursor = ScbValueCursor::new(&frame[12..])?;
        let count = cursor.read_record_field_count()?;
        if count != 4 {
            return Err(ScbError::new(ScbErrorCode::FieldMissing));
        }
        let mut values = Vec::with_capacity(4);
        for index in 0_u64..4 {
            let tag = cursor.read_uvar(32)?;
            if tag != index + 1 {
                return Err(ScbError::new(ScbErrorCode::FieldOrder));
            }
            values.push(cursor.read_sized_payload()?.to_vec());
        }
        cursor.check_finished()?;
        if read_uvar(&values[0])? != RUN_RESPONSE_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        let status_tag = u32::try_from(read_uvar(&values[1])?)
            .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
        let status = RunStatus::from_tag(status_tag)
            .ok_or_else(|| ScbError::new(ScbErrorCode::UnionInvalid))?;
        let code = u32::try_from(read_uvar(&values[2])?)
            .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
        let mut evidence_cursor = ScbValueCursor::new(&values[3])?;
        let (evidence_tag, evidence_bytes) = evidence_cursor.read_union()?;
        evidence_cursor.check_finished()?;
        let evidence = match evidence_tag {
            0 if evidence_bytes.is_empty() => None,
            1 => Some(RunEvidence::parse_record(evidence_bytes)?),
            _ => return Err(ScbError::new(ScbErrorCode::UnionInvalid)),
        };
        let response = Self {
            status,
            code,
            evidence,
        };
        response.validate_shape()?;
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RunRequest {
        let declared_limits = NativeDeclaredLimits {
            fuel: 100,
            memory_bytes: 4_096,
            output_bytes: 64,
            effect_count: 0,
            call_depth: 8,
            wall_timeout_millis: 1_000,
        };
        RunRequest {
            workspace: WorkspaceId::from_bytes([1; 32]),
            principal: PrincipalId::from_bytes([2; 32]),
            candidate_id: Some(CandidateId::from_bytes([3; 32])),
            plan_id: sley_id::NativeTestPlanId::from_bytes([4; 32]),
            test_object: ObjectId::from_bytes([5; 32]),
            test_entity: EntityId::from_bytes([6; 32]),
            target_function: EntityId::from_bytes([7; 32]),
            policy_root: PolicyRootId::from_bytes([8; 32]),
            declared_limits,
            wall_ms: 1_000,
            nonce: [9; 32],
            worker_frame: WorkerRequest {
                program_bytes: vec![1, 2, 3],
                input_hashes: vec![[10; 32]],
                declared_limits,
                implementation_limits:
                    sley_vm::native_execution::NativeImplementationLimits::HARD_MAXIMA,
            }
            .encode_frame()
            .expect("worker frame"),
        }
    }

    #[test]
    fn request_roundtrips_exactly() {
        let frame = request().encode_frame().expect("encodes");
        assert!(frame.len() <= MAX_REQUEST_BYTES);
        assert_eq!(&frame[..8], RUN_MAGIC);
        assert_eq!(
            RunRequest::decode_frame(&frame).expect("decodes"),
            request()
        );
        let explicit = RunRequest {
            candidate_id: None,
            ..request()
        };
        let explicit_frame = explicit.encode_frame().expect("explicit root encodes");
        assert_eq!(
            RunRequest::decode_frame(&explicit_frame).expect("explicit root decodes"),
            explicit
        );
        assert_ne!(explicit_frame, frame);
        let mut large = request();
        let mut worker = large.worker_request().expect("embedded worker");
        worker.program_bytes = vec![0x41; 70_000];
        large.worker_frame = worker.encode_frame().expect("large worker frame");
        let large_frame = large.encode_frame().expect("bounded large request");
        assert!(large_frame.len() > 65_536);
        assert_eq!(
            RunRequest::decode_frame(&large_frame).expect("large request decodes"),
            large
        );
    }

    #[test]
    fn embedded_worker_limits_and_outer_wall_must_agree() {
        let valid = request();
        let mut changed = valid.clone();
        let mut worker = changed.worker_request().expect("embedded worker");
        worker.declared_limits.fuel = 99;
        changed.worker_frame = worker.encode_frame().expect("changed worker frame");
        assert_eq!(
            changed
                .encode_frame()
                .expect_err("mismatched limits")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut raw = valid.encode_frame().expect("outer frame");
        assert_eq!(valid.worker_frame.len(), changed.worker_frame.len());
        let offset = raw
            .windows(valid.worker_frame.len())
            .position(|part| part == valid.worker_frame)
            .expect("embedded frame position");
        raw[offset..offset + changed.worker_frame.len()].copy_from_slice(&changed.worker_frame);
        assert_eq!(
            RunRequest::decode_frame(&raw)
                .expect_err("mismatched nested frame")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        for wall_ms in [0, valid.declared_limits.wall_timeout_millis + 1] {
            let changed = RunRequest {
                wall_ms,
                ..valid.clone()
            };
            assert_eq!(
                changed
                    .encode_frame()
                    .expect_err("invalid wall budget")
                    .code(),
                ScbErrorCode::ResourceLimit
            );
        }
    }

    #[test]
    fn request_refuses_tampered_frames() {
        let frame = request().encode_frame().expect("encodes");
        let mut bad_magic = frame.clone();
        bad_magic[0] ^= 0x01;
        assert_eq!(
            RunRequest::decode_frame(&bad_magic)
                .expect_err("magic")
                .code(),
            ScbErrorCode::MagicInvalid
        );
        let mut bad_len = frame.clone();
        bad_len[11] = bad_len[11].wrapping_add(1);
        assert_eq!(
            RunRequest::decode_frame(&bad_len)
                .expect_err("length")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        assert_eq!(
            RunRequest::decode_frame(&frame[..frame.len() - 1])
                .expect_err("truncated")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        assert_eq!(
            RunRequest::decode_frame(&[]).expect_err("empty").code(),
            ScbErrorCode::LengthOverflow
        );
        assert_eq!(
            read_option_candidate(&encode_union(0, &[1]).expect("union"))
                .expect_err("nonempty absent candidate")
                .code(),
            ScbErrorCode::UnionInvalid
        );
        assert_eq!(
            read_option_candidate(&encode_union(1, &[1; 31]).expect("union"))
                .expect_err("short candidate")
                .code(),
            ScbErrorCode::LengthOverflow
        );
        assert_eq!(
            read_option_candidate(&encode_union(2, &[]).expect("union"))
                .expect_err("unknown candidate tag")
                .code(),
            ScbErrorCode::UnionInvalid
        );
    }

    #[test]
    fn response_refusal_roundtrips_and_complete_requires_evidence() {
        let incomplete = RunResponse {
            status: RunStatus::Complete,
            code: 0,
            evidence: None,
        };
        assert_eq!(
            incomplete
                .encode_frame()
                .expect_err("missing evidence")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let refused = RunResponse {
            status: RunStatus::Refused,
            code: 7,
            evidence: None,
        };
        let frame = refused.encode_frame().expect("encodes");
        assert_eq!(RunResponse::decode_frame(&frame).expect("decodes"), refused);
        assert_eq!(
            RunResponse::decode_frame(&vec![0; MAX_RESPONSE_FRAME_BYTES + 1])
                .expect_err("oversized response before parse")
                .code(),
            ScbErrorCode::LengthOverflow
        );
    }

    #[test]
    fn status_tags_are_frozen() {
        assert_eq!(RunStatus::Complete.tag(), 1);
        assert_eq!(RunStatus::Refused.tag(), 2);
        assert_eq!(RunStatus::Failed.tag(), 3);
        assert_eq!(RunStatus::from_tag(0), None);
        assert_eq!(RunStatus::from_tag(4), None);
    }
}

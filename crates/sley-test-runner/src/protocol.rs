//! Closed `RunNativeTest` IPC protocol.
//!
//! The daemon exposes exactly one request shape. There is no command
//! string, executable path, unit-property map, credential selection, or
//! signing-key field anywhere in these types: the type system, not a
//! runtime check, excludes caller-chosen authority. Requests arrive with a
//! 4-byte big-endian length prefix and are refused past
//! [`MAX_REQUEST_BYTES`](crate::config::MAX_REQUEST_BYTES).

use sley_id::{CandidateId, EntityId, ObjectId, PolicyRootId, PrincipalId, WorkspaceId};
use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_record, encode_uvar};
use sley_vm::native_execution::NativeDeclaredLimits;

use crate::config::MAX_REQUEST_BYTES;

/// IPC magic for the closed run protocol.
pub const RUN_MAGIC: &[u8; 8] = b"SLEYRUN1";
/// IPC record version.
pub const RUN_VERSION: u64 = 1;

/// Daemon-owned closed run request. The worker executable, unit properties,
/// and signing keys come from administrator configuration, never from these
/// bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunRequest {
    /// Workspace under test; must match the caller's authorized workspace.
    pub workspace: WorkspaceId,
    /// Principal the caller runs as; must match the authorized principal.
    pub principal: PrincipalId,
    /// Candidate the plan was derived from.
    pub candidate_id: CandidateId,
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

/// Daemon-owned closed run response with daemon-bounded output bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunResponse {
    /// Terminal status of the run.
    pub status: RunStatus,
    /// Stable refusal/failure code; zero for `Complete`.
    pub code: u32,
    /// Daemon-owned bounded worker output; empty unless `Complete`.
    pub output: Vec<u8>,
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

impl RunRequest {
    /// Encodes one length-delimited request frame for the socket.
    ///
    /// # Errors
    ///
    /// Returns `SCB_RESOURCE_LIMIT` when the frame exceeds the socket bound.
    pub fn encode_frame(&self) -> Result<Vec<u8>, ScbError> {
        let record = encode_record(&[
            (1, encode_uvar(RUN_VERSION)),
            (2, self.workspace.as_bytes().to_vec()),
            (3, self.principal.as_bytes().to_vec()),
            (4, self.candidate_id.as_bytes().to_vec()),
            (5, self.plan_id.as_bytes().to_vec()),
            (6, self.test_object.as_bytes().to_vec()),
            (7, self.test_entity.as_bytes().to_vec()),
            (8, self.target_function.as_bytes().to_vec()),
            (9, self.policy_root.as_bytes().to_vec()),
            (10, declared_record(self.declared_limits)?),
            (11, encode_uvar(self.wall_ms)),
            (12, self.nonce.to_vec()),
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
        if count != 12 {
            return Err(ScbError::new(ScbErrorCode::FieldMissing));
        }
        let mut tags = Vec::with_capacity(12);
        let mut values = Vec::with_capacity(12);
        for _ in 0..12 {
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
        Ok(Self {
            workspace: WorkspaceId::from_bytes(read_id(&values[1])?),
            principal: PrincipalId::from_bytes(read_id(&values[2])?),
            candidate_id: CandidateId::from_bytes(read_id(&values[3])?),
            plan_id: sley_id::NativeTestPlanId::from_bytes(read_id(&values[4])?),
            test_object: ObjectId::from_bytes(read_id(&values[5])?),
            test_entity: EntityId::from_bytes(read_id(&values[6])?),
            target_function: EntityId::from_bytes(read_id(&values[7])?),
            policy_root: PolicyRootId::from_bytes(read_id(&values[8])?),
            declared_limits: parse_declared(&values[9])?,
            wall_ms: read_uvar(&values[10])?,
            nonce: read_nonce(&values[11])?,
        })
    }
}

impl RunResponse {
    /// Encodes one length-delimited response frame.
    ///
    /// # Errors
    ///
    /// Returns `SCB_RESOURCE_LIMIT` for oversized daemon-owned output.
    pub fn encode_frame(&self, output_limit: usize) -> Result<Vec<u8>, ScbError> {
        if self.output.len() > output_limit {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let record = encode_record(&[
            (1, encode_uvar(RUN_VERSION)),
            (2, encode_uvar(u64::from(self.status.tag()))),
            (3, encode_uvar(u64::from(self.code))),
            (4, self.output.clone()),
        ])?;
        let mut frame = RUN_MAGIC.to_vec();
        let len =
            u32::try_from(record.len()).map_err(|_| ScbError::new(ScbErrorCode::ResourceLimit))?;
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(&record);
        Ok(frame)
    }

    /// Strictly decodes one length-delimited response frame.
    ///
    /// # Errors
    ///
    /// Returns the first stable SCB1 refusal for magic, length, shape,
    /// version, or status violations.
    pub fn decode_frame(frame: &[u8], output_limit: usize) -> Result<Self, ScbError> {
        if frame.len() < 12 {
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
        if read_uvar(&values[0])? != RUN_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        let status_tag = u32::try_from(read_uvar(&values[1])?)
            .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
        let status = RunStatus::from_tag(status_tag)
            .ok_or_else(|| ScbError::new(ScbErrorCode::UnionInvalid))?;
        let code = u32::try_from(read_uvar(&values[2])?)
            .map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
        if values[3].len() > output_limit {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(Self {
            status,
            code,
            output: values[3].clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RunRequest {
        RunRequest {
            workspace: WorkspaceId::from_bytes([1; 32]),
            principal: PrincipalId::from_bytes([2; 32]),
            candidate_id: CandidateId::from_bytes([3; 32]),
            plan_id: sley_id::NativeTestPlanId::from_bytes([4; 32]),
            test_object: ObjectId::from_bytes([5; 32]),
            test_entity: EntityId::from_bytes([6; 32]),
            target_function: EntityId::from_bytes([7; 32]),
            policy_root: PolicyRootId::from_bytes([8; 32]),
            declared_limits: NativeDeclaredLimits {
                fuel: 100,
                memory_bytes: 4_096,
                output_bytes: 64,
                effect_count: 0,
                call_depth: 8,
                wall_timeout_millis: 1_000,
            },
            wall_ms: 1_000,
            nonce: [9; 32],
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
    }

    #[test]
    fn response_roundtrips_with_bounded_output() {
        let response = RunResponse {
            status: RunStatus::Complete,
            code: 0,
            output: vec![0xaa; 128],
        };
        let frame = response.encode_frame(256).expect("encodes");
        assert_eq!(
            RunResponse::decode_frame(&frame, 256).expect("decodes"),
            response
        );
        assert_eq!(
            RunResponse::decode_frame(&frame, 64)
                .expect_err("over limit")
                .code(),
            ScbErrorCode::ResourceLimit
        );
        let oversized = RunResponse {
            status: RunStatus::Complete,
            code: 0,
            output: vec![0xaa; 257],
        };
        assert_eq!(
            oversized.encode_frame(256).expect_err("oversized").code(),
            ScbErrorCode::ResourceLimit
        );
        let refused = RunResponse {
            status: RunStatus::Refused,
            code: 7,
            output: Vec::new(),
        };
        let frame = refused.encode_frame(256).expect("encodes");
        assert_eq!(
            RunResponse::decode_frame(&frame, 256).expect("decodes"),
            refused
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

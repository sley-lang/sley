//! Private worker entry and its closed request envelope.
//!
//! The worker is a distinct dynamic-UID process with no repository,
//! supervisor socket, issuer keys, or signing keys. It receives exactly one
//! length-delimited [`WorkerRequest`] frame on stdin (the daemon-owned
//! read-only input binding) and writes exactly one length-delimited
//! [`WorkerReply`] frame on stdout (the daemon-owned bounded channel).
//!
//! Execution dispatch lands with the N5 commit path, which owns
//! plans-to-inputs construction and the portable program artifact. Until
//! then [`dispatch`] strictly decodes and explicitly refuses with
//! [`WorkerRefusal::ExecutionNotWired`]: a well-formed envelope never
//! becomes a silent success.

use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_record, encode_uvar};
use sley_vm::native_execution::{NativeDeclaredLimits, NativeImplementationLimits, profile_id};

fn read_id(value: &[u8]) -> Result<[u8; 32], ScbError> {
    <[u8; 32]>::try_from(value).map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))
}

/// Worker envelope magic.
pub const WORKER_MAGIC: &[u8; 8] = b"SLEYWRK1";
/// Worker envelope version.
pub const WORKER_VERSION: u64 = 1;
/// Maximum worker request frame bytes, including envelope and digest.
pub const MAX_WORKER_FRAME: usize = 262_144;

/// Closed worker request: opaque program artifact plus enforced ceilings.
///
/// `program_bytes` is an N5-owned portable artifact the worker executes
/// without repository access; this crate never interprets it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerRequest {
    /// Opaque N5-owned program artifact bytes.
    pub program_bytes: Vec<u8>,
    /// Ordered canonical input hashes bound by the execution report.
    pub input_hashes: Vec<[u8; 32]>,
    /// Literal `TestCase` limits, without conversions or clamping.
    pub declared_limits: NativeDeclaredLimits,
    /// Separately bound implementation ceilings.
    pub implementation_limits: NativeImplementationLimits,
}

/// Worker refusal with a stable machine tag. Refusals are the only
/// observable outcome until N5 wires execution dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerRefusal {
    /// Malformed envelope or unknown version/profile, with the stable SCB
    /// registry string of the underlying refusal.
    Malformed(&'static str),
    /// Envelope decodes but execution dispatch is not wired yet (N5).
    ExecutionNotWired,
}

impl WorkerRefusal {
    /// Frozen machine tag for logs and probe receipts.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Malformed(_) => 1,
            Self::ExecutionNotWired => 2,
        }
    }
}

impl core::fmt::Display for WorkerRefusal {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Malformed(_) => formatter.write_str("NATIVE_WORKER_MALFORMED_ENVELOPE"),
            Self::ExecutionNotWired => formatter.write_str("NATIVE_WORKER_EXECUTION_NOT_WIRED"),
        }
    }
}

impl std::error::Error for WorkerRefusal {}

fn integer_record(values: &[u64]) -> Result<Vec<u8>, ScbError> {
    let mut fields = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let tag =
            u32::try_from(index + 1).map_err(|_| ScbError::new(ScbErrorCode::IntegerOverflow))?;
        fields.push((tag, encode_uvar(*value)));
    }
    encode_record(&fields)
}

fn parse_declared_record(value: &[u8]) -> Result<[u64; 6], ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_record_field_count()?;
    if count != 6 {
        return Err(ScbError::new(ScbErrorCode::FieldMissing));
    }
    let mut out = [0_u64; 6];
    for (index, slot) in out.iter_mut().enumerate() {
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
    Ok(out)
}

impl WorkerRequest {
    /// Encodes one length-delimited worker frame.
    ///
    /// # Errors
    ///
    /// Returns `SCB_RESOURCE_LIMIT` for oversized artifacts or hash lists.
    pub fn encode_frame(&self) -> Result<Vec<u8>, ScbError> {
        if self.program_bytes.len() > MAX_WORKER_FRAME || self.input_hashes.len() > 65_535 {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        let declared = self.declared_limits;
        let implementation = self.implementation_limits;
        let mut hashes = Vec::with_capacity(self.input_hashes.len());
        for hash in &self.input_hashes {
            hashes.push(hash.to_vec());
        }
        let record = encode_record(&[
            (1, encode_uvar(WORKER_VERSION)),
            (2, profile_id().as_bytes().to_vec()),
            (
                3,
                integer_record(&[
                    declared.fuel,
                    declared.memory_bytes,
                    declared.output_bytes,
                    declared.effect_count,
                    declared.call_depth,
                    declared.wall_timeout_millis,
                ])?,
            ),
            (
                4,
                integer_record(&[
                    implementation.max_instructions,
                    implementation.max_value_units,
                    implementation.max_output_units,
                    implementation.max_call_depth,
                    implementation.max_report_bytes,
                ])?,
            ),
            (5, {
                let mut list = encode_uvar(hashes.len() as u64);
                for hash in &hashes {
                    list.extend_from_slice(hash);
                }
                list
            }),
            (6, self.program_bytes.clone()),
        ])?;
        let mut frame = WORKER_MAGIC.to_vec();
        let len =
            u32::try_from(record.len()).map_err(|_| ScbError::new(ScbErrorCode::ResourceLimit))?;
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(&record);
        if frame.len() > MAX_WORKER_FRAME {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(frame)
    }

    /// Strictly decodes one length-delimited worker frame.
    ///
    /// # Errors
    ///
    /// Returns the first stable SCB1 refusal for magic, length, shape,
    /// version, profile, or bound violations.
    pub fn decode_frame(frame: &[u8]) -> Result<Self, ScbError> {
        if frame.len() < 12 || frame.len() > MAX_WORKER_FRAME {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        if frame[..8] != *WORKER_MAGIC {
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
        if count != 6 {
            return Err(ScbError::new(ScbErrorCode::FieldMissing));
        }
        let mut values = Vec::with_capacity(6);
        for index in 0_u64..6 {
            let tag = cursor.read_uvar(32)?;
            if tag != index + 1 {
                return Err(ScbError::new(ScbErrorCode::FieldOrder));
            }
            values.push(cursor.read_sized_payload()?.to_vec());
        }
        cursor.check_finished()?;
        let mut version = ScbValueCursor::new(&values[0])?;
        if version.read_uvar(64)? != WORKER_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        version.check_finished()?;
        if values[1].as_slice() != profile_id().as_bytes() {
            return Err(ScbError::new(ScbErrorCode::ContractUnknown));
        }
        // The six-slot declared layout decodes here; the five-slot
        // implementation layout has its own hard-maxima parser below.
        let declared = parse_declared_record(&values[2])?;
        let implementation = parse_implementation_record(&values[3])?;
        let hashes = parse_hash_list(&values[4])?;
        if values[5].is_empty() {
            return Err(ScbError::new(ScbErrorCode::FieldMissing));
        }
        Ok(Self {
            program_bytes: values[5].clone(),
            input_hashes: hashes,
            declared_limits: NativeDeclaredLimits {
                fuel: declared[0],
                memory_bytes: declared[1],
                output_bytes: declared[2],
                effect_count: declared[3],
                call_depth: declared[4],
                wall_timeout_millis: declared[5],
            },
            implementation_limits: implementation,
        })
    }
}

fn parse_implementation_record(value: &[u8]) -> Result<NativeImplementationLimits, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_record_field_count()?;
    if count != 5 {
        return Err(ScbError::new(ScbErrorCode::FieldMissing));
    }
    let mut out = [0_u64; 5];
    for (index, slot) in out.iter_mut().enumerate() {
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
    let limits = NativeImplementationLimits {
        max_instructions: out[0],
        max_value_units: out[1],
        max_output_units: out[2],
        max_call_depth: out[3],
        max_report_bytes: out[4],
    };
    if limits.within_hard_maxima() {
        Ok(limits)
    } else {
        Err(ScbError::new(ScbErrorCode::ResourceLimit))
    }
}

fn parse_hash_list(value: &[u8]) -> Result<Vec<[u8; 32]>, ScbError> {
    let mut cursor = ScbValueCursor::new(value)?;
    let count = cursor.read_uvar(64)?;
    if count > 65_535 {
        return Err(ScbError::new(ScbErrorCode::ResourceLimit));
    }
    let mut hashes = Vec::new();
    for _ in 0..count {
        let bytes = cursor.read_exact_bytes(32)?;
        hashes.push(read_id(bytes)?);
    }
    cursor.check_finished()?;
    Ok(hashes)
}

/// Strictly decodes one worker frame and dispatches.
///
/// Execution dispatch lands with N5; today every well-formed envelope
/// refuses with [`WorkerRefusal::ExecutionNotWired`] so no silent success
/// can ever escape the worker boundary.
///
/// # Errors
///
/// Returns `Malformed` for undecodable envelopes and `ExecutionNotWired`
/// for every well-formed envelope until N5 wires dispatch.
pub fn dispatch(frame: &[u8]) -> Result<Vec<u8>, WorkerRefusal> {
    let _request = WorkerRequest::decode_frame(frame)
        .map_err(|error| WorkerRefusal::Malformed(error.code().as_str()))?;
    Err(WorkerRefusal::ExecutionNotWired)
}

/// Private worker entry over standard streams.
///
/// Reads exactly one length-delimited frame from `input`, dispatches, and
/// writes the refusal code to `output` as two big-endian `u32` words
/// (refusal tag, detail code). Returns the process exit code: always
/// nonzero until N5 wires execution.
pub fn run_stdio(input: &mut dyn std::io::Read, output: &mut dyn std::io::Write) -> i32 {
    let mut header = [0_u8; 12];
    if input.read_exact(&mut header).is_err() {
        return write_refusal(output, WorkerRefusal::Malformed("SCB_LENGTH_OVERFLOW"));
    }
    if header[..8] != *WORKER_MAGIC {
        return write_refusal(output, WorkerRefusal::Malformed("SCB_MAGIC_INVALID"));
    }
    let len = u32::from_be_bytes(header[8..12].try_into().unwrap_or([0; 4]));
    if len as usize > MAX_WORKER_FRAME {
        return write_refusal(output, WorkerRefusal::Malformed("SCB_RESOURCE_LIMIT"));
    }
    let mut body = vec![0_u8; len as usize];
    if input.read_exact(&mut body).is_err() {
        return write_refusal(output, WorkerRefusal::Malformed("SCB_LENGTH_OVERFLOW"));
    }
    let mut frame = header.to_vec();
    frame.extend_from_slice(&body);
    match dispatch(&frame) {
        Ok(_) => 0,
        Err(refusal) => write_refusal(output, refusal),
    }
}

fn write_refusal(output: &mut dyn std::io::Write, refusal: WorkerRefusal) -> i32 {
    let detail = match refusal {
        WorkerRefusal::Malformed(code) => code,
        WorkerRefusal::ExecutionNotWired => "NATIVE_WORKER_EXECUTION_NOT_WIRED",
    };
    let mut bytes = refusal.tag().to_be_bytes().to_vec();
    bytes.extend_from_slice(detail.as_bytes());
    if output.write_all(&bytes).is_err() {
        return 3;
    }
    match refusal {
        WorkerRefusal::Malformed(_) => 1,
        WorkerRefusal::ExecutionNotWired => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> WorkerRequest {
        WorkerRequest {
            program_bytes: vec![0x01, 0x02, 0x03],
            input_hashes: vec![[0x11; 32], [0x22; 32]],
            declared_limits: NativeDeclaredLimits {
                fuel: 100,
                memory_bytes: 4_096,
                output_bytes: 64,
                effect_count: 0,
                call_depth: 8,
                wall_timeout_millis: 1_000,
            },
            implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
        }
    }

    #[test]
    fn envelope_roundtrips_and_dispatch_refuses_explicitly() {
        let frame = request().encode_frame().expect("encodes");
        assert_eq!(&frame[..8], WORKER_MAGIC);
        assert_eq!(
            WorkerRequest::decode_frame(&frame).expect("decodes"),
            request()
        );
        // Well-formed but unwired: explicit refusal, never silent success.
        assert_eq!(dispatch(&frame), Err(WorkerRefusal::ExecutionNotWired));
        assert_eq!(WorkerRefusal::ExecutionNotWired.tag(), 2);
    }

    #[test]
    fn envelope_refuses_malformed_frames() {
        let frame = request().encode_frame().expect("encodes");
        let mut bad_magic = frame.clone();
        bad_magic[0] ^= 0x01;
        assert_eq!(
            dispatch(&bad_magic),
            Err(WorkerRefusal::Malformed("SCB_MAGIC_INVALID"))
        );
        assert_eq!(
            dispatch(&frame[..frame.len() - 1]),
            Err(WorkerRefusal::Malformed("SCB_LENGTH_OVERFLOW"))
        );
        assert_eq!(
            dispatch(&[]),
            Err(WorkerRefusal::Malformed("SCB_LENGTH_OVERFLOW"))
        );
        let mut loose = request();
        loose.implementation_limits = NativeImplementationLimits {
            max_call_depth: NativeImplementationLimits::HARD_MAXIMA.max_call_depth + 1,
            ..NativeImplementationLimits::HARD_MAXIMA
        };
        // Over-hard-maxima ceilings encode but refuse at strict decode.
        let loose_frame = loose.encode_frame().expect("encodes");
        assert_eq!(
            dispatch(&loose_frame),
            Err(WorkerRefusal::Malformed("SCB_RESOURCE_LIMIT"))
        );
        assert_eq!(WorkerRefusal::Malformed("SCB_MAGIC_INVALID").tag(), 1);
    }

    #[test]
    fn stdio_entry_reports_refusal_words() {
        let frame = request().encode_frame().expect("encodes");
        let mut input = std::io::Cursor::new(frame);
        let mut output = Vec::new();
        // Well-formed envelope reaches the unwired dispatch refusal.
        assert_eq!(run_stdio(&mut input, &mut output), 2);
        assert_eq!(u32::from_be_bytes(output[..4].try_into().unwrap()), 2);
        assert_eq!(&output[4..], b"NATIVE_WORKER_EXECUTION_NOT_WIRED");
        let mut bad = std::io::Cursor::new(vec![0x00; 4]);
        let mut output = Vec::new();
        assert_eq!(run_stdio(&mut bad, &mut output), 1);
    }
}

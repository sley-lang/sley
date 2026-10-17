//! Private worker entry and its closed request envelope.
//!
//! The worker is a distinct dynamic-UID process with no repository,
//! supervisor socket, issuer keys, or signing keys. Its argv is exactly
//! the transient unit's (`<worker> __native-test-worker --credential`,
//! [`crate::unit::render_transient_unit`]): it reads exactly one
//! length-delimited [`WorkerRequest`] frame from a private systemd credential
//! copied from the daemon-owned staged input. A completed execution
//! writes one canonical `SLEYNEX1` report to stdout (the daemon-owned bounded
//! channel); refusals write one tag and ASCII detail instead. The report is
//! pure VM evidence only, not a host measurement or admission decision.
//! Its exit statuses ([`EXIT_MALFORMED`],
//! [`EXIT_SOURCE_INVALID`], [`EXIT_INPUT_UNREADABLE`], [`EXIT_OUTPUT_FAILED`])
//! are disjoint from the CLI's own statuses 2 through 5, so the launcher
//! can tell a worker refusal from a CLI usage or input failure.

use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_record, encode_uvar};
use sley_vm::native_execution::{NativeDeclaredLimits, NativeImplementationLimits, profile_id};
use std::ffi::OsStr;
use std::fs::File;
use std::os::fd::{AsFd, OwnedFd};
use std::path::Component;

use nix::errno::Errno;
use nix::fcntl::{OFlag, OpenHow, ResolveFlag, openat, openat2};
use nix::sys::stat::Mode;

use crate::{
    config::MAX_WORKER_OUTPUT_BYTES, execution::report_portable_test, program::PortableTestProgram,
};

fn read_id(value: &[u8]) -> Result<[u8; 32], ScbError> {
    <[u8; 32]>::try_from(value).map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))
}

/// Exit status: malformed envelope (refusal tag 1).
pub const EXIT_MALFORMED: i32 = 1;
/// Exit status: envelope decodes but its portable source is invalid (tag 2).
pub const EXIT_SOURCE_INVALID: i32 = 6;
/// Exit status: the input binding could not be opened (refusal tag 3).
pub const EXIT_INPUT_UNREADABLE: i32 = 7;
/// Exit status: the refusal words could not be written to the output.
pub const EXIT_OUTPUT_FAILED: i32 = 8;

/// Worker envelope magic.
pub const WORKER_MAGIC: &[u8; 8] = b"SLEYWRK1";
/// Worker envelope version.
pub const WORKER_VERSION: u64 = 1;
/// Maximum worker request frame bytes, including envelope and digest.
pub const MAX_WORKER_FRAME: usize = 262_144;
/// Fixed systemd credential name for the private worker request.
pub const WORKER_INPUT_CREDENTIAL: &str = "sley-input";
/// Daemon byte permitting execution after it verifies the live unit.
pub const WORKER_START_GATE: u8 = 0xa5;
/// Daemon byte permitting worker exit after it captures live telemetry.
pub const WORKER_RELEASE_GATE: u8 = 0x5a;

/// Closed worker request: opaque program artifact plus enforced ceilings.
///
/// `program_bytes` is a portable artifact the worker will execute without
/// repository access. The program codec and outer-request check are separate
/// from the pure worker dispatch and later measured admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerRequest {
    /// Canonical portable program artifact bytes.
    pub program_bytes: Vec<u8>,
    /// Ordered canonical input hashes bound by the execution report.
    pub input_hashes: Vec<[u8; 32]>,
    /// Literal `TestCase` limits, without conversions or clamping.
    pub declared_limits: NativeDeclaredLimits,
    /// Separately bound implementation ceilings.
    pub implementation_limits: NativeImplementationLimits,
}

/// Worker refusal with a stable machine tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerRefusal {
    /// Malformed envelope or unknown version/profile, with the stable SCB
    /// registry string of the underlying refusal.
    Malformed(&'static str),
    /// Envelope decodes but its bound program or source is invalid.
    SourceInvalid,
    /// The fixed service credential or direct diagnostic input could not be opened.
    InputUnreadable,
}

impl WorkerRefusal {
    /// Frozen machine tag for logs and probe receipts.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Malformed(_) => 1,
            Self::SourceInvalid => 2,
            Self::InputUnreadable => 3,
        }
    }
}

impl core::fmt::Display for WorkerRefusal {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Malformed(_) => formatter.write_str("NATIVE_WORKER_MALFORMED_ENVELOPE"),
            Self::SourceInvalid => formatter.write_str("NATIVE_WORKER_SOURCE_INVALID"),
            Self::InputUnreadable => formatter.write_str("NATIVE_WORKER_INPUT_UNREADABLE"),
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
/// A valid portable program executes through the pure native test owner and
/// returns one canonical bounded `SLEYNEX1` report. This never signs a
/// measurement or admits a test.
///
/// # Errors
///
/// Returns `Malformed` for invalid codecs and `SourceInvalid` for a decoded
/// envelope whose source or bound execution cannot be trusted.
pub fn dispatch(frame: &[u8]) -> Result<Vec<u8>, WorkerRefusal> {
    let request = WorkerRequest::decode_frame(frame)
        .map_err(|error| WorkerRefusal::Malformed(error.code().as_str()))?;
    let program = PortableTestProgram::parse(&request.program_bytes)
        .map_err(|error| WorkerRefusal::Malformed(error.code().as_str()))?;
    let report =
        report_portable_test(&program, &request).map_err(|_| WorkerRefusal::SourceInvalid)?;
    Ok(report.stored_bytes().to_vec())
}

/// Private worker entry over its input binding.
///
/// Opens `input_path` as one regular file without following symlinks or
/// blocking on a FIFO, then runs [`run_stdio`] over it. An absent or
/// non-regular binding refuses with tag 3 and [`EXIT_INPUT_UNREADABLE`].
pub fn run_input_path(input_path: &std::path::Path, output: &mut dyn std::io::Write) -> i32 {
    let Ok(relative) = input_path.strip_prefix("/") else {
        return write_refusal(output, WorkerRefusal::InputUnreadable);
    };
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return write_refusal(output, WorkerRefusal::InputUnreadable);
    }
    let Ok(root) = File::open("/") else {
        return write_refusal(output, WorkerRefusal::InputUnreadable);
    };
    let how = OpenHow::new()
        .flags(OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC)
        .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);
    let opened = match openat2(&root, relative, how) {
        Ok(opened) => Ok(opened),
        Err(Errno::ENOSYS) => open_components_no_symlinks(&root, relative),
        Err(error) => Err(error),
    };
    let Ok(opened) = opened else {
        return write_refusal(output, WorkerRefusal::InputUnreadable);
    };
    let mut file = File::from(opened);
    match file.metadata() {
        Ok(metadata) if metadata.is_file() => run_stdio(&mut file, output),
        _ => write_refusal(output, WorkerRefusal::InputUnreadable),
    }
}

// Some systemd worker sandboxes return ENOSYS for openat2. Walking from a
// pinned root fd with O_NOFOLLOW on every component keeps the same no-symlink,
// no-parent-traversal boundary without loosening the transient unit.
fn open_components_no_symlinks(root: &File, relative: &std::path::Path) -> Result<OwnedFd, Errno> {
    fn walk<Fd: AsFd>(directory: Fd, components: &[&OsStr]) -> Result<OwnedFd, Errno> {
        let (first, rest) = components.split_first().ok_or(Errno::EINVAL)?;
        if rest.is_empty() {
            return openat(
                directory,
                std::path::Path::new(first),
                OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
                Mode::empty(),
            );
        }
        let next = openat(
            directory,
            std::path::Path::new(first),
            OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )?;
        walk(&next, rest)
    }
    let components = relative.iter().collect::<Vec<_>>();
    if components.is_empty()
        || components.len() > 64
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(Errno::EINVAL);
    }
    walk(root.as_fd(), &components)
}

/// Reads the fixed service credential installed by systemd for the worker.
///
/// The environment supplies only the service-owned credential directory;
/// the credential name is fixed here. The daemon sends one start byte on
/// stdin after checking the live unit, then one release byte after receiving
/// the flushed report and capturing telemetry while the cgroup still exists.
/// A missing start refuses before execution; a missing release makes the
/// already-written report non-successful through the worker exit status.
pub fn run_credential_input(
    control: &mut dyn std::io::Read,
    output: &mut dyn std::io::Write,
) -> i32 {
    let Some(directory) = std::env::var_os("CREDENTIALS_DIRECTORY") else {
        return write_refusal(output, WorkerRefusal::InputUnreadable);
    };
    let mut gate = [0_u8; 1];
    if control.read_exact(&mut gate).is_err() || gate[0] != WORKER_START_GATE {
        return write_refusal(output, WorkerRefusal::InputUnreadable);
    }
    let status = run_input_path(
        &std::path::PathBuf::from(directory).join(WORKER_INPUT_CREDENTIAL),
        output,
    );
    if status != 0 {
        return status;
    }
    if output.flush().is_err() {
        return EXIT_OUTPUT_FAILED;
    }
    if control.read_exact(&mut gate).is_err() || gate[0] != WORKER_RELEASE_GATE {
        return EXIT_OUTPUT_FAILED;
    }
    0
}

/// Private worker entry over a byte stream.
///
/// Reads exactly one length-delimited frame from `input` and writes either a
/// bounded canonical execution report or one big-endian refusal tag followed
/// by its ASCII detail. Exit zero means complete worker output, not measured
/// or admitted native test passage.
pub fn run_stdio(input: &mut dyn std::io::Read, output: &mut dyn std::io::Write) -> i32 {
    let mut header = [0_u8; 12];
    if input.read_exact(&mut header).is_err() {
        return write_refusal(output, WorkerRefusal::Malformed("SCB_LENGTH_OVERFLOW"));
    }
    if header[..8] != *WORKER_MAGIC {
        return write_refusal(output, WorkerRefusal::Malformed("SCB_MAGIC_INVALID"));
    }
    let len = u32::from_be_bytes(header[8..12].try_into().unwrap_or([0; 4]));
    if len as usize > MAX_WORKER_FRAME - header.len() {
        return write_refusal(output, WorkerRefusal::Malformed("SCB_RESOURCE_LIMIT"));
    }
    let mut body = vec![0_u8; len as usize];
    if input.read_exact(&mut body).is_err() {
        return write_refusal(output, WorkerRefusal::Malformed("SCB_LENGTH_OVERFLOW"));
    }
    let mut trailing = [0_u8; 1];
    if !matches!(input.read(&mut trailing), Ok(0)) {
        return write_refusal(output, WorkerRefusal::Malformed("SCB_LENGTH_OVERFLOW"));
    }
    let mut frame = header.to_vec();
    frame.extend_from_slice(&body);
    match dispatch(&frame) {
        Ok(report) => {
            if report.len() > MAX_WORKER_OUTPUT_BYTES || output.write_all(&report).is_err() {
                EXIT_OUTPUT_FAILED
            } else {
                0
            }
        }
        Err(refusal) => write_refusal(output, refusal),
    }
}

fn write_refusal(output: &mut dyn std::io::Write, refusal: WorkerRefusal) -> i32 {
    let detail = match refusal {
        WorkerRefusal::Malformed(code) => code,
        WorkerRefusal::SourceInvalid => "NATIVE_WORKER_SOURCE_INVALID",
        WorkerRefusal::InputUnreadable => "NATIVE_WORKER_INPUT_UNREADABLE",
    };
    let mut bytes = refusal.tag().to_be_bytes().to_vec();
    bytes.extend_from_slice(detail.as_bytes());
    if output.write_all(&bytes).is_err() {
        return EXIT_OUTPUT_FAILED;
    }
    match refusal {
        WorkerRefusal::Malformed(_) => EXIT_MALFORMED,
        WorkerRefusal::SourceInvalid => EXIT_SOURCE_INVALID,
        WorkerRefusal::InputUnreadable => EXIT_INPUT_UNREADABLE,
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
    fn envelope_roundtrips_and_invalid_program_refuses_explicitly() {
        let frame = request().encode_frame().expect("encodes");
        assert_eq!(&frame[..8], WORKER_MAGIC);
        assert_eq!(
            WorkerRequest::decode_frame(&frame).expect("decodes"),
            request()
        );
        assert_eq!(
            dispatch(&frame),
            Err(WorkerRefusal::Malformed("SCB_LENGTH_OVERFLOW"))
        );
        assert_eq!(WorkerRefusal::SourceInvalid.tag(), 2);
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
        assert_eq!(run_stdio(&mut input, &mut output), EXIT_MALFORMED);
        assert_eq!(u32::from_be_bytes(output[..4].try_into().unwrap()), 1);
        assert_eq!(&output[4..], b"SCB_LENGTH_OVERFLOW");
        let mut bad = std::io::Cursor::new(vec![0x00; 4]);
        let mut output = Vec::new();
        assert_eq!(run_stdio(&mut bad, &mut output), EXIT_MALFORMED);
    }

    #[test]
    fn stdio_entry_refuses_trailing_bytes_after_one_frame() {
        let mut frame = request().encode_frame().expect("encodes");
        frame.push(0x42);
        let mut output = Vec::new();
        assert_eq!(
            run_stdio(&mut std::io::Cursor::new(frame), &mut output),
            EXIT_MALFORMED
        );
        assert_eq!(u32::from_be_bytes(output[..4].try_into().unwrap()), 1);
        assert_eq!(&output[4..], b"SCB_LENGTH_OVERFLOW");
    }

    #[test]
    fn exit_statuses_are_disjoint_from_the_cli_statuses() {
        // The CLI's own statuses are 0 and 2 through 5 (SLEY_CLI_V1 section 4).
        assert_eq!(
            [
                EXIT_MALFORMED,
                EXIT_SOURCE_INVALID,
                EXIT_INPUT_UNREADABLE,
                EXIT_OUTPUT_FAILED
            ],
            [1, 6, 7, 8]
        );
        for status in [
            EXIT_MALFORMED,
            EXIT_SOURCE_INVALID,
            EXIT_INPUT_UNREADABLE,
            EXIT_OUTPUT_FAILED,
        ] {
            assert!(![0, 2, 3, 4, 5].contains(&status), "{status}");
        }
    }

    #[test]
    fn input_path_entry_reads_the_binding_and_refuses_an_absent_one() {
        use std::os::unix::fs::symlink;

        let frame = request().encode_frame().expect("encodes");
        let dir = std::env::temp_dir().join(format!("sley-worker-input-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("input.bin");
        std::fs::write(&path, &frame).expect("write");
        let root = File::open("/").expect("root");
        assert!(open_components_no_symlinks(&root, path.strip_prefix("/").unwrap()).is_ok());
        let mut output = Vec::new();
        assert_eq!(run_input_path(&path, &mut output), EXIT_MALFORMED);
        assert_eq!(&output[4..], b"SCB_LENGTH_OVERFLOW");
        let mut output = Vec::new();
        assert_eq!(
            run_input_path(&dir.join("absent.bin"), &mut output),
            EXIT_INPUT_UNREADABLE
        );
        assert_eq!(u32::from_be_bytes(output[..4].try_into().unwrap()), 3);
        assert_eq!(&output[4..], b"NATIVE_WORKER_INPUT_UNREADABLE");
        let link = dir.join("linked.bin");
        symlink(&path, &link).expect("create symlink");
        assert!(open_components_no_symlinks(&root, link.strip_prefix("/").unwrap()).is_err());
        let mut output = Vec::new();
        assert_eq!(run_input_path(&link, &mut output), EXIT_INPUT_UNREADABLE);
        assert_eq!(&output[4..], b"NATIVE_WORKER_INPUT_UNREADABLE");
        let linked_directory = dir.join("linked-directory");
        symlink(&dir, &linked_directory).expect("create parent symlink");
        assert!(
            open_components_no_symlinks(
                &root,
                linked_directory
                    .join("input.bin")
                    .strip_prefix("/")
                    .unwrap()
            )
            .is_err()
        );
        let mut output = Vec::new();
        assert_eq!(
            run_input_path(&linked_directory.join("input.bin"), &mut output),
            EXIT_INPUT_UNREADABLE
        );
        assert_eq!(&output[4..], b"NATIVE_WORKER_INPUT_UNREADABLE");
        let mut output = Vec::new();
        assert_eq!(run_input_path(&dir, &mut output), EXIT_INPUT_UNREADABLE);
        assert_eq!(&output[4..], b"NATIVE_WORKER_INPUT_UNREADABLE");
        let fifo = dir.join("pipe.bin");
        nix::unistd::mkfifo(
            &fifo,
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )
        .expect("create FIFO");
        let mut output = Vec::new();
        assert_eq!(run_input_path(&fifo, &mut output), EXIT_INPUT_UNREADABLE);
        assert_eq!(&output[4..], b"NATIVE_WORKER_INPUT_UNREADABLE");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}

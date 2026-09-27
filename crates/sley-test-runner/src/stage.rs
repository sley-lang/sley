//! Daemon-owned, bounded worker-input staging before a privileged launch.
//!
//! The socket request is checked against its complete Sley program before any
//! file is created. A nonce-derived regular file is created exclusively under
//! a daemon-owned private input directory. Holding [`StagedWorkerInput`] keeps
//! that binding alive for the worker; dropping it removes the file. This does
//! not launch a unit, measure execution, or grant native test admission.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use nix::fcntl::{OFlag, OpenHow, ResolveFlag, openat2};

use crate::config::{RunnerConfig, valid_admin_path};
use crate::enforce::EnforceError;
use crate::protocol::RunRequest;
use crate::unit::{TransientUnit, render_transient_unit};

/// Stable failure before any system transient unit starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageError {
    /// The administrator configuration is invalid.
    InvalidConfiguration,
    /// The request's portable Sley program or worker binding is invalid.
    InvalidRequest,
    /// An obvious non-random all-zero attempt nonce was supplied.
    InvalidNonce,
    /// Runtime/input directory owner, mode, type, or symlink resolution fails.
    UnsafeDirectory,
    /// A file already exists for this attempt nonce.
    PathCollision,
    /// The bounded frame could not be durably staged.
    WriteFailure,
    /// Explicit removal of the staged file failed.
    CleanupFailure,
}

impl core::fmt::Display for StageError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let symbol = match self {
            Self::InvalidConfiguration => "NATIVE_STAGE_CONFIG_INVALID",
            Self::InvalidRequest => "NATIVE_STAGE_REQUEST_INVALID",
            Self::InvalidNonce => "NATIVE_STAGE_NONCE_INVALID",
            Self::UnsafeDirectory => "NATIVE_STAGE_DIRECTORY_UNSAFE",
            Self::PathCollision => "NATIVE_STAGE_NONCE_COLLISION",
            Self::WriteFailure => "NATIVE_STAGE_WRITE_FAILED",
            Self::CleanupFailure => "NATIVE_STAGE_CLEANUP_FAILED",
        };
        formatter.write_str(symbol)
    }
}

impl std::error::Error for StageError {}

/// One staged input binding. The daemon must hold this until the worker and
/// all descendants have exited, then call `remove` or let it drop.
#[derive(Debug)]
pub struct StagedWorkerInput {
    path: PathBuf,
    config: RunnerConfig,
    nonce_hex: String,
    requested_memory: u64,
    wall_ms: u64,
    removed: bool,
}

impl StagedWorkerInput {
    /// Exact absolute path to bind read-only into the worker unit.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Renders the exact transient unit for this staged request's path,
    /// attempt nonce, literal memory limit, and supervisor wall budget.
    ///
    /// # Errors
    ///
    /// Refuses invalid administrator configuration or unaccommodating limits.
    pub fn render_unit(&self) -> Result<TransientUnit, EnforceError> {
        let path = self.path.to_str().ok_or(EnforceError::InvalidBudget)?;
        render_transient_unit(
            &self.config,
            &self.nonce_hex,
            path,
            self.requested_memory,
            self.wall_ms,
        )
    }

    /// Removes the staged input and reports a cleanup failure.
    ///
    /// # Errors
    ///
    /// Returns `CleanupFailure` if the file cannot be removed; drop retries.
    pub fn remove(mut self) -> Result<(), StageError> {
        fs::remove_file(&self.path).map_err(|_| StageError::CleanupFailure)?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for StagedWorkerInput {
    fn drop(&mut self) {
        if !self.removed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn directory_metadata(path: &Path, expected_uid: u32) -> Result<fs::Metadata, StageError> {
    let relative = path
        .strip_prefix("/")
        .map_err(|_| StageError::UnsafeDirectory)?;
    let root = File::open("/").map_err(|_| StageError::UnsafeDirectory)?;
    let how = OpenHow::new()
        .flags(OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC)
        .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);
    let opened = openat2(&root, relative, how).map_err(|_| StageError::UnsafeDirectory)?;
    let metadata = File::from(opened)
        .metadata()
        .map_err(|_| StageError::UnsafeDirectory)?;
    if !metadata.is_dir() || metadata.uid() != expected_uid {
        return Err(StageError::UnsafeDirectory);
    }
    Ok(metadata)
}

fn nonce_hex(nonce: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(64);
    for byte in nonce {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}

/// Stages a verified worker frame under the administrator runtime directory.
///
/// Runtime directory ownership by root and symlink-free resolution are
/// checked before a private `input` directory is created. The exact request
/// nonce chooses a single exclusive filename; a duplicate nonce cannot
/// overwrite another attempt's input.
///
/// # Errors
///
/// Refuses invalid request/configuration, unsafe directories, collisions, or
/// incomplete writes. A write failure triggers removal of any partial file.
pub fn stage_worker_input(
    config: &RunnerConfig,
    request: &RunRequest,
) -> Result<StagedWorkerInput, StageError> {
    stage_worker_input_for_uid(config, request, 0)
}

fn stage_worker_input_for_uid(
    config: &RunnerConfig,
    request: &RunRequest,
    expected_uid: u32,
) -> Result<StagedWorkerInput, StageError> {
    config
        .validate()
        .map_err(|_| StageError::InvalidConfiguration)?;
    request
        .verified_program()
        .map_err(|_| StageError::InvalidRequest)?;
    if request.nonce == [0; 32] {
        return Err(StageError::InvalidNonce);
    }
    if !valid_admin_path(&config.runtime_dir) {
        return Err(StageError::InvalidConfiguration);
    }
    let runtime = Path::new(&config.runtime_dir);
    let runtime_metadata = directory_metadata(runtime, expected_uid)?;
    if runtime_metadata.permissions().mode() & 0o022 != 0 {
        return Err(StageError::UnsafeDirectory);
    }
    let input_dir = runtime.join("input");
    match DirBuilder::new().mode(0o700).create(&input_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(StageError::UnsafeDirectory),
    }
    let input_metadata = directory_metadata(&input_dir, expected_uid)?;
    if input_metadata.permissions().mode() & 0o077 != 0 {
        return Err(StageError::UnsafeDirectory);
    }
    let attempt_hex = nonce_hex(request.nonce);
    let path = input_dir.join(format!("{attempt_hex}.bin"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                StageError::PathCollision
            } else {
                StageError::WriteFailure
            }
        })?;
    let result = file
        .metadata()
        .map_err(|_| StageError::WriteFailure)
        .and_then(|metadata| {
            if metadata.is_file()
                && metadata.uid() == expected_uid
                && metadata.permissions().mode().trailing_zeros() >= 6
            {
                Ok(())
            } else {
                Err(StageError::WriteFailure)
            }
        })
        .and_then(|()| {
            file.write_all(&request.worker_frame)
                .map_err(|_| StageError::WriteFailure)
        })
        .and_then(|()| file.sync_all().map_err(|_| StageError::WriteFailure));
    if let Err(error) = result {
        drop(file);
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    drop(file);
    Ok(StagedWorkerInput {
        path,
        config: config.clone(),
        nonce_hex: attempt_hex,
        requested_memory: request.declared_limits.memory_bytes,
        wall_ms: request.wall_ms,
        removed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::config::{AllowedCaller, default_config};
    use crate::program::PortableTestProgram;
    use crate::worker::WorkerRequest;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> (PathBuf, RunnerConfig, RunRequest, u32) {
        let runtime = std::env::temp_dir().join(format!(
            "sley-native-stage-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new()
            .mode(0o700)
            .create(&runtime)
            .expect("runtime");
        let uid = fs::metadata(&runtime).expect("metadata").uid();
        let request = fixture_request();
        let config = default_config(
            runtime.to_str().expect("ASCII runtime path"),
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
        (runtime, config, request, uid)
    }

    #[test]
    fn stages_exact_checked_frame_and_binds_unit_path() {
        let (runtime, config, request, uid) = fixture();
        let staged = stage_worker_input_for_uid(&config, &request, uid).expect("stage");
        assert_eq!(fs::read(staged.path()).expect("read"), request.worker_frame);
        let metadata = fs::symlink_metadata(staged.path()).expect("metadata");
        assert!(metadata.is_file());
        assert_eq!(metadata.uid(), uid);
        assert_eq!(metadata.permissions().mode() & 0o077, 0);
        let unit = staged.render_unit().expect("unit renderer");
        assert_eq!(unit.argv.last().map(String::as_str), staged.path().to_str());
        assert!(matches!(
            stage_worker_input_for_uid(&config, &request, uid),
            Err(StageError::PathCollision)
        ));
        let path = staged.path().to_path_buf();
        staged.remove().expect("remove");
        assert!(!path.exists());
        fs::remove_dir(runtime.join("input")).expect("remove input dir");
        fs::remove_dir(runtime).expect("remove runtime");
    }

    #[test]
    fn refuses_substitution_unsafe_directory_and_zero_nonce() {
        let (runtime, config, mut request, uid) = fixture();
        let mut worker = WorkerRequest::decode_frame(&request.worker_frame).expect("worker");
        worker.input_hashes[0] = [99; 32];
        request.worker_frame = worker.encode_frame().expect("changed worker");
        assert!(matches!(
            stage_worker_input_for_uid(&config, &request, uid),
            Err(StageError::InvalidRequest)
        ));
        assert!(!runtime.join("input").exists());
        request = fixture_request();
        request.nonce = [0; 32];
        assert!(matches!(
            stage_worker_input_for_uid(&config, &request, uid),
            Err(StageError::InvalidNonce)
        ));
        request.nonce = [9; 32];
        fs::create_dir(runtime.join("elsewhere")).expect("other dir");
        std::os::unix::fs::symlink(runtime.join("elsewhere"), runtime.join("input"))
            .expect("symlink");
        assert!(matches!(
            stage_worker_input_for_uid(&config, &request, uid),
            Err(StageError::UnsafeDirectory)
        ));
        fs::remove_file(runtime.join("input")).expect("remove symlink");
        fs::remove_dir(runtime.join("elsewhere")).expect("remove other dir");
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o777)).expect("unsafe mode");
        assert!(matches!(
            stage_worker_input_for_uid(&config, &request, uid),
            Err(StageError::UnsafeDirectory)
        ));
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).expect("restore mode");
        fs::remove_dir(runtime).expect("remove runtime");
    }

    fn fixture_request() -> RunRequest {
        let worker = WorkerRequest::decode_frame(include_bytes!(
            "../../../conformance/native-worker/v1/observed-input.bin"
        ))
        .expect("canonical worker vector");
        let program = PortableTestProgram::parse(&worker.program_bytes).expect("portable program");
        RunRequest::from_portable_program(&program, 1_000, [9; 32]).expect("supervisor request")
    }
}

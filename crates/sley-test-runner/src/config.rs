//! Root daemon local configuration.
//!
//! This is the administrator-owned daemon configuration, distinct from the
//! attested `SupervisorConfigV1` (`SLEYNHC1`) the daemon reports per run.
//! The worker executable digest is computed here with SHA-256 over the exact
//! installed worker binary; paths, PIDs, and runtime unit names never enter
//! attested records.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path};

use nix::fcntl::{OFlag, OpenHow, ResolveFlag, openat2};
use sha2::{Digest, Sha256};
use sley_id::{PrincipalId, WorkspaceId};

use crate::enforce::DEFAULT_PAGE_SIZE;
use crate::worker::MAX_WORKER_FRAME;

/// Fixed transient-unit name prefix; restart reconciliation kills orphans
/// with this prefix and never signs an orphan as a prior success.
pub const UNIT_PREFIX: &str = "sley-native-test-";
/// Authenticated local supervisor socket filename under the service runtime
/// directory.
pub const SOCKET_NAME: &str = "supervisor.sock";
/// Maximum accepted `RunNativeTest` request bytes on the socket. The bound
/// accommodates one complete worker frame plus the fixed outer bindings.
pub const MAX_REQUEST_BYTES: usize = MAX_WORKER_FRAME + 4_096;
/// Maximum daemon-owned worker output bytes per run.
pub const MAX_WORKER_OUTPUT_BYTES: usize = 262_144;
/// Maximum installed binary bytes hashed at daemon startup.
pub const MAX_PINNED_BINARY_BYTES: u64 = 134_217_728;

/// A configured executable cannot be trusted as the pinned installed binary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryPinError {
    /// The preflight was attempted outside the root daemon.
    Unprivileged,
    /// The executable path is nonabsolute, ambiguous, or crosses a symlink.
    UnsafePath,
    /// The path could not be opened or read as a regular file.
    Unavailable,
    /// The executable is not owned by root or is writable by another UID.
    UnsafeMetadata,
    /// The file is empty or exceeds the bounded hash ceiling.
    InvalidSize,
    /// The installed bytes differ from the administrator's pinned digest.
    DigestMismatch,
}

impl core::fmt::Display for BinaryPinError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Unprivileged => "NATIVE_BINARY_PIN_UNPRIVILEGED",
            Self::UnsafePath => "NATIVE_BINARY_PIN_UNSAFE_PATH",
            Self::Unavailable => "NATIVE_BINARY_PIN_UNAVAILABLE",
            Self::UnsafeMetadata => "NATIVE_BINARY_PIN_UNSAFE_METADATA",
            Self::InvalidSize => "NATIVE_BINARY_PIN_INVALID_SIZE",
            Self::DigestMismatch => "NATIVE_BINARY_PIN_DIGEST_MISMATCH",
        })
    }
}

impl std::error::Error for BinaryPinError {}

/// Local daemon configuration error with a stable machine tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    /// A required path, digest, or caller set is missing or malformed.
    InvalidField,
    /// Duplicate UID/workspace/principal grant.
    DuplicateCaller,
}

impl ConfigError {
    /// Frozen machine tag for logs and probe receipts.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::InvalidField => 1,
            Self::DuplicateCaller => 2,
        }
    }
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidField => formatter.write_str("NATIVE_RUNNER_CONFIG_INVALID_FIELD"),
            Self::DuplicateCaller => formatter.write_str("NATIVE_RUNNER_CONFIG_DUPLICATE_CALLER"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// One administrator-authorized caller: peer UID plus the workspace and
/// principal it may run for. Socket peer credentials and the request
/// workspace binding must agree with exactly one entry here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllowedCaller {
    /// Peer UID the manager reports for the socket connection.
    pub uid: u32,
    /// Workspace the caller may request runs for.
    pub workspace: WorkspaceId,
    /// Principal the caller may run as.
    pub principal: PrincipalId,
}

/// Administrator-owned root daemon configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnerConfig {
    /// Directory holding the authenticated socket (service runtime dir).
    pub runtime_dir: String,
    /// Exact installed worker executable path; pinned by digest below.
    pub worker_path: String,
    /// SHA-256 over the exact installed worker binary bytes.
    pub worker_sha256: [u8; 32],
    /// Digest of the running supervisor binary, bound per attestation.
    pub supervisor_sha256: [u8; 32],
    /// Machine page size for cap flooring; defaults to 4096.
    pub page_size: u64,
    /// Authorized UID/workspace/principal grants; exact tuples must be unique.
    pub allowed_callers: Vec<AllowedCaller>,
    /// Measurement-signing key path, root-only and absent from workers.
    pub measurement_key_path: String,
    /// Trust-manifest directory with role-separated historical policies.
    pub trust_manifest_dir: String,
}

impl RunnerConfig {
    /// Validates administrator configuration without touching the filesystem.
    ///
    /// Paths must be absolute; digests must be nonzero; at least one caller
    /// is required and exact grants must be unique; the page size must be a
    /// nonzero power of two.
    ///
    /// # Errors
    ///
    /// Returns the first stable configuration refusal.
    pub fn validate(&self) -> Result<(), ConfigError> {
        for path in [
            &self.runtime_dir,
            &self.worker_path,
            &self.measurement_key_path,
            &self.trust_manifest_dir,
        ] {
            if !valid_admin_path(path) {
                return Err(ConfigError::InvalidField);
            }
        }
        if self.worker_sha256 == [0; 32] || self.supervisor_sha256 == [0; 32] {
            return Err(ConfigError::InvalidField);
        }
        if self.page_size == 0 || !self.page_size.is_power_of_two() {
            return Err(ConfigError::InvalidField);
        }
        if self.allowed_callers.is_empty() || self.allowed_callers.len() > 256 {
            return Err(ConfigError::InvalidField);
        }
        let mut grants = BTreeSet::new();
        for caller in &self.allowed_callers {
            if !grants.insert((
                caller.uid,
                *caller.workspace.as_bytes(),
                *caller.principal.as_bytes(),
            )) {
                return Err(ConfigError::DuplicateCaller);
            }
        }
        Ok(())
    }

    /// Socket path derived from the runtime directory; never caller-supplied.
    #[must_use]
    pub fn socket_path(&self) -> String {
        format!("{}/{}", self.runtime_dir, SOCKET_NAME)
    }

    /// Checks whether the peer UID has any administrator-approved grant.
    #[must_use]
    pub fn has_caller_uid(&self, uid: u32) -> bool {
        self.allowed_callers.iter().any(|caller| caller.uid == uid)
    }

    /// Finds the exact grant for a peer UID and requested scope.
    #[must_use]
    pub fn caller_for_scope(
        &self,
        uid: u32,
        workspace: WorkspaceId,
        principal: PrincipalId,
    ) -> Option<&AllowedCaller> {
        self.allowed_callers.iter().find(|caller| {
            caller.uid == uid && caller.workspace == workspace && caller.principal == principal
        })
    }

    /// Pins the installed worker and running supervisor executables before
    /// any privileged test unit is launched.
    ///
    /// # Errors
    ///
    /// Refuses unprivileged use, unsafe files, missing bytes, or digest
    /// mismatch. The root-owned binaries must remain immutable to nonroot
    /// users between this check and manager exec.
    pub fn verify_installed_binaries(&self) -> Result<(), BinaryPinError> {
        if !nix::unistd::geteuid().is_root() {
            return Err(BinaryPinError::Unprivileged);
        }
        self.validate().map_err(|_| BinaryPinError::UnsafePath)?;
        verify_binary_for_owner(Path::new(&self.worker_path), self.worker_sha256, 0)?;
        let current = std::env::current_exe().map_err(|_| BinaryPinError::Unavailable)?;
        verify_binary_for_owner(&current, self.supervisor_sha256, 0)
    }
}

fn verify_binary_for_owner(
    path: &Path,
    expected_digest: [u8; 32],
    expected_uid: u32,
) -> Result<(), BinaryPinError> {
    let relative = path
        .strip_prefix("/")
        .map_err(|_| BinaryPinError::UnsafePath)?;
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(BinaryPinError::UnsafePath);
    }
    let root = File::open("/").map_err(|_| BinaryPinError::Unavailable)?;
    let how = OpenHow::new()
        .flags(OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC)
        .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);
    let opened = openat2(&root, relative, how).map_err(|_| BinaryPinError::UnsafePath)?;
    let mut file = File::from(opened);
    let metadata = file.metadata().map_err(|_| BinaryPinError::Unavailable)?;
    if !metadata.is_file()
        || metadata.uid() != expected_uid
        || metadata.permissions().mode() & 0o022 != 0
        || metadata.nlink() != 1
    {
        return Err(BinaryPinError::UnsafeMetadata);
    }
    if metadata.len() == 0 || metadata.len() > MAX_PINNED_BINARY_BYTES {
        return Err(BinaryPinError::InvalidSize);
    }
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 8_192];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| BinaryPinError::Unavailable)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or(BinaryPinError::InvalidSize)?;
        if total > MAX_PINNED_BINARY_BYTES {
            return Err(BinaryPinError::InvalidSize);
        }
        hasher.update(&buffer[..count]);
    }
    if total != metadata.len() {
        return Err(BinaryPinError::Unavailable);
    }
    if hasher.finalize().as_slice() != expected_digest {
        return Err(BinaryPinError::DigestMismatch);
    }
    Ok(())
}

/// Restricts administrator paths to unambiguous absolute ASCII components.
/// These paths enter systemd property values as well as filesystem calls;
/// whitespace, escapes, empty components, and dot traversal are refused.
#[must_use]
pub(crate) fn valid_admin_path(path: &str) -> bool {
    path.strip_prefix('/').is_some_and(|rest| {
        !rest.is_empty()
            && rest.split('/').all(|part| {
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                    })
            })
    })
}

/// Computes SHA-256 over exact worker binary bytes for config pinning.
#[must_use]
pub fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Builds a validated config with the platform-default page size.
///
/// # Errors
///
/// Returns the first stable configuration refusal.
pub fn default_config(
    runtime_dir: &str,
    worker_path: &str,
    worker_sha256: [u8; 32],
    supervisor_sha256: [u8; 32],
    allowed_callers: Vec<AllowedCaller>,
    measurement_key_path: &str,
    trust_manifest_dir: &str,
) -> Result<RunnerConfig, ConfigError> {
    let config = RunnerConfig {
        runtime_dir: runtime_dir.to_owned(),
        worker_path: worker_path.to_owned(),
        worker_sha256,
        supervisor_sha256,
        page_size: DEFAULT_PAGE_SIZE,
        allowed_callers,
        measurement_key_path: measurement_key_path.to_owned(),
        trust_manifest_dir: trust_manifest_dir.to_owned(),
    };
    config.validate()?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use sley_id::{PrincipalId, WorkspaceId};

    use super::*;

    static NEXT_BINARY: AtomicU64 = AtomicU64::new(0);

    fn caller(uid: u32) -> AllowedCaller {
        AllowedCaller {
            uid,
            workspace: WorkspaceId::from_bytes([11; 32]),
            principal: PrincipalId::from_bytes([12; 32]),
        }
    }

    fn config() -> RunnerConfig {
        RunnerConfig {
            runtime_dir: "/run/sley-test-supervisor".to_owned(),
            worker_path: "/usr/lib/sley/sley-native-test-worker".to_owned(),
            worker_sha256: [7; 32],
            supervisor_sha256: [8; 32],
            page_size: DEFAULT_PAGE_SIZE,
            allowed_callers: vec![caller(1000)],
            measurement_key_path: "/etc/sley-test-supervisor/measurement.key".to_owned(),
            trust_manifest_dir: "/etc/sley-test-supervisor/trust".to_owned(),
        }
    }

    #[test]
    fn valid_config_passes_and_derives_socket() {
        let config = config();
        assert_eq!(config.validate(), Ok(()));
        assert_eq!(
            config.socket_path(),
            "/run/sley-test-supervisor/supervisor.sock"
        );
        assert!(config.has_caller_uid(1000));
        assert!(!config.has_caller_uid(0));
        assert_eq!(
            config.caller_for_scope(1000, caller(1000).workspace, caller(1000).principal),
            Some(&caller(1000))
        );
    }

    #[test]
    fn one_uid_can_have_distinct_explicit_grants_without_cross_product_authority() {
        let mut config = config();
        config.allowed_callers.push(AllowedCaller {
            uid: 1000,
            workspace: WorkspaceId::from_bytes([13; 32]),
            principal: PrincipalId::from_bytes([0; 32]),
        });
        assert_eq!(config.validate(), Ok(()));
        assert!(
            config
                .caller_for_scope(1000, caller(1000).workspace, caller(1000).principal)
                .is_some()
        );
        assert!(
            config
                .caller_for_scope(
                    1000,
                    config.allowed_callers[1].workspace,
                    config.allowed_callers[1].principal
                )
                .is_some()
        );
        assert!(
            config
                .caller_for_scope(
                    1000,
                    caller(1000).workspace,
                    config.allowed_callers[1].principal
                )
                .is_none()
        );
    }

    #[test]
    fn installed_binary_pin_checks_bytes_owner_mode_and_symlinks() {
        let root = std::env::temp_dir().join(format!(
            "sley-binary-pin-{}-{}",
            std::process::id(),
            NEXT_BINARY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("worker");
        let bytes = b"verified native worker executable";
        std::fs::write(&path, bytes).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let uid = std::fs::metadata(&path).unwrap().uid();
        let digest = sha256_bytes(bytes);
        assert_eq!(verify_binary_for_owner(&path, digest, uid), Ok(()));
        assert_eq!(
            verify_binary_for_owner(&path, [7; 32], uid),
            Err(BinaryPinError::DigestMismatch)
        );
        assert_eq!(
            verify_binary_for_owner(&path, digest, uid.wrapping_add(1)),
            Err(BinaryPinError::UnsafeMetadata)
        );
        let linked = root.join("linked");
        std::os::unix::fs::symlink(&path, &linked).unwrap();
        assert_eq!(
            verify_binary_for_owner(&linked, digest, uid),
            Err(BinaryPinError::UnsafePath)
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(
            verify_binary_for_owner(&path, digest, uid),
            Err(BinaryPinError::UnsafeMetadata)
        );
        std::fs::remove_file(linked).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn config_refusals_keep_stable_tags() {
        assert_eq!(config().validate(), Ok(()));
        let mut relative = config();
        relative.runtime_dir = "run/relative".to_owned();
        assert_eq!(relative.validate(), Err(ConfigError::InvalidField));
        for path in [
            "/run/sley test",
            "/run/sley\n--property=NoNewPrivileges=no",
            "/run/../etc",
            "/run//input",
            "/run/input/",
            "/run/input:other",
        ] {
            let mut unsafe_path = config();
            unsafe_path.worker_path = path.to_owned();
            assert_eq!(
                unsafe_path.validate(),
                Err(ConfigError::InvalidField),
                "{path}"
            );
        }
        let mut zero_digest = config();
        zero_digest.worker_sha256 = [0; 32];
        assert_eq!(zero_digest.validate(), Err(ConfigError::InvalidField));
        let mut bad_page = config();
        bad_page.page_size = 3_000;
        assert_eq!(bad_page.validate(), Err(ConfigError::InvalidField));
        let mut no_callers = config();
        no_callers.allowed_callers.clear();
        assert_eq!(no_callers.validate(), Err(ConfigError::InvalidField));
        let mut too_many = config();
        too_many.allowed_callers = (0..257).map(caller).collect();
        assert_eq!(too_many.validate(), Err(ConfigError::InvalidField));
        let mut duplicate = config();
        duplicate.allowed_callers.push(caller(1000));
        assert_eq!(duplicate.validate(), Err(ConfigError::DuplicateCaller));
        assert_eq!(ConfigError::InvalidField.tag(), 1);
        assert_eq!(ConfigError::DuplicateCaller.tag(), 2);
    }

    #[test]
    fn sha256_pins_exact_worker_bytes() {
        // Independent vector: SHA-256("abc").
        assert_eq!(
            sha256_bytes(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
        assert_ne!(sha256_bytes(b"abc"), sha256_bytes(b"abd"));
    }
}

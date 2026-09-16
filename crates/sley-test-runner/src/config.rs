//! Root daemon local configuration.
//!
//! This is the administrator-owned daemon configuration, distinct from the
//! attested `SupervisorConfigV1` (`SLEYNHC1`) the daemon reports per run.
//! The worker executable digest is computed here with SHA-256 over the exact
//! installed worker binary; paths, PIDs, and runtime unit names never enter
//! attested records.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use sley_id::{PrincipalId, WorkspaceId};

use crate::enforce::DEFAULT_PAGE_SIZE;

/// Fixed transient-unit name prefix; restart reconciliation kills orphans
/// with this prefix and never signs an orphan as a prior success.
pub const UNIT_PREFIX: &str = "sley-native-test-";
/// Authenticated local supervisor socket filename under the service runtime
/// directory.
pub const SOCKET_NAME: &str = "supervisor.sock";
/// Maximum accepted `RunNativeTest` request bytes on the socket.
pub const MAX_REQUEST_BYTES: usize = 65_536;
/// Maximum daemon-owned worker output bytes per run.
pub const MAX_WORKER_OUTPUT_BYTES: usize = 262_144;

/// Local daemon configuration error with a stable machine tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    /// A required path, digest, or caller set is missing or malformed.
    InvalidField,
    /// Duplicate UID or overlapping workspace scope.
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
    /// Authorized callers; UIDs must be unique.
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
    /// is required and UIDs must be unique; the page size must be a nonzero
    /// power of two.
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
            if !path.starts_with('/') || path.is_empty() {
                return Err(ConfigError::InvalidField);
            }
        }
        if self.worker_sha256 == [0; 32] || self.supervisor_sha256 == [0; 32] {
            return Err(ConfigError::InvalidField);
        }
        if self.page_size == 0 || !self.page_size.is_power_of_two() {
            return Err(ConfigError::InvalidField);
        }
        if self.allowed_callers.is_empty() {
            return Err(ConfigError::InvalidField);
        }
        let mut uids = BTreeSet::new();
        for caller in &self.allowed_callers {
            if !uids.insert(caller.uid) {
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

    /// Finds the authorized caller for one peer UID, if any.
    #[must_use]
    pub fn caller_for_uid(&self, uid: u32) -> Option<&AllowedCaller> {
        self.allowed_callers.iter().find(|caller| caller.uid == uid)
    }
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
    use sley_id::{PrincipalId, WorkspaceId};

    use super::*;

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
        assert_eq!(config.caller_for_uid(1000), Some(&caller(1000)));
        assert_eq!(config.caller_for_uid(0), None);
    }

    #[test]
    fn config_refusals_keep_stable_tags() {
        assert_eq!(config().validate(), Ok(()));
        let mut relative = config();
        relative.runtime_dir = "run/relative".to_owned();
        assert_eq!(relative.validate(), Err(ConfigError::InvalidField));
        let mut zero_digest = config();
        zero_digest.worker_sha256 = [0; 32];
        assert_eq!(zero_digest.validate(), Err(ConfigError::InvalidField));
        let mut bad_page = config();
        bad_page.page_size = 3_000;
        assert_eq!(bad_page.validate(), Err(ConfigError::InvalidField));
        let mut no_callers = config();
        no_callers.allowed_callers.clear();
        assert_eq!(no_callers.validate(), Err(ConfigError::InvalidField));
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

//! Root-provisioned current measurement trust for the native supervisor.
//!
//! The daemon reads one fixed manifest name from its administrator-owned
//! directory. The socket request carries no trust bytes or policy selector.
//! Parsed bytes become authority only because the root service chose this
//! protected location; historical manifests remain the receiver's separate
//! storage responsibility.

use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use nix::fcntl::{OFlag, OpenHow, ResolveFlag, openat2};
use sley_scb1::ScbErrorCode;
use sley_tests::{HistoricalTrustPolicyV1, ROLE_MEASUREMENT};

use crate::config::RunnerConfig;
use crate::outcome::{Ed25519MeasurementSigner, Signer, SignerError};

/// Fixed root-provisioned current measurement manifest filename.
pub const MEASUREMENT_MANIFEST_FILE: &str = "measurement.sleyntr1";
/// Bound above the maximum SLEYNTR1 entry/set encoding size.
pub const MAX_MEASUREMENT_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;

/// Failure to load trusted measurement authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustLoadError {
    /// Only the root service may select its configured trust directory.
    Unprivileged,
    /// The configured path or permissions do not protect authority.
    UnsafePath,
    /// The root-provisioned file is absent or unreadable.
    Unavailable,
    /// The file exceeds the bounded trust manifest size.
    TooLarge,
    /// Manifest bytes are malformed or fail their canonical digest.
    InvalidManifest(ScbErrorCode),
}

impl core::fmt::Display for TrustLoadError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Unprivileged => "NATIVE_TRUST_LOAD_UNPRIVILEGED",
            Self::UnsafePath => "NATIVE_TRUST_LOAD_UNSAFE_PATH",
            Self::Unavailable => "NATIVE_TRUST_LOAD_UNAVAILABLE",
            Self::TooLarge => "NATIVE_TRUST_LOAD_TOO_LARGE",
            Self::InvalidManifest(_) => "NATIVE_TRUST_LOAD_INVALID_MANIFEST",
        })
    }
}

impl std::error::Error for TrustLoadError {}

fn load_for_owner(
    config: &RunnerConfig,
    expected_uid: u32,
) -> Result<HistoricalTrustPolicyV1, TrustLoadError> {
    config.validate().map_err(|_| TrustLoadError::UnsafePath)?;
    let relative = Path::new(&config.trust_manifest_dir)
        .strip_prefix("/")
        .map_err(|_| TrustLoadError::UnsafePath)?;
    let root = File::open("/").map_err(|_| TrustLoadError::Unavailable)?;
    let directory = File::from(
        openat2(
            &root,
            relative,
            OpenHow::new()
                .flags(OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC)
                .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS),
        )
        .map_err(|_| TrustLoadError::Unavailable)?,
    );
    let directory_meta = directory
        .metadata()
        .map_err(|_| TrustLoadError::Unavailable)?;
    if !directory_meta.is_dir()
        || directory_meta.uid() != expected_uid
        || directory_meta.permissions().mode() & 0o022 != 0
    {
        return Err(TrustLoadError::UnsafePath);
    }
    let mut file = File::from(
        openat2(
            &directory,
            MEASUREMENT_MANIFEST_FILE,
            OpenHow::new()
                .flags(OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC)
                .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS),
        )
        .map_err(|_| TrustLoadError::Unavailable)?,
    );
    let metadata = file.metadata().map_err(|_| TrustLoadError::Unavailable)?;
    if !metadata.is_file()
        || metadata.uid() != expected_uid
        || metadata.permissions().mode() & 0o022 != 0
        || metadata.nlink() != 1
    {
        return Err(TrustLoadError::UnsafePath);
    }
    if metadata.len() == 0 || metadata.len() > MAX_MEASUREMENT_MANIFEST_BYTES {
        return Err(TrustLoadError::TooLarge);
    }
    let mut stored = Vec::new();
    file.by_ref()
        .take(MAX_MEASUREMENT_MANIFEST_BYTES + 1)
        .read_to_end(&mut stored)
        .map_err(|_| TrustLoadError::Unavailable)?;
    if u64::try_from(stored.len()).map_err(|_| TrustLoadError::TooLarge)? != metadata.len() {
        return Err(TrustLoadError::Unavailable);
    }
    HistoricalTrustPolicyV1::parse(&stored)
        .map_err(|error| TrustLoadError::InvalidManifest(error.code()))
}

/// Loads the current measurement manifest from the fixed protected file.
///
/// This requires a root-owned, non-writable-by-others directory and regular
/// single-link file, both opened without symlinks. The root daemon may select
/// the path only through its administrator configuration. Possession of
/// manifest bytes by a caller never installs trust.
///
/// # Errors
///
/// Refuses nonroot use, unsafe/missing storage, oversized or invalid bytes.
pub fn load_measurement_trust(
    config: &RunnerConfig,
) -> Result<HistoricalTrustPolicyV1, TrustLoadError> {
    if !nix::unistd::geteuid().is_root() {
        return Err(TrustLoadError::Unprivileged);
    }
    load_for_owner(config, 0)
}

/// Provisioned authority loaded before accepting native test work.
///
/// The configuration, current trust manifest, and private signing key are
/// held together so the service cannot use a different configuration after
/// authenticating a peer or launching its unit.
pub struct ProvisionedMeasurementAuthority {
    config: RunnerConfig,
    trust: HistoricalTrustPolicyV1,
    signer: Ed25519MeasurementSigner,
}

/// Failure to establish the root service's measurement authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityLoadError {
    /// The administrator's measurement manifest could not be trusted.
    Trust(TrustLoadError),
    /// The private measurement key could not be loaded.
    Key(SignerError),
    /// The manifest does not grant the loaded key the measurement role.
    KeyNotGranted,
}

impl core::fmt::Display for AuthorityLoadError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Trust(_) => "NATIVE_AUTHORITY_TRUST_UNAVAILABLE",
            Self::Key(_) => "NATIVE_AUTHORITY_KEY_UNAVAILABLE",
            Self::KeyNotGranted => "NATIVE_AUTHORITY_KEY_NOT_GRANTED",
        })
    }
}

impl std::error::Error for AuthorityLoadError {}

impl ProvisionedMeasurementAuthority {
    /// Loads the administrator's immutable service configuration, current
    /// measurement trust, and root-only private key as one startup gate.
    ///
    /// # Errors
    ///
    /// Refuses unavailable or unsafe authority before accepting a worker.
    pub fn load(config: RunnerConfig) -> Result<Self, AuthorityLoadError> {
        let trust = load_measurement_trust(&config).map_err(AuthorityLoadError::Trust)?;
        let signer =
            Ed25519MeasurementSigner::from_key_file(Path::new(&config.measurement_key_path))
                .map_err(AuthorityLoadError::Key)?;
        let key_id = signer.public_key();
        if !trust
            .entries()
            .iter()
            .any(|entry| entry.key_id == key_id && entry.role == ROLE_MEASUREMENT)
        {
            return Err(AuthorityLoadError::KeyNotGranted);
        }
        Ok(Self {
            config,
            trust,
            signer,
        })
    }

    /// Returns the immutable administrator configuration bound to this authority.
    #[must_use]
    pub const fn config(&self) -> &RunnerConfig {
        &self.config
    }

    pub(crate) const fn trust(&self) -> &HistoricalTrustPolicyV1 {
        &self.trust
    }

    pub(crate) fn signer(&self) -> &dyn Signer {
        &self.signer
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        config: RunnerConfig,
        trust: HistoricalTrustPolicyV1,
        signer: Ed25519MeasurementSigner,
    ) -> Self {
        Self {
            config,
            trust,
            signer,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    use sley_id::{PrincipalId, WorkspaceId};
    use sley_tests::{HistoricalTrustPolicyParts, ROLE_MEASUREMENT, TrustEntry};

    use super::*;
    use crate::config::{AllowedCaller, default_config};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn loads_only_canonical_bytes_from_a_protected_directory() {
        let directory = std::env::temp_dir().join(format!(
            "sley-trust-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create trust directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("private dir");
        let policy = HistoricalTrustPolicyV1::build(HistoricalTrustPolicyParts {
            policy_nonce: [4; 32],
            entries: vec![TrustEntry {
                key_id: [3; 32],
                role: ROLE_MEASUREMENT,
                workspaces: vec![[1; 32]],
                profiles: vec![[2; 32]],
                valid_from_unix_millis: 0,
                valid_until_unix_millis: u64::MAX,
            }],
        })
        .expect("canonical policy");
        let file = directory.join(MEASUREMENT_MANIFEST_FILE);
        fs::write(&file, policy.stored_bytes()).expect("write policy");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).expect("private file");
        let config = default_config(
            "/run/sley-test-supervisor",
            "/usr/lib/sley/sley-native-test-worker",
            [7; 32],
            [8; 32],
            vec![AllowedCaller {
                uid: 1_000,
                workspace: WorkspaceId::from_bytes([1; 32]),
                principal: PrincipalId::from_bytes([2; 32]),
            }],
            "/etc/sley-test-supervisor/measurement.key",
            directory.to_str().expect("path"),
        )
        .expect("config");
        let uid = nix::unistd::getuid().as_raw();
        assert_eq!(load_for_owner(&config, uid), Ok(policy.clone()));
        let mut damaged = policy.stored_bytes().to_vec();
        damaged[9] ^= 1;
        fs::write(&file, damaged).expect("damage bytes");
        assert!(matches!(
            load_for_owner(&config, uid),
            Err(TrustLoadError::InvalidManifest(_))
        ));
        fs::remove_file(&file).expect("remove damaged file");
        symlink("/etc/passwd", &file).expect("substitute symlink");
        assert_eq!(
            load_for_owner(&config, uid),
            Err(TrustLoadError::Unavailable)
        );
        fs::remove_file(&file).expect("remove symlink");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o777)).expect("unsafe dir");
        assert_eq!(
            load_for_owner(&config, uid),
            Err(TrustLoadError::UnsafePath)
        );
        fs::remove_dir_all(directory).expect("cleanup fixture");
    }
}

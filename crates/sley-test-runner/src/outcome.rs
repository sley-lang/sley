//! Measurement-admission predicate and the measurement-signer boundary.
//!
//! The daemon signs a measured attestation only when every success fact
//! holds: an execution report exists, termination is `Complete`, worker
//! output is complete, the cgroup reaped empty, all memory events are zero,
//! the peak fits the installed cap within the request, and elapsed stays
//! strictly below the wall budget. Anything else stays diagnostic and can
//! never become a signed pass.
//!
//! Only the root daemon holds the measurement-signing key. Signing itself
//! goes through [`Signer`]; [`Ed25519MeasurementSigner`] provides the strict
//! RFC 8032 implementation over a root-provisioned 32-byte secret file.

use crate::enforce::{EnforceError, check_elapsed, check_memory_evidence};
use ed25519_dalek::{Signer as _, SigningKey};
use nix::fcntl::{OFlag, OpenHow, ResolveFlag, openat2};
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path};
use zeroize::Zeroize;

/// Worker termination classes the daemon can observe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservedTermination {
    /// Worker exited complete with bounded output.
    Complete,
    /// Refused before spawn (zero/overflowing budget, missing enforcer).
    PrelaunchRefused,
    /// Daemon deadline killed the unit.
    Timeout,
    /// Worker killed (signal, OOM-kill, manager kill).
    Killed,
    /// Worker crashed (nonzero exit, malformed output).
    Crash,
    /// Enforcer or telemetry unavailable.
    EnforcerError,
}

impl ObservedTermination {
    /// Frozen attestation termination tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Complete => 1,
            Self::PrelaunchRefused => 2,
            Self::Timeout => 3,
            Self::Killed => 4,
            Self::Crash => 5,
            Self::EnforcerError => 6,
        }
    }
}

/// Complete daemon-observed attempt facts for one run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttemptOutcome {
    /// Whether an execution report ID exists for this attempt.
    pub has_execution_report: bool,
    /// Observed worker termination.
    pub termination: ObservedTermination,
    /// Whether complete bounded worker output arrived.
    pub complete_output: bool,
    /// Whether the cgroup reaped confirmed-empty.
    pub empty_cgroup_confirmed: bool,
    /// Measured peak worker bytes.
    pub measured_peak: u64,
    /// Installed `MemoryMax` cap.
    pub installed_cap: u64,
    /// Requested memory ceiling.
    pub requested_cap: u64,
    /// Recorded `memory.max` events.
    pub max_events: u64,
    /// Recorded OOM events.
    pub oom_events: u64,
    /// Recorded OOM-kill events.
    pub oom_kill_events: u64,
    /// Daemon-measured elapsed nanoseconds.
    pub elapsed_ns: u64,
    /// Supervisor wall budget in milliseconds.
    pub wall_ms: u64,
}

/// Admission refusal with a stable machine tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionRefusal {
    /// No execution report to bind.
    MissingExecutionReport,
    /// Termination is not `Complete`.
    IncompleteTermination(ObservedTermination),
    /// Worker output missing or truncated.
    IncompleteOutput,
    /// Cgroup did not reap confirmed-empty.
    CgroupNotEmpty,
    /// Peak or events violate the memory evidence.
    MemoryEvidence(EnforceError),
    /// Elapsed reached the strict wall deadline.
    Deadline(EnforceError),
}

impl AdmissionRefusal {
    /// Frozen machine tag for logs and probe receipts.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::MissingExecutionReport => 1,
            Self::IncompleteTermination(_) => 2,
            Self::IncompleteOutput => 3,
            Self::CgroupNotEmpty => 4,
            Self::MemoryEvidence(_) => 5,
            Self::Deadline(_) => 6,
        }
    }
}

/// Decides whether an attempt may be signed as a measured pass.
///
/// Every success fact is rechecked here in fixed order; a pass admits
/// exactly the attempts the attestation codec can carry as successful.
///
/// # Errors
///
/// Returns the first admission refusal in fixed order: missing report,
/// incomplete termination, incomplete output, dirty cgroup, memory
/// evidence, then deadline.
pub fn admit_for_signature(outcome: &AttemptOutcome) -> Result<(), AdmissionRefusal> {
    if !outcome.has_execution_report {
        return Err(AdmissionRefusal::MissingExecutionReport);
    }
    if outcome.termination != ObservedTermination::Complete {
        return Err(AdmissionRefusal::IncompleteTermination(outcome.termination));
    }
    if !outcome.complete_output {
        return Err(AdmissionRefusal::IncompleteOutput);
    }
    if !outcome.empty_cgroup_confirmed {
        return Err(AdmissionRefusal::CgroupNotEmpty);
    }
    check_memory_evidence(
        outcome.measured_peak,
        outcome.installed_cap,
        outcome.requested_cap,
        outcome.max_events,
        outcome.oom_events,
        outcome.oom_kill_events,
    )
    .map_err(AdmissionRefusal::MemoryEvidence)?;
    check_elapsed(outcome.elapsed_ns, outcome.wall_ms).map_err(AdmissionRefusal::Deadline)?;
    Ok(())
}

/// Root-daemon measurement signer boundary.
///
/// Implementations hold the root-only measurement key and sign the exact
/// domain-separated attestation preimage from `sley-tests`; they never sign
/// caller-supplied bytes.
pub trait Signer {
    /// Signs one attestation preimage; failures never produce a signature.
    ///
    /// # Errors
    ///
    /// Returns `KeyUnavailable` when the key cannot be read, and
    /// `OperationFailed` when signing itself fails.
    fn sign(&self, preimage: &[u8]) -> Result<[u8; 64], SignerError>;
    /// Raw measurement public key bound as the attestation key ID.
    fn public_key(&self) -> [u8; 32];
}

/// Root-daemon Ed25519 measurement signer.
///
/// The signer accepts exactly one raw 32-byte RFC 8032 secret seed. The
/// underlying key zeroizes secret material on drop. The production file
/// loader requires a root-owned, private, symlink-free regular file.
pub struct Ed25519MeasurementSigner {
    key: SigningKey,
}

impl Ed25519MeasurementSigner {
    /// Constructs a signer from one exact 32-byte secret seed.
    #[must_use]
    pub fn from_secret_bytes(mut secret: [u8; 32]) -> Self {
        let key = SigningKey::from_bytes(&secret);
        secret.zeroize();
        Self { key }
    }

    /// Loads one exact 32-byte secret seed from a provisioned key file.
    ///
    /// # Errors
    ///
    /// Returns `KeyUnavailable` for an unsafe path, owner, mode, file type,
    /// or read failure; `OperationFailed` for a non-32-byte seed.
    pub fn from_key_file(path: &Path) -> Result<Self, SignerError> {
        if !nix::unistd::geteuid().is_root() {
            return Err(SignerError::KeyUnavailable);
        }
        Self::from_key_file_for_owner(path, 0)
    }

    fn from_key_file_for_owner(path: &Path, expected_uid: u32) -> Result<Self, SignerError> {
        let relative = path
            .strip_prefix("/")
            .map_err(|_| SignerError::KeyUnavailable)?;
        if relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(SignerError::KeyUnavailable);
        }
        let root = File::open("/").map_err(|_| SignerError::KeyUnavailable)?;
        let how = OpenHow::new()
            .flags(OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC)
            .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);
        let opened = openat2(&root, relative, how).map_err(|_| SignerError::KeyUnavailable)?;
        let mut file = File::from(opened);
        let metadata = file.metadata().map_err(|_| SignerError::KeyUnavailable)?;
        let mode = metadata.permissions().mode() & 0o7777;
        if !metadata.is_file()
            || metadata.uid() != expected_uid
            || !matches!(mode, 0o400 | 0o600)
            || metadata.nlink() != 1
        {
            return Err(SignerError::KeyUnavailable);
        }
        if metadata.len() != 32 {
            return Err(SignerError::OperationFailed);
        }
        let mut secret = [0_u8; 32];
        if file.read_exact(&mut secret).is_err() {
            secret.zeroize();
            return Err(SignerError::KeyUnavailable);
        }
        let mut trailing = [0_u8; 1];
        if !matches!(file.read(&mut trailing), Ok(0)) {
            secret.zeroize();
            return Err(SignerError::OperationFailed);
        }
        Ok(Self::from_secret_bytes(secret))
    }
}

impl Signer for Ed25519MeasurementSigner {
    fn sign(&self, preimage: &[u8]) -> Result<[u8; 64], SignerError> {
        if preimage.is_empty() {
            return Err(SignerError::OperationFailed);
        }
        Ok(self.key.sign(preimage).to_bytes())
    }

    fn public_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
}

/// Signer failure with a stable machine tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignerError {
    /// Key unavailable (missing, wrong permissions, degraded supervisor).
    KeyUnavailable,
    /// Signing operation failed; no signature bytes exist.
    OperationFailed,
}

impl SignerError {
    /// Frozen machine tag for logs and probe receipts.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::KeyUnavailable => 1,
            Self::OperationFailed => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passing() -> AttemptOutcome {
        AttemptOutcome {
            has_execution_report: true,
            termination: ObservedTermination::Complete,
            complete_output: true,
            empty_cgroup_confirmed: true,
            measured_peak: 8_192,
            installed_cap: 8_192,
            requested_cap: 8_193,
            max_events: 0,
            oom_events: 0,
            oom_kill_events: 0,
            elapsed_ns: 999_999,
            wall_ms: 1_000,
        }
    }

    /// Explicit test double proving that the trait boundary propagates
    /// signer failures independently of the production Ed25519 signer.
    struct TestSigner;

    impl Signer for TestSigner {
        fn sign(&self, preimage: &[u8]) -> Result<[u8; 64], SignerError> {
            if preimage.is_empty() {
                return Err(SignerError::OperationFailed);
            }
            let mut signature = [0x5a; 64];
            signature[..preimage.len().min(64)]
                .copy_from_slice(&preimage[..preimage.len().min(64)]);
            Ok(signature)
        }

        fn public_key(&self) -> [u8; 32] {
            [0xa5; 32]
        }
    }

    #[test]
    fn admission_predicate_passes_only_on_complete_evidence() {
        assert_eq!(admit_for_signature(&passing()), Ok(()));
        let mut missing_report = passing();
        missing_report.has_execution_report = false;
        assert_eq!(
            admit_for_signature(&missing_report),
            Err(AdmissionRefusal::MissingExecutionReport)
        );
        for termination in [
            ObservedTermination::PrelaunchRefused,
            ObservedTermination::Timeout,
            ObservedTermination::Killed,
            ObservedTermination::Crash,
            ObservedTermination::EnforcerError,
        ] {
            let mut attempt = passing();
            attempt.termination = termination;
            assert_eq!(
                admit_for_signature(&attempt),
                Err(AdmissionRefusal::IncompleteTermination(termination))
            );
            assert!(termination.tag() >= 2);
        }
        let mut partial_output = passing();
        partial_output.complete_output = false;
        assert_eq!(
            admit_for_signature(&partial_output),
            Err(AdmissionRefusal::IncompleteOutput)
        );
        let mut dirty_cgroup = passing();
        dirty_cgroup.empty_cgroup_confirmed = false;
        assert_eq!(
            admit_for_signature(&dirty_cgroup),
            Err(AdmissionRefusal::CgroupNotEmpty)
        );
        let mut over_peak = passing();
        over_peak.measured_peak = 8_193;
        assert!(matches!(
            admit_for_signature(&over_peak),
            Err(AdmissionRefusal::MemoryEvidence(_))
        ));
        let mut late = passing();
        late.elapsed_ns = 1_000_000_000;
        assert!(matches!(
            admit_for_signature(&late),
            Err(AdmissionRefusal::Deadline(_))
        ));
        assert_eq!(ObservedTermination::Complete.tag(), 1);
    }

    #[test]
    fn test_double_signs_only_nonempty_preimages() {
        let signer = TestSigner;
        assert_eq!(signer.public_key(), [0xa5; 32]);
        assert!(signer.sign(b"preimage").is_ok());
        assert_eq!(signer.sign(&[]), Err(SignerError::OperationFailed));
        assert_eq!(SignerError::KeyUnavailable.tag(), 1);
        assert_eq!(SignerError::OperationFailed.tag(), 2);
    }

    #[test]
    fn ed25519_measurement_signer_loads_exact_seed_and_signs_strictly() {
        use ed25519_dalek::{Signature, VerifyingKey};

        let path =
            std::env::temp_dir().join(format!("sley-measurement-key-{}", std::process::id()));
        std::fs::write(&path, [0x42; 32]).expect("writes test key");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("private test key");
        let uid = std::fs::metadata(&path).expect("metadata").uid();
        let signer = Ed25519MeasurementSigner::from_key_file_for_owner(&path, uid)
            .expect("loads private test key");
        assert!(matches!(
            Ed25519MeasurementSigner::from_key_file_for_owner(&path, uid.wrapping_add(1)),
            Err(SignerError::KeyUnavailable)
        ));
        if uid != 0 {
            assert!(matches!(
                Ed25519MeasurementSigner::from_key_file(&path),
                Err(SignerError::KeyUnavailable)
            ));
        }

        let preimage = b"sley2 measured attestation preimage";
        let signature = signer.sign(preimage).expect("signs");
        let key = VerifyingKey::from_bytes(&signer.public_key()).expect("public key parses");
        key.verify_strict(preimage, &Signature::from_bytes(&signature))
            .expect("strict verification passes");
        assert_eq!(signer.sign(&[]), Err(SignerError::OperationFailed));

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400))
            .expect("make read-only");
        assert!(Ed25519MeasurementSigner::from_key_file_for_owner(&path, uid).is_ok());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640))
            .expect("make group-readable");
        assert!(matches!(
            Ed25519MeasurementSigner::from_key_file_for_owner(&path, uid),
            Err(SignerError::KeyUnavailable)
        ));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("restore private mode");
        let linked = path.with_extension("link");
        std::os::unix::fs::symlink(&path, &linked).expect("symlink key");
        assert!(matches!(
            Ed25519MeasurementSigner::from_key_file_for_owner(&linked, uid),
            Err(SignerError::KeyUnavailable)
        ));
        std::fs::remove_file(&linked).expect("remove symlink");
        std::fs::hard_link(&path, &linked).expect("hardlink key");
        assert!(matches!(
            Ed25519MeasurementSigner::from_key_file_for_owner(&path, uid),
            Err(SignerError::KeyUnavailable)
        ));
        std::fs::remove_file(&linked).expect("remove hardlink");
        std::fs::write(&path, [0x42; 31]).expect("writes malformed key");
        assert!(matches!(
            Ed25519MeasurementSigner::from_key_file_for_owner(&path, uid),
            Err(SignerError::OperationFailed)
        ));
        std::fs::remove_file(&path).expect("removes malformed key");
    }
}

//! Explicit local receiver authority for the v3 native test route.
//!
//! The endpoint accepts only a user-selected private configuration. It opens
//! the named files without symlinks, checks owner/mode/link/size before
//! reading, and never accepts trust bytes from a protocol request. This only
//! provisions the client side; the root supervisor still needs independent
//! installation and host qualification before a test can be admitted.

use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path};

use nix::fcntl::{OFlag, OpenHow, ResolveFlag, openat2};
use serde::Deserialize;
use sley_test_runner::config::SOCKET_NAME;
use sley_tests::{HistoricalTrustPolicyV1, ROLE_ACCEPTANCE, ROLE_MEASUREMENT};
use sley_txn::{Ed25519AcceptanceSigner, NativeAcceptanceSigner, SocketNativeCommitExecutor};
use zeroize::Zeroize;

use crate::NativeAuthority;

const MAX_CONFIG_BYTES: u64 = 65_536;
const MAX_TRUST_BYTES: u64 = 8 * 1024 * 1024;
const SOCKET_PATH: &str = "/run/sley-test-supervisor/supervisor.sock";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityDocument {
    version: u32,
    acceptance_key_path: String,
    measurement_trust_path: String,
    acceptance_trust_path: String,
}

fn private_file(path: &Path, max_bytes: u64, uid: u32, allow_root: bool) -> Result<Vec<u8>, ()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(());
    }
    let relative = path.strip_prefix("/").map_err(|_| ())?;
    let root = File::open("/").map_err(|_| ())?;
    let mut file = File::from(
        openat2(
            &root,
            relative,
            OpenHow::new()
                .flags(OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC)
                .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS),
        )
        .map_err(|_| ())?,
    );
    let metadata = file.metadata().map_err(|_| ())?;
    let mode = metadata.permissions().mode() & 0o7777;
    if !metadata.is_file()
        || !(metadata.uid() == uid || (allow_root && metadata.uid() == 0))
        || metadata.nlink() != 1
        || if allow_root {
            mode & 0o022 != 0
        } else {
            !matches!(mode, 0o400 | 0o600)
        }
        || metadata.len() == 0
        || metadata.len() > max_bytes
    {
        return Err(());
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if u64::try_from(bytes.len()).map_err(|_| ())? != metadata.len() {
        return Err(());
    }
    Ok(bytes)
}

/// Loads a private configuration and its key/manifests without granting
/// authority from a protocol request or from project metadata.
///
/// # Errors
///
/// Returns a stable `NATIVE_CLI_*` symbol when the configuration, key, or
/// trust manifests cannot be loaded and validated.
pub fn load_native_authority(path: &Path) -> Result<NativeAuthority, &'static str> {
    let uid = nix::unistd::geteuid().as_raw();
    let config_bytes = private_file(path, MAX_CONFIG_BYTES, uid, false)
        .map_err(|()| "NATIVE_CLI_CONFIG_UNAVAILABLE")?;
    let document: AuthorityDocument =
        serde_json::from_slice(&config_bytes).map_err(|_| "NATIVE_CLI_CONFIG_INVALID")?;
    if document.version != 1 {
        return Err("NATIVE_CLI_CONFIG_INVALID");
    }
    let mut key = private_file(Path::new(&document.acceptance_key_path), 32, uid, false)
        .map_err(|()| "NATIVE_CLI_KEY_UNAVAILABLE")?;
    if key.len() != 32 {
        key.zeroize();
        return Err("NATIVE_CLI_KEY_UNAVAILABLE");
    }
    let mut secret = [0_u8; 32];
    secret.copy_from_slice(&key);
    key.zeroize();
    let signer = Ed25519AcceptanceSigner::from_secret_bytes(secret);
    secret.zeroize();
    let measurement_stored = private_file(
        Path::new(&document.measurement_trust_path),
        MAX_TRUST_BYTES,
        uid,
        true,
    )
    .map_err(|()| "NATIVE_CLI_MEASUREMENT_TRUST_UNAVAILABLE")?;
    let acceptance_stored = private_file(
        Path::new(&document.acceptance_trust_path),
        MAX_TRUST_BYTES,
        uid,
        true,
    )
    .map_err(|()| "NATIVE_CLI_ACCEPTANCE_TRUST_UNAVAILABLE")?;
    let measurement_trust = HistoricalTrustPolicyV1::parse(&measurement_stored)
        .map_err(|_| "NATIVE_CLI_MEASUREMENT_TRUST_INVALID")?;
    let acceptance_trust = HistoricalTrustPolicyV1::parse(&acceptance_stored)
        .map_err(|_| "NATIVE_CLI_ACCEPTANCE_TRUST_INVALID")?;
    if !measurement_trust
        .entries()
        .iter()
        .any(|entry| entry.role == ROLE_MEASUREMENT)
    {
        return Err("NATIVE_CLI_MEASUREMENT_TRUST_INVALID");
    }
    if !acceptance_trust
        .entries()
        .iter()
        .any(|entry| entry.role == ROLE_ACCEPTANCE && entry.key_id == signer.key_id())
    {
        return Err("NATIVE_CLI_ACCEPTANCE_KEY_NOT_GRANTED");
    }
    let socket = Path::new(SOCKET_PATH);
    if socket.file_name() != Some(std::ffi::OsStr::new(SOCKET_NAME)) {
        return Err("NATIVE_CLI_SOCKET_INVALID");
    }
    let executor =
        SocketNativeCommitExecutor::new(socket.to_path_buf(), uid, measurement_trust.clone())
            .map_err(|_| "NATIVE_CLI_SOCKET_INVALID")?;
    Ok(NativeAuthority::provision(
        Box::new(executor.clone()),
        Box::new(signer),
        measurement_trust,
        acceptance_trust,
    )
    .with_candidate_diagnostic_executor(Box::new(executor)))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    use sley_tests::{HistoricalTrustPolicyParts, TrustEntry};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        directory: std::path::PathBuf,
        config: std::path::PathBuf,
        key: std::path::PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    fn fixture() -> Fixture {
        let directory = std::env::temp_dir().join(format!(
            "sley-cli-native-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("fixture directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("fixture permissions");
        let config = directory.join("config.json");
        let key = directory.join("acceptance.key");
        let measurement = directory.join("measurement.sleyntr1");
        let acceptance = directory.join("acceptance.sleyntr1");
        let signer = Ed25519AcceptanceSigner::from_secret_bytes([7; 32]);
        let policy = |key_id: [u8; 32], role: u32| {
            HistoricalTrustPolicyV1::build(HistoricalTrustPolicyParts {
                policy_nonce: [u8::try_from(role).expect("fixture role fits in a byte"); 32],
                entries: vec![TrustEntry {
                    key_id,
                    role,
                    workspaces: vec![[4; 32]],
                    profiles: vec![[3; 32]],
                    valid_from_unix_millis: 1,
                    valid_until_unix_millis: u64::MAX,
                }],
            })
            .expect("trust policy")
        };
        fs::write(&key, [7; 32]).expect("key");
        fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).expect("key permissions");
        fs::write(
            &measurement,
            policy([8; 32], ROLE_MEASUREMENT).stored_bytes(),
        )
        .expect("measurement trust");
        fs::write(
            &acceptance,
            policy(signer.key_id(), ROLE_ACCEPTANCE).stored_bytes(),
        )
        .expect("acceptance trust");
        fs::write(
            &config,
            serde_json::to_vec(&serde_json::json!({
                "version": 1,
                "acceptance_key_path": key,
                "measurement_trust_path": measurement,
                "acceptance_trust_path": acceptance,
            }))
            .expect("config JSON"),
        )
        .expect("config");
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600))
            .expect("config permissions");
        Fixture {
            directory,
            config,
            key,
        }
    }

    #[test]
    fn receiver_authority_needs_private_key_and_matching_trust() {
        let fixture = fixture();
        let loaded = load_native_authority(&fixture.config).expect("valid receiver authority");
        assert_eq!(loaded.measurement_trust().entries().len(), 1);
        fs::set_permissions(&fixture.key, fs::Permissions::from_mode(0o644))
            .expect("unsafe key mode");
        assert_eq!(
            load_native_authority(&fixture.config).err(),
            Some("NATIVE_CLI_KEY_UNAVAILABLE")
        );
        fs::set_permissions(&fixture.key, fs::Permissions::from_mode(0o600))
            .expect("restore key permissions");
        fs::write(&fixture.key, [9; 32]).expect("different key");
        assert_eq!(
            load_native_authority(&fixture.config).err(),
            Some("NATIVE_CLI_ACCEPTANCE_KEY_NOT_GRANTED")
        );
    }

    #[test]
    fn receiver_authority_refuses_symlinked_key_and_duplicate_config_field() {
        let fixture = fixture();
        let replacement = fixture.directory.join("key-link");
        symlink(&fixture.key, &replacement).expect("symlink");
        let text = fs::read_to_string(&fixture.config).expect("config text");
        let edited = text.replace(
            fixture.key.to_str().expect("key path"),
            replacement.to_str().expect("link path"),
        );
        fs::write(&fixture.config, edited).expect("config with key link");
        assert_eq!(
            load_native_authority(&fixture.config).err(),
            Some("NATIVE_CLI_KEY_UNAVAILABLE")
        );
        fs::write(&fixture.config, "{\"version\":1,\"version\":1}").expect("duplicate field");
        assert_eq!(
            load_native_authority(&fixture.config).err(),
            Some("NATIVE_CLI_CONFIG_INVALID")
        );
    }
}

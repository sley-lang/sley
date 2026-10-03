//! Closed, root-provisioned supervisor configuration file.
//!
//! The installed daemon reads one fixed JSON file before opening its socket
//! or touching a measurement key. The file is root-owned, private, bounded,
//! and opened without symlinks. Its JSON object rejects missing, duplicate,
//! and unknown fields. Hex identities are exact lower-case 32-byte values.

use std::fs::File;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use nix::fcntl::{OFlag, OpenHow, ResolveFlag, openat2};
use serde::Deserialize;
use sley_id::{PrincipalId, WorkspaceId};

use crate::config::{AllowedCaller, ConfigError, RunnerConfig, valid_admin_path};

/// The only production configuration path for the root service.
pub const ADMIN_CONFIG_PATH: &str = "/etc/sley-test-supervisor/config.json";
/// Upper bound for one administrator configuration file.
pub const MAX_ADMIN_CONFIG_BYTES: u64 = 65_536;

/// Failure before a root service can trust its local configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminConfigError {
    /// Only the root daemon may select its protected configuration.
    Unprivileged,
    /// A path, owner, mode, link count, or file type is unsafe.
    UnsafePath,
    /// The configured file is absent or unreadable.
    Unavailable,
    /// The configuration file is empty or too large.
    InvalidSize,
    /// JSON syntax, duplicate/unknown field, or required type is invalid.
    InvalidJson,
    /// Version or lower-case hex identity is invalid.
    InvalidField,
    /// Parsed runner configuration violates its own constraints.
    Config(ConfigError),
}

impl core::fmt::Display for AdminConfigError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Unprivileged => "NATIVE_ADMIN_CONFIG_UNPRIVILEGED",
            Self::UnsafePath => "NATIVE_ADMIN_CONFIG_UNSAFE_PATH",
            Self::Unavailable => "NATIVE_ADMIN_CONFIG_UNAVAILABLE",
            Self::InvalidSize => "NATIVE_ADMIN_CONFIG_INVALID_SIZE",
            Self::InvalidJson => "NATIVE_ADMIN_CONFIG_INVALID_JSON",
            Self::InvalidField => "NATIVE_ADMIN_CONFIG_INVALID_FIELD",
            Self::Config(_) => "NATIVE_ADMIN_CONFIG_INVALID_RUNNER",
        })
    }
}

impl std::error::Error for AdminConfigError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdminCallerDocument {
    uid: u32,
    workspace: String,
    principal: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdminConfigDocument {
    version: u32,
    runtime_dir: String,
    worker_path: String,
    worker_sha256: String,
    supervisor_sha256: String,
    page_size: u64,
    allowed_callers: Vec<AdminCallerDocument>,
    measurement_key_path: String,
    trust_manifest_dir: String,
}

fn hex_id(value: &str) -> Result<[u8; 32], AdminConfigError> {
    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        return Err(AdminConfigError::InvalidField);
    }
    let nibble = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    };
    let mut parsed = [0_u8; 32];
    for (slot, pair) in parsed.iter_mut().zip(bytes.chunks_exact(2)) {
        let high = nibble(pair[0]).ok_or(AdminConfigError::InvalidField)?;
        let low = nibble(pair[1]).ok_or(AdminConfigError::InvalidField)?;
        *slot = (high << 4) | low;
    }
    Ok(parsed)
}

/// Parses one closed configuration document without installing authority.
///
/// # Errors
///
/// Refuses malformed JSON, duplicate/unknown fields, unsupported version,
/// invalid hex IDs, caller count, or runner configuration.
pub fn parse_admin_config(bytes: &[u8]) -> Result<RunnerConfig, AdminConfigError> {
    if bytes.is_empty()
        || u64::try_from(bytes.len()).map_err(|_| AdminConfigError::InvalidSize)?
            > MAX_ADMIN_CONFIG_BYTES
    {
        return Err(AdminConfigError::InvalidSize);
    }
    let document: AdminConfigDocument =
        serde_json::from_slice(bytes).map_err(|_| AdminConfigError::InvalidJson)?;
    if document.version != 1
        || document.allowed_callers.is_empty()
        || document.allowed_callers.len() > 256
    {
        return Err(AdminConfigError::InvalidField);
    }
    let mut callers = Vec::with_capacity(document.allowed_callers.len());
    for caller in document.allowed_callers {
        callers.push(AllowedCaller {
            uid: caller.uid,
            workspace: WorkspaceId::from_bytes(hex_id(&caller.workspace)?),
            principal: PrincipalId::from_bytes(hex_id(&caller.principal)?),
        });
    }
    let config = RunnerConfig {
        runtime_dir: document.runtime_dir,
        worker_path: document.worker_path,
        worker_sha256: hex_id(&document.worker_sha256)?,
        supervisor_sha256: hex_id(&document.supervisor_sha256)?,
        page_size: document.page_size,
        allowed_callers: callers,
        measurement_key_path: document.measurement_key_path,
        trust_manifest_dir: document.trust_manifest_dir,
    };
    config.validate().map_err(AdminConfigError::Config)?;
    Ok(config)
}

fn load_for_owner(path: &Path, expected_uid: u32) -> Result<RunnerConfig, AdminConfigError> {
    let text = path.to_str().ok_or(AdminConfigError::UnsafePath)?;
    if !valid_admin_path(text) {
        return Err(AdminConfigError::UnsafePath);
    }
    let relative = path
        .strip_prefix("/")
        .map_err(|_| AdminConfigError::UnsafePath)?;
    let root = File::open("/").map_err(|_| AdminConfigError::Unavailable)?;
    let mut file = File::from(
        openat2(
            &root,
            relative,
            OpenHow::new()
                .flags(OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC)
                .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS),
        )
        .map_err(|_| AdminConfigError::Unavailable)?,
    );
    let metadata = file.metadata().map_err(|_| AdminConfigError::Unavailable)?;
    let mode = metadata.permissions().mode() & 0o7777;
    if !metadata.is_file()
        || metadata.uid() != expected_uid
        || !matches!(mode, 0o400 | 0o600)
        || metadata.nlink() != 1
    {
        return Err(AdminConfigError::UnsafePath);
    }
    if metadata.len() == 0 || metadata.len() > MAX_ADMIN_CONFIG_BYTES {
        return Err(AdminConfigError::InvalidSize);
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_ADMIN_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AdminConfigError::Unavailable)?;
    if u64::try_from(bytes.len()).map_err(|_| AdminConfigError::InvalidSize)? != metadata.len() {
        return Err(AdminConfigError::Unavailable);
    }
    parse_admin_config(&bytes)
}

/// Loads the root service's fixed administrator configuration file.
///
/// # Errors
///
/// Refuses nonroot use, unsafe storage, malformed bytes, or invalid fields.
pub fn load_admin_config() -> Result<RunnerConfig, AdminConfigError> {
    if !nix::unistd::geteuid().is_root() {
        return Err(AdminConfigError::Unprivileged);
    }
    load_for_owner(Path::new(ADMIN_CONFIG_PATH), 0)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn valid_document() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "version": 1,
            "runtime_dir": "/run/sley-test-supervisor",
            "worker_path": "/usr/lib/sley/sley-native-test-worker",
            "worker_sha256": "07".repeat(32),
            "supervisor_sha256": "08".repeat(32),
            "page_size": 4096,
            "allowed_callers": [{
                "uid": 1000,
                "workspace": "01".repeat(32),
                "principal": "02".repeat(32)
            }],
            "measurement_key_path": "/etc/sley-test-supervisor/measurement.key",
            "trust_manifest_dir": "/etc/sley-test-supervisor/trust"
        }))
        .expect("document")
    }

    #[test]
    fn closed_config_rejects_duplicate_unknown_and_uppercase_identity_fields() {
        let parsed = parse_admin_config(&valid_document()).expect("valid config");
        assert_eq!(parsed.worker_sha256, [7; 32]);
        let duplicated = String::from_utf8(valid_document()).expect("utf8").replacen(
            "\"version\":1",
            "\"version\":1,\"version\":1",
            1,
        );
        assert_eq!(
            parse_admin_config(duplicated.as_bytes()),
            Err(AdminConfigError::InvalidJson)
        );
        let unknown = String::from_utf8(valid_document()).expect("utf8").replacen(
            "\"version\":1",
            "\"version\":1,\"command\":\"sh\"",
            1,
        );
        assert_eq!(
            parse_admin_config(unknown.as_bytes()),
            Err(AdminConfigError::InvalidJson)
        );
        let uppercase = String::from_utf8(valid_document()).expect("utf8").replacen(
            &"07".repeat(32),
            &"0A".repeat(32),
            1,
        );
        assert_eq!(
            parse_admin_config(uppercase.as_bytes()),
            Err(AdminConfigError::InvalidField)
        );
    }

    #[test]
    fn protected_document_accepts_distinct_grants_for_one_uid_only() {
        let mut document: serde_json::Value =
            serde_json::from_slice(&valid_document()).expect("fixture document");
        let callers = document["allowed_callers"].as_array_mut().expect("callers");
        callers.push(serde_json::json!({
            "uid": 1000,
            "workspace": "01".repeat(32),
            "principal": "00".repeat(32)
        }));
        let distinct = serde_json::to_vec(&document).expect("distinct document");
        let parsed = parse_admin_config(&distinct).expect("two distinct grants");
        assert_eq!(parsed.allowed_callers.len(), 2);
        document["allowed_callers"][1]["principal"] = serde_json::json!("02".repeat(32));
        assert_eq!(
            parse_admin_config(&serde_json::to_vec(&document).expect("duplicate document")),
            Err(AdminConfigError::Config(ConfigError::DuplicateCaller))
        );
    }

    #[test]
    fn protected_file_refuses_symlink_and_unsafe_mode() {
        let directory = std::env::temp_dir().join(format!(
            "sley-admin-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).expect("create fixture directory");
        let path = directory.join("config.json");
        fs::write(&path, valid_document()).expect("write config");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("private file");
        let uid = nix::unistd::getuid().as_raw();
        assert!(load_for_owner(&path, uid).is_ok());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("unsafe file");
        assert_eq!(
            load_for_owner(&path, uid),
            Err(AdminConfigError::UnsafePath)
        );
        fs::remove_file(&path).expect("remove file");
        symlink("/etc/passwd", &path).expect("symlink");
        assert_eq!(
            load_for_owner(&path, uid),
            Err(AdminConfigError::Unavailable)
        );
        fs::remove_dir_all(directory).expect("cleanup fixture");
    }
}

//! Prepare reviewable, private F0 authority files without installing a service.
//!
//! Usage: `prepare_authority STAGE_MANIFEST STATE CANDIDATE UID PAGE_SIZE MEASUREMENT_PUBLIC_KEY OUT_DIR`.
//! `OUT_DIR` must not exist. Copying any output into administrator paths is a
//! separate host-installation action; this example never does it. The root
//! administrator creates and retains the measurement seed elsewhere.

use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::SigningKey;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sley_mutate::import_candidate;
use sley_policy::fixed_native_admission_profile;
use sley_test_runner::admin_config::parse_admin_config;
use sley_test_runner::unit::configured_supervisor_profile;
use sley_tests::{
    HistoricalTrustPolicyParts, HistoricalTrustPolicyV1, ROLE_ACCEPTANCE, ROLE_MEASUREMENT,
    TrustEntry, native_execution_profile_id,
};
use zeroize::Zeroize;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str, io::Error> {
    value[name]
        .as_str()
        .ok_or_else(|| invalid("missing text field"))
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, io::Error> {
    if !hex.len().is_multiple_of(2) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid("invalid hex bytes"));
    }
    let mut bytes = vec![0_u8; hex.len() / 2];
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| invalid("invalid identity byte"))?;
    }
    Ok(bytes)
}

fn id(hex: &str) -> Result<[u8; 32], io::Error> {
    decode_hex(hex)?
        .try_into()
        .map_err(|_| invalid("invalid 32-byte identity"))
}

fn sha256(bytes: &[u8]) -> String {
    hex_bytes(Sha256::digest(bytes).as_ref())
}

fn hex_bytes(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    result
}

fn staged_digest(
    manifest: &Value,
    stage_dir: &Path,
    install_path: &str,
) -> Result<String, io::Error> {
    let entry = manifest["files"]
        .as_array()
        .ok_or_else(|| invalid("stage file list missing"))?
        .iter()
        .find(|entry| entry["install_path"].as_str() == Some(install_path))
        .ok_or_else(|| invalid("required staged binary missing"))?;
    let declared = field(entry, "sha256")?;
    id(declared)?;
    let staged = stage_dir.join(install_path.trim_start_matches('/'));
    let actual = sha256(&fs::read(staged)?);
    if actual != declared {
        return Err(invalid("staged binary digest mismatch"));
    }
    Ok(actual)
}

fn random_seed() -> Result<[u8; 32], io::Error> {
    let mut bytes = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn private_file(path: &Path, bytes: &[u8]) -> Result<(), io::Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn manifest(
    key: [u8; 32],
    role: u32,
    workspace: [u8; 32],
    profile: [u8; 32],
    valid_from: u64,
    valid_until: u64,
) -> Result<HistoricalTrustPolicyV1, Box<dyn Error>> {
    let policy = HistoricalTrustPolicyV1::build(HistoricalTrustPolicyParts {
        policy_nonce: random_seed()?,
        entries: vec![TrustEntry {
            key_id: key,
            role,
            workspaces: vec![workspace],
            profiles: vec![profile],
            valid_from_unix_millis: valid_from,
            valid_until_unix_millis: valid_until,
        }],
    })?;
    Ok(policy)
}

// Keep the one-time authority construction in one ordered, auditable flow.
#[allow(clippy::too_many_lines)]
fn run() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.len() != 8 {
        return Err(invalid(
            "usage: prepare_authority STAGE_MANIFEST STATE CANDIDATE UID PAGE_SIZE MEASUREMENT_PUBLIC_KEY OUT_DIR",
        )
        .into());
    }
    let stage_path = PathBuf::from(&args[1]).canonicalize()?;
    let stage_dir = stage_path
        .parent()
        .ok_or_else(|| invalid("stage directory missing"))?;
    let build_manifest_bytes = fs::read(&stage_path)?;
    let build_manifest: Value = serde_json::from_slice(&build_manifest_bytes)?;
    if field(&build_manifest, "source_commit")?.len() != 40
        || build_manifest["source_clean"] != true
        || build_manifest["root_service_installed"] != false
    {
        return Err(invalid("stage does not describe a clean uninstalled build").into());
    }
    let worker_digest = staged_digest(&build_manifest, stage_dir, "/usr/lib/sley/sley")?;
    let supervisor_digest = staged_digest(
        &build_manifest,
        stage_dir,
        "/usr/lib/sley/sley-test-supervisor",
    )?;
    let workspace_bytes = fs::read(&args[2])?;
    let workspace_data: Value = serde_json::from_slice(&workspace_bytes)?;
    let workspace = field(&workspace_data, "workspace_id")?;
    let principal = field(&workspace_data, "principal_id")?;
    let workspace_id = id(workspace)?;
    let principal_id = id(principal)?;
    if principal_id == [0; 32] {
        return Err(
            invalid("commit principal must differ from the zero diagnostic principal").into(),
        );
    }
    let candidate_bytes = fs::read(&args[3])?;
    let candidate: Value = serde_json::from_slice(&candidate_bytes)?;
    if candidate["native_plan_selected_test_count"] != 1
        || id(field(&candidate, "native_execution_profile_id")?)?
            != *native_execution_profile_id().as_bytes()
    {
        return Err(
            invalid("fixture has no single selected native test under this core profile").into(),
        );
    }
    let imported = import_candidate(&decode_hex(field(&candidate, "candidate_stored_hex")?)?)?;
    if imported.candidate_id.as_bytes() != &id(field(&candidate, "candidate_id")?)?
        || imported.record.workspace_id.as_bytes() != &workspace_id
        || imported.record.principal_id.as_bytes() != &principal_id
        || imported.record.base_transaction_id.as_bytes()
            != &id(field(&workspace_data, "transaction_id")?)?
        || imported.record.base_root.as_bytes() != &id(field(&workspace_data, "state_root")?)?
        || imported.record.policy_root_id.as_bytes() != &id(field(&workspace_data, "policy_root")?)?
        || imported.record.schema_epoch_id.as_bytes()
            != &id(field(&workspace_data, "schema_epoch")?)?
        || field(&candidate, "target_function_id")? != field(&workspace_data, "function_id")?
    {
        return Err(invalid("candidate does not match the accepted fixture scope").into());
    }
    let limits = &candidate["declared_limits"];
    let memory_bytes = limits["memory_bytes"]
        .as_u64()
        .ok_or_else(|| invalid("test memory missing"))?;
    let wall_ms = limits["wall_timeout_millis"]
        .as_u64()
        .ok_or_else(|| invalid("test wall bound missing"))?;
    let uid = args[4].to_string_lossy().parse::<u32>()?;
    let page_size = args[5].to_string_lossy().parse::<u64>()?;
    let measurement_key = id(&args[6].to_string_lossy())?;
    ed25519_dalek::VerifyingKey::from_bytes(&measurement_key)
        .map_err(|_| invalid("measurement public key is not a valid Ed25519 point"))?;
    let out = PathBuf::from(&args[7]);
    if !out.is_absolute() || out.exists() {
        return Err(invalid("output directory must be a new absolute path").into());
    }
    let root_config = json!({
        "version": 1,
        "runtime_dir": "/run/sley-test-supervisor",
        "worker_path": "/usr/lib/sley/sley",
        "worker_sha256": worker_digest,
        "supervisor_sha256": supervisor_digest,
        "page_size": page_size,
        "allowed_callers": [
            {"uid": uid, "workspace": workspace, "principal": principal},
            {"uid": uid, "workspace": workspace, "principal": "00".repeat(32)}
        ],
        "measurement_key_path": "/etc/sley-test-supervisor/measurement.key",
        "trust_manifest_dir": "/etc/sley-test-supervisor/trust"
    });
    let root_config_bytes = serde_json::to_vec_pretty(&root_config)?;
    let parsed_config = parse_admin_config(&root_config_bytes)?;
    let supervisor = configured_supervisor_profile(&parsed_config, memory_bytes, wall_ms)?;
    let admission = fixed_native_admission_profile()?;
    let now = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let valid_from = now.saturating_sub(60_000);
    let valid_until = now
        .checked_add(7 * 24 * 60 * 60 * 1000)
        .ok_or_else(|| invalid("time overflow"))?;
    let mut acceptance_seed = random_seed()?;
    let acceptance_key = *SigningKey::from_bytes(&acceptance_seed)
        .verifying_key()
        .as_bytes();
    if acceptance_key == measurement_key {
        acceptance_seed.zeroize();
        return Err(invalid("acceptance and measurement keys must be distinct").into());
    }
    let measurement = manifest(
        measurement_key,
        ROLE_MEASUREMENT,
        workspace_id,
        *supervisor.id().as_bytes(),
        valid_from,
        valid_until,
    )?;
    let acceptance = manifest(
        acceptance_key,
        ROLE_ACCEPTANCE,
        workspace_id,
        *admission.id().as_bytes(),
        valid_from,
        valid_until,
    )?;
    fs::create_dir(&out)?;
    fs::set_permissions(&out, fs::Permissions::from_mode(0o700))?;
    private_file(&out.join("root-config.json"), &root_config_bytes)?;
    private_file(&out.join("acceptance.key"), &acceptance_seed)?;
    acceptance_seed.zeroize();
    private_file(
        &out.join("measurement.sleyntr1"),
        measurement.stored_bytes(),
    )?;
    private_file(&out.join("acceptance.sleyntr1"), acceptance.stored_bytes())?;
    let receiver_config = json!({
        "version": 1,
        "acceptance_key_path": out.join("acceptance.key"),
        "measurement_trust_path": out.join("measurement.sleyntr1"),
        "acceptance_trust_path": out.join("acceptance.sleyntr1")
    });
    private_file(
        &out.join("receiver-config.json"),
        &serde_json::to_vec_pretty(&receiver_config)?,
    )?;
    let plan = json!({
        "contract": "sley.native-f0-authority-plan.v0",
        "state": "DRAFT_AWAITING_ROOT_MEASUREMENT_KEY_INSTALLATION",
        "stage_manifest_sha256": sha256(&build_manifest_bytes),
        "state_sha256": sha256(&workspace_bytes),
        "candidate_sha256": sha256(&candidate_bytes),
        "source_commit": field(&build_manifest, "source_commit")?,
        "workspace_id": workspace,
        "commit_principal_id": principal,
        "diagnostic_principal_id": "00".repeat(32),
        "receiver_uid": uid,
        "page_size": page_size,
        "requested_memory_bytes": memory_bytes,
        "wall_ms": wall_ms,
        "supervisor_config_id": hex_bytes(supervisor.id().as_bytes()),
        "native_admission_profile_id": hex_bytes(admission.id().as_bytes()),
        "measurement_key_id": hex_bytes(&measurement_key),
        "acceptance_key_id": hex_bytes(&acceptance_key),
        "measurement_manifest_id": hex_bytes(measurement.id().as_bytes()),
        "acceptance_manifest_id": hex_bytes(acceptance.id().as_bytes()),
        "valid_from_unix_millis": valid_from,
        "valid_until_unix_millis": valid_until,
        "files": ["root-config.json", "receiver-config.json", "acceptance.key", "measurement.sleyntr1", "acceptance.sleyntr1"]
    });
    private_file(&out.join("plan.json"), &serde_json::to_vec_pretty(&plan)?)?;
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("authority preparation failed: {error}");
        std::process::exit(1);
    }
}

//! Typed systemd manager verification for a gated native-test worker.
//!
//! `systemctl show` formats durations and capability sets for display. The
//! supervisor instead reads the manager's typed D-Bus properties and checks
//! the installed unit before sending the worker's stdin start byte. This
//! module only verifies an existing unit; it cannot launch or attest one.

use std::collections::BTreeMap;
use std::io::{ErrorKind, Read};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::config::{RunnerConfig, UNIT_PREFIX};
use crate::unit::TransientUnit;
use crate::worker::WORKER_INPUT_CREDENTIAL;

const SYSTEMD_BUS: &str = "org.freedesktop.systemd1";
const SYSTEMD_MANAGER: &str = "/org/freedesktop/systemd1";
const MAX_BUS_REPLY_BYTES: usize = 131_072;

/// Refusal to trust the manager's installed transient-unit state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagerError {
    /// The manager or typed D-Bus response could not be read.
    Unavailable,
    /// A required typed property was missing, malformed, or different.
    PropertyMismatch,
}

impl core::fmt::Display for ManagerError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "NATIVE_MANAGER_UNAVAILABLE",
            Self::PropertyMismatch => "NATIVE_MANAGER_PROPERTY_MISMATCH",
        })
    }
}

impl std::error::Error for ManagerError {}

/// Manager facts needed to pin live cgroup telemetry to the worker process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledWorker {
    /// Exact cgroup path reported by the manager for this transient service.
    pub control_group: String,
    /// Main process ID reported by the manager before worker release.
    pub main_pid: u32,
}

/// Reads and verifies one live transient unit from the system manager.
///
/// This must be called after `systemd-run` has installed and started the
/// worker, but while that worker is still blocked on its stdin start gate.
/// The caller must also verify live cgroup limits and process membership.
///
/// # Errors
///
/// Refuses unavailable manager state and every mismatched installed setting.
pub fn verify_system_unit(
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
) -> Result<InstalledWorker, ManagerError> {
    let service_name = format!("{}.service", unit.unit_name);
    let object = busctl(&[
        "call",
        SYSTEMD_BUS,
        SYSTEMD_MANAGER,
        "org.freedesktop.systemd1.Manager",
        "GetUnit",
        "s",
        &service_name,
    ])?;
    let path = object_path(&object)?;
    let unit_properties = get_all(path, "org.freedesktop.systemd1.Unit")?;
    let service_properties = get_all(path, "org.freedesktop.systemd1.Service")?;
    verify_snapshot(
        &unit_properties,
        &service_properties,
        unit,
        config,
        worker_input_path,
    )
}

/// Confirms that the manager has no live process or control group for this
/// unit and its system.slice cgroup path is absent. A failed unit may remain
/// loaded after SIGKILL; that is terminal only with `MainPID=0` and an empty
/// manager `ControlGroup`.
///
/// # Errors
///
/// Refuses malformed names, manager failures, or unreadable cgroup state.
pub fn confirm_system_unit_reaped(unit_name: &str) -> Result<bool, ManagerError> {
    let nonce = unit_name
        .strip_prefix(UNIT_PREFIX)
        .ok_or(ManagerError::PropertyMismatch)?;
    if nonce.len() != 64
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ManagerError::PropertyMismatch);
    }
    let service_name = format!("{unit_name}.service");
    let listing = busctl(&[
        "call",
        SYSTEMD_BUS,
        SYSTEMD_MANAGER,
        "org.freedesktop.systemd1.Manager",
        "ListUnitsByPatterns",
        "asas",
        "0",
        "1",
        &service_name,
    ])?;
    let entries = listed_units(&listing)?;
    if entries.len() > 1 {
        return Err(ManagerError::PropertyMismatch);
    }
    if let Some(entry) = entries.first() {
        let fields = entry.as_array().ok_or(ManagerError::Unavailable)?;
        if fields.len() != 10 || fields[0].as_str() != Some(&service_name) {
            return Err(ManagerError::PropertyMismatch);
        }
        let object = busctl(&[
            "call",
            SYSTEMD_BUS,
            SYSTEMD_MANAGER,
            "org.freedesktop.systemd1.Manager",
            "GetUnit",
            "s",
            &service_name,
        ])?;
        let path = object_path(&object)?;
        let unit = get_all(path, "org.freedesktop.systemd1.Unit")?;
        let service = get_all(path, "org.freedesktop.systemd1.Service")?;
        check(&unit, "Id", "s", &Value::from(service_name.as_str()))?;
        let state = property(&unit, "ActiveState", "s")?
            .as_str()
            .ok_or(ManagerError::PropertyMismatch)?;
        if !matches!(state, "inactive" | "failed") {
            return Ok(false);
        }
        check(&service, "MainPID", "u", &Value::from(0))?;
        check(&service, "ControlGroup", "s", &Value::from(""))?;
    }
    let system_slice = Path::new("/sys/fs/cgroup/system.slice");
    let parent = std::fs::symlink_metadata(system_slice).map_err(|_| ManagerError::Unavailable)?;
    if !parent.is_dir() || parent.uid() != 0 {
        return Err(ManagerError::Unavailable);
    }
    let cgroup_path = system_slice.join(service_name);
    match std::fs::symlink_metadata(cgroup_path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Ok(_) => Ok(false),
        Err(_) => Err(ManagerError::Unavailable),
    }
}

fn listed_units(value: &Value) -> Result<&[Value], ManagerError> {
    if value.get("type").and_then(Value::as_str) != Some("a(ssssssouso)") {
        return Err(ManagerError::Unavailable);
    }
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or(ManagerError::Unavailable)?;
    if data.len() != 1 {
        return Err(ManagerError::Unavailable);
    }
    data[0]
        .as_array()
        .map(Vec::as_slice)
        .ok_or(ManagerError::Unavailable)
}

fn busctl(args: &[&str]) -> Result<Value, ManagerError> {
    let mut child = Command::new("/usr/bin/busctl")
        .args(["--system", "--json=short", "--timeout=2"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ManagerError::Unavailable)?;
    let stdout = child.stdout.take().ok_or(ManagerError::Unavailable)?;
    let mut bytes = Vec::new();
    let read = stdout
        .take((MAX_BUS_REPLY_BYTES + 1) as u64)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > MAX_BUS_REPLY_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ManagerError::Unavailable);
    }
    let status = child.wait().map_err(|_| ManagerError::Unavailable)?;
    if !status.success() {
        return Err(ManagerError::Unavailable);
    }
    serde_json::from_slice(&bytes).map_err(|_| ManagerError::Unavailable)
}

fn object_path(value: &Value) -> Result<&str, ManagerError> {
    if value.get("type").and_then(Value::as_str) != Some("o") {
        return Err(ManagerError::Unavailable);
    }
    let items = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or(ManagerError::Unavailable)?;
    if items.len() != 1 {
        return Err(ManagerError::Unavailable);
    }
    let path = items[0].as_str().ok_or(ManagerError::Unavailable)?;
    if !path.starts_with("/org/freedesktop/systemd1/unit/") {
        return Err(ManagerError::Unavailable);
    }
    Ok(path)
}

fn get_all(path: &str, interface: &str) -> Result<BTreeMap<String, Value>, ManagerError> {
    let value = busctl(&[
        "call",
        SYSTEMD_BUS,
        path,
        "org.freedesktop.DBus.Properties",
        "GetAll",
        "s",
        interface,
    ])?;
    if value.get("type").and_then(Value::as_str) != Some("a{sv}") {
        return Err(ManagerError::Unavailable);
    }
    let items = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or(ManagerError::Unavailable)?;
    if items.len() != 1 {
        return Err(ManagerError::Unavailable);
    }
    serde_json::from_value(items[0].clone()).map_err(|_| ManagerError::Unavailable)
}

fn property<'a>(
    properties: &'a BTreeMap<String, Value>,
    name: &str,
    signature: &str,
) -> Result<&'a Value, ManagerError> {
    let value = properties.get(name).ok_or(ManagerError::PropertyMismatch)?;
    if value.get("type").and_then(Value::as_str) != Some(signature) {
        return Err(ManagerError::PropertyMismatch);
    }
    value.get("data").ok_or(ManagerError::PropertyMismatch)
}

fn check(
    properties: &BTreeMap<String, Value>,
    name: &str,
    signature: &str,
    expected: &Value,
) -> Result<(), ManagerError> {
    if property(properties, name, signature)? != expected {
        return Err(ManagerError::PropertyMismatch);
    }
    Ok(())
}

fn verify_snapshot(
    unit_properties: &BTreeMap<String, Value>,
    service: &BTreeMap<String, Value>,
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
) -> Result<InstalledWorker, ManagerError> {
    use serde_json::json;

    let service_name = format!("{}.service", unit.unit_name);
    check(unit_properties, "Id", "s", &json!(service_name))?;
    check(unit_properties, "Transient", "b", &json!(true))?;
    check(
        unit_properties,
        "BindsTo",
        "as",
        &json!(["sley-test-supervisor.service"]),
    )?;

    for name in [
        "DynamicUser",
        "MemoryAccounting",
        "NoNewPrivileges",
        "PrivateNetwork",
        "PrivateTmp",
        "ProtectControlGroups",
        "SendSIGKILL",
    ] {
        check(service, name, "b", &json!(true))?;
    }
    check(service, "CapabilityBoundingSet", "t", &json!(0))?;
    check(service, "KillMode", "s", &json!("control-group"))?;
    check(service, "MemorySwapMax", "t", &json!(0))?;
    check(service, "ProtectHome", "s", &json!("yes"))?;
    check(service, "ProtectSystem", "s", &json!("strict"))?;
    check(service, "TasksMax", "t", &json!(1))?;
    check(service, "TimeoutStopUSec", "t", &json!(2_000_000))?;
    check(service, "MemoryMax", "t", &json!(unit.installed_memory))?;
    check(
        service,
        "RuntimeMaxUSec",
        "t",
        &json!(unit.runtime_max_usec),
    )?;

    check(service, "Slice", "s", &json!("system.slice"))?;
    check(
        service,
        "LoadCredential",
        "a(ss)",
        &json!([[WORKER_INPUT_CREDENTIAL, worker_input_path]]),
    )?;
    check(service, "TemporaryFileSystem", "a(ss)", &json!([]))?;
    check(
        service,
        "BindReadOnlyPaths",
        "a(ssbt)",
        &json!([[config.worker_path, config.worker_path, false, 16_384]]),
    )?;
    let exec_start = property(service, "ExecStart", "a(sasbttttuii)")?
        .as_array()
        .ok_or(ManagerError::PropertyMismatch)?;
    if exec_start.len() != 1
        || exec_start[0].as_array().is_none_or(|entry| {
            entry.len() != 10
                || entry[0] != json!(config.worker_path)
                || entry[1] != json!([config.worker_path, "__native-test-worker", "--credential"])
                || entry[2] != json!(false)
        })
    {
        return Err(ManagerError::PropertyMismatch);
    }

    let expected_group = format!("/system.slice/{service_name}");
    check(service, "ControlGroup", "s", &json!(expected_group))?;
    let main_pid = property(service, "MainPID", "u")?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value != 0)
        .ok_or(ManagerError::PropertyMismatch)?;
    Ok(InstalledWorker {
        control_group: expected_group,
        main_pid,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sley_id::{PrincipalId, WorkspaceId};

    use super::*;
    use crate::config::{AllowedCaller, default_config};

    fn insert(map: &mut BTreeMap<String, Value>, name: &str, signature: &str, data: Value) {
        let mut value = json!({"type":signature});
        value["data"] = data;
        map.insert(name.to_owned(), value);
    }

    fn fixture_config(worker: &str) -> RunnerConfig {
        default_config(
            "/run/sley-test-supervisor",
            worker,
            [1; 32],
            [2; 32],
            vec![AllowedCaller {
                uid: 1000,
                workspace: WorkspaceId::from_bytes([11; 32]),
                principal: PrincipalId::from_bytes([12; 32]),
            }],
            "/etc/sley-test-supervisor/measurement.key",
            "/etc/sley-test-supervisor/trust",
        )
        .unwrap()
    }

    fn fixture() -> (
        BTreeMap<String, Value>,
        BTreeMap<String, Value>,
        TransientUnit,
        RunnerConfig,
    ) {
        let name = format!("sley-native-test-{}", "a".repeat(64));
        let worker = "/usr/lib/sley/sley-native-test-worker";
        let input = "/run/sley-test-supervisor/input/abc.bin";
        let config = fixture_config(worker);
        let unit = TransientUnit {
            unit_name: name.clone(),
            argv: vec![],
            requested_memory: 8192,
            installed_memory: 8192,
            runtime_max_usec: 3_000_000,
        };
        let mut unit_map = BTreeMap::new();
        let mut service = BTreeMap::new();
        insert(&mut unit_map, "Id", "s", json!(format!("{name}.service")));
        insert(&mut unit_map, "Transient", "b", json!(true));
        insert(
            &mut unit_map,
            "BindsTo",
            "as",
            json!(["sley-test-supervisor.service"]),
        );
        for key in [
            "DynamicUser",
            "MemoryAccounting",
            "NoNewPrivileges",
            "PrivateNetwork",
            "PrivateTmp",
            "ProtectControlGroups",
            "SendSIGKILL",
        ] {
            insert(&mut service, key, "b", json!(true));
        }
        for (key, value) in [
            ("CapabilityBoundingSet", 0),
            ("MemorySwapMax", 0),
            ("TasksMax", 1),
            ("TimeoutStopUSec", 2_000_000),
            ("MemoryMax", 8192),
            ("RuntimeMaxUSec", 3_000_000),
        ] {
            insert(&mut service, key, "t", json!(value));
        }
        for (key, value) in [
            ("KillMode", "control-group"),
            ("ProtectHome", "yes"),
            ("ProtectSystem", "strict"),
            ("Slice", "system.slice"),
            ("ControlGroup", &*format!("/system.slice/{name}.service")),
        ] {
            insert(&mut service, key, "s", json!(value));
        }
        insert(
            &mut service,
            "LoadCredential",
            "a(ss)",
            json!([["sley-input", input]]),
        );
        insert(&mut service, "TemporaryFileSystem", "a(ss)", json!([]));
        insert(
            &mut service,
            "BindReadOnlyPaths",
            "a(ssbt)",
            json!([[worker, worker, false, 16_384]]),
        );
        insert(
            &mut service,
            "ExecStart",
            "a(sasbttttuii)",
            json!([[
                worker,
                [worker, "__native-test-worker", "--credential"],
                false,
                0,
                0,
                0,
                0,
                0,
                0,
                0
            ]]),
        );
        insert(&mut service, "MainPID", "u", json!(123));
        (unit_map, service, unit, config)
    }

    #[test]
    fn checks_typed_installed_values_and_fixed_launch_mapping() {
        let (unit_map, mut service, unit, config) = fixture();
        let input = "/run/sley-test-supervisor/input/abc.bin";
        let installed = verify_snapshot(&unit_map, &service, &unit, &config, input).unwrap();
        assert_eq!(installed.main_pid, 123);
        insert(&mut service, "RuntimeMaxUSec", "t", json!(3_000_001));
        assert_eq!(
            verify_snapshot(&unit_map, &service, &unit, &config, input),
            Err(ManagerError::PropertyMismatch)
        );
        insert(&mut service, "RuntimeMaxUSec", "s", json!("3000000"));
        assert_eq!(
            verify_snapshot(&unit_map, &service, &unit, &config, input),
            Err(ManagerError::PropertyMismatch)
        );
    }

    #[test]
    fn refuses_substituted_credential_and_missing_manager_property() {
        let (mut unit_map, mut service, unit, config) = fixture();
        let input = "/run/sley-test-supervisor/input/abc.bin";
        insert(
            &mut service,
            "LoadCredential",
            "a(ss)",
            json!([["sley-input", "/tmp/other"]]),
        );
        assert_eq!(
            verify_snapshot(&unit_map, &service, &unit, &config, input),
            Err(ManagerError::PropertyMismatch)
        );
        service.remove("LoadCredential");
        assert_eq!(
            verify_snapshot(&unit_map, &service, &unit, &config, input),
            Err(ManagerError::PropertyMismatch)
        );
        let (_, service, _, _) = fixture();
        insert(&mut unit_map, "Transient", "b", json!(false));
        assert_eq!(
            verify_snapshot(&unit_map, &service, &unit, &config, input),
            Err(ManagerError::PropertyMismatch)
        );
    }

    #[test]
    fn reaped_unit_list_requires_the_exact_typed_empty_reply() {
        assert!(
            listed_units(&json!({"type":"a(ssssssouso)","data":[[]]}))
                .unwrap()
                .is_empty()
        );
        assert!(listed_units(&json!({"type":"a(ssssssouso)","data":[[["unit"]]]})).is_ok());
        assert_eq!(
            listed_units(&json!({"type":"as","data":[[]]})),
            Err(ManagerError::Unavailable)
        );
    }
}

//! Typed systemd manager verification for a gated native-test worker.
//!
//! `systemctl show` formats durations and capability sets for display. The
//! supervisor instead reads the manager's typed D-Bus properties and checks
//! the installed unit before sending the worker's stdin start byte. This
//! module only verifies an existing unit; it cannot launch or attest one.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{ErrorKind, Read};
use std::os::fd::AsFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};

use nix::errno::Errno;
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::config::{RunnerConfig, UNIT_PREFIX};
use crate::unit::TransientUnit;
use crate::worker::WORKER_INPUT_CREDENTIAL;

const SYSTEMD_BUS: &str = "org.freedesktop.systemd1";
const SYSTEMD_MANAGER: &str = "/org/freedesktop/systemd1";
const MAX_BUS_REPLY_BYTES: usize = 131_072;
/// Startup reconciliation refuses an unbounded number of worker units.
pub const MAX_NATIVE_UNITS: usize = 256;

/// Refusal to trust the manager's installed transient-unit state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagerError {
    /// The manager or typed D-Bus response could not be read.
    Unavailable,
    /// The owned read-only manager command could not be confirmed reaped.
    CommandCleanupUnconfirmed,
    /// The transient service exists but has not exposed its worker PID yet.
    NotReady,
    /// A required typed property was missing, malformed, or different.
    PropertyMismatch,
    /// Any loaded unit, including an already reaped failure, owns this name.
    UnitOccupied,
}

impl core::fmt::Display for ManagerError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "NATIVE_MANAGER_UNAVAILABLE",
            Self::CommandCleanupUnconfirmed => "NATIVE_MANAGER_COMMAND_CLEANUP_UNCONFIRMED",
            Self::NotReady => "NATIVE_MANAGER_NOT_READY",
            Self::PropertyMismatch => "NATIVE_MANAGER_PROPERTY_MISMATCH",
            Self::UnitOccupied => "NATIVE_MANAGER_UNIT_OCCUPIED",
        })
    }
}

impl std::error::Error for ManagerError {}

/// A manager invocation observed to start after the current launch boundary.
/// Fields stay private so production callers cannot manufacture this proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerInvocation {
    id: [u8; 16],
    main_pid: u32,
    started_usec: u64,
}

impl WorkerInvocation {
    pub(crate) const fn main_pid(self) -> u32 {
        self.main_pid
    }

    #[cfg(test)]
    pub(crate) const fn fixture() -> Self {
        Self {
            id: [1; 16],
            main_pid: 123,
            started_usec: 100,
        }
    }
}

/// Reads the same boot-relative microsecond clock as manager start timestamps.
///
/// # Errors
/// Refuses an unavailable, negative, or overflowing clock value.
pub(crate) fn monotonic_usec() -> Result<u64, ManagerError> {
    let now = nix::time::clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC)
        .map_err(|_| ManagerError::Unavailable)?;
    let seconds = u64::try_from(now.tv_sec()).map_err(|_| ManagerError::Unavailable)?;
    let nanos = u64::try_from(now.tv_nsec()).map_err(|_| ManagerError::Unavailable)?;
    seconds
        .checked_mul(1_000_000)
        .and_then(|value| value.checked_add(nanos / 1_000))
        .ok_or(ManagerError::Unavailable)
}

fn require_absent_unit_listing(value: &Value, unit_name: &str) -> Result<(), ManagerError> {
    if !valid_native_unit_name(unit_name) {
        return Err(ManagerError::PropertyMismatch);
    }
    let names = native_unit_names(value)?;
    match names.as_slice() {
        [] => Ok(()),
        [name] if name == unit_name => Err(ManagerError::UnitOccupied),
        _ => Err(ManagerError::PropertyMismatch),
    }
}

/// Refuses every loaded unit with the requested name before a launcher starts.
/// Absence alone is not launch proof; a fresh invocation must also be observed.
///
/// # Errors
/// Refuses occupied names, malformed replies and unavailable manager state.
pub fn require_system_unit_absent(unit_name: &str) -> Result<(), ManagerError> {
    require_system_unit_absent_before(unit_name, default_deadline())
}

pub(crate) fn require_system_unit_absent_before(
    unit_name: &str,
    deadline: Instant,
) -> Result<(), ManagerError> {
    if !valid_native_unit_name(unit_name) {
        return Err(ManagerError::PropertyMismatch);
    }
    let service_name = format!("{unit_name}.service");
    let listing = busctl(
        &[
            "call",
            SYSTEMD_BUS,
            SYSTEMD_MANAGER,
            "org.freedesktop.systemd1.Manager",
            "ListUnitsByPatterns",
            "asas",
            "0",
            "1",
            &service_name,
        ],
        deadline,
    )?;
    accept_before(require_absent_unit_listing(&listing, unit_name), deadline)
}

type ManagerProperties = BTreeMap<String, Value>;

fn read_unit_snapshot(
    unit_name: &str,
    deadline: Instant,
) -> Result<(ManagerProperties, ManagerProperties), ManagerError> {
    let service_name = format!("{unit_name}.service");
    let object = busctl(
        &[
            "call",
            SYSTEMD_BUS,
            SYSTEMD_MANAGER,
            "org.freedesktop.systemd1.Manager",
            "GetUnit",
            "s",
            &service_name,
        ],
        deadline,
    )?;
    let path = object_path(&object)?;
    let snapshot = (
        get_all(path, "org.freedesktop.systemd1.Unit", deadline)?,
        get_all(path, "org.freedesktop.systemd1.Service", deadline)?,
    );
    accept_before(Ok(snapshot), deadline)
}

fn invocation_snapshot(
    unit: &BTreeMap<String, Value>,
    service: &BTreeMap<String, Value>,
) -> Result<WorkerInvocation, ManagerError> {
    let bytes = property(unit, "InvocationID", "ay")?
        .as_array()
        .ok_or(ManagerError::PropertyMismatch)?;
    let mut id = [0; 16];
    if bytes.len() != id.len() {
        return Err(ManagerError::PropertyMismatch);
    }
    for (byte, value) in id.iter_mut().zip(bytes) {
        *byte = value
            .as_u64()
            .and_then(|value| u8::try_from(value).ok())
            .ok_or(ManagerError::PropertyMismatch)?;
    }
    let main_pid = property(service, "ExecMainPID", "u")?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(ManagerError::PropertyMismatch)?;
    let started_usec = property(service, "ExecMainStartTimestampMonotonic", "t")?
        .as_u64()
        .ok_or(ManagerError::PropertyMismatch)?;
    if id == [0; 16] || main_pid == 0 || started_usec == 0 {
        return Err(ManagerError::NotReady);
    }
    Ok(WorkerInvocation {
        id,
        main_pid,
        started_usec,
    })
}

fn fresh_invocation_snapshot(
    unit: &BTreeMap<String, Value>,
    service: &BTreeMap<String, Value>,
    launched_after_usec: u64,
    observed_usec: u64,
) -> Result<WorkerInvocation, ManagerError> {
    let invocation = invocation_snapshot(unit, service)?;
    if invocation.started_usec <= launched_after_usec || invocation.started_usec > observed_usec {
        return Err(ManagerError::PropertyMismatch);
    }
    Ok(invocation)
}

/// Observes an exact installed invocation created after this launch began.
/// A fast OOM can supply these facts even before the normal live phase sees it.
///
/// # Errors
/// Refuses old, missing, malformed or inconsistent manager invocation facts.
pub fn verify_launched_invocation(
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
    launched_after_usec: u64,
) -> Result<WorkerInvocation, ManagerError> {
    verify_launched_invocation_before(
        unit,
        config,
        worker_input_path,
        launched_after_usec,
        default_deadline(),
    )
}

pub(crate) fn verify_launched_invocation_before(
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
    launched_after_usec: u64,
    deadline: Instant,
) -> Result<WorkerInvocation, ManagerError> {
    let (unit_properties, service) = read_unit_snapshot(&unit.unit_name, deadline)?;
    verify_installed_settings(&unit_properties, &service, unit, config, worker_input_path)?;
    accept_before(
        fresh_invocation_snapshot(
            &unit_properties,
            &service,
            launched_after_usec,
            monotonic_usec()?,
        ),
        deadline,
    )
}
/// Manager facts needed to pin live cgroup telemetry to the worker process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledWorker {
    /// Exact cgroup path reported by the manager for this transient service.
    pub control_group: String,
    /// Main process ID reported by the manager before worker release.
    pub main_pid: u32,
}

/// Manager facts retained after the exact worker unit failed from OOM.
///
/// These facts are not cgroup event counters or a signed test result. The
/// verifier also requires the unit's cgroup to be absent after teardown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReapedOomWorker {
    /// Main process recorded for the failed transient service.
    pub exec_main_pid: u32,
    /// Peak memory retained by the system manager for that service.
    pub memory_peak: u64,
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
    let deadline = default_deadline();
    let (properties, service) = read_unit_snapshot(&unit.unit_name, deadline)?;
    accept_before(
        verify_snapshot(&properties, &service, unit, config, worker_input_path),
        deadline,
    )
}

pub(crate) fn verify_live_invocation(
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
    expected: WorkerInvocation,
    deadline: Instant,
) -> Result<InstalledWorker, ManagerError> {
    let (properties, service) = read_unit_snapshot(&unit.unit_name, deadline)?;
    accept_before(
        verify_live_invocation_snapshot(
            &properties,
            &service,
            unit,
            config,
            worker_input_path,
            expected,
        ),
        deadline,
    )
}

fn verify_live_invocation_snapshot(
    properties: &ManagerProperties,
    service: &ManagerProperties,
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
    expected: WorkerInvocation,
) -> Result<InstalledWorker, ManagerError> {
    let installed = verify_snapshot(properties, service, unit, config, worker_input_path)?;
    if invocation_snapshot(properties, service)? != expected
        || installed.main_pid != expected.main_pid
    {
        return Err(ManagerError::PropertyMismatch);
    }
    Ok(installed)
}

fn same_invocation_terminal(
    properties: &ManagerProperties,
    service: &ManagerProperties,
    expected: WorkerInvocation,
) -> Result<bool, ManagerError> {
    if invocation_snapshot(properties, service)? != expected {
        return Err(ManagerError::PropertyMismatch);
    }
    let state = property(properties, "ActiveState", "s")?
        .as_str()
        .ok_or(ManagerError::PropertyMismatch)?;
    if !matches!(state, "inactive" | "failed") {
        return Ok(false);
    }
    check(service, "MainPID", "u", &Value::from(0))?;
    check(service, "ControlGroup", "s", &Value::from(""))?;
    Ok(true)
}

/// Missing manager history is acceptable only after a live cgroup was bound.
/// These sampled facts require a trusted manager and exclusive lifecycle owner.
pub(crate) fn confirm_invocation_reaped(
    name: &str,
    expected: WorkerInvocation,
    bound_cgroup: bool,
    deadline: Instant,
) -> Result<bool, ManagerError> {
    match require_system_unit_absent_before(name, deadline) {
        Ok(()) if bound_cgroup => return confirm_system_unit_reaped_before(name, deadline),
        Ok(()) => return Err(ManagerError::Unavailable),
        Err(ManagerError::UnitOccupied) => {}
        Err(error) => return Err(error),
    }
    let (properties, service) = read_unit_snapshot(name, deadline)?;
    check(
        &properties,
        "Id",
        "s",
        &Value::from(format!("{name}.service")),
    )?;
    if !same_invocation_terminal(&properties, &service, expected)? {
        return Ok(false);
    }
    confirm_system_unit_reaped_before(name, deadline)
}

/// Verifies a failed OOM outcome for the exact rendered worker unit.
///
/// This checks the same installed launch and limit properties as the live
/// start gate, plus the manager's retained OOM result and killed main PID.
/// It never substitutes manager data for missing cgroup event counters.
///
/// # Errors
/// Refuses a collected unit, changed settings, another failure reason,
/// missing process identity, or an implausible manager memory peak.
pub fn verify_reaped_oom_unit(
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
    expected_invocation: WorkerInvocation,
) -> Result<ReapedOomWorker, ManagerError> {
    verify_reaped_oom_unit_before(
        unit,
        config,
        worker_input_path,
        expected_invocation,
        default_deadline(),
    )
}

pub(crate) fn verify_reaped_oom_unit_before(
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
    expected_invocation: WorkerInvocation,
    deadline: Instant,
) -> Result<ReapedOomWorker, ManagerError> {
    let service_name = format!("{}.service", unit.unit_name);
    let object = busctl(
        &[
            "call",
            SYSTEMD_BUS,
            SYSTEMD_MANAGER,
            "org.freedesktop.systemd1.Manager",
            "GetUnit",
            "s",
            &service_name,
        ],
        deadline,
    )?;
    let path = object_path(&object)?;
    let unit_properties = get_all(path, "org.freedesktop.systemd1.Unit", deadline)?;
    let service_properties = get_all(path, "org.freedesktop.systemd1.Service", deadline)?;
    let facts = verify_reaped_oom_snapshot(
        &unit_properties,
        &service_properties,
        unit,
        config,
        worker_input_path,
        expected_invocation,
    )?;
    if !confirm_system_unit_reaped_before(&unit.unit_name, deadline)? {
        return Err(ManagerError::PropertyMismatch);
    }
    accept_before(Ok(facts), deadline)
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
    confirm_system_unit_reaped_before(unit_name, default_deadline())
}

pub(crate) fn confirm_system_unit_reaped_before(
    unit_name: &str,
    deadline: Instant,
) -> Result<bool, ManagerError> {
    if !valid_native_unit_name(unit_name) {
        return Err(ManagerError::PropertyMismatch);
    }
    let service_name = format!("{unit_name}.service");
    let listing = busctl(
        &[
            "call",
            SYSTEMD_BUS,
            SYSTEMD_MANAGER,
            "org.freedesktop.systemd1.Manager",
            "ListUnitsByPatterns",
            "asas",
            "0",
            "1",
            &service_name,
        ],
        deadline,
    )?;
    let entries = listed_units(&listing)?;
    if entries.len() > 1 {
        return Err(ManagerError::PropertyMismatch);
    }
    if let Some(entry) = entries.first() {
        let fields = entry.as_array().ok_or(ManagerError::Unavailable)?;
        if fields.len() != 10 || fields[0].as_str() != Some(&service_name) {
            return Err(ManagerError::PropertyMismatch);
        }
        let object = busctl(
            &[
                "call",
                SYSTEMD_BUS,
                SYSTEMD_MANAGER,
                "org.freedesktop.systemd1.Manager",
                "GetUnit",
                "s",
                &service_name,
            ],
            deadline,
        )?;
        let path = object_path(&object)?;
        let unit = get_all(path, "org.freedesktop.systemd1.Unit", deadline)?;
        let service = get_all(path, "org.freedesktop.systemd1.Service", deadline)?;
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
    accept_before(
        match std::fs::symlink_metadata(cgroup_path) {
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
            Ok(_) => Ok(false),
            Err(_) => Err(ManagerError::Unavailable),
        },
        deadline,
    )
}

/// Exact native transient-unit stem, excluding `.service`.
#[must_use]
pub fn valid_native_unit_name(unit_name: &str) -> bool {
    unit_name.strip_prefix(UNIT_PREFIX).is_some_and(|nonce| {
        nonce.len() == 64
            && nonce
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn native_unit_names(value: &Value) -> Result<Vec<String>, ManagerError> {
    let entries = listed_units(value)?;
    if entries.len() > MAX_NATIVE_UNITS {
        return Err(ManagerError::PropertyMismatch);
    }
    let mut names = BTreeSet::new();
    for entry in entries {
        let fields = entry.as_array().ok_or(ManagerError::Unavailable)?;
        let service_name = fields
            .first()
            .and_then(Value::as_str)
            .ok_or(ManagerError::Unavailable)?;
        let unit_name = service_name
            .strip_suffix(".service")
            .ok_or(ManagerError::PropertyMismatch)?;
        if fields.len() != 10 || !valid_native_unit_name(unit_name) {
            return Err(ManagerError::PropertyMismatch);
        }
        if !names.insert(unit_name.to_owned()) {
            return Err(ManagerError::PropertyMismatch);
        }
    }
    Ok(names.into_iter().collect())
}

/// Lists all loaded system transient units with the frozen native prefix.
///
/// The typed manager reply is bounded and every returned name must have an
/// exact 64-character lowercase nonce. This includes inactive and failed
/// loaded units; a separate cgroup enumeration covers manager-absent groups.
///
/// # Errors
///
/// Refuses unavailable or malformed manager replies or too many units.
pub fn list_native_system_units() -> Result<Vec<String>, ManagerError> {
    let deadline = default_deadline();
    let listing = busctl(
        &[
            "call",
            SYSTEMD_BUS,
            SYSTEMD_MANAGER,
            "org.freedesktop.systemd1.Manager",
            "ListUnitsByPatterns",
            "asas",
            "0",
            "1",
            "sley-native-test-*.service",
        ],
        deadline,
    )?;
    accept_before(native_unit_names(&listing), deadline)
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

fn default_deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}

fn remaining(deadline: Instant) -> Result<Duration, ManagerError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or(ManagerError::Unavailable)
}

fn accept_before<T>(result: Result<T, ManagerError>, deadline: Instant) -> Result<T, ManagerError> {
    let value = result?;
    remaining(deadline)?;
    Ok(value)
}

fn busctl(args: &[&str], deadline: Instant) -> Result<Value, ManagerError> {
    let mut command = Command::new("/usr/bin/busctl");
    command
        .args(["--system", "--json=short", "--timeout=2"])
        .args(args);
    command_json(&mut command, deadline)
}

const COMMAND_CLEANUP_RESERVE: Duration = Duration::from_millis(20);
const COMMAND_POLL: Duration = Duration::from_millis(1);

struct ManagerCommand {
    child: Child,
    deadline: Instant,
    reaped: bool,
}

impl ManagerCommand {
    fn stop_and_reap(&mut self) -> Result<(), ManagerError> {
        if self.reaped {
            return Ok(());
        }
        let _ = self.child.kill();
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    self.reaped = true;
                    return Ok(());
                }
                Err(_) => return Err(ManagerError::CommandCleanupUnconfirmed),
                Ok(None) => {}
            }
            let left =
                remaining(self.deadline).map_err(|_| ManagerError::CommandCleanupUnconfirmed)?;
            std::thread::sleep(left.min(COMMAND_POLL));
        }
    }

    fn collect(
        &mut self,
        stdout: &mut ChildStdout,
        work_deadline: Instant,
    ) -> Result<Vec<u8>, ManagerError> {
        fcntl(&*stdout, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
            .map_err(|_| ManagerError::Unavailable)?;
        let mut bytes = Vec::new();
        let mut buffer = [0; 4096];
        let mut eof = false;
        let mut status = None;
        loop {
            let left = remaining(work_deadline)?;
            if status.is_none() {
                status = self
                    .child
                    .try_wait()
                    .map_err(|_| ManagerError::Unavailable)?;
                self.reaped = status.is_some();
            }
            if eof {
                if let Some(status) = status {
                    if !status.success() {
                        return Err(ManagerError::Unavailable);
                    }
                    return accept_before(Ok(bytes), work_deadline);
                }
                std::thread::sleep(left.min(COMMAND_POLL));
                continue;
            }
            let timeout = PollTimeout::try_from(left.as_millis().clamp(1, 20))
                .map_err(|_| ManagerError::Unavailable)?;
            let mut fds = [PollFd::new(
                stdout.as_fd(),
                PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR,
            )];
            match poll(&mut fds, timeout) {
                Ok(0) | Err(Errno::EINTR) => continue,
                Err(_) => return Err(ManagerError::Unavailable),
                Ok(_) => {}
            }
            remaining(work_deadline)?;
            match stdout.read(&mut buffer) {
                Ok(0) => eof = true,
                Ok(count) => {
                    if bytes.len() + count > MAX_BUS_REPLY_BYTES {
                        return Err(ManagerError::Unavailable);
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
                Err(_) => return Err(ManagerError::Unavailable),
            }
        }
    }
}

impl Drop for ManagerCommand {
    fn drop(&mut self) {
        let _ = self.stop_and_reap();
    }
}

fn command_json(command: &mut Command, deadline: Instant) -> Result<Value, ManagerError> {
    // Reserve cleanup inside the caller's one absolute budget. A short or
    // expired budget refuses before spawn. Scheduling/kernel stalls can still
    // exhaust it; that produces explicit unknown cleanup, never late success.
    let work_deadline = deadline
        .checked_sub(COMMAND_CLEANUP_RESERVE)
        .ok_or(ManagerError::Unavailable)?;
    remaining(work_deadline)?;
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ManagerError::Unavailable)?;
    let mut owned = ManagerCommand {
        child,
        deadline,
        reaped: false,
    };
    let read = if let Some(mut stdout) = owned.child.stdout.take() {
        owned.collect(&mut stdout, work_deadline)
    } else {
        Err(ManagerError::Unavailable)
    };
    // Even a malformed/oversized/late reply must not leave our child running.
    owned.stop_and_reap()?;
    let bytes = read?;
    accept_before(
        serde_json::from_slice(&bytes).map_err(|_| ManagerError::Unavailable),
        deadline,
    )
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

fn get_all(
    path: &str,
    interface: &str,
    deadline: Instant,
) -> Result<BTreeMap<String, Value>, ManagerError> {
    let value = busctl(
        &[
            "call",
            SYSTEMD_BUS,
            path,
            "org.freedesktop.DBus.Properties",
            "GetAll",
            "s",
            interface,
        ],
        deadline,
    )?;
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
    accept_before(
        serde_json::from_value(items[0].clone()).map_err(|_| ManagerError::Unavailable),
        deadline,
    )
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

fn verify_installed_settings(
    unit_properties: &BTreeMap<String, Value>,
    service: &BTreeMap<String, Value>,
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
) -> Result<(), ManagerError> {
    use serde_json::json;

    check(service, "Restart", "s", &Value::from("no"))?;

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

    Ok(())
}

fn verify_snapshot(
    unit_properties: &BTreeMap<String, Value>,
    service: &BTreeMap<String, Value>,
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
) -> Result<InstalledWorker, ManagerError> {
    verify_installed_settings(unit_properties, service, unit, config, worker_input_path)?;
    let service_name = format!("{}.service", unit.unit_name);
    let state = property(unit_properties, "ActiveState", "s")?
        .as_str()
        .ok_or(ManagerError::PropertyMismatch)?;
    if state == "activating" {
        return Err(ManagerError::NotReady);
    }
    if state != "active" {
        return Err(ManagerError::PropertyMismatch);
    }

    let expected_group = format!("/system.slice/{service_name}");
    let actual_group = property(service, "ControlGroup", "s")?
        .as_str()
        .ok_or(ManagerError::PropertyMismatch)?;
    if actual_group.is_empty() {
        return Err(ManagerError::NotReady);
    }
    if actual_group != expected_group {
        return Err(ManagerError::PropertyMismatch);
    }
    let main_pid = property(service, "MainPID", "u")?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(ManagerError::PropertyMismatch)?;
    if main_pid == 0 {
        return Err(ManagerError::NotReady);
    }
    Ok(InstalledWorker {
        control_group: expected_group,
        main_pid,
    })
}

fn verify_reaped_oom_snapshot(
    unit_properties: &BTreeMap<String, Value>,
    service: &BTreeMap<String, Value>,
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
    expected_invocation: WorkerInvocation,
) -> Result<ReapedOomWorker, ManagerError> {
    use serde_json::json;

    verify_installed_settings(unit_properties, service, unit, config, worker_input_path)?;
    if invocation_snapshot(unit_properties, service)? != expected_invocation {
        return Err(ManagerError::PropertyMismatch);
    }
    check(unit_properties, "ActiveState", "s", &json!("failed"))?;
    check(service, "Result", "s", &json!("oom-kill"))?;
    check(service, "MainPID", "u", &json!(0))?;
    check(service, "ControlGroup", "s", &json!(""))?;
    check(service, "ExecMainCode", "i", &json!(2))?;
    check(service, "ExecMainStatus", "i", &json!(9))?;
    let exec_main_pid = property(service, "ExecMainPID", "u")?
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value != 0)
        .ok_or(ManagerError::PropertyMismatch)?;
    if expected_invocation.main_pid != exec_main_pid {
        return Err(ManagerError::PropertyMismatch);
    }
    let memory_peak = property(service, "MemoryPeak", "t")?
        .as_u64()
        .filter(|value| *value > 0 && *value <= unit.installed_memory)
        .ok_or(ManagerError::PropertyMismatch)?;
    Ok(ReapedOomWorker {
        exec_main_pid,
        memory_peak,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sley_id::{PrincipalId, WorkspaceId};

    use super::*;
    use crate::config::{AllowedCaller, default_config};

    #[test]
    fn manager_command_rejects_success_after_its_absolute_deadline() {
        let mut command = Command::new("/usr/bin/python3");
        command.args([
            "-I",
            "-B",
            "-c",
            "import time; time.sleep(0.04); print('{}')",
        ]);
        assert_eq!(
            command_json(&mut command, Instant::now() + Duration::from_millis(5)),
            Err(ManagerError::Unavailable),
            "late successful manager reply must never be accepted",
        );
    }

    fn python_command(script: &str) -> Command {
        let mut command = Command::new("/usr/bin/python3");
        command.args(["-I", "-B", "-c", script]);
        command
    }

    #[test]
    fn manager_command_requires_bounded_json_and_successful_exit() {
        assert_eq!(
            command_json(&mut python_command("print('{}')"), default_deadline()),
            Ok(json!({}))
        );
        for script in [
            "print('{}'); raise SystemExit(7)",
            "print('{')",
            "print('x' * 131073)",
        ] {
            assert_eq!(
                command_json(&mut python_command(script), default_deadline()),
                Err(ManagerError::Unavailable)
            );
        }
        assert_eq!(
            accept_before(Ok(json!({})), Instant::now()),
            Err(ManagerError::Unavailable)
        );
    }

    #[test]
    fn manager_command_reaps_partial_output_and_early_eof_children() {
        for (index, output) in ["print('{', flush=True)", "os.close(1)"].iter().enumerate() {
            let marker = std::env::temp_dir()
                .join(format!("sley-manager-child-{}-{index}", std::process::id()));
            let script = format!(
                "import os, sys, time; open(sys.argv[1], 'x').write(str(os.getpid())); {output}; time.sleep(0.5)"
            );
            let mut command = python_command(&script);
            command.arg(&marker);
            let result = command_json(&mut command, Instant::now() + Duration::from_millis(150));
            let pid = std::fs::read_to_string(&marker).unwrap();
            std::fs::remove_file(marker).unwrap();
            assert_eq!(result, Err(ManagerError::Unavailable));
            assert!(
                !Path::new(&format!("/proc/{pid}")).exists(),
                "owned command must be reaped"
            );
        }
    }

    #[test]
    fn consecutive_manager_calls_share_the_original_deadline() {
        let deadline = Instant::now() + Duration::from_millis(500);
        assert!(command_json(&mut python_command("print('{}')"), deadline).is_ok());
        std::thread::sleep(
            remaining(deadline)
                .unwrap()
                .saturating_sub(Duration::from_millis(40)),
        );
        let second = command_json(
            &mut python_command("import time; time.sleep(0.08); print('{}')"),
            deadline,
        );
        assert_eq!(second, Err(ManagerError::Unavailable));
    }

    fn insert(map: &mut BTreeMap<String, Value>, name: &str, signature: &str, data: Value) {
        let mut value = json!({"type":signature});
        value["data"] = data;
        map.insert(name.to_owned(), value);
    }

    #[test]
    fn terminal_cleanup_requires_the_same_invocation_and_zero_live_state() {
        let (mut properties, mut service, _, _) = fixture();
        let expected = WorkerInvocation::fixture();
        insert(&mut properties, "InvocationID", "ay", json!(vec![1; 16]));
        insert(&mut service, "ExecMainPID", "u", json!(123));
        insert(
            &mut service,
            "ExecMainStartTimestampMonotonic",
            "t",
            json!(100),
        );
        assert_eq!(
            same_invocation_terminal(&properties, &service, expected),
            Ok(false)
        );
        insert(&mut properties, "ActiveState", "s", json!("failed"));
        insert(&mut service, "MainPID", "u", json!(0));
        insert(&mut service, "ControlGroup", "s", json!(""));
        assert_eq!(
            same_invocation_terminal(&properties, &service, expected),
            Ok(true)
        );
        for (field, signature, changed) in [
            ("ExecMainPID", "u", json!(124)),
            ("ExecMainStartTimestampMonotonic", "t", json!(101)),
        ] {
            let mut replacement = service.clone();
            insert(&mut replacement, field, signature, changed);
            assert_eq!(
                same_invocation_terminal(&properties, &replacement, expected),
                Err(ManagerError::PropertyMismatch)
            );
        }
        insert(&mut properties, "InvocationID", "ay", json!(vec![2; 16]));
        assert_eq!(
            same_invocation_terminal(&properties, &service, expected),
            Err(ManagerError::PropertyMismatch)
        );
    }

    #[test]
    fn live_binding_rejects_replacement_invocation_and_worker_restart() {
        let (mut properties, mut service, unit, config) = fixture();
        let expected = WorkerInvocation::fixture();
        insert(&mut properties, "InvocationID", "ay", json!(vec![1; 16]));
        insert(&mut service, "ExecMainPID", "u", json!(123));
        insert(
            &mut service,
            "ExecMainStartTimestampMonotonic",
            "t",
            json!(100),
        );
        let read = |properties: &ManagerProperties, service: &ManagerProperties| {
            verify_live_invocation_snapshot(
                properties,
                service,
                &unit,
                &config,
                "/run/sley-test-supervisor/input/abc.bin",
                expected,
            )
        };
        assert!(read(&properties, &service).is_ok());
        insert(&mut properties, "InvocationID", "ay", json!(vec![2; 16]));
        assert_eq!(
            read(&properties, &service),
            Err(ManagerError::PropertyMismatch)
        );
        insert(&mut properties, "InvocationID", "ay", json!(vec![1; 16]));
        insert(&mut service, "Restart", "s", json!("always"));
        assert_eq!(
            read(&properties, &service),
            Err(ManagerError::PropertyMismatch)
        );
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
        insert(&mut unit_map, "ActiveState", "s", json!("active"));
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
            ("Restart", "no"),
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
    fn native_unit_listing_accepts_only_exact_names() {
        let name = format!("sley-native-test-{}.service", "a".repeat(64));
        let listed = |service_name: &str| {
            json!({
                "type": "a(ssssssouso)",
                "data": [[[
                    service_name, "worker", "loaded", "failed", "failed", "",
                    "/org/freedesktop/systemd1/unit/worker", 0, "", "/"
                ]]]
            })
        };
        assert_eq!(
            native_unit_names(&listed(&name)),
            Ok(vec![name.trim_end_matches(".service").to_owned()])
        );
        assert_eq!(
            require_absent_unit_listing(&listed(&name), name.trim_end_matches(".service")),
            Err(ManagerError::UnitOccupied)
        );
        assert_eq!(
            require_absent_unit_listing(
                &json!({"type":"a(ssssssouso)","data":[[]]}),
                name.trim_end_matches(".service")
            ),
            Ok(())
        );
        assert_eq!(
            native_unit_names(&listed("sley-native-test-short.service")),
            Err(ManagerError::PropertyMismatch)
        );
        assert_eq!(
            native_unit_names(&listed(&name.to_uppercase())),
            Err(ManagerError::PropertyMismatch)
        );
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
    fn waits_only_for_a_real_startup_state() {
        let (mut unit_map, mut service, unit, config) = fixture();
        let input = "/run/sley-test-supervisor/input/abc.bin";
        insert(&mut unit_map, "ActiveState", "s", json!("activating"));
        assert_eq!(
            verify_snapshot(&unit_map, &service, &unit, &config, input),
            Err(ManagerError::NotReady)
        );
        insert(&mut unit_map, "ActiveState", "s", json!("failed"));
        assert_eq!(
            verify_snapshot(&unit_map, &service, &unit, &config, input),
            Err(ManagerError::PropertyMismatch)
        );
        insert(&mut unit_map, "ActiveState", "s", json!("active"));
        insert(&mut service, "MainPID", "u", json!(0));
        assert_eq!(
            verify_snapshot(&unit_map, &service, &unit, &config, input),
            Err(ManagerError::NotReady)
        );
    }

    fn oom_fixture() -> (
        BTreeMap<String, Value>,
        BTreeMap<String, Value>,
        TransientUnit,
        RunnerConfig,
    ) {
        let (mut unit_map, mut service, unit, config) = fixture();
        insert(&mut unit_map, "ActiveState", "s", json!("failed"));
        insert(&mut service, "Result", "s", json!("oom-kill"));
        insert(&mut service, "MainPID", "u", json!(0));
        insert(&mut service, "ControlGroup", "s", json!(""));
        insert(&mut service, "ExecMainPID", "u", json!(123));
        insert(&mut service, "ExecMainCode", "i", json!(2));
        insert(&mut service, "ExecMainStatus", "i", json!(9));
        insert(&mut service, "MemoryPeak", "t", json!(8192));
        insert(&mut unit_map, "InvocationID", "ay", json!(vec![1; 16]));
        insert(
            &mut service,
            "ExecMainStartTimestampMonotonic",
            "t",
            json!(101),
        );
        (unit_map, service, unit, config)
    }

    #[test]
    fn reaped_oom_requires_exact_manager_result_and_original_unit_settings() {
        let (mut unit_map, mut service, unit, config) = oom_fixture();
        let input = "/run/sley-test-supervisor/input/abc.bin";
        let invocation = fresh_invocation_snapshot(&unit_map, &service, 100, 200).unwrap();
        let check = |unit_map: &BTreeMap<String, Value>,
                     service: &BTreeMap<String, Value>,
                     expected_invocation| {
            verify_reaped_oom_snapshot(
                unit_map,
                service,
                &unit,
                &config,
                input,
                expected_invocation,
            )
        };
        assert_eq!(
            check(&unit_map, &service, invocation),
            Ok(ReapedOomWorker {
                exec_main_pid: 123,
                memory_peak: 8192,
            })
        );
        assert_eq!(
            check(
                &unit_map,
                &service,
                WorkerInvocation {
                    main_pid: 124,
                    ..invocation
                }
            ),
            Err(ManagerError::PropertyMismatch)
        );
        assert_eq!(
            check(
                &unit_map,
                &service,
                WorkerInvocation {
                    id: [2; 16],
                    ..invocation
                }
            ),
            Err(ManagerError::PropertyMismatch)
        );
        assert_eq!(
            check(
                &unit_map,
                &service,
                WorkerInvocation {
                    started_usec: 102,
                    ..invocation
                }
            ),
            Err(ManagerError::PropertyMismatch)
        );
        insert(&mut service, "Result", "s", json!("signal"));
        assert_eq!(
            check(&unit_map, &service, invocation),
            Err(ManagerError::PropertyMismatch)
        );
        insert(&mut service, "Result", "s", json!("oom-kill"));
        insert(&mut service, "ExecMainStatus", "i", json!(15));
        assert_eq!(
            check(&unit_map, &service, invocation),
            Err(ManagerError::PropertyMismatch)
        );
        insert(&mut service, "ExecMainStatus", "i", json!(9));
        insert(&mut service, "MemoryPeak", "t", json!(8193));
        assert_eq!(
            check(&unit_map, &service, invocation),
            Err(ManagerError::PropertyMismatch)
        );
        insert(&mut service, "MemoryPeak", "t", json!(8192));
        insert(&mut service, "MemoryMax", "t", json!(8193));
        assert_eq!(
            check(&unit_map, &service, invocation),
            Err(ManagerError::PropertyMismatch)
        );
        insert(&mut service, "MemoryMax", "t", json!(8192));
        insert(&mut unit_map, "Transient", "b", json!(false));
        assert_eq!(
            check(&unit_map, &service, invocation),
            Err(ManagerError::PropertyMismatch)
        );
    }

    #[test]
    fn reused_nonce_with_changed_request_cannot_reuse_an_old_manager_invocation() {
        use sha2::{Digest, Sha256};
        let old = crate::program::tests::request_with_memory_and_expectation(8192, true);
        let mut changed = crate::program::tests::request_with_memory_and_expectation(8192, false);
        // Deliberately reuse this attempt while changing its canonical request.
        changed.nonce = old.nonce;
        assert!(old.verified_program().is_ok() && changed.verified_program().is_ok());
        assert_eq!(old.nonce, changed.nonce);
        assert_eq!(old.declared_limits, changed.declared_limits);
        assert_ne!(
            Sha256::digest(old.encode_frame().unwrap()),
            Sha256::digest(changed.encode_frame().unwrap())
        );

        let (mut unit_map, mut service, unit, config) = oom_fixture();
        insert(&mut unit_map, "InvocationID", "ay", json!(vec![3; 16]));
        insert(&mut service, "ExecMainPID", "u", json!(123));
        insert(
            &mut service,
            "ExecMainStartTimestampMonotonic",
            "t",
            json!(100),
        );
        // The previous failed invocation predates this request, even if a
        // launcher exists or a race exposes old properties after preflight.
        assert_eq!(
            fresh_invocation_snapshot(&unit_map, &service, 200, 300),
            Err(ManagerError::PropertyMismatch)
        );
        assert_eq!(
            fresh_invocation_snapshot(&unit_map, &service, 100, 300),
            Err(ManagerError::PropertyMismatch)
        );
        let old_unit = unit_map.clone();
        let old_service = service.clone();
        insert(
            &mut service,
            "ExecMainStartTimestampMonotonic",
            "t",
            json!(201),
        );
        let fresh = fresh_invocation_snapshot(&unit_map, &service, 200, 300).unwrap();
        let input = "/run/sley-test-supervisor/input/abc.bin";
        assert_eq!(
            verify_reaped_oom_snapshot(&old_unit, &old_service, &unit, &config, input, fresh),
            Err(ManagerError::PropertyMismatch)
        );
        assert_eq!(
            verify_reaped_oom_snapshot(&unit_map, &service, &unit, &config, input, fresh)
                .unwrap()
                .exec_main_pid,
            123
        );
        assert_eq!(fresh.main_pid, 123);
        assert_eq!(fresh.id, [3; 16]);
        assert_eq!(
            fresh_invocation_snapshot(&unit_map, &service, 100, 200),
            Err(ManagerError::PropertyMismatch)
        );
        insert(&mut unit_map, "InvocationID", "ay", json!(vec![0; 16]));
        assert_eq!(
            fresh_invocation_snapshot(&unit_map, &service, 200, 300),
            Err(ManagerError::NotReady)
        );
        insert(&mut unit_map, "InvocationID", "ay", json!(vec![3; 15]));
        assert_eq!(
            fresh_invocation_snapshot(&unit_map, &service, 200, 300),
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

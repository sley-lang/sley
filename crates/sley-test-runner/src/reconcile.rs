//! Restart reconciliation for native worker transient units.
//!
//! Before a root supervisor accepts requests, it compares the system
//! manager's loaded units with the system.slice cgroup directory. Every exact
//! native-worker unit found in either source receives a bounded SIGKILL, then
//! must pass the manager and cgroup empty check. An unavailable source or
//! unconfirmed survivor prevents the daemon from serving new work.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::UNIT_PREFIX;
use crate::manager::{
    MAX_NATIVE_UNITS, ManagerError, confirm_system_unit_reaped, list_native_system_units,
    valid_native_unit_name,
};

/// Maximum startup time spent killing and confirming old worker units.
pub const RECONCILE_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const SYSTEM_SLICE: &str = "/sys/fs/cgroup/system.slice";

/// Startup failure that prevents new native test work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconcileError {
    /// Only the root daemon can reconcile system units.
    Unprivileged,
    /// The manager's typed listing or reap check failed.
    Manager(ManagerError),
    /// The system.slice cgroup directory is missing or unsafe.
    CgroupUnavailable,
    /// A prefixed cgroup has a malformed name or unsafe file type.
    InvalidCgroup,
    /// More than the bounded number of native worker units exist.
    TooManyUnits,
    /// The fixed systemctl SIGKILL command could not be owned to completion.
    KillFailed,
    /// Cleanup exceeded the one monotonic startup deadline.
    Deadline,
}

impl core::fmt::Display for ReconcileError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Unprivileged => "NATIVE_RECONCILE_UNPRIVILEGED",
            Self::Manager(_) => "NATIVE_RECONCILE_MANAGER_UNAVAILABLE",
            Self::CgroupUnavailable => "NATIVE_RECONCILE_CGROUP_UNAVAILABLE",
            Self::InvalidCgroup => "NATIVE_RECONCILE_CGROUP_INVALID",
            Self::TooManyUnits => "NATIVE_RECONCILE_TOO_MANY_UNITS",
            Self::KillFailed => "NATIVE_RECONCILE_KILL_FAILED",
            Self::Deadline => "NATIVE_RECONCILE_DEADLINE",
        })
    }
}

impl std::error::Error for ReconcileError {}

/// Exact units discovered and confirmed empty during startup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileReport {
    /// Sorted exact transient-unit stems considered for cleanup.
    pub units: Vec<String>,
}

fn cgroup_units_under(root: &Path, expected_uid: u32) -> Result<Vec<String>, ReconcileError> {
    let metadata = fs::symlink_metadata(root).map_err(|_| ReconcileError::CgroupUnavailable)?;
    if !metadata.is_dir() || metadata.uid() != expected_uid {
        return Err(ReconcileError::CgroupUnavailable);
    }
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(root).map_err(|_| ReconcileError::CgroupUnavailable)? {
        let entry = entry.map_err(|_| ReconcileError::CgroupUnavailable)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(ReconcileError::InvalidCgroup)?;
        if !name.starts_with(UNIT_PREFIX) {
            continue;
        }
        let stem = name
            .strip_suffix(".service")
            .ok_or(ReconcileError::InvalidCgroup)?;
        if !valid_native_unit_name(stem)
            || !entry
                .file_type()
                .map_err(|_| ReconcileError::CgroupUnavailable)?
                .is_dir()
        {
            return Err(ReconcileError::InvalidCgroup);
        }
        names.insert(stem.to_owned());
        if names.len() > MAX_NATIVE_UNITS {
            return Err(ReconcileError::TooManyUnits);
        }
    }
    Ok(names.into_iter().collect())
}

fn before(deadline: Instant) -> Result<(), ReconcileError> {
    if Instant::now() < deadline {
        Ok(())
    } else {
        Err(ReconcileError::Deadline)
    }
}

const SYSTEMCTL: &str = "/usr/bin/systemctl";

fn signal_all(units: &[String], deadline: Instant) -> Result<(), ReconcileError> {
    signal_all_with(Path::new(SYSTEMCTL), units, deadline)
}

fn signal_all_with(
    systemctl: &Path,
    units: &[String],
    deadline: Instant,
) -> Result<(), ReconcileError> {
    let mut child = Command::new(systemctl)
        .args([
            "--system",
            "--no-ask-password",
            "kill",
            "--kill-whom=all",
            "--signal=SIGKILL",
        ])
        .args(units.iter().map(|name| format!("{name}.service")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ReconcileError::KillFailed)?;
    loop {
        if before(deadline).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ReconcileError::Deadline);
        }
        match child.try_wait() {
            // A nonzero status can mean a unit disappeared between listing and
            // signaling. The independent reap check is decisive.
            Ok(Some(_)) => return Ok(()),
            Ok(None) => std::thread::sleep(POLL_INTERVAL),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ReconcileError::KillFailed);
            }
        }
    }
}

/// Kills and confirms empty every prior native worker unit before serving.
///
/// This function signs nothing and does not treat an old report as a result.
/// The manager listing and cgroup tree must both be readable even when empty.
///
/// # Errors
///
/// Refuses nonroot use, unavailable or malformed enumeration, excessive
/// units, failed kill ownership, or any unconfirmed live unit at the deadline.
pub fn reconcile_orphans() -> Result<ReconcileReport, ReconcileError> {
    if !nix::unistd::geteuid().is_root() {
        return Err(ReconcileError::Unprivileged);
    }
    let deadline = Instant::now()
        .checked_add(RECONCILE_TIMEOUT)
        .ok_or(ReconcileError::Deadline)?;
    let mut units = BTreeSet::new();
    for unit in list_native_system_units().map_err(ReconcileError::Manager)? {
        units.insert(unit);
    }
    for unit in cgroup_units_under(Path::new(SYSTEM_SLICE), 0)? {
        units.insert(unit);
    }
    if units.len() > MAX_NATIVE_UNITS {
        return Err(ReconcileError::TooManyUnits);
    }
    let units: Vec<_> = units.into_iter().collect();
    if !units.is_empty() {
        before(deadline)?;
        signal_all(&units, deadline)?;
    }
    for unit in &units {
        loop {
            before(deadline)?;
            match confirm_system_unit_reaped(unit) {
                Ok(true) => break,
                Ok(false) | Err(_) => std::thread::sleep(POLL_INTERVAL),
            }
        }
    }
    Ok(ReconcileReport { units })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn cgroup_enumeration_requires_exact_native_directories() {
        let root = std::env::temp_dir().join(format!(
            "sley-reconcile-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("root");
        let name = format!("sley-native-test-{}", "a".repeat(64));
        fs::create_dir(root.join(format!("{name}.service"))).expect("native cgroup");
        fs::create_dir(root.join("other.service")).expect("unrelated cgroup");
        let uid = nix::unistd::getuid().as_raw();
        assert_eq!(cgroup_units_under(&root, uid), Ok(vec![name]));
        fs::create_dir(root.join("sley-native-test-short.service"))
            .expect("malformed native cgroup");
        assert_eq!(
            cgroup_units_under(&root, uid),
            Err(ReconcileError::InvalidCgroup)
        );
        fs::remove_dir_all(root).expect("cleanup fixture");
    }

    fn fixture_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "sley-reconcile-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("root");
        root
    }

    #[test]
    fn missing_substituted_or_foreign_slice_is_unavailable() {
        let uid = nix::unistd::getuid().as_raw();
        let root = fixture_root("slice");
        assert_eq!(
            cgroup_units_under(&root.join("absent"), uid),
            Err(ReconcileError::CgroupUnavailable)
        );
        // A symlink to a real slice directory is not the slice.
        let real = root.join("real");
        fs::create_dir(&real).expect("real slice");
        let linked = root.join("linked");
        std::os::unix::fs::symlink(&real, &linked).expect("slice symlink");
        assert_eq!(
            cgroup_units_under(&linked, uid),
            Err(ReconcileError::CgroupUnavailable)
        );
        let file = root.join("file");
        fs::write(&file, b"").expect("regular file");
        assert_eq!(
            cgroup_units_under(&file, uid),
            Err(ReconcileError::CgroupUnavailable)
        );
        // The slice must belong to the expected owner (root in production).
        assert_eq!(
            cgroup_units_under(&real, uid.wrapping_add(1)),
            Err(ReconcileError::CgroupUnavailable)
        );
        assert_eq!(cgroup_units_under(&real, uid), Ok(Vec::new()));
        fs::remove_dir_all(root).expect("cleanup fixture");
    }

    #[test]
    fn more_native_cgroups_than_the_unit_bound_refuse() {
        let uid = nix::unistd::getuid().as_raw();
        let root = fixture_root("bound");
        for index in 0..MAX_NATIVE_UNITS {
            fs::create_dir(root.join(format!("sley-native-test-{index:064x}.service")))
                .expect("native cgroup");
        }
        assert_eq!(
            cgroup_units_under(&root, uid).map(|units| units.len()),
            Ok(MAX_NATIVE_UNITS)
        );
        fs::create_dir(root.join(format!("sley-native-test-{MAX_NATIVE_UNITS:064x}.service")))
            .expect("one more native cgroup");
        assert_eq!(
            cgroup_units_under(&root, uid),
            Err(ReconcileError::TooManyUnits)
        );
        fs::remove_dir_all(root).expect("cleanup fixture");
    }

    #[test]
    fn an_unspawnable_kill_command_fails_instead_of_skipping_cleanup() {
        let root = fixture_root("kill");
        let units = vec![format!("sley-native-test-{}", "a".repeat(64))];
        let deadline = Instant::now() + Duration::from_secs(5);
        assert_eq!(
            signal_all_with(&root.join("absent-systemctl"), &units, deadline),
            Err(ReconcileError::KillFailed)
        );
        let not_executable = root.join("systemctl");
        fs::write(&not_executable, b"#!/bin/sh\nexit 0\n").expect("script");
        assert_eq!(
            signal_all_with(&not_executable, &units, deadline),
            Err(ReconcileError::KillFailed)
        );
        fs::remove_dir_all(root).expect("cleanup fixture");
    }

    #[test]
    fn reconciliation_refuses_a_nonroot_daemon() {
        if nix::unistd::geteuid().is_root() {
            return;
        }
        assert_eq!(reconcile_orphans(), Err(ReconcileError::Unprivileged));
    }
}

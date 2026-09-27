//! Bounded cgroup-v2 telemetry for one live system transient unit.
//!
//! The manager must supply the unit's `ControlGroup` and `MainPID` after the
//! worker has started but before the daemon sends its stdin start byte. The
//! exact system.slice scope is checked before a symlink-free cgroup open.
//! Samples are taken while the worker remains alive after flushing its
//! report; cgroup files become unreadable when systemd removes the group.
//! This module reads host facts. It does not launch a unit or sign evidence.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use nix::fcntl::{OFlag, OpenHow, ResolveFlag, openat2};
use sley_tests::MemoryEvents;

use crate::config::UNIT_PREFIX;

const MAX_TELEMETRY_FILE_BYTES: u64 = 4_096;

/// Refusal to trust or read the live cgroup's host facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TelemetryError {
    /// The manager's scope is not the exact unit the daemon launched.
    ScopeMismatch,
    /// The cgroup or one required controller file is missing or unreadable.
    Unavailable,
    /// A controller value is malformed, duplicated, or oversized.
    InvalidValue,
    /// The installed memory or swap ceiling differs from the rendered unit.
    LimitMismatch,
    /// The live group does not contain exactly the manager's main PID.
    ProcessMismatch,
}

impl core::fmt::Display for TelemetryError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::ScopeMismatch => "NATIVE_TELEMETRY_SCOPE_MISMATCH",
            Self::Unavailable => "NATIVE_TELEMETRY_UNAVAILABLE",
            Self::InvalidValue => "NATIVE_TELEMETRY_INVALID_VALUE",
            Self::LimitMismatch => "NATIVE_TELEMETRY_LIMIT_MISMATCH",
            Self::ProcessMismatch => "NATIVE_TELEMETRY_PROCESS_MISMATCH",
        })
    }
}

impl std::error::Error for TelemetryError {}

/// One bounded sample while the worker's cgroup is still alive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveTelemetrySample {
    /// Peak charged bytes in the unit cgroup.
    pub memory_peak: u64,
    /// Required `memory.events` counters for the signed attestation.
    pub memory_events: MemoryEvents,
    /// Additional group-kill counter; any nonzero value blocks success.
    pub oom_group_kill: u64,
    /// Manager main PID found as the sole cgroup process.
    pub main_pid: u32,
}

impl LiveTelemetrySample {
    /// All limit and OOM events must remain zero for success.
    #[must_use]
    pub const fn events_clean(self) -> bool {
        self.memory_events.max == 0
            && self.memory_events.oom == 0
            && self.memory_events.oom_kill == 0
            && self.oom_group_kill == 0
    }
}

/// A pinned directory for the manager's exact live system-unit cgroup.
#[derive(Debug)]
pub struct LiveCgroupTelemetry {
    directory: File,
    expected_memory_cap: u64,
}

impl LiveCgroupTelemetry {
    /// Opens the root-owned cgroup-v2 scope for the exact transient unit.
    ///
    /// # Errors
    ///
    /// Refuses a substituted scope, unsafe path, non-root cgroup, or an
    /// unavailable controller directory.
    pub fn open_system_unit(
        unit_name: &str,
        control_group: &str,
        expected_memory_cap: u64,
    ) -> Result<Self, TelemetryError> {
        Self::open_from_root(
            Path::new("/sys/fs/cgroup"),
            unit_name,
            control_group,
            expected_memory_cap,
            true,
        )
    }

    pub(crate) fn open_from_root(
        root: &Path,
        unit_name: &str,
        control_group: &str,
        expected_memory_cap: u64,
        require_root_owner: bool,
    ) -> Result<Self, TelemetryError> {
        let Some(nonce) = unit_name.strip_prefix(UNIT_PREFIX) else {
            return Err(TelemetryError::ScopeMismatch);
        };
        if nonce.len() != 64
            || !nonce
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || control_group != format!("/system.slice/{unit_name}.service")
            || expected_memory_cap == 0
        {
            return Err(TelemetryError::ScopeMismatch);
        }
        let root = File::open(root).map_err(|_| TelemetryError::Unavailable)?;
        let relative = control_group.trim_start_matches('/');
        let how = OpenHow::new()
            .flags(OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC)
            .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);
        let opened = openat2(&root, relative, how).map_err(|_| TelemetryError::Unavailable)?;
        let directory = File::from(opened);
        let metadata = directory
            .metadata()
            .map_err(|_| TelemetryError::Unavailable)?;
        if !metadata.is_dir() || (require_root_owner && metadata.uid() != 0) {
            return Err(TelemetryError::Unavailable);
        }
        Ok(Self {
            directory,
            expected_memory_cap,
        })
    }

    /// Reads bounded live counters and verifies installed limits and PID.
    ///
    /// # Errors
    ///
    /// Refuses missing, malformed, substituted, or unavailable telemetry.
    pub fn sample(&self, expected_main_pid: u32) -> Result<LiveTelemetrySample, TelemetryError> {
        if expected_main_pid == 0 {
            return Err(TelemetryError::ProcessMismatch);
        }
        let memory_max = parse_decimal(&self.read_file("memory.max")?)?;
        let swap_max = parse_decimal(&self.read_file("memory.swap.max")?)?;
        if memory_max != self.expected_memory_cap || swap_max != 0 {
            return Err(TelemetryError::LimitMismatch);
        }
        let processes = self.read_file("cgroup.procs")?;
        if processes.trim_end_matches('\n') != expected_main_pid.to_string() {
            return Err(TelemetryError::ProcessMismatch);
        }
        let memory_peak = parse_decimal(&self.read_file("memory.peak")?)?;
        let event_text = self.read_file("memory.events")?;
        let counters = parse_events(&event_text)?;
        Ok(LiveTelemetrySample {
            memory_peak,
            memory_events: MemoryEvents {
                max: *counters.get("max").ok_or(TelemetryError::InvalidValue)?,
                oom: *counters.get("oom").ok_or(TelemetryError::InvalidValue)?,
                oom_kill: *counters
                    .get("oom_kill")
                    .ok_or(TelemetryError::InvalidValue)?,
            },
            oom_group_kill: counters.get("oom_group_kill").copied().unwrap_or(0),
            main_pid: expected_main_pid,
        })
    }

    fn read_file(&self, name: &str) -> Result<String, TelemetryError> {
        let how = OpenHow::new()
            .flags(OFlag::O_RDONLY | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC)
            .resolve(ResolveFlag::RESOLVE_BENEATH | ResolveFlag::RESOLVE_NO_SYMLINKS);
        let opened =
            openat2(&self.directory, name, how).map_err(|_| TelemetryError::Unavailable)?;
        let file = File::from(opened);
        if !file
            .metadata()
            .map_err(|_| TelemetryError::Unavailable)?
            .is_file()
        {
            return Err(TelemetryError::Unavailable);
        }
        let mut bytes = Vec::new();
        file.take(MAX_TELEMETRY_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| TelemetryError::Unavailable)?;
        if bytes.len() as u64 > MAX_TELEMETRY_FILE_BYTES {
            return Err(TelemetryError::InvalidValue);
        }
        String::from_utf8(bytes).map_err(|_| TelemetryError::InvalidValue)
    }
}

fn parse_decimal(value: &str) -> Result<u64, TelemetryError> {
    let text = value.strip_suffix('\n').unwrap_or(value);
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(TelemetryError::InvalidValue);
    }
    text.parse().map_err(|_| TelemetryError::InvalidValue)
}

fn parse_events(value: &str) -> Result<BTreeMap<&str, u64>, TelemetryError> {
    let mut counters = BTreeMap::new();
    for line in value.lines() {
        let mut words = line.split(' ');
        let (Some(name), Some(number), None) = (words.next(), words.next(), words.next()) else {
            return Err(TelemetryError::InvalidValue);
        };
        if name.is_empty() || counters.len() >= 16 || counters.contains_key(name) {
            return Err(TelemetryError::InvalidValue);
        }
        counters.insert(name, parse_decimal(number)?);
    }
    Ok(counters)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const UNIT: &str =
        "sley-native-test-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const GROUP: &str = "/system.slice/sley-native-test-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.service";

    struct TestCgroup(std::path::PathBuf);

    impl TestCgroup {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "sley-cgroup-telemetry-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let group = root.join(GROUP.trim_start_matches('/'));
            std::fs::create_dir_all(&group).unwrap();
            for (name, value) in [
                ("memory.max", "8192\n"),
                ("memory.swap.max", "0\n"),
                ("memory.peak", "4096\n"),
                (
                    "memory.events",
                    "low 0\nhigh 0\nmax 0\noom 0\noom_kill 0\noom_group_kill 0\n",
                ),
                ("cgroup.procs", "123\n"),
            ] {
                std::fs::write(group.join(name), value).unwrap();
            }
            Self(root)
        }

        fn set(&self, name: &str, value: &str) {
            std::fs::write(self.0.join(GROUP.trim_start_matches('/')).join(name), value).unwrap();
        }

        fn open(&self) -> Result<LiveCgroupTelemetry, TelemetryError> {
            LiveCgroupTelemetry::open_from_root(&self.0, UNIT, GROUP, 8192, false)
        }
    }

    impl Drop for TestCgroup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn live_sample_requires_exact_scope_limits_pid_and_zero_events() {
        let fixture = TestCgroup::new();
        let telemetry = fixture.open().unwrap();
        let sample = telemetry.sample(123).unwrap();
        assert_eq!(sample.memory_peak, 4096);
        assert_eq!(sample.main_pid, 123);
        assert!(sample.events_clean());
        assert_eq!(telemetry.sample(124), Err(TelemetryError::ProcessMismatch));
        fixture.set("memory.max", "8193\n");
        assert_eq!(telemetry.sample(123), Err(TelemetryError::LimitMismatch));
        fixture.set("memory.max", "8192\n");
        fixture.set(
            "memory.events",
            "max 0\noom 0\noom_kill 0\noom_group_kill 1\n",
        );
        assert!(!telemetry.sample(123).unwrap().events_clean());
    }

    #[test]
    fn malformed_and_substituted_cgroup_facts_refuse() {
        let fixture = TestCgroup::new();
        assert!(matches!(
            LiveCgroupTelemetry::open_from_root(
                &fixture.0,
                UNIT,
                "/system.slice/another.service",
                8192,
                false,
            ),
            Err(TelemetryError::ScopeMismatch)
        ));
        let telemetry = fixture.open().unwrap();
        fixture.set("memory.events", "max 0\nmax 0\noom 0\noom_kill 0\n");
        assert_eq!(telemetry.sample(123), Err(TelemetryError::InvalidValue));
        fixture.set("memory.events", "max 0\noom 0\noom_kill 0\n");
        fixture.set("memory.peak", "max\n");
        assert_eq!(telemetry.sample(123), Err(TelemetryError::InvalidValue));
        fixture.set("memory.peak", "4096\n");
        fixture.set("cgroup.procs", "123\n124\n");
        assert_eq!(telemetry.sample(123), Err(TelemetryError::ProcessMismatch));
        fixture.set("cgroup.procs", "123\n");
        let peak = fixture
            .0
            .join(GROUP.trim_start_matches('/'))
            .join("memory.peak");
        std::fs::remove_file(&peak).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", &peak).unwrap();
        assert_eq!(telemetry.sample(123), Err(TelemetryError::Unavailable));
    }
}

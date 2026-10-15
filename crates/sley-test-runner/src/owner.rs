//! One transient system unit owned from launch through confirmed reap.
//!
//! The unit is rendered afresh from administrator configuration before any
//! spawn. A process guard keeps best-effort SIGKILL cleanup armed until the
//! manager and cgroup both confirm no worker remains. Cleanup never signals a
//! unit by name: only a bound cgroup kill descriptor may be written. Missing
//! confirmation has its own error so a daemon can degrade rather than sign a result.
//! This module returns host facts; it never signs or admits a native test.

use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::channel::check_peer_connected;
use crate::config::{BinaryPinError, RunnerConfig, UNIT_PREFIX};
use crate::enforce::{REAP_BUDGET_USEC, check_elapsed};
use crate::manager::{
    ManagerError, ReapedOomWorker, WorkerInvocation, confirm_invocation_reaped, monotonic_usec,
    require_system_unit_absent_before, verify_launched_invocation_before, verify_live_invocation,
    verify_reaped_oom_unit_before,
};
use crate::phase::{GatedWorkerResult, PhaseError, run_system_unit_phase};
use crate::telemetry::LiveCgroupTelemetry;
use crate::unit::{TransientUnit, render_transient_unit};

const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Failure to own one complete host attempt safely.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerError {
    /// Only the root system supervisor may launch a worker.
    Unprivileged,
    /// The caller supplied a unit that differs from the frozen renderer.
    UnitMismatch,
    /// An installed worker or supervisor binary failed its root-owned digest pin.
    BinaryPin(BinaryPinError),
    /// Zero, overflowing, or already expired wall budget.
    InvalidDeadline,
    /// The fixed `systemd-run` process could not be spawned or piped.
    LaunchFailure,
    /// A prior unit owns this request's name; no new launcher was started.
    UnitOccupied,
    /// The manager, cgroup, gated output, or requesting peer refused.
    Phase(PhaseError),
    /// The deadline was observed with clean live counters, and the launched
    /// unit was killed and confirmed empty before returning these facts.
    MeasuredTimeout(OwnedTimeoutFacts),
    /// The exact installed unit failed with manager-observed OOM and was
    /// confirmed empty; cgroup event counters were already unavailable.
    ManagerOom(OwnedOomFacts),
    /// The requesting peer was lost after the gated phase.
    PeerLost,
    /// Worker launcher exited nonzero after a complete report.
    WorkerExit,
    /// Launch through confirmed reap reached the strict wall deadline.
    Deadline,
    /// The unit and cgroup could not be confirmed empty after cleanup.
    CleanupUnconfirmed,
}

impl core::fmt::Display for OwnerError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Unprivileged => "NATIVE_OWNER_UNPRIVILEGED",
            Self::UnitMismatch => "NATIVE_OWNER_UNIT_MISMATCH",
            Self::BinaryPin(_) => "NATIVE_OWNER_BINARY_PIN_REFUSED",
            Self::InvalidDeadline => "NATIVE_OWNER_INVALID_DEADLINE",
            Self::LaunchFailure => "NATIVE_OWNER_LAUNCH_FAILED",
            Self::UnitOccupied => "NATIVE_OWNER_UNIT_OCCUPIED",
            Self::Phase(_) => "NATIVE_OWNER_PHASE_REFUSED",
            Self::MeasuredTimeout(_) => "NATIVE_OWNER_MEASURED_TIMEOUT",
            Self::ManagerOom(_) => "NATIVE_OWNER_MANAGER_OOM",
            Self::PeerLost => "NATIVE_OWNER_PEER_LOST",
            Self::WorkerExit => "NATIVE_OWNER_WORKER_EXIT",
            Self::Deadline => "NATIVE_OWNER_DEADLINE",
            Self::CleanupUnconfirmed => "NATIVE_OWNER_CLEANUP_UNCONFIRMED",
        })
    }
}

impl std::error::Error for OwnerError {}

/// Report and live counters with a zero worker exit and confirmed empty group.
pub struct OwnedWorkerResult {
    /// Canonical pure report and final live cgroup sample.
    pub gated: GatedWorkerResult,
    /// Monotonic launch-through-reap duration in nanoseconds.
    pub elapsed_ns: u64,
}

/// A deadline snapshot captured while the worker lived, followed by verified
/// kill and reap. No worker execution report is claimed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnedTimeoutFacts {
    /// Last verified live cgroup sample at the report deadline.
    pub telemetry: crate::telemetry::LiveTelemetrySample,
    /// Monotonic launch-through-confirmed-reap duration.
    pub elapsed_ns: u64,
}

/// Trusted manager facts captured after a verified OOM and confirmed reap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnedOomFacts {
    /// Typed manager result for the exact rendered worker unit.
    pub manager: ReapedOomWorker,
    /// Monotonic launch-through-confirmed-reap duration.
    pub elapsed_ns: u64,
}

fn exact_rendered_unit(
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
    wall_ms: u64,
) -> bool {
    let Some(nonce) = unit.unit_name.strip_prefix(UNIT_PREFIX) else {
        return false;
    };
    render_transient_unit(
        config,
        nonce,
        worker_input_path,
        unit.requested_memory,
        wall_ms,
    )
    .is_ok_and(|expected| expected == *unit)
}

fn manager_owner_error(error: ManagerError) -> OwnerError {
    if error == ManagerError::CommandCleanupUnconfirmed {
        OwnerError::CleanupUnconfirmed
    } else {
        OwnerError::Phase(PhaseError::Manager(error))
    }
}

fn oom_after_cleanup(
    result: Result<ReapedOomWorker, ManagerError>,
) -> Result<Option<ReapedOomWorker>, OwnerError> {
    match result {
        Ok(facts) => Ok(Some(facts)),
        Err(ManagerError::CommandCleanupUnconfirmed) => Err(OwnerError::CleanupUnconfirmed),
        Err(_) => Ok(None),
    }
}

fn before(deadline: Instant) -> Result<(), OwnerError> {
    if Instant::now() < deadline {
        Ok(())
    } else {
        Err(OwnerError::Deadline)
    }
}

fn observe_current_invocation(
    peer: &UnixStream,
    deadline: Instant,
    mut read: impl FnMut() -> Result<WorkerInvocation, ManagerError>,
) -> Result<WorkerInvocation, OwnerError> {
    loop {
        check_peer_connected(peer).map_err(|_| OwnerError::PeerLost)?;
        before(deadline)?;
        match read() {
            Ok(invocation) => {
                before(deadline)?;
                return Ok(invocation);
            }
            Err(ManagerError::Unavailable | ManagerError::NotReady) => {
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(error) => return Err(manager_owner_error(error)),
        }
    }
}

fn wait_child(
    child: &mut Child,
    deadline: Instant,
    peer: Option<&UnixStream>,
) -> Result<std::process::ExitStatus, OwnerError> {
    loop {
        if let Some(peer) = peer {
            check_peer_connected(peer).map_err(|_| OwnerError::PeerLost)?;
        }
        before(deadline)?;
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => std::thread::sleep(POLL_INTERVAL),
            Err(_) => return Err(OwnerError::WorkerExit),
        }
    }
}

fn confirm_reap_before(
    name: &str,
    expected: WorkerInvocation,
    bound: bool,
    deadline: Instant,
) -> Result<(), OwnerError> {
    loop {
        before(deadline)?;
        match confirm_invocation_reaped(name, expected, bound, deadline) {
            Ok(true) => {
                before(deadline)?;
                return Ok(());
            }
            Ok(false) => std::thread::sleep(POLL_INTERVAL),
            Err(_) => return Err(OwnerError::CleanupUnconfirmed),
        }
    }
}

struct UnitGuard {
    child: Child,
    unit_name: String,
    reaped: bool,
    invocation: Option<WorkerInvocation>,
    telemetry: Option<LiveCgroupTelemetry>,
    cleanup_deadline: Option<Instant>,
    kill_sent: bool,
}

impl UnitGuard {
    fn spawn_for_available_unit(
        unit: &TransientUnit,
        peer: &UnixStream,
        deadline: Instant,
        check_absent: impl FnOnce(&str) -> Result<(), ManagerError>,
    ) -> Result<(Self, u64), OwnerError> {
        check_peer_connected(peer).map_err(|_| OwnerError::PeerLost)?;
        match check_absent(&unit.unit_name) {
            Ok(()) => {}
            Err(ManagerError::UnitOccupied) => return Err(OwnerError::UnitOccupied),
            Err(error) => return Err(manager_owner_error(error)),
        }
        before(deadline)?;
        let launched_after_usec =
            monotonic_usec().map_err(|error| OwnerError::Phase(PhaseError::Manager(error)))?;
        Ok((Self::spawn_for_peer(unit, peer)?, launched_after_usec))
    }

    fn spawn_for_peer(unit: &TransientUnit, peer: &UnixStream) -> Result<Self, OwnerError> {
        check_peer_connected(peer).map_err(|_| OwnerError::PeerLost)?;
        Self::spawn(unit)
    }

    fn spawn(unit: &TransientUnit) -> Result<Self, OwnerError> {
        let child = Command::new(&unit.argv[0])
            .args(&unit.argv[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| OwnerError::LaunchFailure)?;
        let guard = Self {
            child,
            unit_name: unit.unit_name.clone(),
            reaped: false,
            invocation: None,
            telemetry: None,
            cleanup_deadline: None,
            kill_sent: false,
        };
        if guard.child.stdin.is_none() || guard.child.stdout.is_none() {
            return Err(OwnerError::LaunchFailure);
        }
        Ok(guard)
    }

    fn abort(&mut self) -> Result<(), OwnerError> {
        self.abort_with(
            |telemetry, _| {
                let _ = telemetry.kill_owned();
            },
            confirm_reap_before,
        )
    }

    fn abort_with(
        &mut self,
        signal: impl FnOnce(&LiveCgroupTelemetry, Instant),
        confirm: impl FnOnce(&str, WorkerInvocation, bool, Instant) -> Result<(), OwnerError>,
    ) -> Result<(), OwnerError> {
        let deadline = if let Some(deadline) = self.cleanup_deadline {
            deadline
        } else {
            let deadline = Instant::now()
                .checked_add(Duration::from_micros(REAP_BUDGET_USEC))
                .ok_or(OwnerError::CleanupUnconfirmed)?;
            self.cleanup_deadline = Some(deadline);
            deadline
        };
        // Close both gates before stopping and positively reaping our launcher.
        self.child.stdin.take();
        self.child.stdout.take();
        let _ = self.child.kill();
        wait_child(&mut self.child, deadline, None).map_err(|_| OwnerError::CleanupUnconfirmed)?;
        let expected = self.invocation.ok_or(OwnerError::CleanupUnconfirmed)?;
        if !self.kill_sent
            && let Some(telemetry) = &self.telemetry
        {
            self.kill_sent = true;
            signal(telemetry, deadline);
        }
        confirm(
            &self.unit_name,
            expected,
            self.telemetry.is_some(),
            deadline,
        )
        .map_err(|_| OwnerError::CleanupUnconfirmed)?;
        before(deadline).map_err(|_| OwnerError::CleanupUnconfirmed)?;
        self.reaped = true;
        Ok(())
    }

    fn bind_live(
        &mut self,
        unit: &TransientUnit,
        config: &RunnerConfig,
        worker_input_path: &str,
        peer: &UnixStream,
        deadline: Instant,
    ) -> Result<(), PhaseError> {
        let invocation = self
            .invocation
            .ok_or(PhaseError::Manager(ManagerError::PropertyMismatch))?;
        let installed = loop {
            before(deadline).map_err(|_| PhaseError::Deadline)?;
            check_peer_connected(peer).map_err(PhaseError::Channel)?;
            match verify_live_invocation(unit, config, worker_input_path, invocation, deadline) {
                Ok(installed) => break installed,
                Err(ManagerError::Unavailable | ManagerError::NotReady) => {
                    std::thread::sleep(POLL_INTERVAL);
                }
                Err(error) => return Err(PhaseError::Manager(error)),
            }
        };
        let telemetry = LiveCgroupTelemetry::open_system_unit(
            &unit.unit_name,
            &installed.control_group,
            unit.installed_memory,
        )
        .map_err(PhaseError::Telemetry)?;
        telemetry
            .sample(invocation.main_pid())
            .map_err(PhaseError::Telemetry)?;
        let rechecked =
            verify_live_invocation(unit, config, worker_input_path, invocation, deadline)
                .map_err(PhaseError::Manager)?;
        if rechecked != installed {
            return Err(PhaseError::Manager(ManagerError::PropertyMismatch));
        }
        before(deadline).map_err(|_| PhaseError::Deadline)?;
        check_peer_connected(peer).map_err(PhaseError::Channel)?;
        self.telemetry = Some(telemetry);
        Ok(())
    }

    fn oom_facts(
        &self,
        unit: &TransientUnit,
        config: &RunnerConfig,
        worker_input_path: &str,
        started: Instant,
    ) -> Result<Option<OwnedOomFacts>, OwnerError> {
        let deadline = self
            .cleanup_deadline
            .ok_or(OwnerError::CleanupUnconfirmed)?;
        let invocation = self.invocation.ok_or(OwnerError::CleanupUnconfirmed)?;
        let Some(manager) = oom_after_cleanup(verify_reaped_oom_unit_before(
            unit,
            config,
            worker_input_path,
            invocation,
            deadline,
        ))?
        else {
            return Ok(None);
        };
        let elapsed_ns =
            u64::try_from(started.elapsed().as_nanos()).map_err(|_| OwnerError::InvalidDeadline)?;
        before(deadline)?;
        Ok(Some(OwnedOomFacts {
            manager,
            elapsed_ns,
        }))
    }

    fn finish(&mut self, peer: &UnixStream, deadline: Instant) -> Result<(), OwnerError> {
        self.child.stdin.take();
        let status = wait_child(&mut self.child, deadline, Some(peer))?;
        if !status.success() {
            return Err(OwnerError::WorkerExit);
        }
        loop {
            check_peer_connected(peer).map_err(|_| OwnerError::PeerLost)?;
            before(deadline)?;
            match confirm_invocation_reaped(
                &self.unit_name,
                self.invocation.ok_or(OwnerError::CleanupUnconfirmed)?,
                self.telemetry.is_some(),
                deadline,
            ) {
                Ok(true) => {
                    before(deadline)?;
                    self.reaped = true;
                    return Ok(());
                }
                Ok(false) => std::thread::sleep(POLL_INTERVAL),
                Err(_) => return Err(OwnerError::CleanupUnconfirmed),
            }
        }
    }
}

impl Drop for UnitGuard {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.abort();
        }
    }
}

/// Owns one system transient unit from launch through confirmed exit/reap.
///
/// This does not stage the request, bind a report to the request, or sign a
/// measurement. The root daemon must do those checks around this call and
/// degrade on `CleanupUnconfirmed`. Installed binary digests are pinned
/// before launch.
///
/// # Errors
///
/// Refuses an unprivileged caller, altered unit, launch/phase failure,
/// nonzero worker exit, deadline, peer loss, or unconfirmed cleanup.
pub fn run_owned_system_unit(
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
    peer: &UnixStream,
    wall_ms: u64,
) -> Result<OwnedWorkerResult, OwnerError> {
    if !nix::unistd::geteuid().is_root() {
        return Err(OwnerError::Unprivileged);
    }
    if !exact_rendered_unit(unit, config, worker_input_path, wall_ms) {
        return Err(OwnerError::UnitMismatch);
    }
    config
        .verify_installed_binaries()
        .map_err(OwnerError::BinaryPin)?;
    let started = Instant::now();
    let deadline = started
        .checked_add(Duration::from_millis(wall_ms))
        .ok_or(OwnerError::InvalidDeadline)?;
    let (mut guard, launched_after_usec) =
        UnitGuard::spawn_for_available_unit(unit, peer, deadline, |name| {
            require_system_unit_absent_before(name, deadline)
        })?;
    let invocation = match observe_current_invocation(peer, deadline, || {
        verify_launched_invocation_before(
            unit,
            config,
            worker_input_path,
            launched_after_usec,
            deadline,
        )
    }) {
        Ok(invocation) => invocation,
        Err(error) => {
            // A launcher process is not proof that systemd created a worker.
            // Without a fresh manager invocation, never inspect or sign an
            // old unit's retained OOM facts, even after confirmed cleanup.
            guard.abort()?;
            return Err(error);
        }
    };
    guard.invocation = Some(invocation);
    let phase = (|| {
        guard.bind_live(unit, config, worker_input_path, peer, deadline)?;
        let control = guard.child.stdin.as_mut().ok_or(PhaseError::GateWrite)?;
        let output = guard.child.stdout.as_mut().ok_or(PhaseError::GateWrite)?;
        run_system_unit_phase(
            guard.telemetry.as_ref().ok_or(PhaseError::GateWrite)?,
            invocation.main_pid(),
            output,
            control,
            peer,
            deadline,
        )
    })();
    let gated = match phase {
        Ok(gated) => gated,
        Err(PhaseError::Manager(ManagerError::CommandCleanupUnconfirmed)) => {
            let _ = guard.abort();
            return Err(OwnerError::CleanupUnconfirmed);
        }
        Err(PhaseError::MeasuredTimeout(telemetry)) => {
            guard.abort()?;
            let elapsed_ns = u64::try_from(started.elapsed().as_nanos())
                .map_err(|_| OwnerError::InvalidDeadline)?;
            return Err(OwnerError::MeasuredTimeout(OwnedTimeoutFacts {
                telemetry,
                elapsed_ns,
            }));
        }
        Err(error) => {
            guard.abort()?;
            if let Some(facts) = guard.oom_facts(unit, config, worker_input_path, started)? {
                return Err(OwnerError::ManagerOom(facts));
            }
            return Err(OwnerError::Phase(error));
        }
    };
    if let Err(error) = guard.finish(peer, deadline) {
        if error == OwnerError::CleanupUnconfirmed {
            let _ = guard.abort();
            return Err(error);
        }
        guard.abort()?;
        if let Some(facts) = guard.oom_facts(unit, config, worker_input_path, started)? {
            return Err(OwnerError::ManagerOom(facts));
        }
        return Err(error);
    }
    let elapsed_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| OwnerError::InvalidDeadline)?;
    check_elapsed(elapsed_ns, wall_ms).map_err(|_| OwnerError::Deadline)?;
    Ok(OwnedWorkerResult { gated, elapsed_ns })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use sley_id::{PrincipalId, WorkspaceId};

    use super::*;
    use crate::config::{AllowedCaller, default_config};

    static NEXT_MARKER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn prelaunch_manager_child_unknown_preserves_cleanup_degradation() {
        let unit = TransientUnit {
            unit_name: format!("{UNIT_PREFIX}{}", "e".repeat(64)),
            argv: vec!["/unreachable-no-launch".to_owned()],
            requested_memory: 4096,
            installed_memory: 4096,
            runtime_max_usec: 3_000_000,
        };
        let (peer, _client) = UnixStream::pair().unwrap();
        let result = UnitGuard::spawn_for_available_unit(
            &unit,
            &peer,
            Instant::now() + Duration::from_secs(1),
            |_| Err(ManagerError::CommandCleanupUnconfirmed),
        );
        assert!(matches!(result, Err(OwnerError::CleanupUnconfirmed)));
    }

    #[test]
    fn oom_diagnostic_cannot_erase_an_unknown_manager_child() {
        assert_eq!(
            oom_after_cleanup(Err(ManagerError::CommandCleanupUnconfirmed)),
            Err(OwnerError::CleanupUnconfirmed)
        );
        assert_eq!(oom_after_cleanup(Err(ManagerError::Unavailable)), Ok(None));
    }

    #[test]
    fn late_cleanup_confirmation_never_marks_the_guard_reaped() {
        let mut guard = std::mem::ManuallyDrop::new(harmless_guard());
        guard.invocation = Some(WorkerInvocation::fixture());
        guard.cleanup_deadline = Some(Instant::now() + Duration::from_millis(20));
        let result = guard.abort_with(
            |_, _| panic!("unbound"),
            |_, _, _, _| {
                std::thread::sleep(Duration::from_millis(40));
                Ok(())
            },
        );
        let claimed_reaped = guard.reaped;
        guard.reaped = true;
        drop(std::mem::ManuallyDrop::into_inner(guard));
        assert_eq!(result, Err(OwnerError::CleanupUnconfirmed));
        assert!(!claimed_reaped);
    }

    fn harmless_guard() -> UnitGuard {
        UnitGuard::spawn(&TransientUnit {
            unit_name: format!("{UNIT_PREFIX}{}", "d".repeat(64)),
            argv: vec!["/usr/bin/sleep".to_owned(), "1".to_owned()],
            requested_memory: 4096,
            installed_memory: 4096,
            runtime_max_usec: 3_000_000,
        })
        .unwrap()
    }

    #[test]
    fn launcher_only_drop_reaps_child_without_manager_access() {
        let guard = harmless_guard();
        let child = guard.child.id();
        drop(guard);
        assert!(!std::path::Path::new(&format!("/proc/{child}")).exists());
    }

    #[test]
    fn bound_cleanup_writes_held_control_once_and_shares_its_deadline() {
        let root = std::env::temp_dir().join(format!(
            "sley-owned-kill-{}-{}",
            std::process::id(),
            NEXT_MARKER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut guard = std::mem::ManuallyDrop::new(harmless_guard());
        let group = format!("/system.slice/{}.service", guard.unit_name);
        let directory = root.join(group.trim_start_matches('/'));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("cgroup.type"), "domain\n").unwrap();
        fs::write(directory.join("cgroup.kill"), "0").unwrap();
        let mut telemetry =
            LiveCgroupTelemetry::open_from_root(&root, &guard.unit_name, &group, 4096, false)
                .unwrap();
        telemetry.arm_fixture_kill().unwrap();
        guard.invocation = Some(WorkerInvocation::fixture());
        guard.telemetry = Some(telemetry);
        let first = guard.abort_with(
            |held, _| held.kill_owned().unwrap(),
            |_, _, bound, _| {
                assert!(bound);
                Err(OwnerError::CleanupUnconfirmed)
            },
        );
        let deadline = guard.cleanup_deadline;
        let second = guard.abort_with(
            |_, _| panic!("cleanup must not send a second kill"),
            |_, _, bound, actual| {
                assert!(bound);
                assert_eq!(Some(actual), deadline);
                Ok(())
            },
        );
        let reaped = guard.reaped;
        guard.reaped = true;
        drop(std::mem::ManuallyDrop::into_inner(guard));
        assert_eq!(first, Err(OwnerError::CleanupUnconfirmed));
        assert_eq!(second, Ok(()));
        assert!(reaped);
        assert_eq!(fs::read(directory.join("cgroup.kill")).unwrap(), b"1");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn observed_terminal_invocation_needs_no_live_kill_handle() {
        let mut guard = std::mem::ManuallyDrop::new(harmless_guard());
        let expected = WorkerInvocation::fixture();
        guard.invocation = Some(expected);
        let result = guard.abort_with(
            |_, _| panic!("no live cgroup may be signalled"),
            |_, actual, bound, _| {
                assert_eq!(actual, expected);
                assert!(!bound);
                Ok(())
            },
        );
        let reaped = guard.reaped;
        guard.reaped = true;
        drop(std::mem::ManuallyDrop::into_inner(guard));
        assert_eq!(result, Ok(()));
        assert!(reaped);
    }

    #[test]
    fn observed_invocation_with_unknown_or_replaced_terminal_state_stays_unconfirmed() {
        let mut guard = std::mem::ManuallyDrop::new(harmless_guard());
        guard.invocation = Some(WorkerInvocation::fixture());
        let result = guard.abort_with(
            |_, _| panic!("unbound cleanup cannot signal"),
            |_, _, _, _| Err(OwnerError::CleanupUnconfirmed),
        );
        let reaped = guard.reaped;
        guard.reaped = true;
        drop(std::mem::ManuallyDrop::into_inner(guard));
        assert_eq!(result, Err(OwnerError::CleanupUnconfirmed));
        assert!(!reaped);
    }

    #[test]
    fn rejected_late_occupant_is_never_signalled_or_claimed_reaped() {
        // The real launcher is harmless, has no descendants, and self-expires
        // even if this test panics before its explicit kill/wait cleanup.
        let unit = TransientUnit {
            unit_name: format!("{UNIT_PREFIX}{}", "c".repeat(64)),
            argv: vec!["/usr/bin/sleep".to_owned(), "1".to_owned()],
            requested_memory: 4096,
            installed_memory: 4096,
            runtime_max_usec: 3_000_000,
        };
        let (peer, _client) = UnixStream::pair().unwrap();
        let (guard, _) = UnitGuard::spawn_for_available_unit(
            &unit,
            &peer,
            Instant::now() + Duration::from_secs(1),
            |_| Ok(()),
        )
        .expect("absence snapshot permits the harmless owned launcher");
        // Never run the production Drop fallback against a host manager,
        // including when a regression assertion panics.
        let mut guard = std::mem::ManuallyDrop::new(guard);
        let invocation =
            observe_current_invocation(&peer, Instant::now() + Duration::from_secs(1), || {
                Err(ManagerError::PropertyMismatch)
            });
        let mut signalled_names = Vec::new();
        let result = match invocation {
            Ok(_) => panic!("late independent occupant must not verify"),
            Err(_) => guard.abort_with(
                |_, _| signalled_names.push(unit.unit_name.clone()),
                // The independently owned manager unit remains occupied.
                |_, _, _, _| Err(OwnerError::CleanupUnconfirmed),
            ),
        };
        let claimed_reaped = guard.reaped;
        let launcher_reaped = guard.child.try_wait().unwrap().is_some();
        // This disarms only the test fixture's production Drop fallback,
        // after retaining its actual cleanup state and reaping our child.
        guard.reaped = true;
        drop(std::mem::ManuallyDrop::into_inner(guard));

        assert!(launcher_reaped, "the owned launcher must still be reaped");
        assert_eq!(result, Err(OwnerError::CleanupUnconfirmed));
        assert!(
            !claimed_reaped,
            "an unknown manager unit is not owned cleanup"
        );
        assert!(
            signalled_names.is_empty(),
            "cleanup signalled an unowned late occupant: {signalled_names:?}"
        );
    }

    #[test]
    fn retained_failed_unit_prevents_a_new_launcher_and_cleanup_guard() {
        let marker = std::env::temp_dir().join(format!(
            "sley-occupied-unit-{}-{}",
            std::process::id(),
            NEXT_MARKER.fetch_add(1, Ordering::Relaxed),
        ));
        let unit = TransientUnit {
            unit_name: format!("{UNIT_PREFIX}{}", "b".repeat(64)),
            argv: vec![
                "/usr/bin/touch".to_owned(),
                marker.to_str().unwrap().to_owned(),
            ],
            requested_memory: 4096,
            installed_memory: 4096,
            runtime_max_usec: 3_000_000,
        };
        let (peer, _client) = UnixStream::pair().unwrap();
        let result = UnitGuard::spawn_for_available_unit(
            &unit,
            &peer,
            Instant::now() + Duration::from_secs(1),
            |name| {
                assert_eq!(name, unit.unit_name);
                Err(ManagerError::UnitOccupied)
            },
        );
        assert!(matches!(result, Err(OwnerError::UnitOccupied)));
        assert!(!marker.exists());
    }

    #[test]
    fn launcher_without_a_fresh_manager_invocation_never_produces_oom_facts() {
        let (peer, _client) = UnixStream::pair().unwrap();
        assert_eq!(
            observe_current_invocation(&peer, Instant::now() + Duration::from_secs(1), || {
                Err(ManagerError::PropertyMismatch)
            }),
            Err(OwnerError::Phase(PhaseError::Manager(
                ManagerError::PropertyMismatch
            )))
        );
        assert_eq!(
            observe_current_invocation(&peer, Instant::now() + Duration::from_millis(10), || {
                Err(ManagerError::Unavailable)
            }),
            Err(OwnerError::Deadline)
        );
    }

    #[test]
    fn disconnected_peer_never_starts_launcher() {
        let marker = std::env::temp_dir().join(format!(
            "sley-disconnected-peer-{}-{}",
            std::process::id(),
            NEXT_MARKER.fetch_add(1, Ordering::Relaxed)
        ));
        assert!(!marker.exists());
        let unit = TransientUnit {
            unit_name: format!("{UNIT_PREFIX}{}", "b".repeat(64)),
            argv: vec![
                "/usr/bin/touch".to_owned(),
                marker.to_str().unwrap().to_owned(),
            ],
            requested_memory: 4096,
            installed_memory: 4096,
            runtime_max_usec: 3_000_000,
        };
        let (peer, client) = UnixStream::pair().unwrap();
        drop(client);
        let result = UnitGuard::spawn_for_peer(&unit, &peer);
        let error = match result {
            Ok(mut guard) => {
                guard.child.wait().unwrap();
                guard.reaped = true;
                None
            }
            Err(error) => Some(error),
        };
        let launched = marker.exists();
        if launched {
            fs::remove_file(marker).unwrap();
        }
        assert_eq!(error, Some(OwnerError::PeerLost));
        assert!(!launched);
    }

    #[test]
    fn refuses_every_change_to_the_frozen_launch_argv() {
        let config = default_config(
            "/run/sley-test-supervisor",
            "/usr/lib/sley/sley-native-test-worker",
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
        .unwrap();
        let input = "/run/sley-test-supervisor/input/a.bin";
        let mut unit = render_transient_unit(&config, &"a".repeat(64), input, 8192, 1000).unwrap();
        assert!(exact_rendered_unit(&unit, &config, input, 1000));
        unit.argv.push("--property=NoNewPrivileges=no".to_owned());
        assert!(!exact_rendered_unit(&unit, &config, input, 1000));
        unit.argv.pop();
        assert!(!exact_rendered_unit(&unit, &config, input, 999));
    }

    #[test]
    fn child_wait_has_a_strict_deadline_and_peer_watch() {
        let (peer, client) = UnixStream::pair().unwrap();
        let mut child = Command::new("/usr/bin/sleep").arg("1").spawn().unwrap();
        assert_eq!(
            wait_child(&mut child, Instant::now(), Some(&peer)),
            Err(OwnerError::Deadline)
        );
        drop(client);
        assert_eq!(
            wait_child(
                &mut child,
                Instant::now() + Duration::from_secs(1),
                Some(&peer)
            ),
            Err(OwnerError::PeerLost)
        );
        child.kill().unwrap();
        child.wait().unwrap();
    }
}

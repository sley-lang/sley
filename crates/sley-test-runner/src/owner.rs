//! One transient system unit owned from launch through confirmed reap.
//!
//! The unit is rendered afresh from administrator configuration before any
//! spawn. A process guard keeps best-effort SIGKILL cleanup armed until the
//! manager and cgroup both confirm no worker remains. Missing confirmation
//! has its own error so a daemon can degrade rather than sign a result.
//! This module returns host facts; it never signs or admits a native test.

use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::channel::check_peer_connected;
use crate::config::{BinaryPinError, RunnerConfig, UNIT_PREFIX};
use crate::enforce::{REAP_BUDGET_USEC, check_elapsed};
use crate::manager::confirm_system_unit_reaped;
use crate::phase::{GatedWorkerResult, PhaseError, run_system_unit_phase};
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
    /// The manager, cgroup, gated output, or requesting peer refused.
    Phase(PhaseError),
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
            Self::Phase(_) => "NATIVE_OWNER_PHASE_REFUSED",
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

fn before(deadline: Instant) -> Result<(), OwnerError> {
    if Instant::now() < deadline {
        Ok(())
    } else {
        Err(OwnerError::Deadline)
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

fn signal_system_unit(unit_name: &str, deadline: Instant) {
    let service = format!("{unit_name}.service");
    let Ok(mut command) = Command::new("/usr/bin/systemctl")
        .args([
            "--system",
            "--no-ask-password",
            "kill",
            "--kill-whom=all",
            "--signal=SIGKILL",
            &service,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    if wait_child(&mut command, deadline, None).is_err() {
        let _ = command.kill();
        let _ = command.wait();
    }
}

fn confirm_reap_before(unit_name: &str, deadline: Instant) -> Result<(), OwnerError> {
    loop {
        before(deadline)?;
        match confirm_system_unit_reaped(unit_name) {
            Ok(true) => return Ok(()),
            Ok(false) | Err(_) => std::thread::sleep(POLL_INTERVAL),
        }
    }
}

struct UnitGuard {
    child: Child,
    unit_name: String,
    reaped: bool,
}

impl UnitGuard {
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
        };
        if guard.child.stdin.is_none() || guard.child.stdout.is_none() {
            return Err(OwnerError::LaunchFailure);
        }
        Ok(guard)
    }

    fn abort(&mut self) -> Result<(), OwnerError> {
        let deadline = Instant::now()
            .checked_add(Duration::from_micros(REAP_BUDGET_USEC))
            .ok_or(OwnerError::CleanupUnconfirmed)?;
        signal_system_unit(&self.unit_name, deadline);
        let _ = self.child.kill();
        let _ = wait_child(&mut self.child, deadline, None);
        confirm_reap_before(&self.unit_name, deadline)
            .map_err(|_| OwnerError::CleanupUnconfirmed)?;
        self.reaped = true;
        Ok(())
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
            match confirm_system_unit_reaped(&self.unit_name) {
                Ok(true) => {
                    self.reaped = true;
                    return Ok(());
                }
                Ok(false) | Err(_) => std::thread::sleep(POLL_INTERVAL),
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
    let mut guard = UnitGuard::spawn_for_peer(unit, peer)?;
    let phase = {
        let control = guard
            .child
            .stdin
            .as_mut()
            .ok_or(OwnerError::LaunchFailure)?;
        let output = guard
            .child
            .stdout
            .as_mut()
            .ok_or(OwnerError::LaunchFailure)?;
        run_system_unit_phase(
            unit,
            config,
            worker_input_path,
            output,
            control,
            peer,
            deadline,
        )
    };
    let gated = match phase {
        Ok(gated) => gated,
        Err(error) => {
            guard.abort()?;
            return Err(OwnerError::Phase(error));
        }
    };
    if let Err(error) = guard.finish(peer, deadline) {
        guard.abort()?;
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

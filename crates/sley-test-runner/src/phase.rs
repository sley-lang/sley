//! Ordered worker gate, canonical report, and live cgroup sampling.
//!
//! A system unit must already be launched and owned by the caller. The
//! production entry verifies the manager's installed properties and opens
//! its exact cgroup before it sends the start byte. The worker remains alive
//! after its report, allowing a final telemetry sample before release. The
//! caller still owns worker exit, empty-cgroup confirmation, cleanup, and
//! measurement signing; this phase cannot claim native test admission.

use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use sley_tests::NativeExecutionReportV1;

use crate::channel::{
    ChannelError, check_peer_connected, read_report_before, require_output_eof_before,
};
use crate::config::RunnerConfig;
use crate::manager::{ManagerError, verify_system_unit};
use crate::telemetry::{LiveCgroupTelemetry, LiveTelemetrySample, TelemetryError};
use crate::unit::TransientUnit;
use crate::worker::{WORKER_RELEASE_GATE, WORKER_START_GATE};

/// Failure before the host attempt can be admitted as measured evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseError {
    /// Only the root system supervisor may open this production phase.
    Unprivileged,
    /// The manager's typed installed unit is absent or inconsistent.
    Manager(ManagerError),
    /// The exact live cgroup could not be trusted or read.
    Telemetry(TelemetryError),
    /// A limit, OOM, or group-kill event occurred before or during execution.
    DirtyEvents,
    /// The report deadline elapsed while the exact worker PID and clean
    /// counters were still observable in its verified cgroup.
    MeasuredTimeout(LiveTelemetrySample),
    /// The daemon deadline elapsed before the phase completed.
    Deadline,
    /// Worker stdin gate could not be sent.
    GateWrite,
    /// Worker output or requesting peer failed the closed channel.
    Channel(ChannelError),
}

impl core::fmt::Display for PhaseError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Unprivileged => "NATIVE_PHASE_UNPRIVILEGED",
            Self::Manager(_) => "NATIVE_PHASE_MANAGER_REFUSED",
            Self::Telemetry(_) => "NATIVE_PHASE_TELEMETRY_REFUSED",
            Self::DirtyEvents => "NATIVE_PHASE_MEMORY_EVENTS",
            Self::MeasuredTimeout(_) => "NATIVE_PHASE_MEASURED_TIMEOUT",
            Self::Deadline => "NATIVE_PHASE_DEADLINE",
            Self::GateWrite => "NATIVE_PHASE_GATE_WRITE",
            Self::Channel(_) => "NATIVE_PHASE_CHANNEL_REFUSED",
        })
    }
}

impl std::error::Error for PhaseError {}

/// Complete pure worker report and final live cgroup counters.
pub struct GatedWorkerResult {
    /// Canonical report from the native Sley worker.
    pub report: NativeExecutionReportV1,
    /// Last live sample, while the worker held its cgroup open.
    pub telemetry: LiveTelemetrySample,
}

fn check_deadline(deadline: Instant) -> Result<(), PhaseError> {
    if Instant::now() >= deadline {
        Err(PhaseError::Deadline)
    } else {
        Ok(())
    }
}

fn run_gated_phase<Output: AsFd + Read, Control: Write>(
    telemetry: &LiveCgroupTelemetry,
    main_pid: u32,
    output: &mut Output,
    control: &mut Control,
    peer: &UnixStream,
    deadline: Instant,
) -> Result<GatedWorkerResult, PhaseError> {
    let initial = telemetry.sample(main_pid).map_err(PhaseError::Telemetry)?;
    if !initial.events_clean() {
        return Err(PhaseError::DirtyEvents);
    }
    check_peer_connected(peer).map_err(PhaseError::Channel)?;
    check_deadline(deadline)?;
    control
        .write_all(&[WORKER_START_GATE])
        .map_err(|_| PhaseError::GateWrite)?;
    let report = match read_report_before(output, peer, deadline) {
        Ok(report) => report,
        Err(ChannelError::Deadline) => {
            let sample = telemetry.sample(main_pid).map_err(PhaseError::Telemetry)?;
            if !sample.events_clean() {
                return Err(PhaseError::DirtyEvents);
            }
            return Err(PhaseError::MeasuredTimeout(sample));
        }
        Err(error) => return Err(PhaseError::Channel(error)),
    };
    let final_sample = telemetry.sample(main_pid).map_err(PhaseError::Telemetry)?;
    if !final_sample.events_clean() {
        return Err(PhaseError::DirtyEvents);
    }
    check_peer_connected(peer).map_err(PhaseError::Channel)?;
    check_deadline(deadline)?;
    control
        .write_all(&[WORKER_RELEASE_GATE])
        .map_err(|_| PhaseError::GateWrite)?;
    require_output_eof_before(output, peer, deadline).map_err(PhaseError::Channel)?;
    Ok(GatedWorkerResult {
        report,
        telemetry: final_sample,
    })
}

/// Runs the gated I/O phase for an already-owned system transient unit.
///
/// The caller must keep a kill-and-reap guard active across this call. Any
/// error after launch leaves the unit requiring explicit cleanup; even a
/// successful phase requires confirmed worker exit and an empty group before
/// the report may be measured or signed.
///
/// # Errors
///
/// Refuses a nonroot caller, manager or cgroup mismatch, dirty events, lost
/// peer, deadline, malformed output, or failed control gate.
pub fn run_system_unit_phase<Output: AsFd + Read, Control: Write>(
    unit: &TransientUnit,
    config: &RunnerConfig,
    worker_input_path: &str,
    output: &mut Output,
    control: &mut Control,
    peer: &UnixStream,
    deadline: Instant,
) -> Result<GatedWorkerResult, PhaseError> {
    if !nix::unistd::geteuid().is_root() {
        return Err(PhaseError::Unprivileged);
    }
    check_deadline(deadline)?;
    check_peer_connected(peer).map_err(PhaseError::Channel)?;
    let installed = loop {
        check_deadline(deadline)?;
        check_peer_connected(peer).map_err(PhaseError::Channel)?;
        match verify_system_unit(unit, config, worker_input_path) {
            Ok(installed) => break installed,
            Err(ManagerError::Unavailable | ManagerError::NotReady) => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(PhaseError::Manager(error)),
        }
    };
    check_deadline(deadline)?;
    let telemetry = LiveCgroupTelemetry::open_system_unit(
        &unit.unit_name,
        &installed.control_group,
        unit.installed_memory,
    )
    .map_err(PhaseError::Telemetry)?;
    run_gated_phase(
        &telemetry,
        installed.main_pid,
        output,
        control,
        peer,
        deadline,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{ErrorKind, Read as _, Write as _};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::Duration;

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const UNIT: &str =
        "sley-native-test-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const GROUP: &str = "/system.slice/sley-native-test-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.service";
    const REPORT: &[u8] =
        include_bytes!("../../../conformance/native-worker/v1/observed-report.bin");

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "sley-gated-phase-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let group = root.join(GROUP.trim_start_matches('/'));
            fs::create_dir_all(&group).unwrap();
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
                fs::write(group.join(name), value).unwrap();
            }
            Self(root)
        }

        fn telemetry(&self) -> LiveCgroupTelemetry {
            LiveCgroupTelemetry::open_from_root(&self.0, UNIT, GROUP, 8192, false).unwrap()
        }

        fn dirty_events(&self) {
            fs::write(
                self.0
                    .join(GROUP.trim_start_matches('/'))
                    .join("memory.events"),
                "max 1\noom 0\noom_kill 0\noom_group_kill 0\n",
            )
            .unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn gates_execution_around_a_complete_report_and_live_samples() {
        let fixture = Fixture::new();
        let telemetry = fixture.telemetry();
        let (mut control, mut worker_control) = UnixStream::pair().unwrap();
        let (mut output, mut worker_output) = UnixStream::pair().unwrap();
        let (peer, client) = UnixStream::pair().unwrap();
        let worker = thread::spawn(move || {
            let mut gate = [0_u8; 1];
            worker_control.read_exact(&mut gate).unwrap();
            assert_eq!(gate[0], WORKER_START_GATE);
            worker_output.write_all(REPORT).unwrap();
            worker_control.read_exact(&mut gate).unwrap();
            assert_eq!(gate[0], WORKER_RELEASE_GATE);
        });
        let result = run_gated_phase(
            &telemetry,
            123,
            &mut output,
            &mut control,
            &peer,
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(result.report.stored_bytes(), REPORT);
        assert_eq!(result.telemetry.memory_peak, 4096);
        worker.join().unwrap();
        drop(client);
    }

    #[test]
    fn report_deadline_carries_only_a_verified_live_snapshot() {
        let fixture = Fixture::new();
        let telemetry = fixture.telemetry();
        let (mut control, _worker_control) = UnixStream::pair().unwrap();
        let (mut output, _worker_output) = UnixStream::pair().unwrap();
        let (peer, _client) = UnixStream::pair().unwrap();
        let outcome = run_gated_phase(
            &telemetry,
            123,
            &mut output,
            &mut control,
            &peer,
            Instant::now() + Duration::from_millis(25),
        );
        match outcome {
            Err(PhaseError::MeasuredTimeout(sample)) => {
                assert_eq!(sample.main_pid, 123);
                assert_eq!(sample.memory_peak, 4096);
                assert!(sample.events_clean());
            }
            _ => panic!("expected a measured deadline without worker output"),
        }
    }

    #[test]
    fn dirty_baseline_refuses_before_start_and_dirty_final_refuses_release() {
        let fixture = Fixture::new();
        let telemetry = fixture.telemetry();
        fixture.dirty_events();
        let (mut control, mut worker_control) = UnixStream::pair().unwrap();
        let (mut output, _worker_output) = UnixStream::pair().unwrap();
        let (peer, _client) = UnixStream::pair().unwrap();
        assert_eq!(
            run_gated_phase(
                &telemetry,
                123,
                &mut output,
                &mut control,
                &peer,
                Instant::now() + Duration::from_secs(1)
            )
            .err(),
            Some(PhaseError::DirtyEvents)
        );
        worker_control.set_nonblocking(true).unwrap();
        assert_eq!(
            worker_control.read(&mut [0_u8; 1]).unwrap_err().kind(),
            ErrorKind::WouldBlock
        );

        fs::write(
            fixture
                .0
                .join(GROUP.trim_start_matches('/'))
                .join("memory.events"),
            "max 0\noom 0\noom_kill 0\noom_group_kill 0\n",
        )
        .unwrap();
        let telemetry = fixture.telemetry();
        let group = fixture.0.join(GROUP.trim_start_matches('/'));
        let (mut control, mut worker_control) = UnixStream::pair().unwrap();
        let (mut output, mut worker_output) = UnixStream::pair().unwrap();
        let worker = thread::spawn(move || {
            let mut gate = [0_u8; 1];
            worker_control.read_exact(&mut gate).unwrap();
            assert_eq!(gate[0], WORKER_START_GATE);
            fs::write(
                group.join("memory.events"),
                "max 1\noom 0\noom_kill 0\noom_group_kill 0\n",
            )
            .unwrap();
            worker_output.write_all(REPORT).unwrap();
            worker_control
                .set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            assert!(worker_control.read_exact(&mut gate).is_err());
        });
        assert_eq!(
            run_gated_phase(
                &telemetry,
                123,
                &mut output,
                &mut control,
                &peer,
                Instant::now() + Duration::from_secs(1)
            )
            .err(),
            Some(PhaseError::DirtyEvents)
        );
        worker.join().unwrap();
    }
}

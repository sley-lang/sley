//! Frozen systemd transient-unit rendering.
//!
//! The daemon never accepts caller-chosen unit properties: every transient
//! unit is rendered from administrator configuration plus the admitted run
//! ceilings through this module. The 14-property required projection plus
//! the two resource-instantiated properties match `SLEYNHC1`
//! (`SupervisorConfigV1`); unknown, missing, or extra properties refuse
//! under that profile rather than relying on manager defaults.
//!
//! The two resource-instantiated properties use normalized values
//! `MemoryMax=<page-floored>` and `RuntimeMaxUSec=<wall-plus-cleanup>`.
//! Binary/input/output mapping is fixed `launch_profile1` and cannot be
//! supplied by the caller: the worker binary and the daemon-owned input
//! bind read-only, output goes to a daemon-owned bounded channel, and the
//! unit binds to the supervisor service lifetime.

use sley_tests::supervisor::SUPERVISOR_CLEANUP_MILLIS;

use crate::config::{RunnerConfig, UNIT_PREFIX};
use crate::enforce::{EnforceError, floor_page_cap, runtime_max_usec};

/// Fixed launch-profile identity; the only mapping the daemon renders.
pub const LAUNCH_PROFILE: u32 = 1;
/// Manager stop timeout in microseconds; matches the attested projection.
pub const TIMEOUT_STOP_USEC: u64 = SUPERVISOR_CLEANUP_MILLIS * 1_000;
/// Maximum worker tasks; matches the attested projection.
pub const TASKS_MAX: u64 = 1;

/// Frozen required unit properties in canonical order, before the two
/// resource-instantiated entries.
pub const REQUIRED_PROPERTIES: [(&str, &str); 14] = [
    ("CapabilityBoundingSet", "empty"),
    ("DynamicUser", "yes"),
    ("KillMode", "control-group"),
    ("MemoryAccounting", "yes"),
    ("MemorySwapMax", "0"),
    ("NoNewPrivileges", "yes"),
    ("PrivateNetwork", "yes"),
    ("PrivateTmp", "yes"),
    ("ProtectControlGroups", "yes"),
    ("ProtectHome", "yes"),
    ("ProtectSystem", "strict"),
    ("SendSIGKILL", "yes"),
    ("TasksMax", "1"),
    ("TimeoutStopUSec", "2000000"),
];

/// One rendered transient unit: the exact manager argv plus the installed
/// ceilings bound into the run attestation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransientUnit {
    /// Manager unit name with the fixed orphan-reconciliation prefix.
    pub unit_name: String,
    /// Exact `systemd-run --system` argv, properties in canonical order.
    pub argv: Vec<String>,
    /// Requested memory ceiling from the admitted run.
    pub requested_memory: u64,
    /// Page-floored cap installed as `MemoryMax`.
    pub installed_memory: u64,
    /// Manager runtime backstop installed as `RuntimeMaxUSec`.
    pub runtime_max_usec: u64,
}

/// Renders one closed transient unit for an admitted run.
///
/// # Errors
///
/// Returns the first stable enforcement refusal for zero/overflowing wall
/// budgets or unaccommodating memory caps; no unit renders on refusal.
pub fn render_transient_unit(
    config: &RunnerConfig,
    unit_nonce_hex: &str,
    worker_input_path: &str,
    requested_memory: u64,
    wall_ms: u64,
) -> Result<TransientUnit, EnforceError> {
    config.validate().map_err(|_| EnforceError::InvalidBudget)?;
    if unit_nonce_hex.is_empty()
        || !unit_nonce_hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        || worker_input_path.is_empty()
        || !worker_input_path.starts_with('/')
    {
        return Err(EnforceError::InvalidBudget);
    }
    let (requested_memory, installed_memory) = floor_page_cap(requested_memory, config.page_size)?;
    let runtime_max = runtime_max_usec(wall_ms)?;
    let unit_name = format!("{UNIT_PREFIX}{unit_nonce_hex}");
    let mut argv = vec![
        "systemd-run".to_owned(),
        "--system".to_owned(),
        format!("--unit={unit_name}"),
        "--pipe".to_owned(),
    ];
    for (name, value) in REQUIRED_PROPERTIES {
        argv.push(format!("--property={name}={value}"));
    }
    argv.push(format!("--property=MemoryMax={installed_memory}"));
    argv.push(format!("--property=RuntimeMaxUSec={runtime_max}"));
    // Fixed launch_profile1 mapping: read-only worker/input bindings, a
    // private empty scratch directory, and lifetime binding to the
    // supervisor service. None of these accept caller input.
    argv.push(format!(
        "--property=BindReadOnlyPaths={} {}",
        config.worker_path, worker_input_path
    ));
    argv.push("--property=TemporaryFileSystem=/run/sley-scratch:ro".to_owned());
    argv.push("--property=BindsTo=sley-test-supervisor.service".to_owned());
    argv.push(config.worker_path.clone());
    argv.push("__native-test-worker".to_owned());
    argv.push(worker_input_path.to_owned());
    Ok(TransientUnit {
        unit_name,
        argv,
        requested_memory,
        installed_memory,
        runtime_max_usec: runtime_max,
    })
}

#[cfg(test)]
mod tests {
    use sley_id::{PrincipalId, WorkspaceId};

    use super::*;
    use crate::config::{AllowedCaller, default_config};

    fn runner_config() -> RunnerConfig {
        default_config(
            "/run/sley-test-supervisor",
            "/usr/lib/sley/sley-native-test-worker",
            [7; 32],
            [8; 32],
            vec![AllowedCaller {
                uid: 1000,
                workspace: WorkspaceId::from_bytes([11; 32]),
                principal: PrincipalId::from_bytes([12; 32]),
            }],
            "/etc/sley-test-supervisor/measurement.key",
            "/etc/sley-test-supervisor/trust",
        )
        .expect("config validates")
    }

    #[test]
    fn required_projection_matches_attested_names_and_values() {
        let names: Vec<&str> = REQUIRED_PROPERTIES.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "CapabilityBoundingSet",
                "DynamicUser",
                "KillMode",
                "MemoryAccounting",
                "MemorySwapMax",
                "NoNewPrivileges",
                "PrivateNetwork",
                "PrivateTmp",
                "ProtectControlGroups",
                "ProtectHome",
                "ProtectSystem",
                "SendSIGKILL",
                "TasksMax",
                "TimeoutStopUSec",
            ]
        );
        assert_eq!(LAUNCH_PROFILE, 1);
        assert_eq!(TIMEOUT_STOP_USEC, 2_000_000);
    }

    #[test]
    fn rendered_argv_pins_every_property_in_order() {
        let unit = render_transient_unit(
            &runner_config(),
            "9f2c",
            "/run/sley-test-supervisor/input/9f2c.bin",
            8_193,
            1_000,
        )
        .expect("renders");
        assert_eq!(unit.unit_name, "sley-native-test-9f2c");
        assert_eq!(unit.requested_memory, 8_193);
        assert_eq!(unit.installed_memory, 8_192);
        assert_eq!(unit.runtime_max_usec, 3_000_000);
        let argv = unit.argv.join("\n");
        let expected = [
            "systemd-run",
            "--system",
            "--unit=sley-native-test-9f2c",
            "--pipe",
            "--property=CapabilityBoundingSet=empty",
            "--property=DynamicUser=yes",
            "--property=KillMode=control-group",
            "--property=MemoryAccounting=yes",
            "--property=MemorySwapMax=0",
            "--property=NoNewPrivileges=yes",
            "--property=PrivateNetwork=yes",
            "--property=PrivateTmp=yes",
            "--property=ProtectControlGroups=yes",
            "--property=ProtectHome=yes",
            "--property=ProtectSystem=strict",
            "--property=SendSIGKILL=yes",
            "--property=TasksMax=1",
            "--property=TimeoutStopUSec=2000000",
            "--property=MemoryMax=8192",
            "--property=RuntimeMaxUSec=3000000",
            "--property=BindReadOnlyPaths=/usr/lib/sley/sley-native-test-worker /run/sley-test-supervisor/input/9f2c.bin",
            "--property=TemporaryFileSystem=/run/sley-scratch:ro",
            "--property=BindsTo=sley-test-supervisor.service",
            "/usr/lib/sley/sley-native-test-worker",
            "__native-test-worker",
            "/run/sley-test-supervisor/input/9f2c.bin",
        ];
        assert_eq!(unit.argv, expected);
        assert!(argv.contains("--property=MemorySwapMax=0"));
    }

    #[test]
    fn rendering_refuses_before_any_unit_exists() {
        let config = runner_config();
        assert_eq!(
            render_transient_unit(&config, "zz", "/run/input.bin", 8_192, 1_000)
                .expect_err("nonce")
                .tag(),
            EnforceError::InvalidBudget.tag()
        );
        assert_eq!(
            render_transient_unit(&config, "9f2c", "relative.bin", 8_192, 1_000)
                .expect_err("input")
                .tag(),
            EnforceError::InvalidBudget.tag()
        );
        assert_eq!(
            render_transient_unit(&config, "9f2c", "/run/input.bin", 100, 1_000)
                .expect_err("cap")
                .tag(),
            EnforceError::UnaccommodatingCap.tag()
        );
        assert_eq!(
            render_transient_unit(&config, "9f2c", "/run/input.bin", 8_192, 0)
                .expect_err("wall")
                .tag(),
            EnforceError::InvalidBudget.tag()
        );
    }
}

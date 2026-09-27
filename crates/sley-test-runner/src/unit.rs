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
//! supplied by the caller: systemd copies the daemon-owned, root-only input
//! into a private service credential readable by the dynamic worker UID.
//! Output goes to a daemon-owned bounded channel, and the unit binds to the
//! supervisor service lifetime.

use sley_scb1::{ScbError, ScbErrorCode};
use sley_tests::supervisor::SUPERVISOR_CLEANUP_MILLIS;
use sley_tests::{Caller, Property, SupervisorConfigParts, SupervisorConfigV1};

use crate::config::{RunnerConfig, UNIT_PREFIX, valid_admin_path};
use crate::enforce::{EnforceError, floor_page_cap, runtime_max_usec};
use crate::protocol::RunRequest;
use crate::worker::WORKER_INPUT_CREDENTIAL;

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

fn manager_property_arg(name: &str, normalized_value: &str) -> String {
    // SLEYNHC1 names the empty capability set as `empty`; systemd's unit
    // syntax installs it with an empty right-hand side.
    let value = if name == "CapabilityBoundingSet" {
        ""
    } else {
        normalized_value
    };
    format!("--property={name}={value}")
}

/// Constructs the exact configuration the administrator and selected run
/// require the system manager to install.
///
/// This is an expectation for the host verifier, not evidence that systemd
/// installed the properties. The daemon may sign a successful measurement
/// only after checking the actual transient unit and cgroup against it.
///
/// # Errors
///
/// Refuses an invalid request, caller mapping, page-floor/deadline budget, or
/// malformed configuration envelope.
pub fn expected_supervisor_config(
    config: &RunnerConfig,
    request: &RunRequest,
    caller_uid: u32,
) -> Result<SupervisorConfigV1, ScbError> {
    let mismatch = || ScbError::new(ScbErrorCode::ContractUnknown);
    config.validate().map_err(|_| mismatch())?;
    request.verified_program()?;
    let caller = config.caller_for_uid(caller_uid).ok_or_else(mismatch)?;
    if caller.workspace != request.workspace || caller.principal != request.principal {
        return Err(mismatch());
    }
    let (_, installed_memory) =
        floor_page_cap(request.declared_limits.memory_bytes, config.page_size)
            .map_err(|_| ScbError::new(ScbErrorCode::ResourceLimit))?;
    let manager_runtime = runtime_max_usec(request.wall_ms)
        .map_err(|_| ScbError::new(ScbErrorCode::ResourceLimit))?;
    let mut properties = REQUIRED_PROPERTIES
        .iter()
        .map(|(name, value)| Property {
            name: (*name).to_owned(),
            value: (*value).to_owned(),
        })
        .collect::<Vec<_>>();
    properties.push(Property {
        name: "MemoryMax".to_owned(),
        value: installed_memory.to_string(),
    });
    properties.push(Property {
        name: "RuntimeMaxUSec".to_owned(),
        value: manager_runtime.to_string(),
    });
    properties.sort_by(|left, right| left.name.cmp(&right.name));
    let mut callers = config
        .allowed_callers
        .iter()
        .map(|caller| Caller {
            uid: caller.uid,
            workspace: caller.workspace,
            principal: caller.principal,
        })
        .collect::<Vec<_>>();
    callers.sort_by_key(|caller| {
        (
            caller.uid,
            *caller.workspace.as_bytes(),
            *caller.principal.as_bytes(),
        )
    });
    SupervisorConfigV1::build(SupervisorConfigParts {
        worker_digest: config.worker_sha256,
        supervisor_digest: config.supervisor_sha256,
        properties,
        callers,
        page_size: config.page_size,
        cleanup_millis: SUPERVISOR_CLEANUP_MILLIS,
        launch_profile: LAUNCH_PROFILE,
    })
}

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
        || !valid_admin_path(worker_input_path)
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
        argv.push(manager_property_arg(name, value));
    }
    argv.push(format!("--property=MemoryMax={installed_memory}"));
    argv.push(format!("--property=RuntimeMaxUSec={runtime_max}"));
    // Fixed launch_profile1 mapping: the manager copies the root-only input
    // into the dynamic worker's private credential directory. The worker
    // path stays read-only; scratch and lifetime bindings are fixed.
    argv.push(format!(
        "--property=LoadCredential={WORKER_INPUT_CREDENTIAL}:{worker_input_path}"
    ));
    argv.push(format!(
        "--property=BindReadOnlyPaths={}",
        config.worker_path
    ));
    argv.push("--property=TemporaryFileSystem=/run/sley-scratch:ro".to_owned());
    argv.push("--property=BindsTo=sley-test-supervisor.service".to_owned());
    argv.push(config.worker_path.clone());
    argv.push("__native-test-worker".to_owned());
    argv.push("--credential".to_owned());
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
    use crate::program::PortableTestProgram;
    use crate::worker::WorkerRequest;

    fn selected_request() -> RunRequest {
        let worker = WorkerRequest::decode_frame(include_bytes!(
            "../../../conformance/native-worker/v1/observed-input.bin"
        ))
        .expect("canonical worker vector");
        let program = PortableTestProgram::parse(&worker.program_bytes).expect("portable program");
        RunRequest::from_portable_program(&program, 1_000, [9; 32])
            .expect("selected supervisor request")
    }

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
    fn expected_configuration_matches_rendered_unit_and_authenticated_scope() {
        let request = selected_request();
        let mut config = runner_config();
        config.allowed_callers[0].workspace = request.workspace;
        config.allowed_callers[0].principal = request.principal;
        let expected = expected_supervisor_config(&config, &request, 1_000)
            .expect("expected attested projection");
        let unit = render_transient_unit(
            &config,
            "9f2c",
            "/run/sley-test-supervisor/input/9f2c.bin",
            request.declared_limits.memory_bytes,
            request.wall_ms,
        )
        .expect("rendered transient unit");
        assert_eq!(expected.properties().len(), 16);
        assert_eq!(expected.callers().len(), 1);
        for property in expected.properties() {
            assert!(
                unit.argv
                    .contains(&manager_property_arg(&property.name, &property.value))
            );
        }
        assert_eq!(
            expected_supervisor_config(&config, &request, 1_001)
                .expect_err("unapproved caller")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        config.allowed_callers[0].principal = PrincipalId::from_bytes([0xff; 32]);
        assert_eq!(
            expected_supervisor_config(&config, &request, 1_000)
                .expect_err("wrong principal mapping")
                .code(),
            ScbErrorCode::ContractUnknown
        );
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
            "--property=CapabilityBoundingSet=",
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
            "--property=LoadCredential=sley-input:/run/sley-test-supervisor/input/9f2c.bin",
            "--property=BindReadOnlyPaths=/usr/lib/sley/sley-native-test-worker",
            "--property=TemporaryFileSystem=/run/sley-scratch:ro",
            "--property=BindsTo=sley-test-supervisor.service",
            "/usr/lib/sley/sley-native-test-worker",
            "__native-test-worker",
            "--credential",
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
            render_transient_unit(&config, "9f2c", "/run/input:bad.bin", 8_192, 1_000)
                .expect_err("credential source property injection")
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

//! Root service for bounded native Sley test execution.
//!
//! Startup admits no requests until protected configuration, binary pins,
//! signing authority, and orphan cleanup all pass. A failed internal run
//! stops the service. Unknown cleanup suppresses automatic restart; a manual
//! start still explicitly invokes reserved-prefix orphan reconciliation.

use std::error::Error;
use std::process::ExitCode;
use std::time::Duration;

use sley_test_runner::admin_config::load_admin_config;
use sley_test_runner::reconcile::reconcile_orphans;
use sley_test_runner::service::{ServiceError, serve_one_root};
use sley_test_runner::socket::bind_supervisor_socket;
use sley_test_runner::trust_store::ProvisionedMeasurementAuthority;

const INGRESS_TIMEOUT: Duration = Duration::from_secs(5);
const CLEANUP_UNCONFIRMED_EXIT: u8 = 78;

fn failure_exit_status(error: &(dyn Error + 'static)) -> u8 {
    if error.downcast_ref::<ServiceError>() == Some(&ServiceError::CleanupUnconfirmed) {
        CLEANUP_UNCONFIRMED_EXIT
    } else {
        1
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = load_admin_config()?;
    config.verify_installed_binaries()?;
    let authority = ProvisionedMeasurementAuthority::load(config)?;
    reconcile_orphans()?;
    let bound = bind_supervisor_socket(authority.config())?;
    eprintln!("NATIVE_SUPERVISOR_READY");

    loop {
        match serve_one_root(bound.listener(), &authority, INGRESS_TIMEOUT) {
            Ok(()) | Err(ServiceError::Ingress(_) | ServiceError::WriteFailure) => {}
            Err(error) => return Err(Box::new(error)),
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(failure_exit_status(error.as_ref()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_cleanup_exit_is_paired_with_packaged_restart_prevention() {
        assert_eq!(failure_exit_status(&ServiceError::CleanupUnconfirmed), 78);
        assert_eq!(failure_exit_status(&ServiceError::ClockUnavailable), 1);
        assert_eq!(failure_exit_status(&std::io::Error::other("startup")), 1);
        let unit =
            include_str!("../../../../packaging/sley-test-supervisor/sley-test-supervisor.service");
        assert!(unit.lines().any(|line| line == "Restart=on-failure"));
        assert!(
            unit.lines()
                .any(|line| line == format!("RestartPreventExitStatus={CLEANUP_UNCONFIRMED_EXIT}"))
        );
    }
}

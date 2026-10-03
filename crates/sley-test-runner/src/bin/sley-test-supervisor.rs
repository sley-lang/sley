//! Root service for bounded native Sley test execution.
//!
//! Startup admits no requests until protected configuration, binary pins,
//! signing authority, and orphan cleanup all pass. A failed internal run
//! stops the service so its next start reconciles prior transient units.

use std::error::Error;
use std::process::ExitCode;
use std::time::Duration;

use sley_test_runner::admin_config::load_admin_config;
use sley_test_runner::reconcile::reconcile_orphans;
use sley_test_runner::service::{ServiceError, serve_one_root};
use sley_test_runner::socket::bind_supervisor_socket;
use sley_test_runner::trust_store::ProvisionedMeasurementAuthority;

const INGRESS_TIMEOUT: Duration = Duration::from_secs(5);

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
            ExitCode::FAILURE
        }
    }
}

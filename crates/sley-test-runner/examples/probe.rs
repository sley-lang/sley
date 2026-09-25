//! Unprivileged enforcer readiness probe: prints the live check lines
//! plus the pending privileged section, and exits nonzero on any fail.
use sley_test_runner::probe::{probe_unprivileged, render_report};

fn main() {
    let socket_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/run/sley-test-supervisor".to_owned());
    let report = probe_unprivileged(&socket_dir, None);
    print!("{}", render_report(&report));
    if report.checks.iter().any(|check| !check.passed) {
        std::process::exit(1);
    }
}

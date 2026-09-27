//! Host-side native test supervision (N3) from `NATIVE_TEST_EXECUTION_V1.md`
//! section 7.
//!
//! This crate owns the root daemon's local configuration, the closed
//! `RunNativeTest` IPC protocol, the frozen systemd transient-unit rendering,
//! the checked enforcement math (page-aligned memory caps, monotonic
//! deadlines, peak/event admission), the measurement-admission predicate, the
//! worker request envelope, bounded portable program artifact, root-owned
//! worker-input staging, pure native
//! execution/report bridge, observed-report request binding, typed bounded
//! report/measurement/configuration response transport, bounded root-peer
//! socket client, authenticated socket
//! ingress boundary, fail-closed
//! one-connection service with prelaunch program checks, live cgroup telemetry,
//! and readiness probes. The root service loop and privileged transient-unit
//! execution are not wired yet.
//!
//! It performs no policy selection, grants no commit authority, and holds no
//! acceptance key. Measurement signing goes through [`outcome::Signer`];
//! [`outcome::Ed25519MeasurementSigner`] owns the concrete Ed25519 key, and
//! privileged install/probe steps need the authenticated privilege handoff.
//! The private worker now waits on stdin start/release gates around executing
//! a portable selected `TestCase` and emitting one canonical pure report.
//! The owner derives bounded worker input hashes and
//! supervisor request bindings from the validated selected `TestCase`; the
//! socket service rechecks both before its explicit refusal. Measured launch
//! and admission still belong to N5; the service remains refusal-only.

pub mod client;
pub mod config;
pub mod enforce;
pub mod execution;
pub mod ingress;
pub mod outcome;
pub mod probe;
pub mod program;
pub mod protocol;
pub mod response;
pub mod service;
pub mod stage;
pub mod telemetry;
pub mod unit;
pub mod worker;

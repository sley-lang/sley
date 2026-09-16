//! Host-side native test supervision (N3) from `NATIVE_TEST_EXECUTION_V1.md`
//! section 7.
//!
//! This crate owns the root daemon's local configuration, the closed
//! `RunNativeTest` IPC protocol, the frozen systemd transient-unit rendering,
//! the checked enforcement math (page-aligned memory caps, monotonic
//! deadlines, peak/event admission), the measurement-admission predicate, the
//! worker request envelope with its private entry, and the readiness probes.
//!
//! It performs no policy selection, grants no commit authority, and holds no
//! acceptance key. Measurement signing goes through [`outcome::Signer`];
//! Ed25519 wiring lands with the vendored crypto dependency (pending), and
//! privileged install/probe steps need the authenticated privilege handoff.
//! Worker execution dispatch lands with the N5 commit path, which owns
//! plans-to-inputs construction; until then the worker entry strictly
//! decodes and explicitly refuses with [`worker::WorkerRefusal`].

pub mod config;
pub mod enforce;
pub mod outcome;
pub mod probe;
pub mod protocol;
pub mod unit;
pub mod worker;

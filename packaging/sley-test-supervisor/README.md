# Root supervisor installation — authenticated privilege handoff

N3 source (crate `sley-test-runner`, CLI `__native-test-worker` entry,
transient-unit renderer, enforcement math, worker envelope, readiness
probes) is landed and tested. The steps below need root on each intended
host and are explicitly **pending**, not waived:

1. Build the release binaries (`sley`, supervisor daemon once its event
   loop lands) and install the worker at the configured
   `/usr/lib/sley/sley-native-test-worker` path.
2. Pin `worker_sha256` over the exact installed worker bytes
   (`sley-test-runner::config::sha256_bytes`) and write the administrator
   `RunnerConfig` (socket dir, callers, measurement key path, trust
   manifest dir). Keys are root-only (`0400 root:root`); the worker
   namespace must not contain them.
3. Install the `sley-test-supervisor.service` unit (ships with the daemon
   binary, not before) and start it; verify the authenticated socket
   appears with root ownership.
4. Run the unprivileged probe (`cargo run -p sley-test-runner --example
   probe -- /run/sley-test-supervisor`) and keep the receipt. Every
   `fail` names a platform prerequisite to implement; none may be waived
   with a documented assumption.
5. Run the privileged probe set (pre-exec placement, transient units,
   UID/key isolation, manager backstop, caller and daemon SIGKILL
   cleanup, orphan reconciliation) on **both** intended hosts and keep
   the receipts. Mock success cannot qualify an enforcer.
6. Wire worker execution dispatch (N5) and the Ed25519 measurement signer
   with the pinned Ed25519 implementation before any attestation is trusted.

Known operational state: `sudo -n` on primary-host requires a password, so
no privileged step was attempted from this session. Operator approval
for the handoff is already granted; the handoff itself has not happened.

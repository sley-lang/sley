# Root supervisor installation — authenticated privilege handoff

N3 source (crate `sley-test-runner`, CLI `__native-test-worker` entry,
transient-unit renderer, enforcement math, worker envelope, readiness
probes, and one-connection authenticated refusal service) is landed and
tested. The service answers a valid request with
`RUN_REFUSAL_EXECUTION_NOT_WIRED`; invalid peers receive no response. It has
no production daemon entry, worker launch, or measurement signature. The
steps below need root on each intended host and are explicitly **pending**,
not waived:

The closed runner request carries `candidate_id=Some` for candidate-affected
runs and `None` for explicit-root diagnostics, matching the owner-derived plan
modes. It also carries one exact bounded worker frame; ingress checks its
declared limits against the authenticated outer request and refuses a zero or
expanded wall budget. This internal socket format has no installed daemon or
external compatibility claim yet. A bounded portable program artifact now
encodes the exact plan, root, live object inventory, and selected native
`TestCase`; its strict parser and `RunRequest::verified_program` reject
cross-boundary identity, limit, or ordered input-hash substitution. The pure
owner derives a bounded worker request from the statically validated selected
`TestCase` before launch. The refusal-only service does
not yet invoke that check. A pure `execute_portable_test` bridge now projects
the bound objects, rechecks the selected native `TestCase`, executes the
existing Sley VM, and compares VM-derived ordered input hashes. The pure
`sley-tests` owner reserves the complete execution report before the VM runs
and can build a canonical observed report and expectation comparison, or a
canonical rejected report when the VM refuses before observation. These
results have no host memory/time measurement and do not pass admission. A
separate `RunRequest::verified_observed_worker_report` check parses a bounded
worker report and binds its plan, selected test, root, function, ordered input
hashes, schema hashes, profile, and limits to the authenticated request. It
checks data consistency only; the refusal-only service does not call it yet.
The private worker now dispatches a valid portable program to the pure VM
and writes a bounded canonical report. Owner-side portable artifact sourcing,
measured launch, and admission still need to be wired before either mode can
execute through the service.

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
   probe -- /run/sley-test-supervisor`) and keep the receipt. Its socket
   check requires a root-owned runtime directory without group/world write,
   a root-owned Unix socket, and a live listener. Every `fail` names a
   platform prerequisite to implement, including the Linux `openat2` path
   resolution used for worker input bindings; none may be waived with a
   documented assumption.
5. Run the privileged probe set (pre-exec placement, transient units,
   UID/key isolation, manager backstop, caller and daemon SIGKILL
   cleanup, orphan reconciliation) on **both** intended hosts and keep
   the receipts. Mock success cannot qualify an enforcer.
6. Wire worker execution dispatch (N5) and the Ed25519 measurement signer
   with the pinned Ed25519 implementation before any attestation is trusted.

Known operational state: `sudo -n` on primary-host requires a password, so
no privileged step was attempted from this session. Operator approval
for the handoff is already granted; the handoff itself has not happened.

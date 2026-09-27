# Native test supervisor installation

The root service source is `crates/sley-test-runner/src/bin/sley-test-supervisor.rs`.
The accompanying `sley-test-supervisor.service` owns
`/run/sley-test-supervisor`, loads one administrator configuration, pins the
installed worker and supervisor bytes, loads root-provisioned measurement
authority, reconciles old worker units, and then accepts authenticated local
requests. The worker executes selected native Sley `TestCase` programs through
the existing VM. This service is host machinery; it is not a Sley program.

## Current evidence

The daemon builds, the runner's unprivileged unit tests pass, and its service
unit source is present. The actual root service has **not** been installed or
qualified on the intended hosts. No native test admission or release-readiness
claim follows from the local unit tests. The authenticated refusal-only
handler remains available for negative protocol tests; the production daemon
uses the root handler.

The development `sley serve --protocol-profile v3-capable` command now accepts
an explicit `--native-authority-config` file for the receiver's separate
acceptance key and trust manifests. Its file shape and permission checks are
in [the CLI contract](../../docs/spec/SLEY_CLI_V1.md#11-explicit-native-commit-authority-development-revision-12).
It supplies the existing native commit route with a socket executor and signer;
it does not qualify this root service or enable native tests on the selected
2.0.1 release binary. Diagnostic `tests.selected` and `tests.affected` remain
unprovisioned.

## Protected administrator configuration

The daemon reads exactly `/etc/sley-test-supervisor/config.json`. It must be a
single-link, root-owned regular file with mode `0400` or `0600`, reachable
without symlinks. The closed JSON object requires these fields:

| Field | Meaning |
|---|---|
| `version` | `1` |
| `runtime_dir` | `/run/sley-test-supervisor`, matching the unit's `RuntimeDirectory` |
| `worker_path` | Exact installed, root-owned private worker executable path; the current worker entry is `__native-test-worker --credential` |
| `worker_sha256` | Lowercase SHA-256 hex of the installed worker bytes |
| `supervisor_sha256` | Lowercase SHA-256 hex of the installed daemon bytes |
| `page_size` | Host page size as a power of two |
| `allowed_callers` | Nonempty array of unique `uid`, `workspace`, and `principal` identities; each identity is lowercase 32-byte hex |
| `measurement_key_path` | Root-only Ed25519 measurement key file |
| `trust_manifest_dir` | Root-owned directory containing `measurement.sleyntr1` |

The manifest must grant the loaded key the measurement role. The key and
manifest are separate from request data and never enter the worker namespace.
The socket has mode `0666` so configured nonroot UIDs can connect; the daemon
checks kernel peer credentials against `allowed_callers` **before reading** a
request. The runtime directory must be root-owned and not writable by other
users.

## Qualification before admission

1. Build and install exact worker and daemon binaries. A copy of `sley` may
   serve as the worker executable when it contains the private credential
   entry. Preserve its exact installed bytes while the digest pin is active.
2. Provision the private configuration, signing key, and measurement trust
   manifest. Install the included service unit, start it, and retain its
   startup log. `NATIVE_SUPERVISOR_READY` is a startup marker, not a passing
   execution receipt.
3. Run `cargo run -p sley-test-runner --example probe --
   /run/sley-test-supervisor` as an unprivileged user and retain the receipt.
4. Run the privileged N3 checks on both intended hosts: real transient-unit
   placement, memory and wall enforcement, UID and key isolation, cgroup
   telemetry, peer authentication, daemon/worker kill handling, and restart
   orphan reconciliation. Verify complete response and signature against the
   exact selected test and supervisor configuration.
5. Only then use the transaction owner's native test admission path through a
   receiver-provisioned v3 server and its candidate/root binding checks. A
   static `TestCase` validation, a mock
   response, or a pure VM comparison does not grant admission.

`sudo -n` currently requires a password on the primary host, so the root
installation and live qualification are pending. The local source and tests
do not waive this gate.

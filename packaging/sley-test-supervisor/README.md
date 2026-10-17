# Native test supervisor installation

The root service source is `crates/sley-test-runner/src/bin/sley-test-supervisor.rs`.
The accompanying `sley-test-supervisor.service` owns
`/run/sley-test-supervisor`, loads one administrator configuration, pins the
installed worker and supervisor bytes, loads root-provisioned measurement
authority, reconciles old worker units, and then accepts authenticated local
requests. The worker executes selected native Sley `TestCase` programs through
the existing VM. This service is host machinery; it is not a Sley program.

## Current source and recorded host evidence

The source supports authenticated no-result diagnostics for unsupported short
wall budgets, measured deadlines with confirmed teardown, and exact
manager-observed OOM. The OOM diagnostic is separate from measured worker
reports and never fabricates missing `memory.events` counters or a native
receipt; see [its evidence contract](../../docs/status/NATIVE_MANAGER_OOM_DIAGNOSTIC.md).
Missing, ambiguous, untrusted or dirty evidence remains refused or unknown.

The retained greyarch host checks in [issue #24](https://github.com/sley-lang/sley/issues/24)
used unreleased development core `8752598` on September 28, 2026:

- A real selected native TestCase under a 1 MiB cap returned
  `NATIVE_TEST_RESOURCE_REFUSED` with an unchanged accepted root/transaction.
  The exact reaped worker had typed manager `Result=oom-kill`, matching
  `MemoryMax=MemoryPeak=1048576`, and no remaining cgroup. The signed diagnostic
  supplied the receiver with an authenticated resource failure, not a test
  success. [Pinned OOM record](https://github.com/sley-lang/tools/blob/a63e4e2/evidence/compatibility/f0-greyarch-real-native-oom-classification-8752598-2026-09-28.json).
- A real selected native TestCase with a supported 1,000 ms wall budget
  returned the same resource refusal after 1,028 ms, with an unchanged root and
  transaction. The worker was killed and reaped; its retained memory peak was
  below the installed cap and no OOM result was reported.
  [Pinned wall-deadline record](https://github.com/sley-lang/tools/blob/b729fc6/evidence/compatibility/f0-greyarch-real-native-timeout-8752598-2026-09-28.json).
- Those records report restoration of the previous binaries, configuration
  and trust, an inactive/disabled supervisor and no loaded native worker
  units. This describes the recorded cleanup, not a live host-state check.

Earlier signed prelaunch and synthetic-worker checks are historical evidence
in that issue. The later real-worker checks supersede their narrower coverage,
but do not qualify this integration's rebuilt binaries. Released-core/tools
compatibility and clean installed-package F0 acceptance remain open. Any new
installation requires exact reviewed artifacts and fresh host qualification.
The authenticated refusal-only handler remains available for negative protocol
tests; the production daemon uses the root handler.

Caller budgets that cannot form a valid supervisor configuration, such as
zero or sub-page memory and unrepresentable deadlines, receive an unsigned
prelaunch refusal. They do not stop the shared service or produce measurement
evidence. A supported configuration with a wall budget below 1,000 ms still
uses the existing signed prelaunch unsupported-wall refusal under its trust
grant. A loaded unit with a reused attempt name also refuses before launch. After launch, the
daemon must observe a nonzero manager invocation ID and worker PID whose
monotonic start time follows this launch boundary. Terminal OOM facts must
match that exact invocation, PID and start time. A launcher process alone,
an old failed unit, or unavailable freshness evidence cannot authorize a
signed OOM diagnostic. These source checks require fresh exact-build host
qualification, including fast OOM and failed-launch cases.

The development `sley serve --protocol-profile v3-capable` command now accepts
an explicit `--native-authority-config` file for the receiver's separate
acceptance key and trust manifests. Its file shape and permission checks are
in [the CLI contract](../../docs/spec/SLEY_CLI_V1.md#11-explicit-native-commit-authority-development-revision-12).
It supplies the existing native commit route with a socket executor and signer.
The same receiver-selected adapter serves candidate-bound `tests.affected`
and explicit accepted-root `tests.selected`. The latter receives the complete
owner-loaded root and object snapshot; its request uses the zero diagnostic
principal defined by the native plan. An administrator must explicitly grant
that workspace/principal pair in `allowed_callers` to run it. Neither route
qualifies this root service or changes the published 2.1.1 binaries. This
integration is development source; released-core/tools compatibility and
installed-package qualification remain separate requirements.

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
| `allowed_callers` | Nonempty array of unique `(uid, workspace, principal)` grants; one UID may have multiple explicit scopes; each identity is lowercase 32-byte hex |
| `measurement_key_path` | Root-only Ed25519 measurement key file |
| `trust_manifest_dir` | Root-owned directory containing `measurement.sleyntr1` |

The manifest must grant the loaded key the measurement role. The key and
manifest are separate from request data and never enter the worker namespace.
The socket has mode `0666` so configured nonroot UIDs can connect; the daemon
checks kernel peer credentials against `allowed_callers` **before reading** a
request. The runtime directory must be root-owned and not writable by other
users.

## Unprivileged authority preparation

`prepare_authority` is a development example for one F0 fixture. It reads an
exact staged binary manifest, the toolkit workspace state, and the native
`TestCase` candidate. It verifies the staged binary hashes and candidate's
canonical workspace, principal, parent, policy, epoch, ID, and remaining
lifetime. A candidate that has expired or expires within two minutes is refused
before any output directory is created. It derives the
same per-limit `SupervisorConfigV1` that the daemon checks, plus the fixed
native admission profile. It creates a new mode `0700` output directory with
mode `0600` root and receiver configuration drafts, a receiver acceptance key,
scoped trust manifests, and a public `plan.json`. It requires the measurement
public key from a separate root-owned key generation step and never creates,
reads, or stores the measurement seed. Its grants expire after seven days. It
never writes `/etc`, installs a unit, or starts a supervisor.

```sh
cargo run --locked --offline -p sley-test-runner --example prepare_authority -- \
  /path/to/stage/manifest.json \
  /path/to/workspace/.sley-tools/state.json \
  /path/to/native-test-candidate.json \
  "$(id -u)" 4096 ROOT_MEASUREMENT_PUBLIC_KEY_HEX \
  /absolute/new/private/output-directory
```

The native test candidate expires ten minutes after construction. Regenerate
it against the same accepted graph workspace immediately before using this
example; do not reuse an old candidate or a draft that binds an old candidate
digest. The preparation plan records the exact candidate expiry.

The root configuration draft names `/usr/lib/sley/sley`,
`/usr/lib/sley/sley-test-supervisor`, and the future root-owned files under
`/etc/sley-test-supervisor`. It includes separate exact grants for the fixture's
commit principal and the zero diagnostic principal under the same receiver
UID. The receiver configuration names the private files in the output
directory and can be loaded for an unprivileged protocol handshake. Before
host qualification, an administrator must review and install the exact bytes,
keep the measurement seed root-only, and verify the profile IDs against a real
supervised request.

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

The September 28 records bind older development binaries; they are not a
current inventory of the installed host. Any installation or update needs a
reviewed migration that preserves the root measurement authority and exact
binary pins, followed by fresh live qualification. The local source and tests
do not waive that gate.

The worker launch hides the configured supervisor runtime directory with an
exact `InaccessiblePaths` mapping, verified through the manager before the
worker start gate opens. This blocks access to the supervisor's pathname Unix
socket; peer authentication remains a separate boundary for authorized host
clients. The copied `sley-input` credential and private gate/report pipes are
outside this mask. Qualification of an updated installation must retain the
actual worker-view socket isolation and credential/pipe behavior; receipts for
older binary pins do not establish this mapping. A mount mask does not close
inherited descriptors, so the worker's closed descriptor handoff remains part
of the isolation boundary.

### Ownership-bound cleanup and unknown creation

The launcher alone is not proof that systemd created the requested invocation.
Before an invocation is observed, cleanup closes its gates and kills/reaps only
that owned launcher; manager creation remains unknown and the daemon degrades.
An observed invocation without a live cgroup handle may be accepted as terminal
only against the same InvocationID, ExecMainPID and monotonic start tuple, with
zero live PID, empty manager cgroup and confirmed cgroup removal. This retains
the separately verified fast-OOM diagnostic; missing history is not a pass.

For a live invocation, the supervisor brackets cgroup opening and sampling with
exact manager invocation checks. It holds root-owned cgroup2 domain directory
and cgroup.kill descriptors, samples that same directory throughout the phase,
and kills only through that held control file. It never falls back to killing
an attempt's unit name, including in Drop. Worker Restart=no is explicitly
rendered and manager-verified to prevent automatic invocation reuse. The fixed
launch-profile mapping changes with the pinned binaries; canonical property
configuration grammar is unchanged.

These checks assume a trusted manager and no concurrent privileged lifecycle,
process migration, or mount writers. A held object prevents pathname replacement
from redirecting cleanup; it cannot protect against root repopulating that same
live cgroup. Fresh settings/start observations are not a causal creation receipt.
Initial unknown creation always degrades instead of claiming cleanup success.

**Upgrade the supervisor binary and packaged service unit together.** Unknown
cleanup exits with status 78, paired with RestartPreventExitStatus=78 while
other unexpected failures retain Restart=on-failure. This prevents an automatic
restart from entering name-based orphan reconciliation after ambiguous creation.
An operator/manual restart still explicitly invokes the existing reserved-prefix
reconciliation policy, which is unchanged. This does not promise preservation
across manual restarts, root interference or independently altered unit policy.
Stopping the supervisor still has the intended systemd BindsTo effects on units
that depend on it. An independent late-occupancy fixture must not declare that
dependency; exit 78 prevents subsequent broad reconciliation, not every possible
side effect of supervisor termination on deliberately dependent foreign units.

Per-attempt manager reads share the caller's absolute run deadline, or the one
stored cleanup deadline after abort. GetUnit, both GetAll calls, listings,
terminal checks and OOM diagnostics do not start new per-call budgets. Replies
are read in capped nonblocking chunks with polling; EOF, command exit and JSON
parsing must all complete before acceptance. The fixed busctl two-second option
is only an inner backstop. A 20 ms command-cleanup reserve is taken from the
remaining absolute interval; insufficient time refuses before spawning. Errors
kill and poll-reap only that owned read-only command child within the same
interval. Abort and Drop do not manufacture another cleanup budget.

If scheduling or a kernel stall exhausts the interval before that child can be
confirmed reaped, CommandCleanupUnconfirmed remains explicit. Per-attempt paths,
including absence preflight before a worker guard exists and OOM diagnostics,
promote it to cleanup degradation/exit 78; late successful replies never qualify.
This is not a kernel hard-real-time guarantee. Fixed busctl is a trusted direct
read-only child; no general descendant supervision contract is introduced.
Public convenience/startup helpers use a default two-second aggregate budget;
startup error/reconciliation and exit policy remain separate and unchanged.

# Private manager OOM diagnostic

The development supervisor can return a signed resource failure when an exact
native worker unit is killed by its installed `MemoryMax` and systemd retains
the OOM result after the cgroup disappears. This is a private `RunResponse`
version 3 evidence union tag 3, with `Failed` status and code 2. It is not a
`SLEYMTA1` measured attestation, a VM report, or a native receipt entry.

The owner first confirms worker teardown. It then reads typed systemd D-Bus
properties for the exact rendered transient unit and checks the installed
launch and limits, `ActiveState=failed`, `Result=oom-kill`, `MainPID=0`, empty
`ControlGroup`, `ExecMainCode=2`, `ExecMainStatus=9`, nonzero `ExecMainPID`,
and nonzero `MemoryPeak` no greater than `MemoryMax`. It also confirms the
unit cgroup path is absent. Failure or ambiguity returns the previous unknown
outcome.

The root signer records a bounded canonical claim with these fields, in tag
order: version 1, SHA-256 of the exact request frame, authenticated caller UID,
measurement public key, measurement trust policy ID, supervisor configuration
ID, installed memory cap, retained memory peak, retained main PID, monotonic
elapsed nanoseconds, historical Unix milliseconds, and Ed25519 signature.
The signature covers `sley2.native-manager-oom.v1` followed by the canonical
record of fields 1 through 11. The response also carries the complete
supervisor configuration named by the claim. It carries no invented cgroup
`memory.events` values.

The receiver checks the request hash, caller UID, exact configuration and
page-floored cap, provisioned measurement trust for the workspace,
configuration and historical time, then the signature. Only this verified
tag 3 diagnostic maps to `NATIVE_TEST_RESOURCE_REFUSED`. A malformed,
untrusted, unsigned, or unrelated failure cannot be mapped this way. Older
clients reject tag 3 and therefore fail closed; compatible supervisor and
receiver binaries are required for this development extension.

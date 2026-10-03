# Native worker start/release gate — unprivileged manager probe

- UTC: 2026-09-27 21:06
- Host: greyarch, kernel 7.2.5-3-omarchy, systemd 261 (261.2-1-arch)
- Scope: `systemd-run --user --pipe --collect` with the real `target/debug/sley`
  binary, `__native-test-worker --credential`, and a mode 0600 source installed
  as `LoadCredential=sley-input:<source>`
- Input SHA-256: `4c769be0547685d0ae2115953fd0f52eb50acb215236ba87cd0d3c7c02c2e357`
- Expected report SHA-256: `66bb48b34f1686730599a5412cdd1d195b0b6eb307f3cec7a8f3cf23efd7274c`
- Worker binary SHA-256: `554e0c542a253c16d0336ffc5de637506fe7fda5f14c9a047fcf2d4087fc612e`

The probe launched a transient user service with `MemoryAccounting=yes`,
`MemoryMax=16777216`, and `MemorySwapMax=0`. Before sending the fixed start
byte, `systemctl --user show` reported `MainPID=1633016`, and its live
`cgroup.procs` contained only `1633016`. After sending `0xa5`, stdout
contained the exact 618-byte conformance report. The process remained alive,
and `MainPID` and `cgroup.procs` still agreed. After reading live memory
counters, the probe sent `0x5a`: the worker exited 0 with no extra stdout.

A separate gated user-service probe read live `memory.events` with `max=0`,
`oom=0`, `oom_kill=0`, `oom_group_kill=0`, `memory.peak=2621440`,
`memory.max=16777216`, and `memory.swap.max=0` before release. Opening those
files after the unit exited failed, even when their descriptors were opened
while it was live. A `RemainAfterExit=yes` trial kept the unit active but left
`ControlGroup` empty after its process exited. The two-byte gate is therefore
needed for live telemetry collection on this host.

These observations validate the worker/credential/gate mechanism in a user
service only. They do not prove the root system-service sandbox, signed
measurement, daemon cleanup, or native test admission. The privileged F0
qualification remains open.

# Native supervisor socket readiness — greyarch

- UTC: 2026-09-27T18:22:50Z
- Host: greyarch, kernel 7.2.5-3-omarchy
- Source branch: `feat/native-supervisor-ingress`, after the socket probe correction
- Command: `cargo run --locked --offline -p sley-test-runner --example probe -- /run/sley-test-supervisor`
- Exit: 1 (supervisor runtime directory is absent)
- Focused tests: `cargo test --locked --offline -p sley-test-runner` — 32 passed

The previous probe used `socket-dir-writable`, which could pass for a plain
writable directory without a daemon. The corrected unprivileged check requires
a root-owned runtime directory without group/world write, a root-owned Unix
socket, and a successful connection to a listener. It sends no run request.
Local socket tests cover a live listener and missing, stale, regular-file,
wrong-owner, and unsafe-directory refusals. The earlier
`sley-test-supervisor-probe-primary-host-2026-09-16.md` is preserved as
historical evidence of the older probe.

```text
cgroup2-mounted=pass /sys/fs/cgroup
memory-controller=pass cpuset cpu io memory hugetlb pids rdma misc dmem
system-manager-version=pass systemd 261 (261.2-1-arch)
page-size=pass 4096
supervisor-socket-listening=fail /run/sley-test-supervisor: runtime directory unavailable: No such file or directory (os error 2)
pending-privileged:
  - caller SIGKILL peer-death cleanup
  - daemon SIGKILL crash cleanup
  - dynamic-UID and key isolation
  - manager runtime backstop
  - orphan restart reconciliation
  - pre-exec cgroup placement
  - system transient-unit launch
```

The root service loop, worker program handoff, worker dispatch, privileged
installation, and seven privileged isolation checks remain open. This receipt
does not qualify native test execution or measurement signing.

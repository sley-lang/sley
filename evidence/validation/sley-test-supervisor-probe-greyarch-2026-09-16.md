# Native test supervisor readiness probe — primary-host, unprivileged subset

- Date (UTC): 2026-09-16T05:41:22Z
- Host: primary-host, kernel 7.2.5-3-omarchy
- Command: `cargo run --offline -p sley-test-runner --example probe -- /run/sley-test-supervisor`
- Exit status: 1 (expected: supervisor not installed, socket dir absent)
- Operator: unprivileged user session; no `sudo -n` attempted (password required)

## Live output

cgroup2-mounted=pass /sys/fs/cgroup
memory-controller=pass cpuset cpu io memory hugetlb pids rdma misc dmem
system-manager-version=pass systemd 261 (261.2-1-arch)
page-size=pass 4096
socket-dir-writable=fail /run/sley-test-supervisor
pending-privileged:
  - caller SIGKILL peer-death cleanup
  - daemon SIGKILL crash cleanup
  - dynamic-UID and key isolation
  - manager runtime backstop
  - orphan restart reconciliation
  - pre-exec cgroup placement
  - system transient-unit launch

## Reading

The platform prerequisites visible without privilege all pass: cgroup2 is
mounted, the memory controller is enabled, systemd 261 supports system
transient units, and the page size is 4096. The socket-dir check fails
because nothing is installed yet — that is the pending privileged handoff
(`packaging/sley-test-supervisor/README.md`), not a waived prerequisite.
The seven privileged checks above were not attempted and are recorded as
pending, never as mocked passes.

//! Enforcer readiness probes.
//!
//! Probes are structured, evidence-carrying checks. The unprivileged subset
//! runs anywhere (cgroup layout, controller availability, manager version,
//! socket directory, page size) and is executed live by operators; the
//! privileged subset (pre-exec placement, transient units, UID/key
//! isolation, peer-death and daemon-crash cleanup, manager backstop) needs
//! the authenticated privilege handoff and is reported as pending, never
//! as a mocked pass.

use std::collections::BTreeSet;

/// One readiness check outcome with attached evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeCheck {
    /// Stable check name.
    pub name: &'static str,
    /// Whether the check passed.
    pub passed: bool,
    /// Human-readable evidence (paths, versions, digests, pending reason).
    pub evidence: String,
}

/// Complete readiness report: unprivileged checks plus privileged pending.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeReport {
    /// Unprivileged checks with live evidence.
    pub checks: Vec<ProbeCheck>,
    /// Privileged checks that need the authenticated handoff.
    pub pending_privileged: Vec<&'static str>,
}

/// Privileged checks deferred to the authenticated privilege handoff.
pub const PENDING_PRIVILEGED: [&str; 7] = [
    "pre-exec cgroup placement",
    "system transient-unit launch",
    "dynamic-UID and key isolation",
    "manager runtime backstop",
    "caller SIGKILL peer-death cleanup",
    "daemon SIGKILL crash cleanup",
    "orphan restart reconciliation",
];

/// Parses `/proc/self/mountinfo` content for a cgroup2 mount point.
///
/// Returns the mount point path when the filesystem type field is `cgroup2`.
#[must_use]
pub fn parse_cgroup2_mount(mountinfo: &str) -> Option<String> {
    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let Some(separator) = fields.iter().position(|field| *field == "-") else {
            continue;
        };
        if fields.get(separator + 1) == Some(&"cgroup2") {
            return fields.get(4).map(|path| (*path).to_owned());
        }
    }
    None
}

/// Parses `cgroup.controllers` content for the memory controller.
#[must_use]
pub fn parse_memory_controller(controllers: &str) -> bool {
    controllers
        .split_whitespace()
        .any(|entry| entry == "memory")
}

/// Parses `systemctl --version` output for the systemd major version.
#[must_use]
pub fn parse_systemd_version(output: &str) -> Option<u32> {
    let first = output.lines().next()?;
    let version = first.strip_prefix("systemd ")?;
    version.split_whitespace().next()?.parse().ok()
}

/// Parses `getconf PAGESIZE` output.
#[must_use]
pub fn parse_page_size(output: &str) -> Option<u64> {
    output.trim().parse().ok().filter(|size| *size > 0)
}

/// Runs one shell-free command and returns trimmed stdout on success.
fn run_capture(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Collects the unprivileged readiness checks on this host.
///
/// Every check carries live evidence; failures name the missing platform
/// prerequisite instead of waiving it.
#[must_use]
pub fn probe_unprivileged(socket_dir: &str, page_size_override: Option<u64>) -> ProbeReport {
    let mut checks = Vec::new();
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    let cgroup2 = parse_cgroup2_mount(&mountinfo);
    checks.push(ProbeCheck {
        name: "cgroup2-mounted",
        passed: cgroup2.is_some(),
        evidence: cgroup2.unwrap_or_else(|| "no cgroup2 mount in /proc/self/mountinfo".to_owned()),
    });
    let controllers_path = "/sys/fs/cgroup/cgroup.controllers";
    let controllers = std::fs::read_to_string(controllers_path).unwrap_or_default();
    let memory = parse_memory_controller(&controllers);
    checks.push(ProbeCheck {
        name: "memory-controller",
        passed: memory,
        evidence: if controllers.is_empty() {
            format!("{controllers_path} unreadable or empty")
        } else {
            controllers.trim().to_owned()
        },
    });
    let version_output = run_capture("systemctl", &["--version"]).unwrap_or_default();
    let version = parse_systemd_version(&version_output);
    checks.push(ProbeCheck {
        name: "system-manager-version",
        passed: version.is_some_and(|major| major >= 252),
        evidence: version_output
            .lines()
            .next()
            .unwrap_or("systemctl unavailable")
            .to_owned(),
    });
    let page_size = page_size_override.or_else(|| {
        run_capture("getconf", &["PAGESIZE"]).and_then(|output| parse_page_size(&output))
    });
    checks.push(ProbeCheck {
        name: "page-size",
        passed: page_size.is_some_and(u64::is_power_of_two),
        evidence: page_size.map_or_else(|| "unknown".to_owned(), |size| size.to_string()),
    });
    let socket_writable = std::fs::metadata(socket_dir)
        .map(|metadata| !metadata.permissions().readonly())
        .unwrap_or(false);
    checks.push(ProbeCheck {
        name: "socket-dir-writable",
        passed: socket_writable,
        evidence: socket_dir.to_owned(),
    });
    ProbeReport {
        checks,
        pending_privileged: PENDING_PRIVILEGED.to_vec(),
    }
}

/// Renders a probe report as stable `name=pass|fail evidence` lines plus the
/// pending privileged section.
#[must_use]
pub fn render_report(report: &ProbeReport) -> String {
    let mut out = String::new();
    for check in &report.checks {
        out.push_str(check.name);
        out.push('=');
        out.push_str(if check.passed { "pass" } else { "fail" });
        out.push(' ');
        out.push_str(&check.evidence);
        out.push('\n');
    }
    out.push_str("pending-privileged:\n");
    let mut pending = BTreeSet::new();
    for name in &report.pending_privileged {
        pending.insert(name);
    }
    for name in pending {
        out.push_str("  - ");
        out.push_str(name);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mountinfo_parser_finds_cgroup2() {
        let mountinfo = "30 23 0:26 / /sys/fs/cgroup ro,nosuid,nodev,noexec shared:4 - cgroup2 cgroup2 ro\n\
             31 23 0:27 / /sys/fs/cgroup/memory ro - cgroup cgroup ro,memory\n";
        assert_eq!(
            parse_cgroup2_mount(mountinfo),
            Some("/sys/fs/cgroup".to_owned())
        );
        assert_eq!(parse_cgroup2_mount("bogus\nno separator here\n"), None);
        assert_eq!(parse_cgroup2_mount(""), None);
    }

    #[test]
    fn controller_and_version_parsers_are_exact() {
        assert!(parse_memory_controller("cpu cpuset memory pids"));
        assert!(!parse_memory_controller("cpu cpuset pids"));
        assert!(!parse_memory_controller(""));
        assert_eq!(
            parse_systemd_version("systemd 261 (261.1-1)\n+PAM ..."),
            Some(261)
        );
        assert_eq!(parse_systemd_version("systemd 252\n"), Some(252));
        assert_eq!(parse_systemd_version("not systemd"), None);
        assert_eq!(parse_systemd_version(""), None);
        assert_eq!(parse_page_size("4096\n"), Some(4_096));
        assert_eq!(parse_page_size("0\n"), None);
        assert_eq!(parse_page_size("huge\n"), None);
    }

    #[test]
    fn report_rendering_pins_pending_privileged() {
        let report = ProbeReport {
            checks: vec![ProbeCheck {
                name: "cgroup2-mounted",
                passed: true,
                evidence: "/sys/fs/cgroup".to_owned(),
            }],
            pending_privileged: PENDING_PRIVILEGED.to_vec(),
        };
        let rendered = render_report(&report);
        assert!(rendered.starts_with("cgroup2-mounted=pass /sys/fs/cgroup\n"));
        assert!(rendered.contains("pending-privileged:\n"));
        for name in PENDING_PRIVILEGED {
            assert!(rendered.contains(name), "{name} listed");
        }
        assert_eq!(PENDING_PRIVILEGED.len(), 7);
    }
}

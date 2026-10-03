//! Conservative complete user-address-space working-memory bound on Linux.
//!
//! A precise resident baseline comes from `smaps_rollup`. Process `VmPeak` bounds
//! every later resident set, including stacks, allocator overhead/cache and raw
//! mappings. `VmPeak` minus that fixed baseline therefore bounds additional
//! resident working memory. It includes virtual slack and peaks before planning;
//! it is intentionally an upper bound, not an exact RSS delta. Checks precede
//! publication; this is not an instantaneous OS allocation/address-space cap.

use std::cell::Cell;
use std::fs::File;
use std::io::Read;

use serde_json::{Value, json};

use crate::error::Result;

const MAX_OBSERVATION_BYTES: u64 = 16_384;

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
}

/// Enables complete process-memory observations for the single-thread CLI.
/// Library callers remain explicitly unobserved unless they enable this mode.
/// On unsupported platforms an enabled observation refuses instead of claiming
/// that allocator-only measurement establishes the complete memory ceiling.
pub fn enable_current_thread() {
    ENABLED.with(|enabled| enabled.set(true));
}

pub(super) enum Monitor {
    Disabled,
    Unavailable,
    Active {
        baseline_resident: usize,
        peak_virtual: Cell<usize>,
        invalid: Cell<bool>,
    },
}

impl Monitor {
    pub(super) fn start() -> Self {
        if !ENABLED.with(Cell::get) {
            return Self::Disabled;
        }
        let Some((baseline_resident, peak_virtual)) = baseline() else {
            return Self::Unavailable;
        };
        Self::Active {
            baseline_resident,
            peak_virtual: Cell::new(peak_virtual),
            invalid: Cell::new(false),
        }
    }

    pub(super) fn check(&self, limit: usize) -> Result<()> {
        match self {
            Self::Disabled => Ok(()),
            Self::Unavailable => Err(super::limit(
                "complete planner working-memory observation unavailable; use ordinary AF1-X",
            )),
            Self::Active {
                baseline_resident,
                peak_virtual,
                invalid,
            } => {
                let now = observation("/proc/self/status", "VmPeak:");
                if invalid.get() || now.is_none_or(|now| now < peak_virtual.get()) {
                    invalid.set(true);
                    return Err(super::limit(
                        "complete planner working-memory observation became invalid; use ordinary AF1-X",
                    ));
                }
                peak_virtual.set(now.expect("checked observation"));
                if peak_virtual.get().saturating_sub(*baseline_resident) > limit {
                    return Err(super::limit(
                        "complete planner working-memory bound exceeds its aggregate ceiling; use ordinary AF1-X",
                    ));
                }
                Ok(())
            }
        }
    }

    pub(super) fn usage(&self, limit: usize) -> Value {
        let mut value = json!({
            "status":match self {Self::Disabled=>"not_enabled", Self::Unavailable=>"observation_unavailable", Self::Active{..}=>"complete_upper_bound_checked_at_budget_checkpoints"},
            "limit_additional_bytes":limit,
            "model":"process_peak_virtual_bytes_minus_precise_baseline_resident_bytes",
            "baseline_source":"/proc/self/smaps_rollup Rss",
            "peak_source":"/proc/self/status VmPeak",
            "includes":"resident user address space: heap, allocator cache/metadata, stacks and OS mappings",
            "overcounts":"unresident virtual slack and pre-planning virtual peaks; may require conservative fallback",
            "excludes":"kernel-owned memory outside the process user address space; ordinary trial after preparation",
            "enforcement":"aggregate checkpoint/prepublication refusal; individual allocations can overshoot; not an instantaneous OS cap",
        });
        if let Self::Active {
            baseline_resident,
            peak_virtual,
            invalid,
        } = self
        {
            let additional = peak_virtual.get().saturating_sub(*baseline_resident);
            value["baseline_resident_bytes"] = json!(baseline_resident);
            value["peak_virtual_bytes"] = json!(peak_virtual.get());
            value["additional_resident_upper_bound_bytes"] = json!(additional);
            value["exhausted"] = json!(additional > limit);
            value["observation_valid"] = json!(!invalid.get());
        }
        value
    }
}

fn baseline() -> Option<(usize, usize)> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    // Capture the resident baseline first; the later virtual peak also covers
    // any new pages allocated while obtaining these bounded observations.
    let resident = observation("/proc/self/smaps_rollup", "Rss:")?;
    let peak = observation("/proc/self/status", "VmPeak:")?;
    (resident <= peak).then_some((resident, peak))
}

fn observation(path: &str, field: &str) -> Option<usize> {
    let mut text = String::new();
    File::open(path)
        .ok()?
        .take(MAX_OBSERVATION_BYTES + 1)
        .read_to_string(&mut text)
        .ok()?;
    if text.len() > usize::try_from(MAX_OBSERVATION_BYTES).ok()? {
        return None;
    }
    field_bytes(&text, field)
}

fn field_bytes(text: &str, field: &str) -> Option<usize> {
    let mut found = None;
    for line in text.lines().filter(|line| line.starts_with(field)) {
        if found.is_some() {
            return None;
        }
        let parts: Vec<_> = line[field.len()..].split_whitespace().collect();
        if parts.len() != 2 || parts[1] != "kB" || !parts[0].bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        found = parts[0].parse::<usize>().ok()?.checked_mul(1024);
        found?;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::{Monitor, field_bytes};
    use std::cell::Cell;

    #[test]
    fn complete_memory_sources_require_exact_units_unique_fields_and_checked_bytes() {
        assert_eq!(field_bytes("VmPeak: 300 kB\n", "VmPeak:"), Some(307_200));
        assert_eq!(field_bytes("Rss: 0 kB\n", "Rss:"), Some(0));
        for text in [
            "",
            "Other: 12 kB",
            "Rss: 1 MB",
            "Rss: -1 kB",
            "Rss: +1 kB",
            "Rss: 1 kB extra",
            "Rss: 1 kB\nRss: 2 kB",
            "Rss: 18446744073709551615 kB",
        ] {
            assert_eq!(field_bytes(text, "Rss:"), None, "{text}");
        }
    }

    #[test]
    fn unavailable_complete_observation_refuses_and_disabled_library_mode_is_explicit() {
        assert!(Monitor::Unavailable.check(usize::MAX).is_err());
        assert_eq!(
            Monitor::Unavailable.usage(256)["status"],
            "observation_unavailable"
        );
        assert!(Monitor::Disabled.check(0).is_ok());
        assert_eq!(Monitor::Disabled.usage(256)["status"], "not_enabled");
    }

    #[test]
    fn invalid_complete_observation_is_sticky_without_fabricating_a_peak() {
        let monitor = Monitor::Active {
            baseline_resident: 1,
            peak_virtual: Cell::new(usize::MAX),
            invalid: Cell::new(false),
        };
        assert!(monitor.check(usize::MAX).is_err());
        assert!(monitor.check(usize::MAX).is_err());
        assert_eq!(monitor.usage(256)["observation_valid"], false);
        assert_eq!(
            monitor.usage(256)["additional_resident_upper_bound_bytes"],
            usize::MAX - 1
        );
    }
}

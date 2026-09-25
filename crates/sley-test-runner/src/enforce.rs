//! Checked enforcement math for memory caps, deadlines, and admission.
//!
//! All arithmetic is checked: overflow, zero wall budgets, zero page sizes,
//! and zero installed caps refuse before any worker launches. Boundary
//! equality follows the spec exactly: peak equal to the installed cap
//! passes, while elapsed equal to the wall budget refuses (the daemon
//! deadline is strictly less than `wall_ms * 1_000_000`).

/// Machine page size fallback in bytes when the supervisor config leaves it
/// to the platform default.
pub const DEFAULT_PAGE_SIZE: u64 = 4_096;

/// Daemon reap-confirmation budget after a deadline kill, in microseconds.
pub const REAP_BUDGET_USEC: u64 = 2_000_000;

/// Cleanup allowance added to the manager runtime backstop, in microseconds.
pub const CLEANUP_ALLOWANCE_USEC: u64 = 2_000_000;

/// Checked enforcement failure with a stable machine tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnforceError {
    /// Zero or overflowing input; refuses before launch.
    InvalidBudget,
    /// Installed cap cannot accommodate the request or is zero.
    UnaccommodatingCap,
    /// Measured peak or events violate the admitted evidence.
    EvidenceViolation,
    /// Elapsed time reached the strict wall deadline.
    DeadlineReached,
}

impl EnforceError {
    /// Frozen machine tag for logs and probe receipts.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::InvalidBudget => 1,
            Self::UnaccommodatingCap => 2,
            Self::EvidenceViolation => 3,
            Self::DeadlineReached => 4,
        }
    }
}

impl core::fmt::Display for EnforceError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidBudget => formatter.write_str("NATIVE_RUNNER_INVALID_BUDGET"),
            Self::UnaccommodatingCap => formatter.write_str("NATIVE_RUNNER_UNACCOMMODATING_CAP"),
            Self::EvidenceViolation => formatter.write_str("NATIVE_RUNNER_EVIDENCE_VIOLATION"),
            Self::DeadlineReached => formatter.write_str("NATIVE_RUNNER_DEADLINE_REACHED"),
        }
    }
}

impl std::error::Error for EnforceError {}

/// Largest page-aligned cap no greater than requested.
///
/// Records both values: the request the `TestCase` declared and the cap the
/// manager installs.
///
/// # Errors
///
/// Returns `InvalidBudget` for zero page size or request, and
/// `UnaccommodatingCap` when the request is below one page.
pub fn floor_page_cap(request_bytes: u64, page_size: u64) -> Result<(u64, u64), EnforceError> {
    if page_size == 0 || request_bytes == 0 {
        return Err(EnforceError::InvalidBudget);
    }
    let installed = (request_bytes / page_size) * page_size;
    if installed == 0 {
        return Err(EnforceError::UnaccommodatingCap);
    }
    Ok((request_bytes, installed))
}

/// Strict daemon deadline in nanoseconds: `wall_ms * 1_000_000`.
///
/// The elapsed check below requires strictly less than this bound.
///
/// # Errors
///
/// Returns `InvalidBudget` for zero or overflowing wall budgets.
pub fn deadline_ns(wall_ms: u64) -> Result<u64, EnforceError> {
    if wall_ms == 0 {
        return Err(EnforceError::InvalidBudget);
    }
    wall_ms
        .checked_mul(1_000_000)
        .ok_or(EnforceError::InvalidBudget)
}

/// Manager runtime backstop in microseconds: wall plus cleanup allowance.
///
/// Configured no later than the daemon deadline plus the bounded teardown
/// allowance.
///
/// # Errors
///
/// Returns `InvalidBudget` for zero or overflowing wall budgets.
pub fn runtime_max_usec(wall_ms: u64) -> Result<u64, EnforceError> {
    if wall_ms == 0 {
        return Err(EnforceError::InvalidBudget);
    }
    wall_ms
        .checked_mul(1_000)
        .and_then(|wall_usec| wall_usec.checked_add(CLEANUP_ALLOWANCE_USEC))
        .ok_or(EnforceError::InvalidBudget)
}

/// Requires `elapsed_ns` strictly below the wall budget in nanoseconds.
///
/// # Errors
///
/// Returns `InvalidBudget` for zero or overflowing wall budgets, and
/// `DeadlineReached` when elapsed meets or exceeds the bound.
pub fn check_elapsed(elapsed_ns: u64, wall_ms: u64) -> Result<(), EnforceError> {
    let bound = deadline_ns(wall_ms)?;
    if elapsed_ns < bound {
        Ok(())
    } else {
        Err(EnforceError::DeadlineReached)
    }
}

/// Requires `peak <= installed <= request` with zero limit/OOM/kill events.
///
/// Equality at the installed cap passes; one byte over refuses. Any recorded
/// memory event refuses even when the peak fits.
///
/// # Errors
///
/// Returns `UnaccommodatingCap` for zero or oversized installed caps, and
/// `EvidenceViolation` for over-peak measurements or nonzero events.
pub fn check_memory_evidence(
    peak_bytes: u64,
    installed_cap: u64,
    request_bytes: u64,
    max_events: u64,
    oom_events: u64,
    oom_kill_events: u64,
) -> Result<(), EnforceError> {
    if installed_cap == 0 || installed_cap > request_bytes {
        return Err(EnforceError::UnaccommodatingCap);
    }
    if peak_bytes > installed_cap {
        return Err(EnforceError::EvidenceViolation);
    }
    if max_events != 0 || oom_events != 0 || oom_kill_events != 0 {
        return Err(EnforceError::EvidenceViolation);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_floor_records_request_and_installed() {
        assert_eq!(floor_page_cap(8_192, 4_096), Ok((8_192, 8_192)));
        assert_eq!(floor_page_cap(8_193, 4_096), Ok((8_193, 8_192)));
        assert_eq!(
            floor_page_cap(4_095, 4_096),
            Err(EnforceError::UnaccommodatingCap)
        );
        assert_eq!(floor_page_cap(0, 4_096), Err(EnforceError::InvalidBudget));
        assert_eq!(floor_page_cap(8_192, 0), Err(EnforceError::InvalidBudget));
        assert_eq!(
            floor_page_cap(10_000_000_000, 4_096),
            Ok((10_000_000_000, 9_999_998_976))
        );
    }

    #[test]
    fn deadlines_are_strict_and_checked() {
        assert_eq!(deadline_ns(1), Ok(1_000_000));
        assert_eq!(deadline_ns(30_000), Ok(30_000_000_000));
        assert_eq!(deadline_ns(0), Err(EnforceError::InvalidBudget));
        assert_eq!(deadline_ns(u64::MAX), Err(EnforceError::InvalidBudget));
        assert_eq!(runtime_max_usec(30_000), Ok(32_000_000));
        assert_eq!(runtime_max_usec(0), Err(EnforceError::InvalidBudget));
        assert_eq!(runtime_max_usec(u64::MAX), Err(EnforceError::InvalidBudget));
        assert_eq!(check_elapsed(29_999_999_999, 30_000), Ok(()));
        // Equality at the bound refuses: strictly less than wall budget.
        assert_eq!(
            check_elapsed(30_000_000_000, 30_000),
            Err(EnforceError::DeadlineReached)
        );
    }

    #[test]
    fn memory_evidence_requires_exact_bounds_and_zero_events() {
        assert_eq!(check_memory_evidence(8_192, 8_192, 8_193, 0, 0, 0), Ok(()));
        assert_eq!(
            check_memory_evidence(8_193, 8_192, 8_193, 0, 0, 0),
            Err(EnforceError::EvidenceViolation)
        );
        assert_eq!(
            check_memory_evidence(8_192, 8_192, 8_193, 1, 0, 0),
            Err(EnforceError::EvidenceViolation)
        );
        assert_eq!(
            check_memory_evidence(8_192, 8_192, 8_193, 0, 1, 0),
            Err(EnforceError::EvidenceViolation)
        );
        assert_eq!(
            check_memory_evidence(8_192, 8_192, 8_193, 0, 0, 1),
            Err(EnforceError::EvidenceViolation)
        );
        assert_eq!(
            check_memory_evidence(8_192, 0, 8_193, 0, 0, 0),
            Err(EnforceError::UnaccommodatingCap)
        );
        assert_eq!(
            check_memory_evidence(8_192, 8_194, 8_193, 0, 0, 0),
            Err(EnforceError::UnaccommodatingCap)
        );
        assert_eq!(EnforceError::InvalidBudget.tag(), 1);
        assert_eq!(EnforceError::UnaccommodatingCap.tag(), 2);
        assert_eq!(EnforceError::EvidenceViolation.tag(), 3);
        assert_eq!(EnforceError::DeadlineReached.tag(), 4);
    }
}

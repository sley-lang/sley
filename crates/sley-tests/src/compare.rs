//! Factored expected-evidence comparison kernel.
//!
//! Factored out of `sley-conformance` in N8 so the lifecycle crates can use
//! the match rule without depending on the restricted envelopes: the rule
//! compares one hash-only expected projection against one observation and
//! nothing else. `sley-conformance` re-exports these items unchanged, so
//! the old restricted reports and the native reports cannot diverge on the
//! match rule. None of the [`RestrictedComparison`] arms means final test
//! pass; see `docs/spec/REPORT_ENVELOPE_PROFILE_V1.md` section 9.1.

use sley_id::ValueHash;

/// Hash-only expected `TestCase` outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedEvidence {
    /// Exact expected value hash.
    Value(ValueHash),
    /// Exact frozen trap code.
    FailureCode(u32),
}

/// Restricted comparison only; none of these arms means final test pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestrictedComparison {
    /// Expected and observed restricted projections match.
    Match,
    /// Expected and observed restricted projections differ.
    Mismatch,
    /// Execution was rejected before an observation existed.
    ExecutionRejected,
}

impl RestrictedComparison {
    /// Frozen wire tag (`docs/spec/REPORT_ENVELOPE_PROFILE_V1.md`): 1
    /// match, 2 mismatch, 3 execution-rejected.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Match => 1,
            Self::Mismatch => 2,
            Self::ExecutionRejected => 3,
        }
    }
}

/// Compares one expected projection against one observation.
///
/// An unobserved execution is execution-rejected; otherwise an exact hash
/// or trap-code equality decides match versus mismatch.
#[must_use]
pub fn compare_expected_evidence(
    expected: ExpectedEvidence,
    observed_success: Option<ValueHash>,
    observed_trap_tag: Option<u32>,
    observed: bool,
) -> RestrictedComparison {
    if !observed {
        return RestrictedComparison::ExecutionRejected;
    }
    let matches = match expected {
        ExpectedEvidence::Value(want) => observed_success == Some(want),
        ExpectedEvidence::FailureCode(want) => observed_trap_tag == Some(want),
    };
    if matches {
        RestrictedComparison::Match
    } else {
        RestrictedComparison::Mismatch
    }
}

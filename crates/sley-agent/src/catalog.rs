//! Decoded refusals: decision and phase names, retryability, the checked-in
//! hint catalog (`data/hints.json`), and rendered locators.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::OnceLock;

use serde_json::{Value, json};
use sley_policy::{CandidateValidationOutput, RefusalLocator};

use crate::explain;
use crate::names::Names;
use crate::workspace::Program;

const HINTS: &str = include_str!("../data/hints.json");

/// The phase names of `CANDIDATE_RESULT_V1.md` section 6.
#[must_use]
pub const fn phase_name(phase: u32) -> &'static str {
    match phase {
        1 => "frame",
        2 => "schema/limits",
        3 => "stale base/preimage",
        4 => "identity",
        5 => "graph/references",
        6 => "type",
        7 => "control flow",
        8 => "effects",
        9 => "capability/policy",
        10 => "contracts",
        11 => "test plan",
        12 => "resource analysis",
        13 => "root build",
        14 => "final digest",
        _ => "unknown",
    }
}

/// The reference-graph relationship names (phase 5 locators).
#[must_use]
pub const fn relationship_name(tag: u32) -> &'static str {
    match tag {
        1 => "member",
        2 => "type",
        3 => "value",
        4 => "block target",
        5 => "function reference",
        6 => "effect",
        7 => "capability requirement",
        8 => "contract",
        9 => "initializer",
        10 => "test target",
        11 => "adapter",
        12 => "type definition",
        _ => "reference",
    }
}

struct Catalog {
    symbols: BTreeMap<String, String>,
    decisions: BTreeMap<String, String>,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let value: Value = serde_json::from_str(HINTS).unwrap_or(Value::Null);
        let table = |key: &str| -> BTreeMap<String, String> {
            value
                .get(key)
                .and_then(Value::as_object)
                .map(|object| {
                    object
                        .iter()
                        .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
                        .collect()
                })
                .unwrap_or_default()
        };
        Catalog {
            symbols: table("symbols"),
            decisions: table("decisions"),
        }
    })
}

/// The remediation hint for a symbol, falling back to the decision's hint,
/// and to "no hint" when neither is catalogued.
#[must_use]
pub fn hint(symbol: &str, decision: &str) -> String {
    let catalog = catalog();
    catalog
        .symbols
        .get(symbol)
        .or_else(|| catalog.decisions.get(decision))
        .cloned()
        .unwrap_or_else(|| "no hint".to_owned())
}

/// Whether the catalog names a symbol (tests and `help refusals`).
#[must_use]
pub fn has_symbol_hint(symbol: &str) -> bool {
    catalog().symbols.contains_key(symbol)
}

/// Every catalogued symbol with its hint.
#[must_use]
pub fn symbol_hints() -> Vec<(String, String)> {
    catalog()
        .symbols
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// One decoded validation verdict.
#[derive(Clone, Debug)]
pub struct Verdict {
    /// Whether all fourteen phases passed.
    pub valid: bool,
    /// Decision name (`Valid`, `ResourceLimit`, ...).
    pub decision: String,
    /// Failed phase, for refusals.
    pub phase: Option<u32>,
    /// Source symbol, for refusals.
    pub symbol: Option<String>,
    /// Source numeric code, when the owner assigns one.
    pub code: Option<u32>,
    /// Retryability name, for refusals.
    pub retry: Option<String>,
    /// Remediation hint, for refusals.
    pub hint: Option<String>,
    /// Rendered locator (`where`), when the refusing phase knows one.
    pub location: Option<String>,
    /// `TestCases` the kernel selected for this candidate.
    pub selected_tests: usize,
}

impl Verdict {
    /// Decodes a validation output. `program` is the post-candidate state
    /// used to name the entities a locator mentions.
    #[must_use]
    pub fn of(output: &CandidateValidationOutput, program: &Program, names: &Names) -> Self {
        let record = &output.result().record;
        let decision = format!("{:?}", record.decision);
        let selected_tests = record.selected_tests.len();
        if output.is_valid() {
            return Self {
                valid: true,
                decision,
                phase: None,
                symbol: None,
                code: None,
                retry: None,
                hint: None,
                location: None,
                selected_tests,
            };
        }
        let diagnostic = record.diagnostics.first();
        let symbol = diagnostic.map(|d| d.source_symbol.clone());
        let phase = diagnostic.map(|d| d.phase_tag);
        let location = output.refusal_locator().map(|locator| {
            render_locator(locator, symbol.as_deref().unwrap_or(""), program, names)
        });
        Self {
            valid: false,
            hint: Some(hint(symbol.as_deref().unwrap_or(""), &decision)),
            decision,
            phase,
            code: diagnostic.and_then(|d| d.source_numeric_code),
            retry: diagnostic.map(|d| format!("{:?}", d.retryability)),
            symbol,
            location,
            selected_tests,
        }
    }

    /// Renders the verdict as JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        if self.valid {
            return json!({"valid": true, "decision": self.decision, "selected_tests": self.selected_tests});
        }
        json!({
            "valid": false,
            "decision": self.decision,
            "phase": self.phase,
            "phase_name": self.phase.map(phase_name),
            "symbol": self.symbol,
            "code": self.code,
            "retry": self.retry,
            "where": self.location,
            "hint": self.hint,
        })
    }

    /// The one-line summary (`Valid`, `REFUSED ResourceLimit at phase 12 ...`).
    #[must_use]
    pub fn headline(&self) -> String {
        if self.valid {
            return "Valid".to_owned();
        }
        format!(
            "REFUSED {} at phase {} ({})",
            self.decision,
            self.phase.unwrap_or(0),
            self.phase.map_or("unknown", phase_name),
        )
    }

    /// The indented detail lines of a refusal (empty when Valid).
    #[must_use]
    pub fn details(&self) -> String {
        if self.valid {
            return String::new();
        }
        let mut text = format!("  symbol: {}", self.symbol.as_deref().unwrap_or("none"));
        if let Some(code) = self.code {
            let _ = write!(text, " ({code})");
        }
        text.push('\n');
        if let Some(location) = &self.location {
            let _ = writeln!(text, "  where: {location}");
        }
        if let Some(hint) = &self.hint {
            let _ = writeln!(text, "  hint: {hint}");
        }
        text
    }

    /// Renders the verdict as compact text lines.
    #[must_use]
    pub fn to_text(&self) -> String {
        let details = self.details();
        if details.is_empty() {
            self.headline()
        } else {
            format!("{}\n{}", self.headline(), details.trim_end())
        }
    }
}

/// Renders a locator in terms of local names:
/// `test t3 .resource_limits.memory_bytes 1000000 > ceiling 1000`.
#[must_use]
pub fn render_locator(
    locator: &RefusalLocator,
    symbol: &str,
    program: &Program,
    names: &Names,
) -> String {
    let mut parts = Vec::new();
    if let Some(operation) = locator.operation {
        parts.push(format!("operation {operation}"));
    }
    if let Some(subject) = &locator.subject {
        let kind = program
            .body(subject)
            .map_or("Entity", |body| crate::names::kind_name(body.kind_tag()));
        parts.push(format!("{kind} {}", names.name(subject)));
    }
    if let Some(field) = locator.field {
        parts.push(format!(".{field}"));
    }
    // Policy-grant ceilings are the principal's; the rest are profile limits.
    let granted = locator.field.is_some_and(|field| {
        matches!(
            field,
            "resource_limits.fuel"
                | "resource_limits.memory_bytes"
                | "resource_limits.output_bytes"
                | "resource_limits.effect_count"
                | "max_mutation_count"
        )
    });
    match (locator.requested, locator.ceiling) {
        (Some(requested), Some(ceiling)) if granted => {
            parts.push(format!("{requested} > grant ceiling {ceiling}"));
        }
        (Some(requested), Some(ceiling)) => parts.push(format!("{requested} > ceiling {ceiling}")),
        (Some(requested), None) => parts.push(format!("= {requested}")),
        _ => {}
    }
    if let Some(related) = &locator.related {
        let relation = locator.relationship.map_or("references", relationship_name);
        let state = if program.contains(related) {
            ""
        } else {
            " (not live)"
        };
        parts.push(format!("{relation} -> {}{state}", names.name(related)));
    }
    let mut text = parts.join(" ");
    if locator.subject.is_some()
        && locator.field.is_none()
        && locator.related.is_none()
        && let Some(detail) = explain::detail(symbol, locator, program, names)
    {
        text.push_str(": ");
        text.push_str(&detail);
    }
    text
}

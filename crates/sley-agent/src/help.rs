//! `sley-agent help [topic]`: the agent guide and on-demand reference.
//!
//! The guide (`data/GUIDE.md`, at most 3,500 bytes) covers the workflow; the
//! schema-sized material loads only when asked for. Every example in these
//! texts is executed by the crate's tests.

use std::fmt::Write as _;

use crate::catalog;
use crate::opcodes::{ImmediateKind, OPCODES};

/// The agent guide.
pub const GUIDE: &str = include_str!("../data/GUIDE.md");
/// The AF1 reference.
pub const AF1: &str = include_str!("../data/help-af1.md");
/// The AF1-X (authoring dialect) reference.
pub const AFX: &str = include_str!("../data/help-afx.md");
/// The concise AF1-X reference with an executable collection fold.
/// Served by default; the full contract is available as `afx-reference`.
pub const AFX_QUICK: &str = include_str!("../data/help-afx-quick.md");
/// The type shorthand reference.
pub const TYPES: &str = include_str!("../data/help-types.md");
/// The `TestCase` reference.
pub const TESTS: &str = include_str!("../data/help-tests.md");
/// The drafts and delta-repair reference.
pub const DRAFTS: &str = include_str!("../data/help-drafts.md");
/// The verified search reference.
pub const SEARCH: &str = include_str!("../data/help-search.md");
/// The implemented residual authoring commands and fragment contracts.
pub const RESIDUAL: &str = include_str!("../data/help-residual.md");
/// The default residual guide; the full contract is loaded on demand.
pub const RESIDUAL_QUICK: &str = include_str!("../data/help-residual-quick.md");

/// Structured function bodies (`"body"` instead of `"blocks"`).
pub const STRUCTURED: &str = include_str!("../data/help-structured.md");

/// The optional familiar authoring syntax (ADR-0055).
#[cfg(feature = "familiar")]
pub const FAMILIAR: &str = include_str!("../data/help-familiar.md");
/// What `help familiar` says when the frontend is not built in.
#[cfg(not(feature = "familiar"))]
pub const FAMILIAR: &str = "# Familiar syntax\nThis sley-agent was built without the optional familiar frontend \
(cargo feature `familiar`). Author with JSON frames: `help structured`, `help afx`.\n";

/// Topic names.
pub const TOPICS: &[&str] = &[
    "guide",
    "af1",
    "afx",
    "afx-quick",
    "afx-reference",
    "drafts",
    "opcodes",
    "types",
    "tests",
    "search",
    "structured",
    "familiar",
    "residual",
    "residual-quick",
    "residual-reference",
    "refusals",
];

/// Returns a topic's text.
#[must_use]
pub fn topic(name: &str) -> Option<String> {
    Some(match name {
        "" | "guide" => GUIDE.to_owned(),
        "af1" | "frames" => AF1.to_owned(),
        "afx" | "af1-x" | "dialect" | "afx-quick" => AFX_QUICK.to_owned(),
        "afx-reference" => AFX.to_owned(),
        "types" => TYPES.to_owned(),
        "tests" | "testcase" => TESTS.to_owned(),
        "drafts" | "draft" | "fill" => DRAFTS.to_owned(),
        "search" => SEARCH.to_owned(),
        "structured" | "body" => STRUCTURED.to_owned(),
        "familiar" => FAMILIAR.to_owned(),
        "residual" | "residual-quick" => RESIDUAL_QUICK.to_owned(),
        "residual-reference" => RESIDUAL.to_owned(),
        "opcodes" => opcodes(),
        "refusals" => refusals(),
        _ => return None,
    })
}

fn opcodes() -> String {
    let mut text = String::from(
        "# Opcodes (AF1 op: [\"name\", \"opcode\", immediate?, operands...])\n\
         Result types are derived; add {\"type\": ...} (object form) only when asked.\n\n",
    );
    for row in OPCODES {
        let immediate = match row.immediate {
            ImmediateKind::None => "",
            ImmediateKind::Entity if row.tag == 1 => " <constant name | literal>",
            ImmediateKind::Entity if row.tag == 18 => " <Type>",
            ImmediateKind::Entity => " <entity>",
            ImmediateKind::Index => " <index>",
            ImmediateKind::Field => " <Type.field>",
            ImmediateKind::Variant => " <Type.Case>",
            ImmediateKind::Observation => " <observation id>",
            ImmediateKind::Function => " <function>",
        };
        let _ = writeln!(
            text,
            "{:>3} {:<12}{immediate}  ({})",
            row.tag, row.mnemonic, row.name
        );
    }
    text.push_str(
        "\nChecked integer ops (add sub mul div rem neg shl shr) return Result<T,ArithmeticError>;\n\
         branch on them with [\"switch\", v, [\"Ok\", \"blk\", \"$\"], [\"Err\", \"overflow\"]].\n\
         div truncates toward zero. Comparisons and not/and/or return bool.\n",
    );
    text
}

fn refusals() -> String {
    let mut text = String::from(
        "# Refusal symbols and hints\n\
         A refused `try` prints the decision, phase, symbol, where, and a hint.\n\
         `sley-agent explain <handle>` repeats it with the affected functions.\n\n",
    );
    for (symbol, hint) in catalog::symbol_hints() {
        let _ = writeln!(text, "{symbol}\n  {hint}");
    }
    text
}

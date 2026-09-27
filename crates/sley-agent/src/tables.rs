//! Compact test tables (`test_tables`, AF1-X frames only).
//!
//! A table names its target function and default limits once; each row
//! gives arguments and an independently written expectation. Every row
//! lowers to one ordinary AF1 `tests` entry, so argument and result
//! encodings, grant clamping and the explicit-limit refusal stay the AF1
//! test rules. A row is named `row.name`, or `<table>_<i>` after its
//! position; the source map leads each lowered test back to its row.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::afx::{MapEntry, Obligation, Role, SourceMap};
use crate::error::AgentErrorCode;
use crate::names::{NAME_GRAMMAR, is_identifier};

const TABLE_KEYS: &[&str] = &["name", "fn", "defaults", "cases", "comment"];
const ROW_KEYS: &[&str] = &["name", "args", "expect", "limits", "comment"];

/// The test name a row makes: its own `name`, or `<table>_<i>`; `None`
/// for a `name` that is not a name.
fn row_name(table: &str, index: usize, row: &Map<String, Value>) -> Option<String> {
    match row.get("name") {
        None => Some(format!("{table}_{index}")),
        Some(Value::String(given)) if is_identifier(given) => Some(given.clone()),
        Some(_) => None,
    }
}

/// One test a table row makes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RowTest {
    /// The table's name.
    pub table: String,
    /// The test's name.
    pub name: String,
    /// Where the name is written: the row's `name`, or the row itself for a
    /// derived name.
    pub at: String,
}

/// The named tables of a frame's `test_tables`, and the test each of their
/// rows makes (rows whose name is not a name are left to `lower`).
pub(crate) fn row_tests(frame: &Value) -> (Vec<String>, Vec<RowTest>) {
    let mut tables = Vec::new();
    let mut rows = Vec::new();
    let listed = frame.get("test_tables").and_then(Value::as_array);
    for (t, table) in listed.into_iter().flatten().enumerate() {
        let Some(name) = table
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| is_identifier(name))
        else {
            continue;
        };
        tables.push(name.to_owned());
        let cases = table.get("cases").and_then(Value::as_array);
        for (i, row) in cases.into_iter().flatten().enumerate() {
            let Some(row) = row.as_object() else {
                continue;
            };
            let Some(test) = row_name(name, i, row) else {
                continue;
            };
            let at = if row.contains_key("name") {
                format!("/test_tables/{t}/cases/{i}/name")
            } else {
                format!("/test_tables/{t}/cases/{i}")
            };
            rows.push(RowTest {
                table: name.to_owned(),
                name: test,
                at,
            });
        }
    }
    (tables, rows)
}

/// Lowers `frame["test_tables"]` into `out["tests"]` (after the frame's own
/// tests); returns the number of rows lowered.
#[allow(clippy::too_many_lines)]
pub(crate) fn lower(
    frame: &Map<String, Value>,
    out: &mut Map<String, Value>,
    map: &mut SourceMap,
    obligations: &mut Vec<Obligation>,
) -> u64 {
    let Some(tables) = frame.get("test_tables") else {
        return 0;
    };
    let mut invalid = |at: &str, decision: String| {
        obligations.push(Obligation::new(
            AgentErrorCode::TestTableInvalid,
            at,
            decision,
        ));
    };
    let Some(tables) = tables.as_array() else {
        invalid(
            "/test_tables",
            "test_tables is a list of tables: [{\"name\", \"fn\", \"cases\": [...]}]".to_owned(),
        );
        return 0;
    };
    let mut names: BTreeSet<String> = frame
        .get("tests")
        .and_then(Value::as_array)
        .map(|tests| {
            tests
                .iter()
                .filter_map(|test| test.get("name").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let mut tables_seen = BTreeSet::new();
    let mut lowered: Vec<Value> = Vec::new();
    let base = out
        .get("tests")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    for (t, table) in tables.iter().enumerate() {
        let at = format!("/test_tables/{t}");
        let Some(table) = table.as_object() else {
            invalid(
                &at,
                "a table is an object {\"name\", \"fn\", \"cases\"}".to_owned(),
            );
            continue;
        };
        let mut valid = true;
        for key in table.keys() {
            if !TABLE_KEYS.contains(&key.as_str()) {
                invalid(
                    &format!("{at}/{key}"),
                    format!("unknown table key (a table has {})", TABLE_KEYS.join(", ")),
                );
                valid = false;
            }
        }
        let name = table.get("name").and_then(Value::as_str);
        let Some(name) = name.filter(|name| is_identifier(name)) else {
            invalid(
                &at,
                format!(
                    "a table needs a \"name\" ({NAME_GRAMMAR}); layering replaces a table by it"
                ),
            );
            continue;
        };
        if !tables_seen.insert(name.to_owned()) {
            invalid(&at, format!("table `{name}` is declared twice"));
            continue;
        }
        let Some(function) = table.get("fn").and_then(Value::as_str) else {
            invalid(&at, "missing \"fn\": the function the rows test".to_owned());
            continue;
        };
        let Some(cases) = table.get("cases").and_then(Value::as_array) else {
            invalid(
                &at,
                "missing \"cases\": a list of {\"args\", \"expect\"} rows".to_owned(),
            );
            continue;
        };
        let defaults = match table.get("defaults") {
            None => Map::new(),
            Some(Value::Object(defaults)) => {
                if let Some(key) = defaults.keys().find(|key| key.as_str() != "limits") {
                    invalid(
                        &format!("{at}/defaults/{key}"),
                        "unknown defaults key (defaults hold \"limits\")".to_owned(),
                    );
                    continue;
                }
                match defaults.get("limits") {
                    None => Map::new(),
                    Some(Value::Object(limits)) => limits.clone(),
                    Some(_) => {
                        invalid(
                            &format!("{at}/defaults/limits"),
                            "limits are an object, e.g. {\"fuel\": 10000}".to_owned(),
                        );
                        continue;
                    }
                }
            }
            Some(_) => {
                invalid(
                    &format!("{at}/defaults"),
                    "defaults are an object: {\"limits\": {...}}".to_owned(),
                );
                continue;
            }
        };
        if !valid {
            continue;
        }
        let mut seen_args: Vec<(usize, Value)> = Vec::new();
        for (i, row) in cases.iter().enumerate() {
            let row_at = format!("{at}/cases/{i}");
            let Some(row) = row.as_object() else {
                invalid(
                    &row_at,
                    "a row is an object {\"args\": [...], \"expect\": v}".to_owned(),
                );
                continue;
            };
            if let Some(key) = row.keys().find(|key| !ROW_KEYS.contains(&key.as_str())) {
                invalid(
                    &format!("{row_at}/{key}"),
                    format!("unknown row key (a row has {})", ROW_KEYS.join(", ")),
                );
                continue;
            }
            let Some(test_name) = row_name(name, i, row) else {
                invalid(
                    &format!("{row_at}/name"),
                    format!("a row name is a name ({NAME_GRAMMAR})"),
                );
                continue;
            };
            let args = row.get("args").cloned().unwrap_or(Value::Array(Vec::new()));
            if let Some((first, _)) = seen_args.iter().find(|(_, seen)| *seen == args) {
                invalid(
                    &row_at,
                    format!("rows {first} and {i} of table `{name}` have the same args: keep one"),
                );
                continue;
            }
            seen_args.push((i, args));
            if !names.insert(test_name.clone()) {
                invalid(
                    &row_at,
                    format!(
                        "the row's test name `{test_name}` is taken by another test of this frame: give the row a \"name\""
                    ),
                );
                continue;
            }
            let mut test = Map::new();
            test.insert("name".to_owned(), Value::from(test_name.clone()));
            test.insert("fn".to_owned(), Value::from(function));
            for key in ["args", "expect"] {
                if let Some(value) = row.get(key) {
                    test.insert(key.to_owned(), value.clone());
                }
            }
            let expanded = format!("/tests/{}", base + lowered.len());
            let mut limits = defaults.clone();
            let mut from_defaults: Vec<String> = defaults.keys().cloned().collect();
            match row.get("limits") {
                None => {}
                Some(Value::Object(own)) => {
                    for (key, value) in own {
                        limits.insert(key.clone(), value.clone());
                        from_defaults.retain(|default| default != key);
                    }
                }
                Some(_) => {
                    invalid(
                        &format!("{row_at}/limits"),
                        "limits are an object, e.g. {\"fuel\": 10000}".to_owned(),
                    );
                    continue;
                }
            }
            if !limits.is_empty() {
                test.insert("limits".to_owned(), Value::Object(limits));
            }
            map.entries.push(MapEntry {
                expanded: expanded.clone(),
                authored: row_at,
                role: Role::TableRow,
                name: test_name.clone(),
            });
            map.entries.push(MapEntry {
                expanded: format!("{expanded}/fn"),
                authored: format!("{at}/fn"),
                role: Role::TableRow,
                name: test_name.clone(),
            });
            for key in from_defaults {
                map.entries.push(MapEntry {
                    expanded: format!("{expanded}/limits/{key}"),
                    authored: format!("{at}/defaults/limits/{key}"),
                    role: Role::TableRow,
                    name: test_name.clone(),
                });
            }
            lowered.push(Value::Object(test));
        }
    }
    let count = lowered.len() as u64;
    if !lowered.is_empty() {
        let tests = out
            .entry("tests".to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(tests) = tests {
            tests.extend(lowered);
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(frame: &Value) -> (Map<String, Value>, SourceMap, Vec<Obligation>, u64) {
        let frame = frame.as_object().unwrap().clone();
        let mut out = frame.clone();
        let mut map = SourceMap::default();
        let mut obligations = Vec::new();
        let rows = lower(&frame, &mut out, &mut map, &mut obligations);
        (out, map, obligations, rows)
    }

    #[test]
    fn rows_lower_after_the_frames_own_tests() {
        let (out, map, obligations, rows) = run(&json!({
            "tests": [{"name": "t0", "fn": "f", "args": [1], "expect": 1}],
            "test_tables": [{"name": "t_f", "fn": "f", "defaults": {"limits": {"fuel": 500, "call_depth": 8}},
                "cases": [{"args": [2], "expect": 4}, {"name": "big", "args": [3], "expect": 9, "limits": {"fuel": 900}}]}]}));
        assert!(obligations.is_empty(), "{obligations:?}");
        assert_eq!(rows, 2);
        assert_eq!(
            out["tests"],
            json!([{"name": "t0", "fn": "f", "args": [1], "expect": 1},
                   {"name": "t_f_0", "fn": "f", "args": [2], "expect": 4, "limits": {"fuel": 500, "call_depth": 8}},
                   {"name": "big", "fn": "f", "args": [3], "expect": 9, "limits": {"fuel": 900, "call_depth": 8}}])
        );
        assert_eq!(
            map.authored("/tests/2/args/0").as_deref(),
            Some("/test_tables/0/cases/1/args/0")
        );
        assert_eq!(
            map.authored("/tests/1/limits/fuel").as_deref(),
            Some("/test_tables/0/defaults/limits/fuel")
        );
        assert_eq!(
            map.authored("/tests/2/limits/fuel").as_deref(),
            Some("/test_tables/0/cases/1/limits/fuel")
        );
    }

    #[test]
    fn malformed_tables_are_refused_with_the_row() {
        let (_, _, obligations, rows) = run(&json!({
            "tests": [{"name": "t_f_1", "fn": "f", "args": [9], "expect": 9}],
            "test_tables": [
                {"name": "t_f", "fn": "f", "cases": [{"args": [1], "expect": 1}, {"args": [2], "expect": 2}, {"args": [1], "expect": 3}]},
                {"name": "u", "cases": []},
                {"name": "v", "fn": "f"},
                {"name": "w", "fn": "f", "cases": [{"args": [], "expect": 0, "wat": 1}], "extra": true}]}));
        let lines: Vec<(AgentErrorCode, String)> = obligations
            .iter()
            .map(|o| (o.symbol, o.at.clone()))
            .collect();
        assert!(
            lines
                .iter()
                .all(|(symbol, _)| *symbol == AgentErrorCode::TestTableInvalid)
        );
        let at: Vec<&str> = lines.iter().map(|(_, at)| at.as_str()).collect();
        assert_eq!(
            at,
            [
                "/test_tables/0/cases/1",
                "/test_tables/0/cases/2",
                "/test_tables/1",
                "/test_tables/2",
                "/test_tables/3/extra"
            ]
        );
        assert_eq!(rows, 1);
    }
}

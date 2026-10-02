//! The ordinary builder permits one body restatement per function. Multiple
//! operation edits form one restatement; other duplicate owners cannot compose.

use crate::{error::Result, residual::frontier::Budget};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub(super) fn check(
    declarations: &Map<String, Value>,
    replaced: Option<&str>,
    budget: &mut Budget,
) -> Result<()> {
    let mut owners = BTreeMap::new();
    for key in ["fns", "functions", "patch", "edit"] {
        let Some(items) = declarations.get(key).and_then(Value::as_array) else {
            continue;
        };
        for (index, item) in items.iter().enumerate() {
            budget.checkpoint()?;
            let name = item
                .get("fn")
                .or_else(|| (key == "edit").then(|| item.get("function")).flatten())
                .or_else(|| item.get("name"))
                .and_then(Value::as_str);
            let Some(name) = name.filter(|name| Some(*name) != replaced) else {
                continue;
            };
            let at = format!("/{key}/{index}");
            if let Some((prior_kind, prior_at)) = owners.get(name) {
                if key == "edit" && *prior_kind == "edit" {
                    continue;
                }
                return Err(super::conflict(
                    &at,
                    &format!(
                        "function `{name}` is restated more than once: conflicts with {prior_at}; fns/functions, patch and each edit group must use distinct function owners"
                    ),
                ));
            }
            owners.insert(name, (key, at));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AgentErrorCode;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn restatement_scan_preserves_target_replacement_and_groups_edit_aliases() {
        let value = json!({"fns":[{"fn":"replaced"}],"patch":[{"name":"replaced"}],
            "edit":[{"fn":"helper"},{"function":"helper"},{"name":"helper"}]});
        let source = value.as_object().unwrap();
        check(source, Some("replaced"), &mut Budget::default()).unwrap();
        let error = check(source, None, &mut Budget::default()).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("/fns/0"), "{error}");
        assert!(error.detail().contains("/patch/0"), "{error}");
        // Unknown names remain the caller's existing deferral obligation.
        check(
            json!({"fns":[{}],"patch":[{}]}).as_object().unwrap(),
            None,
            &mut Budget::default(),
        )
        .unwrap();
    }

    #[test]
    fn restatement_scan_charges_each_entry_without_resetting_shared_work() {
        let value = json!({"fns":[{"fn":"one"}],"patch":[{"fn":"two"}],
            "edit":[{"fn":"three"},{"name":"three"}]});
        let mut budget = Budget::limited(Duration::from_secs(2), 4);
        check(value.as_object().unwrap(), None, &mut budget).unwrap();
        assert_eq!(budget.usage()["charged_work"], 4);
        for _ in 0..2 {
            let error = check(value.as_object().unwrap(), None, &mut budget).unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
        }
    }
}

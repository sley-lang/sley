//! `try --on <handle>`: a frame layered on the frame a stored candidate was
//! made from, so a follow-up (tests, a patch, an edit) never restates the
//! whole frame.
//!
//! Layering is plain JSON work on AF1 frames; the result is an ordinary
//! frame that `try` compiles against the head like any other.
//!
//! - `types`, `consts`, `fns` and named `tests`: an entry replaces the base
//!   entry of the same name; others are appended.
//! - `patch`: on a function the base defines in `fns`, the patch is applied
//!   to that definition (a block replaces the block of its name, `null`
//!   deletes it, `params` and `returns` replace); on a function the base
//!   patches, the two patches merge; otherwise it is appended.
//! - `edit`: on a function the base defines or patches, the operation is
//!   replaced in that block; an edit of the same operation replaces the base
//!   edit; otherwise it is appended.
//! - `test_tables`: a table replaces the base table of the same name.
//! - `ripple`: an intent replaces the base intent of the same kind and
//!   target (`arity` of the same function; `guard` with the same checker
//!   and parameter) where it stands; others are appended in order. Only
//!   the base's intents are replaced, never one the same follow-up states.
//!   When either side has several intents of one identity, an intent
//!   replaces the base intent of that identity naming the same functions
//!   (`in`); one naming others is appended once every base intent of that
//!   identity is replaced, and is otherwise refused as ambiguous. An
//!   `arity` intent the follow-up does not restate records, in `after`, the
//!   functions, tests and test tables the follow-up states (a test without
//!   a name as [`UNNAMED_TESTS`]): its `frame_calls` does not cover them. A
//!   restating intent keeps that list, unless it restates the function's
//!   parameters (a new change) or says `frame_calls` itself (which then
//!   covers everything the frame states).
//! - `delete` entries are added; `namespace` replaces.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::error::{AgentError, Result, frame};

/// How an intent's `after` lists the tests without a name a follow-up
/// states: a test without a name cannot be told from another, so this one
/// entry stands for all of them.
pub(crate) const UNNAMED_TESTS: &str = "(unnamed)";

/// `delta` layered on `base`, both AF1 frames.
///
/// # Errors
///
/// `AGENT_FRAME_INVALID` when either is not an AF1 object frame.
pub fn layer(base: &Value, delta: &Value) -> Result<Value> {
    let base = base
        .as_object()
        .ok_or_else(|| frame("", "the base candidate's frame is not an AF1 object"))?;
    let delta = delta
        .as_object()
        .ok_or_else(|| frame("", "try --on takes an AF1 frame object, not raw operations"))?;
    if delta.get("af1") != Some(&Value::from(1)) {
        return Err(frame("/af1", "an AF1 frame starts with \"af1\": 1"));
    }
    let stated = stated_names(delta);
    let mut merged = base.clone();
    // `functions` is an alias of `fns`: layer on one list.
    if let Some(functions) = merged.remove("functions") {
        list(&mut merged, "fns").extend(functions.as_array().cloned().unwrap_or_default());
    }
    for (key, value) in delta {
        let entries = value.as_array().cloned().unwrap_or_default();
        match key.as_str() {
            // Intents are layered below, where a restated intent replaces
            // the same intent in place.
            "af1" | "ripple" => {}
            "types" | "consts" | "tests" | "test_tables" => {
                replace_named(list(&mut merged, key), entries, "name");
            }
            "fns" | "functions" => replace_named(list(&mut merged, "fns"), entries, "fn"),
            "delete" => {
                let deletes = list(&mut merged, "delete");
                for entry in entries {
                    if !deletes.contains(&entry) {
                        deletes.push(entry);
                    }
                }
            }
            "patch" => {
                for entry in entries {
                    apply_patch(&mut merged, entry);
                }
            }
            "edit" => {
                for entry in entries {
                    apply_edit(&mut merged, entry);
                }
            }
            // `namespace` replaces; unknown keys pass through, and compiling
            // reports them.
            _ => {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    let intents = delta
        .get("ripple")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    layer_intents(list(&mut merged, "ripple"), intents, &stated, delta)?;
    merged.retain(|_, value| !matches!(value, Value::Array(items) if items.is_empty()));
    Ok(Value::Object(merged))
}

/// The functions (`fns`, `patch`, `edit`), tests and test tables a frame
/// states, by name; [`UNNAMED_TESTS`] when it states a test without one.
fn stated_names(frame: &Map<String, Value>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if frame
        .get("tests")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|test| name_of(test, "name").is_none())
    {
        out.insert(UNNAMED_TESTS.to_owned());
    }
    for (key, field) in [
        ("fns", "fn"),
        ("functions", "fn"),
        ("patch", "fn"),
        ("edit", "fn"),
        ("tests", "name"),
        ("test_tables", "name"),
    ] {
        for entry in frame
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(name) = name_of(entry, field).or_else(|| name_of(entry, "name")) {
                out.insert(name.to_owned());
            }
        }
    }
    out
}

/// The names under `key` of an intent.
fn names_at(intent: &Value, key: &str) -> BTreeSet<String> {
    intent
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// Sets `key` of an intent to `names` (removed when empty).
fn set_names(intent: &mut Value, key: &str, names: &BTreeSet<String>) {
    let Some(object) = intent.as_object_mut() else {
        return;
    };
    if names.is_empty() {
        object.remove(key);
    } else {
        object.insert(
            key.to_owned(),
            Value::Array(names.iter().cloned().map(Value::from).collect()),
        );
    }
}

/// An intent's identity, owned.
type Identity = (&'static str, String, String);

fn identity(intent: &Value) -> Option<Identity> {
    intent_key(intent).map(|(kind, a, b)| (kind, a.to_owned(), b.to_owned()))
}

/// The base intent each follow-up intent replaces (`None`: appended). Only
/// the base's own intents are replaced. With one intent of an identity on
/// each side, it is replaced; with several on either side, a follow-up
/// intent replaces the one naming the same functions (`in`), and one
/// naming others is appended when every base intent of its identity is
/// replaced; otherwise which one it replaces cannot be told.
fn slots(base: &[Value], entries: &[Value]) -> Result<Vec<Option<usize>>> {
    let functions = |intent: &Value| names_at(intent, "in");
    let mut out: Vec<Option<usize>> = vec![None; entries.len()];
    let mut taken = vec![false; base.len()];
    let mut unmatched = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let Some(key) = identity(entry) else {
            continue;
        };
        let same: Vec<usize> = (0..base.len())
            .filter(|at| identity(&base[*at]).as_ref() == Some(&key))
            .collect();
        let peers = entries
            .iter()
            .filter(|other| identity(other).as_ref() == Some(&key))
            .count();
        let chosen = match same.as_slice() {
            [] => None,
            [only] if peers == 1 => Some(*only),
            _ => {
                let exact: Vec<usize> = same
                    .iter()
                    .copied()
                    .filter(|at| key.0 == "guard" && functions(&base[*at]) == functions(entry))
                    .collect();
                match exact.as_slice() {
                    [at] if !taken[*at] => Some(*at),
                    [] => {
                        unmatched.push((index, same));
                        continue;
                    }
                    _ => return Err(ambiguous(index, &key, base, &same)),
                }
            }
        };
        if let Some(at) = chosen {
            taken[at] = true;
        }
        out[index] = chosen;
    }
    for (index, same) in unmatched {
        if same.iter().any(|at| !taken[*at]) {
            let key = identity(&entries[index]).expect("a matched identity");
            return Err(ambiguous(index, &key, base, &same));
        }
    }
    Ok(out)
}

/// The refusal of a follow-up intent that could replace several base
/// intents of its identity, or be a new one.
fn ambiguous(index: usize, key: &Identity, base: &[Value], same: &[usize]) -> AgentError {
    let what = if key.0 == "arity" {
        format!("arity of `{}`", key.1)
    } else {
        format!("guard `{}` on `{}`", key.1, key.2)
    };
    let listed: Vec<String> = same
        .iter()
        .map(|at| {
            format!(
                "/ripple/{at} (\"in\": {})",
                base[*at].get("in").unwrap_or(&Value::Null)
            )
        })
        .collect();
    frame(
        &format!("/ripple/{index}"),
        format!(
            "the draft has {} intents with this identity ({what}): {}; which one this intent replaces, or whether it is a new one, cannot be told: state it with the \"in\" of the one it replaces, or set \"/ripple\" to the whole list of intents to keep",
            same.len(),
            listed.join(", ")
        ),
    )
}

/// Layers the follow-up's intents on the base's: each replaces the base
/// intent with the same identity where it stands, or is appended; `arity`
/// intents record in `after` what the follow-ups state after them.
fn layer_intents(
    base: &mut Vec<Value>,
    entries: Vec<Value>,
    stated: &BTreeSet<String>,
    delta: &Map<String, Value>,
) -> Result<()> {
    let slots = slots(base, &entries)?;
    let replaced: BTreeSet<usize> = slots.iter().flatten().copied().collect();
    for (index, intent) in base.iter_mut().enumerate() {
        if !replaced.contains(&index) && matches!(identity(intent), Some(key) if key.0 == "arity") {
            let after: BTreeSet<String> =
                names_at(intent, "after").union(stated).cloned().collect();
            set_names(intent, "after", &after);
        }
    }
    for (mut entry, slot) in entries.into_iter().zip(slots) {
        let Some(index) = slot else {
            base.push(entry);
            continue;
        };
        if let Some((kind, target, _)) = identity(&entry)
            && kind == "arity"
            && entry.get("frame_calls").is_none()
            && !restates_parameters(delta, &target)
        {
            let after: BTreeSet<String> = names_at(&base[index], "after")
                .union(&names_at(&entry, "after"))
                .chain(stated)
                .cloned()
                .collect();
            set_names(&mut entry, "after", &after);
        }
        base[index] = entry;
    }
    Ok(())
}

/// Whether a follow-up restates the parameters of `function`.
fn restates_parameters(delta: &Map<String, Value>, function: &str) -> bool {
    ["fns", "functions", "patch"].iter().any(|key| {
        delta
            .get(*key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|entry| name_of(entry, "fn") == Some(function) && entry.get("params").is_some())
    })
}

/// The list under `key`, created empty when absent.
fn list<'a>(frame: &'a mut Map<String, Value>, key: &str) -> &'a mut Vec<Value> {
    let slot = frame
        .entry(key.to_owned())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !slot.is_array() {
        *slot = Value::Array(Vec::new());
    }
    slot.as_array_mut().expect("just made an array")
}

fn name_of<'a>(entry: &'a Value, key: &str) -> Option<&'a str> {
    entry.get(key).and_then(Value::as_str)
}

/// Each entry replaces the base entry with the same `key` value, or is appended.
fn replace_named(base: &mut Vec<Value>, entries: Vec<Value>, key: &str) {
    for entry in entries {
        let slot = name_of(&entry, key).and_then(|name| {
            base.iter()
                .position(|item| name_of(item, key) == Some(name))
        });
        match slot {
            Some(index) => base[index] = entry,
            None => base.push(entry),
        }
    }
}

/// An intent's identity: its kind and target, and for a guard the
/// parameter it checks. `None` for an entry that names no enabled intent.
fn intent_key(entry: &Value) -> Option<(&'static str, &str, &str)> {
    let object = entry.as_object()?;
    if let Some(target) = object.get("arity").and_then(Value::as_str) {
        return (!object.contains_key("guard")).then_some(("arity", target, ""));
    }
    let checker = object.get("guard").and_then(Value::as_str)?;
    Some(("guard", checker, object.get("arg").and_then(Value::as_str)?))
}

/// The index of the base `fns` definition of `function`.
fn definition(frame: &Map<String, Value>, function: &str) -> Option<usize> {
    frame
        .get("fns")?
        .as_array()?
        .iter()
        .position(|item| name_of(item, "fn") == Some(function))
}

fn apply_patch(frame: &mut Map<String, Value>, patch: Value) {
    let Some(function) = name_of(&patch, "fn").map(str::to_owned) else {
        list(frame, "patch").push(patch);
        return;
    };
    if let Some(index) = definition(frame, &function) {
        let definition = &mut list(frame, "fns")[index];
        for key in ["params", "returns"] {
            if let Some(value) = patch.get(key) {
                definition[key] = value.clone();
            }
        }
        let blocks = patch
            .get("blocks")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let Some(list) = definition.get_mut("blocks").and_then(Value::as_array_mut) else {
            return;
        };
        for (name, spec) in blocks {
            let at = list
                .iter()
                .position(|block| name_of(block, "name") == Some(name.as_str()));
            match (spec, at) {
                (Value::Null, Some(at)) => {
                    list.remove(at);
                }
                (Value::Null, None) => {}
                (Value::Object(mut block), at) => {
                    block.insert("name".to_owned(), Value::from(name.clone()));
                    match at {
                        Some(at) => list[at] = Value::Object(block),
                        None => list.push(Value::Object(block)),
                    }
                }
                (other, _) => list.push(other),
            }
        }
        return;
    }
    let patches = list(frame, "patch");
    if let Some(existing) = patches
        .iter_mut()
        .find(|item| name_of(item, "fn") == Some(function.as_str()))
    {
        for key in ["params", "returns"] {
            if let Some(value) = patch.get(key) {
                existing[key] = value.clone();
            }
        }
        if let Some(blocks) = patch.get("blocks").and_then(Value::as_object) {
            let target = existing
                .as_object_mut()
                .expect("a patch entry found by name is an object")
                .entry("blocks")
                .or_insert_with(|| Value::Object(Map::new()));
            if let Some(target) = target.as_object_mut() {
                for (name, spec) in blocks {
                    target.insert(name.clone(), spec.clone());
                }
            }
        }
        return;
    }
    patches.push(patch);
}

/// Replaces the operation named `op` in a block's `ops` with `with`,
/// keeping its name; whether it was found.
fn replace_op(block: &mut Value, op: &str, with: &Value) -> bool {
    let Some(ops) = block.get_mut("ops").and_then(Value::as_array_mut) else {
        return false;
    };
    let named = |item: &Value| match item {
        Value::Array(items) => items.first().and_then(Value::as_str) == Some(op),
        Value::Object(_) => name_of(item, "name") == Some(op),
        _ => false,
    };
    let Some(at) = ops.iter().position(named) else {
        return false;
    };
    ops[at] = match with {
        Value::Array(items) => {
            let mut op_items = vec![Value::from(op)];
            op_items.extend(items.iter().cloned());
            Value::Array(op_items)
        }
        Value::Object(object) => {
            let mut object = object.clone();
            object.insert("name".to_owned(), Value::from(op));
            Value::Object(object)
        }
        other => other.clone(),
    };
    true
}

fn apply_edit(frame: &mut Map<String, Value>, edit: Value) {
    let target = name_of(&edit, "fn").map(str::to_owned).zip(
        name_of(&edit, "replace_op")
            .and_then(|path| path.split_once('.'))
            .map(|(block, op)| (block.to_owned(), op.to_owned())),
    );
    let (Some((function, (block, op))), Some(with)) = (target, edit.get("with").cloned()) else {
        list(frame, "edit").push(edit);
        return;
    };
    if let Some(index) = definition(frame, &function) {
        let blocks = list(frame, "fns")[index]
            .get_mut("blocks")
            .and_then(Value::as_array_mut);
        let replaced = blocks
            .and_then(|blocks| {
                blocks
                    .iter_mut()
                    .find(|item| name_of(item, "name") == Some(block.as_str()))
            })
            .is_some_and(|found| replace_op(found, &op, &with));
        if replaced {
            return;
        }
    }
    let patched = list(frame, "patch")
        .iter_mut()
        .find(|item| name_of(item, "fn") == Some(function.as_str()))
        .and_then(|patch| patch.get_mut("blocks"))
        .and_then(|blocks| blocks.get_mut(block.as_str()))
        .is_some_and(|spec| replace_op(spec, &op, &with));
    if patched {
        return;
    }
    let edits = list(frame, "edit");
    let same = |item: &Value| {
        name_of(item, "fn") == Some(function.as_str())
            && name_of(item, "replace_op") == name_of(&edit, "replace_op")
    };
    match edits.iter().position(same) {
        Some(at) => edits[at] = edit,
        None => edits.push(edit),
    }
}

#[cfg(test)]
mod tests {
    use super::layer;
    use serde_json::json;

    fn base() -> serde_json::Value {
        json!({"af1": 1,
          "types": [{"name": "E", "variant": ["Bad"]}],
          "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": "i64", "blocks": [
              {"name": "entry", "ops": [["one", "const", 1], ["r", "add", "a", "one"]], "term": ["return", "r"]},
              {"name": "spare", "ops": [], "term": ["return", "a"]}]}],
          "patch": [{"fn": "g", "blocks": {"entry": {"ops": [["x", "const", 2]], "term": ["return", "x"]}}}],
          "tests": [{"name": "t1", "fn": "f", "args": [1], "expect": 2}]})
    }

    #[test]
    fn tests_are_added_without_restating_the_frame() {
        let merged = layer(
            &base(),
            &json!({"af1": 1, "tests": [{"fn": "f", "args": [2], "expect": 3}]}),
        )
        .unwrap();
        assert_eq!(merged["fns"], base()["fns"]);
        assert_eq!(merged["tests"].as_array().unwrap().len(), 2);
        let renamed = layer(
            &base(),
            &json!({"af1": 1, "tests": [{"name": "t1", "fn": "f", "args": [5], "expect": 6}]}),
        )
        .unwrap();
        assert_eq!(
            renamed["tests"],
            json!([{"name": "t1", "fn": "f", "args": [5], "expect": 6}])
        );
    }

    #[test]
    fn a_patch_or_edit_applies_to_the_functions_the_base_defines() {
        let merged = layer(&base(), &json!({"af1": 1,
            "patch": [{"fn": "f", "returns": "i64", "blocks": {"spare": null, "extra": {"ops": [], "term": ["return", "a"]}}}],
            "edit": [{"fn": "f", "replace_op": "entry.one", "with": ["const", 7]}]})).unwrap();
        let blocks = merged["fns"][0]["blocks"].as_array().unwrap();
        let names: Vec<&str> = blocks
            .iter()
            .map(|block| block["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["entry", "extra"]);
        assert_eq!(blocks[0]["ops"][0], json!(["one", "const", 7]));
        assert!(merged.get("edit").is_none());
        assert_eq!(merged["patch"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn patches_and_edits_of_head_functions_merge_or_append() {
        let merged = layer(&base(), &json!({"af1": 1,
            "patch": [{"fn": "g", "blocks": {"more": {"ops": [], "term": ["return", "y"]}}}, {"fn": "h", "blocks": {}}],
            "edit": [{"fn": "g", "replace_op": "entry.x", "with": ["const", 3]}, {"fn": "k", "replace_op": "entry.z", "with": ["const", 4]}]})).unwrap();
        let patches = merged["patch"].as_array().unwrap();
        assert_eq!(patches.len(), 2);
        assert_eq!(
            patches[0]["blocks"]["entry"]["ops"][0],
            json!(["x", "const", 3])
        );
        assert!(patches[0]["blocks"].get("more").is_some());
        assert_eq!(
            merged["edit"],
            json!([{"fn": "k", "replace_op": "entry.z", "with": ["const", 4]}])
        );
    }

    #[test]
    fn definitions_replace_by_name_and_raw_operations_are_refused() {
        let merged = layer(
            &base(),
            &json!({"af1": 1, "types": [{"name": "E", "variant": ["Bad", "Worse"]}],
            "fns": [{"fn": "n", "params": [], "returns": "i64", "blocks": []}], "delete": ["old"]}),
        )
        .unwrap();
        assert_eq!(
            merged["types"],
            json!([{"name": "E", "variant": ["Bad", "Worse"]}])
        );
        assert_eq!(merged["fns"].as_array().unwrap().len(), 2);
        assert_eq!(merged["delete"], json!(["old"]));
        assert!(layer(&base(), &json!([])).is_err());
        assert!(layer(&base(), &json!({"tests": []})).is_err());
    }

    #[test]
    fn a_restated_intent_replaces_the_same_intent_in_place() {
        let base = json!({"af1": 1, "afx": 1, "ripple": [
            {"arity": "f", "value": 0},
            {"guard": "g", "arg": "p", "in": ["a"]},
            {"guard": "g", "arg": "q", "in": ["a"]}]});
        let merged = layer(
            &base,
            &json!({"af1": 1, "ripple": [
                {"guard": "g", "arg": "p", "in": ["a", "b"], "mode": "entry"},
                {"arity": "f", "value": 5},
                {"arity": "h"},
                {"effect": "f"}]}),
        )
        .unwrap();
        assert_eq!(
            merged["ripple"],
            json!([
                {"arity": "f", "value": 5},
                {"guard": "g", "arg": "p", "in": ["a", "b"], "mode": "entry"},
                {"guard": "g", "arg": "q", "in": ["a"]},
                {"arity": "h"},
                {"effect": "f"}])
        );
    }

    #[test]
    fn two_guards_in_one_follow_up_do_not_replace_each_other() {
        let base = json!({"af1": 1, "afx": 1, "ripple": [
            {"guard": "check", "arg": "x", "in": ["first"]},
            {"guard": "check", "arg": "x", "in": ["second"], "handler": "H"}]});
        let second = json!({"af1": 1, "afx": 1, "ripple": [
            {"guard": "check", "arg": "x", "in": ["second"], "handler": "H"}]});
        assert_eq!(layer(&base, &second).unwrap()["ripple"], base["ripple"]);
        let empty = json!({"af1": 1, "afx": 1});
        assert_eq!(layer(&empty, &base).unwrap()["ripple"], base["ripple"]);
    }

    #[test]
    fn an_unnamed_follow_up_test_is_recorded_after_the_intent() {
        let base = json!({"af1": 1, "afx": 1, "ripple": [
            {"arity": "f", "frame_calls": "old"}]});
        let delta = json!({"af1": 1, "afx": 1, "tests": [
            {"fn": "f", "args": [2, 5], "expect": 3}]});
        assert_eq!(
            layer(&base, &delta).unwrap()["ripple"][0]["after"],
            json!(["(unnamed)"])
        );
    }
}

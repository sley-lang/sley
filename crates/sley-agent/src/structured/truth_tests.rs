//! Authoring-only truth assertions become ordinary Boolean `TestCases`.
//! Non-Boolean targets use private predicates built from existing operations.

use super::{Context, Lowered, Map, Obligation, R, Value, fail, json, normal};
use crate::error::AgentErrorCode;
use std::collections::BTreeMap;

pub(super) fn lower(
    cx: &Context<'_>,
    frame: &Map<String, Value>,
) -> Result<Lowered, Vec<Obligation>> {
    let mut out = frame.clone();
    let mut origins = Vec::new();
    let mut helpers = BTreeMap::new();
    let mut problems = Vec::new();
    let mut next_name = 0;
    if let Some(tests) = frame.get("tests").and_then(Value::as_array) {
        let mut tests = tests.clone();
        for (index, test) in tests.iter_mut().enumerate() {
            if test.get("truthy").is_none() {
                continue;
            }
            let at = format!("/tests/{index}");
            let result =
                predicate(cx, test, &at, &mut helpers, &mut next_name).and_then(|helper| {
                    if let Some(helper) = helper {
                        let pointer = append_helper(&mut out, helper)?;
                        origins.push((pointer, at.clone()));
                    }
                    Ok(())
                });
            if let Err(error) = result {
                problems.push(Obligation::new(
                    AgentErrorCode::FrameInvalid,
                    &error.at,
                    error.message,
                ));
            }
        }
        out.insert("tests".to_owned(), Value::Array(tests));
    }
    if problems.is_empty() {
        Ok(Lowered {
            frame: out,
            origins,
        })
    } else {
        Err(problems)
    }
}

fn predicate(
    cx: &Context<'_>,
    test: &mut Value,
    at: &str,
    helpers: &mut BTreeMap<String, String>,
    next_name: &mut usize,
) -> R<Option<Value>> {
    let Some(expected) = test.get("truthy").and_then(Value::as_bool) else {
        return fail(&format!("{at}/truthy"), "truthy must be a boolean");
    };
    if test.get("expect").is_some() {
        return fail(at, "use either expect or truthy, not both");
    }
    let key = ["fn", "target", "function"]
        .into_iter()
        .find(|key| test.get(*key).is_some())
        .unwrap_or("fn");
    let Some(target) = test.get(key).and_then(Value::as_str) else {
        return fail(&format!("{at}/{key}"), "expected a function name");
    };
    let Some((params, result)) = cx.signature(target) else {
        return fail(&format!("{at}/{key}"), "the target has no signature");
    };
    let Some(result) = result else {
        return fail(&format!("{at}/{key}"), "the target has no return type");
    };
    let mut helper = None;
    let target = if normal(&cx.render(&result)) == "bool" {
        target.to_owned()
    } else if let Some(name) = helpers.get(target) {
        name.clone()
    } else {
        let name = loop {
            let name = format!("_sley_assert_{}", *next_name);
            *next_name += 1;
            if !cx.top.contains(&name) && cx.names.resolve(&name).is_none() {
                break name;
            }
        };
        let mut arguments = vec![json!("call"), json!(target)];
        let mut parameters = Vec::new();
        for (index, ty) in params.iter().enumerate() {
            let Some(ty) = ty else {
                return fail(at, "the target has an unknown parameter type");
            };
            let parameter = format!("arg{index}");
            parameters.push(json!([parameter, cx.render(ty)]));
            arguments.push(json!(parameter));
        }
        helper = Some(json!({
            "fn": name, "visibility": "private", "params": parameters,
            "returns": "bool", "body": [["return", ["truthy", arguments]]]
        }));
        helpers.insert(target.to_owned(), name.clone());
        name
    };
    test[key] = json!(target);
    test["expect"] = json!(expected);
    if let Some(test) = test.as_object_mut() {
        test.remove("truthy");
    }
    Ok(helper)
}

fn append_helper(frame: &mut Map<String, Value>, helper: Value) -> R<String> {
    let key = if !frame.contains_key("fns") && frame.contains_key("functions") {
        "functions"
    } else {
        "fns"
    };
    let Some(functions) = frame.entry(key).or_insert_with(|| json!([])).as_array_mut() else {
        return fail(&format!("/{key}"), "expected an array");
    };
    let at = format!("/{key}/{}", functions.len());
    functions.push(helper);
    Ok(at)
}

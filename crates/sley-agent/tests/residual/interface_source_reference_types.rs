use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn source(term: &Value, dialect: bool) -> Value {
    let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":[
        {"name":"entry","ops":[],"term":term},
        {"name":"done","unreachable":true,"params":[["value","i8"]],"ops":[],"term":["return","value"]}]}]});
    if dialect {
        source["afx"] = json!(1);
    }
    source
}

#[test]
fn indexed_function_parameter_conditions_keep_declared_type_before_generation() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let mut source = source(&json!(["cond", "x#1", "done", "done"]), dialect);
        source["fns"][0]["blocks"][1]["unreachable"] = json!(false);
        source["fns"][0]["blocks"][1]["params"] = json!([]);
        source["fns"][0]["blocks"][1]["term"] = json!(["return", "x"]);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/0/term/1")
                && error.detail().contains("requires bool")
                && error.detail().contains("i8"),
            "{error}"
        );
    }
}

#[test]
fn indexed_function_parameter_edges_keep_declared_type_before_generation() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let mut source = source(&json!(["br", "done", "ready#4294967295"]), dialect);
        source["fns"][0]["blocks"][1]["unreachable"] = json!(false);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/0/term/2")
                && error.detail().contains("has type bool")
                && error.detail().contains("requires i8"),
            "{error}"
        );
    }
}

#[test]
fn indexed_function_parameter_returns_keep_declared_type_before_generation() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let source = source(&json!(["return", "ready#+0"]), dialect);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/0/term/1")
                && error.detail().contains("return value has type bool")
                && error.detail().contains("requires i8"),
            "{error}"
        );
    }
}

#[test]
fn nested_expression_result_types_keep_indexed_parameter_bindings() {
    let fixture = Fixture::new();
    let source = source(&json!(["return", ["tuple", "x#1", "ready#00"]]), true);
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(
        error.detail().contains("/fns/0/blocks/0/term/1")
            && error.detail().contains("(i8,bool)")
            && error.detail().contains("requires i8"),
        "{error}"
    );
}

fn reads(report: &Value) -> &[Value] {
    report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
        .as_array()
        .unwrap()
}

#[test]
fn valid_parameter_suffix_contexts_match_actual_compiler_and_vm() {
    let mut comparisons = 0;
    let mut vm_cases = 0;
    for dialect in [false, true] {
        for mode in 0..4 {
            if mode == 3 && !dialect {
                continue;
            }
            let fixture = Fixture::new();
            for suffix in ["", "#0", "#00", "#+0", "#1", "#01", "#+1", "#4294967295"] {
                let mut source = source(&json!(["return", format!("x{suffix}")]), dialect);
                source["fns"][0]["blocks"]
                    .as_array_mut()
                    .unwrap()
                    .truncate(1);
                let reference = if mode == 0 {
                    format!("x{suffix}")
                } else if mode == 1 {
                    format!("ready{suffix}")
                } else {
                    format!("value{suffix}")
                };
                if mode == 1 {
                    source["fns"][0]["blocks"][0]["term"] =
                        json!(["cond", reference, ["done", "x"], ["done", "x"]]);
                    source["fns"][0]["blocks"].as_array_mut().unwrap().push(json!({"name":"done","params":[["value","i8"]],"ops":[],"term":["return","value"]}));
                } else if mode == 2 {
                    source["fns"][0]["blocks"][0]["term"] = json!(["br", "done", "x"]);
                    source["fns"][0]["blocks"].as_array_mut().unwrap().push(json!({"name":"done","params":[["value","i8"]],"ops":[],"term":["return",reference]}));
                } else if mode == 3 {
                    source["fns"][0]["blocks"][0]["ops"] =
                        json!([["value", "add?fallback", "x", "x"]]);
                    source["fns"][0]["blocks"][0]["term"] = json!(["return", reference]);
                    source["fns"][0]["blocks"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"name":"fallback","ops":[],"term":["trap","unreachable"]}));
                }
                let original = super::bytes(&source);
                let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
                let read = reads(&report)
                    .iter()
                    .find(|r| r["name"] == reference)
                    .unwrap();
                assert_eq!(
                    read["actual_type"],
                    if mode == 1 { "bool" } else { "i8" },
                    "{report}"
                );
                assert_eq!(read["rewritten"], false);
                assert_eq!(super::bytes(&source), original);
                let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
                assert_eq!(code, 0, "mode {mode} suffix {suffix}: {result}");
                comparisons += 1;
                let (code, call) = cli(
                    &fixture.dir,
                    &[
                        "call",
                        "helper",
                        "7",
                        "true",
                        "--on",
                        result["handle"].as_str().unwrap(),
                    ],
                );
                assert_eq!(code, 0, "{call}");
                assert_eq!(call["result"], if mode == 3 { json!(14) } else { json!(7) });
                vm_cases += 1;
            }
        }
    }
    assert_eq!(comparisons, 56);
    assert_eq!(vm_cases, 56);
    eprintln!("REFERENCE_TYPE_ORACLE comparisons={comparisons} vm_cases={vm_cases}");
}

#[test]
fn checked_continuation_and_qualified_block_parameter_types_do_not_take_outer_types() {
    let fixture = Fixture::new();
    for reference in ["value#1", "entry.value#+0"] {
        let mut source = source(&json!(["cond", reference, "done", "done"]), true);
        source["fns"][0]["blocks"][0]["ops"] = json!([["value", "add?fallback", "x", "x"]]);
        source["fns"][0]["blocks"][1]["unreachable"] = json!(false);
        source["fns"][0]["blocks"][1]["params"] = json!([]);
        source["fns"][0]["blocks"][1]["term"] = json!(["return", "x"]);
        source["fns"][0]["blocks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"fallback","ops":[],"term":["trap","unreachable"]}));
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert!(
            error.detail().contains("requires bool") && error.detail().contains("i8"),
            "{error}"
        );
    }
    for dialect in [false, true] {
        let mut source = source(&json!(["br", "done", "ready"]), dialect);
        source["fns"][0]["blocks"][1]["unreachable"] = json!(false);
        source["fns"][0]["blocks"][1]["params"] = json!([["value", "bool"]]);
        source["fns"][0]["blocks"][1]["term"] = json!(["return", "done.value#01"]);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert!(
            error.detail().contains("return value has type bool")
                && error.detail().contains("requires i8"),
            "{error}"
        );
    }
}

#[test]
fn nearest_differently_typed_after_split_result_drives_context_instead_of_far_hint() {
    let fixture = Fixture::new();
    let mut source = json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"u8","blocks":[
        {"name":"entry","ops":[["value","const",{"type":"i8","value":7}]],"term":["br","middle"]},
        {"name":"middle","ops":[["check","add?fallback","x","x"],["value","const",{"type":"u8","value":9}]],"term":["br","done"]},
        {"name":"fallback","ops":[],"term":["trap","unreachable"]},
        {"name":"done","ops":[],"term":["return","value"]}]}]});
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    let read = reads(&report)
        .iter()
        .find(|r| r["at"] == "/fns/0/blocks/3/term/1")
        .unwrap();
    assert_eq!(read["definition"], "/fns/0/blocks/1/ops/1");
    assert_eq!(read["actual_type"], "u8");
    source["fns"][0]["returns"] = json!("i8");
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert!(
        error.detail().contains("return value has type u8")
            && error.detail().contains("requires i8"),
        "{error}"
    );
}

fn inventory(root: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        path: &std::path::Path,
        out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn indexed_parameter_type_refusals_preserve_existing_draft_and_head_bytes() {
    for dialect in [false, true] {
        let fixture = Fixture::new();
        let source = source(&json!(["return", "ready#1"]), dialect);
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert!([1, 2].contains(&code), "{result}");
        let mut request = predicate_request(json!(false));
        request["base"] = result["draft"].clone();
        let before = inventory(&fixture.dir);
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        assert!(
            result["detail"]
                .as_str()
                .unwrap()
                .contains("return value has type bool")
        );
        let after = inventory(&fixture.dir);
        let changes: std::collections::BTreeSet<_> = before
            .keys()
            .chain(after.keys())
            .filter(|p| before.get(*p) != after.get(*p))
            .collect();
        assert!(
            changes
                .iter()
                .all(|p| p.as_path() == std::path::Path::new(".sley/events.jsonl")),
            "{changes:?}"
        );
    }
}

#[test]
fn nested_boolean_contexts_preserve_parameter_types_and_ordinary_execution() {
    let fixture = Fixture::new();
    for suffix in ["", "#0", "#00", "#+0", "#1", "#01", "#+1", "#4294967295"] {
        let mut source = source(
            &json!([
                "return",
                [
                    "and",
                    format!("ready{suffix}"),
                    ["not", ["not", format!("ready{suffix}")]]
                ]
            ]),
            true,
        );
        source["fns"][0]["returns"] = json!("bool");
        source["fns"][0]["blocks"]
            .as_array_mut()
            .unwrap()
            .truncate(1);
        let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        assert!(
            reads(&report)
                .iter()
                .filter(|r| r["name"] == format!("ready{suffix}"))
                .all(|r| r["actual_type"] == "bool")
        );
        let input =
            &report["declared_body_control_flow"]["checked_functions"][0]["terminator_inputs"][0];
        assert_eq!(input["actual_type"], "bool");
        assert_eq!(input["type_connection"], "checked");
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        for input in ["true", "false"] {
            let (code, call) = cli(
                &fixture.dir,
                &[
                    "call",
                    "helper",
                    "7",
                    input,
                    "--on",
                    result["handle"].as_str().unwrap(),
                ],
            );
            assert_eq!(code, 0, "{call}");
            assert_eq!(call["result"], input == "true");
        }
    }
    eprintln!("REFERENCE_NESTED_ORACLE comparisons=8 vm_cases=16 ordinary_incomplete=0");
}

#[test]
fn retained_operation_aliases_use_exact_canonical_result_types() {
    let fixture = Fixture::new();
    let source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":[
        {"name":"entry","ops":[["value","const",{"type":"i8","value":7}]],"term":["br","done"]},
        {"name":"done","ops":[],"term":["return","entry.value"]}]}]});
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    for reference in ["value", "value#00", "entry.value#+0", "entry.value#0"] {
        let patch = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{"done":{"ops":[["marker","const",{"type":"bool","value":true}]],"term":["return",reference]}}}]});
        let report = check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        let read = reads(&report)
            .iter()
            .find(|r| r["name"] == reference)
            .unwrap();
        assert_eq!(read["actual_type"], "i8");
        assert_eq!(read["definition"], "accepted value `helper.entry.value`");
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, call) = cli(
            &fixture.dir,
            &[
                "call",
                "helper",
                "9",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{call}");
        assert_eq!(call["result"], 7);
    }
    eprintln!("REFERENCE_RETAINED_ORACLE comparisons=4 vm_cases=4");
}

#[test]
fn unresolved_nested_reads_keep_unknown_type_instead_of_borrowing_context() {
    let fixture = Fixture::new();
    let source = source(&json!(["return", ["tuple", "unknown#00", "ready#1"]]), true);
    let original = super::bytes(&source);
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    let read = reads(&report)
        .iter()
        .find(|r| r["name"] == "unknown#00")
        .unwrap();
    assert_eq!(read["actual_type"], Value::Null);
    assert_eq!(read["availability"], "availability_deferred");
    let input =
        &report["declared_body_control_flow"]["checked_functions"][0]["terminator_inputs"][0];
    assert_eq!(input["actual_type"], Value::Null);
    assert_eq!(input["type_connection"], "deferred");
    assert_eq!(super::bytes(&source), original);
}

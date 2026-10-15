use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn diamond(dialect: bool, dominating: bool) -> Value {
    let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":[
        {"name":"entry","ops":[],"term":["cond","ready","left","right"]},
        {"name":"left","ops":[],"term":["br","done"]},
        {"name":"right","ops":[],"term":["br","done"]},
        {"name":"done","ops":[],"term":["return","left.value"]}]}]});
    let owner = usize::from(!dominating);
    source["fns"][0]["blocks"][owner]["ops"] = json!([["value","const",{"type":"i8","value":7}]]);
    if dominating {
        source["fns"][0]["blocks"][3]["term"][1] = json!("entry.value");
    }
    if dialect {
        source["afx"] = json!(1);
    }
    source
}

fn bypass_patch() -> Value {
    // Only left and done remain reachable; keep declaration flags valid so the
    // ordinary kernel can independently isolate the retained operand defect.
    json!({"af1":1,"patch":[{"fn":"helper","entry":"left","blocks":{
        "entry":{"unreachable":true,"ops":[["value","const",{"type":"i8","value":7}]],"term":["cond","ready","left","right"]},
        "right":{"unreachable":true,"ops":[],"term":["br","done"]}
    }}]})
}

#[test]
fn bypassed_authored_result_refuses_before_generation_with_use_and_definition() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let source = diamond(dialect, false);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/3/term/1")
                && error.detail().contains("/fns/0/blocks/1/ops/0")
                && error.detail().contains("does not dominate"),
            "{error}"
        );
    }
}

#[test]
fn changed_entry_refuses_a_bypassed_retained_result_before_generation() {
    let fixture = Fixture::new();
    let source = diamond(false, true);
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    for dialect in [false, true] {
        let mut patch = bypass_patch();
        if dialect {
            patch["afx"] = json!(1);
        }
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("retained block `helper.done`")
                && error.detail().contains("helper.entry.value")
                && error.detail().contains("does not dominate"),
            "{error}"
        );
    }
}

#[test]
fn dominating_authored_and_retained_results_keep_actual_vm_behavior() {
    for dialect in [false, true] {
        let fixture = Fixture::new();
        let source = diamond(dialect, true);
        let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        assert_eq!(
            report["declared_body_control_flow"]["checked_functions"][0]["control_projection"]["status"],
            "complete_block_start_projection"
        );
        assert!(
            report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
                .as_array()
                .unwrap()
                .iter()
                .any(|read| read["name"] == "entry.value"
                    && read["availability"] == "dominance_checked")
        );
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        for x in [-128, 0, 127] {
            for ready in [false, true] {
                let (code, call) = cli(
                    &fixture.dir,
                    &[
                        "call",
                        "helper",
                        &x.to_string(),
                        &ready.to_string(),
                        "--on",
                        result["handle"].as_str().unwrap(),
                    ],
                );
                assert_eq!(code, 0, "{call}");
                assert_eq!(call["result"], json!(7));
            }
        }
        let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        let mut patch = json!({"af1":1,"patch":[{"fn":"helper","params":[["ready","bool"],["x","i8"]],"blocks":{}}]});
        if dialect {
            patch["afx"] = json!(1);
        }
        let report = check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        assert!(
            report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
                .as_array()
                .unwrap()
                .iter()
                .any(|read| read["name"] == "helper.entry.value"
                    && read["availability"] == "dominance_checked")
        );
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        for ready in [false, true] {
            let (code, call) = cli(
                &fixture.dir,
                &[
                    "call",
                    "helper",
                    &ready.to_string(),
                    "9",
                    "--on",
                    result["handle"].as_str().unwrap(),
                ],
            );
            assert_eq!(code, 0, "{call}");
            assert_eq!(call["result"], json!(7));
        }
    }
}

fn routed(checked: bool, bypass: bool) -> Value {
    let mut source = diamond(true, false);
    source["fns"][0]["blocks"] = json!([
        {"name":"entry","ops":[],"term":["br","left"]},
        {"name":"left","ops":[["value","const",{"type":"i8","value":7}]],"term":["br","done"]},
        {"name":"fallback","ops":[],"term":["trap","unreachable"]},
        {"name":"done","ops":[],"term":["return","left.value"]}]);
    if bypass {
        source["fns"][0]["blocks"][2]["term"] = json!(["br", "done"]);
    }
    if checked {
        source["fns"][0]["returns"] = json!("Result<i8,ArithmeticError>");
        source["fns"][0]["blocks"][0]["ops"] = json!([["checked", "add?fallback", "x", "x"]]);
        source["fns"][0]["blocks"][3]["term"] = json!(["ok", "left.value"]);
    } else {
        source["fns"][0]["blocks"][0]["ops"] = json!([["!fallback", "if", "ready"]]);
    }
    source
}

#[test]
fn early_exit_paths_participate_and_terminal_handlers_keep_valid_dominance() {
    let fixture = Fixture::new();
    let error = check(
        &fixture,
        &predicate_request(json!(false)),
        &routed(false, true),
    )
    .unwrap_err();
    assert!(
        error.detail().contains("/blocks/3/term/1") && error.detail().contains("does not dominate"),
        "{error}"
    );
    let source = routed(false, false);
    check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    for ready in [false, true] {
        let (code, call) = cli(
            &fixture.dir,
            &[
                "call",
                "helper",
                "9",
                &ready.to_string(),
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{call}");
        if ready {
            assert_eq!(call["result"]["trap"], 1);
        } else {
            assert_eq!(call["result"], json!(7));
        }
    }
}

#[test]
fn checked_handler_paths_participate_without_turning_error_cases_into_blocks() {
    let fixture = Fixture::new();
    let error = check(
        &fixture,
        &predicate_request(json!(false)),
        &routed(true, true),
    )
    .unwrap_err();
    assert!(error.detail().contains("does not dominate"), "{error}");
    let source = routed(true, false);
    check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    for x in [7, 127] {
        let (code, call) = cli(
            &fixture.dir,
            &[
                "call",
                "helper",
                &x.to_string(),
                "false",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{call}");
        if x == 127 {
            assert_eq!(call["result"]["trap"], 1);
        } else {
            assert_eq!(call["result"], json!({"Ok":7}));
        }
    }
}

#[test]
fn split_definitions_gain_proofs_while_ambiguous_routes_stay_deferred() {
    let fixture = Fixture::new();
    let mut source = diamond(true, true);
    source["fns"][0]["returns"] = json!("Result<i8,ArithmeticError>");
    source["fns"][0]["blocks"][0]["ops"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!(["checked", "add?", "x", "x"]));
    source["fns"][0]["blocks"][3]["term"] = json!(["ok", "entry.value"]);
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    assert!(
        report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |read| read["name"] == "entry.value" && read["availability"] == "dominance_checked"
            )
    );
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    for x in [7, 127] {
        let (code, call) = cli(
            &fixture.dir,
            &[
                "call",
                "helper",
                &x.to_string(),
                "false",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{call}");
        if x == 127 {
            assert!(call["result"].get("Err").is_some());
        } else {
            assert_eq!(call["result"], json!({"Ok":7}));
        }
    }
    source["types"] = json!([{"name":"ExitError","variant":[["Stop",null]]}]);
    source["fns"][0]["returns"] = json!("Result<i8,ExitError>");
    source["fns"][0]["blocks"][0]["ops"] =
        json!([["value","const",{"type":"i8","value":7}],["checked","add?Stop","x","x"]]);
    source["fns"][0]["blocks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"Stop","ops":[],"term":["fail","Stop"]}));
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    assert_eq!(
        report["declared_body_control_flow"]["checked_functions"][0]["control_projection"]["status"],
        "deferred"
    );
    assert!(
        report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
            .as_array()
            .unwrap()
            .iter()
            .any(|read| read["name"] == "entry.value"
                && read["availability"] == "availability_deferred")
    );
}

#[test]
fn retained_operation_reads_refuse_without_publishing_or_changing_existing_bytes() {
    for dialect in [false, true] {
        let fixture = Fixture::new();
        let mut source = diamond(false, true);
        source["fns"][0]["blocks"][3]["ops"] = json!([
            ["pair", "tuple", "entry.value", "x"],
            ["out", "tuple_get", 0, "pair"]
        ]);
        source["fns"][0]["blocks"][3]["term"] = json!(["return", "out"]);
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        let mut patch = bypass_patch();
        if dialect {
            patch["afx"] = json!(1);
        }
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error
                .detail()
                .contains("retained operation `helper.done.pair` operand 0")
                && error.detail().contains("helper.entry.value")
                && error.detail().contains("does not dominate"),
            "{error}"
        );
        let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
        assert!([1, 2].contains(&code), "{result}");
        assert!(result.to_string().contains("CFG_DOMINANCE"), "{result}");
        let before = inventory(&fixture.dir);
        let mut request = predicate_request(json!(false));
        request["base"] = result["draft"].clone();
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        let after = inventory(&fixture.dir);
        let changed: std::collections::BTreeSet<_> = before
            .keys()
            .chain(after.keys())
            .filter(|path| before.get(*path) != after.get(*path))
            .collect();
        assert!(
            changed
                .iter()
                .all(|p| p.as_path() == std::path::Path::new(".sley/events.jsonl")),
            "{changed:?}"
        );
    }
}

#[test]
fn small_graph_projection_judgments_agree_with_the_canonical_kernel() {
    let graphs: [&[&[usize]]; 6] = [
        &[&[1], &[2], &[3], &[]],
        &[&[1, 2], &[3], &[3], &[]],
        &[&[1], &[1, 2], &[3], &[]],
        &[&[1], &[2], &[1, 3], &[]],
        &[&[1], &[], &[3], &[2]],
        &[&[1, 2], &[0, 3], &[3], &[]],
    ];
    for dialect in [false, true] {
        let fixture = Fixture::new();
        for (graph, edges) in graphs.iter().enumerate() {
            for owner in 0..4 {
                for using in 0..4 {
                    let mut source = diamond(dialect, true);
                    let blocks = source["fns"][0]["blocks"].as_array_mut().unwrap();
                    blocks.clear();
                    for (b, targets) in edges.iter().enumerate() {
                        let term = if b == using {
                            json!(["return", format!("b{owner}.value")])
                        } else {
                            match targets {
                                [] => json!(["return", "x"]),
                                [target] => json!(["br", format!("b{target}")]),
                                [left, right] => json!([
                                    "cond",
                                    "ready",
                                    format!("b{left}"),
                                    format!("b{right}")
                                ]),
                                _ => unreachable!(),
                            }
                        };
                        blocks.push(json!({"name":format!("b{b}"),"ops":[["value","const",{"type":"i8","value":7}]],"term":term}));
                    }
                    // Declare reachability correctly to isolate the kernel's value-use judgment.
                    let mut seen = [false; 4];
                    let mut pending = vec![0];
                    while let Some(b) = pending.pop() {
                        if seen[b] {
                            continue;
                        }
                        seen[b] = true;
                        if b != using {
                            pending.extend(edges[b].iter().copied());
                        }
                    }
                    for (b, block) in blocks.iter_mut().enumerate() {
                        block["unreachable"] = json!(!seen[b]);
                    }
                    let preflight = check(&fixture, &predicate_request(json!(false)), &source);
                    let (code, result) =
                        cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
                    assert_eq!(
                        preflight.is_ok(),
                        code == 0,
                        "graph {graph}, owner {owner}, use {using}, dialect {dialect}: {preflight:?}; {result}"
                    );
                    if let Err(error) = preflight {
                        assert_eq!(
                            error.code(),
                            AgentErrorCode::ResidualConstraintConflict,
                            "{error}"
                        );
                        assert!(error.detail().contains("does not dominate"), "{error}");
                    }
                    if (graph == 2 || graph == 3) && owner == 0 && using == 3 {
                        let (code, call) = cli(
                            &fixture.dir,
                            &[
                                "call",
                                "helper",
                                "9",
                                "false",
                                "--on",
                                result["handle"].as_str().unwrap(),
                            ],
                        );
                        assert_eq!(code, 0, "{call}");
                        assert_eq!(call["result"], json!(7));
                    }
                }
            }
        }
    }
}

fn inventory(root: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        at: &std::path::Path,
        files: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = std::collections::BTreeMap::new();
    walk(root, root, &mut files);
    files
}

use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn reachability(dialect: bool, flag: bool) -> Value {
    let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":[
        {"name":"entry","unreachable":flag,"ops":[],"term":["return","x"]},
        {"name":"unused","unreachable":true,"ops":[],"term":["return","x"]}]}]});
    if dialect {
        source["afx"] = json!(1);
    }
    source
}

fn split_value(bypass: bool) -> Value {
    json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>","blocks":[
        {"name":"entry","ops":[["checked","add?handler","x","x"],["value","const",{"type":"i8","value":7}]],"term":["br","done"]},
        {"name":"handler","ops":[],"term":if bypass {json!(["br","done"])} else {json!(["trap","unreachable"]) }},
        {"name":"done","ops":[],"term":["ok","entry.value"]}]}]})
}

#[test]
fn reachable_block_declared_unreachable_refuses_before_generation() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let source = reachability(dialect, true);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/0/unreachable")
                && error.detail().contains("reachability"),
            "{error}"
        );
    }
}

#[test]
fn split_handler_bypasses_later_definition_and_refuses_before_generation() {
    let fixture = Fixture::new();
    let source = split_value(true);
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(
        error.detail().contains("/fns/0/blocks/2/term/1")
            && error.detail().contains("/fns/0/blocks/0/ops/1")
            && error.detail().contains("does not dominate"),
        "{error}"
    );
}

fn vm(fixture: &Fixture, source: &Value, x: i8, expected: &Value) {
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, call) = cli(
        &fixture.dir,
        &[
            "call",
            "helper",
            &x.to_string(),
            "--on",
            result["handle"].as_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{call}");
    assert_eq!(&call["result"], expected);
}

#[test]
fn reachable_and_unreachable_flags_match_ordinary_default_interpretation() {
    for dialect in [false, true] {
        let fixture = Fixture::new();
        for flag in [json!(false), json!("ignored"), Value::Null] {
            let mut source = reachability(dialect, false);
            source["fns"][0]["blocks"][0]["unreachable"] = flag;
            let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
            let rows = &report["declared_body_control_flow"]["checked_functions"][0]["control_projection"]
                ["reachability_declarations"];
            assert!(
                rows.as_array()
                    .unwrap()
                    .iter()
                    .all(|r| r["status"] == "checked")
            );
            vm(&fixture, &source, -128, &json!(-128));
        }
        let mut source = reachability(dialect, false);
        source["fns"][0]["blocks"][1]
            .as_object_mut()
            .unwrap()
            .remove("unreachable");
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert!(
            error.detail().contains("/blocks/1/unreachable")
                && error.detail().contains("reachability"),
            "{error}"
        );
    }
}

#[test]
fn retained_flag_checks_use_the_new_entry_without_rewriting_accepted_blocks() {
    for dialect in [false, true] {
        let fixture = Fixture::new();
        let source = reachability(false, false);
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
        assert_eq!(code, 0, "{result}");
        let mut patch = json!({"af1":1,"patch":[{"fn":"helper","entry":"unused","blocks":{}}]});
        if dialect {
            patch["afx"] = json!(1);
        }
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error
                .detail()
                .contains("retained block `helper.entry` reachability"),
            "{error}"
        );
        patch["patch"][0]["blocks"] =
            json!({"entry":{"unreachable":true,"ops":[],"term":["return","x"]}});
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error
                .detail()
                .contains("retained block `helper.unused` reachability"),
            "{error}"
        );
        patch["patch"][0]["blocks"] = json!({"entry":{"unreachable":true,"ops":[],"term":["return","x"]},"unused":{"ops":[],"term":["return","x"]}});
        check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        vm(&fixture, &patch, 127, &json!(127));
    }
}

#[test]
fn after_split_definitions_gain_valid_proofs_and_actual_vm_results() {
    let fixture = Fixture::new();
    let source = split_value(false);
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    assert!(
        report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["name"] == "entry.value" && r["availability"] == "dominance_checked")
    );
    for x in [-64, 0, 63] {
        vm(&fixture, &source, x, &json!({"Ok":7}));
    }
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, call) = cli(
        &fixture.dir,
        &[
            "call",
            "helper",
            "127",
            "--on",
            result["handle"].as_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{call}");
    assert_eq!(call["result"]["trap"], 1);
    let (code, result) = cli(
        &fixture.dir,
        &["try", &split_value(true).to_string(), "--no-test"],
    );
    assert!(
        [1, 2].contains(&code) && result.to_string().contains("CFG_DOMINANCE"),
        "{result}"
    );
}

#[test]
fn nested_checks_and_early_exits_place_later_definitions_after_their_boundaries() {
    let fixture = Fixture::new();
    for early in [false, true] {
        let mut source = split_value(false);
        source["fns"][0]["params"] = json!([["x", "i8"], ["ready", "bool"]]);
        source["fns"][0]["blocks"][0]["ops"] = if early {
            json!([["!handler","if","ready"],["value","const",{"type":"i8","value":7}]])
        } else {
            json!([
                ["value", "tuple", ["add?handler", "x", "x"], "x"],
                ["out", "tuple_get", 0, "value"]
            ])
        };
        if !early {
            source["fns"][0]["blocks"][2]["term"] = json!(["ok", "entry.out"]);
        }
        check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
        for ready in [false, true] {
            let (code, call) = cli(
                &fixture.dir,
                &[
                    "call",
                    "helper",
                    "7",
                    &ready.to_string(),
                    "--on",
                    result["handle"].as_str().unwrap(),
                ],
            );
            assert_eq!(code, 0, "{call}");
            if early && ready {
                assert_eq!(call["result"]["trap"], 1);
            } else {
                assert_eq!(call["result"], json!({"Ok":if early {7} else {14}}));
            }
        }
        source["fns"][0]["blocks"][1]["term"] = json!(["br", "done"]);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert!(error.detail().contains("does not dominate"), "{error}");
    }
}

#[test]
fn reachability_marker_judgments_agree_with_canonical_kernel_across_small_graphs() {
    let graphs: [&[&[usize]]; 4] = [
        &[&[1], &[2], &[]],
        &[&[1, 2], &[], &[]],
        &[&[1], &[1, 2], &[]],
        &[&[], &[2], &[1]],
    ];
    for dialect in [false, true] {
        let fixture = Fixture::new();
        for edges in graphs {
            for flags in 0..8 {
                let mut blocks = Vec::new();
                for (b, targets) in edges.iter().enumerate() {
                    let term = match targets {
                        [] => json!(["return", "x"]),
                        [t] => json!(["br", format!("b{t}")]),
                        [a, c] => json!(["cond", "ready", format!("b{a}"), format!("b{c}")]),
                        _ => unreachable!(),
                    };
                    blocks.push(json!({"name":format!("b{b}"),"unreachable":flags&(1<<b)!=0,"ops":[],"term":term}));
                }
                let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":blocks}]});
                if dialect {
                    source["afx"] = json!(1);
                }
                let preflight = check(&fixture, &predicate_request(json!(false)), &source);
                let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
                assert_eq!(
                    preflight.is_ok(),
                    code == 0,
                    "{source}: {preflight:?}; {result}"
                );
                if let Err(error) = preflight {
                    assert!(error.detail().contains("reachability"), "{error}");
                }
            }
        }
    }
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
fn actual_invalid_draft_refusals_preserve_all_preexisting_bytes() {
    let mut sources = Vec::new();
    for dialect in [false, true] {
        sources.push(reachability(dialect, true));
        let mut source = reachability(dialect, false);
        source["fns"][0]["blocks"][1]["unreachable"] = json!(false);
        sources.push(source);
    }
    sources.push(split_value(true));
    for source in sources {
        let fixture = Fixture::new();
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
fn multiple_split_definition_judgments_agree_with_actual_expanded_kernel() {
    let fixture = Fixture::new();
    for nested in [false, true] {
        for placement in 0..3 {
            for first in 0..3 {
                for second in 0..3 {
                    let check1 = if nested {
                        json!(["one", "tuple", ["add?first", "x", "x"], "x"])
                    } else {
                        json!(["one", "add?first", "x", "x"])
                    };
                    let mut ops = vec![check1, json!(["two", "add?second", "x", "x"])];
                    ops.insert(placement, json!(["value","const",{"type":"i8","value":7}]));
                    let handler = |mode| match mode {
                        0 => json!(["trap", "unreachable"]),
                        1 => json!(["br", "done"]),
                        2 => json!(["br", "entry"]),
                        _ => unreachable!(),
                    };
                    let source = json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>","blocks":[
                        {"name":"entry","ops":ops,"term":["br","done"]},
                        {"name":"first","ops":[],"term":handler(first)},
                        {"name":"second","ops":[],"term":handler(second)},
                        {"name":"done","ops":[],"term":["ok","entry.value"]}]}]});
                    let preflight = check(&fixture, &predicate_request(json!(false)), &source);
                    let (code, result) =
                        cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
                    assert_eq!(
                        preflight.is_ok(),
                        code == 0,
                        "nested {nested}, position {placement}, first {first}, second {second}: {preflight:?}; {result}"
                    );
                    if let Err(error) = preflight {
                        assert_eq!(
                            error.code(),
                            AgentErrorCode::ResidualConstraintConflict,
                            "{error}"
                        );
                        assert!(
                            error.detail().contains("does not dominate")
                                && result.to_string().contains("CFG_DOMINANCE"),
                            "{error}; {result}"
                        );
                    }
                    if first == 0 && second == 0 {
                        for x in [7, 127] {
                            let (code, call) = cli(
                                &fixture.dir,
                                &[
                                    "call",
                                    "helper",
                                    &x.to_string(),
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
                }
            }
        }
    }
}

#[test]
fn generated_looking_user_block_keeps_its_canonical_reachability_obligation() {
    let fixture = Fixture::new();
    let source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":[
        {"name":"entry","ops":[],"term":["br","__none"]},
        {"name":"__none","ops":[],"term":["return","x"]}]}]});
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let mut patch = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{"entry":{"ops":[],"term":["return","x"]}}}]});
    let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
    assert!(
        error
            .detail()
            .contains("retained block `helper.__none` reachability"),
        "{error}"
    );
    patch["patch"][0]["blocks"]["__none"] =
        json!({"unreachable":true,"ops":[],"term":["return","x"]});
    check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
    vm(&fixture, &patch, 7, &json!(7));
}

use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn diamond(rebind: bool) -> Value {
    json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":[
        {"name":"entry","ops":if rebind {json!([["value","const",{"type":"i8","value":7}]])} else {json!([])},"term":["cond","ready","left","right"]},
        {"name":"left","ops":[["value","const",{"type":"i8","value":9}]],"term":["br","done"]},
        {"name":"right","ops":[],"term":["br","done"]},
        {"name":"done","ops":[],"term":["return","value"]}]}]})
}

#[test]
fn bare_result_without_a_dominating_definition_refuses_before_generation() {
    let fixture = Fixture::new();
    let source = diamond(false);
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(
        error.detail().contains("/fns/0/blocks/3/term/1")
            && error.detail().contains("/fns/0/blocks/1/ops/0")
            && error.detail().contains("does not dominate"),
        "{error}"
    );
}

#[test]
fn path_rebinding_of_a_bare_result_refuses_before_generation() {
    let fixture = Fixture::new();
    let source = diamond(true);
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(
        error.detail().contains("/fns/0/blocks/3/term/1")
            && error.detail().contains("/fns/0/blocks/0/ops/0")
            && error.detail().contains("/fns/0/blocks/1/ops/0")
            && error.detail().contains("rebind"),
        "{error}"
    );
}

#[test]
fn nearest_block_parameter_hides_an_outer_operation_before_generation() {
    let fixture = Fixture::new();
    let source = json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":[
        {"name":"entry","ops":[["value","const",{"type":"i8","value":7}]],"term":["br","middle","x"]},
        {"name":"middle","params":[["value","i8"]],"ops":[],"term":["br","done"]},
        {"name":"done","ops":[],"term":["return","value"]}]}]});
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
    assert!(
        error.detail().contains("/fns/0/blocks/2/term/1")
            && error.detail().contains("/fns/0/blocks/1/params/0")
            && error.detail().contains("visible only"),
        "{error}"
    );
}

#[test]
fn known_operation_result_index_out_of_range_refuses_before_generation() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[["value","const",{"type":"i8","value":7}]],"term":["return","value#1"]}]}]});
        if dialect {
            source["afx"] = json!(1);
        }
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/0/term/1")
                && error.detail().contains("/fns/0/blocks/0/ops/0")
                && error.detail().contains("result index"),
            "{error}"
        );
    }
}

#[test]
fn index_grammar_and_parameter_reference_behavior_match_actual_compiler_and_vm() {
    let suffixes = [
        "",
        "#0",
        "#00",
        "#+0",
        "#1",
        "#01",
        "#+1",
        "#4294967295",
        "#4294967296",
        "#-1",
        "#bad",
        "#",
        "#0#0",
    ];
    let mut comparisons = 0;
    let mut vm_cases = 0;
    for dialect in [false, true] {
        for kind in 0..4 {
            if kind == 3 && !dialect {
                continue;
            }
            let fixture = Fixture::new();
            for suffix in suffixes {
                let (name, ops) = if kind == 0 {
                    ("value", json!([["value","const",{"type":"i8","value":7}]]))
                } else if kind == 3 {
                    ("value", json!([["value", "add?fallback", "x", "x"]]))
                } else {
                    ("x", json!([]))
                };
                let mut blocks = json!([{ "name":"entry","ops":ops,"term":["return",format!("{name}{suffix}")]}]);
                if kind == 2 {
                    blocks = json!([{"name":"entry","ops":[],"term":["br","middle","x"]},{"name":"middle","params":[["value","i8"]],"ops":[],"term":["return",format!("value{suffix}")]}]);
                }
                if kind == 3 {
                    blocks
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"name":"fallback","ops":[],"term":["trap","unreachable"]}));
                }
                let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":blocks}]});
                if dialect {
                    source["afx"] = json!(1);
                }
                let preflight = check(&fixture, &predicate_request(json!(false)), &source);
                let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
                assert_eq!(
                    preflight.is_ok(),
                    code == 0,
                    "kind {kind}, dialect {dialect}, suffix {suffix}: {preflight:?}; {result}"
                );
                comparisons += 1;
                if let Err(error) = &preflight {
                    assert!(error.detail().contains("result index"), "{error}");
                }
                if code == 0 {
                    let (code, call) = cli(
                        &fixture.dir,
                        &[
                            "call",
                            "helper",
                            "7",
                            "--on",
                            result["handle"].as_str().unwrap(),
                        ],
                    );
                    assert_eq!(code, 0, "{call}");
                    assert_eq!(call["result"], json!(if kind == 3 { 14 } else { 7 }));
                    vm_cases += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 91);
    assert_eq!(vm_cases, 48);
    eprintln!("NAME_INDEX_ORACLE comparisons={comparisons} vm_cases={vm_cases}");
}

#[test]
fn nearest_and_rebinding_judgments_match_canonical_compiler_across_small_graphs() {
    let graphs: [&[&[usize]]; 4] = [
        &[&[1], &[2], &[3], &[]],
        &[&[1, 2], &[3], &[3], &[]],
        &[&[1], &[1, 2], &[3], &[]],
        &[&[1], &[2], &[1, 3], &[]],
    ];
    let fixture = Fixture::new();
    let mut comparisons = 0;
    let mut vm_cases = 0;
    for edges in graphs {
        for entry_value in [false, true] {
            for declarations in 0..27 {
                if !entry_value && declarations == 0 {
                    continue;
                }
                let mut kinds = [usize::from(entry_value), 0, 0, 0];
                let mut rest = declarations;
                for kind in kinds.iter_mut().skip(1) {
                    *kind = rest % 3;
                    rest /= 3;
                }
                let target = |b: usize| {
                    if kinds[b] == 2 {
                        json!([format!("b{b}"), "x"])
                    } else {
                        json!(format!("b{b}"))
                    }
                };
                let mut blocks = Vec::new();
                for (b, targets) in edges.iter().enumerate() {
                    let term = if b == 3 {
                        json!(["return", "value"])
                    } else {
                        match targets {
                            [] => json!(["return", "x"]),
                            [t] => json!(["br", target(*t)]),
                            [a, c] => json!(["cond", "ready", target(*a), target(*c)]),
                            _ => unreachable!(),
                        }
                    };
                    blocks.push(json!({"name":format!("b{b}"),"params":if kinds[b]==2 {json!([["value","i8"]])} else {json!([])},"ops":if kinds[b]==1 {json!([["value","const",{"type":"i8","value":(b+1)*7}]])} else {json!([])},"term":term}));
                }
                let source = json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":blocks}]});
                let preflight = check(&fixture, &predicate_request(json!(false)), &source);
                let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
                assert_eq!(
                    preflight.is_ok(),
                    code == 0,
                    "{source}: {preflight:?}; {result}"
                );
                comparisons += 1;
                if let Ok(report) = preflight {
                    let read =
                        report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|r| r["at"] == "/fns/0/blocks/3/term/1")
                            .unwrap();
                    let locator = read["definition"].as_str().unwrap();
                    let b: usize = locator.split('/').nth(4).unwrap().parse().unwrap();
                    let expected = if kinds[b] == 2 { 9 } else { (b + 1) * 7 };
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
                    assert_eq!(call["result"], json!(expected));
                    vm_cases += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 212);
    eprintln!("NAME_NEAREST_ORACLE comparisons={comparisons} vm_cases={vm_cases}");
}

#[test]
fn nearest_after_multiple_splits_matches_actual_expanded_binding() {
    let fixture = Fixture::new();
    let mut comparisons = 0;
    let mut vm_cases = 0;
    for nested in [false, true] {
        for placement in 0..3 {
            for first in 0..3 {
                for second in 0..3 {
                    let one = if nested {
                        json!(["one", "tuple", ["add?first", "x", "x"], "x"])
                    } else {
                        json!(["one", "add?first", "x", "x"])
                    };
                    let mut ops = vec![one, json!(["two", "add?second", "x", "x"])];
                    ops.insert(placement, json!(["value","const",{"type":"i8","value":7}]));
                    let handler = |mode| match mode {
                        0 => json!(["trap", "unreachable"]),
                        1 => json!(["br", "done"]),
                        2 => json!(["br", "entry"]),
                        _ => unreachable!(),
                    };
                    let source = json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>","blocks":[
                        {"name":"entry","ops":ops,"term":["br","done"]},{"name":"first","ops":[],"term":handler(first)},{"name":"second","ops":[],"term":handler(second)},{"name":"done","ops":[],"term":["ok","value"]}]}]});
                    let preflight = check(&fixture, &predicate_request(json!(false)), &source);
                    let (code, result) =
                        cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
                    assert_eq!(
                        preflight.is_ok(),
                        code == 0,
                        "{source}: {preflight:?}; {result}"
                    );
                    comparisons += 1;
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
                            vm_cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 54);
    assert_eq!(vm_cases, 12);
    eprintln!("NAME_SPLIT_ORACLE comparisons={comparisons} vm_cases={vm_cases}");
}

#[test]
fn retained_declarations_keep_nearest_identity_and_exact_index_cardinality() {
    let fixture = Fixture::new();
    let source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[["value","const",{"type":"i8","value":7}]],"term":["br","done"]},{"name":"done","ops":[],"term":["return","entry.value"]}]}]});
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    for reference in ["value", "value#0", "entry.value#0"] {
        let patch = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{"done":{"ops":[["marker","const",{"type":"bool","value":true}]],"term":["return",reference]}}}]});
        let report = check(&fixture, &predicate_request(json!(false)), &patch).unwrap();
        let read = report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == reference)
            .unwrap();
        assert_eq!(
            read["result_index_judgment"],
            "operation_result_index_checked"
        );
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
        assert_eq!(call["result"], json!(7));
    }
    for reference in ["value#1", "entry.value#1"] {
        let patch = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{"done":{"ops":[],"term":["return",reference]}}}]});
        let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
        assert!(
            error.detail().contains("result index")
                && error
                    .detail()
                    .contains("accepted value `helper.entry.value`"),
            "{error}"
        );
    }
}

#[test]
fn unknown_names_and_incomplete_graphs_keep_deferred_bindings_without_rewriting() {
    let fixture = Fixture::new();
    let mut source = diamond(false);
    source["fns"][0]["blocks"][3]["term"][1] = json!("unknown");
    let original = super::bytes(&source);
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    assert!(
        report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["name"] == "unknown" && r["availability"] == "availability_deferred")
    );
    assert_eq!(super::bytes(&source), original);
    source["fns"][0]["blocks"][3]["term"][1] = json!("value");
    source["fns"][0]["blocks"][0]["term"][3] = json!("pending");
    assert!(check(&fixture, &predicate_request(json!(false)), &source).is_err());
    // An unsupported operation makes the preparatory view partial; the source
    // ownership/target checker remains independent of far-name speculation.
    source["fns"][0]["blocks"][0]["term"][3] = json!("right");
    source["fns"][0]["blocks"][0]["ops"] = json!([["opaque", "unimplemented", "x"]]);
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    assert_eq!(
        report["declared_body_control_flow"]["checked_functions"][0]["control_projection"]["status"],
        "deferred"
    );
}

#[test]
fn differing_outer_types_and_retained_hidden_parameters_keep_ordinary_bindings() {
    let fixture = Fixture::new();
    let source = json!({"af1":1,"afx":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"u8","blocks":[
        {"name":"entry","ops":[["value","const",{"type":"i8","value":7}]],"term":["br","middle"]},
        {"name":"middle","ops":[["check","add?fallback","x","x"],["value","const",{"type":"u8","value":9}]],"term":["br","done"]},
        {"name":"fallback","ops":[],"term":["trap","unreachable"]},
        {"name":"done","ops":[],"term":["return","value"]}]}]});
    let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    let read = report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["at"] == "/fns/0/blocks/3/term/1")
        .unwrap();
    assert_eq!(read["definition"], "/fns/0/blocks/1/ops/1");
    assert_eq!(read["availability"], "nearest_definition_checked");
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    for x in [7, 63, 64] {
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
        if x == 64 {
            assert_eq!(call["result"]["trap"], 1);
        } else {
            assert_eq!(call["result"], json!(9));
        }
    }
    let fixture = Fixture::new();
    let source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":[
        {"name":"entry","ops":[["value","const",{"type":"i8","value":7}]],"term":["br","middle","x"]},
        {"name":"middle","params":[["value","i8"]],"ops":[],"term":["br","done"]},
        {"name":"done","ops":[],"term":["return","entry.value"]}]}]});
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    let patch = json!({"af1":1,"afx":1,"patch":[{"fn":"helper","blocks":{"done":{"ops":[],"term":["return","value"]}}}]});
    let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
    assert!(
        error
            .detail()
            .contains("accepted value `helper.middle.value`")
            && error.detail().contains("visible only"),
        "{error}"
    );
    let mut plain = diamond(false);
    plain.as_object_mut().unwrap().remove("afx");
    let error = check(&fixture, &predicate_request(json!(false)), &plain).unwrap_err();
    assert!(
        error.detail().contains("plain AF1") && error.detail().contains("without qualification"),
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
fn source_name_and_index_refusals_preserve_existing_draft_and_head_bytes() {
    let mut sources = vec![diamond(false), diamond(true)];
    for dialect in [false, true] {
        let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[["value","const",{"type":"i8","value":7}]],"term":["return","value#1"]}]}]});
        if dialect {
            source["afx"] = json!(1);
        }
        sources.push(source);
    }
    let mut source = diamond(false);
    source["fns"][0]["blocks"][3]["term"][1] = json!("x#bad");
    sources.push(source);
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

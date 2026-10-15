use super::interface_tests::{check, predicate_request};
use super::{Fixture, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;

fn source(ops: Value, term: Value, dialect: bool) -> Value {
    let mut source = json!({"af1":1,"fns":[{"fn":"helper","params":[["x","i8"],["ready","bool"]],"returns":"i8","blocks":[{"name":"entry","ops":[],"term":[]}]}]});
    source["fns"][0]["blocks"][0]["ops"] = ops;
    source["fns"][0]["blocks"][0]["term"] = term;
    if dialect {
        source["afx"] = json!(1);
    }
    source
}

#[test]
fn authored_forward_reads_refuse_before_generation_at_use_and_definition() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let source = source(
            json!([["early","add","later","x"],["later","const",{"type":"i8","value":2}]]),
            json!(["return", "x"]),
            dialect,
        );
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/0/ops/0/2")
                && error.detail().contains("/fns/0/blocks/0/ops/1")
                && error.detail().contains("before its definition"),
            "{error}"
        );
    }
}

#[test]
fn qualified_parameters_of_another_authored_block_refuse_before_generation() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let mut source = source(json!([]), json!(["br", "other", "x"]), dialect);
        source["fns"][0]["blocks"].as_array_mut().unwrap().extend([
            json!({"name":"other","params":[["p","i8"]],"ops":[],"term":["br","done"]}),
            json!({"name":"done","ops":[],"term":["return","other.p"]}),
        ]);
        let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/fns/0/blocks/2/term/1")
                && error.detail().contains("/fns/0/blocks/1/params/0")
                && error.detail().contains("visible only in its own block"),
            "{error}"
        );
    }
}

#[test]
fn self_qualified_indexed_and_shadowing_reads_obey_local_order() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        for ops in [
            json!([["out", "tuple", "out", "x"]]),
            json!([["early","tuple","entry.later#0","ready"],["later","const",{"type":"i8","value":7}]]),
            json!([["early","tuple","x","ready"],["x","const",{"type":"i8","value":7}]]),
        ] {
            let source = source(ops, json!(["return", "x"]), dialect);
            let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            assert!(
                error.detail().contains("/ops/0/2")
                    && error.detail().contains("before its definition"),
                "{error}"
            );
        }
    }
    let source = source(
        json!([["out", "tuple_get", 0, ["tuple", "out", "x"]]]),
        json!(["return", "x"]),
        true,
    );
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert!(
        error.detail().contains("/ops/0/3/1") && error.detail().contains("before its definition"),
        "{error}"
    );
}

#[test]
fn valid_local_results_and_own_block_parameters_execute_at_integer_boundaries() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        for block_parameter in [false, true] {
            let mut source = source(
                json!([
                    ["pair", "tuple", "x", "ready"],
                    ["out", "tuple_get", 0, "pair"]
                ]),
                json!(["return", "entry.out#0"]),
                dialect,
            );
            if block_parameter {
                source["fns"][0]["blocks"] = json!([
                    {"name":"entry","ops":[],"term":["br","work","x"]},
                    {"name":"work","params":[["p","i8"]],"ops":[["pair","tuple","work.p#0","ready"],["out","tuple_get",0,"pair"]],"term":["return","out"]}]);
            }
            let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
            assert!(
                !report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{result}");
            for x in [-128, -1, 0, 1, 127] {
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
                    assert_eq!(call["result"], json!(x));
                }
            }
        }
    }
}

#[test]
fn opcode_immediates_and_literal_strings_do_not_become_future_value_reads() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let mut source = source(
            json!([
            ["note","const",{"type":"text","value":"callee"}],
            ["out","call","callee","x"],
            ["callee","const",{"type":"i8","value":7}]]),
            json!(["return", "out"]),
            dialect,
        );
        source["fns"].as_array_mut().unwrap().push(json!({"fn":"callee","params":[["v","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":[],"term":["return","v"]}]}));
        let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        assert!(
            report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
                .as_array()
                .unwrap()
                .iter()
                .all(|read| read["name"] != "callee")
        );
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
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
        assert_eq!(call["result"], json!(9));
    }
}

#[test]
fn cross_block_operations_gain_dominance_while_far_and_unknown_names_stay_deferred() {
    let fixture = Fixture::new();
    for dialect in [false, true] {
        let mut source = source(
            json!([["value","const",{"type":"i8","value":7}]]),
            json!(["br", "done"]),
            dialect,
        );
        source["fns"][0]["blocks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"done","ops":[],"term":["return","entry.value"]}));
        let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        let reads = report["declared_body_control_flow"]["checked_functions"][0]["value_reads"]
            .as_array()
            .unwrap();
        assert!(reads.iter().any(
            |read| read["name"] == "entry.value" && read["availability"] == "dominance_checked"
        ));
        let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{result}");
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
        if dialect {
            source["fns"][0]["blocks"][1]["term"][1] = json!("value");
            check(&fixture, &predicate_request(json!(false)), &source).unwrap();
            let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{result}");
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
        source["fns"][0]["blocks"][1]["term"][1] = json!("unknown");
        let report = check(&fixture, &predicate_request(json!(false)), &source).unwrap();
        assert_eq!(report["composition"], "partial");
        assert!(
            report["declared_body_control_flow"]["deferred"]
                .as_array()
                .unwrap()
                .contains(&json!("/fns/0/blocks/1/term/1"))
        );
    }
}

#[test]
fn checked_continuations_cannot_be_read_through_another_authored_block() {
    let fixture = Fixture::new();
    let mut source = source(
        json!([["out", "add?", "x", "x"]]),
        json!(["br", "done"]),
        true,
    );
    source["fns"][0]["returns"] = json!("Result<i8,ArithmeticError>");
    source["fns"][0]["blocks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"done","ops":[],"term":["ok","entry.out"]}));
    let error = check(&fixture, &predicate_request(json!(false)), &source).unwrap_err();
    assert!(
        error.detail().contains("/blocks/1/term/1")
            && error.detail().contains("/blocks/0/ops/0")
            && error.detail().contains("visible only in its own block"),
        "{error}"
    );
    source["fns"][0]["blocks"][0]["term"] = json!(["br", "done", "out"]);
    source["fns"][0]["blocks"][1]["params"] = json!([["out", "i8"]]);
    source["fns"][0]["blocks"][1]["term"] = json!(["ok", "out"]);
    check(&fixture, &predicate_request(json!(false)), &source).unwrap();
    let (code, result) = cli(&fixture.dir, &["try", &source.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, call) = cli(
        &fixture.dir,
        &[
            "call",
            "helper",
            "7",
            "false",
            "--on",
            result["handle"].as_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{call}");
    assert_eq!(call["result"], json!({"Ok":14}));
}

#[test]
fn incompatible_authored_patches_preserve_existing_workspace_bytes_on_refusal() {
    for dialect in [false, true] {
        for foreign in [false, true] {
            let fixture = Fixture::new();
            let mut initial = source(json!([]), json!(["br", "other", "x"]), false);
            initial["fns"][0]["blocks"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name":"other","params":[["p","i8"]],"ops":[],"term":["return","p"]}));
            let (code, result) = cli(&fixture.dir, &["try", &initial.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{result}");
            let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
            assert_eq!(code, 0, "{result}");
            let mut patch = if foreign {
                json!({"af1":1,"patch":[{"fn":"helper","blocks":{
            "entry":{"ops":[],"term":["br","done"]},"done":{"ops":[],"term":["return","other.p"]}}}]})
            } else {
                json!({"af1":1,"patch":[{"fn":"helper","blocks":{
            "other":{"params":[["p","i8"]],"ops":[["early","tuple","later","ready"],["later","const",{"type":"i8","value":7}]],"term":["return","p"]}}}]})
            };
            if dialect {
                patch["afx"] = json!(1);
            }
            let error = check(&fixture, &predicate_request(json!(false)), &patch).unwrap_err();
            assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
            assert!(
                error.detail().contains(if foreign {
                    "accepted value `helper.other.p`"
                } else {
                    "/patch/0/blocks/other/ops/1"
                }),
                "{error}"
            );
            let (code, result) = cli(&fixture.dir, &["try", &patch.to_string(), "--no-test"]);
            assert!([1, 2].contains(&code), "{result}");
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

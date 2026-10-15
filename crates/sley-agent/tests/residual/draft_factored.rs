use std::fs;

use serde_json::{Value, json};
use sley_agent::residual::{
    binding::DraftSource, dependencies, factored, frontier::Budget, parse_request,
};
use sley_agent::{
    candidate, hex,
    names::{NameMap, Names},
};

use super::dependency_tests::literal_function;
use super::{Fixture, bytes, cli, per_target_request, runtime};

fn source(functions: Vec<Value>) -> Fixture {
    let fixture = Fixture::new();
    let (code, result) = cli(
        &fixture.dir,
        &[
            "try",
            &json!({"af1":1,"fns":Value::Array(functions)}).to_string(),
            "--no-test",
        ],
    );
    assert_eq!(code, 0, "{result}");
    fixture
}

fn request(names: &[String]) -> Value {
    let mut request = per_target_request(json!({}));
    request["base"] = json!("d1@r1");
    request["scope"] = json!(
        names
            .iter()
            .map(|name| format!("{name}.entry.amount"))
            .collect::<Vec<_>>()
    );
    let constraints: Vec<_> = names
        .iter()
        .map(|name| {
            let path = format!("/bindings/values/{name}.entry.amount");
            json!({"fields":[path],"rows":[{&path:3},{&path:7}]})
        })
        .collect();
    request["choices"] = json!({"version":2,"contract":"author_closed_constraints","constraints":constraints,"dependencies":[]});
    request
}

fn names(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("site{i}")).collect()
}

fn fill(plan: &Value, value: i64) -> Value {
    let choose: serde_json::Map<String, Value> = plan["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| (field["path"].as_str().unwrap().to_owned(), json!(value)))
        .collect();
    json!({"residual":1,"plan":plan["plan"],"choose":choose})
}

fn candidate(fixture: &Fixture, handle: &str) -> sley_mutate::ImportedCandidate {
    let path = fixture.dir.join(format!(".sley/candidates/{handle}.hex"));
    sley_mutate::import_candidate(&hex::decode(fs::read_to_string(path).unwrap().trim()).unwrap())
        .unwrap()
}

#[test]
fn draft_factoring_uses_created_graph_and_never_materializes_the_million_completions() {
    let names = names(20);
    let fixture = source(names.iter().map(|name| literal_function(name)).collect());
    let head = fixture.workspace.read_head().unwrap();
    let old = candidate(&fixture, "c1");
    let program = candidate::applied_program(&head, &old).unwrap();
    let map = NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap();
    let symbols = Names::build(&program, &map);
    let request = request(&names);
    let (code, plan) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 1, "{plan}");
    assert_eq!(plan["entitlement"]["components"], 20);
    assert_eq!(plan["entitlement"]["description_count_decimal"], "1048576");
    assert_eq!(plan["entitlement"]["materialized_component_rows"], 40);
    assert_eq!(plan["entitlement"]["global_product_materialized"], false);
    assert_eq!(plan["family_checks"]["source_kernel"], "valid");
    assert_eq!(plan["family_checks"]["kernel"], "not_run");
    let handle = plan["plan"].as_str().unwrap();
    let (_, shown) = cli(&fixture.dir, &["residual", "show", handle, "--provenance"]);
    assert_eq!(
        shown["dependencies"]["profile"],
        "draft-integer-literal-dependencies-v1"
    );
    assert_eq!(shown["dependencies"]["source"]["revision"], "d1@r1");
    assert_eq!(
        shown["dependencies"]["source"]["candidate_sha256"],
        sley_agent::draft::sha256(&old.stored_bytes)
    );
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill(&plan, 7).to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["draft"], "d1@r2");
    let new = candidate(&fixture, result["handle"].as_str().unwrap());
    assert_eq!(new.record.candidate_nonce, old.record.candidate_nonce);
    let after = candidate::applied_program(&head, &new).unwrap();
    let targets: Vec<_> = names
        .iter()
        .map(|name| symbols.resolve(&format!("{name}.entry.amount")).unwrap())
        .collect();
    for object in program.objects() {
        let id = object.record().entity_id;
        if !targets.contains(&id) {
            assert_eq!(
                object.stored_bytes(),
                after.object(&id).unwrap().stored_bytes()
            );
        }
    }
    let events = fs::read_to_string(fixture.dir.join(".sley/events.jsonl")).unwrap();
    let event: Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
    assert_eq!(event["residual"]["source_kernel_validations"], 1);
    assert_eq!(event["residual"]["kernel_validations"], 2);
    for name in names {
        let (_, ran) = cli(
            &fixture.dir,
            &[
                "call",
                &name,
                "5",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(ran["result"], json!({"Ok":12}));
    }
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        head.transaction_id()
    );
}

#[test]
fn draft_factoring_caller_coupling_cannot_be_declared_away() {
    let names = names(9);
    let mut functions: Vec<_> = names.iter().map(|name| literal_function(name)).collect();
    let ops: Vec<_> = names
        .iter()
        .map(|name| json!([name, "call", name, "x"]))
        .collect();
    functions.push(json!({"fn":"bridge","params":[["x","i8"]],"returns":"i8",
        "blocks":[{"name":"entry","ops":ops,"term":["return","x"]}]}));
    let fixture = source(functions);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "plan", &request(&names).to_string()],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_LIMIT");
    assert!(!fixture.dir.join(".sley/residual").exists());
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
}

#[test]
fn bound_draft_factoring_rechecks_receipts_preserves_budget_and_does_not_weaken_public_api() {
    let names = names(2);
    let fixture = source(names.iter().map(|name| literal_function(name)).collect());
    let input = bytes(&request(&names));
    let source = DraftSource::capture(
        &fixture.workspace,
        &input,
        &runtime(),
        &mut Budget::default(),
    )
    .unwrap();
    let choices = source
        .analyze_factors(&fixture.workspace, &mut Budget::default())
        .unwrap();
    assert_eq!(choices.summary()["components"], 2);
    assert!(!fixture.dir.join(".sley/residual").exists());
    let head = fixture.workspace.read_head().unwrap();
    let program = candidate::applied_program(&head, &candidate(&fixture, "c1")).unwrap();
    let symbols = Names::build(
        &program,
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let parsed = parse_request(&input).unwrap();
    assert!(dependencies::extract(&program, &symbols, &parsed, &mut Budget::default()).is_err());
    assert!(factored::analyze(&parsed, &program, &symbols, &mut Budget::default()).is_err());
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 0);
    assert_eq!(
        source
            .analyze_factors(&fixture.workspace, &mut budget)
            .err()
            .unwrap()
            .code(),
        sley_agent::AgentErrorCode::ResidualLimit
    );
    let path = fixture.dir.join(".sley/drafts/d1/r1/frame.json");
    let mut changed = fs::read(&path).unwrap();
    changed.push(b' ');
    fs::write(path, changed).unwrap();
    assert_eq!(
        source
            .analyze_factors(&fixture.workspace, &mut Budget::default())
            .err()
            .unwrap()
            .code(),
        sley_agent::AgentErrorCode::ResidualBindingStale
    );
}

#[test]
fn draft_factored_singletons_keep_noop_and_public_check_semantics() {
    for value in [3, 7] {
        let names = names(2);
        let fixture = source(names.iter().map(|name| literal_function(name)).collect());
        let mut request = request(&names);
        for table in request["choices"]["constraints"].as_array_mut().unwrap() {
            let path = table["fields"][0].as_str().unwrap().to_owned();
            table["rows"] = json!([{path:value}]);
        }
        let cases = fixture.dir.join("public.json");
        fs::write(
            &cases,
            json!([{"function":"site0","args":[5],"expect":{"Ok":8}}]).to_string(),
        )
        .unwrap();
        let (code, result) = cli(
            &fixture.dir,
            &[
                "residual",
                "try",
                &request.to_string(),
                "--public",
                cases.to_str().unwrap(),
            ],
        );
        assert_eq!(code, i32::from(value != 3), "{result}");
        assert_eq!(result["kernel"], "valid");
        assert_eq!(
            result["public_checks"],
            if value == 3 { "passed" } else { "failed" }
        );
        if value == 3 {
            assert_eq!(result["no_change"], true);
            assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
        } else {
            assert_eq!(result["draft"], "d1@r2");
        }
        let reference = result[if value == 3 { "residual" } else { "draft" }]
            .as_str()
            .unwrap();
        let (_, shown) = cli(
            &fixture.dir,
            &["residual", "show", reference, "--provenance"],
        );
        for origin in shown["resolution"]["origins"].as_object().unwrap().values() {
            assert_eq!(origin["class"], "DERIVED");
            assert_eq!(
                *request
                    .pointer(origin["value_source"]["pointer"].as_str().unwrap())
                    .unwrap(),
                value
            );
        }
    }
}

#[test]
fn draft_factored_stale_fill_and_invalid_domains_publish_no_successor() {
    let names = names(2);
    let fixture = source(names.iter().map(|name| literal_function(name)).collect());
    let mut invalid = request(&names);
    invalid["choices"]["constraints"][0]["rows"][1]["/bindings/values/site0.entry.amount"] =
        json!(128);
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &invalid.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert!(!fixture.dir.join(".sley/residual").exists());
    let (_, plan) = cli(
        &fixture.dir,
        &["residual", "plan", &request(&names).to_string()],
    );
    assert!(plan["plan"].is_string(), "{plan}");
    let (code, next) = cli(
        &fixture.dir,
        &["try", r#"{"af1":1}"#, "--on", "d1@r1", "--no-test"],
    );
    assert_eq!(code, 0, "{next}");
    let (code, result) = cli(
        &fixture.dir,
        &[
            "residual",
            "fill",
            plan["plan"].as_str().unwrap(),
            &fill(&plan, 7).to_string(),
        ],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_BINDING_STALE");
    assert!(!fixture.dir.join(".sley/residual/fills").exists());
    assert!(!fixture.dir.join(".sley/candidates/c3.hex").exists());
}

#[test]
fn draft_factored_correlated_values_keep_exact_answer_and_constraint_provenance() {
    let names = names(2);
    let fixture = source(names.iter().map(|name| literal_function(name)).collect());
    let mut request = request(&names);
    request["choices"]["constraints"] = json!([{
        "fields":["/bindings/values/site0.entry.amount","/bindings/values/site1.entry.amount"],
        "rows":[{"/bindings/values/site0.entry.amount":3,"/bindings/values/site1.entry.amount":3},
            {"/bindings/values/site0.entry.amount":7,"/bindings/values/site1.entry.amount":9}]}]);
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    assert_eq!(plan["unresolved"].as_array().unwrap().len(), 1);
    let path = plan["unresolved"][0]["path"].as_str().unwrap();
    let answer = json!({"residual":1,"plan":plan["plan"],"choose":{
        path:request["choices"]["constraints"][0]["rows"][1][path]}});
    let (code, result) = cli(
        &fixture.dir,
        &[
            "residual",
            "fill",
            plan["plan"].as_str().unwrap(),
            &answer.to_string(),
        ],
    );
    assert_eq!(code, 0, "{result}");
    let (_, shown) = cli(
        &fixture.dir,
        &[
            "residual",
            "show",
            result["draft"].as_str().unwrap(),
            "--provenance",
        ],
    );
    let proof = &shown["resolution"];
    assert_eq!(
        proof["selected_constraint_rows"],
        json!([{"constraint":0,"row":1}])
    );
    assert_eq!(proof["dependencies"]["source"]["revision"], "d1@r1");
    let original_plan = json!({"artifacts":{"residual-request.json":request}});
    for (field, origin) in proof["origins"].as_object().unwrap() {
        let expected = proof["request"].pointer(field).unwrap();
        if origin["class"] == "AUTHORED" {
            assert_eq!(origin["document"], "residual-fill.json");
            assert_eq!(
                answer.pointer(origin["pointer"].as_str().unwrap()).unwrap(),
                expected
            );
        } else {
            assert_eq!(origin["class"], "DERIVED");
            assert_eq!(origin["value_source"]["document"], "residual-plan.json");
            assert_eq!(
                original_plan
                    .pointer(origin["value_source"]["pointer"].as_str().unwrap())
                    .unwrap(),
                expected
            );
        }
    }
    let entries = shown["provenance"]["entries"].as_object().unwrap();
    assert!(
        entries
            .values()
            .any(|origin| origin["rule"] == "closed-author-constraints-v1")
    );
    for (name, expected) in [("site0", 12), ("site1", 14)] {
        let (_, ran) = cli(
            &fixture.dir,
            &[
                "call",
                name,
                "5",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(ran["result"], json!({"Ok":expected}));
    }
}

#[test]
fn draft_factoring_valid_status_cannot_substitute_for_source_kernel_validation() {
    let names = names(1);
    let fixture = source(names.iter().map(|name| literal_function(name)).collect());
    let mut record = candidate(&fixture, "c1").record;
    record.expiry = sley_mutate::CandidateExpiry::unix_millis(1);
    let expired = sley_mutate::build_candidate(&record).unwrap();
    fs::write(
        fixture.dir.join(".sley/candidates/c1.hex"),
        hex::encode(&expired.stored_bytes),
    )
    .unwrap();
    let path = fixture.dir.join(".sley/drafts/d1/r1/status.json");
    let mut status: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(status["state"], "valid");
    status["candidate_sha256"] = json!(sley_agent::draft::sha256(&expired.stored_bytes));
    fs::write(path, status.to_string()).unwrap();
    let source = DraftSource::capture(
        &fixture.workspace,
        &bytes(&request(&names)),
        &runtime(),
        &mut Budget::default(),
    )
    .unwrap();
    let error = source
        .analyze_factors(&fixture.workspace, &mut Budget::default())
        .err()
        .unwrap();
    assert_eq!(error.code(), sley_agent::AgentErrorCode::ResidualPreserve);
    assert!(error.detail().contains("failed kernel validation"));
    assert!(!fixture.dir.join(".sley/residual").exists());
}

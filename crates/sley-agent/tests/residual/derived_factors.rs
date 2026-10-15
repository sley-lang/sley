use super::{
    Fixture, branch_request, bytes, cli, fixture_names, planning_request, relation_request, request,
};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::residual::{factored, frontier::Budget, parse_request};
use std::time::Duration;

fn guard() -> Value {
    let mut value = request("current");
    value["bindings"] = json!({"params":[["x","i8"]],"returns":"Option<i8>",
        "guards":[{"when":false,"fail":["fail"]}],
        "success":{"ops":[],"term":["return",["some","x"]]}});
    value
}

fn domain(mut request: Value, path: &str, values: &[Value]) -> Value {
    let (parent, key) = path.rsplit_once('/').unwrap();
    request
        .pointer_mut(parent)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(key);
    request["choices"] = json!({"version":2,"contract":"author_closed_constraints",
        "constraints":[{"fields":[path],"rows":values.iter().map(|v| json!({path:v})).collect::<Vec<_>>() }],
        "dependencies":[]});
    request
}

fn correlated() -> Value {
    let mut request = relation_request();
    let rows = request["choices"]["rows"].clone();
    request["choices"] = json!({"version":2,"contract":"author_closed_constraints",
        "constraints":[{"fields":["/bindings/params","/bindings/returns","/bindings/rounding"],"rows":rows}],
        "dependencies":[]});
    request
}

fn no_artifacts(fixture: &Fixture) {
    for path in ["residual", "drafts", "candidates"] {
        assert!(!fixture.dir.join(".sley").join(path).exists(), "{path}");
    }
}

#[test]
fn derived_constraint_plans_fill_and_execute_all_three_fragment_families() {
    for (request, path, value, argument, expected) in [
        (
            domain(
                guard(),
                "/bindings/guards/0/when",
                &[json!(true), json!(false)],
            ),
            "/bindings/guards/0/when",
            json!(false),
            "5",
            json!({"Some":5}),
        ),
        (
            domain(
                planning_request(),
                "/bindings/result",
                &[json!("answer"), json!("scaled")],
            ),
            "/bindings/result",
            json!("scaled"),
            "4",
            json!({"Ok":12}),
        ),
        (
            domain(
                branch_request(),
                "/bindings/cases/1/values",
                &[json!([17]), json!([19])],
            ),
            "/bindings/cases/1/values",
            json!([19]),
            "\"None\"",
            json!({"Ok":19}),
        ),
    ] {
        let fixture = Fixture::new();
        let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
        assert_eq!(code, 0, "{plan}");
        assert_eq!(plan["entitlement"]["components"], 1);
        assert_eq!(
            plan["family_checks"]["interface_preflight"]["completions"],
            2
        );
        assert_eq!(
            plan["family_checks"]["interface_preflight"]["composition"],
            "partial"
        );
        assert!(plan["family_checks"].get("inherited_widths").is_none());
        assert_eq!(plan["family_checks"]["compiler"], "not_run");
        let handle = plan["plan"].as_str().unwrap();
        let fill = json!({"residual":1,"plan":handle,"choose":{path:value}});
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "fill", handle, &fill.to_string()],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["kernel"], "valid");
        let (code, called) = cli(
            &fixture.dir,
            &[
                "call",
                request["scope"][0].as_str().unwrap(),
                argument,
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{called}");
        assert_eq!(called["result"], expected);
    }
}

#[test]
fn derived_correlated_interfaces_keep_authored_domains_and_exact_fill_sources() {
    let fixture = Fixture::new();
    let mut request = correlated();
    // Supplying the same mandatory edge in reverse order is valid and does
    // not create a duplicate when the adapter adds its own coupling.
    request["choices"]["dependencies"] = json!([{"kind":"binding","fields":[
        "/bindings/rounding","/bindings/returns","/bindings/params"]}]);
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    assert_eq!(plan["unresolved"].as_array().unwrap().len(), 1);
    let path = plan["unresolved"][0]["path"].as_str().unwrap();
    let value = request["choices"]["constraints"][0]["rows"][0][path].clone();
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":{path:value}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    let (_, shown) = cli(&fixture.dir, &["residual", "show", "d1@r1", "--provenance"]);
    assert_eq!(
        shown["resolution"]["request"]["bindings"]["returns"],
        "Result<i8,ArithmeticError>"
    );
    assert_eq!(shown["resolution"]["origins"][path]["class"], "AUTHORED");
    assert_eq!(
        shown["resolution"]["selected_constraint_rows"],
        json!([{"constraint":0,"row":0}])
    );
}

#[test]
fn derived_crossed_interfaces_refuse_the_family_without_pruning_bad_completions() {
    let fixture = Fixture::new();
    let mut request = correlated();
    request["choices"]["constraints"] = json!([
        {"fields":["/bindings/params"],"rows":[{"/bindings/params":[["x","i8"]]},{"/bindings/params":[["x","i64"]]}]},
        {"fields":["/bindings/returns"],"rows":[{"/bindings/returns":"Result<i8,ArithmeticError>"},{"/bindings/returns":"Result<i64,ArithmeticError>"}]},
        {"fields":["/bindings/rounding"],"rows":[{"/bindings/rounding":"toward_zero"}]}]);
    let original = bytes(&request);
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
    assert_eq!(bytes(&request), original);
    no_artifacts(&fixture);
}

#[test]
fn derived_decisions_cannot_claim_independence_to_escape_the_component_bound() {
    let fixture = Fixture::new();
    let mut request = guard();
    request["bindings"]["guards"] =
        json!((0..9).map(|_| json!({"fail":["fail"]})).collect::<Vec<_>>());
    request["choices"] = json!({"version":2,"contract":"author_closed_constraints","dependencies":[],
        "constraints":(0..9).map(|index|{let path=format!("/bindings/guards/{index}/when");
            json!({"fields":[path],"rows":[{&path:false},{&path:true}]})}).collect::<Vec<_>>()});
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_LIMIT");
    no_artifacts(&fixture);
}

#[test]
fn derived_constraints_resolve_types_from_the_exact_validated_draft() {
    let fixture = Fixture::new();
    let frame = json!({"af1":1,"types":[{"name":"Box","record":[["n","i8"]]}]});
    let (code, source) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{source}");
    let mut request = guard();
    request["base"] = json!("d1@r1");
    request["bindings"]["params"] = json!([["x", "Box"]]);
    request["bindings"]["returns"] = json!("Option<Box>");
    let request = domain(
        request,
        "/bindings/guards/0/when",
        &[json!(true), json!(false)],
    );
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    assert_eq!(
        plan["entitlement"]["dependency_profile"],
        "draft-derived-fragment-dependencies-v1"
    );
    assert_eq!(plan["family_checks"]["source_kernel"], "valid");
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":{"/bindings/guards/0/when":false}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
}

#[test]
fn derived_constraints_bind_names_and_refuse_stale_fill_without_claiming() {
    let fixture = Fixture::new();
    let request = domain(
        guard(),
        "/bindings/guards/0/when",
        &[json!(true), json!(false)],
    );
    let (_, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    let handle = plan["plan"].as_str().unwrap();
    let path = fixture.dir.join(".sley/names.json");
    // This fresh workspace had no names file when the plan was bound.
    sley_agent::names::NameMap::default().write(&path).unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":{"/bindings/guards/0/when":false}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_BINDING_STALE");
    assert!(!fixture.dir.join(".sley/residual/fills").exists());
}

#[test]
fn derived_constraint_analysis_refuses_bad_scope_and_shared_budget_exhaustion() {
    let fixture = Fixture::new();
    let mut request = correlated();
    request["scope"] = json!(["one", "two"]);
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_SCOPE");
    let head = fixture.workspace.read_head().unwrap();
    let names = fixture_names(&fixture, head.program());
    let mut budget = Budget::limited(Duration::from_secs(2), 1);
    let error = factored::analyze(
        &parse_request(&bytes(&correlated())).unwrap(),
        head.program(),
        &names,
        &mut budget,
    )
    .err()
    .unwrap();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
    no_artifacts(&fixture);
}

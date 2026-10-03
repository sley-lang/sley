use super::dependency_tests::{committed_functions, literal_function};
use super::*;

fn factored_request() -> Value {
    let request = per_target_request(json!({}));
    with_constraints(request)
}

fn with_constraints(mut request: Value) -> Value {
    let tables: Vec<_> = request["scope"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| {
            let path = format!("/bindings/values/{}", name.as_str().unwrap());
            json!({"fields":[path],"rows":[{&path:3},{&path:7}]})
        })
        .collect();
    request["choices"] = json!({"version":2,"contract":"author_closed_constraints","constraints":tables,"dependencies":[]});
    request
}

fn answers(plan: &Value, value: i64) -> serde_json::Map<String, Value> {
    plan["unresolved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| (field["path"].as_str().unwrap().to_owned(), json!(value)))
        .collect()
}

#[test]
fn factored_cli_plans_and_fills_twenty_sites_without_materializing_the_global_product() {
    let names: Vec<_> = (0..20).map(|index| format!("site{index}")).collect();
    let fixture = committed_functions(names.iter().map(|name| literal_function(name)).collect());
    let mut request = per_target_request(json!({}));
    request["scope"] = json!(
        names
            .iter()
            .map(|name| format!("{name}.entry.amount"))
            .collect::<Vec<_>>()
    );
    let request = with_constraints(request);
    let (code, plan) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 1, "{plan}");
    assert_eq!(plan["route"], "closed_author_constraints");
    assert_eq!(plan["entitlement"]["components"], 20);
    assert_eq!(plan["entitlement"]["description_count_decimal"], "1048576");
    assert_eq!(plan["entitlement"]["materialized_component_rows"], 40);
    assert_eq!(plan["entitlement"]["global_product_materialized"], false);
    assert_eq!(plan["unresolved"].as_array().unwrap().len(), 20);
    assert_eq!(
        plan["family_checks"]["inherited_widths"],
        "all_declared_values_passed"
    );
    assert_eq!(plan["family_checks"]["compiler"], "not_run");
    assert_eq!(
        plan["family_checks"]["compilation_policy"],
        "selected_completion_on_trial"
    );
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
    let handle = plan["plan"].as_str().unwrap();
    let (_, shown) = cli(&fixture.dir, &["residual", "show", handle, "--provenance"]);
    assert_eq!(shown["planning_resources"]["planned_fields"], 20);
    assert_eq!(
        shown["dependencies"]["components"]
            .as_array()
            .unwrap()
            .len(),
        20
    );
    let fill = json!({"residual":1,"plan":handle,"choose":answers(&plan,7)});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(result["edit"]["verification"], "passed");
    for name in names {
        let (code, ran) = cli(
            &fixture.dir,
            &[
                "call",
                &name,
                "5",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{ran}");
        assert_eq!(ran["result"], json!({"Ok":12}));
    }
    let (code, repeated) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 2, "{repeated}");
    assert!(!fixture.dir.join(".sley/drafts/d3").exists());
}

#[test]
fn factored_cli_preserves_exact_answer_and_derived_sources_for_correlated_sites() {
    let fixture = literal_fixture();
    let mut request = factored_request();
    request["choices"]["constraints"] = json!([{"fields":["/bindings/values/adjust.entry.amount","/bindings/values/other.entry.amount"],
        "rows":[{"/bindings/values/adjust.entry.amount":3,"/bindings/values/other.entry.amount":3},
        {"/bindings/values/adjust.entry.amount":7,"/bindings/values/other.entry.amount":9}]}]);
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    assert_eq!(plan["unresolved"].as_array().unwrap().len(), 1);
    let path = plan["unresolved"][0]["path"].as_str().unwrap();
    let choose = serde_json::Map::from_iter([(
        path.to_owned(),
        request["choices"]["constraints"][0]["rows"][1][path].clone(),
    )]);
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":choose});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
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
    let origins = proof["origins"].as_object().unwrap();
    assert_eq!(origins.len(), 2);
    for (field, origin) in origins {
        let expected = &proof["request"].pointer(field).unwrap();
        if origin["class"] == "AUTHORED" {
            assert_eq!(origin["document"], "residual-fill.json");
            assert_eq!(
                &fill.pointer(origin["pointer"].as_str().unwrap()).unwrap(),
                expected
            );
        } else {
            assert_eq!(origin["class"], "DERIVED");
            assert_eq!(origin["value_source"]["document"], "residual-plan.json");
            let original_plan = json!({"artifacts":{"residual-request.json":request}});
            assert_eq!(
                &original_plan
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
    for (name, value) in [("adjust", 12), ("other", 14)] {
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
        assert_eq!(ran["result"], json!({"Ok":value}));
    }
}

#[test]
fn factored_singletons_resolve_directly_and_preserve_noop_behavior() {
    for value in [3, 7] {
        let fixture = literal_fixture();
        let mut request = factored_request();
        for table in request["choices"]["constraints"].as_array_mut().unwrap() {
            let path = table["fields"][0].as_str().unwrap().to_owned();
            table["rows"] = json!([{path:value}]);
        }
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        let reference = if value == 3 {
            assert_eq!(result["no_change"], true);
            assert_eq!(result["kernel"], "not_run");
            assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
            result["residual"].as_str().unwrap()
        } else {
            assert_eq!(result["kernel"], "valid");
            result["draft"].as_str().unwrap()
        };
        let (_, shown) = cli(
            &fixture.dir,
            &["residual", "show", reference, "--provenance"],
        );
        for origin in shown["resolution"]["origins"].as_object().unwrap().values() {
            assert_eq!(origin["class"], "DERIVED");
            assert_eq!(origin["value_source"]["document"], "residual-request.json");
            assert_eq!(
                request
                    .pointer(origin["value_source"]["pointer"].as_str().unwrap())
                    .unwrap(),
                value
            );
        }
    }
}

#[test]
fn factored_bad_answers_do_not_consume_the_plan() {
    let fixture = literal_fixture();
    let request = factored_request();
    let (_, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    let handle = plan["plan"].as_str().unwrap();
    let mut bad = vec![json!({}), json!({"/bindings/value":7})];
    for value in [json!(true), json!("7"), json!(9)] {
        let mut choose = answers(&plan, 7);
        choose.insert("/bindings/values/adjust.entry.amount".into(), value);
        bad.push(json!(choose));
    }
    for choose in bad {
        let fill = json!({"residual":1,"plan":handle,"choose":choose});
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "fill", handle, &fill.to_string()],
        );
        assert_eq!(code, 2, "{result}");
        assert!(!fixture.dir.join(".sley/residual/fills").exists());
        assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    }
    let fill = json!({"residual":1,"plan":handle,"choose":answers(&plan,7)});
    assert_eq!(
        cli(
            &fixture.dir,
            &["residual", "fill", handle, &fill.to_string()]
        )
        .0,
        0
    );
}

#[test]
fn factored_invalid_and_contradictory_domains_refuse_without_plan_publication() {
    let fixture = literal_fixture();
    let mut cases = Vec::new();
    let mut fixed = factored_request();
    fixed["bindings"]["values"]["adjust.entry.amount"] = json!(128);
    fixed["choices"]["constraints"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    cases.push(fixed);
    let mut request = factored_request();
    request["choices"]["constraints"][0]["rows"][1]["/bindings/values/adjust.entry.amount"] =
        json!(128);
    cases.push(request);
    let mut request = factored_request();
    request["choices"]["constraints"].as_array_mut().unwrap().push(json!({"fields":["/bindings/values/adjust.entry.amount"],"rows":[{"/bindings/values/adjust.entry.amount":4}]}));
    cases.push(request);
    let mut request = factored_request();
    request["bindings"]["values"]["adjust.entry.amount"] = json!(3);
    cases.push(request);
    let mut request = factored_request();
    request["choices"]["dependencies"] =
        json!([{"kind":"binding","fields":["/bindings/values/adjust.entry.amount","unknown"]}]);
    cases.push(request);
    for request in cases {
        let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
        assert_eq!(code, 2, "{request}: {result}");
        assert!(!fixture.dir.join(".sley/residual").exists());
        assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    }
}

#[test]
fn factored_graph_coupling_enforces_component_limits_before_publication() {
    let names: Vec<_> = (0..9).map(|index| format!("site{index}")).collect();
    let mut functions: Vec<_> = names.iter().map(|name| literal_function(name)).collect();
    let ops: Vec<_> = names
        .iter()
        .map(|name| json!([name, "call", name, "x"]))
        .collect();
    functions.push(json!({"fn":"bridge","params":[["x","i8"]],"returns":"i8","blocks":[{"name":"entry","ops":ops,"term":["return","x"]}]}));
    let fixture = committed_functions(functions);
    let mut request = per_target_request(json!({}));
    request["scope"] = json!(
        names
            .iter()
            .map(|name| format!("{name}.entry.amount"))
            .collect::<Vec<_>>()
    );
    let request = with_constraints(request);
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_LIMIT");
    assert!(!fixture.dir.join(".sley/residual").exists());
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
}

#[test]
fn factored_public_checks_never_prune_the_author_domain() {
    let fixture = literal_fixture();
    let request = factored_request();
    let public = fixture.dir.join("public.json");
    fs::write(
        &public,
        json!([{"function":"adjust","args":[5],"expect":{"Ok":8}}]).to_string(),
    )
    .unwrap();
    let (code, plan) = cli(
        &fixture.dir,
        &[
            "residual",
            "try",
            &request.to_string(),
            "--public",
            public.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 1, "{plan}");
    assert_eq!(plan["entitlement"]["description_count_decimal"], "4");
    assert_eq!(plan["public_checks"], "not_run");
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":answers(&plan,7)});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 1, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(result["public_checks"], "failed");
}

#[test]
fn factored_fill_refuses_changed_names_and_changed_accepted_head() {
    let fixture = literal_fixture();
    let request = factored_request();
    let (_, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":answers(&plan,7)});
    let path = fixture.dir.join(".sley/names.json");
    let original = fs::read(&path).unwrap();
    let mut changed = original.clone();
    changed.push(b'\n');
    fs::write(&path, changed).unwrap();
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_BINDING_STALE");
    assert!(!fixture.dir.join(".sley/residual/fills").exists());
    fs::write(&path, original).unwrap();
    let (code, edit) = cli(
        &fixture.dir,
        &[
            "residual",
            "try",
            &literal_request(9).to_string(),
            "--no-test",
        ],
    );
    assert_eq!(code, 0, "{edit}");
    assert_eq!(
        cli(&fixture.dir, &["commit", edit["handle"].as_str().unwrap()]).0,
        0
    );
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_BINDING_STALE");
    assert!(!fixture.dir.join(".sley/residual/fills").exists());
    let (code, historical) = cli(&fixture.dir, &["residual", "show", handle, "--decisions"]);
    assert_eq!(code, 0, "{historical}");
    assert_eq!(historical["request"], request);
}

#[test]
fn factored_contract_schema_is_strict_before_workspace_access() {
    let fixture = Fixture::new();
    let mut invalid = Vec::new();
    let mut request = factored_request();
    request["choices"]["extra"] = json!(true);
    invalid.push(request);
    let mut request = factored_request();
    request["choices"]["contract"] = json!("sampled_candidates");
    invalid.push(request);
    let mut request = factored_request();
    request["choices"]["constraints"][0]["rows"] = json!([]);
    invalid.push(request);
    let mut request = factored_request();
    request["choices"]["constraints"][0]["rows"][0] = json!({});
    invalid.push(request);
    let mut request = factored_request();
    request["choices"]["constraints"][0]["rows"][1] =
        request["choices"]["constraints"][0]["rows"][0].clone();
    invalid.push(request);
    let mut request = factored_request();
    request["choices"]["fields"] = json!([]);
    invalid.push(request);
    for request in invalid {
        let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
        assert_eq!(code, 2, "{result}");
        assert!(!fixture.dir.join(".sley").exists());
    }
    let duplicate =
        factored_request()
            .to_string()
            .replacen("\"version\":2", "\"version\":2,\"version\":2", 1);
    assert_eq!(cli(&fixture.dir, &["residual", "plan", &duplicate]).0, 2);
    assert!(!fixture.dir.join(".sley").exists());
}

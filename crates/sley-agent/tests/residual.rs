//! Adversarial parsing and real-workspace binding checks for GHOSTWEAVE.

#[path = "residual/authority.rs"]
mod authority_tests;
#[path = "residual/compact_edit.rs"]
mod compact_edit_tests;
#[path = "residual/composition.rs"]
mod composition_tests;
#[path = "residual/dependencies.rs"]
mod dependency_tests;
#[path = "residual/derived_factors.rs"]
mod derived_factor_tests;
#[path = "residual/draft_authoring.rs"]
mod draft_authoring_tests;
#[path = "residual/draft_cli.rs"]
mod draft_cli_tests;
#[path = "residual/draft_edit.rs"]
mod draft_edit_tests;
#[path = "residual/draft_factored.rs"]
mod draft_factored_tests;
#[path = "residual/draft_inherited_constants.rs"]
mod draft_inherited_constant_tests;
#[path = "residual/draft_receipt.rs"]
mod draft_receipt_tests;
#[path = "residual/draft_replay.rs"]
mod draft_replay_tests;
#[path = "residual/draft_source.rs"]
mod draft_source_tests;
#[path = "residual/expansion_budget.rs"]
mod expansion_budget_tests;
#[path = "residual/factored.rs"]
mod factored_tests;
#[path = "residual/help.rs"]
mod help_tests;
#[path = "residual/interface_admission.rs"]
mod interface_admission_tests;
#[path = "residual/interface_arithmetic.rs"]
mod interface_arithmetic_tests;
#[path = "residual/interface_bindings.rs"]
mod interface_binding_tests;
#[path = "residual/interface_body_effects.rs"]
mod interface_body_effect_tests;
#[path = "residual/interface_calls.rs"]
mod interface_call_tests;
#[path = "residual/interface_cells.rs"]
mod interface_cell_tests;
#[path = "residual/interface_closure.rs"]
mod interface_closure_tests;
#[path = "residual/interface_comparisons.rs"]
mod interface_comparison_tests;
#[path = "residual/interface_constant_uses.rs"]
mod interface_constant_use_tests;
#[path = "residual/interface_continuation_indexes.rs"]
mod interface_continuation_index_tests;
#[path = "residual/interface_control_inputs.rs"]
mod interface_control_input_tests;
#[path = "residual/interface_declarations.rs"]
mod interface_declaration_tests;
#[path = "residual/interface_edit_effects.rs"]
mod interface_edit_effect_tests;
#[path = "residual/interface_effects.rs"]
mod interface_effect_tests;
#[path = "residual/interface_exits.rs"]
mod interface_exit_tests;
#[path = "residual/interface_expression_closure.rs"]
mod interface_expression_closure_tests;
#[path = "residual/interface_floats.rs"]
mod interface_float_tests;
#[path = "residual/interface_signatures.rs"]
mod interface_signature_tests;
#[path = "residual/interface_source_dominance.rs"]
mod interface_source_dominance_tests;

#[path = "residual/interface_source_availability.rs"]
mod interface_source_availability_tests;

#[path = "residual/interface_function_exits.rs"]
mod interface_function_exit_tests;
#[path = "residual/interface_generic_payloads.rs"]
mod interface_generic_payload_tests;
#[path = "residual/interface_hashing.rs"]
mod interface_hash_tests;
#[path = "residual/interface_key_traits.rs"]
mod interface_key_trait_tests;
#[path = "residual/interface_literals.rs"]
mod interface_literal_tests;
#[path = "residual/interface_maps.rs"]
mod interface_map_tests;
#[path = "residual/interface_named.rs"]
mod interface_named_tests;
#[path = "residual/interface_operation_indexes.rs"]
mod interface_operation_index_tests;
#[path = "residual/interface_order.rs"]
mod interface_order_tests;
#[path = "residual/interface_parameter_indexes.rs"]
mod interface_parameter_index_tests;
#[path = "residual/interface_patch_edges.rs"]
mod interface_patch_edge_tests;
#[path = "residual/interface_patch_effects.rs"]
mod interface_patch_effect_tests;
#[path = "residual/interface_pipeline_opcodes.rs"]
mod interface_pipeline_opcode_tests;
#[path = "residual/interface_references.rs"]
mod interface_reference_tests;
#[path = "residual/interface_regions.rs"]
mod interface_region_tests;
#[path = "residual/interface_routes.rs"]
mod interface_route_tests;
#[path = "residual/interface_source_edges.rs"]
mod interface_source_edge_tests;
#[path = "residual/interface_source_topology.rs"]
mod interface_source_topology_tests;
#[path = "residual/interface_terminals.rs"]
mod interface_terminal_tests;
#[path = "residual/interfaces.rs"]
mod interface_tests;
#[path = "residual/interface_tuples.rs"]
mod interface_tuple_tests;
#[path = "residual/interface_vectors.rs"]
mod interface_vector_tests;
#[path = "residual/isolation.rs"]
mod isolation_tests;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::residual::binding::{Binding, MAX_BOUND_ARTIFACT_BYTES, RuntimeIdentity};
use sley_agent::residual::{MAX_JSON_VALUES, MAX_REQUEST_BYTES, parse_request};
use sley_agent::workspace::Workspace;
use sley_agent::{genesis, hex};

struct Fixture {
    dir: PathBuf,
    workspace: Workspace,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ghostweave-binding-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        let workspace = genesis::init(&dir, Some([205; 32]), genesis::INIT_CEILINGS).unwrap();
        Self { dir, workspace }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn runtime() -> RuntimeIdentity {
    RuntimeIdentity::new(
        b"fragments-1",
        b"grammar-1",
        b"build-1",
        "epoch1-afx1",
        None,
        None,
    )
    .unwrap()
}

fn request(base: &str) -> Value {
    json!({
        "residual": 1, "base": base, "operation": "derive",
        "fragment": {"id": "ordered_guard_chain", "version": 1},
        "bindings": {}, "scope": ["checked"],
    })
}

fn planning_request() -> Value {
    let example = sley_agent::help::RESIDUAL
        .lines()
        .find(|line| line.starts_with("{\"residual\":"))
        .unwrap();
    serde_json::from_str(example).unwrap()
}

fn relation_request() -> Value {
    let mut request = planning_request();
    for key in ["params", "returns", "rounding"] {
        request["bindings"].as_object_mut().unwrap().remove(key);
    }
    request["choices"] = json!({"version":1,"contract":"author_closed_relation","rows":[
        {"/bindings/params":[["x","i8"]],"/bindings/returns":"Result<i8,ArithmeticError>","/bindings/rounding":"toward_zero"},
        {"/bindings/params":[["x","i64"]],"/bindings/returns":"Result<i64,ArithmeticError>","/bindings/rounding":"toward_zero"}
    ]});
    request
}

#[test]
fn residual_closed_relation_uses_a_frontier_and_preserves_checked_semantic_origins() {
    let fixture = Fixture::new();
    let request = relation_request();
    let (code, plan) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 1, "{plan}");
    assert_eq!(plan["route"], "closed_author_relation");
    assert_eq!(plan["family_checks"]["compiler"], "all_rows_passed");
    assert_eq!(plan["kernel"], "not_run");
    assert!(!fixture.dir.join(".sley/candidates").exists());
    let fields = plan["unresolved"].as_array().unwrap();
    assert_eq!(
        fields.len(),
        1,
        "three semantic fields, one distinguishing answer: {plan}"
    );
    let field = fields[0]["path"].as_str().unwrap();
    assert_ne!(
        field, "/bindings/rounding",
        "the constant is entailed by the explicit relation"
    );
    assert_eq!(fields[0]["supported_values"].as_array().unwrap().len(), 2);
    let handle = plan["plan"].as_str().unwrap();
    let answers = serde_json::Map::from_iter([(
        field.to_owned(),
        request["choices"]["rows"][0][field].clone(),
    )]);
    let (_, preview) = cli(&fixture.dir, &["residual", "show", handle, "--provenance"]);
    assert_eq!(
        preview["planning_resources"]["planned_fields"], 3,
        "one analysis per plan"
    );
    let fill = json!({"residual":1,"plan":handle,"choose":answers});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(result["resolution"]["task_correctness"], "not_established");
    let (_, ran) = cli(&fixture.dir, &["call", "scaled_half", "-5", "--on", "c1"]);
    assert_eq!(ran["result"], json!({"Ok":-7}));
    let (code, shown) = cli(&fixture.dir, &["residual", "show", "d1@r1", "--provenance"]);
    assert_eq!(code, 0, "{shown}");
    let proof = &shown["resolution"];
    assert_eq!(proof["selected_row"], 0);
    assert_eq!(proof["request"]["bindings"]["params"], json!([["x", "i8"]]));
    assert!(proof["request"].get("choices").is_none());
    let resources = &shown["planning_resources"];
    assert_eq!(resources["planned_fields"], 3, "one analysis per fill");
    assert_eq!(resources["work_limit"], 12_000_000);
    assert_eq!(resources["wall_limit_micros"], 2_000_000);
    assert!(resources["wall_micros"].as_u64().unwrap() < 2_000_000);
    assert_frontier_memory_usage(resources);
    assert!(
        !result
            .as_object()
            .unwrap()
            .contains_key("planning_resources"),
        "observations stay in opt-in inspection"
    );
    let origin = &proof["origins"][field];
    assert_eq!(origin["class"], "AUTHORED");
    assert_eq!(
        fill.pointer(origin["pointer"].as_str().unwrap()).unwrap(),
        &answers[field]
    );
    let source: Value = serde_json::from_slice(
        &fs::read(fixture.dir.join(".sley/drafts/d1/r1/residual-plan.json")).unwrap(),
    )
    .unwrap();
    for (path, origin) in proof["origins"].as_object().unwrap() {
        if path == field {
            continue;
        }
        assert_eq!(origin["class"], "DERIVED");
        assert_eq!(origin["rule"], "closed-author-relation-v1");
        assert_eq!(
            source
                .pointer(origin["value_source"]["pointer"].as_str().unwrap())
                .unwrap(),
            &request["choices"]["rows"][0][path]
        );
    }
    let entries = shown["provenance"]["entries"].as_object().unwrap();
    for path in ["/fns/0/params", "/fns/0/returns"] {
        assert!(
            entries[path]["class"] == "DERIVED"
                || entries[path]["document"] == "residual-fill.json",
            "{path}: {}",
            entries[path]
        );
    }
}

fn assert_frontier_memory_usage(resources: &Value) {
    assert_eq!(
        resources["memory_enforcement"],
        "frontier_scratch_and_retained_payload_reservations"
    );
    assert_eq!(resources["aggregate_memory_enforcement"], "not_implemented");
    // The relation is still owned by Inspection when this receipt is captured.
    let retained = resources["memory"]["reserved_bytes"].as_u64().unwrap();
    assert!(retained > 0);
    assert_eq!(resources["memory"]["limit_bytes"], 256 * 1024 * 1024);
    assert!(resources["memory"]["peak_reserved_bytes"].as_u64().unwrap() > retained);
}

#[test]
fn residual_singleton_requires_an_explicit_complete_author_contract() {
    let fixture = Fixture::new();
    let mut request = relation_request();
    request["choices"]["rows"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(result["resolution"]["frontier"]["fields"], json!([]));
    assert!(
        !fixture.dir.join(".sley/residual").exists(),
        "no question round for fully declared values"
    );
    let (_, shown) = cli(&fixture.dir, &["residual", "show", "d1@r1", "--provenance"]);
    let origin = &shown["resolution"]["origins"]["/bindings/rounding"];
    assert_eq!(origin["class"], "DERIVED");
    assert_eq!(origin["contract"]["document"], "residual-request.json");
    assert_eq!(
        request
            .pointer(origin["value_source"]["pointer"].as_str().unwrap())
            .unwrap(),
        "toward_zero"
    );
    request.as_object_mut().unwrap().remove("choices");
    let (code, result) = cli(&fixture.dir, &["residual", "try", &request.to_string()]);
    assert_eq!(code, 1, "{result}");
    assert_eq!(
        result["unresolved"].as_array().unwrap().len(),
        3,
        "no implicit singleton inference"
    );
}

#[test]
fn residual_closed_relation_refuses_bad_rows_without_pruning_or_recording_a_plan() {
    let original = relation_request();
    let mut cases = Vec::new();
    let mut value = original.clone();
    value["choices"]["contract"] = json!("sampled_candidates");
    cases.push(value);
    let mut value = original.clone();
    value["choices"].as_object_mut().unwrap().remove("contract");
    cases.push(value);
    let mut value = original.clone();
    value["choices"]["rows"][1]
        .as_object_mut()
        .unwrap()
        .remove("/bindings/rounding");
    cases.push(value);
    let mut value = original.clone();
    value["choices"]["rows"][1]["/bindings/result"] = json!(0);
    cases.push(value);
    let mut value = original.clone();
    value["choices"]["rows"][1]["/bindings/rounding"] = json!("floor");
    cases.push(value);
    let mut value = original.clone();
    value["choices"]["rows"][1] = value["choices"]["rows"][0].clone();
    cases.push(value);
    let mut value = original.clone();
    value["choices"]["rows"] = json!([]);
    cases.push(value);
    for request in cases {
        let fixture = Fixture::new();
        let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
        assert_eq!(code, 2, "{request}: {result}");
        assert!(!fixture.dir.join(".sley").exists());
    }
    // This row passes fragment grammar but cannot compile in the bound graph.
    // It must refuse the entire relation instead of optimizing the surviving row.
    let fixture = Fixture::new();
    let mut request = original;
    request["choices"]["rows"][1]["/bindings/returns"] = json!("Result<i64,MissingType>");
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert!(
        result["detail"]
            .as_str()
            .unwrap()
            .contains("/choices/rows/1")
    );
    assert!(!fixture.dir.join(".sley/residual").exists());
    assert!(!fixture.dir.join(".sley/candidates").exists());
}

#[test]
fn residual_relation_answers_and_pruned_replays_are_refused() {
    let fixture = Fixture::new();
    let request = relation_request();
    let original = Binding::capture(&fixture.workspace, &bytes(&request), &runtime()).unwrap();
    let mut pruned = request.clone();
    pruned["choices"]["rows"].as_array_mut().unwrap().pop();
    assert_eq!(
        original
            .recheck(&fixture.workspace, &bytes(&pruned), &runtime())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualBindingStale
    );
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    let handle = plan["plan"].as_str().unwrap();
    let field = plan["unresolved"][0]["path"].as_str().unwrap();
    for choose in [
        json!({}),
        json!({field:true}),
        json!({"/bindings/rounding":"toward_zero"}),
    ] {
        let fill = json!({"residual":1,"plan":handle,"choose":choose});
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "fill", handle, &fill.to_string()],
        );
        assert_eq!(code, 2, "{result}");
        assert!(!fixture.dir.join(".sley/residual/fills").exists());
    }
}

#[test]
fn residual_relation_edit_checks_every_width_and_retains_noop_resolution() {
    let fixture = literal_fixture();
    let mut request = literal_request(3);
    request["bindings"].as_object_mut().unwrap().remove("value");
    request["choices"] = json!({"version":1,"contract":"author_closed_relation","rows":[
        {"/bindings/value":3},{"/bindings/value":128}
    ]});
    let (code, rejected) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{rejected}");
    assert!(
        rejected["detail"]
            .as_str()
            .unwrap()
            .contains("/choices/rows/1")
    );
    assert!(!fixture.dir.join(".sley/residual").exists());
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());

    request["choices"]["rows"][1]["/bindings/value"] = json!(7);
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    assert_eq!(plan["family_checks"]["compiler"], "all_rows_passed");
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":{"/bindings/value":3}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["no_change"], true);
    assert_eq!(result["kernel"], "not_run");
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
    assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    let record = result["residual"].as_str().unwrap();
    let (code, shown) = cli(&fixture.dir, &["residual", "show", record, "--provenance"]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["resolution"]["selected_row"], 0);
    assert_eq!(shown["planning_resources"]["planned_fields"], 1);
    let origin = &shown["resolution"]["origins"]["/bindings/value"];
    assert_eq!(origin["class"], "AUTHORED");
    assert_eq!(origin["document"], "residual-fill.json");
    assert_eq!(
        fill.pointer(origin["pointer"].as_str().unwrap()).unwrap(),
        3
    );
}

#[test]
fn residual_relation_fill_layers_and_relocates_origins_on_the_exact_draft() {
    let fixture = Fixture::new();
    make_draft(&fixture);
    let mut request = relation_request();
    request["base"] = json!("d1@r1");
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
    let handle = plan["plan"].as_str().unwrap();
    let field = plan["unresolved"][0]["path"].as_str().unwrap();
    let answers = serde_json::Map::from_iter([(
        field.to_owned(),
        request["choices"]["rows"][1][field].clone(),
    )]);
    let fill = json!({"residual":1,"plan":handle,"choose":answers});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["draft"], "d1@r2");
    for (name, argument, expected) in [
        ("identity", "17", json!(17)),
        ("scaled_half", "-5", json!({"Ok":-7})),
    ] {
        let (code, ran) = cli(&fixture.dir, &["call", name, argument, "--on", "c2"]);
        assert_eq!(code, 0, "{ran}");
        assert_eq!(ran["result"], expected);
    }
    let (code, shown) = cli(&fixture.dir, &["residual", "show", "d1@r2", "--provenance"]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["resolution"]["selected_row"], 1);
    let entries = shown["provenance"]["entries"].as_object().unwrap();
    for path in ["/fns/1/params", "/fns/1/returns"] {
        assert!(
            entries[path]["class"] == "DERIVED"
                || entries[path]["document"] == "residual-fill.json",
            "{path}: {}",
            entries[path]
        );
    }
}

#[test]
fn residual_relation_is_not_pruned_by_public_case_results() {
    let fixture = Fixture::new();
    let request = relation_request();
    let path = fixture.dir.join("public.json");
    // i8 has intermediate overflow; i64 does not. The test must not silently
    // narrow the author's declared relation to the row that happens to pass.
    fs::write(
        &path,
        json!([{"function":"scaled_half","args":[100],"expect":{"Ok":150}}]).to_string(),
    )
    .unwrap();
    let (code, plan) = cli(
        &fixture.dir,
        &[
            "residual",
            "try",
            &request.to_string(),
            "--public",
            path.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 1, "{plan}");
    assert_eq!(plan["entitlement"]["rows"], 2);
    assert_eq!(plan["public_checks"], "not_run");
    let handle = plan["plan"].as_str().unwrap();
    let field = plan["unresolved"][0]["path"].as_str().unwrap();
    let answers = serde_json::Map::from_iter([(
        field.to_owned(),
        request["choices"]["rows"][0][field].clone(),
    )]);
    let fill = json!({"residual":1,"plan":handle,"choose":answers});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 1, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(result["public_checks"], "failed");
    let (_, shown) = cli(&fixture.dir, &["residual", "show", "d1@r1", "--provenance"]);
    assert_eq!(shown["resolution"]["selected_row"], 0);
}

fn planned_fixture() -> (Fixture, Value, Value) {
    let fixture = Fixture::new();
    let mut request = planning_request();
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("rounding");
    let (code, plan) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 1, "{plan}");
    (fixture, request, plan)
}

#[test]
fn residual_plan_preserves_singleton_policy_as_an_author_decision_and_fill_executes() {
    let (fixture, request, plan) = planned_fixture();
    assert_eq!(plan["construction"], "incomplete");
    assert_eq!(plan["kernel"], "not_run");
    assert_eq!(plan["public_checks"], "not_run");
    assert_eq!(plan["unresolved"].as_array().unwrap().len(), 1);
    assert_eq!(plan["unresolved"][0]["path"], "/bindings/rounding");
    assert_eq!(plan["unresolved"][0]["class"], "UNRESOLVED");
    assert_eq!(
        plan["unresolved"][0]["supported_values"],
        json!(["toward_zero"])
    );
    assert!(!fixture.dir.join(".sley/drafts").exists());
    assert!(!fixture.dir.join(".sley/candidates").exists());
    let handle = plan["plan"].as_str().unwrap();
    let (code, shown) = cli(&fixture.dir, &["residual", "show", handle, "--expanded"]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["available"], false);
    let (_, resources) = cli(&fixture.dir, &["residual", "show", handle, "--provenance"]);
    assert_eq!(
        resources["planning_resources"]["planning_route"],
        "explicit_fast_path"
    );
    assert_eq!(
        resources["planning_resources"]["wall_limit_micros"],
        250_000
    );
    let (code, shown) = cli(&fixture.dir, &["residual", "show", handle, "--decisions"]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["request"], request);
    let fill = json!({"residual":1,"plan":handle,"choose":{"/bindings/rounding":"toward_zero"}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(
        result["public_checks"], "not_run",
        "original try options must survive planning"
    );
    let (_, ran) = cli(&fixture.dir, &["call", "scaled_half", "-5", "--on", "c1"]);
    assert_eq!(ran["result"], json!({"Ok":-7}));
    let revision = fixture.dir.join(".sley/drafts/d1/r1");
    assert!(revision.join("residual-plan.json").is_file());
    let stored: Value =
        serde_json::from_slice(&fs::read(revision.join("residual-fill.json")).unwrap()).unwrap();
    assert_eq!(stored, fill);
    let budget: Value =
        serde_json::from_slice(&fs::read(revision.join("residual-budget.json")).unwrap()).unwrap();
    assert_eq!(budget["planning_route"], "explicit_fast_path");
    assert_eq!(budget["wall_limit_micros"], 250_000);
    let (code, repeated) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 2, "{repeated}");
    assert_eq!(repeated["error"], "AGENT_RESIDUAL_BINDING_STALE");
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
}

#[test]
fn residual_ready_plan_previews_without_validation_and_empty_fill_uses_the_same_expansion() {
    let fixture = Fixture::new();
    let request = planning_request();
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    assert_eq!(plan["construction"], "complete");
    assert_eq!(plan["kernel"], "not_run");
    assert_eq!(plan["unresolved"], json!([]));
    assert!(!fixture.dir.join(".sley/candidates").exists());
    let handle = plan["plan"].as_str().unwrap();
    let (_, preview) = cli(&fixture.dir, &["residual", "show", handle, "--expanded"]);
    assert_eq!(preview["expanded"], expand(&request).frame);
    let fill = json!({"residual":1,"plan":handle,"choose":{}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
}

#[test]
fn residual_plan_fill_retains_public_checks_and_preview_event_is_not_a_trial() {
    let fixture = Fixture::new();
    let path = fixture.dir.join("public.json");
    fs::write(
        &path,
        json!([{"function":"scaled_half","args":[-5],"expect":{"Ok":-7}}]).to_string(),
    )
    .unwrap();
    let mut request = planning_request();
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("rounding");
    let (code, plan) = cli(
        &fixture.dir,
        &[
            "residual",
            "try",
            &request.to_string(),
            "--public",
            path.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 1, "{plan}");
    let events = fs::read_to_string(fixture.dir.join(".sley/events.jsonl")).unwrap();
    let event: Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
    assert_eq!(event["residual"]["plan"], true);
    assert_eq!(event["residual"]["try"], false);
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":{"/bindings/rounding":"toward_zero"}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["public_checks"], "passed");
}

#[test]
fn residual_fill_strict_answers_cannot_overwrite_authored_fields_or_start_another_round() {
    let (fixture, _, plan) = planned_fixture();
    let handle = plan["plan"].as_str().unwrap();
    for choose in [
        json!({}),
        json!({"/bindings/rounding":true}),
        json!({"/bindings/rounding":1}),
        json!({"/bindings/rounding":"floor"}),
        json!({"/bindings/result":"scaled"}),
        json!({"/bindings/rounding":"toward_zero","/bindings/steps":[]}),
    ] {
        let fill = json!({"residual":1,"plan":handle,"choose":choose});
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "fill", handle, &fill.to_string()],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["kernel"], "not_run");
        assert!(!fixture.dir.join(".sley/residual/fills").exists());
    }
    let mut request = guarded_request();
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("success");
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":{"/bindings/success":{"fragment":{"id":"checked_pipeline","version":1},"bindings":{}}}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_CHOICE_MISSING");
    assert!(!fixture.dir.join(".sley/residual/fills").exists());
}

#[test]
fn residual_fill_refuses_changed_binding_but_historical_plan_remains_inspectable() {
    let (fixture, _, plan) = planned_fixture();
    fs::write(fixture.dir.join(".sley/names.json"), "{}").unwrap();
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":{"/bindings/rounding":"toward_zero"}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_BINDING_STALE");
    assert!(!fixture.dir.join(".sley/candidates").exists());
    assert!(!fixture.dir.join(".sley/residual/fills").exists());
    let (code, shown) = cli(&fixture.dir, &["residual", "show", handle]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["historical"], true);
}

#[test]
fn residual_concurrent_fills_allow_only_one_trial_for_a_current_base_plan() {
    let (fixture, _, plan) = planned_fixture();
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":{"/bindings/rounding":"toward_zero"}})
        .to_string();
    let barrier = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let run = || {
            barrier.wait();
            cli(&fixture.dir, &["residual", "fill", handle, &fill])
        };
        let first = scope.spawn(run);
        let second = scope.spawn(run);
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(
        results.iter().filter(|(code, _)| *code == 0).count(),
        1,
        "{results:?}"
    );
    let (_, loser) = results.iter().find(|(code, _)| *code != 0).unwrap();
    assert_eq!(loser["error"], "AGENT_RESIDUAL_BINDING_STALE");
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
}

#[test]
fn residual_plan_fill_layers_on_the_exact_source_draft_revision() {
    let fixture = Fixture::new();
    make_draft(&fixture);
    let mut request = planning_request();
    request["base"] = json!("d1@r1");
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("rounding");
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    assert!(!fixture.dir.join(".sley/drafts/d1/r2").exists());
    let handle = plan["plan"].as_str().unwrap();
    let fill = json!({"residual":1,"plan":handle,"choose":{"/bindings/rounding":"toward_zero"}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["draft"], "d1@r2");
    let (_, ran) = cli(&fixture.dir, &["call", "identity", "17", "--on", "c2"]);
    assert_eq!(ran["result"], 17);
}

#[test]
fn residual_decision_inventory_batches_nested_missing_fields_and_enforces_aggregate_bounds() {
    use sley_agent::residual::plan;
    let mut request = guarded_request();
    request["bindings"]["guards"][0]
        .as_object_mut()
        .unwrap()
        .remove("fail");
    request["bindings"]["success"]["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("rounding");
    let decisions = plan::decisions(&parse_request(&bytes(&request)).unwrap()).unwrap();
    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[0]["path"], "/bindings/guards/0/fail");
    assert_eq!(decisions[1]["path"], "/bindings/success/bindings/rounding");
    request["bindings"]["guards"] = json!(vec![json!({}); 33]);
    assert_eq!(
        plan::decisions(&parse_request(&bytes(&request)).unwrap())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    let fixture = Fixture::new();
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert!(!fixture.dir.join(".sley").exists());
}

#[test]
fn residual_missing_fields_round_trip_all_fragment_interfaces_without_implicit_values() {
    use sley_agent::residual::plan;
    let paths = [
        "/bindings/params",
        "/bindings/returns",
        "/bindings/guards",
        "/bindings/success",
        "/bindings/guards/0/when",
        "/bindings/guards/0/fail",
        "/bindings/success/bindings/steps",
        "/bindings/success/bindings/arithmetic_failure",
        "/bindings/success/bindings/rounding",
        "/bindings/success/bindings/result",
        "/bindings/steps",
        "/bindings/arithmetic_failure",
        "/bindings/rounding",
        "/bindings/result",
        "/bindings/input",
        "/bindings/cases",
        "/bindings/join",
        "/bindings/join/params",
        "/bindings/join/ops",
        "/bindings/join/term",
        "/bindings/cases/0/case",
        "/bindings/cases/0/payload",
        "/bindings/cases/0/ops",
        "/bindings/cases/0/values",
    ];
    for original in [guarded_request(), planning_request(), branch_request()] {
        let expected = expand(&original).frame;
        for path in paths {
            let Some(value) = original.pointer(path) else {
                continue;
            };
            let mut missing = original.clone();
            let (parent, key) = path.rsplit_once('/').unwrap();
            missing
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            let decisions = plan::decisions(&parse_request(&bytes(&missing)).unwrap()).unwrap();
            assert_eq!(decisions.len(), 1, "{path}: {decisions:?}");
            assert_eq!(decisions[0]["path"], path);
            let answers = serde_json::Map::from_iter([(path.to_owned(), value.clone())]);
            let filled = plan::fill(&missing, &answers).unwrap();
            assert_eq!(filled, original);
            assert_eq!(expand(&filled).frame, expected);
        }
    }
}

#[test]
fn residual_fill_duplicate_keys_and_wrong_plan_are_rejected_before_workspace_access() {
    let fixture = Fixture::new();
    for fill in [
        r#"{"residual":1,"plan":"r1@1","choose":{"/bindings/rounding":true,"/bindings/rounding":"toward_zero"}}"#,
        r#"{"residual":true,"plan":"r1@1","choose":{}}"#,
        r#"{"residual":1,"plan":"r2@1","choose":{}}"#,
        r#"{"residual":1,"plan":"r1@1","choose":{},"unknown":true}"#,
    ] {
        let (code, result) = cli(&fixture.dir, &["residual", "fill", "r1@1", fill]);
        assert_eq!(code, 2, "{result}");
        assert!(!fixture.dir.join(".sley").exists());
    }
    // A fill or request of another envelope version is refused as a version.
    let fill = r#"{"residual":true,"plan":"r1@1","choose":{}}"#;
    let (code, result) = cli(&fixture.dir, &["residual", "fill", "r1@1", fill]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_VERSION");
    let mut request = planning_request();
    request["residual"] = json!(2);
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_VERSION");
    assert!(!fixture.dir.join(".sley").exists());
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn cli(dir: &Path, args: &[&str]) -> (i32, Value) {
    let mut words = vec![
        "--workspace".into(),
        dir.display().to_string(),
        "--json".into(),
    ];
    words.extend(args.iter().map(|arg| (*arg).to_owned()));
    let mut out = Vec::new();
    let code = sley_agent::cli::run(&words, &mut out);
    let value = serde_json::from_slice(&out)
        .unwrap_or_else(|_| panic!("unexpected CLI output: {}", String::from_utf8_lossy(&out)));
    (code, value)
}

fn frame() -> Value {
    json!({"af1":1, "afx":1, "fns":[{
        "fn":"identity", "params":[["value","i64"]], "returns":"i64",
        "blocks":[{"name":"entry","ops":[],"term":["return","value"]}]
    }]})
}

fn literal_fixture() -> Fixture {
    let fixture = Fixture::new();
    let mut functions = Vec::new();
    for name in ["adjust", "other"] {
        functions.push(
            json!({"fn":name,"params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
            "blocks":[{"name":"entry","ops":[["amount","const",{"type":"i8","value":3}],
                ["sum","add","x","amount"]],"term":["return","sum"]}]}),
        );
    }
    for (name, callee) in [("caller", "adjust"), ("outer", "caller")] {
        functions.push(json!({"fn":name,"params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
            "blocks":[{"name":"entry","ops":[["result","call",callee,"x"]],"term":["return","result"]}]}));
    }
    let frame = json!({"af1":1,"fns":functions});
    let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    fixture
}

fn literal_request(value: i64) -> Value {
    json!({"residual":1,"base":"current","operation":"edit",
        "fragment":{"id":"checked_pipeline","version":1},
        "bindings":{"lens":"integer_literal","value":value},
        "scope":["adjust.entry.amount"],"preserve":{"outside_scope":true,"boundaries":true}})
}

fn per_target_request(values: Value) -> Value {
    let mut request = literal_request(3);
    request["scope"] = json!(["adjust.entry.amount", "other.entry.amount"]);
    request["bindings"] = json!({"lens":"integer_literal"});
    request["bindings"]["values"] = values;
    request
}

#[test]
fn residual_per_target_values_preserve_noop_sites_and_record_exact_origins() {
    use sley_agent::candidate::{self, Authority, Store};
    let fixture = literal_fixture();
    let before = fixture.workspace.read_head().unwrap();
    let names = fixture_names(&fixture, before.program());
    let changed = names.resolve("other.entry.amount").unwrap();
    let request = per_target_request(json!({"adjust.entry.amount":3,"other.entry.amount":9}));
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(result["edit"]["verification"], "passed");
    let stored = Store::open(&fixture.workspace)
        .unwrap()
        .load(result["handle"].as_str().unwrap())
        .unwrap();
    let validation =
        candidate::validate(&before, &Authority::of(&before).unwrap(), &stored).unwrap();
    let after = candidate::proposed_program(&before, &validation).unwrap();
    for object in before.program().objects() {
        let id = object.record().entity_id;
        if id != changed {
            assert_eq!(
                object.stored_bytes(),
                after.object(&id).unwrap().stored_bytes()
            );
        }
    }
    for (name, expected) in [("adjust", 8), ("other", 14), ("outer", 8)] {
        let (code, ran) = cli(
            &fixture.dir,
            &[
                "call",
                name,
                "5",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{ran}");
        assert_eq!(ran["result"], json!({"Ok":expected}));
    }
    let (code, shown) = cli(
        &fixture.dir,
        &[
            "residual",
            "show",
            result["draft"].as_str().unwrap(),
            "--provenance",
        ],
    );
    assert_eq!(code, 0, "{shown}");
    let origin = &shown["provenance"]["entries"]["/edit/0/with/1/value"];
    assert_eq!(origin["class"], "AUTHORED");
    assert_eq!(origin["pointer"], "/bindings/values/other.entry.amount");
    assert_eq!(
        request
            .pointer(origin["pointer"].as_str().unwrap())
            .unwrap(),
        9
    );
    let repeated = sley_agent::residual::edit::expand(
        &after,
        &names,
        &parse_request(&bytes(&request)).unwrap(),
    )
    .unwrap();
    assert!(repeated.contract.no_change());
    repeated.contract.verify(&after, &after).unwrap();
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before.transaction_id()
    );
}

#[test]
fn residual_per_target_plan_requires_every_missing_site_without_overwriting_authored_values() {
    let fixture = literal_fixture();
    let request = per_target_request(json!({"adjust.entry.amount":7}));
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    assert_eq!(plan["unresolved"].as_array().unwrap().len(), 1);
    assert_eq!(
        plan["unresolved"][0]["path"],
        "/bindings/values/other.entry.amount"
    );
    let handle = plan["plan"].as_str().unwrap();
    for choose in [
        json!({}),
        json!({"/bindings/values/adjust.entry.amount":8}),
        json!({"/bindings/values/other.entry.amount":128}),
    ] {
        let fill = json!({"residual":1,"plan":handle,"choose":choose});
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "fill", handle, &fill.to_string()],
        );
        assert_eq!(code, 2, "{result}");
        assert!(!fixture.dir.join(".sley/residual/fills").exists());
    }
    let fill =
        json!({"residual":1,"plan":handle,"choose":{"/bindings/values/other.entry.amount":9}});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (name, expected) in [("adjust", 12), ("other", 14)] {
        let (code, ran) = cli(
            &fixture.dir,
            &[
                "call",
                name,
                "5",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{ran}");
        assert_eq!(ran["result"], json!({"Ok":expected}));
    }
}

#[test]
fn residual_per_target_relation_reconstructs_values_and_checks_unselected_widths() {
    let fixture = literal_fixture();
    let mut request = per_target_request(json!({}));
    request["choices"] = json!({"version":1,"contract":"author_closed_relation","rows":[
        {"/bindings/values/adjust.entry.amount":3,"/bindings/values/other.entry.amount":3},
        {"/bindings/values/adjust.entry.amount":7,"/bindings/values/other.entry.amount":128}
    ]});
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert!(!fixture.dir.join(".sley/residual").exists());
    request["choices"]["rows"][1]["/bindings/values/other.entry.amount"] = json!(9);
    let (code, plan) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 0, "{plan}");
    assert_eq!(plan["unresolved"].as_array().unwrap().len(), 1);
    assert_eq!(plan["family_checks"]["compiler"], "all_rows_passed");
    let path = plan["unresolved"][0]["path"].as_str().unwrap();
    let handle = plan["plan"].as_str().unwrap();
    let choose = serde_json::Map::from_iter([(
        path.to_owned(),
        request["choices"]["rows"][0][path].clone(),
    )]);
    let fill = json!({"residual":1,"plan":handle,"choose":choose});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "fill", handle, &fill.to_string()],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["no_change"], true);
    assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
    let (_, shown) = cli(
        &fixture.dir,
        &[
            "residual",
            "show",
            result["residual"].as_str().unwrap(),
            "--provenance",
        ],
    );
    let origins = shown["resolution"]["origins"].as_object().unwrap();
    assert_eq!(origins.len(), 2);
    assert_eq!(
        origins
            .values()
            .filter(|origin| origin["class"] == "AUTHORED")
            .count(),
        1
    );
    assert_eq!(
        origins
            .values()
            .filter(|origin| origin["class"] == "DERIVED")
            .count(),
        1
    );
}

#[test]
fn residual_per_target_schema_refuses_mixed_modes_extraneous_sites_and_bad_types() {
    let fixture = literal_fixture();
    let mut cases = Vec::new();
    for values in [
        json!([]),
        json!(null),
        json!({"unknown.entry.amount":3}),
        json!({"adjust.entry.amount":true}),
        json!({"adjust.entry.amount":"3"}),
    ] {
        cases.push(per_target_request(values));
    }
    for (key, value) in [("value", json!(3)), ("overrides", json!({}))] {
        let mut request = per_target_request(json!({}));
        request["bindings"][key] = value;
        cases.push(request);
    }
    for scope in [
        json!([]),
        json!(["adjust.entry.amount", "adjust.entry.amount"]),
        json!([1]),
        json!(["bad/name"]),
    ] {
        let mut request = per_target_request(json!({}));
        request["scope"] = scope;
        cases.push(request);
    }
    for request in cases {
        let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
        assert_eq!(code, 2, "{request}: {result}");
        assert_eq!(result["kernel"], "not_run");
    }
    assert!(!fixture.dir.join(".sley/residual").exists());
    assert!(!fixture.dir.join(".sley/drafts/d2").exists());
}

#[test]
fn residual_per_target_inventory_is_bounded_and_round_trips_all_64_decisions() {
    use sley_agent::residual::plan;
    let mut request = per_target_request(json!({}));
    request["scope"] = json!(
        (0..64)
            .map(|index| format!("f{index}.entry.amount"))
            .collect::<Vec<_>>()
    );
    let decisions = plan::decisions(&parse_request(&bytes(&request)).unwrap()).unwrap();
    assert_eq!(decisions.len(), 64);
    let choose = decisions
        .iter()
        .enumerate()
        .map(|(index, field)| (field["path"].as_str().unwrap().to_owned(), json!(index)))
        .collect();
    let filled = plan::fill(&request, &choose).unwrap();
    assert!(
        plan::decisions(&parse_request(&bytes(&filled)).unwrap())
            .unwrap()
            .is_empty()
    );
    for (path, value) in &choose {
        assert_eq!(filled.pointer(path), Some(value));
    }
    request["scope"]
        .as_array_mut()
        .unwrap()
        .push(json!("f64.entry.amount"));
    assert_eq!(
        plan::decisions(&parse_request(&bytes(&request)).unwrap())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualLimit
    );
}

fn fixture_names(
    fixture: &Fixture,
    program: &sley_agent::workspace::Program,
) -> sley_agent::names::Names {
    let mut map = sley_agent::names::NameMap::read(&fixture.dir.join("names.json")).unwrap();
    map.extend(&sley_agent::names::NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap());
    sley_agent::names::Names::build(program, &map)
}

#[test]
fn residual_literal_edit_preserves_shared_constants_and_every_non_target_entity() {
    use sley_agent::candidate::{self, Authority, Store};
    use sley_mutate::value::EntityBodyValue;
    use sley_ssmc::Immediate;
    let fixture = literal_fixture();
    let before = fixture.workspace.read_head().unwrap();
    let names = fixture_names(&fixture, before.program());
    let target = names.resolve("adjust.entry.amount").unwrap();
    let other = names.resolve("other.entry.amount").unwrap();
    let constant = |id| match before.program().body(&id).unwrap() {
        EntityBodyValue::Operation(operation) => operation.immediate.clone(),
        _ => panic!("operation"),
    };
    assert_eq!(
        constant(target),
        constant(other),
        "fixture must share its constant"
    );
    let request = literal_request(7);
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(result["edit"]["verification"], "passed");
    let callers = result["edit"]["boundaries"]["possible_static_callers"]
        .as_array()
        .unwrap();
    assert!(
        callers.contains(&json!("caller")) && callers.contains(&json!("outer")),
        "{result}"
    );
    let stored = Store::open(&fixture.workspace)
        .unwrap()
        .load(result["handle"].as_str().unwrap())
        .unwrap();
    let validation =
        candidate::validate(&before, &Authority::of(&before).unwrap(), &stored).unwrap();
    let after = candidate::proposed_program(&before, &validation).unwrap();
    let repeated = sley_agent::residual::edit::expand(
        &after,
        &names,
        &parse_request(&bytes(&request)).unwrap(),
    )
    .unwrap();
    assert!(repeated.contract.no_change());
    assert_eq!(repeated.expansion.frame["edit"], json!([]));
    repeated.contract.verify(&after, &after).unwrap();
    for object in before.program().objects() {
        let id = object.record().entity_id;
        if id != target {
            assert_eq!(
                object.stored_bytes(),
                after.object(&id).unwrap().stored_bytes(),
                "{}",
                names.name(&id)
            );
        }
    }
    assert_ne!(
        before.program().object(&target).unwrap().stored_bytes(),
        after.object(&target).unwrap().stored_bytes()
    );
    let Immediate::Entity(old_constant) = constant(target) else {
        panic!("constant")
    };
    assert_eq!(
        before
            .program()
            .object(&old_constant)
            .unwrap()
            .stored_bytes(),
        after.object(&old_constant).unwrap().stored_bytes()
    );
    for (name, expected) in [("adjust", 12), ("other", 8), ("caller", 12), ("outer", 12)] {
        let (code, ran) = cli(
            &fixture.dir,
            &[
                "call",
                name,
                "5",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{ran}");
        assert_eq!(ran["result"], json!({"Ok":expected}), "{ran}");
    }
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before.transaction_id()
    );
}

#[test]
fn residual_literal_noop_is_inspectable_without_a_new_candidate_or_draft() {
    let fixture = literal_fixture();
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    let request = literal_request(3);
    let (code, result) = cli(&fixture.dir, &["residual", "try", &request.to_string()]);
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["no_change"], true);
    assert_eq!(result["construction"], "complete");
    assert_eq!(result["kernel"], "not_run");
    assert_eq!(result["public_checks"], "zero_ran");
    assert_eq!(result["residual"], "r1@1");
    assert!(!fixture.dir.join(".sley/drafts/d2").exists());
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
    let (code, shown) = cli(&fixture.dir, &["residual", "show", "r1@1", "--expanded"]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["expanded"]["edit"], json!([]));
    let (code, shown) = cli(&fixture.dir, &["residual", "show", "r1@1", "--decisions"]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["request"], request);
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
    let path = fixture.dir.join(".sley/residual/r1.json");
    let mut record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record["record"]["report"]["kernel"] = json!("valid");
    fs::write(path, record.to_string()).unwrap();
    let (code, shown) = cli(&fixture.dir, &["residual", "show", "r1@1"]);
    assert_eq!(code, 2, "{shown}");
    assert_eq!(shown["error"], "AGENT_RESIDUAL_BINDING_STALE");
}

#[test]
fn residual_literal_default_and_exceptions_are_typed_and_scope_is_exact() {
    let fixture = literal_fixture();
    let mut request = literal_request(7);
    request["scope"] = json!(["adjust.entry.amount", "other.entry.amount"]);
    request["bindings"]["overrides"] = json!({"other.entry.amount":9});
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    for (name, expected) in [("adjust", 12), ("other", 14)] {
        let (code, ran) = cli(
            &fixture.dir,
            &[
                "call",
                name,
                "5",
                "--on",
                result["handle"].as_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "{ran}");
        assert_eq!(ran["result"], json!({"Ok":expected}), "{ran}");
    }
    let mut invalid = Vec::new();
    for value in [json!(128), json!(-129), json!(true), json!("7")] {
        let mut request = literal_request(7);
        request["bindings"]["value"] = value;
        invalid.push(request);
    }
    for scope in [
        json!(["amount"]),
        json!(["adjust.entry.amount", "adjust.entry.amount"]),
        json!(["adjust.entry.sum"]),
    ] {
        let mut request = literal_request(7);
        request["scope"] = scope;
        invalid.push(request);
    }
    let mut request = literal_request(7);
    request["bindings"]["overrides"] = json!({"other.entry.amount":9});
    invalid.push(request);
    let mut request = literal_request(7);
    request["preserve"]["boundaries"] = json!(false);
    invalid.push(request);
    for request in invalid {
        let (code, result) = cli(&fixture.dir, &["residual", "try", &request.to_string()]);
        assert_eq!(code, 2, "{request}: {result}");
        assert_eq!(result["kernel"], "not_run");
    }
    assert!(!fixture.dir.join(".sley/drafts/d3").exists());
}

#[test]
fn residual_literal_concurrent_noops_publish_distinct_complete_records() {
    let fixture = literal_fixture();
    let request = literal_request(3).to_string();
    let barrier = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            barrier.wait();
            cli(&fixture.dir, &["residual", "try", &request, "--no-test"])
        });
        let second = scope.spawn(|| {
            barrier.wait();
            cli(&fixture.dir, &["residual", "try", &request, "--no-test"])
        });
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_ne!(results[0].1["residual"], results[1].1["residual"]);
    for (code, result) in results {
        assert_eq!(code, 0, "{result}");
        let (code, shown) = cli(
            &fixture.dir,
            &[
                "residual",
                "show",
                result["residual"].as_str().unwrap(),
                "--decisions",
            ],
        );
        assert_eq!(code, 0, "{shown}");
        assert_eq!(shown["request"].to_string(), request);
    }
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
}

#[test]
fn residual_literal_preservation_checks_reject_extra_or_missing_mutations() {
    use sley_agent::candidate::{self, Authority};
    use sley_agent::residual::edit;
    let fixture = literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let names = fixture_names(&fixture, head.program());
    let request = parse_request(&bytes(&literal_request(7))).unwrap();
    let edit = edit::expand(head.program(), &names, &request).unwrap();
    let authority = Authority::of(&head).unwrap();
    let nonce = candidate::fresh_nonce().unwrap();
    let compile = |frame: &Value| {
        sley_agent::frame::compile(
            head.program(),
            &names,
            &authority.ceilings,
            frame,
            nonce,
            &mut candidate::random32,
        )
        .unwrap()
    };
    let good = compile(&edit.expansion.frame);
    edit.contract.check(head.program(), &good).unwrap();
    let mut missing = good.clone();
    missing.ops.pop();
    assert_eq!(
        edit.contract
            .check(head.program(), &missing)
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualPreserve
    );
    let mut extra_frame = edit.expansion.frame.clone();
    extra_frame["edit"].as_array_mut().unwrap().push(
        json!({"fn":"other","replace_op":"entry.amount","with":["const",{"type":"i8","value":9}]}),
    );
    let extra = compile(&extra_frame);
    assert_eq!(
        edit.contract
            .check(head.program(), &extra)
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualPreserve
    );
    let candidate = candidate::assemble(&head, &authority, nonce, extra.ops).unwrap();
    let validation = candidate::validate(&head, &authority, &candidate.stored_bytes).unwrap();
    assert!(
        validation.is_valid(),
        "unrelated edits are kernel-valid but not authorized by this lens"
    );
    let after = candidate::proposed_program(&head, &validation).unwrap();
    assert_eq!(
        edit.contract
            .verify(head.program(), &after)
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualPreserve
    );
    let mut wrong_frame = edit.expansion.frame.clone();
    wrong_frame["edit"][0]["with"][1]["value"] = json!(9);
    assert_eq!(
        edit.contract
            .check(head.program(), &compile(&wrong_frame))
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualPreserve
    );
}

#[test]
fn residual_literal_noop_honors_requested_checks_and_keeps_failures() {
    let fixture = literal_fixture();
    let request = literal_request(3).to_string();
    let path = fixture.dir.join("public.json");
    for (argument, expect, skip, status, exit) in [
        (json!(5), json!({"Ok":8}), false, "passed", 0),
        (json!(5), json!({"Ok":9}), false, "failed", 1),
        (json!(5), json!({"Ok":9}), true, "not_run", 0),
        (json!("bad"), json!({"Ok":8}), false, "failed", 2),
    ] {
        fs::write(
            &path,
            json!([{"function":"adjust","args":[argument],"expect":expect}]).to_string(),
        )
        .unwrap();
        let mut args = vec![
            "residual",
            "try",
            &request,
            "--public",
            path.to_str().unwrap(),
        ];
        if skip {
            args.push("--no-test");
        }
        let (code, result) = cli(&fixture.dir, &args);
        assert_eq!(code, exit, "{result}");
        assert_eq!(result["public_checks"], status, "{result}");
        assert_eq!(result["kernel"], "not_run");
        assert_eq!(result["construction"], "complete");
        assert_eq!(result["no_change"], true);
        let (code, shown) = cli(
            &fixture.dir,
            &["residual", "show", result["residual"].as_str().unwrap()],
        );
        assert_eq!(code, 0, "{shown}");
        assert_eq!(shown["public_checks"], status);
    }
    assert!(!fixture.dir.join(".sley/candidates/c2.hex").exists());
}

fn make_draft(fixture: &Fixture) {
    let (code, result) = cli(&fixture.dir, &["try", &frame().to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    assert!(fixture.dir.join(".sley/drafts/d1/r1/status.json").is_file());
}

fn pipeline() -> Value {
    json!({
        "fragment":{"id":"checked_pipeline","version":1},
        "bindings":{
            "steps":[["scaled",["mul","x",3]],["value",["div","scaled","divisor"]]],
            "arithmetic_failure":"Arithmetic",
            "rounding":"toward_zero","result":"value"
        }
    })
}

fn guarded_request() -> Value {
    let mut value = request("current");
    value["bindings"] = json!({
        "params":[["x","i8"],["divisor","i8"]],
        "returns":"Result<i8,Error>",
        "guards":[
            {"when":["eq","x",0],"fail":["fail","ZeroInput"]},
            {"when":["eq","divisor",0],"fail":["fail","ZeroDivisor"]}
        ],
        "success":pipeline()
    });
    value
}

fn expand(value: &Value) -> sley_agent::residual::fragments::Expansion {
    sley_agent::residual::fragments::expand(&parse_request(&bytes(value)).unwrap()).unwrap()
}

#[test]
fn composed_guards_and_pipeline_preserve_error_priority_rounding_and_intermediate_overflow() {
    let fixture = Fixture::new();
    let mut expansion = expand(&guarded_request());
    // Type creation is explicit authoring, not an implicit fragment prerequisite.
    expansion.frame["types"] = json!([{
        "name":"Error","variant":["ZeroInput","ZeroDivisor","Arithmetic"]
    }]);
    let (code, output) = cli(
        &fixture.dir,
        &["try", &expansion.frame.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{output}");
    for (x, divisor, expected) in [
        (0, 0, json!({"Err":"ZeroInput"})),
        (100, 0, json!({"Err":"ZeroDivisor"})),
        (43, 3, json!({"Err":"Arithmetic"})),
        (-43, 3, json!({"Err":"Arithmetic"})),
        (5, 2, json!({"Ok":7})),
        (-5, 2, json!({"Ok":-7})),
        (5, -2, json!({"Ok":-7})),
        (-5, -2, json!({"Ok":7})),
    ] {
        let (code, output) = cli(
            &fixture.dir,
            &[
                "call",
                "checked",
                &x.to_string(),
                &divisor.to_string(),
                "--on",
                "c1",
            ],
        );
        assert_eq!(code, 0, "{output}");
        assert_eq!(
            output["result"], expected,
            "x={x}, divisor={divisor}: {output}"
        );
    }
}

#[test]
fn fragment_expansion_is_deterministic_and_records_decision_origins() {
    let value = guarded_request();
    let first = expand(&value);
    let second = expand(&value);
    assert_eq!(first.frame, second.frame);
    assert_eq!(first.provenance, second.provenance);
    assert_eq!(
        first.provenance["/fns/0/blocks/0/term/1"]["pointer"],
        "/bindings/guards/0/when"
    );
    assert_eq!(
        first.provenance["/fns/0/blocks/4/ops/0"]["class"],
        "DERIVED"
    );
    assert_eq!(first.provenance["/fns/0/params"]["class"], "AUTHORED");
    let mut colliding = value.clone();
    colliding["bindings"]["params"][0][0] = json!("gw_b0");
    let renamed = expand(&colliding);
    assert_eq!(renamed.frame["fns"][0]["entry"], "gwg_b0");
    assert_eq!(
        renamed.frame["fns"][0]["params"],
        colliding["bindings"]["params"]
    );
}

fn branch_request() -> Value {
    let mut value = request("current");
    value["fragment"]["id"] = json!("typed_branch_result");
    value["bindings"] = json!({
        "params":[["maybe","Option<i8>"]], "returns":"Result<i8,ArithmeticError>",
        "input":"maybe",
        "cases":[
            {"case":"Some", "payload":["x","i8"],
             "ops":[["negated","neg?","x"]], "values":["negated"]},
            {"case":"None", "payload":null, "ops":[], "values":[17]}
        ],
        "join":{"params":[["answer","i8"]], "ops":[], "term":["ok","answer"]}
    });
    value
}

#[test]
fn residual_cli_runs_one_shot_and_keeps_inspectable_atomic_artifacts() {
    let fixture = Fixture::new();
    let request = branch_request();
    let before = fixture.workspace.head().unwrap().transaction_id();
    let (code, result) = cli(&fixture.dir, &["residual", "try", &request.to_string()]);
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["construction"], "complete");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(result["public_checks"], "zero_ran");
    assert_eq!(result["native_admission"], "not_attempted");
    assert_eq!(result["draft"], "d1@r1");
    assert_eq!(result["body_omitted"], true);
    assert_eq!(fixture.workspace.head().unwrap().transaction_id(), before);
    assert!(!fixture.dir.join("final_candidate.hex").exists());
    let revision = fixture.dir.join(".sley/drafts/d1/r1");
    for name in [
        "residual-request.json",
        "residual-binding.json",
        "residual-expanded.json",
        "residual-provenance.json",
        "residual-source-map.json",
        "expanded.json",
        "sourcemap.json",
    ] {
        assert!(revision.join(name).is_file(), "{name}");
    }
    assert_eq!(
        fs::read(revision.join("input.txt")).unwrap(),
        request.to_string().as_bytes()
    );
    let (code, shown) = cli(&fixture.dir, &["residual", "show", "d1@r1", "--expanded"]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["representation"], "AF1");
    assert_eq!(shown["expanded"]["fns"][0]["fn"], "checked");
    let (code, shown) = cli(&fixture.dir, &["residual", "show", "d1@r1", "--decisions"]);
    assert_eq!(code, 0, "{shown}");
    assert_eq!(shown["request"], request);
    let (code, shown) = cli(&fixture.dir, &["residual", "show", "d1@r1", "--provenance"]);
    assert_eq!(code, 0, "{shown}");
    assert!(
        !shown["composed_source_map"]["entries"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let (code, called) = cli(
        &fixture.dir,
        &["call", "checked", r#"{"Some":5}"#, "--on", "c1"],
    );
    assert_eq!(code, 0, "{called}");
    assert_eq!(called["result"], json!({"Ok":-5}));
    let event: Value = serde_json::from_str(
        fs::read_to_string(fixture.dir.join(".sley/events.jsonl"))
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(event["cmd"], "residual");
    assert_eq!(
        event["residual"]["request_bytes"],
        request.to_string().len()
    );
    assert!(event["residual"]["expanded_bytes"].as_u64().unwrap() > 0);
    assert!(!event.to_string().contains("maybe"));
}

#[test]
fn residual_cli_strict_refusals_write_no_workspace_state() {
    for input in [
        r#"{"residual":1,"residual":1}"#.to_owned(),
        r#"{"residual":true}"#.to_owned(),
        r#"{"residual":1,"base":"current","operation":"derive","fragment":{"id":"checked_pipeline","version":1},"bindings":{"x":1.5},"scope":["f"]}"#.to_owned(),
        "{".repeat(MAX_REQUEST_BYTES + 1),
    ] {
        let fixture = Fixture::new();
        let (code, output) = cli(&fixture.dir, &["residual","try",&input]);
        assert_eq!(code, 2, "{output}");
        assert_eq!(output["kernel"], "not_run");
        assert!(!fixture.dir.join(".sley").exists());
    }
}

#[test]
fn residual_cli_distinguishes_skipped_passed_failed_and_unexecutable_checks() {
    for (expect, skip, expected_status, expected_code) in [
        (json!({"Ok":-5}), false, "passed", 0),
        (json!({"Ok":5}), false, "failed", 1),
        (json!({"Ok":5}), true, "not_run", 0),
        (json!("invalid-result-type"), false, "failed", 2),
    ] {
        let fixture = Fixture::new();
        let path = fixture.dir.join("public.json");
        fs::write(
            &path,
            json!([{
                "name":"sign", "function":"checked",
                "args":if expected_code == 2 {json!([{"Some":"bad"}])} else {json!([{"Some":5}])},
                "expect":expect
            }])
            .to_string(),
        )
        .unwrap();
        let request = branch_request().to_string();
        let path = path.to_str().unwrap();
        let mut args = vec!["residual", "try", &request, "--public", path];
        if skip {
            args.push("--no-test");
        }
        let (code, output) = cli(&fixture.dir, &args);
        assert_eq!(code, expected_code, "{output}");
        assert_eq!(output["public_checks"], expected_status, "{output}");
        assert_eq!(output["kernel"], "valid");
        assert_eq!(output["native_admission"], "not_attempted");
        if expected_code == 1 {
            assert_eq!(output["failures"][0]["case"]["expected"], json!({"Ok":5}));
        }
        if expected_code == 2 {
            assert!(output["public_refusal"].is_string());
        }
    }
}

#[test]
fn residual_cli_layers_exact_drafts_and_rejects_stale_revisions() {
    let fixture = Fixture::new();
    make_draft(&fixture);
    let mut request = branch_request();
    request["base"] = json!("d1@r1");
    let (code, output) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{output}");
    assert_eq!(output["draft"], "d1@r2");
    let dir = fixture.dir.join(".sley/drafts/d1/r2");
    let frame: Value = serde_json::from_slice(&fs::read(dir.join("frame.json")).unwrap()).unwrap();
    assert_eq!(frame["fns"][0]["fn"], "identity");
    assert_eq!(
        frame["fns"][0]["blocks"][0]["term"],
        json!(["return", "value"])
    );
    assert_eq!(frame["fns"][1]["fn"], "checked");
    let provenance: Value =
        serde_json::from_slice(&fs::read(dir.join("residual-provenance.json")).unwrap()).unwrap();
    assert_eq!(provenance["entries"]["/fns/1/fn"]["pointer"], "/scope/0");
    assert!(provenance["entries"].get("/fns/0/fn").is_none());
    let (code, output) = cli(&fixture.dir, &["residual", "try", &request.to_string()]);
    assert_eq!(code, 2, "{output}");
    assert_eq!(output["error"], "AGENT_RESIDUAL_BINDING_STALE");
    assert!(!fixture.dir.join(".sley/drafts/d1/r3").exists());
    assert!(!fixture.dir.join(".sley/candidates/c3.hex").exists());
    // An ordinary draft fill can still repair the retained AF1-X frame.
    let delta = json!({"set":[{"at":"/fns/1/blocks/1/term","value":["ok",3]}]});
    let (code, output) = cli(
        &fixture.dir,
        &[
            "fill",
            "d1",
            &delta.to_string(),
            "--revision",
            "2",
            "--no-test",
        ],
    );
    assert_eq!(code, 0, "{output}");
}

#[test]
fn residual_cli_preserves_compiler_obligations_and_kernel_refusal_symbols() {
    for problem in ["missing_case", "constant_type", "deferred_type"] {
        let fixture = Fixture::new();
        let mut value = branch_request();
        if problem == "missing_case" {
            value["bindings"]["cases"].as_array_mut().unwrap().pop();
        } else {
            value["bindings"]["cases"][0]["ops"] = if problem == "constant_type" {
                json!([["bad", "const", {"type":"text","value":"wrong"}]])
            } else {
                // The library retains ordinary expansion, while CLI readiness
                // now refuses unknown connections before creating a draft.
                json!([["bad", "cell_get", ["future_expression"]]])
            };
            value["bindings"]["cases"][0]["values"] = json!(["bad"]);
        }
        let (code, output) = cli(&fixture.dir, &["residual", "try", &value.to_string()]);
        assert_ne!(code, 0, "{output}");
        assert_eq!(output["public_checks"], "not_run");
        {
            // All three composition failures now precede generation.
            assert_eq!(output["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
            assert_eq!(output["construction"], "refused");
            assert_eq!(output["kernel"], "not_run");
            assert!(
                output["detail"]
                    .as_str()
                    .unwrap()
                    .contains("/bindings/cases"),
                "{output}"
            );
            assert!(!fixture.dir.join(".sley/drafts").exists());
            assert!(!fixture.dir.join(".sley/candidates").exists());
        }
        if problem != "deferred_type" {
            continue;
        }
        // Explicit ordinary authoring still exposes the same downstream
        // compiler obligations. The residual refusal does not erase them.
        let expansion = expand(&value);
        let (code, output) = cli(
            &fixture.dir,
            &["try", &expansion.frame.to_string(), "--no-test"],
        );
        assert_ne!(code, 0, "{output}");
        assert!(!output["obligations"].as_array().unwrap().is_empty());
        if output["verdict"]["valid"] == false {
            assert!(output["verdict"]["symbol"].is_string(), "{output}");
        }
        let (code, shown) = cli(&fixture.dir, &["draft", "d1@r1", "--frame"]);
        assert_eq!(code, 0, "{shown}");
        assert!(shown["frame"].is_object(), "{shown}");
    }
}

#[test]
fn concurrent_residual_followups_never_merge_or_publish_two_revisions() {
    let fixture = Fixture::new();
    make_draft(&fixture);
    let mut request = branch_request();
    request["base"] = json!("d1@r1");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let barrier = barrier.clone();
            let dir = fixture.dir.clone();
            let request = request.to_string();
            std::thread::spawn(move || {
                barrier.wait();
                cli(&dir, &["residual", "try", &request, "--no-test"])
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(
        results.iter().filter(|(code, _)| *code == 0).count(),
        1,
        "{results:?}"
    );
    let (_, refused) = results.iter().find(|(code, _)| *code != 0).unwrap();
    assert!(
        ["AGENT_RESIDUAL_BINDING_STALE", "AGENT_DRAFT_STALE"]
            .contains(&refused["error"].as_str().unwrap()),
        "{refused}"
    );
    assert!(fixture.dir.join(".sley/drafts/d1/r2/status.json").is_file());
    assert!(!fixture.dir.join(".sley/drafts/d1/r3").exists());
    assert!(!fixture.dir.join(".sley/candidates/c3.hex").exists());
}

#[test]
fn help_residual_example_executes_and_reports_signed_rounding() {
    let fixture = Fixture::new();
    let example = sley_agent::help::RESIDUAL
        .lines()
        .find(|line| line.starts_with("{\"residual\":"))
        .unwrap();
    let (code, result) = cli(&fixture.dir, &["residual", "try", example]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["call", "scaled_half", "-5", "--on", "c1"]);
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!({"Ok":-7}));
    let (_, overflow) = cli(
        &fixture.dir,
        &["call", "scaled_half", &i64::MAX.to_string(), "--on", "c1"],
    );
    assert_eq!(
        overflow["result"],
        json!({"Err":{"ArithmeticError":"Overflow"}})
    );
}

#[test]
fn residual_io_failure_does_not_erase_completed_kernel_validation() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.dir.join(".sley")).unwrap();
    // Make candidate storage fail after compilation and kernel validation.
    fs::write(fixture.dir.join(".sley/candidates"), b"occupied").unwrap();
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &branch_request().to_string()],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_IO_FAILED");
    assert_eq!(result["construction"], "complete");
    assert_eq!(result["kernel"], "valid");
    assert_eq!(result["public_checks"], "not_run");
    assert_eq!(result["native_admission"], "not_attempted");
    assert!(!fixture.dir.join(".sley/drafts/d1/r1").exists());
}

#[test]
fn native_arithmetic_propagation_is_an_explicit_typed_policy() {
    let example = sley_agent::help::RESIDUAL
        .lines()
        .find(|line| line.starts_with("{\"residual\":"))
        .unwrap();
    let mut request: Value = serde_json::from_str(example).unwrap();
    for policy in [
        json!({"propagate":1}),
        json!({"propagate":false}),
        json!({"propagate":true,"map":"Overflow"}),
    ] {
        request["bindings"]["arithmetic_failure"] = policy;
        let parsed = parse_request(&bytes(&request)).unwrap();
        assert_eq!(
            sley_agent::residual::fragments::expand(&parsed)
                .unwrap_err()
                .code(),
            AgentErrorCode::ResidualFragmentShape
        );
    }
}

#[test]
fn typed_branches_bind_payloads_join_values_and_preserve_checked_failures() {
    let fixture = Fixture::new();
    let expansion = expand(&branch_request());
    let (code, output) = cli(
        &fixture.dir,
        &["try", &expansion.frame.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{output}");
    for (argument, expected) in [
        (json!({"Some":5}), json!({"Ok":-5})),
        (json!("None"), json!({"Ok":17})),
        (
            json!({"Some":-128}),
            json!({"Err":{"ArithmeticError":"Overflow"}}),
        ),
    ] {
        let (code, output) = cli(
            &fixture.dir,
            &["call", "checked", &argument.to_string(), "--on", "c1"],
        );
        assert_eq!(code, 0, "{output}");
        assert_eq!(output["result"], expected, "{output}");
    }
    assert_eq!(
        expansion.provenance["/fns/0/blocks/2/params/0"]["pointer"],
        "/bindings/cases/0/payload"
    );
    assert_eq!(
        expansion.provenance["/fns/0/blocks/1/term"]["pointer"],
        "/bindings/join/term"
    );
}

#[test]
fn typed_branch_missing_cases_and_payload_type_conflicts_remain_obligations() {
    for missing in [true, false] {
        let fixture = Fixture::new();
        let mut value = branch_request();
        if missing {
            value["bindings"]["cases"].as_array_mut().unwrap().pop();
        } else {
            value["bindings"]["cases"][0]["payload"][1] = json!("text");
        }
        let expansion = expand(&value);
        let (code, output) = cli(
            &fixture.dir,
            &["try", &expansion.frame.to_string(), "--no-test"],
        );
        assert_ne!(code, 0, "{output}");
        let status: Value = serde_json::from_slice(
            &fs::read(fixture.dir.join(".sley/drafts/d1/r1/status.json")).unwrap(),
        )
        .unwrap();
        assert_ne!(status["state"], "valid", "{status}");
        assert!(
            !status["obligations"].as_array().unwrap().is_empty(),
            "{status}"
        );
        // The workbench may retain a kernel-refused candidate for diagnosis.
        // Neither a compiler obligation nor a kernel refusal may be submitted.
        let (code, output) = cli(&fixture.dir, &["submit", "d1", "--untested"]);
        assert_eq!(code, 2, "{output}");
        assert!(!fixture.dir.join("final_candidate.hex").exists());
    }
}

#[test]
fn nesting_limit_counts_fragment_applications_and_not_terminal_regions() {
    use sley_agent::residual::fragments::MAX_FRAGMENT_DEPTH;
    let mut value = guarded_request();
    let mut nested = json!({"ops":[], "term":["ok",0]});
    for _ in 1..MAX_FRAGMENT_DEPTH {
        nested = json!({"fragment":{"id":"ordered_guard_chain","version":1},
            "bindings":{"guards":[],"success":nested}});
    }
    value["bindings"]["success"] = nested;
    let expansion = expand(&value);
    assert_eq!(
        expansion.frame["fns"][0]["blocks"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
}

#[test]
fn lowered_source_locations_keep_residual_decisions_and_derivation_inputs() {
    use sley_agent::names::{NameMap, Names};
    fn check_leaves(
        value: &Value,
        pointer: &str,
        request: &Value,
        expansion: &sley_agent::residual::fragments::Expansion,
        map: &sley_agent::afx::SourceMap,
    ) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    check_leaves(
                        value,
                        &format!("{pointer}/{escaped}"),
                        request,
                        expansion,
                        map,
                    );
                }
            }
            Value::Array(array) => {
                for (index, value) in array.iter().enumerate() {
                    check_leaves(
                        value,
                        &format!("{pointer}/{index}"),
                        request,
                        expansion,
                        map,
                    );
                }
            }
            _ => {
                let origin = expansion
                    .lowered_origin(map, pointer)
                    .unwrap_or_else(|| panic!("missing origin at {pointer}"));
                check_origin(&origin, request, pointer);
            }
        }
    }
    let fixture = Fixture::new();
    let head = fixture.workspace.head().unwrap();
    let names = Names::build(head.program(), &NameMap::default());
    let request = branch_request();
    let expansion = expand(&request);
    let lowered = sley_agent::afx::expand(head.program(), &names, &expansion.frame).unwrap();
    assert!(lowered.obligations.is_empty(), "{:?}", lowered.obligations);
    check_leaves(
        &lowered.frame["fns"],
        "/fns",
        &request,
        &expansion,
        &lowered.map,
    );
    let pipeline = expand(&guarded_request());
    let origin = pipeline.origin("/fns/0/blocks/4/ops/0/2").unwrap();
    assert_eq!(origin["class"], "DERIVED");
    assert_eq!(origin["inputs"].as_array().unwrap().len(), 3);
    assert!(pipeline.origin("/fns/01/blocks").is_none());
    assert!(pipeline.origin("/types/0").is_none());
}

fn check_origin(origin: &Value, request: &Value, pointer: &str) {
    match origin["class"].as_str().unwrap() {
        "AUTHORED" => {
            let at = origin["pointer"].as_str().unwrap();
            assert!(
                request.pointer(at).is_some(),
                "{pointer} maps to missing {at}"
            );
        }
        "FRAGMENT_DEFINED" => {
            assert!(
                request
                    .pointer(origin["selection"].as_str().unwrap())
                    .is_some()
            );
        }
        "DERIVED" => {
            assert_eq!(origin["rule"], "af1-x-lowering-v1");
            for source in origin["origins"].as_array().unwrap() {
                check_origin(source, request, pointer);
            }
        }
        _ => panic!("unjustified origin: {origin}"),
    }
}

#[test]
fn fragment_refuses_missing_policies_unknown_shapes_and_budget_exhaustion() {
    use sley_agent::residual::fragments::{MAX_FRAGMENT_DEPTH, MAX_FRAGMENT_ITEMS};
    let base = guarded_request();
    let mut cases = Vec::new();
    let mut value = base.clone();
    value["bindings"]["success"]["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("rounding");
    cases.push((value, AgentErrorCode::ResidualChoiceMissing));
    let mut value = base.clone();
    value["bindings"]["success"]["bindings"]["rounding"] = json!("floor");
    cases.push((value, AgentErrorCode::ResidualConstraintConflict));
    let mut value = base.clone();
    value["bindings"]["success"]["bindings"]["steps"][0][1] = json!(["call", "guess", "x"]);
    cases.push((value, AgentErrorCode::ResidualFragmentShape));
    let mut value = base.clone();
    value["fragment"]["version"] = json!(2);
    cases.push((value, AgentErrorCode::ResidualFragmentUnknown));
    let mut value = base.clone();
    value["bindings"]["guards"] = json!(vec![
        json!({"when":true,"fail":["fail","ZeroInput"]});
        MAX_FRAGMENT_ITEMS + 1
    ]);
    cases.push((value, AgentErrorCode::ResidualLimit));
    let mut value = base.clone();
    let mut nested = pipeline();
    for _ in 0..MAX_FRAGMENT_DEPTH {
        nested = json!({"fragment":{"id":"ordered_guard_chain","version":1},
            "bindings":{"guards":[],"success":nested}});
    }
    value["bindings"]["success"] = nested;
    cases.push((value, AgentErrorCode::ResidualLimit));
    for (value, expected) in cases {
        let parsed = parse_request(&bytes(&value)).unwrap();
        let error = sley_agent::residual::fragments::expand(&parsed).unwrap_err();
        assert_eq!(error.code(), expected, "{error}");
    }
}

#[test]
fn strict_envelope_refuses_ambiguous_and_out_of_domain_input() {
    let valid = request("current");
    assert!(parse_request(&bytes(&valid)).is_ok());
    assert!(parse_request(&bytes(&request("d1@r2"))).is_ok());
    for base in ["d1", "d1@r0", "../d1@r1", "A".repeat(64).as_str()] {
        assert!(parse_request(&bytes(&request(base))).is_err(), "{base}");
    }
    for (key, value) in [
        ("residual", json!(true)),
        ("residual", json!(2)),
        ("residual", json!("1")),
        ("operation", json!("guess")),
        ("bindings", json!([])),
        ("scope", json!("all")),
        (
            "fragment",
            json!({"id":"ordered_guard_chain","version":true}),
        ),
        (
            "fragment",
            json!({"id":"ordered_guard_chain","version":4_294_967_296_u64}),
        ),
        (
            "fragment",
            json!({"id":"ordered_guard_chain","version":1,"unknown":0}),
        ),
        ("preserve", json!([])),
        ("unknown", json!(0)),
    ] {
        let mut value_request = valid.clone();
        value_request[key] = value;
        assert!(
            parse_request(&bytes(&value_request)).is_err(),
            "{value_request}"
        );
    }
    let mut edit = valid.clone();
    edit["operation"] = json!("edit");
    assert_eq!(
        parse_request(&bytes(&edit)).unwrap_err().code(),
        AgentErrorCode::ResidualPreserve
    );
    edit["preserve"] = json!([]);
    assert!(parse_request(&bytes(&edit)).is_ok());
}

#[test]
fn duplicates_are_rejected_after_unescaping_at_every_depth() {
    for invalid in [
        r#"{"residual":1,"residual":1}"#,
        r#"{"bindings":{"policy":1,"\u0070olicy":2}}"#,
        r#"{"bindings":[{"nested":{"policy":1,"policy":2}}]}"#,
    ] {
        let error = parse_request(invalid.as_bytes()).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualParse);
        assert!(
            error.detail().contains("duplicate object member"),
            "{error}"
        );
    }
}

#[test]
fn invalid_utf8_floats_and_parser_budgets_refuse() {
    assert!(parse_request(&[b'{', b'"', 0xff, b'"', b':', b'1', b'}']).is_err());
    for number in [
        "1.0",
        "1e0",
        "NaN",
        "Infinity",
        "18446744073709551616",
        "-9223372036854775809",
    ] {
        let text = format!(r#"{{"residual":{number}}}"#);
        assert!(parse_request(text.as_bytes()).is_err(), "{number}");
    }
    assert!(parse_request(&vec![b' '; MAX_REQUEST_BYTES + 1]).is_err());
    let nested = format!("{}0{}", "[".repeat(33), "]".repeat(33));
    assert!(
        parse_request(nested.as_bytes())
            .unwrap_err()
            .detail()
            .contains("nesting")
    );
    let many = format!("[{}]", vec!["0"; MAX_JSON_VALUES].join(","));
    assert!(
        parse_request(many.as_bytes())
            .unwrap_err()
            .detail()
            .contains("value limit")
    );
}

#[test]
fn canonical_binding_is_read_only_and_preserves_json_types() {
    let fixture = Fixture::new();
    let input = bytes(&request("current"));
    let original = Binding::capture(&fixture.workspace, &input, &runtime()).unwrap();
    let spaced = serde_json::to_vec_pretty(&request("current")).unwrap();
    assert_eq!(
        original,
        Binding::capture(&fixture.workspace, &spaced, &runtime()).unwrap()
    );
    original
        .recheck(&fixture.workspace, &input, &runtime())
        .unwrap();
    assert!(
        !fixture.dir.join(".sley").exists(),
        "capture must not create state"
    );
    let reversed = br#"{"scope":["checked"],"bindings":{},"fragment":{"version":1,"id":"ordered_guard_chain"},"operation":"derive","base":"current","residual":1}"#;
    assert_eq!(
        original,
        Binding::capture(&fixture.workspace, reversed, &runtime()).unwrap()
    );
    let mut boolean = request("current");
    boolean["bindings"]["decision"] = json!(true);
    let mut integer = boolean.clone();
    integer["bindings"]["decision"] = json!(1);
    let a = Binding::capture(&fixture.workspace, &bytes(&boolean), &runtime()).unwrap();
    let b = Binding::capture(&fixture.workspace, &bytes(&integer), &runtime()).unwrap();
    assert_ne!(a.digest(), b.digest());
    assert_eq!(
        original.to_json()["snapshot"]["workspace"]["policy_root"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
}

#[test]
fn runtime_changes_and_cross_workspace_replay_require_replanning() {
    let fixture = Fixture::new();
    let input = bytes(&request("current"));
    let original = Binding::capture(&fixture.workspace, &input, &runtime()).unwrap();
    for changed in [
        RuntimeIdentity::new(
            b"fragments-2",
            b"grammar-1",
            b"build-1",
            "epoch1-afx1",
            None,
            None,
        ),
        RuntimeIdentity::new(
            b"fragments-1",
            b"grammar-2",
            b"build-1",
            "epoch1-afx1",
            None,
            None,
        ),
        RuntimeIdentity::new(
            b"fragments-1",
            b"grammar-1",
            b"build-2",
            "epoch1-afx1",
            None,
            None,
        ),
        RuntimeIdentity::new(
            b"fragments-1",
            b"grammar-1",
            b"build-1",
            "epoch2",
            None,
            None,
        ),
        RuntimeIdentity::new(
            b"fragments-1",
            b"grammar-1",
            b"build-1",
            "epoch1-afx1",
            Some("cost2"),
            None,
        ),
        RuntimeIdentity::new(
            b"fragments-1",
            b"grammar-1",
            b"build-1",
            "epoch1-afx1",
            None,
            Some("tokenizer2"),
        ),
    ] {
        assert_eq!(
            original
                .recheck(&fixture.workspace, &input, &changed.unwrap())
                .unwrap_err()
                .code(),
            AgentErrorCode::ResidualBindingStale,
        );
    }
    let other = Fixture::new(); // Deliberately has the same semantic WorkspaceId.
    assert_eq!(
        fixture.workspace.read_head().unwrap().workspace(),
        other.workspace.read_head().unwrap().workspace()
    );
    assert_eq!(
        original
            .recheck(&other.workspace, &input, &runtime())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualBindingStale,
    );
    assert!(!other.dir.join(".sley").exists());
}

#[test]
fn explicit_root_and_accepted_head_changes_are_checked() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let input = bytes(&request(&hex::encode(head.state_root().root.as_bytes())));
    let original = Binding::capture(&fixture.workspace, &input, &runtime()).unwrap();
    assert_eq!(
        Binding::capture(
            &fixture.workspace,
            &bytes(&request(&"0".repeat(64))),
            &runtime()
        )
        .unwrap_err()
        .code(),
        AgentErrorCode::ResidualBindingStale,
    );
    make_draft(&fixture);
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    assert_ne!(
        head.transaction_id(),
        fixture.workspace.read_head().unwrap().transaction_id()
    );
    assert_eq!(
        original
            .recheck(&fixture.workspace, &input, &runtime())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualBindingStale
    );
}

#[test]
fn name_maps_are_bound_and_duplicate_local_members_refuse() {
    let fixture = Fixture::new();
    let input = bytes(&request("current"));
    let before = Binding::capture(&fixture.workspace, &input, &runtime()).unwrap();
    let path = fixture.dir.join("names.json");
    fs::write(&path, b"{}").unwrap();
    assert_eq!(
        before
            .recheck(&fixture.workspace, &input, &runtime())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualBindingStale
    );
    let id = "0".repeat(64);
    fs::write(&path, format!(r#"{{"{id}":"a","{id}":"b"}}"#)).unwrap();
    assert!(
        Binding::capture(&fixture.workspace, &input, &runtime())
            .unwrap_err()
            .detail()
            .contains("duplicate")
    );
    fs::write(&path, format!(r#"{{"{id}":"a"}}"#)).unwrap();
    let bound = Binding::capture(&fixture.workspace, &input, &runtime()).unwrap();
    fs::write(&path, format!(r#"{{"{id}":"b"}}"#)).unwrap();
    assert_eq!(
        bound
            .recheck(&fixture.workspace, &input, &runtime())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualBindingStale
    );
}

#[test]
fn draft_artifacts_and_new_revisions_invalidate_binding() {
    let fixture = Fixture::new();
    make_draft(&fixture);
    let input = bytes(&request("d1@r1"));
    let original = Binding::capture(&fixture.workspace, &input, &runtime()).unwrap();
    for relative in [
        ".sley/drafts/d1/r1/input.txt",
        ".sley/drafts/d1/r1/frame.json",
        ".sley/drafts/d1/r1/status.json",
        ".sley/candidates/c1.hex",
    ] {
        let path = fixture.dir.join(relative);
        let saved = fs::read(&path).unwrap();
        let mut mutated = saved.clone();
        if relative == ".sley/candidates/c1.hex" {
            mutated[0] = if mutated[0] == b'0' { b'1' } else { b'0' };
        } else {
            mutated.push(b' ');
        }
        fs::write(&path, mutated).unwrap();
        assert_eq!(
            original
                .recheck(&fixture.workspace, &input, &runtime())
                .unwrap_err()
                .code(),
            AgentErrorCode::ResidualBindingStale,
            "{relative}"
        );
        fs::write(&path, saved).unwrap();
        original
            .recheck(&fixture.workspace, &input, &runtime())
            .unwrap();
    }
    let (code, result) = cli(
        &fixture.dir,
        &["try", &frame().to_string(), "--on", "d1@r1", "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(
        original
            .recheck(&fixture.workspace, &input, &runtime())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualBindingStale
    );
    assert!(Binding::capture(&fixture.workspace, &bytes(&request("d1@r2")), &runtime()).is_ok());
}

#[test]
fn incomplete_authored_drafts_can_be_bound_for_repair() {
    let fixture = Fixture::new();
    let mut incomplete = frame();
    incomplete["fns"][0]["blocks"][0]["term"] = json!(["return", "missing"]);
    let (code, _) = cli(&fixture.dir, &["try", &incomplete.to_string(), "--no-test"]);
    assert_ne!(code, 0);
    let status: Value = serde_json::from_slice(
        &fs::read(fixture.dir.join(".sley/drafts/d1/r1/status.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(status["state"], "incomplete");
    let bound =
        Binding::capture(&fixture.workspace, &bytes(&request("d1@r1")), &runtime()).unwrap();
    assert!(bound.to_json()["snapshot"]["workspace"]["draft"]["candidate"].is_null());
}

#[test]
fn missing_head_does_not_trigger_seeding_and_large_artifacts_refuse() {
    let fixture = Fixture::new();
    let seed_only = fixture.dir.join("unseeded");
    fs::create_dir(&seed_only).unwrap();
    fs::copy(fixture.dir.join("base.pack"), seed_only.join("base.pack")).unwrap();
    let input = bytes(&request("current"));
    assert!(Binding::capture(&Workspace::at(&seed_only), &input, &runtime()).is_err());
    assert!(!seed_only.join("repo").exists());
    assert!(!seed_only.join(".sley").exists());
    let file = fs::File::create(fixture.dir.join("names.json")).unwrap();
    file.set_len(MAX_BOUND_ARTIFACT_BYTES as u64 + 1).unwrap();
    assert_eq!(
        Binding::capture(&fixture.workspace, &input, &runtime())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualLimit
    );
}

#[path = "residual/interface_source_pieces.rs"]
mod interface_source_pieces_tests;

#[path = "residual/interface_source_names.rs"]
mod interface_source_name_tests;

#[path = "residual/interface_source_reference_types.rs"]
mod interface_source_reference_type_tests;

#[path = "residual/interface_body_typing.rs"]
mod interface_body_typing_tests;

#[path = "residual/interface_nonconstant_uses.rs"]
mod interface_nonconstant_uses;

#[path = "residual/interface_readiness.rs"]
mod interface_readiness;

#[path = "residual/source_operation_types.rs"]
mod source_operation_types;

#[path = "residual/source_immediates.rs"]
mod source_immediates;

#[path = "residual/retained_operation_types.rs"]
mod retained_operation_types;

#[path = "residual/pipeline_literal_contexts.rs"]
mod pipeline_literal_contexts;

#[path = "residual/parameter_selector_types.rs"]
mod parameter_selector_types;

use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::AgentErrorCode;
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};

fn request(returns: &str, ops: Value, term: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!(returns);
    request["bindings"]["success"] = json!({"ops":[],"term":[]});
    request["bindings"]["success"]["ops"] = ops;
    request["bindings"]["success"]["term"] = term;
    request
}

fn connection<'a>(report: &'a Value, at: &str) -> &'a Value {
    report["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["at"] == at)
        .unwrap()
}

fn publish(fixture: &Fixture, request: &Value) {
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
}

#[test]
fn cell_aliases_preserve_read_write_order_and_execute_without_rewriting() {
    for (new, get, set) in [
        ("cell", "cell_get", "cell_set"),
        ("cell_new", "cell_get", "cell_set"),
        ("176", "177", "178"),
    ] {
        let fixture = Fixture::new();
        let request = request(
            "Tuple<i8,i8>",
            json!([
                ["c",new,"x"], ["before",get,"c"],
                {"name":"written","opcode":set,"operands":["c",7],"type":"unit"},
                ["after",get,"c"]
            ]),
            json!(["return", ["tuple", "before", "after"]]),
        );
        let before = bytes(&request);
        let report = check(&fixture, &request, &json!({})).unwrap();
        for index in 0..4 {
            let entry = connection(&report, &format!("/bindings/success/ops/{index}"));
            assert_eq!(entry["expression_types"], "connections_checked", "{report}");
            assert_eq!(entry["cells"][0]["ownership"], "ordinary_compiler");
            assert_eq!(entry["cells"][0]["evaluated"], false);
        }
        assert_eq!(
            connection(&report, "/bindings/success/ops/0")["cells"][0]["storage_eligibility"],
            "persistable_checked"
        );
        publish(&fixture, &request);
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", "-3", "true", "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!([-3, 7]));
        assert_eq!(bytes(&request), before);
    }
}

#[test]
fn cell_type_arity_storage_and_annotation_conflicts_refuse_before_publication() {
    for ops in [
        json!([["c", "cell"]]),
        json!([["c", "cell", "x", true]]),
        json!([["v", "cell_get", "x"]]),
        json!([["v", "cell_get", "x", true]]),
        json!([["c", "cell", "x"], ["w", "cell_set", "c", true]]),
        json!([["c", "cell", "x"], ["w", "cell_set", "c"]]),
        json!([["c", "cell", "x"], ["nested", "cell", "c"]]),
        json!([{ "name":"c","op":"cell","args":["x"],"type":"i8"}]),
        json!([{ "name":"c","op":"cell","args":[128],"type":"Cell<i8>"}]),
        json!([["c","cell","x"],{"name":"w","op":"cell_set","args":["c",0],"type":"bool"}]),
        json!([["c", "cell?", ["vec"]]]),
    ] {
        let fixture = Fixture::new();
        let request = request("i8", ops, json!(["return", "x"]));
        let before = fixture.workspace.read_head().unwrap().transaction_id();
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(
            result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
            "{result}"
        );
        assert!(
            result["detail"]
                .as_str()
                .unwrap()
                .contains("/bindings/success/ops")
        );
        assert_eq!(
            fixture.workspace.read_head().unwrap().transaction_id(),
            before
        );
        for artifact in ["drafts", "candidates", "residual"] {
            assert!(!fixture.dir.join(".sley").join(artifact).exists());
        }
    }
}

#[test]
fn checked_cell_reads_use_the_stored_option_and_preserve_native_none() {
    let fixture = Fixture::new();
    let mut request = request(
        "Option<i8>",
        json!([["c", "cell", "maybe"], ["v", "cell_get?", "c"]]),
        json!(["return", ["some", "v"]]),
    );
    request["bindings"]["params"] = json!([["maybe", "Option<i8>"]]);
    let report = check(&fixture, &request, &json!({})).unwrap();
    let entry = connection(&report, "/bindings/success/ops/1");
    assert_eq!(entry["propagation"][0]["failure_route"], "preserve_failure");
    publish(&fixture, &request);
    for input in [json!({"Some":5}), json!("None")] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", &input.to_string(), "--on", "c1"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], input);
    }
}

#[test]
fn unresolved_cell_operands_remain_partial_even_with_a_known_unit_result() {
    let fixture = Fixture::new();
    for (op, result) in [
        (json!(["c", "cell", 1]), Value::Null),
        (json!(["c", "cell_get", ["future_expression"]]), Value::Null),
        (
            json!(["c", "cell_set", ["future_expression"], true]),
            json!("unit"),
        ),
    ] {
        let request = request("i8", json!([op]), json!(["return", "x"]));
        let report = check(&fixture, &request, &json!({})).unwrap();
        let entry = connection(&report, "/bindings/success/ops/0");
        assert_eq!(entry["expression_types"], "partial");
        assert_eq!(entry["cells"][0]["result_type"], result);
    }
}

#[test]
fn cell_storage_checks_bound_named_definitions_and_defers_incomplete_shapes() {
    for accepted in [false, true] {
        let fixture = Fixture::new();
        let declarations = json!({"af1":1,"types":[{"name":"Pack","record":[["value","i8"]]}]});
        let (code, result) = cli(
            &fixture.dir,
            &["try", &declarations.to_string(), "--no-test"],
        );
        assert_eq!(code, 0, "{result}");
        if accepted {
            let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
            assert_eq!(code, 0, "{result}");
        }
        let mut request = request(
            "Pack",
            json!([["c", "cell", "pack"], ["v", "cell_get", "c"]]),
            json!(["return", "v"]),
        );
        request["bindings"]["params"] = json!([["pack", "Pack"]]);
        if !accepted {
            request["base"] = json!("d1@r1");
        }
        let report = check(
            &fixture,
            &request,
            &if accepted { json!({}) } else { declarations },
        )
        .unwrap();
        assert_eq!(
            connection(&report, "/bindings/success/ops/0")["cells"][0]["storage_eligibility"],
            "persistable_checked"
        );
        publish(&fixture, &request);
        let (code, result) = cli(
            &fixture.dir,
            &["call", "checked", "{\"value\":9}", "--on", "c2"],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!({"value":9}));
        let bad = json!({"types":[{"name":"Pack","record":[["value","Cell<i8>"]]}]});
        assert!(
            check(&fixture, &request, &bad)
                .unwrap_err()
                .detail()
                .contains("not persistable")
        );
        let incomplete = json!({"types":[{"name":"Pack","record":["value"]}]});
        let error = check(&fixture, &request, &incomplete).unwrap_err();
        super::interface_closure_tests::assert_incomplete(&error, "/bindings/params/");
    }
}

#[test]
fn annotated_cell_literals_use_exact_width_and_the_shared_budget() {
    let fixture = Fixture::new();
    let request = request(
        "i8",
        json!([
            {"name":"c","op":"cell","args":[-128],"type":"Cell<i8>"},
            ["v","cell_get","c"]
        ]),
        json!(["return", "v"]),
    );
    publish(&fixture, &request);
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "0", "false", "--on", "c1"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], -128);
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 2);
    let error = interfaces::check(
        head.program(),
        &names,
        &serde_json::Map::new(),
        &parse_request(&bytes(&request)).unwrap(),
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
}

#[test]
fn declared_execution_local_results_refuse_at_the_vm_boundary_before_expansion() {
    for returns in [
        "Cell<i8>",
        "Tuple<bool,Cell<i8>>",
        "Option<Cell<i8>>",
        "Vec<Cell<i8>>",
    ] {
        let fixture = Fixture::new();
        let request = request(returns, json!([]), json!(["return", ["cell", "x"]]));
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(error.detail().contains("/bindings/returns"));
        assert!(error.detail().contains("execution-local"));
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        for artifact in ["drafts", "candidates", "residual"] {
            assert!(!fixture.dir.join(".sley").join(artifact).exists());
        }
    }
}

#[test]
fn local_cells_cannot_be_operands_of_non_cell_operations() {
    for operation in [
        json!(["packed", "tuple", "c"]),
        json!(["packed", "tuple_new", "c"]),
        json!(["packed", "16", "c"]),
        json!(["packed", "vec", "c"]),
        json!(["packed", "some", "c"]),
        json!({"name":"packed","opcode":"result_ok","operands":["c"],"type":"Result<Cell<i8>,ArithmeticError>"}),
        json!(["packed", "tuple", ["cell", "x"]]),
        json!(["packed", "tuple", ["cell_get", "c"], "c"]),
    ] {
        let fixture = Fixture::new();
        let request = request(
            "i8",
            json!([["c", "cell", "x"], operation]),
            json!(["return", "x"]),
        );
        let before = bytes(&request);
        let error = check(&fixture, &request, &json!({})).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error.detail().contains("/bindings/success/ops/1"),
            "{error}"
        );
        assert!(error.detail().contains("local cell"), "{error}");
        let (code, result) = cli(
            &fixture.dir,
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 2, "{result}");
        assert_eq!(result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
        assert_eq!(bytes(&request), before);
        for artifact in ["drafts", "candidates", "residual"] {
            assert!(!fixture.dir.join(".sley").join(artifact).exists());
        }
    }
}

#[test]
fn contained_cells_and_cell_call_arguments_follow_the_vm_operand_rule() {
    let fixture = Fixture::new();
    let definitions = json!({"af1":1,"afx":1,"fns":[
        {"fn":"read_cell","params":[["c","Cell<i8>"]],"returns":"i8",
         "blocks":[{"name":"entry","ops":[["v","cell_get","c"]],"term":["return","v"]}]}
    ]});
    for (ty, operation, location) in [
        (
            "Tuple<i8,Cell<i8>>",
            json!(["v", "tuple_get", 0, "input"]),
            "/3",
        ),
        ("Vec<Cell<i8>>", json!(["v", "vec_len", "input"]), "/2"),
        ("Cell<i8>", json!(["v", "call", "read_cell", "input"]), "/3"),
    ] {
        let mut request = request("i8", json!([operation]), json!(["return", 0]));
        request["bindings"]["params"] = json!([["input", ty]]);
        let error = check(&fixture, &request, &definitions).unwrap_err();
        assert_eq!(error.code(), AgentErrorCode::ResidualConstraintConflict);
        assert!(
            error
                .detail()
                .contains(&format!("/bindings/success/ops/0{location}")),
            "{error}"
        );
        assert!(error.detail().contains("local cell"), "{error}");
    }
}

#[test]
fn nested_cell_reads_restore_the_outer_operand_rule_and_preserve_execution() {
    let fixture = Fixture::new();
    let request = request(
        "Tuple<i8,i8>",
        json!([
            ["c", "cell", "x"],
            ["v", "tuple", ["cell_get", "c"], ["cell_get", ["cell", "x"]]]
        ]),
        json!(["return", "v"]),
    );
    let report = check(&fixture, &request, &json!({})).unwrap();
    let evidence = connection(&report, "/bindings/success/ops/1")["cell_operands"]
        .as_array()
        .unwrap();
    for (at, status, opcode) in [
        ("/bindings/success/ops/1/2/1", "cell_operation_exempt", 177),
        ("/bindings/success/ops/1/2", "absent_checked", 16),
        ("/bindings/success/ops/1/3/1/1", "absent_checked", 176),
        ("/bindings/success/ops/1/3/1", "cell_operation_exempt", 177),
        ("/bindings/success/ops/1/3", "absent_checked", 16),
    ] {
        let found = evidence.iter().find(|entry| entry["at"] == at).unwrap();
        assert_eq!(found["local_cell_operand"], status);
        assert_eq!(found["opcode"], opcode);
    }
    publish(&fixture, &request);
    let (code, result) = cli(
        &fixture.dir,
        &["call", "checked", "7", "true", "--on", "c1"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"], json!([7, 7]));
}

#[test]
fn unresolved_operands_do_not_claim_local_cell_absence() {
    let fixture = Fixture::new();
    let request = request(
        "i8",
        json!([["v", "tuple", ["future_expression"]]]),
        json!(["return", "x"]),
    );
    let report = check(&fixture, &request, &json!({})).unwrap();
    let entry = connection(&report, "/bindings/success/ops/0");
    assert_eq!(entry["expression_types"], "partial");
    assert_eq!(
        entry["cell_operands"][0]["local_cell_operand"],
        "type_deferred"
    );
    assert_eq!(entry["cell_operands"][0]["at"], "/bindings/success/ops/0/2");
}

#[test]
fn local_cell_operand_refusals_match_the_canonical_vm_judgment() {
    use sley_check::TypeEnvironment;
    use sley_id::EntityId;
    use sley_ssmc::{Immediate, IntegerWidth, Opcode, TypeExpr};
    use sley_vm::extended::{LoweringContext, judge_extended_operation};

    let types = TypeEnvironment::new(vec![]).unwrap();
    let context = LoweringContext {
        types: &types,
        constants: &[],
        globals: &[],
        functions: &[],
        parameters: &[],
        contracts: &[],
        adapters: &[],
        function: EntityId::from_bytes([1; 32]),
    };
    let scalar = TypeExpr::SInt(IntegerWidth::from_bits(8));
    let cell = TypeExpr::LocalCell(Box::new(scalar.clone()));
    for (opcode, ordinary_result, cell_result) in [
        (
            Opcode::TupleNew,
            TypeExpr::Tuple(vec![scalar.clone()]),
            TypeExpr::Tuple(vec![cell.clone()]),
        ),
        (
            Opcode::VectorNew,
            TypeExpr::Vector(Box::new(scalar.clone())),
            TypeExpr::Vector(Box::new(cell.clone())),
        ),
        (
            Opcode::OptionSome,
            TypeExpr::Option(Box::new(scalar.clone())),
            TypeExpr::Option(Box::new(cell.clone())),
        ),
        (
            Opcode::ResultOk,
            TypeExpr::Result {
                ok: Box::new(scalar.clone()),
                error: Box::new(TypeExpr::Bool),
            },
            TypeExpr::Result {
                ok: Box::new(cell.clone()),
                error: Box::new(TypeExpr::Bool),
            },
        ),
    ] {
        assert_eq!(
            judge_extended_operation(
                &context,
                opcode,
                &Immediate::None,
                &[&scalar],
                std::slice::from_ref(&ordinary_result)
            )
            .unwrap(),
            ordinary_result
        );
        let error =
            judge_extended_operation(&context, opcode, &Immediate::None, &[&cell], &[cell_result])
                .unwrap_err();
        assert_eq!(error.code(), sley_vm::LowerErrorCode::SignatureMismatch);
    }
    assert_eq!(
        judge_extended_operation(
            &context,
            Opcode::CellGet,
            &Immediate::None,
            &[&cell],
            std::slice::from_ref(&scalar)
        )
        .unwrap(),
        scalar
    );
    assert_eq!(
        judge_extended_operation(
            &context,
            Opcode::CellSet,
            &Immediate::None,
            &[&cell, &scalar],
            &[TypeExpr::Unit]
        )
        .unwrap(),
        TypeExpr::Unit
    );
}

use super::interface_tests::{check, predicate_request};
use super::{Fixture, bytes, cli};
use serde_json::{Value, json};
use sley_agent::residual::{frontier::Budget, interfaces, parse_request};
use sley_agent::{
    AgentErrorCode,
    candidate::{self, Authority, PlannedOp},
    names::{NameMap, Names},
};
use sley_mutate::{
    MutationPayload,
    value::{ContractBody, EntityBodyValue, EntityIdSet},
};
use sley_ssmc::{ContractBinding, ContractKind, ContractSource};

fn fixture() -> Fixture {
    let fixture = Fixture::new();
    let frame = json!({"af1":1,"fns":[
        {"fn":"predicate","params":[["flag","bool"]],"returns":"bool",
         "blocks":[{"name":"entry","ops":[],"term":["return","flag"]}]},
        {"fn":"checked","params":[["x","i8"],["ready","bool"]],"returns":"bool",
         "blocks":[{"name":"entry","ops":[],"term":["return","ready"]}]}
    ]});
    let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    add_contract(&fixture);
    fixture
}

fn add_contract(fixture: &Fixture) {
    // The CLI frame dialect does not author Contract entities. Create this
    // fixture through the public candidate API and validate its actual graph.
    let head = fixture.workspace.read_head().unwrap();
    let path = fixture.dir.join(".sley/names.json");
    let mut map = NameMap::read(&path).unwrap();
    let names = Names::build(head.program(), &map);
    let target = names.resolve("checked").unwrap();
    let EntityBodyValue::Function(mut body) = head.program().body(&target).unwrap().clone() else {
        panic!("function")
    };
    let nonce = candidate::fresh_nonce().unwrap();
    let id = candidate::created_id(head.program(), nonce, 13, 0);
    body.contracts = EntityIdSet::from_unsorted(vec![id]).unwrap();
    let contract = ContractBody {
        target,
        contract_kind: ContractKind::Precondition,
        predicate: names.resolve("predicate").unwrap(),
        bindings: vec![ContractBinding {
            predicate_parameter: 0,
            source: ContractSource::Parameter(names.resolve("checked.ready").unwrap()),
        }],
        resource_limits: None,
    };
    let authority = Authority::of(&head).unwrap();
    let candidate = candidate::assemble(
        &head,
        &authority,
        nonce,
        vec![
            PlannedOp {
                kind: 13,
                target: id,
                field_tag: None,
                payload: MutationPayload::CreateEntity(EntityBodyValue::Contract(contract)),
            },
            PlannedOp {
                kind: 5,
                target,
                field_tag: None,
                payload: MutationPayload::ReplaceEntityVersion(EntityBodyValue::Function(body)),
            },
        ],
    )
    .unwrap();
    let checked = candidate::validate(&head, &authority, &candidate.stored_bytes).unwrap();
    assert!(checked.is_valid(), "{checked:?}");
    sley_agent::genesis::commit(&head, &fixture.workspace.repo(), &candidate.stored_bytes).unwrap();
    map.insert(*id.as_bytes(), "condition");
    map.write(&path).unwrap();
}

fn request(expression: Value) -> Value {
    let mut request = predicate_request(json!(false));
    request["bindings"]["guards"] = json!([]);
    request["bindings"]["returns"] = json!("Result<unit,ContractViolation>");
    request["bindings"]["success"] = json!({"ops":[],"term":["return",null]});
    request["bindings"]["success"]["term"][1] = expression;
    request
}

#[test]
fn excluded_opcode_aliases_and_statement_forms_refuse_before_expansion() {
    for words in [
        ["assert", "contract_assert", "144"],
        ["observe", "test_observe", "145"],
        ["effect", "effect_request", "160"],
        ["adapter", "adapter_invoke", "161"],
        ["narrow", "capability_narrow", "162"],
    ] {
        for word in words {
            for checked in [false, true] {
                let fixture = Fixture::new();
                let word = format!("{word}{}", if checked { "?" } else { "" });
                for object in [false, true] {
                    let mut request = request(json!(["future_expression"]));
                    request["bindings"]["success"]["ops"] = if object {
                        json!([{"name":"out","opcode":word,"operands":["unresolved_immediate",true]}])
                    } else {
                        json!([["out", word, "unresolved_immediate", true]])
                    };
                    let before = bytes(&request);
                    let (code, result) = cli(
                        &fixture.dir,
                        &["residual", "try", &request.to_string(), "--no-test"],
                    );
                    assert_eq!(code, 2, "{result}");
                    assert_eq!(result["error"], "AGENT_RESIDUAL_FRAGMENT_SHAPE");
                    assert_eq!(result["kernel"], "not_run");
                    let detail = result["detail"].as_str().unwrap();
                    assert!(detail.contains("/bindings/success/ops/0"), "{result}");
                    assert!(detail.contains("CANDIDATE_OPERATION_ANALYSIS_UNSUPPORTED"));
                    assert_eq!(bytes(&request), before);
                }
                for path in [".sley/candidates", ".sley/drafts", ".sley/residual"] {
                    assert!(!fixture.dir.join(path).exists());
                }
            }
        }
    }
}

#[test]
fn excluded_nested_guard_and_return_expressions_are_not_hidden_by_context() {
    let fixture = Fixture::new();
    for word in ["assert", "observe", "effect", "adapter", "narrow"] {
        for guard in [false, true] {
            let request = if guard {
                predicate_request(json!([
                    "and",
                    true,
                    [word, "unresolved_immediate", "ready"]
                ]))
            } else {
                request(json!(["hash", [word, "unresolved_immediate", "ready"]]))
            };
            let error = check(&fixture, &request, &json!({})).unwrap_err();
            assert_eq!(
                error.code(),
                AgentErrorCode::ResidualFragmentShape,
                "{error}"
            );
            assert!(
                error.detail().contains(if guard {
                    "/bindings/guards/0/when/2"
                } else {
                    "/bindings/success/term/1/1"
                }),
                "{error}"
            );
        }
    }
}

#[test]
fn well_typed_contract_is_refused_early_and_ordinary_authoring_preserves_kernel_gate() {
    let fixture = fixture();
    let request = request(json!(["assert", "condition", "ready"]));
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_FRAGMENT_SHAPE");
    assert_eq!(result["kernel"], "not_run");
    for path in [
        ".sley/candidates/c2.hex",
        ".sley/drafts/d2",
        ".sley/residual",
    ] {
        assert!(!fixture.dir.join(path).exists());
    }
    // Control: this well-typed assertion passes contract checking through the
    // unchanged ordinary path, then fails the actual kernel resource phase.
    let frame = json!({"af1":1,"afx":1,"fns":[{
        "fn":"checked","params":[["x","i8"],["ready","bool"]],
        "returns":"Result<unit,ContractViolation>",
        "blocks":[{"name":"entry","ops":[["out","assert","condition","ready"]],"term":["return","out"]}]
    }]});
    let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_ne!(code, 0, "{result}");
    assert_eq!(result["state"], "refused", "{result}");
    assert_eq!(result["verdict"]["valid"], false);
    assert_eq!(result["verdict"]["phase"], 12);
    assert_eq!(
        result["verdict"]["symbol"],
        "CANDIDATE_OPERATION_ANALYSIS_UNSUPPORTED"
    );
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
}

#[test]
fn closed_relation_cannot_publish_a_question_for_an_excluded_operation() {
    let fixture = Fixture::new();
    let mut request = request(json!(["assert", "unresolved_immediate", "ready"]));
    request["bindings"]
        .as_object_mut()
        .unwrap()
        .remove("returns");
    request["choices"] = json!({"version":1,"contract":"author_closed_relation","rows":[
        {"/bindings/returns":"Result<unit,ContractViolation>"},
        {"/bindings/returns":"Option<i8>"}
    ]});
    let (code, result) = cli(&fixture.dir, &["residual", "plan", &request.to_string()]);
    assert_eq!(code, 2, "{result}");
    assert_eq!(result["error"], "AGENT_RESIDUAL_FRAGMENT_SHAPE", "{result}");
    for path in [".sley/candidates", ".sley/drafts", ".sley/residual"] {
        assert!(!fixture.dir.join(path).exists());
    }
}

#[test]
fn admission_checks_do_not_restart_an_exhausted_budget() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(
        head.program(),
        &NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap(),
    );
    let request = parse_request(&bytes(&request(json!(["assert", "condition", "ready"])))).unwrap();
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 0);
    let error = interfaces::check(
        head.program(),
        &names,
        &serde_json::Map::new(),
        &request,
        &mut budget,
    )
    .unwrap_err();
    assert_eq!(error.code(), AgentErrorCode::ResidualLimit);
}

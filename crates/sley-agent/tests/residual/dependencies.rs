use super::*;
use sley_agent::residual::{
    dependencies,
    frontier::{Budget, factors},
};
use sley_agent::workspace::Program;
use sley_mutate::value::EntityBodyValue;

fn declaration(request: &Value) -> Value {
    let paths: Vec<_> = request["scope"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| format!("/bindings/values/{}", name.as_str().unwrap()))
        .collect();
    json!({"fields":paths.iter().map(|path| json!({"name":path,"cost":1,"eligible":true})).collect::<Vec<_>>(),
        "constraints":paths.iter().map(|path| json!({"fields":[path],"rows":[{path:3},{path:7}]})).collect::<Vec<_>>(),
        "dependencies":[]})
}

pub(super) fn committed_functions(functions: Vec<Value>) -> Fixture {
    let fixture = Fixture::new();
    let mut frame = json!({"af1":1});
    frame["fns"] = Value::Array(functions);
    let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(code, 0, "{result}");
    let (code, result) = cli(&fixture.dir, &["commit", "c1"]);
    assert_eq!(code, 0, "{result}");
    fixture
}

pub(super) fn literal_function(name: &str) -> Value {
    json!({"fn":name,"params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
        "blocks":[{"name":"entry","ops":[["amount","const",{"type":"i8","value":3}],
        ["sum","add","x","amount"]],"term":["return","sum"]}]})
}

#[test]
fn twenty_actual_independent_functions_factor_without_materializing_a_million_programs() {
    let names: Vec<_> = (0..20).map(|index| format!("site{index}")).collect();
    let fixture = committed_functions(names.iter().map(|name| literal_function(name)).collect());
    let head = fixture.workspace.read_head().unwrap();
    let symbols = fixture_names(&fixture, head.program());
    let mut request = per_target_request(json!({}));
    request["scope"] = json!(
        names
            .iter()
            .map(|name| format!("{name}.entry.amount"))
            .collect::<Vec<_>>()
    );
    let parsed = parse_request(&bytes(&request)).unwrap();
    let mut budget = Budget::default();
    let graph = dependencies::extract(head.program(), &symbols, &parsed, &mut budget).unwrap();
    let summary = graph.summary();
    assert_eq!(
        summary["components"].as_array().unwrap().len(),
        20,
        "{summary}"
    );
    assert!(
        summary["excluded_edges"]["preserved_integer_constant"]
            .as_u64()
            .unwrap()
            >= 20
    );
    let problem = graph
        .constrain(
            head.program(),
            &symbols,
            &parsed,
            &declaration(&request),
            &mut budget,
        )
        .unwrap();
    let frontier = factors::plan(&problem, &mut budget, false).unwrap();
    let report = frontier.summary();
    assert_eq!(report["materialized_component_rows"], 40);
    assert_eq!(report["description_count_decimal"], "1048576");
    assert_eq!(report["global_product_materialized"], false);
    let full = request["scope"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                format!("/bindings/values/{}", name.as_str().unwrap()),
                json!(if index % 2 == 0 { 3 } else { 7 }),
            )
        })
        .collect();
    let answers = frontier.encode(&problem, &full).unwrap();
    assert_eq!(frontier.decode(&problem, &answers).unwrap(), full);
    let filled = sley_agent::residual::plan::fill(&request, &full).unwrap();
    let (code, result) = cli(
        &fixture.dir,
        &["residual", "try", &filled.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["kernel"], "valid");
    for (index, name) in names.iter().enumerate() {
        let (code, result) = cli(&fixture.dir, &["call", name, "5", "--on", "c2"]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(
            result["result"],
            json!({"Ok":if index % 2 == 0 {8} else {12}})
        );
    }
}

#[test]
fn typed_calls_and_function_references_couple_sites_even_with_cartesian_domains() {
    for op in ["call", "fnref"] {
        let mut ops = vec![json!(["a", op, "adjust"]), json!(["b", op, "other"])];
        if op == "call" {
            for instruction in &mut ops {
                instruction.as_array_mut().unwrap().push(json!("x"));
            }
        }
        let bridge = json!({"fn":"bridge","params":[["x","i8"]],"returns":"i8",
            "blocks":[{"name":"entry","ops":ops,"term":["return","x"]}]});
        let fixture = committed_functions(vec![
            literal_function("adjust"),
            literal_function("other"),
            bridge,
        ]);
        let head = fixture.workspace.read_head().unwrap();
        let symbols = fixture_names(&fixture, head.program());
        let request = per_target_request(json!({}));
        let parsed = parse_request(&bytes(&request)).unwrap();
        let mut budget = Budget::default();
        let graph = dependencies::extract(head.program(), &symbols, &parsed, &mut budget).unwrap();
        assert_eq!(
            graph.summary()["components"].as_array().unwrap().len(),
            1,
            "{op}"
        );
        let problem = graph
            .constrain(
                head.program(),
                &symbols,
                &parsed,
                &declaration(&request),
                &mut budget,
            )
            .unwrap();
        let frontier = factors::plan(&problem, &mut budget, true).unwrap();
        assert_eq!(
            frontier.summary()["components"].as_array().unwrap().len(),
            1
        );
        assert_eq!(frontier.summary()["materialized_component_rows"], 4);
    }
}

#[test]
fn same_function_order_and_error_flow_keep_literal_sites_together() {
    let mut function = literal_function("adjust");
    function["blocks"][0]["ops"] = json!([
        ["amount","const",{"type":"i8","value":3}],
        ["second","const",{"type":"i8","value":4}],
        ["sum","add","amount","second"]]);
    let fixture = committed_functions(vec![function]);
    let head = fixture.workspace.read_head().unwrap();
    let symbols = fixture_names(&fixture, head.program());
    let mut request = per_target_request(json!({}));
    request["scope"] = json!(["adjust.entry.amount", "adjust.entry.second"]);
    let parsed = parse_request(&bytes(&request)).unwrap();
    let graph =
        dependencies::extract(head.program(), &symbols, &parsed, &mut Budget::default()).unwrap();
    assert_eq!(graph.summary()["components"].as_array().unwrap().len(), 1);
}

fn change_object(
    program: &Program,
    id: sley_id::EntityId,
    update: impl FnOnce(&mut EntityBodyValue),
) -> Program {
    let mut record = program.object(&id).unwrap().record().clone();
    update(&mut record.body);
    let replacement = sley_mutate::build_entity_object(program.epoch(), &record).unwrap();
    let objects = program
        .objects()
        .iter()
        .map(|object| {
            if object.record().entity_id == id {
                replacement.clone()
            } else {
                object.clone()
            }
        })
        .collect();
    Program::new(
        program.epoch(),
        program.root(),
        program.workspace(),
        objects,
    )
}

#[test]
fn exact_objects_and_request_are_rechecked_even_if_a_caller_reuses_the_root_label() {
    let fixture = literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let symbols = fixture_names(&fixture, head.program());
    let request = per_target_request(json!({}));
    let parsed = parse_request(&bytes(&request)).unwrap();
    let graph =
        dependencies::extract(head.program(), &symbols, &parsed, &mut Budget::default()).unwrap();
    let changed = change_object(
        head.program(),
        symbols.resolve("other.entry.amount").unwrap(),
        |body| {
            let EntityBodyValue::Operation(operation) = body else {
                panic!("operation")
            };
            operation.ordinal += 1;
        },
    );
    let result = graph.constrain(
        &changed,
        &symbols,
        &parsed,
        &declaration(&request),
        &mut Budget::default(),
    );
    assert_eq!(
        result.unwrap_err().code(),
        AgentErrorCode::ResidualBindingStale
    );
    // Even identical component/domain declarations carry the exact graph
    // context, so a newly extracted problem cannot reuse the old frontier.
    let mut budget = Budget::default();
    let original = graph
        .constrain(
            head.program(),
            &symbols,
            &parsed,
            &declaration(&request),
            &mut budget,
        )
        .unwrap();
    let frontier = factors::plan(&original, &mut budget, false).unwrap();
    let next_graph = dependencies::extract(&changed, &symbols, &parsed, &mut budget).unwrap();
    assert_eq!(
        graph.summary()["components"],
        next_graph.summary()["components"]
    );
    let next = next_graph
        .constrain(
            &changed,
            &symbols,
            &parsed,
            &declaration(&request),
            &mut budget,
        )
        .unwrap();
    assert_eq!(
        frontier
            .decode(&next, &serde_json::Map::new())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualBindingStale
    );
    let mut new_request = request.clone();
    new_request["scope"].as_array_mut().unwrap().reverse();
    let parsed = parse_request(&bytes(&new_request)).unwrap();
    assert_eq!(
        graph
            .constrain(
                head.program(),
                &symbols,
                &parsed,
                &declaration(&request),
                &mut Budget::default()
            )
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualBindingStale
    );
}

#[test]
fn missing_references_and_exhausted_budgets_refuse_without_partial_evidence() {
    let fixture = literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let symbols = fixture_names(&fixture, head.program());
    let parsed = parse_request(&bytes(&per_target_request(json!({})))).unwrap();
    let changed = change_object(
        head.program(),
        symbols.resolve("caller.entry.result").unwrap(),
        |body| {
            let EntityBodyValue::Operation(operation) = body else {
                panic!("operation")
            };
            let sley_ssmc::Immediate::Function(reference) = &mut operation.immediate else {
                panic!("call")
            };
            reference.function = sley_id::EntityId::from_bytes([254; 32]);
        },
    );
    let failure = dependencies::extract(&changed, &symbols, &parsed, &mut Budget::default())
        .err()
        .unwrap();
    assert_eq!(failure.code(), AgentErrorCode::ResidualInconclusive);
    assert!(failure.detail().contains("GRAPH_UNRESOLVED_REFERENCE"));
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 2);
    assert_eq!(
        dependencies::extract(head.program(), &symbols, &parsed, &mut budget)
            .err()
            .unwrap()
            .code(),
        AgentErrorCode::ResidualLimit
    );
    assert_eq!(
        dependencies::extract(head.program(), &symbols, &parsed, &mut budget)
            .err()
            .unwrap()
            .code(),
        AgentErrorCode::ResidualLimit
    );
}

#[test]
fn author_constraints_can_merge_components_but_cannot_replace_the_site_vocabulary() {
    let fixture = literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let symbols = fixture_names(&fixture, head.program());
    let request = per_target_request(json!({}));
    let parsed = parse_request(&bytes(&request)).unwrap();
    let graph =
        dependencies::extract(head.program(), &symbols, &parsed, &mut Budget::default()).unwrap();
    assert_eq!(graph.summary()["components"].as_array().unwrap().len(), 2);
    let mut declared = declaration(&request);
    declared["dependencies"] = json!([{"kind":"author_relation","fields":["/bindings/values/adjust.entry.amount","/bindings/values/other.entry.amount"]}]);
    let mut budget = Budget::default();
    let problem = graph
        .constrain(head.program(), &symbols, &parsed, &declared, &mut budget)
        .unwrap();
    assert_eq!(
        factors::plan(&problem, &mut budget, false)
            .unwrap()
            .summary()["components"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let unrelated = json!({"fields":[{"name":"arbitrary","cost":1,"eligible":true}],"constraints":[{"fields":["arbitrary"],"rows":[{"arbitrary":0}]}],"dependencies":[]});
    assert_eq!(
        graph
            .constrain(
                head.program(),
                &symbols,
                &parsed,
                &unrelated,
                &mut Budget::default()
            )
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualConstraintConflict
    );
}

fn append_object(program: &Program, id: sley_id::EntityId, body: EntityBodyValue) -> Program {
    let record = sley_mutate::EntityObjectRecord {
        entity_id: id,
        body,
        label: None,
        semantic_fingerprint: None,
    };
    let mut objects = program.objects().to_vec();
    objects.push(sley_mutate::build_entity_object(program.epoch(), &record).unwrap());
    Program::new(
        program.epoch(),
        program.root(),
        program.workspace(),
        objects,
    )
}

#[test]
fn function_reference_inside_a_constant_keeps_the_referenced_region_connected() {
    use sley_id::EntityId;
    use sley_mutate::value::{ConstantBody, OperationBody};
    use sley_ssmc::{
        ConstData, ConstValue, FunctionRefValue, FunctionType, Immediate, IntegerWidth, TypeExpr,
    };
    let fixture = literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let symbols = fixture_names(&fixture, head.program());
    let target = symbols.resolve("other").unwrap();
    let block = symbols.resolve("adjust.entry").unwrap();
    let EntityBodyValue::Function(function) = head.program().body(&target).unwrap() else {
        panic!("function")
    };
    let ty = TypeExpr::FunctionRef(FunctionType {
        parameters: vec![TypeExpr::SInt(IntegerWidth::from_bits(8))],
        result: Box::new(function.result_type.clone()),
        effects: vec![],
    });
    let constant = EntityId::from_bytes([241; 32]);
    let operation = EntityId::from_bytes([242; 32]);
    let program = append_object(
        head.program(),
        constant,
        EntityBodyValue::Constant(ConstantBody {
            value: ConstValue {
                value_type: ty.clone(),
                data: ConstData::FunctionRef(FunctionRefValue {
                    function: target,
                    type_arguments: vec![],
                }),
            },
        }),
    );
    let program = append_object(
        &program,
        operation,
        EntityBodyValue::Operation(OperationBody {
            block,
            ordinal: 2,
            opcode: 1,
            operands: vec![],
            result_types: vec![ty],
            immediate: Immediate::Entity(constant),
        }),
    );
    let program = change_object(&program, block, |body| {
        let EntityBodyValue::Block(block) = body else {
            panic!("block")
        };
        block.operations.push(operation);
    });
    let request = parse_request(&bytes(&per_target_request(json!({})))).unwrap();
    let graph =
        dependencies::extract(&program, &symbols, &request, &mut Budget::default()).unwrap();
    assert_eq!(graph.summary()["components"].as_array().unwrap().len(), 1);
    assert!(
        graph.summary()["included_relationship_tags"]["5"]
            .as_u64()
            .unwrap()
            >= 3
    );
}

#[test]
fn shared_effects_and_contract_predicates_couple_functions() {
    use sley_id::EntityId;
    use sley_mutate::value::{ContractBody, EffectDefBody, EntityIdSet};
    use sley_ssmc::{ContractKind, EffectKind, TypeExpr, Visibility};
    let predicate = json!({"fn":"predicate","params":[],"returns":"bool","blocks":[{
        "name":"entry","ops":[["yes","const",true]],"term":["return","yes"]}]});
    let fixture = committed_functions(vec![
        literal_function("adjust"),
        literal_function("other"),
        predicate,
    ]);
    let head = fixture.workspace.read_head().unwrap();
    let symbols = fixture_names(&fixture, head.program());
    let request = parse_request(&bytes(&per_target_request(json!({})))).unwrap();
    for mode in ["effects", "contracts"] {
        let mut program = Program::new(
            head.program().epoch(),
            head.program().root(),
            head.program().workspace(),
            head.program().objects().to_vec(),
        );
        let effect = EntityId::from_bytes([243; 32]);
        if mode == "effects" {
            program = append_object(
                &program,
                effect,
                EntityBodyValue::EffectDef(EffectDefBody {
                    effect_kind: EffectKind::StdoutWrite,
                    scope_type: TypeExpr::Unit,
                    request_type: TypeExpr::Text,
                    response_type: TypeExpr::Unit,
                    failure_type: TypeExpr::Unit,
                    visibility: Visibility::Private,
                }),
            );
        }
        for (index, name) in ["adjust", "other"].into_iter().enumerate() {
            let function = symbols.resolve(name).unwrap();
            let contract = EntityId::from_bytes([244 + u8::try_from(index).unwrap(); 32]);
            if mode == "contracts" {
                program = append_object(
                    &program,
                    contract,
                    EntityBodyValue::Contract(ContractBody {
                        target: function,
                        contract_kind: ContractKind::Precondition,
                        predicate: symbols.resolve("predicate").unwrap(),
                        bindings: vec![],
                        resource_limits: None,
                    }),
                );
            }
            program = change_object(&program, function, |body| {
                let EntityBodyValue::Function(function) = body else {
                    panic!("function")
                };
                if mode == "effects" {
                    function.effects = EntityIdSet::from_unsorted(vec![effect]).unwrap();
                } else {
                    function.contracts = EntityIdSet::from_unsorted(vec![contract]).unwrap();
                }
            });
        }
        let graph =
            dependencies::extract(&program, &symbols, &request, &mut Budget::default()).unwrap();
        assert_eq!(
            graph.summary()["components"].as_array().unwrap().len(),
            1,
            "{mode}"
        );
    }
}

#[test]
fn global_policy_prevents_splitting_and_old_frontiers_reject_a_different_graph() {
    use sley_id::EntityId;
    use sley_mutate::value::{EntityIdSet, PolicyBindingBody};
    let fixture = literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let symbols = fixture_names(&fixture, head.program());
    let request = per_target_request(json!({}));
    let parsed = parse_request(&bytes(&request)).unwrap();
    let mut budget = Budget::default();
    let graph = dependencies::extract(head.program(), &symbols, &parsed, &mut budget).unwrap();
    let problem = graph
        .constrain(
            head.program(),
            &symbols,
            &parsed,
            &declaration(&request),
            &mut budget,
        )
        .unwrap();
    let frontier = factors::plan(&problem, &mut budget, false).unwrap();
    let namespace = EntityId::from_bytes([247; 32]);
    let with_namespace = append_object(
        head.program(),
        namespace,
        EntityBodyValue::Namespace(sley_mutate::value::NamespaceBody {
            parent: None,
            members: EntityIdSet::from_unsorted(vec![
                symbols.resolve("adjust").unwrap(),
                symbols.resolve("other").unwrap(),
            ])
            .unwrap(),
        }),
    );
    let administrative =
        dependencies::extract(&with_namespace, &symbols, &parsed, &mut budget).unwrap();
    assert_eq!(
        administrative.summary()["components"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let changed = append_object(
        &with_namespace,
        EntityId::from_bytes([246; 32]),
        EntityBodyValue::PolicyBinding(PolicyBindingBody {
            subject: namespace,
            requirements: EntityIdSet::from_unsorted(vec![]).unwrap(),
        }),
    );
    let graph = dependencies::extract(&changed, &symbols, &parsed, &mut budget).unwrap();
    assert_eq!(graph.summary()["components"].as_array().unwrap().len(), 1);
    assert_eq!(
        graph.summary()["global_coupling_entities"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let next = graph
        .constrain(
            &changed,
            &symbols,
            &parsed,
            &declaration(&request),
            &mut budget,
        )
        .unwrap();
    assert_eq!(
        frontier
            .decode(&next, &serde_json::Map::new())
            .unwrap_err()
            .code(),
        AgentErrorCode::ResidualBindingStale
    );
}

#[test]
fn every_declared_literal_domain_value_must_fit_before_factoring() {
    let fixture = literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let symbols = fixture_names(&fixture, head.program());
    let request = per_target_request(json!({}));
    let parsed = parse_request(&bytes(&request)).unwrap();
    let graph =
        dependencies::extract(head.program(), &symbols, &parsed, &mut Budget::default()).unwrap();
    for value in [json!(128), json!(-129), json!(true), json!("7")] {
        let mut declared = declaration(&request);
        declared["constraints"][1]["rows"][1]["/bindings/values/other.entry.amount"] = value;
        assert!(
            graph
                .constrain(
                    head.program(),
                    &symbols,
                    &parsed,
                    &declared,
                    &mut Budget::default()
                )
                .is_err()
        );
    }
}

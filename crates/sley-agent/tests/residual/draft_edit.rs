use super::{Fixture, bytes, literal_fixture, literal_request};
use serde_json::{Value, json};
use sley_agent::candidate::{self, Authority};
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{edit, frontier::Budget, parse_request};
use sley_agent::{exec, frame, values};
use sley_mutate::{ImportedCandidate, MutationClass};

fn parent(fixture: &Fixture, frame: &Value, mut map: NameMap) -> (ImportedCandidate, NameMap) {
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(head.program(), &map);
    let authority = Authority::of(&head).unwrap();
    let nonce = candidate::fresh_nonce().unwrap();
    let compiled = frame::compile(
        head.program(),
        &names,
        &authority.ceilings,
        frame,
        nonce,
        &mut candidate::random32,
    )
    .unwrap();
    map.extend(&compiled.names);
    let candidate = candidate::assemble(&head, &authority, nonce, compiled.ops).unwrap();
    assert!(
        candidate::validate(&head, &authority, &candidate.stored_bytes)
            .unwrap()
            .is_valid()
    );
    (candidate, map)
}

fn new_parent(fixture: &Fixture) -> (ImportedCandidate, NameMap) {
    let functions: Vec<_> = ["adjust", "other"]
        .into_iter()
        .map(|name| {
            json!({
                "fn":name,"params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
                "blocks":[{"name":"entry","ops":[["amount","const",{"type":"i8","value":3}],
                    ["sum","add","x","amount"]],"term":["return","sum"]}]
            })
        })
        .collect();
    parent(
        fixture,
        &json!({"af1":1,"fns":functions}),
        NameMap::default(),
    )
}

fn request(value: i64) -> sley_agent::residual::Request {
    let mut value = literal_request(value);
    value["base"] = json!("d1@r1");
    parse_request(&bytes(&value)).unwrap()
}

fn run(program: &sley_agent::workspace::Program, map: &NameMap, name: &str) -> Value {
    let names = Names::build(program, map);
    let id = names.resolve(name).unwrap();
    let mut executor = exec::Executor::new(program).unwrap();
    let ty = &executor.parameter_types(&id)[0];
    let argument = values::read(
        &json!(5),
        ty,
        &values::ProgramTypes {
            program,
            names: &names,
        },
        "",
    )
    .unwrap();
    let outcome = executor
        .run(&id, vec![argument], exec::call_limits())
        .unwrap();
    exec::termination_json(&outcome.termination, &names)
}

#[test]
fn draft_composition_preserves_created_identities_and_unrelated_bytes() {
    let fixture = Fixture::new();
    let (parent, map) = new_parent(&fixture);
    let head = fixture.workspace.read_head().unwrap();
    let original = parent.stored_bytes.clone();
    let source = candidate::applied_program(&head, &parent).unwrap();
    let names = Names::build(&source, &map);
    let target = names.resolve("adjust.entry.amount").unwrap();
    let prepared = edit::draft::prepare(
        &head,
        &parent.stored_bytes,
        &map,
        &request(7),
        &mut Budget::default(),
    )
    .unwrap();
    for object in source.objects() {
        let id = object.record().entity_id;
        if id != target {
            assert_eq!(
                object.stored_bytes(),
                prepared.program.object(&id).unwrap().stored_bytes()
            );
        }
    }
    assert_eq!(parent.stored_bytes, original);
    assert_eq!(
        prepared.candidate.record.candidate_nonce,
        parent.record.candidate_nonce
    );
    let creates = |candidate: &ImportedCandidate| {
        candidate
            .record
            .operations
            .iter()
            .filter(|op| op.class == MutationClass::CreateEntity)
            .map(|op| op.target_entity)
            .collect::<Vec<_>>()
    };
    assert!(creates(&prepared.candidate).starts_with(&creates(&parent)));
    assert_eq!(
        creates(&prepared.candidate).len(),
        creates(&parent).len() + 1
    );
    assert_eq!(run(&prepared.program, &map, "adjust"), json!({"Ok":12}));
    assert_eq!(run(&prepared.program, &map, "other"), json!({"Ok":8}));
    assert_eq!(prepared.report["source_graph_preservation"], "verified");
    assert_eq!(prepared.report["receipt_binding"], "caller_required");
    let replay = edit::draft::prepare(
        &head,
        &parent.stored_bytes,
        &map,
        &request(7),
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(
        replay.candidate.stored_bytes,
        prepared.candidate.stored_bytes
    );
    assert!(!fixture.dir.join(".sley").exists());
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        head.transaction_id()
    );
}

#[test]
fn draft_composition_allocates_distinct_appended_constants_for_sparse_overrides() {
    let fixture = Fixture::new();
    let (parent, map) = new_parent(&fixture);
    let head = fixture.workspace.read_head().unwrap();
    let mut selected = request(7);
    selected.scope = json!(["adjust.entry.amount", "other.entry.amount"]);
    selected
        .bindings
        .insert("overrides".into(), json!({"other.entry.amount":9}));
    let prepared = edit::draft::prepare(
        &head,
        &parent.stored_bytes,
        &map,
        &selected,
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(run(&prepared.program, &map, "adjust"), json!({"Ok":12}));
    assert_eq!(run(&prepared.program, &map, "other"), json!({"Ok":14}));
    let source = candidate::applied_program(&head, &parent).unwrap();
    assert_eq!(prepared.program.objects().len(), source.objects().len() + 2);
}

#[test]
fn draft_composition_noop_keeps_exact_candidate_and_later_edits_keep_previous_constants() {
    let fixture = Fixture::new();
    let (parent, map) = new_parent(&fixture);
    let head = fixture.workspace.read_head().unwrap();
    let noop = edit::draft::prepare(
        &head,
        &parent.stored_bytes,
        &map,
        &request(3),
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(noop.candidate.stored_bytes, parent.stored_bytes);
    assert!(noop.contract.no_change());
    let first = edit::draft::prepare(
        &head,
        &parent.stored_bytes,
        &map,
        &request(9),
        &mut Budget::default(),
    )
    .unwrap();
    let second = edit::draft::prepare(
        &head,
        &first.candidate.stored_bytes,
        &map,
        &request(3),
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(run(&second.program, &map, "adjust"), json!({"Ok":8}));
    assert_eq!(
        second.program.objects().len(),
        first.program.objects().len()
    );
    for object in first.program.objects().iter().filter(|object| {
        matches!(
            object.record().body,
            sley_mutate::value::EntityBodyValue::Constant(_)
        )
    }) {
        assert_eq!(
            object.stored_bytes(),
            second
                .program
                .object(&object.record().entity_id)
                .unwrap()
                .stored_bytes()
        );
    }
}

#[test]
fn draft_composition_updates_existing_replacements_and_appends_untouched_accepted_targets() {
    let fixture = literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let map = NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap();
    let names = Names::build(head.program(), &map);
    let accepted_request = parse_request(&bytes(&literal_request(7))).unwrap();
    let expansion = edit::expand(head.program(), &names, &accepted_request)
        .unwrap()
        .expansion;
    let (parent, map) = parent(&fixture, &expansion.frame, map);
    let mut request = request(11);
    request.scope = json!(["adjust.entry.amount", "other.entry.amount"]);
    let prepared = edit::draft::prepare(
        &head,
        &parent.stored_bytes,
        &map,
        &request,
        &mut Budget::default(),
    )
    .unwrap();
    for name in ["adjust", "other", "caller", "outer"] {
        assert_eq!(run(&prepared.program, &map, name), json!({"Ok":16}));
    }
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        head.transaction_id()
    );
}

#[test]
fn draft_composition_refuses_wrong_base_invalid_bytes_bad_values_and_exhausted_budget() {
    let fixture = Fixture::new();
    let (parent, map) = new_parent(&fixture);
    let head = fixture.workspace.read_head().unwrap();
    let accepted_request = parse_request(&bytes(&literal_request(3))).unwrap();
    assert!(
        edit::draft::prepare(
            &head,
            &parent.stored_bytes,
            &map,
            &accepted_request,
            &mut Budget::default()
        )
        .is_err()
    );
    assert!(
        edit::draft::prepare(
            &head,
            b"not a candidate",
            &map,
            &request(3),
            &mut Budget::default()
        )
        .is_err()
    );
    assert!(
        edit::draft::prepare(
            &head,
            &parent.stored_bytes,
            &map,
            &request(128),
            &mut Budget::default()
        )
        .is_err()
    );
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 0);
    let error = edit::draft::prepare(&head, &parent.stored_bytes, &map, &request(7), &mut budget)
        .err()
        .unwrap();
    assert_eq!(error.code(), sley_agent::AgentErrorCode::ResidualLimit);
    let mut invalid = parent.record.clone();
    invalid.expiry = sley_mutate::CandidateExpiry::unix_millis(1);
    let invalid = sley_mutate::build_candidate(&invalid).unwrap();
    assert!(
        edit::draft::prepare(
            &head,
            &invalid.stored_bytes,
            &map,
            &request(7),
            &mut Budget::default()
        )
        .is_err()
    );
    assert!(!fixture.dir.join(".sley").exists());
}

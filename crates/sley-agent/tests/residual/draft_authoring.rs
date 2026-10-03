use serde_json::{Value, json};
use sley_agent::afx::{self, Role};
use sley_agent::names::{NameMap, Names};
use sley_agent::residual::{binding::DraftSource, edit::draft::authoring, frontier::Budget};

use super::{Fixture, bytes, cli, literal_request, runtime};

#[test]
fn generated_literal_repairs_keep_checked_syntax_tables_and_later_layering() {
    let fixture = Fixture::new();
    let frame = json!({"af1":1,"afx":1,"namespace":null,"functions":[{
        "fn":"adjust","params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
        "blocks":[{"name":"entry","ops":[["a","add?","x",3]],
            "term":["ok",["mul?","a",2]]}]}],
        "test_tables":[{"name":"t","fn":"adjust","cases":[{"args":[5],"expect":{"Ok":16}}]}]});
    let (code, report) = cli(&fixture.dir, &["try", &frame.to_string()]);
    assert_eq!(code, 0, "{report}");
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(head.program(), &NameMap::default());
    let expansion = afx::expand(head.program(), &names, &frame).unwrap();
    let literal = expansion
        .map
        .entries
        .iter()
        .find(|e| e.role == Role::Literal)
        .unwrap();
    let (block_pointer, _) = literal.expanded.rsplit_once("/ops/").unwrap();
    let block = expansion.frame.pointer(block_pointer).unwrap()["name"]
        .as_str()
        .unwrap();
    let mut request = literal_request(7);
    request["base"] = json!("d1@r1");
    request["scope"] = json!([format!("adjust.{block}.{}", literal.name)]);
    let source = DraftSource::capture(
        &fixture.workspace,
        &bytes(&request),
        &runtime(),
        &mut Budget::default(),
    )
    .unwrap();
    let prepared = source
        .prepare(&fixture.workspace, &mut Budget::default())
        .unwrap();
    let retained = prepared.authoring.unwrap();
    let mut expected = frame.clone();
    *expected.pointer_mut(&literal.authored).unwrap() = json!(7);
    expected["consts"] = json!([{"name":"k_3","type":"i8","value":3}]);
    assert_eq!(retained.frame, expected);
    assert_eq!(retained.frame["test_tables"], frame["test_tables"]);
    assert_eq!(retained.expanded["tests"], expansion.frame["tests"]);
    assert_eq!(retained.changes[0]["authored"], literal.authored);
    assert!(
        !retained.source_map["entries"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let (code, report) = cli(
        &fixture.dir,
        &["try", &retained.frame.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{report}");
    // An ordinary subsequent layer must still run the edited computation.
    // Keep the original table expectation: its now-failing assertion must not
    // be rewritten to make the edit appear behaviorally unchanged.
    let (code, report) = cli(&fixture.dir, &["try", r#"{"af1":1}"#, "--on", "d2@r1"]);
    assert_ne!(code, 0, "{report}");
    let status: Value = serde_json::from_slice(
        &std::fs::read(fixture.dir.join(".sley/drafts/d2/r2/status.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(status["state"], "valid");
    assert_eq!(report["tests"][0]["actual"], "Ok(24)", "{report}");
    assert_eq!(report["tests"][0]["expected"], "Ok(16)", "{report}");
}

#[test]
fn retained_plain_patch_edit_and_shared_constant_reference_are_local() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(head.program(), &NameMap::default());
    for (source, pointer) in [
        (
            json!({"af1":1,"namespace":"example","patch":[{"fn":"f","blocks":{"entry":{"ops":[["n","const","shared"]]}}}],"consts":[{"name":"shared","type":"i8","value":3}]}),
            "/patch/0/blocks/entry/ops/0/2",
        ),
        (
            json!({"af1":1,"edit":[{"fn":"f","replace_op":"entry.n","with":["const",{"type":"i8","value":3}]}]}),
            "/edit/0/with/1",
        ),
    ] {
        let delta = json!({"af1":1,"namespace":null,"edit":[{"fn":"f","replace_op":"entry.n","with":["const",{"type":"i8","value":7}]}]});
        let retained = authoring::retain(
            head.program(),
            &names,
            &source,
            &delta,
            &mut Budget::default(),
        )
        .unwrap();
        let mut expected = source.clone();
        *expected.pointer_mut(pointer).unwrap() = json!({"type":"i8","value":7});
        assert_eq!(retained.frame, expected);
        assert_eq!(retained.expanded, expected);
        assert_eq!(retained.frame["consts"], source["consts"]);
    }
}

#[test]
fn retained_noop_is_exact_and_accepted_target_appends_without_namespace_override() {
    let fixture = super::literal_fixture();
    let head = fixture.workspace.read_head().unwrap();
    let map = NameMap::read(&fixture.dir.join(".sley/names.json")).unwrap();
    let names = Names::build(head.program(), &map);
    let source = json!({"af1":1,"namespace":"kept","tests":[]});
    let no_op = json!({"af1":1,"namespace":null,"edit":[]});
    let retained = authoring::retain(
        head.program(),
        &names,
        &source,
        &no_op,
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(retained.frame, source);
    assert_eq!(retained.changes, json!([]));
    let delta = json!({"af1":1,"namespace":null,"edit":[{"fn":"adjust","replace_op":"entry.amount","with":["const",{"type":"i8","value":7}]}]});
    let retained = authoring::retain(
        head.program(),
        &names,
        &source,
        &delta,
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(retained.frame["namespace"], "kept");
    assert_eq!(retained.frame["edit"], delta["edit"]);
}

#[test]
fn object_operation_aliases_retain_their_original_syntax() {
    for afx in [false, true] {
        for annotated in [false, true] {
            let fixture = Fixture::new();
            let mut operation = json!({"name":"amount","opcode":"constant_ref",
                "operands":[{"type":"i8","value":3}]});
            if annotated {
                operation["type"] = json!("i8");
            }
            let mut frame = json!({"af1":1,"functions":[{
                "name":"adjust","params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
                "blocks":[{"name":"entry","ops":[operation,
                    ["sum","add","x","amount"]],"term":["return","sum"]}]}]});
            if afx {
                frame["afx"] = json!(1);
            }
            let (code, report) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
            assert_eq!(code, 0, "{report}");
            let mut request = literal_request(7);
            request["base"] = json!("d1@r1");
            let source = DraftSource::capture(
                &fixture.workspace,
                &bytes(&request),
                &runtime(),
                &mut Budget::default(),
            )
            .unwrap();
            let prepared = source
                .prepare(&fixture.workspace, &mut Budget::default())
                .unwrap();
            frame["functions"][0]["blocks"][0]["ops"][0]["operands"][0]["value"] = json!(7);
            frame["consts"] = json!([{"name":"k_3","type":"i8","value":3}]);
            assert_eq!(prepared.authoring.unwrap().frame, frame);
        }
    }
}

#[test]
fn authoring_repair_refuses_nonliteral_ambiguous_missing_and_exhausted_sources() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap();
    let names = Names::build(head.program(), &NameMap::default());
    let delta = json!({"af1":1,"edit":[{"fn":"f","replace_op":"entry.n","with":["const",{"type":"i8","value":7}]}]});
    for ops in [
        json!([["n", "add", "x", "x"]]),
        json!([["n", "const", 3], ["n", "const", 3]]),
        json!([]),
    ] {
        let source = json!({"af1":1,"fns":[{"fn":"f","blocks":[{"name":"entry","ops":ops}]}]});
        assert!(
            authoring::retain(
                head.program(),
                &names,
                &source,
                &delta,
                &mut Budget::default()
            )
            .is_err()
        );
    }
    let mut budget = Budget::limited(std::time::Duration::from_secs(2), 0);
    let error = authoring::retain(
        head.program(),
        &names,
        &json!({"af1":1}),
        &delta,
        &mut budget,
    )
    .err()
    .unwrap();
    assert_eq!(error.code(), sley_agent::AgentErrorCode::ResidualLimit);
}

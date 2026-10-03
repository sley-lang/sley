//! Real kernel controls: advisory artifacts never grant mutation authority.

use super::{Fixture, cli, expand, literal_request, planning_request, relation_request};
use serde_json::{Value, json};
use sley_agent::{candidate::Authority, genesis, workspace::Workspace};
use sley_mutate::MutationClass;
use sley_policy::{PolicyRootBuilder, PrincipalGrantBuilder};
use sley_state_root::StateRootBuilder;
use sley_store::ObjectStore;
use sley_txn::{TransactionRepository, TrustedGenesisInput};
use std::{fs, path::Path};

fn restricted(parent: &Fixture, budget_denial: bool) -> Workspace {
    let head = parent.workspace.read_head().unwrap();
    let dir = parent.dir.join("restricted");
    let workspace = Workspace::at(&dir);
    let repo = workspace.repo();
    fs::create_dir_all(&repo).unwrap();
    let mut ceilings = genesis::INIT_CEILINGS;
    if budget_denial {
        ceilings.max_mutation_count = 1;
    }
    let mut grant =
        PrincipalGrantBuilder::new(ceilings).mutation_class(MutationClass::ReplaceEntityVersion);
    if budget_denial {
        grant = grant.mutation_class(MutationClass::CreateEntity);
    }
    let policy = PolicyRootBuilder::new(head.workspace())
        .principal_grant(
            Authority::of(&head).unwrap().principal,
            grant.build().unwrap(),
        )
        .build(&sley_policy::conformance_registry().unwrap())
        .unwrap();
    let record = &head.state_root().record;
    let state = StateRootBuilder::new(
        head.workspace(),
        record.contract_root,
        record.test_root,
        policy.root(),
    )
    .build(&sley_state_root::conformance_registry().unwrap())
    .unwrap();
    let epoch = head.epoch();
    let verifier = move |bytes: &[u8]| {
        sley_mutate::import_entity_object(epoch, bytes).map(|object| object.object_id())
    };
    for id in [record.contract_root, record.test_root] {
        let bytes = ObjectStore::new(parent.workspace.repo())
            .read(id, &verifier)
            .unwrap();
        ObjectStore::new(&repo).put(id, &bytes, &verifier).unwrap();
    }
    let genesis = TransactionRepository::new(&repo)
        .initialize_trusted_genesis(TrustedGenesisInput::new(&state, &policy, &[], &[]))
        .unwrap();
    sley_repo::BranchRepository::new(&repo)
        .create_branch("main", genesis.transaction_id())
        .unwrap();
    workspace
}

fn refusal(dir: &Path, report: &Value, symbol: &str) {
    assert_eq!(report["kernel"], "refused", "{report}");
    assert_eq!(report["verdict"]["symbol"], symbol, "{report}");
    assert_eq!(report["verdict"]["phase"], 9, "{report}");
    assert_eq!(report["native_admission"], "not_attempted", "{report}");
    let handle = report["handle"].as_str().unwrap();
    // Forge the saved advisory summary while retaining the actual candidate.
    fs::write(
        dir.join(format!(".sley/candidates/{handle}.json")),
        json!({"valid":true,"kernel":"valid","native_admission":"passed",
            "capabilities":["CreateEntity"],"tests":{"passed":100}})
        .to_string(),
    )
    .unwrap();
    for command in ["submit", "commit"] {
        let (code, result) = cli(dir, &[command, handle]);
        assert_ne!(code, 0, "{result}");
        assert_eq!(result["error"], "AGENT_SUBMISSION_REFUSED", "{result}");
    }
    assert!(!dir.join("final_candidate.hex").exists());
}

#[test]
fn residual_direct_trials_preserve_policy_and_budget_refusals_despite_forged_summaries() {
    for (budget_denial, symbol) in [
        (false, "POLICY_GRANT_DENIED"),
        (true, "CAP_BUDGET_EXCEEDED"),
    ] {
        let parent = Fixture::new();
        let workspace = restricted(&parent, budget_denial);
        let before = workspace.read_head().unwrap().transaction_id();
        let request = planning_request();
        let (code, ordinary) = cli(
            workspace.dir(),
            &["try", &expand(&request).frame.to_string(), "--no-test"],
        );
        assert_ne!(code, 0, "{ordinary}");
        assert_eq!(ordinary["verdict"]["symbol"], symbol, "{ordinary}");
        let (code, residual) = cli(
            workspace.dir(),
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_ne!(code, 0, "{residual}");
        refusal(workspace.dir(), &residual, symbol);
        assert_eq!(workspace.read_head().unwrap().transaction_id(), before);
    }
}

#[test]
fn residual_closed_relation_fill_does_not_mint_policy_or_budget_authority() {
    for (budget_denial, symbol) in [
        (false, "POLICY_GRANT_DENIED"),
        (true, "CAP_BUDGET_EXCEEDED"),
    ] {
        let parent = Fixture::new();
        let workspace = restricted(&parent, budget_denial);
        let before = workspace.read_head().unwrap().transaction_id();
        let request = relation_request();
        let (code, plan) = cli(
            workspace.dir(),
            &["residual", "try", &request.to_string(), "--no-test"],
        );
        assert_eq!(code, 1, "{plan}");
        assert_eq!(plan["kernel"], "not_run", "{plan}");
        assert_eq!(
            plan["family_checks"]["compiler"], "all_rows_passed",
            "{plan}"
        );
        let handle = plan["plan"].as_str().unwrap();
        let field = plan["unresolved"][0]["path"].as_str().unwrap();
        let choose = serde_json::Map::from_iter([(
            field.to_owned(),
            request["choices"]["rows"][0][field].clone(),
        )]);
        let fill = json!({"residual":1,"plan":handle,"choose":choose});
        let (code, report) = cli(
            workspace.dir(),
            &["residual", "fill", handle, &fill.to_string()],
        );
        assert_ne!(code, 0, "{report}");
        refusal(workspace.dir(), &report, symbol);
        assert_eq!(workspace.read_head().unwrap().transaction_id(), before);
    }
}

#[test]
fn residual_passing_selected_tests_do_not_become_native_transaction_evidence() {
    let fixture = Fixture::new();
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    let frame = json!({"af1":1,"afx":1,"fns":[{
        "fn":"adjust","params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>",
        "blocks":[{"name":"entry","ops":[["amount","const",{"type":"i8","value":3}],
            ["sum","add","x","amount"]],"term":["return","sum"]}]}],
        "test_tables":[{"name":"table","fn":"adjust","cases":[{"args":[5],"expect":{"Ok":12}}]}]});
    let (_, ordinary) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(ordinary["verdict"]["valid"], true, "{ordinary}");
    let (code, ordinary_commit) = cli(&fixture.dir, &["commit", "c1"]);
    assert_ne!(code, 0, "{ordinary_commit}");
    assert_eq!(
        ordinary_commit["detail"],
        "commit refused: TXN_TEST_EVIDENCE_UNSUPPORTED"
    );
    let mut request = literal_request(7);
    request["base"] = json!("d1@r1");
    let (code, report) = cli(&fixture.dir, &["residual", "try", &request.to_string()]);
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["kernel"], "valid", "{report}");
    assert_eq!(report["public_checks"], "passed", "{report}");
    assert_eq!(report["checks"]["cases"], 1, "{report}");
    assert_eq!(report["native_admission"], "not_attempted", "{report}");
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
    assert!(!fixture.dir.join("final_candidate.hex").exists());
    let handle = report["handle"].as_str().unwrap();
    let (code, submitted) = cli(&fixture.dir, &["submit", handle]);
    assert_eq!(code, 0, "{submitted}");
    assert_eq!(submitted["selected_tests"], 1, "{submitted}");
    fs::write(
        fixture.dir.join(format!(".sley/candidates/{handle}.json")),
        json!({"valid":true,"native_admission":"passed","test_evidence":"approved"}).to_string(),
    )
    .unwrap();
    let (code, committed) = cli(&fixture.dir, &["commit", handle]);
    assert_ne!(code, 0, "{committed}");
    assert_eq!(
        committed["error"], "AGENT_SUBMISSION_REFUSED",
        "{committed}"
    );
    assert_eq!(
        committed["detail"], ordinary_commit["detail"],
        "{committed}"
    );
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
}

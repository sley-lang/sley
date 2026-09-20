//! Live-trial strict-case driver for the sley_2_0 arm.
//!
//! The Python trial oracle (`bench/fixtures/sley2_live_judge.py`) cannot
//! judge computed values over SMP1: execution reports carry observation
//! hashes by product design, never values. This frozen driver executes
//! strict cases in-process (the exact S3 conformance pattern) and reports
//! values, codes, and fuel as JSON the Python judge compares.
//!
//! Environment in (all required unless noted):
//! - `SUCC_JUDGE_REPO`: scratch repository path (already seeded copy).
//! - `SUCC_JUDGE_MANIFEST`: task_manifest.json path (principal source).
//! - `SUCC_JUDGE_CANDIDATE`: candidate record hex, or empty to skip commit.
//! - `SUCC_JUDGE_FUNCTION`: target function entity hex.
//! - `SUCC_JUDGE_CASES`: JSON array of
//!   `{"inputs": [{"type": "SInt", "bits": 64, "value": N} | {"type":
//!   "Bool", "value": b}], "expect": {"value": {...same...}} |
//!   {"failure": "VM_CODE"}}`.
//! - `SUCC_JUDGE_BASELINE`: `1` to also run cases pre-commit.
//! Stdout: one `LIVE_JUDGE_RESULT {...}` line. Exit 0 with a verdict
//! inside the JSON; any driver failure prints a harness-error verdict.

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId};
use sley_repo::RepositoryObjectVerifier;
use sley_ssmc::{ConstData, ConstValue, IntegerWidth, TypeExpr};
use sley_state_root::conformance_epoch_id as state_epoch_id;
use sley_store::ObjectStore;
use sley_txn::TransactionRepository;
use sley_vm::{
    CacheProfile, ExecutionLimits, ExecutionRequest, ExecutionTermination, LoweringInput,
    execute_function,
};

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

fn const_of(item: &serde_json::Value) -> ConstValue {
    let kind = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "SInt" => {
            let bits = item.get("bits").and_then(|v| v.as_u64()).unwrap_or(64);
            let value = item.get("value").and_then(|v| v.as_i64()).unwrap_or(0);
            ConstValue {
                value_type: TypeExpr::SInt(IntegerWidth::from_bits(bits as u16)),
                data: ConstData::SInt(value as i128),
            }
        }
        "Bool" => {
            let value = item.get("value").and_then(|v| v.as_bool()).unwrap_or(false);
            ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(value),
            }
        }
        other => panic!("unsupported const kind {other}"),
    }
}

struct ProgramSlice {
    function: sley_ssmc::FunctionGraph,
    types: sley_check::TypeEnvironment,
}

fn function_inputs(
    complete: &sley_policy::complete_entities::CompleteEntities,
    func: EntityId,
) -> ProgramSlice {
    // Only the entry graph is target-selected. Parameters, blocks,
    // and operations travel as the complete inventories production
    // lowering requires (see the LoweringInput construction): the
    // driver never slices the root.
    let function = complete
        .functions
        .iter()
        .find(|f| f.entity_id == func)
        .unwrap_or_else(|| panic!("function bound"))
        .clone();
    ProgramSlice {
        function,
        types: TypeEnvironment::new(complete.type_definitions.clone()).unwrap(),
    }
}

fn epoch() -> SchemaEpochId {
    sley_state_root::conformance_epoch_id().unwrap()
}

fn outcome_json(
    outcome: &Result<sley_vm::ExecutionOutcome, sley_vm::ExecutionError>,
) -> serde_json::Value {
    match outcome {
        Ok(ok) => {
            let value = match &ok.termination {
                ExecutionTermination::Success(value) => const_summary(&value.data),
                other => serde_json::json!({"non_success": format!("{other:?}")}),
            };
            serde_json::json!({"ok": true, "value": value,
                "instructions": ok.instruction_count, "fuel": ok.fuel_used,
                "peak_value_units": ok.peak_value_units})
        }
        Err(error) => serde_json::json!({"ok": false, "code": error.to_string()}),
    }
}

fn const_summary(data: &ConstData) -> serde_json::Value {
    match data {
        ConstData::Bool(flag) => serde_json::json!({"Bool": flag}),
        ConstData::SInt(number) => serde_json::json!({"SInt": number.to_string()}),
        ConstData::UInt(number) => serde_json::json!({"UInt": number.to_string()}),
        ConstData::Result(result) => match result {
            sley_ssmc::ResultConst::Ok(inner) => serde_json::json!({
                "Result": {"Ok": const_summary(&inner.data)},
            }),
            sley_ssmc::ResultConst::Err(inner) => serde_json::json!({
                "Result": {"Err": const_summary(&inner.data)},
            }),
        },
        ConstData::BuiltinFailure(failure) => serde_json::json!({
            "BuiltinFailure": {"kind": format!("{:?}", failure.kind), "code": failure.code},
        }),
        other => serde_json::json!({"other": format!("{other:?}")}),
    }
}

fn run_cases(
    complete: &sley_policy::complete_entities::CompleteEntities,
    program: &ProgramSlice,
    state_root: sley_id::StateRoot,
    cases: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for case in cases {
        let inputs: Vec<ConstValue> = case["inputs"]
            .as_array()
            .unwrap_or_else(|| panic!("case inputs shape"))
            .iter()
            .map(const_of)
            .collect();
        let request = ExecutionRequest {
            inputs,
            limits: ExecutionLimits {
                max_instructions: 1_000,
                max_fuel: 10_000,
                max_value_units: 100_000,
                max_output_units: 10_000,
                cancel_at_fuel: None,
            },
        };
        let input = LoweringInput {
            types: &program.types,
            function: &program.function,
            // Complete root inventories: production lowering narrows
            // per function itself (owned_inventory) and judges
            // call_direct callees against the root context (contract
            // E6). A root sliced to the target starves callee
            // signature resolution (VM_LOWER_IMMEDIATE_MISMATCH) and
            // callee narrowing. Only the entry graph itself is
            // target-selected; everything else stays complete.
            parameters: &complete.parameters,
            blocks: &complete.blocks,
            operations: &complete.operations,
            schema_epoch: epoch(),
            state_root,
            profile: CacheProfile::EXTENDED_V1,
            constants: &complete.constants,
            globals: &complete.globals,
            functions: &complete.functions,
            contracts: &[],
            adapters: &[],
        };
        out.push(outcome_json(&execute_function(input, request)));
    }
    out
}

#[test]
fn live_case_driver() {
    // Env: SUCC_JUDGE_REPO (seeded scratch repo), SUCC_JUDGE_FUNCTION
    // (target entity hex), SUCC_JUDGE_CASES (JSON array of
    // {"inputs": [{"type": "SInt"|"Bool", ...}], ...}).
    // Stdout: one LIVE_JUDGE_RESULT {"ok": true, "cases": [...]} line
    // with per-case values, codes, and fuel. Never commits, never
    // writes: the Python judge owns state changes.
    let repo_path = std::env::var("SUCC_JUDGE_REPO").unwrap();
    let function = EntityId::from_bytes(hex32(&std::env::var("SUCC_JUDGE_FUNCTION").unwrap()));
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::env::var("SUCC_JUDGE_CASES").unwrap()).unwrap();
    let root = std::path::PathBuf::from(&repo_path);
    let repo = TransactionRepository::new(&root);
    let epoch = state_epoch_id().unwrap();
    let store = ObjectStore::new(&root);
    let verifier = RepositoryObjectVerifier::new(epoch);
    let head = repo.accepted_head().unwrap();
    let mut objects = Vec::new();
    for meta in head.objects() {
        let bytes = store.read(meta.object_id(), &verifier).unwrap();
        objects.push(sley_mutate::import_entity_object(epoch, &bytes).unwrap());
    }
    objects.sort_by_key(|o: &sley_mutate::EntityObject| o.record().entity_id);
    let complete = sley_policy::complete_entities::project_complete_entities(&objects).unwrap();
    let program = function_inputs(&complete, function);
    let results = run_cases(&complete, &program, head.state_root().root, &cases);
    let report = serde_json::json!({"ok": true, "cases": results});
    println!(
        "LIVE_JUDGE_RESULT {}",
        serde_json::to_string(&report).unwrap()
    );
}

fn hex32(text: &str) -> [u8; 32] {
    let bytes = unhex(text);
    bytes.try_into().unwrap_or_else(|_| panic!("hex32 length"))
}

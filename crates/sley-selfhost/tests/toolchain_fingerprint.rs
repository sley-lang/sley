//! `fp_*`: Sley semantic fingerprints against the bootstrap gate.
mod common;
use common::*;
use sley_id::EntityId;
use sley_selfhost::canonical::{self, CanonicalS};
use sley_selfhost::native;
use sley_selfhost::seed::{Program, t};
use sley_selfhost::toolchain::fingerprint::{
    FP_FUNCTION_MISSING, FP_INVENTORY_INVALID, FP_LOCAL_REFERENCE_INVALID,
};
use sley_ssmc::fingerprint::{FingerprintErrorCode, FunctionFingerprintInput, fingerprint_function};
use sley_ssmc::{ConstData, ConstValue, Parameter, ParameterRole, TypeExpr};
use sley_vm::bootstrap::{
    BootstrapProfileInput, BootstrapProfileReport, BootstrapProfileVersion, judge_bootstrap_profile,
};
use std::time::Instant;
use sley_selfhost::native::Runner;
use sley_vm::ExecutionTermination;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The modules `fp_*`/`pk_*` need (independent of unrelated modules under
/// concurrent development); identities match the full toolchain module.
fn program() -> &'static (Program, HashMap<String, EntityId>) {
    use sley_selfhost::seed::Module;
    use sley_selfhost::toolchain::{
        TOOLCHAIN_SEED, codec, fingerprint, inv, model, package, rt, scb, types,
    };
    static P: OnceLock<(Program, HashMap<String, EntityId>)> = OnceLock::new();
    P.get_or_init(|| {
        let mut m = Module::new(TOOLCHAIN_SEED);
        model::declare(&mut m);
        rt::define(&mut m);
        codec::define(&mut m);
        types::define(&mut m);
        inv::define(&mut m);
        fingerprint::define(&mut m);
        package::define(&mut m);
        scb::define(&mut m);
        scb::define_entry_key(&mut m);
        m.finish()
    })
}

/// Runs `entry` of [`program`] (lowered once) and reports the instructions.
fn run_counted(entry: &str, inputs: Vec<ConstValue>) -> (ConstValue, u64) {
    static RUNNERS: OnceLock<Mutex<HashMap<String, &'static Runner<'static>>>> = OnceLock::new();
    let runner = {
        let mut guard = RUNNERS.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
        *guard.entry(entry.to_owned()).or_insert_with(|| {
            let (program, names) = program();
            Box::leak(Box::new(Runner::new(program, names[entry])))
        })
    };
    let outcome = runner.run(inputs, native::big_limits()).expect("executes");
    match outcome.termination {
        ExecutionTermination::Success(value) => (value, outcome.instruction_count),
        other => panic!("{entry}: {other:?}"),
    }
}

fn ids_input(ids: &[EntityId]) -> ConstValue {
    ConstValue {
        value_type: t::vec(TypeExpr::Bytes),
        data: ConstData::Sequence(ids.iter().map(|id| bytes(id.as_bytes())).collect()),
    }
}

fn boolv(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

fn byte_list(value: &ConstValue) -> Vec<Vec<u8>> {
    match &value.data {
        ConstData::Sequence(items) => items.iter().map(as_bytes).collect(),
        other => panic!("not a vector: {other:?}"),
    }
}

/// The pruned closure of `entry`, its canonical S, and the gate report.
fn gate_fixture(entry: &str) -> (Program, CanonicalS, BootstrapProfileReport) {
    let (program, names) = program();
    let entry = names[entry];
    let pruned = canonical::prune_to_closure(program, entry);
    let s = canonical::build(&pruned, entry, "fp-test");
    let lowered = native::lower(&pruned, entry).expect("lowers");
    let types = native::type_environment(&pruned);
    let entry_fn = pruned.functions.iter().find(|f| f.entity_id == entry).unwrap();
    let report = judge_bootstrap_profile(&BootstrapProfileInput {
        types: &types,
        profile_version: BootstrapProfileVersion::V3,
        schema_epoch: s.epoch,
        presented_image_bytes: &lowered.bytes,
        entry: entry_fn,
        functions: &pruned.functions,
        parameters: &pruned.parameters,
        blocks: &pruned.blocks,
        operations: &pruned.operations,
        adapters: &pruned.imports,
        constants: &pruned.constants,
    })
    .expect("gate admits the closure");
    (pruned, s, report)
}

/// The gate's narrowing plus the native fingerprint (error code on failure).
fn native_fingerprint(program: &Program, s: &CanonicalS, fid: EntityId) -> Result<Vec<u8>, u32> {
    let blocks: Vec<_> = program.blocks.iter().filter(|b| b.function == fid).cloned().collect();
    let operations: Vec<_> = program
        .operations
        .iter()
        .filter(|op| blocks.iter().any(|b| b.entity_id == op.block))
        .cloned()
        .collect();
    let parameters: Vec<_> = program
        .parameters
        .iter()
        .filter(|p| {
            p.owner == fid
                || blocks
                    .iter()
                    .any(|b| b.entity_id == p.owner && b.parameters.contains(&p.entity_id))
        })
        .cloned()
        .collect();
    let function = program.functions.iter().find(|f| f.entity_id == fid).unwrap();
    fingerprint_function(
        s.epoch,
        FunctionFingerprintInput {
            function,
            parameters: &parameters,
            blocks: &blocks,
            operations: &operations,
        },
    )
    .map(|fp| fp.as_bytes().to_vec())
    .map_err(|e| match e.code() {
        FingerprintErrorCode::InventoryInvalid => FP_INVENTORY_INVALID,
        FingerprintErrorCode::LocalReferenceInvalid => FP_LOCAL_REFERENCE_INVALID,
        other => panic!("unexpected native code {other:?}"),
    })
}

fn sley_fps(s: &CanonicalS, fids: &[EntityId], closure: bool) -> (Result<Vec<Vec<u8>>, u32>, u64) {
    let (value, instructions) = run_counted(
        "t_fp_closure",
        vec![
            objects_input(s),
            bytes(s.epoch.as_bytes()),
            ids_input(fids),
            boolv(closure),
        ],
    );
    (result(value).map(|v| byte_list(&v)), instructions)
}

fn check_closure(entry: &str) {
    let (pruned, s, report) = gate_fixture(entry);
    let fids = report.functions().to_vec();
    let expected: Vec<Vec<u8>> = report
        .closure_fingerprints()
        .iter()
        .map(|fp| fp.as_bytes().to_vec())
        .collect();
    let operations = pruned.operations.len();
    // Baseline: drv_load (plus fp_owned) with no function.
    let (_, baseline) = sley_fps(&s, &[], true);
    for closure in [true, false] {
        let start = Instant::now();
        let (got, instructions) = sley_fps(&s, &fids, closure);
        let elapsed = start.elapsed();
        let got = got.expect("fingerprints");
        assert_eq!(got.len(), expected.len());
        for (i, (g, e)) in got.iter().zip(&expected).enumerate() {
            assert_eq!(g, e, "{entry}: function {i} ({:?}) differs", fids[i]);
        }
        eprintln!(
            "{entry}: {} functions, {operations} operations, {} objects; {}: {instructions} \
             instructions total, {} beyond the drv_load+fp_owned baseline of {baseline}; {elapsed:?}",
            fids.len(),
            s.objects.len(),
            if closure { "fp_closure" } else { "fp_function x n" },
            instructions - baseline,
        );
    }
}

#[test]
fn closure_fingerprints_match_the_gate_cd_object() {
    check_closure("cd_object");
}

#[test]
fn closure_fingerprints_match_the_gate_drv_load() {
    check_closure("drv_load");
}

/// Types naming the enclosing function encode it as `u32(2)` (and as an
/// ordinary reference everywhere else, through the shared memo).
#[test]
fn self_references_match_native() {
    use sley_ssmc::{FunctionType, NamedType};
    let (program, names) = program();
    let entry = names["cd_object"];
    let mut pruned = canonical::prune_to_closure(program, entry);
    let a = pruned.functions[1].entity_id;
    let b = pruned.functions[2].entity_id;
    let other = EntityId::from_bytes([0x11; 32]);
    let mut effects = vec![a, other];
    effects.sort();
    let named = TypeExpr::Named(NamedType {
        definition: a,
        arguments: vec![TypeExpr::AdapterHandle(a)],
    });
    let weird = TypeExpr::Tuple(vec![
        named.clone(),
        TypeExpr::FunctionRef(FunctionType {
            parameters: vec![named, TypeExpr::CapabilityToken(other)],
            result: Box::new(TypeExpr::Option(Box::new(TypeExpr::TypeParameter(3)))),
            effects,
        }),
    ]);
    pruned.functions[1].result_type = weird.clone();
    pruned.functions[2].result_type = weird;
    let s = canonical::build(&pruned, entry, "fp-test");
    let expected = vec![
        native_fingerprint(&pruned, &s, a).unwrap(),
        native_fingerprint(&pruned, &s, b).unwrap(),
    ];
    assert_ne!(expected[0], expected[1]);
    for closure in [true, false] {
        let (got, _) = sley_fps(&s, &[a, b], closure);
        assert_eq!(got.unwrap(), expected);
        let (got, _) = sley_fps(&s, &[b, a], closure);
        assert_eq!(got.unwrap(), vec![expected[1].clone(), expected[0].clone()]);
    }
}

#[test]
fn refusals_match_native() {
    let (program, names) = program();
    let entry = names["cd_object"];
    let pruned = canonical::prune_to_closure(program, entry);

    // Not a function: the gate's ImmediateMismatch.
    let s = canonical::build(&pruned, entry, "fp-test");
    let (got, _) = sley_fps(&s, &[s.entry_point], true);
    assert_eq!(got, Err(FP_FUNCTION_MISSING));

    // An unlisted parameter claiming a function.
    let ptarget = pruned
        .functions
        .iter()
        .find(|f| f.entity_id != entry && !f.parameters.is_empty())
        .unwrap()
        .entity_id;
    let target = ptarget;
    let mut extra = pruned.clone();
    extra.parameters.push(Parameter {
        entity_id: EntityId::from_bytes([0xee; 32]),
        owner: target,
        role: ParameterRole::Function,
        ordinal: 99,
        value_type: TypeExpr::Bool,
    });
    let s = canonical::build(&extra, entry, "fp-test");
    assert_eq!(native_fingerprint(&extra, &s, target), Err(FP_INVENTORY_INVALID));
    let (got, _) = sley_fps(&s, &[target], true);
    assert_eq!(got, Err(FP_INVENTORY_INVALID));
    // fp_function sees only listed entities: it fingerprints as before.
    let (got, _) = sley_fps(&s, &[target], false);
    assert_eq!(got.unwrap()[0], native_fingerprint(&pruned, &s, target).unwrap());

    // A block that claims its function but is not listed.
    let (index, target) = pruned
        .functions
        .iter()
        .enumerate()
        .find(|(_, f)| f.blocks.len() >= 2)
        .map(|(i, f)| (i, f.entity_id))
        .unwrap();
    let mut unlisted = pruned.clone();
    unlisted.functions[index].blocks.pop();
    let s = canonical::build(&unlisted, entry, "fp-test");
    assert_eq!(native_fingerprint(&unlisted, &s, target), Err(FP_INVENTORY_INVALID));
    let (got, _) = sley_fps(&s, &[target], true);
    assert_eq!(got, Err(FP_INVENTORY_INVALID));

    // A listed parameter with the wrong ordinal.
    let target = ptarget;
    let mut ordinal = pruned.clone();
    let param = ordinal
        .parameters
        .iter_mut()
        .find(|p| p.owner == target && p.role == ParameterRole::Function)
        .unwrap();
    param.ordinal += 7;
    let s = canonical::build(&ordinal, entry, "fp-test");
    assert_eq!(native_fingerprint(&ordinal, &s, target), Err(FP_INVENTORY_INVALID));
    for closure in [true, false] {
        let (got, _) = sley_fps(&s, &[target], closure);
        assert_eq!(got, Err(FP_INVENTORY_INVALID));
    }
}

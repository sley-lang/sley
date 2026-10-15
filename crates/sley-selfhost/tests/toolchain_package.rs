//! `pk_*`: the Sley `EXEC_PACKAGE_V3` envelope against the native encoder.
mod common;
use common::*;
use sley_id::EntityId;
use sley_selfhost::canonical;
use sley_selfhost::native;
use sley_selfhost::native::Runner;
use sley_selfhost::seed::Program;
use sley_selfhost::seed::t;
use sley_selfhost::toolchain::package::PK_MALFORMED;
use sley_ssmc::{ConstData, ConstValue, TypeExpr};
use sley_vm::{ExecutionLimits, ExecutionTermination};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

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
        let mut guard = RUNNERS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
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

fn vec_of(element: TypeExpr, items: Vec<ConstValue>) -> ConstValue {
    ConstValue {
        value_type: t::vec(element),
        data: ConstData::Sequence(items),
    }
}

fn boolv(value: bool) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bool,
        data: ConstData::Bool(value),
    }
}

/// How a run varies the reference package.
#[derive(Clone, Copy)]
enum Case {
    /// The reference package as built.
    Exact,
    /// The image replaced by a multi-MiB byte string (sized entry point).
    LargeImage,
    /// A gate operation count above `u32` (must refuse).
    WideGateCount,
}

fn check_envelope(entry_name: &str, limits: ExecutionLimits, sized: bool, case: Case) {
    let (program, names) = program();
    let entry = names[entry_name];
    let pruned = canonical::prune_to_closure(program, entry);
    let s = canonical::build(&pruned, entry, "pk-test");
    // The native seed build: S decoded in inventory order, lowered, judged
    // by the V3 gate, packaged with `encode_package_envelope_v3`.
    let reference = native::c0_build(&s, limits).expect("native C0 build");
    let package = &reference.package;
    assert_eq!(package.entry, entry);
    // Independent cross-check of the reference inventory order.
    let mut constants = pruned.constants.clone();
    constants.sort_by_key(|c| c.entity_id);
    assert_eq!(package.constants, constants);
    let mut package = package.clone();
    if let Case::LargeImage = case {
        let mut large = Vec::new();
        while large.len() < 3 * 1_048_576 {
            large.extend_from_slice(&package.image_bytes);
        }
        large.extend_from_slice(b"tail");
        package.image_bytes = large;
    }
    let expected = &sley_vm::encode_package_envelope_v3(&package).expect("native envelope");
    let digest = sley_vm::package_digests_v3(&package)
        .expect("digests")
        .package_digest;
    if let Case::Exact = case {
        assert_eq!(*expected, reference.envelope);
    }
    let image = &package.image_bytes;
    let fps = &package.gate_closure_fingerprints;
    let gate_ops = match case {
        Case::WideGateCount => 1_u64 << 32,
        _ => u64::from(package.gate_operation_count),
    };

    let start = Instant::now();
    let (value, instructions) = run_counted(
        "t_pk_envelope",
        vec![
            objects_input(&s),
            bytes(s.epoch.as_bytes()),
            bytes(image),
            u64v(image.len() as u64),
            boolv(sized),
            bytes(entry.as_bytes()),
            bytes(s.root.root.as_bytes()),
            vec_of(
                t::u64(),
                vec![
                    u64v(limits.max_instructions),
                    u64v(limits.max_fuel),
                    u64v(limits.max_value_units),
                    u64v(limits.max_output_units),
                    u64v(limits.cancel_at_fuel.unwrap_or(0)),
                ],
            ),
            boolv(limits.cancel_at_fuel.is_some()),
            u64v(gate_ops),
            u64v(u64::from(package.gate_bridge_uses)),
            vec_of(
                TypeExpr::Bytes,
                fps.iter().map(|fp| bytes(fp.as_bytes())).collect(),
            ),
        ],
    );
    let elapsed = start.elapsed();
    if let Case::WideGateCount = case {
        assert_eq!(result(value), Err(PK_MALFORMED));
        return;
    }
    let items = tuple_items(&result(value).expect("envelope"));
    let got = as_bytes(&items[0]);
    if got != *expected {
        let first = got.iter().zip(expected.iter()).position(|(a, b)| a != b);
        panic!(
            "{entry_name}: envelope differs (sley {} bytes, native {} bytes, first difference at {first:?})",
            got.len(),
            expected.len()
        );
    }
    assert_eq!(
        as_bytes(&items[1]),
        digest.to_vec(),
        "{entry_name}: package digest"
    );
    eprintln!(
        "{entry_name}: envelope {} bytes (image {}, {} constants, {} layouts, {} imports, {} \
         fingerprints); {instructions} instructions (drv_load included), {elapsed:?}",
        expected.len(),
        image.len(),
        package.constants.len(),
        package.type_definitions.len(),
        package.imports.len(),
        package.gate_closure_fingerprints.len(),
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn envelope_matches_native_cd_object() {
    let limits = ExecutionLimits {
        max_instructions: 1_000_000,
        max_fuel: 2_000_000_000,
        max_value_units: 300,
        max_output_units: 0,
        cancel_at_fuel: None,
    };
    check_envelope("cd_object", limits, false, Case::Exact);
    check_envelope("cd_object", limits, true, Case::WideGateCount);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn envelope_matches_native_drv_load() {
    let limits = ExecutionLimits {
        max_instructions: u64::MAX,
        max_fuel: 10_000_000_000,
        max_value_units: 100_000_000_000_000,
        max_output_units: 10_000_000_000,
        cancel_at_fuel: Some(77_777),
    };
    check_envelope("drv_load", limits, true, Case::Exact);
    check_envelope("drv_load", limits, true, Case::LargeImage);
}

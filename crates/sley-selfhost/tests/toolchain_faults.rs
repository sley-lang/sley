//! Fault seeding (REWEAVE §15): deliberately defective toolchains must be
//! detected by the differential corpus. Each case builds the toolchain with
//! one fault seed active, runs it on a case where the fault matters, and
//! requires the result to differ from the native reference.
mod common;
use common::*;
use sley_mutate::value::{EntityBodyValue, EntityIdSet, WorkspaceBody};
use sley_selfhost::canonical::{self, CanonicalS};
use sley_selfhost::native::{self, Runner};
use sley_selfhost::seed::*;
use sley_selfhost::toolchain;
use sley_ssmc::{ConstData, ConstValue, OperationResultRef, TypeExpr, ValueRef};
use sley_vm::ExecutionTermination;

fn limits_value() -> ConstValue {
    let l = native::toolchain_limits();
    ConstValue {
        value_type: t::tuple(vec![
            t::u64(),
            t::u64(),
            t::u64(),
            t::u64(),
            t::bool(),
            t::u64(),
        ]),
        data: ConstData::Sequence(vec![
            u64v(l.max_instructions),
            u64v(l.max_fuel),
            u64v(l.max_value_units),
            u64v(l.max_output_units),
            ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(false),
            },
            u64v(0),
        ]),
    }
}

/// Runs `drv_build` of a toolchain built with `faults` on S.
fn sley_build(faults: &[&str], s: &CanonicalS) -> Result<Vec<u8>, u32> {
    let (program, names) = with_faults(faults, || toolchain::module().finish());
    let runner = Runner::new(&program, names["drv_build"]);
    let outcome = runner
        .run(
            vec![
                objects_input(s),
                bytes(&s.root.stored_bytes),
                bytes(s.epoch.as_bytes()),
                limits_value(),
            ],
            native::big_limits(),
        )
        .expect("executes");
    match outcome.termination {
        ExecutionTermination::Success(value) => result(value).map(|v| as_bytes(&v)),
        other => panic!("{other:?}"),
    }
}

fn small_program() -> (Program, sley_id::EntityId) {
    let mut m = Module::new("fault-small");
    m.func(
        "main",
        &[("c", t::bool()), ("n", t::u64())],
        t::u64(),
        |f| {
            f.var("out", u64c(0));
            f.if_else(
                v("c"),
                |f| {
                    f.let_("a", add(v("n"), u64c(2)));
                    f.set("out", v("a"));
                },
                |f| {
                    f.let_("b", add(v("n"), u64c(3)));
                    f.set("out", v("b"));
                },
            );
            f.ret(v("out"));
        },
    );
    let (program, names) = m.finish();
    (program, names["main"])
}

fn build(program: &Program, entry: sley_id::EntityId) -> CanonicalS {
    canonical::build(program, entry, "fault-seed")
}

fn native_result(s: &CanonicalS) -> Result<Vec<u8>, String> {
    native::c0_build(s, native::toolchain_limits()).map(|b| b.envelope)
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn reference_toolchain_agrees_on_the_fault_corpus() {
    let (program, entry) = small_program();
    let s = build(&program, entry);
    assert_eq!(
        sley_build(&[], &s).expect("builds"),
        native_result(&s).unwrap()
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn lowering_fault_is_detected() {
    let (program, entry) = small_program();
    let s = build(&program, entry);
    let sley = sley_build(&["lower-register"], &s);
    assert_ne!(
        sley.ok(),
        native_result(&s).ok(),
        "faulty lowering went unnoticed"
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn package_digest_fault_is_detected() {
    let (program, entry) = small_program();
    let s = build(&program, entry);
    let sley = sley_build(&["package-digest"], &s);
    assert_ne!(
        sley.ok(),
        native_result(&s).ok(),
        "faulty package digest went unnoticed"
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn incorrect_type_rule_is_detected() {
    // `vector_len` returns UInt(64); declare it (and the function result)
    // as UInt(32). The graph and edges stay consistent; only the operation
    // signature is wrong.
    let mut m = Module::new("fault-type-rule");
    m.func("main", &[("xs", t::vec(t::u8()))], t::u64(), |f| {
        f.ret(vlen(v("xs")));
    });
    let (mut program, names) = m.finish();
    let entry = names["main"];
    program
        .operations
        .iter_mut()
        .find(|op| op.opcode == sley_ssmc::Opcode::VectorLen)
        .unwrap()
        .result_types = vec![t::u32()];
    program.functions[0].result_type = t::u32();
    let s = build(&program, entry);
    assert!(
        native_result(&s).is_err(),
        "native refuses the mistyped operation"
    );
    assert_eq!(
        sley_build(&[], &s),
        Err(26_002),
        "reference toolchain reports the signature"
    );
    assert!(
        sley_build(&["judge-declared"], &s).is_ok(),
        "the faulty type rule accepts the mistyped program, so parity detects it"
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn disabled_dominance_check_is_detected() {
    let (mut program, entry) = small_program();
    // Make the else arm's addition read the then arm's constant.
    let adds: Vec<usize> = program
        .operations
        .iter()
        .enumerate()
        .filter(|(_, op)| op.opcode == sley_ssmc::Opcode::IntAddChecked)
        .map(|(i, _)| i)
        .collect();
    let then_block = program.operations[adds[0]].block;
    let foreign = program
        .operations
        .iter()
        .find(|op| op.block == then_block && op.opcode == sley_ssmc::Opcode::ConstantRef)
        .unwrap()
        .entity_id;
    program.operations[adds[1]].operands[1] = ValueRef::OperationResult(OperationResultRef {
        operation: foreign,
        result_index: 0,
    });
    let s = build(&program, entry);
    assert!(
        native_result(&s).is_err(),
        "native refuses the dominance violation"
    );
    assert_eq!(
        sley_build(&[], &s),
        Err(22_016),
        "reference toolchain reports CFG_DOMINANCE"
    );
    assert_ne!(
        sley_build(&["check-dominance"], &s),
        Err(22_016),
        "the disabled check misses the violation, so parity detects it"
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn disabled_canonical_checks_are_detected() {
    // Magic check: a corrupt object refuses natively with SCB_MAGIC_INVALID.
    let (program, entry) = small_program();
    let mut s = build(&program, entry);
    let mut bytes0 = s.objects[0].stored_bytes().to_vec();
    bytes0[0] ^= 0xff;
    let native_code = sley_mutate::import_entity_object(s.epoch, &bytes0)
        .map(|_| ())
        .map_err(|e| toolchain::codes::scb_code(e.code()));
    assert_eq!(native_code, Err(10_000));
    let corrupt = |s: &mut CanonicalS| {
        // Swap in the corrupt bytes (the entity object type holds them).
        let mut objects = objects_input(s);
        if let ConstData::Sequence(items) = &mut objects.data {
            items[0] = bytes(&bytes0);
        }
        objects
    };
    let objects = corrupt(&mut s);
    let run = |faults: &[&str]| {
        let (program, names) = with_faults(faults, || toolchain::module().finish());
        let runner = Runner::new(&program, names["drv_build"]);
        let outcome = runner
            .run(
                vec![
                    objects.clone(),
                    bytes(&s.root.stored_bytes),
                    bytes(s.epoch.as_bytes()),
                    limits_value(),
                ],
                native::big_limits(),
            )
            .unwrap();
        match outcome.termination {
            ExecutionTermination::Success(value) => result(value).map(|_| ()),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(run(&[]), Err(10_000));
    assert_ne!(
        run(&["codec-magic"]),
        Err(10_000),
        "skipped magic check detected"
    );

    // Set order: a workspace whose package set is out of order.
    let a = sley_id::EntityId::from_bytes([1; 32]);
    let b = sley_id::EntityId::from_bytes([2; 32]);
    let object = sley_mutate::build_entity_object(
        s.epoch,
        &sley_mutate::EntityObjectRecord {
            entity_id: sley_id::EntityId::from_bytes([9; 32]),
            body: EntityBodyValue::Workspace(WorkspaceBody {
                packages: EntityIdSet::from_unsorted(vec![a, b]).unwrap(),
                root_namespace: a,
                capability_requirements: EntityIdSet::from_unsorted(vec![]).unwrap(),
                contracts: EntityIdSet::from_unsorted(vec![]).unwrap(),
                tests: EntityIdSet::from_unsorted(vec![]).unwrap(),
            }),
            label: None,
            semantic_fingerprint: None,
        },
    )
    .unwrap();
    let mut stored = object.stored_bytes().to_vec();
    let pa = stored.windows(32).position(|w| w == a.as_bytes()).unwrap();
    let pb = pa + 33; // [len 0x20][a 32][len 0x20][b 32]
    assert_eq!(&stored[pb..pb + 32], b.as_bytes());
    stored[pa..pa + 32].copy_from_slice(b.as_bytes());
    stored[pb..pb + 32].copy_from_slice(a.as_bytes());
    let n = stored.len();
    let digest = blake3::Hasher::new()
        .update(b"sley2.object.v1")
        .update(&stored[..n - 32])
        .finalize();
    stored[n - 32..].copy_from_slice(digest.as_bytes());
    let native = sley_mutate::import_entity_object(s.epoch, &stored)
        .map(|_| ())
        .map_err(|e| toolchain::codes::scb_code(e.code()));
    assert_eq!(native, Err(10_018));
    let decode = |faults: &[&str]| {
        let (program, names) = with_faults(faults, || toolchain::module().finish());
        let runner = Runner::new(&program, names["cd_object"]);
        let outcome = runner
            .run(
                vec![bytes(&stored), bytes(s.epoch.as_bytes())],
                native::big_limits(),
            )
            .unwrap();
        match outcome.termination {
            ExecutionTermination::Success(value) => result(value).map(|_| ()),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(decode(&[]), Err(10_018));
    assert!(
        decode(&["codec-set-order"]).is_ok(),
        "skipped order check detected"
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "release-only: cargo test --release -p sley-selfhost"
)]
fn substituted_input_with_disabled_binding_check_is_detected() {
    let (program, entry) = small_program();
    let mut s = build(&program, entry);
    // Substitute one constant object: same entity, different body.
    let index = s
        .objects
        .iter()
        .position(|o| matches!(o.record().body, EntityBodyValue::Constant(_)))
        .unwrap();
    let mut record = s.objects[index].record().clone();
    record.body = EntityBodyValue::Constant(sley_mutate::value::ConstantBody {
        value: ConstValue {
            value_type: t::u64(),
            data: ConstData::UInt(41),
        },
    });
    s.objects[index] = sley_mutate::build_entity_object(s.epoch, &record).unwrap();
    assert!(
        native_result(&s).is_err(),
        "native refuses the unbound object"
    );
    assert_eq!(
        sley_build(&[], &s),
        Err(29_002),
        "reference toolchain refuses it"
    );
    assert!(
        sley_build(&["driver-binding"], &s).is_ok(),
        "the disabled binding check accepts it"
    );
}

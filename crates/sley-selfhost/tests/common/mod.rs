//! Shared helpers for toolchain tests.
#![allow(dead_code)]

use sley_id::EntityId;
use sley_selfhost::canonical::{self, CanonicalS};
use sley_selfhost::native::{Runner, big_limits, execute};
use sley_selfhost::seed::{Program, t};
use sley_selfhost::toolchain;
use sley_ssmc::{ConstData, ConstValue, ResultConst, TypeExpr};
use sley_vm::ExecutionTermination;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The complete toolchain module (all functions, including test entries).
pub fn program() -> &'static (Program, HashMap<String, EntityId>) {
    static P: OnceLock<(Program, HashMap<String, EntityId>)> = OnceLock::new();
    P.get_or_init(|| toolchain::module().finish())
}

/// One cached runner per entry (lowering happens once per entry).
fn runner(entry: &str) -> &'static Runner<'static> {
    static RUNNERS: OnceLock<Mutex<HashMap<String, &'static Runner<'static>>>> = OnceLock::new();
    let map = RUNNERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap();
    if let Some(found) = guard.get(entry) {
        return found;
    }
    let (program, names) = program();
    let created: &'static Runner<'static> = Box::leak(Box::new(Runner::new(program, names[entry])));
    guard.insert(entry.to_owned(), created);
    created
}

/// Runs one toolchain function natively and returns its value.
pub fn run(entry: &str, inputs: Vec<ConstValue>) -> ConstValue {
    run_counted(entry, inputs).0
}

/// Runs and reports (value, instruction count).
pub fn run_counted(entry: &str, inputs: Vec<ConstValue>) -> (ConstValue, u64) {
    let outcome = runner(entry).run(inputs, big_limits()).expect("executes");
    match outcome.termination {
        ExecutionTermination::Success(value) => (value, outcome.instruction_count),
        other => panic!("{entry}: {other:?}"),
    }
}

/// Runs through the full lowering path (no cache).
pub fn run_uncached(entry: &str, inputs: Vec<ConstValue>) -> ConstValue {
    let (program, names) = program();
    let outcome = execute(program, names[entry], inputs, big_limits()).expect("executes");
    match outcome.termination {
        ExecutionTermination::Success(value) => value,
        other => panic!("{entry}: {other:?}"),
    }
}

/// Unpacks `Result<T, UInt(32)>`.
pub fn result(value: ConstValue) -> Result<ConstValue, u32> {
    match value.data {
        ConstData::Result(ResultConst::Ok(inner)) => Ok(*inner),
        ConstData::Result(ResultConst::Err(code)) => match code.data {
            ConstData::UInt(code) => Err(u32::try_from(code).unwrap()),
            _ => panic!("error code is not an integer"),
        },
        other => panic!("not a Result: {other:?}"),
    }
}

pub fn bytes(value: &[u8]) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(value.to_vec()),
    }
}

pub fn u64v(value: u64) -> ConstValue {
    ConstValue {
        value_type: t::u64(),
        data: ConstData::UInt(u128::from(value)),
    }
}

pub fn as_u64(value: &ConstValue) -> u64 {
    match value.data {
        ConstData::UInt(x) => u64::try_from(x).unwrap(),
        _ => panic!("not an integer: {value:?}"),
    }
}

pub fn as_bytes(value: &ConstValue) -> Vec<u8> {
    match &value.data {
        ConstData::Bytes(b) => b.clone(),
        _ => panic!("not bytes: {value:?}"),
    }
}

pub fn tuple_items(value: &ConstValue) -> Vec<ConstValue> {
    match &value.data {
        ConstData::Sequence(items) => items.clone(),
        _ => panic!("not a tuple: {value:?}"),
    }
}

/// The objects of `s` as the `Vector<Bytes>` driver input.
pub fn objects_input(s: &CanonicalS) -> ConstValue {
    ConstValue {
        value_type: t::vec(TypeExpr::Bytes),
        data: ConstData::Sequence(
            s.objects
                .iter()
                .map(|object| bytes(object.stored_bytes()))
                .collect(),
        ),
    }
}

/// Canonical S for the full toolchain program with `entry` exposed.
pub fn toolchain_s(entry: &str) -> CanonicalS {
    let (program, names) = program();
    canonical::build(program, names[entry], "toolchain-test")
}

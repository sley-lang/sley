//! The committed Rust bindings of `tests/fixtures/bindgen/` compile, bind
//! their program and call it with Rust types (`tests/bindgen.rs` keeps
//! them equal to what `embed::bindgen::rust` generates).
#![cfg(feature = "familiar")]

use sley_agent::embed::{CallResult, EmbedOptions, compile_python};

mod accounts {
    include!("fixtures/bindgen/accounts.rs");
}

// A private module: the methods this test does not call are dead code here.
#[allow(dead_code)]
mod points {
    include!("fixtures/bindgen/points.rs");
}

const FUEL: u64 = 1_000_000;

fn bound<T>(source: &str, bind: impl FnOnce(sley_agent::embed::Module) -> T) -> T {
    bind(compile_python(source, &EmbedOptions::default()).unwrap())
}

#[test]
fn records_enums_and_results_cross_with_rust_types() {
    use accounts::{Account, ArithmeticError, Program, WithdrawError};
    let mut program = bound(include_str!("fixtures/bindgen/accounts.txt"), |module| {
        Program::new(module).unwrap()
    });
    let ada = Account {
        owner: "ada".to_owned(),
        balance: 10,
        kind: 2,
    };
    let paid = program.withdraw(&ada, 4, FUEL).unwrap();
    assert_eq!(
        paid.result,
        CallResult::Value(Ok(Account {
            balance: 6,
            ..ada.clone()
        }))
    );
    assert!(paid.fuel_used > 0 && paid.instructions > 0);
    assert_eq!(
        program.withdraw(&ada, 40, FUEL).unwrap().result,
        CallResult::Value(Err(WithdrawError::Insufficient))
    );
    assert_eq!(
        program.withdraw(&ada, 0, FUEL).unwrap().result,
        CallResult::Value(Err(WithdrawError::NotPositive))
    );
    assert_eq!(
        program
            .total(&[ada.clone(), ada.clone()], FUEL)
            .unwrap()
            .result,
        CallResult::Value(20)
    );
    assert_eq!(
        program.product(&[i64::MAX, 2], FUEL).unwrap().result,
        CallResult::Value(Err(ArithmeticError::Overflow))
    );
    assert_eq!(
        program.owns(&ada, "ada", FUEL).unwrap().result,
        CallResult::Value(true)
    );
    // Limits and their counts carry over unchanged.
    let starved = program.total(&[ada], 3).unwrap();
    assert_eq!(starved.result, CallResult::OutOfFuel);
    assert_eq!(starved.fuel_used, 3);
}

#[test]
fn python_list_results_are_vectors() {
    use points::{Point, Program};
    let mut program = bound(include_str!("fixtures/bindgen/points.py"), |module| {
        Program::new(module).unwrap()
    });
    let shifted = program
        .shift(&[Point { x: 1, y: 2 }, Point { x: -3, y: 4 }], 10, FUEL)
        .unwrap();
    assert_eq!(
        shifted.result,
        CallResult::Value(vec![Point { x: 11, y: 2 }, Point { x: 7, y: 4 }])
    );
    assert_eq!(
        program.norm1(&Point { x: -3, y: 4 }, FUEL).unwrap().result,
        CallResult::Value(7)
    );
}

#[test]
fn bindings_refuse_a_program_with_another_interface() {
    // `withdraw` takes a u64 amount here: the bindings' i64 would not fit.
    let other = include_str!("fixtures/bindgen/accounts.txt")
        .replace("amount: i64", "amount: u64")
        .replace("amount <= 0", "amount == 0u64")
        .replace("amount > a.balance", "to(i64, amount) > a.balance")
        .replace("a.balance - amount", "a.balance - to(i64, amount)");
    let error = bound(&other, |module| accounts::Program::new(module).unwrap_err());
    assert_eq!(error.code, "AGENT_INPUT_INVALID");
    assert!(
        error.message.contains("withdraw(a: Account, amount: i64)"),
        "{error}"
    );
    // A program with more functions binds: the bindings use a subset.
    let more = format!(
        "{}fn spare() -> i64 {{\n    return 1\n}}\n",
        include_str!("fixtures/bindgen/accounts.txt")
    );
    bound(&more, |module| accounts::Program::new(module).unwrap());
}

#[test]
fn integers_convert_exactly_or_are_refused() {
    use sley_agent::embed::{EmbedValue, FromEmbed, ToEmbed};
    assert_eq!(u8::MAX.to_embed().unwrap(), EmbedValue::Int(255));
    assert_eq!(u8::from_embed(&EmbedValue::Int(256)), None);
    assert_eq!(i8::from_embed(&EmbedValue::Int(-128)), Some(i8::MIN));
    let error = u64::MAX.to_embed().unwrap_err();
    assert_eq!(error.code, "AGENT_INPUT_INVALID");
    assert!(error.message.contains("18446744073709551615"), "{error}");
    assert!(i128::MIN.to_embed().is_err());
    assert_eq!(u64::from_embed(&EmbedValue::Int(-1)), None);
}

#[test]
fn the_bound_module_stays_reachable() {
    use accounts::Program;
    let mut program = bound(include_str!("fixtures/bindgen/accounts.txt"), |module| {
        Program::new(module).unwrap()
    });
    assert!(program.module().signature("owns").is_some());
    let module = program.into_module();
    assert!(Program::new(module).is_ok());
}

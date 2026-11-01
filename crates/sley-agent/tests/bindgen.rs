//! Generated bindings (`embed::bindgen`) and the JSON calling convention
//! (`Module::call_json`). The generated text is committed under
//! `tests/fixtures/bindgen/`; `SLEY_BLESS=1 cargo test --test bindgen`
//! rewrites it. `tests/bindgen_generated.rs` compiles and calls the Rust.
#![cfg(feature = "familiar")]

use std::path::PathBuf;

use serde_json::{Value, json};
use sley_agent::embed::{EmbedOptions, Module, bindgen, compile_python};

const ACCOUNTS: &str = include_str!("fixtures/bindgen/accounts.txt");
const POINTS: &str = include_str!("fixtures/bindgen/points.py");
/// Names that clash with the generated code, with reserved words, and with
/// each other once suffixed.
const NAMES: &str = include_str!("fixtures/bindgen/names.txt");

fn module(source: &str) -> Module {
    compile_python(source, &EmbedOptions::default()).unwrap_or_else(|error| panic!("{error}"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bindgen")
        .join(name)
}

/// The committed text equals the generated text.
fn golden(name: &str, generated: &str) {
    let path = fixture(name);
    if std::env::var_os("SLEY_BLESS").is_some() {
        std::fs::write(&path, generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == generated,
        "{} differs from the generated text (SLEY_BLESS=1 rewrites it):\n{generated}",
        path.display()
    );
}

#[test]
fn generated_bindings_are_committed() {
    let accounts = module(ACCOUNTS);
    let points = module(POINTS);
    golden("accounts.rs", &bindgen::rust(&accounts));
    golden("accounts.ts", &bindgen::typescript(&accounts));
    golden("points.rs", &bindgen::rust(&points));
    golden("points.ts", &bindgen::typescript(&points));
    let names = module(NAMES);
    golden("names.rs", &bindgen::rust(&names));
    golden("names.ts", &bindgen::typescript(&names));
}

#[test]
fn generation_is_deterministic() {
    let seeded = compile_python(
        ACCOUNTS,
        &EmbedOptions {
            seed: 99,
            now_unix_millis: 5,
        },
    )
    .unwrap();
    // A different seed gives a different program digest, not other text.
    assert_ne!(seeded.program_digest(), module(ACCOUNTS).program_digest());
    assert_eq!(bindgen::rust(&seeded), bindgen::rust(&module(ACCOUNTS)));
    assert_eq!(
        bindgen::typescript(&seeded),
        bindgen::typescript(&module(ACCOUNTS))
    );
}

#[test]
fn signatures_and_types_print_as_sley_spells_them() {
    let accounts = module(ACCOUNTS);
    assert_eq!(
        accounts.signature("withdraw").unwrap().to_string(),
        "withdraw(a: Account, amount: i64) -> Result<Account, WithdrawError>"
    );
    assert_eq!(
        accounts.signature("total").unwrap().to_string(),
        "total(xs: list[Account]) -> i64"
    );
    let types: Vec<String> = accounts.types().iter().map(ToString::to_string).collect();
    assert_eq!(
        types,
        [
            "record Account { owner: str, balance: i64, kind: u8 }",
            "variant ArithmeticError { Overflow, DivideByZero, InvalidShift }",
            "variant WithdrawError { NotPositive, Insufficient }",
        ]
    );
}

#[test]
fn check_interface_names_what_differs() {
    let accounts = module(ACCOUNTS);
    accounts
        .check_interface(
            &["total(xs: list[Account]) -> i64"],
            &["variant WithdrawError { NotPositive, Insufficient }"],
        )
        .unwrap();
    let error = accounts
        .check_interface(&["total(xs: list[i64]) -> i64"], &[])
        .unwrap_err();
    assert_eq!(error.code, "AGENT_INPUT_INVALID");
    assert!(
        error.message.contains("total(xs: list[Account]) -> i64"),
        "{error}"
    );
    let error = accounts
        .check_interface(&["gone() -> i64"], &[])
        .unwrap_err();
    assert!(error.message.contains("no such function"), "{error}");
    let error = accounts
        .check_interface(&[], &["variant WithdrawError { NotPositive }"])
        .unwrap_err();
    assert!(error.message.contains("does not declare"), "{error}");
}

fn reply(module: &mut Module, function: &str, args: &Value) -> Value {
    let text = module
        .call_json(function, &args.to_string(), 1_000_000)
        .unwrap_or_else(|error| panic!("{error}"));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn call_json_takes_and_gives_the_call_form() {
    let mut accounts = module(ACCOUNTS);
    let ada = json!({"owner": "ada", "balance": 10, "kind": 2});
    let ok = reply(&mut accounts, "withdraw", &json!([ada, 4]));
    assert_eq!(
        ok["result"],
        json!({"value": {"Ok": {"owner": "ada", "balance": 6, "kind": 2}}})
    );
    assert!(ok["fuel_used"].as_u64().unwrap() > 0);
    assert!(ok["instructions"].as_u64().unwrap() > 0);
    // Fields in any order; a variant case as its name.
    let swapped = json!({"kind": 2, "balance": 10, "owner": "ada"});
    let err = reply(&mut accounts, "withdraw", &json!([swapped, 40]));
    assert_eq!(err["result"], json!({"value": {"Err": "Insufficient"}}));
    // A built-in failure, an integer given as a decimal string.
    let overflow = reply(
        &mut accounts,
        "product",
        &json!([["9223372036854775807", 2]]),
    );
    assert_eq!(
        overflow["result"],
        json!({"value": {"Err": {"ArithmeticError": "Overflow"}}})
    );
    let fuel = accounts
        .call_json("total", &json!([[ada, ada]]).to_string(), 3)
        .unwrap();
    let fuel: Value = serde_json::from_str(&fuel).unwrap();
    assert_eq!(fuel["result"], json!({"limit": "Fuel"}));
    assert_eq!(fuel["fuel_used"], 3);
    // A Python list result is an array.
    let mut points = module(POINTS);
    let shifted = reply(&mut points, "shift", &json!([[{"x": 1, "y": 2}], 10]));
    assert_eq!(shifted["result"], json!({"value": [{"x": 11, "y": 2}]}));
}

#[test]
fn call_json_refuses_what_does_not_fit() {
    let mut accounts = module(ACCOUNTS);
    let refuse = |module: &mut Module, function: &str, args: &str| {
        module.call_json(function, args, 1_000).unwrap_err()
    };
    let error = refuse(&mut accounts, "nothing", "[]");
    assert_eq!(error.code, "AGENT_NAME_UNKNOWN");
    for (args, needle) in [
        ("[1", "not JSON"),
        ("{}", "JSON array"),
        ("[[1]]", "takes 2 argument"),
        (r#"[["x"]]"#, "arg 0"),
        ("[[1.5]]", "integer"),
    ] {
        let function = if args == "[[1]]" {
            "withdraw"
        } else {
            "product"
        };
        let error = refuse(&mut accounts, function, args);
        assert_eq!(error.code, "AGENT_INPUT_INVALID", "{args}: {error}");
        let error = error.to_string();
        assert!(error.contains(needle), "{args}: {error}");
    }
    let bad_field = json!([{"owner": "ada", "balance": 1, "kind": 1, "extra": 0}, 1]);
    let error = refuse(&mut accounts, "withdraw", &bad_field.to_string());
    assert!(error.message.contains("no field `extra`"), "{error}");
    let bad_case = json!([{"owner": "ada", "balance": 1, "kind": 300}, 1]);
    let error = refuse(&mut accounts, "withdraw", &bad_case.to_string());
    assert_eq!(error.code, "AGENT_INPUT_INVALID", "{error}");
}

#[cfg(feature = "native")]
fn cli(args: &[&str]) -> (i32, String) {
    let words: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let mut out = Vec::new();
    let status = sley_agent::cli::run(&words, &mut out);
    (status, String::from_utf8(out).unwrap())
}

#[cfg(feature = "native")]
#[test]
fn the_bindgen_command_prints_the_same_text() {
    let accounts = fixture("accounts.txt").display().to_string();
    let points = fixture("points.py").display().to_string();
    let (status, rust) = cli(&["bindgen", &points, "--rust"]);
    assert_eq!(status, 0, "{rust}");
    assert_eq!(rust, bindgen::rust(&module(POINTS)));
    let (status, typescript) = cli(&["bindgen", &accounts, "--typescript"]);
    assert_eq!(status, 0, "{typescript}");
    assert_eq!(typescript, bindgen::typescript(&module(ACCOUNTS)));
    let (status, json) = cli(&["--json", "bindgen", &points, "--typescript"]);
    assert_eq!(status, 0, "{json}");
    let json: Value = serde_json::from_str(json.trim()).unwrap();
    assert_eq!(json["language"], "typescript");
    assert_eq!(json["text"], bindgen::typescript(&module(POINTS)));
}

#[cfg(feature = "native")]
#[test]
fn the_bindgen_command_refuses_usage_and_text_errors() {
    let points = fixture("points.py").display().to_string();
    for args in [
        vec!["bindgen", points.as_str()],
        vec!["bindgen", points.as_str(), "--rust", "--typescript"],
        vec!["bindgen", "--rust"],
        vec!["bindgen", "no-such-file.py", "--rust"],
    ] {
        let (status, out) = cli(&args);
        assert_eq!(status, 2, "{args:?}: {out}");
        assert!(out.contains("AGENT_USAGE_INVALID"), "{args:?}: {out}");
    }
    let broken = std::env::temp_dir().join(format!("sley-bindgen-{}.py", std::process::id()));
    std::fs::write(&broken, "def f(x: int) -> int:\n    return x +\n").unwrap();
    let path = broken.display().to_string();
    let (status, out) = cli(&["bindgen", &path, "--rust"]);
    let _ = std::fs::remove_file(&broken);
    assert_eq!(status, 2, "{out}");
    assert!(out.contains("AGENT_FRAME_INVALID"), "{out}");
    assert!(out.contains(&format!("{path}:2:15:")), "{out}");
}

#[cfg(feature = "native")]
#[test]
fn call_json_prints_what_sley_agent_call_prints() {
    let dir = std::env::temp_dir().join(format!("sley-bindgen-call-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    sley_agent::genesis::init(&dir, None, sley_agent::genesis::INIT_CEILINGS).unwrap();
    let workspace = dir.display().to_string();
    let accounts = fixture("accounts.txt").display().to_string();
    let (status, out) = cli(&["--workspace", &workspace, "try", "--familiar", &accounts]);
    assert_eq!(status, 0, "{out}");
    let mut module = module(ACCOUNTS);
    for (function, args) in [
        ("product", json!([["9223372036854775807", 2]])),
        ("product", json!([[2, 3]])),
        (
            "withdraw",
            json!([{"owner": "ada", "balance": 10, "kind": 1}, 40]),
        ),
        (
            "withdraw",
            json!([{"owner": "ada", "balance": 10, "kind": 1}, 4]),
        ),
    ] {
        let mut words = vec![
            "--workspace".to_owned(),
            workspace.clone(),
            "--json".into(),
            "call".into(),
            function.into(),
        ];
        words.extend(args.as_array().unwrap().iter().map(Value::to_string));
        words.extend(["--on".to_owned(), "c1".to_owned()]);
        let mut out = Vec::new();
        assert_eq!(sley_agent::cli::run(&words, &mut out), 0);
        let called: Value = serde_json::from_slice(&out).unwrap();
        let replied = reply(&mut module, function, &args);
        assert_eq!(
            replied["result"]["value"], called["result"],
            "{function} {args}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

//! Structured function bodies: lowering to blocks, execution semantics, integer
//! conversions, refusals at authored body locations, and the help example.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sley_agent::genesis;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "sley-structured-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn workspace(label: &str) -> TempDir {
    let temp = TempDir::new(label);
    genesis::init(&temp.path, Some([7; 32]), genesis::INIT_CEILINGS).unwrap();
    temp
}

fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let mut words = vec!["--workspace".to_owned(), dir.display().to_string()];
    words.extend(args.iter().map(|arg| (*arg).to_owned()));
    let mut out = Vec::new();
    let status = sley_agent::cli::run(&words, &mut out);
    (status, String::from_utf8(out).unwrap())
}

fn run_json(dir: &Path, args: &[&str]) -> (i32, Value) {
    let mut words = vec!["--json"];
    words.extend_from_slice(args);
    let (status, text) = run(dir, &words);
    (
        status,
        serde_json::from_str(text.trim()).unwrap_or_else(|_| panic!("not JSON: {text}")),
    )
}

#[allow(clippy::needless_pass_by_value)] // frames are built inline at each call
fn function(name: &str, params: Value, returns: &str, body: Value) -> Value {
    json!({"af1": 1, "afx": 1, "fns": [{"fn": name, "params": params, "returns": returns, "body": body}]})
}

/// Tries `frame` and returns the candidate handle.
fn valid(dir: &Path, frame: &Value) -> String {
    let (status, report) = run_json(dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(status, 0, "{report:#}");
    report["handle"].as_str().unwrap().to_owned()
}

/// The JSON result of calling `name` with `args` on `handle`.
fn call(dir: &Path, handle: &str, name: &str, args: &[Value]) -> Value {
    let mut words: Vec<String> = vec!["call".into(), name.into()];
    words.extend(args.iter().map(Value::to_string));
    words.extend(["--on".into(), handle.into()]);
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    let (_, report) = run_json(dir, &refs);
    report.get("result").cloned().unwrap_or(report)
}

fn refusal(dir: &Path, frame: &Value) -> String {
    let (status, text) = run(dir, &["try", &frame.to_string(), "--no-test"]);
    assert_ne!(status, 0, "{text}");
    text
}

#[test]
fn a_fold_with_two_accumulators_runs_without_authored_blocks() {
    let temp = workspace("fold");
    let frame = function(
        "clamp_count",
        json!([["xs", "Vec<i64>"], ["lo", "i64"], ["hi", "i64"]]),
        "i64",
        json!([
            ["var", "sum", "i64", 0],
            ["var", "clamped", "i64", 0],
            [
                "for",
                "x",
                "xs",
                [
                    ["let", "c", ["clamp", "x", "lo", "hi"]],
                    [
                        "if",
                        ["ne", "c", "x"],
                        [["set", "clamped", ["add", "clamped", 1]]]
                    ],
                    ["set", "sum", ["add", "sum", "c"]]
                ]
            ],
            ["return", ["add", ["mul", "sum", 100], "clamped"]]
        ]),
    );
    let handle = valid(&temp.path, &frame);
    assert_eq!(
        call(
            &temp.path,
            &handle,
            "clamp_count",
            &[json!([]), json!(0), json!(5)]
        ),
        json!(0)
    );
    assert_eq!(
        call(
            &temp.path,
            &handle,
            "clamp_count",
            &[json!([-3, 2, 9]), json!(0), json!(5)]
        ),
        json!(702)
    );
    assert_eq!(
        call(
            &temp.path,
            &handle,
            "clamp_count",
            &[json!([4]), json!(3), json!(1)]
        ),
        json!(101)
    );
}

#[test]
fn checked_failures_trap_propagate_or_return_a_case() {
    let temp = workspace("failures");
    let max = json!(i64::MAX);
    let trap = function(
        "t",
        json!([["xs", "Vec<i64>"]]),
        "i64",
        json!([
            ["var", "s", "i64", 0],
            ["for", "x", "xs", [["set", "s", ["add", "s", "x"]]]],
            ["return", "s"]
        ]),
    );
    let handle = valid(&temp.path, &trap);
    assert_eq!(call(&temp.path, &handle, "t", &[json!([1, 2])]), json!(3));
    assert!(
        call(&temp.path, &handle, "t", &[json!([max, 1])])
            .to_string()
            .contains("trap")
    );
    let propagate = function(
        "p",
        json!([["xs", "Vec<i64>"]]),
        "Result<i64,ArithmeticError>",
        json!([
            ["var", "s", "i64", 0],
            ["for", "x", "xs", [["set", "s", ["add?", "s", "x"]]]],
            ["ok", "s"]
        ]),
    );
    let handle = valid(&temp.path, &propagate);
    assert_eq!(
        call(&temp.path, &handle, "p", &[json!([max, 1])]),
        json!({"Err": {"ArithmeticError": "Overflow"}})
    );
    let mut case = function(
        "c",
        json!([["a", "i64"], ["b", "i64"]]),
        "Result<i64,DivError>",
        json!([["ok", ["div?Zero", "a", "b"]]]),
    );
    case["types"] = json!([{"name": "DivError", "variant": ["Zero"]}]);
    let handle = valid(&temp.path, &case);
    assert_eq!(
        call(&temp.path, &handle, "c", &[json!(-7), json!(2)]),
        json!({"Ok": -3})
    );
    assert_eq!(
        call(&temp.path, &handle, "c", &[json!(1), json!(0)]),
        json!({"Err": "Zero"})
    );
}

#[test]
fn an_if_value_runs_only_the_chosen_arm() {
    let temp = workspace("lazy");
    let frame = function(
        "safe",
        json!([["a", "i64"], ["b", "i64"]]),
        "i64",
        json!([["return", ["if", ["eq", "b", 0], 0, ["div", "a", "b"]]]]),
    );
    let handle = valid(&temp.path, &frame);
    assert_eq!(
        call(&temp.path, &handle, "safe", &[json!(9), json!(0)]),
        json!(0)
    );
    assert_eq!(
        call(&temp.path, &handle, "safe", &[json!(9), json!(-2)]),
        json!(-4)
    );
}

#[test]
fn integer_conversions_are_exact_in_range_and_fail_outside_it() {
    let temp = workspace("to");
    let frame = function(
        "index_of",
        json!([["xs", "Vec<i64>"], ["target", "i64"]]),
        "i64",
        json!([
            ["var", "i", "u64", 0],
            [
                "while",
                ["lt", "i", ["len", "xs"]],
                [
                    [
                        "if",
                        ["eq", ["get", "xs", "i"], "target"],
                        [["return", ["to", "i64", "i"]]]
                    ],
                    ["set", "i", ["add", "i", 1]]
                ]
            ],
            ["return", -1]
        ]),
    );
    let handle = valid(&temp.path, &frame);
    assert_eq!(
        call(
            &temp.path,
            &handle,
            "index_of",
            &[json!([4, 5, 6]), json!(6)]
        ),
        json!(2)
    );
    assert_eq!(
        call(
            &temp.path,
            &handle,
            "index_of",
            &[json!([4, 5, 6]), json!(7)]
        ),
        json!(-1)
    );
    let narrow = function(
        "n",
        json!([["x", "u64"]]),
        "Result<i64,ArithmeticError>",
        json!([["ok", ["to?", "i64", "x"]]]),
    );
    let handle = valid(&temp.path, &narrow);
    assert_eq!(
        call(&temp.path, &handle, "n", &[json!(i64::MAX as u64)]),
        json!({"Ok": i64::MAX})
    );
    assert_eq!(
        call(&temp.path, &handle, "n", &[json!(1_u64 << 63)]),
        json!({"Err": {"ArithmeticError": "Overflow"}})
    );
    let signed = function(
        "s",
        json!([["x", "i64"]]),
        "Result<i32,ArithmeticError>",
        json!([["ok", ["to?", "i32", "x"]]]),
    );
    let handle = valid(&temp.path, &signed);
    assert_eq!(
        call(&temp.path, &handle, "s", &[json!(i32::MIN)]),
        json!({"Ok": i32::MIN})
    );
    assert_eq!(
        call(&temp.path, &handle, "s", &[json!(i64::from(i32::MIN) - 1)]),
        json!({"Err": {"ArithmeticError": "Overflow"}})
    );
    let unsigned = function(
        "u",
        json!([["x", "i64"]]),
        "Result<u64,ArithmeticError>",
        json!([["ok", ["to?", "u64", "x"]]]),
    );
    let handle = valid(&temp.path, &unsigned);
    assert_eq!(
        call(&temp.path, &handle, "u", &[json!(-1)]),
        json!({"Err": {"ArithmeticError": "Overflow"}})
    );
    assert_eq!(
        call(&temp.path, &handle, "u", &[json!(i64::MAX)]),
        json!({"Ok": i64::MAX})
    );
}

/// (function, source type, target type, rows of (argument, expected result)).
type ConversionCase = (
    &'static str,
    &'static str,
    &'static str,
    Vec<(Value, Value)>,
);

#[test]
#[allow(clippy::too_many_lines)] // one row per boundary
fn conversion_boundaries_hold_for_signed_and_unsigned_targets() {
    let temp = workspace("to-bounds");
    let overflow = json!({"Err": {"ArithmeticError": "Overflow"}});
    // (name, from, to, [(argument, expected)]) with `["ok", ["to?", to, x]]`;
    // results wider than i64 are converted back to check exactness.
    let cases: Vec<ConversionCase> = vec![
        (
            "u2i",
            "u64",
            "i64",
            vec![
                (json!(0), json!({"Ok": 0})),
                (json!(i64::MAX as u64), json!({"Ok": i64::MAX})),
                (json!(1_u64 << 63), overflow.clone()),
                (json!(u64::MAX), overflow.clone()),
            ],
        ),
        (
            "i2u",
            "i64",
            "u64",
            vec![
                (json!(0), json!({"Ok": 0})),
                (json!(-1), overflow.clone()),
                (json!(i64::MIN), overflow.clone()),
                (json!(i64::MAX), json!({"Ok": i64::MAX})),
            ],
        ),
        (
            "i2i8",
            "i64",
            "i8",
            vec![
                (json!(-128), json!({"Ok": -128})),
                (json!(-129), overflow.clone()),
                (json!(127), json!({"Ok": 127})),
                (json!(128), overflow.clone()),
                (json!(i64::MIN), overflow.clone()),
            ],
        ),
        (
            "i2i32",
            "i64",
            "i32",
            vec![
                (json!(i64::MIN), overflow.clone()),
                (json!(i32::MAX), json!({"Ok": i32::MAX})),
                (json!(i64::from(i32::MAX) + 1), overflow.clone()),
            ],
        ),
        (
            "u2u8",
            "u64",
            "u8",
            vec![
                (json!(255), json!({"Ok": 255})),
                (json!(256), overflow.clone()),
            ],
        ),
        (
            "u2u32",
            "u64",
            "u32",
            vec![
                (json!(u32::MAX), json!({"Ok": u32::MAX})),
                (json!(u64::MAX), overflow.clone()),
            ],
        ),
        (
            "i8u8",
            "i8",
            "u8",
            vec![
                (json!(-1), overflow.clone()),
                (json!(-128), overflow.clone()),
                (json!(127), json!({"Ok": 127})),
            ],
        ),
        (
            "u8i8",
            "u8",
            "i8",
            vec![
                (json!(127), json!({"Ok": 127})),
                (json!(128), overflow.clone()),
                (json!(255), overflow.clone()),
            ],
        ),
    ];
    for (name, from, to, rows) in cases {
        let frame = function(
            name,
            json!([["x", from]]),
            &format!("Result<{to},ArithmeticError>"),
            json!([["ok", ["to?", to, "x"]]]),
        );
        let handle = valid(&temp.path, &frame);
        for (argument, expected) in rows {
            assert_eq!(
                call(&temp.path, &handle, name, std::slice::from_ref(&argument)),
                expected,
                "{name}({argument})"
            );
        }
    }
    // Widening is exact at the minimum signed values: through i128 and back,
    // and i8 -> i64 -> i8.
    let round = function(
        "round",
        json!([["x", "i64"], ["y", "i8"]]),
        "Result<i64,ArithmeticError>",
        json!([
            ["let", "wide", ["to?", "i128", "x"]],
            ["let", "back", ["to?", "i64", "wide"]],
            ["let", "small", ["to?", "i8", ["to?", "i64", "y"]]],
            ["ok", ["add?", "back", ["to?", "i64", "small"]]]
        ]),
    );
    let handle = valid(&temp.path, &round);
    assert_eq!(
        call(&temp.path, &handle, "round", &[json!(i64::MIN), json!(0)]),
        json!({"Ok": i64::MIN})
    );
    assert_eq!(
        call(&temp.path, &handle, "round", &[json!(0), json!(-128)]),
        json!({"Ok": -128})
    );
    assert_eq!(
        call(&temp.path, &handle, "round", &[json!(i64::MAX), json!(0)]),
        json!({"Ok": i64::MAX})
    );
    // Without `?`, a conversion outside the target traps.
    let trapping = function(
        "t",
        json!([["x", "i64"]]),
        "u8",
        json!([["return", ["to", "u8", "x"]]]),
    );
    let handle = valid(&temp.path, &trapping);
    assert_eq!(call(&temp.path, &handle, "t", &[json!(200)]), json!(200));
    assert!(
        call(&temp.path, &handle, "t", &[json!(-5)])
            .to_string()
            .contains("trap")
    );
}

#[test]
fn unsupported_constructs_are_refused_at_their_authored_location() {
    let temp = workspace("refusals");
    for (body, at, words) in [
        (
            json!([["let", "a", 1], ["set", "a", 2], ["return", "a"]]),
            "/fns/0/body/1",
            "`set` needs a `var`",
        ),
        (
            json!([["return", ["add", "x", ["len", "xs"]]]]),
            "/fns/0/body/0/1/2",
            "operands mix",
        ),
        (
            json!([["return", 1], ["return", 2]]),
            "/fns/0/body/1",
            "can never run",
        ),
        (json!([["let", "a", 1]]), "/fns/0/body", "reach its end"),
        (
            json!([["return", ["pow", "x", 2]]]),
            "/fns/0/body/0/1",
            "unknown operation",
        ),
        (
            json!([["return", ["add?", "x", 1]]]),
            "/fns/0/body/0/1",
            "must return a Result",
        ),
    ] {
        let frame = function("f", json!([["x", "i64"], ["xs", "Vec<i64>"]]), "i64", body);
        let text = refusal(&temp.path, &frame);
        assert!(
            text.contains(&format!("{at}: ")) && text.contains(words),
            "{text}"
        );
    }
    let mut both = function("f", json!([["x", "i64"]]), "i64", json!([["return", "x"]]));
    both["fns"][0]["blocks"] = json!([]);
    assert!(refusal(&temp.path, &both).contains("blocks or body, not both"));
    let patch = json!({"af1": 1, "afx": 1, "patch": [{"fn": "f", "body": [["return", 1]]}]});
    assert!(refusal(&temp.path, &patch).contains("/patch/0/body"));
}

#[test]
fn a_compiler_refusal_points_into_the_body() {
    // The lowering does not check error case names; the compiler does, and its
    // pointer into the lowered blocks comes back as the authored statement.
    let temp = workspace("compiler");
    let mut frame = function(
        "g",
        json!([["x", "i64"]]),
        "Result<i64,GError>",
        json!([["if", ["lt", "x", 0], [["fail", "Negative"]]], ["ok", "x"]]),
    );
    frame["types"] = json!([{"name": "GError", "variant": ["Small"]}]);
    let text = refusal(&temp.path, &frame);
    assert!(text.contains("/fns/0/body/0/2/0"), "{text}");
}

#[test]
fn lowering_is_deterministic() {
    let frame = function(
        "total",
        json!([["xs", "Vec<i64>"]]),
        "i64",
        json!([
            ["var", "t", "i64", 0],
            [
                "for",
                "x",
                "xs",
                [["set", "t", ["add", "t", ["max", "x", 0]]]]
            ],
            ["return", "t"]
        ]),
    );
    let views: Vec<String> = (0..2)
        .map(|k| {
            let temp = workspace(&format!("det{k}"));
            let handle = valid(&temp.path, &frame);
            // Entity identities and the root differ per run (fresh randomness);
            // everything the lowering decides must not.
            run(&temp.path, &["view", "--after", &handle])
                .1
                .lines()
                .filter(|line| !line.starts_with('#'))
                .map(|line| line.split("   [").next().unwrap_or(line))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect();
    assert_eq!(views[0], views[1]);
}

#[test]
fn the_help_example_runs() {
    let text = sley_agent::help::topic("structured").unwrap();
    let start = text.find("{\"af1\"").unwrap();
    let frame: Value = serde_json::from_str(text[start..].trim()).unwrap();
    let temp = workspace("help");
    let handle = valid(&temp.path, &frame);
    assert_eq!(
        call(&temp.path, &handle, "product", &[json!([2, -3, 4])]),
        json!({"Ok": -24})
    );
    assert_eq!(
        call(&temp.path, &handle, "product", &[json!([])]),
        json!({"Ok": 1})
    );
}

//! Checked range and indexed iteration against explicit reference loops.
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
fn ranges_match_explicit_loops_at_every_integer_width_and_extreme() {
    let temp = workspace("range-edges");
    for bits in [8, 16, 32, 64, 128] {
        for signed in [false, true] {
            let ty = format!("{}{bits}", if signed { "i" } else { "u" });
            let frame = function(
                "count",
                json!([["lo", ty], ["hi", ty]]),
                "u64",
                json!([
                    ["var", "n", "u64", 0],
                    ["range", "i", "lo", "hi", [["set", "n", ["add", "n", 1]]]],
                    ["return", "n"]
                ]),
            );
            let handle = valid(&temp.path, &frame);
            let reference = function(
                "reference",
                json!([["lo", ty], ["hi", ty]]),
                "u64",
                json!([
                    ["var", "n", "u64", 0],
                    ["var", "j", ty, "lo"],
                    [
                        "while",
                        ["lt", "j", "hi"],
                        [["set", "n", ["add", "n", 1]], ["set", "j", ["add", "j", 1]]]
                    ],
                    ["return", "n"]
                ]),
            );
            let reference = valid(&temp.path, &reference);
            let max = if signed {
                (1_u128 << (bits - 1)) - 1
            } else {
                u128::MAX >> (128 - bits)
            };
            let min = if signed {
                if bits == 128 {
                    i128::MIN
                } else {
                    -(1_i128 << (bits - 1))
                }
            } else {
                0
            };
            let rows = [
                ("0".to_owned(), "0".to_owned(), 0),
                ("1".to_owned(), "0".to_owned(), 0),
                ("0".to_owned(), "7".to_owned(), 7),
                ((max - 3).to_string(), max.to_string(), 3),
                (max.to_string(), max.to_string(), 0),
                (min.to_string(), (min + 3).to_string(), 3),
                (max.to_string(), min.to_string(), 0),
            ];
            for (lo, hi, expected) in rows {
                let args = [json!(lo), json!(hi)];
                let answer = call(&temp.path, &handle, "count", &args);
                assert_eq!(answer, json!(expected), "{ty} {args:?}");
                assert_eq!(answer, call(&temp.path, &reference, "reference", &args));
            }
        }
    }
}

#[test]
fn bounds_are_snapshots_and_control_flow_preserves_held_values() {
    let temp = workspace("range-snapshot");
    let frame = function(
        "snapshot",
        json!([["flag", "bool"]]),
        "i64",
        json!([
            ["var", "end", "i64", 4],
            ["var", "start", "i64", 1],
            ["var", "sum", "i64", 0],
            [
                "range",
                "i",
                "start",
                ["if", "flag", "end", 3],
                [
                    ["set", "sum", ["add", ["mul", "sum", 10], "i"]],
                    ["set", "end", 0],
                    ["set", "start", 99]
                ]
            ],
            ["return", "sum"]
        ]),
    );
    let handle = valid(&temp.path, &frame);
    assert_eq!(
        call(&temp.path, &handle, "snapshot", &[json!(true)]),
        json!(123)
    );
    assert_eq!(
        call(&temp.path, &handle, "snapshot", &[json!(false)]),
        json!(12)
    );
    let early = function(
        "first",
        json!([["lo", "i64"], ["hi", "i64"]]),
        "i64",
        json!([
            ["range", "i", "lo", "hi", [["return", "i"]]],
            ["return", -99]
        ]),
    );
    let handle = valid(&temp.path, &early);
    assert_eq!(
        call(&temp.path, &handle, "first", &[json!(5), json!(9)]),
        json!(5)
    );
    assert_eq!(
        call(&temp.path, &handle, "first", &[json!(9), json!(5)]),
        json!(-99)
    );
}

#[test]
fn bound_failures_keep_left_to_right_order_even_when_the_range_is_empty() {
    let temp = workspace("range-failures");
    let mut frame = function(
        "ordered",
        json!([]),
        "Result<i64,BoundsError>",
        json!([
            ["range", "i", ["div?Start", 1, 0], ["div?End", 1, 0], []],
            ["ok", 1]
        ]),
    );
    frame["types"] = json!([{"name":"BoundsError", "variant":["Start","End"]}]);
    let handle = valid(&temp.path, &frame);
    assert_eq!(
        call(&temp.path, &handle, "ordered", &[]),
        json!({"Err":"Start"})
    );
    frame["fns"][0]["body"][0][2] = json!(i64::MAX);
    let handle = valid(&temp.path, &frame);
    assert_eq!(
        call(&temp.path, &handle, "ordered", &[]),
        json!({"Err":"End"})
    );
    let overflow = function(
        "overflow",
        json!([]),
        "Result<i8,ArithmeticError>",
        json!([
            ["range", "i", ["add?", {"type":"i8","value":127}, 1], {"type":"i8","value":0}, []], ["ok", 0]
        ]),
    );
    let handle = valid(&temp.path, &overflow);
    assert_eq!(
        call(&temp.path, &handle, "overflow", &[]),
        json!({"Err":{"ArithmeticError":"Overflow"}})
    );
}

#[test]
fn indexed_iteration_and_nested_ranges_match_pairwise_reference() {
    let temp = workspace("indexed");
    let frame = function(
        "pairs",
        json!([["xs", "Vec<i64>"]]),
        "i64",
        json!([
            ["var", "count", "i64", 0],
            [
                "for-indexed",
                "i",
                "x",
                "xs",
                [[
                    "range",
                    "j",
                    ["add", "i", 1],
                    ["len", "xs"],
                    [[
                        "if",
                        ["gt", "x", ["get", "xs", "j"]],
                        [["set", "count", ["add", "count", 1]]]
                    ]]
                ]]
            ],
            ["return", "count"]
        ]),
    );
    let handle = valid(&temp.path, &frame);
    // Exhaust all vectors of length <= 4 over three values, including duplicates.
    for len in 0..=4_u32 {
        for mut code in 0..3_usize.pow(len) {
            let xs: Vec<i64> = (0..len)
                .map(|_| {
                    let value = i64::try_from(code % 3).unwrap() - 1;
                    code /= 3;
                    value
                })
                .collect();
            let expected: usize = xs
                .iter()
                .enumerate()
                .map(|(i, x)| xs[i + 1..].iter().filter(|y| x > *y).count())
                .sum();
            assert_eq!(
                call(&temp.path, &handle, "pairs", &[json!(xs)]),
                json!(expected)
            );
        }
    }
    let early = function(
        "position",
        json!([["xs", "Vec<i64>"]]),
        "u64",
        json!([
            [
                "for-indexed",
                "i",
                "x",
                "xs",
                [["if", ["lt", "x", 0], [["return", "i"]]]]
            ],
            ["return", ["len", "xs"]]
        ]),
    );
    let handle = valid(&temp.path, &early);
    assert_eq!(
        call(&temp.path, &handle, "position", &[json!([2, 4, -1, 8])]),
        json!(2)
    );
    assert_eq!(
        call(&temp.path, &handle, "position", &[json!([])]),
        json!(0)
    );
}

#[test]
fn range_and_index_names_are_read_only_local_and_unambiguous() {
    let temp = workspace("range-refusals");
    let duplicate = refusal(
        &temp.path,
        &function(
            "duplicate",
            json!([["xs", "Vec<i64>"]]),
            "i64",
            json!([["for-indexed", "i", "i", "xs", []], ["return", 0]]),
        ),
    );
    assert!(duplicate.contains("/fns/0/body/0/2"), "{duplicate}");
    for body in [
        json!([["range", "i", {"type":"i8","value":0}, {"type":"u64","value":2}, []], ["return", 0]]),
        json!([["range", "i", false, true, []], ["return", 0]]),
        json!([["range", "i", 0, 3, [["set", "i", 2]]], ["return", 0]]),
        json!([
            ["var", "i", "i64", 0],
            ["range", "i", 0, 3, []],
            ["return", 0]
        ]),
        json!([["range", "i", 0, 3, [["let", "i", 2]]], ["return", 0]]),
        json!([["range", "i", 0, 3, []], ["return", "i"]]),
        json!([["for-indexed", "i", "i", "xs", []], ["return", 0]]),
        json!([
            ["for-indexed", "i", "x", "xs", [["set", "i", 0]]],
            ["return", 0]
        ]),
        json!([
            ["for-indexed", "i", "x", "xs", [["let", "x", 0]]],
            ["return", 0]
        ]),
    ] {
        let error = refusal(
            &temp.path,
            &function("refused", json!([["xs", "Vec<i64>"]]), "i64", body),
        );
        assert!(error.contains("/fns/0/body"), "{error}");
    }
}

#[cfg(feature = "familiar")]
#[test]
fn familiar_loops_lower_to_the_same_structured_form_and_infer_checked_call_bounds() {
    let text = "fn indexed(xs: Vec<i64>) -> i64 { var n: i64 = 0\n for i, x in xs { for j in i+1..len(xs) { if x > xs[j] { n = n + 1 } } } return n }";
    let parsed = sley_agent::familiar::parse(text).unwrap();
    assert_eq!(parsed.frame["fns"][0]["body"][1][0], "for-indexed");
    assert_eq!(parsed.frame["fns"][0]["body"][1][4][0][0], "range");
    let temp = workspace("range-familiar");
    let file = temp.path.join("proposal.txt");
    fs::write(&file, text).unwrap();
    let (status, report) = run_json(&temp.path, &["try", "--familiar", file.to_str().unwrap()]);
    assert_eq!(status, 0, "{report}");
    assert_eq!(
        call(
            &temp.path,
            report["handle"].as_str().unwrap(),
            "indexed",
            &[json!([5, 3, 1])]
        ),
        json!(3)
    );
    let ordinary = valid(&temp.path, &parsed.frame);
    assert_eq!(
        call(&temp.path, &ordinary, "indexed", &[json!([5, 3, 1])]),
        json!(3)
    );
    fs::write(&file, "fn bound() -> Result<u64,ArithmeticError> { return ok(3) }\nfn count() -> Result<u64,ArithmeticError> { var n: u64 = 0\nfor i in try(bound())..try(bound()) { n = n + 1 } return ok(n) }").unwrap();
    let (status, report) = run_json(&temp.path, &["try", "--familiar", file.to_str().unwrap()]);
    assert_eq!(status, 0, "{report}");
    assert_eq!(
        call(&temp.path, report["handle"].as_str().unwrap(), "count", &[]),
        json!({"Ok":0})
    );
}

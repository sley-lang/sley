//! Sley 2.1.1 authoring ergonomics in the optional familiar frontend
//! (ADR-0055): the familiar additions (inferred and assignable variables,
//! compound assignment, generator folds, tail values, if expressions, C-family
//! logical spellings) and the Python dialect, which keeps Python's meaning
//! where Sley can express it exactly (floor division and modulo, truthiness,
//! short-circuit and/or, chained comparisons, membership, views) and refuses
//! the rest at its line and column.
#![cfg(all(feature = "native", feature = "familiar"))]

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
            "sley-familiar-python-{label}-{}-{}",
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

fn try_text(dir: &Path, text: &str) -> (i32, Value) {
    let path = dir.join("proposal.txt");
    fs::write(&path, text).unwrap();
    let path = path.display().to_string();
    let (status, out) = run(
        dir,
        &["--json", "try", "--familiar", path.as_str(), "--no-test"],
    );
    (
        status,
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not JSON: {out}")),
    )
}

fn accepted(dir: &Path, text: &str) -> String {
    let (status, report) = try_text(dir, text);
    assert_eq!(status, 0, "{report:#}\n{text}");
    report["handle"].as_str().unwrap().to_owned()
}

fn inline_tests(dir: &Path, text: &str, on: Option<&str>) -> (i32, Value) {
    let path = dir.join("truthiness.py");
    fs::write(&path, text).unwrap();
    let path = path.display().to_string();
    let mut args = vec!["--json", "try", "--familiar", path.as_str()];
    if let Some(handle) = on {
        args.extend(["--on", handle]);
    }
    let (status, output) = run(dir, &args);
    (status, serde_json::from_str(output.trim()).unwrap())
}

#[test]
fn python_raises_matches_builtin_ancestors_and_aliases() {
    let temp = workspace("raises-inheritance");
    for (raised, expected, prefix) in [
        ("IndexError", "LookupError", ""),
        ("ZeroDivisionError", "ArithmeticError", ""),
        ("ConnectionResetError", "OSError", ""),
        ("IOError", "EnvironmentError", ""),
        (
            "BadIndex",
            "LookupError",
            "class BadIndex(IndexError):\n    pass\n\n",
        ),
    ] {
        let text = format!(
            "{prefix}def fail(x: int) -> int:\n    raise {raised}(\"boom\")\n\ndef test_failure():\n    with pytest.raises({expected}):\n        fail(0)\n"
        );
        let (status, report) = inline_tests(&temp.path, &text, None);
        assert_eq!(status, 0, "{raised} -> {expected}: {report:#}");
        assert_eq!(report["tests"].as_array().unwrap().len(), 1, "{report:#}");
        assert_eq!(report["tests"][0]["pass"], true, "{report:#}");
    }
    let text = "def fail(x: int) -> int:\n    raise IndexError(\"boom\")\n\ndef test_failure():\n    with pytest.raises(ArithmeticError):\n        fail(0)\n";
    let (status, report) = inline_tests(&temp.path, text, None);
    assert_eq!(status, 2, "{report:#}");
    assert!(
        report["detail"]
            .as_str()
            .unwrap()
            .contains("cannot raise ArithmeticError")
    );
    assert_eq!(report["obligations"][0]["text"]["line"], 5, "{report:#}");
}

#[test]
fn python_bare_assertions_use_truthiness() {
    let temp = workspace("truthiness");
    let text = "\
def number(x: int) -> int:
    return x

def word(x: str) -> str:
    return x

def items(x: list[int]) -> list[int]:
    return x

def flag(x: int) -> bool:
    return x > 0

def test_truth():
    assert number(7)
    assert not number(0)
    assert word(\"hello\")
    assert not word(\"\")
    assert items([1])
    assert not items([])
    assert flag(7)
    assert not flag(0)
";
    let (status, report) = inline_tests(&temp.path, text, None);
    assert_eq!(status, 0, "{report:#}");
    let tests = report["tests"].as_array().unwrap();
    assert_eq!(tests.len(), 8);
    assert!(tests.iter().all(|test| test["pass"] == true), "{report:#}");

    let false_assertion = text.replace("assert number(7)", "assert number(0)");
    let (status, report) = inline_tests(&temp.path, &false_assertion, None);
    assert_eq!(status, 1, "{report:#}");
    assert_eq!(report["verdict"]["valid"], true, "{report:#}");
    assert_eq!(report["tests"][0]["pass"], false, "{report:#}");
}

#[test]
fn python_bare_assertions_can_test_existing_functions() {
    let temp = workspace("truthiness-existing");
    let handle = accepted(
        &temp.path,
        "def number(x: int) -> int:\n    return x\n\ndef flag(x: int) -> bool:\n    return x > 0\n\ndef _sley_assert_0() -> int:\n    return 42\n",
    );
    let text = "def test_existing():\n    assert number(7)\n    assert not number(0)\n    assert flag(7)\n    assert not flag(0)\n";
    let (status, report) = inline_tests(&temp.path, text, Some(&handle));
    assert_eq!(status, 0, "{report:#}");
    let tests = report["tests"].as_array().unwrap();
    assert_eq!(tests.len(), 4);
    assert!(tests.iter().all(|test| test["pass"] == true), "{report:#}");
    // Candidate-on-candidate reports include the inherited functions. Commit
    // the code-only base to check which functions the assertions themselves add.
    let (status, output) = run(&temp.path, &["commit", &handle]);
    assert_eq!(status, 0, "{output}");
    let (status, report) = inline_tests(&temp.path, text, None);
    assert_eq!(status, 0, "{report:#}");
    let mut added = 0;
    for change in report["changed"].as_array().unwrap() {
        if change["kind"] == "fn" {
            added += 1;
            assert_eq!(change["visibility"], "private", "{report:#}");
            assert_ne!(change["name"], "number");
            assert_ne!(change["name"], "flag");
            assert_ne!(change["name"], "_sley_assert_0");
        }
    }
    assert_eq!(added, 1, "positive and negative cases share a predicate");
    let handle = report["handle"].as_str().unwrap();
    assert_eq!(call(&temp.path, handle, "_sley_assert_0", &[]), json!(42));
}

#[test]
fn python_bare_assertions_preserve_traps_and_error_locations() {
    let temp = workspace("truthiness-errors");
    let text = "def divide(x: int) -> int:\n    return 1 // x\n\ndef test_divide():\n    assert not divide(0)\n";
    let (status, report) = inline_tests(&temp.path, text, None);
    assert_eq!(status, 1, "{report:#}");
    assert_eq!(report["verdict"]["valid"], true, "{report:#}");
    assert_eq!(report["tests"][0]["pass"], false, "{report:#}");

    let text = "class R:\n    x: int\n\ndef record(r: R) -> R:\n    return r\n\ndef test_record():\n    assert record(R(1))\n";
    let (status, report) = inline_tests(&temp.path, text, None);
    assert_eq!(status, 2, "{report:#}");
    assert!(
        report["detail"]
            .as_str()
            .unwrap()
            .contains("no truth value")
    );
    assert_eq!(report["obligations"][0]["text"]["line"], 8, "{report:#}");
    assert_eq!(report["obligations"][0]["text"]["column"], 5, "{report:#}");
}

fn call(dir: &Path, handle: &str, name: &str, args: &[Value]) -> Value {
    let mut words: Vec<String> = vec!["--json".into(), "call".into(), name.into()];
    words.extend(args.iter().map(Value::to_string));
    words.extend(["--on".into(), handle.into()]);
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    let (_, out) = run(dir, &refs);
    let report: Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not JSON: {out}"));
    report.get("result").cloned().unwrap_or(report)
}

/// Asserts `name(args) == expected` for every case.
fn check(dir: &Path, handle: &str, name: &str, cases: &[(Value, Value)]) {
    for (args, expected) in cases {
        let args = args.as_array().unwrap();
        let got = call(dir, handle, name, args);
        assert_eq!(&got, expected, "{name}{args:?}");
    }
}

fn traps(value: &Value) -> bool {
    value.to_string().contains("trap")
}

fn refused(dir: &Path, text: &str, needle: &str, line: u64, column: u64) {
    let (status, report) = try_text(dir, text);
    assert_eq!(status, 2, "{report:#}");
    let detail = report["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains(needle) && detail.contains(&format!("line {line}, column {column}")),
        "wanted `{needle}` at {line}:{column}, got {detail}"
    );
}

#[test]
fn familiar_additions_lower_to_ordinary_sley() {
    let temp = workspace("familiar");
    let text = "\
fn clamp_sum(xs: Vec<i64>, low: i64, high: i64) -> i64 {
    sum(clamp(x, low, high) for x in xs)
}
fn stats(xs: Vec<i64>) -> i64 {
    let mut total: i64 = 0
    var n = 0
    for x in xs { total += x; n += 1 }
    let big = count(x > 10 for x in xs)
    let pos = any(x > 0 for x in xs) && !all(x > 0 for x in xs)
    let top = max(x for x in xs if x < 100)
    if pos || n == 0 { total * 1000 + big * 100 + top } else { total }
}
fn sign(a: i64) -> i64 {
    let s = if a < 0 { -1 } else if a == 0 { 0 } else { 1 }
    s
}
";
    let h = accepted(&temp.path, text);
    check(
        &temp.path,
        &h,
        "clamp_sum",
        &[
            (json!([[1, 5, 9, -3], 0, 6]), json!(12)),
            (json!([[], 0, 6]), json!(0)),
        ],
    );
    // total 26, two above 10, mixed signs, largest below 100 is 20
    check(
        &temp.path,
        &h,
        "stats",
        &[(json!([[12, -3, 20, -3]]), json!(26220))],
    );
    check(&temp.path, &h, "stats", &[(json!([[1, 2]]), json!(3))]);
    check(
        &temp.path,
        &h,
        "sign",
        &[
            (json!([-5]), json!(-1)),
            (json!([0]), json!(0)),
            (json!([7]), json!(1)),
        ],
    );
    // max over nothing traps
    assert!(traps(&call(&temp.path, &h, "stats", &[json!([200])])));
}

#[test]
fn python_arithmetic_and_truth_follow_python() {
    let temp = workspace("python-arith");
    let text = "\
def div(a: int, b: int) -> int:
    return a // b

def mod(a: int, b: int) -> int:
    return a % b

def power(a: int, b: int) -> int:
    return -a ** b

def g(a: int, b: int, c: int) -> int:
    return math.gcd(math.gcd(a, b), c)

def truth(xs: list[int], n: int) -> int:
    if not xs:
        return -1
    return 1 if n else 0

def lazy(xs: list[int], k: int) -> bool:
    # items[k - 1] is evaluated only when k is nonzero
    return k and xs[k - 1] == 7

def between(a: int, b: int, c: int) -> bool:
    return a < b <= c
";
    let h = accepted(&temp.path, &format!("import math\n{text}"));
    check(
        &temp.path,
        &h,
        "div",
        &[
            (json!([-7, 2]), json!(-4)),
            (json!([7, -2]), json!(-4)),
            (json!([7, 2]), json!(3)),
            (json!([-8, 2]), json!(-4)),
        ],
    );
    check(
        &temp.path,
        &h,
        "mod",
        &[
            (json!([-7, 2]), json!(1)),
            (json!([7, -2]), json!(-1)),
            (json!([7, 3]), json!(1)),
        ],
    );
    check(
        &temp.path,
        &h,
        "power",
        &[(json!([10, 2]), json!(-100)), (json!([3, 0]), json!(-1))],
    );
    assert!(traps(&call(
        &temp.path,
        &h,
        "power",
        &[json!(2), json!(-1)]
    )));
    assert!(traps(&call(&temp.path, &h, "div", &[json!(1), json!(0)])));
    check(
        &temp.path,
        &h,
        "g",
        &[
            (json!([12, -18, 8]), json!(2)),
            (json!([0, 0, 0]), json!(0)),
        ],
    );
    check(
        &temp.path,
        &h,
        "truth",
        &[
            (json!([[], 5]), json!(-1)),
            (json!([[1], 5]), json!(1)),
            (json!([[1], 0]), json!(0)),
        ],
    );
    check(
        &temp.path,
        &h,
        "lazy",
        &[
            (json!([[7], 0]), json!(false)),
            (json!([[7], 1]), json!(true)),
        ],
    );
    check(
        &temp.path,
        &h,
        "between",
        &[
            (json!([1, 2, 2]), json!(true)),
            (json!([1, 3, 2]), json!(false)),
        ],
    );
}

/// Python's `//` and `%` compute the truncating remainder once and reuse it;
/// a literal divisor's sign is folded into the correction test. The answers
/// (Python's, recorded from `CPython` 3.13) and the traps are unchanged.
#[test]
fn python_floor_operations_compute_one_remainder() {
    let temp = workspace("python-floor-once");
    let text = "\
def d3(a: int) -> int:
    return a // 3

def m3(a: int) -> int:
    return a % 3

def dn3(a: int) -> int:
    return a // -3

def mn3(a: int) -> int:
    return a % -3

def mp(a: int) -> int:
    return a % 1000000007

def dv(a: int, b: int) -> int:
    return a // b

def mv(a: int, b: int) -> int:
    return a % b
";
    let h = accepted(&temp.path, text);
    let min = i64::MIN;
    let max = i64::MAX;
    // (a, a // 3, a % 3, a // -3, a % -3, a % 1000000007), from `CPython` 3.13.
    let rows: [(i64, i64, i64, i64, i64, i64); 9] = [
        (-7, -3, 2, 2, -1, 1_000_000_000),
        (-6, -2, 0, 2, 0, 1_000_000_001),
        (-1, -1, 2, 0, -1, 1_000_000_006),
        (0, 0, 0, 0, 0, 0),
        (1, 0, 1, -1, -2, 1),
        (5, 1, 2, -2, -1, 5),
        (7, 2, 1, -3, -2, 7),
        (
            min,
            -3_074_457_345_618_258_603,
            1,
            3_074_457_345_618_258_602,
            -2,
            708_828_003,
        ),
        (
            max,
            3_074_457_345_618_258_602,
            1,
            -3_074_457_345_618_258_603,
            -2,
            291_172_003,
        ),
    ];
    for (a, d3, m3, dn3, mn3, mp) in rows {
        for (name, want) in [
            ("d3", d3),
            ("m3", m3),
            ("dn3", dn3),
            ("mn3", mn3),
            ("mp", mp),
        ] {
            assert_eq!(
                call(&temp.path, &h, name, &[json!(a)]),
                json!(want),
                "{name}({a})"
            );
        }
        for (b, d, m) in [(3, d3, m3), (-3, dn3, mn3)] {
            assert_eq!(
                call(&temp.path, &h, "dv", &[json!(a), json!(b)]),
                json!(d),
                "{a} // {b}"
            );
            assert_eq!(
                call(&temp.path, &h, "mv", &[json!(a), json!(b)]),
                json!(m),
                "{a} % {b}"
            );
        }
    }
    for name in ["dv", "mv"] {
        assert!(
            traps(&call(&temp.path, &h, name, &[json!(1), json!(0)])),
            "{name} by 0"
        );
        assert!(
            traps(&call(&temp.path, &h, name, &[json!(min), json!(-1)])),
            "{name} MIN, -1"
        );
    }
    let (status, out) = run(&temp.path, &["commit", h.as_str()]);
    assert_eq!(status, 0, "{out}");
    for name in ["d3", "m3", "dn3", "mn3", "mp", "dv", "mv"] {
        let (status, view) = run(&temp.path, &["view", name]);
        assert_eq!(status, 0, "{view}");
        let remainders = view.lines().filter(|l| l.contains(" = rem ")).count();
        assert_eq!(remainders, 1, "{name} computes one remainder:\n{view}");
    }
}

#[test]
fn python_loops_views_and_generators() {
    let temp = workspace("python-loops");
    let text = "\
def max_gap(items: list[int]) -> int:
    return max((abs(a - b) for a, b in zip(items, items[1:])), default=0)

def tail_sum(items: list[int]) -> int:
    total = 0
    for x in items[1:]:
        total += x
    return total + (items[-1] if items else 0)

def inversions(items: list[int]) -> int:
    return sum(1 for i in range(len(items)) for j in range(i + 1, len(items)) if items[i] > items[j])

def first_index(items: list[int], target: int) -> int:
    for i, x in enumerate(items):
        if x == target:
            return i
    return -1

def has(items: list[int], v: int) -> int:
    return len([x for x in items if x == v]) if v in items else -len(items)

def collatz(n: int) -> int:
    s = 0
    while n != 1:
        n = n // 2 if n % 2 == 0 else 3 * n + 1
        s += 1
    return s

def best_run(items: list[int]) -> int:
    best = run = 0
    for k, x in enumerate(items):
        run = run + 1 if k and x == items[k - 1] else 1
        best = max(best, run)
    return best
";
    let h = accepted(&temp.path, text);
    check(
        &temp.path,
        &h,
        "max_gap",
        &[
            (json!([[1, 7, 4]]), json!(6)),
            (json!([[5]]), json!(0)),
            (json!([[]]), json!(0)),
        ],
    );
    check(
        &temp.path,
        &h,
        "tail_sum",
        &[(json!([[1, 2, 3]]), json!(8)), (json!([[]]), json!(0))],
    );
    check(
        &temp.path,
        &h,
        "inversions",
        &[(json!([[3, 1, 2]]), json!(2)), (json!([[]]), json!(0))],
    );
    check(
        &temp.path,
        &h,
        "first_index",
        &[
            (json!([[4, 5, 6], 6]), json!(2)),
            (json!([[4], 9]), json!(-1)),
        ],
    );
    check(
        &temp.path,
        &h,
        "has",
        &[
            (json!([[1, 2, 1], 1]), json!(2)),
            (json!([[1, 2], 9]), json!(-2)),
        ],
    );
    check(
        &temp.path,
        &h,
        "collatz",
        &[(json!([6]), json!(8)), (json!([1]), json!(0))],
    );
    check(
        &temp.path,
        &h,
        "best_run",
        &[
            (json!([[1, 1, 2, 2, 2]]), json!(3)),
            (json!([[]]), json!(0)),
        ],
    );
}

#[test]
fn python_refusals_name_their_position() {
    let temp = workspace("python-refusals");
    refused(
        &temp.path,
        "def f(a: int) -> int:\n    return a / 2\n",
        "float division",
        2,
        14,
    );
    refused(
        &temp.path,
        "def f(a: int) -> int:\n    try:\n        return a\n    except ValueError:\n        return 0\n",
        "`try`",
        2,
        5,
    );
    refused(
        &temp.path,
        "def f(a: int) -> int:\n    return 1.5\n",
        "floating-point",
        2,
        12,
    );
    refused(
        &temp.path,
        "def f(xs: list[int]) -> list[int]:\n    return xs[1:]\n",
        "slices",
        2,
        14,
    );
    refused(
        &temp.path,
        "def f(a) -> int:\n    return a\n",
        "annotate the parameter",
        1,
        8,
    );
    refused(
        &temp.path,
        "def f(a: int) -> int:\n\treturn a\n",
        "spaces, not tabs",
        2,
        1,
    );
    refused(
        &temp.path,
        "def f(a: float) -> int:\n    return 0\n",
        "floating-point",
        1,
        10,
    );
}

#[test]
fn python_scoping_break_continue_and_tuples() {
    let temp = workspace("python-scoping");
    let text = "\
from dataclasses import dataclass

@dataclass
class Invoice:
    quantity: int
    price: int
    region: str

def first_at_least(items: list[int], floor: int) -> int:
    found = -1
    for i in range(len(items)):
        if items[i] < floor:
            continue
        found = i
        break
    return found

def gcd_all(items: list[int]) -> int:
    g = 0
    for x in items:
        a, b = abs(x), g
        while b:
            a, b = b, a % b
        g = a
    return g

def clamp_each(items: list[int], low: int, high: int) -> int:
    total = 0
    for v in items:
        if v < low:
            v = low
        elif v > high:
            v = high
        total += v
    return total

def branch_value(n: int) -> int:
    if n < 0:
        sign = -1
    elif n == 0:
        sign = 0
    else:
        sign = 1
    q, r = divmod(n, 3)
    return sign * 100 + q * 10 + r
";
    let h = accepted(&temp.path, text);
    check(
        &temp.path,
        &h,
        "first_at_least",
        &[
            (json!([[1, 5, 9], 4]), json!(1)),
            (json!([[1], 4]), json!(-1)),
        ],
    );
    check(
        &temp.path,
        &h,
        "gcd_all",
        &[(json!([[12, -18, 0]]), json!(6)), (json!([[]]), json!(0))],
    );
    check(
        &temp.path,
        &h,
        "clamp_each",
        &[(json!([[1, 5, 9], 2, 6]), json!(13))],
    );
    check(
        &temp.path,
        &h,
        "branch_value",
        &[
            (json!([7]), json!(121)),
            (json!([-7]), json!(-128)),
            (json!([0]), json!(0)),
        ],
    );
}

#[test]
fn python_tuple_assignment_reassigns_parameters() {
    // Regression: a parameter named only after the first target of a tuple
    // assignment (`b` in `a, b = b, a % b`) was not recognised as assigned,
    // so the frame used `set b` without a `var` in scope (AGENT_FRAME_INVALID).
    let temp = workspace("python-tuple-params");
    let text = "\
def gcd(a: int, b: int) -> int:
    while b != 0:
        a, b = b, a % b
    return a

def swap_diff(a: int, b: int) -> int:
    a, b = b, a
    return a - b

def second_only(a: int, b: int) -> int:
    c, b = a, b + 1
    return c * 10 + b

def three(a: int, b: int, c: int) -> int:
    a, b, c = c, a, b
    return a * 100 + b * 10 + c

def step(x: int, n: int) -> int:
    q = 0
    for i in range(n):
        q, x = i, x + q
    return x
";
    let h = accepted(&temp.path, text);
    check(
        &temp.path,
        &h,
        "gcd",
        &[
            (json!([48, 18]), json!(6)),
            (json!([17, 0]), json!(17)),
            (json!([0, 5]), json!(5)),
        ],
    );
    check(&temp.path, &h, "swap_diff", &[(json!([3, 10]), json!(7))]);
    check(&temp.path, &h, "second_only", &[(json!([4, 1]), json!(42))]);
    check(&temp.path, &h, "three", &[(json!([1, 2, 3]), json!(312))]);
    check(&temp.path, &h, "step", &[(json!([1, 4]), json!(4))]);
}

#[test]
fn python_stepped_ranges_shifts_masks_and_distinct_counts() {
    let temp = workspace("python-stepped");
    let text = "\
def walk(a: int, b: int, s: int) -> int:
    t = 0
    for i in range(a, b, s):
        if i % 3 == 0:
            continue
        t = t * 7 + i
    return t + sum(i for i in range(a, b, s) if i % 2 == 0)

def bits(x: int, k: int) -> int:
    y = x >> k
    y <<= 1
    return y + (x & 7) + (x & 1)

def mod_pow(b: int, e: int, m: int) -> int:
    return pow(b, e, m) + pow(2, e % 5)

def distinct(xs: list[str]) -> int:
    return len(set(xs))
";
    let h = accepted(&temp.path, text);
    // Expected values are Python's own results for the same source.
    check(
        &temp.path,
        &h,
        "walk",
        &[
            (json!([0, 12, 1]), json!(1_140_978)),
            (json!([10, -7, -3]), json!(186_291)),
            (json!([-5, 12, 5]), json!(-190)),
            (json!([3, 4, 2]), json!(0)),
        ],
    );
    assert!(traps(&call(
        &temp.path,
        &h,
        "walk",
        &[json!(0), json!(5), json!(0)]
    )));
    check(
        &temp.path,
        &h,
        "bits",
        &[(json!([45, 2]), json!(28)), (json!([-45, 2]), json!(-20))],
    );
    check(
        &temp.path,
        &h,
        "mod_pow",
        &[
            (json!([1000, 60, 997]), json!(125)),
            (json!([5, 0, 1]), json!(1)),
        ],
    );
    check(
        &temp.path,
        &h,
        "distinct",
        &[
            (json!([["a", "b", "a", ""]]), json!(3)),
            (json!([[]]), json!(0)),
        ],
    );
}

#[test]
fn python_layered_failures_use_current_authored_test_lines() {
    for reference in ["c1", "d1"] {
        let temp = workspace("python-layered-test-lines");
        let path = temp.path.join("layered.py");
        let file = path.display().to_string();
        fs::write(
            &path,
            "def value(x: int) -> int:\n    return x\n\ndef test_old():\n    assert value(1) == 2\n\ndef test_kept():\n    assert value(3) == 4\n",
        )
        .unwrap();
        let (status, out) = run(&temp.path, &["try", "--familiar", &file]);
        assert_eq!(status, 1, "{out}");
        fs::write(
            &path,
            "\n\n\n\n\ndef test_new():\n    assert value(2) == 3\n\ndef test_old():\n    assert value(4) == 5\n",
        )
        .unwrap();
        let (status, out) = run(&temp.path, &["try", "--on", reference, "--familiar", &file]);
        assert_eq!(status, 1, "{reference}: {out}");
        assert!(out.contains("3 failed, 0 passed"), "{out}");
        for expected in [
            "FAIL test_new_0 (value): expected 3, got 2 (line 7)",
            "FAIL test_old_0 (value): expected 5, got 4 (line 10)",
            "FAIL test_kept_0 (value): expected 4, got 3",
        ] {
            assert!(
                out.lines().any(|line| line == expected),
                "{reference}: {out}"
            );
        }
    }
}

#[test]
fn python_tests_and_record_classes_run_in_one_try() {
    let temp = workspace("python-inline-tests");
    let text = "\
import pytest
from dataclasses import dataclass


@dataclass
class Point:
    x: int
    y: int


def manhattan(a: Point, b: Point) -> int:
    if a.x < 0:
        raise ValueError(\"negative\")
    return abs(a.x - b.x) + abs(a.y - b.y)


def scale(x: int) -> int:
    return x * 3 + 7 - 11


def test_manhattan():
    assert manhattan(Point(1, 2), Point(4, -2)) == 7
    assert manhattan(Point(0, 0), Point(0, 0)) == 1
    assert manhattan(Point(2, 0), Point(0, 0)) == 2
    assert manhattan(Point(0, 3), Point(0, 0)) == 3
    with pytest.raises(ValueError):
        manhattan(Point(-1, 0), Point(0, 0))
";
    let path = temp.path.join("records.py");
    fs::write(&path, text).unwrap();
    let file = path.display().to_string();
    let (status, out) = run(&temp.path, &["try", "--familiar", &file]);
    assert_eq!(status, 1, "{out}");
    // Like `pytest -q`: the failure at its line and one count line.
    assert!(
        out.starts_with("FAIL test_manhattan_1 (manhattan): expected 1, got 0 (line 23)\n"),
        "{out}"
    );
    assert!(
        out.ends_with(": 1 failed, 4 passed (advisory) d1@r1\n"),
        "{out}"
    );
    assert_eq!(out.lines().count(), 2, "{out}");
    fs::write(
        &path,
        text.replace("Point(0, 0)) == 1", "Point(0, 0)) == 0"),
    )
    .unwrap();
    let (status, out) = run(&temp.path, &["try", "--familiar", &file]);
    assert_eq!(status, 0, "{out}");
    assert_eq!(out, "c2: 5 passed (advisory) d2@r1\n");
}

/// The full `call` report (result, fuel, instructions), optionally on the
/// reference runner.
fn call_report(dir: &Path, handle: &str, name: &str, args: &[Value], reference: bool) -> Value {
    let mut words: Vec<String> = vec!["--json".into(), "call".into()];
    if reference {
        words.push("--reference".into());
    }
    words.push(name.into());
    words.extend(args.iter().map(Value::to_string));
    words.extend(["--on".into(), handle.into()]);
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    let (_, out) = run(dir, &refs);
    let mut report: Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not JSON: {out}"));
    if let Some(map) = report.as_object_mut() {
        map.remove("vm_micros");
    }
    report
}

/// Indexed loops (`fast_*`) and twins indexing by a converted copy (`slow_*`).
const INDEX_PROGRAMS: &str = "\
def fast_inv(xs: list[int]) -> int:
    count = 0
    n = len(xs)
    for i in range(n):
        x = xs[i]
        for j in range(i + 1, n):
            if xs[j] < x:
                count += 1
    return count

def slow_inv(xs: list[int]) -> int:
    count = 0
    n = len(xs)
    for i in range(n):
        a = i * 1
        x = xs[a]
        for j in range(i + 1, n):
            b = j * 1
            if xs[b] < x:
                count += 1
    return count

def fast_diff(xs: list[int], lo: int, k: int) -> int:
    total = 0
    for i in range(lo, len(xs)):
        total += xs[i] - xs[i - k] + xs[i + 1 - 1]
    return total

def slow_diff(xs: list[int], lo: int, k: int) -> int:
    total = 0
    for i in range(lo, len(xs)):
        a = i * 1
        total += xs[a] - xs[a - k] + xs[a + 1 - 1]
    return total

def fast_from(xs: list[int], s: int) -> int:
    total = 0
    for i in range(2, len(xs)):
        for j in range(i, len(xs)):
            total += xs[j] * (j - i + s)
        total += xs[i + s] if i + s < len(xs) else 0
    return total

def slow_from(xs: list[int], s: int) -> int:
    total = 0
    for i in range(2, len(xs)):
        for j in range(i, len(xs)):
            b = j * 1
            total += xs[b] * (j - i + s)
        a = i * 1 + s
        total += xs[a] if i + s < len(xs) else 0
    return total

def fast_enum(xs: list[int]) -> int:
    total = 0
    for k, x in enumerate(xs):
        total += k * x + (xs[k - 1] if k else 0)
    return total

def slow_enum(xs: list[int]) -> int:
    total = 0
    for k, x in enumerate(xs):
        a = k * 1
        total += a * x + (xs[a - 1] if a else 0)
    return total
";

#[test]
fn python_indexes_by_loop_counters_match_converted_indexes() {
    // Each `fast` function indexes by a range counter (directly, at an
    // offset, or through a nested range starting at it), which lowers to a
    // u64 copy of the counter; each `slow` twin indexes by a plain variable
    // equal to it (`i * 1`), which converts on every access. They agree on every
    // input, failures included, and the fast tier agrees with the
    // reference runner on fuel and instructions.
    let temp = workspace("python-index-shadow");
    let text = INDEX_PROGRAMS;
    let h = accepted(&temp.path, text);
    let lists = [
        json!([]),
        json!([7]),
        json!([3, 1, 2]),
        json!([5, -4, 3, 2, 2, -1]),
        json!([9, 8, 7, 6, 5, 4, 3, 2, 1, 0]),
    ];
    let mut failures = 0;
    for xs in &lists {
        let mut cases: Vec<(&str, Vec<Value>)> =
            vec![("inv", vec![xs.clone()]), ("enum", vec![xs.clone()])];
        for lo in [-2, 0, 1, 3] {
            for k in [0, 1, 2] {
                cases.push(("diff", vec![xs.clone(), json!(lo), json!(k)]));
            }
        }
        for s in [-3, 0, 1, 2] {
            cases.push(("from", vec![xs.clone(), json!(s)]));
        }
        for (name, args) in cases {
            let fast = call_report(&temp.path, &h, &format!("fast_{name}"), &args, false);
            let slow = call(&temp.path, &h, &format!("slow_{name}"), &args);
            assert_eq!(fast["result"], slow, "{name}{args:?}");
            failures += usize::from(traps(&slow));
            let reference = call_report(&temp.path, &h, &format!("fast_{name}"), &args, true);
            assert_eq!(fast, reference, "{name}{args:?} against the reference");
        }
    }
    assert!(failures > 0, "the cases include failing indexes");
    let xs = [json!([5, -4, 3, 2, 2, -1, 7, 8])];
    let count = |name: &str| {
        call_report(&temp.path, &h, name, &xs, false)["instructions"]
            .as_u64()
            .unwrap()
    };
    for name in ["inv", "enum"] {
        let (fast, slow) = (
            count(&format!("fast_{name}")),
            count(&format!("slow_{name}")),
        );
        assert!(
            fast < slow,
            "{name}: the copy saves the conversions ({fast} vs {slow})"
        );
    }
}

/// Floor division and modulo on values that are never negative (`fast_*`),
/// and twins (`slow_*`) that add a parameter `z` (always called with 0), so
/// the analysis cannot prove the operands non-negative and keeps Python's
/// sign correction. `neg_*` functions take values below zero on purpose.
const FLOOR_PROGRAMS: &str = "\
def fast_digits(n: int) -> int:
    total = 0
    for i in range(n):
        x = i
        while x > 0:
            total += x % 10
            x //= 10
    return total

def slow_digits(n: int, z: int) -> int:
    total = 0
    for i in range(n):
        x = i + z
        while x > 0:
            total += x % 10
            x //= 10
    return total

def fast_mods(n: int, m: int) -> int:
    total = 0
    for i in range(n):
        for k in range(m):
            total += i % k + i // k
    return total

def slow_mods(n: int, m: int, z: int) -> int:
    total = 0
    for i in range(n):
        for k in range(m):
            total += (i + z) % (k + z) + (i + z) // (k + z)
    return total

def fast_euclid(n: int) -> int:
    total = 0
    for i in range(1, n):
        for j in range(1, n):
            a = i
            b = j
            while b != 0:
                a, b = b, a % b
            total += a
    return total

def slow_euclid(n: int, z: int) -> int:
    total = 0
    for i in range(1, n):
        for j in range(1, n):
            a = i + z
            b = j + z
            while b != 0:
                a, b = b, a % b
            total += a
    return total

def neg_mixed(n: int) -> int:
    x = 5
    x = x - n
    return (x // 3) * 100 + x % 3

def neg_counter(n: int) -> int:
    total = 0
    for i in range(-n, n):
        total += (i // 4) * 10 + i % 4
    return total
";

#[test]
fn python_floor_operations_on_non_negative_values_skip_the_sign_fixup() {
    let temp = workspace("python-floor-nonneg");
    let h = accepted(&temp.path, FLOOR_PROGRAMS);
    let mut cases: Vec<(&str, Vec<Value>)> = Vec::new();
    for n in [0, 1, 9, 10, 11, 99, 100, 1234] {
        cases.push(("digits", vec![json!(n)]));
    }
    // `m >= 1` makes `k` start at 0: a zero divisor fails in both forms.
    for (n, m) in [(0, 0), (5, 0), (7, 1), (12, 4), (30, 7)] {
        cases.push(("mods", vec![json!(n), json!(m)]));
    }
    for n in [1, 2, 6, 13] {
        cases.push(("euclid", vec![json!(n)]));
    }
    let mut failures = 0;
    for (name, args) in &cases {
        let fast = call_report(&temp.path, &h, &format!("fast_{name}"), args, false);
        let mut slow_args = args.clone();
        slow_args.push(json!(0));
        let slow = call(&temp.path, &h, &format!("slow_{name}"), &slow_args);
        assert_eq!(fast["result"], slow, "{name}{args:?}");
        failures += usize::from(traps(&slow));
        let reference = call_report(&temp.path, &h, &format!("fast_{name}"), args, true);
        assert_eq!(fast, reference, "{name}{args:?} against the reference");
    }
    assert!(failures > 0, "the cases include a zero divisor");
    // Values that go negative keep Python's floor semantics (from CPython 3.13).
    check(
        &temp.path,
        &h,
        "neg_mixed",
        &[
            (json!([0]), json!(102)),
            (json!([5]), json!(0)),
            (json!([7]), json!(-99)),
            (json!([12]), json!(-298)),
        ],
    );
    check(
        &temp.path,
        &h,
        "neg_counter",
        &[
            (json!([0]), json!(0)),
            (json!([3]), json!(-21)),
            (json!([9]), json!(-63)),
        ],
    );
    let count = |name: &str, args: &[Value]| {
        call_report(&temp.path, &h, name, args, false)["instructions"]
            .as_u64()
            .unwrap()
    };
    for (name, args) in [
        ("digits", vec![json!(500)]),
        ("mods", vec![json!(20), json!(5)]),
        ("euclid", vec![json!(12)]),
    ] {
        let mut slow_args = args.clone();
        slow_args.push(json!(0));
        let (fast, slow) = (
            count(&format!("fast_{name}"), &args),
            count(&format!("slow_{name}"), &slow_args),
        );
        assert!(fast < slow, "{name}: no sign fixup ({fast} vs {slow})");
    }
}

#[test]
fn python_refusals_name_the_fix() {
    let temp = workspace("python-fix-hints");
    refused(
        &temp.path,
        "def f(xs: list[int]) -> int:\n    xs.append(1)\n    return 0\n",
        "lists cannot change",
        2,
        8,
    );
    refused(
        &temp.path,
        "def f(xs: list[int]) -> int:\n    return xs.count(1)\n",
        "sum(1 for y in xs if y == x)",
        2,
        15,
    );
    refused(
        &temp.path,
        "def f(xs: list[int]) -> int:\n    d = {}\n    return 0\n",
        "len(set(xs))",
        2,
        9,
    );
    refused(
        &temp.path,
        "def f(x: int) -> int:\n    if x > 0:\n        return x\n    return None\n",
        "raise ValueError",
        4,
        12,
    );
    refused(
        &temp.path,
        "def f(x: int) -> int:\n    print(x)\n    return x\n",
        "return the value",
        2,
        5,
    );
    refused(
        &temp.path,
        "def f(xs: list[int]) -> int:\n    out = []\n    return len(out)\n",
        "keep a count, sum or best value",
        2,
        11,
    );
    refused(
        &temp.path,
        "def f(xs: list[int]) -> int:\n    ys = sorted(xs)\n    return ys[0]\n",
        "nested loops",
        2,
        10,
    );
    refused(
        &temp.path,
        "def f(x: int) -> int:\n    return len(str(x))\n",
        "x // 10",
        2,
        16,
    );
}

/// `help python-task` is `help python` without the inline-test lines, and its
/// "locals are inferred" line holds: unannotated locals of every kind compile.
#[test]
fn python_task_card_drops_test_syntax_and_locals_need_no_types() {
    let full = sley_agent::help::topic("python").unwrap();
    let task = sley_agent::help::topic("python-task").unwrap();
    assert!(task.len() < full.len());
    for absent in ["def test_", "pytest", "TestCases", "assert f(", "predicate"] {
        assert!(full.contains(absent), "{absent}");
        assert!(!task.contains(absent), "{absent}");
    }
    // Every line of the task card is a line of the full card.
    for line in task.lines() {
        assert!(full.lines().any(|l| l == line), "{line}");
    }
    assert!(task.contains("locals are inferred (total = 0)"));
    let temp = workspace("python-task-card");
    let h = accepted(
        &temp.path,
        "\
def run_sum(xs: list[int], lo: int) -> int:
    total = 0
    seen = False
    best = xs[0] if xs else lo
    for x in xs:
        v = x * 2 if x % 2 else x
        total += v
        if v > best:
            best = v
            seen = True
    return total + best if seen else total
",
    );
    check(
        &temp.path,
        &h,
        "run_sum",
        &[
            (json!([[], 5]), json!(0)),
            (json!([[1, 2, 3], 0]), json!(16)),
            (json!([[4], 0]), json!(4)),
        ],
    );
}

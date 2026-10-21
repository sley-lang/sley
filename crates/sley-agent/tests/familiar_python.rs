//! Sley 2.1.1 authoring ergonomics in the optional familiar frontend
//! (ADR-0055): the familiar additions (inferred and assignable variables,
//! compound assignment, generator folds, tail values, if expressions, C-family
//! logical spellings) and the Python dialect, which keeps Python's meaning
//! where Sley can express it exactly (floor division and modulo, truthiness,
//! short-circuit and/or, chained comparisons, membership, views) and refuses
//! the rest at its line and column.
#![cfg(feature = "familiar")]

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
    assert!(
        out.contains(
            "changed: fn +manhattan +scale; type +Point; const +4; test +test_manhattan_0..4\n"
        ),
        "{out}"
    );
    assert!(out.contains("tests: 4/5 passed [authored 5]"), "{out}");
    assert!(
        out.contains("FAIL test_manhattan_1 (manhattan): expected 1, got 0"),
        "{out}"
    );
    assert!(
        out.contains("next: fix the text and run try --familiar again"),
        "{out}"
    );
    fs::write(
        &path,
        text.replace("Point(0, 0)) == 1", "Point(0, 0)) == 0"),
    )
    .unwrap();
    let (status, out) = run(&temp.path, &["try", "--familiar", &file]);
    assert_eq!(status, 0, "{out}");
    assert!(out.contains("tests: 5/5 passed"), "{out}");
}

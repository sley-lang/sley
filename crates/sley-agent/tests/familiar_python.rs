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

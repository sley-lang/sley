//! `try` says where a failing advisory test's trap read out of range: the
//! index, the length and the read's line. The note is advisory only: the
//! test's verdict, observed trap and fuel are those of the candidate, and
//! the probed program behind the note is never stored.
#![cfg(all(feature = "native", feature = "familiar"))]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sley_agent::genesis;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "sley-index-reads-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        genesis::init(&path, None, genesis::INIT_CEILINGS).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let mut words = vec!["--workspace".to_owned(), dir.display().to_string()];
    words.extend(args.iter().map(|arg| (*arg).to_owned()));
    let mut out = Vec::new();
    let status = sley_agent::cli::run(&words, &mut out);
    (status, String::from_utf8(out).unwrap())
}

fn write(dir: &Path, file: &str, text: &str) -> String {
    let path = dir.join(file);
    fs::write(&path, text).unwrap();
    path.display().to_string()
}

/// `try --json --familiar`: the report's tests by name.
fn tests_of(dir: &Path, file: &str, text: &str, on: Option<&str>) -> Vec<Value> {
    let path = write(dir, file, text);
    let mut args = vec!["--json", "try", "--familiar", path.as_str()];
    if let Some(on) = on {
        args.extend(["--on", on]);
    }
    let (_, out) = run(dir, &args);
    let report: Value = serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("{out}"));
    report["tests"].as_array().cloned().unwrap_or_default()
}

fn named<'a>(tests: &'a [Value], name: &str) -> &'a Value {
    tests
        .iter()
        .find(|test| test["test"] == name)
        .unwrap_or_else(|| panic!("no {name} in {tests:#?}"))
}

/// `(index, length, line)` of a test's out-of-range read.
fn read(tests: &[Value], name: &str) -> (String, String, Option<u64>) {
    let read = &named(tests, name)["index_read"];
    assert!(read.is_object(), "{name}: no index_read in {tests:#?}");
    (
        read["index"].as_str().unwrap().to_owned(),
        read["length"].as_str().unwrap().to_owned(),
        read["text"]["line"].as_u64(),
    )
}

const READS: &str = "\
def at(xs: list[int], i: int) -> int:
    return xs[i]

def last(xs: list[int]) -> int:
    return xs[-1]

def outer(xs: list[int], i: int) -> int:
    return at(xs, i + 1) + 1

def put(n: int, i: int) -> list[int]:
    out = [0] * n
    out[i] = 7
    return out

def grow(n: int) -> int:
    ys: list[int] = []
    for k in range(n):
        ys.append(k)
    return ys[n]

def big(x: int) -> int:
    return x * x

def test_reads():
    assert at([5, 6, 7], 1) == 6
    assert at([5, 6, 7], 3) == 7
    assert at([1, 2], -1) == 2
    assert last([]) == 0
    assert outer([1, 2, 3], 2) == 0
    assert put(3, 5) == [0, 0, 0]
    assert grow(4) == 3
    assert big(4000000000) == 0
";

#[test]
fn out_of_range_reads_name_index_length_and_line() {
    let temp = TempDir::new("reads");
    let tests = tests_of(&temp.path, "reads.py", READS, None);
    let s = |text: &str| text.to_owned();
    // xs[i] past the end, and a negative Python index before it converts.
    assert_eq!(read(&tests, "test_reads_1"), (s("3"), s("3"), Some(2)));
    assert_eq!(read(&tests, "test_reads_2"), (s("-1"), s("2"), Some(2)));
    // xs[-1] of an empty list: before its `len - 1` underflows.
    assert_eq!(read(&tests, "test_reads_3"), (s("-1"), s("0"), Some(5)));
    // A read in a function the test reaches through another call.
    assert_eq!(read(&tests, "test_reads_4"), (s("3"), s("3"), Some(2)));
    // A store to a missing index, and a read of a grown list.
    assert_eq!(read(&tests, "test_reads_5"), (s("5"), s("3"), Some(12)));
    assert_eq!(read(&tests, "test_reads_6"), (s("4"), s("4"), Some(19)));
    // Overflow is not a read: no note. A passing test has none either.
    assert!(named(&tests, "test_reads_7").get("index_read").is_none());
    assert!(named(&tests, "test_reads_0").get("index_read").is_none());
    // The observed outcome is the candidate's own, unchanged.
    for name in ["test_reads_1", "test_reads_5", "test_reads_7"] {
        assert_eq!(named(&tests, name)["actual"], "trap(1)");
        assert_eq!(named(&tests, name)["pass"], false);
    }
    // Only the tried candidate is stored: the probed program is not.
    let stored: Vec<String> = fs::read_dir(temp.path.join(".sley/candidates"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !stored.is_empty() && stored.iter().all(|file| file.starts_with("c1")),
        "{stored:?}"
    );
}

#[test]
fn text_report_appends_the_read_to_the_failure() {
    let temp = TempDir::new("text");
    let path = write(
        &temp.path,
        "pick.py",
        "\
def pick(xs: list[int], i: int) -> int:
    return xs[i]

def test_pick():
    assert pick([5, 6, 7], 3) == 7
",
    );
    let (status, out) = run(&temp.path, &["try", "--familiar", path.as_str()]);
    assert_eq!(status, 1, "{out}");
    assert!(
        out.contains(
            "FAIL test_pick_0 (pick): expected 7, got trap(1) (line 5); index 3 is out of range for length 3 (line 2)\n"
        ),
        "{out}"
    );
}

#[test]
fn frame_bodies_report_the_read_pointer() {
    let temp = TempDir::new("frame");
    let path = write(
        &temp.path,
        "nth.json",
        r#"{"af1": 1, "afx": 1,
 "fns": [{"fn": "nth", "params": [["xs", "Vec<i64>"], ["i", "u64"]], "returns": "i64",
          "body": [["return", ["get", "xs", "i"]]]}],
 "test_tables": [{"name": "nth_cases", "fn": "nth", "cases": [
   {"args": [[4, 5], 2], "expect": 5}]}]}"#,
    );
    let (_, out) = run(&temp.path, &["try", path.as_str()]);
    assert!(
        out.contains("index 2 is out of range for length 2 at /fns/0/body/0/1\n"),
        "{out}"
    );
    // A Result read (`get?Case`) fails as a value, not a trap: no probe.
    let path = write(
        &temp.path,
        "nth_ok.json",
        r#"{"af1": 1, "afx": 1,
 "types": [{"name": "Missing", "variant": ["Gone"]}],
 "fns": [{"fn": "nth_ok", "params": [["xs", "Vec<i64>"], ["i", "u64"]], "returns": "Result<i64,Missing>",
          "body": [["ok", ["get?Gone", "xs", "i"]]]}],
 "test_tables": [{"name": "nth_ok_cases", "fn": "nth_ok", "cases": [
   {"args": [[4, 5], 2], "expect": {"Ok": 5}}]}]}"#,
    );
    let (_, out) = run(&temp.path, &["try", path.as_str()]);
    assert!(out.contains("FAIL nth_ok_cases_0"), "{out}");
    assert!(!out.contains("out of range"), "{out}");
}

#[test]
fn layered_text_locates_its_own_functions_only() {
    let temp = TempDir::new("layered");
    let first = tests_of(&temp.path, "reads.py", READS, None);
    assert_eq!(read(&first, "test_reads_1").2, Some(2));
    let layered = tests_of(
        &temp.path,
        "more.py",
        "\
def first(xs: list[int]) -> int:
    return xs[0]

def test_first():
    assert first([]) == 0
",
        Some("d1"),
    );
    // The new text's function sits after the draft's in the layered frame,
    // and locates at its own line.
    assert_eq!(
        read(&layered, "test_first_0"),
        ("0".to_owned(), "0".to_owned(), Some(2))
    );
    // The draft's functions are not in this text: their pointer, no line.
    let (index, length, line) = read(&layered, "test_reads_1");
    assert_eq!((index.as_str(), length.as_str(), line), ("3", "3", None));
    assert_eq!(
        named(&layered, "test_reads_1")["index_read"]["at"],
        "/fns/0/body/0/1"
    );
}

#[test]
fn stores_of_computed_values_keep_every_note() {
    let temp = TempDir::new("stores");
    let tests = tests_of(
        &temp.path,
        "stores.py",
        "\
def rev(xs: list[int]) -> list[int]:
    out = [0] * len(xs)
    for i, x in enumerate(xs):
        out[len(xs) - i] = x + 1
    return out

def bump(n: int, i: int) -> list[int]:
    ys = [0] * n
    ys[i] += 5
    return ys

def at(xs: list[int], i: int) -> int:
    return xs[i]

def test_stores():
    assert rev([1, 2, 3]) == [4, 3, 2]
    assert bump(2, 2) == [0, 0]
    assert at([1, 2, 3], 3) == 0
    assert bump(2, 1) == [0, 5]
",
        None,
    );
    let s = |text: &str| text.to_owned();
    assert_eq!(read(&tests, "test_stores_0"), (s("3"), s("3"), Some(4)));
    // `ys[i] += 5` reads `ys[i]` first: the read's note.
    assert_eq!(read(&tests, "test_stores_1"), (s("2"), s("2"), Some(9)));
    assert_eq!(read(&tests, "test_stores_2"), (s("3"), s("3"), Some(13)));
    assert_eq!(named(&tests, "test_stores_3")["pass"], true);
}

#[test]
fn a_computed_negative_index_says_it_does_not_count_from_the_end() {
    let temp = TempDir::new("negative");
    let path = write(
        &temp.path,
        "neg.py",
        "\
def at(xs: list[int], i: int) -> int:
    return xs[i]

def test_neg():
    assert at([1, 2], -1) == 2
    assert at([1, 2], -3) == 0
",
    );
    let (_, out) = run(&temp.path, &["try", "--familiar", path.as_str()]);
    assert!(
        out.contains(
            "index -1 is out of range for length 2 (a computed negative index does not count from the end) (line 2)"
        ),
        "{out}"
    );
    // Past the start in Python too: no aside.
    assert!(
        out.contains("index -3 is out of range for length 2 (line 2)"),
        "{out}"
    );
}

//! The optional familiar frontend (ADR-0055): familiar text and the
//! equivalent structured JSON reach the same frame and the same lowering;
//! nested control flow, carried variables, scoping and early exits keep Sley
//! semantics; unsupported text is refused at its line and column; refusals
//! inside the body map back to the text; and a committed program needs no
//! frontend afterwards.
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
            "sley-familiar-{label}-{}-{}",
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

/// Writes `text` next to the workspace and tries it with `--familiar`.
fn try_text(dir: &Path, text: &str, json: bool) -> (i32, String) {
    let path = dir.join("proposal.txt");
    fs::write(&path, text).unwrap();
    let path = path.display().to_string();
    let mut words = vec!["try", "--familiar", path.as_str(), "--no-test"];
    if json {
        words.insert(0, "--json");
    }
    run(dir, &words)
}

fn valid_text(dir: &Path, text: &str) -> String {
    let (status, out) = try_text(dir, text, true);
    assert_eq!(status, 0, "{out}");
    let report: Value = serde_json::from_str(out.trim()).unwrap();
    report["handle"].as_str().unwrap().to_owned()
}

fn valid_frame(dir: &Path, frame: &Value) -> String {
    let (status, report) = run_json(dir, &["try", &frame.to_string(), "--no-test"]);
    assert_eq!(status, 0, "{report:#}");
    report["handle"].as_str().unwrap().to_owned()
}

fn call(dir: &Path, handle: &str, name: &str, args: &[Value]) -> Value {
    let mut words: Vec<String> = vec!["call".into(), name.into()];
    words.extend(args.iter().map(Value::to_string));
    words.extend(["--on".into(), handle.into()]);
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    let (_, report) = run_json(dir, &refs);
    report.get("result").cloned().unwrap_or(report)
}

/// The latest revision directory of draft `d1`.
fn revision(dir: &Path, number: u64) -> PathBuf {
    let found: Vec<PathBuf> = walk(dir)
        .into_iter()
        .filter(|path| path.ends_with(format!("d1/r{number}/status.json")))
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    found[0].parent().unwrap().to_path_buf()
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

/// The lowered blocks Sley built, without per-run identities.
fn lowered(dir: &Path, handle: &str) -> String {
    run(dir, &["view", "--after", handle])
        .1
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split("   [").next().unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

const NESTED: &str = "\
# nested loops, carried state, early exits
fn scan(xs: Vec<i64>, limit: i64) -> Result<i64,ArithmeticError> {
    var total: i64 = 0
    var seen: u64 = 0
    for x in xs {
        if x < 0 { trap }
        var k: i64 = x
        while k > 0 {
            total = try(total + k)
            if total > limit { return ok(-1) }
            k = k - 1
        }
        seen = seen + 1
    }
    return ok(total * to(i64, seen) if seen > 0 else 0)
}
";

fn nested_json() -> Value {
    json!({"af1": 1, "afx": 1, "fns": [{
        "fn": "scan",
        "params": [["xs", "Vec<i64>"], ["limit", "i64"]],
        "returns": "Result<i64,ArithmeticError>",
        "body": [
            ["var", "total", "i64", 0],
            ["var", "seen", "u64", 0],
            ["for", "x", "xs", [
                ["if", ["lt", "x", 0], [["trap"]]],
                ["var", "k", "i64", "x"],
                ["while", ["gt", "k", 0], [
                    ["set", "total", ["add?", "total", "k"]],
                    ["if", ["gt", "total", "limit"], [["ok", -1]]],
                    ["set", "k", ["sub", "k", 1]]
                ]],
                ["set", "seen", ["add", "seen", 1]]
            ]],
            ["ok", ["if", ["gt", "seen", 0], ["mul", "total", ["to", "i64", "seen"]], 0]]
        ]
    }]})
}

#[test]
fn familiar_text_and_structured_json_reach_the_same_frame_and_lowering() {
    let parsed = sley_agent::familiar::parse(NESTED).unwrap();
    assert_eq!(parsed.frame, nested_json());
    let (a, b) = (workspace("same-text"), workspace("same-json"));
    let from_text = valid_text(&a.path, NESTED);
    let from_json = valid_frame(&b.path, &nested_json());
    assert_eq!(lowered(&a.path, &from_text), lowered(&b.path, &from_json));
    // The draft keeps the text as its input and the structured frame as the
    // frame every later step reads.
    let rev = revision(&a.path, 1);
    assert_eq!(fs::read_to_string(rev.join("input.txt")).unwrap(), NESTED);
    let frame: Value = serde_json::from_slice(&fs::read(rev.join("frame.json")).unwrap()).unwrap();
    assert_eq!(frame, nested_json());
    for args in [
        json!([[1, 2], 100]),
        json!([[3], 4]),
        json!([[2, -1], 100]),
        json!([[], 5]),
        json!([[0, 0], 5]),
    ] {
        let args = args.as_array().unwrap();
        assert_eq!(
            call(&a.path, &from_text, "scan", args),
            call(&b.path, &from_json, "scan", args),
            "{args:?}"
        );
    }
    // 1 + (2 + 1) = 4, two elements seen: 8; total 6 > 4 exits early with -1;
    // a negative element aborts with a trap.
    assert_eq!(
        call(&a.path, &from_text, "scan", &[json!([1, 2]), json!(100)]),
        json!({"Ok": 8})
    );
    assert_eq!(
        call(&a.path, &from_text, "scan", &[json!([3]), json!(4)]),
        json!({"Ok": -1})
    );
    assert!(
        call(&a.path, &from_text, "scan", &[json!([2, -1]), json!(100)])
            .to_string()
            .contains("trap")
    );
    // An overflow inside try(..) returns the failure from the middle of the
    // loop instead of trapping.
    assert_eq!(
        call(
            &a.path,
            &from_text,
            "scan",
            &[json!([i64::MAX, 2]), json!(i64::MAX)]
        ),
        json!({"Err": {"ArithmeticError": "Overflow"}})
    );
}

#[test]
fn scoping_rebinds_in_one_list_and_refuses_redeclaring_an_outer_name() {
    let temp = workspace("scope");
    // Same-list rebinding is unambiguous: the later binding wins.
    let handle = valid_text(
        &temp.path,
        "fn f(a: i64) -> i64 {\n  let x = a + 1\n  let x = x * 2\n  return x\n}\n",
    );
    assert_eq!(call(&temp.path, &handle, "f", &[json!(3)]), json!(8));
    // A variable declared inside a block is gone after it.
    let (status, out) = try_text(
        &temp.path,
        "fn g(c: bool) -> i64 {\n  if c { let y = 1 }\n  return y\n}\n",
        false,
    );
    assert_eq!(status, 2, "{out}");
    assert!(
        out.contains("unknown name `y`") && out.contains("line 3, column 10"),
        "{out}"
    );
    // Redeclaring an outer name, a parameter or a loop variable inside a
    // nested block is refused for text and JSON alike.
    for (text, pointer, line) in [
        (
            "fn h(c: bool) -> i64 {\n  let x = 1\n  if c { let x = 2 }\n  return x\n}\n",
            "/fns/0/body/1/2/0",
            3,
        ),
        (
            "fn h(c: bool, n: i64) -> i64 {\n  var t: i64 = 0\n  while c { var n: i64 = 1\n t = n }\n  return t\n}\n",
            "/fns/0/body/1/2/0",
            3,
        ),
        (
            "fn h(xs: Vec<i64>) -> i64 {\n  let x = 0\n  for x in xs { }\n  return x\n}\n",
            "/fns/0/body/1/1",
            3,
        ),
    ] {
        let (status, out) = try_text(&temp.path, text, false);
        assert_eq!(status, 2, "{out}");
        assert!(out.contains("declared outside"), "{out}");
        assert!(out.contains(&format!("{pointer} is line {line}")), "{out}");
        let frame = sley_agent::familiar::parse(text).unwrap().frame;
        let (status, json_out) = run(&temp.path, &["try", &frame.to_string(), "--no-test"]);
        assert_eq!(status, 2, "{json_out}");
        assert!(json_out.contains(&format!("{pointer}: ")), "{json_out}");
    }
}

#[test]
fn unsupported_text_is_refused_and_kept_without_a_candidate() {
    let temp = workspace("refused");
    let text =
        "fn f(xs: Vec<i64>) -> i64 {\n  var t: i64 = 0\n  for x in xs { t += x }\n  return t\n}\n";
    let (status, report) = {
        let (status, out) = try_text(&temp.path, text, true);
        (status, serde_json::from_str::<Value>(out.trim()).unwrap())
    };
    assert_eq!(status, 2, "{report:#}");
    assert_eq!(report["state"], "text", "{report:#}");
    let detail = report["detail"].as_str().unwrap();
    assert!(
        detail.contains("line 3, column 19") && detail.contains("compound assignment"),
        "{detail}"
    );
    assert_eq!(
        report["obligations"][0]["text"],
        json!({"line": 3, "column": 19})
    );
    let rev = revision(&temp.path, 1);
    assert_eq!(fs::read_to_string(rev.join("input.txt")).unwrap(), text);
    assert!(!rev.join("frame.json").exists());
    // The frontend is selected explicitly: the same text without the switch
    // is just text that is not JSON.
    let path = temp.path.join("proposal.txt").display().to_string();
    let (status, out) = run(&temp.path, &["try", &path, "--no-test"]);
    assert_eq!(status, 2);
    assert!(out.contains("not JSON"), "{out}");
    // And it never reads inline text.
    let (status, out) = run(
        &temp.path,
        &["try", "--familiar", "fn f() -> i64 { return 1 }"],
    );
    assert_eq!(status, 2);
    assert!(out.contains("reads a file or -"), "{out}");
}

#[test]
fn refusals_inside_the_body_name_the_authored_line_and_column() {
    let temp = workspace("diagnostics");
    // A lowering refusal: integer types never mix.
    let (status, out) = try_text(
        &temp.path,
        "fn f(xs: Vec<i64>, k: i64) -> i64 {\n  var t: i64 = 0\n  for x in xs {\n    t = t + len(xs)\n  }\n  return t + k\n}\n",
        false,
    );
    assert_eq!(status, 2, "{out}");
    assert!(out.contains("operands mix i64 and u64"), "{out}");
    assert!(
        out.contains("/fns/0/body/1/3/0/2/2 is line 4, column 13"),
        "{out}"
    );
    // A compiler refusal inside the lowered blocks: an error case the
    // declared type does not have.
    let types = json!({"af1": 1, "afx": 1, "types": [{"name": "GError", "variant": ["Small"]}]});
    let (status, _) = run(&temp.path, &["try", &types.to_string(), "--no-test"]);
    assert_eq!(status, 0);
    let (status, out) = run(&temp.path, &["commit"]);
    assert_eq!(status, 0, "{out}");
    let (status, out) = try_text(
        &temp.path,
        "fn g(x: i64) -> Result<i64,GError> {\n  if x < 0 {\n    return err(Negative)\n  }\n  return ok(x)\n}\n",
        true,
    );
    assert_eq!(status, 2, "{out}");
    let report: Value = serde_json::from_str(out.trim()).unwrap();
    let located = report["obligations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["at"] == "/fns/0/body/0/2/0")
        .unwrap_or_else(|| panic!("{report:#}"));
    assert_eq!(located["text"], json!({"line": 3, "column": 5}));
}

#[test]
fn a_committed_program_needs_no_frontend() {
    // The same program committed from text and from JSON is the same
    // program: identical lowering, results and exported state shape.
    let (a, b) = (workspace("commit-text"), workspace("commit-json"));
    let from_text = valid_text(&a.path, NESTED);
    let from_json = valid_frame(&b.path, &nested_json());
    for (dir, handle) in [(&a.path, &from_text), (&b.path, &from_json)] {
        let (status, out) = run(dir, &["commit", handle]);
        assert_eq!(status, 0, "{out}");
    }
    let view = |dir: &Path| {
        run(dir, &["view", "scan"])
            .1
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| line.split("   [").next().unwrap_or(line).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(view(&a.path), view(&b.path));
    // Every later step on the text-made program uses the ordinary JSON and
    // graph paths: query, call, edit by a JSON frame, validate, export.
    let (status, report) = run_json(&a.path, &["call", "scan", "[1, 2]", "100"]);
    assert_eq!(status, 0, "{report:#}");
    assert_eq!(report["result"], json!({"Ok": 8}));
    let edit = json!({"af1": 1, "afx": 1, "fns": [{
        "fn": "scan", "params": [["xs", "Vec<i64>"], ["limit", "i64"]],
        "returns": "Result<i64,ArithmeticError>",
        "body": [["ok", ["to?", "i64", ["len", "xs"]]]]
    }]});
    let handle = valid_frame(&a.path, &edit);
    assert_eq!(
        call(&a.path, &handle, "scan", &[json!([5, 6, 7]), json!(0)]),
        json!({"Ok": 3})
    );
    let pack = a.path.join("exported.pack").display().to_string();
    let (status, out) = run(&a.path, &["export", &pack]);
    assert_eq!(status, 0, "{out}");
    assert!(fs::metadata(&pack).unwrap().len() > 0);
    // Nothing the workspace keeps as program state is the text.
    let repo = a.path.join("repo");
    if repo.is_dir() {
        for file in walk(&repo) {
            let bytes = fs::read(&file).unwrap();
            assert!(
                !bytes.windows(9).any(|window| window == b"var total"),
                "{}",
                file.display()
            );
        }
    }
}

#[test]
fn the_familiar_help_example_runs() {
    let help = sley_agent::help::topic("familiar").unwrap();
    let start = help.find("fn product").unwrap();
    let temp = workspace("help");
    let handle = valid_text(&temp.path, &help[start..]);
    assert_eq!(
        call(&temp.path, &handle, "product", &[json!([2, -3, 4])]),
        json!({"Ok": -24})
    );
    let (status, out) = run(&temp.path, &["help", "familiar"]);
    assert_eq!(status, 0);
    assert!(out.contains("try --familiar"), "{out}");
}

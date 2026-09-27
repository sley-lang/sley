//! Focused views (`view --focus`) and the AF1-X-shaped rendering
//! (`view --x`): parity with the full view, byte stability, the output
//! bound and its expansion commands, exact-pattern sugaring with fallback,
//! and the JSON shape (`docs/spec/SLEY_AGENT_V1.md` section 4).

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
            "sley-agent-views-{label}-{}-{}",
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

const SEED: [u8; 32] = [7; 32];
const BOUND: usize = 4000;

fn workspace(label: &str) -> TempDir {
    let temp = TempDir::new(label);
    genesis::init(&temp.path, Some(SEED), genesis::INIT_CEILINGS).unwrap();
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

/// Runs a printed `sley-agent ...` command line in the workspace.
fn run_printed(dir: &Path, command: &str) -> (i32, String) {
    let words: Vec<&str> = command.split_whitespace().collect();
    assert_eq!(words[0], "sley-agent", "{command}");
    run(dir, &words[1..])
}

fn try_ok(dir: &Path, frame: &Value) {
    let (status, text) = run(dir, &["try", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
}

/// `text` with every eight-digit identity (`root=…`, `[…]`) masked.
fn mask_ids(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let line = match line.split_once("root=") {
            Some((head, rest)) => format!("{head}root=#{}", &rest[8..]),
            None => line.to_owned(),
        };
        let line = match line.rsplit_once("   [") {
            Some((head, rest)) if rest.len() == 9 && rest.ends_with(']') => format!("{head}   [#]"),
            _ => line,
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// The text after the header line.
fn body(view: &str) -> &str {
    view.split_once('\n').map_or("", |(_, rest)| rest)
}

/// A plain program without generated names: a clamping function, the type
/// it names, a constant it reads, a helper it calls and a caller.
fn plain_frame() -> Value {
    json!({"af1": 1,
      "types": [{"name": "RangeError", "variant": ["Inverted"]}],
      "consts": [{"name": "limit", "type": "i64", "value": 100}],
      "fns": [
        {"fn": "floor", "params": [["x", "i64"]], "returns": "i64", "visibility": "private",
         "blocks": [{"name": "entry", "term": ["return", "x"]}]},
        {"fn": "bound", "params": [["value", "i64"], ["low", "i64"], ["high", "i64"]],
         "returns": "Result<i64,RangeError>",
         "blocks": [
          {"name": "entry", "ops": [["inverted", "lt", "high", "low"]], "term": ["cond", "inverted", "invalid", "check"]},
          {"name": "invalid", "ops": [["e", "variant", "RangeError.Inverted"], ["r", "err", "e"]], "term": ["return", "r"]},
          {"name": "check", "ops": [["cap", "const", "limit"], ["over", "gt", "value", "cap"]], "term": ["cond", "over", "above", "inside"]},
          {"name": "above", "ops": [["r", "ok", "high"]], "term": ["return", "r"]},
          {"name": "inside", "ops": [["f", "call", "floor", "value"], ["r", "ok", "f"]], "term": ["return", "r"]}]},
        {"fn": "use_bound", "params": [["v", "i64"]], "returns": "Result<i64,RangeError>",
         "blocks": [{"name": "entry", "ops": [["r", "call", "bound", "v", "v", "v"]], "term": ["return", "r"]}]}],
      "tests": [
        {"fn": "bound", "args": [5, 0, 10], "expect": {"Ok": 5}, "name": "bound-inside"},
        {"fn": "bound", "args": [5, 10, 0], "expect": {"Err": "RangeError.Inverted"}, "name": "bound-inverted"}]})
}

fn plain_program(label: &str) -> TempDir {
    let temp = workspace(label);
    try_ok(&temp.path, &plain_frame());
    temp
}

fn fail_block(case: &str) -> Value {
    json!({"name": format!("__fail_{case}"),
           "ops": [["v", "variant", format!("CalcError.{case}")], ["e", "err", "v"]],
           "term": ["return", "e"]})
}

/// Functions in the shape the authoring-dialect expansion produces,
/// written as plain AF1: checked operations, conditional exits, hoisted
/// literals and nested operations, `ok`, `fail`, and the shared exits.
fn expanded_frame() -> Value {
    json!({"af1": 1,
     "types": [{"name": "CalcError", "variant": ["Overflow", "DivZero", ["Bad", "i64"]]}],
     "fns": [
      {"fn": "pick", "params": [["x", "i64"], ["y", "i64"]], "returns": "i64",
       "blocks": [{"name": "entry", "term": ["return", "x"]}]},
      {"fn": "parse", "params": [["a", "i64"]], "returns": "Result<i64,i64>",
       "blocks": [{"name": "entry", "ops": [["r", "err", "a"]], "term": ["return", "r"]}]},
      {"fn": "calc", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,CalcError>",
       "blocks": [
        {"name": "entry", "ops": [["s__a1", "const", {"type": "i64", "value": 3}], ["s__r", "add", "a", "s__a1"]],
         "term": ["switch", "s__r", ["Ok", "entry__s", "$"], ["Err", "__fail_Overflow"]]},
        {"name": "entry__s", "params": [["s", "i64"]],
         "ops": [["entry__if0__a1", "const", {"type": "i64", "value": 0}], ["entry__if0", "eq", "b", "entry__if0__a1"]],
         "term": ["cond", "entry__if0", "__fail_DivZero", ["entry__if0", "s"]]},
        {"name": "entry__if0", "params": [["s", "i64"]], "ops": [["q__r", "div", "s", "b"]],
         "term": ["switch", "q__r", ["Ok", "entry__q", "$", "s"], ["Err", "__fail_DivZero"]]},
        {"name": "entry__q", "params": [["q", "i64"], ["s", "i64"]],
         "ops": [["m__a0", "call", "pick", "q", "s"], ["m__a1", "const", {"type": "i64", "value": 4}],
                 ["m", "call", "pick", "m__a0", "m__a1"], ["entry__ok", "ok", "m"]],
         "term": ["return", "entry__ok"]},
        fail_block("Overflow"),
        fail_block("DivZero")]},
      {"fn": "check", "params": [["a", "i64"]], "returns": "Result<i64,CalcError>",
       "blocks": [
        {"name": "entry", "ops": [["entry__if0__a1", "const", {"type": "i64", "value": 0}], ["entry__if0", "lt", "a", "entry__if0__a1"]],
         "term": ["cond", "entry__if0", ["__fail_Bad", "a"], "entry__if0"]},
        {"name": "entry__if0", "term": ["br", "__fail_Overflow"]},
        {"name": "__fail_Bad", "params": [["p", "i64"]],
         "ops": [["v", "variant", "CalcError.Bad", "p"], ["e", "err", "v"]], "term": ["return", "e"]},
        fail_block("Overflow")]},
      {"fn": "safe", "params": [["a", "i64"]], "returns": "Result<i64,CalcError>",
       "blocks": [
        {"name": "entry", "ops": [["n__r", "call", "parse", "a"]],
         "term": ["switch", "n__r", ["Ok", "entry__n", "$"], ["Err", "__fail_Bad", "$"]]},
        {"name": "entry__n", "params": [["n", "i64"]], "ops": [["entry__ok", "ok", "n"]], "term": ["return", "entry__ok"]},
        {"name": "__fail_Bad", "params": [["p", "i64"]],
         "ops": [["v", "variant", "CalcError.Bad", "p"], ["e", "err", "v"]], "term": ["return", "e"]}]},
      {"fn": "pass", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
       "blocks": [
        {"name": "entry", "ops": [["s__a1", "const", {"type": "i64", "value": 1}], ["s__r", "add", "a", "s__a1"]],
         "term": ["switch", "s__r", ["Ok", "entry__s", "$"], ["Err", "__err", "$"]]},
        {"name": "entry__s", "params": [["s", "i64"]], "ops": [["entry__ok", "ok", "s"]], "term": ["return", "entry__ok"]},
        {"name": "__err", "params": [["e", "ArithmeticError"]], "ops": [["r", "err", "e"]], "term": ["return", "r"]}]},
      {"fn": "first", "params": [["v", "Vec<i64>"], ["i", "u64"]], "returns": "Option<i64>",
       "blocks": [
        {"name": "entry", "ops": [["x__r", "vec_get", "v", "i"]],
         "term": ["switch", "x__r", ["Some", "entry__x", "$"], ["None", "__none"]]},
        {"name": "entry__x", "params": [["x", "i64"]],
         "ops": [["c__a1", "const", {"type": "i64", "value": 0}], ["c", "lt", "x", "c__a1"]],
         "term": ["cond", "c", "neg", ["pos", "x"]]},
        {"name": "neg", "term": ["br", "__none"]},
        {"name": "pos", "params": [["x", "i64"]], "ops": [["r", "some", "x"]], "term": ["return", "r"]},
        {"name": "__none", "ops": [["r", "none"]], "term": ["return", "r"]}]}],
     "tests": [{"fn": "calc", "args": [4, 2], "expect": {"Ok": 3}, "name": "calc-ok"},
               {"fn": "calc", "args": [4, 0], "expect": {"Err": "CalcError.DivZero"}, "name": "calc-zero"},
               {"fn": "check", "args": [-2], "expect": {"Err": {"Bad": -2}}, "name": "check-bad"},
               {"fn": "safe", "args": [7], "expect": {"Err": {"Bad": 7}}, "name": "safe-bad"},
               {"fn": "first", "args": [[5], 0], "expect": {"Some": 5}, "name": "first-some"},
               {"fn": "first", "args": [[5], 3], "expect": "None", "name": "first-none"}]})
}

fn expanded_program(label: &str) -> TempDir {
    let temp = workspace(label);
    let (status, text) = run(&temp.path, &["try", &expanded_frame().to_string()]);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("tests: 6/6 passed"), "{text}");
    temp
}

#[test]
fn x_view_sugars_exact_expansion_shapes() {
    let temp = expanded_program("x-shapes");
    let (status, view) = run(
        &temp.path,
        &[
            "view", "--x", "--after", "c1", "calc", "check", "safe", "pass",
        ],
    );
    assert_eq!(status, 0, "{view}");
    let header = view.lines().next().unwrap();
    assert!(
        header.starts_with("# sley view (AV1-X, non-canonical, output only) root=")
            && header.ends_with(" after=c1"),
        "{view}"
    );
    let expected = "\
fn calc(a: i64, b: i64) -> Result<i64,CalcError>   [#]
  entry:
    s = add?Overflow a, 3   # entry.s__a1, entry.s__r, entry__s, __fail_Overflow
    !DivZero if (eq b, 0)   # entry__s.entry__if0__a1, entry__s.entry__if0, entry__if0, __fail_DivZero
    q = div?DivZero s, b   # entry__if0.q__r, entry__q, __fail_DivZero
    m = call pick (call pick q, s), 4   # entry__q.m__a0, entry__q.m__a1
    ok m   # entry__q.entry__ok
fn check(a: i64) -> Result<i64,CalcError>   [#]
  entry:
    !Bad if (lt a, 0), a   # entry.entry__if0__a1, entry.entry__if0, entry__if0, __fail_Bad
    fail Overflow   # __fail_Overflow
fn safe(a: i64) -> Result<i64,CalcError>   [#]
  entry:
    n = call?Bad parse a   # entry.n__r, entry__n, __fail_Bad
    ok n   # entry__n.entry__ok
fn pass(a: i64) -> Result<i64,ArithmeticError>   [#]
  entry:
    s = add? a, 1   # entry.s__a1, entry.s__r, entry__s, __err
    ok s   # entry__s.entry__ok
";
    let masked: String = body(&view)
        .lines()
        .map(|line| match line.rsplit_once("   [") {
            Some((head, _)) if line.starts_with("fn ") => format!("{head}   [#]\n"),
            _ => format!("{line}\n"),
        })
        .collect();
    assert_eq!(masked, expected);
    // Every name a route comment gives resolves as `<fn>.<name>` and shows
    // the expanded function.
    let mut function = "";
    for line in body(&view).lines() {
        if let Some(rest) = line.strip_prefix("fn ") {
            function = rest.split('(').next().unwrap();
        }
        let Some((_, route)) = line.split_once("   # ") else {
            continue;
        };
        for name in route.split(", ") {
            let qualified = format!("{function}.{name}");
            let (status, shown) = run(&temp.path, &["view", "--after", "c1", &qualified]);
            assert_eq!(status, 0, "{qualified}: {shown}");
            assert!(shown.contains(&format!("fn {function}(")), "{shown}");
        }
    }
    // The Option form: bare `?` and `fail` to `__none`, which is hidden
    // because every edge into it is sugared.
    let (_, first) = run(&temp.path, &["view", "--x", "--after", "c1", "first"]);
    let expected = "\
  entry:
    x = vec_get? v, i   # entry.x__r, entry__x, __none
    c = lt x, 0   # entry__x.c__a1
    cond c -> neg, pos(x)
  neg:
    fail   # __none
  pos(x: i64):
    r = some x
    return r
";
    assert!(body(&first).ends_with(expected), "{first}");
}

/// A change to the expanded frame that breaks one pattern.
type Change = Box<dyn Fn(&mut Value)>;

#[test]
fn x_view_falls_back_to_av1_when_a_pattern_is_not_exact() {
    let base = expanded_frame();
    let variants: Vec<(&str, Change)> = vec![
        // The failure block does more than variant, err, return.
        (
            "extra-op",
            Box::new(|frame: &mut Value| {
                frame["fns"][2]["blocks"][4]["ops"]
                    .as_array_mut()
                    .unwrap()
                    .insert(0, json!(["w", "const", {"type": "i64", "value": 9}]));
            }),
        ),
        // The failure block builds a different case than its name says.
        (
            "wrong-case",
            Box::new(|frame: &mut Value| {
                frame["fns"][2]["blocks"][4]["ops"][0] =
                    json!(["v", "variant", "CalcError.DivZero"]);
            }),
        ),
        // A threaded value changes its name at the continuation.
        (
            "renamed",
            Box::new(|frame: &mut Value| {
                let blocks = frame["fns"][2]["blocks"].as_array_mut().unwrap();
                blocks[3]["params"][1][0] = json!("t");
                blocks[3]["ops"][0] = json!(["m__a0", "call", "pick", "q", "t"]);
            }),
        ),
        // A continuation defines a value under a leaf the region has.
        (
            "shadowed",
            Box::new(|frame: &mut Value| {
                let block = &mut frame["fns"][2]["blocks"][1];
                block["ops"][0][0] = json!("s__a1");
                block["ops"][1] = json!(["entry__if0", "eq", "b", "s__a1"]);
            }),
        ),
        // An unrelated operation sits between a hoisted literal and its use.
        (
            "interleaved",
            Box::new(|frame: &mut Value| {
                frame["fns"][2]["blocks"][0]["ops"]
                    .as_array_mut()
                    .unwrap()
                    .insert(1, json!(["z", "lt", "a", "b"]));
            }),
        ),
    ];
    for (label, change) in variants {
        let temp = workspace(label);
        let mut frame = base.clone();
        change(&mut frame);
        let (status, text) = run(&temp.path, &["try", &frame.to_string()]);
        assert!(status == 0 || status == 1, "{label}: {text}");
        let (_, av1) = run(&temp.path, &["view", "--after", "c1", "calc"]);
        let (_, x) = run(&temp.path, &["view", "--x", "--after", "c1", "calc"]);
        match label {
            "extra-op" | "wrong-case" => {
                // The first checked region keeps its switch; the rest still
                // sugars, and the non-exact exit block stays in AV1.
                assert!(
                    x.contains("    s__r = add a, 3   # entry.s__a1\n"),
                    "{label}: {x}"
                );
                assert!(
                    x.contains("    switch s__r: Ok -> entry__s($), Err -> __fail_Overflow\n"),
                    "{label}: {x}"
                );
                assert!(x.contains("  entry__s(s: i64):\n"), "{label}: {x}");
                assert!(x.contains("  __fail_Overflow:\n"), "{label}: {x}");
                let exit = av1.split("  __fail_Overflow:\n").nth(1).unwrap();
                assert!(
                    x.contains(&exit[..exit.find("  __fail_DivZero").unwrap()]),
                    "{label}: {x}"
                );
            }
            "renamed" => {
                assert!(
                    x.contains("    switch q__r: Ok -> entry__q($, s), Err -> __fail_DivZero\n"),
                    "{label}: {x}"
                );
                assert!(x.contains("  entry__q(q: i64, t: i64):\n"), "{label}: {x}");
                assert!(x.contains("  __fail_DivZero:\n"), "{label}: {x}");
            }
            "shadowed" => {
                assert!(
                    x.contains("    switch s__r: Ok -> entry__s($), Err -> __fail_Overflow\n"),
                    "{label}: {x}"
                );
                assert!(x.contains("  entry__s(s: i64):\n    s__a1 = const k_0 (0)\n    !DivZero if (eq b, s__a1)"), "{label}: {x}");
            }
            _ => {
                assert!(
                    x.contains("    s__a1 = const k_3 (3)\n    z = lt a, b\n"),
                    "{label}: {x}"
                );
                assert!(
                    x.contains("    s = add?Overflow a, s__a1   # entry.s__r"),
                    "{label}: {x}"
                );
            }
        }
    }
}

#[test]
fn x_view_without_generated_names_equals_av1() {
    let temp = plain_program("x-plain");
    for args in [
        vec!["view"],
        vec!["view", "--after", "c1"],
        vec![
            "view",
            "--after",
            "c1",
            "bound",
            "use_bound",
            "RangeError",
            "limit",
            "bound-inside",
        ],
        vec![
            "view",
            "--after",
            "c1",
            "--package",
            "--types",
            "--ids",
            "--limits",
        ],
        vec!["view", "--after", "c1", "--focus", "bound"],
    ] {
        let (status, av1) = run(&temp.path, &args);
        assert_eq!(status, 0, "{av1}");
        let mut with_x = args.clone();
        with_x.push("--x");
        let (status, x) = run(&temp.path, &with_x);
        assert_eq!(status, 0, "{x}");
        assert!(
            av1.starts_with("# sley view (AV1, non-canonical) root="),
            "{av1}"
        );
        assert!(
            x.starts_with("# sley view (AV1-X, non-canonical, output only) root="),
            "{x}"
        );
        assert_eq!(body(&av1), body(&x), "{args:?}");
    }
}

#[test]
fn x_view_is_never_accepted_as_input() {
    let temp = expanded_program("x-no-parser");
    let (_, view) = run(&temp.path, &["view", "--x", "--after", "c1", "calc"]);
    assert!(view.contains("add?Overflow"), "{view}");
    let (status, text) = run(&temp.path, &["try", &view]);
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("AGENT_IO_FAILED") || text.contains("AGENT_FRAME_INVALID"),
        "{text}"
    );
    let (status, text) = run(&temp.path, &["try", body(&view)]);
    assert_eq!(status, 2, "{text}");
}

#[test]
fn focused_view_lists_the_directly_relevant_context() {
    let temp = plain_program("focus-context");
    let (status, view) = run(&temp.path, &["view", "--focus", "bound", "--after", "c1"]);
    assert_eq!(status, 0, "{view}");
    let (_, full) = run(&temp.path, &["view", "--after", "c1", "bound"]);
    let (_, floor) = run(&temp.path, &["view", "--after", "c1", "floor"]);
    let floor_signature = body(&floor).lines().next().unwrap();
    let expected = format!(
        "{}# focus bound: context, not a completeness certificate\n{}\
types: 1\ntype RangeError = Inverted\n\
consts: 1\nconst limit: i64 = 100\n\
calls: 1\n{floor_signature}\n\
callers: 1: use_bound\n\
tests: 2: bound-inside, bound-inverted\n\
boundary: exported; effects none; namespace none; callers outside it: 1\n",
        full.lines().next().unwrap().to_owned() + "\n",
        body(&full)
    );
    assert_eq!(view, expected);
    let (_, helper) = run(&temp.path, &["view", "--focus", "floor", "--after", "c1"]);
    assert!(helper.contains("callers: 1: bound\n"), "{helper}");
    assert!(
        helper.contains("boundary: private; effects none;"),
        "{helper}"
    );
    // A function-scoped name focuses on its function.
    let (_, block) = run(
        &temp.path,
        &["view", "--focus", "bound.check.over", "--after", "c1"],
    );
    assert_eq!(block, view);
    // Types, constants and tests have their own context.
    let (_, ty) = run(
        &temp.path,
        &["view", "--focus", "RangeError", "--after", "c1"],
    );
    assert!(ty.contains("type RangeError = Inverted\ntypes: none\nused by: 2: bound, use_bound\nboundary: exported; namespace none; used outside it: 2\n"), "{ty}");
    let (_, constant) = run(&temp.path, &["view", "--focus", "limit", "--after", "c1"]);
    assert!(
        constant.contains("const limit: i64 = 100\ntypes: none\nused by: 1: bound\n"),
        "{constant}"
    );
    let (_, test_view) = run(
        &temp.path,
        &["view", "--focus", "bound-inside", "--after", "c1"],
    );
    assert!(
        test_view.contains(&format!(
            "calls: 1\n{}",
            body(&full).lines().next().unwrap()
        )),
        "{test_view}"
    );
    // Usage and name refusals.
    let (status, text) = run(&temp.path, &["view", "--focus", "bound", "floor"]);
    assert_eq!(status, 2, "{text}");
    assert!(text.contains("AGENT_USAGE_INVALID"), "{text}");
    let (status, text) = run(
        &temp.path,
        &["view", "--focus", "nothing_here", "--after", "c1"],
    );
    assert_eq!(status, 2, "{text}");
    assert!(text.contains("AGENT_NAME_UNKNOWN"), "{text}");
}

#[test]
fn focused_view_entity_section_matches_the_full_view_line_for_line() {
    let plain = plain_program("parity-plain");
    let expanded = expanded_program("parity-x");
    for (dir, name, x) in [
        (&plain.path, "bound", false),
        (&plain.path, "use_bound", false),
        (&expanded.path, "calc", false),
        (&expanded.path, "calc", true),
        (&expanded.path, "first", true),
    ] {
        let mut args = vec!["view", "--after", "c1"];
        if x {
            args.push("--x");
        }
        let mut focus_args = args.clone();
        focus_args.extend(["--focus", name]);
        args.push(name);
        let (_, full) = run(dir, &args);
        let (_, focused) = run(dir, &focus_args);
        assert_eq!(
            full.lines().next(),
            focused.lines().next(),
            "same header: {focused}"
        );
        let entity: Vec<&str> = body(&full).lines().collect();
        let section: Vec<&str> = focused.lines().skip(2).take(entity.len()).collect();
        assert_eq!(section, entity, "{name} (x: {x})");
        assert_eq!(
            focused
                .lines()
                .nth(2 + entity.len())
                .map(|line| line.starts_with("types: ")),
            Some(true),
            "{focused}"
        );
        // Every rendered context line is the first line of its own view.
        for line in focused.lines().skip(2 + entity.len()) {
            if let Some(rest) = line
                .strip_prefix("type ")
                .or_else(|| line.strip_prefix("const "))
            {
                let entity_name = rest.split([' ', ':', '<']).next().unwrap();
                let (_, own) = run(dir, &["view", "--after", "c1", entity_name]);
                assert_eq!(body(&own).lines().next(), Some(line), "{line}");
            }
            if let Some(rest) = line.strip_prefix("fn ") {
                let entity_name = rest.split(['(', '<']).next().unwrap();
                let (_, own) = run(dir, &["view", "--after", "c1", entity_name]);
                assert_eq!(body(&own).lines().next(), Some(line), "{line}");
            }
        }
    }
}

#[test]
fn focused_view_is_byte_stable() {
    let temp = expanded_program("focus-stable");
    for args in [
        vec!["view", "--focus", "calc", "--after", "c1"],
        vec!["view", "--focus", "calc", "--after", "c1", "--x"],
        vec!["--json", "view", "--focus", "calc", "--after", "c1"],
        vec!["view", "--x", "--after", "c1"],
    ] {
        let (status, first) = run(&temp.path, &args);
        assert_eq!(status, 0, "{first}");
        let (_, second) = run(&temp.path, &args);
        assert_eq!(first, second, "{args:?}");
    }
    // A fresh workspace built the same way renders the same text; only
    // the identities (fresh member nonces) differ.
    let again = expanded_program("focus-stable-2");
    let (_, one) = run(
        &temp.path,
        &["view", "--focus", "calc", "--after", "c1", "--x"],
    );
    let (_, two) = run(
        &again.path,
        &["view", "--focus", "calc", "--after", "c1", "--x"],
    );
    assert_eq!(mask_ids(&one), mask_ids(&two));
}

/// A function that calls thirty helpers over twelve record types, with
/// twenty callers and sixty tests: far more context than the bound holds.
fn large_frame() -> Value {
    let mut types = Vec::new();
    for i in 0..12 {
        types.push(json!({"name": format!("Record{i:02}WithALongishName"),
            "record": [["alpha_field", "i64"], ["beta_field", "i64"], ["gamma_field", "bool"]]}));
    }
    let mut fns = Vec::new();
    for i in 0..30 {
        fns.push(json!({"fn": format!("helper_{i:02}_with_a_long_name"),
            "params": [[format!("first_{i}"), "i64"], ["second_parameter", "i64"],
                       ["r", format!("Record{:02}WithALongishName", i % 12)]],
            "returns": "i64", "blocks": [{"name": "entry", "term": ["return", format!("first_{i}")]}]}));
    }
    let mut ops = Vec::new();
    let mut previous = "x".to_owned();
    for i in 0..30 {
        ops.push(json!([
            format!("rec{i}"),
            "record",
            format!("Record{:02}WithALongishName", i % 12),
            "x",
            "x",
            "flag"
        ]));
        ops.push(json!([
            format!("v{i}"),
            "call",
            format!("helper_{i:02}_with_a_long_name"),
            previous,
            "x",
            format!("rec{i}")
        ]));
        previous = format!("v{i}");
    }
    fns.push(
        json!({"fn": "big", "params": [["x", "i64"], ["flag", "bool"]], "returns": "i64",
        "blocks": [{"name": "entry", "ops": ops, "term": ["return", previous]}]}),
    );
    for i in 0..20 {
        fns.push(
            json!({"fn": format!("caller_{i:02}"), "params": [["x", "i64"]], "returns": "i64",
            "blocks": [{"name": "entry", "ops": [["t", "const", {"type": "bool", "value": true}],
                ["r", "call", "big", "x", "t"]], "term": ["return", "r"]}]}),
        );
    }
    let tests: Vec<Value> = (0..60)
        .map(|i| json!({"fn": "big", "args": [i, true], "expect": i, "name": format!("big-case-{i:03}")}))
        .collect();
    json!({"af1": 1, "types": types, "fns": fns, "tests": tests})
}

/// Runs every omission line's command in a focused view and checks it
/// shows exactly the stated count; returns the omission lines seen and the
/// `fn helper_` signatures shown.
fn check_omission_lines(dir: &Path, view: &str) -> (usize, usize) {
    let mut omissions = 0;
    let mut shown_calls = 0;
    for line in view.lines() {
        if line.starts_with("fn helper_") {
            shown_calls += 1;
        }
        if let Some(rest) = line
            .strip_prefix("# ")
            .filter(|rest| rest.contains(" more "))
        {
            omissions += 1;
            let (count, command) = rest.split_once(" more ").unwrap();
            let count: usize = count.parse().unwrap();
            let (section, command) = command.split_once(": ").unwrap();
            let (status, expanded) = run_printed(dir, command);
            assert_eq!(status, 0, "{command}: {expanded}");
            let prefix = if section == "calls" { "fn " } else { "type " };
            let shown = expanded
                .lines()
                .filter(|line| line.starts_with(prefix))
                .count();
            assert_eq!(shown, count, "{line}");
        }
        for section in ["callers", "tests"] {
            if let Some(rest) = line.strip_prefix(&format!("{section}: "))
                && let Some((count, command)) = rest.split_once(" (names: ")
            {
                omissions += 1;
                let command = command.strip_suffix(')').unwrap();
                let (status, listed) = run_printed(dir, command);
                assert_eq!(status, 0, "{listed}");
                let listed: Value = serde_json::from_str(listed.trim()).unwrap();
                assert_eq!(
                    listed["focus"][section].as_array().unwrap().len(),
                    count.parse::<usize>().unwrap()
                );
            }
        }
    }
    (omissions, shown_calls)
}

#[test]
fn focused_view_is_bounded_and_every_omission_expands() {
    let temp = workspace("focus-bound");
    try_ok(&temp.path, &large_frame());
    let (status, view) = run(&temp.path, &["view", "--focus", "big", "--after", "c1"]);
    assert_eq!(status, 0, "{view}");
    assert!(view.len() <= BOUND, "{} bytes:\n{view}", view.len());
    let (_, full) = run(&temp.path, &["view", "--after", "c1", "big"]);
    // The body gives way first only after the name lists and rendered
    // context; its signature stays, with the exact expansion command.
    assert_eq!(view.lines().nth(2), body(&full).lines().next());
    assert!(
        view.contains("\n# body omitted (62 lines): sley-agent view --after c1 big\n"),
        "{view}"
    );
    assert!(view.contains("\ntypes: 12\n"), "{view}");
    assert!(view.contains("\ncalls: 30\n"), "{view}");
    assert!(
        view.contains(
            "\nboundary: exported; effects none; namespace none; callers outside it: 20\n"
        ),
        "{view}"
    );
    let (_, focus) = run_json(&temp.path, &["view", "--focus", "big", "--after", "c1"]);
    let lists = &focus["focus"];
    assert_eq!(lists["types"].as_array().unwrap().len(), 12);
    assert_eq!(lists["calls"].as_array().unwrap().len(), 30);
    assert_eq!(lists["callers"].as_array().unwrap().len(), 20);
    assert_eq!(lists["tests"].as_array().unwrap().len(), 60);
    assert_eq!(lists["bytes"], json!(view.len()));
    assert_eq!(focus["view"].as_str().unwrap(), view);
    let (omissions, shown_calls) = check_omission_lines(&temp.path, &view);
    assert!(omissions >= 3, "{view}");
    assert!(shown_calls > 0 && shown_calls < 30, "{view}");
    let omitted = lists["omitted"].as_array().unwrap();
    assert!(
        omitted.iter().any(|entry| entry["section"] == "target"),
        "{omitted:?}"
    );
    for entry in omitted {
        let names = entry["names"].as_array().unwrap();
        if entry["section"] == "target" {
            assert_eq!(entry["count"], 62, "{entry}");
            assert_eq!(names, &[json!("big")], "{entry}");
        } else {
            assert_eq!(entry["count"], json!(names.len()), "{entry}");
        }
        assert!(
            entry["expand"].as_str().unwrap().starts_with("sley-agent "),
            "{entry}"
        );
    }
    // A smaller target keeps its body and drops context from the end.
    let (_, helper) = run(
        &temp.path,
        &[
            "view",
            "--focus",
            "helper_03_with_a_long_name",
            "--after",
            "c1",
        ],
    );
    assert!(helper.len() <= BOUND);
    assert!(!helper.contains("body omitted"), "{helper}");
    // The bound holds with --x and with ids too.
    for extra in [["--x", "--after"], ["--ids", "--after"]] {
        let (_, view) = run(
            &temp.path,
            &["view", "--focus", "big", extra[0], extra[1], "c1"],
        );
        assert!(view.len() <= BOUND, "{extra:?}: {}", view.len());
        assert!(
            view.contains(&format!("sley-agent view {} --after c1 big", extra[0])),
            "{view}"
        );
    }
}

#[test]
fn view_json_shapes() {
    let temp = expanded_program("json");
    let (status, focus) = run_json(&temp.path, &["view", "--focus", "calc", "--after", "c1"]);
    assert_eq!(status, 0, "{focus}");
    let keys: Vec<&String> = focus.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["focus", "view"]);
    let lists = &focus["focus"];
    assert_eq!(lists["target"], "calc");
    assert_eq!(lists["kind"], "fn");
    assert_eq!(lists["types"], json!(["CalcError"]));
    assert_eq!(lists["calls"], json!(["pick"]));
    assert_eq!(lists["callers"], json!([]));
    assert_eq!(lists["tests"], json!(["calc-ok", "calc-zero"]));
    assert_eq!(lists["consts"].as_array().unwrap().len(), 3);
    assert_eq!(
        lists["boundary"],
        json!({"visibility": "exported", "exported": true, "effects": [], "namespaces": [],
               "callers_outside": 0, "entrypoints": []})
    );
    assert_eq!(lists["omitted"], json!([]));
    assert_eq!(lists["bound"], BOUND);
    let (_, text) = run(&temp.path, &["view", "--focus", "calc", "--after", "c1"]);
    assert_eq!(focus["view"].as_str().unwrap(), text);
    let (_, ty) = run_json(
        &temp.path,
        &["view", "--focus", "CalcError", "--after", "c1"],
    );
    assert_eq!(ty["focus"]["kind"], "type");
    assert_eq!(ty["focus"]["used_by"], json!(["calc", "check", "safe"]));
    let (status, x) = run_json(&temp.path, &["view", "--x", "--after", "c1", "calc"]);
    assert_eq!(status, 0, "{x}");
    let keys: Vec<&String> = x.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["view"]);
    assert!(
        x["view"]
            .as_str()
            .unwrap()
            .contains("s = add?Overflow a, 3"),
        "{x}"
    );
}

#[test]
fn a_focused_view_stays_bounded_when_one_line_is_long() {
    // Review regression: a 200-case variant renders on one line, and a
    // 200-parameter function has a 200-parameter signature line.
    let temp = workspace("focus-long-line");
    let cases: Vec<String> = (0..200)
        .map(|index| format!("Case{index:03}_xxxxxxxxxxxxxxxxxxxx"))
        .collect();
    let params: Vec<Value> = (0..200)
        .map(|index| json!([format!("p{index:03}_yyyyyyyyyyyyyyyyyyyy"), "i64"]))
        .collect();
    let frame = json!({"af1": 1, "types": [{"name": "Huge", "variant": cases}],
        "fns": [{"fn": "wide", "params": params, "returns": "i64",
                 "blocks": [{"name": "entry", "ops": [], "term": ["return", "p000_yyyyyyyyyyyyyyyyyyyy"]}]}]});
    let (status, text) = run(&temp.path, &["try", "--no-test", &frame.to_string()]);
    assert_eq!(status, 0, "{text}");
    for target in ["Huge", "wide"] {
        let (status, text) = run(&temp.path, &["view", "--focus", target, "--after", "c1"]);
        assert_eq!(status, 0, "{text}");
        assert!(
            text.len() <= BOUND,
            "{target}: {} bytes\n{text}",
            text.len()
        );
        assert!(
            text.contains(" more bytes: sley-agent view --after c1 "),
            "{text}"
        );
        let (_, value) = run_json(&temp.path, &["view", "--focus", target, "--after", "c1"]);
        let focus = &value["focus"];
        assert!(focus["bytes"].as_u64().unwrap() <= BOUND as u64, "{focus}");
        let omitted = focus["omitted"].as_array().unwrap();
        assert!(
            omitted
                .iter()
                .any(|entry| entry["section"] == "target" && entry["unit"] == "bytes"),
            "{focus}"
        );
        // The disclosed command shows the line whole.
        let (status, whole) = run(&temp.path, &["view", "--after", "c1", target]);
        assert_eq!(status, 0);
        assert!(whole.len() > BOUND, "{target}");
    }
}

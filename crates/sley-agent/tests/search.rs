//! Verified search, end to end over fresh `init` workspaces
//! (`docs/spec/SLEY_AGENT_V1.md`, section 13): each generator, typed
//! ambiguity, the neighbor and wall limits, seed and oracle refusals,
//! determinism, the ranking rule, the per-seed use limit, the events
//! ledger, and repairs applied with `try --on`.

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
            "sley-agent-search-{label}-{}-{}",
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

/// Writes the public cases and returns their path.
fn cases(dir: &Path, name: &str, cases: &Value) -> String {
    let path = dir.join(name);
    fs::write(&path, cases.to_string()).unwrap();
    path.display().to_string()
}

/// Tries a frame and returns the candidate handle it made.
fn seed(dir: &Path, frame: &Value) -> String {
    let (_, value) = run_json(dir, &["try", &frame.to_string()]);
    value["handle"]
        .as_str()
        .unwrap_or_else(|| panic!("no handle: {value}"))
        .to_owned()
}

/// `search --json`: status and report.
fn search(dir: &Path, args: &[&str]) -> (i32, Value) {
    let mut words = vec!["search"];
    words.extend_from_slice(args);
    run_json(dir, &words)
}

fn neighbors(report: &Value) -> &Vec<Value> {
    report["neighbors"].as_array().unwrap()
}

/// The neighbor of a generator at a place with a change.
fn find<'a>(report: &'a Value, generator: &str, at: &str, change: &str) -> &'a Value {
    neighbors(report)
        .iter()
        .find(|n| n["generator"] == generator && n["at"] == at && n["change"] == change)
        .unwrap_or_else(|| panic!("no {generator} at {at}: {change} in {report:#}"))
}

fn one_fn(name: &str, params: &Value, returns: &str, blocks: &Value) -> Value {
    json!({"af1": 1, "fns": [{"fn": name, "params": params, "returns": returns, "blocks": blocks}]})
}

/// `diff(a, b)` should be `a - b` and adds instead.
fn diff_frame() -> Value {
    one_fn(
        "diff",
        &json!([["a", "i64"], ["b", "i64"]]),
        "Result<i64,ArithmeticError>",
        &json!([{"name": "entry", "ops": [["r", "add", "a", "b"]], "term": ["return", "r"]}]),
    )
}

fn diff_cases() -> Value {
    json!([{"name": "t1", "function": "diff", "args": [5, 3], "expect": {"Ok": 2}},
           {"name": "t2", "function": "diff", "args": [1, 4], "expect": {"Ok": -3}},
           {"name": "t3", "function": "diff", "args": [0, 0], "expect": {"Ok": 0}}])
}

/// `sign(a)` routes a negative `a` to the positive answer.
fn sign_frame() -> Value {
    one_fn(
        "sign",
        &json!([["a", "i64"]]),
        "i64",
        &json!([
            {"name": "entry", "ops": [["zero", "const", 0], ["neg", "lt", "a", "zero"]],
             "term": ["cond", "neg", "plus", "minus"]},
            {"name": "plus", "ops": [["one", "const", 1]], "term": ["return", "one"]},
            {"name": "minus", "ops": [["m", "const", -1]], "term": ["return", "m"]}]),
    )
}

fn sign_cases() -> Value {
    json!([{"name": "neg", "function": "sign", "args": [-5], "expect": -1},
           {"name": "pos", "function": "sign", "args": [7], "expect": 1}])
}

/// `go(light)` answers the two cases the wrong way round.
fn light_frame() -> Value {
    json!({"af1": 1, "types": [{"name": "Light", "variant": ["Red", "Green"]}],
      "fns": [{"fn": "go", "params": [["l", "Light"]], "returns": "bool", "blocks": [
        {"name": "entry", "term": ["switch", "l", ["Red", "yes"], ["Green", "no"]]},
        {"name": "yes", "ops": [["t", "const", true]], "term": ["return", "t"]},
        {"name": "no", "ops": [["f", "const", false]], "term": ["return", "f"]}]}]})
}

fn light_cases() -> Value {
    json!([{"name": "red", "function": "go", "args": ["Red"], "expect": false},
           {"name": "green", "function": "go", "args": ["Green"], "expect": true}])
}

/// The ranking key of a JSON neighbor.
fn key(neighbor: &Value) -> (i64, u64, usize, u64) {
    let order = [
        "opcode swap",
        "operand permutation",
        "operand substitution",
        "constant nudge",
        "edge swap",
        "negation",
    ];
    (
        -neighbor["public"]["passed"].as_i64().unwrap(),
        neighbor["size"].as_u64().unwrap(),
        order
            .iter()
            .position(|name| neighbor["generator"] == *name)
            .unwrap(),
        neighbor["index"].as_u64().unwrap(),
    )
}

/// Applies a neighbor frame the way the `next:` line says and returns the
/// trial's text.
fn apply(dir: &Path, on: Option<&str>, frame: &Value, public: &str) -> (i32, String) {
    let frame = frame.to_string();
    let mut args = vec!["try"];
    if let Some(on) = on {
        args.extend(["--on", on]);
    }
    args.extend([frame.as_str(), "--public", public]);
    run(dir, &args)
}

#[test]
fn opcode_swap_repairs_a_wrong_opcode_and_try_on_applies_it() {
    let temp = workspace("opcode");
    let public = cases(&temp.path, "cases.json", &diff_cases());
    let c1 = seed(&temp.path, &diff_frame());
    let (status, report) = search(&temp.path, &["diff", "--public", &public, "--from", &c1]);
    assert_eq!(status, 0, "{report:#}");
    assert_eq!(report["seed"]["public"]["passed"], 1);
    let top = &neighbors(&report)[0];
    assert_eq!(top["rank"], 1);
    assert_eq!(top["generator"], "opcode swap");
    assert_eq!(top["at"], "entry.r");
    assert_eq!(top["change"], "add -> sub");
    assert_eq!(top["size"], 1);
    assert_eq!(top["status"], "evaluated");
    assert_eq!(top["kernel"]["valid"], true);
    assert_eq!(top["public"]["passed"], 3);
    assert_eq!(
        top["frame"],
        json!({"af1": 1, "edit": [{"fn": "diff", "replace_op": "entry.r", "with": ["sub", "a", "b"]}]})
    );
    let outcomes = top["public"]["outcomes"].as_array().unwrap();
    assert_eq!(outcomes.len(), 3);
    assert_eq!(outcomes[1]["outcome"], "pass");
    assert_eq!(outcomes[1]["actual"], json!({"Ok": -3}));
    assert_eq!(report["rule"], sley_agent::search::RULE);
    assert_eq!(report["claim"], sley_agent::search::CLAIM);
    let next = report["next"].as_str().unwrap();
    assert!(
        next.starts_with(&format!("sley-agent try --on {c1} '{}'", top["frame"])),
        "{next}"
    );
    // Search made no candidate: the next handle is still c2.
    let (status, text) = apply(&temp.path, Some(&c1), &top["frame"], &public);
    assert_eq!(status, 0, "{text}");
    assert!(text.starts_with("c2: Valid"), "{text}");
    assert!(text.contains("public: 3/3 passed"), "{text}");
}

#[test]
fn operand_permutation_swaps_the_operands_of_a_non_commutative_op() {
    let temp = workspace("permute");
    let frame = one_fn(
        "back",
        &json!([["a", "i64"], ["b", "i64"]]),
        "Result<i64,ArithmeticError>",
        &json!([{"name": "entry", "ops": [["r", "sub", "a", "b"]], "term": ["return", "r"]}]),
    );
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"function": "back", "args": [5, 3], "expect": {"Ok": -2}},
                {"function": "back", "args": [1, 4], "expect": {"Ok": 3}}]),
    );
    let c1 = seed(&temp.path, &frame);
    let (status, report) = search(&temp.path, &["back", "--public", &public, "--from", &c1]);
    assert_eq!(status, 0, "{report:#}");
    let top = &neighbors(&report)[0];
    assert_eq!(top["generator"], "operand permutation");
    assert_eq!(top["change"], "a, b -> b, a");
    assert_eq!(top["size"], 2);
    assert_eq!(top["public"]["passed"], 2);
    assert_eq!(top["frame"]["edit"][0]["with"], json!(["sub", "b", "a"]));
    // Commutative operations are never permuted.
    let (_, report) = search(
        &temp.path,
        &[
            "diff",
            "--public",
            &cases(&temp.path, "d.json", &diff_cases()),
            "--from",
            &seed(&temp.path, &diff_frame()),
        ],
    );
    assert!(
        !neighbors(&report)
            .iter()
            .any(|n| n["generator"] == "operand permutation"),
        "{report:#}"
    );
}

#[test]
fn operand_substitution_proposes_same_typed_visible_values() {
    let temp = workspace("substitute");
    // `above` returns `low` where it should return `high`.
    let frame = json!({"af1": 1,
      "fns": [{"fn": "bound", "params": [["value", "i64"], ["low", "i64"], ["high", "i64"]], "returns": "i64",
               "blocks": [
        {"name": "entry", "ops": [["is_above", "gt", "value", "high"]], "term": ["cond", "is_above", "above", "inside"]},
        {"name": "above", "ops": [], "term": ["return", "low"]},
        {"name": "inside", "ops": [], "term": ["return", "value"]}]}]});
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"name": "in", "function": "bound", "args": [5, 0, 10], "expect": 5},
                {"name": "over", "function": "bound", "args": [11, 0, 10], "expect": 10}]),
    );
    let c1 = seed(&temp.path, &frame);
    let (status, report) = search(&temp.path, &["bound", "--public", &public, "--from", &c1]);
    assert_eq!(status, 0, "{report:#}");
    let top = &neighbors(&report)[0];
    assert_eq!(top["generator"], "operand substitution");
    assert_eq!(top["at"], "above (term)");
    assert_eq!(top["change"], "return low -> high");
    assert_eq!(
        top["frame"],
        json!({"af1": 1, "patch": [{"fn": "bound", "blocks": {"above": {"ops": [], "term": ["return", "high"]}}}]})
    );
    // Operands too: value, low and high are the i64 values visible there.
    find(
        &report,
        "operand substitution",
        "entry.is_above",
        "operand 1: high -> low",
    );
    find(
        &report,
        "operand substitution",
        "entry.is_above",
        "operand 1: high -> value",
    );
    let (status, text) = apply(&temp.path, Some(&c1), &top["frame"], &public);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("public: 2/2 passed"), "{text}");
}

#[test]
fn a_value_without_a_same_typed_substitute_gets_no_substitution() {
    let temp = workspace("ambiguous");
    let frame = one_fn(
        "flip",
        &json!([["flag", "bool"], ["count", "i64"]]),
        "bool",
        &json!([{"name": "entry", "ops": [["n", "not", "flag"]], "term": ["return", "n"]}]),
    );
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"function": "flip", "args": [true, 1], "expect": false}]),
    );
    let c1 = seed(&temp.path, &frame);
    let (_, report) = search(&temp.path, &["flip", "--public", &public, "--from", &c1]);
    // `flag` is the only bool visible to `n`: nothing replaces it, and an
    // i64 is never offered. Only the returned value has a substitute.
    let all = neighbors(&report);
    assert_eq!(all.len(), 1, "{report:#}");
    assert_eq!(all[0]["generator"], "operand substitution");
    assert_eq!(all[0]["change"], "return n -> flag");
    assert_eq!(report["counts"]["generated"], 1);
}

#[test]
fn constant_nudge_proposes_new_literals() {
    let temp = workspace("nudge");
    let frame = one_fn(
        "double",
        &json!([["a", "i64"]]),
        "Result<i64,ArithmeticError>",
        &json!([{"name": "entry", "ops": [["k", "const", 3], ["r", "mul", "a", "k"]], "term": ["return", "r"]}]),
    );
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"function": "double", "args": [2], "expect": {"Ok": 4}},
                {"function": "double", "args": [5], "expect": {"Ok": 10}}]),
    );
    let c1 = seed(&temp.path, &frame);
    let (status, report) = search(&temp.path, &["double", "--public", &public, "--from", &c1]);
    assert_eq!(status, 0, "{report:#}");
    let top = &neighbors(&report)[0];
    assert_eq!(top["generator"], "constant nudge");
    assert_eq!(top["at"], "entry.k");
    assert_eq!(top["change"], "3 -> 2");
    assert_eq!(top["frame"]["edit"][0]["with"], json!(["const", 2]));
    find(&report, "constant nudge", "entry.k", "3 -> 4");
    find(&report, "constant nudge", "entry.k", "3 -> -3");
    let (status, text) = apply(&temp.path, Some(&c1), &top["frame"], &public);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("public: 2/2 passed"), "{text}");
    // An unsigned zero is not nudged below zero, and zero is not negated.
    let unsigned = one_fn(
        "u",
        &json!([]),
        "u8",
        &json!([{"name": "entry", "ops": [["z", "const", {"type": "u8", "value": 0}]], "term": ["return", "z"]}]),
    );
    let u_public = cases(
        &temp.path,
        "u.json",
        &json!([{"function": "u", "args": [], "expect": 1}]),
    );
    let (_, report) = search(
        &temp.path,
        &[
            "u",
            "--public",
            &u_public,
            "--from",
            &seed(&temp.path, &unsigned),
        ],
    );
    let changes: Vec<&str> = neighbors(&report)
        .iter()
        .map(|n| n["change"].as_str().unwrap())
        .collect();
    assert_eq!(
        changes,
        ["{\"type\":\"u8\",\"value\":0} -> {\"type\":\"u8\",\"value\":1}"],
        "{report:#}"
    );
    assert_eq!(neighbors(&report)[0]["public"]["passed"], 1);
}

#[test]
fn edge_swap_and_negation_rewrite_conditions_and_switch_targets() {
    let temp = workspace("edges");
    let public = cases(&temp.path, "cases.json", &sign_cases());
    let c1 = seed(&temp.path, &sign_frame());
    let (status, report) = search(&temp.path, &["sign", "--public", &public, "--from", &c1]);
    assert_eq!(status, 0, "{report:#}");
    let swap = find(
        &report,
        "edge swap",
        "entry (term)",
        "then plus, else minus -> then minus, else plus",
    );
    assert_eq!(swap["size"], 2);
    assert_eq!(swap["public"]["passed"], 2);
    assert_eq!(
        swap["frame"]["patch"][0]["blocks"]["entry"]["term"],
        json!(["cond", "neg", "minus", "plus"])
    );
    let negation = find(
        &report,
        "negation",
        "entry (term)",
        "condition neg -> not neg",
    );
    assert_eq!(negation["public"]["passed"], 2);
    assert_eq!(
        negation["frame"]["patch"][0]["blocks"]["entry"],
        json!({"ops": [["zero", "const", 0], ["neg", "lt", "a", "zero"], ["not_neg", "not", "neg"]],
               "term": ["cond", "not_neg", "plus", "minus"]})
    );
    // An existing `not` is removed rather than doubled.
    let negated = one_fn(
        "big",
        &json!([["a", "i64"]]),
        "i64",
        &json!([
            {"name": "entry", "ops": [["zero", "const", 0], ["small", "lt", "a", "zero"], ["large", "not", "small"]],
             "term": ["cond", "large", "neg", "pos"]},
            {"name": "neg", "ops": [["m", "const", -1]], "term": ["return", "m"]},
            {"name": "pos", "ops": [["p", "const", 1]], "term": ["return", "p"]}]),
    );
    let big_public = cases(
        &temp.path,
        "big.json",
        &json!([{"function": "big", "args": [-5], "expect": -1}, {"function": "big", "args": [7], "expect": 1}]),
    );
    let (_, report) = search(
        &temp.path,
        &[
            "big",
            "--public",
            &big_public,
            "--from",
            &seed(&temp.path, &negated),
        ],
    );
    let removal = find(
        &report,
        "negation",
        "entry (term)",
        "condition large -> small",
    );
    assert_eq!(removal["public"]["passed"], 2);
    assert_eq!(
        removal["frame"]["patch"][0]["blocks"]["entry"]["term"],
        json!(["cond", "small", "neg", "pos"])
    );
    assert!(
        !neighbors(&report)
            .iter()
            .any(|n| n["change"].as_str().unwrap().contains("not large")),
        "{report:#}"
    );
}

#[test]
fn a_wrong_switch_edge_is_repaired_by_the_top_neighbor() {
    let temp = workspace("switch");
    let public = cases(&temp.path, "cases.json", &light_cases());
    let c1 = seed(&temp.path, &light_frame());
    let (status, report) = search(&temp.path, &["go", "--public", &public, "--from", &c1]);
    assert_eq!(status, 0, "{report:#}");
    assert_eq!(report["seed"]["public"]["passed"], 0);
    let top = &neighbors(&report)[0];
    assert_eq!(top["generator"], "edge swap");
    assert_eq!(
        top["change"],
        "Red -> yes, Green -> no become Red -> no, Green -> yes"
    );
    assert_eq!(
        top["frame"]["patch"][0]["blocks"]["entry"]["term"],
        json!(["switch", "l", ["Red", "no"], ["Green", "yes"]])
    );
    let (status, text) = apply(&temp.path, Some(&c1), &top["frame"], &public);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("public: 2/2 passed"), "{text}");
}

#[test]
fn neighbors_rank_by_cases_then_size_then_generator_then_index() {
    let temp = workspace("ranking");
    let public = cases(&temp.path, "cases.json", &sign_cases());
    let c1 = seed(&temp.path, &sign_frame());
    let (_, report) = search(&temp.path, &["sign", "--public", &public, "--from", &c1]);
    let ranked: Vec<&Value> = neighbors(&report)
        .iter()
        .filter(|n| !n["rank"].is_null())
        .collect();
    assert_eq!(
        Some(ranked.len() as u64),
        report["counts"]["evaluated"].as_u64()
    );
    for (position, neighbor) in ranked.iter().enumerate() {
        assert_eq!(neighbor["rank"], position + 1);
    }
    for pair in ranked.windows(2) {
        assert!(
            key(pair[0]) < key(pair[1]),
            "{:#} before {:#}",
            pair[0],
            pair[1]
        );
    }
    // The five full repairs: size-1 opcode swaps, then the size-1 negation,
    // then the size-2 permutation before the size-2 edge swap.
    let full: Vec<(String, String)> = ranked
        .iter()
        .filter(|n| n["public"]["passed"] == 2)
        .map(|n| {
            (
                n["generator"].as_str().unwrap().to_owned(),
                n["change"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let expected = [
        ("opcode swap", "lt -> gt"),
        ("opcode swap", "lt -> ge"),
        ("negation", "condition neg -> not neg"),
        ("operand permutation", "a, zero -> zero, a"),
        (
            "edge swap",
            "then plus, else minus -> then minus, else plus",
        ),
    ];
    assert_eq!(
        full,
        expected
            .iter()
            .map(|(g, c)| ((*g).to_owned(), (*c).to_owned()))
            .collect::<Vec<_>>()
    );
    // The text lists the top five with their frames and states the rule.
    let (_, text) = run(
        &temp.path,
        &["search", "sign", "--public", &public, "--from", "d1"],
    );
    assert!(
        text.contains(&format!("ranking: {}\n", sley_agent::search::RULE)),
        "{text}"
    );
    assert!(
        text.contains(" 1. #2 opcode swap at entry.neg: lt -> gt (size 1): public 2/2\n"),
        "{text}"
    );
    assert!(
        text.contains(
            " 3. #14 negation at entry (term): condition neg -> not neg (size 1): public 2/2\n"
        ),
        "{text}"
    );
    assert_eq!(text.matches("\n    {\"af1\":1,").count(), 5, "{text}");
    assert!(
        text.contains(&format!("verified: {}\n", sley_agent::search::CLAIM)),
        "{text}"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn the_neighbor_and_wall_limits_are_labeled() {
    let temp = workspace("limits");
    // Twenty comparisons: 60 opcode swaps and 20 permutations exceed 64.
    let ops: Vec<Value> = (0..20)
        .map(|i| json!([format!("c{i}"), "lt", "a", "b"]))
        .collect();
    let frame = one_fn(
        "many",
        &json!([["a", "i64"], ["b", "i64"]]),
        "bool",
        &json!([{"name": "entry", "ops": ops, "term": ["return", "c19"]}]),
    );
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"function": "many", "args": [1, 2], "expect": true}]),
    );
    let c1 = seed(&temp.path, &frame);
    let (status, text) = run(
        &temp.path,
        &["search", "many", "--public", &public, "--from", &c1],
    );
    assert_eq!(status, 0, "{text}");
    assert!(
        text.contains("neighbors: 64 generated, 64 kernel-valid, 0 refused, 64 evaluated\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "limits: neighbor limit reached: generation stopped at 64 neighbors, more exist"
        ),
        "{text}"
    );
    let (_, report) = search(&temp.path, &["many", "--public", &public, "--from", &c1]);
    assert_eq!(report["limits"]["neighbor_limit_reached"], true);
    assert_eq!(report["limits"]["wall_limit_reached"], false);
    assert_eq!(neighbors(&report).len(), 64);
    // An explicit bound, in another attempt (a workspace's head gets two
    // searches).
    let temp = workspace("limits-3");
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"function": "many", "args": [1, 2], "expect": true}]),
    );
    let c2 = seed(&temp.path, &frame);
    let (_, report) = search(
        &temp.path,
        &[
            "many",
            "--public",
            &public,
            "--from",
            &c2,
            "--max-neighbors",
            "3",
        ],
    );
    assert_eq!(report["counts"]["generated"], 3);
    assert_eq!(report["limits"]["max_neighbors"], 3);
    assert_eq!(report["limits"]["neighbor_limit_reached"], true);
    // No wall time: nothing is generated or run, and the output says so.
    let temp = workspace("limits-0");
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"function": "many", "args": [1, 2], "expect": true}]),
    );
    let c3 = seed(&temp.path, &frame);
    let (status, text) = run(
        &temp.path,
        &[
            "search",
            "many",
            "--public",
            &public,
            "--from",
            &c3,
            "--max-millis",
            "0",
        ],
    );
    assert_eq!(status, 1, "{text}");
    assert!(
        text.contains("the seed passes 0/0 public cases (partial: 0 of 1 run, wall limit reached)"),
        "{text}"
    );
    assert!(
        text.contains("limits: wall limit reached (bound 0 ms, stopped after "),
        "{text}"
    );
    assert!(
        text.contains("generation stopped; 0 of 0 neighbors evaluated"),
        "{text}"
    );
    assert!(text.contains("next: no neighbor was evaluated;"), "{text}");
    let (_, report) = search(
        &temp.path,
        &[
            "many",
            "--public",
            &public,
            "--from",
            &c3,
            "--max-millis",
            "0",
        ],
    );
    assert_eq!(report["limits"]["wall_limit_reached"], true);
    assert_eq!(report["seed"]["public"]["complete"], false);
    let events = fs::read_to_string(temp.path.join(".sley/events.jsonl")).unwrap();
    let last: Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
    assert_eq!(last["afx"]["search_exhausted"], true);
    // Bounds are checked.
    for (flag, value) in [
        ("--max-neighbors", "5000"),
        ("--max-neighbors", "x"),
        ("--max-millis", "-1"),
        ("--max-millis", "3600001"),
    ] {
        let (status, text) = run(
            &temp.path,
            &["search", "many", "--public", &public, flag, value],
        );
        assert_eq!(status, 2, "{text}");
        assert!(text.contains("AGENT_USAGE_INVALID"), "{text}");
    }
}

#[test]
fn a_looping_neighbor_stops_at_its_fuel_cap() {
    let temp = workspace("loop");
    let frame = json!({"af1": 1, "afx": 1,
     "types": [{"name": "SumError", "variant": ["Negative", "Overflow"]}],
     "fns": [{"fn": "sum_to", "params": [["n", "i64"]], "returns": "Result<i64,SumError>",
              "blocks": [
       {"name": "entry", "ops": [["!Negative", "if", ["lt", "n", 0]]], "term": ["br", "loop", 0, 1]},
       {"name": "loop", "params": [["acc", "i64"], ["i", "i64"]], "ops": [["more", "lt", "i", "n"]],
        "term": ["cond", "more", "body", "done"]},
       {"name": "body", "params": [["acc", "i64"], ["i", "i64"]],
        "ops": [["next", "add?Overflow", "acc", "i"]], "term": ["br", "loop", "next", ["add?Overflow", "i", 1]]},
       {"name": "done", "params": [["acc", "i64"]], "term": ["ok", "acc"]}]}]});
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"name": "s4", "function": "sum_to", "args": [4], "expect": {"Ok": 10}},
                {"name": "s1", "function": "sum_to", "args": [1], "expect": {"Ok": 1}},
                {"name": "neg", "function": "sum_to", "args": [-1], "expect": {"Err": "Negative"}}]),
    );
    let c1 = seed(&temp.path, &frame);
    let (status, report) = search(&temp.path, &["sum_to", "--public", &public, "--from", &c1]);
    assert_eq!(status, 0, "{report:#}");
    let top = &neighbors(&report)[0];
    assert_eq!(top["change"], "lt -> le");
    assert_eq!(top["frame"]["afx"], 1);
    // Swapping the loop's exit never terminates: that case is labeled.
    let swap = find(
        &report,
        "edge swap",
        "loop (term)",
        "then body, else done -> then done, else body",
    );
    let limited: Vec<&Value> = swap["public"]["outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|outcome| outcome["outcome"] == "resource limit")
        .collect();
    assert!(!limited.is_empty(), "{swap:#}");
    assert_eq!(limited[0]["actual"], json!({"limit": "Fuel"}));
    assert_eq!(report["limits"]["wall_limit_reached"], false);
}

#[test]
fn an_af1x_seed_gets_af1x_neighbors_that_layer_on_its_draft() {
    let temp = workspace("afx");
    let frame = json!({"af1": 1, "afx": 1,
      "types": [{"name": "E", "variant": ["Big", ["Math", "ArithmeticError"]]}],
      "fns": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,E>",
        "blocks": [{"name": "entry", "ops": [["c", "lt", "a", "b"]], "term": ["cond", "c", "yes", "no"]},
                   {"name": "yes", "ops": [["r", "add?Math", "a", ["mul?Math", "b", 3]]], "term": ["ok", "r"]},
                   {"name": "no", "ops": [["!Big", "if", ["gt", "a", 100]]], "term": ["ok", ["sub?Math", "a", 3]]}]}]});
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"name": "t1", "function": "f", "args": [1, 5], "expect": {"Ok": 11}},
                {"name": "t2", "function": "f", "args": [5, 2], "expect": {"Ok": 2}},
                {"name": "t3", "function": "f", "args": [500, 2], "expect": {"Err": "Big"}}]),
    );
    seed(&temp.path, &frame);
    let (status, report) = search(&temp.path, &["f", "--public", &public, "--from", "d1"]);
    assert_eq!(status, 0, "{report:#}");
    assert_eq!(report["search"]["seed"], "d1@r1");
    let top = &neighbors(&report)[0];
    assert_eq!(top["generator"], "constant nudge");
    assert_eq!(top["at"], "yes.r");
    assert_eq!(top["change"], "mul?Math 3 -> 2");
    assert_eq!(top["pointer"], "/fns/0/blocks/1/ops/0/3/2");
    assert_eq!(
        top["frame"],
        json!({"af1": 1, "afx": 1, "edit": [{"fn": "f", "replace_op": "yes.r",
               "with": ["add?Math", "a", ["mul?Math", "b", 2]]}]})
    );
    // Exits and nested operations are stated as their author wrote them.
    let exit = find(
        &report,
        "negation",
        "no (!Big if)",
        "condition [\"gt\",\"a\",100] -> [\"not\",[\"gt\",\"a\",100]]",
    );
    assert_eq!(
        exit["frame"]["patch"][0]["blocks"]["no"]["ops"][0],
        json!(["!Big", "if", ["not", ["gt", "a", 100]]])
    );
    find(&report, "opcode swap", "yes.r", "mul?Math -> sub?Math");
    // No neighbor names a generated value.
    for neighbor in neighbors(&report) {
        assert!(
            !neighbor["frame"].to_string().contains("__"),
            "{neighbor:#}"
        );
    }
    assert!(report["counts"]["skipped"].as_u64().unwrap() >= 1);
    let (status, text) = apply(&temp.path, Some("d1@r1"), &top["frame"], &public);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains(" draft d1@r2\n"), "{text}");
    assert!(text.contains("public: 3/3 passed"), "{text}");
}

#[test]
fn the_head_is_the_default_seed() {
    let temp = workspace("head");
    let c1 = seed(&temp.path, &diff_frame());
    let (status, text) = run(&temp.path, &["commit", &c1]);
    assert_eq!(status, 0, "{text}");
    let public = cases(&temp.path, "cases.json", &diff_cases());
    let (status, text) = run(&temp.path, &["search", "diff", "--public", &public]);
    assert_eq!(status, 0, "{text}");
    assert!(
        text.starts_with(
            "search diff from head: the seed passes 1/3 public cases (fail: t1, t2)\n"
        ),
        "{text}"
    );
    let next = text
        .lines()
        .find_map(|line| line.strip_prefix("next: "))
        .unwrap();
    assert!(
        next.starts_with("sley-agent try '{\"af1\":1,\"edit\":"),
        "{next}"
    );
    let (_, report) = search(&temp.path, &["diff", "--public", &public]);
    let top = &neighbors(&report)[0];
    let (status, text) = apply(&temp.path, None, &top["frame"], &public);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("public: 3/3 passed"), "{text}");
}

#[test]
#[allow(clippy::too_many_lines)]
fn unusable_seeds_are_refused_and_cost_no_search() {
    let temp = workspace("seeds");
    let public = cases(&temp.path, "cases.json", &diff_cases());
    // A kernel-refused candidate and its draft.
    let refused = json!({"af1": 1, "fns": [{"fn": "bad", "params": [["a", "i64"]], "returns": "bool", "blocks": [
        {"name": "entry", "ops": [["c", "lt", "a", "a"]], "term": ["cond", "c", "l", "r"]},
        {"name": "l", "ops": [["m", "lt", "a", "a"]], "term": ["br", "j"]},
        {"name": "r", "ops": [["c2", "lt", "a", "a"]], "term": ["cond", "c2", "j", "w"]},
        {"name": "w", "ops": [], "term": ["return", "a"]},
        {"name": "j", "ops": [], "term": ["return", "l.m"]}]}]});
    let (status, _) = run(&temp.path, &["try", &refused.to_string()]);
    assert_eq!(status, 1);
    let bad_public = cases(
        &temp.path,
        "bad.json",
        &json!([{"function": "bad", "args": [1], "expect": false}]),
    );
    for (from, detail) in [
        ("c1", "c1 is not Valid against the current head (REFUSED"),
        (
            "d1",
            "d1@r1 is refused: search starts from a revision whose candidate is Valid",
        ),
    ] {
        let (status, text) = run(
            &temp.path,
            &["search", "bad", "--public", &bad_public, "--from", from],
        );
        assert_eq!(status, 2, "{text}");
        assert!(
            text.starts_with(&format!("error AGENT_SEARCH_SEED_INVALID: {detail}")),
            "{text}"
        );
    }
    // An incomplete draft (d2) and a text draft (d3).
    let (status, _) = run(
        &temp.path,
        &["try", "{\"af1\": 1, \"fns\": [{\"fn\": \"x\"}]}"],
    );
    assert_eq!(status, 2);
    let (status, _) = run(&temp.path, &["try", "{\"af1\": 1,"]);
    assert_eq!(status, 2);
    for (from, state) in [("d2", "d2@r1 is incomplete"), ("d3", "d3@r1 is text")] {
        let (status, text) = run(
            &temp.path,
            &["search", "diff", "--public", &public, "--from", from],
        );
        assert_eq!(status, 2, "{text}");
        assert!(
            text.contains(&format!("AGENT_SEARCH_SEED_INVALID: {state}")),
            "{text}"
        );
    }
    // A Valid seed: unknown names and names that are not functions.
    let valid = seed(&temp.path, &light_frame());
    for (name, detail) in [
        ("nothing", format!("no function named `nothing` in {valid}")),
        ("Light", format!("`Light` is not a function in {valid}")),
    ] {
        let (status, text) = run(
            &temp.path,
            &["search", name, "--public", &public, "--from", &valid],
        );
        assert_eq!(status, 2, "{text}");
        assert!(
            text.contains(&format!("AGENT_SEARCH_SEED_INVALID: {detail}")),
            "{text}"
        );
    }
    // A Valid candidate kept without a frame (submitted from its bytes).
    let bytes = temp.path.join("light.hex");
    fs::copy(
        temp.path
            .join(".sley/candidates")
            .join(format!("{valid}.hex")),
        &bytes,
    )
    .unwrap();
    let (status, text) = run(
        &temp.path,
        &["submit", &bytes.display().to_string(), "--untested"],
    );
    assert_eq!(status, 0, "{text}");
    let frameless = text.split_whitespace().nth(1).unwrap().to_owned();
    let (status, text) = run(
        &temp.path,
        &["search", "go", "--public", &public, "--from", &frameless],
    );
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains(&format!(
            "AGENT_SEARCH_SEED_INVALID: {frameless} was not made from an AF1 frame"
        )),
        "{text}"
    );
    let (status, text) = run(
        &temp.path,
        &["search", "go", "--public", &public, "--from", "c99"],
    );
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_HANDLE_UNKNOWN"), "{text}");
    let (status, text) = run(
        &temp.path,
        &["search", "go", "--public", &public, "--from", "latest-ish"],
    );
    assert_eq!(status, 2);
    assert!(
        text.contains("AGENT_SEARCH_SEED_INVALID: `latest-ish` is not a candidate handle"),
        "{text}"
    );
    let (status, text) = run(&temp.path, &["search", "go", "--from", &valid]);
    assert_eq!(status, 2);
    assert!(text.contains("AGENT_USAGE_INVALID"), "{text}");
    // None of these used a search: the Valid seed still has both.
    let light_public = cases(&temp.path, "light.json", &light_cases());
    for _ in 0..2 {
        let (status, text) = run(
            &temp.path,
            &["search", "go", "--public", &light_public, "--from", &valid],
        );
        assert_eq!(status, 0, "{text}");
    }
}

#[test]
fn a_case_file_without_a_case_for_the_function_is_no_oracle() {
    let temp = workspace("oracle");
    let c1 = seed(&temp.path, &diff_frame());
    let other = cases(
        &temp.path,
        "other.json",
        &json!([{"function": "sign", "args": [1], "expect": 1}]),
    );
    let no_expect = cases(
        &temp.path,
        "no-expect.json",
        &json!([{"function": "diff", "args": [1, 2]}]),
    );
    let empty = cases(&temp.path, "empty.json", &json!([]));
    let object = cases(&temp.path, "object.json", &json!({"function": "diff"}));
    let text_file = temp.path.join("text.json");
    fs::write(&text_file, "not json").unwrap();
    let missing = temp.path.join("missing.json");
    for (file, detail) in [
        (other.clone(), "has no public case for `diff`".to_owned()),
        (no_expect, "has no public case for `diff`".to_owned()),
        (empty, "holds no case".to_owned()),
        (object, "public cases are a JSON array".to_owned()),
        (text_file.display().to_string(), "is not JSON".to_owned()),
        (
            missing.display().to_string(),
            "public cases are a JSON array".to_owned(),
        ),
    ] {
        let (status, text) = run(
            &temp.path,
            &["search", "diff", "--public", &file, "--from", &c1],
        );
        assert_eq!(status, 2, "{text}");
        assert!(text.starts_with("error AGENT_SEARCH_NO_ORACLE: "), "{text}");
        assert!(text.contains(&detail), "{file}: {text}");
    }
    // Cases never come from the candidate: no search was counted either.
    assert_eq!(uses(&temp.path), 0);
}

#[test]
fn the_same_seed_and_cases_give_the_same_report() {
    let temp = workspace("determinism");
    let public = cases(&temp.path, "cases.json", &sign_cases());
    let c1 = seed(&temp.path, &sign_frame());
    let strip = |mut value: Value| {
        value.as_object_mut().unwrap().remove("resources");
        value["search"].as_object_mut().unwrap().remove("use");
        value
    };
    let (_, first) = search(&temp.path, &["sign", "--public", &public, "--from", &c1]);
    let (_, second) = search(&temp.path, &["sign", "--public", &public, "--from", &c1]);
    assert!(first["resources"]["wall_millis"].is_u64());
    assert_eq!(strip(first), strip(second));
    // Text: everything but the resources line.
    let (status, text) = run(&temp.path, &["commit", &c1]);
    assert_eq!(status, 0, "{text}");
    let lines = |text: String| -> Vec<String> {
        text.lines()
            .filter(|line| !line.starts_with("resources: "))
            .map(str::to_owned)
            .collect()
    };
    let (_, first) = run(&temp.path, &["search", "sign", "--public", &public]);
    let (_, second) = run(&temp.path, &["search", "sign", "--public", &public]);
    assert!(first.contains("\nresources: wall "), "{first}");
    assert_eq!(lines(first), lines(second));
}

/// The searches recorded in a workspace (one slot file each).
fn uses(dir: &Path) -> usize {
    fs::read_dir(dir.join(".sley/search")).map_or(0, |entries| {
        entries
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".json"))
            .count()
    })
}

#[test]
fn an_attempt_gets_two_searches_whatever_its_seeds() {
    let temp = workspace("uses");
    let public = cases(&temp.path, "cases.json", &diff_cases());
    let c1 = seed(&temp.path, &diff_frame());
    assert_eq!(
        search(&temp.path, &["diff", "--public", &public, "--from", &c1]).0,
        0
    );
    assert_eq!(
        search(&temp.path, &["diff", "--public", &public, "--from", "d1"]).0,
        0
    );
    assert_eq!(uses(&temp.path), 2);
    // The same whole frame tried again, a draft started on c1 and another
    // function: all the same attempt.
    let c2 = seed(&temp.path, &diff_frame());
    let (status, _) = run(
        &temp.path,
        &["try", "--on", &c1, &json!({"af1": 1, "edit": [{"fn": "diff", "replace_op": "entry.r", "with": ["mul", "a", "b"]}]}).to_string()],
    );
    assert_eq!(status, 0);
    let c4 = seed(&temp.path, &sign_frame());
    let sign_public = cases(&temp.path, "sign.json", &sign_cases());
    for (function, file, from) in [
        ("diff", &public, Some(c2.as_str())),
        ("diff", &public, Some("d3")),
        ("diff", &public, Some("c3")),
        ("sign", &sign_public, Some(c4.as_str())),
    ] {
        let mut args = vec!["search", function, "--public", file];
        if let Some(from) = from {
            args.extend(["--from", from]);
        }
        let (status, text) = run(&temp.path, &args);
        assert_eq!(status, 2, "{args:?}: {text}");
        assert!(
            text.starts_with(
                "error AGENT_SEARCH_SEED_INVALID: this attempt has used its 2 searches (head "
            ),
            "{text}"
        );
        assert!(
            text.contains(&format!(": {c1} diff, d1@r1 diff)")),
            "{text}"
        );
    }
    assert_eq!(uses(&temp.path), 2);
    // A commit makes a new head: a new attempt.
    let (status, text) = run(&temp.path, &["commit", &c2]);
    assert_eq!(status, 0, "{text}");
    assert_eq!(search(&temp.path, &["diff", "--public", &public]).0, 0);
    assert_eq!(uses(&temp.path), 3);
}

#[test]
fn a_long_chain_of_try_on_stays_one_attempt() {
    let temp = workspace("chain");
    let public = cases(&temp.path, "cases.json", &diff_cases());
    let mut handle = seed(&temp.path, &diff_frame());
    for _ in 0..2 {
        assert_eq!(
            search(
                &temp.path,
                &["diff", "--public", &public, "--from", &handle]
            )
            .0,
            0
        );
    }
    // More hops than any chain walk would follow.
    let edit = json!({"af1": 1, "edit": [{"fn": "diff", "replace_op": "entry.r", "with": ["add", "a", "b"]}]}).to_string();
    for _ in 0..40 {
        let (_, value) = run_json(&temp.path, &["try", "--on", &handle, &edit]);
        handle = value["handle"].as_str().unwrap().to_owned();
    }
    let (status, text) = run(
        &temp.path,
        &["search", "diff", "--public", &public, "--from", &handle],
    );
    assert_eq!(status, 2, "{text}");
    assert!(
        text.contains("this attempt has used its 2 searches"),
        "{text}"
    );
    assert_eq!(uses(&temp.path), 2);
}

#[test]
fn concurrent_searches_claim_at_most_two_slots() {
    let temp = workspace("race");
    let public = cases(&temp.path, "cases.json", &diff_cases());
    let c1 = seed(&temp.path, &diff_frame());
    let dir = temp.path.clone();
    let workers: Vec<_> = (0..12)
        .map(|_| {
            let (dir, public, c1) = (dir.clone(), public.clone(), c1.clone());
            std::thread::spawn(move || {
                run(
                    &dir,
                    &["search", "diff", "--public", &public, "--from", &c1],
                )
            })
        })
        .collect();
    let results: Vec<(i32, String)> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    let ran = results.iter().filter(|(status, _)| *status == 0).count();
    assert_eq!(ran, 2, "{results:?}");
    for (status, text) in &results {
        if *status != 0 {
            assert_eq!(*status, 2);
            assert!(
                text.starts_with(
                    "error AGENT_SEARCH_SEED_INVALID: this attempt has used its 2 searches"
                ),
                "{text}"
            );
        }
    }
    assert_eq!(uses(&temp.path), 2);
    // No scratch file is left behind.
    let left: Vec<String> = fs::read_dir(temp.path.join(".sley/search"))
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_none_or(|extension| extension != "json")
        })
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn the_events_ledger_counts_the_search() {
    let temp = workspace("ledger");
    let public = cases(&temp.path, "cases.json", &diff_cases());
    let c1 = seed(&temp.path, &diff_frame());
    let (_, report) = search(&temp.path, &["diff", "--public", &public, "--from", &c1]);
    let events = fs::read_to_string(temp.path.join(".sley/events.jsonl")).unwrap();
    let last: Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
    assert_eq!(last["cmd"], "search");
    assert_eq!(last["candidate"], c1.as_str());
    assert_eq!(last["draft"], "d1@r1");
    assert_eq!(
        last["afx"]["search_neighbors"],
        report["counts"]["generated"]
    );
    assert_eq!(last["afx"]["search_valid"], report["counts"]["valid"]);
    assert_eq!(
        last["afx"]["search_evaluated"],
        report["counts"]["evaluated"]
    );
    assert_eq!(last["afx"]["search_exhausted"], false);
    assert_eq!(last["input_bytes"], fs::read(&public).unwrap().len());
    assert!(
        !events.contains("\"sub\""),
        "no program content in the ledger"
    );
    let (status, _) = run(
        &temp.path,
        &[
            "search",
            "diff",
            "--public",
            "/nonexistent.json",
            "--from",
            &c1,
        ],
    );
    assert_eq!(status, 2);
    let events = fs::read_to_string(temp.path.join(".sley/events.jsonl")).unwrap();
    let last: Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
    assert_eq!(last["refusal"], "AGENT_SEARCH_NO_ORACLE");
}

#[test]
fn the_search_help_example_runs() {
    let temp = workspace("help");
    let help = sley_agent::help::SEARCH;
    let blocks: Vec<&str> = help
        .split("```json\n")
        .skip(1)
        .map(|rest| rest.split("```").next().unwrap())
        .collect();
    assert_eq!(blocks.len(), 2, "a frame and its public cases");
    let frame: Value = serde_json::from_str(blocks[0]).unwrap();
    let public_cases: Value = serde_json::from_str(blocks[1]).unwrap();
    fs::write(temp.path.join("diff.json"), frame.to_string()).unwrap();
    let public = cases(&temp.path, "cases.json", &public_cases);
    let (status, text) = run(
        &temp.path,
        &[
            "try",
            &temp.path.join("diff.json").display().to_string(),
            "--public",
            &public,
        ],
    );
    assert_eq!(status, 1, "{text}");
    assert!(text.contains("public: 0/2 passed"), "{text}");
    let (status, text) = run(
        &temp.path,
        &["search", "diff", "--public", &public, "--from", "c1"],
    );
    assert_eq!(status, 0, "{text}");
    // Every output line the help quotes is in the real output.
    let quoted: Vec<&str> = help
        .lines()
        .filter_map(|line| line.strip_prefix("    > "))
        .collect();
    assert!(quoted.len() >= 3, "{help}");
    for line in &quoted {
        let line = line.replace("cases.json", &public);
        assert!(text.contains(&line), "help line `{line}` not in:\n{text}");
    }
    let next = text
        .lines()
        .find_map(|line| line.strip_prefix("next: "))
        .unwrap();
    assert!(next.starts_with("sley-agent try --on c1 "), "{next}");
    let (_, report) = search(&temp.path, &["diff", "--public", &public, "--from", "d1"]);
    let (status, text) = apply(
        &temp.path,
        Some("c1"),
        &neighbors(&report)[0]["frame"],
        &public,
    );
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("public: 2/2 passed"), "{text}");
    assert_eq!(sley_agent::help::topic("search").as_deref(), Some(help));
    let (status, text) = run(&temp.path, &["help", "search"]);
    assert_eq!(status, 0);
    assert!(text.starts_with("# search"), "{text}");
}

/// Every file under a directory with its bytes, but the given names.
fn snapshot(dir: &Path, skip: &[&str]) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(entries) = fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if !skip.iter().any(|name| {
                path.ends_with(name) || path.parent().is_some_and(|parent| parent.ends_with(name))
            }) {
                out.push((path.clone(), fs::read(&path).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn search_changes_nothing_but_its_record_and_only_the_searched_function() {
    let temp = workspace("boundary");
    // `twice` calls `diff`; search on `diff` never proposes a change to
    // `twice`, and the repository, candidates and drafts stay as they were.
    let frame = json!({"af1": 1, "fns": [
        {"fn": "diff", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,ArithmeticError>",
         "blocks": [{"name": "entry", "ops": [["r", "add", "a", "b"]], "term": ["return", "r"]}]},
        {"fn": "twice", "params": [["a", "i64"]], "returns": "Result<i64,ArithmeticError>",
         "blocks": [{"name": "entry", "ops": [["k", "const", 2], ["r", "call", "diff", "a", "k"]],
                     "term": ["return", "r"]}]}]});
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"function": "diff", "args": [5, 3], "expect": {"Ok": 2}},
                {"function": "twice", "args": [5], "expect": {"Ok": 3}}]),
    );
    let c1 = seed(&temp.path, &frame);
    let before = snapshot(&temp.path, &["events.jsonl", ".sley/search"]);
    let (status, report) = search(&temp.path, &["diff", "--public", &public, "--from", &c1]);
    assert_eq!(status, 0, "{report:#}");
    assert_eq!(
        before,
        snapshot(&temp.path, &["events.jsonl", ".sley/search"])
    );
    // The caller's case runs too: the top neighbor fixes both.
    assert_eq!(neighbors(&report)[0]["public"]["passed"], 2);
    for neighbor in neighbors(&report) {
        let frame = &neighbor["frame"];
        let entries = frame
            .get("edit")
            .or_else(|| frame.get("patch"))
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(entries.len(), 1, "{neighbor:#}");
        assert_eq!(entries[0]["fn"], "diff", "{neighbor:#}");
        let keys: Vec<&String> = frame.as_object().unwrap().keys().collect();
        assert!(
            keys.len() == 2 && keys.contains(&&"af1".to_owned()),
            "{frame}"
        );
    }
}

#[test]
fn constant_nudges_stay_in_their_type_range() {
    let temp = workspace("ranges");
    let frame = json!({"af1": 1, "fns": [
        {"fn": "top", "params": [], "returns": "i64",
         "blocks": [{"name": "entry", "ops": [["k", "const", i64::MAX]], "term": ["return", "k"]}]}]});
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"function": "top", "args": [], "expect": 0}]),
    );
    let (_, report) = search(
        &temp.path,
        &[
            "top",
            "--public",
            &public,
            "--from",
            &seed(&temp.path, &frame),
        ],
    );
    let changes: Vec<&str> = neighbors(&report)
        .iter()
        .filter(|n| n["generator"] == "constant nudge")
        .map(|n| n["change"].as_str().unwrap())
        .collect();
    assert_eq!(
        changes,
        [
            "9223372036854775807 -> 9223372036854775806",
            "9223372036854775807 -> -9223372036854775807"
        ]
    );
    let frame = json!({"af1": 1, "fns": [
        {"fn": "bottom", "params": [], "returns": "i64",
         "blocks": [{"name": "entry", "ops": [["k", "const", i64::MIN]], "term": ["return", "k"]}]}]});
    let public = cases(
        &temp.path,
        "bottom.json",
        &json!([{"function": "bottom", "args": [], "expect": 0}]),
    );
    let (_, report) = search(
        &temp.path,
        &[
            "bottom",
            "--public",
            &public,
            "--from",
            &seed(&temp.path, &frame),
        ],
    );
    let changes: Vec<&str> = neighbors(&report)
        .iter()
        .map(|n| n["change"].as_str().unwrap())
        .collect();
    assert_eq!(changes, ["-9223372036854775808 -> -9223372036854775807"]);
}

#[test]
fn a_substitution_never_drops_a_nested_failure_path_and_counts_what_it_removes() {
    let temp = workspace("nested");
    // r = add?Overflow(b, mul?Overflow(a, a)): replacing the nested checked
    // multiplication by a name would delete its overflow route.
    let frame = json!({"af1": 1, "afx": 1,
      "types": [{"name": "E", "variant": ["Overflow", "Neg"]}],
      "fns": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,E>",
        "blocks": [{"name": "entry", "ops": [["r", "add?Overflow", "b", ["mul?Overflow", "a", "a"]]],
                    "term": ["ok", "r"]}]}]});
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"name": "small", "function": "f", "args": [2, 2], "expect": {"Ok": 4}},
                {"name": "big", "function": "f", "args": [4_000_000_000_i64, 2], "expect": {"Ok": 4}}]),
    );
    seed(&temp.path, &frame);
    let (_, report) = search(&temp.path, &["f", "--public", &public, "--from", "d1"]);
    for neighbor in neighbors(&report) {
        let change = neighbor["change"].as_str().unwrap();
        assert!(
            !change.starts_with("operand 1: [\"mul?Overflow\""),
            "{neighbor:#}"
        );
        assert!(
            neighbor["frame"].to_string().contains("mul?")
                || neighbor["generator"] == "opcode swap",
            "{neighbor:#}"
        );
    }
    assert!(
        report["counts"]["skipped"].as_u64().unwrap() >= 2,
        "{report:#}"
    );
    // A pure nested operation may be replaced; its operations count.
    let frame = json!({"af1": 1, "afx": 1,
      "fns": [{"fn": "g", "params": [["a", "i64"], ["flag", "bool"]], "returns": "bool",
        "blocks": [{"name": "entry", "ops": [["c", "and", ["lt", "a", 0], "flag"]], "term": ["return", "c"]}]}]});
    let g_public = cases(
        &temp.path,
        "g.json",
        &json!([{"function": "g", "args": [5, true], "expect": true}]),
    );
    seed(&temp.path, &frame);
    let (_, report) = search(&temp.path, &["g", "--public", &g_public, "--from", "d2"]);
    let dropped = find(
        &report,
        "operand substitution",
        "entry.c",
        "operand 0: [\"lt\",\"a\",0] -> flag",
    );
    assert_eq!(
        dropped["size"], 3,
        "the operand, lt and its literal: {dropped:#}"
    );
    assert_eq!(dropped["public"]["passed"], 1);
    // It passes like the size-1 `and -> or`, and ranks after it.
    let swap = find(&report, "opcode swap", "entry.c", "and -> or");
    assert!(
        swap["rank"].as_u64() < dropped["rank"].as_u64(),
        "{report:#}"
    );
}

#[test]
fn changes_no_frame_can_state_are_skipped_before_the_budget() {
    // Live code written in the authoring dialect carries generated names; an
    // AF1-X seed that does not restate a block cannot name them.
    let temp = workspace("generated");
    let live = json!({"af1": 1, "afx": 1,
      "types": [{"name": "E", "variant": ["Overflow", "Neg", "Big"]}],
      "fns": [{"fn": "score", "params": [["x", "i64"], ["y", "i64"]], "returns": "Result<i64,E>",
        "blocks": [{"name": "entry",
          "ops": [["!Neg", "if", ["lt", "x", 0]], ["d", "sub?Overflow", "y", "x"], ["big", "gt", "d", 100]],
          "term": ["cond", "big", "high", "low"]},
         {"name": "high", "params": [["d", "i64"]], "ops": [["h", "mul?Overflow", "d", 2]], "term": ["br", "join", "h"]},
         {"name": "low", "params": [["d", "i64"]], "ops": [["l", "add?Overflow", "d", ["mul?Overflow", "x", 3]]], "term": ["br", "join", "l"]},
         {"name": "join", "params": [["v", "i64"], ["d", "i64"]], "ops": [["!Big", "if", ["ge", "v", 1000]]],
          "term": ["ok", ["add?Overflow", "v", "d"]]}]}]});
    let c1 = seed(&temp.path, &live);
    assert_eq!(run(&temp.path, &["commit", &c1]).0, 0);
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"name": "s1", "function": "score", "args": [1, 5], "expect": {"Ok": 8}},
                {"name": "s3", "function": "score", "args": [1, 500], "expect": {"Ok": 1497}}]),
    );
    let patch = json!({"af1": 1, "afx": 1, "patch": [{"fn": "score", "blocks": {"high": {"params": [["d", "i64"]],
        "ops": [["h", "mul?Overflow", "d", 3]], "term": ["br", "join", "h"]}}}]});
    seed(&temp.path, &patch);
    let (_, report) = search(
        &temp.path,
        &[
            "score",
            "--public",
            &public,
            "--from",
            "d2",
            "--max-neighbors",
            "20",
        ],
    );
    // Only the restated block `high` is searched: its neighbors fit the budget.
    assert_eq!(
        report["limits"]["neighbor_limit_reached"], false,
        "{report:#}"
    );
    assert!(
        report["counts"]["skipped"].as_u64().unwrap() > 50,
        "{report:#}"
    );
    assert!(report["counts"]["generated"].as_u64().unwrap() <= 20);
    for neighbor in neighbors(&report) {
        assert!(
            !neighbor["frame"].to_string().contains("__"),
            "{neighbor:#}"
        );
        assert!(
            neighbor["at"].as_str().unwrap().starts_with("high"),
            "{neighbor:#}"
        );
        let detail = neighbor["kernel"]["detail"].as_str().unwrap_or("");
        assert!(!detail.contains("reserved"), "{neighbor:#}");
    }
    // An AF1-X seed that does not state `score` at all: nothing is stated,
    // nothing uses a slot, and the output says how to search it.
    let unrelated = json!({"af1": 1, "afx": 1, "fns": [{"fn": "helper", "params": [["q", "i64"]], "returns": "i64",
        "blocks": [{"name": "entry", "ops": [], "term": ["return", "q"]}]}]});
    seed(&temp.path, &unrelated);
    let (status, text) = run(
        &temp.path,
        &["search", "score", "--public", &public, "--from", "d3"],
    );
    assert_eq!(status, 1, "{text}");
    assert!(
        text.contains("neighbors: 0 generated, 0 kernel-valid, 0 refused, 0 evaluated; "),
        "{text}"
    );
    assert!(
        text.contains("next: no neighbor was evaluated: none of the ")
            && text.contains("restate the blocks of score with patch"),
        "{text}"
    );
    // Code a ripple derivation rewrote: stated as the head states it.
    let temp = workspace("rippled");
    let live = json!({"af1": 1,
     "fns": [{"fn": "f", "params": [["a", "i64"]], "returns": "i64",
       "blocks": [{"name": "entry", "ops": [], "term": ["return", "a"]}]},
      {"fn": "g", "params": [["x", "i64"], ["y", "i64"]], "returns": "i64",
       "blocks": [{"name": "entry", "ops": [["s", "sub", "x", "y"], ["t", "call", "f", "x"]], "term": ["return", "t"]}]}]});
    let c1 = seed(&temp.path, &live);
    assert_eq!(run(&temp.path, &["commit", &c1]).0, 0);
    let rippled = json!({"af1": 1, "afx": 1,
     "patch": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "blocks": {"entry": {"ops": [], "term": ["return", "b"]}}}],
     "ripple": [{"arity": "f", "value": 0}]});
    seed(&temp.path, &rippled);
    let g_public = cases(
        &temp.path,
        "g.json",
        &json!([{"name": "g1", "function": "g", "args": [5, 3], "expect": 3}]),
    );
    let (_, report) = search(&temp.path, &["g", "--public", &g_public, "--from", "d2"]);
    // A caller the arity intent rewrites is searched as the head states it,
    // in patches the intent derives again: none is refused.
    assert_eq!(report["search"]["derived"], true, "{report:#}");
    assert!(
        report["counts"]["generated"].as_u64().unwrap() > 0,
        "{report:#}"
    );
    assert_eq!(report["counts"]["refused"], 0, "{report:#}");
    assert_eq!(report["counts"]["skipped"], 0, "{report:#}");
}

#[test]
fn a_negation_never_evaluates_a_nested_operation_twice() {
    let temp = workspace("unnegate");
    let frame = json!({"af1": 1, "afx": 1,
      "types": [{"name": "E", "variant": ["Overflow", "Neg"]}],
      "fns": [{"fn": "u", "params": [["x", "i64"], ["y", "i64"]], "returns": "Result<i64,E>",
        "blocks": [{"name": "entry",
          "ops": [["nc", "not", ["lt", ["sub?Overflow", "x", "y"], 0]], ["!Neg", "if", "nc"]],
          "term": ["ok", "x"]}]}]});
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"name": "a", "function": "u", "args": [5, 3], "expect": {"Ok": 5}},
                {"name": "b", "function": "u", "args": [3, 5], "expect": {"Err": "Neg"}}]),
    );
    seed(&temp.path, &frame);
    let (_, report) = search(&temp.path, &["u", "--public", &public, "--from", "d1"]);
    for neighbor in neighbors(&report) {
        assert!(
            neighbor["frame"]
                .to_string()
                .matches("sub?Overflow")
                .count()
                <= 1,
            "{neighbor:#}"
        );
    }
    assert!(
        !neighbors(&report)
            .iter()
            .any(|n| n["generator"] == "negation"),
        "{report:#}"
    );
    // `not x -> x` where x is named stays available.
    let named = json!({"af1": 1, "afx": 1,
      "types": [{"name": "E", "variant": ["Overflow", "Neg"]}],
      "fns": [{"fn": "v", "params": [["x", "i64"], ["y", "i64"]], "returns": "Result<i64,E>",
        "blocks": [{"name": "entry",
          "ops": [["small", "lt", ["sub?Overflow", "x", "y"], 0], ["nc", "not", "small"], ["!Neg", "if", "nc"]],
          "term": ["ok", "x"]}]}]});
    let v_public = cases(
        &temp.path,
        "v.json",
        &json!([{"function": "v", "args": [3, 5], "expect": {"Err": "Neg"}}]),
    );
    seed(&temp.path, &named);
    let (_, report) = search(&temp.path, &["v", "--public", &v_public, "--from", "d2"]);
    let removal = find(
        &report,
        "negation",
        "entry (!Neg if)",
        "condition nc -> small",
    );
    assert_eq!(removal["public"]["passed"], 1, "{removal:#}");
}

#[test]
fn the_wall_limit_stops_between_runs_and_reports_the_elapsed_time() {
    // A seed that never ends: every case runs to its fuel cap.
    let spin = json!({"af1": 1, "fns": [{"fn": "spin", "params": [["n", "i64"]], "returns": "i64",
      "blocks": [{"name": "entry", "ops": [["zero", "const", 0]], "term": ["br", "loop", "zero"]},
        {"name": "loop", "params": [["i", "i64"]], "ops": [["one", "const", 1], ["j", "add", "i", "one"], ["done", "lt", "n", "i"]],
         "term": ["cond", "done", ["out", "i"], ["next", "j", "i"]]},
        {"name": "next", "params": [["r", "Result<i64,ArithmeticError>"], ["k", "i64"]], "ops": [],
         "term": ["switch", "r", ["Ok", "loop", "$"], ["Err", "out", "k"]]},
        {"name": "out", "params": [["v", "i64"]], "ops": [], "term": ["return", "v"]}]}]});
    let case = |i: usize| json!({"name": format!("c{i}"), "function": "spin", "args": [1_000_000_000_000_000_i64], "expect": 5});
    let one = workspace("wall-one");
    let one_public = cases(&one.path, "one.json", &json!([case(0)]));
    let c1 = seed(&one.path, &spin);
    let (_, report) = search(
        &one.path,
        &[
            "spin",
            "--public",
            &one_public,
            "--from",
            &c1,
            "--max-neighbors",
            "0",
        ],
    );
    let case_millis = report["resources"]["wall_millis"].as_u64().unwrap().max(1);
    let temp = workspace("wall");
    let public = cases(
        &temp.path,
        "cases.json",
        &Value::Array((0..8).map(case).collect()),
    );
    let c1 = seed(&temp.path, &spin);
    let bound = case_millis + case_millis / 10 + 5;
    let (status, report) = search(
        &temp.path,
        &[
            "spin",
            "--public",
            &public,
            "--from",
            &c1,
            "--max-millis",
            &bound.to_string(),
        ],
    );
    assert_eq!(status, 1, "{report:#}");
    let seed_run = &report["seed"]["public"];
    assert_eq!(seed_run["complete"], false);
    let ran = seed_run["run"].as_u64().unwrap();
    assert!((1..8).contains(&ran), "{report:#}");
    // At most the run in progress at the bound finishes.
    let elapsed = report["resources"]["wall_millis"].as_u64().unwrap();
    assert!(
        elapsed < bound + 3 * case_millis,
        "{elapsed} ms for a {bound} ms bound, {case_millis} ms per case"
    );
    assert_eq!(report["limits"]["wall_limit_reached"], true);
    let (_, text) = run(
        &temp.path,
        &[
            "search",
            "spin",
            "--public",
            &public,
            "--from",
            &c1,
            "--max-millis",
            &bound.to_string(),
        ],
    );
    let reported = text
        .lines()
        .find_map(|line| line.strip_prefix("resources: wall "))
        .and_then(|rest| rest.split(' ').next())
        .unwrap()
        .to_owned();
    assert!(
        text.contains(&format!("wall limit reached (bound {bound} ms, stopped after {reported} ms: the bound is checked before each case run, TestCase run and neighbor, and a run in progress finishes)")),
        "{text}"
    );
}

#[test]
fn a_flag_given_twice_is_refused() {
    let temp = workspace("flags");
    let public = cases(&temp.path, "cases.json", &diff_cases());
    let other = cases(&temp.path, "other.json", &sign_cases());
    let c1 = seed(&temp.path, &diff_frame());
    for args in [
        vec![
            "search",
            "diff",
            "--public",
            &public,
            "--from",
            &c1,
            "--max-neighbors",
            "3",
            "--max-neighbors",
            "70",
        ],
        vec![
            "search", "diff", "--public", &public, "--public", &other, "--from", &c1,
        ],
        vec![
            "search", "diff", "--public", &public, "--from", &c1, "--from", "c9",
        ],
        vec![
            "search",
            "diff",
            "--public",
            &public,
            "--max-millis",
            "5",
            "--max-millis",
            "9",
        ],
        vec!["try", "--on", &c1, "--on", "c9", "{\"af1\": 1}"],
        vec!["try", "--no-test", "--no-test", "{\"af1\": 1}"],
        vec!["view", "--x", "--x"],
        vec!["--json", "--json", "find"],
    ] {
        let (status, text) = run(&temp.path, &args);
        assert_eq!(status, 2, "{args:?}: {text}");
        assert!(
            text.contains("AGENT_USAGE_INVALID") && text.contains("is given twice; give it once"),
            "{args:?}: {text}"
        );
    }
    let mut words = vec!["--workspace".to_owned(), temp.path.display().to_string()];
    words.extend(["--workspace", "/elsewhere", "find"].map(str::to_owned));
    let mut out = Vec::new();
    assert_eq!(sley_agent::cli::run(&words, &mut out), 2);
    assert!(
        String::from_utf8(out)
            .unwrap()
            .contains("--workspace is given twice")
    );
    assert_eq!(uses(&temp.path), 0);
}

#[test]
fn only_a_case_that_can_run_is_an_oracle() {
    let temp = workspace("runnable");
    let c1 = seed(&temp.path, &diff_frame());
    for (file, detail) in [
        (
            json!([{"name": "t1", "function": "diff", "expect": {"Ok": 2}}]),
            "has no public case for `diff`".to_owned(),
        ),
        (
            json!([{"name": "t1", "function": "diff", "args": [5, 3, 9], "expect": {"Ok": 2}}]),
            format!(
                "no public case for `diff` in {{file}} can run on {c1}: t1: diff takes 2 argument(s), got 3"
            ),
        ),
        (
            json!([{"name": "t1", "function": "diff", "args": ["five", 3], "expect": {"Ok": 2}}]),
            format!(
                "no public case for `diff` in {{file}} can run on {c1}: t1: arg 0: expected an integer"
            ),
        ),
    ] {
        let path = cases(&temp.path, "bad.json", &file);
        let (status, text) = run(
            &temp.path,
            &["search", "diff", "--public", &path, "--from", &c1],
        );
        assert_eq!(status, 2, "{text}");
        assert!(text.starts_with("error AGENT_SEARCH_NO_ORACLE: "), "{text}");
        assert!(text.contains(&detail.replace("{file}", &path)), "{text}");
    }
    assert_eq!(uses(&temp.path), 0, "a refused search uses nothing");
    // One runnable case is enough; the others are reported unknown.
    let mixed = cases(
        &temp.path,
        "mixed.json",
        &json!([{"name": "bad", "function": "diff", "args": [1], "expect": {"Ok": 2}},
                {"name": "good", "function": "diff", "args": [5, 3], "expect": {"Ok": 2}}]),
    );
    let (_, report) = search(&temp.path, &["diff", "--public", &mixed, "--from", &c1]);
    assert_eq!(
        report["seed"]["public"]["outcomes"][0]["outcome"],
        "unknown"
    );
    assert_eq!(neighbors(&report)[0]["public"]["passed"], 1);
    assert_eq!(uses(&temp.path), 1);
}

#[test]
fn the_functions_test_cases_run_as_evidence_beside_the_public_cases() {
    let temp = workspace("own-tests");
    // Two authored rows: `own_1` fails once `add` becomes `sub`.
    let frame = json!({"af1": 1, "afx": 1, "fns": [{"fn": "diff", "params": [["a", "i64"], ["b", "i64"]],
      "returns": "Result<i64,ArithmeticError>",
      "blocks": [{"name": "entry", "ops": [["r", "add", "a", "b"]], "term": ["return", "r"]}]}],
     "test_tables": [{"name": "own", "fn": "diff", "cases": [{"args": [0, 0], "expect": {"Ok": 0}}, {"args": [2, 2], "expect": {"Ok": 4}}]}]});
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"name": "p1", "function": "diff", "args": [5, 5], "expect": {"Ok": 0}}]),
    );
    seed(&temp.path, &frame);
    let (status, text) = run(
        &temp.path,
        &["search", "diff", "--public", &public, "--from", "d1"],
    );
    assert_eq!(status, 1, "the top neighbor fails a TestCase: {text}");
    assert!(
        text.contains(&format!(
            "tests: {}; the seed passes 2/2\n",
            sley_agent::search::TESTS.replace("<fn>", "diff")
        )),
        "{text}"
    );
    assert!(text.contains(" 1. #1 opcode swap at entry.r: add -> sub (size 1): public 1/1; tests 1/2 (fail: own_1)\n"), "{text}");
    let next = text
        .lines()
        .find_map(|line| line.strip_prefix("next: "))
        .unwrap();
    assert!(next.starts_with("sley-agent try --on d1@r1 "), "{next}");
    assert!(
        next.ends_with("  # caution: it fails 1 of the 2 TestCases of diff: own_1"),
        "{next}"
    );
    let (_, report) = search(&temp.path, &["diff", "--public", &public, "--from", "d1"]);
    assert_eq!(report["seed"]["tests"]["passed"], 2);
    let top = &neighbors(&report)[0];
    assert_eq!(top["tests"]["total"], 2);
    assert_eq!(
        top["tests"]["outcomes"][1],
        json!({"name": "own_1", "outcome": "fail", "expected": "Ok(4)", "actual": "Ok(0)"})
    );
    // The ranking stays by public cases.
    assert_eq!(top["public"]["passed"], 1);
    // Without TestCases the output says only the public cases ran.
    let temp = workspace("no-tests");
    let public = cases(&temp.path, "cases.json", &diff_cases());
    let c1 = seed(&temp.path, &diff_frame());
    let (status, text) = run(
        &temp.path,
        &["search", "diff", "--public", &public, "--from", &c1],
    );
    assert_eq!(status, 0, "{text}");
    assert!(
        text.contains("tests: no TestCase targets diff in the seed; only the public cases ran\n"),
        "{text}"
    );
}

#[test]
fn an_arity_seed_searches_the_callers_it_rewrites_and_the_intent_derives_again() {
    let temp = workspace("arity-seed");
    // Live callers written in plain AF1; `ship` subtracts where it should add.
    let live = json!({"af1": 1, "fns": [
     {"fn": "line_total", "params": [["quantity", "i64"], ["price", "i64"]], "returns": "Result<i64,ArithmeticError>",
      "blocks": [{"name": "entry", "ops": [["t", "mul", "quantity", "price"]], "term": ["return", "t"]}]},
     {"fn": "order_total", "params": [["quantity", "i64"], ["price", "i64"]], "returns": "Result<i64,ArithmeticError>",
      "blocks": [{"name": "entry", "ops": [["t", "call", "line_total", "quantity", "price"]],
                  "term": ["switch", "t", ["Ok", "ship", "$"], ["Err", "fail", "$"]]},
                 {"name": "ship", "params": [["v", "i64"]], "ops": [["s", "const", 3], ["r", "sub", "v", "s"]], "term": ["return", "r"]},
                 {"name": "fail", "params": [["e", "ArithmeticError"]], "ops": [["r", "err", "e"]], "term": ["return", "r"]}]}]});
    let c1 = seed(&temp.path, &live);
    assert_eq!(run(&temp.path, &["commit", &c1]).0, 0);
    // The seed adds a fee parameter; its arity intent passes 1 at every call.
    let arity = json!({"af1": 1, "afx": 1,
     "patch": [{"fn": "line_total", "params": [["quantity", "i64"], ["price", "i64"], ["fee", "i64"]],
                "blocks": {"entry": {"ops": [["t", "mul?", "quantity", "price"]], "term": ["return", ["add", "t", "fee"]]}}}],
     "ripple": [{"arity": "line_total", "value": 1}]});
    seed(&temp.path, &arity);
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"name": "o1", "function": "order_total", "args": [2, 5], "expect": {"Ok": 14}},
                {"name": "o2", "function": "order_total", "args": [1, 1], "expect": {"Ok": 5}}]),
    );
    let (status, text) = run(
        &temp.path,
        &["search", "order_total", "--public", &public, "--from", "d2"],
    );
    assert_eq!(status, 0, "{text}");
    assert!(
        text.contains("derived: the ripple intents of d2@r1 rewrite order_total; each neighbor is a patch of order_total as the head states it, and the intents derive it again\n"),
        "{text}"
    );
    assert!(
        text.contains(" 1. #1 opcode swap at ship.r: sub -> add (size 1): public 2/2\n"),
        "{text}"
    );
    let (_, report) = search(
        &temp.path,
        &["order_total", "--public", &public, "--from", "d2"],
    );
    assert_eq!(report["search"]["derived"], true);
    assert_eq!(report["counts"]["refused"], 0, "{report:#}");
    assert_eq!(report["counts"]["skipped"], 0, "{report:#}");
    // Every neighbor is a patch in the seed's dialect; one of the call
    // block is derived again (the fee still passed).
    for neighbor in neighbors(&report) {
        assert!(
            neighbor["frame"].get("patch").is_some() && neighbor["frame"]["afx"] == 1,
            "{neighbor:#}"
        );
    }
    let call = find(
        &report,
        "operand substitution",
        "entry.t",
        "operand 1: price -> quantity",
    );
    assert_eq!(call["status"], "evaluated");
    assert_eq!(
        call["public"]["outcomes"][0]["actual"],
        json!({"Ok": 2}),
        "2*2 + 1 - 3: {call:#}"
    );
    let top = &neighbors(&report)[0];
    let (status, text) = apply(&temp.path, Some("d2@r1"), &top["frame"], &public);
    assert_eq!(status, 0, "{text}");
    assert!(text.contains("public: 2/2 passed"), "{text}");
    // A guard intent refuses every frame that restates the guarded function:
    // its changes are skipped, and the output says why.
    let temp = workspace("guard-seed");
    let base = json!({"af1": 1, "types": [{"name": "SE", "variant": ["Neg", "Big"]}],
     "fns": [
     {"fn": "nonneg", "params": [["v", "i64"]], "returns": "Result<i64,SE>",
      "blocks": [{"name": "entry", "ops": [["zero", "const", 0], ["neg", "lt", "v", "zero"]], "term": ["cond", "neg", "no", "yes"]},
                 {"name": "no", "ops": [["e", "variant", "SE.Neg"], ["r", "err", "e"]], "term": ["return", "r"]},
                 {"name": "yes", "ops": [["r", "ok", "v"]], "term": ["return", "r"]}]},
     {"fn": "f", "params": [["n", "i64"]], "returns": "i64",
      "blocks": [{"name": "entry", "ops": [["x", "call", "nonneg", "n"]], "term": ["switch", "x", ["Ok", "done", "$"], ["Err", "bad", "$"]]},
                 {"name": "done", "params": [["v", "i64"]], "term": ["return", "v"]},
                 {"name": "bad", "params": [["e", "SE"]], "ops": [["m", "const", -1]], "term": ["return", "m"]}]}]});
    let c1 = seed(&temp.path, &base);
    assert_eq!(run(&temp.path, &["commit", &c1]).0, 0);
    let guard = json!({"af1": 1, "afx": 1,
     "fns": [{"fn": "small", "params": [["n", "i64"]], "returns": "Result<i64,SE>",
              "blocks": [{"name": "entry", "ops": [["!Big", "if", ["gt", "n", 100]]], "term": ["ok", "n"]}]}],
     "ripple": [{"guard": "small", "arg": "n", "in": ["f"], "mode": "entry", "handler": "bad"}]});
    seed(&temp.path, &guard);
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"function": "f", "args": [12], "expect": 13}]),
    );
    let (status, text) = run(
        &temp.path,
        &["search", "f", "--public", &public, "--from", "d2"],
    );
    assert_eq!(status, 1, "{text}");
    assert!(
        text.contains("neighbors: 0 generated, 0 kernel-valid, 0 refused, 0 evaluated; "),
        "{text}"
    );
    assert!(
        text.contains("next: no neighbor was evaluated: the guard intent of d2@r1 rewrites f and refuses any frame that restates it"),
        "{text}"
    );
}

#[test]
fn an_af1x_seed_leaving_live_af1x_code_unstated_counts_skips_not_refusals() {
    let temp = workspace("unstated-live");
    let base = json!({"af1": 1, "afx": 1,
     "types": [{"name": "E", "variant": ["Neg", "Overflow"]}],
     "fns": [{"fn": "f", "params": [["a", "i64"], ["b", "i64"]], "returns": "Result<i64,E>",
       "blocks": [{"name": "entry",
         "ops": [["!Neg", "if", ["lt", "a", 0]], ["x", "mul?Overflow", "a", "b"], ["y", "add?Overflow", "x", 3]],
         "term": ["ok", "y"]}]}]});
    let c1 = seed(&temp.path, &base);
    assert_eq!(run(&temp.path, &["commit", &c1]).0, 0);
    let other = json!({"af1": 1, "afx": 1,
     "fns": [{"fn": "g", "params": [["a", "i64"]], "returns": "Result<i64,E>",
       "blocks": [{"name": "entry", "ops": [["r", "call?", "f", "a", 2]], "term": ["ok", "r"]}]}]});
    let c2 = seed(&temp.path, &other);
    let public = cases(
        &temp.path,
        "cases.json",
        &json!([{"name": "q1", "function": "f", "args": [2, 3], "expect": {"Ok": 7}},
                {"name": "q2", "function": "f", "args": [-1, 3], "expect": {"Err": "Neg"}}]),
    );
    let (_, report) = search(&temp.path, &["f", "--public", &public, "--from", &c2]);
    assert_eq!(report["counts"]["refused"], 0, "{report:#}");
    assert_eq!(report["counts"]["generated"], 0, "{report:#}");
    assert!(report["counts"]["skipped"].as_u64().unwrap() > 0);
}

#[test]
fn a_search_ledger_line_counts_the_case_file_whether_it_runs_or_is_refused() {
    let temp = workspace("input-bytes");
    let public = cases(&temp.path, "cases.json", &diff_cases());
    let size = fs::metadata(&public).unwrap().len();
    let c1 = seed(&temp.path, &diff_frame());
    for _ in 0..3 {
        run(
            &temp.path,
            &[
                "search",
                "diff",
                "--public",
                &public,
                "--from",
                &c1,
                "--max-neighbors",
                "1",
            ],
        );
    }
    let (status, _) = run(
        &temp.path,
        &["search", "nothing", "--public", &public, "--from", &c1],
    );
    assert_eq!(status, 2);
    let events = fs::read_to_string(temp.path.join(".sley/events.jsonl")).unwrap();
    let lines: Vec<Value> = events
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|line| line["cmd"] == "search")
        .collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[2]["refusal"], "AGENT_SEARCH_SEED_INVALID");
    for line in &lines {
        assert_eq!(line["input_bytes"], size, "{line}");
    }
    // Without a readable case file, the command line.
    let (status, _) = run(
        &temp.path,
        &[
            "search",
            "diff",
            "--public",
            "/nonexistent.json",
            "--from",
            &c1,
        ],
    );
    assert_eq!(status, 2);
    let events = fs::read_to_string(temp.path.join(".sley/events.jsonl")).unwrap();
    let last: Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
    let words = [
        "search",
        "diff",
        "--public",
        "/nonexistent.json",
        "--from",
        &c1,
    ];
    let line_bytes = words.iter().map(|word| word.len()).sum::<usize>() + words.len() - 1;
    assert_eq!(last["input_bytes"], line_bytes);
}

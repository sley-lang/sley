//! `call --fuel` lowers the dev-loop fuel ceiling: exhausting runs report the
//! limit, defaults are unchanged, invalid and above-ceiling values are
//! refused, and batches honor the same ceiling.
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(path: &std::path::Path, args: &[&str]) -> (i32, Value) {
    let mut words = vec![
        "--workspace".into(),
        path.display().to_string(),
        "--json".into(),
    ];
    words.extend(args.iter().map(|a| (*a).to_owned()));
    let mut output = Vec::new();
    let code = sley_agent::cli::run(&words, &mut output);
    (code, serde_json::from_slice(&output).unwrap_or(Value::Null))
}

fn init(path: &std::path::Path) {
    fs::create_dir_all(path).unwrap();
    sley_agent::genesis::init(path, None, sley_agent::genesis::INIT_CEILINGS).unwrap();
}

/// A call of `keep` is unchanged without `--fuel` and at the full ceiling.
#[test]
fn fuel_default_matches_full_ceiling() {
    let path = std::env::temp_dir().join(format!(
        "sley-call-fuel-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    init(&path);
    let temp = Temp(path);
    let frame = json!({"af1":1,"afx":1,"fns":[{"fn":"keep","params":[["x","i64"]],
        "returns":"i64","body":[["return","x"]]}]})
    .to_string();
    let (code, proposal) = run(&temp.0, &["try", &frame]);
    assert_eq!(code, 0, "{proposal}");
    let handle = proposal["handle"].as_str().unwrap();
    let (_, default) = run(&temp.0, &["call", "keep", "5", "--on", handle]);
    let (other, full) = run(
        &temp.0,
        &["call", "keep", "5", "--on", handle, "--fuel", "100000000"],
    );
    assert_eq!(other, 0, "{full}");
    for run in [&default, &full] {
        assert_eq!(run["result"], 5, "{run}");
        assert_eq!(run["fuel"], 1, "{run}");
    }
    let stripped = |value: &Value| {
        let mut value = value.clone();
        value.as_object_mut().unwrap().remove("vm_micros");
        value
    };
    assert_eq!(stripped(&default), stripped(&full));
}

/// A low ceiling exhausts: the run reports the Fuel limit, charges at most
/// the ceiling, and a large-enough ceiling on the same call answers.
#[test]
fn low_fuel_exhausts_and_a_higher_ceiling_answers() {
    let path = std::env::temp_dir().join(format!(
        "sley-call-fuel-low-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    init(&path);
    let temp = Temp(path);
    let source = temp.0.join("fib.py");
    fs::write(&source, "def fib(n: int) -> int:\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\n").unwrap();
    let (code, proposal) = run(&temp.0, &["try", "--familiar", source.to_str().unwrap()]);
    assert_eq!(code, 0, "{proposal}");
    let handle = proposal["handle"].as_str().unwrap();
    let (code, starved) = run(
        &temp.0,
        &["call", "fib", "12", "--on", handle, "--fuel", "300"],
    );
    assert_eq!(code, 0, "{starved}");
    assert_eq!(starved["result"], json!({"limit": "Fuel"}), "{starved}");
    assert_eq!(starved["fuel"], 300, "{starved}");
    let (code, fed) = run(
        &temp.0,
        &["call", "fib", "12", "--on", handle, "--fuel", "7000"],
    );
    assert_eq!(code, 0, "{fed}");
    assert_eq!(fed["result"], 144, "{fed}");
    assert!(
        fed["fuel"].as_u64().unwrap() > 300,
        "the ceiling was the difference: {fed}"
    );
}

/// `--fuel` honors its bounds (1 to the fixed ceiling) and refuses to raise
/// the ceiling: above-`call_limits` fuel, non-numbers, zero, a missing value
/// and a repeated flag are `AGENT_USAGE_INVALID`.
#[test]
fn fuel_bounds_and_above_ceiling_are_refused() {
    let path = std::env::temp_dir().join(format!(
        "sley-call-fuel-refuse-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    init(&path);
    let temp = Temp(path);
    let frame = json!({"af1":1,"afx":1,"fns":[{"fn":"keep","params":[["x","i64"]],
        "returns":"i64","body":[["return","x"]]}]})
    .to_string();
    let (code, proposal) = run(&temp.0, &["try", &frame]);
    assert_eq!(code, 0, "{proposal}");
    let handle = proposal["handle"].as_str().unwrap().to_owned();
    for value in ["0", "100000001", "18446744073709551615", "abc", "-3", "1.5"] {
        let (code, refused) = run(
            &temp.0,
            &["call", "keep", "5", "--on", &handle, "--fuel", value],
        );
        assert_eq!(code, 2, "{value}: {refused}");
        assert_eq!(
            refused["error"], "AGENT_USAGE_INVALID",
            "{value}: {refused}"
        );
    }
    // A `--fuel` without a value and a repeated flag are the same refusal.
    let mut output = Vec::new();
    let code = sley_agent::cli::run(
        &[
            "--workspace".into(),
            temp.0.display().to_string(),
            "--json".into(),
            "call".into(),
            "keep".into(),
            "5".into(),
            "--on".into(),
            handle.clone(),
            "--fuel".into(),
        ],
        &mut output,
    );
    assert_eq!(code, 2, "{}", String::from_utf8_lossy(&output));
    let refused: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(refused["error"], "AGENT_USAGE_INVALID", "{refused}");
    let (code, _) = run(
        &temp.0,
        &[
            "call", "keep", "5", "--on", &handle, "--fuel", "5", "--fuel", "8",
        ],
    );
    assert_eq!(code, 2);
    // The bounds themselves hold: 1 is the lowest accepted ceiling.
    let (code, one) = run(
        &temp.0,
        &["call", "keep", "5", "--on", &handle, "--fuel", "1"],
    );
    assert_eq!(code, 0, "{one}");
    assert_eq!(one["result"], 5, "{one}");
}

/// A batch run honors the same lowered ceiling: every row reports the Fuel
/// limit under it, and the batch answers at a larger ceiling.
#[test]
fn batch_runs_under_the_fuel_ceiling() {
    let path = std::env::temp_dir().join(format!(
        "sley-call-fuel-batch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    init(&path);
    let temp = Temp(path);
    let source = temp.0.join("fib.py");
    fs::write(&source, "def fib(n: int) -> int:\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\n").unwrap();
    let (code, proposal) = run(&temp.0, &["try", "--familiar", source.to_str().unwrap()]);
    assert_eq!(code, 0, "{proposal}");
    let handle = proposal["handle"].as_str().unwrap().to_owned();
    let rows = "[[12],[12],[2]]\n".to_owned();
    let batch = temp.0.join("rows.json");
    fs::write(&batch, &rows).unwrap();
    let batched = |fuel: &str| {
        let words = vec![
            "--workspace".into(),
            temp.0.display().to_string(),
            "--json".into(),
            "call".into(),
            "fib".into(),
            "--batch".into(),
            batch.display().to_string(),
            "--on".into(),
            handle.clone(),
            "--fuel".into(),
            fuel.into(),
        ];
        let mut output = Vec::new();
        let code = sley_agent::cli::run(&words, &mut output);
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        (code, lines)
    };
    let (code, starved) = batched("300");
    assert_eq!(code, 0);
    // The fib(12) rows exhaust at 300 fuel; fib(2) still answers.
    assert_eq!(
        starved,
        vec![json!({"limit": "Fuel"}), json!({"limit": "Fuel"}), json!(1)],
        "{starved:?}"
    );
    let (code, fed) = batched("7000");
    assert_eq!(code, 0);
    assert_eq!(fed, vec![json!(144), json!(144), json!(1)], "{fed:?}");
}

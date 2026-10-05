//! Explicit reference execution preserves public answers/accounting and state.
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
    (code, serde_json::from_slice(&output).unwrap())
}

#[test]
fn reference_call_matches_result_fuel_instructions_and_preserves_head() {
    let path = std::env::temp_dir().join(format!(
        "sley-reference-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let temp = Temp(path);
    fs::create_dir_all(&temp.0).unwrap();
    sley_agent::genesis::init(&temp.0, Some([82; 32]), sley_agent::genesis::INIT_CEILINGS).unwrap();
    let before = run(&temp.0, &["view"]).1["root"].clone();
    let frame = json!({"af1":1,"afx":1,"fns":[{"fn":"narrow","params":[["x","i64"]],
        "returns":"Result<i8,ArithmeticError>","body":[["ok",["to?","i8","x"]]]}]})
    .to_string();
    let (code, proposal) = run(&temp.0, &["try", &frame]);
    assert_eq!(code, 0, "{proposal}");
    let handle = proposal["handle"].as_str().unwrap();
    for input in ["-129", "-128", "0", "127", "128", "9223372036854775807"] {
        let (code, mut compact) = run(&temp.0, &["call", "narrow", input, "--on", handle]);
        let (other, mut reference) = run(
            &temp.0,
            &["call", "narrow", input, "--on", handle, "--reference"],
        );
        assert_eq!(code, other);
        compact.as_object_mut().unwrap().remove("vm_micros");
        reference.as_object_mut().unwrap().remove("vm_micros");
        assert_eq!(compact, reference);
    }
    assert_eq!(run(&temp.0, &["view"]).1["root"], before);
}

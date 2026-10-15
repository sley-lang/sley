//! Headerless proposals inherit their signature from the exact accepted graph.
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sley_agent::{genesis, workspace::Workspace};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "sley-scope-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        genesis::init(&path, Some([7; 32]), genesis::INIT_CEILINGS).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(path: &Path, args: &[&str]) -> (i32, Value) {
    let mut words = vec![
        "--workspace".to_owned(),
        path.display().to_string(),
        "--json".to_owned(),
    ];
    words.extend(args.iter().map(|s| (*s).to_owned()));
    let mut out = Vec::new();
    let code = sley_agent::cli::run(&words, &mut out);
    let text = String::from_utf8(out).unwrap();
    (
        code,
        serde_json::from_str(text.trim()).unwrap_or_else(|_| panic!("{text}")),
    )
}
fn root(path: &Path) -> String {
    run(path, &["view"]).1["root"].as_str().unwrap().to_owned()
}
fn frame(name: &str, n: i64) -> String {
    json!({"af1":1,"afx":1,"fns":[{"fn":name,"params":[],"returns":"i64","body":[["return",n]]}]})
        .to_string()
}
fn commit(path: &Path, value: &str) {
    let (code, report) = run(path, &["try", value]);
    assert_eq!(code, 0, "{report}");
    let (code, report) = run(path, &["commit", report["handle"].as_str().unwrap()]);
    assert_eq!(code, 0, "{report}");
}

#[test]
fn json_body_uses_bound_signature_preserves_parameters_and_keeps_other_functions() {
    let temp = Temp::new();
    let initial = json!({"af1":1,"afx":1,"fns":[
        {"fn":"target","params":[["xs","Vec<i64>"]],"returns":"i64","visibility":"private","body":[["return",0]]},
        {"fn":"other","params":[],"returns":"i64","body":[["return",7]]}
    ]});
    commit(&temp.0, &initial.to_string());
    let ws = Workspace::at(&temp.0);
    let before = ws.read_head().unwrap();
    let base = root(&temp.0);
    let body = json!([
        ["var", "n", "i64", 0],
        ["for", "x", "xs", [["set", "n", ["add", "n", "x"]]]],
        ["return", "n"]
    ]);
    let (code, report) = run(
        &temp.0,
        &[
            "try",
            &body.to_string(),
            "--body",
            "target",
            "--base-root",
            &base,
        ],
    );
    assert_eq!(code, 0, "{report}");
    let candidate = report["handle"].as_str().unwrap();
    assert_eq!(root(&temp.0), base);
    assert_eq!(
        run(&temp.0, &["call", "target", "[2,3,-1]", "--on", candidate]).1["result"],
        4
    );
    assert_eq!(run(&temp.0, &["commit", candidate]).0, 0);
    let after = ws.read_head().unwrap();
    for object in before.objects() {
        if let sley_mutate::value::EntityBodyValue::Parameter(_) = object.record().body {
            assert_eq!(
                after
                    .program()
                    .object(&object.record().entity_id)
                    .unwrap()
                    .object_id(),
                object.object_id()
            );
        }
        if let sley_mutate::value::EntityBodyValue::Function(old) = &object.record().body {
            let Some(sley_mutate::value::EntityBodyValue::Function(new)) =
                after.program().body(&object.record().entity_id)
            else {
                panic!("function disappeared")
            };
            assert_eq!(old.parameters, new.parameters);
            assert_eq!(old.result_type, new.result_type);
            assert_eq!(old.effects, new.effects);
            assert_eq!(old.visibility, new.visibility);
            assert_eq!(old.contracts, new.contracts);
        }
    }
    assert_eq!(run(&temp.0, &["call", "other"]).1["result"], 7);
    let (code, report) = run(
        &temp.0,
        &[
            "try",
            &body.to_string(),
            "--body",
            "target",
            "--base-root",
            &base,
        ],
    );
    assert_eq!(code, 2, "{report}");
    assert_eq!(report["error"], "AGENT_PROPOSAL_STALE");
}

#[test]
fn body_proposals_require_a_live_target_bound_root_and_statement_list() {
    let temp = Temp::new();
    commit(&temp.0, &frame("target", 0));
    let base = root(&temp.0);
    for args in [
        vec!["try", "[[\"return\",1]]", "--body", "target"],
        vec![
            "try",
            "[[\"return\",1]]",
            "--body",
            "missing",
            "--base-root",
            &base,
        ],
        vec!["try", "{}", "--body", "target", "--base-root", &base],
        vec![
            "try",
            "[[\"return\",1]]",
            "--body",
            "target",
            "--base-root",
            &base,
            "--on",
            "c1",
        ],
        vec![
            "try",
            "[[\"return\",1]]",
            "--body",
            "target",
            "--base-root",
            &base,
            "--functions",
            "other",
        ],
    ] {
        let (code, report) = run(&temp.0, &args);
        assert_eq!(code, 2, "{report}");
        assert_eq!(root(&temp.0), base);
    }
    let (code, report) = run(
        &temp.0,
        &[
            "try",
            "[[\"return\",true]]",
            "--body",
            "target",
            "--base-root",
            &base,
        ],
    );
    assert_ne!(code, 0, "{report}");
    assert!(
        report.to_string().contains("returns i64, not bool"),
        "{report}"
    );
}

#[cfg(feature = "familiar")]
#[test]
fn headerless_familiar_positions_are_original_and_headers_cannot_replace_the_target() {
    let temp = Temp::new();
    commit(
        &temp.0,
        &json!({"af1":1,"afx":1,"fns":[
            {"fn":"target","params":[["limit","u64"]],"returns":"u64","body":[["return",0]]}
        ]})
        .to_string(),
    );
    let base = root(&temp.0);
    let file = temp.0.join("body.txt");
    fs::write(
        &file,
        "var n: u64 = 0\nfor i in 0..limit { n = n + i }\nreturn n",
    )
    .unwrap();
    let args = [
        "try",
        "--familiar",
        file.to_str().unwrap(),
        "--body",
        "target",
        "--base-root",
        &base,
    ];
    let (code, report) = run(&temp.0, &args);
    assert_eq!(code, 0, "{report}");
    assert_eq!(
        run(
            &temp.0,
            &[
                "call",
                "target",
                "4",
                "--on",
                report["handle"].as_str().unwrap()
            ]
        )
        .1["result"],
        6
    );
    fs::write(&file, "# body positions\nreturn missing").unwrap();
    let (code, report) = run(&temp.0, &args);
    assert_ne!(code, 0, "{report}");
    assert_eq!(report["obligations"][0]["text"]["line"], 2, "{report}");
    fs::write(&file, "fn other() -> i64 { return 9 }").unwrap();
    let (code, report) = run(&temp.0, &args);
    assert_ne!(code, 0, "{report}");
    assert_eq!(report["state"], "text");
    assert_eq!(root(&temp.0), base);
}

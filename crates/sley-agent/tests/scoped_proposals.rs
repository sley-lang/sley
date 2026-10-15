//! Root binding and exact preservation for controller-owned function scopes.
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
fn scoped_json_preserves_every_existing_protected_object() {
    let temp = Temp::new();
    commit(&temp.0, &frame("sentinel", 7));
    let ws = Workspace::locate(&temp.0).unwrap();
    let before = ws.read_head().unwrap();
    let base = root(&temp.0);
    let (code, report) = run(
        &temp.0,
        &[
            "try",
            &frame("added", 9),
            "--base-root",
            &base,
            "--functions",
            "added",
        ],
    );
    assert_eq!(code, 0, "{report}");
    assert_eq!(root(&temp.0), base, "try must not commit");
    let (code, report) = run(&temp.0, &["commit", report["handle"].as_str().unwrap()]);
    assert_eq!(code, 0, "{report}");
    let after = ws.read_head().unwrap();
    // A new function may extend a module's membership, but no other old
    // object (including the sentinel's descendants) may be rewritten.
    for object in before.objects() {
        if matches!(
            object.record().body,
            sley_mutate::value::EntityBodyValue::Namespace(_)
        ) {
            continue;
        }
        assert_eq!(
            after
                .program()
                .object(&object.record().entity_id)
                .unwrap()
                .object_id(),
            object.object_id()
        );
    }
}

#[test]
fn stale_and_out_of_scope_proposals_never_create_candidates() {
    let temp = Temp::new();
    let stale = root(&temp.0);
    commit(&temp.0, &frame("sentinel", 7));
    let base = root(&temp.0);
    for (proposal, pinned, scope, expected) in [
        (
            frame("added", 9),
            stale.as_str(),
            "added",
            "AGENT_PROPOSAL_STALE",
        ),
        (
            frame("sentinel", 99),
            base.as_str(),
            "added",
            "AGENT_PROPOSAL_SCOPE",
        ),
        (
            json!({"af1":1,"delete":["sentinel"]}).to_string(),
            base.as_str(),
            "added",
            "AGENT_PROPOSAL_SCOPE",
        ),
        (
            frame("added", 9),
            base.as_str(),
            "added,added",
            "AGENT_PROPOSAL_SCOPE",
        ),
        (frame("added", 9), "00", "added", "AGENT_USAGE_INVALID"),
    ] {
        let (code, report) = run(
            &temp.0,
            &[
                "try",
                &proposal,
                "--base-root",
                pinned,
                "--functions",
                scope,
            ],
        );
        assert_eq!(code, 2, "{report}");
        assert!(report.to_string().contains(expected), "{report}");
        assert_eq!(root(&temp.0), base);
    }
    let (code, report) = run(
        &temp.0,
        &[
            "try",
            &frame("added", 9),
            "--base-root",
            &base,
            "--functions",
            "added",
        ],
    );
    assert_eq!(code, 0, "{report}");
    assert_eq!(
        report["handle"], "c2",
        "refusals must not allocate candidates"
    );
}

#[cfg(feature = "familiar")]
#[test]
fn familiar_signature_and_caller_update_is_one_atomic_candidate() {
    let temp = Temp::new();
    let file = temp.0.join("proposal.txt");
    fs::write(
        &file,
        "fn scale(x: i64) -> i64 { return x * 2 }\nfn total(x: i64) -> i64 { return scale(x) }",
    )
    .unwrap();
    let (code, report) = run(&temp.0, &["try", "--familiar", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{report}");
    assert_eq!(
        run(&temp.0, &["commit", report["handle"].as_str().unwrap()]).0,
        0
    );
    let base = root(&temp.0);
    fs::write(&file, "fn scale(x: i64, factor: i64) -> i64 { return x * factor }\nfn total(x: i64) -> i64 { return scale(x, 3) }").unwrap();
    let (code, report) = run(
        &temp.0,
        &[
            "try",
            "--familiar",
            file.to_str().unwrap(),
            "--base-root",
            &base,
            "--functions",
            "scale,total",
        ],
    );
    assert_eq!(code, 0, "{report}");
    assert_eq!(root(&temp.0), base);
    let candidate = report["handle"].as_str().unwrap();
    let (code, answer) = run(&temp.0, &["call", "total", "4", "--on", candidate]);
    assert_eq!(code, 0, "{answer}");
    assert_eq!(answer["result"], 12);
    assert_eq!(run(&temp.0, &["commit", candidate]).0, 0);
    let next = root(&temp.0);
    assert_ne!(base, next);
    fs::write(&file, "fn total(x: i64) -> i64 { return scale(x, 4) }").unwrap();
    let (code, report) = run(
        &temp.0,
        &[
            "try",
            "--familiar",
            file.to_str().unwrap(),
            "--base-root",
            &next,
            "--functions",
            "total",
        ],
    );
    assert_eq!(code, 0, "{report}");
}

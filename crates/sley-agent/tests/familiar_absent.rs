//! Built without the optional familiar frontend (`--no-default-features`,
//! ADR-0055), `try --familiar` is refused and says why; JSON frames are
//! unaffected.
#![cfg(not(feature = "familiar"))]

use std::fs;

use sley_agent::genesis;

#[test]
fn the_familiar_switch_is_refused_without_the_frontend() {
    let dir = std::env::temp_dir().join(format!("sley-familiar-absent-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    genesis::init(&dir, Some([7; 32]), genesis::INIT_CEILINGS).unwrap();
    let text = dir.join("proposal.txt");
    fs::write(&text, "fn f(x: i64) -> i64 { return x }\n").unwrap();
    let run = |args: &[&str]| {
        let mut words = vec!["--workspace".to_owned(), dir.display().to_string()];
        words.extend(args.iter().map(|arg| (*arg).to_owned()));
        let mut out = Vec::new();
        let status = sley_agent::cli::run(&words, &mut out);
        (status, String::from_utf8(out).unwrap())
    };
    let path = text.display().to_string();
    let (status, out) = run(&["try", "--familiar", &path, "--no-test"]);
    assert_ne!(status, 0, "{out}");
    assert!(
        out.contains("built without the optional familiar frontend"),
        "{out}"
    );
    let frame = r#"{"af1":1,"afx":1,"fns":[{"fn":"f","params":[["x","i64"]],"returns":"i64","body":[["return","x"]]}]}"#;
    let (status, out) = run(&["try", frame, "--no-test"]);
    assert_eq!(status, 0, "{out}");
    let (_, help) = run(&["help", "familiar"]);
    assert!(help.contains("built without"), "{help}");
    let _ = fs::remove_dir_all(&dir);
}

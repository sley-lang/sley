use super::{Fixture, cli, literal_fixture};
use serde_json::json;
use sley_agent::help;

fn examples() -> Vec<&'static str> {
    let examples: Vec<_> = help::RESIDUAL_QUICK
        .split("```json\n")
        .skip(1)
        .map(|rest| rest.split("```").next().unwrap().trim())
        .collect();
    assert_eq!(examples.len(), 2, "derive and sparse edit examples");
    examples
}

#[test]
fn help_residual_defaults_to_bounded_quick_reference() {
    assert!(help::RESIDUAL_QUICK.len() <= 3_500);
    assert!(help::RESIDUAL_QUICK.len() * 3 < help::RESIDUAL.len());
    assert!(help::RESIDUAL_QUICK.contains("help residual-reference"));
    let fixture = Fixture::new();
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    for (topic, expected) in [
        ("residual", help::RESIDUAL_QUICK),
        ("residual-quick", help::RESIDUAL_QUICK),
        ("residual-reference", help::RESIDUAL),
    ] {
        assert!(help::TOPICS.contains(&topic));
        assert_eq!(help::topic(topic).as_deref(), Some(expected));
        let args = vec![
            "--workspace".into(),
            fixture.dir.display().to_string(),
            "help".into(),
            topic.into(),
        ];
        let mut output = Vec::new();
        assert_eq!(sley_agent::cli::run(&args, &mut output), 0);
        assert_eq!(
            String::from_utf8(output).unwrap().trim_end(),
            expected.trim_end()
        );
        assert_eq!(
            fixture.workspace.read_head().unwrap().transaction_id(),
            before
        );
    }
}

#[test]
fn help_residual_quick_pipeline_executes_with_checked_rounding() {
    let fixture = Fixture::new();
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    let (code, candidate) = cli(
        &fixture.dir,
        &["residual", "try", examples()[0], "--no-test"],
    );
    assert_eq!(code, 0, "{candidate}");
    assert_eq!(candidate["kernel"], "valid");
    let handle = candidate["handle"].as_str().unwrap();
    for (argument, expected) in [
        (-5, json!({"Ok": -7})),
        (5, json!({"Ok": 7})),
        (0, json!({"Ok": 0})),
        (i64::MAX, json!({"Err": {"ArithmeticError": "Overflow"}})),
    ] {
        let (code, result) = cli(
            &fixture.dir,
            &["call", "scaled_half", &argument.to_string(), "--on", handle],
        );
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], expected);
    }
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
}

#[test]
fn help_residual_quick_edit_changes_only_the_selected_literal() {
    let fixture = literal_fixture();
    let before = fixture.workspace.read_head().unwrap().transaction_id();
    let (code, candidate) = cli(
        &fixture.dir,
        &["residual", "try", examples()[1], "--no-test"],
    );
    assert_eq!(code, 0, "{candidate}");
    assert_eq!(candidate["kernel"], "valid");
    let handle = candidate["handle"].as_str().unwrap();
    for (function, expected) in [("adjust", 12), ("other", 8), ("caller", 12)] {
        let (code, result) = cli(&fixture.dir, &["call", function, "5", "--on", handle]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"], json!({"Ok": expected}));
    }
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        before
    );
}

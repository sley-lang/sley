use super::interface_tests::check;
use super::{Fixture, bytes, cli, expand, guarded_request};
use serde_json::{Value, json};
use sley_agent::opcodes;

fn request(word: &str, operands: &[Value]) -> Value {
    let mut expression = vec![json!(word)];
    expression.extend_from_slice(operands);
    json!({"residual":1,"base":"current","operation":"derive",
        "fragment":{"id":"checked_pipeline","version":1},"scope":["sample"],
        "bindings":{"params":[["x","i8"],["y","i8"],["amount","u32"]],
            "returns":"Result<i8,ArithmeticError>","steps":[["out",expression]],
            "arithmetic_failure":{"propagate":true},"rounding":"toward_zero","result":"out"}})
}

#[test]
fn all_pipeline_opcode_spellings_match_ordinary_af1_x_execution() {
    for row in opcodes::OPCODES
        .iter()
        .filter(|row| (64..=71).contains(&row.tag))
    {
        let fixture = Fixture::new();
        let operands = match row.tag {
            69 => vec![json!("x")],
            70 | 71 => vec![json!("x"), json!("amount")],
            _ => vec![json!("x"), json!("y")],
        };
        // Independent ordinary AF1-X reference returns the raw checked result.
        // The residual pipeline instead unwraps/repackages it through explicit
        // propagation. Compare actual VM values, including every failure code.
        let mut operation = vec![json!("out"), json!(row.mnemonic)];
        operation.extend_from_slice(&operands);
        let frame = json!({"af1":1,"afx":1,"fns":[{"fn":"sample",
            "params":[["x","i8"],["y","i8"],["amount","u32"]],
            "returns":"Result<i8,ArithmeticError>","blocks":[{"name":"entry",
                "ops":[operation],"term":["return","out"]}]}]});
        let (code, reference) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
        assert_eq!(code, 0, "{reference}");
        assert_eq!(reference["verdict"]["valid"], true, "{reference}");
        let reference_handle = reference["handle"].as_str().unwrap().to_owned();
        let inputs = [
            [7, 2, 1],
            [-7, 2, 0],
            [127, 1, 7],
            [-128, 0, 8],
            [1, 0, 32],
            [-128, -1, 0],
            [0, 0, 0],
        ];
        let expected: Vec<_> = inputs
            .iter()
            .map(|args| {
                let [x, y, amount] = args.map(|value| value.to_string());
                let (code, output) = cli(
                    &fixture.dir,
                    &["call", "sample", &x, &y, &amount, "--on", &reference_handle],
                );
                assert_eq!(code, 0, "{output}");
                output["result"].clone()
            })
            .collect();
        for word in [
            row.mnemonic.to_owned(),
            row.name.to_owned(),
            row.tag.to_string(),
        ] {
            let request = request(&word, &operands);
            let original = bytes(&request);
            let preflight = check(&fixture, &request, &json!({})).unwrap();
            assert_eq!(
                preflight["connections"][0]["expression_types"],
                "constraints_checked"
            );
            let expanded = expand(&request);
            assert_eq!(
                expanded.frame["fns"][0]["blocks"][0]["ops"][0][1],
                format!("{word}?")
            );
            let (code, candidate) = cli(
                &fixture.dir,
                &["residual", "try", &request.to_string(), "--no-test"],
            );
            assert_eq!(code, 0, "{word}: {candidate}");
            assert_eq!(candidate["kernel"], "valid");
            let handle = candidate["handle"].as_str().unwrap();
            for (args, expected) in inputs.iter().zip(&expected) {
                let [x, y, amount] = args.map(|value| value.to_string());
                let (code, output) = cli(
                    &fixture.dir,
                    &["call", "sample", &x, &y, &amount, "--on", handle],
                );
                assert_eq!(code, 0, "{word}: {output}");
                assert_eq!(&output["result"], expected, "{word}, {args:?}: {output}");
            }
            assert_eq!(bytes(&request), original);
        }
    }
}

#[test]
fn pipeline_aliases_preserve_type_conflicts_and_the_closed_arithmetic_vocabulary() {
    let fixture = Fixture::new();
    let head = fixture.workspace.read_head().unwrap().transaction_id();
    for row in opcodes::OPCODES
        .iter()
        .filter(|row| (64..=71).contains(&row.tag))
    {
        for word in [
            row.mnemonic.to_owned(),
            row.name.to_owned(),
            row.tag.to_string(),
        ] {
            let mut bad = if row.tag == 69 {
                let mut request = request(&word, &[json!("x")]);
                request["bindings"]["params"][0][1] = json!("u8");
                request["bindings"]["returns"] = json!("Result<u8,ArithmeticError>");
                request
            } else if matches!(row.tag, 70 | 71) {
                let mut request = request(&word, &[json!("x"), json!("amount")]);
                request["bindings"]["params"][2][1] = json!("i8");
                request
            } else {
                request(&word, &[json!("x"), json!({"type":"u8","value":1})])
            };
            let original = bytes(&bad);
            let (code, report) = cli(
                &fixture.dir,
                &["residual", "try", &bad.to_string(), "--no-test"],
            );
            assert_eq!(code, 2, "{word}: {report}");
            assert_eq!(report["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT");
            assert!(
                report["detail"]
                    .as_str()
                    .unwrap()
                    .contains("/bindings/steps/0/1"),
                "{report}"
            );
            assert_eq!(bytes(&bad), original);
            bad["bindings"]["steps"][0][1] = json!([word]);
            assert!(
                check(&fixture, &bad, &json!({}))
                    .unwrap_err()
                    .detail()
                    .contains("operand count")
            );
        }
    }
    for row in opcodes::OPCODES
        .iter()
        .filter(|row| !(64..=71).contains(&row.tag))
    {
        for word in [
            row.mnemonic.to_owned(),
            row.name.to_owned(),
            row.tag.to_string(),
        ] {
            let bad = request(&word, &[]);
            assert!(
                check(&fixture, &bad, &json!({}))
                    .unwrap_err()
                    .detail()
                    .contains("unsupported pipeline arithmetic opcode")
            );
            assert!(
                sley_agent::residual::fragments::expand(
                    &sley_agent::residual::parse_request(&bytes(&bad)).unwrap()
                )
                .unwrap_err()
                .detail()
                .contains("unsupported arithmetic opcode")
            );
        }
    }
    for word in ["int_add_checked?", "64?Route", "future_operation"] {
        let bad = request(word, &[json!("x"), json!("y")]);
        assert!(check(&fixture, &bad, &json!({})).is_err());
    }
    assert_eq!(
        fixture.workspace.read_head().unwrap().transaction_id(),
        head
    );
    for artifact in ["drafts", "candidates", "residual"] {
        assert!(!fixture.dir.join(".sley").join(artifact).exists());
    }
}

#[test]
fn nested_pipeline_aliases_keep_guard_priority_rounding_and_explicit_error_mapping() {
    let fixture = Fixture::new();
    let declarations = json!({"af1":1,"types":[{"name":"Error",
        "variant":["ZeroInput","ZeroDivisor","Arithmetic"]}]});
    let (code, report) = cli(
        &fixture.dir,
        &["try", &declarations.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{report}");
    let mut request = guarded_request();
    request["base"] = json!("d1@r1");
    request["bindings"]["success"]["bindings"]["steps"] = json!([
        ["scaled", ["int_mul_checked", "x", 3]],
        ["value", ["67", "scaled", "divisor"]]
    ]);
    let original = bytes(&request);
    let preflight = check(&fixture, &request, &declarations).unwrap();
    assert_eq!(
        preflight["interfaces"][1]["fragment"]["id"],
        "checked_pipeline"
    );
    let (code, candidate) = cli(
        &fixture.dir,
        &["residual", "try", &request.to_string(), "--no-test"],
    );
    assert_eq!(code, 0, "{candidate}");
    assert_eq!(candidate["kernel"], "valid");
    let handle = candidate["handle"].as_str().unwrap();
    for (x, divisor, expected) in [
        (0, 0, json!({"Err":"ZeroInput"})),
        (100, 0, json!({"Err":"ZeroDivisor"})),
        (43, 3, json!({"Err":"Arithmetic"})),
        (-43, 3, json!({"Err":"Arithmetic"})),
        (5, 2, json!({"Ok":7})),
        (-5, 2, json!({"Ok":-7})),
        (5, -2, json!({"Ok":-7})),
        (-5, -2, json!({"Ok":7})),
    ] {
        let (code, report) = cli(
            &fixture.dir,
            &[
                "call",
                "checked",
                &x.to_string(),
                &divisor.to_string(),
                "--on",
                handle,
            ],
        );
        assert_eq!(code, 0, "{report}");
        assert_eq!(report["result"], expected, "{report}");
    }
    assert_eq!(bytes(&request), original);
}

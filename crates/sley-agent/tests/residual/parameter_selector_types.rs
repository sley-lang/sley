//! Numeric parameter selectors keep identity; operation selectors retain bounds.
use super::{Fixture, cli, expand};
use serde_json::{Value, json};

fn request(returns: &str, params: Value, ops: Value, term: Value) -> Value {
    let mut request = json!({"residual":1,"base":"current","operation":"derive",
        "fragment":{"id":"ordered_guard_chain","version":1},"scope":["checked"],
        "bindings":{"params":null,"returns":returns,"guards":[],"success":{"ops":null,"term":null}}});
    request["bindings"]["params"] = params;
    request["bindings"]["success"]["ops"] = ops;
    request["bindings"]["success"]["term"] = term;
    request
}

fn execute(request: &Value, residual: bool, cases: &[(&[&str], Value)]) {
    let fixture = Fixture::new();
    let frame = if residual {
        request.clone()
    } else {
        expand(request).frame
    };
    let text = frame.to_string();
    let args = if residual {
        vec!["residual", "try", &text, "--no-test"]
    } else {
        vec!["try", &text, "--no-test"]
    };
    let (code, trial) = cli(&fixture.dir, &args);
    assert_eq!(code, 0, "{trial}");
    let handle = trial["handle"].as_str().unwrap();
    for (input, expected) in cases {
        let mut args = vec!["call", "checked"];
        args.extend_from_slice(input);
        args.extend_from_slice(&["--on", handle]);
        let (code, value) = cli(&fixture.dir, &args);
        assert_eq!(code, 0, "{value}");
        assert_eq!(&value["result"], expected);
    }
}

#[test]
fn parameter_selectors_materialize_wrappers_and_preserve_execution() {
    for (returns, cases) in [
        (
            "Result<i8,ArithmeticError>",
            vec![
                (&["7"][..], json!({"Ok":7})),
                (&["-128"][..], json!({"Ok":-128})),
            ],
        ),
        (
            "Option<i8>",
            vec![
                (&["7"][..], json!({"Some":7})),
                (&["-128"][..], json!({"Some":-128})),
            ],
        ),
    ] {
        for suffix in ["#0", "#00", "#1", "#0007", "#+1", "#4294967295"] {
            let term = if returns.starts_with("Option") {
                json!(["return", ["some", format!("x{suffix}")]])
            } else {
                json!(["ok", format!("x{suffix}")])
            };
            let request = request(returns, json!([["x", "i8"]]), json!([]), term);
            for residual in [false, true] {
                execute(&request, residual, &cases);
            }
        }
    }
}

#[test]
fn branch_payload_join_and_checked_continuation_selectors_preserve_execution() {
    let make = |suffix: &str| {
        json!({"residual":1,"base":"current","operation":"derive",
        "fragment":{"id":"typed_branch_result","version":1},"scope":["checked"],
        "bindings":{"params":[["maybe","Option<i8>"]],"returns":"Result<i8,ArithmeticError>","input":"maybe",
        "cases":[{"case":"Some","payload":["item","i8"],"ops":[["out","add?",format!("item{suffix}"),1]],"values":[format!("out{suffix}")]},
          {"case":"None","payload":null,"ops":[["fallback","const",{"type":"i8","value":17}]],"values":["fallback#00"]}],
        "join":{"params":[["merged","i8"]],"ops":[],"term":["ok",format!("merged{suffix}")]}}})
    };
    let cases = [
        (&["{\"Some\":126}"][..], json!({"Ok":127})),
        (
            &["{\"Some\":127}"][..],
            json!({"Err":{"ArithmeticError":"Overflow"}}),
        ),
        (&["\"None\""][..], json!({"Ok":17})),
    ];
    for suffix in ["#0", "#00", "#7", "#4294967295"] {
        for residual in [false, true] {
            execute(&make(suffix), residual, &cases);
        }
    }
}

#[test]
fn zero_operation_selectors_keep_types_without_aliasing_nonzero_results() {
    let make = |suffix: &str| {
        request(
            "Result<i8,ArithmeticError>",
            json!([["value", "i8"]]),
            json!([["value","const",{"type":"i8","value":7}],["sum","add",format!("value{suffix}"),1]]),
            json!(["return", "sum"]),
        )
    };
    let cases = [(&["5"][..], json!({"Ok":8}))];
    for suffix in ["#0", "#00", "#000"] {
        for residual in [false, true] {
            execute(&make(suffix), residual, &cases);
        }
    }
    for suffix in ["#1", "#4294967295"] {
        let request = make(suffix);
        for residual in [false, true] {
            let fixture = Fixture::new();
            let frame = if residual {
                request.clone()
            } else {
                expand(&request).frame
            };
            let text = frame.to_string();
            let args = if residual {
                vec!["residual", "try", &text, "--no-test"]
            } else {
                vec!["try", &text, "--no-test"]
            };
            let (code, result) = cli(&fixture.dir, &args);
            assert_ne!(code, 0, "{result}");
            if residual {
                assert_eq!(
                    result["error"], "AGENT_RESIDUAL_CONSTRAINT_CONFLICT",
                    "{result}"
                );
                assert!(!fixture.dir.join(".sley/drafts").exists());
            }
        }
    }
}

#[test]
fn malformed_parameter_selectors_and_foreign_block_parameters_remain_refused() {
    for suffix in ["#", "#-1", "#4294967296", "#0#0", "#wat"] {
        let request = request(
            "Result<i8,ArithmeticError>",
            json!([["x", "i8"]]),
            json!([]),
            json!(["ok", format!("x{suffix}")]),
        );
        for residual in [false, true] {
            let fixture = Fixture::new();
            let frame = if residual {
                request.clone()
            } else {
                expand(&request).frame
            };
            let text = frame.to_string();
            let args = if residual {
                vec!["residual", "try", &text, "--no-test"]
            } else {
                vec!["try", &text, "--no-test"]
            };
            let (code, result) = cli(&fixture.dir, &args);
            assert_ne!(code, 0, "{result}");
        }
    }
    let fixture = Fixture::new();
    let frame = json!({"af1":1,"afx":1,"fns":[{"fn":"bad","params":[["x","i8"]],"returns":"Result<i8,ArithmeticError>","blocks":[
        {"name":"entry","ops":[],"term":["br","tail","x"]},
        {"name":"tail","params":[["value","i8"]],"ops":[],"term":["br","end"]},
        {"name":"end","ops":[["sum","add?","tail.value#7",1]],"term":["ok","sum"]}]}]});
    let (code, result) = cli(&fixture.dir, &["try", &frame.to_string(), "--no-test"]);
    assert_ne!(code, 0, "{result}");
    assert!(!result["verdict"]["valid"].as_bool().unwrap_or(false));
}

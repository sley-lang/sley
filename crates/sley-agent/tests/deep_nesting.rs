//! Deeply nested familiar text is refused with a diagnostic, never a stack
//! overflow: hostile shapes through the embed API (the path a host that
//! compiles untrusted programs takes) and through `familiar::parse`.
#![cfg(feature = "familiar")]

use std::fmt::Write as _;

use sley_agent::embed::{EmbedOptions, compile_python};
use sley_agent::familiar::{self, MAX_NESTING};

const OPTIONS: EmbedOptions = EmbedOptions {
    seed: 7,
    now_unix_millis: 1_760_000_000_000,
};

/// A main thread's stack. Unoptimized frames are several times larger than
/// release ones, so this is ample for a debug test build too.
const STACK: usize = 8 * 1024 * 1024;

fn on_main_sized_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(work)
        .unwrap()
        .join()
        .unwrap()
}

fn program(returns: &str, expression: &str) -> String {
    format!("def act(x: int) -> {returns}:\n    return {expression}\n")
}

fn wrapped(open: &str, close: &str, depth: usize) -> String {
    format!("{}x{}", open.repeat(depth), close.repeat(depth))
}

fn hostile() -> Vec<(&'static str, String, u32)> {
    let mut elif = String::new();
    for k in 1..=5_000 {
        writeln!(elif, "    elif x == {k}:\n        return {k}").unwrap();
    }
    let mut indented = String::new();
    for k in 0..5_000 {
        writeln!(indented, "{}if x > {k}:", "    ".repeat(k + 1)).unwrap();
    }
    vec![
        // The four shapes that aborted a release build on an 8 MiB stack.
        (
            "3,500 nested parentheses",
            program("int", &wrapped("(", ")", 3_500)),
            2,
        ),
        (
            "a 30,000-term sum",
            program("int", &format!("x{}", "+1".repeat(29_999))),
            2,
        ),
        (
            "60,000 unary minuses",
            program("int", &format!("{}x", "-".repeat(60_000))),
            2,
        ),
        (
            "15,000 nots",
            program("bool", &format!("{}x", "not ".repeat(15_000))),
            2,
        ),
        // Other ways to nest.
        (
            "5,000 nested calls",
            program("int", &wrapped("abs(", ")", 5_000)),
            2,
        ),
        (
            "30,000 max arguments",
            program("int", &format!("max(x{})", ", x".repeat(30_000))),
            2,
        ),
        (
            "30,000 subscripts",
            program("int", &format!("x{}", "[0]".repeat(30_000))),
            2,
        ),
        (
            "a 30,000-link comparison chain",
            program("bool", &format!("x{}", " < x".repeat(30_000))),
            2,
        ),
        (
            "a 30,000-term and chain",
            program("bool", &format!("x{}", " and x".repeat(30_000))),
            2,
        ),
        (
            "30,000 powers",
            program("int", &format!("x{}", " ** x".repeat(30_000))),
            2,
        ),
        (
            "5,000 elif arms",
            format!(
                "def act(x: int) -> int:\n    if x == 0:\n        return 0\n{elif}    return 1\n"
            ),
            0,
        ),
        (
            "5,000 indented blocks",
            format!(
                "def act(x: int) -> int:\n{indented}{}return x\n    return 0\n",
                "    ".repeat(5_001)
            ),
            0,
        ),
        (
            "a 30,000-deep list type",
            format!(
                "def act(x: {}int{}) -> int:\n    return 1\n",
                "list[".repeat(30_000),
                "]".repeat(30_000)
            ),
            1,
        ),
        (
            "a 30,000-deep test argument",
            format!(
                "def act(x: list[int]) -> int:\n    return 1\n\ndef test_act() -> None:\n    assert act({}1{}) == 1\n",
                "[".repeat(30_000),
                "]".repeat(30_000)
            ),
            5,
        ),
    ]
}

#[test]
fn hostile_nesting_is_refused_by_compile_python() {
    for (label, source, line) in hostile() {
        let error = on_main_sized_stack(move || compile_python(&source, &OPTIONS).err())
            .unwrap_or_else(|| panic!("{label}: compiled"));
        assert_eq!(error.code, "AGENT_FRAME_INVALID", "{label}: {error}");
        assert!(
            error.message.contains("nests more than"),
            "{label}: {error}"
        );
        if line > 0 {
            assert_eq!(error.line, Some(line), "{label}: {error}");
        }
    }
}

#[test]
fn hostile_nesting_is_refused_by_the_parser() {
    for (label, source, _) in hostile() {
        let error = on_main_sized_stack(move || familiar::parse(&source).err())
            .unwrap_or_else(|| panic!("{label}: parsed"));
        assert!(
            error.message.contains("nests more than"),
            "{label}: {}",
            error.message
        );
    }
}

/// Text just inside the limit takes the whole compile path (frame compiler,
/// kernel validation, lowering) without exhausting the stack.
#[test]
fn nesting_just_inside_the_limit_compiles() {
    let inside = MAX_NESTING - 8;
    for (label, source) in [
        ("parentheses", program("int", &wrapped("(", ")", inside))),
        (
            "a sum",
            program("int", &format!("x{}", "+1".repeat(inside))),
        ),
        (
            "unary minuses",
            program("int", &format!("{}x", "-".repeat(inside))),
        ),
        ("calls", program("int", &wrapped("abs(", ")", inside))),
        (
            "nots",
            program("bool", &format!("{}x", "not ".repeat(inside / 2))),
        ),
    ] {
        let result = on_main_sized_stack(move || compile_python(&source, &OPTIONS).err());
        assert_eq!(result, None, "{label}");
    }
}

/// Every frame the frontend accepts reads back through `serde_json`, as a
/// draft's `frame.json` is read by the next step: near the limit, text is
/// either refused or yields a frame that loads. Before the bound matched the
/// reader, a 124-term sum compiled and its draft then failed to load.
#[test]
fn accepted_frames_near_the_limit_read_back_as_json() {
    let elif = |arms: usize| {
        let mut text = String::from("def act(x: int) -> int:\n    if x == 0:\n        return 0\n");
        for k in 1..=arms {
            writeln!(text, "    elif x == {k}:\n        return {k}").unwrap();
        }
        text + "    return 1\n"
    };
    let argument = |depth: usize| {
        format!(
            "def act(x: list[int]) -> int:\n    return 1\n\ndef test_act() -> None:\n    assert act({}{}) == 1\n",
            "[".repeat(depth),
            "]".repeat(depth)
        )
    };
    let expected = |depth: usize| {
        format!(
            "def act(x: list[int]) -> list[int]:\n    return x\n\ndef test_act() -> None:\n    assert act([]) == {}{}\n",
            "[".repeat(depth),
            "]".repeat(depth)
        )
    };
    let near = MAX_NESTING - 6..MAX_NESTING + 6;
    let shapes: Vec<(&str, Vec<String>)> = vec![
        (
            "sums",
            near.clone()
                .map(|n| program("int", &format!("x{}", "+1".repeat(n))))
                .collect(),
        ),
        (
            "calls",
            near.clone()
                .map(|n| program("int", &wrapped("abs(", ")", n)))
                .collect(),
        ),
        (
            "and chains",
            near.clone()
                .map(|n| program("bool", &format!("x > 0{}", " and x > 1".repeat(n / 2))))
                .collect(),
        ),
        ("elif chains", near.clone().map(|n| elif(n / 2)).collect()),
        ("test arguments", near.clone().map(argument).collect()),
        ("expected values", near.clone().map(expected).collect()),
    ];
    for (label, texts) in shapes {
        let outcomes = on_main_sized_stack(move || {
            texts
                .iter()
                .map(|text| match familiar::parse(text) {
                    Ok(parsed) => {
                        let json = serde_json::to_string(&parsed.frame).unwrap();
                        Ok(serde_json::from_str::<serde_json::Value>(&json)
                            .map(drop)
                            .map_err(|error| error.to_string()))
                    }
                    Err(error) => Err(error.message),
                })
                .collect::<Vec<_>>()
        });
        assert!(
            outcomes.first().is_some_and(Result::is_ok),
            "{label}: {outcomes:?}"
        );
        assert!(
            outcomes.last().is_some_and(Result::is_err),
            "{label}: {outcomes:?}"
        );
        for outcome in outcomes {
            match outcome {
                Ok(reloaded) => assert_eq!(reloaded, Ok(()), "{label}"),
                Err(message) => assert!(message.contains("nests more than"), "{label}: {message}"),
            }
        }
    }
}

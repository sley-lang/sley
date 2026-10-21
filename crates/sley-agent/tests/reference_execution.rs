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

/// The compact plan's answers, fuel and instruction counts equal the
/// reference runner's on the runtime benchmark programs (Python dialect):
/// loops, nested loops, calls and recursion, at several sizes.
#[test]
fn benchmark_programs_match_reference_fuel_and_instructions() {
    let programs: [(&str, &str, &[&str]); 5] = [
        (
            "fib",
            "def fib(n: int) -> int:\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\n",
            &["0", "1", "2", "12"],
        ),
        (
            "loopsum",
            "def loopsum(n: int) -> int:\n    s = 0\n    for i in range(n):\n        s = (s + i * i) % 1000000007\n    return s\n",
            &["0", "1", "50", "3000"],
        ),
        (
            "collatz",
            "def collatz(limit: int) -> int:\n    best = 0\n    best_n = 0\n    for n in range(1, limit):\n        x = n\n        steps = 0\n        while x != 1:\n            if x % 2 == 0:\n                x = x // 2\n            else:\n                x = 3 * x + 1\n            steps += 1\n        if steps > best:\n            best = steps\n            best_n = n\n    return best_n\n",
            &["1", "2", "40"],
        ),
        (
            "primes",
            "def primes(n: int) -> int:\n    count = 0\n    for k in range(2, n):\n        d = 2\n        is_p = True\n        while d * d <= k:\n            if k % d == 0:\n                is_p = False\n                break\n            d += 1\n        if is_p:\n            count += 1\n    return count\n",
            &["2", "3", "200"],
        ),
        (
            "gcdsum",
            "def gcd(a: int, b: int) -> int:\n    while b != 0:\n        a, b = b, a % b\n    return a\n\n\ndef gcdsum(n: int) -> int:\n    s = 0\n    for i in range(1, n):\n        for j in range(1, n):\n            s += gcd(i, j)\n    return s\n",
            &["1", "2", "25"],
        ),
    ];
    for (name, text, inputs) in programs {
        let path = std::env::temp_dir().join(format!(
            "sley-reference-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let temp = Temp(path);
        fs::create_dir_all(&temp.0).unwrap();
        sley_agent::genesis::init(&temp.0, Some([83; 32]), sley_agent::genesis::INIT_CEILINGS)
            .unwrap();
        let source = temp.0.join(format!("{name}.py"));
        fs::write(&source, text).unwrap();
        let (code, proposal) = run(&temp.0, &["try", "--familiar", source.to_str().unwrap()]);
        assert_eq!(code, 0, "{proposal}");
        let handle = proposal["handle"].as_str().unwrap();
        for input in inputs {
            let (code, mut compact) = run(&temp.0, &["call", name, input, "--on", handle]);
            let (other, mut reference) = run(
                &temp.0,
                &["call", name, input, "--on", handle, "--reference"],
            );
            assert_eq!(code, other, "{name} {input}");
            compact.as_object_mut().unwrap().remove("vm_micros");
            reference.as_object_mut().unwrap().remove("vm_micros");
            assert_eq!(compact, reference, "{name} {input}");
            assert!(
                compact["fuel"].as_u64().unwrap() > 0,
                "{name} {input}: {compact}"
            );
        }
    }
}

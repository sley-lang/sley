//! The seeded random-program differential fuzzer (`sley_agent::vmfuzz`):
//! reference interpreter against the derived tiers, on results, fuel,
//! instructions, peak value units and observation digests, at random cuts
//! of every limit.
//!
//! - `cargo test --test vm_differential` checks a fixed seed range.
//! - `SLEY_VMFUZZ_SEEDS=1000..200000 cargo test --release --test
//!   vm_differential -- --ignored long_run` checks any range (`make vm-fuzz`).
//! - `SLEY_VMFUZZ_SEED=1234 cargo test --test vm_differential -- --ignored
//!   replay` replays one seed and prints its program.
//!
//! A disagreement is shrunk to a small program that still disagrees; with
//! `SLEY_VMFUZZ_SAVE=1` it is written to `tests/fixtures/vm_differential/`,
//! where `fixtures_agree` keeps it as a regression case.
#![cfg(feature = "familiar")]

use std::fmt::Write as _;
use std::ops::Range;
use std::path::PathBuf;

use sley_agent::embed::EmbedValue;
use sley_agent::vmfuzz::{self, Verdict};
use sley_vm::ExecutionLimits;

/// The seeds every `cargo test` checks.
const CI_SEEDS: Range<u64> = 0..24;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vm_differential")
}

/// What a sweep compared: pairs of runs, and programs whose entry ran on a
/// derived tier (the others compare the reference interpreter with itself).
struct Swept {
    runs: usize,
    derived: usize,
}

/// Checks `seeds`; returns the first finding, shrunk, as a report.
fn sweep(seeds: Range<u64>) -> Result<Swept, String> {
    let mut swept = Swept {
        runs: 0,
        derived: 0,
    };
    for seed in seeds {
        let program = vmfuzz::program(seed);
        let text = program.text();
        match vmfuzz::check(&text, &program.args(), seed) {
            Ok(Verdict::Agree { runs, derived }) => {
                swept.runs += runs;
                swept.derived += usize::from(derived);
            }
            Ok(Verdict::Disagree(finding)) => {
                let small = vmfuzz::shrink_finding(&program, &finding, 2_000);
                let mut report = format!(
                    "seed {seed}: the tiers disagree\n{finding}\n\nshrunk program:\n{}",
                    small.text()
                );
                if std::env::var_os("SLEY_VMFUZZ_SAVE").is_some() {
                    let path = fixtures().join(format!("seed-{seed}.py"));
                    let args = args_json(&finding.args);
                    let limits = limits_line(&finding.limits);
                    let saved = format!(
                        "# seed {seed}\n# args: {args}\n# limits: {limits}\n{}",
                        small.text()
                    );
                    std::fs::write(&path, saved).unwrap();
                    let _ = write!(report, "\nsaved {}", path.display());
                }
                return Err(report);
            }
            Err(error) => {
                return Err(format!(
                    "seed {seed}: the generated program does not compile: {error}\n{text}"
                ));
            }
        }
    }
    Ok(swept)
}

/// `fuel instructions value-units output-units cancel` (`-` for no cancel).
fn limits_line(limits: &ExecutionLimits) -> String {
    format!(
        "{} {} {} {} {}",
        limits.max_fuel,
        limits.max_instructions,
        limits.max_value_units,
        limits.max_output_units,
        limits
            .cancel_at_fuel
            .map_or_else(|| "-".to_owned(), |cut| cut.to_string())
    )
}

fn parse_limits(line: &str) -> ExecutionLimits {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let [fuel, instructions, units, output, cancel] = parts.as_slice() else {
        panic!("limits are `fuel instructions value-units output-units cancel`: {line}");
    };
    let number = |text: &str| text.parse::<u64>().expect("a limit");
    ExecutionLimits {
        max_fuel: number(fuel),
        max_instructions: number(instructions),
        max_value_units: number(units),
        max_output_units: number(output),
        cancel_at_fuel: (*cancel != "-").then(|| number(cancel)),
    }
}

fn args_json(args: &[EmbedValue]) -> String {
    let parts: Vec<String> = args
        .iter()
        .map(|arg| match arg {
            EmbedValue::Int(number) => number.to_string(),
            EmbedValue::IntList(items) => format!(
                "[{}]",
                items
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            other => panic!("not a generated argument: {other:?}"),
        })
        .collect();
    format!("[{}]", parts.join(", "))
}

/// `a, b, [xs]` from a fixture's `# args:` line.
fn parse_args(line: &str) -> Vec<EmbedValue> {
    let inner = line
        .trim()
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .expect("an args array");
    let (scalars, list) = inner.split_once('[').expect("a, b, [xs]");
    let mut out: Vec<EmbedValue> = scalars
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| EmbedValue::Int(part.parse().expect("an integer")))
        .collect();
    let items = list
        .trim_end_matches(']')
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().expect("an integer"))
        .collect();
    out.push(EmbedValue::IntList(items));
    out
}

fn range_from_env(name: &str, default: Range<u64>) -> Range<u64> {
    let Ok(text) = std::env::var(name) else {
        return default;
    };
    let (start, end) = text.split_once("..").expect("a range such as 0..1000");
    start.parse().expect("a start seed")..end.parse().expect("an end seed")
}

#[test]
fn a_fixed_seed_range_agrees() {
    match sweep(CI_SEEDS) {
        Ok(swept) => {
            assert!(
                swept.runs > CI_SEEDS.count() * 20,
                "only {} runs",
                swept.runs
            );
            // Most programs must compare two tiers, not the reference with
            // itself.
            assert!(
                swept.derived * 2 > CI_SEEDS.count(),
                "only {} of {} programs ran on a derived tier",
                swept.derived,
                CI_SEEDS.count()
            );
        }
        Err(report) => panic!("{report}"),
    }
}

#[test]
fn fixtures_agree() {
    let Ok(entries) = std::fs::read_dir(fixtures()) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("py") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let args = text
            .lines()
            .find_map(|line| line.strip_prefix("# args: "))
            .map_or_else(
                || panic!("{}: no `# args:` line", path.display()),
                parse_args,
            );
        // The finding's own limits, as well as fresh cuts.
        let limits: Vec<ExecutionLimits> = text
            .lines()
            .filter_map(|line| line.strip_prefix("# limits: "))
            .map(parse_limits)
            .collect();
        match vmfuzz::check_with(&text, &[args], 0, &limits) {
            Ok(Verdict::Agree { .. }) => {}
            Ok(Verdict::Disagree(finding)) => panic!("{}: {finding}", path.display()),
            Err(error) => panic!("{}: {error}", path.display()),
        }
    }
}

#[test]
fn generated_programs_reach_every_construct() {
    // Generating is cheap: scan more seeds than the checked range, since
    // records and list locals (which keep a program off the derived tiers)
    // are deliberately rare.
    let mut text = String::new();
    for seed in 0..200 {
        text.push_str(&vmfuzz::program(seed).text());
    }
    for construct in [
        "for ", "while ", "break", "continue", " if ", "else:", ".append(", "] = ", " = P(", ".a",
        "sum(", "any(", "len(", " // ", " % ", " << ", " >> ", " & ", " ** ", "abs(", "min(",
        "max(", " and ", " or ", "not ", "return r", "(n - 1, ",
    ] {
        assert!(text.contains(construct), "no seed generates `{construct}`");
    }
}

#[test]
#[ignore = "a long run: SLEY_VMFUZZ_SEEDS=a..b (make vm-fuzz)"]
fn long_run() {
    let seeds = range_from_env("SLEY_VMFUZZ_SEEDS", 1_000..11_000);
    let count = seeds.end.saturating_sub(seeds.start);
    match sweep(seeds) {
        Ok(swept) => eprintln!(
            "{count} programs ({} on a derived tier), {} pairs of runs agreed",
            swept.derived, swept.runs
        ),
        Err(report) => panic!("{report}"),
    }
}

#[test]
#[ignore = "replays SLEY_VMFUZZ_SEED"]
fn replay() {
    let seed: u64 = std::env::var("SLEY_VMFUZZ_SEED")
        .expect("SLEY_VMFUZZ_SEED")
        .parse()
        .expect("a seed");
    let program = vmfuzz::program(seed);
    eprintln!("{}", program.text());
    if let Err(report) = sweep(seed..seed + 1) {
        panic!("{report}");
    }
}

#[test]
fn fixture_args_round_trip() {
    let args = vec![
        EmbedValue::Int(-3),
        EmbedValue::Int(i64::MAX),
        EmbedValue::IntList(vec![1, -2, 3]),
    ];
    assert_eq!(parse_args(&args_json(&args)), args);
    let limits = ExecutionLimits {
        cancel_at_fuel: Some(17),
        ..vmfuzz::LIMITS
    };
    assert_eq!(parse_limits(&limits_line(&limits)), limits);
    assert_eq!(parse_limits(&limits_line(&vmfuzz::LIMITS)), vmfuzz::LIMITS);
    assert_eq!(
        parse_args("[0, 1, []]"),
        vec![
            EmbedValue::Int(0),
            EmbedValue::Int(1),
            EmbedValue::IntList(vec![])
        ]
    );
}

//! Development timing of the public executor after loading and typed decoding.
//! Usage: `needleshift_sustained WORKSPACE FUNCTION INPUT_JSON REPEATS [--prepared]`
//! Emits every answer and elapsed nanoseconds; callers verify independent oracles.

use std::error::Error;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};
use sley_agent::candidate::{self, Authority, Store};
use sley_agent::exec::{Executor, call_limits};
use sley_agent::names::{NameMap, Names};
use sley_agent::values::{self, ProgramTypes};
use sley_agent::workspace::{NAMES_FILE, STATE_DIR, Workspace};
use sley_ssmc::{ConstData, ConstValue};
use sley_vm::ExecutionTermination;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(args.len() == 4 || (args.len() == 5 && args[4] == "--prepared")) {
        return Err("expected WORKSPACE FUNCTION INPUT_JSON REPEATS [--prepared]".into());
    }
    let repeats: usize = args[3].parse()?;
    if repeats == 0 {
        return Err("REPEATS must be positive".into());
    }
    let workspace = Workspace::at(&args[0]);
    let head = workspace.head()?;
    let store = Store::open(&workspace)?;
    let stored = store.load(&store.resolve(Some("c1"))?)?;
    let validation = candidate::validate(&head, &Authority::of(&head)?, &stored)?;
    if !validation.is_valid() {
        return Err("candidate is invalid".into());
    }
    let program = candidate::proposed_program(&head, &validation)
        .ok_or("candidate has no proposed program")?;
    let mut map = NameMap::read(&workspace.dir().join(NAMES_FILE))?;
    map.extend(&NameMap::read(
        &workspace.dir().join(STATE_DIR).join(NAMES_FILE),
    )?);
    let names = Names::build(&program, &map);
    let function = names.resolve(&args[1]).ok_or("unknown function")?;
    let mut executor = Executor::new(&program)?;
    executor.prepare(&function)?;
    let signature = executor.parameter_types(&function);
    let defs = ProgramTypes {
        program: &program,
        names: &names,
    };
    let rows: Vec<Vec<Value>> =
        serde_json::from_str(&std::fs::read_to_string(Path::new(&args[2]))?)?;
    let typed: Vec<Vec<ConstValue>> = rows
        .iter()
        .map(|row| {
            if row.len() != signature.len() {
                return Err("wrong argument count".into());
            }
            row.iter()
                .zip(&signature)
                .map(|(value, ty)| values::read(value, ty, &defs, "input").map_err(Into::into))
                .collect::<Result<_, Box<dyn Error>>>()
        })
        .collect::<Result<_, _>>()?;
    if typed.is_empty() {
        return Err("input must contain at least one row".into());
    }
    let result = measure(&mut executor, &function, &typed, repeats, args.len() == 5)?;
    println!(
        "{}",
        json!({"kind":"public-executor-development-probe",
        "function":args[1], "rows":typed.len(), "warmups":3,
        "allocator":"Rust default system allocator",
        "nanoseconds":result.samples, "answers":result.answers,
        "first_run_ns":result.first_run_ns,
        "preparation_ns":result.preparation_ns, "mode":result.mode})
    );
    Ok(())
}

struct Measurement {
    samples: Vec<u128>,
    answers: Vec<i64>,
    preparation_ns: u128,
    mode: &'static str,
    first_run_ns: u128,
}

fn integer(outcome: sley_agent::exec::Outcome) -> Result<i64, Box<dyn Error>> {
    let ExecutionTermination::Success(value) = outcome.termination else {
        return Err("execution did not succeed".into());
    };
    let ConstData::SInt(value) = value.data else {
        return Err("expected signed integer result".into());
    };
    Ok(i64::try_from(black_box(value))?)
}

fn collect_samples(
    repeats: usize,
    mut batch: impl FnMut() -> Result<(Vec<i64>, u128), Box<dyn Error>>,
) -> Result<Measurement, Box<dyn Error>> {
    let mut samples = Vec::new();
    let mut expected = None;
    let mut first_run_ns = 0;
    for repetition in 0..(repeats + 3) {
        let (answers, elapsed) = batch()?;
        if repetition == 0 {
            first_run_ns = elapsed;
        }
        if let Some(previous) = &expected {
            if previous != &answers {
                return Err("repetitions returned different answers".into());
            }
        } else {
            expected = Some(answers);
        }
        if repetition >= 3 {
            samples.push(elapsed);
        }
    }
    Ok(Measurement {
        samples,
        answers: expected.unwrap(),
        preparation_ns: 0,
        mode: "admit-each-call",
        first_run_ns,
    })
}

fn measure(
    executor: &mut Executor,
    function: &sley_id::EntityId,
    typed: &[Vec<ConstValue>],
    repeats: usize,
    prepared: bool,
) -> Result<Measurement, Box<dyn Error>> {
    if prepared {
        let started = Instant::now();
        let calls = typed
            .iter()
            .map(|inputs| executor.prepare_call(function, inputs.clone(), call_limits()))
            .collect::<Result<Vec<_>, _>>()?;
        let preparation_ns = started.elapsed().as_nanos();
        let mut result = collect_samples(repeats, || {
            let mut answers = Vec::with_capacity(calls.len());
            let started = Instant::now();
            for call in black_box(&calls) {
                answers.push(integer(call.run()?)?);
            }
            Ok((answers, started.elapsed().as_nanos()))
        })?;
        result.preparation_ns = preparation_ns;
        result.mode = "prepared-fixed-inputs";
        Ok(result)
    } else {
        collect_samples(repeats, || {
            // Ordinary API consumes inputs: copies precede the timer. Admission,
            // execution, observations and consumed-input destruction are timed.
            let inputs = black_box(typed.to_vec());
            let mut answers = Vec::with_capacity(inputs.len());
            let started = Instant::now();
            for input in inputs {
                answers.push(integer(executor.run(function, input, call_limits())?)?);
            }
            Ok((answers, started.elapsed().as_nanos()))
        })
    }
}

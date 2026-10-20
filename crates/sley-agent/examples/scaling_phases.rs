//! Source-level scaling probe; see `docs/bench/SCALING-PHASES.md`.
//! Timings are advisory measurements, never admission evidence.

use std::collections::BTreeMap;
use std::error::Error;
use std::hint::black_box;
use std::time::Instant;

use serde_json::{Value, json};
use sley_agent::candidate::{self, Authority, Store};
use sley_agent::exec::{Executor, call_limits};
use sley_agent::names::{NameMap, Names};
use sley_agent::values::{self, ProgramTypes};
use sley_agent::workspace::{NAMES_FILE, STATE_DIR, Workspace};
use sley_ssmc::ConstData;
use sley_vm::ExecutionTermination;

// The example calls the public API only and uses the system allocator:
// ADR-0052 confines the size-class cache to the binary and the workbench tests.

fn timed<T>(f: impl FnOnce() -> T) -> (T, u128) {
    let start = Instant::now();
    let result = f();
    (result, start.elapsed().as_nanos())
}

fn memory() -> Value {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return Value::Null;
    };
    let mut result = serde_json::Map::new();
    for (key, label) in [("VmRSS:", "rss_kib"), ("VmHWM:", "high_water_kib")] {
        if let Some(line) = status.lines().find(|line| line.starts_with(key))
            && let Some(value) = line
                .split_whitespace()
                .nth(1)
                .and_then(|x| x.parse::<u64>().ok())
        {
            result.insert(label.into(), value.into());
        }
    }
    result.into()
}

fn check(outcome: sley_agent::exec::Outcome, expected: i128) -> Result<(), Box<dyn Error>> {
    let ExecutionTermination::Success(value) = outcome.termination else {
        return Err("target failed".into());
    };
    if value.data != ConstData::SInt(expected) {
        return Err("independent result oracle failed".into());
    }
    black_box(value);
    Ok(())
}

// Keep timed phases visibly ordered in one place so boundaries can be audited.
#[allow(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn Error>> {
    let process_body = Instant::now();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--empty"] {
        println!("{{\"kind\":\"empty-process-baseline\"}}");
        return Ok(());
    }
    if args.len() != 2 {
        return Err("usage: scaling_phases WORKSPACE CANDIDATE | --empty".into());
    }
    let workspace = Workspace::at(&args[0]);
    let mut phases = BTreeMap::new();
    let mut resident = BTreeMap::new();
    resident.insert("entry", memory());

    let (head, ns) = timed(|| workspace.read_head());
    let head = head?;
    phases.insert("accepted_head_load", ns);
    resident.insert("accepted_head_load", memory());
    let (program, ns) = timed(|| head.program());
    phases.insert("accepted_program_index", ns);
    resident.insert("accepted_program_index", memory());
    let (map, ns) = timed(|| -> Result<_, Box<dyn Error>> {
        let mut map = NameMap::read(&workspace.dir().join(NAMES_FILE))?;
        map.extend(&NameMap::read(
            &workspace.dir().join(STATE_DIR).join(NAMES_FILE),
        )?);
        Ok(map)
    });
    let map = map?;
    phases.insert("name_map_load", ns);
    let (names, ns) = timed(|| Names::build(program, &map));
    phases.insert("name_index", ns);
    let function = names.resolve("target").ok_or("missing target")?;
    let (executor, ns) = timed(|| Executor::new(program));
    let mut executor = executor?;
    phases.insert("executor_projection_type_and_function_index", ns);
    resident.insert("executor_projection_type_and_function_index", memory());
    let (prepared, ns) = timed(|| executor.prepare(&function));
    prepared?;
    phases.insert("first_lower_and_verify", ns);
    resident.insert("first_lower_and_verify", memory());
    let (signature, ns) = timed(|| executor.parameter_types(&function));
    phases.insert("parameter_type_lookup", ns);
    if signature.len() != 1 {
        return Err("target must have one parameter".into());
    }
    let defs = ProgramTypes {
        program,
        names: &names,
    };
    let (input, ns) = timed(|| values::read(&json!(42), &signature[0], &defs, "input"));
    let input = input?;
    phases.insert("typed_input_decode", ns);
    let first_input = vec![input.clone()];
    let (first, ns) = timed(|| executor.run(&function, first_input, call_limits()));
    check(first?, 42)?;
    phases.insert("first_call_after_explicit_lowering", ns);

    // Input cloning, oracle checks and RSS reads are outside the call timers.
    // Each call still performs admission; only the lowered image is cached.
    let mut repeated = Vec::new();
    for _ in 0..100 {
        let inputs = vec![input.clone()];
        let (outcome, ns) = timed(|| executor.run(&function, inputs, call_limits()));
        check(outcome?, 42)?;
        repeated.push(ns);
    }
    resident.insert("repeated_calls", memory());

    let (loaded, ns) = timed(|| -> Result<_, Box<dyn Error>> {
        let authority = Authority::of(&head)?;
        let store = Store::open(&workspace)?;
        let stored = store.load(&store.resolve(Some(&args[1]))?)?;
        Ok((authority, stored))
    });
    let (authority, stored) = loaded?;
    phases.insert("candidate_and_authority_load", ns);
    let (validation, ns) = timed(|| candidate::validate(&head, &authority, &stored));
    let validation = validation?;
    if !validation.is_valid() {
        return Err("candidate validation refused".into());
    }
    phases.insert("candidate_validation", ns);
    resident.insert("candidate_validation", memory());
    let (proposed, ns) = timed(|| candidate::proposed_program(&head, &validation));
    let proposed = proposed.ok_or("valid candidate has no proposed program")?;
    phases.insert("proposed_program_index", ns);
    resident.insert("proposed_program_index", memory());

    // Fixture verification happens after every measured phase and RSS snapshot.
    let checks_started = Instant::now();
    let count_functions = |p: &sley_agent::workspace::Program| {
        p.objects()
            .iter()
            .filter(|object| {
                matches!(
                    object.record().body,
                    sley_mutate::value::EntityBodyValue::Function(_)
                )
            })
            .count()
    };
    let accepted_functions = count_functions(program);
    if count_functions(&proposed) != accepted_functions {
        return Err("candidate changed function count".into());
    }
    let target = executor
        .function(&function)
        .ok_or("missing accepted function")?;
    let permitted_replacements: std::collections::BTreeSet<_> = std::iter::once(function)
        .chain(target.parameters.iter().copied())
        .chain(target.blocks.iter().copied())
        .collect();
    for object in program.objects() {
        let id = object.record().entity_id;
        if !permitted_replacements.contains(&id) && proposed.object(&id) != Some(object) {
            return Err("candidate changed an object outside target's identity body".into());
        }
    }
    let proposed_names = Names::build(&proposed, &map);
    if proposed_names.resolve("target") != Some(function) {
        return Err("candidate changed target identity".into());
    }
    let mut proposed_executor = Executor::new(&proposed)?;
    check(
        proposed_executor.run(&function, vec![input], call_limits())?,
        7,
    )?;
    let after = workspace.read_head()?;
    if after.transaction_id() != head.transaction_id() || after.state_root() != head.state_root() {
        return Err("accepted head changed during probe".into());
    }
    let imported = sley_mutate::import_candidate(&stored)?;
    let fixture_checks_ns = checks_started.elapsed().as_nanos();
    println!(
        "{}",
        json!({
            "kind":"sley.source-scaling-phases.v1",
            "allocator":"CLI SizeClassCache over system allocator",
            "accepted_objects":program.objects().len(),
            "candidate_bytes":stored.len(),
            "proposed_objects":proposed.objects().len(),
            "phase_ns":phases,
            "repeated_call_ns":repeated,
            "memory":resident,
            "oracle":{"function":"target", "input":42, "expected":42, "passed_calls":101},
            "candidate_valid":true,
        "fixture_checks":{
            "accepted_functions":accepted_functions,
            "proposed_functions":count_functions(&proposed),
            "target_id":sley_agent::hex::encode(function.as_bytes()),
            "accepted_root":sley_agent::hex::encode(program.root().as_bytes()),
            "target_identity_preserved":true,
            "other_accepted_objects_unchanged":true,
            "proposed_target_at_42":7,
            "accepted_head_unchanged":true,
            "candidate_operations":imported.record.operations.len(),
            "elapsed_ns":fixture_checks_ns
        },
            "process_body_ns":process_body.elapsed().as_nanos()
        })
    );
    Ok(())
}

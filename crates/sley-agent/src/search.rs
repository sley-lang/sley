//! `sley-agent search`: bounded, verified local search for small repairs of
//! one function.
//!
//! Six typed generators propose neighbors of a function as a seed program
//! states it (the head, a Valid candidate made from a frame, or a draft
//! revision in state `valid`): opcode swap, operand permutation, operand
//! substitution, constant nudge, edge swap and negation. Generation is local
//! and deterministic.
//! Each neighbor is written as an ordinary `edit` or `patch` frame that
//! `try --on <seed>` layers on the seed's frame, and search compiles exactly
//! that layered frame through the normal path: frame compilation, record
//! assembly and the kernel's candidate validation. Only kernel-Valid
//! neighbors run the public cases the author passed, and the neighbors are
//! ranked by a declared rule.
//!
//! "Verified" means kernel-valid and evaluated against the stated public
//! cases, not proof of correctness for all inputs. Search stores, submits
//! and commits nothing (its only write is the per-seed use record), and an
//! expected value always comes from the case file, never from a candidate.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use sley_id::EntityId;
use sley_mutate::value::{EntityBodyValue, OperationBody};
use sley_ssmc::{
    BuiltinCase, CaseKey, ConstData, ConstValue, Immediate, OperationResultRef, Reachability,
    SwitchArgument, TargetEdge, Terminator, TrapCode, TypeDefForm, TypeExpr, ValueRef,
};
use sley_vm::ExecutionTermination;

use crate::afx::{self, Role};
use crate::candidate::{self, Authority, Store};
use crate::catalog::Verdict;
use crate::cli::{EXIT_NEGATIVE, EXIT_OK};
use crate::draft::{self, DraftRef, Drafts, State};
use crate::error::{AgentError, AgentErrorCode, Result, io};
use crate::exec::{self, Executor};
use crate::names::{NameMap, Names, is_identifier};
use crate::opcodes::{self, ImmediateKind};
use crate::workspace::{Head, Program, STATE_DIR, Workspace};

/// Neighbors one command generates at most (`--max-neighbors` overrides).
pub const MAX_NEIGHBORS: usize = 64;
/// Wall-clock milliseconds one command may use (`--max-millis` overrides).
pub const MAX_MILLIS: u64 = 10_000;
/// The most `--max-neighbors` may ask for.
pub const NEIGHBOR_CEILING: usize = 4096;
/// The most `--max-millis` may ask for (one hour).
pub const MILLIS_CEILING: u64 = 3_600_000;
/// Search commands per seed lineage.
pub const SEARCHES_PER_SEED: u64 = 2;
/// Fuel a seed's public case may use.
pub const SEED_FUEL: u64 = 10_000_000;
/// Fuel a neighbor's public case may always use (up to [`SEED_FUEL`]).
pub const NEIGHBOR_FUEL_FLOOR: u64 = 1_000_000;
/// A neighbor's case may use this many times the seed's fuel on it.
pub const NEIGHBOR_FUEL_FACTOR: u64 = 10;
/// The use record in the workbench state directory.
pub const SEARCH_FILE: &str = "search.json";
/// Ranked neighbors the text output lists.
pub const SHOWN: usize = 5;
/// The ranking rule, stated in every output.
pub const RULE: &str =
    "public cases passed (desc), then edit size (asc), then generator order, then generation index";
/// What a search result means, stated in every output.
pub const CLAIM: &str = "kernel-valid and evaluated against the stated public cases, not proof of correctness for all inputs";
/// The command line.
pub const USAGE: &str = "search <fn> --public <cases.json> [--from <candidate | draft>] [--max-neighbors <n>] [--max-millis <ms>]";

/// Same-signature opcode families (opcode swap).
const FAMILIES: [&[u32]; 6] = [
    &[64, 65, 66, 67, 68],
    &[70, 71],
    &[80, 81, 82, 83],
    &[96, 97],
    &[98, 99, 100, 101],
    &[103, 104],
];
/// Binary opcodes whose operands do not commute (operand permutation).
const NON_COMMUTATIVE: [u32; 11] = [65, 67, 68, 70, 71, 81, 83, 98, 99, 100, 101];
/// `const`.
const CONST: u32 = 1;
/// `not`.
const NOT: u32 = 102;

/// One of the six neighbor generators, in generator order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Generator {
    /// An opcode replaced by another of its same-signature family.
    OpcodeSwap,
    /// The two operands of a non-commutative binary operation exchanged.
    OperandPermutation,
    /// An operand, a returned value or a `br` argument replaced by another
    /// visible value of the same type.
    OperandSubstitution,
    /// An integer constant plus one, minus one, or negated, as a new literal.
    ConstantNudge,
    /// The targets of a `cond`, or of two `switch` cases of the same arity
    /// and types, exchanged.
    EdgeSwap,
    /// A `cond` condition `c` made `not c`, or `not x` made `x`.
    Negation,
}

impl Generator {
    /// Every generator, in generator order.
    pub const ALL: [Self; 6] = [
        Self::OpcodeSwap,
        Self::OperandPermutation,
        Self::OperandSubstitution,
        Self::ConstantNudge,
        Self::EdgeSwap,
        Self::Negation,
    ];

    /// The generator's name in outputs.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::OpcodeSwap => "opcode swap",
            Self::OperandPermutation => "operand permutation",
            Self::OperandSubstitution => "operand substitution",
            Self::ConstantNudge => "constant nudge",
            Self::EdgeSwap => "edge swap",
            Self::Negation => "negation",
        }
    }

    /// Edit size: the items of an operation or terminator a neighbor of
    /// this generator changes.
    #[must_use]
    pub const fn size(self) -> u64 {
        match self {
            Self::OperandPermutation | Self::EdgeSwap => 2,
            _ => 1,
        }
    }
}

/// The generation sequence: generator by generator (operand substitution,
/// which proposes the most neighbors, last), each in program order.
const PHASES: [Generator; 6] = [
    Generator::OpcodeSwap,
    Generator::OperandPermutation,
    Generator::ConstantNudge,
    Generator::EdgeSwap,
    Generator::Negation,
    Generator::OperandSubstitution,
];

/// One search command.
#[derive(Clone, Debug)]
pub struct Request<'a> {
    /// The function searched.
    pub function: &'a str,
    /// The public case file, as given.
    pub public: &'a str,
    /// The seed: a candidate handle or a draft reference; the head when absent.
    pub from: Option<&'a str>,
    /// Neighbors generated at most.
    pub max_neighbors: usize,
    /// Wall-clock milliseconds at most.
    pub max_millis: u64,
}

/// What a search printed and what the events ledger records of it.
#[derive(Clone, Debug)]
pub struct Report {
    /// The text output.
    pub text: String,
    /// The JSON output.
    pub json: Value,
    /// Exit status: 0 when the top-ranked neighbor passes every public
    /// case, 1 otherwise.
    pub exit: i32,
    /// Counters for the events ledger.
    pub stats: Map<String, Value>,
    /// The seed's draft revision, when it is one or made the candidate.
    pub draft: Option<String>,
    /// The seed's candidate handle, when there is one.
    pub candidate: Option<String>,
    /// Bytes of the case file.
    pub input_bytes: usize,
}

/// Reads `--max-neighbors` and `--max-millis`.
///
/// # Errors
///
/// `AGENT_USAGE_INVALID` when a value is not a number in range.
pub fn limits(neighbors: Option<&str>, millis: Option<&str>) -> Result<(usize, u64)> {
    let neighbors = match neighbors {
        None => MAX_NEIGHBORS,
        Some(text) => text
            .parse::<usize>()
            .ok()
            .filter(|n| *n <= NEIGHBOR_CEILING)
            .ok_or_else(|| {
                crate::error::usage(format!("--max-neighbors takes 0 to {NEIGHBOR_CEILING}"))
            })?,
    };
    let millis = match millis {
        None => MAX_MILLIS,
        Some(text) => text
            .parse::<u64>()
            .ok()
            .filter(|ms| *ms <= MILLIS_CEILING)
            .ok_or_else(|| {
                crate::error::usage(format!(
                    "--max-millis takes 0 to {MILLIS_CEILING} milliseconds"
                ))
            })?,
    };
    Ok((neighbors, millis))
}

fn seed_invalid(detail: impl Into<String>) -> AgentError {
    AgentError::new(AgentErrorCode::SearchSeedInvalid, detail)
}

fn no_oracle(detail: impl Into<String>) -> AgentError {
    AgentError::new(AgentErrorCode::SearchNoOracle, detail)
}

/// Runs one search.
///
/// # Errors
///
/// `AGENT_SEARCH_NO_ORACLE` without a readable public case for the
/// function; `AGENT_SEARCH_SEED_INVALID` for an unusable seed, a name that
/// is not a function of the seed, or a seed lineage that has used its
/// searches; `AGENT_HANDLE_UNKNOWN` for a reference that does not exist.
#[allow(clippy::too_many_lines)]
pub fn run(workspace: &Workspace, request: &Request<'_>) -> Result<Report> {
    let started = Instant::now();
    let cpu_before = cpu_nanos();
    let deadline = started + Duration::from_millis(request.max_millis);
    let file = CaseFile::read(Path::new(request.public))?;
    let head = workspace.head()?;
    let map = crate::cli::name_map(workspace)?;
    let head_names = Names::build(head.program(), &map);
    let authority = Authority::of(&head)?;
    let seed = Seed::resolve(workspace, &head, &authority, &map, request.from)?;
    let function = seed.names.resolve(request.function).ok_or_else(|| {
        seed_invalid(format!(
            "no function named `{}` in {}",
            request.function, seed.label
        ))
    })?;
    let Some(EntityBodyValue::Function(body)) = seed.program.body(&function) else {
        return Err(seed_invalid(format!(
            "`{}` is not a function in {}",
            request.function, seed.label
        )));
    };
    let function_name = seed.names.name(&function);
    let for_function = file
        .cases
        .iter()
        .filter(|case| seed.names.resolve(&case.function) == Some(function))
        .count();
    if for_function == 0 {
        return Err(no_oracle(format!(
            "{} has no public case for `{function_name}`: search evaluates neighbors only against the public cases you pass for the function, never against expectations taken from a candidate",
            request.public
        )));
    }
    record_use(workspace, &seed)?;

    let model = Model::build(&seed.program, &seed.names, body);
    let place = Place::of(seed.frame.as_ref(), &function_name);
    let statement = match place {
        Place::Defined | Place::Patched => seed
            .frame
            .as_ref()
            .and_then(|frame| Stated::of(frame, &head, &head_names, &function_name)),
        _ => None,
    };
    let writer = Writer {
        model: &model,
        head: head.program(),
        function: function_name.clone(),
        place,
        stated: statement,
        afx: seed
            .frame
            .as_ref()
            .is_some_and(|frame| frame.get("afx").and_then(Value::as_u64) == Some(1)),
    };

    // The seed's own results: the baseline, and each case's fuel for the
    // neighbors' caps.
    let seed_caps = vec![SEED_FUEL; file.cases.len()];
    let seed_run = evaluate(
        &seed.program,
        &seed.names,
        &file.cases,
        &seed_caps,
        deadline,
    );
    let caps: Vec<u64> = (0..file.cases.len())
        .map(
            |index| match seed_run.outcomes.iter().find(|run| run.case == index) {
                Some(run) if !matches!(run.outcome, Outcome::Unknown(_) | Outcome::Limit(_)) => run
                    .fuel
                    .saturating_mul(NEIGHBOR_FUEL_FACTOR)
                    .clamp(NEIGHBOR_FUEL_FLOOR, SEED_FUEL),
                _ => NEIGHBOR_FUEL_FLOOR,
            },
        )
        .collect();

    let mut generated = generate(&model, &writer, request.max_neighbors, deadline);
    let context = Context {
        head: &head,
        head_names: &head_names,
        authority: &authority,
        map: &map,
        seed_frame: seed.frame.as_ref(),
        cases: &file.cases,
        caps: &caps,
        deadline,
    };
    let mut wall_reached = generated.wall;
    for neighbor in &mut generated.neighbors {
        if Instant::now() >= deadline {
            wall_reached = true;
            neighbor.status = Status::NotEvaluated;
            continue;
        }
        context.try_neighbor(neighbor)?;
        if matches!(neighbor.status, Status::Partial | Status::NotEvaluated) {
            wall_reached = true;
        }
    }
    if !seed_run.complete {
        wall_reached = true;
    }

    let mut ranked: Vec<usize> = generated
        .neighbors
        .iter()
        .enumerate()
        .filter(|(_, neighbor)| matches!(neighbor.status, Status::Evaluated | Status::Partial))
        .map(|(position, _)| position)
        .collect();
    ranked.sort_by_key(|position| {
        let neighbor = &generated.neighbors[*position];
        (
            std::cmp::Reverse(neighbor.passed()),
            neighbor.generator.size(),
            neighbor.generator,
            neighbor.index,
        )
    });
    for (rank, position) in ranked.iter().enumerate() {
        generated.neighbors[*position].rank = Some(rank + 1);
    }

    let usable = file.cases.len();
    let exit = match ranked
        .first()
        .map(|position| &generated.neighbors[*position])
    {
        Some(top) if top.status == Status::Evaluated && top.passed() == usable => EXIT_OK,
        _ => EXIT_NEGATIVE,
    };
    let counts = Counts::of(&generated);
    let resources = Resources::measure(started, cpu_before);
    let next = next_step(&seed, request.public, &seed_run, &generated, &ranked);
    let summary = Summary {
        request,
        function: &function_name,
        seed: &seed,
        file: &file,
        for_function,
        seed_run: &seed_run,
        generated: &generated,
        ranked: &ranked,
        counts: &counts,
        wall_reached,
        resources: &resources,
        next: &next,
    };
    let mut stats = Map::new();
    stats.insert("search_neighbors".to_owned(), json!(counts.generated));
    stats.insert("search_valid".to_owned(), json!(counts.valid));
    stats.insert("search_evaluated".to_owned(), json!(counts.evaluated));
    stats.insert(
        "search_exhausted".to_owned(),
        json!(wall_reached || generated.more),
    );
    Ok(Report {
        text: summary.text(),
        json: summary.json(),
        exit,
        stats,
        draft: seed.draft.clone(),
        candidate: seed.candidate.clone(),
        input_bytes: file.bytes,
    })
}

// ---------------------------------------------------------------------------
// Public cases

/// One usable public case.
#[derive(Clone, Debug)]
struct Case {
    name: String,
    function: String,
    args: Vec<Value>,
    expect: Value,
}

/// The public case file.
#[derive(Clone, Debug)]
struct CaseFile {
    sha256: String,
    bytes: usize,
    cases: Vec<Case>,
    /// Entries that are not a usable case (not an object, no `function`, no
    /// `expect`, or `args` not an array); never run.
    unusable: usize,
}

impl CaseFile {
    /// Reads `[{"name", "function", "args", "expect"}]`.
    fn read(path: &Path) -> Result<Self> {
        const SHAPE: &str =
            "public cases are a JSON array of {\"name\", \"function\", \"args\", \"expect\"}";
        let bytes = fs::read(path)
            .map_err(|error| no_oracle(format!("{}: {error}; {SHAPE}", path.display())))?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
            no_oracle(format!("{} is not JSON ({error}); {SHAPE}", path.display()))
        })?;
        let entries = value
            .as_array()
            .ok_or_else(|| no_oracle(format!("{}: {SHAPE}", path.display())))?;
        if entries.is_empty() {
            return Err(no_oracle(format!(
                "{} holds no case; {SHAPE}",
                path.display()
            )));
        }
        let mut cases = Vec::new();
        let mut unusable = 0;
        for (index, entry) in entries.iter().enumerate() {
            let function = entry.get("function").and_then(Value::as_str);
            let args = entry.get("args").cloned().unwrap_or_else(|| json!([]));
            match (function, entry.get("expect"), args) {
                (Some(function), Some(expect), Value::Array(args)) => cases.push(Case {
                    name: entry
                        .get("name")
                        .and_then(Value::as_str)
                        .map_or_else(|| format!("case{index}"), str::to_owned),
                    function: function.to_owned(),
                    args,
                    expect: expect.clone(),
                }),
                _ => unusable += 1,
            }
        }
        Ok(Self {
            sha256: draft::sha256(&bytes),
            bytes: bytes.len(),
            cases,
            unusable,
        })
    }
}

/// How one case ended.
#[derive(Clone, Debug)]
enum Outcome {
    /// The result equals the expectation.
    Pass(Value),
    /// The result differs from the expectation.
    Fail(Value),
    /// The case exhausted its resource limit (fuel cap included).
    Limit(Value),
    /// The case could not be run (unknown function, input, lowering).
    Unknown(String),
}

impl Outcome {
    const fn label(&self) -> &'static str {
        match self {
            Self::Pass(_) => "pass",
            Self::Fail(_) => "fail",
            Self::Limit(_) => "resource limit",
            Self::Unknown(_) => "unknown",
        }
    }
}

/// One case run.
#[derive(Clone, Debug)]
struct CaseRun {
    case: usize,
    outcome: Outcome,
    fuel: u64,
}

/// The public cases run on one program state.
#[derive(Clone, Debug, Default)]
struct Evaluation {
    outcomes: Vec<CaseRun>,
    /// Every case ran (the wall limit did not stop the evaluation).
    complete: bool,
}

impl Evaluation {
    fn passed(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|run| matches!(run.outcome, Outcome::Pass(_)))
            .count()
    }

    fn to_json(&self, cases: &[Case]) -> Value {
        let outcomes: Vec<Value> = self
            .outcomes
            .iter()
            .map(|run| {
                let case = &cases[run.case];
                let mut value = json!({"name": case.name, "function": case.function,
                    "outcome": run.outcome.label(), "expected": case.expect});
                match &run.outcome {
                    Outcome::Pass(actual) | Outcome::Fail(actual) | Outcome::Limit(actual) => {
                        value["actual"] = actual.clone();
                    }
                    Outcome::Unknown(detail) => value["detail"] = json!(detail),
                }
                value
            })
            .collect();
        json!({"passed": self.passed(), "run": self.outcomes.len(), "cases": cases.len(),
               "complete": self.complete, "outcomes": outcomes})
    }

    /// `4/4`, or `2/2 (partial: 2 of 4 run, wall limit reached)`, then the
    /// cases that did not pass, by label.
    fn text(&self, cases: &[Case]) -> String {
        let mut text = format!("{}/{}", self.passed(), self.outcomes.len());
        text.push_str(&self.partial(cases));
        text.push_str(&self.misses(cases));
        text
    }

    /// ` (partial: 2 of 4 run, wall limit reached)`, or nothing.
    fn partial(&self, cases: &[Case]) -> String {
        if self.complete {
            String::new()
        } else {
            format!(
                " (partial: {} of {} run, wall limit reached)",
                self.outcomes.len(),
                cases.len()
            )
        }
    }

    /// ` (fail: a, b; unknown: c)`, or nothing when every case run passed.
    fn misses(&self, cases: &[Case]) -> String {
        let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
        for run in &self.outcomes {
            let label = run.outcome.label();
            if label == "pass" {
                continue;
            }
            let name = cases[run.case].name.as_str();
            match groups.iter_mut().find(|(group, _)| *group == label) {
                Some((_, names)) => names.push(name),
                None => groups.push((label, vec![name])),
            }
        }
        if groups.is_empty() {
            return String::new();
        }
        let parts: Vec<String> = groups
            .iter()
            .map(|(label, names)| {
                let shown: Vec<&str> = names.iter().take(3).copied().collect();
                let more = if names.len() > 3 {
                    format!(" +{}", names.len() - 3)
                } else {
                    String::new()
                };
                format!("{label}: {}{more}", shown.join(", "))
            })
            .collect();
        format!(" ({})", parts.join("; "))
    }
}

/// Runs the cases on a program state, each under its fuel cap and the
/// `call` limits otherwise, until the deadline.
fn evaluate(
    program: &Program,
    names: &Names,
    cases: &[Case],
    caps: &[u64],
    deadline: Instant,
) -> Evaluation {
    let mut evaluation = Evaluation {
        outcomes: Vec::new(),
        complete: true,
    };
    let mut executor = match Executor::new(program) {
        Ok(executor) => Some(executor),
        Err(error) => {
            let detail = error.detail().to_owned();
            evaluation.outcomes = (0..cases.len())
                .map(|case| CaseRun {
                    case,
                    outcome: Outcome::Unknown(detail.clone()),
                    fuel: 0,
                })
                .collect();
            None
        }
    };
    let Some(executor) = executor.as_mut() else {
        return evaluation;
    };
    for (index, case) in cases.iter().enumerate() {
        if Instant::now() >= deadline {
            evaluation.complete = false;
            break;
        }
        let (outcome, fuel) = run_case(executor, program, names, case, caps[index]);
        evaluation.outcomes.push(CaseRun {
            case: index,
            outcome,
            fuel,
        });
    }
    evaluation
}

fn run_case(
    executor: &mut Executor,
    program: &Program,
    names: &Names,
    case: &Case,
    fuel: u64,
) -> (Outcome, u64) {
    let unknown = |detail: String| (Outcome::Unknown(detail), 0);
    let Some(id) = names.resolve(&case.function) else {
        return unknown(format!("no function `{}` in this state", case.function));
    };
    if executor.function(&id).is_none() {
        return unknown(format!("`{}` is not a function", case.function));
    }
    let inputs = match crate::cli::typed_inputs(executor, program, names, &id, &case.args) {
        Ok(inputs) => inputs,
        Err(error) => return unknown(error.detail().to_owned()),
    };
    let limits = sley_vm::ExecutionLimits {
        max_fuel: fuel,
        ..exec::call_limits()
    };
    match executor.run(&id, inputs, limits) {
        Err(error) => unknown(error.detail().to_owned()),
        Ok(outcome) => {
            let actual = exec::termination_json(&outcome.termination, names);
            let result = if matches!(outcome.termination, ExecutionTermination::ResourceLimit(_)) {
                Outcome::Limit(actual)
            } else if actual == crate::cli::canonical_expectation(&case.expect, program, names, &id)
            {
                Outcome::Pass(actual)
            } else {
                Outcome::Fail(actual)
            };
            (result, outcome.fuel)
        }
    }
}

// ---------------------------------------------------------------------------
// Seeds and the per-seed use limit

/// The program search starts from.
struct Seed {
    /// `head`, `c3` or `d1@r2`.
    label: String,
    /// What `try --on` takes for this seed (none for the head).
    on: Option<String>,
    /// The frame the seed was made from (none for the head).
    frame: Option<Value>,
    program: Program,
    names: Names,
    /// The use-limit key: the root draft, a candidate, or the head.
    lineage: String,
    draft: Option<String>,
    candidate: Option<String>,
}

impl Seed {
    fn resolve(
        workspace: &Workspace,
        head: &Head,
        authority: &Authority,
        map: &NameMap,
        from: Option<&str>,
    ) -> Result<Self> {
        let Some(reference) = from else {
            let program = head.program().clone();
            let names = Names::build(&program, map);
            let transaction = crate::hex::encode(head.transaction_id().as_bytes());
            return Ok(Self {
                label: "head".to_owned(),
                on: None,
                frame: None,
                program,
                names,
                lineage: format!("head@{}", &transaction[..16]),
                draft: None,
                candidate: None,
            });
        };
        let store = Store::open(workspace)?;
        let drafts = Drafts::open(workspace)?;
        if let Some(parsed) = DraftRef::parse(reference) {
            let (handle, revision) = drafts.resolve(&parsed)?;
            let spelled = draft::spell(&handle, revision);
            let status = drafts.status(&handle, revision)?;
            let state = status["state"].as_str().unwrap_or("unknown");
            if state != State::Valid.as_str() {
                return Err(seed_invalid(format!(
                    "{spelled} is {state}: search starts from a revision whose candidate is Valid (sley-agent draft {spelled} says what is open)"
                )));
            }
            let base = status["base_head"].as_str().unwrap_or("");
            let now = crate::hex::encode(head.transaction_id().as_bytes());
            if base != now {
                return Err(seed_invalid(format!(
                    "{spelled} was made on head {} and the head is now {}: rebuild it first (sley-agent try --on {handle} '{{\"af1\": 1}}' --rebase)",
                    &base[..base.len().min(8)],
                    &now[..8]
                )));
            }
            let frame = drafts.frame(&handle, revision)?.ok_or_else(|| {
                seed_invalid(format!("{spelled} is a text draft: it has no frame"))
            })?;
            let candidate = status["candidate"]
                .as_str()
                .filter(|handle| candidate::is_handle(handle))
                .ok_or_else(|| seed_invalid(format!("{spelled} names no candidate")))?
                .to_owned();
            let stored = store.load(&candidate)?;
            if status["candidate_sha256"].as_str() != Some(draft::sha256(&stored).as_str()) {
                return Err(seed_invalid(format!(
                    "the stored bytes of {candidate} are not the ones {spelled} recorded"
                )));
            }
            let program = valid_program(head, authority, &stored, &spelled)?;
            let lineage = lineage_of_draft(&drafts, &store, &handle, 0);
            return Ok(Self {
                names: Names::build(&program, map),
                label: spelled.clone(),
                on: Some(spelled.clone()),
                frame: Some(frame),
                program,
                lineage,
                draft: Some(spelled),
                candidate: Some(candidate),
            });
        }
        let handle = store.resolve(Some(reference))?;
        if !candidate::is_handle(&handle) {
            return Err(seed_invalid(format!(
                "`{reference}` is not a candidate handle (c3) or a draft (d1, d1@r2)"
            )));
        }
        let stored = store.load(&handle)?;
        let program = valid_program(head, authority, &stored, &handle)?;
        let meta = store.meta(&handle).unwrap_or_default();
        let frame = meta
            .get("frame")
            .filter(|frame| frame.is_object())
            .cloned()
            .ok_or_else(|| {
                seed_invalid(format!(
                    "{handle} was not made from an AF1 frame, so no neighbor can be layered on it"
                ))
            })?;
        let lineage = lineage_of_candidate(&drafts, &store, &handle, 0);
        Ok(Self {
            names: Names::build(&program, map),
            label: handle.clone(),
            on: Some(handle.clone()),
            frame: Some(frame),
            program,
            lineage,
            draft: meta["draft"].as_str().map(str::to_owned),
            candidate: Some(handle),
        })
    }
}

/// The program a Valid candidate proposes, or the seed refusal.
fn valid_program(
    head: &Head,
    authority: &Authority,
    stored: &[u8],
    label: &str,
) -> Result<Program> {
    let output = candidate::validate(head, authority, stored)?;
    if !output.is_valid() {
        let verdict = Verdict::of(&output, head.program(), &Names::default());
        return Err(seed_invalid(format!(
            "{label} is not Valid against the current head ({}): search starts from a Valid candidate",
            verdict.headline()
        )));
    }
    candidate::proposed_program(head, &output)
        .ok_or_else(|| seed_invalid(format!("{label} proposes no program state")))
}

/// Longest `on` chain followed to a lineage root.
const MAX_LINEAGE_DEPTH: usize = 64;

/// A draft's lineage: the lineage of the candidate its first revision was
/// layered on (`try --on cK`), else the draft itself.
fn lineage_of_draft(drafts: &Drafts, store: &Store, handle: &str, depth: usize) -> String {
    if depth < MAX_LINEAGE_DEPTH
        && let Some(on) = drafts
            .status(handle, 1)
            .ok()
            .and_then(|status| status["on"].as_str().map(str::to_owned))
            .filter(|on| candidate::is_handle(on))
    {
        return lineage_of_candidate(drafts, store, &on, depth + 1);
    }
    handle.to_owned()
}

/// A candidate's lineage: the lineage of the draft that made it, else the
/// candidate itself.
fn lineage_of_candidate(drafts: &Drafts, store: &Store, handle: &str, depth: usize) -> String {
    store
        .meta(handle)
        .and_then(|meta| meta["draft"].as_str().and_then(DraftRef::parse))
        .map_or_else(
            || handle.to_owned(),
            |reference| lineage_of_draft(drafts, store, &reference.handle, depth + 1),
        )
}

/// Counts one search against the seed's lineage, refusing the third.
fn record_use(workspace: &Workspace, seed: &Seed) -> Result<()> {
    let path = workspace.dir().join(STATE_DIR).join(SEARCH_FILE);
    let mut record = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<Value>(&text).map_err(|error| {
            AgentError::new(AgentErrorCode::Io, format!("{}: {error}", path.display()))
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(error) => return Err(io(&path, &error)),
    };
    if !record.is_object() {
        record = json!({});
    }
    record["searches_per_seed"] = json!(SEARCHES_PER_SEED);
    if !record["lineages"].is_object() {
        record["lineages"] = json!({});
    }
    let entry = &mut record["lineages"][seed.lineage.as_str()];
    let used = entry["uses"].as_u64().unwrap_or(0);
    if used >= SEARCHES_PER_SEED {
        let seeds: Vec<String> = entry["seeds"]
            .as_array()
            .map(|seeds| {
                seeds
                    .iter()
                    .filter_map(|seed| seed.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        return Err(seed_invalid(format!(
            "{} has used its {SEARCHES_PER_SEED} searches ({}): each seed lineage (a draft with the candidates of its revisions and the drafts started on them, or the head until the next commit) gets {SEARCHES_PER_SEED}; repair by hand with try --on",
            seed.label,
            if seeds.is_empty() {
                seed.lineage.clone()
            } else {
                format!("lineage {}: {}", seed.lineage, seeds.join(", "))
            }
        )));
    }
    entry["uses"] = json!(used + 1);
    if !entry["seeds"].is_array() {
        entry["seeds"] = json!([]);
    }
    if let Some(seeds) = entry["seeds"].as_array_mut() {
        seeds.push(json!(seed.label));
    }
    let state = workspace.state_dir()?;
    let temporary = state.join(format!(".{SEARCH_FILE}.partial"));
    let mut text = serde_json::to_string_pretty(&record).unwrap_or_default();
    text.push('\n');
    fs::write(&temporary, text).map_err(|error| io(&temporary, &error))?;
    fs::rename(&temporary, &path).map_err(|error| io(&path, &error))
}

// ---------------------------------------------------------------------------
// The searched function

/// One block of the function, in function order.
struct Block {
    id: EntityId,
    leaf: String,
    params: Vec<EntityId>,
    ops: Vec<(EntityId, OperationBody)>,
    term: Terminator,
    unreachable: bool,
}

/// The searched function as the seed program states it.
struct Model<'a> {
    program: &'a Program,
    names: &'a Names,
    params: Vec<EntityId>,
    blocks: Vec<Block>,
    /// Strict dominators of each block from the entry inward; `None` when
    /// the block is unreachable from the entry.
    chains: Vec<Option<Vec<usize>>>,
}

impl<'a> Model<'a> {
    fn build(
        program: &'a Program,
        names: &'a Names,
        body: &sley_mutate::value::FunctionBody,
    ) -> Self {
        let blocks: Vec<Block> = body
            .blocks
            .iter()
            .filter_map(|id| match program.body(id) {
                Some(EntityBodyValue::Block(block)) => Some(Block {
                    id: *id,
                    leaf: names.leaf(id),
                    params: block.parameters.clone(),
                    ops: block
                        .operations
                        .iter()
                        .filter_map(|op| match program.body(op) {
                            Some(EntityBodyValue::Operation(body)) => Some((*op, body.clone())),
                            _ => None,
                        })
                        .collect(),
                    term: block.terminator.clone(),
                    unreachable: block.reachability == Reachability::ExplicitlyUnreachable,
                }),
                _ => None,
            })
            .collect();
        let entry = blocks
            .iter()
            .position(|block| block.id == body.entry_block)
            .unwrap_or(0);
        let chains = dominator_chains(&blocks, entry);
        Self {
            program,
            names,
            params: body.parameters.clone(),
            blocks,
            chains,
        }
    }

    fn type_of(&self, value: &ValueRef) -> Option<TypeExpr> {
        match value {
            ValueRef::Parameter(id) => match self.program.body(id) {
                Some(EntityBodyValue::Parameter(parameter)) => Some(parameter.value_type.clone()),
                _ => None,
            },
            ValueRef::OperationResult(result) => match self.program.body(&result.operation) {
                Some(EntityBodyValue::Operation(operation)) => operation
                    .result_types
                    .get(usize::try_from(result.result_index).ok()?)
                    .cloned(),
                _ => None,
            },
        }
    }

    /// The values visible before operation `position` of block `block`
    /// (`ops.len()` for its terminator), in a fixed order: function
    /// parameters, results of the dominating blocks from the entry inward,
    /// the block's parameters, its earlier results.
    fn visible(&self, block: usize, position: usize) -> Vec<ValueRef> {
        let results = |ops: &[(EntityId, OperationBody)], out: &mut Vec<ValueRef>| {
            for (id, body) in ops {
                for index in 0..body.result_types.len() {
                    out.push(ValueRef::OperationResult(OperationResultRef {
                        operation: *id,
                        result_index: u32::try_from(index).unwrap_or(u32::MAX),
                    }));
                }
            }
        };
        let mut out: Vec<ValueRef> = self
            .params
            .iter()
            .copied()
            .map(ValueRef::Parameter)
            .collect();
        for dominator in self.chains[block].iter().flatten() {
            results(&self.blocks[*dominator].ops, &mut out);
        }
        let own = &self.blocks[block];
        out.extend(own.params.iter().copied().map(ValueRef::Parameter));
        results(&own.ops[..position.min(own.ops.len())], &mut out);
        out
    }

    /// The operation defining a value, when it is an operation of this
    /// function: its block and position.
    fn definer(&self, value: &ValueRef) -> Option<(usize, usize)> {
        let ValueRef::OperationResult(result) = value else {
            return None;
        };
        self.blocks.iter().enumerate().find_map(|(block, body)| {
            body.ops
                .iter()
                .position(|(id, _)| *id == result.operation)
                .map(|position| (block, position))
        })
    }

    fn block_index(&self, id: &EntityId) -> Option<usize> {
        self.blocks.iter().position(|block| block.id == *id)
    }

    /// The payload type a switch case binds to `$`.
    /// The payload type a switch case binds to `$` (none for a case
    /// without a payload, or one this model cannot type).
    fn payload(&self, scrutinee: &TypeExpr, key: &CaseKey) -> Option<TypeExpr> {
        match (key, scrutinee) {
            (CaseKey::Builtin(BuiltinCase::Ok), TypeExpr::Result { ok, .. }) => {
                Some((**ok).clone())
            }
            (CaseKey::Builtin(BuiltinCase::Err), TypeExpr::Result { error, .. }) => {
                Some((**error).clone())
            }
            (CaseKey::Builtin(BuiltinCase::Some), TypeExpr::Option(item)) => Some((**item).clone()),
            (CaseKey::Member(member), TypeExpr::Named(named)) => {
                match self.program.body(&named.definition) {
                    Some(EntityBodyValue::TypeDef(typedef)) => match &typedef.form {
                        TypeDefForm::Variant(cases) => cases
                            .iter()
                            .find(|case| case.member_id == *member)
                            .and_then(|case| case.payload_type.as_ref())
                            .map(|payload| crate::values::substitute(payload, &named.arguments)),
                        TypeDefForm::Record(_) => None,
                    },
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Whether switch case `from`'s arguments fit the parameters of case
    /// `to`'s target.
    fn fits(&self, scrutinee: &TypeExpr, from: &sley_ssmc::SwitchCase, target: &EntityId) -> bool {
        let Some(block) = self.block_index(target) else {
            return false;
        };
        let params = &self.blocks[block].params;
        if params.len() != from.edge.arguments.len() {
            return false;
        }
        from.edge
            .arguments
            .iter()
            .zip(params)
            .all(|(argument, param)| {
                let given = match argument {
                    SwitchArgument::Value(value) => self.type_of(value),
                    SwitchArgument::CasePayload => self.payload(scrutinee, &from.case_key),
                };
                given.is_some() && given == self.type_of(&ValueRef::Parameter(*param))
            })
    }

    fn reachable(&self, block: usize) -> bool {
        self.chains[block].is_some()
    }
}

/// The successors of a terminator, by block position.
fn successors(term: &Terminator, index: &BTreeMap<EntityId, usize>) -> Vec<usize> {
    let targets: Vec<EntityId> = match term {
        Terminator::Branch(branch) => vec![branch.edge.target],
        Terminator::CondBranch(cond) => vec![cond.if_true.target, cond.if_false.target],
        Terminator::VariantSwitch(switch) => {
            switch.cases.iter().map(|case| case.edge.target).collect()
        }
        Terminator::Return(_) | Terminator::Trap(_) => Vec::new(),
    };
    targets
        .iter()
        .filter_map(|target| index.get(target).copied())
        .collect()
}

/// Strict dominator chains (entry first) by the iterative algorithm of
/// Cooper, Harvey and Kennedy over reverse postorder.
fn dominator_chains(blocks: &[Block], entry: usize) -> Vec<Option<Vec<usize>>> {
    let count = blocks.len();
    if count == 0 {
        return Vec::new();
    }
    let index: BTreeMap<EntityId, usize> = blocks
        .iter()
        .enumerate()
        .map(|(position, block)| (block.id, position))
        .collect();
    let succ: Vec<Vec<usize>> = blocks
        .iter()
        .map(|block| successors(&block.term, &index))
        .collect();
    let mut postorder = Vec::with_capacity(count);
    let mut seen = vec![false; count];
    let mut stack: Vec<(usize, usize)> = vec![(entry, 0)];
    seen[entry] = true;
    while let Some(top) = stack.last_mut() {
        let (node, next) = *top;
        if next < succ[node].len() {
            top.1 += 1;
            let successor = succ[node][next];
            if !seen[successor] {
                seen[successor] = true;
                stack.push((successor, 0));
            }
        } else {
            postorder.push(node);
            stack.pop();
        }
    }
    let rpo: Vec<usize> = postorder.iter().rev().copied().collect();
    let mut order = vec![usize::MAX; count];
    for (position, block) in rpo.iter().enumerate() {
        order[*block] = position;
    }
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (block, successors) in succ.iter().enumerate() {
        if seen[block] {
            for successor in successors {
                preds[*successor].push(block);
            }
        }
    }
    let mut idom: Vec<Option<usize>> = vec![None; count];
    idom[entry] = Some(entry);
    let intersect = |idom: &[Option<usize>], mut a: usize, mut b: usize| {
        while a != b {
            while order[a] > order[b] {
                a = idom[a].unwrap_or(entry);
            }
            while order[b] > order[a] {
                b = idom[b].unwrap_or(entry);
            }
        }
        a
    };
    let mut changed = true;
    while changed {
        changed = false;
        for block in rpo.iter().skip(1) {
            let mut new = None;
            for pred in &preds[*block] {
                if idom[*pred].is_some() {
                    new = Some(match new {
                        None => *pred,
                        Some(other) => intersect(&idom, *pred, other),
                    });
                }
            }
            if new.is_some() && new != idom[*block] {
                idom[*block] = new;
                changed = true;
            }
        }
    }
    (0..count)
        .map(|block| {
            if !seen[block] {
                return None;
            }
            let mut chain = Vec::new();
            let mut current = block;
            while current != entry && chain.len() < count {
                current = idom[current]?;
                chain.push(current);
            }
            chain.reverse();
            Some(chain)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Changes

/// One proposed change, in terms of the seed program.
#[derive(Clone, Debug)]
enum Change {
    Opcode {
        block: usize,
        op: usize,
        to: u32,
    },
    Permute {
        block: usize,
        op: usize,
    },
    Operand {
        block: usize,
        op: usize,
        slot: usize,
        with: ValueRef,
    },
    Returned {
        block: usize,
        with: ValueRef,
    },
    Argument {
        block: usize,
        slot: usize,
        with: ValueRef,
    },
    Nudge {
        block: usize,
        op: usize,
        value: ConstValue,
    },
    CondSwap {
        block: usize,
    },
    CaseSwap {
        block: usize,
        first: usize,
        second: usize,
    },
    Negate {
        block: usize,
    },
    Unnegate {
        block: usize,
        inner: ValueRef,
        not_op: EntityId,
    },
}

impl Change {
    const fn generator(&self) -> Generator {
        match self {
            Self::Opcode { .. } => Generator::OpcodeSwap,
            Self::Permute { .. } => Generator::OperandPermutation,
            Self::Operand { .. } | Self::Returned { .. } | Self::Argument { .. } => {
                Generator::OperandSubstitution
            }
            Self::Nudge { .. } => Generator::ConstantNudge,
            Self::CondSwap { .. } | Self::CaseSwap { .. } => Generator::EdgeSwap,
            Self::Negate { .. } | Self::Unnegate { .. } => Generator::Negation,
        }
    }

    const fn block(&self) -> usize {
        match self {
            Self::Opcode { block, .. }
            | Self::Permute { block, .. }
            | Self::Operand { block, .. }
            | Self::Returned { block, .. }
            | Self::Argument { block, .. }
            | Self::Nudge { block, .. }
            | Self::CondSwap { block }
            | Self::CaseSwap { block, .. }
            | Self::Negate { block }
            | Self::Unnegate { block, .. } => *block,
        }
    }

    /// The operation the change rewrites (`None` for terminator changes).
    const fn op(&self) -> Option<usize> {
        match self {
            Self::Opcode { op, .. }
            | Self::Permute { op, .. }
            | Self::Operand { op, .. }
            | Self::Nudge { op, .. } => Some(*op),
            _ => None,
        }
    }
}

/// The integer constants a nudge proposes: plus one, minus one, negated,
/// each in its type's range and different from the value.
fn nudges(value: &ConstValue) -> Vec<ConstValue> {
    let mut out: Vec<ConstValue> = Vec::new();
    let mut push = |data: ConstData| {
        let candidate = ConstValue {
            value_type: value.value_type.clone(),
            data,
        };
        if candidate != *value && !out.contains(&candidate) {
            out.push(candidate);
        }
    };
    match (&value.data, &value.value_type) {
        (ConstData::SInt(number), TypeExpr::SInt(width)) => {
            let bits = u32::from(width.bits()).clamp(1, 128);
            let (low, high) = if bits >= 128 {
                (i128::MIN, i128::MAX)
            } else {
                (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
            };
            for candidate in [
                number.checked_add(1),
                number.checked_sub(1),
                number.checked_neg(),
            ]
            .into_iter()
            .flatten()
            {
                if (low..=high).contains(&candidate) {
                    push(ConstData::SInt(candidate));
                }
            }
        }
        (ConstData::UInt(number), TypeExpr::UInt(width)) => {
            let bits = u32::from(width.bits()).clamp(1, 128);
            let high = if bits >= 128 {
                u128::MAX
            } else {
                (1_u128 << bits) - 1
            };
            for candidate in [number.checked_add(1), number.checked_sub(1)]
                .into_iter()
                .flatten()
            {
                if candidate <= high {
                    push(ConstData::UInt(candidate));
                }
            }
        }
        _ => {}
    }
    out
}

/// The neighbors found so far, and why generation stopped.
struct Generated {
    neighbors: Vec<Neighbor>,
    /// Another neighbor existed when the neighbor limit was reached.
    more: bool,
    /// The wall limit stopped generation.
    wall: bool,
    /// Changes no frame layered on the seed can state: generated code, a
    /// permutation of two nested operations, a block beside the seed's
    /// edits of the function.
    skipped: usize,
    /// Changes whose frame equals an earlier neighbor's.
    duplicates: usize,
}

/// Collects neighbors up to the limit.
struct Sink<'w> {
    writer: &'w Writer<'w>,
    max: usize,
    deadline: Instant,
    seen: BTreeSet<String>,
    out: Generated,
    done: bool,
}

impl Sink<'_> {
    /// Offers one change; `false` once generation stops.
    fn offer(&mut self, change: &Change) -> bool {
        if self.done {
            return false;
        }
        if Instant::now() >= self.deadline {
            self.out.wall = true;
            self.done = true;
            return false;
        }
        let Some(written) = self.writer.write(change) else {
            self.out.skipped += 1;
            return true;
        };
        let key = written.frame.to_string();
        if self.seen.contains(&key) {
            self.out.duplicates += 1;
            return true;
        }
        if self.out.neighbors.len() >= self.max {
            self.out.more = true;
            self.done = true;
            return false;
        }
        self.seen.insert(key);
        self.out.neighbors.push(Neighbor {
            index: self.out.neighbors.len() + 1,
            generator: change.generator(),
            at: written.at,
            change: written.change,
            pointer: written.pointer,
            frame: written.frame,
            status: Status::NotEvaluated,
            kernel: None,
            evaluation: None,
            rank: None,
        });
        true
    }
}

/// Runs the generators in the generation sequence.
#[allow(clippy::too_many_lines)]
fn generate(model: &Model<'_>, writer: &Writer<'_>, max: usize, deadline: Instant) -> Generated {
    let mut sink = Sink {
        writer,
        max,
        deadline,
        seen: BTreeSet::new(),
        out: Generated {
            neighbors: Vec::new(),
            more: false,
            wall: false,
            skipped: 0,
            duplicates: 0,
        },
        done: false,
    };
    'phases: for generator in PHASES {
        for (index, block) in model.blocks.iter().enumerate() {
            if !model.reachable(index) {
                continue;
            }
            let end = block.ops.len();
            for (position, (_, body)) in block.ops.iter().enumerate() {
                let changes: Vec<Change> = match generator {
                    Generator::OpcodeSwap => FAMILIES
                        .iter()
                        .find(|family| family.contains(&body.opcode))
                        .filter(|_| body.operands.len() == 2)
                        .map(|family| {
                            family
                                .iter()
                                .filter(|to| **to != body.opcode)
                                .map(|to| Change::Opcode {
                                    block: index,
                                    op: position,
                                    to: *to,
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    Generator::OperandPermutation => {
                        let [first, second] = body.operands.as_slice() else {
                            continue;
                        };
                        if NON_COMMUTATIVE.contains(&body.opcode)
                            && first != second
                            && model.type_of(first).is_some()
                            && model.type_of(first) == model.type_of(second)
                        {
                            vec![Change::Permute {
                                block: index,
                                op: position,
                            }]
                        } else {
                            Vec::new()
                        }
                    }
                    Generator::ConstantNudge => match (&body.immediate, body.opcode) {
                        (Immediate::Entity(constant), CONST) => {
                            match model.program.body(constant) {
                                Some(EntityBodyValue::Constant(constant)) => {
                                    nudges(&constant.value)
                                        .into_iter()
                                        .map(|value| Change::Nudge {
                                            block: index,
                                            op: position,
                                            value,
                                        })
                                        .collect()
                                }
                                _ => Vec::new(),
                            }
                        }
                        _ => Vec::new(),
                    },
                    Generator::OperandSubstitution => {
                        let visible = model.visible(index, position);
                        let mut changes = Vec::new();
                        for (slot, operand) in body.operands.iter().enumerate() {
                            let Some(ty) = model.type_of(operand) else {
                                continue;
                            };
                            for value in &visible {
                                if value != operand
                                    && model.type_of(value).as_ref() == Some(&ty)
                                    && writer.nameable(value, index)
                                {
                                    changes.push(Change::Operand {
                                        block: index,
                                        op: position,
                                        slot,
                                        with: *value,
                                    });
                                }
                            }
                        }
                        changes
                    }
                    Generator::EdgeSwap | Generator::Negation => Vec::new(),
                };
                for change in &changes {
                    if !sink.offer(change) {
                        break 'phases;
                    }
                }
            }
            let changes: Vec<Change> = match (generator, &block.term) {
                (Generator::OperandSubstitution, Terminator::Return(ret)) => {
                    let ty = model.type_of(&ret.value);
                    model
                        .visible(index, end)
                        .into_iter()
                        .filter(|value| {
                            *value != ret.value
                                && ty.is_some()
                                && model.type_of(value) == ty
                                && writer.nameable(value, index)
                        })
                        .map(|with| Change::Returned { block: index, with })
                        .collect()
                }
                (Generator::OperandSubstitution, Terminator::Branch(branch)) => {
                    let visible = model.visible(index, end);
                    let mut changes = Vec::new();
                    for (slot, argument) in branch.edge.arguments.iter().enumerate() {
                        let Some(ty) = model.type_of(argument) else {
                            continue;
                        };
                        for value in &visible {
                            if value != argument
                                && model.type_of(value).as_ref() == Some(&ty)
                                && writer.nameable(value, index)
                            {
                                changes.push(Change::Argument {
                                    block: index,
                                    slot,
                                    with: *value,
                                });
                            }
                        }
                    }
                    changes
                }
                (Generator::EdgeSwap, Terminator::CondBranch(cond))
                    if cond.if_true != cond.if_false =>
                {
                    vec![Change::CondSwap { block: index }]
                }
                (Generator::EdgeSwap, Terminator::VariantSwitch(switch)) => {
                    let mut changes = Vec::new();
                    if let Some(scrutinee) = model.type_of(&switch.value) {
                        for first in 0..switch.cases.len() {
                            for second in first + 1..switch.cases.len() {
                                let (a, b) = (&switch.cases[first], &switch.cases[second]);
                                if a.edge.target != b.edge.target
                                    && a.edge.arguments.len() == b.edge.arguments.len()
                                    && model.fits(&scrutinee, a, &b.edge.target)
                                    && model.fits(&scrutinee, b, &a.edge.target)
                                {
                                    changes.push(Change::CaseSwap {
                                        block: index,
                                        first,
                                        second,
                                    });
                                }
                            }
                        }
                    }
                    changes
                }
                (Generator::Negation, Terminator::CondBranch(cond)) => {
                    let negated = model.definer(&cond.condition).and_then(|(b, p)| {
                        let (id, body) = &model.blocks[b].ops[p];
                        (body.opcode == NOT && body.operands.len() == 1)
                            .then(|| (*id, body.operands[0]))
                    });
                    match negated {
                        Some((not_op, inner)) => {
                            if model.visible(index, end).contains(&inner)
                                && model.type_of(&inner) == Some(TypeExpr::Bool)
                            {
                                vec![Change::Unnegate {
                                    block: index,
                                    inner,
                                    not_op,
                                }]
                            } else {
                                Vec::new()
                            }
                        }
                        None => vec![Change::Negate { block: index }],
                    }
                }
                _ => Vec::new(),
            };
            for change in &changes {
                if !sink.offer(change) {
                    break 'phases;
                }
            }
        }
    }
    sink.out
}

// ---------------------------------------------------------------------------
// Writing neighbors as layerable frames

/// How the seed's frame states the searched function, which decides how a
/// neighbor is written so that `try --on <seed>` layers it as intended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Place {
    /// No seed frame (the head): edits and patches of the live function.
    Live,
    /// The seed frame does not define or patch the function; it may edit it.
    Unstated { edits: bool },
    /// The seed frame patches the function.
    Patched,
    /// The seed frame defines the function in `fns`.
    Defined,
}

/// A frame entry's function name.
fn entry_name(entry: &Value) -> Option<&str> {
    ["fn", "function", "name"]
        .iter()
        .find_map(|key| entry.get(*key).and_then(Value::as_str))
}

impl Place {
    fn of(frame: Option<&Value>, function: &str) -> Self {
        let Some(frame) = frame else {
            return Self::Live;
        };
        let names = |key: &str| {
            frame
                .get(key)
                .and_then(Value::as_array)
                .is_some_and(|entries| {
                    entries
                        .iter()
                        .any(|entry| entry_name(entry) == Some(function))
                })
        };
        if names("fns") || names("functions") {
            Self::Defined
        } else if names("patch") {
            Self::Patched
        } else {
            Self::Unstated {
                edits: names("edit"),
            }
        }
    }
}

/// What an authored construct is (from the AF1-X source map; plain AF1
/// frames are their own expansion).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Op,
    Checked,
    Nested,
    Literal,
    Term,
    Exit,
    Other,
}

impl Kind {
    const fn of(role: Role) -> Self {
        match role {
            Role::Op => Self::Op,
            Role::Checked => Self::Checked,
            Role::Nested => Self::Nested,
            Role::Literal => Self::Literal,
            Role::Term => Self::Term,
            Role::Exit => Self::Exit,
            _ => Self::Other,
        }
    }
}

/// The seed frame's own statement of the function, with the way back from
/// every compiled block, operation and terminator to what the author wrote.
struct Stated {
    frame: Value,
    afx: bool,
    /// The function's blocks as compiled (the expansion of an AF1-X frame):
    /// leaf name to (pointer, block).
    blocks: BTreeMap<String, (String, Value)>,
    /// AF1-X: exact expanded pointer to (authored pointer, kind).
    entries: BTreeMap<String, (String, Kind)>,
}

/// Where an authored construct sits.
struct Site {
    /// Tokens of the authored block's pointer.
    block: Vec<String>,
    /// The authored block's name.
    name: String,
    /// Tokens below the block.
    rest: Vec<String>,
}

fn tokens(pointer: &str) -> Vec<String> {
    pointer
        .split('/')
        .skip(1)
        .map(|token| token.replace("~1", "/").replace("~0", "~"))
        .collect()
}

fn at<'v>(value: &'v Value, tokens: &[String]) -> Option<&'v Value> {
    tokens.iter().try_fold(value, |value, token| match value {
        Value::Array(items) => items.get(token.parse::<usize>().ok()?),
        Value::Object(map) => map.get(token),
        _ => None,
    })
}

fn at_mut<'v>(value: &'v mut Value, tokens: &[String]) -> Option<&'v mut Value> {
    tokens.iter().try_fold(value, |value, token| match value {
        Value::Array(items) => items.get_mut(token.parse::<usize>().ok()?),
        Value::Object(map) => map.get_mut(token),
        _ => None,
    })
}

/// An operation statement's name: `["name", ...]` or `{"name": ...}`
/// (exit statements `["!Case", ...]` have none).
fn item_name(item: &Value) -> Option<&str> {
    match item {
        Value::Array(items) => items
            .first()
            .and_then(Value::as_str)
            .filter(|name| !name.starts_with('!')),
        Value::Object(object) => object.get("name").and_then(Value::as_str),
        _ => None,
    }
}

impl Stated {
    fn of(frame: &Value, head: &Head, head_names: &Names, function: &str) -> Option<Self> {
        let afx = frame.get("afx").and_then(Value::as_u64) == Some(1);
        let (expanded, entries) = if afx {
            let expansion = afx::expand(head.program(), head_names, frame).ok()?;
            if !expansion.obligations.is_empty() {
                return None;
            }
            let mut entries = BTreeMap::new();
            for entry in &expansion.map.entries {
                entries
                    .entry(entry.expanded.clone())
                    .or_insert_with(|| (entry.authored.clone(), Kind::of(entry.role)));
            }
            (expansion.frame, entries)
        } else {
            (frame.clone(), BTreeMap::new())
        };
        let mut blocks = BTreeMap::new();
        for key in ["fns", "functions"] {
            for (index, definition) in expanded
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                if entry_name(definition) != Some(function) {
                    continue;
                }
                for (position, block) in definition
                    .get("blocks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    if let Some(name) = block.get("name").and_then(Value::as_str) {
                        blocks.insert(
                            name.to_owned(),
                            (format!("/{key}/{index}/blocks/{position}"), block.clone()),
                        );
                    }
                }
            }
        }
        for (index, patch) in expanded
            .get("patch")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if entry_name(patch) != Some(function) {
                continue;
            }
            for (name, block) in patch
                .get("blocks")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                if block.is_object() {
                    blocks.insert(
                        name.clone(),
                        (format!("/patch/{index}/blocks/{name}"), block.clone()),
                    );
                }
            }
        }
        Some(Self {
            frame: frame.clone(),
            afx,
            blocks,
            entries,
        })
    }

    /// The authored pointer and kind of a compiled construct.
    fn authored(&self, expanded: &str) -> Option<(String, Kind)> {
        if self.afx {
            return self.entries.get(expanded).cloned();
        }
        let kind = if expanded.ends_with("/term") {
            Kind::Term
        } else {
            Kind::Op
        };
        Some((expanded.to_owned(), kind))
    }

    fn op(&self, block: &str, op: &str) -> Option<(String, Kind)> {
        let (pointer, value) = self.blocks.get(block)?;
        let position = value
            .get("ops")?
            .as_array()?
            .iter()
            .position(|item| item_name(item) == Some(op))?;
        self.authored(&format!("{pointer}/ops/{position}"))
    }

    fn term(&self, block: &str) -> Option<(String, Kind)> {
        let (pointer, _) = self.blocks.get(block)?;
        self.authored(&format!("{pointer}/term"))
    }

    /// The authored block holding a pointer, in the function searched.
    fn site(&self, pointer: &str, function: &str) -> Option<Site> {
        let tokens = tokens(pointer);
        if tokens.len() < 4 || tokens[2] != "blocks" {
            return None;
        }
        let definition = at(&self.frame, &tokens[..2])?;
        if entry_name(definition) != Some(function) {
            return None;
        }
        let block = at(&self.frame, &tokens[..4])?;
        let name = match tokens[0].as_str() {
            "fns" | "functions" => block.get("name")?.as_str()?.to_owned(),
            "patch" => tokens[3].clone(),
            _ => return None,
        };
        Some(Site {
            block: tokens[..4].to_vec(),
            name,
            rest: tokens[4..].to_vec(),
        })
    }
}

/// A neighbor written as a frame.
struct Written {
    frame: Value,
    at: String,
    change: String,
    pointer: Option<String>,
}

/// Writes neighbors as frames that layer on the seed's frame.
struct Writer<'a> {
    model: &'a Model<'a>,
    head: &'a Program,
    function: String,
    place: Place,
    stated: Option<Stated>,
    afx: bool,
}

/// The form of an operation statement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Shape {
    /// `["name", "opcode", immediate?, operands...]`.
    Named,
    /// `{"name", "op", "args": [immediate?, operands...]}`.
    Object,
    /// A nested operation `["opcode", immediate?, operands...]` (AF1-X),
    /// or an `["ok", v]` terminator.
    Nested,
}

/// An operation statement's parts: where its opcode word and operands are.
struct Parts {
    shape: Shape,
    /// First operand index (in the array, or in `args`).
    first: usize,
    /// Immediate index, when the opcode takes one.
    immediate: Option<usize>,
}

impl Parts {
    fn of(item: &Value, rest: &[String], kind: Kind, body: &OperationBody) -> Option<Self> {
        let row = opcodes::by_tag(body.opcode)?;
        let takes = usize::from(row.immediate != ImmediateKind::None);
        let top = rest.len() == 2 && rest[0] == "ops";
        let shape = match item {
            Value::Object(object) if top && object.contains_key("op") => Shape::Object,
            Value::Array(_) if top && matches!(kind, Kind::Op | Kind::Checked) => Shape::Named,
            Value::Array(_) if matches!(kind, Kind::Nested | Kind::Checked | Kind::Term) => {
                Shape::Nested
            }
            _ => return None,
        };
        let parts = match shape {
            Shape::Named => Self {
                shape,
                first: 2 + takes,
                immediate: (takes == 1).then_some(2),
            },
            Shape::Nested => Self {
                shape,
                first: 1 + takes,
                immediate: (takes == 1).then_some(1),
            },
            Shape::Object => Self {
                shape,
                first: takes,
                immediate: (takes == 1).then_some(0),
            },
        };
        // The statement must be this operation: its opcode, its operand count.
        let word = parts.word(item)?;
        let base = word.split('?').next().unwrap_or(word);
        if opcodes::by_word(base).map(|row| row.tag) != Some(body.opcode) {
            return None;
        }
        (parts.operands(item)?.len() == body.operands.len()).then_some(parts)
    }

    fn word<'v>(&self, item: &'v Value) -> Option<&'v str> {
        match self.shape {
            Shape::Named => item.get(1)?.as_str(),
            Shape::Nested => item.get(0)?.as_str(),
            Shape::Object => item.get("op")?.as_str(),
        }
    }

    fn word_mut<'v>(&self, item: &'v mut Value) -> Option<&'v mut Value> {
        match self.shape {
            Shape::Named => item.get_mut(1),
            Shape::Nested => item.get_mut(0),
            Shape::Object => item.get_mut("op"),
        }
    }

    fn list<'v>(&self, item: &'v Value) -> Option<&'v Vec<Value>> {
        match self.shape {
            Shape::Object => item.get("args")?.as_array(),
            _ => item.as_array(),
        }
    }

    fn list_mut<'v>(&self, item: &'v mut Value) -> Option<&'v mut Vec<Value>> {
        match self.shape {
            Shape::Object => item.get_mut("args")?.as_array_mut(),
            _ => item.as_array_mut(),
        }
    }

    fn operands<'v>(&self, item: &'v Value) -> Option<&'v [Value]> {
        self.list(item)?.get(self.first..)
    }
}

/// A value as items show it: a name plainly, anything else as JSON.
fn show(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

/// A nudged constant in the form of the literal it replaces: a bare number
/// stays bare, a typed literal keeps its type key, a constant name becomes
/// a typed literal.
fn literal_like(old: &Value, value: &ConstValue, names: &Names) -> Value {
    let number = crate::values::to_json(value, names);
    match old {
        Value::Number(_) => number,
        Value::Object(object) if object.contains_key("value") => {
            let mut object = object.clone();
            object.insert("value".to_owned(), number);
            Value::Object(object)
        }
        _ => typed_literal(value, names),
    }
}

fn typed_literal(value: &ConstValue, names: &Names) -> Value {
    json!({"type": crate::types::render(&value.value_type, names),
           "value": crate::values::to_json(value, names)})
}

/// Opcodes whose result type the frame compiler derives from the immediate
/// or the operands; the others are restated with their type.
fn derivable(tag: u32, operands: bool) -> bool {
    matches!(
        tag,
        1 | 16..=20 | 33..=35 | 37..=40 | 64..=71 | 80..=85 | 96..=104 | 112 | 128 | 176 | 177 | 192..=194
    ) || (tag == 32 && operands)
}

impl Writer<'_> {
    fn names(&self) -> &Names {
        self.model.names
    }

    /// Whether search can name a value in the neighbor's frame: every value
    /// plainly, except that an AF1-X statement names only authored values.
    fn nameable(&self, value: &ValueRef, block: usize) -> bool {
        let stated = self
            .stated
            .as_ref()
            .is_some_and(|stated| stated.blocks.contains_key(&self.model.blocks[block].leaf));
        !(stated && self.afx) || self.afx_name(value).is_some()
    }

    /// An AF1-X name for a value: its plain name (the dialect qualifies it),
    /// when the author could have written it.
    fn afx_name(&self, value: &ValueRef) -> Option<String> {
        let (id, index) = match value {
            ValueRef::Parameter(id) => (*id, 0),
            ValueRef::OperationResult(result) => (result.operation, result.result_index),
        };
        let leaf = self.names().leaf(&id);
        (index == 0 && !leaf.contains("__") && is_identifier(&leaf)).then_some(leaf)
    }

    /// A value named from a block of the compiled function.
    fn reference(&self, value: &ValueRef, block: usize, afx: bool) -> Option<String> {
        if afx {
            self.afx_name(value)
        } else {
            Some(
                self.names()
                    .value(value, Some(&self.model.blocks[block].id)),
            )
        }
    }

    fn write(&self, change: &Change) -> Option<Written> {
        let block = &self.model.blocks[change.block()];
        let stated = self
            .stated
            .as_ref()
            .filter(|stated| stated.blocks.contains_key(&block.leaf));
        match (stated, self.place) {
            (Some(stated), _) => self.write_stated(stated, change),
            (None, Place::Defined) => None,
            (None, _) => self.write_program(change),
        }
    }

    /// The neighbor frame: `edit` when the change stays inside one named
    /// operation of the authored block, else a `patch` restating the block.
    fn frame_for(&self, site: &Site, block: Value) -> Value {
        let mut entry = None;
        if site.rest.len() >= 2 && site.rest[0] == "ops" {
            let item = at(&block, &site.rest[..2]);
            if let Some(name) = item.and_then(item_name) {
                let with = match item {
                    Some(Value::Array(items)) => Value::Array(items[1..].to_vec()),
                    Some(Value::Object(object)) => {
                        let mut object = object.clone();
                        object.remove("name");
                        Value::Object(object)
                    }
                    _ => Value::Null,
                };
                entry = Some((
                    "edit",
                    json!({"fn": self.function,
                    "replace_op": format!("{}.{name}", site.name), "with": with}),
                ));
            }
        }
        let (key, entry) = entry.unwrap_or_else(|| {
            let mut block = block;
            if let Some(object) = block.as_object_mut() {
                object.remove("name");
            }
            let mut blocks = Map::new();
            blocks.insert(site.name.clone(), block);
            ("patch", json!({"fn": self.function, "blocks": blocks}))
        });
        self.frame(key, entry)
    }

    fn frame(&self, key: &str, entry: Value) -> Value {
        let mut frame = Map::new();
        frame.insert("af1".to_owned(), json!(1));
        if self.afx {
            frame.insert("afx".to_owned(), json!(1));
        }
        frame.insert(key.to_owned(), Value::Array(vec![entry]));
        Value::Object(frame)
    }

    /// Where a site is, for the reader: `block.op` or `block (term)`.
    fn site_label(site: &Site, block: &Value) -> String {
        if site.rest.len() >= 2 && site.rest[0] == "ops" {
            let item = at(block, &site.rest[..2]);
            if let Some(name) = item.and_then(item_name) {
                return format!("{}.{name}", site.name);
            }
            if let Some(word) = item.and_then(|item| item.get(0)).and_then(Value::as_str) {
                return format!("{} ({word} if)", site.name);
            }
        }
        format!("{} (term)", site.name)
    }

    #[allow(clippy::too_many_lines)]
    fn write_stated(&self, stated: &Stated, change: &Change) -> Option<Written> {
        let model = self.model;
        let block = &model.blocks[change.block()];
        let (pointer, kind) = match change.op() {
            Some(op) => stated.op(&block.leaf, &self.names().leaf(&block.ops[op].0))?,
            None => stated.term(&block.leaf)?,
        };
        let site = stated.site(&pointer, &self.function)?;
        let mut value = at(&stated.frame, &site.block)?.clone();
        let original = value.clone();
        let afx = stated.afx;
        let text = if let Some(op) = change.op() {
            let body = &block.ops[op].1;
            let item = at_mut(&mut value, &site.rest)?;
            if kind == Kind::Literal {
                let Change::Nudge { value: nudged, .. } = change else {
                    return None;
                };
                let old = item.clone();
                *item = literal_like(&old, nudged, self.names());
                let text = format!("{} -> {}", show(&old), show(item));
                // A literal inside a nested operation names that operation.
                let parent = at(&value, &site.rest[..site.rest.len().saturating_sub(1)])
                    .filter(|_| site.rest.len() > 2)
                    .and_then(|parent| parent.get(0))
                    .and_then(Value::as_str)
                    .filter(|word| {
                        opcodes::by_word(word.split('?').next().unwrap_or(word)).is_some()
                    });
                match parent {
                    Some(word) => format!("{word} {text}"),
                    None => text,
                }
            } else {
                let parts = Parts::of(item, &site.rest, kind, body)?;
                // A change inside a nested operation names that operation
                // (an opcode swap names it anyway).
                let inner =
                    if parts.shape == Shape::Nested && !matches!(change, Change::Opcode { .. }) {
                        format!("{} ", parts.word(item)?)
                    } else {
                        String::new()
                    };
                let text = match change {
                    Change::Opcode { to, .. } => {
                        let word = parts.word(item)?.to_owned();
                        let suffix = word.find('?').map_or("", |at| &word[at..]);
                        let new = format!("{}{suffix}", opcodes::by_tag(*to)?.mnemonic);
                        *parts.word_mut(item)? = Value::from(new.clone());
                        format!("{word} -> {new}")
                    }
                    Change::Permute { .. } => {
                        let first = parts.first;
                        let list = parts.list_mut(item)?;
                        if list[first].is_array() && list[first + 1].is_array() {
                            // Swapping two nested operations would reorder
                            // their evaluation, not only their use.
                            return None;
                        }
                        let (a, b) = (show(&list[first]), show(&list[first + 1]));
                        list.swap(first, first + 1);
                        format!("{a}, {b} -> {b}, {a}")
                    }
                    Change::Operand { slot, with, .. } => {
                        let name = self.reference(with, change.block(), afx)?;
                        let index = parts.first + slot;
                        let list = parts.list_mut(item)?;
                        let old = show(&list[index]);
                        list[index] = Value::from(name.clone());
                        format!("operand {slot}: {old} -> {name}")
                    }
                    Change::Nudge { value: nudged, .. } => {
                        let index = parts.immediate?;
                        let list = parts.list_mut(item)?;
                        let old = list[index].clone();
                        list[index] = literal_like(&old, nudged, self.names());
                        format!("{} -> {}", show(&old), show(&list[index]))
                    }
                    _ => return None,
                };
                format!("{inner}{text}")
            }
        } else {
            self.term_stated(stated, change, &site, kind, &mut value)?
        };
        if value == original {
            return None;
        }
        Some(Written {
            at: Self::site_label(&site, &value),
            change: text,
            pointer: Some(pointer),
            frame: self.frame_for(&site, value),
        })
    }

    /// A terminator change on an authored block: `cond` targets or
    /// condition, `switch` targets, a returned value, a `br` argument, or an
    /// AF1-X exit's condition.
    #[allow(clippy::too_many_lines)]
    fn term_stated(
        &self,
        stated: &Stated,
        change: &Change,
        site: &Site,
        kind: Kind,
        value: &mut Value,
    ) -> Option<String> {
        let afx = stated.afx;
        let index = change.block();
        let block = &self.model.blocks[index];
        match kind {
            Kind::Exit => {
                // ["!Case", "if", condition, payload?]
                let item = at_mut(value, &site.rest)?;
                if item.get(1).and_then(Value::as_str) != Some("if") {
                    return None;
                }
                let condition = item.get(2)?.clone();
                let replacement = match change {
                    Change::Negate { .. } => json!(["not", condition]),
                    Change::Unnegate { inner, not_op, .. } => {
                        self.unnegated(stated, &condition, inner, not_op, index, true)?
                    }
                    _ => return None,
                };
                item[2] = replacement.clone();
                Some(format!(
                    "condition {} -> {}",
                    show(&condition),
                    show(&replacement)
                ))
            }
            Kind::Term => {
                let term = at(value, &site.rest)?.clone();
                let items = term.as_array()?;
                let word = items.first()?.as_str()?;
                let mut new = term.clone();
                let text = match (change, word) {
                    (Change::CondSwap { .. }, "cond") if items.len() == 4 => {
                        new[2] = items[3].clone();
                        new[3] = items[2].clone();
                        format!(
                            "then {}, else {} -> then {}, else {}",
                            show(&items[2]),
                            show(&items[3]),
                            show(&items[3]),
                            show(&items[2])
                        )
                    }
                    (Change::CaseSwap { first, second, .. }, "switch") => {
                        let Terminator::VariantSwitch(switch) = &block.term else {
                            return None;
                        };
                        let key = |case: &sley_ssmc::SwitchCase| match case.case_key {
                            CaseKey::Builtin(builtin) => format!("{builtin:?}"),
                            CaseKey::Member(member) => {
                                let name = self.names().member_any(&member);
                                name.rsplit('.').next().unwrap_or(&name).to_owned()
                            }
                        };
                        let (a, b) = (key(&switch.cases[*first]), key(&switch.cases[*second]));
                        let find = |key: &str| {
                            items.iter().skip(2).position(|case| {
                                case.get(0).and_then(Value::as_str).is_some_and(|written| {
                                    written == key || written.rsplit('.').next() == Some(key)
                                })
                            })
                        };
                        let (mut i, mut j) = (find(&a)? + 2, find(&b)? + 2);
                        // Described in the author's case order.
                        let (a, b) = if i > j {
                            std::mem::swap(&mut i, &mut j);
                            (b, a)
                        } else {
                            (a, b)
                        };
                        let target_path = |case: &Value| -> Option<Vec<usize>> {
                            match case.get(1)? {
                                Value::String(_) => Some(vec![1]),
                                Value::Array(inner) if inner.first()?.is_string() => {
                                    Some(vec![1, 0])
                                }
                                _ => None,
                            }
                        };
                        let (pi, pj) = (target_path(&items[i])?, target_path(&items[j])?);
                        let get = |case: &Value, path: &[usize]| {
                            path.iter()
                                .try_fold(case, |value, index| value.get(*index))
                                .cloned()
                        };
                        let (ti, tj) = (get(&items[i], &pi)?, get(&items[j], &pj)?);
                        let set = |case: &mut Value, path: &[usize], target: Value| {
                            let slot = path
                                .iter()
                                .try_fold(case, |value, index| value.get_mut(*index));
                            if let Some(slot) = slot {
                                *slot = target;
                            }
                        };
                        set(&mut new[i], &pi, tj.clone());
                        set(&mut new[j], &pj, ti.clone());
                        format!(
                            "{a} -> {}, {b} -> {} become {a} -> {}, {b} -> {}",
                            show(&ti),
                            show(&tj),
                            show(&tj),
                            show(&ti)
                        )
                    }
                    (Change::Negate { .. }, "cond") if items.len() == 4 => {
                        let condition = items[1].clone();
                        if afx {
                            new[1] = json!(["not", condition]);
                        } else {
                            let base = condition.as_str().unwrap_or("cond");
                            let taken: Vec<String> = at(value, &["ops".to_owned()])
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten()
                                .filter_map(|item| item_name(item).map(str::to_owned))
                                .collect();
                            let fresh = self.fresh(index, base, &taken);
                            new[1] = Value::from(fresh.clone());
                            let object = value.as_object_mut()?;
                            let ops = object
                                .entry("ops")
                                .or_insert_with(|| Value::Array(Vec::new()));
                            ops.as_array_mut()?.push(json!([fresh, "not", condition]));
                        }
                        format!("condition {} -> not {}", show(&condition), show(&condition))
                    }
                    (Change::Unnegate { inner, not_op, .. }, "cond") if items.len() == 4 => {
                        let condition = items[1].clone();
                        let replacement =
                            self.unnegated(stated, &condition, inner, not_op, index, afx)?;
                        new[1] = replacement.clone();
                        format!("condition {} -> {}", show(&condition), show(&replacement))
                    }
                    (Change::Returned { with, .. }, "return") if items.len() == 2 => {
                        let name = self.reference(with, index, afx)?;
                        new[1] = Value::from(name.clone());
                        format!("return {} -> {name}", show(&items[1]))
                    }
                    (Change::Argument { slot, with, .. }, "br") => {
                        let name = self.reference(with, index, afx)?;
                        // ["br", "b", args...] or ["br", ["b", args...]]
                        let path = match items.get(1)? {
                            Value::Array(_) if items.len() == 2 => vec![1, slot + 1],
                            Value::String(_) => vec![slot + 2],
                            _ => return None,
                        };
                        let slot_value = path
                            .iter()
                            .try_fold(&mut new, |value, index| value.get_mut(*index))?;
                        let old = show(slot_value);
                        *slot_value = Value::from(name.clone());
                        format!("argument {slot}: {old} -> {name}")
                    }
                    _ => return None,
                };
                *at_mut(value, &site.rest)? = new;
                Some(text)
            }
            _ => None,
        }
    }

    /// The condition that replaces `not x`: `x` as written inside an AF1-X
    /// `["not", x]`, else `x` by name, else the `not` operation's operand as
    /// its author wrote it.
    fn unnegated(
        &self,
        stated: &Stated,
        condition: &Value,
        inner: &ValueRef,
        not_op: &EntityId,
        block: usize,
        afx: bool,
    ) -> Option<Value> {
        if let Some([word, operand]) = condition.as_array().map(Vec::as_slice)
            && word.as_str() == Some("not")
        {
            return Some(operand.clone());
        }
        if let Some(name) = self.reference(inner, block, afx) {
            return Some(Value::from(name));
        }
        let (at_block, at_op) = self.model.blocks.iter().enumerate().find_map(|(b, body)| {
            body.ops
                .iter()
                .position(|(id, _)| id == not_op)
                .map(|p| (b, p))
        })?;
        let owner = &self.model.blocks[at_block];
        let (pointer, kind) = stated.op(&owner.leaf, &self.names().leaf(&owner.ops[at_op].0))?;
        let site = stated.site(&pointer, &self.function)?;
        let mut full = site.block.clone();
        full.extend(site.rest.iter().cloned());
        let item = at(&stated.frame, &full)?;
        let parts = Parts::of(item, &site.rest, kind, &owner.ops[at_op].1)?;
        parts.operands(item)?.first().cloned()
    }

    /// A fresh operation name for a block: `base`, else `base_2`, ...
    fn fresh(&self, block: usize, base: &str, taken: &[String]) -> String {
        let names = self.names();
        let owner = &self.model.blocks[block];
        let mut used: BTreeSet<String> = taken.iter().cloned().collect();
        used.extend(self.model.params.iter().map(|id| names.leaf(id)));
        used.extend(owner.params.iter().map(|id| names.leaf(id)));
        used.extend(owner.ops.iter().map(|(id, _)| names.leaf(id)));
        let stem: String = format!("not_{base}").chars().take(56).collect();
        let stem = if is_identifier(&stem) {
            stem
        } else {
            "not_cond".to_owned()
        };
        (1..=u32::MAX)
            .map(|n| {
                if n == 1 {
                    stem.clone()
                } else {
                    format!("{stem}_{n}")
                }
            })
            .find(|name| !used.contains(name))
            .unwrap_or(stem)
    }

    // -- the program's own statement (the head, or a function the seed's
    //    frame does not state) --

    fn entity(&self, id: &EntityId) -> Value {
        match self.model.program.body(id) {
            // A constant the seed created has no name the head knows: its value.
            Some(EntityBodyValue::Constant(constant)) if !self.head.contains(id) => {
                typed_literal(&constant.value, self.names())
            }
            _ => Value::from(self.names().name(id)),
        }
    }

    /// An operation restated as AF1 (named when `leaf` is given).
    fn render_op(
        &self,
        block: usize,
        body: &OperationBody,
        leaf: Option<&str>,
        literal: Option<Value>,
    ) -> Option<Value> {
        let names = self.names();
        let row = opcodes::by_tag(body.opcode)?;
        let mut args: Vec<Value> = Vec::new();
        match literal {
            Some(literal) => args.push(literal),
            None => match &body.immediate {
                Immediate::None => {}
                Immediate::Entity(id) => args.push(self.entity(id)),
                Immediate::Index(index) => args.push(Value::from(*index)),
                Immediate::Field(member) => args.push(Value::from(names.member_any(member))),
                Immediate::Variant(variant) => args.push(Value::from(
                    names.member(&variant.definition, &variant.member_id),
                )),
                Immediate::Observation(bytes) => args.push(Value::from(crate::hex::encode(bytes))),
                Immediate::Function(function) => {
                    args.push(Value::from(names.name(&function.function)));
                }
            },
        }
        let at = &self.model.blocks[block].id;
        args.extend(
            body.operands
                .iter()
                .map(|operand| Value::from(names.value(operand, Some(at)))),
        );
        if derivable(row.tag, !body.operands.is_empty()) {
            let mut items: Vec<Value> = leaf.map(Value::from).into_iter().collect();
            items.push(Value::from(row.mnemonic));
            items.extend(args);
            return Some(Value::Array(items));
        }
        let mut object = Map::new();
        if let Some(leaf) = leaf {
            object.insert("name".to_owned(), Value::from(leaf));
        }
        object.insert("op".to_owned(), Value::from(row.mnemonic));
        object.insert("args".to_owned(), Value::Array(args));
        if let Some(ty) = body.result_types.first() {
            object.insert(
                "type".to_owned(),
                Value::from(crate::types::render(ty, names)),
            );
        }
        Some(Value::Object(object))
    }

    /// A terminator restated as AF1; `condition` overrides a `cond`'s.
    fn render_term(&self, block: usize, term: &Terminator, condition: Option<&str>) -> Value {
        let names = self.names();
        let at = &self.model.blocks[block].id;
        let value = |v: &ValueRef| Value::from(names.value(v, Some(at)));
        let edge = |edge: &TargetEdge| {
            let mut items = vec![Value::from(names.leaf(&edge.target))];
            items.extend(edge.arguments.iter().map(value));
            Value::Array(items)
        };
        match term {
            Terminator::Return(ret) => json!(["return", value(&ret.value)]),
            Terminator::Branch(branch) => {
                let mut items = vec![Value::from("br")];
                if let Value::Array(rest) = edge(&branch.edge) {
                    items.extend(rest);
                }
                Value::Array(items)
            }
            Terminator::CondBranch(cond) => json!([
                "cond",
                condition.map_or_else(|| value(&cond.condition), Value::from),
                edge(&cond.if_true),
                edge(&cond.if_false)
            ]),
            Terminator::VariantSwitch(switch) => {
                let mut items = vec![Value::from("switch"), value(&switch.value)];
                for case in &switch.cases {
                    let key = match case.case_key {
                        CaseKey::Builtin(builtin) => format!("{builtin:?}"),
                        CaseKey::Member(member) => names.member_any(&member),
                    };
                    let mut case_items =
                        vec![Value::from(key), Value::from(names.leaf(&case.edge.target))];
                    for argument in &case.edge.arguments {
                        case_items.push(match argument {
                            SwitchArgument::Value(v) => value(v),
                            SwitchArgument::CasePayload => Value::from("$"),
                        });
                    }
                    items.push(Value::Array(case_items));
                }
                Value::Array(items)
            }
            Terminator::Trap(trap) => {
                let code = match trap.code {
                    TrapCode::Unreachable => "unreachable",
                    TrapCode::ResourceExhausted => "resource_exhausted",
                    TrapCode::AdapterContractViolation => "adapter_contract_violation",
                    TrapCode::InternalInvariant => "internal_invariant",
                };
                let mut items = vec![Value::from("trap"), Value::from(code)];
                if let Some(payload) = &trap.payload {
                    items.push(value(payload));
                }
                Value::Array(items)
            }
        }
    }

    /// A block restated as a patch block.
    fn restate(&self, block: usize, ops: Vec<Value>, term: Value) -> Value {
        let names = self.names();
        let owner = &self.model.blocks[block];
        let mut object = Map::new();
        if !owner.params.is_empty() {
            let params: Vec<Value> = owner
                .params
                .iter()
                .map(|param| {
                    let ty = self
                        .model
                        .type_of(&ValueRef::Parameter(*param))
                        .map_or_else(|| "unit".to_owned(), |ty| crate::types::render(&ty, names));
                    json!([names.leaf(param), ty])
                })
                .collect();
            object.insert("params".to_owned(), Value::Array(params));
        }
        object.insert("ops".to_owned(), Value::Array(ops));
        object.insert("term".to_owned(), term);
        if owner.unreachable {
            object.insert("unreachable".to_owned(), Value::Bool(true));
        }
        Value::Object(object)
    }

    fn restated_ops(&self, block: usize) -> Option<Vec<Value>> {
        let names = self.names();
        self.model.blocks[block]
            .ops
            .iter()
            .map(|(id, body)| self.render_op(block, body, Some(&names.leaf(id)), None))
            .collect()
    }

    fn patch_frame(&self, block: usize, restated: Value) -> Value {
        let mut blocks = Map::new();
        blocks.insert(self.model.blocks[block].leaf.clone(), restated);
        self.frame("patch", json!({"fn": self.function, "blocks": blocks}))
    }

    #[allow(clippy::too_many_lines)]
    fn write_program(&self, change: &Change) -> Option<Written> {
        let names = self.names();
        let index = change.block();
        let block = &self.model.blocks[index];
        let text_of = |value: &ValueRef| names.value(value, Some(&block.id));
        if let Some(op) = change.op() {
            let (id, body) = &block.ops[op];
            let mut changed = body.clone();
            let mut literal = None;
            let text = match change {
                Change::Opcode { to, .. } => {
                    changed.opcode = *to;
                    format!(
                        "{} -> {}",
                        crate::view::mnemonic(body.opcode),
                        crate::view::mnemonic(*to)
                    )
                }
                Change::Permute { .. } => {
                    changed.operands.swap(0, 1);
                    let (a, b) = (text_of(&body.operands[0]), text_of(&body.operands[1]));
                    format!("{a}, {b} -> {b}, {a}")
                }
                Change::Operand { slot, with, .. } => {
                    changed.operands[*slot] = *with;
                    format!(
                        "operand {slot}: {} -> {}",
                        text_of(&body.operands[*slot]),
                        text_of(with)
                    )
                }
                Change::Nudge { value, .. } => {
                    literal = Some(typed_literal(value, names));
                    let old = match &body.immediate {
                        Immediate::Entity(constant) => match self.model.program.body(constant) {
                            Some(EntityBodyValue::Constant(constant)) => {
                                crate::values::to_text(&constant.value, names)
                            }
                            _ => names.name(constant),
                        },
                        _ => "?".to_owned(),
                    };
                    format!("{old} -> {}", crate::values::to_text(value, names))
                }
                _ => return None,
            };
            let leaf = names.leaf(id);
            let at = format!("{}.{leaf}", block.leaf);
            let frame = if self.place == Place::Patched {
                let mut ops = self.restated_ops(index)?;
                ops[op] = self.render_op(index, &changed, Some(&leaf), literal)?;
                let term = self.render_term(index, &block.term, None);
                self.patch_frame(index, self.restate(index, ops, term))
            } else {
                let with = self.render_op(index, &changed, None, literal)?;
                self.frame(
                    "edit",
                    json!({"fn": self.function, "replace_op": at, "with": with}),
                )
            };
            return Some(Written {
                frame,
                at,
                change: text,
                pointer: None,
            });
        }
        if self.place == (Place::Unstated { edits: true }) {
            // A patch beside the seed's edits of the same function would
            // restate what the edits change.
            return None;
        }
        let mut ops = self.restated_ops(index)?;
        let mut term = block.term.clone();
        let mut condition = None;
        let text = match (change, &mut term) {
            (Change::Returned { with, .. }, Terminator::Return(ret)) => {
                let text = format!("return {} -> {}", text_of(&ret.value), text_of(with));
                ret.value = *with;
                text
            }
            (Change::Argument { slot, with, .. }, Terminator::Branch(branch)) => {
                let old = branch.edge.arguments.get(*slot)?;
                let text = format!("argument {slot}: {} -> {}", text_of(old), text_of(with));
                branch.edge.arguments[*slot] = *with;
                text
            }
            (Change::CondSwap { .. }, Terminator::CondBranch(cond)) => {
                let (t, e) = (
                    names.leaf(&cond.if_true.target),
                    names.leaf(&cond.if_false.target),
                );
                std::mem::swap(&mut cond.if_true, &mut cond.if_false);
                format!("then {t}, else {e} -> then {e}, else {t}")
            }
            (Change::CaseSwap { first, second, .. }, Terminator::VariantSwitch(switch)) => {
                let key = |case: &sley_ssmc::SwitchCase| match case.case_key {
                    CaseKey::Builtin(builtin) => format!("{builtin:?}"),
                    CaseKey::Member(member) => names.member_any(&member),
                };
                let (a, b) = (key(&switch.cases[*first]), key(&switch.cases[*second]));
                let (ta, tb) = (
                    names.leaf(&switch.cases[*first].edge.target),
                    names.leaf(&switch.cases[*second].edge.target),
                );
                let target = switch.cases[*first].edge.target;
                switch.cases[*first].edge.target = switch.cases[*second].edge.target;
                switch.cases[*second].edge.target = target;
                format!("{a} -> {ta}, {b} -> {tb} become {a} -> {tb}, {b} -> {ta}")
            }
            (Change::Negate { .. }, Terminator::CondBranch(cond)) => {
                let shown = text_of(&cond.condition);
                let base = match cond.condition {
                    ValueRef::Parameter(id) => names.leaf(&id),
                    ValueRef::OperationResult(result) => names.leaf(&result.operation),
                };
                let fresh = self.fresh(index, &base, &[]);
                ops.push(json!([fresh, "not", shown]));
                condition = Some(fresh);
                format!("condition {shown} -> not {shown}")
            }
            (Change::Unnegate { inner, .. }, Terminator::CondBranch(cond)) => {
                let text = format!(
                    "condition {} -> {}",
                    text_of(&cond.condition),
                    text_of(inner)
                );
                cond.condition = *inner;
                text
            }
            _ => return None,
        };
        let rendered = self.render_term(index, &term, condition.as_deref());
        Some(Written {
            frame: self.patch_frame(index, self.restate(index, ops, rendered)),
            at: format!("{} (term)", block.leaf),
            change: text,
            pointer: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Neighbors: the normal path, then the public cases

/// Where a neighbor got to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    /// Refused by the frame compiler, record assembly or the kernel.
    Refused,
    /// Kernel-Valid, and every public case ran.
    Evaluated,
    /// Kernel-Valid, and the wall limit stopped its cases.
    Partial,
    /// The wall limit came first.
    NotEvaluated,
}

impl Status {
    const fn label(self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::Evaluated => "evaluated",
            Self::Partial => "partial",
            Self::NotEvaluated => "not evaluated",
        }
    }
}

/// One neighbor.
struct Neighbor {
    /// Generation index, from 1.
    index: usize,
    generator: Generator,
    at: String,
    change: String,
    pointer: Option<String>,
    frame: Value,
    status: Status,
    /// The kernel's verdict or the refusal that stopped the neighbor.
    kernel: Option<Value>,
    evaluation: Option<Evaluation>,
    rank: Option<usize>,
}

impl Neighbor {
    fn passed(&self) -> usize {
        self.evaluation.as_ref().map_or(0, Evaluation::passed)
    }
}

/// What every neighbor is compiled and evaluated against.
struct Context<'a> {
    head: &'a Head,
    head_names: &'a Names,
    authority: &'a Authority,
    map: &'a NameMap,
    seed_frame: Option<&'a Value>,
    cases: &'a [Case],
    caps: &'a [u64],
    deadline: Instant,
}

/// The first line of a refusal (its headline problem).
fn headline(error: &AgentError) -> String {
    error.detail().lines().next().unwrap_or("").to_owned()
}

impl Context<'_> {
    /// Layers, compiles, assembles and validates one neighbor exactly as
    /// `try --on <seed> <frame>` would, then runs the public cases on a
    /// Valid one.
    fn try_neighbor(&self, neighbor: &mut Neighbor) -> Result<()> {
        let refused = |neighbor: &mut Neighbor, stage: &str, symbol: &str, detail: String| {
            neighbor.status = Status::Refused;
            neighbor.kernel =
                Some(json!({"valid": false, "stage": stage, "symbol": symbol, "detail": detail}));
        };
        let layered = match self.seed_frame {
            Some(base) => crate::layer::layer(base, &neighbor.frame),
            None => Ok(neighbor.frame.clone()),
        };
        let layered = match layered {
            Ok(layered) => layered,
            Err(error) => {
                refused(neighbor, "frame", error.code().symbol(), headline(&error));
                return Ok(());
            }
        };
        let nonce = candidate::fresh_nonce()?;
        let mut compiled = match crate::frame::compile(
            self.head.program(),
            self.head_names,
            &self.authority.ceilings,
            &layered,
            nonce,
            &mut candidate::random32,
        ) {
            Ok(compiled) => compiled,
            Err(error) => {
                refused(neighbor, "frame", error.code().symbol(), headline(&error));
                return Ok(());
            }
        };
        if compiled.ops.is_empty() {
            // As `try` refuses it: the layered frame restores the head.
            refused(
                neighbor,
                "frame",
                AgentErrorCode::FrameInvalid.symbol(),
                "the frame changes nothing: everything it states is already live as stated"
                    .to_owned(),
            );
            return Ok(());
        }
        let ops = std::mem::take(&mut compiled.ops);
        let imported = match candidate::assemble(self.head, self.authority, nonce, ops) {
            Ok(imported) => imported,
            Err(error) => {
                refused(neighbor, "record", error.code().symbol(), headline(&error));
                return Ok(());
            }
        };
        let output = match candidate::validate(self.head, self.authority, &imported.stored_bytes) {
            Ok(output) => output,
            Err(error) => {
                refused(neighbor, "record", error.code().symbol(), headline(&error));
                return Ok(());
            }
        };
        if !output.is_valid() {
            let verdict = Verdict::of(&output, self.head.program(), self.head_names);
            neighbor.status = Status::Refused;
            neighbor.kernel = Some(json!({"valid": false, "stage": "kernel",
                "decision": verdict.decision, "phase": verdict.phase, "symbol": verdict.symbol}));
            return Ok(());
        }
        neighbor.kernel = Some(json!({"valid": true}));
        let Some(program) = candidate::proposed_program(self.head, &output) else {
            neighbor.status = Status::Refused;
            return Ok(());
        };
        let mut map = self.map.clone();
        map.extend(&compiled.names);
        let names = Names::build(&program, &map);
        let evaluation = evaluate(&program, &names, self.cases, self.caps, self.deadline);
        neighbor.status = if evaluation.complete {
            Status::Evaluated
        } else if evaluation.outcomes.is_empty() {
            Status::NotEvaluated
        } else {
            Status::Partial
        };
        neighbor.evaluation = Some(evaluation);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Reporting

/// Neighbor counts.
struct Counts {
    generated: usize,
    valid: usize,
    refused: usize,
    evaluated: usize,
    partial: usize,
    not_evaluated: usize,
    skipped: usize,
    duplicates: usize,
}

impl Counts {
    fn of(generated: &Generated) -> Self {
        let count = |status: Status| {
            generated
                .neighbors
                .iter()
                .filter(|neighbor| neighbor.status == status)
                .count()
        };
        Self {
            generated: generated.neighbors.len(),
            valid: generated
                .neighbors
                .iter()
                .filter(|neighbor| {
                    neighbor
                        .kernel
                        .as_ref()
                        .is_some_and(|kernel| kernel["valid"] == true)
                })
                .count(),
            refused: count(Status::Refused),
            evaluated: count(Status::Evaluated),
            partial: count(Status::Partial),
            not_evaluated: count(Status::NotEvaluated),
            skipped: generated.skipped,
            duplicates: generated.duplicates,
        }
    }
}

/// Local resource use of the command.
struct Resources {
    wall_millis: u64,
    cpu_millis: Option<u64>,
    peak_kib: Option<u64>,
}

impl Resources {
    fn measure(started: Instant, cpu_before: Option<u64>) -> Self {
        let cpu_millis = cpu_before
            .zip(cpu_nanos())
            .map(|(before, after)| after.saturating_sub(before) / 1_000_000);
        Self {
            wall_millis: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            cpu_millis,
            peak_kib: peak_kib(),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "wall_millis": self.wall_millis,
            "cpu_millis": self.cpu_millis,
            "cpu": if self.cpu_millis.is_some() { "this thread's CPU time (/proc/thread-self/schedstat)" } else { "not measured" },
            "peak_memory_kib": self.peak_kib,
            "peak_memory": if self.peak_kib.is_some() { "the process's peak resident set (VmHWM), not search alone" } else { "not measured" },
        })
    }

    fn text(&self) -> String {
        let cpu = self.cpu_millis.map_or_else(
            || "cpu not measured".to_owned(),
            |ms| format!("cpu {ms} ms"),
        );
        let memory = self.peak_kib.map_or_else(
            || "peak memory not measured".to_owned(),
            |kib| format!("peak memory {kib} KiB (process)"),
        );
        format!("resources: wall {} ms, {cpu}, {memory}\n", self.wall_millis)
    }
}

/// The current thread's CPU time in nanoseconds (Linux; none elsewhere).
fn cpu_nanos() -> Option<u64> {
    let text = fs::read_to_string("/proc/thread-self/schedstat").ok()?;
    text.split_whitespace().next()?.parse().ok()
}

/// The process's peak resident set in KiB (Linux; none elsewhere).
fn peak_kib() -> Option<u64> {
    let text = fs::read_to_string("/proc/self/status").ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
}

/// The command that applies the top-ranked neighbor, when it passes more
/// public cases than the seed.
fn next_step(
    seed: &Seed,
    public: &str,
    seed_run: &Evaluation,
    generated: &Generated,
    ranked: &[usize],
) -> String {
    let quoted = |text: &str| {
        if !text.is_empty()
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c))
        {
            text.to_owned()
        } else {
            format!("'{}'", text.replace('\'', "'\\''"))
        }
    };
    let on = seed
        .on
        .as_ref()
        .map_or_else(String::new, |on| format!("--on {on} "));
    let by_hand = format!(
        "repair by hand: sley-agent try {on}'{{\"af1\": 1, \"edit\": [...]}}' --public {}",
        quoted(public)
    );
    let Some(top) = ranked
        .first()
        .map(|position| &generated.neighbors[*position])
    else {
        return format!("no neighbor was evaluated; {by_hand}");
    };
    if top.passed() <= seed_run.passed() {
        return format!(
            "no neighbor passes more public cases than the seed ({}/{}); {by_hand}",
            seed_run.passed(),
            seed_run.outcomes.len()
        );
    }
    let frame = quoted(&top.frame.to_string());
    format!("sley-agent try {on}{frame} --public {}", quoted(public))
}

/// Everything the outputs report.
struct Summary<'a> {
    request: &'a Request<'a>,
    function: &'a str,
    seed: &'a Seed,
    file: &'a CaseFile,
    for_function: usize,
    seed_run: &'a Evaluation,
    generated: &'a Generated,
    ranked: &'a [usize],
    counts: &'a Counts,
    wall_reached: bool,
    resources: &'a Resources,
    next: &'a str,
}

impl Summary<'_> {
    fn limits_text(&self) -> String {
        let mut reached = Vec::new();
        if self.generated.more {
            reached.push(format!(
                "neighbor limit reached: generation stopped at {} neighbors, more exist",
                self.request.max_neighbors
            ));
        }
        if self.wall_reached {
            let mut text = format!("wall limit reached ({} ms)", self.request.max_millis);
            if self.generated.wall {
                text.push_str(": generation stopped");
            }
            let _ = write!(
                text,
                "; {} of {} neighbors evaluated, {} partial, {} not evaluated",
                self.counts.evaluated,
                self.counts.generated,
                self.counts.partial,
                self.counts.not_evaluated
            );
            if !self.seed_run.complete {
                text.push_str("; the seed's cases did not all run");
            }
            reached.push(text);
        }
        let bounds = format!(
            "{} neighbors, {} ms; case fuel: seed {SEED_FUEL}, neighbors {NEIGHBOR_FUEL_FACTOR}x the seed's within {NEIGHBOR_FUEL_FLOOR}..{SEED_FUEL}",
            self.request.max_neighbors, self.request.max_millis
        );
        if reached.is_empty() {
            format!("limits: none reached ({bounds})\n")
        } else {
            format!("limits: {} ({bounds})\n", reached.join("; "))
        }
    }

    fn neighbor_json(&self, neighbor: &Neighbor) -> Value {
        json!({
            "index": neighbor.index,
            "rank": neighbor.rank,
            "generator": neighbor.generator.name(),
            "size": neighbor.generator.size(),
            "at": neighbor.at,
            "change": neighbor.change,
            "pointer": neighbor.pointer,
            "frame": neighbor.frame,
            "status": neighbor.status.label(),
            "kernel": neighbor.kernel,
            "public": neighbor.evaluation.as_ref().map(|evaluation| evaluation.to_json(&self.file.cases)),
        })
    }

    fn json(&self) -> Value {
        let mut order: Vec<usize> = self.ranked.to_vec();
        order.extend(
            (0..self.generated.neighbors.len()).filter(|position| !self.ranked.contains(position)),
        );
        let neighbors: Vec<Value> = order
            .iter()
            .map(|position| self.neighbor_json(&self.generated.neighbors[*position]))
            .collect();
        json!({
            "search": {"fn": self.function, "seed": self.seed.label, "lineage": self.seed.lineage},
            "public": {"file": self.request.public, "sha256": self.file.sha256,
                       "cases": self.file.cases.len(), "for_fn": self.for_function,
                       "unusable": self.file.unusable},
            "seed": {"public": self.seed_run.to_json(&self.file.cases)},
            "generators": Generator::ALL.iter().map(|generator| generator.name()).collect::<Vec<_>>(),
            "rule": RULE,
            "counts": {"generated": self.counts.generated, "valid": self.counts.valid,
                       "refused": self.counts.refused, "evaluated": self.counts.evaluated,
                       "partial": self.counts.partial, "not_evaluated": self.counts.not_evaluated,
                       "skipped": self.counts.skipped, "duplicates": self.counts.duplicates},
            "limits": {"max_neighbors": self.request.max_neighbors, "max_millis": self.request.max_millis,
                       "neighbor_limit_reached": self.generated.more, "wall_limit_reached": self.wall_reached,
                       "searches_per_seed": SEARCHES_PER_SEED,
                       "case_fuel": {"seed": SEED_FUEL, "neighbor_factor": NEIGHBOR_FUEL_FACTOR,
                                     "neighbor_floor": NEIGHBOR_FUEL_FLOOR, "neighbor_ceiling": SEED_FUEL}},
            "neighbors": neighbors,
            "resources": self.resources.to_json(),
            "claim": CLAIM,
            "next": self.next,
        })
    }

    fn text(&self) -> String {
        let counts = self.counts;
        let seed = self.seed_run;
        let mut text = format!(
            "search {} from {}: the seed passes {}/{} public cases",
            self.function,
            self.seed.label,
            seed.passed(),
            seed.outcomes.len()
        );
        text.push_str(&seed.partial(&self.file.cases));
        text.push_str(&seed.misses(&self.file.cases));
        text.push('\n');
        let _ = write!(
            text,
            "public: {}, sha256 {}, {} case(s), {} for {}",
            self.request.public,
            &self.file.sha256[..12],
            self.file.cases.len(),
            self.for_function,
            self.function
        );
        if self.file.unusable > 0 {
            let _ = write!(text, ", {} unusable entries not run", self.file.unusable);
        }
        text.push('\n');
        let _ = write!(
            text,
            "neighbors: {} generated, {} kernel-valid, {} refused, {} evaluated",
            counts.generated, counts.valid, counts.refused, counts.evaluated
        );
        if counts.partial > 0 {
            let _ = write!(text, ", {} partial", counts.partial);
        }
        if counts.not_evaluated > 0 {
            let _ = write!(text, ", {} not evaluated", counts.not_evaluated);
        }
        if counts.skipped > 0 {
            let _ = write!(
                text,
                "; {} change(s) skipped: no frame layered on the seed states them",
                counts.skipped
            );
        }
        text.push('\n');
        let _ = writeln!(text, "ranking: {RULE}");
        for position in self.ranked.iter().take(SHOWN) {
            let neighbor = &self.generated.neighbors[*position];
            let evaluation = neighbor
                .evaluation
                .as_ref()
                .map_or_else(String::new, |e| e.text(&self.file.cases));
            let _ = writeln!(
                text,
                "{:>2}. #{} {} at {}: {} (size {}): public {evaluation}\n    {}",
                neighbor.rank.unwrap_or(0),
                neighbor.index,
                neighbor.generator.name(),
                neighbor.at,
                neighbor.change,
                neighbor.generator.size(),
                neighbor.frame
            );
        }
        if self.ranked.len() > SHOWN {
            let _ = writeln!(
                text,
                "shown: {SHOWN} of {} ranked; --json lists every neighbor with its case results",
                self.ranked.len()
            );
        }
        text.push_str(&self.limits_text());
        text.push_str(&self.resources.text());
        let _ = writeln!(text, "verified: {CLAIM}");
        let _ = writeln!(text, "next: {}", self.next);
        text
    }
}

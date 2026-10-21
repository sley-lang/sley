//! The embed API: compile a familiar (Python-dialect) program in memory and
//! call its functions under an exact fuel limit (`docs/EMBEDDING.md`).
//!
//! [`compile_python`] takes the path of `sley-agent try --familiar`: the
//! familiar frontend parses the text into a structured frame, the frame
//! compiler and the candidate assembler build a candidate over a fresh
//! genesis held in memory, and the kernel's in-process candidate validation
//! judges it. Only a Valid candidate becomes a [`Module`]. Nothing touches a
//! file, a clock or the operating system's randomness: every input that
//! would otherwise vary comes from [`EmbedOptions`], so the same text and
//! options give the same program, digest, fuel and instruction counts on
//! every machine, natively and in `wasm32-unknown-unknown`.
//!
//! [`Module::call`] runs one function on the reference VM with
//! `max_fuel` set to the caller's limit (the other limits are
//! [`crate::exec::call_limits`]). Each function is lowered and loaded once
//! when the module is built; a call admits its inputs and executes.
//!
//! Results are advisory execution, as for `sley-agent call`: never
//! admission evidence, never signed, never committed.

use std::collections::BTreeMap;
use std::fmt;

use sha2::{Digest as _, Sha256};
use sley_id::{CandidateNonce, EntityId};
use sley_policy::PolicyResourceCeilings;
use sley_ssmc::{ConstData, ConstValue, TypeExpr};
use sley_vm::{ExecutionLimits, ExecutionTermination, ResourceKind};

use crate::candidate::{self, Authority, StageInputs};
use crate::catalog::Verdict;
use crate::error::{AgentError, AgentErrorCode};
use crate::exec::{self, Executor};
use crate::familiar::{self, Positions};
use crate::genesis;
use crate::locate::Source;
use crate::names::{NameMap, Names};

/// Every input of nondeterminism in a compilation. The same source and the
/// same options give the same [`Module::program_digest`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EmbedOptions {
    /// Derives the genesis nonce (the workspace identity), the candidate
    /// nonce (created entity identities) and the fresh member identities.
    pub seed: u64,
    /// The candidate validation clock (Unix milliseconds). Candidates made
    /// here never expire before 2100, so it changes no verdict.
    pub now_unix_millis: u64,
}

/// A refusal: a stable `AGENT_*` symbol, a message, and the line and column
/// of the familiar text it concerns when one is known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbedError {
    /// The stable symbol (`AGENT_FRAME_INVALID`, `AGENT_CANDIDATE_INVALID`,
    /// `AGENT_NAME_UNKNOWN`, `AGENT_INPUT_INVALID`,
    /// `AGENT_EXECUTION_REFUSED`, ...).
    pub code: String,
    /// What is wrong, as the workbench states it.
    pub message: String,
    /// Line in the source, from 1.
    pub line: Option<u32>,
    /// Column in the source (in characters), from 1.
    pub column: Option<u32>,
}

impl EmbedError {
    fn new(code: AgentErrorCode, message: impl Into<String>) -> Self {
        Self {
            code: code.symbol().to_owned(),
            message: message.into(),
            line: None,
            column: None,
        }
    }

    fn of(error: &AgentError) -> Self {
        Self::new(error.code(), error.detail())
    }

    fn at(mut self, position: Option<familiar::Pos>) -> Self {
        if let Some(position) = position {
            self.line = Some(saturate(position.line));
            self.column = Some(saturate(position.column));
        }
        self
    }
}

fn saturate(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

impl fmt::Display for EmbedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(column)) => write!(
                formatter,
                "{} at line {line}, column {column}: {}",
                self.code, self.message
            ),
            _ => write!(formatter, "{}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for EmbedError {}

/// A value passed to or returned from a function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmbedValue {
    /// An integer of any declared width it fits (`int` is `i64`).
    Int(i64),
    /// A boolean.
    Bool(bool),
    /// Text.
    Str(String),
    /// A list of integers (`list[int]`), the cheap form for large inputs.
    IntList(Vec<i64>),
    /// Any other list (or a tuple).
    List(Vec<EmbedValue>),
}

/// How a call ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallResult {
    /// The function returned.
    Value(EmbedValue),
    /// The program trapped: `unreachable`, `resource_exhausted`,
    /// `adapter_contract_violation`, `internal_invariant`, or `trap_<tag>`.
    Trap {
        /// The trap's name.
        symbol: String,
    },
    /// The call reached its fuel limit.
    OutOfFuel,
    /// The call reached another execution limit.
    LimitExceeded {
        /// Which: `Instruction`, `ValueUnits`, `OutputUnits`, `CallDepth`
        /// or `Cancelled`.
        what: String,
    },
}

/// One call's result and exact resource use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallOutcome {
    /// How the call ended.
    pub result: CallResult,
    /// Fuel charged (at most the limit).
    pub fuel_used: u64,
    /// Instructions executed.
    pub instructions: u64,
}

struct Function {
    id: EntityId,
    parameters: Vec<TypeExpr>,
}

/// A compiled, Valid program with every function lowered and loaded.
pub struct Module {
    executor: Executor,
    functions: BTreeMap<String, Function>,
    digest: [u8; 32],
}

impl fmt::Debug for Module {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Module")
            .field("functions", &self.functions.keys().collect::<Vec<_>>())
            .field("digest", &crate::hex::encode(&self.digest))
            .finish_non_exhaustive()
    }
}

/// Domain of every value derived from [`EmbedOptions::seed`].
const SEED_DOMAIN: &[u8] = b"sley.agent.embed.seed.v1\0";

/// Thirty-two bytes for `purpose` (and `counter`) from the seed.
fn derived(seed: u64, purpose: &[u8], counter: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SEED_DOMAIN);
    hasher.update(purpose);
    hasher.update([0]);
    hasher.update(seed.to_le_bytes());
    hasher.update(counter.to_le_bytes());
    hasher.finalize().into()
}

/// Compiles familiar text (the Python dialect when its first statement is a
/// `def`) as `sley-agent try --familiar` does over a fresh workspace, and
/// keeps the program only when the kernel judges the candidate Valid.
///
/// # Errors
///
/// `AGENT_FRAME_INVALID` (with line and column) for text the frontend or the
/// frame compiler refuses, `AGENT_CANDIDATE_INVALID` for a candidate the
/// kernel refuses (with the authored line and column when the refusal
/// locates one), and the workbench's other refusals as they arise.
pub fn compile_python(source: &str, options: &EmbedOptions) -> Result<Module, EmbedError> {
    compile_python_with(source, options, genesis::INIT_CEILINGS)
}

/// The largest grant an embedded program can use: [`genesis::INIT_CEILINGS`]
/// with the mutation ceiling raised to the kernel's full-v1 operation limit
/// (65,535). Each created entity is one mutation, and one function of a
/// few dozen Python lines creates thousands (a 25-line board scan creates
/// about 3,000), so the `init` grant's 10,000 admits only small programs.
pub const LARGE_CEILINGS: PolicyResourceCeilings = PolicyResourceCeilings::new(
    genesis::INIT_CEILINGS.max_fuel,
    genesis::INIT_CEILINGS.max_memory_bytes,
    genesis::INIT_CEILINGS.max_output_bytes,
    genesis::INIT_CEILINGS.max_effect_count,
    65_535,
    genesis::INIT_CEILINGS.max_adapter_calls,
);

/// [`compile_python`] over a genesis whose policy grants `ceilings` instead
/// of the `init` defaults (for example [`LARGE_CEILINGS`]). The grant is
/// part of the program's identity: it changes [`Module::program_digest`].
/// Its fuel ceiling bounds only `TestCases`, never [`Module::call`].
///
/// # Errors
///
/// As [`compile_python`]; `AGENT_WORKSPACE_INVALID` for ceilings the policy
/// refuses (a zero ceiling, for example).
pub fn compile_python_with(
    source: &str,
    options: &EmbedOptions,
    ceilings: PolicyResourceCeilings,
) -> Result<Module, EmbedError> {
    let parsed = familiar::parse(source).map_err(|error| {
        EmbedError::new(AgentErrorCode::FrameInvalid, error.message).at(Some(error.at))
    })?;
    let frame = parsed.frame;
    let positions = parsed.positions;
    let head = genesis::genesis_head(derived(options.seed, b"genesis", 0), ceilings)
        .map_err(|error| EmbedError::of(&error))?;
    let authority = Authority::of(&head).map_err(|error| EmbedError::of(&error))?;
    let names = Names::build(head.program(), &NameMap::default());
    let mut counter = 0_u64;
    let mut random = || {
        counter += 1;
        Ok(derived(options.seed, b"member", counter))
    };
    let mut now = || options.now_unix_millis;
    let inputs = StageInputs {
        nonce: CandidateNonce::from_bytes(derived(options.seed, b"nonce", 0)),
        random: &mut random,
        now: &mut now,
    };
    let (compiled, imported, output) =
        candidate::stage(&head, &authority, &names, &frame, inputs, |_| Ok(()))
            .map_err(|error| frame_refusal(&error, &positions))?;
    let Some(program) = candidate::proposed_program(&head, &output) else {
        let program =
            candidate::applied_program(&head, &imported).unwrap_or_else(|| head.program().clone());
        let names = Names::build(&program, &compiled.names);
        let mut verdict = Verdict::of(&output, &program, &names);
        verdict.locate(
            &output,
            &Source::of_try(&frame, &compiled.artifacts),
            &program,
            &names,
        );
        let position = verdict
            .authored
            .as_ref()
            .and_then(|authored| authored.items.first())
            .and_then(|(at, _)| positions.locate(at));
        let mut message = verdict.headline();
        if let Some(location) = &verdict.location {
            message = format!("{message}; where: {location}");
        }
        if let Some(hint) = &verdict.hint {
            message = format!("{message}; hint: {hint}");
        }
        return Err(EmbedError::new(AgentErrorCode::CandidateInvalid, message).at(position));
    };
    let names = Names::build(&program, &compiled.names);
    let mut executor = Executor::new(&program).map_err(|error| EmbedError::of(&error))?;
    let mut functions = BTreeMap::new();
    for (name, id) in names.iter() {
        if executor.function(id).is_none() {
            continue;
        }
        executor.prepare(id).map_err(|error| {
            EmbedError::new(error.code(), format!("`{name}`: {}", error.detail()))
        })?;
        functions.insert(
            name.clone(),
            Function {
                id: *id,
                parameters: executor.parameter_types(id),
            },
        );
    }
    Ok(Module {
        executor,
        functions,
        digest: *program.root().as_bytes(),
    })
}

/// A frame refusal at the familiar text position of its first located
/// pointer (`[AGENT_X] /pointer: detail` lines).
fn frame_refusal(error: &AgentError, positions: &Positions) -> EmbedError {
    let position = crate::frame::problem_lines(error).iter().find_map(|line| {
        let rest = line
            .strip_prefix('[')
            .and_then(|rest| rest.split_once("] "))
            .map_or(line.as_str(), |(_, rest)| rest);
        let (pointer, _) = rest.split_once(": ")?;
        (pointer.starts_with('/') && !pointer.contains(' '))
            .then(|| positions.locate(pointer))
            .flatten()
    });
    EmbedError::of(error).at(position)
}

impl Module {
    /// The names of the program's functions, in name order.
    #[must_use]
    pub fn functions(&self) -> Vec<String> {
        self.functions.keys().cloned().collect()
    }

    /// The compiled program's state root: its content identity under the
    /// options it was compiled with.
    #[must_use]
    pub const fn program_digest(&self) -> [u8; 32] {
        self.digest
    }

    /// Calls `function` with `args` under `fuel_limit` fuel.
    ///
    /// # Errors
    ///
    /// `AGENT_NAME_UNKNOWN` for a function the program does not have,
    /// `AGENT_INPUT_INVALID` for arguments that do not fit its parameters,
    /// `AGENT_EXECUTION_REFUSED` when the VM refuses the call or the result
    /// has no [`EmbedValue`] form. Traps and limits are outcomes.
    pub fn call(
        &mut self,
        function: &str,
        args: &[EmbedValue],
        fuel_limit: u64,
    ) -> Result<CallOutcome, EmbedError> {
        let entry = self.functions.get(function).ok_or_else(|| {
            EmbedError::new(
                AgentErrorCode::NameUnknown,
                format!("no function named `{function}`"),
            )
        })?;
        if entry.parameters.len() != args.len() {
            return Err(EmbedError::new(
                AgentErrorCode::InputInvalid,
                format!(
                    "{function} takes {} argument(s), got {}",
                    entry.parameters.len(),
                    args.len()
                ),
            ));
        }
        let inputs = args
            .iter()
            .zip(&entry.parameters)
            .enumerate()
            .map(|(index, (arg, ty))| {
                to_const(arg, ty).map_err(|detail| {
                    EmbedError::new(
                        AgentErrorCode::InputInvalid,
                        format!("arg {index}: {detail}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let limits = ExecutionLimits {
            max_fuel: fuel_limit,
            ..exec::call_limits()
        };
        let id = entry.id;
        let outcome = self
            .executor
            .run(&id, inputs, limits)
            .map_err(|error| EmbedError::of(&error))?;
        let result = match outcome.termination {
            ExecutionTermination::Success(value) => CallResult::Value(
                from_const(&value)
                    .map_err(|detail| EmbedError::new(AgentErrorCode::ExecutionRefused, detail))?,
            ),
            ExecutionTermination::Trap { trap_tag, .. } => CallResult::Trap {
                symbol: trap_symbol(trap_tag),
            },
            ExecutionTermination::ResourceLimit(ResourceKind::Fuel) => CallResult::OutOfFuel,
            ExecutionTermination::ResourceLimit(kind) => CallResult::LimitExceeded {
                what: format!("{kind:?}"),
            },
            ExecutionTermination::Cancelled => CallResult::LimitExceeded {
                what: "Cancelled".to_owned(),
            },
            ExecutionTermination::InternalInvariant => CallResult::Trap {
                symbol: "internal_invariant".to_owned(),
            },
        };
        Ok(CallOutcome {
            result,
            fuel_used: outcome.fuel,
            instructions: outcome.instructions,
        })
    }
}

/// The name of a trap tag, as expectations write it (`exec::trap_code`).
fn trap_symbol(tag: u32) -> String {
    [
        "unreachable",
        "resource_exhausted",
        "adapter_contract_violation",
        "internal_invariant",
    ]
    .into_iter()
    .find(|name| exec::trap_code(name) == Some(tag))
    .map_or_else(|| format!("trap_{tag}"), str::to_owned)
}

/// An integer of `ty`, when `ty` is an integer type it fits.
fn integer(value: i64, ty: &TypeExpr) -> Result<ConstData, String> {
    match ty {
        TypeExpr::SInt(width) => {
            let bits = u32::from(width.bits());
            if bits < 64 {
                let half = 1_i64 << (bits - 1);
                if value < -half || value >= half {
                    return Err(format!("{value} does not fit i{bits}"));
                }
            }
            Ok(ConstData::SInt(i128::from(value)))
        }
        TypeExpr::UInt(width) => {
            let bits = u32::from(width.bits());
            let fits = u64::try_from(value)
                .ok()
                .filter(|value| bits >= 64 || *value < (1_u64 << bits));
            fits.map(|value| ConstData::UInt(u128::from(value)))
                .ok_or_else(|| format!("{value} does not fit u{bits}"))
        }
        _ => Err(format!("expected {}, got an integer", type_name(ty))),
    }
}

/// Converts one argument against its declared type.
fn to_const(value: &EmbedValue, ty: &TypeExpr) -> Result<ConstValue, String> {
    let data = match (value, ty) {
        (EmbedValue::Int(number), _) => integer(*number, ty)?,
        (EmbedValue::Bool(flag), TypeExpr::Bool) => ConstData::Bool(*flag),
        (EmbedValue::Str(text), TypeExpr::Text) => ConstData::Text(text.clone()),
        (EmbedValue::IntList(items), TypeExpr::Vector(item)) => ConstData::Sequence(
            items
                .iter()
                .map(|number| {
                    Ok(ConstValue {
                        value_type: (**item).clone(),
                        data: integer(*number, item)?,
                    })
                })
                .collect::<Result<_, String>>()?,
        ),
        (EmbedValue::IntList(items), TypeExpr::Tuple(types)) if items.len() == types.len() => {
            ConstData::Sequence(
                items
                    .iter()
                    .zip(types)
                    .map(|(number, ty)| {
                        Ok(ConstValue {
                            value_type: ty.clone(),
                            data: integer(*number, ty)?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
            )
        }
        (EmbedValue::List(items), TypeExpr::Vector(item)) => ConstData::Sequence(
            items
                .iter()
                .map(|value| to_const(value, item))
                .collect::<Result<_, _>>()?,
        ),
        (EmbedValue::List(items), TypeExpr::Tuple(types)) if items.len() == types.len() => {
            ConstData::Sequence(
                items
                    .iter()
                    .zip(types)
                    .map(|(value, ty)| to_const(value, ty))
                    .collect::<Result<_, _>>()?,
            )
        }
        _ => return Err(format!("expected {}", type_name(ty))),
    };
    Ok(ConstValue {
        value_type: ty.clone(),
        data,
    })
}

/// Converts a result.
fn from_const(value: &ConstValue) -> Result<EmbedValue, String> {
    let wide = |number: i128| {
        i64::try_from(number).map_err(|_| format!("the result {number} does not fit i64"))
    };
    Ok(match &value.data {
        ConstData::SInt(number) => EmbedValue::Int(wide(*number)?),
        ConstData::UInt(number) => EmbedValue::Int(
            i64::try_from(*number).map_err(|_| format!("the result {number} does not fit i64"))?,
        ),
        ConstData::Bool(flag) => EmbedValue::Bool(*flag),
        ConstData::Text(text) => EmbedValue::Str(text.clone()),
        ConstData::Sequence(items) => match &value.value_type {
            TypeExpr::Vector(item) if matches!(**item, TypeExpr::SInt(_) | TypeExpr::UInt(_)) => {
                EmbedValue::IntList(
                    items
                        .iter()
                        .map(|item| match from_const(item)? {
                            EmbedValue::Int(number) => Ok(number),
                            _ => Err("a list[int] element is not an integer".to_owned()),
                        })
                        .collect::<Result<_, String>>()?,
                )
            }
            _ => EmbedValue::List(items.iter().map(from_const).collect::<Result<_, _>>()?),
        },
        _ => {
            return Err(format!(
                "the result type {} has no embed form (int, bool, str or lists of them)",
                type_name(&value.value_type)
            ));
        }
    })
}

/// A short spelling of a type for refusals.
fn type_name(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Unit => "unit".to_owned(),
        TypeExpr::Bool => "bool".to_owned(),
        TypeExpr::SInt(width) => format!("i{}", width.bits()),
        TypeExpr::UInt(width) => format!("u{}", width.bits()),
        TypeExpr::Text => "str".to_owned(),
        TypeExpr::Bytes => "bytes".to_owned(),
        TypeExpr::Vector(item) => format!("list[{}]", type_name(item)),
        TypeExpr::Tuple(items) => format!(
            "tuple[{}]",
            items.iter().map(type_name).collect::<Vec<_>>().join(", ")
        ),
        _ => "a structured type".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTIONS: EmbedOptions = EmbedOptions {
        seed: 7,
        now_unix_millis: 1_760_000_000_000,
    };

    const BOT: &str = "\
def act(board: list[int], info: list[int], notes: list[int]) -> int:
    total = 0
    for cell in board:
        total += cell
    return total + info[0] + len(notes)
";

    #[test]
    fn compiles_and_calls() {
        let mut module = compile_python(BOT, &OPTIONS).unwrap();
        assert!(module.functions().contains(&"act".to_owned()), "{module:?}");
        let outcome = module
            .call(
                "act",
                &[
                    EmbedValue::IntList(vec![1, 2, 3]),
                    EmbedValue::IntList(vec![10]),
                    EmbedValue::IntList(vec![0; 8]),
                ],
                1_000_000,
            )
            .unwrap();
        assert_eq!(outcome.result, CallResult::Value(EmbedValue::Int(24)));
        assert!(outcome.fuel_used > 0 && outcome.instructions > 0);
    }

    #[test]
    fn refusals_carry_line_and_column() {
        let error = compile_python(
            "def act(x: int) -> int:\n    y = x\n    return y $ 2\n",
            &OPTIONS,
        )
        .unwrap_err();
        assert_eq!(error.code, "AGENT_FRAME_INVALID", "{error}");
        assert_eq!(error.line, Some(3), "{error}");
        assert!(error.column.is_some(), "{error}");
        // A type error the frame compiler finds, located in the text.
        let error =
            compile_python("def act(x: int) -> int:\n    return \"text\"\n", &OPTIONS).unwrap_err();
        assert!(error.code.starts_with("AGENT_"), "{error}");
        assert_eq!(error.line, Some(2), "{error}");
    }

    #[test]
    fn small_fuel_limits_run_out() {
        let mut module = compile_python(BOT, &OPTIONS).unwrap();
        let args = [
            EmbedValue::IntList(vec![1; 2560]),
            EmbedValue::IntList(vec![64]),
            EmbedValue::IntList(vec![0; 8]),
        ];
        let full = module.call("act", &args, 100_000_000).unwrap();
        assert_eq!(
            full.result,
            CallResult::Value(EmbedValue::Int(2560 + 64 + 8))
        );
        let starved = module.call("act", &args, full.fuel_used / 2).unwrap();
        assert_eq!(starved.result, CallResult::OutOfFuel);
        assert!(starved.fuel_used <= full.fuel_used / 2);
        let exact = module.call("act", &args, full.fuel_used).unwrap();
        assert_eq!(exact, full);
    }

    #[test]
    fn traps_and_bad_calls() {
        let mut module = compile_python(
            "def pick(xs: list[int], i: int) -> int:\n    return xs[i]\n",
            &OPTIONS,
        )
        .unwrap();
        let outcome = module
            .call(
                "pick",
                &[EmbedValue::IntList(vec![1, 2]), EmbedValue::Int(5)],
                1_000_000,
            )
            .unwrap();
        assert!(
            matches!(outcome.result, CallResult::Trap { .. }),
            "{outcome:?}"
        );
        let unknown = module.call("nope", &[], 1_000).unwrap_err();
        assert_eq!(unknown.code, "AGENT_NAME_UNKNOWN");
        let arity = module.call("pick", &[], 1_000).unwrap_err();
        assert_eq!(arity.code, "AGENT_INPUT_INVALID");
        let typed = module
            .call("pick", &[EmbedValue::Bool(true), EmbedValue::Int(0)], 1_000)
            .unwrap_err();
        assert_eq!(typed.code, "AGENT_INPUT_INVALID");
    }

    #[test]
    fn larger_programs_need_the_larger_grant() {
        let heavy = "\
def h(board: list[int], info: list[int], notes: list[int]) -> int:
    w = info[0]
    h = info[1]
    best = 0
    for d in range(4):
        score = 0
        for yy in range(h):
            for xx in range(w):
                c = board[yy * w + xx]
                if c & 7 == 0 and abs(xx - d) + abs(yy - d) <= 6:
                    score += 1
        best = max(best, score)
    return best
";
        let mut source = String::new();
        for k in 0..12 {
            source.push_str(&heavy.replace("def h(", &format!("def h{k}(")));
            source.push('\n');
        }
        source.push_str(
            "def act(board: list[int], info: list[int], notes: list[int]) -> int:\n    return h0(board, info, notes) + h11(board, info, notes)\n",
        );
        let error = compile_python(&source, &OPTIONS).unwrap_err();
        assert_eq!(error.code, "AGENT_CANDIDATE_INVALID", "{error}");
        assert!(error.message.contains("max_mutation_count"), "{error}");
        let mut module = compile_python_with(&source, &OPTIONS, LARGE_CEILINGS).unwrap();
        assert_eq!(module.functions().len(), 13, "{module:?}");
        let outcome = module
            .call(
                "act",
                &[
                    EmbedValue::IntList(vec![0; 6]),
                    EmbedValue::IntList(vec![3, 2]),
                    EmbedValue::IntList(vec![0; 8]),
                ],
                10_000_000,
            )
            .unwrap();
        assert_eq!(outcome.result, CallResult::Value(EmbedValue::Int(12)));
    }

    #[test]
    fn compilation_is_deterministic() {
        let first = compile_python(BOT, &OPTIONS).unwrap();
        let second = compile_python(BOT, &OPTIONS).unwrap();
        assert_eq!(first.program_digest(), second.program_digest());
        let later = compile_python(
            BOT,
            &EmbedOptions {
                now_unix_millis: OPTIONS.now_unix_millis + 1,
                ..OPTIONS
            },
        )
        .unwrap();
        assert_eq!(first.program_digest(), later.program_digest());
        let reseeded = compile_python(BOT, &EmbedOptions { seed: 8, ..OPTIONS }).unwrap();
        assert_ne!(first.program_digest(), reseeded.program_digest());
        let args = [
            EmbedValue::IntList((0..2560).collect()),
            EmbedValue::IntList(vec![3; 80]),
            EmbedValue::IntList(vec![0; 8]),
        ];
        let (mut first, mut second) = (first, second);
        let a = first.call("act", &args, 4_000_000).unwrap();
        let b = second.call("act", &args, 4_000_000).unwrap();
        assert_eq!(a, b);
        let c = first.call("act", &args, 4_000_000).unwrap();
        assert_eq!(a, c);
    }
}

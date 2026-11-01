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
use sley_mutate::value::EntityBodyValue;
use sley_policy::PolicyResourceCeilings;
use sley_ssmc::{
    BuiltinFailureValue, ConstData, ConstValue, FieldConst, MapEntryConst, MemberId, NamedType,
    RecordConst, ResultConst, TypeDefForm, TypeExpr, VariantConst,
};
use sley_vm::{ExecutionLimits, ExecutionTermination, ResourceKind};

use crate::candidate::{self, Authority, StageInputs};
use crate::catalog::Verdict;
use crate::error::{AgentError, AgentErrorCode};
use crate::exec::{self, Executor};
use crate::familiar::{self, Positions};
use crate::genesis;
use crate::locate::Source;
use crate::names::{NameMap, Names};
use crate::values::{failure_case, failure_cases, failure_code, substitute};
use crate::workspace::Program;

pub mod bindgen;
mod convert;

pub use convert::{FromEmbed, ToEmbed};

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

/// A value passed to or returned from a function. New forms may be added,
/// so a `match` on one needs a wildcard arm.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EmbedValue {
    /// Unit.
    Unit,
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
    /// A record (a Python record class or a familiar `struct`): its fields
    /// by name. A result lists them in declaration order; an argument may
    /// give them in any order, each field once.
    Record(Vec<(String, EmbedValue)>),
    /// A case of a variant type (a familiar `enum`, or a built-in failure
    /// such as `ArithmeticError`), with its payload when the case has one.
    Variant {
        /// The case's name.
        case: String,
        /// The case's payload.
        payload: Option<Box<EmbedValue>>,
    },
    /// An optional value (`Option<T>`).
    Option(Option<Box<EmbedValue>>),
    /// A success or failure value (`Result<T, E>`).
    Result(Result<Box<EmbedValue>, Box<EmbedValue>>),
    /// An ordered map (`Map<K, V>`): its entries. A result lists them in
    /// the kernel's key order; an argument may give them in any order, each
    /// key once.
    Map(Vec<(EmbedValue, EmbedValue)>),
}

impl EmbedValue {
    /// A record from `(field, value)` pairs.
    pub fn record<N: Into<String>>(fields: impl IntoIterator<Item = (N, Self)>) -> Self {
        Self::Record(
            fields
                .into_iter()
                .map(|(name, value)| (name.into(), value))
                .collect(),
        )
    }

    /// A case without a payload.
    pub fn case(case: impl Into<String>) -> Self {
        Self::Variant {
            case: case.into(),
            payload: None,
        }
    }

    /// A record's field by name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Self> {
        match self {
            Self::Record(fields) => fields
                .iter()
                .find(|(field, _)| field == name)
                .map(|(_, value)| value),
            _ => None,
        }
    }
}

/// A type in a [`Signature`] or a [`TypeDeclaration`].
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EmbedType {
    /// Unit.
    Unit,
    /// A boolean.
    Bool,
    /// An integer of `bits` bits (`int` is signed, 64 bits).
    Int {
        /// Whether it is signed.
        signed: bool,
        /// Its width.
        bits: u16,
    },
    /// Text.
    Str,
    /// A list (`list[T]`).
    List(Box<EmbedType>),
    /// A tuple.
    Tuple(Vec<EmbedType>),
    /// `Option<T>`.
    Option(Box<EmbedType>),
    /// `Result<T, E>`.
    Result {
        /// The success type.
        ok: Box<EmbedType>,
        /// The failure type.
        err: Box<EmbedType>,
    },
    /// `Map<K, V>`.
    Map {
        /// The key type.
        key: Box<EmbedType>,
        /// The value type.
        value: Box<EmbedType>,
    },
    /// A record or variant type, by the name [`Module::types`] declares it
    /// under.
    Named(String),
    /// A type with no [`EmbedValue`] form (floats, bytes, function
    /// references, ...), as Sley spells it.
    Unsupported(String),
}

/// A function's parameters and result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature {
    /// The function's name.
    pub name: String,
    /// Its parameters in order: name and type.
    pub parameters: Vec<(String, EmbedType)>,
    /// Its result type.
    pub result: EmbedType,
}

/// The members of a declared type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeShape {
    /// A record's fields in declaration order.
    Record(Vec<(String, EmbedType)>),
    /// A variant type's cases in declaration order, each with its payload
    /// type when it has one.
    Variant(Vec<(String, Option<EmbedType>)>),
}

/// A record or variant type that a [`Signature`] uses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDeclaration {
    /// The type's name.
    pub name: String,
    /// Its members.
    pub shape: TypeShape,
}

/// How a call ended. `T` is the returned value's form: an [`EmbedValue`],
/// or a Rust type of generated bindings ([`CallOutcome::typed`]).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallResult<T = EmbedValue> {
    /// The function returned.
    Value(T),
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
pub struct CallOutcome<T = EmbedValue> {
    /// How the call ended.
    pub result: CallResult<T>,
    /// Fuel charged (at most the limit).
    pub fuel_used: u64,
    /// Instructions executed.
    pub instructions: u64,
}

impl CallOutcome {
    /// The outcome with its returned value converted to `T`, as generated
    /// bindings ([`bindgen`]) return it. Traps and limits carry over.
    ///
    /// # Errors
    ///
    /// `AGENT_EXECUTION_REFUSED` when the value has no `T` form.
    pub fn typed<T: FromEmbed>(self) -> Result<CallOutcome<T>, EmbedError> {
        let result = match self.result {
            CallResult::Value(value) => {
                CallResult::Value(T::from_embed(&value).ok_or_else(|| {
                    EmbedError::new(
                        AgentErrorCode::ExecutionRefused,
                        format!("the result {value:?} does not have the bindings' type"),
                    )
                })?)
            }
            CallResult::Trap { symbol } => CallResult::Trap { symbol },
            CallResult::OutOfFuel => CallResult::OutOfFuel,
            CallResult::LimitExceeded { what } => CallResult::LimitExceeded { what },
        };
        Ok(CallOutcome {
            result,
            fuel_used: self.fuel_used,
            instructions: self.instructions,
        })
    }
}

struct Function {
    id: EntityId,
    parameters: Vec<TypeExpr>,
    /// A Python function that returns a list local: its result is a
    /// `Map<u64,T>` keyed `0..len`, handed back as a list.
    list_result: bool,
    signature: Signature,
}

/// One type definition: its name, form, and each member's leaf name.
struct TypeDef {
    name: String,
    form: TypeDefForm,
    members: Vec<(MemberId, String)>,
}

/// The program's type definitions, as converting values needs them.
struct Types(BTreeMap<EntityId, TypeDef>);

/// A compiled, Valid program with every function lowered and loaded.
pub struct Module {
    executor: Executor,
    functions: BTreeMap<String, Function>,
    types: Types,
    declarations: Vec<TypeDeclaration>,
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
    let types = Types::of(&program, &names);
    let python = familiar::is_python(source);
    let mut executor = Executor::new(&program).map_err(|error| EmbedError::of(&error))?;
    let mut functions = BTreeMap::new();
    let mut used = Vec::new();
    for (name, id) in names.iter() {
        let Some(graph) = executor.function(id) else {
            continue;
        };
        let result_type = graph.result_type.clone();
        let parameter_names: Vec<String> =
            graph.parameters.iter().map(|id| names.leaf(id)).collect();
        executor.prepare(id).map_err(|error| {
            EmbedError::new(error.code(), format!("`{name}`: {}", error.detail()))
        })?;
        let parameters = executor.parameter_types(id);
        let list_result = python && list_item(&result_type).is_some();
        let result = match list_item(&result_type) {
            Some(item) if list_result => EmbedType::List(Box::new(types.describe(item))),
            _ => types.describe(&result_type),
        };
        let signature = Signature {
            name: name.clone(),
            parameters: parameter_names
                .into_iter()
                .zip(&parameters)
                .map(|(name, ty)| (name, types.describe(ty)))
                .collect(),
            result,
        };
        used.extend(parameters.iter().cloned());
        used.push(result_type);
        functions.insert(
            name.clone(),
            Function {
                id: *id,
                parameters,
                list_result,
                signature,
            },
        );
    }
    let declarations = types.declarations(&used);
    Ok(Module {
        executor,
        functions,
        types,
        declarations,
        digest: *program.root().as_bytes(),
    })
}

/// The element type of a `Map<u64,T>`, the storage of a Python list local.
fn list_item(ty: &TypeExpr) -> Option<&TypeExpr> {
    let TypeExpr::OrderedMap { key, value } = ty else {
        return None;
    };
    matches!(**key, TypeExpr::UInt(width) if width.bits() == 64).then_some(&**value)
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

    /// A function's parameters and result, or `None` for a function the
    /// program does not have.
    #[must_use]
    pub fn signature(&self, function: &str) -> Option<&Signature> {
        self.functions.get(function).map(|entry| &entry.signature)
    }

    /// The record and variant types the functions' signatures use,
    /// directly or through other types, in name order.
    #[must_use]
    pub fn types(&self) -> &[TypeDeclaration] {
        &self.declarations
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
                to_const(arg, ty, &self.types, 0).map_err(|detail| {
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
        let (id, list_result) = (entry.id, entry.list_result);
        let outcome = self
            .executor
            .run(&id, inputs, limits)
            .map_err(|error| EmbedError::of(&error))?;
        let result = match outcome.termination {
            ExecutionTermination::Success(value) => CallResult::Value(
                if list_result {
                    self.types.list_result(&value)
                } else {
                    self.types.result_value(&value, 0)
                }
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

impl Module {
    /// Checks that the program has the interface generated bindings were
    /// made for: each of `signatures` is a function's [`Signature`] as it
    /// prints, and each of `types` a [`TypeDeclaration`] of
    /// [`Self::types`]. The program may have more functions and types.
    ///
    /// # Errors
    ///
    /// `AGENT_INPUT_INVALID`, naming the first signature or type that the
    /// program does not have.
    pub fn check_interface(&self, signatures: &[&str], types: &[&str]) -> Result<(), EmbedError> {
        for expected in signatures {
            let name = expected.split('(').next().unwrap_or_default();
            let found = self.signature(name).map(ToString::to_string);
            if found.as_deref() != Some(*expected) {
                let found = found.unwrap_or_else(|| "no such function".to_owned());
                return Err(EmbedError::new(
                    AgentErrorCode::InputInvalid,
                    format!("the bindings expect `{expected}`; the program has {found}"),
                ));
            }
        }
        for expected in types {
            if !self
                .declarations
                .iter()
                .any(|declaration| declaration.to_string() == *expected)
            {
                return Err(EmbedError::new(
                    AgentErrorCode::InputInvalid,
                    format!("the bindings expect `{expected}`, which the program does not declare"),
                ));
            }
        }
        Ok(())
    }

    /// [`Self::call`] with JSON in and out, for a host that has no Rust
    /// types, such as a `wasm32-unknown-unknown` build called from
    /// JavaScript. `args` is a JSON array, one value per parameter, in the
    /// form `sley-agent call` takes: a record as an object, `"Case"` or
    /// `{"Case": payload}`, `"None"` or `{"Some": v}`, `{"Ok": v}` or
    /// `{"Err": e}`, a map as an array of `[key, value]`. The reply is
    /// `{"result": R, "fuel_used": n, "instructions": n}`, where `R` is
    /// `{"value": v}`, `{"trap": "unreachable"}` or `{"limit": "Fuel"}`
    /// (or another limit's name). [`bindgen::typescript`] types both sides.
    ///
    /// # Errors
    ///
    /// As [`Self::call`], and `AGENT_INPUT_INVALID` when `args` is not a
    /// JSON array of values of the parameters' types.
    pub fn call_json(
        &mut self,
        function: &str,
        args: &str,
        fuel_limit: u64,
    ) -> Result<String, EmbedError> {
        let invalid = |detail: String| EmbedError::new(AgentErrorCode::InputInvalid, detail);
        let (parameters, result_type) = self
            .functions
            .get(function)
            .map(|entry| {
                (
                    entry.signature.parameters.clone(),
                    entry.signature.result.clone(),
                )
            })
            .ok_or_else(|| {
                EmbedError::new(
                    AgentErrorCode::NameUnknown,
                    format!("no function named `{function}`"),
                )
            })?;
        let args: serde_json::Value = serde_json::from_str(args)
            .map_err(|error| invalid(format!("the arguments are not JSON: {error}")))?;
        let args = args
            .as_array()
            .ok_or_else(|| invalid("the arguments are a JSON array".to_owned()))?;
        if args.len() != parameters.len() {
            return Err(invalid(format!(
                "{function} takes {} argument(s), got {}",
                parameters.len(),
                args.len()
            )));
        }
        let values = args
            .iter()
            .zip(&parameters)
            .enumerate()
            .map(|(index, (arg, (_, ty)))| {
                convert::from_json(arg, ty, &self.declarations, 0)
                    .map_err(|detail| invalid(format!("arg {index}: {detail}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let outcome = self.call(function, &values, fuel_limit)?;
        let result = match &outcome.result {
            CallResult::Value(value) => {
                serde_json::json!({"value": convert::to_json(value, &result_type, &self.declarations)})
            }
            CallResult::Trap { symbol } => serde_json::json!({"trap": symbol}),
            CallResult::OutOfFuel => serde_json::json!({"limit": "Fuel"}),
            CallResult::LimitExceeded { what } => serde_json::json!({"limit": what}),
        };
        Ok(serde_json::json!({
            "result": result,
            "fuel_used": outcome.fuel_used,
            "instructions": outcome.instructions,
        })
        .to_string())
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

/// Values nested deeper than this, in an argument or a result, are refused.
const MAX_DEPTH: usize = 64;

/// An integer of `ty`, when `ty` is an integer type it fits.
fn integer(value: i64, ty: &TypeExpr, types: &Types) -> Result<ConstData, String> {
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
        _ => Err(format!("expected {}, got an integer", types.name(ty))),
    }
}

/// Converts one argument against its declared type.
#[allow(clippy::too_many_lines)]
fn to_const(
    value: &EmbedValue,
    ty: &TypeExpr,
    types: &Types,
    depth: usize,
) -> Result<ConstValue, String> {
    if depth >= MAX_DEPTH {
        return Err(format!("the value nests more than {MAX_DEPTH} levels deep"));
    }
    let inner = |value: &EmbedValue, ty: &TypeExpr| to_const(value, ty, types, depth + 1);
    let data = match (value, ty) {
        (EmbedValue::Unit, TypeExpr::Unit) => ConstData::Unit,
        (EmbedValue::Int(number), _) => integer(*number, ty, types)?,
        (EmbedValue::Bool(flag), TypeExpr::Bool) => ConstData::Bool(*flag),
        (EmbedValue::Str(text), TypeExpr::Text) => ConstData::Text(text.clone()),
        (EmbedValue::IntList(items), TypeExpr::Vector(item)) => ConstData::Sequence(
            items
                .iter()
                .map(|number| {
                    Ok(ConstValue {
                        value_type: (**item).clone(),
                        data: integer(*number, item, types)?,
                    })
                })
                .collect::<Result<_, String>>()?,
        ),
        (EmbedValue::IntList(items), TypeExpr::Tuple(items_types))
            if items.len() == items_types.len() =>
        {
            ConstData::Sequence(
                items
                    .iter()
                    .zip(items_types)
                    .map(|(number, ty)| {
                        Ok(ConstValue {
                            value_type: ty.clone(),
                            data: integer(*number, ty, types)?,
                        })
                    })
                    .collect::<Result<_, String>>()?,
            )
        }
        (EmbedValue::List(items), TypeExpr::Vector(item)) => ConstData::Sequence(
            items
                .iter()
                .map(|value| inner(value, item))
                .collect::<Result<_, _>>()?,
        ),
        (EmbedValue::List(items), TypeExpr::Tuple(items_types))
            if items.len() == items_types.len() =>
        {
            ConstData::Sequence(
                items
                    .iter()
                    .zip(items_types)
                    .map(|(value, ty)| inner(value, ty))
                    .collect::<Result<_, _>>()?,
            )
        }
        (EmbedValue::Record(fields), TypeExpr::Named(named)) => {
            types.record(fields, named, depth)?
        }
        (EmbedValue::Variant { case, payload }, TypeExpr::Named(named)) => {
            types.variant(case, payload.as_deref(), named, depth)?
        }
        (
            EmbedValue::Variant {
                case,
                payload: None,
            },
            TypeExpr::BuiltinFailure(kind),
        ) => {
            let code = failure_code(*kind, case).ok_or_else(|| {
                format!("{} has no case `{case}`", crate::types::failure_name(*kind))
            })?;
            ConstData::BuiltinFailure(BuiltinFailureValue { kind: *kind, code })
        }
        (EmbedValue::Option(None), TypeExpr::Option(_)) => ConstData::Option(None),
        (EmbedValue::Option(Some(value)), TypeExpr::Option(item)) => {
            ConstData::Option(Some(Box::new(inner(value, item)?)))
        }
        (EmbedValue::Result(Ok(value)), TypeExpr::Result { ok, .. }) => {
            ConstData::Result(ResultConst::Ok(Box::new(inner(value, ok)?)))
        }
        (EmbedValue::Result(Err(value)), TypeExpr::Result { error, .. }) => {
            ConstData::Result(ResultConst::Err(Box::new(inner(value, error)?)))
        }
        (EmbedValue::Map(entries), TypeExpr::OrderedMap { key, value }) => {
            // In the kernel's order: by the keys' canonical bytes (VM
            // contract E4), each key once.
            let mut keyed = Vec::with_capacity(entries.len());
            for (index, (entry_key, entry_value)) in entries.iter().enumerate() {
                let entry_key =
                    inner(entry_key, key).map_err(|detail| format!("key {index}: {detail}"))?;
                let entry_value = inner(entry_value, value)
                    .map_err(|detail| format!("value {index}: {detail}"))?;
                let bytes = sley_mutate::encode_const_value(&entry_key)
                    .map_err(|_| format!("key {index} has no canonical encoding"))?;
                keyed.push((
                    bytes,
                    MapEntryConst {
                        key: entry_key,
                        value: entry_value,
                    },
                ));
            }
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            if keyed.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err("a map key appears twice".to_owned());
            }
            ConstData::Map(keyed.into_iter().map(|(_, entry)| entry).collect())
        }
        _ => return Err(format!("expected {}", types.name(ty))),
    };
    Ok(ConstValue {
        value_type: ty.clone(),
        data,
    })
}

/// An integer result as `i64`.
fn int_of(value: &ConstValue) -> Result<i64, String> {
    match value.data {
        ConstData::SInt(number) => {
            i64::try_from(number).map_err(|_| format!("the result {number} does not fit i64"))
        }
        ConstData::UInt(number) => {
            i64::try_from(number).map_err(|_| format!("the result {number} does not fit i64"))
        }
        _ => Err("a list[int] element is not an integer".to_owned()),
    }
}

const fn is_int(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::SInt(_) | TypeExpr::UInt(_))
}

impl Types {
    /// Every type definition of `program`, named under `names`.
    fn of(program: &Program, names: &Names) -> Self {
        let mut definitions = BTreeMap::new();
        for object in program.objects() {
            let record = object.record();
            let EntityBodyValue::TypeDef(body) = &record.body else {
                continue;
            };
            let id = record.entity_id;
            let members: Vec<MemberId> = match &body.form {
                TypeDefForm::Record(fields) => fields.iter().map(|field| field.member_id).collect(),
                TypeDefForm::Variant(cases) => cases.iter().map(|case| case.member_id).collect(),
            };
            definitions.insert(
                id,
                TypeDef {
                    name: names.name(&id),
                    form: body.form.clone(),
                    members: members
                        .into_iter()
                        .map(|member| (member, names.member_leaf(&id, &member)))
                        .collect(),
                },
            );
        }
        Self(definitions)
    }

    fn definition_name(&self, id: &EntityId) -> String {
        self.0.get(id).map_or_else(
            || format!("#{}", crate::hex::short(id.as_bytes())),
            |definition| definition.name.clone(),
        )
    }

    fn member(&self, definition: &EntityId, member: &MemberId) -> String {
        self.0
            .get(definition)
            .and_then(|definition| definition.members.iter().find(|(id, _)| id == member))
            .map_or_else(
                || format!("#{}", crate::hex::short(member.as_bytes())),
                |(_, leaf)| leaf.clone(),
            )
    }

    fn definition(&self, named: &NamedType) -> Result<&TypeDef, String> {
        self.0
            .get(&named.definition)
            .ok_or_else(|| "unknown type definition".to_owned())
    }

    /// A record argument: every field of the definition, given once by name.
    fn record(
        &self,
        given: &[(String, EmbedValue)],
        named: &NamedType,
        depth: usize,
    ) -> Result<ConstData, String> {
        let definition = self.definition(named)?;
        let TypeDefForm::Record(fields) = &definition.form else {
            return Err(format!(
                "expected a case of {}, got a record",
                definition.name
            ));
        };
        for (index, (name, _)) in given.iter().enumerate() {
            if given[..index].iter().any(|(earlier, _)| earlier == name) {
                return Err(format!(
                    "{} gives field `{name}` more than once",
                    definition.name
                ));
            }
            if !definition.members.iter().any(|(_, leaf)| leaf == name) {
                return Err(format!("{} has no field `{name}`", definition.name));
            }
        }
        let fields = fields
            .iter()
            .zip(&definition.members)
            .map(|(field, (_, leaf))| {
                let (_, value) = given
                    .iter()
                    .find(|(name, _)| name == leaf)
                    .ok_or_else(|| format!("missing field `{leaf}` of {}", definition.name))?;
                let ty = substitute(&field.value_type, &named.arguments);
                let value = to_const(value, &ty, self, depth + 1)
                    .map_err(|detail| format!("field `{leaf}`: {detail}"))?;
                Ok(FieldConst {
                    member_id: field.member_id,
                    value,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(ConstData::Record(RecordConst {
            definition: named.definition,
            fields,
        }))
    }

    /// A variant argument: a case by name, with a payload exactly when the
    /// case has one.
    fn variant(
        &self,
        case: &str,
        payload: Option<&EmbedValue>,
        named: &NamedType,
        depth: usize,
    ) -> Result<ConstData, String> {
        let definition = self.definition(named)?;
        let TypeDefForm::Variant(cases) = &definition.form else {
            return Err(format!("expected a {} record, got a case", definition.name));
        };
        let (found, _) = cases
            .iter()
            .zip(&definition.members)
            .find(|(_, (_, leaf))| leaf == case)
            .ok_or_else(|| format!("{} has no case `{case}`", definition.name))?;
        let payload = match (&found.payload_type, payload) {
            (None, None) => None,
            (Some(ty), Some(value)) => Some(Box::new(
                to_const(value, &substitute(ty, &named.arguments), self, depth + 1)
                    .map_err(|detail| format!("`{case}`: {detail}"))?,
            )),
            (None, Some(_)) => return Err(format!("`{case}` has no payload")),
            (Some(_), None) => return Err(format!("`{case}` needs a payload")),
        };
        Ok(ConstData::Variant(VariantConst {
            definition: named.definition,
            member_id: found.member_id,
            payload,
        }))
    }

    /// Converts a result.
    fn result_value(&self, value: &ConstValue, depth: usize) -> Result<EmbedValue, String> {
        if depth >= MAX_DEPTH {
            return Err(format!(
                "the result nests more than {MAX_DEPTH} levels deep"
            ));
        }
        let inner = |value: &ConstValue| self.result_value(value, depth + 1);
        let boxed = |value: &ConstValue| inner(value).map(Box::new);
        Ok(match &value.data {
            ConstData::Unit => EmbedValue::Unit,
            ConstData::SInt(_) | ConstData::UInt(_) => EmbedValue::Int(int_of(value)?),
            ConstData::Bool(flag) => EmbedValue::Bool(*flag),
            ConstData::Text(text) => EmbedValue::Str(text.clone()),
            ConstData::Sequence(items) => match &value.value_type {
                TypeExpr::Vector(item) if is_int(item) => {
                    EmbedValue::IntList(items.iter().map(int_of).collect::<Result<_, _>>()?)
                }
                _ => EmbedValue::List(items.iter().map(inner).collect::<Result<_, _>>()?),
            },
            ConstData::Record(record) => EmbedValue::Record(
                record
                    .fields
                    .iter()
                    .map(|field| {
                        Ok((
                            self.member(&record.definition, &field.member_id),
                            inner(&field.value)?,
                        ))
                    })
                    .collect::<Result<_, String>>()?,
            ),
            ConstData::Variant(variant) => EmbedValue::Variant {
                case: self.member(&variant.definition, &variant.member_id),
                payload: variant.payload.as_deref().map(boxed).transpose()?,
            },
            ConstData::Option(option) => {
                EmbedValue::Option(option.as_deref().map(boxed).transpose()?)
            }
            ConstData::Result(ResultConst::Ok(ok)) => EmbedValue::Result(Ok(boxed(ok)?)),
            ConstData::Result(ResultConst::Err(error)) => EmbedValue::Result(Err(boxed(error)?)),
            ConstData::Map(entries) => EmbedValue::Map(
                entries
                    .iter()
                    .map(|entry| Ok((inner(&entry.key)?, inner(&entry.value)?)))
                    .collect::<Result<_, String>>()?,
            ),
            ConstData::BuiltinFailure(failure) => {
                EmbedValue::case(failure_case(failure.kind, failure.code))
            }
            _ => {
                return Err(format!(
                    "the result type {} has no embed form (floats, bytes and function references have none)",
                    self.name(&value.value_type)
                ));
            }
        })
    }

    /// A Python list result: the values of its `Map<u64,T>`, whose keys
    /// are `0..len`, in key order.
    fn list_result(&self, value: &ConstValue) -> Result<EmbedValue, String> {
        let (ConstData::Map(entries), Some(item)) = (&value.data, list_item(&value.value_type))
        else {
            return self.result_value(value, 0);
        };
        let mut keyed = entries
            .iter()
            .map(|entry| match entry.key.data {
                ConstData::UInt(key) => Ok((key, &entry.value)),
                _ => Err("a list result's key is not an integer".to_owned()),
            })
            .collect::<Result<Vec<_>, String>>()?;
        keyed.sort_by_key(|(key, _)| *key);
        if keyed
            .iter()
            .enumerate()
            .any(|(index, (key, _))| u128::try_from(index).ok() != Some(*key))
        {
            return Err("a list result's keys are not 0..len".to_owned());
        }
        Ok(if is_int(item) {
            EmbedValue::IntList(
                keyed
                    .iter()
                    .map(|(_, value)| int_of(value))
                    .collect::<Result<_, _>>()?,
            )
        } else {
            EmbedValue::List(
                keyed
                    .iter()
                    .map(|(_, value)| self.result_value(value, 1))
                    .collect::<Result<_, _>>()?,
            )
        })
    }

    /// A type as a [`Signature`] states it.
    fn describe(&self, ty: &TypeExpr) -> EmbedType {
        let boxed = |ty: &TypeExpr| Box::new(self.describe(ty));
        match ty {
            TypeExpr::Unit => EmbedType::Unit,
            TypeExpr::Bool => EmbedType::Bool,
            TypeExpr::SInt(width) => EmbedType::Int {
                signed: true,
                bits: width.bits(),
            },
            TypeExpr::UInt(width) => EmbedType::Int {
                signed: false,
                bits: width.bits(),
            },
            TypeExpr::Text => EmbedType::Str,
            TypeExpr::Vector(item) => EmbedType::List(boxed(item)),
            TypeExpr::Tuple(items) => {
                EmbedType::Tuple(items.iter().map(|item| self.describe(item)).collect())
            }
            TypeExpr::Option(item) => EmbedType::Option(boxed(item)),
            TypeExpr::Result { ok, error } => EmbedType::Result {
                ok: boxed(ok),
                err: boxed(error),
            },
            TypeExpr::OrderedMap { key, value } => EmbedType::Map {
                key: boxed(key),
                value: boxed(value),
            },
            TypeExpr::Named(named)
                if named.arguments.is_empty() && self.0.contains_key(&named.definition) =>
            {
                EmbedType::Named(self.definition_name(&named.definition))
            }
            TypeExpr::BuiltinFailure(kind) => {
                EmbedType::Named(crate::types::failure_name(*kind).to_owned())
            }
            _ => EmbedType::Unsupported(self.name(ty)),
        }
    }

    /// The record and variant types `used` reaches, in name order.
    fn declarations(&self, used: &[TypeExpr]) -> Vec<TypeDeclaration> {
        let mut found: BTreeMap<String, TypeDeclaration> = BTreeMap::new();
        let mut pending: Vec<&TypeExpr> = used.iter().collect();
        while let Some(ty) = pending.pop() {
            match ty {
                TypeExpr::Named(named) if named.arguments.is_empty() => {
                    let Some(definition) = self.0.get(&named.definition) else {
                        continue;
                    };
                    if found.contains_key(&definition.name) {
                        continue;
                    }
                    let shape = match &definition.form {
                        TypeDefForm::Record(fields) => {
                            pending.extend(fields.iter().map(|field| &field.value_type));
                            TypeShape::Record(
                                fields
                                    .iter()
                                    .zip(&definition.members)
                                    .map(|(field, (_, leaf))| {
                                        (leaf.clone(), self.describe(&field.value_type))
                                    })
                                    .collect(),
                            )
                        }
                        TypeDefForm::Variant(cases) => {
                            pending
                                .extend(cases.iter().filter_map(|case| case.payload_type.as_ref()));
                            TypeShape::Variant(
                                cases
                                    .iter()
                                    .zip(&definition.members)
                                    .map(|(case, (_, leaf))| {
                                        (
                                            leaf.clone(),
                                            case.payload_type.as_ref().map(|ty| self.describe(ty)),
                                        )
                                    })
                                    .collect(),
                            )
                        }
                    };
                    found.insert(
                        definition.name.clone(),
                        TypeDeclaration {
                            name: definition.name.clone(),
                            shape,
                        },
                    );
                }
                TypeExpr::BuiltinFailure(kind) => {
                    let name = crate::types::failure_name(*kind);
                    found
                        .entry(name.to_owned())
                        .or_insert_with(|| TypeDeclaration {
                            name: name.to_owned(),
                            shape: TypeShape::Variant(
                                failure_cases(*kind)
                                    .iter()
                                    .map(|case| ((*case).to_owned(), None))
                                    .collect(),
                            ),
                        });
                }
                TypeExpr::Vector(item) | TypeExpr::Option(item) => pending.push(item),
                TypeExpr::Tuple(items) => pending.extend(items),
                TypeExpr::Result { ok, error } => pending.extend([&**ok, &**error]),
                TypeExpr::OrderedMap { key, value } => pending.extend([&**key, &**value]),
                _ => {}
            }
        }
        found.into_values().collect()
    }

    /// A short spelling of a type for refusals.
    fn name(&self, ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Unit => "unit".to_owned(),
            TypeExpr::Bool => "bool".to_owned(),
            TypeExpr::SInt(width) => format!("i{}", width.bits()),
            TypeExpr::UInt(width) => format!("u{}", width.bits()),
            TypeExpr::Text => "str".to_owned(),
            TypeExpr::Bytes => "bytes".to_owned(),
            TypeExpr::Vector(item) => format!("list[{}]", self.name(item)),
            TypeExpr::Tuple(items) => format!(
                "tuple[{}]",
                items
                    .iter()
                    .map(|item| self.name(item))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            TypeExpr::Option(item) => format!("Option<{}>", self.name(item)),
            TypeExpr::Result { ok, error } => {
                format!("Result<{}, {}>", self.name(ok), self.name(error))
            }
            TypeExpr::OrderedMap { key, value } => {
                format!("Map<{}, {}>", self.name(key), self.name(value))
            }
            TypeExpr::Named(named) => self.definition_name(&named.definition),
            TypeExpr::BuiltinFailure(kind) => crate::types::failure_name(*kind).to_owned(),
            _ => crate::types::render_with(ty, &|id| self.definition_name(id)),
        }
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

    const SHAPES: &str = "\
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

@dataclass
class Segment:
    a: Point
    b: Point

def manhattan(s: Segment) -> int:
    return abs(s.a.x - s.b.x) + abs(s.a.y - s.b.y)

def mirror(p: Point) -> Point:
    return Point(p.y, p.x)

def flip(p: Point) -> Point:
    return Point(y=p.x, x=p.y)

def first(s: Segment) -> Point:
    return s.a

def evens(n: int) -> list[int]:
    out = []
    for i in range(n):
        if i % 2 == 0:
            out.append(i)
    return out

def corners(n: int) -> list[Point]:
    out: list[Point] = []
    for i in range(n):
        out.append(Point(i, -i))
    return out
";

    fn point(x: i64, y: i64) -> EmbedValue {
        EmbedValue::record([("x", EmbedValue::Int(x)), ("y", EmbedValue::Int(y))])
    }

    fn value(module: &mut Module, function: &str, args: &[EmbedValue]) -> EmbedValue {
        match module.call(function, args, 10_000_000).unwrap().result {
            CallResult::Value(value) => value,
            other => panic!("{function}: {other:?}"),
        }
    }

    #[test]
    fn python_records_go_in_and_come_out() {
        let mut module = compile_python(SHAPES, &OPTIONS).unwrap();
        // Fields by name, in any order, nested.
        let segment = EmbedValue::record([
            (
                "b",
                EmbedValue::record([("y", EmbedValue::Int(-2)), ("x", EmbedValue::Int(4))]),
            ),
            ("a", point(1, 2)),
        ]);
        assert_eq!(
            value(&mut module, "manhattan", &[segment]),
            EmbedValue::Int(7)
        );
        // A record result lists its fields in declaration order.
        let mirrored = value(&mut module, "mirror", &[point(3, 9)]);
        assert_eq!(mirrored, point(9, 3));
        assert_eq!(mirrored.field("x"), Some(&EmbedValue::Int(9)));
        assert_eq!(mirrored.field("z"), None);
        assert_eq!(value(&mut module, "flip", &[point(3, 9)]), point(9, 3));
        let segment = EmbedValue::record([("a", point(5, 6)), ("b", point(0, 0))]);
        assert_eq!(value(&mut module, "first", &[segment]), point(5, 6));
    }

    #[test]
    fn python_list_results_are_lists() {
        let mut module = compile_python(SHAPES, &OPTIONS).unwrap();
        // More than 128 items: past where the keys' canonical byte order and
        // their numeric order part.
        let evens = value(&mut module, "evens", &[EmbedValue::Int(400)]);
        assert_eq!(evens, EmbedValue::IntList((0..400).step_by(2).collect()));
        assert_eq!(
            value(&mut module, "evens", &[EmbedValue::Int(0)]),
            EmbedValue::IntList(Vec::new())
        );
        let corners = value(&mut module, "corners", &[EmbedValue::Int(3)]);
        assert_eq!(
            corners,
            EmbedValue::List(vec![point(0, 0), point(1, -1), point(2, -2)])
        );
        assert_eq!(
            module.signature("evens").unwrap().result,
            EmbedType::List(Box::new(EmbedType::Int {
                signed: true,
                bits: 64
            }))
        );
    }

    #[test]
    fn signatures_and_types_describe_the_program() {
        let module = compile_python(SHAPES, &OPTIONS).unwrap();
        let int = EmbedType::Int {
            signed: true,
            bits: 64,
        };
        let manhattan = module.signature("manhattan").unwrap();
        assert_eq!(manhattan.name, "manhattan");
        assert_eq!(
            manhattan.parameters,
            vec![("s".to_owned(), EmbedType::Named("Segment".to_owned()))]
        );
        assert_eq!(manhattan.result, int);
        assert_eq!(
            module.signature("corners").unwrap().result,
            EmbedType::List(Box::new(EmbedType::Named("Point".to_owned())))
        );
        assert!(module.signature("nope").is_none());
        let point = || EmbedType::Named("Point".to_owned());
        assert_eq!(
            module.types(),
            [
                TypeDeclaration {
                    name: "Point".to_owned(),
                    shape: TypeShape::Record(vec![
                        ("x".to_owned(), int.clone()),
                        ("y".to_owned(), int.clone()),
                    ]),
                },
                TypeDeclaration {
                    name: "Segment".to_owned(),
                    shape: TypeShape::Record(vec![
                        ("a".to_owned(), point()),
                        ("b".to_owned(), point()),
                    ]),
                },
            ]
        );
    }

    #[test]
    fn record_arguments_are_checked() {
        let mut module = compile_python(SHAPES, &OPTIONS).unwrap();
        let refused = |module: &mut Module, arg: EmbedValue| {
            let error = module.call("mirror", &[arg], 1_000_000).unwrap_err();
            assert_eq!(error.code, "AGENT_INPUT_INVALID", "{error}");
            error.message
        };
        let missing = refused(&mut module, EmbedValue::record([("x", EmbedValue::Int(1))]));
        assert!(missing.contains("missing field `y`"), "{missing}");
        let extra = refused(
            &mut module,
            EmbedValue::record([
                ("x", EmbedValue::Int(1)),
                ("y", EmbedValue::Int(1)),
                ("z", EmbedValue::Int(1)),
            ]),
        );
        assert!(extra.contains("no field `z`"), "{extra}");
        let twice = refused(
            &mut module,
            EmbedValue::record([
                ("x", EmbedValue::Int(1)),
                ("x", EmbedValue::Int(2)),
                ("y", EmbedValue::Int(1)),
            ]),
        );
        assert!(twice.contains("more than once"), "{twice}");
        let typed = refused(
            &mut module,
            EmbedValue::record([("x", EmbedValue::Bool(true)), ("y", EmbedValue::Int(1))]),
        );
        assert!(typed.contains("field `x`"), "{typed}");
        let shape = refused(&mut module, EmbedValue::case("Point"));
        assert!(shape.contains("got a case"), "{shape}");
    }

    #[test]
    fn record_values_are_checked_when_compiled() {
        let head = "class P:\n    x: int\n    y: int\n\ndef f(a: int) -> P:\n";
        for (body, message) in [
            ("    return P(x=a)\n", "needs field `y`"),
            (
                "    return P(a == 1, a)\n",
                "field `x` of `P` is i64, not bool",
            ),
            ("    return P(a, a, a)\n", "has 2 field"),
        ] {
            let error = compile_python(&format!("{head}{body}"), &OPTIONS).unwrap_err();
            assert_eq!(error.code, "AGENT_FRAME_INVALID", "{error}");
            assert!(error.message.contains(message), "{body}: {error}");
            assert_eq!(error.line, Some(6), "{error}");
        }
    }

    #[test]
    fn field_names_are_never_operations_or_variables() {
        // Fields named after operations (`to`, `get`, `add`) and after the
        // generator's own variable, inside a generator and under `try`.
        let mut module = compile_python(
            "\
class Edge:
    frm: int
    to: int

class P:
    x: int
    get: int

def w(e: Edge) -> int:
    return e.to - e.frm

def g(p: P) -> int:
    return p.x * p.get

def total(xs: list[int]) -> int:
    return sum(w(Edge(frm=i, to=x)) for i, x in enumerate(xs))

def named(xs: list[int]) -> int:
    return sum(g(P(x=x, get=2)) for x in xs)
",
            &OPTIONS,
        )
        .unwrap();
        let xs = EmbedValue::IntList(vec![5, 7, 9]);
        assert_eq!(
            value(&mut module, "total", std::slice::from_ref(&xs)),
            EmbedValue::Int(18)
        );
        assert_eq!(value(&mut module, "named", &[xs]), EmbedValue::Int(42));
        let mut module = compile_python(
            "\
struct Span { to: i64, add: i64 }

struct Q { x: i64, y: i64 }

fn span(a: i64) -> Result<Span,ArithmeticError> {
    ok(try(Span { to: a + 1, add: a }))
}

fn q(a: i64, b: i64) -> Result<Q,ArithmeticError> {
    ok(try(Q { y: a / b, x: a + a }))
}
",
            &OPTIONS,
        )
        .unwrap();
        assert_eq!(
            value(&mut module, "span", &[EmbedValue::Int(1)]),
            EmbedValue::Result(Ok(Box::new(EmbedValue::record([
                ("to", EmbedValue::Int(2)),
                ("add", EmbedValue::Int(1)),
            ]))))
        );
        assert_eq!(
            value(&mut module, "span", &[EmbedValue::Int(i64::MAX)]),
            EmbedValue::Result(Err(Box::new(EmbedValue::case("Overflow"))))
        );
        // Values are evaluated in the order written: `y` (a division by
        // zero) fails before `x` (an overflow) is reached.
        assert_eq!(
            value(
                &mut module,
                "q",
                &[EmbedValue::Int(i64::MAX), EmbedValue::Int(0)]
            ),
            EmbedValue::Result(Err(Box::new(EmbedValue::case("DivideByZero"))))
        );
    }

    #[test]
    fn errors_inside_named_fields_point_at_their_operand() {
        let source =
            "struct P { y: i64 }\nfn f(a: i64, b: u8, c: i64) -> P {\n    P { y: a * b + c }\n}\n";
        let error = compile_python(source, &OPTIONS).unwrap_err();
        assert_eq!(error.line, Some(3), "{error}");
        assert_eq!(
            error.column,
            Some(u32::try_from(source.lines().nth(2).unwrap().find('b').unwrap() + 1).unwrap()),
            "{error}"
        );
    }

    const ACCOUNT: &str = "\
enum WithdrawError { NotPositive, Insufficient }

struct Account { owner: text, balance: i64 }

fn withdraw(account: Account, amount: i64) -> Result<Account,WithdrawError> {
    if amount <= 0 {
        return err(NotPositive)
    }
    if amount > account.balance {
        return err(Insufficient)
    }
    ok(Account { owner: account.owner, balance: account.balance - amount })
}

fn echo(reason: WithdrawError) -> WithdrawError {
    reason
}

fn maybe(x: Option<i64>) -> Option<i64> {
    x
}

fn table(m: Map<text,i64>) -> Map<text,i64> {
    m
}
";

    #[test]
    fn enums_results_options_and_maps() {
        let mut module = compile_python(ACCOUNT, &OPTIONS).unwrap();
        let account = |balance| {
            EmbedValue::record([
                ("owner", EmbedValue::Str("ada".to_owned())),
                ("balance", EmbedValue::Int(balance)),
            ])
        };
        assert_eq!(
            value(&mut module, "withdraw", &[account(10), EmbedValue::Int(4)]),
            EmbedValue::Result(Ok(Box::new(account(6))))
        );
        assert_eq!(
            value(&mut module, "withdraw", &[account(10), EmbedValue::Int(40)]),
            EmbedValue::Result(Err(Box::new(EmbedValue::case("Insufficient"))))
        );
        assert_eq!(
            value(&mut module, "echo", &[EmbedValue::case("NotPositive")]),
            EmbedValue::case("NotPositive")
        );
        let unknown = module
            .call("echo", &[EmbedValue::case("Overdrawn")], 1_000_000)
            .unwrap_err();
        assert!(unknown.message.contains("no case `Overdrawn`"), "{unknown}");
        let payload = module
            .call(
                "echo",
                &[EmbedValue::Variant {
                    case: "NotPositive".to_owned(),
                    payload: Some(Box::new(EmbedValue::Int(1))),
                }],
                1_000_000,
            )
            .unwrap_err();
        assert!(payload.message.contains("has no payload"), "{payload}");
        for option in [
            EmbedValue::Option(None),
            EmbedValue::Option(Some(Box::new(EmbedValue::Int(5)))),
        ] {
            assert_eq!(
                value(&mut module, "maybe", std::slice::from_ref(&option)),
                option
            );
        }
        // Map entries in any order come back in the kernel's order.
        let entry = |key: &str, number| (EmbedValue::Str(key.to_owned()), EmbedValue::Int(number));
        let given = EmbedValue::Map(vec![entry("b", 2), entry("a", 1)]);
        assert_eq!(
            value(&mut module, "table", &[given]),
            EmbedValue::Map(vec![entry("a", 1), entry("b", 2)])
        );
        let twice = module
            .call(
                "table",
                &[EmbedValue::Map(vec![entry("a", 1), entry("a", 2)])],
                1_000_000,
            )
            .unwrap_err();
        assert!(twice.message.contains("appears twice"), "{twice}");
        let withdraw = module.signature("withdraw").unwrap();
        assert_eq!(
            withdraw.result,
            EmbedType::Result {
                ok: Box::new(EmbedType::Named("Account".to_owned())),
                err: Box::new(EmbedType::Named("WithdrawError".to_owned())),
            }
        );
        let names: Vec<&str> = module.types().iter().map(|ty| ty.name.as_str()).collect();
        assert_eq!(names, ["Account", "WithdrawError"]);
        assert_eq!(
            module.types()[1].shape,
            TypeShape::Variant(vec![
                ("NotPositive".to_owned(), None),
                ("Insufficient".to_owned(), None),
            ])
        );
    }

    #[test]
    fn deep_values_are_refused() {
        let mut module = compile_python(
            "def depth(xs: list[list[list[int]]]) -> int:\n    return len(xs)\n",
            &OPTIONS,
        )
        .unwrap();
        let mut deep = EmbedValue::IntList(vec![1]);
        for _ in 0..80 {
            deep = EmbedValue::List(vec![deep]);
        }
        let error = module.call("depth", &[deep], 1_000_000).unwrap_err();
        assert_eq!(error.code, "AGENT_INPUT_INVALID", "{error}");
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

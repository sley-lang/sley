//! Ripple: a change described once (`"ripple"` in an AF1-X frame).
//!
//! `"ripple": [intent, ...]` lists closed, typed graph transformations. They
//! apply in written order after the frame's own definitions are resolved
//! (expanded), and each one derives ordinary AF1 edits into the same plain
//! frame: patches of the functions it rewrites, rewritten calls in the
//! frame's own functions, and restated tests. The unchanged compiler and
//! kernel then judge the whole candidate, so a derivation never bypasses
//! validation. A decision a derivation cannot make mechanically is a hole:
//! an obligation whose pointer leads into the intent, never a guess.
//!
//! Enabled intents:
//!
//! - `{"arity": f}` and `{"arity": f, "value": v}`. The frame restates the
//!   parameters of the live function `f` (in `fns` or a `patch` with
//!   `params`). Every statically resolved `call` of `f` and every
//!   `TestCase` of `f` gets its arguments matched to the new parameters by
//!   name: a kept parameter keeps its argument, a removed one drops it (its
//!   computation still runs), and a new one takes `v` when it is given and
//!   fits the parameter type; otherwise the site is a hole. A call the
//!   frame itself writes is kept when its argument count is the new one and
//!   rewritten when it is the old one; when both counts are equal the count
//!   decides nothing, so it is a hole unless `"frame_calls"` says `"old"`
//!   (rewrite) or `"new"` (keep); `"frame_calls"` covers only what the
//!   intent's revision states, not what later revisions state (`"after"`,
//!   kept by layering). An intent the head already reflects derives
//!   nothing again: what that derivation did cannot be established from the
//!   head, so what the frame states must be what the head has, and each
//!   difference is a hole.
//! - `{"guard": g, "arg": p, "in": [f, ...], "mode": "preserve"|"entry"}`.
//!   The checker `g: P -> Result<P,E>` takes over the checking of parameter
//!   `p` of each live function `f`, read as the frame leaves the program
//!   (its constants, bodies and call graph). `preserve` (the default)
//!   replaces an inline check of `p` that is the same pure check as `g`'s
//!   body, up to names, at its own position; `entry` evaluates `g(p)` once
//!   before anything else `f` does and routes every use of `p` that the
//!   `Ok` payload dominates through it. Its error goes to `f`'s own result
//!   when `f` returns that error type, or to the block named by
//!   `"handler"`; never to a block chosen by type alone. A handler that
//!   reads a value another block of `f` computes is a hole, since the
//!   error now reaches it first.
//!
//! `effect`, `member`, `retype`, `move` and `prune` are specified but not
//! enabled in this build (`AGENT_RIPPLE_INTENT_UNKNOWN`).
//!
//! The exported boundary of `arity`: `f` is referenced by something other
//! than a call or a `TestCase` (an entry point, a package's exports or a
//! namespace they hold, a global's initializer, a contract or a policy
//! binding), so code outside the program's call graph calls it with its
//! current parameters; or a function whose call ripple would rewrite (live
//! or restated by the frame) belongs to other namespaces than `f`. Either is
//! `AGENT_RIPPLE_EXPORTED_BOUNDARY`, never a rewrite. A function's
//! visibility alone is not the boundary: every call in the workspace is
//! visible here. A use of `f` as a function value (`fnref`, or a constant
//! or `TestCase` holding it) is unresolved dispatch: a hole.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::{Map, Value, json};
use sley_id::{CandidateNonce, EntityId};
use sley_mutate::MutationPayload;
use sley_mutate::value::{EntityBodyValue, FunctionBody};
use sley_ssmc::{
    BuiltinCase, CaseKey, ConstData, ConstValue, EffectEnvironment, ExpectedOutcome, Immediate,
    MemberId, Reachability, ResultConst, SwitchArgument, TargetEdge, Terminator, TrapCode,
    TypeExpr, ValueRef, VariantImmediate,
};

use crate::afx::{Context, MAX_NAME, MapEntry, Obligation, Role, SourceMap, read_params, shorten};
use crate::error::{AgentErrorCode, Result};
use crate::names::{NameMap, Names, Scope, kind_name};
use crate::opcodes;
use crate::workspace::Program;

/// Most intents one frame may list.
pub const MAX_RIPPLE_INTENTS: usize = 32;
/// Most call sites and tests one intent may rewrite.
pub const MAX_RIPPLE_SITES: usize = 256;
/// Most functions one guard may name, and most blocks its checker may have.
pub const MAX_GUARD_SIZE: usize = 64;
/// Most comparisons (operation pairs and edges) a preserve guard makes
/// while looking for its check in one function.
pub const MAX_MATCH_STEPS: usize = 1024;

/// Intents that are specified but not enabled in this build.
const DISABLED: [&str; 5] = ["effect", "member", "retype", "move", "prune"];

/// What the intents of one frame derived.
pub(crate) struct Outcome {
    /// The affected-entity and boundary inventory (`ripple.json`).
    pub(crate) inventory: Value,
    /// Intents listed.
    pub(crate) intents: u64,
    /// Edits derived (call sites, tests and guarded functions).
    pub(crate) edits: u64,
    /// Obligations the derivations left.
    pub(crate) holes: u64,
}

/// One parsed intent.
enum Intent {
    Arity {
        target: String,
        value: Option<Value>,
        /// How the calls and tests the frame writes are read: `Some(true)`
        /// against the old parameters, `Some(false)` against the new ones,
        /// `None` by their argument count.
        frame_calls: Option<bool>,
        /// Functions, tests and test tables the frame stated in revisions
        /// after the intent's own (kept by layering): `"frame_calls"` does
        /// not cover them, so only their argument count can tell.
        after: BTreeSet<String>,
    },
    Guard {
        checker: String,
        arg: String,
        functions: Vec<String>,
        entry: bool,
        /// The block of each function that takes the checker's error
        /// (entry mode), when the author names one.
        handler: Option<String>,
    },
}

/// Applies the frame's `ripple` intents to `out`, the frame expanded to
/// plain AF1. Every intent's shape is checked first; the derivations run
/// only when `derive` holds (the rest of the frame expanded without an
/// open decision), since they read the frame's own definitions.
///
/// # Errors
///
/// The compiler's refusal of the frame's own definitions, mapped to the
/// authored frame, when a `guard` needs its checker compiled from the frame
/// and the frame does not compile.
#[allow(clippy::too_many_lines)]
pub(crate) fn apply(
    cx: &Context<'_>,
    frame: &Map<String, Value>,
    out: &mut Map<String, Value>,
    map: &mut SourceMap,
    obligations: &mut Vec<Obligation>,
    derive: bool,
) -> Result<Outcome> {
    let intents = frame.get("ripple").unwrap_or(&Value::Null);
    let mut outcome = Outcome {
        inventory: json!({"intents": [], "changed": {"functions": [], "tests": []}}),
        intents: 0,
        edits: 0,
        holes: 0,
    };
    let Some(list) = intents.as_array() else {
        obligations.push(Obligation::new(
            AgentErrorCode::RippleIntentUnknown,
            "/ripple",
            "ripple is a list of intents: [{\"arity\": \"f\"}, {\"guard\": \"g\", \"arg\": \"p\", \"in\": [\"f\"]}]",
        ));
        return Ok(outcome);
    };
    outcome.intents = list.len() as u64;
    if list.len() > MAX_RIPPLE_INTENTS {
        obligations.push(Obligation::new(
            AgentErrorCode::RippleLimit,
            "/ripple",
            format!(
                "{} intents; a frame lists at most {MAX_RIPPLE_INTENTS}: split the change",
                list.len()
            ),
        ));
        return Ok(outcome);
    }
    let mut parsed = Vec::new();
    let mut problems = Vec::new();
    for (index, entry) in list.iter().enumerate() {
        match parse(entry, index) {
            Ok(intent) => parsed.push(intent),
            Err(problem) => problems.push(problem),
        }
    }
    if !problems.is_empty() || !derive {
        outcome.holes = problems.len() as u64;
        obligations.extend(problems);
        return Ok(outcome);
    }
    let mut ripple = Ripple::new(cx, out, map);
    ripple.tables = frame
        .get("test_tables")
        .and_then(Value::as_array)
        .map(|tables| {
            tables
                .iter()
                .map(|table| {
                    table
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned()
                })
                .collect()
        })
        .unwrap_or_default();
    for (index, intent) in parsed.into_iter().enumerate() {
        match intent {
            Intent::Arity {
                target,
                value,
                frame_calls,
                after,
            } => ripple.arity(
                index,
                &target,
                &Stated {
                    value: value.as_ref(),
                    declared: frame_calls,
                    after: &after,
                },
            ),
            Intent::Guard {
                checker,
                arg,
                functions,
                entry,
                handler,
            } => ripple.guard(
                index,
                &Guard {
                    checker: &checker,
                    arg: &arg,
                    functions: &functions,
                    entry,
                    handler: handler.as_deref(),
                },
            )?,
        }
    }
    if ripple.holes.is_empty() {
        ripple.emit();
    }
    outcome.edits = ripple.edits;
    outcome.holes = ripple.holes.len() as u64;
    outcome.inventory = ripple.inventory();
    obligations.extend(ripple.holes);
    Ok(outcome)
}

/// Reads one intent: its kind (exactly one intent key), then its keys.
#[allow(clippy::too_many_lines)]
fn parse(entry: &Value, index: usize) -> std::result::Result<Intent, Obligation> {
    let at = format!("/ripple/{index}");
    let unknown =
        |decision: String| Obligation::new(AgentErrorCode::RippleIntentUnknown, &at, decision);
    let Some(object) = entry.as_object() else {
        return Err(unknown(
            "an intent is an object: {\"arity\": \"f\"} or {\"guard\": \"g\", \"arg\": \"p\", \"in\": [\"f\"]}"
                .to_owned(),
        ));
    };
    let kinds: Vec<&str> = ["arity", "guard"]
        .iter()
        .chain(DISABLED.iter())
        .copied()
        .filter(|kind| object.contains_key(*kind))
        .collect();
    let kind = match kinds.as_slice() {
        [kind] => *kind,
        [] => {
            return Err(unknown(
                "unknown intent: the enabled intents are {\"arity\": f} and {\"guard\": g, \"arg\": p, \"in\": [f...]}".to_owned(),
            ));
        }
        _ => {
            return Err(unknown(format!(
                "one intent per entry, not {}",
                kinds.join(" and ")
            )));
        }
    };
    if DISABLED.contains(&kind) {
        return Err(unknown(format!(
            "`{kind}` is not enabled in this build; the enabled intents are arity and guard"
        )));
    }
    let allowed: &[&str] = if kind == "arity" {
        &["arity", "value", "frame_calls", "after", "comment"]
    } else {
        &["guard", "arg", "in", "mode", "handler", "comment"]
    };
    let shape = |pointer: String, decision: &str| {
        Obligation::new(AgentErrorCode::FrameInvalid, &pointer, decision)
    };
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(shape(
                format!("{at}/{key}"),
                &format!(
                    "unknown key in a {kind} intent (it has {})",
                    allowed.join(", ")
                ),
            ));
        }
    }
    let name = |key: &str| -> std::result::Result<String, Obligation> {
        object
            .get(key)
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| shape(format!("{at}/{key}"), "expected a name"))
    };
    if kind == "arity" {
        let frame_calls = match object.get("frame_calls") {
            None => None,
            Some(Value::String(word)) if word == "old" => Some(true),
            Some(Value::String(word)) if word == "new" => Some(false),
            Some(_) => {
                return Err(shape(
                    format!("{at}/frame_calls"),
                    "\"frame_calls\" says how the calls and tests this frame writes are read: \"old\" (against the old parameters, rewritten by name) or \"new\" (as written)",
                ));
            }
        };
        let names = |key: &str| match object.get(key) {
            None => Ok(BTreeSet::new()),
            Some(Value::Array(names)) if names.iter().all(Value::is_string) => Ok(names
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()),
            Some(_) => Err(shape(
                format!("{at}/{key}"),
                "a list of the names of functions, tests and test tables: [\"name\", ...]",
            )),
        };
        return Ok(Intent::Arity {
            target: name("arity")?,
            value: object.get("value").cloned(),
            frame_calls,
            after: names("after")?,
        });
    }
    let checker = name("guard")?;
    let arg = name("arg")?;
    let functions: Vec<String> = match object.get("in").and_then(Value::as_array) {
        Some(list) if !list.is_empty() && list.iter().all(Value::is_string) => list
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => {
            return Err(shape(
                format!("{at}/in"),
                "a guard names the functions it applies to: \"in\": [\"f\", ...]",
            ));
        }
    };
    if functions.len() > MAX_GUARD_SIZE {
        return Err(Obligation::new(
            AgentErrorCode::RippleLimit,
            &format!("{at}/in"),
            format!(
                "{} functions; a guard names at most {MAX_GUARD_SIZE}: split it",
                functions.len()
            ),
        ));
    }
    let entry = match object.get("mode") {
        None => false,
        Some(Value::String(mode)) if mode == "preserve" => false,
        Some(Value::String(mode)) if mode == "entry" => true,
        Some(_) => {
            return Err(shape(
                format!("{at}/mode"),
                "the mode is \"preserve\" (the default) or \"entry\"",
            ));
        }
    };
    let handler = match object.get("handler") {
        None => None,
        Some(_) if !entry => {
            return Err(shape(
                format!("{at}/handler"),
                "a handler takes the checker's error in \"mode\": \"entry\"; preserve returns it as the replaced check did",
            ));
        }
        Some(_) => Some(name("handler")?),
    };
    Ok(Intent::Guard {
        checker,
        arg,
        functions,
        entry,
        handler,
    })
}

// ---------------------------------------------------------------------------
// Function bodies as ripple edits them
// ---------------------------------------------------------------------------

/// A value inside one function, by local names.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Val {
    /// A function parameter.
    Param(String),
    /// A block parameter: block, name.
    Block(String, String),
    /// An operation result: block, operation, result index.
    Op(String, String, u32),
}

/// An operation's immediate.
#[derive(Clone, Debug, PartialEq)]
enum Imm {
    None,
    Entity(EntityId),
    Index(u32),
    Field(MemberId),
    Variant(VariantImmediate),
    Observation([u8; 32]),
    Function {
        id: EntityId,
        name: String,
        generic: bool,
    },
    /// A literal a derivation adds: `{"type": T, "value": v}`.
    Literal(Value),
}

#[derive(Clone, Debug)]
struct Op {
    leaf: String,
    tag: u32,
    imm: Imm,
    args: Vec<Val>,
    types: Vec<TypeExpr>,
}

#[derive(Clone, Debug, PartialEq)]
enum Arg {
    Val(Val),
    Payload,
}

#[derive(Clone, Debug, PartialEq)]
struct Edge {
    target: String,
    args: Vec<Arg>,
}

#[derive(Clone, Debug)]
enum Term {
    Return(Val),
    Br(Edge),
    Cond(Val, Edge, Edge),
    Switch(Val, Vec<(CaseKey, Edge)>),
    Trap(TrapCode, Option<Val>),
}

impl Term {
    fn edges(&self) -> Vec<&Edge> {
        match self {
            Self::Br(edge) => vec![edge],
            Self::Cond(_, then, other) => vec![then, other],
            Self::Switch(_, cases) => cases.iter().map(|(_, edge)| edge).collect(),
            Self::Return(_) | Self::Trap(..) => Vec::new(),
        }
    }

    fn values(&self) -> Vec<&Val> {
        let mut out: Vec<&Val> = match self {
            Self::Return(value) | Self::Cond(value, ..) | Self::Switch(value, _) => vec![value],
            Self::Trap(_, payload) => payload.iter().collect(),
            Self::Br(_) => Vec::new(),
        };
        for edge in self.edges() {
            for arg in &edge.args {
                if let Arg::Val(value) = arg {
                    out.push(value);
                }
            }
        }
        out
    }

    fn values_mut(&mut self) -> Vec<&mut Val> {
        match self {
            Self::Return(value) => vec![value],
            Self::Trap(_, payload) => payload.iter_mut().collect(),
            Self::Br(edge) => edge_args_mut(edge),
            Self::Cond(value, then, other) => {
                let mut out = vec![value];
                out.extend(edge_args_mut(then));
                out.extend(edge_args_mut(other));
                out
            }
            Self::Switch(value, cases) => {
                let mut out = vec![value];
                for (_, edge) in cases {
                    out.extend(edge_args_mut(edge));
                }
                out
            }
        }
    }
}

fn edge_args_mut(edge: &mut Edge) -> Vec<&mut Val> {
    edge.args
        .iter_mut()
        .filter_map(|arg| match arg {
            Arg::Val(value) => Some(value),
            Arg::Payload => None,
        })
        .collect()
}

#[derive(Clone, Debug)]
struct Blk {
    leaf: String,
    params: Vec<(String, TypeExpr)>,
    ops: Vec<Op>,
    term: Term,
    unreachable: bool,
    /// The intent (authored pointer) that first changed or made the block.
    changed: Option<String>,
}

impl Blk {
    fn defines(&self) -> Vec<Val> {
        let mut out: Vec<Val> = self
            .params
            .iter()
            .map(|(name, _)| Val::Block(self.leaf.clone(), name.clone()))
            .collect();
        for op in &self.ops {
            for index in 0..op.types.len().max(1) {
                out.push(Val::Op(
                    self.leaf.clone(),
                    op.leaf.clone(),
                    u32::try_from(index).unwrap_or(u32::MAX),
                ));
            }
        }
        out
    }

    fn uses(&self) -> Vec<&Val> {
        let mut out: Vec<&Val> = self.ops.iter().flat_map(|op| op.args.iter()).collect();
        out.extend(self.term.values());
        out
    }
}

/// A live function's body, as the intents of one frame change it.
#[derive(Clone, Debug)]
struct Func {
    name: String,
    id: EntityId,
    params: Vec<(String, TypeExpr)>,
    result: TypeExpr,
    entry: String,
    blocks: Vec<Blk>,
    effects: bool,
    generic: bool,
    /// Live blocks the intents deleted, with the intent that deleted them.
    deleted: Vec<(String, String)>,
    /// The intent that moved the entry.
    entry_changed: Option<String>,
    /// Generated names and the intent that made them.
    generated: BTreeMap<String, String>,
}

impl Func {
    fn block(&self, leaf: &str) -> Option<&Blk> {
        self.blocks.iter().find(|block| block.leaf == leaf)
    }

    fn position(&self, leaf: &str) -> Option<usize> {
        self.blocks.iter().position(|block| block.leaf == leaf)
    }

    /// Every name in the function's scope: parameters, blocks and values.
    fn taken(&self) -> BTreeSet<String> {
        let mut out: BTreeSet<String> = self.params.iter().map(|(name, _)| name.clone()).collect();
        for block in &self.blocks {
            out.insert(block.leaf.clone());
            out.extend(block.params.iter().map(|(name, _)| name.clone()));
            out.extend(block.ops.iter().map(|op| op.leaf.clone()));
        }
        out.extend(self.deleted.iter().map(|(leaf, _)| leaf.clone()));
        out
    }

    fn changed(&self) -> bool {
        self.entry_changed.is_some()
            || !self.deleted.is_empty()
            || self.blocks.iter().any(|block| block.changed.is_some())
    }

    /// Blocks reachable from the entry.
    fn reachable(&self) -> BTreeSet<String> {
        self.reachable_from(&self.entry)
    }

    /// Blocks reachable from `start`.
    fn reachable_from(&self, start: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![start.to_owned()];
        while let Some(leaf) = stack.pop() {
            if !seen.insert(leaf.clone()) {
                continue;
            }
            if let Some(block) = self.block(&leaf) {
                for edge in block.term.edges() {
                    stack.push(edge.target.clone());
                }
            }
        }
        seen
    }

    fn predecessors(&self, leaf: &str) -> Vec<String> {
        self.blocks
            .iter()
            .filter(|block| block.term.edges().iter().any(|edge| edge.target == leaf))
            .map(|block| block.leaf.clone())
            .collect()
    }
}

/// Where bodies and local names come from: the live program, or the
/// frame's own definitions compiled over it.
trait Source {
    fn body(&self, id: &EntityId) -> Option<&EntityBodyValue>;
    fn leaf(&self, id: &EntityId) -> String;
}

struct Live<'a> {
    program: &'a Program,
    names: &'a Names,
}

impl Source for Live<'_> {
    fn body(&self, id: &EntityId) -> Option<&EntityBodyValue> {
        self.program.body(id)
    }

    fn leaf(&self, id: &EntityId) -> String {
        self.names.leaf(id)
    }
}

/// The frame's own definitions compiled over the program: what the
/// candidate holds for the entities the frame states (bodies by identity,
/// names of what it creates).
#[derive(Default)]
struct FrameState {
    bodies: BTreeMap<EntityId, EntityBodyValue>,
    deleted: BTreeSet<EntityId>,
    names: NameMap,
    /// Functions the frame creates, by name.
    created: BTreeMap<String, EntityId>,
}

/// The program as the frame leaves it: the frame's definitions over the
/// live program.
struct View<'s, 'a> {
    live: Live<'a>,
    state: &'s FrameState,
}

impl Source for View<'_, '_> {
    fn body(&self, id: &EntityId) -> Option<&EntityBodyValue> {
        if self.state.deleted.contains(id) {
            return None;
        }
        self.state.bodies.get(id).or_else(|| self.live.body(id))
    }

    fn leaf(&self, id: &EntityId) -> String {
        self.state
            .names
            .get(id.as_bytes())
            .map_or_else(|| self.live.leaf(id), str::to_owned)
    }
}

impl View<'_, '_> {
    /// The function named `name` as the frame leaves the program.
    fn function(&self, name: &str) -> Option<EntityId> {
        if let Some(id) = self.state.created.get(name) {
            return Some(*id);
        }
        let names = self.live.names;
        let id = names.resolve(name)?;
        (names.scope(&id) == Scope::Top
            && matches!(self.body(&id), Some(EntityBodyValue::Function(_))))
        .then_some(id)
    }
}

fn value_of(source: &dyn Source, function: &FunctionBody, value: &ValueRef) -> Option<Val> {
    match value {
        ValueRef::Parameter(id) => {
            if function.parameters.contains(id) {
                return Some(Val::Param(source.leaf(id)));
            }
            let Some(EntityBodyValue::Parameter(param)) = source.body(id) else {
                return None;
            };
            Some(Val::Block(source.leaf(&param.owner), source.leaf(id)))
        }
        ValueRef::OperationResult(result) => {
            let Some(EntityBodyValue::Operation(op)) = source.body(&result.operation) else {
                return None;
            };
            Some(Val::Op(
                source.leaf(&op.block),
                source.leaf(&result.operation),
                result.result_index,
            ))
        }
    }
}

/// A function's body from `source`; `None` when it is not a well-formed
/// function there.
#[allow(clippy::too_many_lines)]
fn build(source: &dyn Source, id: EntityId, name: &str) -> Option<Func> {
    let Some(EntityBodyValue::Function(function)) = source.body(&id) else {
        return None;
    };
    let value = |v: &ValueRef| value_of(source, function, v);
    let edge = |target: &EntityId, args: &[ValueRef]| -> Option<Edge> {
        Some(Edge {
            target: source.leaf(target),
            args: args
                .iter()
                .map(|arg| value(arg).map(Arg::Val))
                .collect::<Option<_>>()?,
        })
    };
    let target = |edge_value: &TargetEdge| edge(&edge_value.target, &edge_value.arguments);
    let mut params = Vec::new();
    for param in &function.parameters {
        let Some(EntityBodyValue::Parameter(body)) = source.body(param) else {
            return None;
        };
        params.push((source.leaf(param), body.value_type.clone()));
    }
    let mut blocks = Vec::new();
    for block_id in &function.blocks {
        let Some(EntityBodyValue::Block(block)) = source.body(block_id) else {
            return None;
        };
        let mut block_params = Vec::new();
        for param in &block.parameters {
            let Some(EntityBodyValue::Parameter(body)) = source.body(param) else {
                return None;
            };
            block_params.push((source.leaf(param), body.value_type.clone()));
        }
        let mut ops = Vec::new();
        for op_id in &block.operations {
            let Some(EntityBodyValue::Operation(op)) = source.body(op_id) else {
                return None;
            };
            let imm = match &op.immediate {
                Immediate::None => Imm::None,
                Immediate::Entity(entity) => Imm::Entity(*entity),
                Immediate::Index(index) => Imm::Index(*index),
                Immediate::Field(member) => Imm::Field(*member),
                Immediate::Variant(variant) => Imm::Variant(*variant),
                Immediate::Observation(bytes) => Imm::Observation(*bytes),
                Immediate::Function(reference) => Imm::Function {
                    id: reference.function,
                    name: source.leaf(&reference.function),
                    generic: !reference.type_arguments.is_empty(),
                },
            };
            ops.push(Op {
                leaf: source.leaf(op_id),
                tag: op.opcode,
                imm,
                args: op.operands.iter().map(value).collect::<Option<_>>()?,
                types: op.result_types.clone(),
            });
        }
        let term = match &block.terminator {
            Terminator::Return(ret) => Term::Return(value(&ret.value)?),
            Terminator::Branch(branch) => Term::Br(target(&branch.edge)?),
            Terminator::CondBranch(cond) => Term::Cond(
                value(&cond.condition)?,
                target(&cond.if_true)?,
                target(&cond.if_false)?,
            ),
            Terminator::VariantSwitch(switch) => {
                let mut cases = Vec::new();
                for case in &switch.cases {
                    let args = case
                        .edge
                        .arguments
                        .iter()
                        .map(|arg| match arg {
                            SwitchArgument::Value(v) => value(v).map(Arg::Val),
                            SwitchArgument::CasePayload => Some(Arg::Payload),
                        })
                        .collect::<Option<_>>()?;
                    cases.push((
                        case.case_key,
                        Edge {
                            target: source.leaf(&case.edge.target),
                            args,
                        },
                    ));
                }
                cases.sort_by_key(|(key, _)| *key);
                Term::Switch(value(&switch.value)?, cases)
            }
            Terminator::Trap(trap) => Term::Trap(
                trap.code,
                match &trap.payload {
                    Some(payload) => Some(value(payload)?),
                    None => None,
                },
            ),
        };
        blocks.push(Blk {
            leaf: source.leaf(block_id),
            params: block_params,
            ops,
            term,
            unreachable: block.reachability == Reachability::ExplicitlyUnreachable,
            changed: None,
        });
    }
    Some(Func {
        name: name.to_owned(),
        id,
        params,
        result: function.result_type.clone(),
        entry: source.leaf(&function.entry_block),
        blocks,
        effects: !function.effects.as_slice().is_empty(),
        generic: !function.type_parameters.is_empty(),
        deleted: Vec::new(),
        entry_changed: None,
        generated: BTreeMap::new(),
    })
}

// ---------------------------------------------------------------------------
// Rendering a changed block back to plain AF1
// ---------------------------------------------------------------------------

fn render_val(block: &str, value: &Val) -> std::result::Result<String, String> {
    Ok(match value {
        Val::Param(name) => name.clone(),
        Val::Block(owner, name) if owner == block => name.clone(),
        Val::Block(owner, name) => {
            return Err(format!(
                "it uses `{name}`, a parameter of block `{owner}`, which AF1 cannot name from another block"
            ));
        }
        Val::Op(owner, name, index) => {
            let mut text = if owner == block {
                name.clone()
            } else {
                format!("{owner}.{name}")
            };
            if *index != 0 {
                let _ = write!(text, "#{index}");
            }
            text
        }
    })
}

fn render_edge(block: &str, edge: &Edge) -> std::result::Result<Vec<Value>, String> {
    let mut items = vec![Value::from(edge.target.as_str())];
    for arg in &edge.args {
        items.push(match arg {
            Arg::Val(value) => Value::from(render_val(block, value)?),
            Arg::Payload => Value::from("$"),
        });
    }
    Ok(items)
}

fn trap_word(code: TrapCode) -> &'static str {
    match code {
        TrapCode::Unreachable => "unreachable",
        TrapCode::ResourceExhausted => "resource_exhausted",
        TrapCode::AdapterContractViolation => "adapter_contract_violation",
        TrapCode::InternalInvariant => "internal_invariant",
    }
}

/// A block as a plain AF1 patch block (without its name).
#[allow(clippy::too_many_lines)]
fn render_block(cx: &Context<'_>, block: &Blk) -> std::result::Result<Value, String> {
    let names = cx.names;
    let leaf = block.leaf.as_str();
    let mut ops = Vec::new();
    for op in &block.ops {
        let row = opcodes::by_tag(op.tag)
            .ok_or_else(|| format!("operation `{}` has an unknown opcode", op.leaf))?;
        let mut args = Vec::new();
        match &op.imm {
            Imm::None => {}
            Imm::Entity(id) => args.push(Value::from(names.name(id))),
            Imm::Index(index) => args.push(Value::from(*index)),
            Imm::Field(member) => args.push(Value::from(names.member_any(member))),
            Imm::Variant(variant) => args.push(Value::from(
                names.member(&variant.definition, &variant.member_id),
            )),
            Imm::Observation(bytes) => args.push(Value::from(crate::hex::encode(bytes))),
            Imm::Function { name, generic, .. } => {
                if *generic {
                    return Err(format!(
                        "operation `{}` calls a function with type arguments",
                        op.leaf
                    ));
                }
                args.push(Value::from(name.as_str()));
            }
            Imm::Literal(value) => args.push(value.clone()),
        }
        for arg in &op.args {
            args.push(Value::from(render_val(leaf, arg)?));
        }
        let mut object = Map::new();
        object.insert("name".to_owned(), Value::from(op.leaf.as_str()));
        object.insert("op".to_owned(), Value::from(row.mnemonic));
        object.insert("args".to_owned(), Value::Array(args));
        match op.types.as_slice() {
            [ty] => {
                object.insert("type".to_owned(), Value::from(cx.render(ty)));
            }
            [] => {}
            _ => {
                return Err(format!(
                    "operation `{}` has several results, which AF1 cannot state",
                    op.leaf
                ));
            }
        }
        ops.push(Value::Object(object));
    }
    let value = |v: &Val| render_val(leaf, v).map(Value::from);
    let edge = |e: &Edge| -> std::result::Result<Value, String> {
        let items = render_edge(leaf, e)?;
        Ok(if items.len() == 1 {
            items[0].clone()
        } else {
            Value::Array(items)
        })
    };
    let term = match &block.term {
        Term::Return(v) => json!(["return", value(v)?]),
        Term::Br(e) => {
            let mut items = vec![Value::from("br")];
            items.extend(render_edge(leaf, e)?);
            Value::Array(items)
        }
        Term::Cond(c, then, other) => json!(["cond", value(c)?, edge(then)?, edge(other)?]),
        Term::Switch(v, cases) => {
            let mut items = vec![Value::from("switch"), value(v)?];
            for (key, e) in cases {
                let key = match key {
                    CaseKey::Builtin(BuiltinCase::Ok) => "Ok".to_owned(),
                    CaseKey::Builtin(BuiltinCase::Err) => "Err".to_owned(),
                    CaseKey::Builtin(BuiltinCase::Some) => "Some".to_owned(),
                    CaseKey::Builtin(BuiltinCase::None) => "None".to_owned(),
                    CaseKey::Member(member) => names.member_any(member),
                };
                let mut case = vec![Value::from(key)];
                case.extend(render_edge(leaf, e)?);
                items.push(Value::Array(case));
            }
            Value::Array(items)
        }
        Term::Trap(code, payload) => {
            let mut items = vec![Value::from("trap"), Value::from(trap_word(*code))];
            if let Some(payload) = payload {
                items.push(value(payload)?);
            }
            Value::Array(items)
        }
    };
    let mut object = Map::new();
    let params: Vec<Value> = block
        .params
        .iter()
        .map(|(name, ty)| json!([name, cx.render(ty)]))
        .collect();
    object.insert("params".to_owned(), Value::Array(params));
    object.insert("ops".to_owned(), Value::Array(ops));
    object.insert("term".to_owned(), term);
    if block.unreachable {
        object.insert("unreachable".to_owned(), Value::Bool(true));
    }
    Ok(Value::Object(object))
}

// ---------------------------------------------------------------------------
// The frame as its author wrote it
// ---------------------------------------------------------------------------

/// What the author's frame restates, read before any derivation.
#[derive(Default)]
struct Authored {
    /// Functions defined whole: name to (list key, index).
    fns: BTreeMap<String, (String, usize)>,
    /// Functions patched: name to index in `patch`.
    patch: BTreeMap<String, usize>,
    /// Functions edited: name to the edited `block.op` paths.
    edit: BTreeMap<String, BTreeSet<String>>,
    /// Names the frame deletes.
    deleted: BTreeSet<String>,
    /// Tests the frame states, by name.
    tests: BTreeSet<String>,
}

fn fn_name(decl: &Value) -> Option<&str> {
    decl.get("fn")
        .or_else(|| decl.get("name"))
        .or_else(|| decl.get("function"))
        .and_then(Value::as_str)
}

impl Authored {
    fn read(out: &Map<String, Value>) -> Self {
        let mut authored = Self::default();
        let list = |key: &str| {
            out.get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        for key in ["fns", "functions"] {
            for (index, decl) in list(key).iter().enumerate() {
                if let Some(name) = fn_name(decl) {
                    authored
                        .fns
                        .entry(name.to_owned())
                        .or_insert((key.to_owned(), index));
                }
            }
        }
        for (index, decl) in list("patch").iter().enumerate() {
            if let Some(name) = fn_name(decl) {
                authored.patch.entry(name.to_owned()).or_insert(index);
            }
        }
        for decl in &list("edit") {
            if let Some(name) = fn_name(decl) {
                let path = decl
                    .get("replace_op")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                authored
                    .edit
                    .entry(name.to_owned())
                    .or_default()
                    .insert(path.to_owned());
            }
        }
        for name in list("delete") {
            if let Some(name) = name.as_str() {
                authored.deleted.insert(name.to_owned());
            }
        }
        for test in &list("tests") {
            if let Some(name) = test.get("name").and_then(Value::as_str) {
                authored.tests.insert(name.to_owned());
            }
        }
        authored
    }

    fn restates(&self, name: &str) -> bool {
        self.fns.contains_key(name) || self.patch.contains_key(name) || self.edit.contains_key(name)
    }

    fn gone(&self, name: &str) -> bool {
        self.deleted.contains(name) && !self.fns.contains_key(name)
    }
}

/// A call (or `fnref`) the frame itself writes.
struct FrameCall {
    /// The function it is in.
    function: String,
    /// Pointer of the block object (`None` for an `edit.with`).
    block: Option<String>,
    /// The block's name (for an `edit.with`, the edited block).
    block_leaf: String,
    /// Pointer of the operation.
    pointer: String,
    /// Operation index in its block.
    index: usize,
    /// Operation name.
    name: String,
    fnref: bool,
    args: Vec<Value>,
}

/// `(tag, callee, args, name)` of an operation that names a function.
fn call_parts(op: &Value) -> Option<(u32, String, Vec<Value>, String)> {
    let (name, word, items): (String, &str, Vec<Value>) = match op {
        Value::Array(items) if items.len() >= 2 => (
            items[0].as_str()?.to_owned(),
            items[1].as_str()?,
            items[2..].to_vec(),
        ),
        Value::Object(object) => (
            object
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            object
                .get("op")
                .or_else(|| object.get("opcode"))
                .and_then(Value::as_str)?,
            object
                .get("args")
                .or_else(|| object.get("operands"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        ),
        _ => return None,
    };
    let row = opcodes::by_word(word)?;
    if !matches!(row.tag, 112 | 194) {
        return None;
    }
    let callee = items.first()?.as_str()?.to_owned();
    Some((row.tag, callee, items[1..].to_vec(), name))
}

/// Replaces the arguments after the callee of a call operation.
fn set_call_args(op: &mut Value, args: Vec<Value>) {
    match op {
        Value::Array(items) => {
            items.truncate(3);
            items.extend(args);
        }
        Value::Object(object) => {
            let key = if object.contains_key("args") {
                "args"
            } else {
                "operands"
            };
            if let Some(Value::Array(items)) = object.get_mut(key) {
                items.truncate(1);
                items.extend(args);
            }
        }
        _ => {}
    }
}

/// The value at `pointer` in the frame object.
fn at_mut<'v>(root: &'v mut Map<String, Value>, pointer: &str) -> Option<&'v mut Value> {
    let rest = pointer.strip_prefix('/')?;
    let (first, tail) = rest.find('/').map_or((rest, ""), |at| rest.split_at(at));
    let value = root.get_mut(first)?;
    if tail.is_empty() {
        Some(value)
    } else {
        value.pointer_mut(tail)
    }
}

fn op_name(op: &Value) -> Option<&str> {
    match op {
        Value::Array(items) => items.first().and_then(Value::as_str),
        Value::Object(object) => object.get("name").and_then(Value::as_str),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The derivation state
// ---------------------------------------------------------------------------

/// How an argument of a rewritten call is found.
#[derive(Clone, Debug)]
enum Slot {
    /// The old argument at this position.
    Keep(usize),
    /// A new parameter.
    New,
    /// A parameter kept by name whose type changed: old position, old type.
    Retyped(usize, TypeExpr),
}

/// The supplied value of a new parameter.
struct Fill {
    /// Position of the new parameter.
    position: usize,
    /// The literal as a `const` immediate: `{"type": T, "value": v}`.
    constant: Value,
    /// The literal as a test argument.
    data: Value,
}

/// What an arity intent states besides its target.
struct Stated<'s> {
    value: Option<&'s Value>,
    /// `"frame_calls"`: `Some(true)` old, `Some(false)` new.
    declared: Option<bool>,
    after: &'s BTreeSet<String>,
}

/// One guard intent.
struct Guard<'g> {
    checker: &'g str,
    arg: &'g str,
    functions: &'g [String],
    entry: bool,
    handler: Option<&'g str>,
}

/// A checker function: its body and its `P -> Result<P,E>` shape.
struct Checker {
    name: String,
    /// Its identity in the program as the frame leaves it.
    id: EntityId,
    func: Func,
    param: TypeExpr,
    error: TypeExpr,
    effects: bool,
}

struct Ripple<'c, 'a> {
    cx: &'c Context<'a>,
    out: &'c mut Map<String, Value>,
    map: &'c mut SourceMap,
    authored: Authored,
    funcs: BTreeMap<String, Func>,
    tests: BTreeMap<String, (Value, String)>,
    holes: Vec<Obligation>,
    edits: u64,
    records: Vec<Value>,
    /// Arity targets derived so far, with their intent.
    arities: BTreeMap<String, String>,
    /// The frame's test tables by position, by name.
    tables: Vec<String>,
}

impl<'c, 'a> Ripple<'c, 'a> {
    fn new(cx: &'c Context<'a>, out: &'c mut Map<String, Value>, map: &'c mut SourceMap) -> Self {
        let authored = Authored::read(out);
        Self {
            cx,
            out,
            map,
            authored,
            funcs: BTreeMap::new(),
            tests: BTreeMap::new(),
            holes: Vec::new(),
            edits: 0,
            records: Vec::new(),
            arities: BTreeMap::new(),
            tables: Vec::new(),
        }
    }

    fn live(&self) -> Live<'a> {
        Live {
            program: self.cx.program,
            names: self.cx.names,
        }
    }

    fn hole(&mut self, symbol: AgentErrorCode, at: &str, decision: impl Into<String>) {
        self.holes.push(Obligation::new(symbol, at, decision));
    }

    fn render(&self, ty: &TypeExpr) -> String {
        self.cx.render(ty)
    }

    fn params_text(&self, params: &[(String, TypeExpr)]) -> Vec<String> {
        params
            .iter()
            .map(|(name, ty)| format!("{name}: {}", self.render(ty)))
            .collect()
    }

    /// The live top-level function `name`, or why it is not one.
    fn live_target(&self, name: &str) -> std::result::Result<(EntityId, &'a FunctionBody), String> {
        if self.authored.gone(name) {
            return Err(format!("`{name}` is deleted by this frame"));
        }
        let names = self.cx.names;
        match names.resolve(name) {
            Some(id) if names.scope(&id) == Scope::Top => match self.cx.program.body(&id) {
                Some(EntityBodyValue::Function(body)) => Ok((id, body)),
                Some(other) => Err(format!(
                    "`{name}` is a {}, not a function",
                    kind_name(other.kind_tag())
                )),
                None => Err(format!("no function named `{name}`")),
            },
            _ if self.authored.fns.contains_key(name) => Err(format!(
                "`{name}` is created by this frame; ripple changes live functions and the code that uses them"
            )),
            _ => Err(format!("no function named `{name}`")),
        }
    }

    /// The body of live function `name` as the intents so far left it.
    fn func(&mut self, name: &str) -> Option<&mut Func> {
        if !self.funcs.contains_key(name) {
            let (id, _) = self.live_target(name).ok()?;
            let func = build(&self.live(), id, name)?;
            self.funcs.insert(name.to_owned(), func);
        }
        self.funcs.get_mut(name)
    }

    /// Whether a top-level name is taken (by the frame or the program).
    fn top_taken(&self, name: &str) -> bool {
        self.cx.top.contains(name)
            || self
                .cx
                .names
                .resolve(name)
                .is_some_and(|id| self.cx.names.scope(&id) == Scope::Top)
    }

    fn unique(&self, base: &str, taken: &BTreeSet<String>) -> String {
        let fit = |name: String| {
            if name.len() <= MAX_NAME {
                name
            } else {
                shorten(&name)
            }
        };
        let free = |name: &String| !taken.contains(name) && !self.top_taken(name);
        let first = fit(base.to_owned());
        if free(&first) {
            return first;
        }
        (2..=u32::MAX)
            .map(|n| fit(format!("{base}_{n}")))
            .find(free)
            .unwrap_or(first)
    }

    /// The namespaces that list `id`, by name.
    fn namespaces_of(&self, id: &EntityId) -> Vec<String> {
        let mut out: Vec<String> = self
            .cx
            .program
            .objects()
            .iter()
            .filter_map(|object| match &object.record().body {
                EntityBodyValue::Namespace(namespace)
                    if namespace.members.as_slice().contains(id) =>
                {
                    Some(self.cx.names.name(&object.record().entity_id))
                }
                _ => None,
            })
            .collect();
        out.sort();
        out
    }

    /// References to `id` outside calls and `TestCases`: code outside the
    /// program's call graph that uses the function's parameters.
    fn references(&self, id: &EntityId) -> Vec<String> {
        let names = self.cx.names;
        // A package that exports a namespace holding `id` (at any depth) is
        // read as exporting `id`: the specifications do not say otherwise.
        let mut holders = BTreeSet::from([*id]);
        loop {
            let before = holders.len();
            for object in self.cx.program.objects() {
                if let EntityBodyValue::Namespace(namespace) = &object.record().body
                    && namespace
                        .members
                        .as_slice()
                        .iter()
                        .any(|member| holders.contains(member))
                {
                    holders.insert(object.record().entity_id);
                }
            }
            if holders.len() == before {
                break;
            }
        }
        let mut out = Vec::new();
        for object in self.cx.program.objects() {
            let record = object.record();
            let name = names.name(&record.entity_id);
            let exported = |package: &sley_mutate::value::PackageBody| {
                package
                    .exports
                    .as_slice()
                    .iter()
                    .find(|export| holders.contains(export))
                    .copied()
            };
            let what = match &record.body {
                EntityBodyValue::EntryPoint(entry) if entry.function == *id => {
                    "the entry point".to_owned()
                }
                EntityBodyValue::Package(package) if exported(package).is_some() => {
                    match exported(package) {
                        Some(export) if export != *id => format!(
                            "exported (in namespace `{}`) by the package",
                            names.name(&export)
                        ),
                        _ => "exported by the package".to_owned(),
                    }
                }
                EntityBodyValue::GlobalValue(global) if global.initializer == *id => {
                    "the initializer of the global".to_owned()
                }
                EntityBodyValue::Contract(contract)
                    if contract.target == *id || contract.predicate == *id =>
                {
                    "named by the contract".to_owned()
                }
                EntityBodyValue::PolicyBinding(binding) if binding.subject == *id => {
                    "the subject of the policy binding".to_owned()
                }
                _ => continue,
            };
            out.push(format!("{what} `{name}`"));
        }
        out
    }

    // --- arity ------------------------------------------------------------

    /// The parameters the frame restates for `name`: the value and its
    /// pointer.
    fn restated_params(&self, name: &str) -> Option<(Value, String)> {
        if let Some((key, index)) = self.authored.fns.get(name) {
            let params = self.out.get(key)?.get(*index)?.get("params")?;
            return Some((params.clone(), format!("/{key}/{index}/params")));
        }
        let index = *self.authored.patch.get(name)?;
        let params = self.out.get("patch")?.get(index)?.get("params")?;
        Some((params.clone(), format!("/patch/{index}/params")))
    }

    /// The supplied value, checked against the one new parameter.
    fn fill(
        &mut self,
        value: &Value,
        new: &[(String, TypeExpr)],
        added: &[usize],
        target: &str,
        at: &str,
    ) -> Option<Fill> {
        let pointer = format!("{at}/value");
        let [position] = added else {
            let listed: Vec<String> = added
                .iter()
                .map(|position| format!("{}: {}", new[*position].0, self.render(&new[*position].1)))
                .collect();
            self.hole(
                AgentErrorCode::RippleHoleUnfilled,
                &pointer,
                if listed.is_empty() {
                    format!("`{target}` gains no parameter, so \"value\" fills nothing: remove it")
                } else {
                    format!(
                        "\"value\" fills one new parameter; `{target}` gains {} ({}): write the calls, or add the parameters in separate frames",
                        listed.len(),
                        listed.join(", ")
                    )
                },
            );
            return None;
        };
        let (param, ty) = &new[*position];
        match self.literal(value, param, ty, target) {
            Ok((constant, data)) => Some(Fill {
                position: *position,
                constant,
                data,
            }),
            Err(decision) => {
                self.hole(AgentErrorCode::RippleHoleUnfilled, &pointer, decision);
                None
            }
        }
    }

    /// The supplied value as a literal of parameter `param` of type `ty`:
    /// the `const` immediate `{"type": T, "value": v}` and the test
    /// argument `v`; or why it is not one.
    fn literal(
        &self,
        value: &Value,
        param: &str,
        ty: &TypeExpr,
        target: &str,
    ) -> std::result::Result<(Value, Value), String> {
        let rendered = self.render(ty);
        let (stated, data) = match value {
            Value::Object(object)
                if object.len() == 2
                    && object.contains_key("type")
                    && object.contains_key("value") =>
            {
                (Some(&object["type"]), object["value"].clone())
            }
            Value::Number(_) | Value::Bool(_) => (None, value.clone()),
            _ => {
                return Err(format!(
                    "state the value of new parameter `{param}` (expected {rendered}) as a literal: 3, true, or {{\"type\": \"{rendered}\", \"value\": ...}}"
                ));
            }
        };
        if let Some(stated) = stated {
            match crate::types::read(stated, self.cx, "") {
                Ok(read) if read == *ty => {}
                _ => {
                    return Err(format!(
                        "the value's type is {stated}, but new parameter `{param}` of `{target}` is {rendered} (expected {rendered})"
                    ));
                }
            }
        } else {
            let fits = match (&data, ty) {
                (Value::Bool(_), TypeExpr::Bool)
                | (Value::Number(_), TypeExpr::F32 | TypeExpr::F64) => true,
                (Value::Number(number), TypeExpr::SInt(_) | TypeExpr::UInt(_)) => {
                    number.is_i64() || number.is_u64()
                }
                _ => false,
            };
            if !fits {
                return Err(format!(
                    "{data} is not a literal of new parameter `{param}`'s type (expected {rendered}): state it as {{\"type\": \"{rendered}\", \"value\": ...}}"
                ));
            }
        }
        // The literal must decode as the parameter type (a live type; a type
        // the frame declares is checked by the compiler).
        if !mentions_frame_type(ty, &self.cx.type_names) {
            let defs = crate::values::ProgramTypes {
                program: self.cx.program,
                names: self.cx.names,
            };
            if let Err(error) = crate::values::read(&data, ty, &defs, "") {
                return Err(format!(
                    "the value does not fit new parameter `{param}` (expected {rendered}): {}",
                    error.detail().trim_start_matches(": ")
                ));
            }
        }
        Ok((json!({"type": rendered, "value": data}), data))
    }

    #[allow(clippy::too_many_lines)]
    fn arity(&mut self, index: usize, target: &str, stated: &Stated<'_>) {
        let (value, declared) = (stated.value, stated.declared);
        let at = format!("/ripple/{index}");
        if let Some(first) = self.arities.get(target) {
            self.hole(
                AgentErrorCode::RippleHoleUnfilled,
                &format!("{at}/arity"),
                format!("the parameters of `{target}` are already propagated by the intent at {first}: keep one intent per function"),
            );
            return;
        }
        self.arities.insert(target.to_owned(), at.clone());
        let (f_id, f_body) = match self.live_target(target) {
            Ok(found) => found,
            Err(why) => {
                self.hole(
                    AgentErrorCode::RippleTargetKind,
                    &format!("{at}/arity"),
                    why,
                );
                return;
            }
        };
        if !f_body.type_parameters.is_empty() {
            self.hole(
                AgentErrorCode::RippleTargetKind,
                &format!("{at}/arity"),
                format!(
                    "`{target}` has type parameters; ripple rewrites calls of non-generic functions"
                ),
            );
            return;
        }
        let old: Vec<(String, TypeExpr)> = f_body
            .parameters
            .iter()
            .map(|param| {
                (
                    self.cx.names.leaf(param),
                    self.cx.parameter_type(param).unwrap_or(TypeExpr::Unit),
                )
            })
            .collect();
        let Some((params_value, _)) = self.restated_params(target) else {
            self.hole(
                AgentErrorCode::RippleHoleUnfilled,
                &format!("{at}/arity"),
                format!(
                    "the frame does not restate the parameters of `{target}`: give its new \"params\" in fns or patch"
                ),
            );
            return;
        };
        // A malformed parameter list or type is the compiler's to report.
        let Some(read) = read_params(Some(&params_value), self.cx) else {
            return;
        };
        let mut new = Vec::new();
        for (name, ty, _) in read {
            let Some(ty) = ty else { return };
            new.push((name, ty));
        }
        if new == old {
            // The head already has these parameters: the intent was applied
            // (by a committed revision this frame is layered on, typically).
            self.applied(&at, target, f_id, &new, stated);
            return;
        }
        let slots: Vec<Slot> = new
            .iter()
            .map(
                |(name, ty)| match old.iter().position(|(old_name, _)| old_name == name) {
                    Some(position) if old[position].1 == *ty => Slot::Keep(position),
                    Some(position) => Slot::Retyped(position, old[position].1.clone()),
                    None => Slot::New,
                },
            )
            .collect();
        let added: Vec<usize> = slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| matches!(slot, Slot::New))
            .map(|(position, _)| position)
            .collect();
        let removed: Vec<String> = old
            .iter()
            .filter(|(name, _)| !new.iter().any(|(new_name, _)| new_name == name))
            .map(|(name, _)| name.clone())
            .collect();
        let fill = match value {
            Some(value) => match self.fill(value, &new, &added, target, &at) {
                Some(fill) => Some(fill),
                None => return,
            },
            None => None,
        };
        let references = self.references(&f_id);
        for reference in &references {
            self.hole(
                AgentErrorCode::RippleExportedBoundary,
                &format!("{at}/arity"),
                format!(
                    "`{target}` is {reference}: code outside the program's calls invokes it with its current parameters, so ripple does not change them; change `{target}` and its users without ripple"
                ),
            );
        }
        if !references.is_empty() {
            return;
        }
        let home = self.namespaces_of(&f_id);
        let values = self.function_values(&f_id);
        let frame_calls = self.frame_calls(target, &f_id);
        let frame_tests = self.frame_tests(target);
        let live_calls = self.live_calls(&f_id);
        let live_tests = self.live_tests(&f_id);
        let total = frame_calls.len() + frame_tests.len() + live_calls.len() + live_tests.len();
        if total > MAX_RIPPLE_SITES {
            self.hole(
                AgentErrorCode::RippleLimit,
                &at,
                format!(
                    "`{target}` has {total} call sites and tests; one intent rewrites at most {MAX_RIPPLE_SITES}: split the change"
                ),
            );
            return;
        }
        let context = Arity {
            target: target.to_owned(),
            at: at.clone(),
            new: new.clone(),
            slots,
            fill,
        };
        let mut calls = Vec::new();
        let mut dispatch = Vec::new();
        let mut changed_fns: BTreeSet<String> = BTreeSet::new();
        let mut changed_tests: BTreeSet<String> = BTreeSet::new();
        // Calls the frame writes, in reverse order within each block so an
        // inserted constant never moves a site not yet rewritten.
        let mut ordered: Vec<&FrameCall> = frame_calls.iter().collect();
        ordered.sort_by(|a, b| {
            (a.block.as_deref(), std::cmp::Reverse(a.index))
                .cmp(&(b.block.as_deref(), std::cmp::Reverse(b.index)))
        });
        let mut frame_records = Vec::new();
        let mut outside = Vec::new();
        for holder in &values {
            dispatch.push(Value::from(holder.clone()));
            self.hole(
                AgentErrorCode::RippleHoleUnfilled,
                &at,
                format!(
                    "{holder} holds `{target}` as a function value: calls through it are unresolved dispatch, which ripple does not rewrite; change that use yourself"
                ),
            );
        }
        let reading = |count: usize| reading(declared, count, old.len(), new.len());
        for call in ordered {
            let authored = self
                .map
                .authored(&call.pointer)
                .unwrap_or_else(|| call.pointer.clone());
            let site = format!("{authored} ({})", call.function);
            if call.fnref {
                dispatch.push(Value::from(site.clone()));
                self.hole(
                    AgentErrorCode::RippleHoleUnfilled,
                    &at,
                    format!(
                        "{site} uses `{target}` as a function value (fnref): calls through it are unresolved dispatch, which ripple does not rewrite; change that use yourself"
                    ),
                );
                continue;
            }
            // Stated in a revision after the intent: only the count can tell.
            let reading = |count: usize| {
                if stated.after.contains(&call.function) {
                    revised_reading(count, old.len(), new.len())
                } else {
                    reading(count)
                }
            };
            let edit = match reading(call.args.len()) {
                Reading::AsWritten => "as written",
                Reading::Compiler => "left to the compiler",
                Reading::Ambiguous | Reading::Contradicts | Reading::Revised => {
                    let why = self.unread(
                        reading(call.args.len()),
                        &site,
                        call.args.len(),
                        &old,
                        &new,
                        target,
                    );
                    self.hole(AgentErrorCode::RippleHoleUnfilled, &at, why);
                    "hole"
                }
                Reading::Rewrite => {
                    let caller_home = self.frame_home(&call.function);
                    if caller_home == home {
                        if self.rewrite_frame_call(&context, call, &site) {
                            changed_fns.insert(call.function.clone());
                            self.edits += 1;
                            "rewritten"
                        } else {
                            "hole"
                        }
                    } else {
                        outside.push(Value::from(call.function.clone()));
                        self.hole(
                            AgentErrorCode::RippleExportedBoundary,
                            &at,
                            format!(
                                "{site} calls `{target}` from namespace {} while `{target}` is in {}: ripple does not edit code across a namespace boundary; change that call yourself",
                                namespace_text(&caller_home),
                                namespace_text(&home)
                            ),
                        );
                        "boundary"
                    }
                }
            };
            frame_records.push(json!({"site": site, "origin": "frame", "edit": edit}));
        }
        frame_records.reverse();
        calls.extend(frame_records);
        for (test_index, name) in frame_tests {
            let test = self
                .out
                .get("tests")
                .and_then(|tests| tests.get(test_index));
            let count = test
                .and_then(|test| test.get("args").or_else(|| test.get("inputs")))
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let authored = self
                .map
                .authored(&format!("/tests/{test_index}"))
                .unwrap_or_else(|| format!("/tests/{test_index}"));
            let label = format!("{authored} (`{name}`)");
            let later = self.stated_in(test_index, &name, stated.after);
            let reading = |count: usize| {
                if later {
                    revised_reading(count, old.len(), new.len())
                } else {
                    reading(count)
                }
            };
            let edit = match reading(count) {
                Reading::AsWritten => "as written",
                Reading::Compiler => "left to the compiler",
                Reading::Ambiguous | Reading::Contradicts | Reading::Revised => {
                    let why = self.unread(reading(count), &label, count, &old, &new, target);
                    self.hole(AgentErrorCode::RippleHoleUnfilled, &at, why);
                    "hole"
                }
                Reading::Rewrite => {
                    if self.rewrite_frame_test(&context, test_index, &label) {
                        changed_tests.insert(name.clone());
                        self.edits += 1;
                        "rewritten"
                    } else {
                        "hole"
                    }
                }
            };
            calls.push(json!({"test": name, "origin": "frame", "edit": edit}));
        }
        for site in &live_calls {
            let label = format!("`{}.{}.{}`", site.function, site.block, site.op);
            if site.fnref {
                dispatch.push(Value::from(label.clone()));
                self.hole(
                    AgentErrorCode::RippleHoleUnfilled,
                    &at,
                    format!(
                        "{label} uses `{target}` as a function value (fnref): calls through it are unresolved dispatch, which ripple does not rewrite; change that use yourself"
                    ),
                );
                continue;
            }
            let caller_home = self
                .func(&site.function)
                .map(|func| func.id)
                .map(|id| self.namespaces_of(&id))
                .unwrap_or_default();
            if caller_home != home {
                outside.push(Value::from(site.function.clone()));
                self.hole(
                    AgentErrorCode::RippleExportedBoundary,
                    &at,
                    format!(
                        "{label} calls `{target}` from namespace {} while `{target}` is in {}: ripple does not edit code across a namespace boundary; change that call yourself",
                        namespace_text(&caller_home),
                        namespace_text(&home)
                    ),
                );
                continue;
            }
            if self.authored.edit.contains_key(&site.function) {
                self.hole(
                    AgentErrorCode::RippleHoleUnfilled,
                    &at,
                    format!(
                        "{label} calls `{target}`, and this frame changes `{}` with edit: restate block `{}` with patch so the call can be rewritten",
                        site.function, site.block
                    ),
                );
                continue;
            }
            if let Some(why) = self.funcs.get(&site.function).and_then(unpatchable) {
                self.hole(
                    AgentErrorCode::RippleHoleUnfilled,
                    &at,
                    format!(
                        "{label} calls `{target}`, but `{}` {why}, which a patch cannot restate: change the call yourself",
                        site.function
                    ),
                );
                continue;
            }
            let edit = if self.rewrite_live_call(&context, site, &label) {
                changed_fns.insert(site.function.clone());
                self.edits += 1;
                "rewritten"
            } else {
                "hole"
            };
            calls.push(json!({"site": label.trim_matches('`'), "origin": "live", "edit": edit}));
        }
        let mut tests = Vec::new();
        for (test_id, name) in &live_tests {
            let edit = if self.rewrite_live_test(&context, test_id, name) {
                changed_tests.insert(name.clone());
                self.edits += 1;
                "rewritten"
            } else {
                "hole"
            };
            tests.push(json!({"test": name, "origin": "live", "edit": edit}));
        }
        let params: Vec<Value> = new
            .iter()
            .zip(&context.slots)
            .map(|((name, _), slot)| match slot {
                Slot::Keep(position) => json!({"param": name, "from": old[*position].0}),
                Slot::New => match &context.fill {
                    Some(fill) => json!({"param": name, "value": fill.data}),
                    None => json!({"param": name, "value": null}),
                },
                Slot::Retyped(position, _) => {
                    json!({"param": name, "from": old[*position].0, "retyped": true})
                }
            })
            .collect();
        self.records.push(json!({
            "at": at,
            "intent": "arity",
            "target": target,
            "old": self.params_text(&old),
            "new": self.params_text(&new),
            "params": params,
            "removed": removed,
            "calls": calls,
            "tests": tests,
            "dispatch": dispatch,
            "boundary": {"namespaces": home, "outside": outside, "references": references},
            "changed": {"functions": changed_fns, "tests": changed_tests},
        }));
    }

    /// Calls and function values of `target` the frame's own functions
    /// write (after expansion), and in its edits.
    #[allow(clippy::too_many_lines)]
    fn frame_calls(&self, target: &str, id: &EntityId) -> Vec<FrameCall> {
        let names = self.cx.names;
        let is_target = |callee: &str| {
            callee == target
                || (!self.cx.top.contains(callee) && names.resolve(callee) == Some(*id))
        };
        let mut out = Vec::new();
        let mut scan = |function: &str, block_pointer: &str, leaf: &str, block: &Value| {
            let Some(ops) = block.get("ops").and_then(Value::as_array) else {
                return;
            };
            for (index, op) in ops.iter().enumerate() {
                let Some((tag, callee, args, name)) = call_parts(op) else {
                    continue;
                };
                if !is_target(&callee) {
                    continue;
                }
                out.push(FrameCall {
                    function: function.to_owned(),
                    block: Some(block_pointer.to_owned()),
                    block_leaf: leaf.to_owned(),
                    pointer: format!("{block_pointer}/ops/{index}"),
                    index,
                    name,
                    fnref: tag == 194,
                    args,
                });
            }
        };
        for key in ["fns", "functions"] {
            let Some(list) = self.out.get(key).and_then(Value::as_array) else {
                continue;
            };
            for (k, decl) in list.iter().enumerate() {
                let Some(function) = fn_name(decl) else {
                    continue;
                };
                let Some(blocks) = decl.get("blocks").and_then(Value::as_array) else {
                    continue;
                };
                for (b, block) in blocks.iter().enumerate() {
                    let leaf = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    scan(function, &format!("/{key}/{k}/blocks/{b}"), leaf, block);
                }
            }
        }
        if let Some(list) = self.out.get("patch").and_then(Value::as_array) {
            for (k, decl) in list.iter().enumerate() {
                let Some(function) = fn_name(decl) else {
                    continue;
                };
                let Some(blocks) = decl.get("blocks").and_then(Value::as_object) else {
                    continue;
                };
                for (leaf, block) in blocks {
                    if block.is_object() {
                        scan(function, &format!("/patch/{k}/blocks/{leaf}"), leaf, block);
                    }
                }
            }
        }
        if let Some(list) = self.out.get("edit").and_then(Value::as_array) {
            for (k, decl) in list.iter().enumerate() {
                let Some(function) = fn_name(decl) else {
                    continue;
                };
                let Some(with) = decl.get("with") else {
                    continue;
                };
                let mut item = with.clone();
                if let Value::Array(items) = &mut item {
                    items.insert(0, Value::from("with"));
                }
                let Some((tag, callee, args, _)) = call_parts(&item) else {
                    continue;
                };
                if !is_target(&callee) {
                    continue;
                }
                let (block_leaf, name) = decl
                    .get("replace_op")
                    .and_then(Value::as_str)
                    .and_then(|path| path.split_once('.'))
                    .map(|(block, op)| (block.to_owned(), op.to_owned()))
                    .unwrap_or_default();
                out.push(FrameCall {
                    function: function.to_owned(),
                    block: None,
                    block_leaf,
                    pointer: format!("/edit/{k}/with"),
                    index: 0,
                    name,
                    fnref: tag == 194,
                    args,
                });
            }
        }
        out
    }

    /// Tests the frame states for `target`: (index in `tests`, name).
    fn frame_tests(&self, target: &str) -> Vec<(usize, String)> {
        let Some(list) = self.out.get("tests").and_then(Value::as_array) else {
            return Vec::new();
        };
        list.iter()
            .enumerate()
            .filter(|(_, test)| {
                test.get("fn")
                    .or_else(|| test.get("target"))
                    .or_else(|| test.get("function"))
                    .and_then(Value::as_str)
                    == Some(target)
            })
            .map(|(index, test)| {
                (
                    index,
                    test.get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(crate::layer::UNNAMED_TESTS)
                        .to_owned(),
                )
            })
            .collect()
    }

    /// Whether the frame test at `index` in `tests`, or the table its row
    /// was lowered from, is in `names`.
    fn stated_in(&self, index: usize, name: &str, names: &BTreeSet<String>) -> bool {
        if names.contains(name) {
            return true;
        }
        let Some(row) = self.map.authored(&format!("/tests/{index}")) else {
            return false;
        };
        row.strip_prefix("/test_tables/")
            .and_then(|rest| rest.split('/').next())
            .and_then(|table| table.parse::<usize>().ok())
            .and_then(|table| self.tables.get(table))
            .is_some_and(|table| names.contains(table))
    }

    /// An arity intent whose restated parameters `f` already has at the
    /// head: it was applied (by a committed revision this frame is layered
    /// on, typically). Deriving it again changes only what differs from what
    /// that derivation left: its unambiguous value fill, and the frame's
    /// carried calls and tests. An explicit edit after the intent is a hole
    /// when its old spelling conflicts with the committed call.
    #[allow(clippy::too_many_lines)]
    fn applied(
        &mut self,
        at: &str,
        target: &str,
        f_id: EntityId,
        params: &[(String, TypeExpr)],
        stated: &Stated<'_>,
    ) {
        let mut calls = Vec::new();
        let mut changed_fns: BTreeSet<String> = BTreeSet::new();
        let mut changed_tests: BTreeSet<String> = BTreeSet::new();
        let mut edits = 0_u64;
        let holes = self.holes.len();
        // The value the committed fills load.
        if let Some(value) = stated.value {
            for site in self.live_calls(&f_id) {
                if site.fnref {
                    continue;
                }
                let label = format!("`{}.{}.{}`", site.function, site.block, site.op);
                let fills = self.fills(&site.function, &site.block, &site.op);
                if fills.len() > 1 {
                    let matching = fills
                        .iter()
                        .filter(|(position, fill)| {
                            params.get(*position).is_some_and(|(param, ty)| {
                                self.literal(value, param, ty, target)
                                    .is_ok_and(|(_, data)| {
                                        self.loads(&site.function, &site.block, fill, ty, &data)
                                    })
                            })
                        })
                        .count();
                    if matching == 0 {
                        self.hole(
                            AgentErrorCode::RippleHoleUnfilled,
                            &format!("{at}/value"),
                            format!("{label} has several generated value fills: which one this intent owns cannot be established from the committed head; change the intended call yourself or drop the value from this intent"),
                        );
                    }
                    continue;
                }
                for (position, fill) in fills {
                    let Some((param, ty)) = params.get(position).cloned() else {
                        continue;
                    };
                    let (constant, data) = match self.literal(value, &param, &ty, target) {
                        Ok(literal) => literal,
                        Err(why) => {
                            self.hole(
                                AgentErrorCode::RippleHoleUnfilled,
                                &format!("{at}/value"),
                                why,
                            );
                            return;
                        }
                    };
                    if self.loads(&site.function, &site.block, &fill, &ty, &data) {
                        continue;
                    }
                    if let Some(why) = self.cannot_patch(&site.function, &f_id, target) {
                        self.hole(
                            AgentErrorCode::RippleHoleUnfilled,
                            at,
                            format!("{label} passes the value the intent filled for `{param}`, but {why}"),
                        );
                        continue;
                    }
                    let Some(func) = self.funcs.get_mut(&site.function) else {
                        continue;
                    };
                    let Some(block) = func
                        .blocks
                        .iter_mut()
                        .find(|block| block.leaf == site.block)
                    else {
                        continue;
                    };
                    if let Some(op) = block.ops.iter_mut().find(|op| op.leaf == fill) {
                        op.imm = Imm::Literal(constant);
                        block.changed.get_or_insert(at.to_owned());
                        edits += 1;
                        changed_fns.insert(site.function.clone());
                        calls.push(json!({"site": label.trim_matches('`'), "origin": "live", "edit": "value"}));
                    }
                }
            }
        }
        // The frame's own calls and tests of `f` written before the change:
        // read as the head has them.
        let mut frame_calls = self.frame_calls(target, &f_id);
        frame_calls.sort_by(|a, b| {
            (a.block.as_deref(), std::cmp::Reverse(a.index))
                .cmp(&(b.block.as_deref(), std::cmp::Reverse(b.index)))
        });
        for call in &frame_calls {
            let authored = self
                .map
                .authored(&call.pointer)
                .unwrap_or_else(|| call.pointer.clone());
            let site = format!("{authored} ({})", call.function);
            if call.fnref {
                continue;
            }
            let revised = stated.after.contains(&call.function);
            let committed = self.committed_args(call, &f_id);
            if revised && stated.declared != Some(false) && committed.as_ref() != Some(&call.args) {
                self.hole(
                    AgentErrorCode::RippleHoleUnfilled,
                    at,
                    format!("{site} was explicitly stated after the intent and differs from the committed call: write it for the current parameters, or restate the intent with \"frame_calls\": \"new\""),
                );
                continue;
            }
            if call.args.len() == params.len() && (stated.declared != Some(true) || revised) {
                // A count that fits the parameters the head has: as written
                // when "new" says so or it is the committed call; otherwise
                // it may be written for the parameters before the change.
                // Stated after the intent with no committed form: new code
                // for the parameters the head has.
                if stated.declared == Some(false) && !revised
                    || committed.as_ref() == Some(&call.args)
                    || revised && committed.is_none()
                {
                    continue;
                }
                self.hole(
                    AgentErrorCode::RippleHoleUnfilled,
                    at,
                    format!(
                        "{site} passes {} arguments to `{target}`, which fits its parameters ({}), but it differs from the committed call and {}: write it as the head has it (sley-agent view {}), or say \"frame_calls\": \"old\" (rewritten as committed) or \"new\" (kept as written)",
                        call.args.len(),
                        self.params_text(params).join(", "),
                        if revised {
                            "was stated after the intent"
                        } else {
                            "may be written for the parameters before the change"
                        },
                        call.function
                    ),
                );
                continue;
            }
            match self.adopt_call(call, &f_id, stated.value, params, target) {
                Ok(true) => {
                    edits += 1;
                    changed_fns.insert(call.function.clone());
                    calls.push(json!({"site": site, "origin": "frame", "edit": "as committed"}));
                }
                Ok(false) => {}
                Err(why) => {
                    self.hole(
                        AgentErrorCode::RippleHoleUnfilled,
                        at,
                        format!(
                            "{site} was written for the parameters `{target}` had before ({} argument(s)), and {why}: write it for ({})",
                            call.args.len(),
                            self.params_text(params).join(", ")
                        ),
                    );
                }
            }
        }
        for (index, name) in self.frame_tests(target) {
            let args = self
                .out
                .get("tests")
                .and_then(|tests| tests.get(index))
                .and_then(|test| test.get("args").or_else(|| test.get("inputs")))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let count = args.len();
            let again = self.stated_in(index, &name, stated.after);
            if count == params.len() && !again && stated.declared == Some(false) {
                continue;
            }
            let equal = count == params.len() && (again || stated.declared != Some(true));
            let live = self
                .live_tests(&f_id)
                .into_iter()
                .chain(self.live_test_named(&f_id, &name))
                .find(|(_, live)| *live == name);
            let inputs = live.and_then(|(id, _)| match self.cx.program.body(&id) {
                Some(EntityBodyValue::TestCase(test)) => Some(
                    test.inputs
                        .iter()
                        .map(|input| crate::values::to_json(input, self.cx.names))
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            });
            let authored = self
                .map
                .authored(&format!("/tests/{index}"))
                .unwrap_or_else(|| format!("/tests/{index}"));
            if equal && (inputs.is_none() || inputs.as_ref() == Some(&args)) {
                continue;
            }
            if equal {
                self.hole(
                    AgentErrorCode::RippleHoleUnfilled,
                    at,
                    format!(
                        "{authored} (`{name}`) passes {count} arguments to `{target}`, which fits its parameters ({}), but they differ from the committed test and {}: write them as the head has them, or say \"frame_calls\": \"old\" (rewritten as committed) or \"new\" (kept as written)",
                        self.params_text(params).join(", "),
                        if again {
                            "it was stated after the intent"
                        } else {
                            "they may be written for the parameters before the change"
                        }
                    ),
                );
                continue;
            }
            match inputs {
                Some(inputs) => {
                    let key = if self.out["tests"][index].get("args").is_some() {
                        "args"
                    } else {
                        "inputs"
                    };
                    if let Some(slot) = at_mut(self.out, &format!("/tests/{index}/{key}")) {
                        *slot = Value::Array(inputs);
                    }
                    edits += 1;
                    changed_tests.insert(name.clone());
                    calls.push(json!({"test": name, "origin": "frame", "edit": "as committed"}));
                }
                None => self.hole(
                    AgentErrorCode::RippleHoleUnfilled,
                    at,
                    format!(
                        "{authored} (`{name}`) was written for the parameters `{target}` had before ({count} argument(s)), and the head has no committed form of it: write its arguments for ({})",
                        self.params_text(params).join(", ")
                    ),
                ),
            }
        }
        self.edits += edits;
        let edit = if edits == 0 && self.holes.len() == holes {
            "already applied"
        } else {
            "applied again"
        };
        self.records.push(json!({
            "at": at,
            "intent": "arity",
            "target": target,
            "edit": edit,
            "old": self.params_text(params),
            "new": self.params_text(params),
            "calls": calls,
            "tests": [],
            "changed": {"functions": changed_fns, "tests": changed_tests},
        }));
    }

    /// A frame test the frame restates, live at the head under that name
    /// and targeting `f` (the frame's own tests are left out of
    /// [`Self::live_tests`]).
    fn live_test_named(&self, f_id: &EntityId, name: &str) -> Option<(EntityId, String)> {
        let id = self.cx.names.resolve(name)?;
        match self.cx.program.body(&id) {
            Some(EntityBodyValue::TestCase(test)) if test.target == *f_id => {
                Some((id, name.to_owned()))
            }
            _ => None,
        }
    }

    /// The value fills of a live call: `(argument position, operation)` for
    /// each argument that is a constant load named `<call>__v<k>` (the
    /// operation an arity intent adds for a supplied value).
    fn fills(&mut self, function: &str, block: &str, op: &str) -> Vec<(usize, String)> {
        let Some(func) = self.func(function) else {
            return Vec::new();
        };
        let Some(blk) = func.block(block) else {
            return Vec::new();
        };
        let Some(call) = blk.ops.iter().find(|candidate| candidate.leaf == op) else {
            return Vec::new();
        };
        call.args
            .iter()
            .enumerate()
            .filter_map(|(position, arg)| match arg {
                Val::Op(owner, name, 0)
                    if *owner == blk.leaf
                        && is_fill(op, name, position)
                        && blk
                            .ops
                            .iter()
                            .any(|load| load.leaf == *name && load.tag == 1) =>
                {
                    Some((position, name.clone()))
                }
                _ => None,
            })
            .collect()
    }

    /// Whether the constant load `fill` already loads `data` (of type `ty`).
    fn loads(&self, function: &str, block: &str, fill: &str, ty: &TypeExpr, data: &Value) -> bool {
        let Some(op) = self
            .funcs
            .get(function)
            .and_then(|func| func.block(block))
            .and_then(|blk| blk.ops.iter().find(|op| op.leaf == fill))
        else {
            return false;
        };
        match &op.imm {
            Imm::Literal(literal) => literal.get("value") == Some(data),
            Imm::Entity(id) => {
                let defs = crate::values::ProgramTypes {
                    program: self.cx.program,
                    names: self.cx.names,
                };
                match (
                    self.cx.program.body(id),
                    crate::values::read(data, ty, &defs, ""),
                ) {
                    (Some(EntityBodyValue::Constant(constant)), Ok(wanted)) => {
                        constant.value == wanted
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Why a live caller of `f` cannot be patched here, if it cannot.
    fn cannot_patch(&mut self, function: &str, f_id: &EntityId, target: &str) -> Option<String> {
        let home = self.namespaces_of(f_id);
        let caller = self.func(function).map(|func| func.id)?;
        let caller_home = self.namespaces_of(&caller);
        if caller_home != home {
            return Some(format!(
                "`{function}` is in namespace {} while `{target}` is in {}: ripple does not edit code across a namespace boundary",
                namespace_text(&caller_home),
                namespace_text(&home)
            ));
        }
        if self.authored.edit.contains_key(function) {
            return Some(format!("this frame changes `{function}` with edit"));
        }
        if self.authored.fns.contains_key(function) {
            return Some(format!("this frame restates `{function}`"));
        }
        self.funcs
            .get(function)
            .and_then(unpatchable)
            .map(|why| format!("`{function}` {why}, which a patch cannot restate"))
    }

    /// The arguments of the live call a frame call restates (the operation
    /// of the same name in the same block), as the frame would write them.
    fn committed_args(&mut self, call: &FrameCall, f_id: &EntityId) -> Option<Vec<Value>> {
        let live = self
            .func(&call.function)
            .and_then(|func| func.block(&call.block_leaf).cloned())?;
        let op = live.ops.iter().find(|op| {
            op.leaf == call.name && matches!(&op.imm, Imm::Function { id, .. } if id == f_id)
        })?;
        op.args
            .iter()
            .map(|arg| render_val(&live.leaf, arg).ok().map(Value::from))
            .collect()
    }

    /// Reads a frame call written for the old parameters as the head has it:
    /// its arguments become those of the live operation of the same name
    /// in the same block (with the value fills that call loads, loading
    /// `value` when one is stated). `Ok(false)` when the call already is the
    /// live one; why not, when there is no such live call or it takes a
    /// value the frame's block cannot state.
    fn adopt_call(
        &mut self,
        call: &FrameCall,
        f_id: &EntityId,
        value: Option<&Value>,
        params: &[(String, TypeExpr)],
        target: &str,
    ) -> std::result::Result<bool, String> {
        let no_form = || "the head has no committed form of it".to_owned();
        let live = self
            .func(&call.function)
            .and_then(|func| func.block(&call.block_leaf).cloned())
            .ok_or_else(no_form)?;
        let op = live
            .ops
            .iter()
            .find(|op| {
                op.leaf == call.name && matches!(&op.imm, Imm::Function { id, .. } if id == f_id)
            })
            .cloned()
            .ok_or_else(no_form)?;
        let mut args = Vec::new();
        let mut loads = Vec::new();
        for (position, arg) in op.args.iter().enumerate() {
            args.push(Value::from(render_val(&live.leaf, arg).map_err(|why| {
                format!("its committed form cannot be stated here: {why}")
            })?));
            if let Val::Op(owner, name, 0) = arg
                && *owner == live.leaf
                && is_fill(&call.name, name, position)
            {
                let load = live
                    .ops
                    .iter()
                    .find(|load| load.leaf == *name)
                    .ok_or_else(no_form)?;
                let imm = match (value, params.get(position)) {
                    (Some(value), Some((param, ty))) => self.literal(value, param, ty, target)?.0,
                    _ => match &load.imm {
                        Imm::Entity(id) => Value::from(self.cx.names.name(id)),
                        Imm::Literal(literal) => literal.clone(),
                        _ => return Err(no_form()),
                    },
                };
                loads.push(json!([name, "const", imm]));
            }
        }
        if args == call.args && loads.is_empty() {
            return Ok(false);
        }
        let Some(block_pointer) = &call.block else {
            if !loads.is_empty() {
                return Err("an edit cannot add the operation its committed form loads".to_owned());
            }
            if let Some(Value::Array(items)) = at_mut(self.out, &call.pointer) {
                items.truncate(2);
                items.extend(args);
            }
            return Ok(true);
        };
        let ops_pointer = format!("{block_pointer}/ops");
        let Some(Value::Array(ops)) = at_mut(self.out, &ops_pointer) else {
            return Err(no_form());
        };
        let defined: BTreeSet<String> = ops.iter().filter_map(op_name).map(str::to_owned).collect();
        let loads: Vec<Value> = loads
            .into_iter()
            .filter(|load| !defined.contains(load[0].as_str().unwrap_or_default()))
            .collect();
        let Some(slot) = ops.get_mut(call.index) else {
            return Err(no_form());
        };
        set_call_args(slot, args);
        for load in loads.into_iter().rev() {
            ops.insert(call.index, load);
            self.map.shift_ops(&ops_pointer, call.index);
            self.map.entries.push(MapEntry {
                expanded: format!("{ops_pointer}/{}", call.index),
                authored: call.pointer.clone(),
                role: Role::Ripple,
                name: call.name.clone(),
            });
        }
        Ok(true)
    }

    /// Why a call or test the frame writes cannot be read mechanically.
    fn unread(
        &self,
        reading: Reading,
        site: &str,
        count: usize,
        old: &[(String, TypeExpr)],
        new: &[(String, TypeExpr)],
        target: &str,
    ) -> String {
        if reading == Reading::Revised {
            format!(
                "{site} passes {count} arguments to `{target}`, which fits both its old parameters ({}) and its new ones ({}), and it was stated in a revision after the intent, which \"frame_calls\" does not cover: restate the intent with \"frame_calls\" in this revision (it then covers everything the frame states), or remove it from the intent's \"after\" to read it like the intent's own revision",
                self.params_text(old).join(", "),
                self.params_text(new).join(", ")
            )
        } else if reading == Reading::Ambiguous {
            format!(
                "{site} passes {count} arguments to `{target}`, which fits both its old parameters ({}) and its new ones ({}): say how this frame's calls and tests of `{target}` are written with \"frame_calls\": \"old\" (rewritten by name) or \"new\" (kept as written)",
                self.params_text(old).join(", "),
                self.params_text(new).join(", ")
            )
        } else {
            format!(
                "{site} passes {count} arguments to `{target}`, but \"frame_calls\": \"old\" says it is written for the old parameters ({}): write it for them, or say \"new\"",
                self.params_text(old).join(", ")
            )
        }
    }

    /// The namespaces a function the frame writes belongs to: a live one
    /// keeps its own; a new one joins the frame's namespace.
    fn frame_home(&self, function: &str) -> Vec<String> {
        if let Some(id) = self.cx.names.resolve(function)
            && self.cx.names.scope(&id) == Scope::Top
            && matches!(
                self.cx.program.body(&id),
                Some(EntityBodyValue::Function(_))
            )
            && !self.authored.deleted.contains(function)
        {
            return self.namespaces_of(&id);
        }
        let namespaces: Vec<EntityId> = self
            .cx
            .program
            .objects()
            .iter()
            .filter(|object| matches!(object.record().body, EntityBodyValue::Namespace(_)))
            .map(|object| object.record().entity_id)
            .collect();
        match self.out.get("namespace") {
            Some(Value::String(name)) => vec![name.clone()],
            None if namespaces.len() == 1 => vec![self.cx.names.name(&namespaces[0])],
            Some(_) | None => Vec::new(),
        }
    }

    /// Live entities that hold `id` as a function value in a constant: a
    /// constant, or a `TestCase` input or expectation.
    fn function_values(&self, id: &EntityId) -> Vec<String> {
        let mut out = Vec::new();
        for object in self.cx.program.objects() {
            let record = object.record();
            let name = self.cx.names.name(&record.entity_id);
            let holds = match &record.body {
                EntityBodyValue::Constant(constant) => refers(&constant.value, id),
                EntityBodyValue::TestCase(test) => {
                    test.inputs.iter().any(|input| refers(input, id))
                        || matches!(&test.expected, ExpectedOutcome::Value(value) if refers(value, id))
                }
                _ => false,
            };
            if holds {
                let kind = if matches!(record.body, EntityBodyValue::Constant(_)) {
                    "constant"
                } else {
                    "TestCase"
                };
                out.push(format!("{kind} `{name}`"));
            }
        }
        out
    }

    /// Calls and function values of `id` in live code the frame does not
    /// restate (functions it does not define whole, blocks its patches keep,
    /// operations its edits keep).
    fn live_calls(&mut self, id: &EntityId) -> Vec<LiveSite> {
        let mut candidates: BTreeSet<String> = self.funcs.keys().cloned().collect();
        for object in self.cx.program.objects() {
            let record = object.record();
            let EntityBodyValue::Function(function) = &record.body else {
                continue;
            };
            if self.cx.names.scope(&record.entity_id) != Scope::Top {
                continue;
            }
            let uses = function.blocks.iter().any(|block| {
                let Some(EntityBodyValue::Block(block)) = self.cx.program.body(block) else {
                    return false;
                };
                block.operations.iter().any(|op| {
                    matches!(
                        self.cx.program.body(op),
                        Some(EntityBodyValue::Operation(op))
                            if matches!(&op.immediate, Immediate::Function(reference) if reference.function == *id)
                    )
                })
            });
            if uses {
                candidates.insert(self.cx.names.name(&record.entity_id));
            }
        }
        let mut out = Vec::new();
        for function in candidates {
            if self.authored.fns.contains_key(&function) || self.authored.gone(&function) {
                continue;
            }
            let restated: BTreeSet<String> = self
                .authored
                .patch
                .get(&function)
                .and_then(|index| {
                    self.out
                        .get("patch")?
                        .get(*index)?
                        .get("blocks")?
                        .as_object()
                })
                .map(|blocks| blocks.keys().cloned().collect())
                .unwrap_or_default();
            let edited = self
                .authored
                .edit
                .get(&function)
                .cloned()
                .unwrap_or_default();
            let Some(func) = self.func(&function) else {
                continue;
            };
            for block in &func.blocks {
                if restated.contains(&block.leaf) {
                    continue;
                }
                for op in &block.ops {
                    let Imm::Function { id: callee, .. } = &op.imm else {
                        continue;
                    };
                    if callee != id || edited.contains(&format!("{}.{}", block.leaf, op.leaf)) {
                        continue;
                    }
                    out.push(LiveSite {
                        function: function.clone(),
                        block: block.leaf.clone(),
                        op: op.leaf.clone(),
                        fnref: op.tag == 194,
                    });
                }
            }
        }
        out
    }

    /// Live `TestCases` of `id` the frame neither restates nor deletes.
    fn live_tests(&self, id: &EntityId) -> Vec<(EntityId, String)> {
        let mut out: Vec<(EntityId, String)> = self
            .cx
            .program
            .objects()
            .iter()
            .filter_map(|object| {
                let record = object.record();
                match &record.body {
                    EntityBodyValue::TestCase(test) if test.target == *id => {
                        let name = self.cx.names.name(&record.entity_id);
                        (!self.authored.tests.contains(&name)
                            && !self.authored.deleted.contains(&name))
                        .then_some((record.entity_id, name))
                    }
                    _ => None,
                }
            })
            .collect();
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out
    }

    /// The new argument list of a site from its old arguments; a hole for
    /// each argument it cannot derive. `None` in the result stands for the
    /// supplied value.
    fn arguments<T: Clone>(
        &mut self,
        context: &Arity,
        old_args: &[T],
        site: &str,
        test: bool,
    ) -> Option<Vec<Option<T>>> {
        let mut out = Vec::new();
        let mut complete = true;
        for (position, slot) in context.slots.iter().enumerate() {
            let (param, ty) = &context.new[position];
            let rendered = self.render(ty);
            match slot {
                Slot::Keep(old) => match old_args.get(*old) {
                    Some(arg) => out.push(Some(arg.clone())),
                    None => complete = false,
                },
                Slot::New
                    if context
                        .fill
                        .as_ref()
                        .is_some_and(|fill| fill.position == position) =>
                {
                    out.push(None);
                }
                Slot::New => {
                    complete = false;
                    let fix = if test {
                        "add \"value\" to this intent, or restate the test in \"tests\""
                    } else {
                        "add \"value\" to this intent, or write the call"
                    };
                    self.hole(
                        AgentErrorCode::RippleHoleUnfilled,
                        &context.at,
                        format!(
                            "{site} gives `{}` no argument for its new parameter `{param}` (expected {rendered}): {fix}",
                            context.target
                        ),
                    );
                }
                Slot::Retyped(_, from) => {
                    complete = false;
                    self.hole(
                        AgentErrorCode::RippleHoleUnfilled,
                        &context.at,
                        format!(
                            "{site} passes an argument of type {} for parameter `{param}`, which `{}` now takes as {rendered} (expected {rendered}): ripple adds no coercion; write the argument",
                            self.render(from),
                            context.target
                        ),
                    );
                }
            }
        }
        complete.then_some(out)
    }

    fn rewrite_frame_call(&mut self, context: &Arity, call: &FrameCall, site: &str) -> bool {
        let Some(args) = self.arguments(context, &call.args, site, false) else {
            return false;
        };
        let needs_value = args.iter().any(Option::is_none);
        let Some(block_pointer) = &call.block else {
            if needs_value {
                self.hole(
                    AgentErrorCode::RippleHoleUnfilled,
                    &context.at,
                    format!(
                        "{site} is an edit, which cannot add the operation for the new argument: restate its block with patch"
                    ),
                );
                return false;
            }
            let args: Vec<Value> = args.into_iter().flatten().collect();
            if let Some(with) = at_mut(self.out, &call.pointer) {
                match with {
                    Value::Array(items) => {
                        items.truncate(2);
                        items.extend(args);
                    }
                    other => set_call_args(other, args),
                }
            }
            return true;
        };
        let mut constant_name = None;
        if needs_value {
            let taken = self.frame_function_names(&call.function);
            let position = context.fill.as_ref().map_or(0, |fill| fill.position);
            let base = if call.name.is_empty() {
                "ripple".to_owned()
            } else {
                format!("{}__v{position}", call.name)
            };
            constant_name = Some(self.unique(&base, &taken));
        }
        let args: Vec<Value> = args
            .into_iter()
            .map(|arg| {
                arg.unwrap_or_else(|| Value::from(constant_name.clone().unwrap_or_default()))
            })
            .collect();
        let ops_pointer = format!("{block_pointer}/ops");
        let Some(Value::Array(ops)) = at_mut(self.out, &ops_pointer) else {
            return false;
        };
        let Some(op) = ops.get_mut(call.index) else {
            return false;
        };
        set_call_args(op, args);
        if let (Some(name), Some(fill)) = (constant_name, &context.fill) {
            ops.insert(call.index, json!([name, "const", fill.constant]));
            self.map.shift_ops(&ops_pointer, call.index);
            self.map.entries.push(MapEntry {
                expanded: format!("{ops_pointer}/{}", call.index),
                authored: context.at.clone(),
                role: Role::Ripple,
                name: name.clone(),
            });
            self.map
                .names
                .entry(call.function.clone())
                .or_default()
                .insert(name, context.at.clone());
        }
        true
    }

    /// Every name in a function the frame writes (both its `fns` and its
    /// `patch` statements), for fresh generated names.
    fn frame_function_names(&self, function: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut add_block = |block: &Value| {
            if let Some(name) = block.get("name").and_then(Value::as_str) {
                out.insert(name.to_owned());
            }
            for param in block
                .get("params")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(name) = param.get(0).and_then(Value::as_str) {
                    out.insert(name.to_owned());
                }
            }
            for op in block
                .get("ops")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(name) = op_name(op) {
                    out.insert(name.to_owned());
                }
            }
        };
        if let Some((key, index)) = self.authored.fns.get(function)
            && let Some(decl) = self.out.get(key).and_then(|list| list.get(*index))
        {
            for block in decl
                .get("blocks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                add_block(block);
            }
        }
        if let Some(index) = self.authored.patch.get(function)
            && let Some(decl) = self.out.get("patch").and_then(|list| list.get(*index))
        {
            let blocks: Vec<(&String, &Value)> = decl
                .get("blocks")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .collect();
            for (_, block) in &blocks {
                add_block(block);
            }
            out.extend(blocks.into_iter().map(|(leaf, _)| leaf.clone()));
        }
        for decl in [self
            .authored
            .fns
            .get(function)
            .map(|(key, index)| (key.as_str(), *index))]
        .into_iter()
        .flatten()
        {
            let params = self
                .out
                .get(decl.0)
                .and_then(|list| list.get(decl.1))
                .and_then(|decl| decl.get("params"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for param in params {
                if let Some(name) = param.get(0).and_then(Value::as_str) {
                    out.insert(name.to_owned());
                }
            }
        }
        if let Some(func) = self.funcs.get(function) {
            out.extend(func.taken());
        } else if let Ok((id, _)) = self.live_target(function)
            && let Some(func) = build(&self.live(), id, function)
        {
            out.extend(func.taken());
        }
        out
    }

    fn rewrite_frame_test(&mut self, context: &Arity, index: usize, label: &str) -> bool {
        let Some(test) = self.out.get("tests").and_then(|tests| tests.get(index)) else {
            return false;
        };
        let key = if test.get("args").is_some() {
            "args"
        } else {
            "inputs"
        };
        let old_args = test
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let Some(args) = self.arguments(context, &old_args, label, true) else {
            return false;
        };
        let data = context.fill.as_ref().map(|fill| fill.data.clone());
        let args: Vec<Value> = args
            .into_iter()
            .map(|arg| arg.unwrap_or_else(|| data.clone().unwrap_or(Value::Null)))
            .collect();
        if let Some(slot) = at_mut(self.out, &format!("/tests/{index}/{key}")) {
            *slot = Value::Array(args);
        }
        true
    }

    fn rewrite_live_call(&mut self, context: &Arity, site: &LiveSite, label: &str) -> bool {
        let Some(func) = self.funcs.get(&site.function) else {
            return false;
        };
        let Some(b) = func.position(&site.block) else {
            return false;
        };
        // By name: a constant inserted for an earlier site of the same
        // block moves the ones after it.
        let Some(position) = func.blocks[b].ops.iter().position(|op| op.leaf == site.op) else {
            return false;
        };
        let old_args = func.blocks[b].ops[position].args.clone();
        let Some(args) = self.arguments(context, &old_args, label, false) else {
            return false;
        };
        let at = context.at.clone();
        let fill = context.fill.as_ref().map(|fill| {
            (
                fill.constant.clone(),
                context.new[fill.position].1.clone(),
                fill.position,
            )
        });
        let taken = self.funcs[&site.function].taken();
        let constant = fill
            .as_ref()
            .map(|(_, _, position)| self.unique(&format!("{}__v{position}", site.op), &taken));
        let func = self.funcs.get_mut(&site.function).expect("present");
        let block = &mut func.blocks[b];
        let leaf = block.leaf.clone();
        let mut index = position;
        if let (Some(name), Some((literal, ty, _))) = (&constant, &fill)
            && args.iter().any(Option::is_none)
        {
            block.ops.insert(
                index,
                Op {
                    leaf: name.clone(),
                    tag: 1,
                    imm: Imm::Literal(literal.clone()),
                    args: Vec::new(),
                    types: vec![ty.clone()],
                },
            );
            index += 1;
            func.generated.insert(name.clone(), at.clone());
        }
        let block = &mut func.blocks[b];
        block.ops[index].args = args
            .into_iter()
            .map(|arg| {
                arg.unwrap_or_else(|| {
                    Val::Op(leaf.clone(), constant.clone().unwrap_or_default(), 0)
                })
            })
            .collect();
        block.changed.get_or_insert(at);
        true
    }

    fn rewrite_live_test(&mut self, context: &Arity, id: &EntityId, name: &str) -> bool {
        let Some(EntityBodyValue::TestCase(test)) = self.cx.program.body(id) else {
            return false;
        };
        let label = format!("TestCase `{name}`");
        if test.effect_environment != EffectEnvironment::Replay(Vec::new())
            || !test.observations.is_empty()
        {
            self.hole(
                AgentErrorCode::RippleHoleUnfilled,
                &context.at,
                format!(
                    "{label} of `{}` has an effect environment or observations that AF1 cannot restate: restate or delete it yourself",
                    context.target
                ),
            );
            return false;
        }
        let names = self.cx.names;
        let old_args: Vec<Value> = test
            .inputs
            .iter()
            .map(|input| crate::values::to_json(input, names))
            .collect();
        let Some(args) = self.arguments(context, &old_args, &label, true) else {
            return false;
        };
        let data = context.fill.as_ref().map(|fill| fill.data.clone());
        let args: Vec<Value> = args
            .into_iter()
            .map(|arg| arg.unwrap_or_else(|| data.clone().unwrap_or(Value::Null)))
            .collect();
        let expect = match &test.expected {
            ExpectedOutcome::Value(value) => crate::values::to_json(value, names),
            ExpectedOutcome::FailureCode(code) => json!({"trap": code}),
        };
        let limits = test.resource_limits;
        let entry = json!({
            "name": name,
            "fn": context.target,
            "args": args,
            "expect": expect,
            "limits": {
                "fuel": limits.fuel,
                "memory_bytes": limits.memory_bytes,
                "output_bytes": limits.output_bytes,
                "effect_count": limits.effect_count,
                "call_depth": limits.call_depth,
                "wall_timeout_millis": limits.wall_timeout_millis,
            },
        });
        self.tests
            .entry(name.to_owned())
            .or_insert_with(|| (Value::Null, context.at.clone()))
            .0 = entry;
        true
    }

    // --- guard ------------------------------------------------------------

    #[allow(clippy::too_many_lines)]
    fn guard(&mut self, index: usize, intent: &Guard<'_>) -> Result<()> {
        let at = format!("/ripple/{index}");
        let (arg, entry) = (intent.arg, intent.entry);
        // Constants, types and functions as the frame defines them.
        let state = self.frame_state()?;
        let view = View {
            live: self.live(),
            state: &state,
        };
        let Some(checker) = self.checker(&view, intent.checker, &at) else {
            return Ok(());
        };
        let mut seen = BTreeSet::new();
        let mut results = Vec::new();
        let mut changed = BTreeSet::new();
        for (position, function) in intent.functions.iter().enumerate() {
            let fat = format!("{at}/in/{position}");
            if !seen.insert(function.clone()) {
                self.hole(
                    AgentErrorCode::RippleGuardOrder,
                    &fat,
                    format!("`{function}` is listed twice; a guard applies once per function"),
                );
                continue;
            }
            if *function == checker.name {
                self.hole(
                    AgentErrorCode::RippleTargetKind,
                    &fat,
                    format!("`{function}` is the checker itself"),
                );
                continue;
            }
            if let Err(why) = self.live_target(function) {
                self.hole(AgentErrorCode::RippleTargetKind, &fat, why);
                continue;
            }
            if self.authored.restates(function) {
                self.hole(
                    AgentErrorCode::RippleTargetKind,
                    &fat,
                    format!(
                        "`{function}` is restated by this frame; a guard rewrites live functions: write the check in the restated function, or apply the guard in its own frame"
                    ),
                );
                continue;
            }
            let Some(func) = self.func(function) else {
                self.hole(
                    AgentErrorCode::RippleTargetKind,
                    &fat,
                    format!("`{function}` is not a well-formed live function"),
                );
                continue;
            };
            if func.generic || (func.effects && !entry) {
                let why = unpatchable(func).unwrap_or_default();
                self.hole(
                    AgentErrorCode::RippleTargetKind,
                    &fat,
                    format!("`{function}` {why}, which a patch cannot restate: guard leaves it"),
                );
                continue;
            }
            let Some((_, ty)) = func.params.iter().find(|(name, _)| name == arg).cloned() else {
                let list: Vec<String> = func.params.iter().map(|(name, _)| name.clone()).collect();
                self.hole(
                    AgentErrorCode::RippleTargetKind,
                    &fat,
                    format!(
                        "`{function}` has no parameter `{arg}` (it takes {})",
                        if list.is_empty() {
                            "none".to_owned()
                        } else {
                            list.join(", ")
                        }
                    ),
                );
                continue;
            };
            if ty != checker.param {
                let (want, found) = (self.render(&checker.param), self.render(&ty));
                self.hole(
                    AgentErrorCode::RippleGuardShape,
                    &fat,
                    format!(
                        "`{}` checks {want}, but parameter `{arg}` of `{function}` is {found} (expected {want})",
                        checker.name
                    ),
                );
                continue;
            }
            let record = if entry {
                self.guard_entry(&view, &fat, function, intent, &checker)
            } else {
                self.guard_preserve(&view, &fat, function, arg, &checker)
            };
            if let Some(record) = record {
                if record["edit"] != "already applied" {
                    self.edits += 1;
                    changed.insert(function.clone());
                }
                results.push(record);
            }
        }
        self.records.push(json!({
            "at": at,
            "intent": "guard",
            "checker": checker.name,
            "arg": arg,
            "mode": if entry { "entry" } else { "preserve" },
            "functions": results,
            "changed": {"functions": changed, "tests": []},
        }));
        Ok(())
    }

    /// The checker `name` with its `P -> Result<P,E>` shape and its body as
    /// the frame leaves it (the frame's own statement when it restates the
    /// checker, else the live body as earlier intents left it).
    fn checker(&mut self, view: &View<'_, '_>, name: &str, at: &str) -> Option<Checker> {
        let pointer = format!("{at}/guard");
        let restated = self.authored.restates(name);
        if !restated && let Err(why) = self.live_target(name) {
            self.hole(AgentErrorCode::RippleTargetKind, &pointer, why);
            return None;
        }
        let Some((params, result)) = self.cx.signature(name) else {
            self.hole(
                AgentErrorCode::RippleTargetKind,
                &pointer,
                format!("`{name}` has no readable signature"),
            );
            return None;
        };
        let shape = |ripple: &Self| -> std::result::Result<(TypeExpr, TypeExpr), String> {
            let rendered: Vec<String> = params
                .iter()
                .map(|ty| {
                    ty.as_ref()
                        .map_or_else(|| "?".to_owned(), |ty| ripple.render(ty))
                })
                .collect();
            let returns = result
                .as_ref()
                .map_or_else(|| "?".to_owned(), |ty| ripple.render(ty));
            let [Some(param)] = params.as_slice() else {
                return Err(format!(
                    "`{name}` takes ({}) and returns {returns}; a guard is one parameter P -> Result<P,E>",
                    rendered.join(", ")
                ));
            };
            match &result {
                Some(TypeExpr::Result { ok, error }) if **ok == *param => {
                    Ok((param.clone(), (**error).clone()))
                }
                Some(TypeExpr::Result { .. }) => Err(format!(
                    "`{name}` returns {returns} for a {} parameter; a guard keeps the checked type: P -> Result<P,E>",
                    rendered[0]
                )),
                Some(TypeExpr::Option(_)) => Err(format!(
                    "`{name}` returns {returns}; a guard is P -> Result<P,E>, and no error case is inferred for None"
                )),
                _ => Err(format!(
                    "`{name}` returns {returns}; a guard is P -> Result<P,E>"
                )),
            }
        };
        let (param, error) = match shape(self) {
            Ok(shape) => shape,
            Err(why) => {
                self.hole(AgentErrorCode::RippleGuardShape, &pointer, why);
                return None;
            }
        };
        let stated = || view.function(name).and_then(|id| build(view, id, name));
        let func = if restated {
            stated()
        } else {
            self.funcs.get(name).cloned().or_else(stated)
        };
        let Some(func) = func else {
            self.hole(
                AgentErrorCode::RippleTargetKind,
                &pointer,
                format!("`{name}` is not a well-formed function"),
            );
            return None;
        };
        if func.generic {
            self.hole(
                AgentErrorCode::RippleGuardShape,
                &pointer,
                format!("`{name}` has type parameters; a guard is a non-generic P -> Result<P,E>"),
            );
            return None;
        }
        if func.blocks.len() > MAX_GUARD_SIZE {
            self.hole(
                AgentErrorCode::RippleLimit,
                &pointer,
                format!(
                    "`{name}` has {} blocks; a checker has at most {MAX_GUARD_SIZE}: split it",
                    func.blocks.len()
                ),
            );
            return None;
        }
        Some(Checker {
            name: name.to_owned(),
            id: func.id,
            effects: func.effects,
            func,
            param,
            error,
        })
    }

    /// The frame's own definitions compiled over the program (its tests
    /// aside): constants, types and functions as the candidate holds them.
    fn frame_state(&self) -> Result<FrameState> {
        let mut state = FrameState::default();
        let defines = [
            "types",
            "consts",
            "fns",
            "functions",
            "patch",
            "edit",
            "delete",
        ]
        .iter()
        .any(|key| self.out.contains_key(*key));
        if !defines {
            return Ok(state);
        }
        let mut frame = self.out.clone();
        frame.remove("tests");
        let mut counter: u64 = 0;
        let mut random = || {
            counter += 1;
            let mut bytes = [0x5a_u8; 32];
            bytes[..8].copy_from_slice(&counter.to_be_bytes());
            Ok(bytes)
        };
        let compiled = crate::frame::compile(
            self.cx.program,
            self.cx.names,
            &crate::genesis::INIT_CEILINGS,
            &Value::Object(frame),
            CandidateNonce::from_bytes([0x5a; 32]),
            &mut random,
        )
        .map_err(|error| self.map.rewrite(&error))?;
        for op in compiled.ops {
            match op.payload {
                MutationPayload::CreateEntity(body) => {
                    if op.kind == 5
                        && let Some(name) = compiled.names.get(op.target.as_bytes())
                    {
                        state.created.insert(name.to_owned(), op.target);
                    }
                    state.bodies.insert(op.target, body);
                }
                MutationPayload::ReplaceEntityVersion(body) => {
                    state.bodies.insert(op.target, body);
                }
                MutationPayload::DeleteEntityBinding => {
                    state.deleted.insert(op.target);
                }
                _ => {}
            }
        }
        state.names = compiled.names;
        Ok(state)
    }

    /// The functions `name` calls or takes as a value, by name, as the frame
    /// and the earlier intents leave it (both bodies, when they differ).
    fn callees(&self, view: &View<'_, '_>, name: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        if let Some(func) = self.funcs.get(name) {
            for block in &func.blocks {
                for op in &block.ops {
                    if let Imm::Function { name, .. } = &op.imm {
                        out.insert(name.clone());
                    }
                }
            }
        }
        if let Some(id) = view.function(name)
            && let Some(EntityBodyValue::Function(body)) = view.body(&id)
        {
            for block in &body.blocks {
                let Some(EntityBodyValue::Block(block)) = view.body(block) else {
                    continue;
                };
                for op in &block.operations {
                    if let Some(EntityBodyValue::Operation(op)) = view.body(op)
                        && let Immediate::Function(reference) = &op.immediate
                    {
                        out.insert(view.leaf(&reference.function));
                    }
                }
            }
        }
        out
    }

    /// A chain of calls from `from` to `to` in the candidate, when there is
    /// one: `[from, ..., to]`.
    fn call_path(&self, view: &View<'_, '_>, from: &str, to: &str) -> Option<Vec<String>> {
        let mut parent: BTreeMap<String, String> = BTreeMap::new();
        let mut queue = std::collections::VecDeque::from([from.to_owned()]);
        let mut seen = BTreeSet::from([from.to_owned()]);
        while let Some(current) = queue.pop_front() {
            for callee in self.callees(view, &current) {
                if !seen.insert(callee.clone()) {
                    continue;
                }
                parent.insert(callee.clone(), current.clone());
                if callee == to {
                    let mut path = vec![callee];
                    while let Some(previous) = parent.get(path.last().expect("non-empty")) {
                        path.push(previous.clone());
                    }
                    path.reverse();
                    return Some(path);
                }
                queue.push_back(callee);
            }
        }
        None
    }

    /// Where `func` already evaluates `checker`: a call of it, or a call of
    /// a function that reaches it, whatever the argument (a block parameter
    /// or a helper may carry the checked value).
    fn evaluates(&self, view: &View<'_, '_>, func: &Func, checker: &str) -> Option<String> {
        let mut through: BTreeMap<String, Option<Vec<String>>> = BTreeMap::new();
        for block in &func.blocks {
            for op in &block.ops {
                let Imm::Function { name, .. } = &op.imm else {
                    continue;
                };
                let site = format!("`{}.{}.{}`", func.name, block.leaf, op.leaf);
                if name == checker {
                    return Some(site);
                }
                let path = through
                    .entry(name.clone())
                    .or_insert_with(|| self.call_path(view, name, checker));
                if let Some(path) = path {
                    return Some(format!("{site}, through {}", path.join(" -> ")));
                }
            }
        }
        None
    }

    #[allow(clippy::too_many_lines)]
    fn guard_preserve(
        &mut self,
        view: &View<'_, '_>,
        at: &str,
        function: &str,
        arg: &str,
        checker: &Checker,
    ) -> Option<Value> {
        let g = &checker.func;
        let q = g.params.first().map(|(name, _)| name.clone())?;
        let kinds = match classify(g, &q, view) {
            Ok(kinds) => kinds,
            Err(why) => {
                self.hole(
                    AgentErrorCode::RippleGuardOrder,
                    at,
                    format!(
                        "preserve replaces a check that is the same pure check as `{}`'s body, but {why}: use \"mode\": \"entry\", or write the call",
                        checker.name
                    ),
                );
                return None;
            }
        };
        let f = self.funcs.get(function)?.clone();
        let errors_fit =
            matches!(&f.result, TypeExpr::Result { error, .. } if **error == checker.error);
        let mut regions: Vec<Region> = Vec::new();
        let mut why_not: Vec<String> = Vec::new();
        let mut steps = 0;
        for block in &f.blocks {
            let mut matcher = Matcher {
                g,
                f: &f,
                kinds: &kinds,
                q: q.clone(),
                p: arg.to_owned(),
                vals: BTreeMap::new(),
                blocks: BTreeMap::new(),
                taken: BTreeSet::new(),
                cont: None,
                steps,
                source: view,
            };
            let anchored = matcher.anchor(block);
            steps = matcher.steps;
            if steps > MAX_MATCH_STEPS {
                self.hole(
                    AgentErrorCode::RippleLimit,
                    at,
                    format!(
                        "comparing `{}` with the checks of `{function}` takes more than {MAX_MATCH_STEPS} steps: write the call",
                        checker.name
                    ),
                );
                return None;
            }
            if !anchored {
                continue;
            }
            match matcher.region(&block.leaf) {
                Ok(region) => regions.push(region),
                Err(why) => why_not.push(why),
            }
        }
        if let Some(why) = why_not.first() {
            self.hole(
                AgentErrorCode::RippleGuardOrder,
                at,
                format!(
                    "a check in `{function}` has the shape of `{}` on `{arg}` but {why}: preserve replaces it only when nothing else uses the check's values; write the call",
                    checker.name
                ),
            );
            return None;
        }
        if regions.is_empty() {
            if let Some(site) = guarded_call(&f, &checker.name, arg, false, None) {
                return Some(json!({"fn": function, "edit": "already applied", "site": site}));
            }
            let already =
                self.evaluates(view, &f, &checker.name)
                    .map_or_else(String::new, |site| {
                        format!(
                            " (`{function}` already evaluates `{}` at {site})",
                            checker.name
                        )
                    });
            self.hole(
                AgentErrorCode::RippleGuardOrder,
                at,
                format!(
                    "no check in `{function}` is the same as `{}` on `{arg}`{already}: preserve replaces a check only where the same operations run on `{arg}` in the same order at the end of a block, fail with the same errors and continue to one block; use \"mode\": \"entry\", or write the call",
                    checker.name
                ),
            );
            return None;
        }
        if !errors_fit {
            self.hole(
                AgentErrorCode::RippleGuardOrder,
                at,
                format!(
                    "`{function}` returns {}, so it cannot return `{}`'s error {} unchanged: use \"mode\": \"entry\" with a handler block, or write the call",
                    self.render(&f.result),
                    checker.name,
                    self.render(&checker.error)
                ),
            );
            return None;
        }
        for (i, a) in regions.iter().enumerate() {
            for b in &regions[i + 1..] {
                if a.anchor == b.anchor
                    || !a.blocks.is_disjoint(&b.blocks)
                    || a.blocks.contains(&b.anchor)
                    || b.blocks.contains(&a.anchor)
                {
                    self.hole(
                        AgentErrorCode::RippleGuardOrder,
                        at,
                        format!(
                            "two checks in `{function}` overlap (at blocks `{}` and `{}`): write the calls",
                            a.anchor, b.anchor
                        ),
                    );
                    return None;
                }
            }
        }
        let reach_before = f.reachable();
        let mut func = f;
        let err_block = error_exit(&mut func, &checker.error, at, self);
        let mut sites = Vec::new();
        let taken = func.taken();
        let mut extra = BTreeSet::new();
        for region in &regions {
            let mut all = taken.clone();
            all.extend(extra.iter().cloned());
            let call = self.unique(&format!("{arg}__{}", checker.name), &all);
            extra.insert(call.clone());
            let position = func.position(&region.anchor)?;
            let block = &mut func.blocks[position];
            block.ops.truncate(region.start);
            block.ops.push(Op {
                leaf: call.clone(),
                tag: 112,
                imm: Imm::Function {
                    id: checker.id,
                    name: checker.name.clone(),
                    generic: false,
                },
                args: vec![Val::Param(arg.to_owned())],
                types: vec![TypeExpr::Result {
                    ok: Box::new(checker.param.clone()),
                    error: Box::new(checker.error.clone()),
                }],
            });
            let mut cases = vec![
                (CaseKey::Builtin(BuiltinCase::Ok), region.cont.clone()),
                (
                    CaseKey::Builtin(BuiltinCase::Err),
                    Edge {
                        target: err_block.clone(),
                        args: vec![Arg::Payload],
                    },
                ),
            ];
            cases.sort_by_key(|(key, _)| *key);
            block.term = Term::Switch(Val::Op(block.leaf.clone(), call.clone(), 0), cases);
            block.changed.get_or_insert(at.to_owned());
            func.generated.insert(call.clone(), at.to_owned());
            sites.push(Value::from(format!("{function}.{}", region.anchor)));
        }
        // The replaced checks' own blocks are no longer reached: they go.
        let reach_after = func.reachable();
        let gone: Vec<String> = func
            .blocks
            .iter()
            .filter(|block| {
                reach_before.contains(&block.leaf) && !reach_after.contains(&block.leaf)
            })
            .map(|block| block.leaf.clone())
            .collect();
        let gone_values: BTreeSet<Val> = func
            .blocks
            .iter()
            .filter(|block| gone.contains(&block.leaf))
            .flat_map(Blk::defines)
            .collect();
        let dangling = func
            .blocks
            .iter()
            .filter(|block| !gone.contains(&block.leaf))
            .find_map(|block| {
                block
                    .uses()
                    .into_iter()
                    .find(|value| gone_values.contains(value))
                    .map(|value| (block.leaf.clone(), value.clone()))
            });
        if let Some((user, value)) = dangling {
            self.hole(
                AgentErrorCode::RippleGuardOrder,
                at,
                format!(
                    "block `{user}` of `{function}` uses {}, which the replaced check defines: write the call",
                    render_val("", &value).unwrap_or_else(|why| why)
                ),
            );
            return None;
        }
        for leaf in &gone {
            if let Some(position) = func.position(leaf) {
                let block = func.blocks.remove(position);
                if !func.generated.contains_key(&block.leaf) {
                    func.deleted.push((block.leaf, at.to_owned()));
                }
            }
        }
        let error = err_block.clone();
        self.funcs.insert(function.to_owned(), func);
        Some(
            json!({"fn": function, "edit": "replaced", "checks": sites, "deleted": gone, "error": error}),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn guard_entry(
        &mut self,
        view: &View<'_, '_>,
        at: &str,
        function: &str,
        intent: &Guard<'_>,
        checker: &Checker,
    ) -> Option<Value> {
        let arg = intent.arg;
        let f = self.funcs.get(function)?.clone();
        if let Some(path) = self.call_path(view, &checker.name, function) {
            self.hole(
                AgentErrorCode::RippleGuardOrder,
                at,
                format!(
                    "`{}` calls `{function}` ({}): evaluating it at the entry of `{function}` would never end; write the check",
                    checker.name,
                    path.join(" -> ")
                ),
            );
            return None;
        }
        if checker.effects {
            self.hole(
                AgentErrorCode::RippleGuardOrder,
                at,
                format!(
                    "`{}` performs effects; evaluating it at the entry of `{function}` would move them: write the call",
                    checker.name
                ),
            );
            return None;
        }
        if let Some(site) = effect_site(&f, view) {
            self.hole(
                AgentErrorCode::RippleGuardOrder,
                at,
                format!(
                    "`{function}` performs effects ({site}); evaluating `{}` first could skip them: use \"mode\": \"preserve\", or write the call",
                    checker.name
                ),
            );
            return None;
        }
        // A function whose entry is already this guard's call (a committed
        // revision derived it) needs nothing.
        if let Some(site) = guarded_call(&f, &checker.name, arg, true, intent.handler) {
            return Some(json!({"fn": function, "edit": "already applied", "site": site}));
        }
        // One evaluation of the checker per call: a function that already
        // evaluates it, on any value, directly or through another function,
        // is refused rather than checked twice.
        if let Some(site) = self.evaluates(view, &f, &checker.name) {
            self.hole(
                AgentErrorCode::RippleGuardOrder,
                at,
                format!(
                    "`{function}` already evaluates `{}` at {site}; evaluating it again at entry could run it twice: use \"mode\": \"preserve\", or write the call",
                    checker.name
                ),
            );
            return None;
        }
        if f.block(&f.entry)
            .is_some_and(|block| !block.params.is_empty())
        {
            self.hole(
                AgentErrorCode::RippleGuardOrder,
                at,
                format!("the entry block of `{function}` takes parameters: write the call"),
            );
            return None;
        }
        // Where the checker's error goes: the block the author names, or
        // the function's own result when it returns that error type; never
        // a block chosen by its parameter type alone.
        let error = &checker.error;
        let rendered = self.render(error);
        let mut func = f;
        let route = if let Some(handler) = intent.handler {
            match func.block(handler) {
                Some(block) if takes_one(block, error) && !block.unreachable => handler.to_owned(),
                _ => {
                    self.hole(
                        AgentErrorCode::RippleHoleUnfilled,
                        at,
                        format!(
                            "`{function}` has no block `{handler}` that takes one {rendered} (the error of `{}`): add it, or name another handler (expected {rendered})",
                            checker.name
                        ),
                    );
                    return None;
                }
            }
        } else if matches!(&func.result, TypeExpr::Result { error: declared, .. } if **declared == *error)
        {
            error_exit(&mut func, error, at, self)
        } else {
            self.hole(
                AgentErrorCode::RippleHoleUnfilled,
                at,
                format!(
                    "`{}` fails with {rendered}, which `{function}` (returning {}) cannot return: name the block of `{function}` that takes the error with \"handler\", or write the call (expected {rendered})",
                    checker.name,
                    self.render(&func.result)
                ),
            );
            return None;
        };
        // Entry guards run in written order: a new one goes after the
        // guards the function already starts with, and checks the value
        // they leave for `arg`.
        let chain = entry_chain(&func);
        let (continuation, tail) = match chain.last() {
            Some(link) => (link.next.clone(), Some(link.guarded.clone())),
            None => (func.entry.clone(), None),
        };
        let mut current = Val::Param(arg.to_owned());
        for link in &chain {
            if link.arg == current {
                current = link.checked.clone();
            }
        }
        // The new guard branches to the checked continuation and to the
        // error route; only what the error route cannot reach is dominated
        // by the checked value.
        let mut reach = func.reachable_from(&continuation);
        let mut from_route = BTreeSet::new();
        let mut stack = vec![route.clone()];
        while let Some(leaf) = stack.pop() {
            if !from_route.insert(leaf.clone()) {
                continue;
            }
            if let Some(block) = func.block(&leaf) {
                stack.extend(
                    block
                        .term
                        .edges()
                        .into_iter()
                        .map(|edge| edge.target.clone()),
                );
            }
        }
        reach.retain(|leaf| !from_route.contains(leaf));
        let taken = func.taken();
        let mut all = taken.clone();
        let guard_leaf = self.unique(&format!("{arg}__guard"), &all);
        all.insert(guard_leaf.clone());
        let checked_leaf = self.unique(&format!("{arg}__guarded"), &all);
        all.insert(checked_leaf.clone());
        let result = self.unique(&format!("{arg}__r"), &all);
        all.insert(result.clone());
        let payload = self.unique(&format!("{arg}__v"), &all);
        all.insert(payload.clone());
        let tuple = self.unique(&format!("{arg}__t"), &all);
        all.insert(tuple.clone());
        let unwrapped = self.unique(&format!("{arg}__ok"), &all);
        let checked_value = Val::Op(checked_leaf.clone(), unwrapped.clone(), 0);
        let mut uses = 0_u64;
        for block in &mut func.blocks {
            if !reach.contains(&block.leaf) {
                continue;
            }
            let mut touched = false;
            for op in &mut block.ops {
                for value in &mut op.args {
                    if *value == current {
                        *value = checked_value.clone();
                        touched = true;
                        uses += 1;
                    }
                }
            }
            for value in block.term.values_mut() {
                if *value == current {
                    *value = checked_value.clone();
                    touched = true;
                    uses += 1;
                }
            }
            if touched {
                block.changed.get_or_insert(at.to_owned());
            }
        }
        let mut cases = vec![
            (
                CaseKey::Builtin(BuiltinCase::Ok),
                Edge {
                    target: checked_leaf.clone(),
                    args: vec![Arg::Payload],
                },
            ),
            (
                CaseKey::Builtin(BuiltinCase::Err),
                Edge {
                    target: route.clone(),
                    args: vec![Arg::Payload],
                },
            ),
        ];
        cases.sort_by_key(|(key, _)| *key);
        func.blocks.push(Blk {
            leaf: guard_leaf.clone(),
            params: Vec::new(),
            ops: vec![Op {
                leaf: result.clone(),
                tag: 112,
                imm: Imm::Function {
                    id: checker.id,
                    name: checker.name.clone(),
                    generic: false,
                },
                args: vec![current],
                types: vec![TypeExpr::Result {
                    ok: Box::new(checker.param.clone()),
                    error: Box::new(error.clone()),
                }],
            }],
            term: Term::Switch(Val::Op(guard_leaf.clone(), result.clone(), 0), cases),
            unreachable: false,
            changed: Some(at.to_owned()),
        });
        func.blocks.push(Blk {
            leaf: checked_leaf.clone(),
            params: vec![(payload.clone(), checker.param.clone())],
            ops: vec![
                Op {
                    leaf: tuple.clone(),
                    tag: 16,
                    imm: Imm::None,
                    args: vec![Val::Block(checked_leaf.clone(), payload.clone())],
                    types: vec![TypeExpr::Tuple(vec![checker.param.clone()])],
                },
                Op {
                    leaf: unwrapped.clone(),
                    tag: 17,
                    imm: Imm::Index(0),
                    args: vec![Val::Op(checked_leaf.clone(), tuple.clone(), 0)],
                    types: vec![checker.param.clone()],
                },
            ],
            term: Term::Br(Edge {
                target: continuation,
                args: Vec::new(),
            }),
            unreachable: false,
            changed: Some(at.to_owned()),
        });
        for name in [
            &guard_leaf,
            &checked_leaf,
            &result,
            &payload,
            &tuple,
            &unwrapped,
        ] {
            func.generated.insert(name.clone(), at.to_owned());
        }
        if let Some(tail) =
            tail.and_then(|tail| func.blocks.iter_mut().find(|block| block.leaf == tail))
        {
            tail.term = Term::Br(Edge {
                target: guard_leaf,
                args: Vec::new(),
            });
            tail.changed.get_or_insert(at.to_owned());
        } else {
            func.entry = guard_leaf;
            func.entry_changed = Some(at.to_owned());
        }
        // The error edge reaches the named handler before anything else
        // `f` does, so what the handler (or a block after it) reads from
        // another block must still come first on every path to it.
        if let Some(handler) = intent.handler
            && let Some((block, value)) = undominated(&func, &from_route)
        {
            self.hole(
                AgentErrorCode::RippleHoleUnfilled,
                at,
                format!(
                    "the error of `{}` reaches handler `{handler}` of `{function}` before anything else `{function}` does, but block `{block}` reads `{value}`, which is then not computed on every path to it: name another handler, change `{handler}` to compute what it reads from its parameter and those of `{function}` (commit that first: a guard rewrites live functions), or write the call",
                    checker.name
                ),
            );
            return None;
        }
        self.funcs.insert(function.to_owned(), func);
        Some(json!({"fn": function, "edit": "entry", "uses": uses, "error": route}))
    }

    // --- output -------------------------------------------------------------

    /// Writes every derived change into the frame: a patch per rewritten
    /// function (or blocks added to the author's own patch), and the
    /// restated tests.
    #[allow(clippy::too_many_lines)]
    fn emit(&mut self) {
        let funcs = std::mem::take(&mut self.funcs);
        for (name, func) in &funcs {
            if !func.changed() {
                continue;
            }
            let mut blocks = Map::new();
            let mut origins = Vec::new();
            for block in &func.blocks {
                let Some(origin) = &block.changed else {
                    continue;
                };
                match render_block(self.cx, block) {
                    Ok(rendered) => {
                        blocks.insert(block.leaf.clone(), rendered);
                        origins.push((block.leaf.clone(), origin.clone()));
                    }
                    Err(why) => {
                        self.hole(
                            AgentErrorCode::RippleHoleUnfilled,
                            origin,
                            format!(
                                "block `{name}.{}` cannot be restated as AF1 because {why}: change it yourself",
                                block.leaf
                            ),
                        );
                    }
                }
            }
            for (leaf, origin) in &func.deleted {
                blocks.insert(leaf.clone(), Value::Null);
                origins.push((leaf.clone(), origin.clone()));
            }
            let first = func
                .entry_changed
                .clone()
                .or_else(|| origins.first().map(|(_, origin)| origin.clone()))
                .unwrap_or_else(|| "/ripple".to_owned());
            let patches = self
                .out
                .entry("patch".to_owned())
                .or_insert_with(|| Value::Array(Vec::new()));
            let Value::Array(patches) = patches else {
                continue;
            };
            let (index, fresh) = if let Some(index) = self.authored.patch.get(name) {
                (*index, false)
            } else {
                patches.push(json!({"fn": name, "blocks": {}}));
                (patches.len() - 1, true)
            };
            let Some(Value::Object(patch)) = patches.get_mut(index) else {
                continue;
            };
            if let Some(Value::Object(existing)) = patch.get_mut("blocks") {
                existing.extend(blocks);
            } else {
                patch.insert("blocks".to_owned(), Value::Object(blocks));
            }
            if func.entry_changed.is_some() {
                patch.insert("entry".to_owned(), Value::from(func.entry.as_str()));
            }
            if fresh {
                self.map.entries.push(MapEntry {
                    expanded: format!("/patch/{index}"),
                    authored: first,
                    role: Role::Ripple,
                    name: name.clone(),
                });
            }
            for (leaf, origin) in origins {
                self.map.entries.push(MapEntry {
                    expanded: format!("/patch/{index}/blocks/{leaf}"),
                    authored: origin,
                    role: Role::Ripple,
                    name: leaf,
                });
            }
            if !func.generated.is_empty() {
                self.map
                    .names
                    .entry(name.clone())
                    .or_default()
                    .extend(func.generated.clone());
            }
        }
        self.funcs = funcs;
        let tests = std::mem::take(&mut self.tests);
        for (name, (entry, origin)) in tests {
            let list = self
                .out
                .entry("tests".to_owned())
                .or_insert_with(|| Value::Array(Vec::new()));
            let Value::Array(list) = list else { continue };
            list.push(entry);
            self.map.entries.push(MapEntry {
                expanded: format!("/tests/{}", list.len() - 1),
                authored: origin,
                role: Role::Ripple,
                name,
            });
        }
    }

    fn inventory(&self) -> Value {
        let functions: BTreeSet<String> = self
            .funcs
            .iter()
            .filter(|(_, func)| func.changed())
            .map(|(name, _)| name.clone())
            .chain(self.records.iter().flat_map(|record| {
                record["changed"]["functions"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|name| name.as_str().map(str::to_owned))
            }))
            .collect();
        let tests: BTreeSet<String> = self
            .records
            .iter()
            .flat_map(|record| {
                record["changed"]["tests"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|name| name.as_str().map(str::to_owned))
            })
            .collect();
        json!({
            "intents": self.records,
            "changed": {"functions": functions, "tests": tests},
            "edits": self.edits,
        })
    }
}

/// How ripple reads one call or test the frame itself writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reading {
    /// Written for the new parameters: kept.
    AsWritten,
    /// Written for the old parameters: rewritten by name.
    Rewrite,
    /// Its argument count fits both parameter lists: the author decides.
    Ambiguous,
    /// Declared old, but its argument count is not the old one.
    Contradicts,
    /// Its argument count fits neither list: the compiler reports it.
    Compiler,
    /// Stated in a revision after the intent, with a count that fits both
    /// lists.
    Revised,
}

/// The reading of a call or test stated after the intent: by its count
/// alone.
fn revised_reading(count: usize, old: usize, new: usize) -> Reading {
    match reading(None, count, old, new) {
        Reading::Ambiguous => Reading::Revised,
        other => other,
    }
}

/// The reading of a frame-written call of `count` arguments, given the
/// author's declaration (`Some(true)`: old, `Some(false)`: new) and the old
/// and new parameter counts.
fn reading(declared: Option<bool>, count: usize, old: usize, new: usize) -> Reading {
    match declared {
        Some(false) => Reading::AsWritten,
        Some(true) if count == old => Reading::Rewrite,
        Some(true) => Reading::Contradicts,
        None if count == old && count == new => Reading::Ambiguous,
        None if count == new => Reading::AsWritten,
        None if count == old => Reading::Rewrite,
        None => Reading::Compiler,
    }
}

/// The arity change one intent derives from.
struct Arity {
    target: String,
    at: String,
    new: Vec<(String, TypeExpr)>,
    slots: Vec<Slot>,
    fill: Option<Fill>,
}

/// A call (or `fnref`) of the target in live code.
struct LiveSite {
    function: String,
    block: String,
    op: String,
    fnref: bool,
}

/// Whether `name` is the operation an arity intent adds to load the
/// supplied value for argument `position` of the call named `call`:
/// `<call>__v<position>`, with a collision suffix `_2`, `_3`, ...
fn is_fill(call: &str, name: &str, position: usize) -> bool {
    let base = format!("{call}__v{position}");
    name == base
        || name
            .strip_prefix(&base)
            .and_then(|rest| rest.strip_prefix('_'))
            .is_some_and(all_digits)
        || (name.len() > MAX_NAME - 20 && name == shorten(&base))
}

fn all_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// Whether a constant value holds a reference to function `id`.
fn refers(value: &ConstValue, id: &EntityId) -> bool {
    match &value.data {
        ConstData::FunctionRef(reference) => reference.function == *id,
        ConstData::Sequence(items) => items.iter().any(|item| refers(item, id)),
        ConstData::Record(record) => record.fields.iter().any(|field| refers(&field.value, id)),
        ConstData::Variant(variant) => variant
            .payload
            .as_ref()
            .is_some_and(|payload| refers(payload, id)),
        ConstData::Map(entries) => entries
            .iter()
            .any(|entry| refers(&entry.key, id) || refers(&entry.value, id)),
        ConstData::Option(item) => item.as_ref().is_some_and(|item| refers(item, id)),
        ConstData::Result(ResultConst::Ok(item) | ResultConst::Err(item)) => refers(item, id),
        _ => false,
    }
}

fn namespace_text(namespaces: &[String]) -> String {
    if namespaces.is_empty() {
        "none".to_owned()
    } else {
        namespaces.join(", ")
    }
}

fn mentions_frame_type(ty: &TypeExpr, frame_types: &BTreeMap<EntityId, String>) -> bool {
    let mut found = false;
    visit_named(ty, &mut |id| found |= frame_types.contains_key(id));
    found
}

fn visit_named(ty: &TypeExpr, visit: &mut dyn FnMut(&EntityId)) {
    match ty {
        TypeExpr::Named(named) => {
            visit(&named.definition);
            for argument in &named.arguments {
                visit_named(argument, visit);
            }
        }
        TypeExpr::Option(item) | TypeExpr::Vector(item) | TypeExpr::LocalCell(item) => {
            visit_named(item, visit);
        }
        TypeExpr::Result { ok, error } => {
            visit_named(ok, visit);
            visit_named(error, visit);
        }
        TypeExpr::OrderedMap { key, value } => {
            visit_named(key, visit);
            visit_named(value, visit);
        }
        TypeExpr::Tuple(items) => {
            for item in items {
                visit_named(item, visit);
            }
        }
        _ => {}
    }
}

/// One entry guard a function starts with: the block calling the checker,
/// the block holding its checked value, where that one continues, the
/// checker, the value it checks, the value it leaves and its error edge.
struct Link {
    block: String,
    call: String,
    guarded: String,
    next: String,
    checker: String,
    arg: Val,
    checked: Val,
    error: Edge,
}

/// The entry guards `func` starts with, in the order they run: each block
/// holds one call of a checker and switches on it, `Ok` passing the value
/// to a block that holds it (`tuple`, `tuple_get 0`) and branches on.
fn entry_chain(func: &Func) -> Vec<Link> {
    let mut out: Vec<Link> = Vec::new();
    let mut current = func.entry.clone();
    for _ in 0..func.blocks.len() {
        let Some(block) = func.block(&current) else {
            break;
        };
        let ([op], true) = (block.ops.as_slice(), block.params.is_empty()) else {
            break;
        };
        let (Imm::Function { name, .. }, [arg]) = (&op.imm, op.args.as_slice()) else {
            break;
        };
        let Term::Switch(Val::Op(owner, value, 0), cases) = &block.term else {
            break;
        };
        let case = |key: BuiltinCase| {
            cases
                .iter()
                .find(|(case, _)| *case == CaseKey::Builtin(key))
                .map(|(_, edge)| edge.clone())
        };
        let (Some(ok), Some(error)) = (case(BuiltinCase::Ok), case(BuiltinCase::Err)) else {
            break;
        };
        let Some(guarded) = func.block(&ok.target) else {
            break;
        };
        let holds = match (
            guarded.params.as_slice(),
            guarded.ops.as_slice(),
            &guarded.term,
        ) {
            ([(param, _)], [tuple, first], Term::Br(next))
                if tuple.tag == 16
                    && tuple.args == [Val::Block(guarded.leaf.clone(), param.clone())]
                    && first.tag == 17
                    && first.imm == Imm::Index(0)
                    && first.args == [Val::Op(guarded.leaf.clone(), tuple.leaf.clone(), 0)]
                    && next.args.is_empty() =>
            {
                Some((next.target.clone(), first.leaf.clone()))
            }
            _ => None,
        };
        let Some((next, unwrapped)) = holds else {
            break;
        };
        if *owner != block.leaf
            || *value != op.leaf
            || op.tag != 112
            || cases.len() != 2
            || ok.args != [Arg::Payload]
            || error.args != [Arg::Payload]
            || out.iter().any(|link| link.block == next)
        {
            break;
        }
        out.push(Link {
            block: block.leaf.clone(),
            call: op.leaf.clone(),
            guarded: guarded.leaf.clone(),
            next: next.clone(),
            checker: name.clone(),
            arg: arg.clone(),
            checked: Val::Op(guarded.leaf.clone(), unwrapped, 0),
            error,
        });
        current = next;
    }
    out
}

/// Where `func` already has the shape a guard of `checker` on `arg`
/// derives: a block whose last operation calls the checker on the parameter
/// and which switches on that call, its `Err` case passing the error to the
/// block that returns it (or to `handler`). For `entry` the block is the
/// function's entry block holding that call alone, and `Ok` passes the
/// checked value on; for `preserve`, `Ok` continues without it.
fn guarded_call(
    func: &Func,
    checker: &str,
    arg: &str,
    entry: bool,
    handler: Option<&str>,
) -> Option<String> {
    let error_fits = |edge: &Edge| {
        edge.args == [Arg::Payload]
            && func
                .block(&edge.target)
                .is_some_and(|target| match handler {
                    Some(handler) => target.leaf == handler,
                    None => {
                        target.params.len() == 1 && is_error_return(target, &target.params[0].0)
                    }
                })
    };
    if entry {
        // A guard in the chain the function starts with, on the value the
        // guards before it leave for `arg`.
        let mut current = Val::Param(arg.to_owned());
        for link in entry_chain(func) {
            if link.arg == current {
                if link.checker == checker && error_fits(&link.error) {
                    return Some(format!("{}.{}.{}", func.name, link.block, link.call));
                }
                current = link.checked;
            }
        }
        return None;
    }
    func.blocks.iter().find_map(|block| {
        let op = block.ops.last()?;
        let calls = matches!(&op.imm, Imm::Function { name, .. } if name == checker)
            && op.tag == 112
            && op.args == [Val::Param(arg.to_owned())];
        let Term::Switch(Val::Op(owner, name, 0), cases) = &block.term else {
            return None;
        };
        let case = |key: BuiltinCase| {
            cases
                .iter()
                .find(|(case, _)| *case == CaseKey::Builtin(key))
                .map(|(_, edge)| edge)
        };
        let (ok, err) = (case(BuiltinCase::Ok)?, case(BuiltinCase::Err)?);
        let ok_fits = !ok.args.contains(&Arg::Payload);
        let err_fits = error_fits(err);
        (calls
            && *owner == block.leaf
            && *name == op.leaf
            && cases.len() == 2
            && ok_fits
            && err_fits)
            .then(|| format!("{}.{}.{}", func.name, block.leaf, op.leaf))
    })
}

/// Why a patch cannot restate a live function whole: AF1 states neither
/// type parameters nor declared effects.
/// The first use, in one of `blocks`, of a value another block defines
/// that does not dominate the using block: `(block, "owner.value")`.
fn undominated(func: &Func, blocks: &BTreeSet<String>) -> Option<(String, String)> {
    let reachable = func.reachable();
    let mut dominators: BTreeMap<String, BTreeSet<String>> = reachable
        .iter()
        .map(|leaf| {
            let all = if *leaf == func.entry {
                BTreeSet::from([leaf.clone()])
            } else {
                reachable.clone()
            };
            (leaf.clone(), all)
        })
        .collect();
    // Iterative dataflow; each round only shrinks the sets, so it ends
    // within the block count.
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            if block.leaf == func.entry || !reachable.contains(&block.leaf) {
                continue;
            }
            let mut meet: Option<BTreeSet<String>> = None;
            for pred in func.predecessors(&block.leaf) {
                let Some(set) = dominators.get(&pred) else {
                    continue;
                };
                meet = Some(match meet {
                    None => set.clone(),
                    Some(meet) => meet.intersection(set).cloned().collect(),
                });
            }
            let mut next = meet.unwrap_or_default();
            next.insert(block.leaf.clone());
            if dominators.get(&block.leaf) != Some(&next) {
                dominators.insert(block.leaf.clone(), next);
                changed = true;
            }
        }
    }
    for block in &func.blocks {
        if !blocks.contains(&block.leaf) || !reachable.contains(&block.leaf) {
            continue;
        }
        for value in block.uses() {
            let (Val::Op(owner, name, _) | Val::Block(owner, name)) = value else {
                continue;
            };
            let dominated = dominators
                .get(&block.leaf)
                .is_some_and(|set| set.contains(owner));
            if *owner != block.leaf && !dominated {
                return Some((block.leaf.clone(), format!("{owner}.{name}")));
            }
        }
    }
    None
}

fn unpatchable(func: &Func) -> Option<&'static str> {
    if func.generic {
        Some("has type parameters")
    } else if func.effects {
        Some("declares effects")
    } else {
        None
    }
}

/// Whether a block takes exactly one parameter, of type `ty`.
fn takes_one(block: &Blk, ty: &TypeExpr) -> bool {
    matches!(block.params.as_slice(), [(_, param)] if param == ty)
}

/// Whether a block is `(e: E) { r = err e; return r }`.
fn is_error_return(block: &Blk, param: &str) -> bool {
    let [op] = block.ops.as_slice() else {
        return false;
    };
    op.tag == 131
        && op.args == [Val::Block(block.leaf.clone(), param.to_owned())]
        && matches!(&block.term, Term::Return(Val::Op(owner, name, 0)) if *owner == block.leaf && *name == op.leaf)
}

/// A block of `func` that returns an `E` error unchanged: an existing one,
/// or a new one.
fn error_exit(func: &mut Func, error: &TypeExpr, at: &str, ripple: &Ripple<'_, '_>) -> String {
    for block in &func.blocks {
        if let [(param, ty)] = block.params.as_slice()
            && ty == error
            && !block.unreachable
            && is_error_return(block, param)
        {
            return block.leaf.clone();
        }
    }
    let taken = func.taken();
    let leaf = ripple.unique("__err", &taken);
    let mut all = taken;
    all.insert(leaf.clone());
    let param = ripple.unique("e", &all);
    all.insert(param.clone());
    let result = ripple.unique("r", &all);
    func.blocks.push(Blk {
        leaf: leaf.clone(),
        params: vec![(param.clone(), error.clone())],
        ops: vec![Op {
            leaf: result.clone(),
            tag: 131,
            imm: Imm::None,
            args: vec![Val::Block(leaf.clone(), param.clone())],
            types: vec![func.result.clone()],
        }],
        term: Term::Return(Val::Op(leaf.clone(), result.clone(), 0)),
        unreachable: false,
        changed: Some(at.to_owned()),
    });
    for name in [&leaf, &param, &result] {
        func.generated.insert(name.clone(), at.to_owned());
    }
    leaf
}

/// Whether the function `name`, as the frame leaves it, declares effects.
fn declares_effects(view: &View<'_, '_>, name: &str) -> bool {
    view.function(name)
        .and_then(|id| view.body(&id))
        .is_some_and(|body| {
            matches!(body, EntityBodyValue::Function(callee) if !callee.effects.as_slice().is_empty())
        })
}

/// Where `func` can perform an effect: an effect, adapter, capability or
/// observation operation, or a call of a function that declares effects.
fn effect_site(func: &Func, view: &View<'_, '_>) -> Option<String> {
    if func.effects {
        return Some("it declares effects".to_owned());
    }
    for block in &func.blocks {
        for op in &block.ops {
            let effectful = match &op.imm {
                Imm::Function { name, .. } if op.tag == 112 => declares_effects(view, name),
                _ => matches!(op.tag, 145 | 160..=162),
            };
            if effectful {
                return Some(format!("`{}.{}.{}`", func.name, block.leaf, op.leaf));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Preserve: structural match of the checker's body inside a function
// ---------------------------------------------------------------------------

/// What a checker block is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    /// Pure operations, then a branch, switch or trap.
    Check,
    /// `r = ok q; return r`: success, with the checked parameter.
    Success,
    /// Pure operations ending `r = err e; return r`.
    Error,
}

/// The checker's blocks by kind, or why the checker is outside the
/// supported subset (pure, acyclic, returning `ok` of its parameter or an
/// `err`).
fn classify(
    g: &Func,
    q: &str,
    view: &View<'_, '_>,
) -> std::result::Result<BTreeMap<String, Kind>, String> {
    if g.effects {
        return Err("it declares effects".to_owned());
    }
    let mut kinds = BTreeMap::new();
    for block in &g.blocks {
        for op in &block.ops {
            let pure = match &op.imm {
                Imm::Function { name, .. } if op.tag == 112 => {
                    *name != g.name && !declares_effects(view, name)
                }
                _ => !matches!(op.tag, 145 | 160..=162 | 176..=178 | 194),
            };
            if !pure {
                return Err(format!(
                    "operation `{}.{}` is not pure",
                    block.leaf, op.leaf
                ));
            }
        }
        let kind = match &block.term {
            Term::Return(value) => {
                let Some(last) = block.ops.last() else {
                    return Err(format!(
                        "block `{}` returns a value it does not make",
                        block.leaf
                    ));
                };
                if *value != Val::Op(block.leaf.clone(), last.leaf.clone(), 0) {
                    return Err(format!(
                        "block `{}` returns a value it does not make",
                        block.leaf
                    ));
                }
                match last.tag {
                    130 => {
                        let success = block.ops.len() == 1
                            && match last.args.as_slice() {
                                [Val::Param(name)] => name == q,
                                [Val::Block(owner, _)] => *owner == block.leaf,
                                _ => false,
                            };
                        if !success {
                            return Err(format!(
                                "block `{}` succeeds with a value other than its parameter",
                                block.leaf
                            ));
                        }
                        Kind::Success
                    }
                    131 => Kind::Error,
                    _ => {
                        return Err(format!(
                            "block `{}` returns a value made without ok or err",
                            block.leaf
                        ));
                    }
                }
            }
            _ => Kind::Check,
        };
        kinds.insert(block.leaf.clone(), kind);
    }
    // A success block that takes its value as a parameter gets the checked
    // parameter on every edge into it.
    for block in &g.blocks {
        if kinds.get(&block.leaf) != Some(&Kind::Success) {
            continue;
        }
        let Some(Val::Block(_, name)) = block.ops[0].args.first() else {
            continue;
        };
        let position = block.params.iter().position(|(param, _)| param == name);
        for pred in &g.blocks {
            for edge in pred.term.edges() {
                if edge.target == block.leaf
                    && position.and_then(|position| edge.args.get(position))
                        != Some(&Arg::Val(Val::Param(q.to_owned())))
                {
                    return Err(format!(
                        "block `{}` succeeds with a value other than its parameter",
                        block.leaf
                    ));
                }
            }
        }
    }
    acyclic(g, &g.entry, &mut Vec::new(), &mut BTreeSet::new())?;
    if !kinds.values().any(|kind| *kind == Kind::Success) {
        return Err("it never succeeds".to_owned());
    }
    Ok(kinds)
}

/// Every edge from `leaf` goes to a block not on the current path.
fn acyclic(
    g: &Func,
    leaf: &str,
    path: &mut Vec<String>,
    done: &mut BTreeSet<String>,
) -> std::result::Result<(), String> {
    if path.iter().any(|seen| seen == leaf) {
        return Err(format!("its body loops (at block `{leaf}`)"));
    }
    if !done.insert(leaf.to_owned()) {
        return Ok(());
    }
    path.push(leaf.to_owned());
    if let Some(block) = g.block(leaf) {
        for edge in block.term.edges() {
            acyclic(g, &edge.target, path, done)?;
        }
    }
    path.pop();
    Ok(())
}

/// One matched check inside a function.
struct Region {
    /// The block whose last operations and terminator are the check's start.
    anchor: String,
    /// Where the check starts in the anchor block.
    start: usize,
    /// Other blocks of the function the check's branching blocks map to
    /// (its error exits, which only return, may be shared).
    blocks: BTreeSet<String>,
    /// The success continuation.
    cont: Edge,
}

struct Matcher<'m> {
    g: &'m Func,
    f: &'m Func,
    kinds: &'m BTreeMap<String, Kind>,
    q: String,
    p: String,
    vals: BTreeMap<Val, Val>,
    blocks: BTreeMap<String, String>,
    taken: BTreeSet<String>,
    cont: Option<Edge>,
    steps: usize,
    /// The program as the frame leaves it (constants by their candidate
    /// values).
    source: &'m dyn Source,
}

impl Matcher<'_> {
    fn map(&self, value: &Val) -> Option<Val> {
        if *value == Val::Param(self.q.clone()) {
            return Some(Val::Param(self.p.clone()));
        }
        self.vals.get(value).cloned()
    }

    fn const_value(&self, id: &EntityId, other: &EntityId) -> bool {
        id == other
            || matches!(
                (self.source.body(id), self.source.body(other)),
                (Some(EntityBodyValue::Constant(a)), Some(EntityBodyValue::Constant(b))) if a.value == b.value
            )
    }

    fn imm_eq(&self, tag: u32, a: &Imm, b: &Imm) -> bool {
        match (a, b) {
            (Imm::Entity(a), Imm::Entity(b)) if tag == 1 => self.const_value(a, b),
            // By name: the same function of the candidate, whichever
            // statement (live or the frame's) each side was read from.
            (
                Imm::Function {
                    name: a,
                    generic: false,
                    ..
                },
                Imm::Function {
                    name: b,
                    generic: false,
                    ..
                },
            ) => a == b,
            (Imm::Function { .. }, _) | (_, Imm::Function { .. }) => false,
            (a, b) => a == b,
        }
    }

    /// `g`'s operations of `gb` against `fb`'s from `start` to its end.
    fn ops(&mut self, gb: &Blk, fb: &Blk, start: usize) -> bool {
        if fb.ops.len() < start || fb.ops.len() - start != gb.ops.len() {
            return false;
        }
        for (go, fo) in gb.ops.iter().zip(&fb.ops[start..]) {
            self.steps += 1;
            if self.steps > MAX_MATCH_STEPS {
                return false;
            }
            if go.tag != fo.tag
                || go.args.len() != fo.args.len()
                || go.types.len() != fo.types.len()
                || !self.imm_eq(go.tag, &go.imm, &fo.imm)
            {
                return false;
            }
            let types_fit = match (go.tag, go.types.first(), fo.types.first()) {
                // `err` differs only in the success type of the result.
                (
                    131,
                    Some(TypeExpr::Result { error: a, .. }),
                    Some(TypeExpr::Result { error: b, .. }),
                ) => a == b,
                _ => go.types == fo.types,
            };
            if !types_fit {
                return false;
            }
            for (ga, fa) in go.args.iter().zip(&fo.args) {
                if self.map(ga).as_ref() != Some(fa) {
                    return false;
                }
            }
            for index in 0..go.types.len() {
                let index = u32::try_from(index).unwrap_or(u32::MAX);
                self.vals.insert(
                    Val::Op(gb.leaf.clone(), go.leaf.clone(), index),
                    Val::Op(fb.leaf.clone(), fo.leaf.clone(), index),
                );
            }
        }
        true
    }

    /// Tries the checker's entry at the end of `fb`.
    fn anchor(&mut self, fb: &Blk) -> bool {
        let Some(gb) = self.g.block(&self.g.entry) else {
            return false;
        };
        if self.kinds.get(&gb.leaf) != Some(&Kind::Check) || fb.ops.len() < gb.ops.len() {
            return false;
        }
        let start = fb.ops.len() - gb.ops.len();
        // The anchor is never also one of the check's other blocks.
        self.taken.insert(fb.leaf.clone());
        self.ops(gb, fb, start) && self.term(gb, fb)
    }

    fn term(&mut self, gb: &Blk, fb: &Blk) -> bool {
        match (&gb.term, &fb.term) {
            (Term::Br(ge), Term::Br(fe)) => self.edge(ge, fe),
            (Term::Cond(gc, g1, g2), Term::Cond(fc, f1, f2)) => {
                self.map(gc).as_ref() == Some(fc) && self.edge(g1, f1) && self.edge(g2, f2)
            }
            (Term::Switch(gv, gcases), Term::Switch(fv, fcases)) => {
                self.map(gv).as_ref() == Some(fv)
                    && gcases.len() == fcases.len()
                    && gcases
                        .iter()
                        .zip(fcases)
                        .all(|((gk, ge), (fk, fe))| gk == fk && self.edge(ge, fe))
            }
            (Term::Trap(gc, gp), Term::Trap(fc, fp)) => {
                gc == fc
                    && match (gp, fp) {
                        (None, None) => true,
                        (Some(gp), Some(fp)) => self.map(gp).as_ref() == Some(fp),
                        _ => false,
                    }
            }
            _ => false,
        }
    }

    fn edge(&mut self, ge: &Edge, fe: &Edge) -> bool {
        self.steps += 1;
        if self.steps > MAX_MATCH_STEPS {
            return false;
        }
        let Some(gt) = self.g.block(&ge.target) else {
            return false;
        };
        match self.kinds.get(&gt.leaf) {
            Some(Kind::Success) => {
                // The success value must be the checked parameter itself.
                let returns_param = match gt.ops[0].args.as_slice() {
                    [Val::Param(name)] => *name == self.q,
                    [Val::Block(_, name)] => {
                        gt.params
                            .iter()
                            .position(|(param, _)| param == name)
                            .and_then(|position| ge.args.get(position))
                            == Some(&Arg::Val(Val::Param(self.q.clone())))
                    }
                    _ => false,
                };
                if !returns_param || fe.args.contains(&Arg::Payload) {
                    return false;
                }
                match &self.cont {
                    None => {
                        self.cont = Some(fe.clone());
                        true
                    }
                    Some(cont) => cont == fe,
                }
            }
            Some(kind) => {
                let kind = *kind;
                if ge.args.len() != fe.args.len() {
                    return false;
                }
                for (ga, fa) in ge.args.iter().zip(&fe.args) {
                    let fits = match (ga, fa) {
                        (Arg::Payload, Arg::Payload) => true,
                        (Arg::Val(ga), Arg::Val(fa)) => self.map(ga).as_ref() == Some(fa),
                        _ => false,
                    };
                    if !fits {
                        return false;
                    }
                }
                if let Some(mapped) = self.blocks.get(&gt.leaf) {
                    return *mapped == fe.target;
                }
                if self.taken.contains(&fe.target) {
                    return false;
                }
                let Some(ft) = self.f.block(&fe.target) else {
                    return false;
                };
                if gt.params.len() != ft.params.len()
                    || gt.params.iter().zip(&ft.params).any(|(a, b)| a.1 != b.1)
                {
                    return false;
                }
                self.blocks.insert(gt.leaf.clone(), ft.leaf.clone());
                self.taken.insert(ft.leaf.clone());
                for ((gp, _), (fp, _)) in gt.params.iter().zip(&ft.params) {
                    self.vals.insert(
                        Val::Block(gt.leaf.clone(), gp.clone()),
                        Val::Block(ft.leaf.clone(), fp.clone()),
                    );
                }
                if !self.ops(gt, ft, 0) {
                    return false;
                }
                match kind {
                    Kind::Error => match (&gt.term, &ft.term) {
                        (Term::Return(gv), Term::Return(fv)) => self.map(gv).as_ref() == Some(fv),
                        _ => false,
                    },
                    _ => self.term(gt, ft),
                }
            }
            None => false,
        }
    }

    /// The matched region, once its values are known not to be used
    /// outside it.
    fn region(&self, anchor: &str) -> std::result::Result<Region, String> {
        let Some(cont) = self.cont.clone() else {
            return Err("never continues".to_owned());
        };
        if self.taken.contains(&cont.target) {
            return Err(format!(
                "continues into block `{}`, which is part of the check",
                cont.target
            ));
        }
        let anchor_block = self.f.block(anchor).ok_or_else(|| "vanished".to_owned())?;
        let start = anchor_block.ops.len() - self.g.block(&self.g.entry).map_or(0, |b| b.ops.len());
        // Values the replacement removes from the anchor.
        let mut removed: BTreeSet<Val> = anchor_block.ops[start..]
            .iter()
            .map(|op| Val::Op(anchor.to_owned(), op.leaf.clone(), 0))
            .collect();
        // Mapped blocks that only the check reaches go with it.
        let mut deletable: BTreeSet<String> = self.blocks.values().cloned().collect();
        loop {
            let before = deletable.len();
            let snapshot = deletable.clone();
            deletable.retain(|leaf| {
                self.f
                    .predecessors(leaf)
                    .iter()
                    .all(|pred| pred == anchor || snapshot.contains(pred))
            });
            if deletable.len() == before {
                break;
            }
        }
        for leaf in &deletable {
            if let Some(block) = self.f.block(leaf) {
                removed.extend(block.defines());
            }
        }
        for arg in &cont.args {
            if let Arg::Val(value) = arg
                && removed.contains(value)
            {
                return Err(format!(
                    "passes {} of the check to its continuation",
                    render_val("", value).unwrap_or_default()
                ));
            }
        }
        for block in &self.f.blocks {
            if deletable.contains(&block.leaf) {
                continue;
            }
            let uses: Vec<&Val> = if block.leaf == anchor {
                anchor_block.ops[..start]
                    .iter()
                    .flat_map(|op| op.args.iter())
                    .collect()
            } else {
                block.uses()
            };
            if let Some(value) = uses.into_iter().find(|value| removed.contains(value)) {
                return Err(format!(
                    "defines {}, which block `{}` uses",
                    render_val("", value).unwrap_or_default(),
                    block.leaf
                ));
            }
        }
        let checks = self
            .blocks
            .iter()
            .filter(|(g, _)| self.kinds.get(*g) == Some(&Kind::Check))
            .map(|(_, f)| f.clone())
            .collect();
        Ok(Region {
            anchor: anchor.to_owned(),
            start,
            blocks: checks,
            cont,
        })
    }
}

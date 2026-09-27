//! AF1-X: the authoring dialect of AF1 frames (`"afx": 1`).
//!
//! An AF1-X frame is an AF1 frame with `"afx": 1`. Inside function blocks
//! (`fns` and `patch`) it allows four conveniences, each expanded here into
//! plain AF1 before the unchanged frame compiler runs:
//!
//! - X1: nested operations and typed literals as operands;
//! - X2: checked propagation, `opcode?` and `opcode?Case`;
//! - X3: conditional exits `["!Case", "if", c]` and the `ok` and `fail`
//!   terminators;
//! - X4: qualification of unique dominating results, value threading
//!   between the generated pieces of a block, and derived trailing edge
//!   arguments.
//!
//! It also lowers `test_tables` into AF1 `tests` ([`crate::tables`]).
//!
//! Expansion is a pure function of the frame, the accepted program and its
//! names: no clock, no randomness. Every generated name derives from its
//! authored position (collision suffixes `_2`, `_3`, ... in a fixed order),
//! and a source map leads every expanded pointer back to the authored one.
//! A decision the frame leaves open (a literal without a type, an ambiguous
//! failure route, a value that is not available) is an [`Obligation`],
//! never a guess.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};
use sley_id::EntityId;
use sley_mutate::value::BlockBody;
use sley_mutate::value::EntityBodyValue;
use sley_ssmc::{
    BuiltinCase, BuiltinFailureKind, CaseKey, FunctionType, Immediate, IntegerWidth, NamedType,
    OperationResultRef, SwitchArgument, Terminator, TypeDefForm, TypeExpr, ValueRef,
};

use crate::error::{AgentError, AgentErrorCode, Result, frame};
use crate::names::{Names, Scope, is_identifier};
use crate::opcodes::{self, ImmediateKind, OpcodeRow};
use crate::types::{self, TypeNames};
use crate::workspace::Program;

/// Deepest nesting of operations inside one operand.
pub const MAX_EXPR_DEPTH: usize = 32;
/// Most operations one expanded function may hold.
pub const MAX_EXPANDED_OPS_PER_FUNCTION: usize = 4096;
/// Most generated blocks (continuations and shared exits) per function.
pub const MAX_GENERATED_BLOCKS_PER_FUNCTION: usize = 1024;
/// Longest name the AF1 name grammar admits.
const MAX_NAME: usize = 64;

/// The result of expanding an AF1-X frame.
#[derive(Clone, Debug)]
pub struct Expansion {
    /// The plain AF1 frame (without `afx` and `test_tables`).
    pub frame: Value,
    /// Expanded pointer to authored pointer, and generated names.
    pub map: SourceMap,
    /// Feature use counts.
    pub stats: AfxStats,
    /// Decisions the frame leaves open; when any is present, `frame` is
    /// incomplete and must not be compiled.
    pub obligations: Vec<Obligation>,
}

/// What a source-map entry's expanded construct is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// The first piece of an authored block.
    Block,
    /// An authored operation.
    Op,
    /// An authored terminator (and the operation an `ok` terminator adds).
    Term,
    /// A nested operation hoisted into the block.
    Nested,
    /// A literal made a constant operation.
    Literal,
    /// The Result/Option operation of a checked operation (`x__r`).
    Checked,
    /// The switch that ends a piece at a checked operation.
    Switch,
    /// A continuation block after a checked operation or an exit.
    Continuation,
    /// The `cond` that ends a piece at an exit operation.
    Exit,
    /// A derived trailing edge argument.
    DerivedArg,
    /// A reference rewritten to the piece that holds the value.
    Qualified,
    /// A test lowered from a test-table row.
    TableRow,
    /// A shared exit block (`__fail_Case`, `__err`, `__none`).
    SharedExit,
}

impl Role {
    /// The role's name in `sourcemap.json`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Op => "op",
            Self::Term => "term",
            Self::Nested => "nested",
            Self::Literal => "literal",
            Self::Checked => "checked",
            Self::Switch => "switch",
            Self::Continuation => "continuation",
            Self::Exit => "exit",
            Self::DerivedArg => "derived-arg",
            Self::Qualified => "qualified",
            Self::TableRow => "table-row",
            Self::SharedExit => "shared-exit",
        }
    }

    /// Whether the construct keeps the authored shape below its pointer, so
    /// a deeper expanded pointer maps to the same path under the authored one.
    const fn carries(self) -> bool {
        matches!(
            self,
            Self::Block | Self::Op | Self::Term | Self::Checked | Self::TableRow
        )
    }
}

/// One source-map entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapEntry {
    /// JSON pointer in the expanded frame.
    pub expanded: String,
    /// JSON pointer in the authored frame.
    pub authored: String,
    /// What the expanded construct is.
    pub role: Role,
    /// The generated (or authored) name of the construct.
    pub name: String,
}

/// Expanded-to-authored pointers and the authored origin of every
/// generated name.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceMap {
    /// Entries in expansion order.
    pub entries: Vec<MapEntry>,
    /// Per function: every generated block and value name to the authored
    /// pointer it comes from (a block's entry wins when a value has the
    /// same name).
    pub names: BTreeMap<String, BTreeMap<String, String>>,
    /// Per function: generated block names only.
    pub blocks: BTreeMap<String, BTreeMap<String, String>>,
    /// Per function: generated value names only.
    pub values: BTreeMap<String, BTreeMap<String, String>>,
}

impl SourceMap {
    /// The map as `sourcemap.json`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let entries: Vec<Value> = self
            .entries
            .iter()
            .map(|entry| {
                json!({"expanded": entry.expanded, "authored": entry.authored,
                       "role": entry.role.name(), "name": entry.name})
            })
            .collect();
        json!({"entries": entries, "names": self.names, "blocks": self.blocks, "values": self.values})
    }

    /// The authored pointer for an expanded pointer: the entry with the
    /// longest expanded prefix; `None` when no entry covers it (the pointer
    /// is then the same in both frames).
    #[must_use]
    pub fn authored(&self, pointer: &str) -> Option<String> {
        let mut best: Option<&MapEntry> = None;
        for entry in &self.entries {
            let covers = pointer == entry.expanded
                || (pointer.starts_with(&entry.expanded)
                    && pointer[entry.expanded.len()..].starts_with('/'));
            if covers && best.is_none_or(|best| entry.expanded.len() > best.expanded.len()) {
                best = Some(entry);
            }
        }
        best.map(|entry| {
            if entry.role.carries() {
                format!("{}{}", entry.authored, &pointer[entry.expanded.len()..])
            } else {
                entry.authored.clone()
            }
        })
    }

    /// A compiler refusal on the expanded frame, with every problem line's
    /// pointer rewritten to the authored one:
    /// `"<authored>: <detail> [expanded <pointer>]"`.
    #[must_use]
    pub fn rewrite(&self, error: &AgentError) -> AgentError {
        let lines: Vec<String> = crate::frame::problem_lines(error)
            .iter()
            .map(|line| self.rewrite_line(line))
            .collect();
        join_lines(error.code(), lines)
    }

    fn rewrite_line(&self, line: &str) -> String {
        let (prefix, rest) = match line
            .strip_prefix('[')
            .and_then(|rest| rest.split_once("] "))
        {
            Some((symbol, rest)) => (format!("[{symbol}] "), rest),
            None => (String::new(), line),
        };
        if !rest.starts_with('/') {
            return line.to_owned();
        }
        let Some((pointer, detail)) = rest.split_once(": ") else {
            return line.to_owned();
        };
        match self.authored(pointer) {
            Some(authored) if authored != pointer => {
                format!("{prefix}{authored}: {detail} [expanded {pointer}]")
            }
            _ => line.to_owned(),
        }
    }

    /// The authored pointer of a generated name of `function`.
    #[must_use]
    pub fn origin(&self, function: &str, name: &str) -> Option<&str> {
        self.names.get(function)?.get(name).map(String::as_str)
    }
}

/// Feature use counts of one expansion (for the events ledger).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AfxStats {
    /// Nested operations hoisted.
    pub nested: u64,
    /// Literals made constant operations.
    pub literals: u64,
    /// Checked operations (`op?`, `op?Case`).
    pub checked: u64,
    /// Exit operations (`["!Case", "if", c]`).
    pub exits: u64,
    /// `ok` and `fail` terminators.
    pub ok_fail: u64,
    /// Derived trailing edge arguments.
    pub derived_args: u64,
    /// Plain names qualified to a value of another block.
    pub qualified: u64,
    /// Test-table rows lowered.
    pub table_rows: u64,
    /// Generated blocks (continuations and shared exits).
    pub generated_blocks: u64,
}

impl AfxStats {
    /// The counts as a JSON object.
    #[must_use]
    pub fn to_json(&self) -> Map<String, Value> {
        let mut map = Map::new();
        for (key, value) in [
            ("nested", self.nested),
            ("literals", self.literals),
            ("checked", self.checked),
            ("exits", self.exits),
            ("ok_fail", self.ok_fail),
            ("derived_args", self.derived_args),
            ("qualified", self.qualified),
            ("table_rows", self.table_rows),
            ("generated_blocks", self.generated_blocks),
        ] {
            map.insert(key.to_owned(), Value::from(value));
        }
        map
    }

    fn add(&mut self, other: &Self) {
        self.nested += other.nested;
        self.literals += other.literals;
        self.checked += other.checked;
        self.exits += other.exits;
        self.ok_fail += other.ok_fail;
        self.derived_args += other.derived_args;
        self.qualified += other.qualified;
        self.table_rows += other.table_rows;
        self.generated_blocks += other.generated_blocks;
    }
}

/// A decision an AF1-X frame leaves to its author.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Obligation {
    /// The refusal symbol.
    pub symbol: AgentErrorCode,
    /// Authored JSON pointer.
    pub at: String,
    /// The type expected there, when known.
    pub expected: Option<String>,
    /// Values of the expected type visible there (`name: type`).
    pub available: Option<Vec<String>>,
    /// What the author must decide.
    pub decision: String,
}

impl Obligation {
    pub(crate) fn new(symbol: AgentErrorCode, at: &str, decision: impl Into<String>) -> Self {
        Self {
            symbol,
            at: at.to_owned(),
            expected: None,
            available: None,
            decision: decision.into(),
        }
    }

    /// The problem line under a refusal whose code is `headline`: prefixed
    /// with its own symbol when that differs.
    #[must_use]
    pub fn line(&self, headline: AgentErrorCode) -> String {
        let text = format!("{}: {}", self.at, self.decision);
        if self.symbol == headline {
            text
        } else {
            format!("[{}] {text}", self.symbol.symbol())
        }
    }

    /// JSON form.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({"symbol": self.symbol.symbol(), "at": self.at, "expected": self.expected,
               "available": self.available, "decision": self.decision})
    }
}

/// One refusal for a list of obligations: the first obligation's symbol,
/// one problem line each.
#[must_use]
pub fn refusal(obligations: &[Obligation]) -> AgentError {
    let code = obligations
        .first()
        .map_or(AgentErrorCode::FrameInvalid, |first| first.symbol);
    let mut lines: Vec<String> = Vec::new();
    for obligation in obligations {
        let line = obligation.line(code);
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    join_lines(code, lines)
}

fn join_lines(code: AgentErrorCode, mut lines: Vec<String>) -> AgentError {
    match lines.len() {
        0 => AgentError::new(code, "the frame is invalid"),
        1 => AgentError::new(code, lines.remove(0)),
        count => {
            let first = lines.remove(0);
            AgentError::new(
                code,
                format!("{first} (1 of {count} problems)\n  {}", lines.join("\n  ")),
            )
        }
    }
}

/// Expands an AF1-X frame into plain AF1.
///
/// # Errors
///
/// `AGENT_FRAME_INVALID` when the frame is not an AF1-X object frame;
/// `AGENT_RIPPLE_INTENT_UNKNOWN` for `ripple` (not enabled in this build).
/// Every other open decision is returned as an [`Obligation`].
pub fn expand(program: &Program, names: &Names, frame_value: &Value) -> Result<Expansion> {
    let object = frame_value
        .as_object()
        .ok_or_else(|| frame("", "an AF1 frame is a JSON object"))?;
    if object.get("af1").and_then(Value::as_u64) != Some(1) || !object["af1"].is_number() {
        return Err(frame("/af1", "declare \"af1\": 1"));
    }
    if object.get("afx").and_then(Value::as_u64) != Some(1) || !object["afx"].is_number() {
        return Err(frame(
            "/afx",
            "declare \"afx\": 1 for the authoring dialect, or remove the key",
        ));
    }
    if object.contains_key("ripple") {
        return Err(AgentError::new(
            AgentErrorCode::RippleIntentUnknown,
            "/ripple: ripple is not enabled in this build; state the change as a frame",
        ));
    }
    let context = Context::new(program, names, object);
    let mut out = object.clone();
    out.remove("afx");
    out.remove("test_tables");
    let mut expander = Expander {
        cx: &context,
        map: SourceMap::default(),
        stats: AfxStats::default(),
        obligations: Vec::new(),
    };
    for key in ["fns", "functions"] {
        if let Some(Value::Array(list)) = object.get(key) {
            let functions: Vec<Value> = list
                .iter()
                .enumerate()
                .map(|(index, decl)| expander.function(decl, &format!("/{key}/{index}"), false))
                .collect();
            out.insert(key.to_owned(), Value::Array(functions));
        }
    }
    if let Some(Value::Array(list)) = object.get("patch") {
        let patches: Vec<Value> = list
            .iter()
            .enumerate()
            .map(|(index, decl)| expander.function(decl, &format!("/patch/{index}"), true))
            .collect();
        out.insert("patch".to_owned(), Value::Array(patches));
    }
    if let Some(Value::Array(edits)) = object.get("edit") {
        for (index, edit) in edits.iter().enumerate() {
            if let Some(with) = edit.get("with")
                && is_extended_op(with)
            {
                expander.obligations.push(Obligation::new(
                    AgentErrorCode::FrameInvalid,
                    &format!("/edit/{index}/with"),
                    "an edit replaces one operation with plain AF1; nested operations, literals, checked operations and exits need the block restated: use patch to restate the block",
                ));
            }
        }
    }
    let rows = crate::tables::lower(
        object,
        &mut out,
        &mut expander.map,
        &mut expander.obligations,
    );
    expander.stats.table_rows = rows;
    Ok(Expansion {
        frame: Value::Object(out),
        map: expander.map,
        stats: expander.stats,
        obligations: expander.obligations,
    })
}

/// Whether an `edit.with` operation uses an AF1-X form.
fn is_extended_op(with: &Value) -> bool {
    let (word, args): (Option<&str>, Vec<&Value>) = match with {
        Value::Array(items) => (
            items.first().and_then(Value::as_str),
            items.iter().skip(1).collect(),
        ),
        Value::Object(object) => (
            object
                .get("op")
                .or_else(|| object.get("opcode"))
                .and_then(Value::as_str),
            object
                .get("args")
                .or_else(|| object.get("operands"))
                .and_then(Value::as_array)
                .map(|args| args.iter().collect())
                .unwrap_or_default(),
        ),
        _ => return false,
    };
    let Some(word) = word else { return false };
    if word.contains('?') || word.starts_with('!') {
        return true;
    }
    let skip = opcodes::by_word(word)
        .is_some_and(|row| row.immediate != ImmediateKind::None)
        .into();
    args.iter().skip(skip).any(|arg| !arg.is_string())
}

// ---------------------------------------------------------------------------
// Typing context
// ---------------------------------------------------------------------------

/// A type the frame declares: its identity inside the expander (the live
/// identity when it restates a live type, a placeholder otherwise) and its
/// members.
struct FrameType {
    id: EntityId,
    variant: bool,
    members: Vec<(String, Option<TypeExpr>)>,
}

type Signature = (Vec<Option<TypeExpr>>, Option<TypeExpr>);

/// The members of a type definition with their payload or field types.
type Members = Vec<(String, Option<TypeExpr>)>;

/// The read-only typing context: the program, its names, and the frame's
/// own declarations.
struct Context<'a> {
    program: &'a Program,
    names: &'a Names,
    types: BTreeMap<String, FrameType>,
    type_names: BTreeMap<EntityId, String>,
    consts: BTreeMap<String, Option<TypeExpr>>,
    signatures: BTreeMap<String, Signature>,
    top: BTreeSet<String>,
}

fn placeholder(index: usize) -> EntityId {
    let mut bytes = [0xa5_u8; 32];
    bytes[..8].copy_from_slice(b"afx-type");
    bytes[24..].copy_from_slice(&(index as u64).to_be_bytes());
    EntityId::from_bytes(bytes)
}

fn list<'v>(frame: &'v Map<String, Value>, key: &str) -> &'v [Value] {
    frame
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

impl<'a> Context<'a> {
    #[allow(clippy::too_many_lines)]
    fn new(program: &'a Program, names: &'a Names, frame: &Map<String, Value>) -> Self {
        let mut cx = Self {
            program,
            names,
            types: BTreeMap::new(),
            type_names: BTreeMap::new(),
            consts: BTreeMap::new(),
            signatures: BTreeMap::new(),
            top: BTreeSet::new(),
        };
        for (index, decl) in list(frame, "types").iter().enumerate() {
            let Some(name) = decl.get("name").and_then(Value::as_str) else {
                continue;
            };
            cx.top.insert(name.to_owned());
            let id = names
                .resolve(name)
                .filter(|id| {
                    names.scope(id) == Scope::Top
                        && matches!(program.body(id), Some(EntityBodyValue::TypeDef(_)))
                })
                .unwrap_or_else(|| placeholder(index));
            cx.types.insert(
                name.to_owned(),
                FrameType {
                    id,
                    variant: decl.get("variant").is_some(),
                    members: Vec::new(),
                },
            );
            cx.type_names.insert(id, name.to_owned());
        }
        let mut members = Vec::new();
        for decl in list(frame, "types") {
            let Some(name) = decl.get("name").and_then(Value::as_str) else {
                continue;
            };
            let cases = decl
                .get("variant")
                .or_else(|| decl.get("record"))
                .and_then(Value::as_array);
            let read: Vec<(String, Option<TypeExpr>)> = cases
                .map(|cases| {
                    cases
                        .iter()
                        .filter_map(|case| match case {
                            Value::String(leaf) => Some((leaf.clone(), None)),
                            Value::Array(pair) if pair.len() == 2 => Some((
                                pair[0].as_str()?.to_owned(),
                                if pair[1].is_null() {
                                    None
                                } else {
                                    types::read(&pair[1], &cx, "").ok()
                                },
                            )),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            members.push((name.to_owned(), read));
        }
        for (name, read) in members {
            if let Some(declared) = cx.types.get_mut(&name) {
                declared.members = read;
            }
        }
        for decl in list(frame, "consts") {
            let Some(name) = decl.get("name").and_then(Value::as_str) else {
                continue;
            };
            let ty = types::read(decl.get("type").unwrap_or(&Value::from("i64")), &cx, "").ok();
            cx.top.insert(name.to_owned());
            cx.consts.insert(name.to_owned(), ty);
        }
        for key in ["fns", "functions"] {
            for decl in list(frame, key) {
                let Some(name) = decl
                    .get("fn")
                    .or_else(|| decl.get("name"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                let params = read_params(decl.get("params"), &cx)
                    .map(|params| params.into_iter().map(|(_, ty, _)| ty).collect())
                    .unwrap_or_default();
                let result = decl
                    .get("returns")
                    .and_then(|ty| types::read(ty, &cx, "").ok());
                cx.top.insert(name.to_owned());
                cx.signatures.insert(name.to_owned(), (params, result));
            }
        }
        for decl in list(frame, "patch") {
            let Some(name) = decl
                .get("fn")
                .or_else(|| decl.get("name"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            let Some((live_params, live_result)) = cx.live_signature(name) else {
                continue;
            };
            let params = match decl.get("params") {
                Some(value) => read_params(Some(value), &cx)
                    .map(|params| params.into_iter().map(|(_, ty, _)| ty).collect())
                    .unwrap_or_default(),
                None => live_params,
            };
            let result = match decl.get("returns") {
                Some(ty) => types::read(ty, &cx, "").ok(),
                None => live_result,
            };
            cx.signatures.insert(name.to_owned(), (params, result));
        }
        for decl in list(frame, "tests") {
            if let Some(name) = decl.get("name").and_then(Value::as_str) {
                cx.top.insert(name.to_owned());
            }
        }
        cx
    }

    fn live_function(
        &self,
        name: &str,
    ) -> Option<(EntityId, &'a sley_mutate::value::FunctionBody)> {
        let id = self.names.resolve(name)?;
        if self.names.scope(&id) != Scope::Top {
            return None;
        }
        match self.program.body(&id) {
            Some(EntityBodyValue::Function(body)) => Some((id, body)),
            _ => None,
        }
    }

    fn live_signature(&self, name: &str) -> Option<Signature> {
        let (_, body) = self.live_function(name)?;
        Some((
            body.parameters
                .iter()
                .map(|param| self.parameter_type(param))
                .collect(),
            Some(body.result_type.clone()),
        ))
    }

    fn parameter_type(&self, param: &EntityId) -> Option<TypeExpr> {
        match self.program.body(param) {
            Some(EntityBodyValue::Parameter(p)) => Some(p.value_type.clone()),
            _ => None,
        }
    }

    fn signature(&self, name: &str) -> Option<Signature> {
        if let Some(signature) = self.signatures.get(name) {
            return Some(signature.clone());
        }
        if self.top.contains(name) {
            return None;
        }
        self.live_signature(name)
    }

    fn const_type(&self, name: &str) -> Option<TypeExpr> {
        if let Some(ty) = self.consts.get(name) {
            return ty.clone();
        }
        let id = self.names.resolve(name)?;
        match self.program.body(&id) {
            Some(EntityBodyValue::Constant(constant)) => Some(constant.value.value_type.clone()),
            _ => None,
        }
    }

    fn global_type(&self, name: &str) -> Option<TypeExpr> {
        let id = self.names.resolve(name)?;
        match self.program.body(&id) {
            Some(EntityBodyValue::GlobalValue(global)) => Some(global.value_type.clone()),
            _ => None,
        }
    }

    fn render(&self, ty: &TypeExpr) -> String {
        types::render_with(ty, &|id| {
            self.type_names
                .get(id)
                .cloned()
                .unwrap_or_else(|| self.names.name(id))
        })
    }

    /// Whether a definition is a variant, and its members with payload or
    /// field types.
    fn members(&self, definition: &EntityId) -> Option<(bool, Members)> {
        if let Some(name) = self.type_names.get(definition) {
            let declared = &self.types[name];
            return Some((declared.variant, declared.members.clone()));
        }
        match self.program.body(definition) {
            Some(EntityBodyValue::TypeDef(typedef)) => Some(match &typedef.form {
                TypeDefForm::Variant(cases) => (
                    true,
                    cases
                        .iter()
                        .map(|case| {
                            (
                                self.names.member_leaf(definition, &case.member_id),
                                case.payload_type.clone(),
                            )
                        })
                        .collect(),
                ),
                TypeDefForm::Record(fields) => (
                    false,
                    fields
                        .iter()
                        .map(|field| {
                            (
                                self.names.member_leaf(definition, &field.member_id),
                                Some(field.value_type.clone()),
                            )
                        })
                        .collect(),
                ),
            }),
            _ => None,
        }
    }

    /// The payload (variant) or field (record) type of `Type.member`:
    /// `None` when there is no such member, `Some(None)` for a unit case.
    #[allow(clippy::option_option)]
    fn member_type(&self, definition: &EntityId, leaf: &str) -> Option<Option<TypeExpr>> {
        self.members(definition)?
            .1
            .into_iter()
            .find(|(name, _)| name == leaf)
            .map(|(_, ty)| ty)
    }
}

impl TypeNames for Context<'_> {
    fn type_definition(&self, name: &str) -> Option<EntityId> {
        if let Some(declared) = self.types.get(name) {
            return Some(declared.id);
        }
        if self.top.contains(name) {
            return None;
        }
        let id = self.names.resolve(name)?;
        matches!(self.program.body(&id), Some(EntityBodyValue::TypeDef(_))).then_some(id)
    }
}

/// `[["name", type], ...]` with the types read; `None` when malformed.
fn read_params(
    value: Option<&Value>,
    cx: &Context<'_>,
) -> Option<Vec<(String, Option<TypeExpr>, Value)>> {
    let Some(value) = value else {
        return Some(Vec::new());
    };
    let mut out = Vec::new();
    for param in value.as_array()? {
        let pair = param.as_array().filter(|pair| pair.len() == 2)?;
        let name = pair[0].as_str()?;
        out.push((
            name.to_owned(),
            types::read(&pair[1], cx, "").ok(),
            pair[1].clone(),
        ));
    }
    Some(out)
}

fn named(definition: EntityId) -> TypeExpr {
    TypeExpr::Named(NamedType {
        definition,
        arguments: Vec::new(),
    })
}

fn uint(bits: u16) -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(bits))
}

// ---------------------------------------------------------------------------
// The authored form, parsed
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Arg {
    operand: Operand,
    pointer: String,
}

#[derive(Clone, Debug)]
enum Operand {
    Name(String),
    Literal { value: Value, typed: Option<Value> },
    Nested(Box<Node>),
    Raw(Value),
}

/// An operation: authored (named) or nested.
#[derive(Clone, Debug)]
struct Node {
    name: Option<String>,
    row: &'static OpcodeRow,
    word: String,
    check: Option<String>,
    immediate: Option<Value>,
    args: Vec<Arg>,
    ty: Option<Value>,
    pointer: String,
}

#[derive(Clone, Debug)]
struct Exit {
    target: String,
    cond: Arg,
    payload: Option<Arg>,
    pointer: String,
}

#[derive(Clone, Debug)]
enum Stmt {
    Op(Node),
    Exit(Exit),
    Raw {
        value: Value,
        pointer: String,
        name: Option<String>,
    },
}

/// How an edge target is written.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Shape {
    /// `"b"`.
    Name,
    /// `["b", args...]`.
    Bracket,
    /// The arguments follow the block name in the enclosing list.
    Flat,
}

#[derive(Clone, Debug)]
struct Target {
    block: String,
    args: Vec<Arg>,
    shape: Shape,
    /// Where the edge is written (the target, or the list holding a flat
    /// target and its arguments).
    pointer: String,
}

#[derive(Clone, Debug)]
enum Term {
    Return(Arg),
    Br {
        word: String,
        target: Target,
    },
    Cond {
        cond: Arg,
        then: Target,
        other: Target,
    },
    Switch {
        value: Arg,
        cases: Vec<(Value, Target)>,
    },
    Trap {
        head: Vec<Value>,
        payload: Option<Arg>,
    },
    Ok(Arg),
    Fail {
        case: Option<String>,
        payload: Option<Arg>,
    },
    Raw(Value),
}

#[derive(Clone, Debug)]
struct ABlock {
    name: String,
    pointer: String,
    object: Map<String, Value>,
    params: Vec<(String, Option<TypeExpr>, Value)>,
    stmts: Vec<Stmt>,
    term: Term,
}

fn split_word(word: &str) -> (&str, Option<&str>) {
    match word.split_once('?') {
        Some((word, check)) => (word, Some(check)),
        None => (word, None),
    }
}

fn is_opcode_word(word: &str) -> bool {
    opcodes::by_word(split_word(word).0).is_some() && !word.is_empty()
}

struct Parser<'o> {
    obligations: &'o mut Vec<Obligation>,
}

impl Parser<'_> {
    fn oblige(&mut self, symbol: AgentErrorCode, at: &str, decision: impl Into<String>) {
        self.obligations.push(Obligation::new(symbol, at, decision));
    }

    fn operand(&mut self, value: &Value, pointer: &str, depth: usize, edge: bool) -> Operand {
        match value {
            Value::String(text) => Operand::Name(text.clone()),
            Value::Number(_) | Value::Bool(_) => Operand::Literal {
                value: value.clone(),
                typed: None,
            },
            Value::Object(object)
                if object.contains_key("value")
                    && object.keys().all(|key| key == "type" || key == "value") =>
            {
                Operand::Literal {
                    value: object["value"].clone(),
                    typed: object.get("type").cloned(),
                }
            }
            Value::Array(items)
                if items
                    .first()
                    .and_then(Value::as_str)
                    .is_some_and(is_opcode_word) =>
            {
                if depth + 1 > MAX_EXPR_DEPTH {
                    self.oblige(
                        AgentErrorCode::XLimit,
                        pointer,
                        format!(
                            "operations nest more than {MAX_EXPR_DEPTH} levels deep: name an inner operation in the block's ops and use its name"
                        ),
                    );
                    return Operand::Raw(value.clone());
                }
                let word = items[0].as_str().unwrap_or_default();
                match self.node(
                    None,
                    word,
                    &items[1..],
                    None,
                    pointer,
                    &|index| format!("{pointer}/{}", index + 1),
                    depth + 1,
                ) {
                    Some(node) => Operand::Nested(Box::new(node)),
                    None => Operand::Raw(value.clone()),
                }
            }
            Value::Array(_) if !edge => {
                self.oblige(
                    AgentErrorCode::FrameInvalid,
                    pointer,
                    "a nested operation starts with its opcode, e.g. [\"add\", \"a\", 1]; any other array is not an operand",
                );
                Operand::Raw(value.clone())
            }
            _ => Operand::Raw(value.clone()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn node(
        &mut self,
        name: Option<&str>,
        word: &str,
        args: &[Value],
        ty: Option<Value>,
        pointer: &str,
        arg_pointer: &dyn Fn(usize) -> String,
        depth: usize,
    ) -> Option<Node> {
        let (base, check) = split_word(word);
        let row = opcodes::by_word(base)?;
        if let Some(check) = check
            && !check.is_empty()
            && !is_identifier(check)
        {
            self.oblige(
                AgentErrorCode::FrameInvalid,
                pointer,
                format!("`{word}`: after `?` comes a case or block name, or nothing"),
            );
            return None;
        }
        let immediate = (row.immediate != ImmediateKind::None)
            .then(|| args.first().cloned())
            .flatten();
        let skip = usize::from(immediate.is_some());
        let args = args
            .iter()
            .enumerate()
            .skip(skip)
            .map(|(index, value)| {
                let pointer = arg_pointer(index);
                Arg {
                    operand: self.operand(value, &pointer, depth, false),
                    pointer,
                }
            })
            .collect();
        Some(Node {
            name: name.map(str::to_owned),
            row,
            word: base.to_owned(),
            check: check.map(str::to_owned),
            immediate,
            args,
            ty,
            pointer: pointer.to_owned(),
        })
    }

    fn stmt(&mut self, value: &Value, pointer: &str) -> Stmt {
        let raw = |name: Option<&str>| Stmt::Raw {
            value: value.clone(),
            pointer: pointer.to_owned(),
            name: name.filter(|name| is_identifier(name)).map(str::to_owned),
        };
        match value {
            Value::Array(items)
                if items
                    .first()
                    .and_then(Value::as_str)
                    .is_some_and(|word| word.starts_with('!')) =>
            {
                self.exit(items, pointer).unwrap_or_else(|| raw(None))
            }
            Value::Array(items) if items.len() >= 2 => {
                let (Some(name), Some(word)) = (items[0].as_str(), items[1].as_str()) else {
                    return raw(None);
                };
                if !is_identifier(name) {
                    return raw(None);
                }
                self.node(
                    Some(name),
                    word,
                    &items[2..],
                    None,
                    pointer,
                    &|index| format!("{pointer}/{}", index + 2),
                    0,
                )
                .map_or_else(|| raw(Some(name)), Stmt::Op)
            }
            Value::Object(object) => {
                let name = object.get("name").and_then(Value::as_str);
                let word = object
                    .get("op")
                    .or_else(|| object.get("opcode"))
                    .and_then(Value::as_str);
                let (Some(name), Some(word)) = (name, word) else {
                    return raw(name);
                };
                if !is_identifier(name) {
                    return raw(None);
                }
                let key = if object.contains_key("args") {
                    "args"
                } else {
                    "operands"
                };
                let args = match object.get(key) {
                    Some(Value::Array(args)) => args.clone(),
                    None => Vec::new(),
                    Some(_) => return raw(Some(name)),
                };
                self.node(
                    Some(name),
                    word,
                    &args,
                    object.get("type").cloned(),
                    pointer,
                    &|index| format!("{pointer}/{key}/{index}"),
                    0,
                )
                .map_or_else(|| raw(Some(name)), Stmt::Op)
            }
            _ => raw(None),
        }
    }

    fn exit(&mut self, items: &[Value], pointer: &str) -> Option<Stmt> {
        let target = items[0].as_str().unwrap_or_default()[1..].to_owned();
        if items.get(1).and_then(Value::as_str) != Some("if") || !(3..=4).contains(&items.len()) {
            self.oblige(
                AgentErrorCode::FrameInvalid,
                pointer,
                "an exit is [\"!Case\", \"if\", condition] or [\"!Case\", \"if\", condition, payload]",
            );
            return None;
        }
        if !target.is_empty() && !is_identifier(&target) {
            self.oblige(
                AgentErrorCode::FrameInvalid,
                pointer,
                format!("`!{target}`: an exit names a case or a handler block"),
            );
            return None;
        }
        let cond_pointer = format!("{pointer}/2");
        let cond = Arg {
            operand: self.operand(&items[2], &cond_pointer, 0, false),
            pointer: cond_pointer,
        };
        let payload = items.get(3).map(|value| {
            let pointer = format!("{pointer}/3");
            Arg {
                operand: self.operand(value, &pointer, 0, true),
                pointer,
            }
        });
        Some(Stmt::Exit(Exit {
            target,
            cond,
            payload,
            pointer: pointer.to_owned(),
        }))
    }

    fn arg(&mut self, value: &Value, pointer: String) -> Arg {
        Arg {
            operand: self.operand(value, &pointer, 0, true),
            pointer,
        }
    }

    /// A `cond`/`br` target: `"b"` or `["b", args...]`.
    fn target(&mut self, value: &Value, pointer: &str) -> Option<Target> {
        match value {
            Value::String(block) => Some(Target {
                block: block.clone(),
                args: Vec::new(),
                shape: Shape::Name,
                pointer: pointer.to_owned(),
            }),
            Value::Array(items) if !items.is_empty() => Some(Target {
                block: items[0].as_str()?.to_owned(),
                args: items[1..]
                    .iter()
                    .enumerate()
                    .map(|(index, arg)| self.arg(arg, format!("{pointer}/{}", index + 1)))
                    .collect(),
                shape: Shape::Bracket,
                pointer: pointer.to_owned(),
            }),
            _ => None,
        }
    }

    fn term(&mut self, value: &Value, pointer: &str) -> Term {
        let raw = || Term::Raw(value.clone());
        let Some(items) = value.as_array() else {
            return raw();
        };
        let Some(word) = items.first().and_then(Value::as_str) else {
            return raw();
        };
        let at = |index: usize| format!("{pointer}/{index}");
        match word {
            "return" if items.len() == 2 => Term::Return(self.arg(&items[1], at(1))),
            "ok" if items.len() == 2 => Term::Ok(self.arg(&items[1], at(1))),
            "fail" if items.len() <= 3 => {
                let case = match items.get(1) {
                    None => None,
                    Some(Value::String(case)) if is_identifier(case) => Some(case.clone()),
                    Some(_) => return raw(),
                };
                let payload = items.get(2).map(|value| self.arg(value, at(2)));
                Term::Fail { case, payload }
            }
            "br" | "jump" if items.len() >= 2 => {
                let target = if items.len() == 2 && items[1].is_array() {
                    match self.target(&items[1], &at(1)) {
                        Some(target) => target,
                        None => return raw(),
                    }
                } else {
                    let Some(block) = items[1].as_str() else {
                        return raw();
                    };
                    Target {
                        block: block.to_owned(),
                        args: items[2..]
                            .iter()
                            .enumerate()
                            .map(|(index, arg)| self.arg(arg, at(index + 2)))
                            .collect(),
                        shape: Shape::Flat,
                        pointer: pointer.to_owned(),
                    }
                };
                Term::Br {
                    word: word.to_owned(),
                    target,
                }
            }
            "cond" if items.len() == 4 => {
                let cond = self.arg(&items[1], at(1));
                match (
                    self.target(&items[2], &at(2)),
                    self.target(&items[3], &at(3)),
                ) {
                    (Some(then), Some(other)) => Term::Cond { cond, then, other },
                    _ => raw(),
                }
            }
            "switch" if items.len() >= 3 => {
                let value = self.arg(&items[1], at(1));
                let mut cases = Vec::new();
                for (index, case) in items[2..].iter().enumerate() {
                    let case_pointer = at(index + 2);
                    let Some(case) = case.as_array().filter(|case| case.len() >= 2) else {
                        return raw();
                    };
                    let target = match &case[1] {
                        Value::Array(_) if case.len() == 2 => {
                            match self.target(&case[1], &format!("{case_pointer}/1")) {
                                Some(target) => target,
                                None => return raw(),
                            }
                        }
                        Value::String(block) => Target {
                            block: block.clone(),
                            args: case[2..]
                                .iter()
                                .enumerate()
                                .map(|(position, arg)| {
                                    self.arg(arg, format!("{case_pointer}/{}", position + 2))
                                })
                                .collect(),
                            shape: Shape::Flat,
                            pointer: case_pointer.clone(),
                        },
                        _ => return raw(),
                    };
                    cases.push((case[0].clone(), target));
                }
                Term::Switch { value, cases }
            }
            "trap" if items.len() <= 3 => Term::Trap {
                head: items[..items.len().min(2)].to_vec(),
                payload: items.get(2).map(|value| self.arg(value, at(2))),
            },
            _ => raw(),
        }
    }

    /// One authored block object; `None` when it is not a block this
    /// expander understands (the compiler then reports it as authored).
    fn block(
        &mut self,
        name: &str,
        value: &Value,
        pointer: &str,
        cx: &Context<'_>,
    ) -> Option<ABlock> {
        let object = value.as_object()?;
        if !is_identifier(name)
            || object.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "name" | "params" | "ops" | "term" | "unreachable" | "comment"
                )
            })
        {
            return None;
        }
        let params = read_params(object.get("params"), cx)?;
        if params.iter().any(|(name, _, _)| !is_identifier(name)) {
            return None;
        }
        let ops = match object.get("ops") {
            Some(Value::Array(ops)) => ops.clone(),
            None => Vec::new(),
            Some(_) => return None,
        };
        let term_value = object.get("term")?;
        let stmts = ops
            .iter()
            .enumerate()
            .map(|(index, op)| self.stmt(op, &format!("{pointer}/ops/{index}")))
            .collect();
        let term = self.term(term_value, &format!("{pointer}/term"));
        Some(ABlock {
            name: name.to_owned(),
            pointer: pointer.to_owned(),
            object: object.clone(),
            params,
            stmts,
            term,
        })
    }
}

// ---------------------------------------------------------------------------
// Expansion of functions
// ---------------------------------------------------------------------------

struct Expander<'c, 'a> {
    cx: &'c Context<'a>,
    map: SourceMap,
    stats: AfxStats,
    obligations: Vec<Obligation>,
}

/// A live block a patch keeps.
struct Kept {
    leaf: String,
    params: Vec<(String, Option<TypeExpr>)>,
    ops: Vec<(String, Option<TypeExpr>)>,
    targets: Vec<String>,
}

impl Kept {
    fn value(&self, name: &str) -> Option<(Kind, Option<TypeExpr>)> {
        self.params
            .iter()
            .find(|(leaf, _)| leaf == name)
            .map(|(_, ty)| (Kind::Param, ty.clone()))
            .or_else(|| {
                self.ops
                    .iter()
                    .find(|(leaf, _)| leaf == name)
                    .map(|(_, ty)| (Kind::Op, ty.clone()))
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    /// A block parameter (authored or threaded).
    Param,
    /// An operation result.
    Op,
    /// The value a checked operation unwraps (a continuation parameter).
    Unwrapped,
}

/// An authored name of a block, for typing and use-before-definition.
#[derive(Clone, Debug)]
struct ADef {
    ty: Option<TypeExpr>,
}

/// A value defined while lowering a block: its piece, kind and type.
#[derive(Clone, Debug)]
struct LDef {
    piece: usize,
    kind: Kind,
    ty: Option<TypeExpr>,
    ty_json: Option<Value>,
}

/// A value reference, resolved when the pieces and their dominance are
/// known.
#[derive(Clone, Debug)]
enum Sym {
    /// A value of the same authored block (authored or generated).
    Local(String, String),
    /// Emitted as written.
    Plain(String),
    /// `b.x` naming another authored block.
    Qualified {
        block: usize,
        leaf: String,
        suffix: String,
        text: String,
        pointer: String,
    },
    /// A plain name to find in a dominating block (X4).
    Far(Box<Far>),
    /// Passed through unchanged.
    Raw(Value),
}

#[derive(Clone, Debug)]
struct Far {
    name: String,
    suffix: String,
    pointer: String,
    derive: Option<Derive>,
}

/// A derived trailing edge argument, decided once the expanded graph is
/// known (an edge back into a loop never takes one).
#[derive(Clone, Debug)]
struct Derive {
    ty: Option<TypeExpr>,
    target: String,
    available: Vec<String>,
    /// The value of that name the edge's own block (or the function's
    /// parameters) holds, with its type.
    local: Option<(Box<Sym>, Option<TypeExpr>)>,
}

/// A terminator template.
#[derive(Clone, Debug)]
enum Tpl {
    Lit(Value),
    Sym(Sym),
    List(Vec<Tpl>),
    /// The threaded parameters of a piece, spliced into the list.
    Threaded(usize),
}

fn lit(text: &str) -> Tpl {
    Tpl::Lit(Value::from(text))
}

#[derive(Clone, Debug)]
struct EOp {
    name: String,
    word: String,
    immediate: Option<Value>,
    operands: Vec<Sym>,
    ty: Option<Value>,
    authored: String,
    role: Role,
    raw: Option<Value>,
}

#[derive(Clone, Debug)]
struct Piece {
    name: String,
    block: usize,
    local: usize,
    authored: String,
    role: Role,
    unwrapped: Option<(String, Value)>,
    threaded: Vec<(String, Value)>,
    ops: Vec<EOp>,
    term: Tpl,
    term_authored: String,
    term_role: Role,
    uses: Vec<String>,
    succ: Vec<String>,
}

#[derive(Clone, Debug)]
enum ExitKind {
    Fail {
        case: String,
        variant: String,
        payload: Option<Value>,
    },
    Err(Value),
    None,
}

#[derive(Clone, Debug)]
struct SharedExit {
    key: String,
    name: String,
    kind: ExitKind,
    authored: String,
}

enum Route {
    Handler(String),
    Case(String, Option<TypeExpr>),
}

/// Lowering state of one authored block.
struct Lower {
    b: usize,
    piece: usize,
    defs: BTreeMap<String, LDef>,
    exits: usize,
    term_k: usize,
    /// A root cause (an unknown name, a surplus edge argument) is already
    /// reported: untyped literals it leaves without context add nothing.
    quiet: bool,
}

/// What a switch case passes on: nothing, or a payload (of a known type).
enum CasePayload {
    Unit,
    Carries(Option<TypeExpr>),
}

struct NodeTypes {
    raw: Option<TypeExpr>,
    value: Option<TypeExpr>,
    contexts: Vec<Option<TypeExpr>>,
}

/// The dominator tree of an expanded function, with what each node
/// defines.
#[derive(Default)]
struct Cfg {
    index: BTreeMap<String, usize>,
    idom: Vec<Option<usize>>,
    /// The authored block (or kept live block) each node belongs to;
    /// `None` for shared exits, which X4 never reads from.
    owner: Vec<Option<String>>,
    /// The values each node defines, by name.
    values: Vec<BTreeMap<String, (Kind, Option<TypeExpr>)>>,
    /// Every block defining a name (authored and kept), in block order.
    definers: BTreeMap<String, Vec<String>>,
    /// The name of each node.
    node_names: Vec<String>,
}

/// What a plain name means at a point, by the nearest block above it on
/// the dominator tree that defines the name.
enum Nearest {
    /// An operation result of `owner`, held by the node named `holder`.
    Op {
        owner: String,
        holder: String,
        ty: Option<TypeExpr>,
    },
    /// A parameter (or unwrapped value) of `owner`, which a plain name
    /// cannot reach outside it; `outer` is an operation result further up
    /// that it shadows.
    Hidden {
        owner: String,
        kind: Kind,
        outer: Option<String>,
    },
    /// No dominating block defines it.
    None,
}

impl Cfg {
    /// The nearest definition of `name` above `node` on the dominator tree,
    /// skipping the nodes of `own` (the using block, resolved in order).
    /// Bounded by the tree depth.
    fn nearest(&self, node: usize, own: &str, name: &str) -> Nearest {
        let mut current = node;
        let mut hidden: Option<(String, Kind)> = None;
        for _ in 0..=self.idom.len() {
            if self.owner[current].as_deref() != Some(own)
                && let Some((kind, ty)) = self.values[current].get(name)
            {
                let owner = self.owner[current].clone().unwrap_or_default();
                match (&hidden, kind) {
                    (None, Kind::Op) => {
                        let holder = self.node_names[current].clone();
                        return Nearest::Op {
                            owner,
                            holder,
                            ty: ty.clone(),
                        };
                    }
                    (None, kind) => hidden = Some((owner, *kind)),
                    (Some((hider, kind)), Kind::Op) => {
                        return Nearest::Hidden {
                            owner: hider.clone(),
                            kind: *kind,
                            outer: Some(owner),
                        };
                    }
                    (Some(_), _) => {}
                }
            }
            match self.idom.get(current).copied().flatten() {
                Some(up) if up != current => current = up,
                _ => break,
            }
        }
        match hidden {
            Some((owner, kind)) => Nearest::Hidden {
                owner,
                kind,
                outer: None,
            },
            None => Nearest::None,
        }
    }

    fn reachable(&self, node: usize) -> bool {
        self.idom.get(node).is_some_and(Option::is_some)
    }

    /// Whether `a` dominates `b` (bounded by the tree depth).
    fn dominates(&self, a: usize, b: usize) -> bool {
        if !self.reachable(b) {
            return false;
        }
        let mut node = b;
        for _ in 0..=self.idom.len() {
            if node == a {
                return true;
            }
            match self.idom[node] {
                Some(up) if up != node => node = up,
                _ => return false,
            }
        }
        false
    }
}

enum Slot {
    Parsed(usize),
    Raw(Value),
}

impl Expander<'_, '_> {
    /// Expands one `fns` (or `functions`) or `patch` entry.
    fn function(&mut self, decl: &Value, pointer: &str, patch: bool) -> Value {
        let Some(object) = decl.as_object() else {
            return decl.clone();
        };
        let Some(name) = object
            .get("fn")
            .or_else(|| object.get("name"))
            .and_then(Value::as_str)
        else {
            return decl.clone();
        };
        let expanded = if patch {
            self.patch_function(object, name, pointer)
        } else {
            self.full_function(object, name, pointer)
        };
        expanded.unwrap_or_else(|| decl.clone())
    }

    fn full_function(
        &mut self,
        object: &Map<String, Value>,
        name: &str,
        pointer: &str,
    ) -> Option<Value> {
        let blocks = object.get("blocks")?.as_array()?;
        let params = read_params(object.get("params"), self.cx)?;
        let result = object
            .get("returns")
            .and_then(|ty| types::read(ty, self.cx, "").ok());
        let mut fx = FnExp::new(self.cx, name, pointer, false, params, result);
        let mut slots = Vec::new();
        {
            let mut parser = Parser {
                obligations: &mut fx.obligations,
            };
            for (index, block) in blocks.iter().enumerate() {
                let block_pointer = format!("{pointer}/blocks/{index}");
                let block_name = block.get("name").and_then(Value::as_str).unwrap_or("");
                if let Some(parsed) = parser.block(block_name, block, &block_pointer, self.cx) {
                    slots.push(Slot::Parsed(fx.blocks.len()));
                    fx.blocks.push(parsed);
                } else {
                    fx.degraded = true;
                    slots.push(Slot::Raw(block.clone()));
                }
            }
        }
        fx.entry = object
            .get("entry")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                blocks
                    .first()
                    .and_then(|block| block.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_default();
        fx.run();
        let mut out_blocks = Vec::new();
        for slot in &slots {
            match slot {
                Slot::Parsed(b) => {
                    let first = fx.block_pieces[*b][0];
                    let at = format!("{pointer}/blocks/{}", out_blocks.len());
                    out_blocks.push(fx.emit_piece(first, &at));
                }
                Slot::Raw(value) => out_blocks.push(value.clone()),
            }
        }
        for b in 0..fx.blocks.len() {
            for &piece in fx.block_pieces[b].clone().iter().skip(1) {
                let at = format!("{pointer}/blocks/{}", out_blocks.len());
                out_blocks.push(fx.emit_piece(piece, &at));
            }
        }
        for index in 0..fx.exits.len() {
            let at = format!("{pointer}/blocks/{}", out_blocks.len());
            out_blocks.push(fx.emit_exit(index, &at));
        }
        fx.check_bounds();
        let mut out = object.clone();
        out.insert("blocks".to_owned(), Value::Array(out_blocks));
        self.absorb(fx);
        Some(Value::Object(out))
    }

    #[allow(clippy::too_many_lines)]
    fn patch_function(
        &mut self,
        object: &Map<String, Value>,
        name: &str,
        pointer: &str,
    ) -> Option<Value> {
        let Some(Value::Object(patched)) = object.get("blocks") else {
            return None;
        };
        let patched = patched.clone();
        let (_, function) = self.cx.live_function(name)?;
        let params = match object.get("params") {
            Some(value) => read_params(Some(value), self.cx)?,
            None => function
                .parameters
                .iter()
                .map(|param| {
                    (
                        self.cx.names.leaf(param),
                        self.cx.parameter_type(param),
                        Value::Null,
                    )
                })
                .collect(),
        };
        let result = match object.get("returns") {
            Some(ty) => types::read(ty, self.cx, "").ok(),
            None => Some(function.result_type.clone()),
        };
        let mut fx = FnExp::new(self.cx, name, pointer, true, params, result);
        // Generated blocks are recognized by their exact generated shape
        // (a continuation reached only from the previous piece, a shared
        // exit's body) as well as their names; a live block the expander did
        // not make is kept as it is, whatever its name.
        let live = LiveGraph::new(self.cx, &function.blocks);
        let mut pieces: BTreeSet<String> = BTreeSet::new();
        for key in patched.keys() {
            pieces.extend(live.pieces_of(key));
        }
        let mut live_order = Vec::new();
        for (leaf, block, body) in &live.blocks {
            let leaf = leaf.clone();
            live_order.push(leaf.clone());
            if patched.contains_key(&leaf) {
                continue;
            }
            if pieces.contains(&leaf) {
                fx.stale.push(leaf);
                continue;
            }
            if live.is_generated_exit(&leaf, body) {
                fx.live_exits.push((leaf, live_targets(self.cx, block)));
                continue;
            }
            fx.kept.push(Kept {
                leaf,
                params: body
                    .parameters
                    .iter()
                    .map(|param| (self.cx.names.leaf(param), self.cx.parameter_type(param)))
                    .collect(),
                ops: body
                    .operations
                    .iter()
                    .map(|op| {
                        let ty = match self.cx.program.body(op) {
                            Some(EntityBodyValue::Operation(op)) => {
                                op.result_types.first().cloned()
                            }
                            _ => None,
                        };
                        (self.cx.names.leaf(op), ty)
                    })
                    .collect(),
                targets: live_targets(self.cx, block),
            });
        }
        let mut raw_keys = BTreeSet::new();
        {
            let mut parser = Parser {
                obligations: &mut fx.obligations,
            };
            for (key, spec) in &patched {
                if spec.is_null() {
                    continue;
                }
                let block_pointer = format!("{pointer}/blocks/{key}");
                if let Some(parsed) = parser.block(key, spec, &block_pointer, self.cx) {
                    fx.blocks.push(parsed);
                } else {
                    fx.degraded = true;
                    raw_keys.insert(key.clone());
                }
            }
        }
        let live_entry = self.cx.names.leaf(&function.entry_block);
        let survives = |leaf: &str| {
            patched.get(leaf).is_none_or(|spec| !spec.is_null())
                && live_order.iter().any(|live| live == leaf)
        };
        fx.entry = object
            .get("entry")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| survives(&live_entry).then(|| live_entry.clone()))
            .or_else(|| {
                live_order
                    .iter()
                    .find(|leaf| survives(leaf) && !fx.stale.contains(leaf))
                    .cloned()
            })
            .or_else(|| patched.keys().find(|key| !patched[*key].is_null()).cloned())
            .unwrap_or_default();
        fx.run();
        let mut out_blocks = Map::new();
        for (key, spec) in &patched {
            if spec.is_null() || raw_keys.contains(key) {
                out_blocks.insert(key.clone(), spec.clone());
            }
        }
        for b in 0..fx.blocks.len() {
            for &piece in &fx.block_pieces[b].clone() {
                let piece_name = fx.pieces[piece].name.clone();
                let at = format!("{pointer}/blocks/{piece_name}");
                let emitted = fx.emit_piece(piece, &at);
                out_blocks.insert(piece_name, emitted);
            }
        }
        for index in 0..fx.exits.len() {
            let exit_name = fx.exits[index].name.clone();
            let at = format!("{pointer}/blocks/{exit_name}");
            let emitted = fx.emit_exit(index, &at);
            out_blocks.insert(exit_name, emitted);
        }
        // Restating never orphans a generated block: stale pieces of a
        // restated (or deleted) block and shared exits no longer used go.
        for leaf in &fx.stale {
            if !out_blocks.contains_key(leaf) {
                out_blocks.insert(leaf.clone(), Value::Null);
            }
        }
        let mut referenced: BTreeSet<String> = BTreeSet::new();
        for kept in &fx.kept {
            referenced.extend(kept.targets.iter().cloned());
        }
        for (leaf, _) in &fx.live_exits {
            if !out_blocks.contains_key(leaf) && !referenced.contains(leaf) {
                out_blocks.insert(leaf.clone(), Value::Null);
            }
        }
        fx.check_bounds();
        let mut out = object.clone();
        out.insert("blocks".to_owned(), Value::Object(out_blocks));
        self.absorb(fx);
        Some(Value::Object(out))
    }

    fn absorb(&mut self, fx: FnExp<'_, '_>) {
        self.obligations.extend(fx.obligations);
        self.map.entries.extend(fx.entries);
        if !fx.value_names.is_empty() || !fx.block_names.is_empty() {
            let function = fx.fn_name.clone();
            // The merged table (a block and a value may share a name, as a
            // continuation and an exit condition do): the block's entry wins.
            let merged = self.map.names.entry(function.clone()).or_default();
            merged.extend(fx.value_names.clone());
            merged.extend(fx.block_names.clone());
            self.map
                .blocks
                .entry(function.clone())
                .or_default()
                .extend(fx.block_names);
            self.map
                .values
                .entry(function)
                .or_default()
                .extend(fx.value_names);
        }
        self.stats.add(&fx.stats);
    }
}

/// A live function's blocks, for recognizing the ones an earlier expansion
/// generated.
struct LiveGraph<'c, 'a> {
    cx: &'c Context<'a>,
    blocks: Vec<(String, EntityId, &'a BlockBody)>,
    index: BTreeMap<String, usize>,
    preds: Vec<Vec<usize>>,
}

impl<'c, 'a> LiveGraph<'c, 'a> {
    fn new(cx: &'c Context<'a>, ids: &[EntityId]) -> Self {
        let mut blocks = Vec::new();
        for id in ids {
            if let Some(EntityBodyValue::Block(body)) = cx.program.body(id) {
                blocks.push((cx.names.leaf(id), *id, body));
            }
        }
        let index: BTreeMap<String, usize> = blocks
            .iter()
            .enumerate()
            .map(|(position, (leaf, _, _))| (leaf.clone(), position))
            .collect();
        let mut preds = vec![Vec::new(); blocks.len()];
        for (position, (_, id, _)) in blocks.iter().enumerate() {
            for target in live_targets(cx, id) {
                if let Some(&target) = index.get(&target)
                    && !preds[target].contains(&position)
                {
                    preds[target].push(position);
                }
            }
        }
        Self {
            cx,
            blocks,
            index,
            preds,
        }
    }

    fn first_param(&self, position: usize) -> Option<String> {
        self.blocks[position]
            .2
            .parameters
            .first()
            .map(|param| self.cx.names.leaf(param))
    }

    /// The generated continuation pieces of the live block `root`, in
    /// order: each is the continuation edge's target of the one before (a
    /// switch's Ok/Some case passing `$` first, or a cond's else target),
    /// has that one as its only predecessor, and has the name the
    /// expansion allocates for it.
    fn pieces_of(&self, root: &str) -> Vec<String> {
        let mut out = Vec::new();
        let Some(&start) = self.index.get(root) else {
            return out;
        };
        let mut current = start;
        for _ in 0..self.blocks.len() {
            let next = match &self.blocks[current].2.terminator {
                Terminator::VariantSwitch(switch) if switch.cases.len() == 2 => switch
                    .cases
                    .iter()
                    .find(|case| {
                        matches!(
                            case.case_key,
                            CaseKey::Builtin(BuiltinCase::Ok | BuiltinCase::Some)
                        ) && case.edge.arguments.first() == Some(&SwitchArgument::CasePayload)
                    })
                    .map(|case| self.cx.names.leaf(&case.edge.target))
                    .filter(|leaf| {
                        self.index
                            .get(leaf)
                            .and_then(|&position| self.first_param(position))
                            .is_some_and(|value| is_allocated(leaf, &format!("{root}__{value}")))
                    }),
                Terminator::CondBranch(cond) => Some(self.cx.names.leaf(&cond.if_false.target))
                    .filter(|leaf| is_exit_piece(leaf, root, self.blocks.len())),
                _ => None,
            };
            let Some((next, position)) =
                next.and_then(|leaf| self.index.get(&leaf).map(|&position| (leaf, position)))
            else {
                break;
            };
            if self.preds[position] != [current] || out.contains(&next) || position == start {
                break;
            }
            out.push(next);
            current = position;
        }
        out
    }

    /// Whether a live block is a shared exit an expansion generated: the
    /// exact body (`variant E.Case [p]; err; return`, `err e; return`,
    /// `none; return`) under the name allocated for it.
    fn is_generated_exit(&self, leaf: &str, body: &BlockBody) -> bool {
        let op = |position: usize| {
            let id = body.operations.get(position)?;
            match self.cx.program.body(id) {
                Some(EntityBodyValue::Operation(op)) => Some((*id, op)),
                _ => None,
            }
        };
        let result = |id: EntityId| {
            ValueRef::OperationResult(OperationResultRef {
                operation: id,
                result_index: 0,
            })
        };
        let returns = |id: EntityId| matches!(&body.terminator, Terminator::Return(ret) if ret.value == result(id));
        let params: Vec<ValueRef> = body
            .parameters
            .iter()
            .map(|param| ValueRef::Parameter(*param))
            .collect();
        match (params.len(), body.operations.len()) {
            (1, 1) => op(0).is_some_and(|(id, op)| {
                op.opcode == 131
                    && op.operands == params
                    && returns(id)
                    && is_allocated(leaf, "__err")
            }),
            (0, 1) => op(0).is_some_and(|(id, op)| {
                op.opcode == 129
                    && op.operands.is_empty()
                    && returns(id)
                    && is_allocated(leaf, "__none")
            }),
            (0 | 1, 2) => match (op(0), op(1)) {
                (Some((variant_id, variant)), Some((err_id, err))) => {
                    let Immediate::Variant(member) = &variant.immediate else {
                        return false;
                    };
                    let case = self
                        .cx
                        .names
                        .member_leaf(&member.definition, &member.member_id);
                    variant.opcode == 20
                        && variant.operands == params
                        && err.opcode == 131
                        && err.operands == [result(variant_id)]
                        && returns(err_id)
                        && is_allocated(leaf, &format!("__fail_{case}"))
                }
                _ => false,
            },
            _ => false,
        }
    }
}

/// A generated name fitted to the name grammar, as allocation fits it.
fn fit(name: String) -> String {
    if name.len() <= MAX_NAME {
        name
    } else {
        shorten(&name)
    }
}

/// Whether `leaf` is a name allocation gives `base`: the base itself or
/// with a collision suffix `_<n>`, shortened when too long.
fn is_allocated(leaf: &str, base: &str) -> bool {
    if leaf == fit(base.to_owned()) {
        return true;
    }
    let suffixed = leaf
        .strip_prefix(base)
        .and_then(|rest| rest.strip_prefix('_'))
        .is_some_and(all_digits);
    if suffixed {
        return fit(leaf.to_owned()) == leaf;
    }
    looks_shortened(leaf) && (2..=64).any(|n| fit(format!("{base}_{n}")) == leaf)
}

/// Whether `leaf` is the block after an exit of `root`: `<root>__if<i>`,
/// possibly suffixed or shortened.
fn is_exit_piece(leaf: &str, root: &str, limit: usize) -> bool {
    let plain = leaf
        .strip_prefix(root)
        .and_then(|rest| rest.strip_prefix("__if"))
        .is_some_and(|rest| match rest.split_once('_') {
            Some((index, suffix)) => all_digits(index) && all_digits(suffix),
            None => all_digits(rest),
        });
    (plain && fit(leaf.to_owned()) == leaf)
        || (looks_shortened(leaf)
            && (0..=limit).any(|i| is_allocated(leaf, &format!("{root}__if{i}"))))
}

fn all_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// Whether `leaf` has the form of a shortened generated name.
fn looks_shortened(leaf: &str) -> bool {
    leaf.len() > 19
        && leaf[leaf.len() - 19..].starts_with("__h")
        && leaf[leaf.len() - 16..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
}

/// The leaves a live block's terminator targets.
fn live_targets(cx: &Context<'_>, block: &EntityId) -> Vec<String> {
    let Some(EntityBodyValue::Block(body)) = cx.program.body(block) else {
        return Vec::new();
    };
    let leaf = |id: &EntityId| cx.names.leaf(id);
    match &body.terminator {
        Terminator::Branch(branch) => vec![leaf(&branch.edge.target)],
        Terminator::CondBranch(cond) => {
            vec![leaf(&cond.if_true.target), leaf(&cond.if_false.target)]
        }
        Terminator::VariantSwitch(switch) => switch
            .cases
            .iter()
            .map(|case| leaf(&case.edge.target))
            .collect(),
        Terminator::Return(_) | Terminator::Trap(_) => Vec::new(),
    }
}

/// The expansion of one function.
struct FnExp<'c, 'a> {
    cx: &'c Context<'a>,
    fn_name: String,
    fn_pointer: String,
    patch: bool,
    params: Vec<(String, Option<TypeExpr>)>,
    result: Option<TypeExpr>,
    entry: String,
    blocks: Vec<ABlock>,
    kept: Vec<Kept>,
    stale: Vec<String>,
    live_exits: Vec<(String, Vec<String>)>,
    defs: Vec<BTreeMap<String, ADef>>,
    block_defs: Vec<BTreeMap<String, LDef>>,
    pieces: Vec<Piece>,
    block_pieces: Vec<Vec<usize>>,
    exits: Vec<SharedExit>,
    value_taken: BTreeSet<String>,
    block_taken: BTreeSet<String>,
    cfg: Option<Cfg>,
    /// A block or terminator the expander does not understand is passed to
    /// the compiler as written; the control-flow graph is then incomplete,
    /// so X4 defers to the compiler's own diagnostics instead of guessing.
    degraded: bool,
    obligations: Vec<Obligation>,
    entries: Vec<MapEntry>,
    /// Generated block names to their authored pointers.
    block_names: BTreeMap<String, String>,
    /// Generated value names to their authored pointers.
    value_names: BTreeMap<String, String>,
    stats: AfxStats,
}

impl<'c, 'a> FnExp<'c, 'a> {
    fn new(
        cx: &'c Context<'a>,
        name: &str,
        pointer: &str,
        patch: bool,
        params: Vec<(String, Option<TypeExpr>, Value)>,
        result: Option<TypeExpr>,
    ) -> Self {
        Self {
            cx,
            fn_name: name.to_owned(),
            fn_pointer: pointer.to_owned(),
            patch,
            params: params.into_iter().map(|(name, ty, _)| (name, ty)).collect(),
            result,
            entry: String::new(),
            blocks: Vec::new(),
            kept: Vec::new(),
            stale: Vec::new(),
            live_exits: Vec::new(),
            defs: Vec::new(),
            block_defs: Vec::new(),
            pieces: Vec::new(),
            block_pieces: Vec::new(),
            exits: Vec::new(),
            value_taken: BTreeSet::new(),
            block_taken: BTreeSet::new(),
            cfg: None,
            degraded: false,
            obligations: Vec::new(),
            entries: Vec::new(),
            block_names: BTreeMap::new(),
            value_names: BTreeMap::new(),
            stats: AfxStats::default(),
        }
    }

    fn oblige(&mut self, symbol: AgentErrorCode, at: &str, decision: impl Into<String>) {
        self.obligations.push(Obligation::new(symbol, at, decision));
    }

    fn run(&mut self) {
        self.declare();
        self.infer_types();
        self.block_defs = vec![BTreeMap::new(); self.blocks.len()];
        self.block_pieces = vec![Vec::new(); self.blocks.len()];
        for b in 0..self.blocks.len() {
            self.lower_block(b);
        }
        self.cfg = Some(self.dominators());
        self.thread();
    }

    /// Authored names: the `__` reservation, duplicates, and the tables the
    /// typing and use-before-definition checks read.
    fn declare(&mut self) {
        let reserved = |name: &str| name.contains("__");
        let mut problems = Vec::new();
        for (index, (name, _)) in self.params.iter().enumerate() {
            if reserved(name) && !self.patch {
                problems.push((format!("{}/params/{index}", self.fn_pointer), name.clone()));
            }
        }
        for (name, _) in &self.params {
            self.value_taken.insert(name.clone());
            self.block_taken.insert(name.clone());
        }
        for block in &self.blocks {
            if reserved(&block.name) {
                problems.push((block.pointer.clone(), block.name.clone()));
            }
            // A block named twice is the compiler's to report.
            if !self.block_taken.insert(block.name.clone()) {
                self.degraded = true;
            }
        }
        for kept in &self.kept {
            self.block_taken.insert(kept.leaf.clone());
            for (name, _) in kept.params.iter().chain(&kept.ops) {
                self.value_taken.insert(name.clone());
            }
        }
        let mut duplicates = Vec::new();
        for block in &self.blocks {
            let mut defs: BTreeMap<String, ADef> = BTreeMap::new();
            for (index, (name, ty, _)) in block.params.iter().enumerate() {
                if reserved(name) {
                    problems.push((format!("{}/params/{index}", block.pointer), name.clone()));
                }
                defs.insert(name.clone(), ADef { ty: ty.clone() });
            }
            for stmt in &block.stmts {
                let (name, pointer) = match stmt {
                    Stmt::Op(node) => (node.name.clone(), node.pointer.clone()),
                    Stmt::Raw { name, pointer, .. } => (name.clone(), pointer.clone()),
                    Stmt::Exit(_) => (None, String::new()),
                };
                let Some(name) = name else { continue };
                if reserved(&name) {
                    problems.push((pointer.clone(), name.clone()));
                }
                if defs.insert(name.clone(), ADef { ty: None }).is_some() {
                    duplicates.push((pointer, name.clone(), block.name.clone()));
                }
            }
            for name in defs.keys() {
                self.value_taken.insert(name.clone());
            }
            self.defs.push(defs);
        }
        for (pointer, name) in problems {
            self.oblige(
                AgentErrorCode::FrameInvalid,
                &pointer,
                format!(
                    "`{name}` contains `__`, which is reserved for generated names in an \"afx\": 1 frame: rename it"
                ),
            );
        }
        for (pointer, name, block) in duplicates {
            self.oblige(
                AgentErrorCode::FrameInvalid,
                &pointer,
                format!("value `{name}` is defined twice in block `{block}`: rename one"),
            );
        }
    }

    // --- typing -----------------------------------------------------------

    fn block_index(&self, name: &str) -> Option<usize> {
        self.blocks.iter().position(|block| block.name == name)
    }

    /// The type of a value reference for typing (definition order and
    /// dominance are checked while lowering).
    fn value_type(&self, b: usize, text: &str) -> Option<TypeExpr> {
        let (base, suffix) = split_suffix(text);
        if !suffix.is_empty() && suffix != "#0" {
            return None;
        }
        if let Some((block, leaf)) = base.split_once('.') {
            if let Some(target) = self.block_index(block) {
                return self.defs[target].get(leaf).and_then(|def| def.ty.clone());
            }
            return self
                .kept
                .iter()
                .find(|kept| kept.leaf == block)
                .and_then(|kept| kept.value(leaf))
                .and_then(|(_, ty)| ty);
        }
        if let Some(def) = self.defs[b].get(base) {
            return def.ty.clone();
        }
        if let Some((_, ty)) = self.params.iter().find(|(name, _)| name == base) {
            return ty.clone();
        }
        let mut found = Vec::new();
        for (d, defs) in self.defs.iter().enumerate() {
            if d != b
                && let Some(def) = defs.get(base)
            {
                found.push(def.ty.clone());
            }
        }
        for kept in &self.kept {
            if let Some((_, ty)) = kept.value(base) {
                found.push(ty);
            }
        }
        if found.len() == 1 {
            found.remove(0)
        } else {
            None
        }
    }

    fn infer_types(&mut self) {
        let named: usize = self.blocks.iter().map(|block| block.stmts.len()).sum();
        for _ in 0..=named {
            let mut changed = false;
            for b in 0..self.blocks.len() {
                for s in 0..self.blocks[b].stmts.len() {
                    let Stmt::Op(node) = &self.blocks[b].stmts[s] else {
                        continue;
                    };
                    let Some(name) = node.name.clone() else {
                        continue;
                    };
                    if self.defs[b].get(&name).is_some_and(|def| def.ty.is_some()) {
                        continue;
                    }
                    if let Some(ty) = self.node_types(b, node, None).value
                        && let Some(def) = self.defs[b].get_mut(&name)
                    {
                        def.ty = Some(ty);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn const_immediate_type(
        &self,
        immediate: Option<&Value>,
        hint: Option<&TypeExpr>,
    ) -> Option<TypeExpr> {
        match immediate? {
            Value::String(name) => self.cx.const_type(name),
            Value::Object(object) if object.contains_key("value") => types::read(
                object.get("type").unwrap_or(&Value::from("i64")),
                self.cx,
                "",
            )
            .ok(),
            Value::Bool(_) => Some(TypeExpr::Bool),
            _ => Some(
                hint.filter(|ty| matches!(ty, TypeExpr::SInt(_) | TypeExpr::UInt(_)))
                    .cloned()
                    .unwrap_or(TypeExpr::SInt(IntegerWidth::from_bits(64))),
            ),
        }
    }

    /// Result and value types of an operation, and the type each operand
    /// position expects (the typing context of literals and nested
    /// operations).
    #[allow(clippy::too_many_lines)]
    fn node_types(&self, b: usize, node: &Node, expected: Option<&TypeExpr>) -> NodeTypes {
        let row = node.row;
        let tag = row.tag;
        let count = node.args.len();
        let explicit = node
            .ty
            .as_ref()
            .and_then(|ty| types::read(ty, self.cx, "").ok());
        let hint = explicit
            .clone()
            .or_else(|| node.check.is_none().then(|| expected.cloned()).flatten());
        let mut contexts: Vec<Option<TypeExpr>> = vec![None; count];
        let mut immediate_type = None;
        let mut variant_payload: Option<Option<TypeExpr>> = None;
        let mut callee_result = None;
        let mut field_type = None;
        let immediate_text = node.immediate.as_ref().and_then(Value::as_str);
        match row.immediate {
            ImmediateKind::Entity if tag == 1 => {
                immediate_type = self.const_immediate_type(node.immediate.as_ref(), hint.as_ref());
            }
            ImmediateKind::Entity if tag == 18 => {
                if let Some(definition) =
                    immediate_text.and_then(|name| self.cx.type_definition(name))
                {
                    immediate_type = Some(named(definition));
                    if let Some((false, fields)) = self.cx.members(&definition) {
                        for (slot, (_, ty)) in contexts.iter_mut().zip(fields) {
                            *slot = ty;
                        }
                    }
                }
            }
            ImmediateKind::Entity if tag == 193 => {
                immediate_type = immediate_text.and_then(|name| self.cx.global_type(name));
            }
            ImmediateKind::Variant => {
                let resolved = immediate_text.and_then(|path| match path.rsplit_once('.') {
                    Some((ty, leaf)) => Some((self.cx.type_definition(ty)?, leaf.to_owned())),
                    None => match &hint {
                        Some(TypeExpr::Named(named)) => Some((named.definition, path.to_owned())),
                        _ => None,
                    },
                });
                if let Some((definition, leaf)) = resolved
                    && let Some(payload) = self.cx.member_type(&definition, &leaf)
                {
                    if tag == 20 {
                        immediate_type = Some(named(definition));
                        if let (Some(slot), Some(payload)) = (contexts.first_mut(), &payload) {
                            *slot = Some(payload.clone());
                        }
                    }
                    variant_payload = Some(payload);
                }
            }
            ImmediateKind::Field => {
                field_type = immediate_text
                    .and_then(|path| path.rsplit_once('.'))
                    .and_then(|(ty, leaf)| {
                        self.cx
                            .member_type(&self.cx.type_definition(ty)?, leaf)
                            .flatten()
                    });
            }
            ImmediateKind::Function => {
                if let Some((params, result)) =
                    immediate_text.and_then(|name| self.cx.signature(name))
                {
                    if tag == 194 {
                        if let (true, Some(result)) = (params.iter().all(Option::is_some), result) {
                            immediate_type = Some(TypeExpr::FunctionRef(FunctionType {
                                parameters: params.into_iter().flatten().collect(),
                                result: Box::new(result),
                                effects: Vec::new(),
                            }));
                        }
                    } else {
                        callee_result = result;
                        for (slot, ty) in contexts.iter_mut().zip(params) {
                            *slot = ty;
                        }
                    }
                }
            }
            _ => {}
        }
        match tag {
            102..=104 => contexts
                .iter_mut()
                .for_each(|slot| *slot = Some(TypeExpr::Bool)),
            70 | 71 if count > 1 => contexts[1] = Some(uint(32)),
            34 | 35 if count > 1 => contexts[1] = Some(uint(64)),
            _ => {}
        }
        // The operand type the result type fixes.
        let raw_expected = explicit
            .as_ref()
            .or(if node.check.is_none() { expected } else { None });
        let from_expected: Option<TypeExpr> = match tag {
            64..=71 => match (&node.check, raw_expected) {
                (Some(_), _) => expected.cloned(),
                (None, Some(TypeExpr::Result { ok, .. })) => Some((**ok).clone()),
                _ => None,
            },
            80..=85 => raw_expected.cloned(),
            128 => match raw_expected {
                Some(TypeExpr::Option(item)) => Some((**item).clone()),
                _ => None,
            },
            130 => match raw_expected {
                Some(TypeExpr::Result { ok, .. }) => Some((**ok).clone()),
                _ => None,
            },
            131 => match raw_expected {
                Some(TypeExpr::Result { error, .. }) => Some((**error).clone()),
                _ => None,
            },
            176 => match raw_expected {
                Some(TypeExpr::LocalCell(item)) => Some((**item).clone()),
                _ => None,
            },
            32 => match raw_expected {
                Some(TypeExpr::Vector(item)) => Some((**item).clone()),
                _ => None,
            },
            _ => None,
        };
        if let Some(ty) = from_expected {
            let positions: Vec<usize> = match tag {
                70 | 71 | 128 | 130 | 131 | 176 => vec![0],
                _ => (0..count).collect(),
            };
            for position in positions {
                if let Some(slot) = contexts.get_mut(position)
                    && slot.is_none()
                {
                    *slot = Some(ty.clone());
                }
            }
        }
        if let (16, Some(TypeExpr::Tuple(items))) = (tag, raw_expected) {
            for (slot, ty) in contexts.iter_mut().zip(items) {
                if slot.is_none() {
                    *slot = Some(ty.clone());
                }
            }
        }
        let mut operand_types: Vec<Option<TypeExpr>> = node
            .args
            .iter()
            .map(|arg| match &arg.operand {
                Operand::Name(text) => self.value_type(b, text),
                Operand::Literal {
                    typed: Some(ty), ..
                } => types::read(ty, self.cx, "").ok(),
                Operand::Literal {
                    value: Value::Bool(_),
                    ..
                } => Some(TypeExpr::Bool),
                _ => None,
            })
            .collect();
        let groups = same_type_groups(tag, count);
        // A sibling typed later can fix a nested operand's context: settle
        // (each round types at least one more operand, or stops).
        for _ in 0..=count {
            fill_from_partners(&groups, &operand_types, &mut contexts);
            dependent_contexts(tag, &operand_types, &mut contexts);
            let mut changed = false;
            for (position, arg) in node.args.iter().enumerate() {
                if let (Operand::Nested(child), None) = (&arg.operand, &operand_types[position]) {
                    let ty = self.node_types(b, child, contexts[position].as_ref()).value;
                    changed |= ty.is_some();
                    operand_types[position] = ty;
                }
            }
            if !changed {
                break;
            }
        }
        fill_from_partners(&groups, &operand_types, &mut contexts);
        dependent_contexts(tag, &operand_types, &mut contexts);
        for (position, arg) in node.args.iter().enumerate() {
            if operand_types[position].is_none()
                && matches!(arg.operand, Operand::Literal { typed: None, .. })
            {
                operand_types[position].clone_from(&contexts[position]);
            }
        }
        if let (17, Some(index), Some(Some(TypeExpr::Tuple(items)))) = (
            tag,
            node.immediate.as_ref().and_then(Value::as_u64),
            operand_types.first(),
        ) {
            field_type = usize::try_from(index)
                .ok()
                .and_then(|index| items.get(index).cloned());
        }
        let raw = explicit.or_else(|| {
            if operand_types.iter().any(Option::is_none) {
                return None;
            }
            let known: Vec<TypeExpr> = operand_types.iter().flatten().cloned().collect();
            if tag == 36 && known.len() >= 2 && known.len().is_multiple_of(2) {
                return Some(TypeExpr::Result {
                    ok: Box::new(TypeExpr::OrderedMap {
                        key: Box::new(known[0].clone()),
                        value: Box::new(known[1].clone()),
                    }),
                    error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::DuplicateKey)),
                });
            }
            crate::frame::infer(
                row,
                &known,
                hint.as_ref(),
                immediate_type.as_ref(),
                variant_payload.as_ref(),
                callee_result.as_ref(),
                field_type.as_ref(),
            )
        });
        let value = match (&node.check, &raw) {
            (None, _) => raw.clone(),
            (Some(_), Some(TypeExpr::Result { ok, .. })) => Some((**ok).clone()),
            (Some(_), Some(TypeExpr::Option(item))) => Some((**item).clone()),
            _ => None,
        };
        NodeTypes {
            raw,
            value,
            contexts,
        }
    }

    // --- naming -------------------------------------------------------------

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

    fn alloc_value(&mut self, base: &str) -> String {
        let name = self.unique(base, &self.value_taken);
        self.value_taken.insert(name.clone());
        name
    }

    fn alloc_block(&mut self, base: &str) -> String {
        let name = self.unique(base, &self.block_taken);
        self.block_taken.insert(name.clone());
        name
    }

    // --- lowering -----------------------------------------------------------

    fn new_piece(
        &mut self,
        name: String,
        b: usize,
        authored: &str,
        role: Role,
        unwrapped: Option<(String, Value)>,
    ) -> usize {
        let index = self.pieces.len();
        let local = self.block_pieces[b].len();
        self.pieces.push(Piece {
            name,
            block: b,
            local,
            authored: authored.to_owned(),
            role,
            unwrapped,
            threaded: Vec::new(),
            ops: Vec::new(),
            term: Tpl::Lit(Value::Null),
            term_authored: authored.to_owned(),
            term_role: role,
            uses: Vec::new(),
            succ: Vec::new(),
        });
        self.block_pieces[b].push(index);
        index
    }

    fn push_op(&mut self, st: &Lower, op: EOp) {
        let piece = &mut self.pieces[st.piece];
        for sym in &op.operands {
            if let Sym::Local(name, _) = sym {
                piece.uses.push(name.clone());
            }
        }
        piece.ops.push(op);
    }

    fn set_term(&mut self, piece: usize, term: Tpl, authored: &str, role: Role, succ: Vec<String>) {
        let mut uses = Vec::new();
        collect_uses(&term, &mut uses);
        let piece = &mut self.pieces[piece];
        piece.uses.extend(uses);
        piece.term = term;
        authored.clone_into(&mut piece.term_authored);
        piece.term_role = role;
        piece.succ = succ.into_iter().filter(|name| !name.is_empty()).collect();
    }

    fn lower_block(&mut self, b: usize) {
        let block = self.blocks[b].clone();
        let first = self.new_piece(block.name.clone(), b, &block.pointer, Role::Block, None);
        let mut st = Lower {
            b,
            piece: first,
            defs: BTreeMap::new(),
            exits: 0,
            term_k: 0,
            quiet: false,
        };
        for (name, ty, json) in &block.params {
            st.defs.insert(
                name.clone(),
                LDef {
                    piece: first,
                    kind: Kind::Param,
                    ty: ty.clone(),
                    ty_json: Some(json.clone()),
                },
            );
        }
        for stmt in &block.stmts {
            match stmt {
                Stmt::Op(node) => {
                    let name = node.name.clone().unwrap_or_default();
                    let types = self.node_types(b, node, None);
                    let operands = self.lower_args(&mut st, node, &types, &name, 0);
                    self.finish_node(&mut st, node, &name, operands, &types, Role::Op);
                }
                Stmt::Exit(exit) => self.lower_exit(&mut st, exit),
                Stmt::Raw {
                    value,
                    pointer,
                    name,
                } => {
                    let op = EOp {
                        name: name.clone().unwrap_or_default(),
                        word: String::new(),
                        immediate: None,
                        operands: Vec::new(),
                        ty: None,
                        authored: pointer.clone(),
                        role: Role::Op,
                        raw: Some(value.clone()),
                    };
                    self.push_op(&st, op);
                    if let Some(name) = name {
                        st.defs.insert(
                            name.clone(),
                            LDef {
                                piece: st.piece,
                                kind: Kind::Op,
                                ty: None,
                                ty_json: None,
                            },
                        );
                    }
                }
            }
        }
        self.lower_term(&mut st, &block);
        self.block_defs[b] = st.defs;
    }

    fn lower_args(
        &mut self,
        st: &mut Lower,
        node: &Node,
        types: &NodeTypes,
        base: &str,
        depth: usize,
    ) -> Vec<Sym> {
        // An operand that names no value leaves its partner literal
        // untyped: report the name, not the literal.
        let untyped = node.args.iter().enumerate().any(|(position, arg)| {
            matches!(&arg.operand, Operand::Literal { typed: None, value } if !value.is_boolean())
                && types.contexts.get(position).is_none_or(Option::is_none)
        });
        let quiet = st.quiet;
        if untyped {
            let unknown = self.unknown_names(st, node);
            for (name, at) in &unknown {
                self.oblige(
                    AgentErrorCode::XScope,
                    at,
                    format!("no value named `{name}` in this function"),
                );
            }
            st.quiet |= !unknown.is_empty();
        }
        let mut out = Vec::with_capacity(node.args.len());
        for (position, arg) in node.args.iter().enumerate() {
            let context = types.contexts.get(position).cloned().flatten();
            out.push(self.lower_operand(
                st,
                arg,
                context.as_ref(),
                &format!("{base}__a{position}"),
                depth,
                true,
            ));
        }
        st.quiet = quiet;
        out
    }

    fn lower_operand(
        &mut self,
        st: &mut Lower,
        arg: &Arg,
        context: Option<&TypeExpr>,
        generated: &str,
        depth: usize,
        nest: bool,
    ) -> Sym {
        match &arg.operand {
            Operand::Name(text) => self.resolve(st, text, &arg.pointer),
            Operand::Literal { value, typed } => {
                self.literal(st, value, typed.as_ref(), context, generated, &arg.pointer)
            }
            Operand::Nested(child) => {
                if !nest {
                    self.oblige(
                        AgentErrorCode::FrameInvalid,
                        &arg.pointer,
                        "only names and literals may appear here, because the operand is evaluated on one path only: name the operation in the target block",
                    );
                    return Sym::Raw(Value::Null);
                }
                let name = self.alloc_value(generated);
                self.value_names.insert(name.clone(), child.pointer.clone());
                let types = self.node_types(st.b, child, context);
                let operands = self.lower_args(st, child, &types, &name, depth + 1);
                self.stats.nested += 1;
                self.finish_node(st, child, &name, operands, &types, Role::Nested);
                Sym::Local(name, String::new())
            }
            Operand::Raw(value) => Sym::Raw(value.clone()),
        }
    }

    fn literal(
        &mut self,
        st: &mut Lower,
        value: &Value,
        typed: Option<&Value>,
        context: Option<&TypeExpr>,
        generated: &str,
        pointer: &str,
    ) -> Sym {
        let (ty_json, ty) = match (typed, value) {
            (Some(ty), _) => (ty.clone(), types::read(ty, self.cx, "").ok()),
            (None, Value::Bool(_)) => (Value::from("bool"), Some(TypeExpr::Bool)),
            (None, _) => {
                let Some(ty) = context else {
                    if st.quiet {
                        return Sym::Raw(value.clone());
                    }
                    self.oblige(
                        AgentErrorCode::FrameInvalid,
                        pointer,
                        format!(
                            "nothing here fixes the type of the literal {value}: state it, e.g. {{\"type\": \"i64\", \"value\": {value}}}"
                        ),
                    );
                    return Sym::Raw(value.clone());
                };
                (Value::from(self.cx.render(ty)), Some(ty.clone()))
            }
        };
        let name = self.alloc_value(generated);
        self.value_names.insert(name.clone(), pointer.to_owned());
        let op = EOp {
            name: name.clone(),
            word: "const".to_owned(),
            immediate: Some(json!({"type": ty_json, "value": value})),
            operands: Vec::new(),
            ty: None,
            authored: pointer.to_owned(),
            role: Role::Literal,
            raw: None,
        };
        self.push_op(st, op);
        st.defs.insert(
            name.clone(),
            LDef {
                piece: st.piece,
                kind: Kind::Op,
                ty,
                ty_json: None,
            },
        );
        self.stats.literals += 1;
        Sym::Local(name, String::new())
    }

    /// Resolves an authored name while lowering: this block's earlier
    /// values, the function's parameters, then (deferred) a unique
    /// dominating result of another block.
    fn resolve(&mut self, st: &Lower, text: &str, pointer: &str) -> Sym {
        if text == "$" {
            return Sym::Plain(text.to_owned());
        }
        let (base, suffix) = split_suffix(text);
        if let Some((block, leaf)) = base.split_once('.') {
            return match self.block_index(block) {
                // `B.x` in B names B's own `x`, as in plain AF1: never a
                // value of another block.
                Some(target) if target == st.b => {
                    if st.defs.contains_key(leaf) {
                        Sym::Local(leaf.to_owned(), suffix.to_owned())
                    } else if self.defs[st.b].contains_key(leaf) {
                        let block = self.blocks[st.b].name.clone();
                        self.oblige(
                            AgentErrorCode::XScope,
                            pointer,
                            format!(
                                "`{text}` is used before its definition in block `{block}`: a block's values are defined in order, so move the use after it"
                            ),
                        );
                        Sym::Plain(text.to_owned())
                    } else {
                        Sym::Plain(text.to_owned())
                    }
                }
                Some(target) => Sym::Qualified {
                    block: target,
                    leaf: leaf.to_owned(),
                    suffix: suffix.to_owned(),
                    text: text.to_owned(),
                    pointer: pointer.to_owned(),
                },
                None => Sym::Plain(text.to_owned()),
            };
        }
        self.resolve_plain(st, base, suffix, pointer)
    }

    fn resolve_plain(&mut self, st: &Lower, name: &str, suffix: &str, pointer: &str) -> Sym {
        if st.defs.contains_key(name) {
            return Sym::Local(name.to_owned(), suffix.to_owned());
        }
        let block = self.blocks[st.b].name.clone();
        if self.defs[st.b].contains_key(name) {
            self.oblige(
                AgentErrorCode::XScope,
                pointer,
                format!(
                    "`{name}` is used before its definition in block `{block}`: a block's values are defined in order, so move the use after it or rename one of them"
                ),
            );
            return Sym::Plain(format!("{name}{suffix}"));
        }
        if self.params.iter().any(|(param, _)| param == name) {
            return Sym::Plain(format!("{name}{suffix}"));
        }
        Sym::Far(Box::new(Far {
            name: name.to_owned(),
            suffix: suffix.to_owned(),
            pointer: pointer.to_owned(),
            derive: None,
        }))
    }

    /// The problem when a checked operation's type is unknown: an operand
    /// that names no value, else (unless a problem inside the operands
    /// explains it) the unknown type itself.
    fn unknown_check_type(&mut self, st: &Lower, node: &Node) {
        let word = &node.word;
        let inner = format!("{}/", node.pointer);
        let unknown = self.unknown_names(st, node);
        for (unknown, at) in &unknown {
            self.oblige(
                AgentErrorCode::XScope,
                at,
                format!("no value named `{unknown}` in this function"),
            );
        }
        if unknown.is_empty()
            && !self
                .obligations
                .iter()
                .any(|obligation| obligation.at.starts_with(&inner))
        {
            self.oblige(
                AgentErrorCode::XPropagation,
                &node.pointer,
                format!(
                    "the result type of `{word}?` is not known here, so its failure route cannot be checked: give its operands known types (a typed literal is {{\"type\": \"i64\", \"value\": 3}})"
                ),
            );
        }
    }

    /// Operand names of `node` (nested ones included) that no block of the
    /// function, kept live block, or function parameter defines.
    fn unknown_names(&self, st: &Lower, node: &Node) -> Vec<(String, String)> {
        let mut found = Vec::new();
        for arg in &node.args {
            match &arg.operand {
                Operand::Name(text) => {
                    let (base, _) = split_suffix(text);
                    let leaf = base.rsplit('.').next().unwrap_or(base);
                    let known = text == "$"
                        || st.defs.contains_key(leaf)
                        || self.params.iter().any(|(param, _)| param == leaf)
                        || self.defs.iter().any(|defs| defs.contains_key(leaf))
                        || self.kept.iter().any(|kept| kept.value(leaf).is_some());
                    if !known {
                        found.push((text.clone(), arg.pointer.clone()));
                    }
                }
                Operand::Nested(inner) => found.extend(self.unknown_names(st, inner)),
                Operand::Literal { .. } | Operand::Raw(_) => {}
            }
        }
        found
    }

    /// Whether a type the frame names needs a stated type on a generated
    /// operation (the compiler infers it only from uses).
    fn stated_type(&self, node: &Node, types: &NodeTypes) -> Option<Value> {
        if node.ty.is_some() {
            return node.ty.clone();
        }
        let needs = matches!(node.row.tag, 36 | 129 | 130 | 131 | 160..=162)
            || (node.row.tag == 32 && node.args.is_empty())
            || (node.row.tag == 20
                && node
                    .immediate
                    .as_ref()
                    .and_then(Value::as_str)
                    .is_some_and(|path| !path.contains('.')));
        if needs {
            types.raw.as_ref().map(|ty| Value::from(self.cx.render(ty)))
        } else {
            None
        }
    }

    fn finish_node(
        &mut self,
        st: &mut Lower,
        node: &Node,
        name: &str,
        operands: Vec<Sym>,
        types: &NodeTypes,
        role: Role,
    ) {
        if let Some(check) = &node.check {
            let result = self.alloc_value(&format!("{name}__r"));
            self.value_names
                .insert(result.clone(), node.pointer.clone());
            let op = EOp {
                name: result.clone(),
                word: node.word.clone(),
                immediate: node.immediate.clone(),
                operands,
                ty: self.stated_type(node, types),
                authored: node.pointer.clone(),
                role: Role::Checked,
                raw: None,
            };
            self.push_op(st, op);
            st.defs.insert(
                result.clone(),
                LDef {
                    piece: st.piece,
                    kind: Kind::Op,
                    ty: types.raw.clone(),
                    ty_json: None,
                },
            );
            self.split(st, node, check, name, &result, types);
        } else {
            let ty = if role == Role::Op {
                node.ty.clone()
            } else {
                self.stated_type(node, types)
            };
            let op = EOp {
                name: name.to_owned(),
                word: node.word.clone(),
                immediate: node.immediate.clone(),
                operands,
                ty,
                authored: node.pointer.clone(),
                role,
                raw: None,
            };
            self.push_op(st, op);
            st.defs.insert(
                name.to_owned(),
                LDef {
                    piece: st.piece,
                    kind: Kind::Op,
                    ty: types.value.clone(),
                    ty_json: None,
                },
            );
        }
    }

    /// X2: ends the piece with a switch on the checked result and continues
    /// in a new piece that takes the unwrapped value.
    fn split(
        &mut self,
        st: &mut Lower,
        node: &Node,
        check: &str,
        name: &str,
        result: &str,
        types: &NodeTypes,
    ) {
        let word = &node.word;
        // A refused split still defines the name, so later uses do not
        // cascade into more problems.
        let define = |st: &mut Lower| {
            st.defs.insert(
                name.to_owned(),
                LDef {
                    piece: st.piece,
                    kind: Kind::Op,
                    ty: None,
                    ty_json: None,
                },
            );
        };
        let (ok_key, err_key, payload) = match &types.raw {
            Some(TypeExpr::Result { error, .. }) => ("Ok", "Err", Some((**error).clone())),
            Some(TypeExpr::Option(_)) => ("Some", "None", None),
            Some(other) => {
                let other = self.cx.render(other);
                self.oblige(
                    AgentErrorCode::XPropagation,
                    &node.pointer,
                    format!(
                        "`{word}?` propagates the failure of a Result or an Option, but `{word}` gives {other}: drop the `?`"
                    ),
                );
                define(st);
                return;
            }
            None => {
                self.unknown_check_type(st, node);
                define(st);
                return;
            }
        };
        let option = ok_key == "Some";
        let failure =
            self.failure_case(st, check, err_key, payload.as_ref(), option, &node.pointer);
        let ty_json = types
            .value
            .as_ref()
            .map_or(Value::from("unit"), |ty| Value::from(self.cx.render(ty)));
        let block = self.blocks[st.b].name.clone();
        let continuation = self.alloc_block(&format!("{block}__{name}"));
        self.block_names
            .insert(continuation.clone(), node.pointer.clone());
        let next = self.new_piece(
            continuation.clone(),
            st.b,
            &node.pointer,
            Role::Continuation,
            Some((name.to_owned(), ty_json.clone())),
        );
        let mut succ = vec![continuation.clone()];
        let failure = match failure {
            Some((tpl, target)) => {
                succ.push(target);
                tpl
            }
            None => Tpl::List(vec![lit(err_key), Tpl::Lit(Value::Null)]),
        };
        let term = Tpl::List(vec![
            lit("switch"),
            Tpl::Sym(Sym::Local(result.to_owned(), String::new())),
            Tpl::List(vec![
                lit(ok_key),
                Tpl::Lit(Value::from(continuation)),
                lit("$"),
                Tpl::Threaded(next),
            ]),
            failure,
        ]);
        self.set_term(st.piece, term, &node.pointer, Role::Switch, succ);
        st.piece = next;
        st.defs.insert(
            name.to_owned(),
            LDef {
                piece: next,
                kind: Kind::Unwrapped,
                ty: types.value.clone(),
                ty_json: Some(ty_json),
            },
        );
        self.stats.checked += 1;
        self.stats.generated_blocks += 1;
    }

    fn error_variant(&self) -> Option<(String, Members)> {
        let Some(TypeExpr::Result { error, .. }) = &self.result else {
            return None;
        };
        let TypeExpr::Named(named) = &**error else {
            return None;
        };
        match self.cx.members(&named.definition)? {
            (true, cases) => Some((self.cx.render(error), cases)),
            (false, _) => None,
        }
    }

    /// `None` when `case` is no case of the error variant, `Some(None)`
    /// for a unit case.
    #[allow(clippy::option_option)]
    fn error_case(&self, case: &str) -> Option<Option<TypeExpr>> {
        self.error_variant()?
            .1
            .into_iter()
            .find(|(leaf, _)| leaf == case)
            .map(|(_, payload)| payload)
    }

    fn is_block(&self, name: &str) -> bool {
        self.block_index(name).is_some() || self.kept.iter().any(|kept| kept.leaf == name)
    }

    fn target_params(&self, name: &str) -> Option<Vec<(String, Option<TypeExpr>)>> {
        if let Some(b) = self.block_index(name) {
            return Some(
                self.blocks[b]
                    .params
                    .iter()
                    .map(|(name, ty, _)| (name.clone(), ty.clone()))
                    .collect(),
            );
        }
        self.kept
            .iter()
            .find(|kept| kept.leaf == name)
            .map(|kept| kept.params.clone())
    }

    fn route(&mut self, name: &str, pointer: &str) -> Option<Route> {
        let block = self.is_block(name);
        let case = self.error_case(name);
        let function = self.fn_name.clone();
        match (block, case) {
            (true, Some(_)) => {
                let variant = self.error_variant().map(|(v, _)| v).unwrap_or_default();
                self.oblige(
                    AgentErrorCode::XPropagation,
                    pointer,
                    format!(
                        "`{name}` names both a block of `{function}` and a case of {variant}, so the failure route is ambiguous: rename the block"
                    ),
                );
                None
            }
            (true, None) => Some(Route::Handler(name.to_owned())),
            (false, Some(payload)) => Some(Route::Case(name.to_owned(), payload)),
            (false, None) => {
                let cases = self
                    .error_variant()
                    .map_or_else(String::new, |(variant, cases)| {
                        format!(
                            " (cases of {variant}: {})",
                            cases
                                .iter()
                                .map(|(leaf, _)| leaf.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    });
                self.oblige(
                    AgentErrorCode::XPropagation,
                    pointer,
                    format!(
                        "`{name}` is neither a block of `{function}` nor a case of its error type{cases}: name a case or a handler block"
                    ),
                );
                None
            }
        }
    }

    fn shared(&mut self, kind: ExitKind, pointer: &str) -> String {
        let key = match &kind {
            ExitKind::Fail { case, .. } => format!("fail:{case}"),
            ExitKind::Err(_) => "err".to_owned(),
            ExitKind::None => "none".to_owned(),
        };
        if let Some(exit) = self.exits.iter().find(|exit| exit.key == key) {
            return exit.name.clone();
        }
        let base = match &kind {
            ExitKind::Fail { case, .. } => format!("__fail_{case}"),
            ExitKind::Err(_) => "__err".to_owned(),
            ExitKind::None => "__none".to_owned(),
        };
        let name = self.alloc_block(&base);
        self.block_names.insert(name.clone(), pointer.to_owned());
        self.stats.generated_blocks += 1;
        self.exits.push(SharedExit {
            key,
            name: name.clone(),
            kind,
            authored: pointer.to_owned(),
        });
        name
    }

    fn shared_fail(&mut self, case: &str, payload: Option<&TypeExpr>, pointer: &str) -> String {
        let variant = self.error_variant().map(|(v, _)| v).unwrap_or_default();
        let payload = payload.map(|ty| Value::from(self.cx.render(ty)));
        self.shared(
            ExitKind::Fail {
                case: case.to_owned(),
                variant,
                payload,
            },
            pointer,
        )
    }

    /// The failure case of a checked operation's switch, and its target.
    #[allow(clippy::too_many_lines)]
    fn failure_case(
        &mut self,
        st: &mut Lower,
        check: &str,
        key: &str,
        payload: Option<&TypeExpr>,
        option: bool,
        pointer: &str,
    ) -> Option<(Tpl, String)> {
        if check.is_empty() {
            let function = self.fn_name.clone();
            match (&self.result, option) {
                (Some(TypeExpr::Result { error, .. }), false) if payload == Some(&**error) => {
                    let ty = Value::from(self.cx.render(error));
                    let exit = self.shared(ExitKind::Err(ty), pointer);
                    return Some((Tpl::List(vec![lit(key), lit(&exit), lit("$")]), exit));
                }
                (Some(TypeExpr::Option(_)), true) => {
                    let exit = self.shared(ExitKind::None, pointer);
                    return Some((Tpl::List(vec![lit(key), lit(&exit)]), exit));
                }
                _ => {}
            }
            let why = match (&self.result, option, payload) {
                (_, true, _) => {
                    format!("the operation gives an Option, but `{function}` does not return one")
                }
                (Some(TypeExpr::Result { error, .. }), false, Some(payload)) => format!(
                    "the operation fails with {} where `{function}` fails with {}",
                    self.cx.render(payload),
                    self.cx.render(error)
                ),
                _ => format!("`{function}` does not return a Result"),
            };
            self.oblige(
                AgentErrorCode::XPropagation,
                pointer,
                format!("a bare `?` passes the failure on unchanged, but {why}: name a case: op?Case, or a handler block"),
            );
            return None;
        }
        match self.route(check, pointer)? {
            Route::Handler(handler) => {
                let params = self.target_params(&handler).unwrap_or_default();
                let mut items = vec![lit(key), lit(&handler)];
                let mut rest = &params[..];
                if !option && !params.is_empty() {
                    let (first_name, first) = &params[0];
                    if let (Some(first), Some(payload)) = (first, payload)
                        && first != payload
                    {
                        let (first, payload) = (self.cx.render(first), self.cx.render(payload));
                        self.oblige(
                            AgentErrorCode::XPropagation,
                            pointer,
                            format!(
                                "handler `{handler}` starts with `{first_name}: {first}`, but the failure payload is {payload}: make its first parameter {payload}, or give it no parameters to drop the payload"
                            ),
                        );
                        return None;
                    }
                    items.push(lit("$"));
                    rest = &params[1..];
                }
                for (param, ty) in rest {
                    let sym = self.derive(st, param, ty.as_ref(), &handler, pointer);
                    items.push(Tpl::Sym(sym));
                }
                Some((Tpl::List(items), handler))
            }
            Route::Case(case, case_payload) => {
                let exit = self.shared_fail(&case, case_payload.as_ref(), pointer);
                match (case_payload, option) {
                    (None, _) => Some((Tpl::List(vec![lit(key), lit(&exit)]), exit)),
                    (Some(expected), false) if payload == Some(&expected) => {
                        Some((Tpl::List(vec![lit(key), lit(&exit), lit("$")]), exit))
                    }
                    (Some(expected), false) => {
                        let expected = self.cx.render(&expected);
                        let payload =
                            payload.map_or_else(|| "none".to_owned(), |p| self.cx.render(p));
                        self.oblige(
                            AgentErrorCode::XPropagation,
                            pointer,
                            format!(
                                "case `{case}` carries a {expected}, but the failure payload here is {payload}: name a unit case to drop it, a case carrying {payload}, or a handler block"
                            ),
                        );
                        None
                    }
                    (Some(expected), true) => {
                        let expected = self.cx.render(&expected);
                        self.oblige(
                            AgentErrorCode::XPropagation,
                            pointer,
                            format!(
                                "case `{case}` carries a {expected}, but a None has no payload to pass: name a unit case or a handler block"
                            ),
                        );
                        None
                    }
                }
            }
        }
    }

    /// Values of type `ty` visible at this point (`name: type`).
    fn visible_of_type(&self, st: &Lower, ty: Option<&TypeExpr>) -> Vec<String> {
        let mut out = Vec::new();
        let matches = |candidate: Option<&TypeExpr>| ty.is_none() || candidate == ty;
        for (name, def) in &st.defs {
            if !name.contains("__") && matches(def.ty.as_ref()) {
                out.push(format!(
                    "{name}: {}",
                    def.ty
                        .as_ref()
                        .map_or("?".to_owned(), |t| self.cx.render(t))
                ));
            }
        }
        for (name, param_ty) in &self.params {
            if !st.defs.contains_key(name) && matches(param_ty.as_ref()) {
                out.push(format!(
                    "{name}: {}",
                    param_ty
                        .as_ref()
                        .map_or("?".to_owned(), |t| self.cx.render(t))
                ));
            }
        }
        out
    }

    fn missing_argument(
        &mut self,
        pointer: &str,
        target: &str,
        param: &str,
        ty: Option<&TypeExpr>,
        available: Vec<String>,
        detail: &str,
    ) {
        let rendered = ty.map(|ty| self.cx.render(ty));
        let listed = if available.is_empty() {
            "none".to_owned()
        } else {
            available.join(", ")
        };
        let mut obligation = Obligation::new(
            AgentErrorCode::XScope,
            pointer,
            format!(
                "block `{target}` takes `{param}: {}`, and {detail}: pass it explicitly (values of that type here: {listed})",
                rendered.as_deref().unwrap_or("?")
            ),
        );
        obligation.expected = rendered;
        obligation.available = Some(available);
        self.obligations.push(obligation);
    }

    /// X4: a derived trailing edge argument, found by name. The edge's own
    /// block and the function's parameters are looked at here; whether the
    /// edge goes back into a loop, and values of dominating blocks, are
    /// decided on the expanded graph ([`Self::resolve_far`]).
    fn derive(
        &mut self,
        st: &Lower,
        param: &str,
        ty: Option<&TypeExpr>,
        target: &str,
        pointer: &str,
    ) -> Sym {
        let local = if let Some(def) = st.defs.get(param) {
            Some((
                Box::new(Sym::Local(param.to_owned(), String::new())),
                def.ty.clone(),
            ))
        } else if let Some((_, param_ty)) = self.params.iter().find(|(name, _)| name == param) {
            Some((Box::new(Sym::Plain(param.to_owned())), param_ty.clone()))
        } else {
            None
        };
        if local.is_none() && self.defs[st.b].contains_key(param) {
            let block = self.blocks[st.b].name.clone();
            self.oblige(
                AgentErrorCode::XScope,
                pointer,
                format!(
                    "block `{target}` takes `{param}`, which block `{block}` defines only after this point: pass the argument explicitly or move the definition"
                ),
            );
            return Sym::Raw(Value::Null);
        }
        let available = self.visible_of_type(st, ty);
        Sym::Far(Box::new(Far {
            name: param.to_owned(),
            suffix: String::new(),
            pointer: pointer.to_owned(),
            derive: Some(Derive {
                ty: ty.cloned(),
                target: target.to_owned(),
                available,
                local,
            }),
        }))
    }

    /// X3: `["!T", "if", cond(, payload)]` ends the piece with a `cond`.
    fn lower_exit(&mut self, st: &mut Lower, exit: &Exit) {
        let block = self.blocks[st.b].name.clone();
        let index = st.exits;
        st.exits += 1;
        let base = format!("{block}__if{index}");
        let cond = self.lower_operand(st, &exit.cond, Some(&TypeExpr::Bool), &base, 0, true);
        self.stats.exits += 1;
        let edge = self.exit_edge(st, exit, &format!("{base}__p"));
        let continuation = self.alloc_block(&base);
        self.block_names
            .insert(continuation.clone(), exit.pointer.clone());
        let next = self.new_piece(
            continuation.clone(),
            st.b,
            &exit.pointer,
            Role::Continuation,
            None,
        );
        let (edge, target) = edge.unwrap_or((Tpl::Lit(Value::Null), String::new()));
        let term = Tpl::List(vec![
            lit("cond"),
            Tpl::Sym(cond),
            edge,
            Tpl::List(vec![
                Tpl::Lit(Value::from(continuation.clone())),
                Tpl::Threaded(next),
            ]),
        ]);
        self.set_term(
            st.piece,
            term,
            &exit.pointer,
            Role::Exit,
            vec![target, continuation],
        );
        st.piece = next;
        self.stats.generated_blocks += 1;
    }

    /// A payload of an exit or a `fail`: a name or a literal (a nested
    /// operation only where `nest` allows), checked against `ty`.
    fn payload(
        &mut self,
        st: &mut Lower,
        arg: &Arg,
        ty: Option<&TypeExpr>,
        generated: &str,
        nest: bool,
        target: &str,
    ) -> Option<Sym> {
        if let (Operand::Name(text), Some(ty)) = (&arg.operand, ty)
            && let Some(found) = self.value_type(st.b, text)
            && found != *ty
        {
            let (found, ty) = (self.cx.render(&found), self.cx.render(ty));
            self.oblige(
                AgentErrorCode::XPropagation,
                &arg.pointer,
                format!("the payload `{text}` is {found}, but `{target}` takes {ty}: pass a {ty}"),
            );
            return None;
        }
        Some(self.lower_operand(st, arg, ty, generated, 0, nest))
    }

    fn exit_edge(&mut self, st: &mut Lower, exit: &Exit, generated: &str) -> Option<(Tpl, String)> {
        if exit.target.is_empty() {
            if matches!(self.result, Some(TypeExpr::Option(_))) && exit.payload.is_none() {
                let name = self.shared(ExitKind::None, &exit.pointer);
                return Some((Tpl::Lit(Value::from(name.clone())), name));
            }
            self.oblige(
                AgentErrorCode::XPropagation,
                &exit.pointer,
                "a bare `!` returns none (no payload) and needs a function that returns an Option: name a case (\"!Case\") or a handler block",
            );
            return None;
        }
        match self.route(&exit.target, &exit.pointer)? {
            Route::Handler(handler) => {
                let params = self.target_params(&handler).unwrap_or_default();
                let mut items = vec![Tpl::Lit(Value::from(handler.clone()))];
                let mut rest = &params[..];
                if let Some(payload) = &exit.payload {
                    let Some((_, first)) = params.first() else {
                        self.oblige(
                            AgentErrorCode::XPropagation,
                            &exit.pointer,
                            format!("handler `{handler}` takes no parameters, so it cannot take the payload: add one, or drop the payload"),
                        );
                        return None;
                    };
                    let first = first.clone();
                    let sym =
                        self.payload(st, payload, first.as_ref(), generated, false, &handler)?;
                    items.push(Tpl::Sym(sym));
                    rest = &params[1..];
                }
                for (param, ty) in rest {
                    let sym = self.derive(st, param, ty.as_ref(), &handler, &exit.pointer);
                    items.push(Tpl::Sym(sym));
                }
                let tpl = if items.len() == 1 {
                    items.remove(0)
                } else {
                    Tpl::List(items)
                };
                Some((tpl, handler))
            }
            Route::Case(case, case_payload) => {
                let name = self.shared_fail(&case, case_payload.as_ref(), &exit.pointer);
                match (case_payload, &exit.payload) {
                    (None, None) => Some((Tpl::Lit(Value::from(name.clone())), name)),
                    (None, Some(_)) => {
                        self.oblige(
                            AgentErrorCode::XPropagation,
                            &exit.pointer,
                            format!("case `{case}` carries no payload: drop the payload, or name a case that carries one"),
                        );
                        None
                    }
                    (Some(expected), None) => {
                        let expected = self.cx.render(&expected);
                        self.oblige(
                            AgentErrorCode::XPropagation,
                            &exit.pointer,
                            format!("case `{case}` carries a {expected}: add it, [\"!{case}\", \"if\", condition, payload]"),
                        );
                        None
                    }
                    (Some(expected), Some(payload)) => {
                        let sym =
                            self.payload(st, payload, Some(&expected), generated, false, &case)?;
                        Some((
                            Tpl::List(vec![Tpl::Lit(Value::from(name.clone())), Tpl::Sym(sym)]),
                            name,
                        ))
                    }
                }
            }
        }
    }

    fn term_name(&self, st: &mut Lower) -> String {
        let index = st.term_k;
        st.term_k += 1;
        format!("{}__t{index}", self.blocks[st.b].name)
    }

    /// The arguments of an edge (cond/switch targets allow names and
    /// literals only; `br` also nested operations), then the derived
    /// trailing ones.
    fn edge_args(
        &mut self,
        st: &mut Lower,
        target: &Target,
        nest: bool,
        pointer: &str,
        case: Option<(&Value, &CasePayload)>,
    ) -> Vec<Tpl> {
        let params = self.target_params(&target.block);
        // More arguments than parameters is the problem to report: the
        // surplus ones have no parameter to type a literal from.
        let surplus = params
            .as_ref()
            .is_some_and(|params| target.args.len() > params.len());
        if let (true, Some(params)) = (surplus, &params) {
            let listed = params
                .iter()
                .map(|(name, ty)| {
                    format!(
                        "{name}: {}",
                        ty.as_ref().map_or("?".to_owned(), |ty| self.cx.render(ty))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            self.oblige(
                AgentErrorCode::FrameInvalid,
                &target.pointer,
                format!(
                    "block `{}` takes {} argument(s) ({listed}); this edge passes {}",
                    target.block,
                    params.len(),
                    target.args.len()
                ),
            );
        }
        let quiet = st.quiet;
        st.quiet |= surplus;
        let mut items = Vec::new();
        for (position, arg) in target.args.iter().enumerate() {
            let generated = self.term_name(st);
            let context = params
                .as_ref()
                .and_then(|params| params.get(position))
                .and_then(|(_, ty)| ty.clone());
            let sym = self.lower_operand(st, arg, context.as_ref(), &generated, 0, nest);
            items.push(Tpl::Sym(sym));
        }
        st.quiet = quiet;
        let Some(params) = params.filter(|_| !surplus) else {
            return items;
        };
        let missing = &params[target.args.len()..];
        if missing.is_empty() {
            return items;
        }
        // A case payload never gives way to a value found by name: without
        // `$` the payload would be dropped unseen.
        let passes_payload = target
            .args
            .iter()
            .any(|arg| matches!(&arg.operand, Operand::Name(text) if text == "$"));
        if let Some((key, CasePayload::Carries(payload))) = case
            && !passes_payload
        {
            let key = key.as_str().unwrap_or("?");
            let carried = payload.as_ref().map_or_else(
                || "a payload".to_owned(),
                |ty| format!("a payload ({})", self.cx.render(ty)),
            );
            let block = &target.block;
            self.oblige(
                AgentErrorCode::XScope,
                &target.pointer,
                format!(
                    "case `{key}` carries {carried} that this edge does not pass, and `{block}` takes more arguments than the edge gives: pass the payload with \"$\" where `{block}` takes it (e.g. [\"{key}\", \"{block}\", \"$\"]), or write every argument"
                ),
            );
            items.extend(missing.iter().map(|_| Tpl::Lit(Value::Null)));
            return items;
        }
        for (param, ty) in missing {
            let sym = self.derive(st, param, ty.as_ref(), &target.block, pointer);
            items.push(Tpl::Sym(sym));
        }
        items
    }

    /// Whether a switch case passes a payload on, by its key and the type
    /// of the switched value.
    fn case_payload(&self, key: &Value, scrutinee: Option<&TypeExpr>) -> CasePayload {
        let Some(key) = key.as_str() else {
            return CasePayload::Unit;
        };
        match (key, scrutinee) {
            ("None", _) => CasePayload::Unit,
            ("Ok", Some(TypeExpr::Result { ok, .. })) => CasePayload::Carries(Some((**ok).clone())),
            ("Err", Some(TypeExpr::Result { error, .. })) => {
                CasePayload::Carries(Some((**error).clone()))
            }
            ("Some", Some(TypeExpr::Option(item))) => CasePayload::Carries(Some((**item).clone())),
            ("Ok" | "Err" | "Some", _) => CasePayload::Carries(None),
            (case, Some(TypeExpr::Named(named))) => {
                let leaf = case.rsplit('.').next().unwrap_or(case);
                match self.cx.member_type(&named.definition, leaf) {
                    Some(None) => CasePayload::Unit,
                    Some(Some(payload)) => CasePayload::Carries(Some(payload)),
                    None => CasePayload::Carries(None),
                }
            }
            _ => CasePayload::Carries(None),
        }
    }

    /// A `cond` target or a bracketed `br`/switch target.
    fn target_tpl(
        &mut self,
        st: &mut Lower,
        target: &Target,
        nest: bool,
        pointer: &str,
        case: Option<(&Value, &CasePayload)>,
    ) -> Tpl {
        let args = self.edge_args(st, target, nest, pointer, case);
        if args.is_empty() && target.shape == Shape::Name {
            return Tpl::Lit(Value::from(target.block.clone()));
        }
        let mut items = vec![Tpl::Lit(Value::from(target.block.clone()))];
        items.extend(args);
        Tpl::List(items)
    }

    #[allow(clippy::too_many_lines)]
    fn lower_term(&mut self, st: &mut Lower, block: &ABlock) {
        let pointer = format!("{}/term", block.pointer);
        let function = self.fn_name.clone();
        match &block.term {
            Term::Return(arg) => {
                let generated = self.term_name(st);
                let result = self.result.clone();
                let sym = self.lower_operand(st, arg, result.as_ref(), &generated, 0, true);
                let term = Tpl::List(vec![lit("return"), Tpl::Sym(sym)]);
                self.set_term(st.piece, term, &pointer, Role::Term, Vec::new());
            }
            Term::Ok(arg) => {
                self.stats.ok_fail += 1;
                let Some(TypeExpr::Result { ok, .. }) = self.result.clone() else {
                    self.oblige(
                        AgentErrorCode::XPropagation,
                        &pointer,
                        format!("[\"ok\", v] returns a Result, but `{function}` does not return one: use [\"return\", v]"),
                    );
                    return;
                };
                let generated = self.term_name(st);
                let sym = self.lower_operand(st, arg, Some(&ok), &generated, 0, true);
                let name = self.alloc_value(&format!("{}__ok", block.name));
                self.value_names.insert(name.clone(), pointer.clone());
                let op = EOp {
                    name: name.clone(),
                    word: "ok".to_owned(),
                    immediate: None,
                    operands: vec![sym],
                    ty: None,
                    authored: pointer.clone(),
                    role: Role::Term,
                    raw: None,
                };
                self.push_op(st, op);
                let term = Tpl::List(vec![
                    lit("return"),
                    Tpl::Sym(Sym::Local(name, String::new())),
                ]);
                self.set_term(st.piece, term, &pointer, Role::Term, Vec::new());
            }
            Term::Fail { case, payload } => {
                self.stats.ok_fail += 1;
                self.lower_fail(st, case.as_deref(), payload.as_ref(), &pointer);
            }
            Term::Br { word, target } => {
                let args = self.edge_args(st, target, true, &pointer, None);
                let term = match target.shape {
                    Shape::Flat => {
                        let mut items =
                            vec![lit(word), Tpl::Lit(Value::from(target.block.clone()))];
                        items.extend(args);
                        Tpl::List(items)
                    }
                    Shape::Bracket | Shape::Name => {
                        let mut inner = vec![Tpl::Lit(Value::from(target.block.clone()))];
                        inner.extend(args);
                        Tpl::List(vec![lit(word), Tpl::List(inner)])
                    }
                };
                self.set_term(
                    st.piece,
                    term,
                    &pointer,
                    Role::Term,
                    vec![target.block.clone()],
                );
            }
            Term::Cond { cond, then, other } => {
                let generated = self.term_name(st);
                let cond = self.lower_operand(st, cond, Some(&TypeExpr::Bool), &generated, 0, true);
                let first = self.target_tpl(st, then, false, &pointer, None);
                let second = self.target_tpl(st, other, false, &pointer, None);
                let term = Tpl::List(vec![lit("cond"), Tpl::Sym(cond), first, second]);
                self.set_term(
                    st.piece,
                    term,
                    &pointer,
                    Role::Term,
                    vec![then.block.clone(), other.block.clone()],
                );
            }
            Term::Switch { value, cases } => {
                let scrutinee = match &value.operand {
                    Operand::Name(text) => {
                        let (base, _) = split_suffix(text);
                        st.defs
                            .get(base)
                            .and_then(|def| def.ty.clone())
                            .or_else(|| self.value_type(st.b, text))
                    }
                    Operand::Nested(node) => self.node_types(st.b, node, None).value,
                    _ => None,
                };
                let generated = self.term_name(st);
                let value = self.lower_operand(st, value, None, &generated, 0, true);
                let mut items = vec![lit("switch"), Tpl::Sym(value)];
                let mut succ = Vec::new();
                for (key, target) in cases {
                    let payload = self.case_payload(key, scrutinee.as_ref());
                    let case_rule = Some((key, &payload));
                    let mut case = vec![Tpl::Lit(key.clone())];
                    match target.shape {
                        Shape::Flat | Shape::Name => {
                            case.push(Tpl::Lit(Value::from(target.block.clone())));
                            case.extend(self.edge_args(st, target, false, &pointer, case_rule));
                        }
                        Shape::Bracket => {
                            case.push(self.target_tpl(st, target, false, &pointer, case_rule));
                        }
                    }
                    items.push(Tpl::List(case));
                    succ.push(target.block.clone());
                }
                self.set_term(st.piece, Tpl::List(items), &pointer, Role::Term, succ);
            }
            Term::Trap { head, payload } => {
                let mut items: Vec<Tpl> = head.iter().cloned().map(Tpl::Lit).collect();
                if let Some(arg) = payload {
                    let generated = self.term_name(st);
                    let sym = self.lower_operand(st, arg, None, &generated, 0, true);
                    items.push(Tpl::Sym(sym));
                }
                self.set_term(st.piece, Tpl::List(items), &pointer, Role::Term, Vec::new());
            }
            Term::Raw(value) => {
                self.degraded = true;
                self.set_term(
                    st.piece,
                    Tpl::Lit(value.clone()),
                    &pointer,
                    Role::Term,
                    Vec::new(),
                );
            }
        }
    }

    fn lower_fail(
        &mut self,
        st: &mut Lower,
        case: Option<&str>,
        payload: Option<&Arg>,
        pointer: &str,
    ) {
        let function = self.fn_name.clone();
        let Some(case) = case else {
            if matches!(self.result, Some(TypeExpr::Option(_))) && payload.is_none() {
                let exit = self.shared(ExitKind::None, pointer);
                let term = Tpl::List(vec![lit("br"), lit(&exit)]);
                self.set_term(st.piece, term, pointer, Role::Term, vec![exit]);
            } else {
                self.oblige(
                    AgentErrorCode::XPropagation,
                    pointer,
                    format!("a bare [\"fail\"] returns none and needs a function that returns an Option; `{function}` fails through its error cases: [\"fail\", \"Case\"]"),
                );
            }
            return;
        };
        let Some(case_payload) = self.error_case(case) else {
            let block = if self.is_block(case) {
                format!("; to go to block `{case}`, use [\"br\", \"{case}\"]")
            } else {
                String::new()
            };
            let variant = self.error_variant().map_or_else(
                || "`{function}` has no variant error type".to_owned(),
                |(variant, cases)| {
                    format!(
                        "the cases of {variant} are {}",
                        cases
                            .iter()
                            .map(|(leaf, _)| leaf.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                },
            );
            self.oblige(
                AgentErrorCode::XPropagation,
                pointer,
                format!(
                    "`{case}` is not a case of the error type of `{function}` ({variant}){block}"
                ),
            );
            return;
        };
        let exit = self.shared_fail(case, case_payload.as_ref(), pointer);
        let mut items = vec![lit("br"), lit(&exit)];
        match (case_payload, payload) {
            (None, None) => {}
            (None, Some(_)) => {
                self.oblige(
                    AgentErrorCode::XPropagation,
                    pointer,
                    format!("case `{case}` carries no payload: write [\"fail\", \"{case}\"]"),
                );
                return;
            }
            (Some(expected), None) => {
                let expected = self.cx.render(&expected);
                self.oblige(
                    AgentErrorCode::XPropagation,
                    pointer,
                    format!(
                        "case `{case}` carries a {expected}: write [\"fail\", \"{case}\", payload]"
                    ),
                );
                return;
            }
            (Some(expected), Some(arg)) => {
                let generated = self.term_name(st);
                let Some(sym) = self.payload(st, arg, Some(&expected), &generated, true, case)
                else {
                    return;
                };
                items.push(Tpl::Sym(sym));
            }
        }
        self.set_term(st.piece, Tpl::List(items), pointer, Role::Term, vec![exit]);
    }

    // --- control flow, threading, emission --------------------------------

    /// Immediate dominators over the expanded blocks (pieces, shared exits,
    /// kept live blocks), computed by the iterative algorithm and bounded by
    /// the block count.
    #[allow(clippy::too_many_lines)]
    fn dominators(&mut self) -> Cfg {
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        let mut succ_names: Vec<Vec<String>> = Vec::new();
        for piece in &self.pieces {
            index.insert(piece.name.clone(), succ_names.len());
            succ_names.push(piece.succ.clone());
        }
        for exit in &self.exits {
            index.insert(exit.name.clone(), succ_names.len());
            succ_names.push(Vec::new());
        }
        for (leaf, targets) in &self.live_exits {
            if !index.contains_key(leaf) {
                index.insert(leaf.clone(), succ_names.len());
                succ_names.push(targets.clone());
            }
        }
        for kept in &self.kept {
            index.insert(kept.leaf.clone(), succ_names.len());
            succ_names.push(kept.targets.clone());
        }
        let count = succ_names.len();
        let succ: Vec<Vec<usize>> = succ_names
            .iter()
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| index.get(name).copied())
                    .collect()
            })
            .collect();
        let mut idom: Vec<Option<usize>> = vec![None; count];
        let (owner, values, definers, node_names) = self.node_tables(&index, count);
        let Some(&entry) = index.get(&self.entry) else {
            return Cfg {
                index,
                idom,
                owner,
                values,
                definers,
                node_names,
            };
        };
        // Reverse postorder by an explicit stack.
        let mut order = Vec::new();
        let mut seen = vec![false; count];
        let mut stack: Vec<(usize, usize)> = vec![(entry, 0)];
        seen[entry] = true;
        while let Some((node, next)) = stack.pop() {
            if let Some(&child) = succ[node].get(next) {
                stack.push((node, next + 1));
                if !seen[child] {
                    seen[child] = true;
                    stack.push((child, 0));
                }
            } else {
                order.push(node);
            }
        }
        order.reverse();
        let mut rank = vec![usize::MAX; count];
        for (position, node) in order.iter().enumerate() {
            rank[*node] = position;
        }
        let mut preds: Vec<Vec<usize>> = vec![Vec::new(); count];
        for (node, targets) in succ.iter().enumerate() {
            if seen[node] {
                for target in targets {
                    preds[*target].push(node);
                }
            }
        }
        idom[entry] = Some(entry);
        let mut settled = false;
        for _ in 0..=count + 1 {
            let mut changed = false;
            for &node in order.iter().skip(1) {
                let mut new = None;
                for &pred in &preds[node] {
                    if idom[pred].is_none() {
                        continue;
                    }
                    new = Some(match new {
                        None => pred,
                        Some(current) => intersect(&idom, &rank, pred, current),
                    });
                }
                if new.is_some() && idom[node] != new {
                    idom[node] = new;
                    changed = true;
                }
            }
            if !changed {
                settled = true;
                break;
            }
        }
        if !settled {
            let pointer = self.fn_pointer.clone();
            self.oblige(
                AgentErrorCode::XLimit,
                &pointer,
                format!("dominance did not settle within {count} passes over the expanded blocks"),
            );
        }
        Cfg {
            index,
            idom,
            owner,
            values,
            definers,
            node_names,
        }
    }

    /// Per expanded node: its owner block, the values it defines and its
    /// name; and every block defining each name.
    #[allow(clippy::type_complexity)]
    fn node_tables(
        &self,
        index: &BTreeMap<String, usize>,
        count: usize,
    ) -> (
        Vec<Option<String>>,
        Vec<BTreeMap<String, (Kind, Option<TypeExpr>)>>,
        BTreeMap<String, Vec<String>>,
        Vec<String>,
    ) {
        let mut owner: Vec<Option<String>> = vec![None; count];
        let mut values: Vec<BTreeMap<String, (Kind, Option<TypeExpr>)>> =
            vec![BTreeMap::new(); count];
        let mut definers: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut node_names = vec![String::new(); count];
        for (name, node) in index {
            node_names[*node].clone_from(name);
        }
        for (b, defs) in self.block_defs.iter().enumerate() {
            let block = &self.blocks[b].name;
            for &piece in &self.block_pieces[b] {
                owner[piece] = Some(block.clone());
            }
            for (name, def) in defs {
                values[def.piece].insert(name.clone(), (def.kind, def.ty.clone()));
                definers
                    .entry(name.clone())
                    .or_default()
                    .push(block.clone());
            }
        }
        for kept in &self.kept {
            let node = index[&kept.leaf];
            owner[node] = Some(kept.leaf.clone());
            for (name, ty) in &kept.params {
                values[node].insert(name.clone(), (Kind::Param, ty.clone()));
                definers
                    .entry(name.clone())
                    .or_default()
                    .push(kept.leaf.clone());
            }
            for (name, ty) in &kept.ops {
                values[node].insert(name.clone(), (Kind::Op, ty.clone()));
                definers
                    .entry(name.clone())
                    .or_default()
                    .push(kept.leaf.clone());
            }
        }
        (owner, values, definers, node_names)
    }

    /// Threads block parameters and unwrapped values into the continuation
    /// pieces that use them, in first-use order.
    fn thread(&mut self) {
        for b in 0..self.blocks.len() {
            let pieces = self.block_pieces[b].clone();
            for (position, &piece) in pieces.iter().enumerate().skip(1) {
                let mut threaded: Vec<(String, Value)> = Vec::new();
                for &later in &pieces[position..] {
                    for name in &self.pieces[later].uses {
                        let Some(def) = self.block_defs[b].get(name) else {
                            continue;
                        };
                        if def.kind == Kind::Op
                            || self.pieces[def.piece].local >= position
                            || threaded.iter().any(|(threaded, _)| threaded == name)
                        {
                            continue;
                        }
                        threaded.push((
                            name.clone(),
                            def.ty_json.clone().unwrap_or_else(|| Value::from("unit")),
                        ));
                    }
                }
                self.pieces[piece].threaded = threaded;
            }
        }
    }

    fn resolve_sym(&mut self, piece: usize, sym: &Sym, pointer: &str) -> Value {
        match sym {
            Sym::Local(name, suffix) => {
                let b = self.pieces[piece].block;
                match self.block_defs[b].get(name) {
                    Some(def) if def.kind == Kind::Op && def.piece != piece => {
                        Value::from(format!("{}.{name}{suffix}", self.pieces[def.piece].name))
                    }
                    _ => Value::from(format!("{name}{suffix}")),
                }
            }
            Sym::Plain(text) => Value::from(text.clone()),
            Sym::Raw(value) => value.clone(),
            Sym::Qualified {
                block,
                leaf,
                suffix,
                text,
                pointer: authored,
            } => match self.block_defs[*block].get(leaf) {
                Some(def) if def.kind == Kind::Op => {
                    let qualified = format!("{}.{leaf}{suffix}", self.pieces[def.piece].name);
                    if qualified != *text {
                        self.entries.push(MapEntry {
                            expanded: pointer.to_owned(),
                            authored: authored.clone(),
                            role: Role::Qualified,
                            name: qualified.clone(),
                        });
                    }
                    Value::from(qualified)
                }
                Some(_) => {
                    let owner = self.blocks[*block].name.clone();
                    let here = self.blocks[self.pieces[piece].block].name.clone();
                    self.oblige(
                        AgentErrorCode::XScope,
                        authored,
                        format!(
                            "`{text}` is a parameter of block `{owner}` (or the value a checked operation unwraps there), visible only in its own block: declare `{leaf}` as a parameter of `{here}`; its edge argument is derived"
                        ),
                    );
                    Value::from(text.clone())
                }
                None => Value::from(text.clone()),
            },
            Sym::Far(far) => self.resolve_far(piece, far, pointer),
        }
    }

    /// X4: a plain name that no earlier value of the block and no function
    /// parameter defines means the nearest definition above this point on
    /// the dominator tree of the expanded graph, when that is an operation
    /// result; a derived edge argument is also never taken on an edge back
    /// into a loop.
    #[allow(clippy::too_many_lines)]
    fn resolve_far(&mut self, piece: usize, far: &Far, pointer: &str) -> Value {
        let b = self.pieces[piece].block;
        let here = self.blocks[b].name.clone();
        let name = &far.name;
        let fallback = Value::from(format!("{name}{}", far.suffix));
        let cfg = self.cfg.take().unwrap_or_default();
        let use_node = cfg.index.get(&self.pieces[piece].name).copied();
        let value = self.resolve_far_in(&cfg, use_node, &here, far, piece, pointer, fallback);
        self.cfg = Some(cfg);
        value
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn resolve_far_in(
        &mut self,
        cfg: &Cfg,
        use_node: Option<usize>,
        here: &str,
        far: &Far,
        piece: usize,
        pointer: &str,
        fallback: Value,
    ) -> Value {
        let name = &far.name;
        let derive = far.derive.as_ref();
        let fail = |this: &mut Self, detail: String| match derive {
            Some(derive) => this.missing_argument(
                &far.pointer,
                &derive.target,
                name,
                derive.ty.as_ref(),
                derive.available.clone(),
                &detail,
            ),
            None => this.oblige(AgentErrorCode::XScope, &far.pointer, detail),
        };
        let mismatch = |found: Option<&TypeExpr>| {
            derive
                .and_then(|derive| derive.ty.as_ref())
                .zip(found)
                .is_some_and(|(want, found)| want != found)
        };
        let reachable = use_node.is_some_and(|node| cfg.reachable(node));
        if let Some(derive) = derive {
            // An edge into a block that dominates the edge's own block is an
            // edge back into a loop: the loop's values are passed explicitly.
            let back = cfg
                .index
                .get(&derive.target)
                .zip(use_node)
                .is_some_and(|(target, node)| cfg.dominates(*target, node));
            if back && !self.degraded {
                let target = &derive.target;
                fail(
                    self,
                    format!(
                        "this edge goes back into `{target}`, which it is inside of (a loop edge), and a loop edge never takes an omitted argument by name"
                    ),
                );
                return Value::Null;
            }
            if let Some((local, ty)) = &derive.local {
                if mismatch(ty.as_ref()) && !self.degraded {
                    let found = ty.as_ref().map(|t| self.cx.render(t)).unwrap_or_default();
                    fail(self, format!("the `{name}` here is {found}"));
                    return Value::Null;
                }
                self.stats.derived_args += 1;
                return self.resolve_sym(piece, local, pointer);
            }
        }
        if self.degraded
            && let Some(holder) = self.unique_op_holder(name, here)
            && !matches!(
                cfg.nearest(use_node.unwrap_or_default(), here, name),
                Nearest::Op { .. }
            )
        {
            // The graph is incomplete: the compiler reports the malformed
            // part; a unique result keeps this name from hiding it.
            return Value::from(format!("{holder}.{name}{}", far.suffix));
        }
        if !reachable {
            if self.degraded {
                return fallback;
            }
            fail(
                self,
                format!(
                    "block `{here}` is not reachable from the entry, so no value named `{name}` is available here"
                ),
            );
            return if derive.is_some() {
                Value::Null
            } else {
                fallback
            };
        }
        let nearest = cfg.nearest(use_node.unwrap_or_default(), here, name);
        let others: Vec<String> = cfg
            .definers
            .get(name)
            .map(|owners| {
                let target = derive.map(|derive| derive.target.as_str());
                let mut owners: Vec<String> = owners
                    .iter()
                    .filter(|owner| owner.as_str() != here && Some(owner.as_str()) != target)
                    .cloned()
                    .collect();
                owners.dedup();
                owners
            })
            .unwrap_or_default();
        match nearest {
            Nearest::Op { owner, holder, ty } => {
                if mismatch(ty.as_ref()) && !self.degraded {
                    let found = ty.as_ref().map(|t| self.cx.render(t)).unwrap_or_default();
                    fail(self, format!("the `{name}` of `{owner}` is {found}"));
                    return Value::Null;
                }
                let role = if derive.is_some() {
                    self.stats.derived_args += 1;
                    Role::DerivedArg
                } else {
                    self.stats.qualified += 1;
                    Role::Qualified
                };
                self.entries.push(MapEntry {
                    expanded: pointer.to_owned(),
                    authored: far.pointer.clone(),
                    role,
                    name: format!("{holder}.{name}"),
                });
                Value::from(format!("{holder}.{name}{}", far.suffix))
            }
            _ if self.degraded => {
                if derive.is_some() {
                    Value::from(name.clone())
                } else {
                    fallback
                }
            }
            Nearest::Hidden { owner, kind, outer } => {
                let what = if kind == Kind::Unwrapped {
                    format!("the value a checked operation of block `{owner}` unwraps")
                } else {
                    format!("a parameter of block `{owner}`")
                };
                let detail = match (&outer, derive) {
                    (Some(outer), None) => format!(
                        "`{name}` here is shadowed: the nearest `{name}` above `{here}` is {what}, visible only there; write `{outer}.{name}` for the value `{outer}` defines, or declare `{name}` as a parameter of `{here}` and pass it on each edge"
                    ),
                    (None, None) => format!(
                        "`{name}` is {what}, visible only in its own block: declare `{name}` as a parameter of `{here}`; its edge argument is derived"
                    ),
                    (_, Some(_)) => format!(
                        "the nearest `{name}` above this edge is {what}, visible only there"
                    ),
                };
                fail(self, detail);
                if derive.is_some() {
                    Value::Null
                } else {
                    fallback
                }
            }
            Nearest::None => {
                if others.is_empty() {
                    if derive.is_some() {
                        fail(self, format!("no value named `{name}` is available here"));
                        return Value::Null;
                    }
                    // The compiler names the unknown value as written.
                    return fallback;
                }
                let detail = match (others.as_slice(), derive) {
                    ([owner], None) => format!(
                        "`{name}` is defined in block `{owner}`, which does not dominate this point of `{here}` (another path reaches it without passing through `{owner}`): declare `{name}` as a parameter of `{here}` and pass it on each edge"
                    ),
                    (owners, None) => format!(
                        "`{name}` is defined in blocks {}, none of which dominates this point of `{here}` (paths join here): declare `{name}` as a parameter of `{here}`; each edge then derives its own",
                        owners.join(", ")
                    ),
                    (owners, Some(_)) => format!(
                        "no value named `{name}` is available on every path here (blocks {} define one, but none dominates this edge)",
                        owners.join(", ")
                    ),
                };
                fail(self, detail);
                if derive.is_some() {
                    Value::Null
                } else {
                    fallback
                }
            }
        }
    }

    /// The node holding the only operation result named `name` outside
    /// block `here`, when exactly one block defines it (degraded mode).
    fn unique_op_holder(&self, name: &str, here: &str) -> Option<String> {
        let mut found = Vec::new();
        for (b, defs) in self.block_defs.iter().enumerate() {
            if self.blocks[b].name != here
                && let Some(def) = defs.get(name)
            {
                found.push((def.kind, self.pieces[def.piece].name.clone()));
            }
        }
        for kept in &self.kept {
            if let Some((kind, _)) = kept.value(name) {
                found.push((kind, kept.leaf.clone()));
            }
        }
        match found.as_slice() {
            [(Kind::Op, holder)] => Some(holder.clone()),
            _ => None,
        }
    }

    fn emit_tpl(&mut self, piece: usize, tpl: &Tpl, pointer: &str) -> Value {
        match tpl {
            Tpl::Lit(value) => value.clone(),
            Tpl::Sym(sym) => self.resolve_sym(piece, sym, pointer),
            Tpl::List(items) => {
                let mut out = Vec::new();
                for item in items {
                    if let Tpl::Threaded(next) = item {
                        for (name, _) in self.pieces[*next].threaded.clone() {
                            out.push(Value::from(name));
                        }
                        continue;
                    }
                    let child = format!("{pointer}/{}", out.len());
                    out.push(self.emit_tpl(piece, item, &child));
                }
                Value::Array(out)
            }
            Tpl::Threaded(_) => Value::Null,
        }
    }

    fn emit_op(&mut self, piece: usize, op: &EOp, pointer: &str) -> Value {
        if let Some(raw) = &op.raw {
            return raw.clone();
        }
        let offset = usize::from(op.immediate.is_some());
        let mut args: Vec<Value> = op.immediate.iter().cloned().collect();
        for (position, sym) in op.operands.iter().enumerate() {
            let at = if op.ty.is_some() {
                format!("{pointer}/args/{}", offset + position)
            } else {
                format!("{pointer}/{}", 2 + offset + position)
            };
            args.push(self.resolve_sym(piece, sym, &at));
        }
        if let Some(ty) = &op.ty {
            return json!({"name": op.name, "op": op.word, "args": args, "type": ty});
        }
        let mut items = vec![Value::from(op.name.clone()), Value::from(op.word.clone())];
        items.extend(args);
        Value::Array(items)
    }

    fn emit_piece(&mut self, index: usize, pointer: &str) -> Value {
        let piece = self.pieces[index].clone();
        let authored = &self.blocks[piece.block].object;
        let mut block = Map::new();
        if !self.patch {
            block.insert("name".to_owned(), Value::from(piece.name.clone()));
        }
        if piece.role == Role::Block {
            for key in ["params", "comment"] {
                if let Some(value) = authored.get(key) {
                    block.insert(key.to_owned(), value.clone());
                }
            }
        } else {
            let params: Vec<Value> = piece
                .unwrapped
                .iter()
                .chain(&piece.threaded)
                .map(|(name, ty)| json!([name, ty]))
                .collect();
            if !params.is_empty() {
                block.insert("params".to_owned(), Value::Array(params));
            }
        }
        if let Some(flag) = authored.get("unreachable") {
            block.insert("unreachable".to_owned(), flag.clone());
        }
        let mut ops = Vec::new();
        for (position, op) in piece.ops.iter().enumerate() {
            let at = format!("{pointer}/ops/{position}");
            ops.push(self.emit_op(index, op, &at));
            self.entries.push(MapEntry {
                expanded: at,
                authored: op.authored.clone(),
                role: op.role,
                name: op.name.clone(),
            });
        }
        block.insert("ops".to_owned(), Value::Array(ops));
        let at = format!("{pointer}/term");
        let term = self.emit_tpl(index, &piece.term, &at);
        block.insert("term".to_owned(), term);
        self.entries.push(MapEntry {
            expanded: pointer.to_owned(),
            authored: piece.authored.clone(),
            role: piece.role,
            name: piece.name.clone(),
        });
        self.entries.push(MapEntry {
            expanded: at,
            authored: piece.term_authored.clone(),
            role: piece.term_role,
            name: piece.name.clone(),
        });
        if piece.role == Role::Continuation
            && let Some((name, _)) = &piece.unwrapped
            && name.contains("__")
        {
            self.value_names
                .insert(name.clone(), piece.authored.clone());
        }
        Value::Object(block)
    }

    fn emit_exit(&mut self, index: usize, pointer: &str) -> Value {
        let exit = self.exits[index].clone();
        let mut block = Map::new();
        if !self.patch {
            block.insert("name".to_owned(), Value::from(exit.name.clone()));
        }
        let (params, ops): (Vec<Value>, Vec<Value>) = match &exit.kind {
            ExitKind::Fail {
                case,
                variant,
                payload,
            } => {
                let member = format!("{variant}.{case}");
                match payload {
                    Some(ty) => (
                        vec![json!(["p", ty])],
                        vec![
                            json!(["v", "variant", member, "p"]),
                            json!(["r", "err", "v"]),
                        ],
                    ),
                    None => (
                        Vec::new(),
                        vec![json!(["v", "variant", member]), json!(["r", "err", "v"])],
                    ),
                }
            }
            ExitKind::Err(ty) => (vec![json!(["e", ty])], vec![json!(["r", "err", "e"])]),
            ExitKind::None => (Vec::new(), vec![json!(["r", "none"])]),
        };
        if !params.is_empty() {
            block.insert("params".to_owned(), Value::Array(params));
        }
        for position in 0..ops.len() {
            self.entries.push(MapEntry {
                expanded: format!("{pointer}/ops/{position}"),
                authored: exit.authored.clone(),
                role: Role::SharedExit,
                name: exit.name.clone(),
            });
        }
        block.insert("ops".to_owned(), Value::Array(ops));
        block.insert("term".to_owned(), json!(["return", "r"]));
        self.entries.push(MapEntry {
            expanded: pointer.to_owned(),
            authored: exit.authored,
            role: Role::SharedExit,
            name: exit.name,
        });
        Value::Object(block)
    }

    fn check_bounds(&mut self) {
        let ops: usize = self
            .pieces
            .iter()
            .map(|piece| piece.ops.len())
            .sum::<usize>()
            + self.exits.len() * 2;
        let generated = self.pieces.len() - self.blocks.len() + self.exits.len();
        let pointer = self.fn_pointer.clone();
        if ops > MAX_EXPANDED_OPS_PER_FUNCTION {
            self.oblige(
                AgentErrorCode::XLimit,
                &pointer,
                format!(
                    "the expanded function has {ops} operations, more than the bound of {MAX_EXPANDED_OPS_PER_FUNCTION}: split it into several functions"
                ),
            );
        }
        if generated > MAX_GENERATED_BLOCKS_PER_FUNCTION {
            self.oblige(
                AgentErrorCode::XLimit,
                &pointer,
                format!(
                    "the expanded function has {generated} generated blocks, more than the bound of {MAX_GENERATED_BLOCKS_PER_FUNCTION}: split it into several functions"
                ),
            );
        }
    }
}

fn intersect(idom: &[Option<usize>], rank: &[usize], mut a: usize, mut b: usize) -> usize {
    let limit = idom.len() + 1;
    for _ in 0..limit * 2 {
        if a == b {
            return a;
        }
        while rank[a] > rank[b] {
            match idom[a] {
                Some(up) if up != a => a = up,
                _ => return b,
            }
        }
        while rank[b] > rank[a] {
            match idom[b] {
                Some(up) if up != b => b = up,
                _ => return a,
            }
        }
    }
    a
}

fn collect_uses(tpl: &Tpl, out: &mut Vec<String>) {
    match tpl {
        Tpl::Sym(Sym::Local(name, _)) => out.push(name.clone()),
        Tpl::Sym(Sym::Far(far)) => {
            if let Some(Derive {
                local: Some((local, _)),
                ..
            }) = &far.derive
                && let Sym::Local(name, _) = &**local
            {
                out.push(name.clone());
            }
        }
        Tpl::List(items) => items.iter().for_each(|item| collect_uses(item, out)),
        _ => {}
    }
}

/// `x#1` into (`x`, `#1`).
fn split_suffix(text: &str) -> (&str, &str) {
    match text.find('#') {
        Some(at) if at > 0 => (&text[..at], &text[at..]),
        _ => (text, ""),
    }
}

/// Operand positions that share one type.
fn same_type_groups(tag: u32, count: usize) -> Vec<Vec<usize>> {
    match tag {
        64..=68 | 80..=83 | 85 | 96..=101 | 32 => vec![(0..count).collect()],
        36 => vec![
            (0..count).step_by(2).collect(),
            (1..count).step_by(2).collect(),
        ],
        _ => Vec::new(),
    }
}

/// Operand contexts the first operand's type fixes (element, key, value
/// and cell item types).
fn dependent_contexts(tag: u32, types: &[Option<TypeExpr>], contexts: &mut [Option<TypeExpr>]) {
    let Some(Some(first)) = types.first() else {
        return;
    };
    let mut set = |position: usize, ty: &TypeExpr| {
        if let Some(slot) = contexts.get_mut(position)
            && slot.is_none()
        {
            *slot = Some(ty.clone());
        }
    };
    match (tag, first) {
        (35, TypeExpr::Vector(item)) => set(2, item),
        (37 | 38 | 40, TypeExpr::OrderedMap { key, .. }) => set(1, key),
        (39, TypeExpr::OrderedMap { key, value }) => {
            set(1, key);
            set(2, value);
        }
        (178, TypeExpr::LocalCell(item)) => set(1, item),
        _ => {}
    }
}

fn fill_from_partners(
    groups: &[Vec<usize>],
    types: &[Option<TypeExpr>],
    contexts: &mut [Option<TypeExpr>],
) {
    for group in groups {
        let known = group
            .iter()
            .find_map(|position| types.get(*position).cloned().flatten());
        if let Some(known) = known {
            for position in group {
                if let Some(slot) = contexts.get_mut(*position)
                    && slot.is_none()
                {
                    *slot = Some(known.clone());
                }
            }
        }
    }
}

/// A name longer than the grammar allows: a prefix and a digest of the
/// whole name (still derived from the authored path alone).
fn shorten(name: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in name.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let cut = name
        .char_indices()
        .map(|(at, _)| at)
        .take_while(|at| *at <= 40)
        .last()
        .unwrap_or(0);
    format!("{}__h{hash:016x}", &name[..cut])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_generated_names_stay_within_the_grammar() {
        let long = format!("{}__a0", "x".repeat(70));
        let short = shorten(&long);
        assert!(short.len() <= MAX_NAME, "{short}");
        assert!(is_identifier(&short));
        assert_eq!(short, shorten(&long), "stable");
        assert_ne!(short, shorten(&format!("{long}_2")));
    }

    #[test]
    fn pointers_map_by_longest_prefix() {
        let map = SourceMap {
            entries: vec![
                MapEntry {
                    expanded: "/fns/0/blocks/2".into(),
                    authored: "/fns/0/blocks/0/ops/1".into(),
                    role: Role::Continuation,
                    name: "entry__x".into(),
                },
                MapEntry {
                    expanded: "/fns/0/blocks/2/ops/0".into(),
                    authored: "/fns/0/blocks/0/ops/2".into(),
                    role: Role::Op,
                    name: "y".into(),
                },
                MapEntry {
                    expanded: "/fns/0/blocks/0".into(),
                    authored: "/fns/0/blocks/0".into(),
                    role: Role::Block,
                    name: "entry".into(),
                },
            ],
            ..SourceMap::default()
        };
        assert_eq!(
            map.authored("/fns/0/blocks/2/ops/0/3").as_deref(),
            Some("/fns/0/blocks/0/ops/2/3")
        );
        assert_eq!(
            map.authored("/fns/0/blocks/2/params/0").as_deref(),
            Some("/fns/0/blocks/0/ops/1")
        );
        assert_eq!(
            map.authored("/fns/0/blocks/0/params/1").as_deref(),
            Some("/fns/0/blocks/0/params/1")
        );
        assert_eq!(map.authored("/fns/0/blocks/20"), None, "segment boundary");
        let error = AgentError::new(
            AgentErrorCode::FrameInvalid,
            "/fns/0/blocks/2/ops/0: bad (1 of 2 problems)\n  /types/0: worse",
        );
        assert_eq!(
            map.rewrite(&error).detail(),
            "/fns/0/blocks/0/ops/2: bad [expanded /fns/0/blocks/2/ops/0] (1 of 2 problems)\n  /types/0: worse"
        );
    }

    #[test]
    fn refusals_carry_each_problems_symbol() {
        let obligations = vec![
            Obligation::new(AgentErrorCode::XScope, "/a", "first"),
            Obligation::new(AgentErrorCode::FrameInvalid, "/b", "second"),
            Obligation::new(AgentErrorCode::XScope, "/a", "first"),
        ];
        let error = refusal(&obligations);
        assert_eq!(error.code(), AgentErrorCode::XScope);
        assert_eq!(
            error.detail(),
            "/a: first (1 of 2 problems)\n  [AGENT_FRAME_INVALID] /b: second"
        );
    }

    #[test]
    fn edit_operations_in_the_dialect_are_detected() {
        assert!(!is_extended_op(&json!(["const", 7])));
        assert!(!is_extended_op(&json!(["add", "a", "b"])));
        assert!(is_extended_op(&json!(["add", "a", 1])));
        assert!(is_extended_op(&json!(["add?", "a", "b"])));
        assert!(is_extended_op(
            &json!({"op": "add", "args": ["a", ["mul", "a", "a"]]})
        ));
        assert!(!is_extended_op(&json!({"op": "call", "args": ["g", "a"]})));
    }
}

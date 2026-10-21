//! Structured function bodies: an AF1-X function may carry `"body"` (a list
//! of statements) instead of `"blocks"`. The lowering here constructs the
//! blocks, loop back edges, index plumbing and state forwarding, and emits an
//! ordinary AF1-X function that the unchanged expander, compiler and kernel
//! then handle. It never changes what a program means: every construct maps
//! onto existing operations and terminators.
//!
//! Statements: `["let", x, e]`, `["var", x, T, e]` (or `["var", x, e]`, typed
//! like `let`), `["set", x, e]`,
//! `["if", c, [..], [..]?]`, `["for", x, xs, [..]]`, `["while", c, [..]]`,
//! `["range", i, start, end, [..]]`, `["for-indexed", i, x, xs, [..]]`,
//! `["return", e]`, `["ok", e]`, `["fail", Case]`, `["trap"]`, `["break"]` and
//! `["continue"]` (the innermost loop: leave it, or go to its next pass).
//!
//! Expressions: a name, an integer, `true`/`false`, a typed literal
//! `{"type": T, "value": v}`, or `[op, args..]`:
//! - `add sub mul div rem neg abs`: checked; a failure traps, unless the
//!   operation is spelled `op?` (return the `ArithmeticError` from a Result
//!   function) or `op?Case` (return error case `Case`).
//! - `floordiv floormod`: Python's `//` and `%` (the quotient rounds toward
//!   negative infinity; the remainder takes the divisor's sign); checked like
//!   `div` and `rem`.
//! - `to`: `["to", T, x]` converts between integer types; a value outside
//!   `T` fails like a checked operation (`to?`, `to?Case`).
//! - `min max clamp`: total; `clamp(x, lo, hi)` is `min(max(x, lo), hi)`.
//! - `eq ne lt le gt ge not`; `and or` evaluate both operands.
//! - `if`: `["if", c, a, b]` computes only the chosen value.
//! - `fold`: `["fold", kind, x, xs, e, c?]` visits every element `x` of `xs`
//!   (where `c` holds, when given): `sum` adds `e` with checked addition,
//!   `count` counts the elements where the bool `e` holds (an `i64`), `any`
//!   and `all` combine the bool `e` with `or` and `and`, `max` and `min`
//!   keep the largest or smallest `e` (no element is a trap). `xs` may also
//!   be `["range", start, end]`: `x` takes each integer of the half-open
//!   range. Every element is visited; `e` and `c` are evaluated for each
//!   element they reach.
//! - `truthy`: a bool as itself, an integer `!= 0`, a vector or text that is
//!   not empty (the Python dialect's conditions).
//! - `pow`: `["pow", a, b]` multiplies `a` by itself `b` times (checked; a
//!   negative `b` traps). `gcd`: the greatest common divisor of the absolute
//!   values (`gcd(0, 0)` is 0).
//! - `len` (u64), `get` (`[xs, u64]`; out of range traps or `get?Case`),
//!   `field` (`[record, "name"]`), `call` (`["f", args..]`; `call?` unwraps a
//!   Result).
//!
//! Integer literals take the type of the other operand, the declared
//! variable or the return type, else `i64`. Evaluation is left to right;
//! `for` visits elements in order.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::afx::{Context, Obligation};
use crate::error::AgentErrorCode;
use crate::types::TypeNames;

mod truth_tests;

const INTS: [&str; 10] = [
    "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128",
];
const COMPARE: [&str; 6] = ["eq", "ne", "lt", "le", "gt", "ge"];

/// A body function lowered to blocks, and where its pieces came from.
pub(crate) struct Lowered {
    /// The frame with every `body` function replaced by a `blocks` function.
    pub(crate) frame: Map<String, Value>,
    /// Lowered pointer to authored pointer, for every generated operation
    /// and terminator.
    pub(crate) origins: Vec<(String, String)>,
}

/// Whether the frame needs structured bodies or truth assertions lowered.
pub(crate) fn uses_bodies(frame: &Map<String, Value>) -> bool {
    ["fns", "functions", "patch"].iter().any(|key| {
        frame
            .get(*key)
            .and_then(Value::as_array)
            .is_some_and(|list| list.iter().any(|f| f.get("body").is_some()))
    }) || frame
        .get("tests")
        .and_then(Value::as_array)
        .is_some_and(|tests| tests.iter().any(|test| test.get("truthy").is_some()))
}

/// Lowers every `body` function of the frame.
///
/// # Errors
///
/// One obligation per function that cannot be lowered, at the authored
/// location of the first problem.
pub(crate) fn lower(
    cx: &Context<'_>,
    frame: &Map<String, Value>,
) -> Result<Lowered, Vec<Obligation>> {
    let prepared = truth_tests::lower(cx, frame)?;
    let frame = &prepared.frame;
    let mut out = frame.clone();
    let mut origins = prepared.origins.clone();
    let mut problems = Vec::new();
    let signatures = frame_signatures(frame);
    let records = frame_records(frame);
    if let Some(list) = frame.get("patch").and_then(Value::as_array) {
        for (index, patch) in list.iter().enumerate() {
            if patch.get("body").is_some() {
                problems.push(Obligation::new(
                    AgentErrorCode::FrameInvalid,
                    &format!("/patch/{index}/body"),
                    "patch restates blocks; restate a function with a body in fns",
                ));
            }
        }
    }
    for key in ["fns", "functions"] {
        let Some(list) = frame.get(key).and_then(Value::as_array) else {
            continue;
        };
        let mut lowered = Vec::with_capacity(list.len());
        for (index, decl) in list.iter().enumerate() {
            let at = format!("/{key}/{index}");
            if decl.get("body").is_none() {
                lowered.push(decl.clone());
                continue;
            }
            match Fn::new(cx, &signatures, &records, decl, &at).and_then(|mut f| {
                let blocks = f.lower()?;
                Ok((blocks, f.origins))
            }) {
                Ok((blocks, fn_origins)) => {
                    let mut object = decl.as_object().cloned().unwrap_or_default();
                    object.remove("body");
                    object.insert("blocks".to_owned(), Value::Array(blocks));
                    lowered.push(Value::Object(object));
                    for ((block, part), where_) in fn_origins {
                        origins.push((
                            format!("{at}/blocks/{block}/{part}"),
                            authored(&prepared.origins, &where_),
                        ));
                    }
                }
                Err(failure) => {
                    problems.push(Obligation::new(
                        AgentErrorCode::FrameInvalid,
                        &authored(&prepared.origins, &failure.at),
                        failure.message,
                    ));
                    lowered.push(decl.clone());
                }
            }
        }
        out.insert(key.to_owned(), Value::Array(lowered));
    }
    if problems.is_empty() {
        Ok(Lowered {
            frame: out,
            origins,
        })
    } else {
        Err(problems)
    }
}

/// The authored pointer for a pointer into the lowered frame: the longest
/// recorded prefix; unchanged when none covers it.
pub(crate) fn authored(origins: &[(String, String)], pointer: &str) -> String {
    let mut best: Option<&(String, String)> = None;
    for entry in origins {
        let covers = pointer == entry.0
            || (pointer.starts_with(&entry.0) && pointer[entry.0.len()..].starts_with('/'));
        if covers && best.is_none_or(|b| entry.0.len() > b.0.len()) {
            best = Some(entry);
        }
    }
    best.map_or_else(|| pointer.to_owned(), |entry| entry.1.clone())
}

fn normal(ty: &str) -> String {
    ty.chars().filter(|c| !c.is_whitespace()).collect()
}

fn is_int(ty: &str) -> bool {
    INTS.contains(&ty)
}

fn signed(ty: &str) -> bool {
    ty.starts_with('i')
}

fn vec_elem(ty: &str) -> Option<String> {
    ty.strip_prefix("Vec<")?
        .strip_suffix('>')
        .map(str::to_owned)
}

fn result_parts(ty: &str) -> Option<(String, String)> {
    let inner = ty.strip_prefix("Result<")?.strip_suffix('>')?;
    let (ok, err) = inner.rsplit_once(',')?;
    Some((ok.to_owned(), err.to_owned()))
}

fn identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= 48
}

/// `add?` -> ("add", Some("")), `add?Big` -> ("add", Some("Big")), `add` -> ("add", None).
fn split_op(op: &str) -> (&str, Option<&str>) {
    match op.split_once('?') {
        Some((base, mode)) => (base, Some(mode)),
        None => (op, None),
    }
}

fn frame_signatures(frame: &Map<String, Value>) -> BTreeMap<String, (Vec<String>, String)> {
    let mut out = BTreeMap::new();
    for key in ["fns", "functions"] {
        for f in frame
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (Some(name), Some(returns)) = (
                f.get("fn").and_then(Value::as_str),
                f.get("returns").and_then(Value::as_str),
            ) else {
                continue;
            };
            let params = f
                .get("params")
                .and_then(Value::as_array)
                .map(|ps| {
                    ps.iter()
                        .filter_map(|p| p.get(1).and_then(Value::as_str).map(normal))
                        .collect()
                })
                .unwrap_or_default();
            out.insert(name.to_owned(), (params, normal(returns)));
        }
    }
    out
}

fn frame_records(frame: &Map<String, Value>) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for t in frame
        .get("types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(name), Some(fields)) = (
            t.get("name").and_then(Value::as_str),
            t.get("record").and_then(Value::as_array),
        ) else {
            continue;
        };
        let fields = fields
            .iter()
            .filter_map(|f| Some((f.get(0)?.as_str()?.to_owned(), normal(f.get(1)?.as_str()?))))
            .collect();
        out.insert(name.to_owned(), fields);
    }
    out
}

struct Failure {
    at: String,
    message: String,
}

type R<T> = std::result::Result<T, Failure>;

fn fail<T>(at: &str, message: impl Into<String>) -> R<T> {
    Err(Failure {
        at: at.to_owned(),
        message: message.into(),
    })
}

#[derive(Clone)]
struct Binding {
    operand: Value,
    ty: String,
    mutable: bool,
}

/// Variables in scope, in declaration order (the order of block parameters).
#[derive(Clone, Default)]
struct Env(Vec<(String, Binding)>);

impl Env {
    fn get(&self, name: &str) -> Option<&Binding> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, b)| b)
    }

    fn set(&mut self, name: &str, binding: Binding) {
        match self.0.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = binding,
            None => self.0.push((name.to_owned(), binding)),
        }
    }

    /// The operand bound to `name` (null when unbound; never for names the lowering just set).
    fn operand(&self, name: &str) -> Value {
        self.get(name).map_or(Value::Null, |b| b.operand.clone())
    }

    fn remove(&mut self, name: &str) {
        self.0.retain(|(n, _)| n != name);
    }

    /// The bindings of `self` for exactly the names of `before`, in its order.
    fn scoped(&self, before: &Env) -> Env {
        Env(before
            .0
            .iter()
            .map(|(n, b)| (n.clone(), self.get(n).cloned().unwrap_or_else(|| b.clone())))
            .collect())
    }

    fn operands(&self) -> Vec<Value> {
        self.0.iter().map(|(_, b)| b.operand.clone()).collect()
    }
}

/// The loop a `break` or `continue` leaves: its exit block and the state it
/// takes, its header and the state that takes, and the index a `continue`
/// advances first (`for` and `range`).
#[derive(Clone)]
struct LoopCx {
    exit: String,
    exit_args: Env,
    header: String,
    before: Env,
    step: Option<String>,
}

struct Block {
    name: String,
    params: Vec<(String, String)>,
    ops: Vec<Value>,
}

struct Fn<'c, 'a> {
    cx: &'c Context<'a>,
    frame_signatures: &'c BTreeMap<String, (Vec<String>, String)>,
    frame_records: &'c BTreeMap<String, BTreeMap<String, String>>,
    decl: &'c Value,
    at: String,
    params: BTreeMap<String, String>,
    returns: String,
    blocks: Vec<Value>,
    cur: Option<Block>,
    counter: usize,
    trap: Option<String>,
    origins: Vec<((usize, String), String)>,
    /// The exit block of the loop being lowered, opened before its body.
    pending_exit: Option<Block>,
    /// How many blocks (`if` branches, loop bodies) enclose the statement
    /// being lowered; 0 at the top of the body.
    depth: usize,
    /// The loops enclosing the statement being lowered, innermost last.
    loops: Vec<LoopCx>,
}

impl<'c, 'a> Fn<'c, 'a> {
    fn new(
        cx: &'c Context<'a>,
        frame_signatures: &'c BTreeMap<String, (Vec<String>, String)>,
        frame_records: &'c BTreeMap<String, BTreeMap<String, String>>,
        decl: &'c Value,
        at: &str,
    ) -> R<Self> {
        if decl.get("blocks").is_some() {
            return fail(at, "a function has blocks or body, not both");
        }
        let returns = decl
            .get("returns")
            .and_then(Value::as_str)
            .map(normal)
            .ok_or_else(|| Failure {
                at: format!("{at}/returns"),
                message: "a function with a body states its return type".into(),
            })?;
        let mut params = BTreeMap::new();
        for (k, p) in decl
            .get("params")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            match (
                p.get(0).and_then(Value::as_str),
                p.get(1).and_then(Value::as_str),
            ) {
                (Some(n), Some(t)) => {
                    params.insert(n.to_owned(), normal(t));
                }
                _ => return fail(&format!("{at}/params/{k}"), "a parameter is [name, type]"),
            }
        }
        Ok(Self {
            cx,
            frame_signatures,
            frame_records,
            decl,
            at: at.to_owned(),
            params,
            returns,
            blocks: Vec::new(),
            cur: None,
            counter: 0,
            trap: None,
            origins: Vec::new(),
            pending_exit: None,
            loops: Vec::new(),
            depth: 0,
        })
    }

    fn fresh(&mut self, base: &str) -> String {
        self.counter += 1;
        format!("{base}-{}", self.counter)
    }

    fn trap_block(&mut self) -> String {
        if self.trap.is_none() {
            self.trap = Some(self.fresh("trap"));
        }
        self.trap.clone().unwrap_or_default()
    }

    fn record_fields(&self, ty: &str) -> Option<BTreeMap<String, String>> {
        if let Some(fields) = self.frame_records.get(ty) {
            return Some(fields.clone());
        }
        let id = self.cx.type_definition(ty)?;
        let (variant, members) = self.cx.members(&id)?;
        if variant {
            return None;
        }
        Some(
            members
                .into_iter()
                .filter_map(|(n, t)| Some((n, normal(&self.cx.render(&t?)))))
                .collect(),
        )
    }

    fn signature(&self, name: &str) -> Option<(Vec<String>, String)> {
        if let Some(s) = self.frame_signatures.get(name) {
            return Some(s.clone());
        }
        let (params, result) = self.cx.signature(name)?;
        let params = params
            .iter()
            .map(|t| t.as_ref().map(|t| normal(&self.cx.render(t))))
            .collect::<Option<Vec<_>>>()?;
        Some((params, normal(&self.cx.render(&result?))))
    }

    fn lit(value: &Value, ty: &str) -> Value {
        json!({"type": ty, "value": value})
    }

    /// Starts a block whose parameters carry every variable in scope.
    fn new_block(&mut self, env: &Env) -> (Block, Env) {
        let name = self.fresh("b");
        let mut params = Vec::new();
        let mut renamed = Env::default();
        for (var, b) in &env.0 {
            let p = self.fresh(var.trim_start_matches('-'));
            params.push((p.clone(), b.ty.clone()));
            renamed.0.push((
                var.clone(),
                Binding {
                    operand: Value::String(p),
                    ty: b.ty.clone(),
                    mutable: b.mutable,
                },
            ));
        }
        (
            Block {
                name,
                params,
                ops: Vec::new(),
            },
            renamed,
        )
    }

    fn emit(&mut self, op: Value, at: &str) {
        let block = self.blocks.len();
        let cur = self.cur.as_mut().expect("an open block");
        self.origins
            .push(((block, format!("ops/{}", cur.ops.len())), at.to_owned()));
        cur.ops.push(op);
    }

    fn emit_value(&mut self, opcode: &str, operands: Vec<Value>, at: &str) -> Value {
        let name = self.fresh("t");
        let mut op = vec![
            Value::String(name.clone()),
            Value::String(opcode.to_owned()),
        ];
        op.extend(operands);
        self.emit(Value::Array(op), at);
        Value::String(name)
    }

    fn end(&mut self, term: Value, at: &str) {
        let block = self.blocks.len();
        self.origins
            .push(((block, "term".to_owned()), at.to_owned()));
        let cur = self.cur.take().expect("an open block");
        let mut object = Map::new();
        object.insert("name".into(), Value::String(cur.name));
        if !cur.params.is_empty() {
            object.insert(
                "params".into(),
                Value::Array(cur.params.into_iter().map(|(n, t)| json!([n, t])).collect()),
            );
        }
        if !cur.ops.is_empty() {
            object.insert("ops".into(), Value::Array(cur.ops));
        }
        object.insert("term".into(), term);
        self.blocks.push(Value::Object(object));
    }

    fn handler(&mut self, mode: Option<&str>, at: &str) -> R<String> {
        match mode {
            None => Ok(self.trap_block()),
            Some("") => {
                if result_parts(&self.returns).is_none() {
                    return fail(
                        at,
                        "`op?` returns the failure, so the function must return a Result",
                    );
                }
                Ok(String::new())
            }
            Some(case) => {
                if result_parts(&self.returns).is_none() {
                    return fail(
                        at,
                        format!(
                            "`op?{case}` returns error case {case}, so the function must return a Result"
                        ),
                    );
                }
                Ok(case.to_owned())
            }
        }
    }

    fn hold(&mut self, env: &mut Env, operand: Value, ty: &str) -> String {
        let key = self.fresh("-held");
        env.set(
            &key,
            Binding {
                operand,
                ty: ty.to_owned(),
                mutable: false,
            },
        );
        key
    }

    fn release(env: &mut Env, keys: &[String]) -> Vec<Value> {
        let values = keys.iter().map(|k| env.operand(k)).collect();
        for k in keys {
            env.remove(k);
        }
        values
    }

    // -------------------------------------------------------------- types

    fn static_type(&self, e: &Value, env: &Env, hint: Option<&str>) -> Option<String> {
        match e {
            Value::Bool(_) => Some("bool".into()),
            Value::Number(_) => hint.filter(|h| is_int(h)).map(str::to_owned),
            Value::Object(o) => o.get("type").and_then(Value::as_str).map(normal),
            Value::String(s) => env
                .get(s)
                .map(|b| b.ty.clone())
                .or_else(|| self.params.get(s).cloned()),
            Value::Array(items) => {
                let (base, mode) = split_op(items.first()?.as_str()?);
                let args = &items[1..];
                match base {
                    b if COMPARE.contains(&b) || matches!(b, "not" | "and" | "or") => {
                        Some("bool".into())
                    }
                    "len" => Some("u64".into()),
                    "truthy" => Some("bool".into()),
                    "pow" | "gcd" | "powmod" => {
                        args.iter().find_map(|a| self.static_type(a, env, None))
                    }
                    "to" => args.first().and_then(Value::as_str).map(normal),
                    "add" | "sub" | "mul" | "div" | "rem" | "neg" | "abs" | "min" | "max"
                    | "clamp" | "floordiv" | "floormod" => {
                        args.iter().find_map(|a| self.static_type(a, env, None))
                    }
                    "get" => vec_elem(&self.static_type(args.first()?, env, None)?),
                    "field" => {
                        let rec = self.static_type(args.first()?, env, None)?;
                        self.record_fields(&rec)?
                            .get(args.get(1)?.as_str()?)
                            .cloned()
                    }
                    "call" | "tcall" => {
                        let (_, returns) = self.signature(args.first()?.as_str()?)?;
                        if mode.is_some() && (base == "call" || result_parts(&returns).is_some()) {
                            result_parts(&returns).map(|p| p.0)
                        } else {
                            Some(returns)
                        }
                    }
                    "if" => self
                        .static_type(args.get(1)?, env, hint)
                        .or_else(|| self.static_type(args.get(2)?, env, hint)),
                    "fold" => match args.first()?.as_str()? {
                        "count" => Some("i64".into()),
                        "any" | "all" => Some("bool".into()),
                        _ => {
                            let elem = self.fold_element(args.get(2)?, env)?;
                            let mut inner = env.clone();
                            inner.set(
                                args.get(1)?.as_str()?,
                                Binding {
                                    operand: Value::Null,
                                    ty: elem,
                                    mutable: false,
                                },
                            );
                            self.static_type(args.get(3)?, &inner, hint)
                        }
                    },
                    _ => None,
                }
            }
            Value::Null => None,
        }
    }

    // -------------------------------------------------------------- expressions

    fn operands(
        &mut self,
        args: &[Value],
        env: &mut Env,
        at: &str,
        hint: Option<&str>,
    ) -> R<(Vec<Value>, String)> {
        let ty = args
            .iter()
            .filter(|a| !a.is_number())
            .find_map(|a| self.static_type(a, env, None))
            .or_else(|| hint.map(str::to_owned))
            .unwrap_or_else(|| "i64".into());
        let mut keys = Vec::new();
        for (k, a) in args.iter().enumerate() {
            let sub = format!("{at}/{}", k + 1);
            let (v, t) = self.expr(a, env, &sub, Some(&ty))?;
            if t != ty {
                return fail(&sub, format!("operands mix {ty} and {t}"));
            }
            keys.push(self.hold(env, v, &t));
        }
        Ok((Self::release(env, &keys), ty))
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::many_single_char_names)]
    fn expr(
        &mut self,
        e: &Value,
        env: &mut Env,
        at: &str,
        hint: Option<&str>,
    ) -> R<(Value, String)> {
        match e {
            Value::Bool(b) => return Ok((Self::lit(&Value::Bool(*b), "bool"), "bool".into())),
            Value::Number(_) => {
                let ty = hint.filter(|h| is_int(h)).unwrap_or("i64").to_owned();
                return Ok((Self::lit(e, &ty), ty));
            }
            Value::Object(o) => {
                return match (
                    o.get("type").and_then(Value::as_str),
                    o.get("value"),
                    o.len(),
                ) {
                    (Some(t), Some(_), 2) => Ok((e.clone(), normal(t))),
                    _ => fail(at, "a typed literal is {\"type\": T, \"value\": v}"),
                };
            }
            Value::String(name) => {
                if let Some(b) = env.get(name) {
                    return Ok((b.operand.clone(), b.ty.clone()));
                }
                if let Some(t) = self.params.get(name) {
                    return Ok((e.clone(), t.clone()));
                }
                return fail(at, format!("unknown name `{name}`"));
            }
            Value::Array(_) | Value::Null => {}
        }
        let Some(items) = e
            .as_array()
            .filter(|i| i.first().is_some_and(Value::is_string))
        else {
            return fail(at, "an expression is a name, a literal, or [op, args...]");
        };
        let word = items[0].as_str().unwrap_or_default();
        let (base, mode) = split_op(word);
        let args = &items[1..];
        let sub = |k: usize| format!("{at}/{}", k + 1);
        match base {
            "add" | "sub" | "mul" | "div" | "rem" | "neg" | "abs" => {
                let arity = if matches!(base, "neg" | "abs") { 1 } else { 2 };
                if args.len() != arity {
                    return fail(at, format!("`{base}` takes {arity} operand(s)"));
                }
                let (vals, ty) = self.operands(args, env, at, hint)?;
                if !is_int(&ty) {
                    return fail(at, format!("`{base}` needs integer operands, not {ty}"));
                }
                let handler = self.handler(mode, at)?;
                if base == "abs" {
                    // neg fails only for the minimum value, which is negative: computing it
                    // before the branch is observably the same as on the negative arm only.
                    let x = vals[0].clone();
                    let negated = self.emit_value(&format!("neg?{handler}"), vec![x.clone()], at);
                    let zero = Self::lit(&json!(0), &ty);
                    return Ok(self.select(
                        "lt",
                        x.clone(),
                        zero,
                        &ty,
                        env,
                        at,
                        Some((negated, x)),
                    ));
                }
                Ok((self.emit_value(&format!("{base}?{handler}"), vals, at), ty))
            }
            "to" => {
                let (Some(target), Some(x)) = (args.first().and_then(Value::as_str), args.get(1))
                else {
                    return fail(at, "`to` takes (integer type, value)");
                };
                let target = normal(target);
                if args.len() != 2 || !is_int(&target) {
                    return fail(at, "`to` takes (integer type, value)");
                }
                let (v, source) = self.expr(x, env, &sub(1), None)?;
                if !is_int(&source) {
                    return fail(&sub(1), format!("`to` converts an integer, not {source}"));
                }
                self.handler(mode, at)?;
                if source == target {
                    return Ok((v, target));
                }
                self.convert(v, &source, &target, mode, env, at)
            }
            "min" | "max" | "clamp" => {
                let arity = if base == "clamp" { 3 } else { 2 };
                if mode.is_some() || args.len() != arity {
                    return fail(
                        at,
                        format!("`{base}` takes {arity} operands and cannot fail"),
                    );
                }
                let (vals, ty) = self.operands(args, env, at, hint)?;
                if !is_int(&ty) {
                    return fail(at, format!("`{base}` needs integer operands"));
                }
                if base == "clamp" {
                    let high = self.hold(env, vals[2].clone(), &ty);
                    let (low, _) =
                        self.select("gt", vals[0].clone(), vals[1].clone(), &ty, env, at, None);
                    let hi = Self::release(env, &[high]).remove(0);
                    return Ok(self.select("lt", low, hi, &ty, env, at, None));
                }
                let test = if base == "min" { "lt" } else { "gt" };
                Ok(self.select(test, vals[0].clone(), vals[1].clone(), &ty, env, at, None))
            }
            b if COMPARE.contains(&b) => {
                if mode.is_some() || args.len() != 2 {
                    return fail(at, format!("`{b}` takes 2 operands"));
                }
                let (vals, _) = self.operands(args, env, at, None)?;
                Ok((self.emit_value(b, vals, at), "bool".into()))
            }
            "fold" => self.fold(args, env, at, hint),
            "floordiv" | "floormod" => {
                if args.len() != 2 {
                    return fail(at, format!("`{base}` takes 2 operands"));
                }
                let (vals, ty) = self.operands(args, env, at, hint)?;
                if !is_int(&ty) {
                    return fail(at, format!("`{base}` needs integer operands, not {ty}"));
                }
                self.handler(mode, at)?;
                // Python's floor division and modulo: the truncating quotient
                // and remainder, corrected when the remainder is nonzero and its
                // sign differs from the divisor's.
                // The remainder is computed once and reused: the operands are
                // plain values, so a second `rem` could only repeat the first
                // one's answer (or its failure, which already happened). When
                // the divisor is an integer literal its sign is known, so the
                // test reduces to the remainder's sign alone.
                let suffix = &word[base.len()..];
                let divisor_sign = vals[1]
                    .get("value")
                    .and_then(Value::as_i64)
                    .map(i64::signum);
                let a = self.hold(env, vals[0].clone(), &ty);
                let b = self.hold(env, vals[1].clone(), &ty);
                let (remainder, _) =
                    self.expr(&json!([format!("rem{suffix}"), a, b]), env, at, Some(&ty))?;
                let r = self.hold(env, remainder, &ty);
                let adjust = match divisor_sign {
                    Some(1) => json!(["lt", r, 0]),
                    Some(-1) => json!(["gt", r, 0]),
                    _ => json!(["and", ["ne", r, 0], ["ne", ["lt", r, 0], ["lt", b, 0]]]),
                };
                let lowered = if base == "floordiv" {
                    let quotient = json!([format!("div{suffix}"), a, b]);
                    json!(["if", adjust, ["sub", quotient, 1], quotient])
                } else {
                    json!(["if", adjust, ["add", r, b], r])
                };
                let result = self.expr(&lowered, env, at, Some(&ty));
                Self::release(env, &[a, b, r]);
                result
            }
            "not" => {
                if args.len() != 1 {
                    return fail(at, "`not` takes 1 operand");
                }
                let (v, t) = self.expr(&args[0], env, &sub(0), None)?;
                if t != "bool" {
                    return fail(at, "`not` needs a bool");
                }
                Ok((self.emit_value("not", vec![v], at), "bool".into()))
            }
            "and" | "or" => {
                if args.len() != 2 {
                    return fail(at, format!("`{base}` takes 2 operands"));
                }
                let (a, ta) = self.expr(&args[0], env, &sub(0), None)?;
                let ka = self.hold(env, a, &ta);
                let (b, tb) = self.expr(&args[1], env, &sub(1), None)?;
                let a = Self::release(env, &[ka]).remove(0);
                if ta != "bool" || tb != "bool" {
                    return fail(at, format!("`{base}` needs bool operands"));
                }
                Ok((self.emit_value(base, vec![a, b], at), "bool".into()))
            }
            "truthy" => {
                if args.len() != 1 {
                    return fail(at, "`truthy` takes 1 operand");
                }
                let (v, t) = self.expr(&args[0], env, &sub(0), None)?;
                if t == "bool" {
                    return Ok((v, t));
                }
                if is_int(&t) {
                    let zero = Self::lit(&json!(0), &t);
                    return Ok((self.emit_value("ne", vec![v, zero], at), "bool".into()));
                }
                if vec_elem(&t).is_some() {
                    let length = self.emit_value("vec_len", vec![v], at);
                    let zero = Self::lit(&json!(0), "u64");
                    return Ok((self.emit_value("ne", vec![length, zero], at), "bool".into()));
                }
                if t == "text" {
                    let empty = json!({"type": "text", "value": ""});
                    return Ok((self.emit_value("ne", vec![v, empty], at), "bool".into()));
                }
                fail(at, format!("a {t} has no truth value"))
            }
            "powmod" => {
                // Python's pow(b, e, m): repeated checked multiplication modulo m
                // (floor modulo; m == 0 fails like a division by zero, a
                // negative e traps).
                if args.len() != 3 {
                    return fail(at, "`powmod` takes 3 operands");
                }
                let (vals, ty) = self.operands(args, env, at, hint)?;
                if !is_int(&ty) {
                    return fail(at, format!("`powmod` needs integer operands, not {ty}"));
                }
                self.handler(mode, at)?;
                let b = self.hold(env, vals[0].clone(), &ty);
                let e = self.hold(env, vals[1].clone(), &ty);
                let m = self.hold(env, vals[2].clone(), &ty);
                let (x, y) = (self.fresh("-x"), self.fresh("-y"));
                let statements = vec![
                    json!(["if", ["lt", e, 0], [["trap"]]]),
                    json!(["var", x, ty, ["floormod", {"type": ty, "value": 1}, m]]),
                    json!(["var", y, ty, {"type": ty, "value": 0}]),
                    json!([
                        "while",
                        ["lt", y, e],
                        [
                            ["set", x, ["floormod", ["mul", x, ["floormod", b, m]], m]],
                            ["set", y, ["add", y, 1]]
                        ]
                    ]),
                ];
                self.stmts(&statements, env, at, true)?;
                let result = env.operand(&x);
                env.remove(&x);
                env.remove(&y);
                Self::release(env, &[b, e, m]);
                Ok((result, ty))
            }
            "pow" | "gcd" => {
                if args.len() != 2 {
                    return fail(at, format!("`{base}` takes 2 operands"));
                }
                let (vals, ty) = self.operands(args, env, at, hint)?;
                if !is_int(&ty) {
                    return fail(at, format!("`{base}` needs integer operands, not {ty}"));
                }
                self.handler(mode, at)?;
                let suffix = &word[base.len()..];
                let a = self.hold(env, vals[0].clone(), &ty);
                let b = self.hold(env, vals[1].clone(), &ty);
                let (x, y) = (self.fresh("-x"), self.fresh("-y"));
                let statements = if base == "pow" {
                    // Repeated checked multiplication; a negative exponent traps
                    // (Python would produce a float).
                    vec![
                        json!(["if", ["lt", b, 0], [["trap"]]]),
                        json!(["var", x, ty, {"type": ty, "value": 1}]),
                        json!(["var", y, ty, {"type": ty, "value": 0}]),
                        json!([
                            "while",
                            ["lt", y, b],
                            [
                                ["set", x, [format!("mul{suffix}"), x, a]],
                                ["set", y, ["add", y, 1]]
                            ]
                        ]),
                    ]
                } else {
                    // Euclid on absolute values: gcd(0, 0) is 0.
                    let rest = format!("gcd_rest_{}", self.counter);
                    vec![
                        json!(["var", x, ty, [format!("abs{suffix}"), a]]),
                        json!(["var", y, ty, [format!("abs{suffix}"), b]]),
                        json!([
                            "while",
                            ["ne", y, 0],
                            [
                                ["let", rest, ["rem", x, y]],
                                ["set", x, y],
                                ["set", y, rest]
                            ]
                        ]),
                    ]
                };
                self.stmts(&statements, env, at, true)?;
                let result = env.operand(&x);
                env.remove(&x);
                env.remove(&y);
                Self::release(env, &[a, b]);
                Ok((result, ty))
            }
            "len" => {
                if args.len() != 1 {
                    return fail(at, "`len` takes 1 operand");
                }
                let (v, t) = self.expr(&args[0], env, &sub(0), None)?;
                if vec_elem(&t).is_none() {
                    return fail(at, "`len` needs a vector");
                }
                Ok((self.emit_value("vec_len", vec![v], at), "u64".into()))
            }
            "get" => {
                if args.len() != 2 {
                    return fail(at, "`get` takes (vector, u64 index)");
                }
                let (v, t) = self.expr(&args[0], env, &sub(0), None)?;
                let Some(elem) = vec_elem(&t) else {
                    return fail(at, "`get` needs a vector");
                };
                let kv = self.hold(env, v, &t);
                let (i, ti) = self.expr(&args[1], env, &sub(1), Some("u64"))?;
                let v = Self::release(env, &[kv]).remove(0);
                if ti != "u64" {
                    return fail(at, format!("a vector index is u64, not {ti}"));
                }
                let handler = self.handler(mode, at)?;
                Ok((
                    self.emit_value(&format!("vec_get?{handler}"), vec![v, i], at),
                    elem,
                ))
            }
            "field" => {
                let Some(field) = args
                    .get(1)
                    .and_then(Value::as_str)
                    .filter(|_| args.len() == 2)
                else {
                    return fail(at, "`field` takes (record, \"name\")");
                };
                let (v, t) = self.expr(&args[0], env, &sub(0), None)?;
                let Some(fields) = self.record_fields(&t) else {
                    return fail(at, format!("`{t}` is not a known record type"));
                };
                let Some(ft) = fields.get(field).cloned() else {
                    return fail(at, format!("record `{t}` has no field `{field}`"));
                };
                Ok((
                    self.emit_value("field", vec![json!(format!("{t}.{field}")), v], at),
                    ft,
                ))
            }
            "call" | "tcall" => {
                let Some(callee) = args.first().and_then(Value::as_str) else {
                    return fail(at, "`call` takes (\"function\", args...)");
                };
                let Some((params, returns)) = self.signature(callee) else {
                    return fail(
                        at,
                        format!("`{callee}` is not a known function with a complete signature"),
                    );
                };
                if params.len() != args.len() - 1 {
                    return fail(at, format!("`{callee}` takes {} argument(s)", params.len()));
                }
                let mut keys = Vec::new();
                for (k, (a, pty)) in args[1..].iter().zip(&params).enumerate() {
                    let (v, t) = self.expr(a, env, &sub(k + 1), Some(pty))?;
                    if &t != pty {
                        return fail(
                            &sub(k + 1),
                            format!("argument of `{callee}` is {pty}, not {t}"),
                        );
                    }
                    keys.push(self.hold(env, v, &t));
                }
                let mut operands = vec![json!(callee)];
                operands.extend(Self::release(env, &keys));
                let unwrap = mode.is_some() && (base == "call" || result_parts(&returns).is_some());
                if !unwrap {
                    return Ok((self.emit_value("call", operands, at), returns));
                }
                let Some((ok, _)) = result_parts(&returns) else {
                    return fail(at, format!("`{callee}` does not return a Result to unwrap"));
                };
                let handler = self.handler(mode, at)?;
                Ok((
                    self.emit_value(&format!("call?{handler}"), operands, at),
                    ok,
                ))
            }
            "if" => {
                if args.len() != 3 {
                    return fail(at, "`if` takes (condition, then-value, else-value)");
                }
                let (c, tc) = self.expr(&args[0], env, &sub(0), None)?;
                if tc != "bool" {
                    return fail(&sub(0), "an `if` condition is bool");
                }
                let probe = self
                    .static_type(&args[1], env, hint)
                    .or_else(|| self.static_type(&args[2], env, hint))
                    .or_else(|| hint.map(str::to_owned));
                self.if_value(&c, &args[1], &args[2], env, at, probe.as_deref())
            }
            _ => fail(at, format!("unknown operation `{word}`")),
        }
    }

    /// `test(a, b) ? first : second` (default `a : b`), by a conditional edge into a join.
    #[allow(clippy::too_many_arguments)]
    fn select(
        &mut self,
        test: &str,
        a: Value,
        b: Value,
        ty: &str,
        env: &mut Env,
        at: &str,
        chosen: Option<(Value, Value)>,
    ) -> (Value, String) {
        let c = self.emit_value(test, vec![a.clone(), b.clone()], at);
        let (first, second) = chosen.unwrap_or((a, b));
        let (mut join, jenv) = self.new_block(env);
        let result = self.fresh("sel");
        join.params.push((result.clone(), ty.to_owned()));
        let args = env.operands();
        let mut yes = vec![json!(join.name)];
        yes.extend(args.clone());
        yes.push(first);
        let mut no = vec![json!(join.name)];
        no.extend(args);
        no.push(second);
        self.end(json!(["cond", c, yes, no]), at);
        self.cur = Some(join);
        *env = jenv;
        (Value::String(result), ty.to_owned())
    }

    fn if_value(
        &mut self,
        cond: &Value,
        then_value: &Value,
        else_value: &Value,
        env: &mut Env,
        at: &str,
        ty: Option<&str>,
    ) -> R<(Value, String)> {
        let before = env.clone();
        let (yes, yenv) = self.new_block(&before);
        let (no, nenv) = self.new_block(&before);
        let args = before.operands();
        let (yn, nn) = (yes.name.clone(), no.name.clone());
        let mut yt = vec![json!(yn)];
        yt.extend(args.clone());
        let mut nt = vec![json!(nn)];
        nt.extend(args);
        self.end(json!(["cond", cond, yt, nt]), at);
        let mut arms = Vec::new();
        for (block, benv, value, slot) in [(yes, yenv, then_value, 2), (no, nenv, else_value, 3)] {
            self.cur = Some(block);
            let mut local = benv;
            let (operand, value_ty) = self.expr(value, &mut local, &format!("{at}/{slot}"), ty)?;
            arms.push((self.cur.take(), local, operand, value_ty));
        }
        if arms[0].3 != arms[1].3 {
            return fail(
                at,
                format!(
                    "the two arms have different types ({} and {})",
                    arms[0].3, arms[1].3
                ),
            );
        }
        let ty = arms[0].3.clone();
        let (mut join, jenv) = self.new_block(&before);
        let result = self.fresh("if");
        join.params.push((result.clone(), ty.clone()));
        let join_name = join.name.clone();
        for (block, local, v, _) in arms {
            self.cur = block;
            let mut term = vec![json!(join_name)];
            term.extend(local.scoped(&before).operands());
            term.push(v);
            self.end(
                Value::Array([json!("br")].into_iter().chain(term).collect()),
                at,
            );
        }
        self.cur = Some(join);
        *env = jenv;
        Ok((Value::String(result), ty))
    }

    /// Integer conversion by binary digits with checked arithmetic in the
    /// target type: exact for every in-range value; out of range fails like
    /// a checked operation under `mode`.
    fn convert(
        &mut self,
        value: Value,
        source: &str,
        target: &str,
        mode: Option<&str>,
        env: &mut Env,
        at: &str,
    ) -> R<(Value, String)> {
        let suffix = mode.map_or(String::new(), |m| format!("?{m}"));
        let n = self.counter + 1;
        let (rest, acc, power, digit, held) = (
            format!("-k{n}"),
            format!("-r{n}"),
            format!("-p{n}"),
            format!("-d{n}"),
            format!("-x{n}"),
        );
        self.counter += 1;
        env.set(
            &held,
            Binding {
                operand: value,
                ty: source.to_owned(),
                mutable: false,
            },
        );
        let typed = |number: i64| json!({"type": target, "value": number});
        let mut body = vec![
            json!(["var", rest, source, held]),
            json!(["var", acc, target, typed(0)]),
            json!(["var", power, target, typed(1)]),
        ];
        if signed(source) {
            let negative = if signed(target) {
                json!(["set", power, typed(-1)])
            } else {
                // A negative value has no unsigned form: fail the way the mode says.
                json!(["set", power, [format!("sub{suffix}"), typed(0), typed(1)]])
            };
            body.push(json!(["if", ["lt", rest, 0], [negative]]));
        }
        body.push(json!([
            "while",
            ["ne", rest, 0],
            [
                ["let", digit, ["rem", rest, 2]],
                ["set", rest, ["div", rest, 2]],
                [
                    "if",
                    ["ne", digit, 0],
                    [["set", acc, [format!("add{suffix}"), acc, power]]]
                ],
                [
                    "if",
                    ["ne", rest, 0],
                    [["set", power, [format!("mul{suffix}"), power, 2]]]
                ]
            ]
        ]));
        self.stmts(&body, env, at, true)?;
        let result = env.operand(&acc);
        for name in [&rest, &acc, &power, &held] {
            env.remove(name);
        }
        Ok((result, target.to_owned()))
    }

    // -------------------------------------------------------------- statements

    /// Lowers a statement list; `Ok(true)` when control falls through.
    /// `internal` statements come from the lowering itself and may use hidden names.
    #[allow(clippy::too_many_lines)]
    fn stmts(&mut self, body: &[Value], env: &mut Env, at: &str, internal: bool) -> R<bool> {
        // Names declared before this list: a nested list may not redeclare
        // them (whether that would hide or overwrite the outer name differs
        // between the languages authors know, so it is refused, not chosen).
        let outer = env.0.len();
        for (k, s) in body.iter().enumerate() {
            let here = if internal {
                at.to_owned()
            } else {
                format!("{at}/{k}")
            };
            if self.cur.is_none() {
                return fail(&here, "statement after return or fail can never run");
            }
            let Some(items) = s
                .as_array()
                .filter(|i| i.first().is_some_and(Value::is_string))
            else {
                return fail(&here, "a statement is [keyword, ...]");
            };
            let kind = items[0].as_str().unwrap_or_default();
            let name_ok = |i: usize| -> R<String> {
                match items.get(i).and_then(Value::as_str) {
                    Some(n) if internal || identifier(n) => Ok(n.to_owned()),
                    _ => fail(&here, "variable names are letters, digits and _"),
                }
            };
            let part = |i: usize| {
                if internal {
                    here.clone()
                } else {
                    format!("{here}/{i}")
                }
            };
            match kind {
                "let" => {
                    if items.len() != 3 {
                        return fail(&here, "[\"let\", name, value]");
                    }
                    let n = name_ok(1)?;
                    self.no_shadow(env, outer, &n, &here, internal)?;
                    let (v, t) = self.expr(&items[2], env, &part(2), None)?;
                    env.set(
                        &n,
                        Binding {
                            operand: v,
                            ty: t,
                            mutable: false,
                        },
                    );
                }
                "var" => {
                    if items.len() == 3 {
                        let n = name_ok(1)?;
                        self.no_shadow(env, outer, &n, &here, internal)?;
                        let (v, t) = self.expr(&items[2], env, &part(2), None)?;
                        env.set(
                            &n,
                            Binding {
                                operand: v,
                                ty: t,
                                mutable: true,
                            },
                        );
                        continue;
                    }
                    if items.len() != 4 {
                        return fail(
                            &here,
                            "[\"var\", name, type, value] or [\"var\", name, value]",
                        );
                    }
                    let n = name_ok(1)?;
                    self.no_shadow(env, outer, &n, &here, internal)?;
                    let Some(ty) = items[2].as_str().map(normal) else {
                        return fail(
                            &here,
                            "[\"var\", name, type, value] or [\"var\", name, value]",
                        );
                    };
                    let (v, t) = self.expr(&items[3], env, &part(3), Some(&ty))?;
                    if t != ty {
                        return fail(&here, format!("`{n}` is {ty} but its value is {t}"));
                    }
                    env.set(
                        &n,
                        Binding {
                            operand: v,
                            ty,
                            mutable: true,
                        },
                    );
                }
                "declare" => {
                    // ["declare", x, witness]: a var of the witness's static type
                    // (never evaluated), holding that type's zero, before branches
                    // that each assign it.
                    if items.len() != 3 {
                        return fail(&here, "[\"declare\", name, witness]");
                    }
                    let n = name_ok(1)?;
                    self.no_shadow(env, outer, &n, &here, internal)?;
                    // An untyped literal witness is an i64, as everywhere else.
                    let Some(ty) = self.static_type(&items[2], env, Some("i64")) else {
                        return fail(
                            &here,
                            format!(
                                "`{n}` is assigned in every branch, but its type is not known before them: assign it once before the if"
                            ),
                        );
                    };
                    let zero = if is_int(&ty) {
                        Self::lit(&json!(0), &ty)
                    } else if ty == "bool" {
                        Self::lit(&json!(false), "bool")
                    } else if ty == "text" {
                        json!({"type": "text", "value": ""})
                    } else {
                        return fail(
                            &here,
                            format!("`{n}` ({ty}) must be assigned once before the if"),
                        );
                    };
                    env.set(
                        &n,
                        Binding {
                            operand: zero,
                            ty,
                            mutable: true,
                        },
                    );
                }
                "set" => {
                    let target = items.get(1).and_then(Value::as_str).unwrap_or_default();
                    let Some(binding) = env
                        .get(target)
                        .filter(|b| b.mutable && items.len() == 3)
                        .cloned()
                    else {
                        return fail(&here, format!("`set` needs a `var` in scope (`{target}`)"));
                    };
                    let (v, t) = self.expr(&items[2], env, &part(2), Some(&binding.ty))?;
                    if t != binding.ty {
                        return fail(
                            &here,
                            format!("`{target}` is {} but the value is {t}", binding.ty),
                        );
                    }
                    env.set(
                        target,
                        Binding {
                            operand: v,
                            ty: t,
                            mutable: true,
                        },
                    );
                }
                "if" => self.if_stmt(items, env, &here, internal)?,
                "for" | "for-indexed" => self.for_stmt(items, env, &here)?,
                "range" => self.range_stmt(items, env, &here)?,
                "while" => self.while_stmt(items, env, &here, internal)?,
                "return" | "ok" => {
                    if items.len() != 2 {
                        return fail(&here, format!("[\"{kind}\", value]"));
                    }
                    let parts = result_parts(&self.returns);
                    if kind == "ok" && parts.is_none() {
                        return fail(&here, "`ok` needs a Result return type; use return");
                    }
                    if kind == "return" && parts.is_some() {
                        return fail(
                            &here,
                            "this function returns a Result: use [\"ok\", v] or [\"fail\", Case]",
                        );
                    }
                    let want = parts.map_or_else(|| self.returns.clone(), |p| p.0);
                    let (v, t) = self.expr(&items[1], env, &part(1), Some(&want))?;
                    if t != want {
                        return fail(&here, format!("the function returns {want}, not {t}"));
                    }
                    self.end(json!([kind, v]), &here);
                }
                "fail" => {
                    if items.len() != 2 || result_parts(&self.returns).is_none() {
                        return fail(&here, "[\"fail\", Case] in a Result function");
                    }
                    self.end(json!(["fail", items[1]]), &here);
                }
                "trap" => self.end(json!(["trap"]), &here),
                "break" | "continue" => {
                    let Some(cx) = self.loops.last().cloned() else {
                        return fail(&here, format!("`{kind}` is outside every loop"));
                    };
                    let mut state = env.clone();
                    let (target, args) = if kind == "break" {
                        (cx.exit, state.scoped(&cx.exit_args).operands())
                    } else {
                        if let Some(step) = &cx.step {
                            self.advance(&mut state, step, &here);
                        }
                        (cx.header, state.scoped(&cx.before).operands())
                    };
                    let mut term = vec![json!("br"), json!(target)];
                    term.extend(args);
                    self.end(Value::Array(term), &here);
                }
                _ => return fail(&here, format!("unknown statement `{kind}`")),
            }
        }
        Ok(self.cur.is_some())
    }

    /// Refuses a declaration in a nested block of a name declared outside
    /// it: a variable from an enclosing list or, inside any block, a
    /// parameter. Redeclaring in the same list rebinds the name.
    fn no_shadow(&self, env: &Env, outer: usize, name: &str, at: &str, internal: bool) -> R<()> {
        if internal || self.depth == 0 {
            return Ok(());
        }
        if env.0[..outer.min(env.0.len())]
            .iter()
            .any(|(n, _)| n == name)
            || self.params.contains_key(name)
        {
            return fail(
                at,
                format!(
                    "`{name}` is declared outside this block; a nested block cannot redeclare it: use another name, or assign a var declared before the block"
                ),
            );
        }
        Ok(())
    }

    fn if_stmt(&mut self, items: &[Value], env: &mut Env, at: &str, internal: bool) -> R<()> {
        if !(3..=4).contains(&items.len()) {
            return fail(at, "[\"if\", condition, [then...], [else...]]");
        }
        let part = |i: usize| {
            if internal {
                at.to_owned()
            } else {
                format!("{at}/{i}")
            }
        };
        let (c, tc) = self.expr(&items[1], env, &part(1), None)?;
        if tc != "bool" {
            return fail(&part(1), "an `if` condition is bool");
        }
        let before = env.clone();
        let (yes, yenv) = self.new_block(&before);
        let (no, nenv) = self.new_block(&before);
        let args = before.operands();
        let mut yt = vec![json!(yes.name)];
        yt.extend(args.clone());
        let mut nt = vec![json!(no.name)];
        nt.extend(args);
        self.end(json!(["cond", c, yt, nt]), at);
        let empty = Vec::new();
        let mut ends = Vec::new();
        for (block, benv, key) in [(yes, yenv, 2), (no, nenv, 3)] {
            let body = items.get(key).map_or(Some(&empty), Value::as_array);
            let Some(body) = body else {
                return fail(&part(key), "a branch is a statement list");
            };
            self.cur = Some(block);
            let mut local = benv;
            self.depth += 1;
            let falls = self.stmts(body, &mut local, &part(key), internal)?;
            self.depth -= 1;
            if falls {
                ends.push((self.cur.take(), local.scoped(&before)));
            }
        }
        if ends.is_empty() {
            self.cur = None;
            return Ok(());
        }
        let (join, jenv) = self.new_block(&before);
        let name = join.name.clone();
        for (block, local) in ends {
            self.cur = block;
            let mut term = vec![json!("br"), json!(name)];
            term.extend(local.operands());
            self.end(Value::Array(term), at);
        }
        self.cur = Some(join);
        *env = jenv;
        Ok(())
    }

    /// `header(state) -> cond -> body(state) ... back edge`, `exit(state)`;
    /// state is every variable in scope, hidden loop variables included.
    fn enter_loop(&mut self, env: &Env, at: &str) -> (Env, String) {
        let before = env.clone();
        let (header, henv) = self.new_block(&before);
        let name = header.name.clone();
        let mut term = vec![json!("br"), json!(name)];
        term.extend(before.operands());
        self.end(Value::Array(term), at);
        self.cur = Some(header);
        (henv, name)
    }

    fn test_loop(&mut self, cond: &Value, local: &Env, at: &str) -> (Env, Env) {
        let (body, benv) = self.new_block(local);
        let (exit, xenv) = self.new_block(local);
        let args = local.operands();
        let mut yt = vec![json!(body.name)];
        yt.extend(args.clone());
        let mut nt = vec![json!(exit.name)];
        nt.extend(args);
        self.end(json!(["cond", cond, yt, nt]), at);
        self.cur = Some(body);
        self.pending_exit = Some(exit);
        (benv, xenv)
    }

    fn close_loop(
        &mut self,
        header: &str,
        inner: Option<&Env>,
        before: &Env,
        exit_env: &Env,
        env: &mut Env,
        at: &str,
    ) {
        if let Some(inner) = inner {
            let mut term = vec![json!("br"), json!(header)];
            term.extend(inner.scoped(before).operands());
            self.end(Value::Array(term), at);
        }
        self.cur = self.pending_exit.take();
        *env = exit_env.scoped(before);
    }

    fn advance(&mut self, env: &mut Env, key: &str, at: &str) {
        let binding = env.get(key).expect("lowering owns the loop index").clone();
        let trap = self.trap_block();
        let next = self.emit_value(
            &format!("add?{trap}"),
            vec![binding.operand.clone(), Self::lit(&json!(1), &binding.ty)],
            at,
        );
        env.set(
            key,
            Binding {
                operand: next,
                ..binding
            },
        );
    }

    /// The element type a fold visits: a vector's elements, or a range's
    /// integers.
    fn fold_element(&self, list: &Value, env: &Env) -> Option<String> {
        if let Some(items) = list.as_array()
            && items.first().and_then(Value::as_str) == Some("range")
        {
            return items[1..]
                .iter()
                .filter(|a| !a.is_number())
                .find_map(|a| self.static_type(a, env, None))
                .or_else(|| Some("i64".into()));
        }
        vec_elem(&self.static_type(list, env, None)?)
    }

    /// `["fold", kind, x, xs, e, c?]`: an accumulator loop over `xs`, lowered
    /// through the ordinary `var`, `for`, `if` and `set` statements.
    #[allow(clippy::too_many_lines)]
    fn fold(
        &mut self,
        args: &[Value],
        env: &mut Env,
        at: &str,
        hint: Option<&str>,
    ) -> R<(Value, String)> {
        const GRAMMAR: &str = "[\"fold\", sum|count|any|all, element, vector, value, condition?]";
        if !(4..=6).contains(&args.len()) {
            return fail(at, GRAMMAR);
        }
        let Some(kind) = args[0].as_str() else {
            return fail(at, GRAMMAR);
        };
        let x = self
            .loop_name(&args[1], env, &format!("{at}/2"))?
            .to_owned();
        let (list, value) = (&args[2], &args[3]);
        let condition = args.get(4).filter(|c| !c.is_null());
        // max/min: the value when no element is visited (else a trap).
        let fallback = args.get(5).filter(|d| !d.is_null());
        let Some(elem) = self.fold_element(list, env) else {
            return fail(&format!("{at}/3"), "a fold visits a vector or a range");
        };
        let mut inner = env.clone();
        inner.set(
            &x,
            Binding {
                operand: Value::Null,
                ty: elem,
                mutable: false,
            },
        );
        let value_ty = self.static_type(value, &inner, hint);
        let acc = self.fresh("-fold");
        let (ty, init, step) = match kind {
            "sum" => {
                let ty = value_ty
                    .clone()
                    .or_else(|| hint.filter(|h| is_int(h)).map(str::to_owned))
                    .unwrap_or_else(|| "i64".into());
                if !is_int(&ty) {
                    return fail(&format!("{at}/4"), format!("`sum` adds integers, not {ty}"));
                }
                let step = json!([["set", acc, ["add", acc, value]]]);
                (ty.clone(), json!({"type": ty, "value": 0}), step)
            }
            "count" => {
                let step = json!([["if", value, [["set", acc, ["add", acc, 1]]]]]);
                ("i64".to_owned(), json!({"type": "i64", "value": 0}), step)
            }
            "any" | "all" => {
                let join = if kind == "any" { "or" } else { "and" };
                let step = json!([["set", acc, [join, acc, value]]]);
                ("bool".to_owned(), json!(kind == "all"), step)
            }
            "max" | "min" => {
                let ty = value_ty
                    .clone()
                    .or_else(|| hint.filter(|h| is_int(h)).map(str::to_owned))
                    .unwrap_or_else(|| "i64".into());
                if !is_int(&ty) {
                    return fail(
                        &format!("{at}/4"),
                        format!("`{kind}` compares integers, not {ty}"),
                    );
                }
                // The per-element value is bound in the loop body, where names
                // must be identifiers.
                let seen = self.fresh("-seen");
                let item = format!("fold_item_{}", self.counter);
                let better = if kind == "max" { "gt" } else { "lt" };
                let step = json!([
                    ["let", item, value],
                    [
                        "if",
                        ["or", ["not", seen], [better, item, acc]],
                        [["set", acc, item]]
                    ],
                    ["set", seen, true]
                ]);
                let body = match condition {
                    Some(c) => json!([["if", c, step]]),
                    None => step,
                };
                let looping = if let Some(items) = list
                    .as_array()
                    .filter(|i| i.first().and_then(Value::as_str) == Some("range"))
                {
                    json!(["range", x, items.get(1), items.get(2), body])
                } else {
                    json!(["for", x, list, body])
                };
                let empty = match fallback {
                    Some(d) => json!(["set", acc, d]),
                    None => json!(["trap"]),
                };
                let statements = [
                    json!(["var", acc, ty, {"type": ty, "value": 0}]),
                    json!(["var", seen, "bool", false]),
                    looping,
                    json!(["if", ["not", seen], [empty]]),
                ];
                self.stmts(&statements, env, at, true)?;
                let result = env.operand(&acc);
                env.remove(&acc);
                env.remove(&seen);
                return Ok((result, ty));
            }
            _ => {
                return fail(
                    &format!("{at}/1"),
                    "a fold is sum, count, any, all, max or min",
                );
            }
        };
        if kind != "sum" && value_ty.as_deref().is_some_and(|t| t != "bool") {
            return fail(
                &format!("{at}/4"),
                format!(
                    "`{kind}` needs a bool, not {}",
                    value_ty.unwrap_or_default()
                ),
            );
        }
        let body = match condition {
            Some(c) => json!([["if", c, step]]),
            None => step,
        };
        let looping = if let Some(items) = list
            .as_array()
            .filter(|i| i.first().and_then(Value::as_str) == Some("range"))
        {
            json!(["range", x, items.get(1), items.get(2), body])
        } else {
            json!(["for", x, list, body])
        };
        let statements = [json!(["var", acc, ty, init]), looping];
        self.stmts(&statements, env, at, true)?;
        let result = env.operand(&acc);
        env.remove(&acc);
        Ok((result, ty))
    }

    fn loop_name<'v>(&self, value: &'v Value, env: &Env, at: &str) -> R<&'v str> {
        let Some(name) = value.as_str().filter(|name| identifier(name)) else {
            return fail(at, "a loop variable needs an identifier");
        };
        if env.get(name).is_some() || self.params.contains_key(name) {
            return fail(
                at,
                format!(
                    "`{name}` is declared outside this loop; the loop variable needs a new name"
                ),
            );
        }
        Ok(name)
    }

    fn range_stmt(&mut self, items: &[Value], env: &mut Env, at: &str) -> R<()> {
        if items.len() != 5 {
            return fail(at, "[\"range\", name, start, end, [body...]]");
        }
        let var = self.loop_name(&items[1], env, &format!("{at}/1"))?;
        let Some(body) = items[4].as_array() else {
            return fail(&format!("{at}/4"), "a range body is a statement list");
        };
        let ty = self
            .static_type(&items[2], env, None)
            .or_else(|| self.static_type(&items[3], env, None))
            .unwrap_or_else(|| "i64".into());
        if !is_int(&ty) {
            return fail(at, "range bounds must have the same integer type");
        }
        let mut keys = Vec::new();
        // Keep the start across any control flow in the end expression;
        // both are evaluated once, left to right, even for an empty range.
        for (index, bound) in items[2..4].iter().enumerate() {
            let here = format!("{at}/{}", index + 2);
            let (value, actual) = self.expr(bound, env, &here, Some(&ty))?;
            if actual != ty {
                return fail(&here, format!("range bounds mix {ty} and {actual}"));
            }
            keys.push(self.hold(env, value, &ty));
        }
        let before = env.clone();
        let (local, header) = self.enter_loop(env, at);
        let more = self.emit_value(
            "lt",
            vec![local.operand(&keys[0]), local.operand(&keys[1])],
            at,
        );
        let (mut inner, exit_env) = self.test_loop(&more, &local, at);
        inner.set(
            var,
            Binding {
                operand: inner.operand(&keys[0]),
                ty: ty.clone(),
                mutable: false,
            },
        );
        let pending = self.pending_exit.take();
        self.loops.push(LoopCx {
            exit: pending.as_ref().map(|b| b.name.clone()).unwrap_or_default(),
            exit_args: local.clone(),
            header: header.clone(),
            before: before.clone(),
            step: Some(keys[0].clone()),
        });
        self.depth += 1;
        let falls = self.stmts(body, &mut inner, &format!("{at}/4"), false)?;
        self.depth -= 1;
        self.loops.pop();
        self.pending_exit = pending;
        if falls {
            // Since index < end <= MAX, this checked step cannot overflow.
            self.advance(&mut inner, &keys[0], at);
        }
        self.close_loop(
            &header,
            falls.then_some(&inner),
            &before,
            &exit_env,
            env,
            at,
        );
        Self::release(env, &keys);
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn for_stmt(&mut self, items: &[Value], env: &mut Env, at: &str) -> R<()> {
        let indexed = items[0] == "for-indexed";
        let offset = usize::from(indexed);
        let grammar = if indexed {
            "[\"for-indexed\", index, element, vector, [body...]]"
        } else {
            "[\"for\", element, vector, [body...]]"
        };
        if items.len() != 4 + offset {
            return fail(at, grammar);
        }
        let var = self.loop_name(&items[1 + offset], env, &format!("{at}/{}", 1 + offset))?;
        let visible_index = if indexed {
            let name = self.loop_name(&items[1], env, &format!("{at}/1"))?;
            if name == var {
                return fail(&format!("{at}/2"), "index and element need different names");
            }
            Some(name)
        } else {
            None
        };
        let list = &items[2 + offset];
        let Some(body) = items[3 + offset].as_array() else {
            return fail(at, grammar);
        };
        let (vec, vty) = self.expr(list, env, &format!("{at}/{}", 2 + offset), None)?;
        let Some(elem) = vec_elem(&vty) else {
            return fail(
                &format!("{at}/{}", 2 + offset),
                format!("`for` iterates a vector, not {vty}"),
            );
        };
        let (index, vector) = (self.fresh("-i"), self.fresh("-vec"));
        env.set(
            &index,
            Binding {
                operand: Self::lit(&json!(0), "u64"),
                ty: "u64".into(),
                mutable: true,
            },
        );
        env.set(
            &vector,
            Binding {
                operand: vec,
                ty: vty,
                mutable: false,
            },
        );
        let before = env.clone();
        let (local, header) = self.enter_loop(env, at);
        let current = local.operand(&vector);
        let length = self.emit_value("vec_len", vec![current], at);
        let position = local.operand(&index);
        let more = self.emit_value("lt", vec![position, length], at);
        let (mut inner, exit_env) = self.test_loop(&more, &local, at);
        let trap = self.trap_block();
        let (current, position) = (inner.operand(&vector), inner.operand(&index));
        let item = self.emit_value(&format!("vec_get?{trap}"), vec![current, position], at);
        if let Some(name) = visible_index {
            inner.set(
                name,
                Binding {
                    operand: inner.operand(&index),
                    ty: "u64".into(),
                    mutable: false,
                },
            );
        }
        inner.set(
            var,
            Binding {
                operand: item,
                ty: elem,
                mutable: false,
            },
        );
        let pending = self.pending_exit.take();
        self.loops.push(LoopCx {
            exit: pending.as_ref().map(|b| b.name.clone()).unwrap_or_default(),
            exit_args: local.clone(),
            header: header.clone(),
            before: before.clone(),
            step: Some(index.clone()),
        });
        self.depth += 1;
        let falls = self.stmts(body, &mut inner, &format!("{at}/{}", 3 + offset), false)?;
        self.depth -= 1;
        self.loops.pop();
        self.pending_exit = pending;
        if falls {
            self.advance(&mut inner, &index, at);
        }
        self.close_loop(
            &header,
            falls.then_some(&inner),
            &before,
            &exit_env,
            env,
            at,
        );
        env.remove(&index);
        env.remove(&vector);
        Ok(())
    }

    fn while_stmt(&mut self, items: &[Value], env: &mut Env, at: &str, internal: bool) -> R<()> {
        let Some(body) = items
            .get(2)
            .and_then(Value::as_array)
            .filter(|_| items.len() == 3)
        else {
            return fail(at, "[\"while\", condition, [body...]]");
        };
        let part = |i: usize| {
            if internal {
                at.to_owned()
            } else {
                format!("{at}/{i}")
            }
        };
        let before = env.clone();
        let (mut local, header) = self.enter_loop(env, at);
        let (c, tc) = self.expr(&items[1], &mut local, &part(1), None)?;
        if tc != "bool" {
            return fail(&part(1), "a `while` condition is bool");
        }
        let (mut inner, exit_env) = self.test_loop(&c, &local, at);
        let pending = self.pending_exit.take();
        self.loops.push(LoopCx {
            exit: pending.as_ref().map(|b| b.name.clone()).unwrap_or_default(),
            exit_args: local.clone(),
            header: header.clone(),
            before: before.clone(),
            step: None,
        });
        self.depth += 1;
        let falls = self.stmts(body, &mut inner, &part(2), internal)?;
        self.depth -= 1;
        self.loops.pop();
        self.pending_exit = pending;
        self.close_loop(
            &header,
            falls.then_some(&inner),
            &before,
            &exit_env,
            env,
            at,
        );
        Ok(())
    }

    fn lower(&mut self) -> R<Vec<Value>> {
        let Some(body) = self.decl.get("body").and_then(Value::as_array) else {
            return fail(&format!("{}/body", self.at), "a body is a statement list");
        };
        let name = self.fresh("b");
        self.cur = Some(Block {
            name,
            params: Vec::new(),
            ops: Vec::new(),
        });
        let mut env = Env::default();
        let at = format!("{}/body", self.at);
        if self.stmts(body, &mut env, &at, false)? {
            return fail(&at, "the body can reach its end without return, ok or fail");
        }
        if let Some(trap) = self.trap.clone() {
            self.cur = Some(Block {
                name: trap,
                params: Vec::new(),
                ops: Vec::new(),
            });
            self.end(json!(["trap"]), &at);
        }
        Ok(std::mem::take(&mut self.blocks))
    }
}

//! Structured function bodies: an AF1-X function may carry `"body"` (a list
//! of statements) instead of `"blocks"`. The lowering here constructs the
//! blocks, loop back edges, index plumbing and state forwarding, and emits an
//! ordinary AF1-X function that the unchanged expander, compiler and kernel
//! then handle. It never changes what a program means: every construct maps
//! onto existing operations and terminators.
//!
//! Statements: `["let", x, e]`, `["var", x, T, e]`, `["set", x, e]`,
//! `["if", c, [..], [..]?]`, `["for", x, xs, [..]]`, `["while", c, [..]]`,
//! `["return", e]`, `["ok", e]`, `["fail", Case]`, `["trap"]`.
//!
//! Expressions: a name, an integer, `true`/`false`, a typed literal
//! `{"type": T, "value": v}`, or `[op, args..]`:
//! - `add sub mul div rem neg abs`: checked; a failure traps, unless the
//!   operation is spelled `op?` (return the `ArithmeticError` from a Result
//!   function) or `op?Case` (return error case `Case`).
//! - `to`: `["to", T, x]` converts between integer types; a value outside
//!   `T` fails like a checked operation (`to?`, `to?Case`).
//! - `min max clamp`: total; `clamp(x, lo, hi)` is `min(max(x, lo), hi)`.
//! - `eq ne lt le gt ge not`; `and or` evaluate both operands.
//! - `if`: `["if", c, a, b]` computes only the chosen value.
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

/// Whether any function of the frame uses a structured body.
pub(crate) fn uses_bodies(frame: &Map<String, Value>) -> bool {
    ["fns", "functions", "patch"].iter().any(|key| {
        frame
            .get(*key)
            .and_then(Value::as_array)
            .is_some_and(|list| list.iter().any(|f| f.get("body").is_some()))
    })
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
    let mut out = frame.clone();
    let mut origins = Vec::new();
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
                        origins.push((format!("{at}/blocks/{block}/{part}"), where_));
                    }
                }
                Err(failure) => {
                    problems.push(Obligation::new(
                        AgentErrorCode::FrameInvalid,
                        &failure.at,
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
                    "to" => args.first().and_then(Value::as_str).map(normal),
                    "add" | "sub" | "mul" | "div" | "rem" | "neg" | "abs" | "min" | "max"
                    | "clamp" => args.iter().find_map(|a| self.static_type(a, env, None)),
                    "get" => vec_elem(&self.static_type(args.first()?, env, None)?),
                    "field" => {
                        let rec = self.static_type(args.first()?, env, None)?;
                        self.record_fields(&rec)?
                            .get(args.get(1)?.as_str()?)
                            .cloned()
                    }
                    "call" => {
                        let (_, returns) = self.signature(args.first()?.as_str()?)?;
                        if mode.is_some() {
                            result_parts(&returns).map(|p| p.0)
                        } else {
                            Some(returns)
                        }
                    }
                    "if" => self
                        .static_type(args.get(1)?, env, hint)
                        .or_else(|| self.static_type(args.get(2)?, env, hint)),
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
                    if items.len() != 4 {
                        return fail(&here, "[\"var\", name, type, value]");
                    }
                    let n = name_ok(1)?;
                    self.no_shadow(env, outer, &n, &here, internal)?;
                    let Some(ty) = items[2].as_str().map(normal) else {
                        return fail(&here, "[\"var\", name, type, value]");
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
                "for" => self.for_stmt(items, env, &here)?,
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

    fn for_stmt(&mut self, items: &[Value], env: &mut Env, at: &str) -> R<()> {
        let (Some(var), Some(list), Some(body)) = (
            items
                .get(1)
                .and_then(Value::as_str)
                .filter(|n| identifier(n)),
            items.get(2),
            items.get(3).and_then(Value::as_array),
        ) else {
            return fail(at, "[\"for\", name, vector, [body...]]");
        };
        if items.len() != 4 {
            return fail(at, "[\"for\", name, vector, [body...]]");
        }
        if env.get(var).is_some() || self.params.contains_key(var) {
            return fail(
                &format!("{at}/1"),
                format!(
                    "`{var}` is declared outside this loop; the loop variable needs a new name"
                ),
            );
        }
        let (vec, vty) = self.expr(list, env, &format!("{at}/2"), None)?;
        let Some(elem) = vec_elem(&vty) else {
            return fail(
                &format!("{at}/2"),
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
        inner.set(
            var,
            Binding {
                operand: item,
                ty: elem,
                mutable: false,
            },
        );
        let pending = self.pending_exit.take();
        self.depth += 1;
        let falls = self.stmts(body, &mut inner, &format!("{at}/3"), false)?;
        self.depth -= 1;
        self.pending_exit = pending;
        if falls {
            let position = inner.operand(&index);
            let next = self.emit_value(
                &format!("add?{trap}"),
                vec![position, Self::lit(&json!(1), "u64")],
                at,
            );
            inner.set(
                &index,
                Binding {
                    operand: next,
                    ty: "u64".into(),
                    mutable: true,
                },
            );
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
        self.depth += 1;
        let falls = self.stmts(body, &mut inner, &part(2), internal)?;
        self.depth -= 1;
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

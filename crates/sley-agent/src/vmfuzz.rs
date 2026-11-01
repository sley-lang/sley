//! A seeded random-program differential fuzzer for the VM's execution
//! tiers (issue #86, item 1).
//!
//! [`program`] turns a seed into a Python-dialect program that always
//! terminates: loops have literal bounds, recursion has a literal depth
//! bound, and every value the program computes is folded into its result.
//! The same seed gives the same program on every machine. [`check`]
//! compiles a program as [`crate::embed::compile_python_with`] does, under
//! [`LARGE_CEILINGS`], and runs
//! each case twice, on the reference interpreter
//! ([`ExecutionMode::Reference`]) and on the derived tiers the program is
//! eligible for ([`ExecutionMode::Auto`]: the word tier or the scalar
//! plan). The two must agree exactly on the termination, fuel,
//! instructions, peak value units and observation digest, under generous
//! limits and again at random cuts of every limit (fuel, cancellation,
//! instructions, value units, output units). [`shrink`] reduces a
//! disagreeing program, deepest statements first, to a smaller one that
//! still disagrees. Every finding replays from its seed.
//!
//! The fuzzer is a test tool: it never touches a workspace, and its runs
//! are advisory like every embedded call.

use std::fmt::{self, Write as _};

use sley_vm::{ExecutionLimits, ExecutionMode, ExecutionOutcome};

use crate::embed::{EmbedError, EmbedOptions, EmbedValue, LARGE_CEILINGS, compile_python_with};
use crate::exec::Executor;

/// The limits a case first runs under; the cuts lie below them.
pub const LIMITS: ExecutionLimits = ExecutionLimits {
    max_instructions: 5_000_000,
    max_fuel: 5_000_000,
    max_value_units: 50_000_000,
    max_output_units: 1_000_000,
    cancel_at_fuel: None,
};

/// The function every generated program exports, and every case calls.
pub const ENTRY: &str = "main";

// ------------------------------------------------------------- random

/// `SplitMix64`: small, fast and the same everywhere.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// A generator for `seed` and a `stream` (separate streams of one seed
    /// do not share draws).
    #[must_use]
    pub const fn new(seed: u64, stream: u64) -> Self {
        Self(seed ^ stream.wrapping_mul(0xA076_1D64_78BD_642F))
    }

    /// The next 64 bits.
    pub const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n` (`n > 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    /// A value in `lo..=hi`.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = hi.abs_diff(lo).saturating_add(1);
        lo.wrapping_add_unsigned(self.below(span))
    }

    /// True with probability `percent` in 100.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        let index = usize::try_from(self.below(items.len() as u64)).unwrap_or(0);
        items[index]
    }
}

// ---------------------------------------------------------------- AST

/// An integer expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    /// A literal.
    Lit(i64),
    /// A local, a parameter or a loop variable.
    Var(String),
    /// `a op b`: `+ - * // % << >>`, `&` with a mask, `**` with a small
    /// literal exponent.
    Bin(&'static str, Box<Expr>, Box<Expr>),
    /// `-e`.
    Neg(Box<Expr>),
    /// `abs(e)`, `min(a, b)`, `max(a, b)`.
    Builtin(&'static str, Vec<Expr>),
    /// A call to a helper of the program.
    Call(String, Vec<Expr>),
    /// `a if c else b`.
    If(Box<Cond>, Box<Expr>, Box<Expr>),
    /// `len(xs)`.
    Len(String),
    /// `xs[e]`.
    Index(String, Box<Expr>),
    /// `p.field`.
    Field(String, &'static str),
    /// `sum(e for x in xs if c)`, or `max`/`min` of the generator.
    Fold(&'static str, String, String, Box<Expr>, Option<Box<Cond>>),
}

/// A condition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Cond {
    /// `a op b` with `< <= > >= == !=`.
    Cmp(&'static str, Expr, Expr),
    /// `a < b < c`.
    Chain(Expr, Expr, Expr),
    /// `c and d`.
    And(Box<Cond>, Box<Cond>),
    /// `c or d`.
    Or(Box<Cond>, Box<Cond>),
    /// `not c`.
    Not(Box<Cond>),
    /// `any(c for x in xs)`.
    Any(String, String, Box<Cond>),
}

/// A statement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Stmt {
    /// `v = e`.
    Set(String, Expr),
    /// `v op= e`.
    Update(String, &'static str, Expr),
    /// `acc = (acc * 31 + e) % 1000003`: an observation folded into the
    /// result.
    Observe(Expr),
    /// `if c: .. else: ..`.
    If(Cond, Vec<Stmt>, Vec<Stmt>),
    /// `for i in range(lo, hi): ..`.
    Range(String, i64, i64, Vec<Stmt>),
    /// `for x in xs: ..`.
    For(String, String, Vec<Stmt>),
    /// `while w < bound: ..; w += 1` (`w` is the loop's own counter).
    While(String, i64, Vec<Stmt>),
    /// `if c: break` (`true`) or `if c: continue`, inside a loop.
    Leave(Cond, bool),
    /// `ys[i] = e` on a list local.
    Store(String, Expr, Expr),
    /// `ys.append(e)` on a list local.
    Append(String, Expr),
    /// `p = P(a, b)`.
    Build(String, Expr, Expr),
}

/// One function: `main(a, b, xs)`, a helper `hN(x, y)`, or a recursive
/// `rN(n, x)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    /// The function's name.
    pub name: String,
    /// The recursion template (`rN`): its step expression; the body is
    /// fixed.
    pub recursive: Option<Expr>,
    /// Integer parameters, then `xs: list[int]` when `list` is set.
    pub params: Vec<String>,
    /// Whether the last parameter is `xs: list[int]`.
    pub list: bool,
    /// Integer locals, each set to 0 first.
    pub locals: Vec<String>,
    /// List locals: name and initial length (all zeros).
    pub lists: Vec<(String, i64)>,
    /// Record locals of type `P`.
    pub records: Vec<String>,
    /// The body.
    pub body: Vec<Stmt>,
    /// The returned expression, combined with the observations in `acc`.
    pub result: Expr,
}

/// A generated program and the cases to run it on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Program {
    /// The seed it came from.
    pub seed: u64,
    /// Whether the record type `P(a, b)` is declared.
    pub record: bool,
    /// Helpers first, then `main`.
    pub functions: Vec<Function>,
    /// The arguments of each case: `a`, `b` and `xs`.
    pub cases: Vec<(i64, i64, Vec<i64>)>,
}

// ---------------------------------------------------------- generator

struct Scope<'p> {
    rng: Rng,
    /// Readable integer names: parameters, locals, loop variables.
    ints: Vec<String>,
    /// Assignable integer names (locals).
    locals: Vec<String>,
    /// Readable vectors and list locals, and whether each is a list local.
    seqs: Vec<(String, bool)>,
    records: Vec<String>,
    /// Lists a `for` is visiting: they may not change.
    visiting: Vec<String>,
    helpers: &'p [(String, usize)],
    depth: usize,
    loops: usize,
    counter: usize,
}

impl Scope<'_> {
    fn fresh(&mut self, base: &str) -> String {
        self.counter += 1;
        format!("{base}{}", self.counter)
    }

    fn lit(&mut self) -> i64 {
        match self.rng.below(20) {
            0 => self
                .rng
                .pick(&[i64::MAX, -i64::MAX, 1 << 40, -(1 << 33), 1_000_003]),
            1..=4 => self.rng.range(-1000, 1000),
            _ => self.rng.range(-9, 9),
        }
    }

    fn expr(&mut self, depth: usize) -> Expr {
        let leaf = depth == 0 || self.rng.chance(30);
        if leaf {
            return if self.ints.is_empty() || self.rng.chance(35) {
                Expr::Lit(self.lit())
            } else {
                let index = usize::try_from(self.rng.below(self.ints.len() as u64)).unwrap_or(0);
                Expr::Var(self.ints[index].clone())
            };
        }
        let sub = |scope: &mut Self| Box::new(scope.expr(depth - 1));
        match self.rng.below(16) {
            0..=3 => {
                let op = self.rng.pick(&["+", "-", "*", "+", "-"]);
                Expr::Bin(op, sub(self), sub(self))
            }
            4 => {
                let op = self.rng.pick(&["//", "%"]);
                Expr::Bin(op, sub(self), sub(self))
            }
            5 => {
                let op = self.rng.pick(&["<<", ">>"]);
                let amount = self.rng.range(0, 6);
                Expr::Bin(op, sub(self), Box::new(Expr::Lit(amount)))
            }
            6 => {
                let mask = self.rng.pick(&[1, 3, 255, 1023]);
                Expr::Bin("&", sub(self), Box::new(Expr::Lit(mask)))
            }
            7 => {
                let power = self.rng.range(0, 3);
                Expr::Bin("**", sub(self), Box::new(Expr::Lit(power)))
            }
            8 => Expr::Neg(sub(self)),
            9 => match self.rng.below(3) {
                0 => Expr::Builtin("abs", vec![self.expr(depth - 1)]),
                1 => Expr::Builtin("min", vec![self.expr(depth - 1), self.expr(depth - 1)]),
                _ => Expr::Builtin("max", vec![self.expr(depth - 1), self.expr(depth - 1)]),
            },
            10 => Expr::If(Box::new(self.cond(depth - 1)), sub(self), sub(self)),
            11 if !self.helpers.is_empty() => {
                let index = usize::try_from(self.rng.below(self.helpers.len() as u64)).unwrap_or(0);
                let (name, arity) = self.helpers[index].clone();
                if name.starts_with('r') {
                    // Recursion depth at most 24.
                    let bound = Expr::Builtin(
                        "min",
                        vec![
                            Expr::Builtin("abs", vec![self.expr(depth - 1)]),
                            Expr::Lit(24),
                        ],
                    );
                    Expr::Call(name, vec![bound, self.expr(depth - 1)])
                } else {
                    Expr::Call(name, (0..arity).map(|_| self.expr(depth - 1)).collect())
                }
            }
            12 | 13 if !self.seqs.is_empty() => {
                let index = usize::try_from(self.rng.below(self.seqs.len() as u64)).unwrap_or(0);
                let name = self.seqs[index].0.clone();
                if self.rng.chance(25) {
                    return Expr::Len(name);
                }
                // Mostly in range; sometimes not (a trap is an outcome too).
                let at = if self.rng.chance(80) {
                    Expr::Bin("%", sub(self), Box::new(Expr::Len(name.clone())))
                } else {
                    self.expr(depth - 1)
                };
                Expr::Index(name, Box::new(at))
            }
            14 if !self.records.is_empty() => {
                let index = usize::try_from(self.rng.below(self.records.len() as u64)).unwrap_or(0);
                Expr::Field(self.records[index].clone(), self.rng.pick(&["a", "b"]))
            }
            15 if self.seqs.iter().any(|(_, local)| !local) => {
                let vectors: Vec<String> = self
                    .seqs
                    .iter()
                    .filter(|(_, local)| !local)
                    .map(|(name, _)| name.clone())
                    .collect();
                let list = vectors
                    [usize::try_from(self.rng.below(vectors.len() as u64)).unwrap_or(0)]
                .clone();
                let var = self.fresh("g");
                self.ints.push(var.clone());
                let body = self.expr(depth - 1);
                let guard = self.rng.chance(50).then(|| Box::new(self.cond(depth - 1)));
                self.ints.pop();
                let kind = self.rng.pick(&["sum", "sum", "max", "min"]);
                Expr::Fold(kind, var, list, Box::new(body), guard)
            }
            _ => Expr::Bin("+", sub(self), sub(self)),
        }
    }

    fn cond(&mut self, depth: usize) -> Cond {
        if depth == 0 || self.rng.chance(50) {
            let op = self.rng.pick(&["<", "<=", ">", ">=", "==", "!="]);
            return Cond::Cmp(op, self.expr(depth), self.expr(depth));
        }
        let sub = |scope: &mut Self| Box::new(scope.cond(depth - 1));
        match self.rng.below(5) {
            0 => Cond::And(sub(self), sub(self)),
            1 => Cond::Or(sub(self), sub(self)),
            2 => Cond::Not(sub(self)),
            3 => Cond::Chain(
                self.expr(depth - 1),
                self.expr(depth - 1),
                self.expr(depth - 1),
            ),
            _ => {
                let vectors: Vec<String> = self
                    .seqs
                    .iter()
                    .filter(|(_, local)| !local)
                    .map(|(name, _)| name.clone())
                    .collect();
                if vectors.is_empty() {
                    return Cond::Not(sub(self));
                }
                let list = vectors
                    [usize::try_from(self.rng.below(vectors.len() as u64)).unwrap_or(0)]
                .clone();
                let var = self.fresh("g");
                self.ints.push(var.clone());
                let body = self.cond(depth - 1);
                self.ints.pop();
                Cond::Any(var, list, Box::new(body))
            }
        }
    }

    /// A block of `lo..=hi` statements.
    fn random_block(&mut self, lo: i64, hi: i64) -> Vec<Stmt> {
        let count = usize::try_from(self.rng.range(lo, hi)).unwrap_or(1);
        self.block(count)
    }

    fn block(&mut self, count: usize) -> Vec<Stmt> {
        (0..count).map(|_| self.stmt()).collect()
    }

    fn stmt(&mut self) -> Stmt {
        let nested = self.depth < 2;
        let pick = self.rng.below(if nested { 14 } else { 7 });
        match pick {
            0 | 1 if !self.locals.is_empty() => {
                let index = usize::try_from(self.rng.below(self.locals.len() as u64)).unwrap_or(0);
                let name = self.locals[index].clone();
                if self.rng.chance(50) {
                    Stmt::Set(name, self.expr(2))
                } else {
                    let op = self.rng.pick(&["+=", "-=", "*=", "//=", "%="]);
                    Stmt::Update(name, op, self.expr(2))
                }
            }
            2 => {
                let lists: Vec<String> = self
                    .seqs
                    .iter()
                    .filter(|(name, local)| *local && !self.visiting.contains(name))
                    .map(|(name, _)| name.clone())
                    .collect();
                if lists.is_empty() {
                    return Stmt::Observe(self.expr(2));
                }
                let list =
                    lists[usize::try_from(self.rng.below(lists.len() as u64)).unwrap_or(0)].clone();
                if self.rng.chance(40) {
                    Stmt::Append(list, self.expr(2))
                } else {
                    let at = if self.rng.chance(85) {
                        Expr::Bin(
                            "%",
                            Box::new(self.expr(2)),
                            Box::new(Expr::Len(list.clone())),
                        )
                    } else {
                        self.expr(1)
                    };
                    Stmt::Store(list, at, self.expr(2))
                }
            }
            3 if !self.records.is_empty() => {
                let index = usize::try_from(self.rng.below(self.records.len() as u64)).unwrap_or(0);
                Stmt::Build(self.records[index].clone(), self.expr(2), self.expr(2))
            }
            4 if self.loops > 0 => Stmt::Leave(self.cond(1), self.rng.chance(50)),
            7 | 8 => {
                let c = self.cond(2);
                self.depth += 1;
                let then = self.random_block(1, 2);
                let otherwise = if self.rng.chance(50) {
                    self.random_block(1, 2)
                } else {
                    Vec::new()
                };
                self.depth -= 1;
                Stmt::If(c, then, otherwise)
            }
            9 | 10 => {
                let var = self.fresh("i");
                let lo = self.rng.range(-3, 3);
                let hi = lo + self.rng.range(0, 12);
                self.enter(&var);
                let body = self.random_block(1, 2);
                self.leave();
                Stmt::Range(var, lo, hi, body)
            }
            11 if self.seqs.iter().any(|(_, local)| !local) => {
                let vectors: Vec<String> = self
                    .seqs
                    .iter()
                    .filter(|(_, local)| !local)
                    .map(|(name, _)| name.clone())
                    .collect();
                let list = vectors
                    [usize::try_from(self.rng.below(vectors.len() as u64)).unwrap_or(0)]
                .clone();
                let var = self.fresh("x");
                self.visiting.push(list.clone());
                self.enter(&var);
                let body = self.random_block(1, 2);
                self.leave();
                self.visiting.pop();
                Stmt::For(var, list, body)
            }
            12 => {
                let counter = self.fresh("w");
                let bound = self.rng.range(0, 10);
                // The counter is readable but not assignable in the body.
                self.enter(&counter);
                let body = self.random_block(1, 2);
                self.leave();
                Stmt::While(counter, bound, body)
            }
            _ => Stmt::Observe(self.expr(2)),
        }
    }

    fn enter(&mut self, var: &str) {
        self.ints.push(var.to_owned());
        self.depth += 1;
        self.loops += 1;
    }

    fn leave(&mut self) {
        self.ints.pop();
        self.depth -= 1;
        self.loops -= 1;
    }
}

/// The program for `seed`.
#[must_use]
pub fn program(seed: u64) -> Program {
    let mut rng = Rng::new(seed, 1);
    let record = rng.chance(40);
    let mut helpers: Vec<(String, usize)> = Vec::new();
    let mut functions = Vec::new();
    for index in 0..rng.below(4) {
        let recursive = rng.chance(30);
        let name = format!("{}{index}", if recursive { 'r' } else { 'h' });
        let params: Vec<String> = if recursive {
            vec!["n".into(), "x".into()]
        } else {
            vec!["x".into(), "y".into()]
        };
        let mut scope = Scope {
            rng: Rng::new(seed, 100 + index),
            ints: params.clone(),
            locals: Vec::new(),
            seqs: Vec::new(),
            records: Vec::new(),
            visiting: Vec::new(),
            helpers: &helpers,
            depth: 0,
            loops: 0,
            counter: 0,
        };
        let function = if recursive {
            // if n <= 0: return x
            // return rN(n - 1, step) + x
            let step = scope.expr(2);
            Function {
                name: name.clone(),
                recursive: Some(step),
                params,
                list: false,
                locals: Vec::new(),
                lists: Vec::new(),
                records: Vec::new(),
                body: Vec::new(),
                result: Expr::Var("x".into()),
            }
        } else {
            scope.locals = vec!["t".into()];
            scope.ints.extend(["t".to_owned(), "acc".to_owned()]);
            let count = usize::try_from(scope.rng.range(1, 3)).unwrap_or(1);
            let body = scope.block(count);
            let result = scope.expr(2);
            Function {
                name: name.clone(),
                recursive: None,
                params,
                list: false,
                locals: vec!["t".into()],
                lists: Vec::new(),
                records: Vec::new(),
                body,
                result,
            }
        };
        functions.push(function);
        helpers.push((name, 2));
    }
    let locals: Vec<String> = (0..rng.range(1, 3)).map(|k| format!("v{k}")).collect();
    let lists: Vec<(String, i64)> = (0..rng.below(3))
        .map(|k| (format!("ys{k}"), rng.range(0, 5)))
        .collect();
    let records: Vec<String> = if record { vec!["p".into()] } else { Vec::new() };
    let mut ints = vec!["a".to_owned(), "b".to_owned(), "acc".to_owned()];
    ints.extend(locals.iter().cloned());
    let mut seqs = vec![("xs".to_owned(), false)];
    seqs.extend(lists.iter().map(|(name, _)| (name.clone(), true)));
    let mut scope = Scope {
        rng: Rng::new(seed, 2),
        ints,
        locals: locals.clone(),
        seqs,
        records: records.clone(),
        visiting: Vec::new(),
        helpers: &helpers,
        depth: 0,
        loops: 0,
        counter: 0,
    };
    let count = usize::try_from(scope.rng.range(2, 5)).unwrap_or(2);
    let body = scope.block(count);
    let result = scope.expr(2);
    functions.push(Function {
        name: ENTRY.into(),
        recursive: None,
        params: vec!["a".into(), "b".into()],
        list: true,
        locals,
        lists,
        records,
        body,
        result,
    });
    let mut cases_rng = Rng::new(seed, 3);
    let cases = (0..3)
        .map(|_| {
            let value = |rng: &mut Rng| {
                if rng.chance(10) {
                    rng.pick(&[i64::MAX, -i64::MAX, 1 << 40, -1])
                } else {
                    rng.range(-20, 20)
                }
            };
            let a = value(&mut cases_rng);
            let b = value(&mut cases_rng);
            let len = cases_rng.below(7);
            let xs = (0..len).map(|_| cases_rng.range(-50, 50)).collect();
            (a, b, xs)
        })
        .collect();
    Program {
        seed,
        record,
        functions,
        cases,
    }
}

// ---------------------------------------------------------- rendering

fn call(f: &mut fmt::Formatter<'_>, name: &str, args: &[Expr]) -> fmt::Result {
    write!(f, "{name}(")?;
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{arg}")?;
    }
    f.write_str(")")
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lit(value) if *value < 0 => write!(f, "({value})"),
            Self::Lit(value) => write!(f, "{value}"),
            Self::Var(name) => f.write_str(name),
            Self::Bin(op, a, b) => write!(f, "({a} {op} {b})"),
            Self::Neg(e) => write!(f, "(-{e})"),
            Self::Builtin(name, args) => call(f, name, args),
            Self::Call(name, args) => call(f, name, args),
            Self::If(c, a, b) => write!(f, "({a} if {c} else {b})"),
            Self::Len(name) => write!(f, "len({name})"),
            Self::Index(name, at) => write!(f, "{name}[{at}]"),
            Self::Field(name, field) => write!(f, "{name}.{field}"),
            Self::Fold(kind, var, list, body, guard) => match guard {
                Some(guard) => write!(f, "{kind}({body} for {var} in {list} if {guard})"),
                None => write!(f, "{kind}({body} for {var} in {list})"),
            },
        }
    }
}

impl fmt::Display for Cond {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cmp(op, a, b) => write!(f, "({a} {op} {b})"),
            Self::Chain(a, b, c) => write!(f, "({a} < {b} < {c})"),
            Self::And(a, b) => write!(f, "({a} and {b})"),
            Self::Or(a, b) => write!(f, "({a} or {b})"),
            Self::Not(c) => write!(f, "(not {c})"),
            Self::Any(var, list, c) => write!(f, "any({c} for {var} in {list})"),
        }
    }
}

fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("    ");
    }
}

fn render_block(out: &mut String, body: &[Stmt], level: usize) {
    if body.is_empty() {
        indent(out, level);
        out.push_str("acc = acc\n");
    }
    for stmt in body {
        render_stmt(out, stmt, level);
    }
}

fn render_stmt(out: &mut String, stmt: &Stmt, level: usize) {
    indent(out, level);
    match stmt {
        Stmt::Set(name, e) => {
            let _ = writeln!(out, "{name} = {e}");
        }
        Stmt::Update(name, op, e) => {
            let _ = writeln!(out, "{name} {op} {e}");
        }
        Stmt::Observe(e) => {
            let _ = writeln!(out, "acc = (acc * 31 + {e}) % 1000003");
        }
        Stmt::If(c, then, otherwise) => {
            let _ = writeln!(out, "if {c}:");
            render_block(out, then, level + 1);
            if !otherwise.is_empty() {
                indent(out, level);
                out.push_str("else:\n");
                render_block(out, otherwise, level + 1);
            }
        }
        Stmt::Range(var, lo, hi, body) => {
            let _ = writeln!(
                out,
                "for {var} in range({}, {}):",
                Expr::Lit(*lo),
                Expr::Lit(*hi)
            );
            render_block(out, body, level + 1);
        }
        Stmt::For(var, list, body) => {
            let _ = writeln!(out, "for {var} in {list}:");
            render_block(out, body, level + 1);
        }
        Stmt::While(counter, bound, body) => {
            let _ = writeln!(out, "{counter} = 0");
            indent(out, level);
            let _ = writeln!(out, "while {counter} < {bound}:");
            indent(out, level + 1);
            let _ = writeln!(out, "{counter} += 1");
            render_block(out, body, level + 1);
        }
        Stmt::Leave(c, brk) => {
            let _ = writeln!(out, "if {c}:");
            indent(out, level + 1);
            out.push_str(if *brk { "break\n" } else { "continue\n" });
        }
        Stmt::Store(list, at, e) => {
            let _ = writeln!(out, "{list}[{at}] = {e}");
        }
        Stmt::Append(list, e) => {
            let _ = writeln!(out, "{list}.append({e})");
        }
        Stmt::Build(name, a, b) => {
            let _ = writeln!(out, "{name} = P({a}, {b})");
        }
    }
}

/// While counters declared by a block, so each is set to 0 first.
fn counters(body: &[Stmt], out: &mut Vec<String>) {
    for stmt in body {
        match stmt {
            Stmt::While(counter, _, inner) => {
                out.push(counter.clone());
                counters(inner, out);
            }
            Stmt::If(_, a, b) => {
                counters(a, out);
                counters(b, out);
            }
            Stmt::Range(_, _, _, inner) | Stmt::For(_, _, inner) => counters(inner, out),
            _ => {}
        }
    }
}

impl Program {
    /// The program as Python-dialect text.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = String::new();
        if self.record {
            out.push_str("@dataclass\nclass P:\n    a: int\n    b: int\n\n");
        }
        for function in &self.functions {
            let mut params: Vec<String> = function
                .params
                .iter()
                .map(|p| format!("{p}: int"))
                .collect();
            if function.list {
                params.push("xs: list[int]".into());
            }
            let _ = writeln!(out, "def {}({}) -> int:", function.name, params.join(", "));
            if let Some(step) = &function.recursive {
                let _ = writeln!(
                    out,
                    "    if n <= 0:\n        return x\n    return {}(n - 1, {step}) + x\n",
                    function.name
                );
                continue;
            }
            out.push_str("    acc = 0\n");
            for local in &function.locals {
                let _ = writeln!(out, "    {local} = 0");
            }
            let mut whiles = Vec::new();
            counters(&function.body, &mut whiles);
            for counter in &whiles {
                let _ = writeln!(out, "    {counter} = 0");
            }
            for (list, len) in &function.lists {
                let _ = writeln!(out, "    {list} = [0] * {len}");
            }
            for record in &function.records {
                let _ = writeln!(out, "    {record} = P(0, 0)");
            }
            render_block(&mut out, &function.body, 1);
            let _ = writeln!(
                out,
                "    return (acc * 7 + {}) % 1000003\n",
                function.result
            );
        }
        out
    }

    /// Each case's arguments, as [`EmbedValue`]s.
    #[must_use]
    pub fn args(&self) -> Vec<Vec<EmbedValue>> {
        self.cases
            .iter()
            .map(|(a, b, xs)| {
                vec![
                    EmbedValue::Int(*a),
                    EmbedValue::Int(*b),
                    EmbedValue::IntList(xs.clone()),
                ]
            })
            .collect()
    }
}

// ------------------------------------------------------------ checking

/// Two executions that should have agreed.
#[derive(Clone, Debug)]
pub struct Disagreement {
    /// The case's arguments.
    pub args: Vec<EmbedValue>,
    /// The limits both ran under.
    pub limits: ExecutionLimits,
    /// The reference interpreter's outcome.
    pub reference: ExecutionOutcome,
    /// The derived tiers' outcome.
    pub derived: ExecutionOutcome,
}

impl fmt::Display for Disagreement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let show = |o: &ExecutionOutcome| {
            format!(
                "{:?}, fuel {}, instructions {}, peak value units {}",
                o.termination, o.fuel_used, o.instruction_count, o.peak_value_units
            )
        };
        write!(
            f,
            "args {:?} under {:?}:\n  reference: {}\n  derived:   {}",
            self.args,
            self.limits,
            show(&self.reference),
            show(&self.derived)
        )
    }
}

/// What checking one program found.
#[derive(Clone, Debug)]
pub enum Verdict {
    /// Every run agreed: how many pairs of runs there were.
    Agree(usize),
    /// A pair of runs disagreed.
    Disagree(Box<Disagreement>),
}

/// The cuts of a case: limits below each total the generous run reached.
fn cuts(baseline: &ExecutionOutcome, rng: &mut Rng) -> Vec<ExecutionLimits> {
    let fuel = baseline.fuel_used;
    let mut fuels: Vec<u64> = (0..6).map(|_| rng.below(fuel + 1)).collect();
    fuels.extend([fuel.saturating_sub(1), fuel, fuel + 1, 0]);
    let mut out: Vec<ExecutionLimits> = fuels
        .iter()
        .map(|&cut| ExecutionLimits {
            max_fuel: cut,
            ..LIMITS
        })
        .collect();
    for _ in 0..3 {
        out.push(ExecutionLimits {
            cancel_at_fuel: Some(rng.below(fuel + 1)),
            ..LIMITS
        });
    }
    let instructions = baseline.instruction_count;
    for cut in [
        rng.below(instructions + 1),
        rng.below(instructions + 1),
        instructions.saturating_sub(1),
    ] {
        out.push(ExecutionLimits {
            max_instructions: cut,
            ..LIMITS
        });
    }
    let units = baseline.peak_value_units;
    for cut in [rng.below(units + 1), units.saturating_sub(1)] {
        out.push(ExecutionLimits {
            max_value_units: cut,
            ..LIMITS
        });
    }
    out.push(ExecutionLimits {
        max_output_units: rng.below(8),
        ..LIMITS
    });
    out
}

/// Runs `args` cases of `text`'s [`ENTRY`] on the reference interpreter and
/// on the derived tiers, under [`LIMITS`] and then at random cuts drawn
/// from `seed`, and compares the whole outcomes.
///
/// # Errors
///
/// The compile refusal when `text` is not a Valid program, and the
/// execution refusal when a case's arguments do not fit.
pub fn check(text: &str, args: &[Vec<EmbedValue>], seed: u64) -> Result<Verdict, EmbedError> {
    let module = compile_python_with(text, &EmbedOptions::default(), LARGE_CEILINGS)?;
    let refused = |error: crate::error::AgentError| EmbedError::of(&error);
    let mut reference = Executor::with_execution_mode(module.program(), ExecutionMode::Reference)
        .map_err(refused)?;
    let mut derived =
        Executor::with_execution_mode(module.program(), ExecutionMode::Auto).map_err(refused)?;
    let mut rng = Rng::new(seed, 4);
    let mut runs = 0;
    for case in args {
        let (id, inputs) = module.inputs(ENTRY, case)?;
        let run = |executor: &mut Executor, limits: ExecutionLimits| {
            executor
                .run_exact(&id, inputs.clone(), limits)
                .map_err(refused)
        };
        let baseline = run(&mut reference, LIMITS)?;
        let mut all = vec![LIMITS];
        all.extend(cuts(&baseline, &mut rng));
        for limits in all {
            let expected = run(&mut reference, limits)?;
            let observed = run(&mut derived, limits)?;
            runs += 1;
            if expected != observed {
                return Ok(Verdict::Disagree(Box::new(Disagreement {
                    args: case.clone(),
                    limits,
                    reference: expected,
                    derived: observed,
                })));
            }
        }
    }
    Ok(Verdict::Agree(runs))
}

/// Generates, compiles and checks the program for `seed`.
///
/// # Errors
///
/// A compile refusal of the generated text: a generator defect, since
/// every generated program is meant to be Valid.
pub fn run(seed: u64) -> Result<Verdict, EmbedError> {
    let program = program(seed);
    check(&program.text(), &program.args(), seed)
}

// ---------------------------------------------------------- shrinking

/// Every block of `functions`, by path: function index, then statement
/// indexes into nested blocks (`then` is branch 0, `else` 1).
fn statement_paths(body: &[Stmt], prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    for (index, stmt) in body.iter().enumerate() {
        prefix.push(index);
        out.push(prefix.clone());
        match stmt {
            Stmt::If(_, a, b) => {
                prefix.push(0);
                statement_paths(a, prefix, out);
                prefix.pop();
                prefix.push(1);
                statement_paths(b, prefix, out);
                prefix.pop();
            }
            Stmt::Range(_, _, _, inner) | Stmt::For(_, _, inner) | Stmt::While(_, _, inner) => {
                prefix.push(0);
                statement_paths(inner, prefix, out);
                prefix.pop();
            }
            _ => {}
        }
        prefix.pop();
    }
}

/// The block holding the statement at `path`, and the statement's index.
fn block_at<'b>(body: &'b mut Vec<Stmt>, path: &[usize]) -> Option<(&'b mut Vec<Stmt>, usize)> {
    match path {
        [index] => Some((body, *index)),
        [index, branch, rest @ ..] => match body.get_mut(*index)? {
            Stmt::If(_, a, b) => block_at(if *branch == 0 { a } else { b }, rest),
            Stmt::Range(_, _, _, inner) | Stmt::For(_, _, inner) | Stmt::While(_, _, inner) => {
                block_at(inner, rest)
            }
            _ => None,
        },
        [] => None,
    }
}

/// Smaller programs one step from `program`: each statement deleted
/// (deepest first), each compound statement replaced by its body, loop
/// bounds halved, expressions replaced by `0`, and each helper deleted.
fn neighbors(program: &Program) -> Vec<Program> {
    let mut out = Vec::new();
    for (f, function) in program.functions.iter().enumerate() {
        let mut paths = Vec::new();
        statement_paths(&function.body, &mut Vec::new(), &mut paths);
        paths.sort_by_key(|path| std::cmp::Reverse(path.len()));
        for path in &paths {
            let mut next = program.clone();
            if let Some((block, index)) = block_at(&mut next.functions[f].body, path) {
                let removed = block.remove(index);
                out.push(next.clone());
                // Replace a compound statement by its body.
                let inner = match removed {
                    Stmt::If(_, a, b) => vec![a, b],
                    Stmt::Range(_, _, _, inner)
                    | Stmt::For(_, _, inner)
                    | Stmt::While(_, _, inner) => {
                        vec![inner]
                    }
                    _ => Vec::new(),
                };
                for body in inner {
                    let mut lifted = program.clone();
                    if let Some((block, index)) = block_at(&mut lifted.functions[f].body, path) {
                        block.splice(index..=index, body);
                        out.push(lifted);
                    }
                }
            }
            let mut halved = program.clone();
            if let Some((block, index)) = block_at(&mut halved.functions[f].body, path) {
                let changed = match &mut block[index] {
                    Stmt::Range(_, lo, hi, _) if *hi > *lo => {
                        *hi = *lo + (*hi - *lo) / 2;
                        true
                    }
                    Stmt::While(_, bound, _) if *bound > 0 => {
                        *bound /= 2;
                        true
                    }
                    Stmt::Set(_, e)
                    | Stmt::Update(_, _, e)
                    | Stmt::Observe(e)
                    | Stmt::Append(_, e)
                        if *e != Expr::Lit(0) =>
                    {
                        *e = Expr::Lit(0);
                        true
                    }
                    _ => false,
                };
                if changed {
                    out.push(halved);
                }
            }
        }
        if function.result != Expr::Lit(0) {
            let mut next = program.clone();
            next.functions[f].result = Expr::Lit(0);
            out.push(next);
        }
        if function.name != ENTRY {
            let mut next = program.clone();
            next.functions.remove(f);
            out.push(next);
        }
    }
    out
}

/// Reduces `program` while `keeps` holds, one smaller neighbor at a time,
/// for at most `budget` checks. `keeps` is typically "still compiles and
/// still disagrees on the same case".
pub fn shrink(
    program: &Program,
    budget: usize,
    mut keeps: impl FnMut(&Program) -> bool,
) -> Program {
    let mut best = program.clone();
    let mut spent = 0;
    'outer: loop {
        for candidate in neighbors(&best) {
            if spent >= budget {
                break 'outer;
            }
            spent += 1;
            if keeps(&candidate) {
                best = candidate;
                continue 'outer;
            }
        }
        break;
    }
    best
}

/// Shrinks a disagreeing program to a smaller one that disagrees on the
/// same case, within `budget` checks.
#[must_use]
pub fn shrink_finding(program: &Program, finding: &Disagreement, budget: usize) -> Program {
    let args = vec![finding.args.clone()];
    let mut only = program.clone();
    only.cases.retain(|(a, b, xs)| {
        vec![
            EmbedValue::Int(*a),
            EmbedValue::Int(*b),
            EmbedValue::IntList(xs.clone()),
        ] == finding.args
    });
    shrink(&only, budget, |candidate| {
        matches!(
            check(&candidate.text(), &args, candidate.seed),
            Ok(Verdict::Disagree(_))
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_program() {
        for seed in 0..20 {
            assert_eq!(program(seed), program(seed));
            assert_eq!(program(seed).text(), program(seed).text());
        }
        assert_ne!(program(1).text(), program(2).text());
    }

    #[test]
    fn shrinking_keeps_the_property_and_removes_the_rest() {
        // Keep any program whose text still has a `%`: the shrinker must
        // find a much smaller one.
        let start = (0..200)
            .map(program)
            .find(|p| p.text().matches('%').count() > 3 && p.text().lines().count() > 25)
            .expect("a large program");
        let small = shrink(&start, 5_000, |p| p.text().contains(" % "));
        assert!(small.text().contains(" % "));
        assert!(
            small.text().lines().count() < start.text().lines().count() / 2,
            "{}",
            small.text()
        );
    }
}

//! The optional familiar authoring frontend (cargo feature `familiar`,
//! ADR-0055): a small statement syntax parsed into exactly the structured
//! `body` frame of [`crate::structured`] (ADR-0054).
//!
//! The text is a proposal at the authoring boundary. The frame it parses to
//! takes the ordinary path: the shared structured lowering, the AF1-X
//! expander, the frame compiler and the kernel's candidate validation. The
//! text adds no semantics of its own, is never canonical program state, is
//! never rendered from a program, and is never needed to query, edit,
//! validate, execute, export or reconstruct a committed program.
//!
//! [`Positions`] map the structured frame's pointers to the line and column
//! of the authored text. They serve diagnostics only and never carry
//! identity.
//!
//! ```text
//! fn name(p: T, q: T) -> T {
//!     let x = e            var t: T = e          t = e
//!     if c { .. } else if c { .. } else { .. }
//!     for x in xs { .. }   while c { .. }
//!     return e             return ok(e)          return err(Case)      trap
//! }
//! ```
//!
//! Expressions, loosest first: `a if c else b` (lazy, on one line), `or`,
//! `and`, `not`, `== != < <= > >=` (no chaining), `+ -`, `* / %`, unary `-`,
//! then `f(..)`, `x.field`, `xs[i]`. Built-ins `min max abs clamp len`,
//! `to(T, x)`, `try(e)` and `try(e, Case)`. Literals `12`, `7u64`, `true`,
//! `false`, `"text"`. Comments are `#` to the end of a line, or a line that
//! starts with `//`. Every other construct is refused at its line and
//! column.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

const KEYWORDS: [&str; 15] = [
    "fn", "let", "var", "if", "else", "for", "in", "while", "return", "trap", "and", "or", "not",
    "true", "false",
];
/// Names a function may not take: the built-ins and the result forms.
const RESERVED_CALLS: [&str; 9] = [
    "min", "max", "abs", "clamp", "len", "to", "try", "ok", "err",
];
const INT_SUFFIXES: [&str; 10] = [
    "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128",
];
/// Operations that `try(..)` turns from trapping into returning the failure.
const FAILING: [&str; 9] = ["add", "sub", "mul", "div", "rem", "neg", "abs", "get", "to"];

/// A textual location: 1-based line and column (in characters), 0-based byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pos {
    /// Line, from 1.
    pub line: usize,
    /// Column in characters, from 1.
    pub column: usize,
    /// Byte offset, from 0.
    pub byte: usize,
}

/// Text that the frontend refuses, with where.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamiliarError {
    /// What is wrong and what to write instead.
    pub message: String,
    /// Where.
    pub at: Pos,
}

/// Structured-frame pointer to authored text position.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Positions(BTreeMap<String, Pos>);

impl Positions {
    /// The position of `pointer`, or of its nearest recorded ancestor.
    #[must_use]
    pub fn locate(&self, pointer: &str) -> Option<Pos> {
        let mut current = pointer;
        loop {
            if let Some(pos) = self.0.get(current) {
                return Some(*pos);
            }
            current = &current[..current.rfind('/')?];
        }
    }

    /// Every recorded pointer and position.
    #[must_use]
    pub fn entries(&self) -> &BTreeMap<String, Pos> {
        &self.0
    }
}

/// A parsed proposal: the structured frame and where its pieces came from.
#[derive(Clone, Debug)]
pub struct Parsed {
    /// `{"af1": 1, "afx": 1, "fns": [..]}` with structured bodies.
    pub frame: Value,
    /// Pointers into `frame` to positions in the text.
    pub positions: Positions,
}

/// Parses familiar text into a structured-body frame.
///
/// # Errors
///
/// The first unsupported, ambiguous or malformed construct, at its position.
pub fn parse(text: &str) -> Result<Parsed, FamiliarError> {
    let mut declared = Declared::default();
    let functions = if is_python(text) {
        let mut parser = Parser {
            toks: py_tokenize(text)?,
            k: 0,
            python: true,
            scopes: Vec::new(),
            hoisted: Vec::new(),
            fresh: 0,
            declared: Declared::default(),
        };
        let functions = parser.py_program()?;
        declared = std::mem::take(&mut parser.declared);
        functions
    } else {
        let mut parser = Parser {
            toks: tokenize(text)?,
            k: 0,
            python: false,
            scopes: Vec::new(),
            hoisted: Vec::new(),
            fresh: 0,
            declared: Declared::default(),
        };
        parser.program()?
    };
    let mut positions = BTreeMap::new();
    let mut fns = Vec::with_capacity(functions.len());
    for (index, f) in functions.into_iter().enumerate() {
        let at = format!("/fns/{index}");
        positions.insert(at.clone(), f.pos);
        let mut params = Vec::with_capacity(f.params.len());
        for (k, (name, ty, pos)) in f.params.into_iter().enumerate() {
            positions.insert(format!("{at}/params/{k}"), pos);
            params.push(json!([name, ty]));
        }
        positions.insert(format!("{at}/returns"), f.returns.1);
        f.body.record(&format!("{at}/body"), &mut positions);
        let mut object = Map::new();
        object.insert("fn".into(), Value::String(f.name));
        object.insert("params".into(), Value::Array(params));
        object.insert("returns".into(), Value::String(f.returns.0));
        object.insert("body".into(), f.body.value());
        fns.push(Value::Object(object));
    }
    let mut frame = json!({"af1": 1, "afx": 1, "fns": fns});
    if !declared.records.is_empty() {
        let mut types = Vec::with_capacity(declared.records.len());
        for (index, (_, _, entry, pos)) in declared.records.into_iter().enumerate() {
            positions.insert(format!("/types/{index}"), pos);
            types.push(entry);
        }
        frame["types"] = Value::Array(types);
    }
    if !declared.tests.is_empty() {
        let mut tests = Vec::with_capacity(declared.tests.len());
        for (index, (case, pos)) in declared.tests.into_iter().enumerate() {
            positions.insert(format!("/tests/{index}"), pos);
            tests.push(case);
        }
        frame["tests"] = Value::Array(tests);
    }
    Ok(Parsed {
        frame,
        positions: Positions(positions),
    })
}

/// Parses a headerless statement list for a controller-selected function.
/// The returned frame contains only `body`; the command fills the function
/// name and signature from the bound accepted graph before ordinary lowering.
///
/// # Errors
///
/// Unsupported syntax (including function headers or outer braces) is refused
/// at the original body position. No synthetic header shifts diagnostics.
pub fn parse_body(text: &str) -> Result<Parsed, FamiliarError> {
    let mut parser = Parser {
        toks: tokenize(text)?,
        k: 0,
        python: false,
        scopes: Vec::new(),
        hoisted: Vec::new(),
        fresh: 0,
        declared: Declared::default(),
    };
    let pos = parser.peek().pos;
    let mut statements = Vec::new();
    while parser.peek().kind != Kind::Eof {
        if parser.at_op(";") {
            parser.bump();
            continue;
        }
        statements.push(parser.statement()?);
    }
    if statements.is_empty() {
        return refuse(pos, "expected at least one body statement");
    }
    let mut body = Node::list(pos, statements);
    resolve_tails(&mut body, true)?;
    let mut positions = BTreeMap::new();
    body.record("/fns/0/body", &mut positions);
    Ok(Parsed {
        frame: json!({"af1":1,"afx":1,"fns":[{"body":body.value()}]}),
        positions: Positions(positions),
    })
}

// ------------------------------------------------------------------ tokens

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Num,
    Str,
    Name,
    Op,
    /// Python dialect: the end of a logical line.
    Newline,
    /// Python dialect: a deeper indentation.
    Indent,
    /// Python dialect: the end of an indented block.
    Dedent,
    Eof,
}

#[derive(Clone, Debug)]
struct Tok {
    kind: Kind,
    text: String,
    pos: Pos,
}

fn refuse<T>(pos: Pos, message: impl Into<String>) -> Result<T, FamiliarError> {
    Err(FamiliarError {
        message: message.into(),
        at: pos,
    })
}

/// Multi-character spellings from other languages, refused with what to
/// write instead. Checked before the operators Sley accepts.
const FOREIGN: [(&str, &str); 9] = [
    ("++", "increment is not supported: write x = x + 1"),
    ("**", "there is no power operator: multiply in a loop"),
    ("::", "paths are not supported"),
    ("=>", "match arms and lambdas are not supported"),
    ("<<", "bitwise operators are not supported"),
    (
        "&",
        "bitwise operators are not supported (logical and is written and)",
    ),
    (
        "|",
        "bitwise operators are not supported (logical or is written or)",
    ),
    ("^", "bitwise operators are not supported"),
    ("~", "bitwise operators are not supported"),
];
const OPERATORS: [&str; 28] = [
    "+=", "-=", "*=", "/=", "%=", "..", "->", "==", "!=", "<=", ">=", "+", "-", "*", "/", "%", "<",
    ">", "=", "(", ")", "{", "}", "[", "]", ",", ".", ":",
];
/// Compound assignment: `x op= e` is `x = x op e`.
const COMPOUND: [(&str, &str); 5] = [
    ("+=", "add"),
    ("-=", "sub"),
    ("*=", "mul"),
    ("/=", "div"),
    ("%=", "rem"),
];
/// Built-ins that take a generator: `sum(e for x in xs if c)`.
const FOLDS: [&str; 6] = ["sum", "count", "any", "all", "max", "min"];

#[allow(clippy::too_many_lines)]
fn tokenize(text: &str) -> Result<Vec<Tok>, FamiliarError> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let (mut i, mut line, mut line_start) = (0usize, 1usize, 0usize);
    let pos_of = |i: usize, line: usize, line_start: usize| Pos {
        line,
        column: text[line_start..i].chars().count() + 1,
        byte: i,
    };
    while i < bytes.len() {
        let c = bytes[i];
        let pos = pos_of(i, line, line_start);
        let rest = &text[i..];
        if c == b'\n' {
            i += 1;
            line += 1;
            line_start = i;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'#' || rest.starts_with("//") {
            if c == b'/' && !text[line_start..i].trim().is_empty() {
                return refuse(
                    pos,
                    "`//` is not an operator: `/` divides (truncating toward zero); a comment after code starts with #",
                );
            }
            i += rest.find('\n').unwrap_or(rest.len());
            continue;
        }
        if c == b';' {
            tokens.push(Tok {
                kind: Kind::Op,
                text: ";".into(),
                pos,
            });
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            let after = &rest[digits..];
            if after.starts_with('.') && after[1..].starts_with(|c: char| c.is_ascii_digit()) {
                return refuse(
                    pos,
                    "familiar text has no floating-point literals (f32/f64 need an AF1 frame: help types)",
                );
            }
            let suffix_len = after
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            let suffix = &after[..suffix_len];
            if !suffix.is_empty() && !INT_SUFFIXES.contains(&suffix) {
                return refuse(
                    pos,
                    format!(
                        "unsupported number literal `{}`: write decimal digits, optionally typed as in 7u64",
                        &rest[..digits + suffix_len]
                    ),
                );
            }
            tokens.push(Tok {
                kind: Kind::Num,
                text: rest[..digits + suffix_len].to_owned(),
                pos,
            });
            i += digits + suffix_len;
            continue;
        }
        if c == b'"' {
            let mut end = 1;
            let mut escaped = false;
            loop {
                let Some(&b) = rest.as_bytes().get(end) else {
                    return refuse(pos, "unterminated text literal");
                };
                if b == b'\n' {
                    return refuse(pos, "unterminated text literal");
                }
                end += 1;
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    break;
                }
            }
            let literal = &rest[..end];
            if serde_json::from_str::<String>(literal).is_err() {
                return refuse(pos, "invalid escape in text literal (JSON escapes only)");
            }
            tokens.push(Tok {
                kind: Kind::Str,
                text: literal.to_owned(),
                pos,
            });
            i += end;
            continue;
        }
        if c == b'\'' {
            return refuse(pos, "text literals use double quotes");
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let len = rest
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            tokens.push(Tok {
                kind: Kind::Name,
                text: rest[..len].to_owned(),
                pos,
            });
            i += len;
            continue;
        }
        // C-family spellings of the logical words.
        if let Some((spelled, word)) = [("&&", "and"), ("||", "or")]
            .iter()
            .find(|(s, _)| rest.starts_with(*s))
        {
            tokens.push(Tok {
                kind: Kind::Name,
                text: (*word).into(),
                pos,
            });
            i += spelled.len();
            continue;
        }
        if rest.starts_with('!') && !rest.starts_with("!=") {
            tokens.push(Tok {
                kind: Kind::Name,
                text: "not".into(),
                pos,
            });
            i += 1;
            continue;
        }
        if let Some((spelled, message)) = FOREIGN.iter().find(|(s, _)| rest.starts_with(*s)) {
            return refuse(pos, format!("`{spelled}`: {message}"));
        }
        if rest.starts_with('?') {
            return refuse(
                pos,
                "`?`: write try(e) to return a failure from the function",
            );
        }
        if let Some(op) = OPERATORS.iter().find(|op| rest.starts_with(**op)) {
            tokens.push(Tok {
                kind: Kind::Op,
                text: (*op).to_owned(),
                pos,
            });
            i += op.len();
            continue;
        }
        let shown = rest.chars().next().unwrap_or(' ');
        return refuse(pos, format!("unexpected character {shown:?}"));
    }
    tokens.push(Tok {
        kind: Kind::Eof,
        text: String::new(),
        pos: pos_of(bytes.len(), line, line_start),
    });
    Ok(tokens)
}

// ------------------------------------------------------------------ tree

/// A structured value with the position it was written at; lists keep the
/// shape of the JSON array they become, so pointers line up.
#[derive(Clone, Debug)]
struct Node {
    pos: Pos,
    kind: NodeKind,
}

#[derive(Clone, Debug)]
enum NodeKind {
    Atom(Value),
    List(Vec<Node>),
}

impl Node {
    fn atom(pos: Pos, value: Value) -> Self {
        Self {
            pos,
            kind: NodeKind::Atom(value),
        }
    }

    fn list(pos: Pos, items: Vec<Node>) -> Self {
        Self {
            pos,
            kind: NodeKind::List(items),
        }
    }

    /// `[op, args..]` at `pos`.
    fn op(pos: Pos, word: &str, args: Vec<Node>) -> Self {
        let mut items = vec![Self::atom(pos, Value::String(word.to_owned()))];
        items.extend(args);
        Self::list(pos, items)
    }

    fn value(&self) -> Value {
        match &self.kind {
            NodeKind::Atom(value) => value.clone(),
            NodeKind::List(items) => Value::Array(items.iter().map(Self::value).collect()),
        }
    }

    fn record(&self, pointer: &str, positions: &mut BTreeMap<String, Pos>) {
        positions.insert(pointer.to_owned(), self.pos);
        if let NodeKind::List(items) = &self.kind {
            for (index, item) in items.iter().enumerate() {
                item.record(&format!("{pointer}/{index}"), positions);
            }
        }
    }

    fn head(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::List(items) => match &items.first()?.kind {
                NodeKind::Atom(Value::String(word)) => Some(word),
                _ => None,
            },
            NodeKind::Atom(_) => None,
        }
    }

    /// The word of an atom.
    fn head_word(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Atom(Value::String(word)) => Some(word),
            _ => None,
        }
    }

    fn is_name(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Atom(Value::String(name)) => Some(name),
            _ => None,
        }
    }
}

/// The last statement of a function (and of each branch of a final `if`)
/// may be a bare value: it is the function's result (`return e`, or
/// `return ok(e)` / `return err(Case)` for `Ok(e)` / `Err(Case)`). A bare
/// value anywhere else is refused.
fn resolve_tails(block: &mut Node, tail: bool) -> Result<(), FamiliarError> {
    let NodeKind::List(items) = &mut block.kind else {
        return Ok(());
    };
    let last = items.len().saturating_sub(1);
    for (index, statement) in items.iter_mut().enumerate() {
        let here = tail && index == last;
        let head = statement.head().map(str::to_owned);
        match head.as_deref() {
            Some(word @ ("value" | "tail-ok" | "tail-fail")) => {
                if !here {
                    return refuse(
                        statement.pos,
                        "a value on its own line does nothing here: write return e (only the last statement may be a bare value)",
                    );
                }
                let renamed = match word {
                    "value" => "return",
                    "tail-ok" => "ok",
                    _ => "fail",
                };
                if let NodeKind::List(parts) = &mut statement.kind {
                    parts[0] = Node::atom(parts[0].pos, Value::String(renamed.to_owned()));
                }
            }
            Some("if") => {
                if let NodeKind::List(parts) = &mut statement.kind {
                    for branch in parts.iter_mut().skip(2) {
                        resolve_tails(branch, here)?;
                    }
                }
            }
            Some("while" | "for" | "range" | "for-indexed") => {
                if let NodeKind::List(parts) = &mut statement.kind
                    && let Some(body) = parts.last_mut()
                {
                    resolve_tails(body, false)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Inside `try(..)`: checked operations without their own failure mode
/// return the failure instead of trapping, and helper calls unwrap a
/// returned Result the same way (`tcall`, [`crate::structured`]).
fn mark_failure(node: &mut Node, suffix: &str) {
    let rename = node.head().and_then(|head| {
        if head.contains('?') {
            None
        } else if FAILING.contains(&head) {
            Some(format!("{head}{suffix}"))
        } else if head == "call" {
            Some(format!("tcall{suffix}"))
        } else {
            None
        }
    });
    if let NodeKind::List(items) = &mut node.kind {
        if let Some(word) = rename {
            items[0].kind = NodeKind::Atom(Value::String(word));
        }
        for item in items.iter_mut().skip(1) {
            mark_failure(item, suffix);
        }
    }
}

struct Function {
    name: String,
    pos: Pos,
    params: Vec<(String, String, Pos)>,
    returns: (String, Pos),
    body: Node,
}

// ------------------------------------------------------------------ parser

struct Parser {
    toks: Vec<Tok>,
    k: usize,
    /// The Python dialect: Python semantics for `//`, `%`, `len`, indexing
    /// and comparisons.
    python: bool,
    /// Python dialect: the names assigned in each enclosing block.
    scopes: Vec<Vec<String>>,
    /// Python dialect: nested `def`s, hoisted to the top level (a helper that
    /// reads its enclosing function's variables is refused by the lowering).
    hoisted: Vec<Function>,
    /// Python dialect: a counter for the dialect's own temporary names.
    fresh: usize,
    /// Python dialect: record classes and pytest-style tests in the file.
    declared: Declared,
}

/// What a Python-dialect file declares besides its functions: record
/// classes (`types`) and `def test_*()` functions (`tests`), each with the
/// position it came from.
#[derive(Default)]
struct Declared {
    /// Record classes in file order: name, field names, AF1 type entry.
    records: Vec<(String, Vec<String>, Value, Pos)>,
    /// `TestCase` entities in file order.
    tests: Vec<(Value, Pos)>,
}

type P<T> = Result<T, FamiliarError>;

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.k]
    }

    fn peek_at(&self, offset: usize) -> &Tok {
        &self.toks[(self.k + offset).min(self.toks.len() - 1)]
    }

    fn previous(&self) -> &Tok {
        &self.toks[self.k.saturating_sub(1)]
    }

    fn bump(&mut self) -> Tok {
        let tok = self.toks[self.k].clone();
        if self.k + 1 < self.toks.len() {
            self.k += 1;
        }
        tok
    }

    fn at_op(&self, op: &str) -> bool {
        let tok = self.peek();
        tok.kind == Kind::Op && tok.text == op
    }

    fn at_word(&self, word: &str) -> bool {
        let tok = self.peek();
        tok.kind == Kind::Name && tok.text == word
    }

    fn found(tok: &Tok) -> String {
        match tok.kind {
            Kind::Eof => "end of input".into(),
            Kind::Newline => "the end of the line".into(),
            Kind::Indent => "an indented line".into(),
            Kind::Dedent => "the end of the block".into(),
            _ => format!("`{}`", tok.text),
        }
    }

    fn expect_op(&mut self, op: &str) -> P<Tok> {
        if self.at_op(op) {
            return Ok(self.bump());
        }
        let tok = self.peek().clone();
        let mut message = format!("expected `{op}`, found {}", Self::found(&tok));
        if op == "{" && tok.text == ":" {
            message.push_str(": blocks are written { ... }");
        } else if op == "]" && tok.text == ":" {
            message.push_str(": slices are not supported");
        } else if op == ")" && tok.text == "," {
            message.push_str(": tuples are not supported");
        }
        refuse(tok.pos, message)
    }

    fn expect_word(&mut self, word: &str) -> P<Tok> {
        if self.at_word(word) {
            return Ok(self.bump());
        }
        let tok = self.peek().clone();
        refuse(
            tok.pos,
            format!("expected `{word}`, found {}", Self::found(&tok)),
        )
    }

    /// A name that is not a keyword.
    fn ident(&mut self, what: &str) -> P<(String, Pos)> {
        let tok = self.peek().clone();
        if tok.kind != Kind::Name {
            return refuse(
                tok.pos,
                format!("expected {what}, found {}", Self::found(&tok)),
            );
        }
        if KEYWORDS.contains(&tok.text.as_str()) {
            return refuse(tok.pos, format!("`{}` is a keyword, not {what}", tok.text));
        }
        self.bump();
        Ok((tok.text, tok.pos))
    }

    fn type_(&mut self) -> P<(String, Pos)> {
        let (name, pos) = self.ident("a type")?;
        if !self.at_op("<") {
            return Ok((name, pos));
        }
        self.bump();
        let mut inner = vec![self.type_()?.0];
        while self.at_op(",") {
            self.bump();
            inner.push(self.type_()?.0);
        }
        self.expect_op(">")?;
        Ok((format!("{name}<{}>", inner.join(",")), pos))
    }

    fn program(&mut self) -> P<Vec<Function>> {
        let mut functions = Vec::new();
        while self.peek().kind != Kind::Eof {
            if self.at_op(";") {
                self.bump();
                continue;
            }
            functions.push(self.function()?);
        }
        if functions.is_empty() {
            return refuse(self.peek().pos, "expected at least one fn");
        }
        Ok(functions)
    }

    fn function(&mut self) -> P<Function> {
        let tok = self.peek().clone();
        if !self.at_word("fn") {
            let message = if matches!(tok.text.as_str(), "def" | "func" | "function") {
                format!(
                    "`{}`: a function is written fn name(x: T) -> T {{ ... }}",
                    tok.text
                )
            } else {
                format!("expected fn, found {}", Self::found(&tok))
            };
            return refuse(tok.pos, message);
        }
        self.bump();
        let (name, _) = self.ident("a function name")?;
        if RESERVED_CALLS.contains(&name.as_str()) {
            return refuse(
                tok.pos,
                format!("`{name}` is a built-in; name the function otherwise"),
            );
        }
        self.expect_op("(")?;
        let mut params = Vec::new();
        while !self.at_op(")") {
            let (param, pos) = self.ident("a parameter name")?;
            self.expect_op(":")?;
            let (ty, _) = self.type_()?;
            params.push((param, ty, pos));
            if !self.at_op(")") {
                self.expect_op(",")?;
            }
        }
        self.expect_op(")")?;
        if !self.at_op("->") {
            let next = self.peek().clone();
            return refuse(
                next.pos,
                format!(
                    "expected `->` and the return type, found {}",
                    Self::found(&next)
                ),
            );
        }
        self.bump();
        let returns = self.type_()?;
        let mut body = self.block()?;
        resolve_tails(&mut body, true)?;
        Ok(Function {
            name,
            pos: tok.pos,
            params,
            returns,
            body,
        })
    }

    fn block(&mut self) -> P<Node> {
        let open = self.expect_op("{")?;
        let mut body = Vec::new();
        while !self.at_op("}") {
            if self.at_op(";") {
                self.bump();
                continue;
            }
            if self.peek().kind == Kind::Eof {
                return refuse(open.pos, "this `{` is never closed");
            }
            body.push(self.statement()?);
        }
        self.bump();
        Ok(Node::list(open.pos, body))
    }

    fn for_loop(&mut self) -> P<Node> {
        let pos = self.bump().pos;
        let (name, name_pos) = self.ident("a loop variable")?;
        let mut names = vec![Node::atom(name_pos, Value::String(name))];
        if self.at_op(",") {
            self.bump();
            let (value, value_pos) = self.ident("an element variable")?;
            names.push(Node::atom(value_pos, Value::String(value)));
        }
        self.expect_word("in")?;
        let first = self.expr()?;
        let kind = if self.at_op("..") {
            if names.len() != 1 {
                return refuse(self.peek().pos, "a range has one loop variable");
            }
            self.bump();
            if self.at_op("=") {
                return refuse(
                    self.peek().pos,
                    "ranges are half-open: write start..end, not start..=end",
                );
            }
            let end = self.expr()?;
            names.extend([first, end]);
            "range"
        } else {
            let indexed = names.len() == 2;
            names.push(first);
            if indexed { "for-indexed" } else { "for" }
        };
        names.insert(0, Node::atom(pos, Value::String(kind.into())));
        names.push(self.block()?);
        Ok(Node::list(pos, names))
    }

    #[allow(clippy::too_many_lines)]
    fn statement(&mut self) -> P<Node> {
        let tok = self.peek().clone();
        let pos = tok.pos;
        let word = (tok.kind == Kind::Name).then_some(tok.text.as_str());
        let keyword = |name: &str| Node::atom(pos, Value::String(name.to_owned()));
        match word {
            Some("let") => {
                self.bump();
                if self.at_word("mut") {
                    self.bump();
                }
                let (name, name_pos) = self.ident("a variable name")?;
                if self.at_op(":") {
                    self.bump();
                    let (ty, ty_pos) = self.type_()?;
                    self.expect_op("=")?;
                    let value = self.expr()?;
                    return Ok(Node::list(
                        pos,
                        vec![
                            keyword("var"),
                            Node::atom(name_pos, Value::String(name)),
                            Node::atom(ty_pos, Value::String(ty)),
                            value,
                        ],
                    ));
                }
                self.expect_op("=")?;
                let value = self.expr()?;
                // A let may be assigned later (as in most languages authors
                // know): it lowers as an inferred var, which is the same value.
                Ok(Node::list(
                    pos,
                    vec![
                        keyword("var"),
                        Node::atom(name_pos, Value::String(name)),
                        value,
                    ],
                ))
            }
            Some("var") => {
                self.bump();
                let (name, name_pos) = self.ident("a variable name")?;
                if self.at_op("=") {
                    self.bump();
                    let value = self.expr()?;
                    return Ok(Node::list(
                        pos,
                        vec![
                            keyword("var"),
                            Node::atom(name_pos, Value::String(name)),
                            value,
                        ],
                    ));
                }
                if !self.at_op(":") {
                    return refuse(self.peek().pos, "write var x = e or var x: T = e");
                }
                self.bump();
                let (ty, ty_pos) = self.type_()?;
                self.expect_op("=")?;
                let value = self.expr()?;
                Ok(Node::list(
                    pos,
                    vec![
                        keyword("var"),
                        Node::atom(name_pos, Value::String(name)),
                        Node::atom(ty_pos, Value::String(ty)),
                        value,
                    ],
                ))
            }
            Some("if") => self.if_rest(),
            Some("for") => self.for_loop(),
            Some("while") => {
                self.bump();
                let condition = self.expr()?;
                let body = self.block()?;
                Ok(Node::list(pos, vec![keyword("while"), condition, body]))
            }
            Some("return") => {
                self.bump();
                if self.at_op("}") || self.at_op(";") || self.peek().kind == Kind::Eof {
                    return refuse(pos, "return needs a value");
                }
                let next = self.peek().clone();
                let call = self.peek_at(1).text == "(" && next.kind == Kind::Name;
                if call && next.text == "ok" {
                    self.bump();
                    self.bump();
                    let value = self.expr()?;
                    self.expect_op(")")?;
                    return Ok(Node::list(pos, vec![keyword("ok"), value]));
                }
                if call && next.text == "err" {
                    self.bump();
                    self.bump();
                    let case = self.peek().clone();
                    if case.kind != Kind::Name || self.peek_at(1).text != ")" {
                        return refuse(case.pos, "err takes an error case name: return err(Case)");
                    }
                    self.bump();
                    self.bump();
                    return Ok(Node::list(
                        pos,
                        vec![
                            keyword("fail"),
                            Node::atom(case.pos, Value::String(case.text)),
                        ],
                    ));
                }
                let value = self.expr()?;
                Ok(Node::list(pos, vec![keyword("return"), value]))
            }
            Some("trap") => {
                self.bump();
                Ok(Node::list(pos, vec![keyword("trap")]))
            }
            Some("elif") => refuse(pos, "`elif`: write else if"),
            Some(word @ ("break" | "continue")) => {
                self.bump();
                Ok(Node::list(pos, vec![keyword(word)]))
            }
            Some("pass") => refuse(pos, "`pass`: an empty block is { }"),
            Some("match" | "loop" | "switch" | "do") => {
                refuse(pos, format!("`{}` is not supported", tok.text))
            }
            Some("const" | "static") => refuse(pos, format!("`{}`: write let", tok.text)),
            Some(name) if !KEYWORDS.contains(&name) && self.peek_at(1).text == "=" => {
                self.bump();
                self.bump();
                let value = self.expr()?;
                Ok(Node::list(
                    pos,
                    vec![
                        keyword("set"),
                        Node::atom(pos, Value::String(name.to_owned())),
                        value,
                    ],
                ))
            }
            Some(name)
                if !KEYWORDS.contains(&name)
                    && COMPOUND.iter().any(|(op, _)| self.peek_at(1).text == *op) =>
            {
                self.bump();
                let op_tok = self.bump();
                let word = COMPOUND
                    .iter()
                    .find(|(op, _)| *op == op_tok.text)
                    .map_or("add", |(_, word)| *word);
                let value = self.expr()?;
                let target = Node::atom(pos, Value::String(name.to_owned()));
                Ok(Node::list(
                    pos,
                    vec![
                        keyword("set"),
                        Node::atom(pos, Value::String(name.to_owned())),
                        Node::op(op_tok.pos, word, vec![target, value]),
                    ],
                ))
            }
            Some(name) if !KEYWORDS.contains(&name) && self.peek_at(1).text == ":" => {
                refuse(pos, "declare a variable with var x = e or var x: T = e")
            }
            // `Ok(e)` / `Err(Case)` as the last statement (Rust style).
            Some("ok" | "Ok") if self.peek_at(1).text == "(" => {
                self.bump();
                self.bump();
                let value = self.expr()?;
                self.expect_op(")")?;
                Ok(Node::op(pos, "tail-ok", vec![value]))
            }
            Some("err" | "Err") if self.peek_at(1).text == "(" => {
                self.bump();
                self.bump();
                let case = self.peek().clone();
                if case.kind != Kind::Name || self.peek_at(1).text != ")" {
                    return refuse(case.pos, "err takes an error case name: err(Case)");
                }
                self.bump();
                self.bump();
                Ok(Node::op(
                    pos,
                    "tail-fail",
                    vec![Node::atom(case.pos, Value::String(case.text))],
                ))
            }
            _ if Self::starts_expression(&tok) => {
                let value = self.expr()?;
                Ok(Node::op(pos, "value", vec![value]))
            }
            _ => refuse(
                pos,
                format!(
                    "expected a statement (let, var, x = e, if, for, while, return, trap), found {}",
                    Self::found(&tok)
                ),
            ),
        }
    }

    /// Whether a statement starting with `tok` is an expression (a value
    /// that is only meaningful as the last statement).
    fn starts_expression(tok: &Tok) -> bool {
        match tok.kind {
            Kind::Num | Kind::Str => true,
            Kind::Op => matches!(tok.text.as_str(), "(" | "-"),
            Kind::Name => {
                !KEYWORDS.contains(&tok.text.as_str())
                    || matches!(tok.text.as_str(), "not" | "true" | "false")
            }
            Kind::Eof | Kind::Newline | Kind::Indent | Kind::Dedent => false,
        }
    }

    /// `if c { a } else if d { b } else { e }` used as a value: each branch
    /// holds one expression; only the chosen branch is evaluated.
    fn if_expr(&mut self) -> P<Node> {
        let pos = self.expect_word("if")?.pos;
        let condition = self.expr()?;
        let then = self.branch_value()?;
        if !self.at_word("else") {
            return refuse(
                self.peek().pos,
                "an if used as a value needs an else branch",
            );
        }
        self.bump();
        let otherwise = if self.at_word("if") {
            self.if_expr()?
        } else {
            self.branch_value()?
        };
        Ok(Node::op(pos, "if", vec![condition, then, otherwise]))
    }

    fn branch_value(&mut self) -> P<Node> {
        self.expect_op("{")?;
        let value = self.expr()?;
        if self.at_op(";") {
            self.bump();
        }
        if !self.at_op("}") {
            return refuse(
                self.peek().pos,
                "a branch of an if used as a value holds one expression",
            );
        }
        self.bump();
        Ok(value)
    }

    fn if_rest(&mut self) -> P<Node> {
        let pos = self.expect_word("if")?.pos;
        let condition = self.expr()?;
        let then = self.block()?;
        let keyword = Node::atom(pos, Value::String("if".into()));
        if !self.at_word("else") {
            return Ok(Node::list(pos, vec![keyword, condition, then]));
        }
        let else_pos = self.bump().pos;
        let otherwise = if self.at_word("if") {
            Node::list(else_pos, vec![self.if_rest()?])
        } else {
            self.block()?
        };
        Ok(Node::list(pos, vec![keyword, condition, then, otherwise]))
    }

    // -------------------------------------------------------------- expressions

    fn expr(&mut self) -> P<Node> {
        let value = self.or_()?;
        // `a if c else b` continues only on the same line: an `if` that
        // starts the next line is the next statement.
        if self.at_word("if") && self.peek().pos.line == self.previous().pos.line {
            let pos = self.bump().pos;
            let condition = self.or_()?;
            let condition = self.truth(condition);
            self.expect_word("else")?;
            let other = self.expr()?;
            return Ok(Node::op(pos, "if", vec![condition, value, other]));
        }
        Ok(value)
    }

    fn or_(&mut self) -> P<Node> {
        let mut value = self.and_()?;
        while self.at_word("or") {
            let pos = self.bump().pos;
            let right = self.and_()?;
            let (left, right) = (self.truth(value), self.truth(right));
            value = if self.python {
                // Python short-circuits: the right side runs only when needed.
                Node::op(
                    pos,
                    "if",
                    vec![left, Node::atom(pos, Value::Bool(true)), right],
                )
            } else {
                Node::op(pos, "or", vec![left, right])
            };
        }
        Ok(value)
    }

    fn and_(&mut self) -> P<Node> {
        let mut value = self.not_()?;
        while self.at_word("and") {
            let pos = self.bump().pos;
            let right = self.not_()?;
            let (left, right) = (self.truth(value), self.truth(right));
            value = if self.python {
                Node::op(
                    pos,
                    "if",
                    vec![left, right, Node::atom(pos, Value::Bool(false))],
                )
            } else {
                Node::op(pos, "and", vec![left, right])
            };
        }
        Ok(value)
    }

    fn not_(&mut self) -> P<Node> {
        if self.at_word("not") {
            let pos = self.bump().pos;
            let operand = self.not_()?;
            let operand = self.truth(operand);
            return Ok(Node::op(pos, "not", vec![operand]));
        }
        self.compare()
    }

    /// In the Python dialect a condition may be any value: `truthy` makes it
    /// a bool (an integer is true when nonzero, a list or text when not empty).
    fn truth(&self, node: Node) -> Node {
        if !self.python {
            return node;
        }
        let pos = node.pos;
        Node::op(pos, "truthy", vec![node])
    }

    fn comparison(&self) -> Option<&'static str> {
        let tok = self.peek();
        if tok.kind != Kind::Op {
            return None;
        }
        Some(match tok.text.as_str() {
            "==" => "eq",
            "!=" => "ne",
            "<" => "lt",
            "<=" => "le",
            ">" => "gt",
            ">=" => "ge",
            _ => return None,
        })
    }

    fn compare(&mut self) -> P<Node> {
        if self.python {
            let value = self.py_bitand()?;
            return self.py_compare(value);
        }
        let value = self.sum()?;
        let Some(op) = self.comparison() else {
            return Ok(value);
        };
        let pos = self.bump().pos;
        let right = self.sum()?;
        if self.comparison().is_some() {
            return refuse(
                self.peek().pos,
                "comparisons do not chain; combine them with and",
            );
        }
        Ok(Node::op(pos, op, vec![value, right]))
    }

    /// Python comparisons: chains (`a < b < c` is `a < b and b < c`) and
    /// membership (`x in xs` is `any(v == x for v in xs)`).
    fn py_compare(&mut self, first: Node) -> P<Node> {
        let mut parts = Vec::new();
        let mut left = first;
        loop {
            let negate_in = self.at_word("not") && self.peek_at(1).text == "in";
            if self.at_word("in") || negate_in {
                let pos = self.bump().pos;
                if negate_in {
                    self.bump();
                }
                let list = self.py_bitand()?;
                let item = Node::atom(pos, Value::String("in_item_".into()));
                let test = Node::op(pos, "eq", vec![item.clone(), left.clone()]);
                let mut found = Node::op(
                    pos,
                    "fold",
                    vec![
                        Node::atom(pos, Value::String("any".into())),
                        item,
                        list.clone(),
                        test,
                    ],
                );
                if negate_in {
                    found = Node::op(pos, "not", vec![found]);
                }
                parts.push(found);
                left = list;
                continue;
            }
            if self.at_word("is") {
                return refuse(
                    self.peek().pos,
                    "`is` is not supported: compare values with ==",
                );
            }
            let Some(op) = self.comparison() else {
                break;
            };
            let pos = self.bump().pos;
            let right = self.py_bitand()?;
            parts.push(Node::op(pos, op, vec![left, right.clone()]));
            left = right;
        }
        let mut parts = parts.into_iter();
        let Some(mut value) = parts.next() else {
            return Ok(left);
        };
        for part in parts {
            let pos = part.pos;
            value = Node::op(pos, "and", vec![value, part]);
        }
        Ok(value)
    }

    /// Python `&` with a mask `2**k - 1` (the low k bits, which is a floor
    /// modulo by `2**k`, exact for negative values too).
    fn py_bitand(&mut self) -> P<Node> {
        let mut value = self.py_shift()?;
        while self.at_op("&") {
            let pos = self.bump().pos;
            let mask = self.py_shift()?;
            value = Self::py_mask(pos, value, &mask)?;
        }
        Ok(value)
    }

    fn py_mask(pos: Pos, value: Node, mask: &Node) -> P<Node> {
        let modulus = match &mask.kind {
            NodeKind::Atom(Value::Number(n)) => n.as_u64().and_then(|m| m.checked_add(1)),
            _ => None,
        };
        match modulus {
            Some(modulus) if modulus.is_power_of_two() => Ok(Node::op(
                pos,
                "floormod",
                vec![value, Node::atom(mask.pos, Value::from(modulus))],
            )),
            _ => refuse(
                pos,
                "`&` takes a literal mask 2**k - 1 (x & 1, x & 255); other bitwise operations are not supported",
            ),
        }
    }

    /// Python `<<` and `>>`: multiplication and floor division by `2**k`.
    fn py_shift(&mut self) -> P<Node> {
        let mut value = self.sum()?;
        while self.at_op("<<") || self.at_op(">>") {
            let tok = self.bump();
            let count = self.sum()?;
            value = Self::py_shifted(tok.pos, &tok.text, value, count);
        }
        Ok(value)
    }

    fn py_shifted(pos: Pos, op: &str, value: Node, count: Node) -> Node {
        let power = Node::op(pos, "pow", vec![Node::atom(pos, Value::from(2)), count]);
        Node::op(
            pos,
            if op == "<<" { "mul" } else { "floordiv" },
            vec![value, power],
        )
    }

    fn sum(&mut self) -> P<Node> {
        let mut value = self.product()?;
        loop {
            let op = match self.peek() {
                tok if tok.kind == Kind::Op && tok.text == "+" => "add",
                tok if tok.kind == Kind::Op && tok.text == "-" => "sub",
                _ => return Ok(value),
            };
            let pos = self.bump().pos;
            let right = self.product()?;
            value = Node::op(pos, op, vec![value, right]);
        }
    }

    fn product(&mut self) -> P<Node> {
        let mut value = self.unary()?;
        loop {
            let python = self.python;
            let op = match self.peek() {
                tok if tok.kind == Kind::Op && tok.text == "*" => "mul",
                tok if tok.kind == Kind::Op && tok.text == "//" && python => "floordiv",
                tok if tok.kind == Kind::Op && tok.text == "/" && python => {
                    return refuse(tok.pos, "/ is float division; Sley ints divide with //");
                }
                tok if tok.kind == Kind::Op && tok.text == "/" => "div",
                tok if tok.kind == Kind::Op && tok.text == "%" && python => "floormod",
                tok if tok.kind == Kind::Op && tok.text == "%" => "rem",
                _ => return Ok(value),
            };
            let pos = self.bump().pos;
            let right = self.unary()?;
            value = Node::op(pos, op, vec![value, right]);
        }
    }

    fn unary(&mut self) -> P<Node> {
        if self.python {
            // Python: -a ** b is -(a ** b); ** binds right to left.
            if self.at_op("-") {
                let pos = self.bump().pos;
                if self.peek().kind == Kind::Num && self.peek_at(1).text != "**" {
                    let tok = self.bump();
                    return Self::literal(&tok, Some(pos));
                }
                let operand = self.unary()?;
                return Ok(Node::op(pos, "neg", vec![operand]));
            }
            if self.at_op("+") {
                self.bump();
                return self.unary();
            }
            let base = self.postfix()?;
            if self.at_op("**") {
                let pos = self.bump().pos;
                let exponent = self.unary()?;
                return Ok(Node::op(pos, "pow", vec![base, exponent]));
            }
            return Ok(base);
        }
        if self.at_op("-") {
            let pos = self.bump().pos;
            if self.peek().kind == Kind::Num {
                let tok = self.bump();
                return Self::literal(&tok, Some(pos));
            }
            let operand = self.unary()?;
            return Ok(Node::op(pos, "neg", vec![operand]));
        }
        self.postfix()
    }

    /// An integer literal; `negative` is the position of a leading minus.
    fn literal(tok: &Tok, negative: Option<Pos>) -> P<Node> {
        let digits_len = tok.text.bytes().take_while(u8::is_ascii_digit).count();
        let (digits, suffix) = tok.text.split_at(digits_len);
        let pos = negative.unwrap_or(tok.pos);
        let Ok(magnitude) = digits.parse::<u128>() else {
            return refuse(pos, "integer literal too large");
        };
        let number = if negative.is_some() {
            if suffix.starts_with('u') {
                return refuse(pos, "an unsigned literal cannot be negative");
            }
            // The minimum i64 is the one magnitude that only fits negated.
            if magnitude > u128::from(i64::MIN.unsigned_abs()) {
                return refuse(pos, "integer literal below the 64-bit range");
            }
            let value = i64::try_from(magnitude).map_or(i64::MIN, |m| -m);
            Value::from(value)
        } else if let Ok(value) = i64::try_from(magnitude) {
            Value::from(value)
        } else if let Ok(value) = u64::try_from(magnitude) {
            Value::from(value)
        } else {
            return refuse(pos, "integer literal beyond the 64-bit range");
        };
        Ok(Node::atom(
            pos,
            if suffix.is_empty() {
                number
            } else {
                json!({"type": suffix, "value": number})
            },
        ))
    }

    fn postfix(&mut self) -> P<Node> {
        let mut value = self.primary()?;
        loop {
            if self.at_op(".") {
                let pos = self.bump().pos;
                let (field, field_pos) = self.ident("a field name")?;
                if self.at_op("(") {
                    return refuse(
                        field_pos,
                        format!(
                            "methods are not supported: write {field}(x) for a built-in or helper"
                        ),
                    );
                }
                value = Node::op(
                    pos,
                    "field",
                    vec![value, Node::atom(field_pos, Value::String(field))],
                );
            } else if self.at_op("[") {
                let pos = self.bump().pos;
                if self.python
                    && self.at_op("-")
                    && self.peek_at(1).kind == Kind::Num
                    && self.peek_at(2).text == "]"
                {
                    // xs[-k] is the k-th element from the end.
                    self.bump();
                    let back = self.bump();
                    self.expect_op("]")?;
                    let k = Self::literal(&back, None)?;
                    let length = Node::op(pos, "len", vec![value.clone()]);
                    let k = Node::atom(back.pos, json!({"type": "u64", "value": k.value()}));
                    value = Node::op(
                        pos,
                        "get",
                        vec![value, Node::op(pos, "sub", vec![length, k])],
                    );
                    continue;
                }
                if self.python && (self.at_op(":") || self.peek_at(1).text == ":") {
                    return refuse(pos, "slices are not supported");
                }
                let mut index = self.expr()?;
                self.expect_op("]")?;
                if self.python {
                    // Python ints are i64; an index converts explicitly (a
                    // negative computed index fails like a checked operation).
                    index = Node::op(
                        pos,
                        "to",
                        vec![Node::atom(pos, Value::String("u64".into())), index],
                    );
                }
                value = Node::op(pos, "get", vec![value, index]);
            } else {
                return Ok(value);
            }
        }
    }

    fn primary(&mut self) -> P<Node> {
        let tok = self.peek().clone();
        match tok.kind {
            Kind::Num => {
                self.bump();
                return Self::literal(&tok, None);
            }
            Kind::Str => {
                self.bump();
                let text: String = serde_json::from_str(&tok.text).unwrap_or_default();
                return Ok(Node::atom(tok.pos, json!({"type": "text", "value": text})));
            }
            Kind::Op if tok.text == "(" => {
                self.bump();
                let value = self.expr()?;
                self.expect_op(")")?;
                return Ok(value);
            }
            Kind::Op if tok.text == "[" && self.python => {
                return refuse(
                    tok.pos,
                    "list values are not supported: loop, or use sum/any/all/len over a generator",
                );
            }
            Kind::Name => {}
            _ => {
                return refuse(
                    tok.pos,
                    format!("expected an expression, found {}", Self::found(&tok)),
                );
            }
        }
        let name = tok.text.as_str();
        match name {
            "true" | "false" => {
                self.bump();
                return Ok(Node::atom(tok.pos, Value::Bool(name == "true")));
            }
            "math" if self.python && self.peek_at(1).text == "." => {
                self.bump();
                self.bump();
                let (function, function_pos) = self.ident("a math function")?;
                if function != "gcd" {
                    return refuse(
                        function_pos,
                        format!("math.{function} is not supported (math.gcd is)"),
                    );
                }
                self.expect_op("(")?;
                return self.py_gcd(tok.pos);
            }
            "True" | "False" if self.python => {
                self.bump();
                return Ok(Node::atom(tok.pos, Value::Bool(name == "True")));
            }
            "True" | "False" => return refuse(tok.pos, "write true or false"),
            "if" => return self.if_expr(),
            "None" | "null" | "nil" => return refuse(tok.pos, "there is no null value"),
            _ if KEYWORDS.contains(&name) => {
                return refuse(tok.pos, format!("`{name}` cannot start an expression"));
            }
            _ => {}
        }
        self.bump();
        if !self.at_op("(") {
            return Ok(Node::atom(tok.pos, Value::String(tok.text)));
        }
        self.call(&tok)
    }

    /// The rest of `kind(e for x in xs if c)` after `e`.
    fn fold(&mut self, pos: Pos, kind: &str, value: Node) -> P<Node> {
        let node = self.fold_until(pos, kind, value, ")")?;
        Ok(node)
    }

    /// `for x in xs if c` then `closer`: the rest of a generator.
    fn fold_until(&mut self, pos: Pos, kind: &str, value: Node, closer: &str) -> P<Node> {
        self.expect_word("for")?;
        let (x, x_pos) = self.ident("a loop variable")?;
        self.expect_word("in")?;
        let list = self.or_()?;
        let mut args = vec![
            Node::atom(pos, Value::String(kind.to_owned())),
            Node::atom(x_pos, Value::String(x)),
            list,
            value,
        ];
        if self.at_word("if") {
            self.bump();
            args.push(self.or_()?);
        }
        self.expect_op(closer)?;
        Ok(Node::op(pos, "fold", args))
    }

    /// `len(set(xs))`: the positions whose element appears at no earlier
    /// position.
    fn py_distinct_count(pos: Pos, list: &Node) -> Node {
        let at = |name: &str| Node::atom(pos, Value::String(name.to_owned()));
        let element = |index: &str| {
            Node::op(
                pos,
                "get",
                vec![
                    list.clone(),
                    Node::op(pos, "to", vec![at("u64"), at(index)]),
                ],
            )
        };
        let earlier = Node::op(
            pos,
            "fold",
            vec![
                at("any"),
                at("set_j_"),
                Node::op(
                    pos,
                    "range",
                    vec![Node::atom(pos, Value::from(0)), at("set_i_")],
                ),
                Node::op(pos, "eq", vec![element("set_j_"), element("set_i_")]),
            ],
        );
        Node::op(
            pos,
            "fold",
            vec![
                at("count"),
                at("set_i_"),
                Node::op(
                    pos,
                    "range",
                    vec![Node::atom(pos, Value::from(0)), len_i64(pos, list)],
                ),
                Node::atom(pos, Value::Bool(true)),
                Node::op(pos, "not", vec![earlier]),
            ],
        )
    }

    /// A built-in applied to a slice: a fold over the window's positions.
    fn py_view_fold(pos: Pos, kind: &str, view: &View) -> Node {
        if kind == "count" {
            // len(xs[a:b]): the window's size, never negative.
            let size = Node::op(pos, "sub", vec![view.end.clone(), view.start.clone()]);
            return Node::op(pos, "max", vec![size, Node::atom(pos, Value::from(0))]);
        }
        let index = Node::atom(pos, Value::String("slice_at_".into()));
        let element = view.element_at(pos, index.clone());
        Node::op(
            pos,
            "fold",
            vec![
                Node::atom(pos, Value::String(kind.to_owned())),
                index,
                Node::op(pos, "range", vec![view.start.clone(), view.end.clone()]),
                element,
            ],
        )
    }

    /// `( value for ... )` as a call's first argument, or `None` (nothing
    /// consumed) when the parentheses hold something else.
    /// `gcd(a, b, ..)` after the `(`.
    fn py_gcd(&mut self, pos: Pos) -> P<Node> {
        let mut values = Vec::new();
        while !self.at_op(")") {
            values.push(self.expr()?);
            if !self.at_op(")") {
                self.expect_op(",")?;
            }
        }
        self.expect_op(")")?;
        let mut values = values.into_iter();
        let Some(mut value) = values.next() else {
            return Ok(Node::atom(pos, Value::from(0)));
        };
        let mut any = false;
        for next in values {
            any = true;
            value = Node::op(pos, "gcd", vec![value, next]);
        }
        if !any {
            value = Node::op(pos, "abs", vec![value]);
        }
        Ok(value)
    }

    fn py_parenthesized_generator(&mut self, pos: Pos, kind: &str) -> P<Option<Node>> {
        if !self.at_op("(") {
            return Ok(None);
        }
        let saved = self.k;
        self.bump();
        let value = match self.expr() {
            Ok(value) if self.at_word("for") => value,
            _ => {
                self.k = saved;
                return Ok(None);
            }
        };
        let value = if kind == "count" {
            Node::atom(pos, Value::Bool(true))
        } else {
            value
        };
        let node = self.py_generator(pos, kind, value)?;
        self.expect_op(")")?;
        Ok(Some(node))
    }

    /// `, default=v)` after a max/min generator, then the call's `)`.
    fn py_fold_default(&mut self, name: &str, node: Node) -> P<Node> {
        let mut node = node;
        if self.at_op(",") {
            self.bump();
            if !(self.at_word("default") && self.peek_at(1).text == "=") {
                return refuse(
                    self.peek().pos,
                    format!("{name}(generator) takes only default=value"),
                );
            }
            self.bump();
            self.bump();
            let fallback = self.expr()?;
            if let NodeKind::List(items) = &mut node.kind {
                if items.len() == 5 {
                    items.push(Node::atom(fallback.pos, Value::Null));
                }
                items.push(fallback);
            }
        }
        self.expect_op(")")?;
        Ok(node)
    }

    /// Python built-ins with Python meaning, after the opening `(`; `None`
    /// leaves the call to the shared grammar.
    #[allow(clippy::too_many_lines)]
    fn py_call(&mut self, pos: Pos, name: &str) -> P<Option<Node>> {
        let fold_kind = match name {
            "len" => "count",
            "sum" | "any" | "all" => name,
            "int" => {
                let value = self.expr()?;
                self.expect_op(")")?;
                return Ok(Some(value));
            }
            "range" | "enumerate" => {
                return refuse(
                    pos,
                    format!("{name}(..) works only as the iterable of a for loop"),
                );
            }
            "gcd" => return self.py_gcd(pos).map(Some),
            "pow" => {
                let mut args = vec![self.expr()?];
                while self.at_op(",") {
                    self.bump();
                    args.push(self.expr()?);
                }
                self.expect_op(")")?;
                let word = match args.len() {
                    2 => "pow",
                    3 => "powmod",
                    _ => return refuse(pos, "pow takes (base, exp) or (base, exp, mod)"),
                };
                return Ok(Some(Node::op(pos, word, args)));
            }
            "bool" => {
                let value = self.expr()?;
                self.expect_op(")")?;
                return Ok(Some(self.truth(value)));
            }
            "min" | "max" => {
                let mut args = Vec::new();
                if self.at_op("[") {
                    self.bump();
                    let value = self.expr()?;
                    let node = self.py_generator(pos, name, value)?;
                    self.expect_op("]")?;
                    return self.py_fold_default(name, node).map(Some);
                }
                if self.py_slice_ahead() {
                    let saved = self.k;
                    let view = self.py_slice()?;
                    if self.at_op(")") || self.at_op(",") {
                        let node = Self::py_view_fold(pos, name, &view);
                        return self.py_fold_default(name, node).map(Some);
                    }
                    self.k = saved;
                }
                if let Some(node) = self.py_parenthesized_generator(pos, name)? {
                    return self.py_fold_default(name, node).map(Some);
                }
                while !self.at_op(")") {
                    args.push(self.expr()?);
                    if args.len() == 1 && self.at_word("for") {
                        let value = args.swap_remove(0);
                        let node = self.py_generator(pos, name, value)?;
                        self.expect_op(")")?;
                        return Ok(Some(node));
                    }
                    if !self.at_op(")") {
                        self.expect_op(",")?;
                    }
                }
                self.expect_op(")")?;
                if args.len() == 1 {
                    // max(xs) / min(xs) over a list.
                    let item = Node::atom(pos, Value::String("list_item_".into()));
                    return Ok(Some(Node::op(
                        pos,
                        "fold",
                        vec![
                            Node::atom(pos, Value::String(name.to_owned())),
                            item.clone(),
                            args.swap_remove(0),
                            item,
                        ],
                    )));
                }
                if args.is_empty() {
                    return refuse(pos, format!("{name} takes values"));
                }
                let mut args = args.into_iter();
                let mut value = args.next().unwrap_or_else(|| Node::atom(pos, Value::Null));
                for next in args {
                    value = Node::op(pos, name, vec![value, next]);
                }
                return Ok(Some(value));
            }
            _ => return Ok(None),
        };
        // len(set(xs)): the number of distinct elements.
        if name == "len" && self.at_word("set") && self.peek_at(1).text == "(" {
            self.bump();
            self.bump();
            let list = self.expr()?;
            if self.at_word("for") {
                return refuse(pos, "set(..) takes a list here, not a generator");
            }
            self.expect_op(")")?;
            self.expect_op(")")?;
            return Ok(Some(Self::py_distinct_count(pos, &list)));
        }
        // A built-in over a slice: sum(xs[a:b]), len(xs[a:]).
        if self.py_slice_ahead() && matches!(self.peek_at(1).text.as_str(), "[") {
            let saved = self.k;
            let view = self.py_slice()?;
            if self.at_op(")") {
                self.bump();
                return Ok(Some(Self::py_view_fold(pos, fold_kind, &view)));
            }
            self.k = saved;
        }
        // A list comprehension or a generator.
        if let Some(node) = self.py_parenthesized_generator(pos, fold_kind)? {
            self.expect_op(")")?;
            return Ok(Some(node));
        }
        let bracket = self.at_op("[");
        if bracket {
            self.bump();
        }
        let value = self.expr()?;
        if self.at_word("for") {
            let value = if fold_kind == "count" {
                Node::atom(pos, Value::Bool(true))
            } else {
                value
            };
            let node = self.py_generator(pos, fold_kind, value)?;
            if bracket {
                self.expect_op("]")?;
            }
            self.expect_op(")")?;
            return Ok(Some(node));
        }
        if bracket {
            return refuse(pos, "list values are not supported");
        }
        self.expect_op(")")?;
        if name == "len" {
            // Python's len is an int (i64).
            let length = Node::op(pos, "len", vec![value]);
            return Ok(Some(Node::op(
                pos,
                "to",
                vec![Node::atom(pos, Value::String("i64".into())), length],
            )));
        }
        // sum / any / all over a list.
        let item = Node::atom(pos, Value::String("list_item_".into()));
        Ok(Some(Node::op(
            pos,
            "fold",
            vec![
                Node::atom(pos, Value::String(name.to_owned())),
                item.clone(),
                value,
                item,
            ],
        )))
    }

    fn call(&mut self, callee: &Tok) -> P<Node> {
        let pos = callee.pos;
        let name = callee.text.as_str();
        match name {
            "ok" | "err" | "Ok" | "Err" => {
                return refuse(
                    pos,
                    "results are returned as return ok(e) or return err(Case), not used as values",
                );
            }
            "Some" => return refuse(pos, "options are not supported in structured bodies"),
            _ => {}
        }
        self.expect_op("(")?;
        if name == "to" {
            let (ty, ty_pos) = self.type_()?;
            self.expect_op(",")?;
            let value = self.expr()?;
            self.expect_op(")")?;
            return Ok(Node::op(
                pos,
                "to",
                vec![Node::atom(ty_pos, Value::String(ty)), value],
            ));
        }
        if self.python
            && let Some(node) = self.py_call(pos, name)?
        {
            return Ok(node);
        }
        let mut args = Vec::new();
        while !self.at_op(")") {
            args.push(self.expr()?);
            if args.len() == 1 && self.at_word("for") {
                if !FOLDS.contains(&name) {
                    return refuse(
                        self.peek().pos,
                        "a generator works only inside sum, count, any, all, max or min",
                    );
                }
                return self.fold(pos, name, args.swap_remove(0));
            }
            if !self.at_op(")") {
                self.expect_op(",")?;
            }
        }
        self.expect_op(")")?;
        let builtin = match name {
            "min" | "max" => Some(2),
            "abs" | "len" => Some(1),
            "clamp" => Some(3),
            _ => None,
        };
        if name == "try" {
            let case = match args.as_slice() {
                [_] => String::new(),
                [_, case] => match case.is_name() {
                    Some(case) => case.to_owned(),
                    None => return refuse(pos, "try(e) or try(e, Case)"),
                },
                _ => return refuse(pos, "try(e) or try(e, Case)"),
            };
            let mut inner = args.swap_remove(0);
            mark_failure(&mut inner, &format!("?{case}"));
            return Ok(inner);
        }
        if let Some(arity) = builtin {
            if args.len() != arity {
                return refuse(pos, format!("{name} takes {arity} argument(s)"));
            }
            return Ok(Node::op(pos, name, args));
        }
        let mut items = vec![Node::atom(pos, Value::String(name.to_owned()))];
        items.extend(args);
        Ok(Node::op(pos, "call", items))
    }
}

// ------------------------------------------------------------------ Python dialect

/// Whether the text is the Python dialect: its first significant line starts
/// with `def`, `import`, `from`, `class` or a decorator (comments and blank
/// lines skipped).
fn is_python(text: &str) -> bool {
    text.lines()
        .map(str::trim_start)
        .find(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("//"))
        .is_some_and(|line| {
            ["def ", "import ", "from ", "class ", "@"]
                .iter()
                .any(|start| line.starts_with(start))
        })
}

/// Multi-character Python operators, longest first.
const PY_OPERATORS: [&str; 32] = [
    "//=", "**=", "<<=", ">>=", "->", "//", "**", "<<", ">>", "==", "!=", "<=", ">=", "+=", "-=",
    "*=", "%=", "&=", "+", "-", "*", "/", "%", "&", "<", ">", "=", "(", ")", "[", "]", ",",
];
/// Python spellings the dialect does not take, with what to write instead.
const PY_FOREIGN: [(&str, &str); 7] = [
    (
        "|",
        "bitwise operators are not supported (logical or is written or)",
    ),
    ("^", "bitwise operators are not supported"),
    ("~", "bitwise operators are not supported"),
    ("@", "decorators and matrix products are not supported"),
    (";", "write one statement per line"),
    ("{", "dicts and sets are not supported"),
    ("}", "dicts and sets are not supported"),
];

/// Tokenizes the Python dialect: the familiar token kinds plus `Newline`,
/// `Indent` and `Dedent` for the block structure. String literals become
/// JSON string text, as in the familiar tokenizer.
#[allow(clippy::too_many_lines)]
fn py_tokenize(text: &str) -> Result<Vec<Tok>, FamiliarError> {
    let text = text.replace("\r\n", "\n");
    let bytes = text.as_bytes();
    let mut tokens: Vec<Tok> = Vec::new();
    let mut indents = vec![0_usize];
    let (mut i, mut line, mut line_start, mut depth) = (0_usize, 1_usize, 0_usize, 0_usize);
    let mut at_line_start = true;
    let at = |i: usize, line: usize, line_start: usize| Pos {
        line,
        column: text[line_start..i].chars().count() + 1,
        byte: i,
    };
    loop {
        if at_line_start && depth == 0 {
            let mut j = i;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                if bytes[j] == b'\t' {
                    return refuse(at(j, line, line_start), "indent with spaces, not tabs");
                }
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
            if bytes[j] == b'\n' {
                i = j + 1;
                line += 1;
                line_start = i;
                continue;
            }
            if bytes[j] == b'#' || bytes[j] == b'@' {
                // A comment line, or a decorator (`@dataclass`), says nothing
                // the dialect uses.
                i = text[j..].find('\n').map_or(bytes.len(), |n| j + n);
                continue;
            }
            let width = j - line_start;
            let pos = at(j, line, line_start);
            if width > *indents.last().unwrap_or(&0) {
                indents.push(width);
                tokens.push(Tok {
                    kind: Kind::Indent,
                    text: String::new(),
                    pos,
                });
            } else {
                while width < *indents.last().unwrap_or(&0) {
                    indents.pop();
                    tokens.push(Tok {
                        kind: Kind::Dedent,
                        text: String::new(),
                        pos,
                    });
                }
                if width != *indents.last().unwrap_or(&0) {
                    return refuse(pos, "this line's indentation matches no enclosing block");
                }
            }
            i = j;
            at_line_start = false;
        }
        if i >= bytes.len() {
            break;
        }
        let c = bytes[i];
        let pos = at(i, line, line_start);
        let rest = &text[i..];
        match c {
            b'\n' => {
                if depth == 0 {
                    if !matches!(tokens.last().map(|t| t.kind), Some(Kind::Newline) | None) {
                        tokens.push(Tok {
                            kind: Kind::Newline,
                            text: String::new(),
                            pos,
                        });
                    }
                    at_line_start = true;
                }
                i += 1;
                line += 1;
                line_start = i;
            }
            b' ' | b'\t' | b'\r' => i += 1,
            b'#' => i += rest.find('\n').unwrap_or(rest.len()),
            b'\\' if rest[1..].starts_with('\n') => {
                i += 2;
                line += 1;
                line_start = i;
            }
            b'0'..=b'9' => {
                let len = rest
                    .bytes()
                    .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'.')
                    .count();
                let spelled = &rest[..len];
                if spelled.contains('.')
                    || spelled.contains(['e', 'E']) && !spelled.starts_with("0x")
                {
                    return refuse(
                        pos,
                        "the Python dialect has no floating-point numbers: ints only (f32/f64 need an AF1 frame: help types)",
                    );
                }
                if spelled.len() > 1
                    && spelled.starts_with('0')
                    && spelled
                        .as_bytes()
                        .get(1)
                        .copied()
                        .is_some_and(|b| b.is_ascii_alphabetic())
                {
                    return refuse(pos, "write integers in decimal");
                }
                let digits: String = spelled.chars().filter(|c| *c != '_').collect();
                if !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return refuse(pos, format!("`{spelled}` is not an integer"));
                }
                tokens.push(Tok {
                    kind: Kind::Num,
                    text: digits,
                    pos,
                });
                i += len;
            }
            b'"' | b'\'' => {
                let quote = c as char;
                let triple: String = std::iter::repeat_n(quote, 3).collect();
                let (open, close) = if rest.starts_with(&triple) {
                    (3, triple.as_str())
                } else {
                    (1, &rest[..1])
                };
                let body_start = i + open;
                let mut j = body_start;
                let mut value = String::new();
                loop {
                    if j >= bytes.len() || (open == 1 && bytes[j] == b'\n') {
                        return refuse(pos, "unterminated string literal");
                    }
                    if text[j..].starts_with(close) {
                        break;
                    }
                    let ch = text[j..].chars().next().unwrap_or_default();
                    if ch == '\\' {
                        let escaped = text[j + 1..].chars().next().unwrap_or_default();
                        value.push(match escaped {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            '0' => '\0',
                            '\\' | '\'' | '"' => escaped,
                            _ => {
                                return refuse(
                                    at(j, line, line_start),
                                    "unsupported escape in a string literal",
                                );
                            }
                        });
                        j += 1 + escaped.len_utf8();
                        continue;
                    }
                    if ch == '\n' {
                        line += 1;
                        line_start = j + 1;
                    }
                    value.push(ch);
                    j += ch.len_utf8();
                }
                tokens.push(Tok {
                    kind: Kind::Str,
                    text: Value::String(value).to_string(),
                    pos,
                });
                i = j + close.len();
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let len = rest
                    .bytes()
                    .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                    .count();
                let word = &rest[..len];
                if matches!(word, "f" | "b" | "r" | "rb" | "br" | "fr" | "rf")
                    && rest[len..].starts_with(['"', '\''])
                {
                    return refuse(pos, "string prefixes (f, b, r) are not supported");
                }
                tokens.push(Tok {
                    kind: Kind::Name,
                    text: word.to_owned(),
                    pos,
                });
                i += len;
            }
            _ => {
                if let Some((spelled, message)) =
                    PY_FOREIGN.iter().find(|(s, _)| rest.starts_with(*s))
                {
                    return refuse(pos, format!("`{spelled}`: {message}"));
                }
                if let Some(op) = PY_OPERATORS.iter().find(|op| rest.starts_with(**op)) {
                    match *op {
                        "(" | "[" => depth += 1,
                        ")" | "]" => depth = depth.saturating_sub(1),
                        _ => {}
                    }
                    if *op == "**=" {
                        return refuse(pos, "`**=`: write x = x ** n");
                    }
                    tokens.push(Tok {
                        kind: Kind::Op,
                        text: (*op).to_owned(),
                        pos,
                    });
                    i += op.len();
                } else if rest.starts_with([':', '.']) {
                    tokens.push(Tok {
                        kind: Kind::Op,
                        text: rest[..1].to_owned(),
                        pos,
                    });
                    i += 1;
                } else if rest.starts_with('!') {
                    return refuse(pos, "`!`: write not");
                } else {
                    let shown = rest.chars().next().unwrap_or_default();
                    return refuse(pos, format!("unexpected character {shown:?}"));
                }
            }
        }
    }
    let end = at(bytes.len(), line, line_start.min(bytes.len()));
    if !matches!(tokens.last().map(|t| t.kind), Some(Kind::Newline) | None) {
        tokens.push(Tok {
            kind: Kind::Newline,
            text: String::new(),
            pos: end,
        });
    }
    while indents.len() > 1 {
        indents.pop();
        tokens.push(Tok {
            kind: Kind::Dedent,
            text: String::new(),
            pos: end,
        });
    }
    tokens.push(Tok {
        kind: Kind::Eof,
        text: String::new(),
        pos: end,
    });
    Ok(tokens)
}

impl Parser {
    fn py_program(&mut self) -> P<Vec<Function>> {
        let mut functions = Vec::new();
        loop {
            match self.peek().kind {
                Kind::Eof => break,
                Kind::Newline => {
                    self.bump();
                    continue;
                }
                Kind::Str => {
                    // A module docstring.
                    self.bump();
                    self.py_end_of_statement()?;
                    continue;
                }
                _ => {}
            }
            if self.at_word("import") || self.at_word("from") {
                while !matches!(self.peek().kind, Kind::Newline | Kind::Eof) {
                    self.bump();
                }
                continue;
            }
            if self.at_word("class") {
                // A class of annotated fields declares a record type; any
                // other class is skipped (its record comes from the program,
                // and a field it lacks is refused where it is used).
                let start = self.k;
                if let Some(record) = self.py_record_class() {
                    self.declared.records.push(record);
                    continue;
                }
                self.k = start;
                while !matches!(self.peek().kind, Kind::Newline | Kind::Eof) {
                    self.bump();
                }
                if self.peek().kind == Kind::Newline {
                    self.bump();
                }
                if self.peek().kind == Kind::Indent {
                    let mut depth = 0_usize;
                    loop {
                        match self.peek().kind {
                            Kind::Indent => depth += 1,
                            Kind::Dedent => {
                                depth -= 1;
                                if depth == 0 {
                                    self.bump();
                                    break;
                                }
                            }
                            Kind::Eof => break,
                            _ => {}
                        }
                        self.bump();
                    }
                }
                continue;
            }
            if !self.at_word("def") {
                let tok = self.peek().clone();
                return refuse(
                    tok.pos,
                    format!("expected def, found {}", Self::found(&tok)),
                );
            }
            if self.at_py_test() {
                self.py_test()?;
                continue;
            }
            functions.push(self.py_function()?);
            functions.append(&mut self.hoisted);
        }
        if functions.is_empty() && self.declared.tests.is_empty() {
            return refuse(self.peek().pos, "expected at least one def");
        }
        Ok(functions)
    }

    /// `class Name:` (bases ignored) whose body is only `field: type` lines,
    /// a docstring or `pass`: the record it declares. `None` (and the caller
    /// skips the class) for anything else.
    fn py_record_class(&mut self) -> Option<(String, Vec<String>, Value, Pos)> {
        let pos = self.expect_word("class").ok()?.pos;
        let (name, _) = self.ident("a class name").ok()?;
        if self.at_op("(") {
            while !self.at_op(")") {
                if matches!(self.peek().kind, Kind::Newline | Kind::Eof) {
                    return None;
                }
                self.bump();
            }
            self.bump();
        }
        self.expect_op(":").ok()?;
        if self.peek().kind != Kind::Newline {
            return None;
        }
        self.bump();
        if self.peek().kind != Kind::Indent {
            return None;
        }
        self.bump();
        let mut names = Vec::new();
        let mut fields = Vec::new();
        loop {
            match self.peek().kind {
                Kind::Dedent => {
                    self.bump();
                    break;
                }
                Kind::Eof => break,
                Kind::Newline => {
                    self.bump();
                    continue;
                }
                Kind::Str => {
                    self.bump();
                }
                _ if self.at_word("pass") => {
                    self.bump();
                }
                _ => {
                    let (field, _) = self.ident("a field name").ok()?;
                    self.expect_op(":").ok()?;
                    let (ty, _) = self.py_type().ok()?;
                    if names.contains(&field) {
                        return None;
                    }
                    fields.push(json!([field.clone(), ty]));
                    names.push(field);
                }
            }
            match self.peek().kind {
                Kind::Newline => {
                    self.bump();
                }
                Kind::Dedent | Kind::Eof => {}
                _ => return None,
            }
        }
        if fields.is_empty() {
            return None;
        }
        let entry = json!({"name": name.clone(), "record": fields});
        Some((name, names, entry, pos))
    }

    /// `def test_...()`: a pytest-style test function.
    fn at_py_test(&self) -> bool {
        self.at_word("def")
            && (self.peek_at(1).text == "test" || self.peek_at(1).text.starts_with("test_"))
            && self.peek_at(2).text == "("
            && self.peek_at(3).text == ")"
    }

    /// `def test_x():` whose body holds `assert f(...) == value`,
    /// `assert f(...)`, `assert not f(...)` and
    /// `with pytest.raises(E): f(...)` lines; each becomes a `TestCase` named
    /// `test_x_<n>`. `pytest.raises` expects a trap: the dialect lowers every
    /// exception (`raise`, a failed check) to one.
    #[allow(clippy::too_many_lines)]
    fn py_test(&mut self) -> P<()> {
        self.expect_word("def")?;
        let (name, _) = self.ident("a test name")?;
        self.expect_op("(")?;
        self.expect_op(")")?;
        if self.at_op("->") {
            self.bump();
            self.expect_word("None")?;
        }
        self.expect_op(":")?;
        if self.peek().kind != Kind::Newline {
            return refuse(self.peek().pos, "write the test body on its own lines");
        }
        self.bump();
        if self.peek().kind != Kind::Indent {
            return refuse(self.peek().pos, "expected an indented block");
        }
        self.bump();
        let mut count = 0_usize;
        loop {
            let tok = self.peek().clone();
            match tok.kind {
                Kind::Dedent => {
                    self.bump();
                    break;
                }
                Kind::Eof => break,
                Kind::Newline => {
                    self.bump();
                    continue;
                }
                Kind::Str => {
                    self.bump();
                    self.py_end_of_statement()?;
                    continue;
                }
                _ => {}
            }
            let (function, args, expectation) = if self.at_word("assert") {
                self.bump();
                let negated = self.at_word("not");
                if negated {
                    self.bump();
                }
                let (function, args) = self.py_test_call()?;
                let expectation = if !negated && self.at_op("==") {
                    self.bump();
                    json!({"expect": self.py_test_value()?})
                } else {
                    json!({"truthy": !negated})
                };
                if self.at_op(",") {
                    // The assertion message.
                    self.bump();
                    if self.peek().kind != Kind::Str {
                        return refuse(self.peek().pos, "an assertion message is a string");
                    }
                    self.bump();
                }
                self.py_end_of_statement()?;
                (function, args, expectation)
            } else if self.at_word("with") {
                self.bump();
                if !(self.at_word("pytest")
                    && self.peek_at(1).text == "."
                    && self.peek_at(2).text == "raises")
                {
                    return refuse(tok.pos, "a test's with block is with pytest.raises(...):");
                }
                self.bump();
                self.bump();
                self.bump();
                self.expect_op("(")?;
                let mut depth = 1_usize;
                while depth > 0 {
                    match self.peek().kind {
                        Kind::Newline | Kind::Eof => {
                            return refuse(self.peek().pos, "expected `)`");
                        }
                        _ if self.at_op("(") => depth += 1,
                        _ if self.at_op(")") => depth -= 1,
                        _ => {}
                    }
                    self.bump();
                }
                if self.at_word("as") {
                    self.bump();
                    self.ident("a name")?;
                }
                self.expect_op(":")?;
                let inline = self.peek().kind != Kind::Newline;
                if !inline {
                    self.bump();
                    if self.peek().kind != Kind::Indent {
                        return refuse(self.peek().pos, "expected an indented block");
                    }
                    self.bump();
                }
                let (function, args) = self.py_test_call()?;
                self.py_end_of_statement()?;
                if !inline {
                    if self.peek().kind != Kind::Dedent {
                        return refuse(self.peek().pos, "a pytest.raises block holds one call");
                    }
                    self.bump();
                }
                (function, args, json!({"expect": {"trap": "unreachable"}}))
            } else {
                return refuse(
                    tok.pos,
                    "a test function holds assert f(...) == value and with pytest.raises(...): f(...) lines",
                );
            };
            let mut test = expectation;
            test["name"] = Value::String(format!("{name}_{count}"));
            test["fn"] = Value::String(function);
            test["args"] = Value::Array(args);
            self.declared.tests.push((test, tok.pos));
            count += 1;
        }
        if count == 0 {
            return refuse(self.previous().pos, format!("`{name}` holds no assertion"));
        }
        Ok(())
    }

    /// `f(value, ...)` in a test: the function and its argument values.
    fn py_test_call(&mut self) -> P<(String, Vec<Value>)> {
        let (function, _) = self.ident("the function a test calls")?;
        self.expect_op("(")?;
        let mut args = Vec::new();
        while !self.at_op(")") {
            args.push(self.py_test_value()?);
            if !self.at_op(")") {
                self.expect_op(",")?;
            }
        }
        self.expect_op(")")?;
        Ok((function, args))
    }

    /// A literal value in a test: an int, True/False, a string, a list of
    /// values, or a record class built positionally or by field name.
    fn py_test_value(&mut self) -> P<Value> {
        let tok = self.peek().clone();
        match tok.kind {
            Kind::Num => {
                self.bump();
                return Ok(Self::literal(&tok, None)?.value());
            }
            Kind::Op if tok.text == "-" && self.peek_at(1).kind == Kind::Num => {
                self.bump();
                let number = self.bump();
                return Ok(Self::literal(&number, Some(tok.pos))?.value());
            }
            Kind::Str => {
                self.bump();
                let text: String = serde_json::from_str(&tok.text).unwrap_or_default();
                return Ok(Value::String(text));
            }
            Kind::Op if tok.text == "[" => {
                self.bump();
                let mut items = Vec::new();
                while !self.at_op("]") {
                    items.push(self.py_test_value()?);
                    if !self.at_op("]") {
                        self.expect_op(",")?;
                    }
                }
                self.bump();
                return Ok(Value::Array(items));
            }
            Kind::Name if tok.text == "True" || tok.text == "False" => {
                self.bump();
                return Ok(Value::Bool(tok.text == "True"));
            }
            Kind::Name => {}
            _ => {
                return refuse(
                    tok.pos,
                    format!("expected a literal value, found {}", Self::found(&tok)),
                );
            }
        }
        self.py_test_record(&tok)
    }

    fn py_test_record(&mut self, tok: &Tok) -> P<Value> {
        let Some(fields) = self
            .declared
            .records
            .iter()
            .find(|(name, ..)| *name == tok.text)
            .map(|(_, fields, ..)| fields.clone())
        else {
            return refuse(
                tok.pos,
                format!(
                    "a test argument is a literal or a record class declared above; `{}` is neither",
                    tok.text
                ),
            );
        };
        self.bump();
        self.expect_op("(")?;
        let mut record = Map::new();
        let mut k = 0_usize;
        let mut seen_keyword = false;
        while !self.at_op(")") {
            let argument_pos = self.peek().pos;
            let field = if self.peek().kind == Kind::Name && self.peek_at(1).text == "=" {
                seen_keyword = true;
                let (field, pos) = self.ident("a field name")?;
                self.bump();
                if !fields.contains(&field) {
                    return refuse(pos, format!("`{}` has no field `{field}`", tok.text));
                }
                field
            } else {
                if seen_keyword {
                    return refuse(
                        argument_pos,
                        "record positional arguments must precede keyword arguments",
                    );
                }
                let Some(field) = fields.get(k) else {
                    return refuse(
                        self.peek().pos,
                        format!("`{}` has {} fields", tok.text, fields.len()),
                    );
                };
                field.clone()
            };
            if record.contains_key(&field) {
                return refuse(
                    argument_pos,
                    format!("`{}` gives field `{field}` more than once", tok.text),
                );
            }
            let value = self.py_test_value()?;
            record.insert(field, value);
            k += 1;
            if !self.at_op(")") {
                self.expect_op(",")?;
            }
        }
        self.expect_op(")")?;
        if record.len() != fields.len() {
            return refuse(tok.pos, format!("give every field of `{}`", tok.text));
        }
        Ok(Value::Object(record))
    }

    fn py_function(&mut self) -> P<Function> {
        let pos = self.expect_word("def")?.pos;
        let (name, _) = self.ident("a function name")?;
        if RESERVED_CALLS.contains(&name.as_str()) || PY_BUILTINS.contains(&name.as_str()) {
            return refuse(
                pos,
                format!("`{name}` is a built-in; name the function otherwise"),
            );
        }
        self.expect_op("(")?;
        let mut params = Vec::new();
        while !self.at_op(")") {
            let (param, param_pos) = self.ident("a parameter name")?;
            if !self.at_op(":") {
                return refuse(
                    self.peek().pos,
                    format!("annotate the parameter: {param}: int"),
                );
            }
            self.bump();
            let (ty, _) = self.py_type()?;
            params.push((param, ty, param_pos));
            if !self.at_op(")") {
                self.expect_op(",")?;
            }
        }
        self.expect_op(")")?;
        if !self.at_op("->") {
            return refuse(
                self.peek().pos,
                "annotate the return type: def f(...) -> int:",
            );
        }
        self.bump();
        let returns = self.py_type()?;
        self.scopes = vec![params.iter().map(|(name, _, _)| name.clone()).collect()];
        let assigned = self.py_assigned_params(&params);
        let mut body = self.py_block()?;
        if let NodeKind::List(items) = &mut body.kind {
            // A parameter the body assigns becomes a variable holding its value.
            for (offset, (name, name_pos)) in assigned.into_iter().enumerate() {
                let atom = Node::atom(name_pos, Value::String(name));
                items.insert(
                    offset,
                    Node::list(
                        name_pos,
                        vec![
                            Node::atom(name_pos, Value::String("var".into())),
                            atom.clone(),
                            atom,
                        ],
                    ),
                );
            }
        }
        Ok(Function {
            name,
            pos,
            params,
            returns,
            body,
        })
    }

    /// The parameters the function body assigns (from `:` to the end of
    /// its block).
    fn py_assigned_params(&self, params: &[(String, String, Pos)]) -> Vec<(String, Pos)> {
        let names: Vec<String> = params.iter().map(|(name, _, _)| name.clone()).collect();
        self.py_block_assigns(&names)
    }

    /// Which of `names` the block that starts here (at its `:`) assigns.
    fn py_block_assigns(&self, names: &[String]) -> Vec<(String, Pos)> {
        let mut found: Vec<(String, Pos)> = Vec::new();
        let mut depth = 0_i64;
        let mut k = self.k;
        while let Some(tok) = self.toks.get(k) {
            match tok.kind {
                Kind::Indent => depth += 1,
                Kind::Dedent => {
                    depth -= 1;
                    if depth <= 0 {
                        break;
                    }
                }
                Kind::Eof => break,
                Kind::Name => {
                    let at_start = k > 0
                        && (matches!(
                            self.toks[k - 1].kind,
                            Kind::Newline | Kind::Indent | Kind::Dedent
                        ) || self.toks[k - 1].text == ":");
                    let assigns = self.toks.get(k + 1).is_some_and(|next| {
                        matches!(
                            next.text.as_str(),
                            "=" | "+=" | "-=" | "*=" | "//=" | "%=" | ">>=" | "<<=" | "&=" | ","
                        )
                    });
                    if at_start && assigns {
                        // Every target of the statement: `x = e`, `x += e`,
                        // or each name of a tuple target `a, b = e1, e2`.
                        let mut targets = vec![k];
                        if self.toks[k + 1].text == "," {
                            let mut j = k;
                            while self.toks.get(j + 1).is_some_and(|t| t.text == ",")
                                && self
                                    .toks
                                    .get(j + 2)
                                    .is_some_and(|t| matches!(t.kind, Kind::Name))
                            {
                                j += 2;
                                targets.push(j);
                            }
                            if self.toks.get(j + 1).is_none_or(|t| t.text != "=") {
                                // Not a tuple assignment: only the first
                                // name keeps the earlier conservative reading.
                                targets.truncate(1);
                            }
                        }
                        for target in targets {
                            let tok = &self.toks[target];
                            if names.contains(&tok.text)
                                && !found.iter().any(|(name, _)| *name == tok.text)
                            {
                                found.push((tok.text.clone(), tok.pos));
                            }
                        }
                    }
                }
                _ => {}
            }
            k += 1;
        }
        found
    }

    fn py_type(&mut self) -> P<(String, Pos)> {
        let (name, pos) = self.ident("a type")?;
        let ty = match name.as_str() {
            "int" => "i64".to_owned(),
            "bool" => "bool".to_owned(),
            "str" => "text".to_owned(),
            "list" | "List" => {
                self.expect_op("[")?;
                let (inner, _) = self.py_type()?;
                self.expect_op("]")?;
                format!("Vec<{inner}>")
            }
            "float" => {
                return refuse(
                    pos,
                    "the Python dialect has no floating-point numbers: use int (f32/f64 need an AF1 frame: help types)",
                );
            }
            "dict" | "Dict" | "set" | "Set" | "tuple" | "Tuple" | "Optional" | "None" => {
                return refuse(
                    pos,
                    format!(
                        "`{name}` is not supported: use int, bool, str, list[T] or a record type"
                    ),
                );
            }
            _ => name,
        };
        Ok((ty, pos))
    }

    /// `:` then a block: an indented statement list, or one simple statement
    /// on the same line.
    fn py_block(&mut self) -> P<Node> {
        let colon = self.expect_op(":")?;
        self.scopes.push(Vec::new());
        let mut body = Vec::new();
        if self.peek().kind == Kind::Newline {
            self.bump();
            if self.peek().kind != Kind::Indent {
                return refuse(self.peek().pos, "expected an indented block");
            }
            self.bump();
            while !matches!(self.peek().kind, Kind::Dedent | Kind::Eof) {
                if self.peek().kind == Kind::Newline {
                    self.bump();
                    continue;
                }
                if let Some(statement) = self.py_statement()? {
                    Self::py_push(&mut body, statement);
                }
            }
            if self.peek().kind == Kind::Dedent {
                self.bump();
            }
        } else if let Some(statement) = self.py_statement()? {
            Self::py_push(&mut body, statement);
        }
        self.scopes.pop();
        hoist_branch_declarations(&mut body);
        Ok(Node::list(colon.pos, body))
    }

    /// Appends a statement, flattening a chained assignment's `seq`.
    fn py_push(body: &mut Vec<Node>, statement: Node) {
        if statement.head() == Some("seq") {
            if let NodeKind::List(mut items) = statement.kind
                && let Some(Node {
                    kind: NodeKind::List(inner),
                    ..
                }) = items.pop()
            {
                body.extend(inner);
            }
            return;
        }
        body.push(statement);
    }

    fn py_end_of_statement(&mut self) -> P<()> {
        match self.peek().kind {
            Kind::Newline => {
                self.bump();
                Ok(())
            }
            Kind::Dedent | Kind::Eof => Ok(()),
            _ => {
                let tok = self.peek().clone();
                refuse(
                    tok.pos,
                    format!("expected the end of the line, found {}", Self::found(&tok)),
                )
            }
        }
    }

    fn py_declared(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .any(|scope| scope.iter().any(|n| n == name))
    }

    fn py_declare(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(name.to_owned());
        }
    }

    #[allow(clippy::too_many_lines)]
    fn py_statement(&mut self) -> P<Option<Node>> {
        let tok = self.peek().clone();
        let pos = tok.pos;
        let keyword = |name: &str| Node::atom(pos, Value::String(name.to_owned()));
        if tok.kind == Kind::Str {
            // A docstring or a bare string does nothing.
            self.bump();
            self.py_end_of_statement()?;
            return Ok(None);
        }
        let word = (tok.kind == Kind::Name).then_some(tok.text.as_str());
        let statement = match word {
            Some("pass") => {
                self.bump();
                self.py_end_of_statement()?;
                return Ok(None);
            }
            Some("return") => {
                self.bump();
                if matches!(self.peek().kind, Kind::Newline | Kind::Dedent | Kind::Eof) {
                    return refuse(pos, "return needs a value");
                }
                let value = self.expr()?;
                self.py_end_of_statement()?;
                Node::list(pos, vec![keyword("return"), value])
            }
            Some("if") => return self.py_if().map(Some),
            Some("while") => {
                self.bump();
                let condition = self.expr()?;
                let condition = self.truth(condition);
                let body = self.py_block()?;
                Node::list(pos, vec![keyword("while"), condition, body])
            }
            Some("for") => return self.py_for().map(Some),
            Some("assert") => {
                self.bump();
                let condition = self.expr()?;
                let condition = self.truth(condition);
                if self.at_op(",") {
                    self.bump();
                    self.expr()?;
                }
                self.py_end_of_statement()?;
                let trap = Node::list(pos, vec![Node::list(pos, vec![keyword("trap")])]);
                Node::list(
                    pos,
                    vec![keyword("if"), Node::op(pos, "not", vec![condition]), trap],
                )
            }
            Some("raise") => {
                while !matches!(self.peek().kind, Kind::Newline | Kind::Dedent | Kind::Eof) {
                    self.bump();
                }
                self.py_end_of_statement()?;
                Node::list(pos, vec![keyword("trap")])
            }
            Some(other @ ("break" | "continue")) => {
                self.bump();
                self.py_end_of_statement()?;
                Node::list(pos, vec![keyword(other)])
            }
            Some(
                other @ ("class" | "lambda" | "with" | "try" | "global" | "nonlocal" | "yield"
                | "del" | "match" | "async" | "await"),
            ) => {
                return refuse(
                    pos,
                    format!("`{other}` is not supported in a function body"),
                );
            }
            Some("def") => {
                let scopes = std::mem::take(&mut self.scopes);
                let helper = self.py_function();
                self.scopes = scopes;
                self.hoisted.push(helper?);
                return Ok(None);
            }
            Some("elif" | "else") => {
                return refuse(pos, format!("`{}` without a matching if", tok.text));
            }
            Some(name) if !PY_KEYWORDS.contains(&name) && self.peek_at(1).kind == Kind::Op => {
                let name = name.to_owned();
                let op = self.peek_at(1).text.clone();
                match op.as_str() {
                    "=" if self.peek_at(2).kind == Kind::Name && self.peek_at(3).text == "=" => {
                        // a = b = e: every name takes the same value.
                        let mut targets = vec![name];
                        self.bump();
                        self.bump();
                        while self.peek().kind == Kind::Name && self.peek_at(1).text == "=" {
                            targets.push(self.bump().text);
                            self.bump();
                        }
                        let value = self.expr()?;
                        self.py_end_of_statement()?;
                        let mut statements = Vec::new();
                        for target in targets {
                            let atom = Node::atom(pos, Value::String(target.clone()));
                            if self.py_declared(&target) {
                                statements.push(Node::list(
                                    pos,
                                    vec![keyword("set"), atom, value.clone()],
                                ));
                            } else {
                                self.py_declare(&target);
                                statements.push(Node::list(
                                    pos,
                                    vec![keyword("var"), atom, value.clone()],
                                ));
                            }
                        }
                        return Ok(Some(Node::list(
                            pos,
                            vec![keyword("seq"), Node::list(pos, statements)],
                        )));
                    }
                    "=" => {
                        self.bump();
                        self.bump();
                        let value = self.expr()?;
                        self.py_end_of_statement()?;
                        let target = Node::atom(pos, Value::String(name.clone()));
                        if self.py_declared(&name) {
                            Node::list(pos, vec![keyword("set"), target, value])
                        } else {
                            self.py_declare(&name);
                            Node::list(pos, vec![keyword("var"), target, value])
                        }
                    }
                    ":" => {
                        self.bump();
                        self.bump();
                        let (ty, ty_pos) = self.py_type()?;
                        self.expect_op("=")?;
                        let value = self.expr()?;
                        self.py_end_of_statement()?;
                        let target = Node::atom(pos, Value::String(name.clone()));
                        let ty = Node::atom(ty_pos, Value::String(ty));
                        if self.py_declared(&name) {
                            Node::list(pos, vec![keyword("set"), target, value])
                        } else {
                            self.py_declare(&name);
                            Node::list(pos, vec![keyword("var"), target, ty, value])
                        }
                    }
                    ">>=" | "<<=" | "&=" => {
                        if !self.py_declared(&name) {
                            return refuse(
                                pos,
                                format!("`{name}` is assigned before it has a value"),
                            );
                        }
                        self.bump();
                        let op_tok = self.bump();
                        let value = self.expr()?;
                        self.py_end_of_statement()?;
                        let target = Node::atom(pos, Value::String(name.clone()));
                        let combined = if op == "&=" {
                            Self::py_mask(op_tok.pos, target, &value)?
                        } else {
                            Self::py_shifted(op_tok.pos, &op[..2], target, value)
                        };
                        Node::list(
                            pos,
                            vec![
                                keyword("set"),
                                Node::atom(pos, Value::String(name)),
                                combined,
                            ],
                        )
                    }
                    "+=" | "-=" | "*=" | "//=" | "%=" => {
                        if !self.py_declared(&name) {
                            return refuse(
                                pos,
                                format!("`{name}` is assigned before it has a value"),
                            );
                        }
                        self.bump();
                        let op_tok = self.bump();
                        let word = match op.as_str() {
                            "+=" => "add",
                            "-=" => "sub",
                            "*=" => "mul",
                            "//=" => "floordiv",
                            _ => "floormod",
                        };
                        let value = self.expr()?;
                        self.py_end_of_statement()?;
                        let target = Node::atom(pos, Value::String(name.clone()));
                        Node::list(
                            pos,
                            vec![
                                keyword("set"),
                                Node::atom(pos, Value::String(name)),
                                Node::op(op_tok.pos, word, vec![target, value]),
                            ],
                        )
                    }
                    "/=" => return refuse(self.peek_at(1).pos, "/ is float division: write //="),
                    "," => return self.py_tuple_assignment().map(Some),
                    _ => return self.py_expression_statement(),
                }
            }
            _ => return self.py_expression_statement(),
        };
        Ok(Some(statement))
    }

    /// `a, b = e1, e2` (every value is computed before any name changes) and
    /// `q, r = divmod(a, b)`.
    fn py_tuple_assignment(&mut self) -> P<Node> {
        let pos = self.peek().pos;
        let mut targets = vec![self.ident("a name")?];
        while self.at_op(",") {
            self.bump();
            targets.push(self.ident("a name")?);
        }
        if !self.at_op("=") {
            return refuse(self.peek().pos, "expected = after the names");
        }
        self.bump();
        let values = if self.at_word("divmod") && self.peek_at(1).text == "(" {
            let call = self.bump().pos;
            self.expect_op("(")?;
            let a = self.expr()?;
            self.expect_op(",")?;
            let b = self.expr()?;
            self.expect_op(")")?;
            vec![
                Node::op(call, "floordiv", vec![a.clone(), b.clone()]),
                Node::op(call, "floormod", vec![a, b]),
            ]
        } else {
            let mut values = vec![self.expr()?];
            while self.at_op(",") {
                self.bump();
                values.push(self.expr()?);
            }
            values
        };
        if values.len() != targets.len() {
            return refuse(
                pos,
                format!(
                    "{} names but {} values (tuples are not supported)",
                    targets.len(),
                    values.len()
                ),
            );
        }
        self.py_end_of_statement()?;
        let word = |w: &str| Node::atom(pos, Value::String(w.to_owned()));
        self.fresh += 1;
        let tag = self.fresh;
        let mut statements = Vec::new();
        for ((name, name_pos), value) in targets.iter().zip(values) {
            statements.push(Node::list(
                *name_pos,
                vec![
                    word("let"),
                    Node::atom(*name_pos, Value::String(format!("{name}_next_{tag}"))),
                    value,
                ],
            ));
        }
        for (name, name_pos) in &targets {
            let held = Node::atom(*name_pos, Value::String(format!("{name}_next_{tag}")));
            let target = Node::atom(*name_pos, Value::String(name.clone()));
            if self.py_declared(name) {
                statements.push(Node::list(*name_pos, vec![word("set"), target, held]));
            } else {
                self.py_declare(name);
                statements.push(Node::list(*name_pos, vec![word("var"), target, held]));
            }
        }
        Ok(Node::list(
            pos,
            vec![word("seq"), Node::list(pos, statements)],
        ))
    }

    fn py_expression_statement(&mut self) -> P<Option<Node>> {
        let tok = self.peek().clone();
        if Self::starts_expression(&tok) {
            return refuse(
                tok.pos,
                "an expression on its own line has no effect: assign it or return it",
            );
        }
        refuse(
            tok.pos,
            format!("expected a statement, found {}", Self::found(&tok)),
        )
    }

    fn py_if(&mut self) -> P<Node> {
        let pos = self.bump().pos;
        let keyword = Node::atom(pos, Value::String("if".into()));
        let condition = self.expr()?;
        let condition = self.truth(condition);
        let then = self.py_block()?;
        if self.at_word("elif") {
            let nested = self.py_if()?;
            return Ok(Node::list(
                pos,
                vec![keyword, condition, then, Node::list(pos, vec![nested])],
            ));
        }
        if self.at_word("else") {
            self.bump();
            let otherwise = self.py_block()?;
            return Ok(Node::list(pos, vec![keyword, condition, then, otherwise]));
        }
        Ok(Node::list(pos, vec![keyword, condition, then]))
    }

    #[allow(clippy::too_many_lines)]
    fn py_for(&mut self) -> P<Node> {
        let pos = self.bump().pos;
        let (first, first_pos) = self.ident("a loop variable")?;
        let second = if self.at_op(",") {
            self.bump();
            Some(self.ident("an element variable")?)
        } else {
            None
        };
        self.expect_word("in")?;
        let keyword = |name: &str| Node::atom(pos, Value::String(name.to_owned()));
        let enumerated = self.at_word("enumerate") && self.peek_at(1).text == "(";
        let mut prelude = Vec::new();
        let mut node = if enumerated {
            let Some((element, element_pos)) = &second else {
                return refuse(
                    first_pos,
                    "enumerate binds two names: for i, x in enumerate(xs)",
                );
            };
            self.bump();
            self.expect_op("(")?;
            let list = self.expr()?;
            self.expect_op(")")?;
            vec![
                keyword("for-indexed"),
                Node::atom(first_pos, Value::String(format!("{first}_u64"))),
                Node::atom(*element_pos, Value::String(element.clone())),
                list,
            ]
        } else {
            match self.py_iterable()? {
                PyIter::List(list) => {
                    if second.is_some() {
                        return refuse(
                            first_pos,
                            "two loop variables need enumerate(xs) or zip(..)",
                        );
                    }
                    vec![
                        keyword("for"),
                        Node::atom(first_pos, Value::String(first.clone())),
                        list,
                    ]
                }
                PyIter::Range(start, end) => {
                    if second.is_some() {
                        return refuse(first_pos, "range binds one name");
                    }
                    vec![
                        keyword("range"),
                        Node::atom(first_pos, Value::String(first.clone())),
                        start,
                        end,
                    ]
                }
                PyIter::Enumerate(_) => {
                    return refuse(
                        first_pos,
                        "enumerate binds two names: for i, x in enumerate(xs)",
                    );
                }
                PyIter::Stepped { start, step, count } => {
                    if second.is_some() {
                        return refuse(first_pos, "range binds one name");
                    }
                    let k_name = format!("{first}_k_");
                    let k = Node::atom(first_pos, Value::String(k_name.clone()));
                    let word = if self.py_declared(&first) {
                        "set"
                    } else {
                        "var"
                    };
                    prelude.push(Node::list(
                        first_pos,
                        vec![
                            Node::atom(first_pos, Value::String(word.into())),
                            Node::atom(first_pos, Value::String(first.clone())),
                            stepped_value(first_pos, &start, &step, k),
                        ],
                    ));
                    vec![
                        keyword("range"),
                        Node::atom(first_pos, Value::String(k_name)),
                        Node::atom(first_pos, Value::from(0)),
                        count,
                    ]
                }
                PyIter::Views(views) => {
                    let names: Vec<(String, Pos)> = std::iter::once((first.clone(), first_pos))
                        .chain(second.clone())
                        .collect();
                    if names.len() != views.len() {
                        return refuse(first_pos, "one loop variable per zipped list");
                    }
                    let index_name = format!("{first}_at_");
                    let index = Node::atom(pos, Value::String(index_name.clone()));
                    let (start, end, elements) = Self::py_view_range(pos, &views, &index);
                    for ((name, name_pos), element) in names.iter().zip(elements) {
                        prelude.push(Node::list(
                            *name_pos,
                            vec![
                                Node::atom(*name_pos, Value::String("var".into())),
                                Node::atom(*name_pos, Value::String(name.clone())),
                                element,
                            ],
                        ));
                    }
                    vec![
                        keyword("range"),
                        Node::atom(pos, Value::String(index_name)),
                        start,
                        end,
                    ]
                }
            }
        };
        // A loop variable that is declared outside the loop, or that the
        // body assigns, is a plain variable in Python: iterate under another
        // name and assign it at the top of each pass.
        let plain = matches!(
            node.first().and_then(Node::head_word),
            Some("for" | "range")
        ) && node.get(1).and_then(Node::head_word) == Some(first.as_str());
        if plain
            && (self.py_declared(&first)
                || !self
                    .py_block_assigns(std::slice::from_ref(&first))
                    .is_empty())
        {
            let iterate = format!("{first}_item_");
            node[1] = Node::atom(first_pos, Value::String(iterate.clone()));
            let word = if self.py_declared(&first) {
                "set"
            } else {
                "var"
            };
            prelude.push(Node::list(
                first_pos,
                vec![
                    Node::atom(first_pos, Value::String(word.into())),
                    Node::atom(first_pos, Value::String(first.clone())),
                    Node::atom(first_pos, Value::String(iterate)),
                ],
            ));
        }
        self.scopes.push(vec![first.clone()]);
        if let Some((element, _)) = &second {
            self.py_declare(element);
        }
        let mut body = self.py_block()?;
        self.scopes.pop();
        if !prelude.is_empty() {
            if let NodeKind::List(items) = &mut body.kind {
                for (offset, item) in prelude.into_iter().enumerate() {
                    items.insert(offset, item);
                }
            }
        } else if enumerated {
            // enumerate's index is an int (i64) like every Python integer.
            let index = Node::list(
                first_pos,
                vec![
                    Node::atom(first_pos, Value::String("var".into())),
                    Node::atom(first_pos, Value::String(first.clone())),
                    Node::op(
                        first_pos,
                        "to",
                        vec![
                            Node::atom(first_pos, Value::String("i64".into())),
                            Node::atom(first_pos, Value::String(format!("{first}_u64"))),
                        ],
                    ),
                ],
            );
            if let NodeKind::List(items) = &mut body.kind {
                items.insert(0, index);
            }
        }
        node.push(body);
        Ok(Node::list(pos, node))
    }
}

/// Python keywords: never assignment targets.
const PY_KEYWORDS: [&str; 26] = [
    "def", "return", "if", "elif", "else", "for", "while", "in", "not", "and", "or", "pass",
    "break", "continue", "True", "False", "None", "import", "from", "class", "lambda", "assert",
    "raise", "is", "with", "try",
];
/// Python built-ins a function may not be named after.
const PY_BUILTINS: [&str; 8] = [
    "sum",
    "any",
    "all",
    "range",
    "enumerate",
    "int",
    "print",
    "sorted",
];

/// A read-only window `list[start:end]` over a list, with i64 bounds.
struct View {
    list: Node,
    start: Node,
    end: Node,
}

impl View {
    /// The element at window position `index` (an i64 node counted from the
    /// start of the list, not of the window).
    fn element_at(&self, pos: Pos, index: Node) -> Node {
        Node::op(
            pos,
            "get",
            vec![
                self.list.clone(),
                Node::op(
                    pos,
                    "to",
                    vec![Node::atom(pos, Value::String("u64".into())), index],
                ),
            ],
        )
    }
}

fn len_i64(pos: Pos, list: &Node) -> Node {
    Node::op(
        pos,
        "to",
        vec![
            Node::atom(pos, Value::String("i64".into())),
            Node::op(pos, "len", vec![list.clone()]),
        ],
    )
}

/// What a Python loop or generator iterates.
enum PyIter {
    /// A list, element by element (the ordinary `for x in xs`).
    List(Node),
    /// `range(start, end)`.
    Range(Node, Node),
    /// `zip(v, w, ..)` or one slice view: index positions of the windows.
    Views(Vec<View>),
    /// `enumerate(xs)`: an i64 index and the element at it.
    Enumerate(Node),
    /// `range(start, stop, step)`: position k of `0..count` is
    /// `start + k * step`.
    Stepped {
        start: Node,
        step: Node,
        count: Node,
    },
}

/// The number of values of `range(start, stop, step)`: `max(0, ceil((stop -
/// start) / step))`, written with floor division (a zero step fails like a
/// division by zero, as Python refuses it).
fn stepped_count(pos: Pos, start: &Node, end: Node, step: &Node) -> Node {
    let gap = Node::op(pos, "sub", vec![start.clone(), end]);
    let floor = Node::op(pos, "floordiv", vec![gap, step.clone()]);
    Node::op(
        pos,
        "max",
        vec![
            Node::op(pos, "neg", vec![floor]),
            Node::atom(pos, Value::from(0)),
        ],
    )
}

fn stepped_value(pos: Pos, start: &Node, step: &Node, k: Node) -> Node {
    Node::op(
        pos,
        "add",
        vec![start.clone(), Node::op(pos, "mul", vec![k, step.clone()])],
    )
}

/// The names a statement list declares at its top level, with the value of
/// each declaration.
fn top_declarations(list: &Node) -> Vec<(String, Node)> {
    let NodeKind::List(items) = &list.kind else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|s| s.head() == Some("var"))
        .filter_map(|s| match &s.kind {
            NodeKind::List(parts) => {
                let name = parts.get(1)?.head_word()?.to_owned();
                Some((name, parts.last()?.clone()))
            }
            NodeKind::Atom(_) => None,
        })
        .collect()
}

/// The names every branch of a complete `if`/`elif`/`else` chain declares.
fn chain_declarations(statement: &Node) -> Option<Vec<(String, Node)>> {
    let NodeKind::List(parts) = &statement.kind else {
        return None;
    };
    let (then, otherwise) = (parts.get(2)?, parts.get(3)?);
    let mut names = top_declarations(then);
    let rest = match &otherwise.kind {
        NodeKind::List(items) if items.len() == 1 && items[0].head() == Some("if") => {
            chain_declarations(&items[0])?
        }
        _ => top_declarations(otherwise),
    };
    names.retain(|(name, _)| rest.iter().any(|(other, _)| other == name));
    Some(names)
}

/// Turns the top-level declarations of `name` in every branch of the chain
/// into assignments.
fn assign_in_chain(statement: &mut Node, name: &str) {
    let NodeKind::List(parts) = &mut statement.kind else {
        return;
    };
    for branch in parts.iter_mut().skip(2) {
        let NodeKind::List(items) = &mut branch.kind else {
            continue;
        };
        if items.len() == 1 && items[0].head() == Some("if") {
            assign_in_chain(&mut items[0], name);
            continue;
        }
        for item in items.iter_mut() {
            if item.head() != Some("var") {
                continue;
            }
            if let NodeKind::List(fields) = &mut item.kind
                && fields.get(1).and_then(Node::head_word) == Some(name)
            {
                let value = fields.pop();
                let target = fields[1].clone();
                let pos = fields[0].pos;
                *fields = vec![Node::atom(pos, Value::String("set".into())), target];
                fields.extend(value);
            }
        }
    }
}

/// Python: a name assigned in every branch of a complete `if`/`elif`/`else`
/// is defined after it. It is declared before the chain (typed by its first
/// value, which is not evaluated there) and each branch assigns it. A name
/// some path leaves unassigned stays local to its branch.
fn hoist_branch_declarations(body: &mut Vec<Node>) {
    let mut index = 0;
    while index < body.len() {
        if body[index].head() == Some("if")
            && let Some(names) = chain_declarations(&body[index])
        {
            for (name, witness) in names {
                assign_in_chain(&mut body[index], &name);
                let pos = body[index].pos;
                body.insert(
                    index,
                    Node::list(
                        pos,
                        vec![
                            Node::atom(pos, Value::String("declare".into())),
                            Node::atom(pos, Value::String(name)),
                            witness,
                        ],
                    ),
                );
                index += 1;
            }
        }
        index += 1;
    }
}

/// Replaces the variable `name` by `with` inside an expression, leaving
/// operation words, field names, conversion types, callee names and inner
/// folds that rebind `name` untouched.
fn substitute(node: &mut Node, name: &str, with: &Node) {
    let replace = matches!(&node.kind, NodeKind::Atom(Value::String(n)) if n == name);
    if replace {
        let pos = node.pos;
        *node = with.clone();
        node.pos = pos;
        return;
    }
    let head = node.head().map(str::to_owned);
    let NodeKind::List(items) = &mut node.kind else {
        return;
    };
    let skip: &[usize] = match head.as_deref() {
        Some("field") => &[0, 2],
        Some("to" | "call" | "tcall") => &[0, 1],
        Some("fold") => {
            if matches!(&items.get(2).map(|n| &n.kind), Some(NodeKind::Atom(Value::String(n))) if n == name)
            {
                // The inner fold rebinds the name: only its iterable sees ours.
                if let Some(list) = items.get_mut(3) {
                    substitute(list, name, with);
                }
                return;
            }
            &[0, 1, 2]
        }
        _ => &[0],
    };
    for (index, item) in items.iter_mut().enumerate() {
        if !skip.contains(&index) {
            substitute(item, name, with);
        }
    }
}

impl Parser {
    /// Whether `name[ ... : ... ]` starts here (a slice, not an index).
    fn py_slice_ahead(&self) -> bool {
        if self.peek().kind != Kind::Name || self.peek_at(1).text != "[" {
            return false;
        }
        let mut depth = 0_usize;
        let mut k = self.k + 1;
        while let Some(tok) = self.toks.get(k) {
            match (tok.kind, tok.text.as_str()) {
                (Kind::Op, "[" | "(") => depth += 1,
                (Kind::Op, "]" | ")") => {
                    depth -= 1;
                    if depth == 0 {
                        return false;
                    }
                }
                (Kind::Op, ":") if depth == 1 => return true,
                (Kind::Newline | Kind::Eof, _) => return false,
                _ => {}
            }
            k += 1;
        }
        false
    }

    /// A slice bound; `-k` counts from the end of `list`.
    fn py_bound(&mut self, list: &Node) -> P<Node> {
        if self.at_op("-")
            && self.peek_at(1).kind == Kind::Num
            && matches!(self.peek_at(2).text.as_str(), ":" | "]")
        {
            let pos = self.bump().pos;
            let k = Self::literal(&self.bump(), None)?;
            return Ok(Node::op(pos, "sub", vec![len_i64(pos, list), k]));
        }
        self.expr()
    }

    /// `name[start:end]` (either bound optional; the end is clamped to the
    /// list's length as in Python).
    fn py_slice(&mut self) -> P<View> {
        let (name, pos) = self.ident("a list")?;
        let list = Node::atom(pos, Value::String(name));
        self.expect_op("[")?;
        let start = if self.at_op(":") {
            Node::atom(pos, Value::from(0))
        } else {
            self.py_bound(&list)?
        };
        self.expect_op(":")?;
        let end = if self.at_op("]") {
            len_i64(pos, &list)
        } else {
            let bound = self.py_bound(&list)?;
            Node::op(pos, "min", vec![bound, len_i64(pos, &list)])
        };
        if self.at_op(":") {
            return refuse(self.peek().pos, "a slice step is not supported");
        }
        self.expect_op("]")?;
        Ok(View { list, start, end })
    }

    /// A zip argument: a slice or a whole list.
    fn py_view(&mut self) -> P<View> {
        if self.py_slice_ahead() {
            return self.py_slice();
        }
        let list = self.or_()?;
        let pos = list.pos;
        Ok(View {
            start: Node::atom(pos, Value::from(0)),
            end: len_i64(pos, &list),
            list,
        })
    }

    /// The iterable after `in`.
    fn py_iterable(&mut self) -> P<PyIter> {
        let callee = (self.peek().kind == Kind::Name && self.peek_at(1).text == "(")
            .then(|| self.peek().text.clone());
        match callee.as_deref() {
            Some("range") => {
                let call = self.bump();
                self.expect_op("(")?;
                let a = self.expr()?;
                let (start, end) = if self.at_op(",") {
                    self.bump();
                    (a, self.expr()?)
                } else {
                    (Node::atom(call.pos, Value::from(0)), a)
                };
                if self.at_op(",") {
                    self.bump();
                    let step = self.expr()?;
                    self.expect_op(")")?;
                    let count = stepped_count(call.pos, &start, end, &step);
                    return Ok(PyIter::Stepped { start, step, count });
                }
                self.expect_op(")")?;
                Ok(PyIter::Range(start, end))
            }
            Some("enumerate") => {
                self.bump();
                self.expect_op("(")?;
                let list = self.expr()?;
                self.expect_op(")")?;
                Ok(PyIter::Enumerate(list))
            }
            Some("zip") => {
                self.bump();
                self.expect_op("(")?;
                let mut views = vec![self.py_view()?];
                while self.at_op(",") {
                    self.bump();
                    views.push(self.py_view()?);
                }
                self.expect_op(")")?;
                Ok(PyIter::Views(views))
            }
            _ if self.py_slice_ahead() => Ok(PyIter::Views(vec![self.py_slice()?])),
            _ => Ok(PyIter::List(self.or_()?)),
        }
    }

    /// Index iteration for views: the range bounds and, for each loop name,
    /// its element at index `index`.
    fn py_view_range(pos: Pos, views: &[View], index: &Node) -> (Node, Node, Vec<Node>) {
        if let [view] = views {
            return (
                view.start.clone(),
                view.end.clone(),
                vec![view.element_at(pos, index.clone())],
            );
        }
        let mut length: Option<Node> = None;
        for view in views {
            let size = Node::op(pos, "sub", vec![view.end.clone(), view.start.clone()]);
            length = Some(match length {
                None => size,
                Some(previous) => Node::op(pos, "min", vec![previous, size]),
            });
        }
        let elements = views
            .iter()
            .map(|view| {
                view.element_at(
                    pos,
                    Node::op(pos, "add", vec![view.start.clone(), index.clone()]),
                )
            })
            .collect();
        (
            Node::atom(pos, Value::from(0)),
            length.unwrap_or_else(|| Node::atom(pos, Value::from(0))),
            elements,
        )
    }

    /// The rest of a Python generator after its value: one or more `for
    /// names in iterable [if c]...` clauses, as nested folds (the caller
    /// consumes the closing bracket). An outer clause of a nested `count` adds
    /// the inner counts; `sum`, `any` and `all` nest as themselves.
    #[allow(clippy::too_many_lines)]
    fn py_generator(&mut self, pos: Pos, kind: &str, value: Node) -> P<Node> {
        let mut clauses = Vec::new();
        while self.at_word("for") {
            self.bump();
            let mut names = vec![self.ident("a loop variable")?];
            while self.at_op(",") {
                self.bump();
                names.push(self.ident("a loop variable")?);
            }
            self.expect_word("in")?;
            let iterable = self.py_iterable()?;
            let mut condition: Option<Node> = None;
            while self.at_word("if") {
                let if_pos = self.bump().pos;
                let c = self.or_()?;
                let c = self.truth(c);
                condition = Some(match condition {
                    None => c,
                    Some(previous) => Node::op(if_pos, "and", vec![previous, c]),
                });
            }
            clauses.push((names, iterable, condition));
        }
        if clauses.is_empty() {
            self.expect_word("for")?;
        }
        if clauses.len() > 1 && matches!(kind, "max" | "min") {
            return refuse(pos, format!("{kind} over a generator takes one for clause"));
        }
        let mut value = if matches!(kind, "any" | "all") {
            self.truth(value)
        } else {
            value
        };
        let innermost = clauses.len() - 1;
        for (depth, (names, iterable, mut condition)) in clauses.into_iter().enumerate().rev() {
            let this_kind = if depth == innermost || kind != "count" {
                kind
            } else {
                "sum"
            };
            let (variable, list) = match iterable {
                PyIter::List(list) => {
                    if names.len() != 1 {
                        return refuse(pos, "two loop variables need zip(..)");
                    }
                    (
                        Node::atom(names[0].1, Value::String(names[0].0.clone())),
                        list,
                    )
                }
                PyIter::Range(start, end) => {
                    if names.len() != 1 {
                        return refuse(pos, "range binds one name");
                    }
                    (
                        Node::atom(names[0].1, Value::String(names[0].0.clone())),
                        Node::op(pos, "range", vec![start, end]),
                    )
                }
                PyIter::Stepped { start, step, count } => {
                    if names.len() != 1 {
                        return refuse(pos, "range binds one name");
                    }
                    let k_name = format!("{}_k_", names[0].0);
                    let k = Node::atom(pos, Value::String(k_name.clone()));
                    let current = stepped_value(pos, &start, &step, k);
                    substitute(&mut value, &names[0].0, &current);
                    if let Some(c) = condition.as_mut() {
                        substitute(c, &names[0].0, &current);
                    }
                    (
                        Node::atom(pos, Value::String(k_name)),
                        Node::op(pos, "range", vec![Node::atom(pos, Value::from(0)), count]),
                    )
                }
                PyIter::Enumerate(list) => {
                    if names.len() != 2 {
                        return refuse(pos, "enumerate binds two names: i, x");
                    }
                    let index = Node::atom(names[0].1, Value::String(names[0].0.clone()));
                    let view = View {
                        start: Node::atom(pos, Value::from(0)),
                        end: len_i64(pos, &list),
                        list,
                    };
                    let element = view.element_at(pos, index.clone());
                    substitute(&mut value, &names[1].0, &element);
                    if let Some(c) = condition.as_mut() {
                        substitute(c, &names[1].0, &element);
                    }
                    (index, Node::op(pos, "range", vec![view.start, view.end]))
                }
                PyIter::Views(views) => {
                    if names.len() != views.len() {
                        return refuse(pos, "one loop variable per zipped list");
                    }
                    let index_name = format!("{}_at_", names[0].0);
                    let index = Node::atom(pos, Value::String(index_name.clone()));
                    let (start, end, elements) = Self::py_view_range(pos, &views, &index);
                    for ((name, _), element) in names.iter().zip(&elements) {
                        substitute(&mut value, name, element);
                        if let Some(c) = condition.as_mut() {
                            substitute(c, name, element);
                        }
                    }
                    (
                        Node::atom(pos, Value::String(index_name)),
                        Node::op(pos, "range", vec![start, end]),
                    )
                }
            };
            let mut args = vec![
                Node::atom(pos, Value::String(this_kind.to_owned())),
                variable,
                list,
                value,
            ];
            if let Some(c) = condition {
                args.push(c);
            }
            value = Node::op(pos, "fold", args);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(text: &str) -> Value {
        parse(text).unwrap().frame["fns"][0]["body"].clone()
    }

    fn refused(text: &str) -> FamiliarError {
        parse(text).expect_err("refused")
    }

    #[test]
    fn python_record_classes_and_tests_join_the_frame() {
        let text = "import pytest\nfrom dataclasses import dataclass\n\n@dataclass\nclass Point:\n    \"\"\"A point.\"\"\"\n    x: int\n    y: int\n\nclass Helper:\n    def run(self) -> int:\n        return 1\n\ndef dist(a: Point, b: Point) -> int:\n    return abs(a.x - b.x) + abs(a.y - b.y)\n\ndef test_dist() -> None:\n    assert dist(Point(1, 2), Point(x=4, y=-2)) == 7, \"seven\"\n    assert not dist(Point(0, 0), Point(0, 0))\n    with pytest.raises(ValueError):\n        dist(Point(0, 0), Point(0, 1))\n";
        let parsed = parse(text).unwrap();
        let frame = &parsed.frame;
        assert_eq!(
            frame["types"],
            json!([{"name": "Point", "record": [["x", "i64"], ["y", "i64"]]}])
        );
        assert_eq!(
            frame["tests"],
            json!([
                {"name": "test_dist_0", "fn": "dist", "args": [{"x": 1, "y": 2}, {"x": 4, "y": -2}], "expect": 7},
                {"name": "test_dist_1", "fn": "dist", "args": [{"x": 0, "y": 0}, {"x": 0, "y": 0}], "truthy": false},
                {"name": "test_dist_2", "fn": "dist", "args": [{"x": 0, "y": 0}, {"x": 0, "y": 1}], "expect": {"trap": "unreachable"}}
            ])
        );
        assert_eq!(frame["fns"].as_array().unwrap().len(), 1);
        assert_eq!(parsed.positions.entries()["/types/0"].line, 5);
        assert_eq!(parsed.positions.entries()["/tests/2"].line, 20);
    }

    #[test]
    fn python_test_records_refuse_duplicate_or_misordered_arguments() {
        let head = "class P:\n    x: int\n    y: int\n\ndef f(p: P) -> int:\n    return p.x\n\n";
        for (call, offending, message) in [
            ("P(x=1, x=2, y=3)", "x=2", "field `x` more than once"),
            ("P(1, 2, x=3)", "x=3", "field `x` more than once"),
            ("P(x=1, 2)", "2", "positional arguments must precede"),
        ] {
            let line = format!("    assert f({call}) == 1");
            let error = refused(&format!("{head}def test_f():\n{line}\n"));
            assert!(error.message.contains(message), "{call}: {}", error.message);
            assert_eq!(error.at.line, head.lines().count() + 2, "{call}");
            assert_eq!(error.at.column, line.find(offending).unwrap() + 1, "{call}");
        }
    }

    #[test]
    fn python_test_record_arguments_preserve_valid_order_and_nesting() {
        let head = "class P:\n    x: int\n    y: int\n\nclass Box:\n    p: P\n\ndef f(p: P) -> int:\n    return p.x\n\ndef g(b: Box) -> int:\n    return b.p.x\n\n";
        for call in ["P(1, 2)", "P(y=2, x=1)", "P(1, y=2)", "P(1, y=2,)"] {
            let frame = parse(&format!("{head}def test_f():\n    assert f({call}) == 1\n"))
                .unwrap()
                .frame;
            assert_eq!(
                frame["tests"][0]["args"],
                json!([{"x": 1, "y": 2}]),
                "{call}"
            );
        }
        for call in ["Box(p=P(1, y=2))", "Box(P(y=2, x=1))"] {
            let frame = parse(&format!("{head}def test_g():\n    assert g({call}) == 1\n"))
                .unwrap()
                .frame;
            assert_eq!(
                frame["tests"][0]["args"],
                json!([{"p": {"x": 1, "y": 2}}]),
                "{call}"
            );
        }
    }

    #[test]
    fn python_tests_refuse_what_they_cannot_state() {
        let head = "class P:\n    x: int\n\ndef f(a: int) -> int:\n    return a\n\n";
        for (test, message) in [
            ("def test_f():\n    x = 1\n", "a test function holds"),
            ("def test_f():\n    assert f(g(1)) == 1\n", "neither"),
            (
                "def test_f():\n    assert f(P(1, 2)) == 1\n",
                "has 1 fields",
            ),
            ("def test_f():\n    assert f(P(z=1)) == 1\n", "no field `z`"),
            (
                "def test_f():\n    with open(1):\n        f(1)\n",
                "pytest.raises",
            ),
            (
                "def test_f():\n    \"\"\"Nothing.\"\"\"\n",
                "holds no assertion",
            ),
        ] {
            let error = refused(&format!("{head}{test}"));
            assert!(error.message.contains(message), "{test}: {}", error.message);
        }
        // A test helper with parameters is an ordinary function.
        let frame = parse(&format!(
            "{head}def test_helper(a: int) -> int:\n    return a\n"
        ))
        .unwrap()
        .frame;
        assert_eq!(frame["fns"][1]["fn"], "test_helper");
        assert!(frame.get("tests").is_none());
    }

    #[test]
    fn statements_and_precedence_match_the_structured_tree() {
        let got = body(
            "fn f(xs: Vec<i64>, k: i64) -> i64 {\n  var s: i64 = 0\n  for x in xs { s = s + x * k }\n  return s if s > 0 else -s\n}",
        );
        assert_eq!(
            got,
            json!([
                ["var", "s", "i64", 0],
                [
                    "for",
                    "x",
                    "xs",
                    [["set", "s", ["add", "s", ["mul", "x", "k"]]]]
                ],
                ["return", ["if", ["gt", "s", 0], "s", ["neg", "s"]]]
            ])
        );
    }

    #[test]
    fn try_marks_failing_operations_and_calls() {
        let got = body(
            "fn f(a: i64, xs: Vec<i64>) -> Result<i64,ArithmeticError> { return ok(try(a * g(xs[0]) + to(i64, len(xs)))) }",
        );
        assert_eq!(
            got,
            json!([[
                "ok",
                [
                    "add?",
                    ["mul?", "a", ["tcall?", "g", ["get?", "xs", 0]]],
                    ["to?", "i64", ["len", "xs"]]
                ]
            ]])
        );
        let case = body("fn f(a: i64) -> Result<i64,E> { return ok(try(a + 1, Big)) }");
        assert_eq!(case, json!([["ok", ["add?Big", "a", 1]]]));
    }

    #[test]
    fn literals_cover_the_64_bit_edges() {
        let got = body(
            "fn f() -> i64 { let a = -9223372036854775808\n let b = 18446744073709551615u64\n let c = -5i32\n return 0 }",
        );
        assert_eq!(got[0][2], json!(i64::MIN));
        assert_eq!(got[1][2], json!({"type": "u64", "value": u64::MAX}));
        assert_eq!(got[2][2], json!({"type": "i32", "value": -5}));
        assert!(
            refused("fn f() -> i64 { return -9223372036854775809 }")
                .message
                .contains("below")
        );
        assert!(
            refused("fn f() -> i64 { return 18446744073709551616 }")
                .message
                .contains("beyond")
        );
        assert!(
            refused("fn f() -> u64 { return -1u64 }")
                .message
                .contains("unsigned")
        );
    }

    #[test]
    fn a_conditional_value_continues_only_on_its_line() {
        let got =
            body("fn f(c: bool) -> i64 {\n var x: i64 = 1\n x = 2\n if c { x = 3 }\n return x\n}");
        assert_eq!(got[2][0], json!("if"));
        assert_eq!(got[1], json!(["set", "x", 2]));
    }

    #[test]
    #[allow(clippy::too_many_lines)] // one row per refused construct
    fn foreign_and_ambiguous_syntax_is_refused_with_its_position() {
        for (text, needle, line, column) in [
            (
                "fn f(a: i64) -> i64 {\n  return a // 2\n}",
                "`//` is not an operator",
                2,
                12,
            ),
            ("fn f(a: i64) -> i64 {\n  a++\n}", "increment", 2, 4),
            (
                "fn f(a: i64) -> i64 { return a.abs() }",
                "methods are not supported",
                1,
                32,
            ),
            (
                "fn f(a: i64) -> i64 {\n  for i in 0..=a { }\n  return a\n}",
                "half-open",
                2,
                15,
            ),
            (
                "fn f(a: i64) -> i64 { if a > 0: return a }",
                "blocks are written",
                1,
                31,
            ),
            (
                "fn f(a: i64) -> i64 { return a < 1 < 2 }",
                "do not chain",
                1,
                36,
            ),
            (
                "fn f(a: i64) -> i64 { const x = a\n return x }",
                "write let",
                1,
                23,
            ),
            (
                "fn f(a: i64) -> i64 { match a { } }",
                "`match` is not supported",
                1,
                23,
            ),
            (
                "fn f(a: i64) -> i64 { return 1.5 }",
                "floating-point",
                1,
                30,
            ),
            ("fn f(a: i64) -> i64 { return a & a }", "bitwise", 1, 32),
            ("fn f(a: bool) -> bool { return a ^ a }", "bitwise", 1, 34),
            (
                "fn f(a: i64) -> Result<i64,E> { return ok(g(a)?) }",
                "try(e)",
                1,
                47,
            ),
            (
                "fn f(a: i64) -> i64 { return True }",
                "true or false",
                1,
                30,
            ),
            (
                "fn f(a: i64) -> i64 { let x: = a\n return x }",
                "expected a type",
                1,
                30,
            ),
            ("def f(a):\n  return a", "annotate the parameter", 1, 8),
            ("function f(a) {}", "fn name", 1, 1),
            ("fn f(a: i64) -> i64 { elif a { } }", "else if", 1, 23),
            ("fn len(a: i64) -> i64 { return a }", "built-in", 1, 1),
            (
                "fn f(a: i64) -> i64 { f(a)\n return a }",
                "does nothing here",
                1,
                23,
            ),
            ("fn f(a: i64) -> i64 { return }", "needs a value", 1, 23),
            (
                "fn f(a: i64) -> i64 { return 0x10 }",
                "unsupported number",
                1,
                30,
            ),
            (
                "fn f(a: i64) -> i64 { let x = ok(a)\n return x }",
                "return ok(e)",
                1,
                31,
            ),
            ("fn f(a: i64) -> i64 {\n  return a\n", "never closed", 1, 21),
        ] {
            let error = refused(text);
            assert!(error.message.contains(needle), "{text}: {}", error.message);
            assert_eq!(
                (error.at.line, error.at.column),
                (line, column),
                "{text}: {}",
                error.message
            );
        }
    }

    #[test]
    fn positions_map_body_pointers_to_lines() {
        let parsed = parse("fn f(xs: Vec<i64>) -> i64 {\n  var s: i64 = 0\n  for x in xs {\n    s = s + x\n  }\n  return s\n}").unwrap();
        let at = |pointer: &str| {
            let pos = parsed.positions.locate(pointer).unwrap();
            (pos.line, pos.column)
        };
        assert_eq!(at("/fns/0/body/1/3/0"), (4, 5));
        assert_eq!(at("/fns/0/body/1/3/0/2"), (4, 11));
        assert_eq!(at("/fns/0/body/2"), (6, 3));
        assert_eq!(at("/fns/0/body/1/3/0/2/9/9"), (4, 11));
    }
}

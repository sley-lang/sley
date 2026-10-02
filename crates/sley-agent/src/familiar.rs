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
    let tokens = tokenize(text)?;
    let mut parser = Parser { toks: tokens, k: 0 };
    let functions = parser.program()?;
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
    Ok(Parsed {
        frame: json!({"af1": 1, "afx": 1, "fns": fns}),
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
const FOREIGN: [(&str, &str); 17] = [
    (
        "+=",
        "compound assignment is not supported: write x = x + e",
    ),
    (
        "-=",
        "compound assignment is not supported: write x = x - e",
    ),
    (
        "*=",
        "compound assignment is not supported: write x = x * e",
    ),
    (
        "/=",
        "compound assignment is not supported: write x = x / e",
    ),
    (
        "%=",
        "compound assignment is not supported: write x = x % e",
    ),
    ("++", "increment is not supported: write x = x + 1"),
    ("**", "there is no power operator: multiply in a loop"),
    ("&&", "write and"),
    ("||", "write or"),
    ("..", "ranges are not supported: count with var and while"),
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
const OPERATORS: [&str; 22] = [
    "->", "==", "!=", "<=", ">=", "+", "-", "*", "/", "%", "<", ">", "=", "(", ")", "{", "}", "[",
    "]", ",", ".", ":",
];

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
                return refuse(pos, "Sley has no floating-point numbers");
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
        if let Some((spelled, message)) = FOREIGN.iter().find(|(s, _)| rest.starts_with(*s)) {
            return refuse(pos, format!("`{spelled}`: {message}"));
        }
        if rest.starts_with('!') && !rest.starts_with("!=") {
            return refuse(pos, "`!`: write not");
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

    fn is_name(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Atom(Value::String(name)) => Some(name),
            _ => None,
        }
    }
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
        if tok.kind == Kind::Eof {
            "end of input".into()
        } else {
            format!("`{}`", tok.text)
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
        let body = self.block()?;
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
                    return refuse(
                        self.peek().pos,
                        "`let mut`: a variable you assign is written var x: T = e",
                    );
                }
                let (name, name_pos) = self.ident("a variable name")?;
                if self.at_op(":") {
                    return refuse(
                        self.peek().pos,
                        "let takes no type: write var x: T = e, or type the literal (0u64)",
                    );
                }
                self.expect_op("=")?;
                let value = self.expr()?;
                Ok(Node::list(
                    pos,
                    vec![
                        keyword("let"),
                        Node::atom(name_pos, Value::String(name)),
                        value,
                    ],
                ))
            }
            Some("var") => {
                self.bump();
                let (name, name_pos) = self.ident("a variable name")?;
                if !self.at_op(":") {
                    return refuse(self.peek().pos, "var states its type: var x: T = e");
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
            Some("for") => {
                self.bump();
                let (name, name_pos) = self.ident("a loop variable")?;
                self.expect_word("in")?;
                let list = self.expr()?;
                let body = self.block()?;
                Ok(Node::list(
                    pos,
                    vec![
                        keyword("for"),
                        Node::atom(name_pos, Value::String(name)),
                        list,
                        body,
                    ],
                ))
            }
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
            Some("break" | "continue") => refuse(
                pos,
                format!(
                    "`{}` is not supported: loop with while and a condition, or return",
                    tok.text
                ),
            ),
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
            Some(name) if !KEYWORDS.contains(&name) && self.peek_at(1).text == ":" => {
                refuse(pos, "declare a variable with var x: T = e")
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
            value = Node::op(pos, "or", vec![value, right]);
        }
        Ok(value)
    }

    fn and_(&mut self) -> P<Node> {
        let mut value = self.not_()?;
        while self.at_word("and") {
            let pos = self.bump().pos;
            let right = self.not_()?;
            value = Node::op(pos, "and", vec![value, right]);
        }
        Ok(value)
    }

    fn not_(&mut self) -> P<Node> {
        if self.at_word("not") {
            let pos = self.bump().pos;
            let operand = self.not_()?;
            return Ok(Node::op(pos, "not", vec![operand]));
        }
        self.compare()
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
            let op = match self.peek() {
                tok if tok.kind == Kind::Op && tok.text == "*" => "mul",
                tok if tok.kind == Kind::Op && tok.text == "/" => "div",
                tok if tok.kind == Kind::Op && tok.text == "%" => "rem",
                _ => return Ok(value),
            };
            let pos = self.bump().pos;
            let right = self.unary()?;
            value = Node::op(pos, op, vec![value, right]);
        }
    }

    fn unary(&mut self) -> P<Node> {
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
                let index = self.expr()?;
                self.expect_op("]")?;
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
            "True" | "False" => return refuse(tok.pos, "write true or false"),
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
        let mut args = Vec::new();
        while !self.at_op(")") {
            args.push(self.expr()?);
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

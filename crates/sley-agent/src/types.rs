//! The AF1/AV1 type shorthand: a closed table of type constructors.
//!
//! A type is written either as a shorthand string drawn from a fixed table
//! (`i64`, `bool`, `Result<i64,CalcError>`, `Vec<u8>`, `(i64, bool)`) or as
//! structured JSON (`{"Result": ["i64", "CalcError"]}`). The table has no
//! operators and nothing is evaluated: every constructor maps one-to-one onto
//! an SSMC1 `TypeExpr` variant, and names resolve to `TypeDef` entities.

use serde_json::Value;
use sley_id::EntityId;
use sley_ssmc::{BuiltinFailureKind, FunctionType, IntegerWidth, NamedType, TypeExpr};

use crate::error::{Result, frame};
use crate::names::Names;

/// Renders a type in shorthand.
#[must_use]
pub fn render(ty: &TypeExpr, names: &Names) -> String {
    match ty {
        TypeExpr::Unit => "unit".into(),
        TypeExpr::Bool => "bool".into(),
        TypeExpr::SInt(width) => format!("i{}", width.bits()),
        TypeExpr::UInt(width) => format!("u{}", width.bits()),
        TypeExpr::F32 => "f32".into(),
        TypeExpr::F64 => "f64".into(),
        TypeExpr::Bytes => "bytes".into(),
        TypeExpr::Text => "text".into(),
        TypeExpr::Tuple(items) => format!("({})", list(items, names)),
        TypeExpr::Named(instance) => {
            if instance.arguments.is_empty() {
                names.name(&instance.definition)
            } else {
                format!(
                    "{}<{}>",
                    names.name(&instance.definition),
                    list(&instance.arguments, names)
                )
            }
        }
        TypeExpr::Vector(item) => format!("Vec<{}>", render(item, names)),
        TypeExpr::OrderedMap { key, value } => {
            format!("Map<{},{}>", render(key, names), render(value, names))
        }
        TypeExpr::Option(item) => format!("Option<{}>", render(item, names)),
        TypeExpr::Result { ok, error } => {
            format!("Result<{},{}>", render(ok, names), render(error, names))
        }
        TypeExpr::FunctionRef(function) => format!(
            "fn({})->{}",
            list(&function.parameters, names),
            render(&function.result, names)
        ),
        TypeExpr::AdapterHandle(id) => format!("Adapter<{}>", names.name(id)),
        TypeExpr::CapabilityToken(id) => format!("Capability<{}>", names.name(id)),
        TypeExpr::LocalCell(item) => format!("Cell<{}>", render(item, names)),
        TypeExpr::TypeParameter(index) => format!("${index}"),
        TypeExpr::BuiltinFailure(kind) => failure_name(*kind).into(),
    }
}

fn list(items: &[TypeExpr], names: &Names) -> String {
    items
        .iter()
        .map(|item| render(item, names))
        .collect::<Vec<_>>()
        .join(",")
}

/// The shorthand name of a built-in failure kind.
#[must_use]
pub const fn failure_name(kind: BuiltinFailureKind) -> &'static str {
    match kind {
        BuiltinFailureKind::Arithmetic => "ArithmeticError",
        BuiltinFailureKind::Index => "IndexError",
        BuiltinFailureKind::DuplicateKey => "DuplicateKeyError",
        BuiltinFailureKind::ContractViolation => "ContractViolation",
        BuiltinFailureKind::Capability => "CapabilityFailure",
    }
}

fn failure_kind(name: &str) -> Option<BuiltinFailureKind> {
    Some(match name {
        "ArithmeticError" => BuiltinFailureKind::Arithmetic,
        "IndexError" => BuiltinFailureKind::Index,
        "DuplicateKeyError" => BuiltinFailureKind::DuplicateKey,
        "ContractViolation" => BuiltinFailureKind::ContractViolation,
        "CapabilityFailure" => BuiltinFailureKind::Capability,
        _ => return None,
    })
}

/// Resolves type-definition names while reading a type.
pub trait TypeNames {
    /// Returns the `TypeDef` a name denotes.
    fn type_definition(&self, name: &str) -> Option<EntityId>;
}

/// Reads a type from shorthand text or structured JSON.
///
/// # Errors
///
/// `AGENT_FRAME_INVALID` at `pointer` for an unknown constructor or name.
///
/// # Panics
///
/// Never: a one-key object always yields its one entry.
pub fn read(value: &Value, resolver: &dyn TypeNames, pointer: &str) -> Result<TypeExpr> {
    match value {
        Value::String(text) => {
            let mut reader = Reader {
                tokens: tokenize(text)
                    .ok_or_else(|| frame(pointer, format!("bad type `{text}`")))?,
                position: 0,
                resolver,
                pointer,
            };
            let ty = reader.ty()?;
            if reader.position != reader.tokens.len() {
                return Err(frame(pointer, format!("trailing text in type `{text}`")));
            }
            Ok(ty)
        }
        Value::Object(object) if object.len() == 1 => {
            let (constructor, argument) = object.iter().next().expect("one entry");
            let arguments: Vec<Value> = match argument {
                Value::Array(items) => items.clone(),
                other => vec![other.clone()],
            };
            let mut read_all = |count: Option<usize>| -> Result<Vec<TypeExpr>> {
                if count.is_some_and(|count| count != arguments.len()) {
                    return Err(frame(
                        pointer,
                        format!("`{constructor}` takes {} types", count.unwrap_or(0)),
                    ));
                }
                arguments
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        read(item, resolver, &format!("{pointer}/{constructor}/{index}"))
                    })
                    .collect()
            };
            constructed(constructor, &mut read_all, resolver, pointer)
        }
        _ => Err(frame(
            pointer,
            "a type is a shorthand string or a one-key object",
        )),
    }
}

fn constructed(
    constructor: &str,
    read_all: &mut dyn FnMut(Option<usize>) -> Result<Vec<TypeExpr>>,
    resolver: &dyn TypeNames,
    pointer: &str,
) -> Result<TypeExpr> {
    let one = |read_all: &mut dyn FnMut(Option<usize>) -> Result<Vec<TypeExpr>>| {
        read_all(Some(1)).map(|mut items| Box::new(items.remove(0)))
    };
    Ok(match constructor {
        "Result" => {
            let mut items = read_all(Some(2))?;
            let error = items.remove(1);
            TypeExpr::Result {
                ok: Box::new(items.remove(0)),
                error: Box::new(error),
            }
        }
        "Option" => TypeExpr::Option(one(read_all)?),
        "Vec" => TypeExpr::Vector(one(read_all)?),
        "Cell" => TypeExpr::LocalCell(one(read_all)?),
        "Map" => {
            let mut items = read_all(Some(2))?;
            let value = items.remove(1);
            TypeExpr::OrderedMap {
                key: Box::new(items.remove(0)),
                value: Box::new(value),
            }
        }
        "Tuple" => TypeExpr::Tuple(read_all(None)?),
        name => {
            let definition = resolver
                .type_definition(name)
                .ok_or_else(|| frame(pointer, format!("unknown type `{name}`")))?;
            TypeExpr::Named(NamedType {
                definition,
                arguments: read_all(None)?,
            })
        }
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Word(String),
    Open,
    Close,
    LeftParen,
    RightParen,
    Comma,
    Arrow,
}

fn tokenize(text: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' => {}
            '<' => tokens.push(Token::Open),
            '>' => tokens.push(Token::Close),
            '(' => tokens.push(Token::LeftParen),
            ')' => tokens.push(Token::RightParen),
            ',' => tokens.push(Token::Comma),
            '-' if chars.peek() == Some(&'>') => {
                chars.next();
                tokens.push(Token::Arrow);
            }
            c if c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '.' => {
                let mut word = String::from(c);
                while let Some(&next) = chars.peek() {
                    // A name may contain `-` (the name grammar), but `->` is
                    // the arrow of a function type.
                    let hyphen = next == '-' && {
                        let mut ahead = chars.clone();
                        ahead.next();
                        ahead.peek() != Some(&'>')
                    };
                    if next.is_ascii_alphanumeric() || next == '_' || next == '.' || hyphen {
                        word.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Word(word));
            }
            _ => return None,
        }
    }
    Some(tokens)
}

struct Reader<'a> {
    tokens: Vec<Token>,
    position: usize,
    resolver: &'a dyn TypeNames,
    pointer: &'a str,
}

impl Reader<'_> {
    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        self.position += 1;
        token
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn expect(&mut self, token: &Token) -> Result<()> {
        if self.next().as_ref() == Some(token) {
            Ok(())
        } else {
            Err(frame(self.pointer, "malformed type"))
        }
    }

    fn list(&mut self, close: &Token) -> Result<Vec<TypeExpr>> {
        let mut items = Vec::new();
        if self.peek() == Some(close) {
            self.next();
            return Ok(items);
        }
        loop {
            items.push(self.ty()?);
            match self.next() {
                Some(Token::Comma) => {}
                Some(ref token) if token == close => return Ok(items),
                _ => return Err(frame(self.pointer, "malformed type list")),
            }
        }
    }

    fn arguments(&mut self) -> Result<Vec<TypeExpr>> {
        if self.peek() == Some(&Token::Open) {
            self.next();
            self.list(&Token::Close)
        } else {
            Ok(Vec::new())
        }
    }

    fn ty(&mut self) -> Result<TypeExpr> {
        match self.next() {
            Some(Token::LeftParen) => Ok(TypeExpr::Tuple(self.list(&Token::RightParen)?)),
            Some(Token::Word(word)) => self.word(&word),
            _ => Err(frame(self.pointer, "malformed type")),
        }
    }

    fn exact(&mut self, word: &str, count: usize) -> Result<Vec<TypeExpr>> {
        let items = self.arguments()?;
        if items.len() == count {
            Ok(items)
        } else {
            Err(frame(
                self.pointer,
                format!("`{word}` takes {count} type(s)"),
            ))
        }
    }

    fn word(&mut self, word: &str) -> Result<TypeExpr> {
        if let Some(scalar) = scalar(word) {
            return Ok(scalar);
        }
        if let Some(kind) = failure_kind(word) {
            return Ok(TypeExpr::BuiltinFailure(kind));
        }
        if let Some(index) = word.strip_prefix('$').and_then(|n| n.parse().ok()) {
            return Ok(TypeExpr::TypeParameter(index));
        }
        Ok(match word {
            "Result" => {
                let mut items = self.exact(word, 2)?;
                let error = items.remove(1);
                TypeExpr::Result {
                    ok: Box::new(items.remove(0)),
                    error: Box::new(error),
                }
            }
            "Option" => TypeExpr::Option(Box::new(self.exact(word, 1)?.remove(0))),
            "Vec" => TypeExpr::Vector(Box::new(self.exact(word, 1)?.remove(0))),
            "Cell" => TypeExpr::LocalCell(Box::new(self.exact(word, 1)?.remove(0))),
            "Map" => {
                let mut items = self.exact(word, 2)?;
                let value = items.remove(1);
                TypeExpr::OrderedMap {
                    key: Box::new(items.remove(0)),
                    value: Box::new(value),
                }
            }
            "Tuple" => TypeExpr::Tuple(self.arguments()?),
            "fn" => {
                self.expect(&Token::LeftParen)?;
                let parameters = self.list(&Token::RightParen)?;
                self.expect(&Token::Arrow)?;
                let result = self.ty()?;
                TypeExpr::FunctionRef(FunctionType {
                    parameters,
                    result: Box::new(result),
                    effects: Vec::new(),
                })
            }
            name => {
                let definition = self
                    .resolver
                    .type_definition(name)
                    .ok_or_else(|| frame(self.pointer, format!("unknown type `{name}`")))?;
                TypeExpr::Named(NamedType {
                    definition,
                    arguments: self.arguments()?,
                })
            }
        })
    }
}

fn scalar(word: &str) -> Option<TypeExpr> {
    let width = |digits: &str| -> Option<IntegerWidth> {
        let bits: u16 = digits.parse().ok()?;
        matches!(bits, 8 | 16 | 32 | 64 | 128).then(|| IntegerWidth::from_bits(bits))
    };
    Some(match word {
        "unit" => TypeExpr::Unit,
        "bool" => TypeExpr::Bool,
        "f32" => TypeExpr::F32,
        "f64" => TypeExpr::F64,
        "bytes" => TypeExpr::Bytes,
        "text" => TypeExpr::Text,
        _ => {
            if let Some(digits) = word.strip_prefix('i') {
                TypeExpr::SInt(width(digits)?)
            } else if let Some(digits) = word.strip_prefix('u') {
                TypeExpr::UInt(width(digits)?)
            } else {
                return None;
            }
        }
    })
}

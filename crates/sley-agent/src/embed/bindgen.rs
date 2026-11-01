//! Typed bindings generated from a compiled [`Module`]'s interface: Rust
//! types and a typed wrapper for a Rust host ([`rust`]), and TypeScript
//! types and a typed wrapper over [`Module::call_json`] for a JavaScript
//! host of a `wasm32-unknown-unknown` build ([`typescript`]).
//!
//! Both read only [`Module::signature`] and [`Module::types`], so the same
//! program gives the same text. A record becomes a struct (an interface in
//! TypeScript) with its fields in declaration order, and a variant type an
//! enum (a union) of its cases. Every function whose types all have an
//! [`EmbedValue`](super::EmbedValue) form becomes a method that converts its
//! arguments, calls the function and converts the result. A function with
//! another type (a float, bytes, a function reference) is listed in a
//! comment and not bound.
//!
//! A name that the target language reserves, or that the generated code
//! uses itself, gets a trailing `_` (`type_`, `Option_`); Rust keywords
//! that can be raw identifiers become `r#type` instead. Values still carry
//! the Sley names. The generated Rust starts with `Program::new`, which
//! refuses a module whose interface differs from the one the bindings were
//! generated from ([`Module::check_interface`]).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use super::convert::MAX_TUPLE;
use super::{EmbedType, Module, Signature, TypeDeclaration, TypeShape};

/// Rust keywords (2015 to 2024 editions, reserved ones included).
const RUST_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "try", "type",
    "typeof", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// Rust keywords that cannot be raw identifiers.
const RUST_NOT_RAW: &[&str] = &["crate", "self", "Self", "super", "_"];

/// Type names the generated Rust uses.
const RUST_TYPES: &[&str] = &[
    "Box",
    "CallOutcome",
    "EmbedError",
    "EmbedValue",
    "Err",
    "FromEmbed",
    "Module",
    "None",
    "Ok",
    "Option",
    "Program",
    "Result",
    "Some",
    "String",
    "ToEmbed",
    "Vec",
];

/// Methods of the generated Rust `Program` besides the program's own.
const RUST_METHODS: &[&str] = &["into_module", "module", "new"];

/// TypeScript reserved words and strict-mode restricted names.
const TS_RESERVED: &[&str] = &[
    "arguments",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "constructor",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// Type names TypeScript predefines or the generated TypeScript uses.
const TS_TYPES: &[&str] = &[
    "Array",
    "BigInt",
    "Boolean",
    "CallJson",
    "CallOutcome",
    "CallResult",
    "Date",
    "Error",
    "Function",
    "JSON",
    "Map",
    "Math",
    "Number",
    "Object",
    "Omit",
    "Partial",
    "Pick",
    "Program",
    "Promise",
    "Readonly",
    "ReadonlyArray",
    "Record",
    "Required",
    "SIGNATURES",
    "Set",
    "String",
    "Symbol",
    "any",
    "bigint",
    "boolean",
    "never",
    "null",
    "number",
    "object",
    "string",
    "symbol",
    "undefined",
    "unknown",
    "void",
];

/// The program's interface: the functions that can be bound, the ones that
/// cannot, and the declared types the bound ones use.
struct Interface<'m> {
    bound: Vec<&'m Signature>,
    unbound: Vec<&'m Signature>,
    types: Vec<&'m TypeDeclaration>,
}

impl<'m> Interface<'m> {
    /// `max_tuple`: the longest tuple the target can bind.
    fn of(module: &'m Module, max_tuple: usize) -> Self {
        let declarations = module.types();
        // A declared type binds when every member type does, which may
        // depend on other declared types: grow the set to a fixed point.
        let mut bindable: BTreeSet<&str> = BTreeSet::new();
        loop {
            let before = bindable.len();
            for declaration in declarations {
                let members: Vec<&EmbedType> = match &declaration.shape {
                    TypeShape::Record(fields) => fields.iter().map(|(_, ty)| ty).collect(),
                    TypeShape::Variant(cases) => {
                        cases.iter().filter_map(|(_, ty)| ty.as_ref()).collect()
                    }
                };
                if members.iter().all(|ty| binds(ty, &bindable, max_tuple)) {
                    bindable.insert(&declaration.name);
                }
            }
            if bindable.len() == before {
                break;
            }
        }
        let (mut bound, mut unbound) = (Vec::new(), Vec::new());
        for name in module.functions() {
            let Some(signature) = module.signature(&name) else {
                continue;
            };
            let ok = signature
                .parameters
                .iter()
                .map(|(_, ty)| ty)
                .chain([&signature.result])
                .all(|ty| binds(ty, &bindable, max_tuple));
            if ok {
                bound.push(signature);
            } else {
                unbound.push(signature);
            }
        }
        // The declared types the bound functions reach.
        let mut used = BTreeSet::new();
        let mut pending: Vec<&EmbedType> = bound
            .iter()
            .flat_map(|signature| {
                signature
                    .parameters
                    .iter()
                    .map(|(_, ty)| ty)
                    .chain([&signature.result])
            })
            .collect();
        while let Some(ty) = pending.pop() {
            match ty {
                EmbedType::Named(name) => {
                    if !used.insert(name.as_str()) {
                        continue;
                    }
                    if let Some(declaration) = declarations.iter().find(|d| d.name == *name) {
                        match &declaration.shape {
                            TypeShape::Record(fields) => {
                                pending.extend(fields.iter().map(|(_, ty)| ty))
                            }
                            TypeShape::Variant(cases) => {
                                pending.extend(cases.iter().filter_map(|(_, ty)| ty.as_ref()));
                            }
                        }
                    }
                }
                EmbedType::List(item) | EmbedType::Option(item) => pending.push(item),
                EmbedType::Tuple(items) => pending.extend(items),
                EmbedType::Result { ok, err } => pending.extend([&**ok, &**err]),
                EmbedType::Map { key, value } => pending.extend([&**key, &**value]),
                _ => {}
            }
        }
        let types = declarations
            .iter()
            .filter(|declaration| used.contains(declaration.name.as_str()))
            .collect();
        Self {
            bound,
            unbound,
            types,
        }
    }
}

/// Whether `ty` has a form in the target: no unsupported type, no tuple
/// longer than `max_tuple`, and only declared types in `bindable`.
fn binds(ty: &EmbedType, bindable: &BTreeSet<&str>, max_tuple: usize) -> bool {
    let inner = |ty: &EmbedType| binds(ty, bindable, max_tuple);
    match ty {
        EmbedType::Unit | EmbedType::Bool | EmbedType::Int { .. } | EmbedType::Str => true,
        EmbedType::List(item) | EmbedType::Option(item) => inner(item),
        EmbedType::Tuple(items) => {
            !items.is_empty() && items.len() <= max_tuple && items.iter().all(inner)
        }
        EmbedType::Result { ok, err } => inner(ok) && inner(err),
        EmbedType::Map { key, value } => inner(key) && inner(value),
        EmbedType::Named(name) => bindable.contains(name.as_str()),
        EmbedType::Unsupported(_) => false,
    }
}

/// A string literal both languages read the same way.
fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}

// --------------------------------------------------------------- Rust

/// A field, parameter or method name.
fn rust_value_name(name: &str) -> String {
    if RUST_NOT_RAW.contains(&name) {
        format!("{name}_")
    } else if RUST_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_owned()
    }
}

/// A type or enum case name.
fn rust_type_name(name: &str) -> String {
    if RUST_KEYWORDS.contains(&name) || RUST_NOT_RAW.contains(&name) || RUST_TYPES.contains(&name) {
        format!("{name}_")
    } else {
        name.to_owned()
    }
}

fn rust_type(ty: &EmbedType) -> String {
    match ty {
        EmbedType::Unit => "()".to_owned(),
        EmbedType::Bool => "bool".to_owned(),
        EmbedType::Int { signed, bits } => format!("{}{bits}", if *signed { 'i' } else { 'u' }),
        EmbedType::Str => "String".to_owned(),
        EmbedType::List(item) => format!("Vec<{}>", rust_type(item)),
        EmbedType::Tuple(items) => {
            let items: Vec<String> = items.iter().map(rust_type).collect();
            if items.len() == 1 {
                format!("({},)", items[0])
            } else {
                format!("({})", items.join(", "))
            }
        }
        EmbedType::Option(item) => format!("Option<{}>", rust_type(item)),
        EmbedType::Result { ok, err } => format!("Result<{}, {}>", rust_type(ok), rust_type(err)),
        EmbedType::Map { key, value } => format!(
            "std::collections::BTreeMap<{}, {}>",
            rust_type(key),
            rust_type(value)
        ),
        EmbedType::Named(name) | EmbedType::Unsupported(name) => rust_type_name(name),
    }
}

/// Rust bindings for `module`: one struct or enum per declared type the
/// bound functions use, with [`ToEmbed`](super::ToEmbed) and
/// [`FromEmbed`](super::FromEmbed), and a `Program` wrapper with one method
/// per bound function. The text is a module body: write it to a file and
/// `mod` or `include!` it. It needs `sley-agent` with the `familiar`
/// feature.
#[must_use]
pub fn rust(module: &Module) -> String {
    let interface = Interface::of(module, MAX_TUPLE);
    let mut out = String::from(
        "// Typed bindings for a Sley program, generated by sley_agent::embed::bindgen::rust.\n\
         // Do not edit: regenerate them when the program's interface changes.\n\
         // Program::new refuses a module whose interface differs from this one.\n\n\
         #[allow(unused_imports)]\n\
         use sley_agent::embed::{CallOutcome, EmbedError, EmbedValue, FromEmbed, Module, ToEmbed};\n",
    );
    for declaration in &interface.types {
        out.push('\n');
        rust_declaration(&mut out, declaration);
    }
    let _ = write!(
        out,
        "\n/// The program's functions with Rust types.\n\
         #[derive(Debug)]\n\
         pub struct Program {{\n    module: Module,\n}}\n\n\
         impl Program {{\n\
         \x20   /// The signatures these bindings were generated from.\n"
    );
    let signatures: Vec<String> = interface.bound.iter().map(ToString::to_string).collect();
    rust_strings(&mut out, "SIGNATURES", &signatures);
    out.push_str("\n    /// The types these bindings were generated from.\n");
    let types: Vec<String> = interface.types.iter().map(ToString::to_string).collect();
    rust_strings(&mut out, "TYPES", &types);
    out.push_str(
        "\n\
         \x20   /// Binds `module`.\n\
         \x20   ///\n\
         \x20   /// # Errors\n\
         \x20   ///\n\
         \x20   /// `AGENT_INPUT_INVALID` when the module's interface differs from the one\n\
         \x20   /// these bindings were generated from.\n\
         \x20   pub fn new(module: Module) -> Result<Self, EmbedError> {\n\
         \x20       module.check_interface(Self::SIGNATURES, Self::TYPES)?;\n\
         \x20       Ok(Self { module })\n\
         \x20   }\n\n\
         \x20   /// The bound module.\n\
         \x20   pub fn module(&mut self) -> &mut Module {\n\
         \x20       &mut self.module\n\
         \x20   }\n\n\
         \x20   /// The bound module, unbound.\n\
         \x20   #[must_use]\n\
         \x20   pub fn into_module(self) -> Module {\n\
         \x20       self.module\n\
         \x20   }\n",
    );
    for signature in &interface.bound {
        out.push('\n');
        rust_method(&mut out, signature);
    }
    out.push_str("}\n");
    for signature in &interface.unbound {
        let _ = writeln!(
            out,
            "\n// Not bound: `{signature}` has a type with no embed form."
        );
    }
    out
}

/// `pub const NAME: &'static [&'static str] = &[..];`, on one line when it
/// fits, as rustfmt lays it out.
fn rust_strings(out: &mut String, name: &str, items: &[String]) {
    let items: Vec<String> = items.iter().map(|item| quoted(item)).collect();
    let head = format!("    pub const {name}: &'static [&'static str] = &[");
    let flat = format!("{head}{}];", items.join(", "));
    if flat.len() <= 100 {
        let _ = writeln!(out, "{flat}");
    } else {
        let _ = writeln!(out, "{head}");
        for item in &items {
            let _ = writeln!(out, "        {item},");
        }
        out.push_str("    ];\n");
    }
}

fn rust_declaration(out: &mut String, declaration: &TypeDeclaration) {
    let name = rust_type_name(&declaration.name);
    let _ = writeln!(out, "/// `{declaration}`");
    out.push_str("#[allow(non_camel_case_types)]\n#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]\n");
    match &declaration.shape {
        TypeShape::Record(fields) => {
            let _ = writeln!(out, "pub struct {name} {{");
            for (field, ty) in fields {
                let _ = writeln!(
                    out,
                    "    pub {}: {},",
                    rust_value_name(field),
                    rust_type(ty)
                );
            }
            out.push_str("}\n\n");
            let _ = write!(
                out,
                "impl ToEmbed for {name} {{\n    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {{\n        Ok(EmbedValue::Record(vec!["
            );
            for (field, _) in fields {
                let _ = write!(
                    out,
                    "\n            ({}.to_owned(), self.{}.to_embed()?),",
                    quoted(field),
                    rust_value_name(field)
                );
            }
            if !fields.is_empty() {
                out.push_str("\n        ");
            }
            out.push_str("]))\n    }\n}\n\n");
            let _ = write!(
                out,
                "impl FromEmbed for {name} {{\n    fn from_embed(value: &EmbedValue) -> Option<Self> {{\n        \
                 if !matches!(value, EmbedValue::Record(fields) if fields.len() == {}) {{\n            return None;\n        }}\n        \
                 Some(Self {{",
                fields.len()
            );
            for (field, _) in fields {
                let _ = write!(
                    out,
                    "\n            {}: FromEmbed::from_embed(value.field({})?)?,",
                    rust_value_name(field),
                    quoted(field)
                );
            }
            if !fields.is_empty() {
                out.push_str("\n        ");
            }
            out.push_str("})\n    }\n}\n");
        }
        TypeShape::Variant(cases) => {
            let _ = writeln!(out, "pub enum {name} {{");
            for (case, payload) in cases {
                match payload {
                    Some(ty) => {
                        let _ = writeln!(out, "    {}({}),", rust_type_name(case), rust_type(ty));
                    }
                    None => {
                        let _ = writeln!(out, "    {},", rust_type_name(case));
                    }
                }
            }
            out.push_str("}\n\n");
            let _ = write!(
                out,
                "impl ToEmbed for {name} {{\n    fn to_embed(&self) -> Result<EmbedValue, EmbedError> {{\n"
            );
            if cases.is_empty() {
                out.push_str("        match *self {}\n");
            } else {
                out.push_str("        Ok(match self {\n");
                for (case, payload) in cases {
                    let rust_case = rust_type_name(case);
                    match payload {
                        Some(_) => {
                            let _ = writeln!(
                                out,
                                "            Self::{rust_case}(payload) => EmbedValue::Variant {{\n                \
                                 case: {}.to_owned(),\n                payload: Some(Box::new(payload.to_embed()?)),\n            }},",
                                quoted(case)
                            );
                        }
                        None => {
                            let _ = writeln!(
                                out,
                                "            Self::{rust_case} => EmbedValue::case({}),",
                                quoted(case)
                            );
                        }
                    }
                }
                out.push_str("        })\n");
            }
            out.push_str("    }\n}\n\n");
            let _ = write!(
                out,
                "impl FromEmbed for {name} {{\n    fn from_embed(value: &EmbedValue) -> Option<Self> {{\n        \
                 let EmbedValue::Variant {{ case, payload }} = value else {{\n            return None;\n        }};\n        \
                 match (case.as_str(), payload.as_deref()) {{\n"
            );
            for (case, payload) in cases {
                let rust_case = rust_type_name(case);
                match payload {
                    Some(_) => {
                        let _ = writeln!(
                            out,
                            "            ({}, Some(payload)) => Some(Self::{rust_case}(FromEmbed::from_embed(payload)?)),",
                            quoted(case)
                        );
                    }
                    None => {
                        let _ = writeln!(
                            out,
                            "            ({}, None) => Some(Self::{rust_case}),",
                            quoted(case)
                        );
                    }
                }
            }
            out.push_str("            _ => None,\n        }\n    }\n}\n");
        }
    }
}

/// How a method takes a parameter: integers, booleans, unit and options by
/// value, text as `&str`, a list as a slice, anything else by reference.
fn rust_parameter_type(ty: &EmbedType) -> String {
    match ty {
        EmbedType::Unit | EmbedType::Bool | EmbedType::Int { .. } | EmbedType::Option(_) => {
            rust_type(ty)
        }
        EmbedType::Str => "&str".to_owned(),
        EmbedType::List(item) => format!("&[{}]", rust_type(item)),
        _ => format!("&{}", rust_type(ty)),
    }
}

fn rust_method(out: &mut String, signature: &Signature) {
    let mut method = rust_value_name(&signature.name);
    if RUST_METHODS.contains(&method.as_str()) {
        method.push('_');
    }
    let parameters: Vec<(String, String)> = signature
        .parameters
        .iter()
        .map(|(name, ty)| {
            let mut name = rust_value_name(name);
            if name == "fuel_limit" {
                name.push('_');
            }
            (name, rust_parameter_type(ty))
        })
        .collect();
    let _ = write!(
        out,
        "    /// `{signature}`\n    ///\n    /// # Errors\n    ///\n    \
         /// As [`Module::call`]; `AGENT_INPUT_INVALID` for an integer outside `i64`.\n"
    );
    // An option is taken by value (`Option<&T>` would not convert).
    if signature
        .parameters
        .iter()
        .any(|(_, ty)| matches!(ty, EmbedType::Option(_)))
    {
        out.push_str("    #[allow(clippy::needless_pass_by_value)]\n");
    }
    let result = rust_type(&signature.result);
    let mut items: Vec<String> = vec!["&mut self".to_owned()];
    items.extend(parameters.iter().map(|(name, ty)| format!("{name}: {ty}")));
    items.push("fuel_limit: u64".to_owned());
    let tail = format!(") -> Result<CallOutcome<{result}>, EmbedError> {{");
    let flat = format!("    pub fn {method}({}{tail}", items.join(", "));
    // As rustfmt lays it out: on one line when it fits, else one parameter
    // per line.
    if flat.len() <= 100 {
        out.push_str(&flat);
    } else {
        let _ = write!(out, "    pub fn {method}(\n");
        for item in &items {
            let _ = writeln!(out, "        {item},");
        }
        let _ = write!(out, "    {tail}");
    }
    let args: Vec<String> = parameters
        .iter()
        .map(|(name, _)| format!("{name}.to_embed()?"))
        .collect();
    let _ = write!(
        out,
        "\n        let args = [{}];\n        self.module.call({}, &args, fuel_limit)?.typed()\n    }}\n",
        args.join(", "),
        quoted(&signature.name)
    );
}

// --------------------------------------------------------- TypeScript

fn ts_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// A property name: bare when it is an identifier, else quoted.
fn ts_property(name: &str) -> String {
    if ts_identifier(name) {
        name.to_owned()
    } else {
        quoted(name)
    }
}

/// A parameter or method name.
fn ts_value_name(name: &str) -> String {
    if TS_RESERVED.contains(&name) || name == "fuelLimit" || !ts_identifier(name) {
        format!(
            "{}_",
            name.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_', "_")
        )
    } else {
        name.to_owned()
    }
}

fn ts_type_name(name: &str) -> String {
    if TS_RESERVED.contains(&name) || TS_TYPES.contains(&name) {
        format!("{name}_")
    } else {
        name.to_owned()
    }
}

fn ts_type(ty: &EmbedType) -> String {
    match ty {
        EmbedType::Unit => "null".to_owned(),
        EmbedType::Bool => "boolean".to_owned(),
        EmbedType::Int { .. } => "number".to_owned(),
        EmbedType::Str => "string".to_owned(),
        EmbedType::List(item) => format!("Array<{}>", ts_type(item)),
        EmbedType::Tuple(items) => {
            format!(
                "[{}]",
                items.iter().map(ts_type).collect::<Vec<_>>().join(", ")
            )
        }
        EmbedType::Option(item) => format!("\"None\" | {{ Some: {} }}", ts_type(item)),
        EmbedType::Result { ok, err } => {
            format!("{{ Ok: {} }} | {{ Err: {} }}", ts_type(ok), ts_type(err))
        }
        EmbedType::Map { key, value } => {
            format!("Array<[{}, {}]>", ts_type(key), ts_type(value))
        }
        EmbedType::Named(name) | EmbedType::Unsupported(name) => ts_type_name(name),
    }
}

/// TypeScript bindings for `module`, for a JavaScript host of a
/// `wasm32-unknown-unknown` build that exports [`Module::call_json`]: one
/// interface or union per declared type the bound functions use, in the
/// JSON form `call_json` reads and writes, and a `Program` class over the
/// host's `call_json` with one method per bound function. Integers are
/// JSON numbers, which JavaScript holds exactly up to 2^53 - 1.
#[must_use]
pub fn typescript(module: &Module) -> String {
    let interface = Interface::of(module, usize::MAX);
    let mut out = String::from(
        "// Typed bindings for a Sley program, generated by sley_agent::embed::bindgen::typescript.\n\
         // Do not edit: regenerate them when the program's interface changes.\n\
         //\n\
         // Values travel in the JSON form of Module::call_json. Integers are JSON\n\
         // numbers, which JavaScript holds exactly up to 2^53 - 1.\n",
    );
    for declaration in &interface.types {
        let name = ts_type_name(&declaration.name);
        let _ = writeln!(out, "\n/** `{declaration}` */");
        match &declaration.shape {
            TypeShape::Record(fields) => {
                let _ = writeln!(out, "export interface {name} {{");
                for (field, ty) in fields {
                    let _ = writeln!(out, "  {}: {};", ts_property(field), ts_type(ty));
                }
                out.push_str("}\n");
            }
            TypeShape::Variant(cases) => {
                let members: Vec<String> = cases
                    .iter()
                    .map(|(case, payload)| match payload {
                        Some(ty) => format!("{{ {}: {} }}", ts_property(case), ts_type(ty)),
                        None => quoted(case),
                    })
                    .collect();
                let members = if members.is_empty() {
                    "never".to_owned()
                } else {
                    members.join(" | ")
                };
                let _ = writeln!(out, "export type {name} = {members};");
            }
        }
    }
    out.push_str(
        "\n/** How a call ended: its value, a trap's name, or the limit it reached. */\n\
         export type CallResult<T> = { value: T } | { trap: string } | { limit: string };\n\n\
         /** One call's result and exact resource use. */\n\
         export interface CallOutcome<T> {\n  result: CallResult<T>;\n  fuel_used: number;\n  instructions: number;\n}\n\n\
         /** The host's export of Module::call_json: a function's name, its arguments\n\
         \x20* as a JSON array and a fuel limit, to the JSON reply. */\n\
         export type CallJson = (name: string, args: string, fuelLimit: number) => string;\n\n\
         /** The signatures these bindings were generated from. */\n\
         export const SIGNATURES: readonly string[] = [",
    );
    for signature in &interface.bound {
        let _ = write!(out, "\n  {},", quoted(&signature.to_string()));
    }
    if !interface.bound.is_empty() {
        out.push('\n');
    }
    out.push_str(
        "];\n\n\
         /** The program's functions with TypeScript types, over the host's call_json. */\n\
         export class Program {\n  private readonly callJson: CallJson;\n\n  \
         constructor(callJson: CallJson) {\n    this.callJson = callJson;\n  }\n",
    );
    for signature in &interface.bound {
        let method = ts_value_name(&signature.name);
        let names: Vec<String> = signature
            .parameters
            .iter()
            .map(|(name, _)| ts_value_name(name))
            .collect();
        let parameters: Vec<String> = names
            .iter()
            .zip(&signature.parameters)
            .map(|(name, (_, ty))| format!("{name}: {}", ts_type(ty)))
            .chain(["fuelLimit: number".to_owned()])
            .collect();
        let result = ts_type(&signature.result);
        let _ = write!(
            out,
            "\n  /** `{signature}` */\n  {method}({}): CallOutcome<{result}> {{\n    \
             const reply = this.callJson({}, JSON.stringify([{}]), fuelLimit);\n    \
             return JSON.parse(reply) as CallOutcome<{result}>;\n  }}\n",
            parameters.join(", "),
            quoted(&signature.name),
            names.join(", "),
        );
    }
    out.push_str("}\n");
    for signature in &interface.unbound {
        let _ = writeln!(
            out,
            "\n// Not bound: `{signature}` has a type with no embed form."
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int() -> EmbedType {
        EmbedType::Int {
            signed: true,
            bits: 64,
        }
    }

    #[test]
    fn reserved_names_get_a_suffix_or_a_raw_prefix() {
        assert_eq!(rust_value_name("type"), "r#type");
        assert_eq!(rust_value_name("self"), "self_");
        assert_eq!(rust_value_name("total"), "total");
        assert_eq!(rust_type_name("Option"), "Option_");
        assert_eq!(rust_type_name("Program"), "Program_");
        assert_eq!(rust_type_name("Self"), "Self_");
        assert_eq!(rust_type_name("Point"), "Point");
        assert_eq!(ts_value_name("delete"), "delete_");
        assert_eq!(ts_value_name("fuelLimit"), "fuelLimit_");
        assert_eq!(ts_value_name("xs"), "xs");
        assert_eq!(ts_type_name("Map"), "Map_");
        assert_eq!(ts_type_name("CallResult"), "CallResult_");
        assert_eq!(ts_property("kind"), "kind");
        assert_eq!(ts_property("two words"), "\"two words\"");
    }

    #[test]
    fn every_type_has_a_rust_and_a_typescript_spelling() {
        let cases = [
            (EmbedType::Unit, "()", "null"),
            (
                EmbedType::Option(Box::new(int())),
                "Option<i64>",
                "\"None\" | { Some: number }",
            ),
            (
                EmbedType::Map {
                    key: Box::new(EmbedType::Str),
                    value: Box::new(int()),
                },
                "std::collections::BTreeMap<String, i64>",
                "Array<[string, number]>",
            ),
            (EmbedType::Tuple(vec![int()]), "(i64,)", "[number]"),
            (
                EmbedType::Tuple(vec![int(), EmbedType::Bool]),
                "(i64, bool)",
                "[number, boolean]",
            ),
            (
                EmbedType::Result {
                    ok: Box::new(EmbedType::List(Box::new(int()))),
                    err: Box::new(EmbedType::Named("Error".into())),
                },
                "Result<Vec<i64>, Error>",
                "{ Ok: Array<number> } | { Err: Error_ }",
            ),
        ];
        for (ty, rust, typescript) in cases {
            assert_eq!(rust_type(&ty), rust, "{ty}");
            assert_eq!(ts_type(&ty), typescript, "{ty}");
        }
        assert_eq!(rust_parameter_type(&EmbedType::Str), "&str");
        assert_eq!(
            rust_parameter_type(&EmbedType::List(Box::new(int()))),
            "&[i64]"
        );
        assert_eq!(rust_parameter_type(&EmbedType::Named("P".into())), "&P");
        assert_eq!(rust_parameter_type(&int()), "i64");
    }

    #[test]
    fn unsupported_types_and_long_tuples_do_not_bind() {
        let declared: BTreeSet<&str> = ["P"].into_iter().collect();
        assert!(binds(&EmbedType::Named("P".into()), &declared, MAX_TUPLE));
        assert!(!binds(&EmbedType::Named("Q".into()), &declared, MAX_TUPLE));
        assert!(!binds(
            &EmbedType::Unsupported("f64".into()),
            &declared,
            MAX_TUPLE
        ));
        let long = EmbedType::Tuple(vec![int(); MAX_TUPLE + 1]);
        assert!(!binds(&long, &declared, MAX_TUPLE));
        assert!(binds(&long, &declared, usize::MAX));
        let nested = EmbedType::Option(Box::new(EmbedType::Unsupported("bytes".into())));
        assert!(!binds(&nested, &declared, usize::MAX));
    }
}

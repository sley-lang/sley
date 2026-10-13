//! The C0 seed assembler: a structured builder that emits typed SSMC1
//! graphs.
//!
//! This is seed-side construction code, not a Sley source language. It
//! never parses text. Rust code calls the builder, and the builder emits the
//! exact Function, Parameter, Block, Operation, Constant, `TypeDefinition`,
//! and `AdapterImport` entities that become canonical program state. After
//! construction the graph is the source of truth; this module is only the
//! seed record that produced it.
//!
//! Mutable variables are SSA values threaded through block parameters, so
//! the emitted graphs use no local cells. Every join and loop header passes
//! the mutable variables in scope as block arguments, the CFG validation
//! profile's canonical phi mechanism.

// Builder methods take expressions by value so authors compose them inline,
// and authoring mistakes panic at construction time by design (the seed is
// run once, by tests and the staging tool, never on untrusted input).
#![allow(clippy::needless_pass_by_value, clippy::missing_panics_doc)]

use std::collections::HashMap;

use sley_id::EntityId;
use sley_ssmc::{
    AdapterImport, Block, BranchTerminator, BuiltinCase, BuiltinFailureKind, CaseKey,
    CondBranchTerminator, ConstData, ConstValue, ConstantDefinition, FunctionGraph,
    FunctionRefValue, Immediate, IntegerWidth, MemberId, NamedType, Opcode, Operation,
    OperationResultRef, Parameter, ParameterRole, Reachability, RecordField, ReturnTerminator,
    SwitchArgument, SwitchCase, SwitchEdge, TargetEdge, Terminator, TrapCode, TrapTerminator,
    TypeDefForm, TypeDefinition, TypeExpr, ValueRef, VariantCase, VariantImmediate,
    VariantSwitchTerminator, Visibility,
};

/// A seed type.
pub type Ty = TypeExpr;

thread_local! {
    static FAULTS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Whether the named fault seed is active (REWEAVE §15 fault seeding).
///
/// Fault seeds construct deliberately defective toolchains so the
/// differential corpus can prove it detects them. They are build-time
/// switches of the seed assembler only: canonical S is never built with
/// one active.
#[must_use]
pub fn fault(name: &str) -> bool {
    FAULTS.with(|faults| faults.borrow().iter().any(|f| f == name))
}

/// Runs `body` (module construction and `finish`) with fault seeds active.
pub fn with_faults<T>(names: &[&str], body: impl FnOnce() -> T) -> T {
    FAULTS.with(|faults| *faults.borrow_mut() = names.iter().map(|n| (*n).to_owned()).collect());
    let out = body();
    FAULTS.with(|faults| faults.borrow_mut().clear());
    out
}

/// Type constructors.
pub mod t {
    use super::{BuiltinFailureKind, IntegerWidth, Ty, TypeExpr};

    /// `Unit`.
    #[must_use]
    pub fn unit() -> Ty {
        TypeExpr::Unit
    }
    /// `Bool`.
    #[must_use]
    pub fn bool() -> Ty {
        TypeExpr::Bool
    }
    /// `UInt(8)`.
    #[must_use]
    pub fn u8() -> Ty {
        TypeExpr::UInt(IntegerWidth::from_bits(8))
    }
    /// `UInt(16)`.
    #[must_use]
    pub fn u16() -> Ty {
        TypeExpr::UInt(IntegerWidth::from_bits(16))
    }
    /// `UInt(32)`.
    #[must_use]
    pub fn u32() -> Ty {
        TypeExpr::UInt(IntegerWidth::from_bits(32))
    }
    /// `UInt(64)`.
    #[must_use]
    pub fn u64() -> Ty {
        TypeExpr::UInt(IntegerWidth::from_bits(64))
    }
    /// `UInt(128)`.
    #[must_use]
    pub fn u128() -> Ty {
        TypeExpr::UInt(IntegerWidth::from_bits(128))
    }
    /// `SInt(64)`.
    #[must_use]
    pub fn i64() -> Ty {
        TypeExpr::SInt(IntegerWidth::from_bits(64))
    }
    /// `SInt(128)`.
    #[must_use]
    pub fn i128() -> Ty {
        TypeExpr::SInt(IntegerWidth::from_bits(128))
    }
    /// `Bytes`.
    #[must_use]
    pub fn bytes() -> Ty {
        TypeExpr::Bytes
    }
    /// `Text`.
    #[must_use]
    pub fn text() -> Ty {
        TypeExpr::Text
    }
    /// `Vector<T>`.
    #[must_use]
    pub fn vec(element: Ty) -> Ty {
        TypeExpr::Vector(Box::new(element))
    }
    /// `Option<T>`.
    #[must_use]
    pub fn opt(element: Ty) -> Ty {
        TypeExpr::Option(Box::new(element))
    }
    /// `Result<T, E>`.
    #[must_use]
    pub fn res(ok: Ty, error: Ty) -> Ty {
        TypeExpr::Result {
            ok: Box::new(ok),
            error: Box::new(error),
        }
    }
    /// `OrderedMap<K, V>`.
    #[must_use]
    pub fn map(key: Ty, value: Ty) -> Ty {
        TypeExpr::OrderedMap {
            key: Box::new(key),
            value: Box::new(value),
        }
    }
    /// `Tuple(...)`.
    #[must_use]
    pub fn tuple(items: Vec<Ty>) -> Ty {
        TypeExpr::Tuple(items)
    }
    /// `BuiltinFailure(kind)`.
    #[must_use]
    pub fn failure(kind: BuiltinFailureKind) -> Ty {
        TypeExpr::BuiltinFailure(kind)
    }
}

/// Primitive operations. Arithmetic, collection, and bridge operations
/// whose native result is a `Result` or `Option` unwrap automatically and
/// trap on failure; the `Try*` and `*Opt` forms return the raw value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Prim {
    /// Checked addition (traps on overflow).
    Add,
    /// Checked subtraction.
    Sub,
    /// Checked multiplication.
    Mul,
    /// Checked division.
    Div,
    /// Checked remainder.
    Rem,
    /// Checked left shift (amount `UInt(32)`).
    Shl,
    /// Checked right shift (amount `UInt(32)`).
    Shr,
    /// Checked signed negation.
    Neg,
    /// Raw checked addition.
    TryAdd,
    /// Raw checked subtraction.
    TrySub,
    /// Raw checked multiplication.
    TryMul,
    /// Raw checked shift left.
    TryShl,
    /// Equality.
    Eq,
    /// Inequality.
    Ne,
    /// Less than.
    Lt,
    /// Less or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater or equal.
    Ge,
    /// Boolean not.
    Not,
    /// Boolean and (both sides evaluated).
    And,
    /// Boolean or (both sides evaluated).
    Or,
    /// Vector length.
    VLen,
    /// Vector element (traps out of range).
    VGet,
    /// Vector element as `Option`.
    VGetOpt,
    /// Persistent vector update (traps out of range).
    VSet,
    /// Map lookup as `Option`.
    MGetOpt,
    /// Map lookup (traps when absent).
    MGet,
    /// Map membership.
    MHas,
    /// Map insertion.
    MIns,
    /// Map removal.
    MDel,
    /// Canonical value hash.
    VHash,
    /// `Bytes -> Vector<UInt(8)>` bridge.
    B2V,
    /// `Vector<UInt(8)> -> Bytes` bridge.
    V2B,
    /// `vector-push` bridge.
    Push,
    /// Raw BLAKE3-256 over a complete preimage (`RHW1`, at most 1 MiB).
    RawBlake3,
    /// Raw BLAKE3-256 with the package ceiling (`RHW2`).
    RawBlake3Large,
    /// Raw SHA-256 (`RSH2`).
    Sha256,
    /// Concatenation of a `Vector<Bytes>` (`BCT1`).
    Concat,
    /// `Bytes` slice `(bytes, start, length)` (`BSL1`; traps out of range).
    Slice,
    /// `Bytes` length (`BLN1`).
    BytesLen,
    /// Option/Result unwrap (traps on `None`/`Err`).
    Unwrap,
    /// `Option` is `Some`.
    IsSome,
    /// `Result` is `Ok`.
    IsOk,
}

/// A seed expression.
#[derive(Clone, Debug)]
pub enum E {
    /// A variable or binding.
    Var(String),
    /// A typed literal.
    Lit(ConstValue),
    /// A primitive operation.
    Prim(Prim, Vec<E>),
    /// A direct call of a module function.
    Call(String, Vec<E>),
    /// A record field.
    Field(Box<E>, String),
    /// A record construction (type name, fields in definition order).
    Record(String, Vec<E>),
    /// A variant construction (type name, case name, payload).
    Variant(String, String, Option<Box<E>>),
    /// `variant_get`: the payload of one case as `Option`.
    VariantGet(Box<E>, String, String),
    /// Whether a value is one variant case.
    IsCase(Box<E>, String, String),
    /// `option_none` of an element type.
    NoneOf(Ty),
    /// `option_some`.
    Some(Box<E>),
    /// `result_ok` with the error type.
    Ok(Box<E>, Ty),
    /// `result_err` with the success type.
    Err(Box<E>, Ty),
    /// `vector_new` with an element type.
    VecNew(Ty, Vec<E>),
    /// `map_new` (unwrapped; traps on duplicate keys).
    MapNew(Ty, Ty, Vec<(E, E)>),
    /// `tuple_new`.
    Tuple(Vec<E>),
    /// `tuple_get`.
    TupleGet(Box<E>, u32),
    /// A conditional expression; only the chosen side runs.
    Select(Box<E>, Box<E>, Box<E>),
    /// `expr?`: the `Ok` payload, or an early return of the `Err` payload
    /// from the enclosing function (whose error type must match).
    Try(Box<E>),
}

/// Variable reference.
#[must_use]
pub fn v(name: &str) -> E {
    E::Var(name.to_owned())
}

fn uint_lit(value: u128, ty: Ty) -> E {
    E::Lit(ConstValue {
        value_type: ty,
        data: ConstData::UInt(value),
    })
}

/// `UInt(8)` literal.
#[must_use]
pub fn u8c(value: u8) -> E {
    uint_lit(u128::from(value), t::u8())
}
/// `UInt(16)` literal.
#[must_use]
pub fn u16c(value: u16) -> E {
    uint_lit(u128::from(value), t::u16())
}
/// `UInt(32)` literal.
#[must_use]
pub fn u32c(value: u32) -> E {
    uint_lit(u128::from(value), t::u32())
}
/// `UInt(64)` literal.
#[must_use]
pub fn u64c(value: u64) -> E {
    uint_lit(u128::from(value), t::u64())
}
/// `UInt(128)` literal.
#[must_use]
pub fn u128c(value: u128) -> E {
    uint_lit(value, t::u128())
}
/// `SInt(64)` literal.
#[must_use]
pub fn i64c(value: i64) -> E {
    E::Lit(ConstValue {
        value_type: t::i64(),
        data: ConstData::SInt(i128::from(value)),
    })
}
/// `Bool` literal.
#[must_use]
pub fn boolc(value: bool) -> E {
    E::Lit(ConstValue {
        value_type: t::bool(),
        data: ConstData::Bool(value),
    })
}
/// `Unit` literal.
#[must_use]
pub fn unitc() -> E {
    E::Lit(ConstValue {
        value_type: t::unit(),
        data: ConstData::Unit,
    })
}
/// `Bytes` literal.
#[must_use]
pub fn bytesc(value: &[u8]) -> E {
    E::Lit(ConstValue {
        value_type: t::bytes(),
        data: ConstData::Bytes(value.to_vec()),
    })
}
/// `Text` literal.
#[must_use]
pub fn textc(value: &str) -> E {
    E::Lit(ConstValue {
        value_type: t::text(),
        data: ConstData::Text(value.to_owned()),
    })
}

macro_rules! prim2 {
    ($($name:ident => $prim:ident),* $(,)?) => {$(
        #[doc = concat!("`", stringify!($prim), "` of two operands.")]
        #[must_use]
        pub fn $name(left: E, right: E) -> E {
            E::Prim(Prim::$prim, vec![left, right])
        }
    )*};
}
macro_rules! prim1 {
    ($($name:ident => $prim:ident),* $(,)?) => {$(
        #[doc = concat!("`", stringify!($prim), "` of one operand.")]
        #[must_use]
        pub fn $name(value: E) -> E {
            E::Prim(Prim::$prim, vec![value])
        }
    )*};
}
prim2! {
    add => Add, sub => Sub, mul => Mul, div => Div, rem => Rem, shl => Shl, shr => Shr,
    try_add => TryAdd, try_sub => TrySub, try_mul => TryMul, try_shl => TryShl,
    eq => Eq, ne => Ne, lt => Lt, le => Le, gt => Gt, ge => Ge, and => And, or => Or,
    vget => VGet, vget_opt => VGetOpt, mget_opt => MGetOpt, mget => MGet, mhas => MHas,
    mdel => MDel, push => Push,
}
prim1! {
    neg => Neg, not => Not, vlen => VLen, vhash => VHash, b2v => B2V, v2b => V2B,
    blake3 => RawBlake3, blake3_large => RawBlake3Large, sha256 => Sha256, concat => Concat,
    blen => BytesLen,
    unwrap => Unwrap, is_some => IsSome, is_ok => IsOk,
}
/// `bytes[start..start + length]`.
#[must_use]
pub fn slice(bytes: E, start: E, length: E) -> E {
    E::Prim(Prim::Slice, vec![bytes, start, length])
}
/// Persistent vector update.
#[must_use]
pub fn vset(vector: E, index: E, value: E) -> E {
    E::Prim(Prim::VSet, vec![vector, index, value])
}
/// Persistent map insertion.
#[must_use]
pub fn mins(map: E, key: E, value: E) -> E {
    E::Prim(Prim::MIns, vec![map, key, value])
}
/// Direct call.
#[must_use]
pub fn call(name: &str, args: Vec<E>) -> E {
    E::Call(name.to_owned(), args)
}
/// Record field access.
#[must_use]
pub fn field(value: E, name: &str) -> E {
    E::Field(Box::new(value), name.to_owned())
}
/// Record construction.
#[must_use]
pub fn rec(type_name: &str, fields: Vec<E>) -> E {
    E::Record(type_name.to_owned(), fields)
}
/// Variant construction.
#[must_use]
pub fn var_new(type_name: &str, case: &str, payload: Option<E>) -> E {
    E::Variant(type_name.to_owned(), case.to_owned(), payload.map(Box::new))
}
/// Variant payload projection.
#[must_use]
pub fn var_get(value: E, type_name: &str, case: &str) -> E {
    E::VariantGet(Box::new(value), type_name.to_owned(), case.to_owned())
}
/// Variant case test.
#[must_use]
pub fn is_case(value: E, type_name: &str, case: &str) -> E {
    E::IsCase(Box::new(value), type_name.to_owned(), case.to_owned())
}
/// `option_none`.
#[must_use]
pub fn none(element: Ty) -> E {
    E::NoneOf(element)
}
/// `option_some`.
#[must_use]
pub fn some(value: E) -> E {
    E::Some(Box::new(value))
}
/// `result_ok`.
#[must_use]
pub fn ok(value: E, error: Ty) -> E {
    E::Ok(Box::new(value), error)
}
/// `result_err`.
#[must_use]
pub fn err(value: E, ok: Ty) -> E {
    E::Err(Box::new(value), ok)
}
/// `vector_new`.
#[must_use]
pub fn vnew(element: Ty, items: Vec<E>) -> E {
    E::VecNew(element, items)
}
/// `map_new`.
#[must_use]
pub fn mnew(key: Ty, value: Ty, entries: Vec<(E, E)>) -> E {
    E::MapNew(key, value, entries)
}
/// `tuple_new`.
#[must_use]
pub fn tup(items: Vec<E>) -> E {
    E::Tuple(items)
}
/// `tuple_get`.
#[must_use]
pub fn tget(value: E, index: u32) -> E {
    E::TupleGet(Box::new(value), index)
}
/// `expr?`.
#[must_use]
pub fn tri(value: E) -> E {
    E::Try(Box::new(value))
}
/// Conditional expression.
#[must_use]
pub fn select(condition: E, then: E, otherwise: E) -> E {
    E::Select(Box::new(condition), Box::new(then), Box::new(otherwise))
}

/// The frozen identity of one host bridge entry, with an optional
/// big-endian `u32` row discriminator in bytes 16..20.
#[must_use]
pub fn bridge_id(code: [u8; 4], discriminator: u32) -> EntityId {
    let mut bytes = [0_u8; 32];
    bytes[..12].copy_from_slice(b"SLY1/BRIDGE/");
    bytes[12..16].copy_from_slice(&code);
    bytes[16..20].copy_from_slice(&discriminator.to_be_bytes());
    EntityId::from_bytes(bytes)
}

fn hash_id(domain: &str, parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sley-selfhost.seed.v1\0");
    hasher.update(domain.as_bytes());
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

/// Named type registry entry.
#[derive(Clone, Debug)]
struct NamedInfo {
    id: EntityId,
    /// Member names with ids and types (record fields or variant cases).
    members: Vec<(String, MemberId, Option<Ty>)>,
    is_record: bool,
}

#[derive(Clone, Debug)]
struct Signature {
    id: EntityId,
    params: Vec<(String, Ty)>,
    result: Ty,
}

/// Bridge rows the program uses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeSet {
    /// `BOOTSTRAP_PROFILE_2`: B2V1, V2B1, one PSH1 row, RHW1.
    Profile2,
}

/// The emitted program inventory.
#[derive(Clone, Debug, Default)]
pub struct Program {
    /// Functions in declaration order.
    pub functions: Vec<FunctionGraph>,
    /// All parameters.
    pub parameters: Vec<Parameter>,
    /// All blocks.
    pub blocks: Vec<Block>,
    /// All operations.
    pub operations: Vec<Operation>,
    /// All constants.
    pub constants: Vec<ConstantDefinition>,
    /// All type definitions.
    pub type_definitions: Vec<TypeDefinition>,
    /// All adapter imports.
    pub imports: Vec<AdapterImport>,
}

impl Program {
    /// Returns the function identity for a module function name.
    #[must_use]
    pub fn function_named(&self, names: &HashMap<String, EntityId>, name: &str) -> EntityId {
        names[name]
    }
}

type Body = Box<dyn FnOnce(&mut FnB<'_>)>;

/// A module under construction.
pub struct Module {
    core: Core,
    bodies: Vec<(String, Body)>,
}

struct Core {
    seed: String,
    types: HashMap<String, NamedInfo>,
    type_order: Vec<String>,
    signatures: HashMap<String, Signature>,
    signature_order: Vec<String>,
    constants: Vec<ConstantDefinition>,
    constant_index: HashMap<String, EntityId>,
    imports: Vec<AdapterImport>,
    push_rows: Vec<Ty>,
    out: Program,
}

impl Module {
    /// Starts a module whose identities derive from `seed`.
    #[must_use]
    pub fn new(seed: &str) -> Self {
        Self {
            core: Core {
                seed: seed.to_owned(),
                types: HashMap::new(),
                type_order: Vec::new(),
                signatures: HashMap::new(),
                signature_order: Vec::new(),
                constants: Vec::new(),
                constant_index: HashMap::new(),
                imports: Vec::new(),
                push_rows: Vec::new(),
                out: Program::default(),
            },
            bodies: Vec::new(),
        }
    }

    /// The named type for `name` (declared or not yet declared).
    #[must_use]
    pub fn ty(&self, name: &str) -> Ty {
        named_type(&self.core.seed, name)
    }

    /// Declares a record type.
    ///
    /// # Panics
    ///
    /// Panics on a duplicate type name.
    pub fn record(&mut self, name: &str, fields: &[(&str, Ty)]) -> Ty {
        self.define(
            name,
            true,
            fields.iter().map(|(n, t)| (*n, Some(t.clone()))).collect(),
        )
    }

    /// Declares a variant type.
    ///
    /// # Panics
    ///
    /// Panics on a duplicate type name.
    pub fn variant(&mut self, name: &str, cases: &[(&str, Option<Ty>)]) -> Ty {
        self.define(
            name,
            false,
            cases.iter().map(|(n, t)| (*n, t.clone())).collect(),
        )
    }

    fn define(&mut self, name: &str, is_record: bool, members: Vec<(&str, Option<Ty>)>) -> Ty {
        assert!(!self.core.types.contains_key(name), "duplicate type {name}");
        let id = type_id(&self.core.seed, name);
        let members = members
            .into_iter()
            .map(|(member, ty)| {
                let member_id = MemberId::from_bytes(hash_id(
                    "member",
                    &[
                        self.core.seed.as_bytes(),
                        name.as_bytes(),
                        member.as_bytes(),
                    ],
                ));
                (member.to_owned(), member_id, ty)
            })
            .collect::<Vec<_>>();
        self.core.types.insert(
            name.to_owned(),
            NamedInfo {
                id,
                members,
                is_record,
            },
        );
        self.core.type_order.push(name.to_owned());
        self.ty(name)
    }

    /// Declares and defines a function.
    ///
    /// # Panics
    ///
    /// Panics on a duplicate function name.
    pub fn func(
        &mut self,
        name: &str,
        params: &[(&str, Ty)],
        result: Ty,
        body: impl FnOnce(&mut FnB<'_>) + 'static,
    ) {
        assert!(
            !self.core.signatures.contains_key(name),
            "duplicate function {name}"
        );
        let id = EntityId::from_bytes(hash_id(
            "function",
            &[self.core.seed.as_bytes(), name.as_bytes()],
        ));
        self.core.signatures.insert(
            name.to_owned(),
            Signature {
                id,
                params: params
                    .iter()
                    .map(|(n, t)| ((*n).to_owned(), t.clone()))
                    .collect(),
                result,
            },
        );
        self.core.signature_order.push(name.to_owned());
        self.bodies.push((name.to_owned(), Box::new(body)));
    }

    /// The identity of a declared function.
    #[must_use]
    pub fn function_id(&self, name: &str) -> EntityId {
        self.core.signatures[name].id
    }

    /// Builds every function and returns the complete inventory plus a
    /// name-to-identity map.
    ///
    /// # Panics
    ///
    /// Panics on any construction error (unknown names, type mismatches).
    #[must_use]
    pub fn finish(mut self) -> (Program, HashMap<String, EntityId>) {
        let bodies = std::mem::take(&mut self.bodies);
        for (name, body) in bodies {
            let mut builder = FnB::new(&mut self.core, &name);
            body(&mut builder);
            builder.finish();
        }
        let mut core = self.core;
        for name in &core.type_order {
            let info = &core.types[name];
            let form = if info.is_record {
                TypeDefForm::Record(
                    info.members
                        .iter()
                        .map(|(_, member_id, ty)| RecordField {
                            member_id: *member_id,
                            value_type: ty.clone().expect("record field type"),
                            visibility: Visibility::Private,
                        })
                        .collect(),
                )
            } else {
                TypeDefForm::Variant(
                    info.members
                        .iter()
                        .map(|(_, member_id, ty)| VariantCase {
                            member_id: *member_id,
                            payload_type: ty.clone(),
                        })
                        .collect(),
                )
            };
            core.out.type_definitions.push(TypeDefinition {
                entity_id: info.id,
                type_parameters: Vec::new(),
                form,
                invariants: Vec::new(),
                visibility: Visibility::Private,
            });
        }
        core.out.constants = core.constants;
        core.out.imports = core.imports;
        let names = core
            .signature_order
            .iter()
            .map(|name| (name.clone(), core.signatures[name].id))
            .collect();
        (core.out, names)
    }
}

/// The identity of the named type `name` in a module seeded `seed`.
#[must_use]
pub fn type_id(seed: &str, name: &str) -> EntityId {
    EntityId::from_bytes(hash_id("type", &[seed.as_bytes(), name.as_bytes()]))
}

/// The member identity of `member` of type `type_name` in a module seeded
/// `seed`.
#[must_use]
pub fn member_id(seed: &str, type_name: &str, member: &str) -> MemberId {
    MemberId::from_bytes(hash_id(
        "member",
        &[seed.as_bytes(), type_name.as_bytes(), member.as_bytes()],
    ))
}

fn named_type(seed: &str, name: &str) -> Ty {
    TypeExpr::Named(NamedType {
        definition: type_id(seed, name),
        arguments: Vec::new(),
    })
}

impl Core {
    fn constant(&mut self, value: ConstValue) -> EntityId {
        let key = format!("{value:?}");
        if let Some(id) = self.constant_index.get(&key) {
            return *id;
        }
        let ordinal = self.constants.len() as u64;
        let id = EntityId::from_bytes(hash_id(
            "constant",
            &[self.seed.as_bytes(), &ordinal.to_be_bytes()],
        ));
        self.constants.push(ConstantDefinition {
            entity_id: id,
            value,
        });
        self.constant_index.insert(key, id);
        id
    }

    fn import(&mut self, row: AdapterImport) -> EntityId {
        if let Some(existing) = self.imports.iter().find(|i| i.entity_id == row.entity_id) {
            assert_eq!(existing, &row, "conflicting bridge rows for one identity");
            return row.entity_id;
        }
        let id = row.entity_id;
        self.imports.push(row);
        id
    }

    /// The discriminator of the push row for one element type (`HOST_ABI_V3`
    /// discriminated rows, allocated from 1 in order of first use).
    fn push_discriminator(&mut self, element: &Ty) -> u32 {
        let position = if let Some(position) = self.push_rows.iter().position(|ty| ty == element) {
            position
        } else {
            self.push_rows.push(element.clone());
            self.push_rows.len() - 1
        };
        u32::try_from(position + 1).expect("push row count fits u32")
    }

    fn named_by_id(&self, id: EntityId) -> Option<(&str, &NamedInfo)> {
        self.types
            .iter()
            .find(|(_, info)| info.id == id)
            .map(|(name, info)| (name.as_str(), info))
    }
}

fn bridge_row(code: [u8; 4], discriminator: u32, request: Ty, response: Ty) -> AdapterImport {
    let id = bridge_id(code, discriminator);
    AdapterImport {
        entity_id: id,
        adapter_id: *id.as_bytes(),
        abi_version: 1,
        request_type: request,
        response_type: response,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    }
}

#[derive(Clone, Debug)]
struct Binding {
    name: String,
    value: ValueRef,
    ty: Ty,
    mutable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Pos {
    scope: usize,
    index: usize,
}

/// A value the builder threads across block boundaries: a named binding or
/// a pending intermediate of the expression being evaluated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Slot {
    Bind(Pos),
    Pend(usize),
}

struct PendingBlock {
    id: EntityId,
    params: Vec<EntityId>,
    ops: Vec<EntityId>,
    terminator: Option<Terminator>,
}

struct LoopCtx {
    header: usize,
    exit: usize,
    slots: Vec<Slot>,
    exit_used: bool,
}

/// One arm of a multi-way split: an optional payload binding and a body
/// that may produce a value for the join.
#[allow(clippy::type_complexity)]
pub struct ArmSpec<'b, 'a> {
    payload: Option<(String, Ty)>,
    body: Box<dyn FnOnce(&mut FnB<'a>) -> Option<(ValueRef, Ty)> + 'b>,
}

/// A function body under construction.
pub struct FnB<'a> {
    core: &'a mut Core,
    name: String,
    function: EntityId,
    result: Ty,
    params: Vec<EntityId>,
    blocks: Vec<PendingBlock>,
    current: Option<usize>,
    scopes: Vec<Vec<Binding>>,
    pending: Vec<(ValueRef, Ty)>,
    block_params: std::collections::HashSet<EntityId>,
    loops: Vec<LoopCtx>,
    trap_block: Option<usize>,
    err_block: Option<usize>,
    trap_sites: u32,
    dead_blocks: Vec<usize>,
    counter: u64,
    parameters: Vec<Parameter>,
    operations: Vec<Operation>,
}

impl<'a> FnB<'a> {
    fn new(core: &'a mut Core, name: &str) -> Self {
        let signature = core.signatures[name].clone();
        let mut builder = Self {
            core,
            name: name.to_owned(),
            function: signature.id,
            result: signature.result.clone(),
            params: Vec::new(),
            blocks: Vec::new(),
            current: None,
            scopes: vec![Vec::new()],
            pending: Vec::new(),
            block_params: std::collections::HashSet::new(),
            loops: Vec::new(),
            trap_block: None,
            err_block: None,
            trap_sites: 0,
            dead_blocks: Vec::new(),
            counter: 0,
            parameters: Vec::new(),
            operations: Vec::new(),
        };
        for (ordinal, (param_name, ty)) in signature.params.iter().enumerate() {
            let id = builder.local_id("param");
            builder.parameters.push(Parameter {
                entity_id: id,
                owner: signature.id,
                role: ParameterRole::Function,
                ordinal: u32::try_from(ordinal).expect("parameter ordinal fits u32"),
                value_type: ty.clone(),
            });
            builder.params.push(id);
            builder.scopes[0].push(Binding {
                name: param_name.clone(),
                value: ValueRef::Parameter(id),
                ty: ty.clone(),
                mutable: false,
            });
        }
        let entry = builder.new_block();
        builder.current = Some(entry);
        builder
    }

    fn local_id(&mut self, kind: &str) -> EntityId {
        self.counter += 1;
        EntityId::from_bytes(hash_id(
            kind,
            &[
                self.core.seed.as_bytes(),
                self.name.as_bytes(),
                &self.counter.to_be_bytes(),
            ],
        ))
    }

    fn new_block(&mut self) -> usize {
        let id = self.local_id("block");
        self.blocks.push(PendingBlock {
            id,
            params: Vec::new(),
            ops: Vec::new(),
            terminator: None,
        });
        self.blocks.len() - 1
    }

    fn block_param(&mut self, block: usize, ty: Ty) -> ValueRef {
        let id = self.local_id("bparam");
        let ordinal = u32::try_from(self.blocks[block].params.len()).expect("ordinal fits u32");
        self.parameters.push(Parameter {
            entity_id: id,
            owner: self.blocks[block].id,
            role: ParameterRole::Block,
            ordinal,
            value_type: ty,
        });
        self.blocks[block].params.push(id);
        self.block_params.insert(id);
        ValueRef::Parameter(id)
    }

    fn cur(&self) -> usize {
        self.current
            .unwrap_or_else(|| panic!("{}: statement after a terminator", self.name))
    }

    /// Whether the current path is still open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.current.is_some()
    }

    fn terminate(&mut self, terminator: Terminator) {
        let block = self.cur();
        assert!(self.blocks[block].terminator.is_none());
        self.blocks[block].terminator = Some(terminator);
        self.current = None;
    }

    fn emit(
        &mut self,
        opcode: Opcode,
        operands: Vec<ValueRef>,
        result: Ty,
        immediate: Immediate,
    ) -> ValueRef {
        let block = self.cur();
        let id = self.local_id("op");
        let ordinal = u32::try_from(self.blocks[block].ops.len()).expect("ordinal fits u32");
        self.operations.push(Operation {
            entity_id: id,
            block: self.blocks[block].id,
            ordinal,
            opcode,
            operands,
            result_types: vec![result],
            immediate,
        });
        self.blocks[block].ops.push(id);
        ValueRef::OperationResult(OperationResultRef {
            operation: id,
            result_index: 0,
        })
    }

    fn lookup(&self, name: &str) -> Option<Pos> {
        for (scope_index, scope) in self.scopes.iter().enumerate().rev() {
            if let Some(index) = scope.iter().rposition(|b| b.name == name) {
                return Some(Pos {
                    scope: scope_index,
                    index,
                });
            }
        }
        None
    }

    fn binding(&self, pos: Pos) -> &Binding {
        &self.scopes[pos.scope][pos.index]
    }

    fn is_block_param(&self, value: ValueRef) -> bool {
        matches!(value, ValueRef::Parameter(id) if self.block_params.contains(&id))
    }

    /// Slots to thread: block-parameter values (always) plus mutable
    /// bindings (when `with_mutable`).
    fn threaded(&self, with_mutable: bool) -> Vec<Slot> {
        let mut out = Vec::new();
        for (scope_index, scope) in self.scopes.iter().enumerate() {
            for (index, binding) in scope.iter().enumerate() {
                if (with_mutable && binding.mutable) || self.is_block_param(binding.value) {
                    out.push(Slot::Bind(Pos {
                        scope: scope_index,
                        index,
                    }));
                }
            }
        }
        for (index, (value, _)) in self.pending.iter().enumerate() {
            if self.is_block_param(*value) {
                out.push(Slot::Pend(index));
            }
        }
        out
    }

    fn slot_value(&self, slot: Slot) -> ValueRef {
        match slot {
            Slot::Bind(pos) => self.binding(pos).value,
            Slot::Pend(index) => self.pending[index].0,
        }
    }

    fn slot_ty(&self, slot: Slot) -> Ty {
        match slot {
            Slot::Bind(pos) => self.binding(pos).ty.clone(),
            Slot::Pend(index) => self.pending[index].1.clone(),
        }
    }

    fn set_slot(&mut self, slot: Slot, value: ValueRef) {
        match slot {
            Slot::Bind(pos) => self.scopes[pos.scope][pos.index].value = value,
            Slot::Pend(index) => self.pending[index].0 = value,
        }
    }

    fn values(&self, slots: &[Slot]) -> Vec<ValueRef> {
        slots.iter().map(|s| self.slot_value(*s)).collect()
    }

    fn set_values(&mut self, slots: &[Slot], values: &[ValueRef]) {
        for (slot, value) in slots.iter().zip(values) {
            self.set_slot(*slot, *value);
        }
    }

    /// The distinct current values of `slots`, in first-occurrence order:
    /// the edge arguments of a carry (one argument per value, never two
    /// for an aliased value, so a carried large value stays unshared).
    fn distinct(&self, slots: &[Slot]) -> Vec<ValueRef> {
        let mut out: Vec<ValueRef> = Vec::new();
        for value in self.values(slots) {
            if !out.contains(&value) {
                out.push(value);
            }
        }
        out
    }

    /// One parameter per distinct carried value on a single-predecessor
    /// block; returns each slot's parameter.
    fn carry_params(&mut self, block: usize, slots: &[Slot]) -> Vec<ValueRef> {
        let mut seen: Vec<(ValueRef, ValueRef)> = Vec::new();
        let mut out = Vec::with_capacity(slots.len());
        for slot in slots {
            let value = self.slot_value(*slot);
            if let Some((_, param)) = seen.iter().find(|(v, _)| *v == value) {
                out.push(*param);
                continue;
            }
            let ty = self.slot_ty(*slot);
            let param = self.block_param(block, ty);
            seen.push((value, param));
            out.push(param);
        }
        out
    }

    /// Adds one parameter per slot to `block` and returns them.
    fn slot_params(&mut self, block: usize, slots: &[Slot]) -> Vec<ValueRef> {
        let mut out = Vec::new();
        for slot in slots {
            let ty = self.slot_ty(*slot);
            out.push(self.block_param(block, ty));
        }
        out
    }

    fn edge(&self, block: usize, arguments: Vec<ValueRef>) -> TargetEdge {
        TargetEdge {
            target: self.blocks[block].id,
            arguments,
        }
    }

    fn branch(&mut self, block: usize, arguments: Vec<ValueRef>) {
        let edge = self.edge(block, arguments);
        self.terminate(Terminator::Branch(BranchTerminator { edge }));
    }

    /// Ends the current block with a multi-way branch into fresh arm
    /// blocks, runs every arm, and joins the open ends. `make_term` builds
    /// the terminator from each arm's target and carried arguments.
    fn split(
        &mut self,
        make_term: impl FnOnce(&Self, &[(usize, bool)], &[ValueRef]) -> Terminator,
        arms: Vec<ArmSpec<'_, 'a>>,
        result: Option<Ty>,
    ) -> Option<ValueRef> {
        let carry = self.threaded(false);
        let carry_values = self.distinct(&carry);
        let join_slots = self.threaded(true);
        let before = self.values(&join_slots);
        let mut prepared = Vec::new();
        for arm in &arms {
            let block = self.new_block();
            let payload = arm
                .payload
                .as_ref()
                .map(|(_, ty)| self.block_param(block, ty.clone()));
            let params = self.carry_params(block, &carry);
            prepared.push((block, payload, params));
        }
        let targets: Vec<(usize, bool)> = prepared
            .iter()
            .map(|(block, payload, _)| (*block, payload.is_some()))
            .collect();
        let terminator = make_term(self, &targets, &carry_values);
        self.terminate(terminator);
        let mut ends = Vec::new();
        for (arm, (block, payload, params)) in arms.into_iter().zip(prepared) {
            self.set_values(&join_slots, &before);
            self.set_values(&carry, &params);
            self.current = Some(block);
            let mut bindings = Vec::new();
            if let (Some((name, ty)), Some(value)) = (arm.payload, payload) {
                bindings.push(Binding {
                    name,
                    value,
                    ty,
                    mutable: false,
                });
            }
            self.scopes.push(bindings);
            let produced = (arm.body)(self);
            self.scopes.pop();
            if let Some(end) = self.current {
                ends.push((end, produced, self.values(&join_slots)));
            }
        }
        self.set_values(&join_slots, &before);
        if ends.is_empty() {
            self.current = None;
            return None;
        }
        let changed: Vec<usize> = (0..join_slots.len())
            .filter(|i| ends.iter().any(|(_, _, values)| values[*i] != before[*i]))
            .collect();
        let join = self.new_block();
        let result_value = result.clone().map(|ty| self.block_param(join, ty));
        let changed_slots: Vec<Slot> = changed.iter().map(|i| join_slots[*i]).collect();
        let join_values = self.slot_params(join, &changed_slots);
        for (end, produced, values) in ends {
            self.current = Some(end);
            let mut arguments = Vec::new();
            if let Some(expected) = &result {
                let (value, ty) = produced.expect("arm produces the join value");
                assert_eq!(&ty, expected, "{}: arm value types differ", self.name);
                arguments.push(value);
            }
            arguments.extend(changed.iter().map(|i| values[*i]));
            self.branch(join, arguments);
        }
        self.set_values(&changed_slots, &join_values);
        self.current = Some(join);
        result_value
    }

    fn switch_term(
        &self,
        value: ValueRef,
        keys: &[CaseKey],
        targets: &[(usize, bool)],
        carried: &[ValueRef],
    ) -> Terminator {
        let mut cases: Vec<SwitchCase> = keys
            .iter()
            .zip(targets)
            .map(|(key, (block, payload))| {
                let mut arguments = Vec::new();
                if *payload {
                    arguments.push(SwitchArgument::CasePayload);
                }
                arguments.extend(carried.iter().map(|v| SwitchArgument::Value(*v)));
                SwitchCase {
                    case_key: *key,
                    edge: SwitchEdge {
                        target: self.blocks[*block].id,
                        arguments,
                    },
                }
            })
            .collect();
        cases.sort_by(|a, b| a.case_key.cmp(&b.case_key));
        Terminator::VariantSwitch(VariantSwitchTerminator { value, cases })
    }

    // ----- statements -----

    /// Binds an immutable name.
    pub fn let_(&mut self, name: &str, value: E) {
        let (value, ty) = self.eval(&value);
        self.scopes.last_mut().expect("scope").push(Binding {
            name: name.to_owned(),
            value,
            ty,
            mutable: false,
        });
    }

    /// Declares a mutable variable.
    pub fn var(&mut self, name: &str, value: E) {
        let (value, ty) = self.eval(&value);
        self.scopes.last_mut().expect("scope").push(Binding {
            name: name.to_owned(),
            value,
            ty,
            mutable: true,
        });
    }

    /// Assigns a mutable variable.
    ///
    /// # Panics
    ///
    /// Panics on an unknown, immutable, or mistyped target.
    pub fn set(&mut self, name: &str, value: E) {
        let (value, ty) = self.eval(&value);
        let pos = self
            .lookup(name)
            .unwrap_or_else(|| panic!("{}: unknown variable {name}", self.name));
        let fname = self.name.clone();
        let binding = &mut self.scopes[pos.scope][pos.index];
        assert!(binding.mutable, "{fname}: {name} is immutable");
        assert_eq!(binding.ty, ty, "{fname}: {name} assigned a different type");
        binding.value = value;
    }

    /// Evaluates an expression for its effects (calls) and discards it.
    pub fn expr(&mut self, value: E) {
        let _ = self.eval(&value);
    }

    /// Returns from the function.
    ///
    /// # Panics
    ///
    /// Panics on a result type mismatch.
    pub fn ret(&mut self, value: E) {
        let (value, ty) = self.eval(&value);
        assert_eq!(ty, self.result, "{}: return type mismatch", self.name);
        self.terminate(Terminator::Return(ReturnTerminator { value }));
    }

    /// Traps unconditionally.
    pub fn trap(&mut self) {
        let site = self.trap_site();
        let (site_value, _) = self.eval(&site);
        let trap = self.trap_target();
        self.branch(trap, vec![site_value]);
    }

    fn trap_site(&mut self) -> E {
        self.trap_sites += 1;
        u32c(self.trap_sites)
    }

    fn trap_target(&mut self) -> usize {
        if let Some(block) = self.trap_block {
            return block;
        }
        let saved = self.current;
        let block = self.new_block();
        let site = self.block_param(block, t::u32());
        self.current = Some(block);
        let name = self.name.clone();
        let (name, _) = self.eval(&textc(&name));
        let payload = self.emit(
            Opcode::TupleNew,
            vec![name, site],
            t::tuple(vec![t::text(), t::u32()]),
            Immediate::None,
        );
        self.terminate(Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: Some(payload),
        }));
        self.current = saved;
        self.trap_block = Some(block);
        block
    }

    /// The shared early-return block: one `Err` payload parameter, wrapped
    /// as the function's `Result` and returned.
    fn err_target(&mut self, error: &Ty) -> usize {
        let TypeExpr::Result {
            error: declared, ..
        } = &self.result
        else {
            panic!(
                "{}: try in a function that does not return Result",
                self.name
            )
        };
        assert_eq!(
            declared.as_ref(),
            error,
            "{}: try error type differs from the function's",
            self.name
        );
        if let Some(block) = self.err_block {
            return block;
        }
        let saved = self.current;
        let block = self.new_block();
        let payload = self.block_param(block, error.clone());
        self.current = Some(block);
        let result = self.result.clone();
        let wrapped = self.emit(Opcode::ResultErr, vec![payload], result, Immediate::None);
        self.terminate(Terminator::Return(ReturnTerminator { value: wrapped }));
        self.current = saved;
        self.err_block = Some(block);
        block
    }

    /// Returns `Err(code)` from a `Result`-returning function.
    ///
    /// # Panics
    ///
    /// Panics outside a `Result` function.
    pub fn ret_err(&mut self, code: E) {
        let TypeExpr::Result { ok, .. } = self.result.clone() else {
            panic!("{}: ret_err outside a Result function", self.name)
        };
        self.ret(err(code, *ok));
    }

    /// Returns `Ok(value)` from a `Result`-returning function.
    ///
    /// # Panics
    ///
    /// Panics outside a `Result` function.
    pub fn ret_ok(&mut self, value: E) {
        let TypeExpr::Result { error, .. } = self.result.clone() else {
            panic!("{}: ret_ok outside a Result function", self.name)
        };
        self.ret(ok(value, *error));
    }

    /// `if condition { then }`.
    pub fn if_(&mut self, condition: E, then: impl FnOnce(&mut Self)) {
        self.if_else(condition, then, |_| {});
    }

    /// `if condition { then } else { otherwise }`.
    pub fn if_else(
        &mut self,
        condition: E,
        then: impl FnOnce(&mut Self),
        otherwise: impl FnOnce(&mut Self),
    ) {
        let (condition, ty) = self.eval(&condition);
        assert_eq!(ty, t::bool(), "{}: if condition must be Bool", self.name);
        let arms = vec![
            ArmSpec {
                payload: None,
                body: Box::new(move |f: &mut Self| {
                    then(f);
                    None
                }),
            },
            ArmSpec {
                payload: None,
                body: Box::new(move |f: &mut Self| {
                    otherwise(f);
                    None
                }),
            },
        ];
        self.split(
            |f, targets, carried| {
                Terminator::CondBranch(CondBranchTerminator {
                    condition,
                    if_true: f.edge(targets[0].0, carried.to_vec()),
                    if_false: f.edge(targets[1].0, carried.to_vec()),
                })
            },
            arms,
            None,
        );
    }

    /// `while condition { body }`.
    pub fn while_(&mut self, condition: E, body: impl FnOnce(&mut Self)) {
        self.loop_with(Some(condition), body);
    }

    /// `loop { body }` (exit with [`FnB::break_`] or return).
    pub fn loop_(&mut self, body: impl FnOnce(&mut Self)) {
        self.loop_with(None, body);
    }

    fn loop_with(&mut self, condition: Option<E>, body: impl FnOnce(&mut Self)) {
        let slots = self.threaded(true);
        let entry_values = self.values(&slots);
        let header = self.new_block();
        let exit = self.new_block();
        let header_values = self.slot_params(header, &slots);
        let exit_values = self.slot_params(exit, &slots);
        self.branch(header, entry_values.clone());
        self.current = Some(header);
        self.set_values(&slots, &header_values);
        self.loops.push(LoopCtx {
            header,
            exit,
            slots: slots.clone(),
            exit_used: condition.is_some(),
        });
        if let Some(condition) = condition {
            let (condition, ty) = self.eval(&condition);
            assert_eq!(ty, t::bool(), "{}: while condition must be Bool", self.name);
            let carry = self.threaded(false);
            let carry_values = self.distinct(&carry);
            let body_block = self.new_block();
            let body_params = self.carry_params(body_block, &carry);
            let exit_arguments = self.values(&slots);
            let terminator = Terminator::CondBranch(CondBranchTerminator {
                condition,
                if_true: self.edge(body_block, carry_values),
                if_false: self.edge(exit, exit_arguments),
            });
            self.terminate(terminator);
            self.current = Some(body_block);
            self.set_values(&carry, &body_params);
        }
        self.scopes.push(Vec::new());
        body(self);
        self.scopes.pop();
        if self.current.is_some() {
            self.continue_();
        }
        let ctx = self.loops.pop().expect("loop context");
        if ctx.exit_used {
            self.current = Some(exit);
            self.set_values(&slots, &exit_values);
        } else {
            // An infinite loop without `break`: its exit never runs.
            let exit_id = self.blocks[exit].id;
            self.parameters.retain(|p| p.owner != exit_id);
            self.blocks[exit].params.clear();
            self.blocks[exit].terminator = Some(Terminator::Trap(TrapTerminator {
                code: TrapCode::Unreachable,
                payload: None,
            }));
            self.dead_blocks.push(exit);
            self.set_values(&slots, &entry_values);
            self.current = None;
        }
    }

    /// Branches to the innermost loop's exit.
    ///
    /// # Panics
    ///
    /// Panics outside a loop.
    pub fn break_(&mut self) {
        let ctx = self.loops.last_mut().expect("break outside a loop");
        ctx.exit_used = true;
        let (exit, slots) = (ctx.exit, ctx.slots.clone());
        let values = self.values(&slots);
        self.branch(exit, values);
    }

    /// Branches to the innermost loop's header.
    ///
    /// # Panics
    ///
    /// Panics outside a loop.
    pub fn continue_(&mut self) {
        let ctx = self.loops.last().expect("continue outside a loop");
        let (header, slots) = (ctx.header, ctx.slots.clone());
        let values = self.values(&slots);
        self.branch(header, values);
    }

    /// `for name in start..end { body }` over `UInt(64)`. The body must
    /// not use [`FnB::continue_`] (it would skip the increment).
    pub fn for_u64(&mut self, name: &str, start: E, end: E, body: impl FnOnce(&mut Self)) {
        self.counter += 1;
        let end_name = format!("#end{}", self.counter);
        let index_name = format!("#i{}", self.counter);
        self.let_(&end_name, end);
        self.var(&index_name, start);
        let name = name.to_owned();
        self.while_(lt(v(&index_name), v(&end_name)), move |f| {
            f.let_(&name, v(&index_name));
            body(f);
            if f.is_open() {
                f.set(&index_name, add(v(&name), u64c(1)));
            }
        });
    }

    /// Matches an `Option`: binds the payload in `some`.
    ///
    /// # Panics
    ///
    /// Panics on a non-`Option` value.
    pub fn match_opt(
        &mut self,
        value: E,
        binding: &str,
        some: impl FnOnce(&mut Self),
        none: impl FnOnce(&mut Self),
    ) {
        let (value, ty) = self.eval(&value);
        let TypeExpr::Option(element) = ty else {
            panic!("{}: match_opt on a non-Option", self.name)
        };
        let arms = vec![
            ArmSpec {
                payload: None,
                body: Box::new(move |f: &mut Self| {
                    none(f);
                    None
                }),
            },
            ArmSpec {
                payload: Some((binding.to_owned(), *element)),
                body: Box::new(move |f: &mut Self| {
                    some(f);
                    None
                }),
            },
        ];
        let keys = [
            CaseKey::Builtin(BuiltinCase::None),
            CaseKey::Builtin(BuiltinCase::Some),
        ];
        self.split(
            |f, targets, carried| f.switch_term(value, &keys, targets, carried),
            arms,
            None,
        );
    }

    /// Matches a `Result`: binds the payload in each arm.
    ///
    /// # Panics
    ///
    /// Panics on a non-`Result` value.
    pub fn match_res(
        &mut self,
        value: E,
        ok_name: &str,
        on_ok: impl FnOnce(&mut Self),
        err_name: &str,
        on_err: impl FnOnce(&mut Self),
    ) {
        let (value, ty) = self.eval(&value);
        let TypeExpr::Result { ok, error } = ty else {
            panic!("{}: match_res on a non-Result", self.name)
        };
        let arms = vec![
            ArmSpec {
                payload: Some((ok_name.to_owned(), *ok)),
                body: Box::new(move |f: &mut Self| {
                    on_ok(f);
                    None
                }),
            },
            ArmSpec {
                payload: Some((err_name.to_owned(), *error)),
                body: Box::new(move |f: &mut Self| {
                    on_err(f);
                    None
                }),
            },
        ];
        let keys = [
            CaseKey::Builtin(BuiltinCase::Ok),
            CaseKey::Builtin(BuiltinCase::Err),
        ];
        self.split(
            |f, targets, carried| f.switch_term(value, &keys, targets, carried),
            arms,
            None,
        );
    }

    /// Matches a named variant. Every case must have an arm; each arm is
    /// `(case name, payload binding name, body)`.
    ///
    /// # Panics
    ///
    /// Panics on a missing, unknown, or duplicate case.
    #[allow(clippy::type_complexity)]
    pub fn match_var(
        &mut self,
        value: E,
        arms: Vec<(&str, &str, Box<dyn FnOnce(&mut Self) + '_>)>,
    ) {
        let (value, ty) = self.eval(&value);
        let info = self.named_info(&ty);
        assert!(!info.is_record, "{}: match_var on a record", self.name);
        assert_eq!(
            arms.len(),
            info.members.len(),
            "{}: match must cover every case",
            self.name
        );
        let mut keys = Vec::new();
        let mut specs = Vec::new();
        for (case_name, binding, body) in arms {
            let (_, member_id, payload) = info
                .members
                .iter()
                .find(|(n, _, _)| n == case_name)
                .unwrap_or_else(|| panic!("{}: unknown case {case_name}", self.name))
                .clone();
            let key = CaseKey::Member(member_id);
            assert!(
                !keys.contains(&key),
                "{}: duplicate case {case_name}",
                self.name
            );
            keys.push(key);
            specs.push(ArmSpec {
                payload: payload.map(|ty| (binding.to_owned(), ty)),
                body: Box::new(move |f: &mut Self| {
                    body(f);
                    None
                }),
            });
        }
        self.split(
            |f, targets, carried| f.switch_term(value, &keys, targets, carried),
            specs,
            None,
        );
    }

    fn named_info(&self, ty: &Ty) -> NamedInfo {
        let TypeExpr::Named(named) = ty else {
            panic!("{}: expected a named type, got {ty:?}", self.name)
        };
        self.core.named_by_id(named.definition).map_or_else(
            || panic!("{}: unknown named type", self.name),
            |(_, info)| info.clone(),
        )
    }

    fn type_of(&self, name: &str) -> NamedInfo {
        self.core
            .types
            .get(name)
            .unwrap_or_else(|| panic!("{}: unknown type {name}", self.name))
            .clone()
    }

    // ----- expressions -----

    /// Evaluates expressions left to right, keeping earlier results live
    /// (and threaded) while later ones split blocks.
    fn eval_all(&mut self, exprs: &[E]) -> Vec<(ValueRef, Ty)> {
        let base = self.pending.len();
        for expr in exprs {
            let value = self.eval(expr);
            self.pending.push(value);
        }
        self.pending.split_off(base)
    }

    /// Evaluates an expression into the current block.
    ///
    /// # Panics
    ///
    /// Panics on construction errors.
    #[allow(clippy::too_many_lines)]
    pub fn eval(&mut self, expr: &E) -> (ValueRef, Ty) {
        match expr {
            E::Var(name) => {
                let pos = self
                    .lookup(name)
                    .unwrap_or_else(|| panic!("{}: unknown name {name}", self.name));
                let binding = self.binding(pos);
                (binding.value, binding.ty.clone())
            }
            E::Lit(value) => {
                let ty = value.value_type.clone();
                let id = self.core.constant(value.clone());
                (
                    self.emit(
                        Opcode::ConstantRef,
                        Vec::new(),
                        ty.clone(),
                        Immediate::Entity(id),
                    ),
                    ty,
                )
            }
            E::Prim(prim, args) => {
                let values = self.eval_all(args);
                self.prim(*prim, values)
            }
            E::Call(name, args) => {
                let signature = self
                    .core
                    .signatures
                    .get(name)
                    .unwrap_or_else(|| panic!("{}: unknown function {name}", self.name))
                    .clone();
                assert_eq!(
                    args.len(),
                    signature.params.len(),
                    "{}: call {name} arity",
                    self.name
                );
                let values = self.eval_all(args);
                for ((_, ty), (param_name, param_ty)) in values.iter().zip(&signature.params) {
                    assert_eq!(
                        ty, param_ty,
                        "{}: call {name} argument {param_name} type",
                        self.name
                    );
                }
                (
                    self.emit(
                        Opcode::CallDirect,
                        values.into_iter().map(|(v, _)| v).collect(),
                        signature.result.clone(),
                        Immediate::Function(FunctionRefValue {
                            function: signature.id,
                            type_arguments: Vec::new(),
                        }),
                    ),
                    signature.result,
                )
            }
            E::Field(value, field_name) => {
                let (value, ty) = self.eval(value);
                let info = self.named_info(&ty);
                let (_, member_id, field_ty) = info
                    .members
                    .iter()
                    .find(|(n, _, _)| n == field_name)
                    .unwrap_or_else(|| panic!("{}: unknown field {field_name}", self.name))
                    .clone();
                let field_ty = field_ty.expect("record field type");
                (
                    self.emit(
                        Opcode::RecordGet,
                        vec![value],
                        field_ty.clone(),
                        Immediate::Field(member_id),
                    ),
                    field_ty,
                )
            }
            E::Record(type_name, fields) => {
                let info = self.type_of(type_name);
                assert!(info.is_record, "{}: {type_name} is not a record", self.name);
                assert_eq!(
                    fields.len(),
                    info.members.len(),
                    "{}: record {type_name} field count",
                    self.name
                );
                let values = self.eval_all(fields);
                for ((_, ty), (field_name, _, field_ty)) in values.iter().zip(&info.members) {
                    assert_eq!(
                        Some(ty),
                        field_ty.as_ref(),
                        "{}: record {type_name}.{field_name} type",
                        self.name
                    );
                }
                let ty = named_type(&self.core.seed, type_name);
                (
                    self.emit(
                        Opcode::RecordNew,
                        values.into_iter().map(|(v, _)| v).collect(),
                        ty.clone(),
                        Immediate::Entity(info.id),
                    ),
                    ty,
                )
            }
            E::Variant(type_name, case_name, payload) => {
                let info = self.type_of(type_name);
                assert!(
                    !info.is_record,
                    "{}: {type_name} is not a variant",
                    self.name
                );
                let (_, member_id, payload_ty) = info
                    .members
                    .iter()
                    .find(|(n, _, _)| n == case_name)
                    .unwrap_or_else(|| panic!("{}: unknown case {case_name}", self.name))
                    .clone();
                let mut operands = Vec::new();
                match (payload, payload_ty) {
                    (Some(expr), Some(expected)) => {
                        let (value, ty) = self.eval(expr);
                        assert_eq!(
                            ty, expected,
                            "{}: {type_name}::{case_name} payload",
                            self.name
                        );
                        operands.push(value);
                    }
                    (None, None) => {}
                    _ => panic!("{}: {type_name}::{case_name} payload presence", self.name),
                }
                let ty = named_type(&self.core.seed, type_name);
                (
                    self.emit(
                        Opcode::VariantNew,
                        operands,
                        ty.clone(),
                        Immediate::Variant(VariantImmediate {
                            definition: info.id,
                            member_id,
                        }),
                    ),
                    ty,
                )
            }
            E::VariantGet(value, type_name, case_name) => {
                let info = self.type_of(type_name);
                let (_, member_id, payload_ty) = info
                    .members
                    .iter()
                    .find(|(n, _, _)| n == case_name)
                    .unwrap_or_else(|| panic!("{}: unknown case {case_name}", self.name))
                    .clone();
                let payload_ty = payload_ty.expect("variant_get needs a payload case");
                let (value, _) = self.eval(value);
                let ty = t::opt(payload_ty);
                (
                    self.emit(
                        Opcode::VariantGet,
                        vec![value],
                        ty.clone(),
                        Immediate::Variant(VariantImmediate {
                            definition: info.id,
                            member_id,
                        }),
                    ),
                    ty,
                )
            }
            E::IsCase(value, type_name, case_name) => {
                let (value, ty) = self.eval(value);
                let info = self.type_of(type_name);
                assert_eq!(ty, named_type(&self.core.seed, type_name));
                let keys: Vec<CaseKey> = info
                    .members
                    .iter()
                    .map(|(_, member_id, _)| CaseKey::Member(*member_id))
                    .collect();
                let arms = info
                    .members
                    .iter()
                    .map(|(name, _, _)| {
                        let hit = name == case_name;
                        ArmSpec {
                            payload: None,
                            body: Box::new(move |f: &mut Self| Some(f.eval(&boolc(hit)))),
                        }
                    })
                    .collect();
                let result = self
                    .split(
                        |f, targets, carried| f.switch_term(value, &keys, targets, carried),
                        arms,
                        Some(t::bool()),
                    )
                    .expect("case test joins");
                (result, t::bool())
            }
            E::NoneOf(element) => {
                let ty = t::opt(element.clone());
                (
                    self.emit(Opcode::OptionNone, Vec::new(), ty.clone(), Immediate::None),
                    ty,
                )
            }
            E::Some(value) => {
                let (value, ty) = self.eval(value);
                let ty = t::opt(ty);
                (
                    self.emit(Opcode::OptionSome, vec![value], ty.clone(), Immediate::None),
                    ty,
                )
            }
            E::Ok(value, error) => {
                let (value, ty) = self.eval(value);
                let ty = t::res(ty, error.clone());
                (
                    self.emit(Opcode::ResultOk, vec![value], ty.clone(), Immediate::None),
                    ty,
                )
            }
            E::Err(value, ok_ty) => {
                let (value, ty) = self.eval(value);
                let ty = t::res(ok_ty.clone(), ty);
                (
                    self.emit(Opcode::ResultErr, vec![value], ty.clone(), Immediate::None),
                    ty,
                )
            }
            E::VecNew(element, items) => {
                let values = self.eval_all(items);
                for (_, ty) in &values {
                    assert_eq!(ty, element, "{}: vector element type", self.name);
                }
                let ty = t::vec(element.clone());
                (
                    self.emit(
                        Opcode::VectorNew,
                        values.into_iter().map(|(v, _)| v).collect(),
                        ty.clone(),
                        Immediate::None,
                    ),
                    ty,
                )
            }
            E::MapNew(key, value_ty, entries) => {
                let flat: Vec<E> = entries
                    .iter()
                    .flat_map(|(k, v)| [k.clone(), v.clone()])
                    .collect();
                let values = self.eval_all(&flat);
                for pair in values.chunks(2) {
                    assert_eq!(&pair[0].1, key);
                    assert_eq!(&pair[1].1, value_ty);
                }
                let map_ty = t::map(key.clone(), value_ty.clone());
                let raw_ty = t::res(map_ty, t::failure(BuiltinFailureKind::DuplicateKey));
                let raw = self.emit(
                    Opcode::MapNew,
                    values.into_iter().map(|(v, _)| v).collect(),
                    raw_ty.clone(),
                    Immediate::None,
                );
                self.unwrap_value(raw, &raw_ty)
            }
            E::Tuple(items) => {
                let values = self.eval_all(items);
                let ty = t::tuple(values.iter().map(|(_, ty)| ty.clone()).collect());
                (
                    self.emit(
                        Opcode::TupleNew,
                        values.into_iter().map(|(v, _)| v).collect(),
                        ty.clone(),
                        Immediate::None,
                    ),
                    ty,
                )
            }
            E::TupleGet(value, index) => {
                let (value, ty) = self.eval(value);
                let TypeExpr::Tuple(items) = ty else {
                    panic!("{}: tuple_get of non-tuple", self.name)
                };
                let element = items[*index as usize].clone();
                (
                    self.emit(
                        Opcode::TupleGet,
                        vec![value],
                        element.clone(),
                        Immediate::Index(*index),
                    ),
                    element,
                )
            }
            E::Try(value) => {
                let (value, ty) = self.eval(value);
                let TypeExpr::Result { ok, error } = ty else {
                    panic!("{}: try of a non-Result", self.name)
                };
                let target = self.err_target(&error);
                let carry = self.threaded(false);
                let carry_values = self.distinct(&carry);
                let next = self.new_block();
                let payload = self.block_param(next, (*ok).clone());
                let params = self.carry_params(next, &carry);
                let mut good = vec![SwitchArgument::CasePayload];
                good.extend(carry_values.iter().map(|v| SwitchArgument::Value(*v)));
                let cases = vec![
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Ok),
                        edge: SwitchEdge {
                            target: self.blocks[next].id,
                            arguments: good,
                        },
                    },
                    SwitchCase {
                        case_key: CaseKey::Builtin(BuiltinCase::Err),
                        edge: SwitchEdge {
                            target: self.blocks[target].id,
                            arguments: vec![SwitchArgument::CasePayload],
                        },
                    },
                ];
                self.terminate(Terminator::VariantSwitch(VariantSwitchTerminator {
                    value,
                    cases,
                }));
                self.current = Some(next);
                self.set_values(&carry, &params);
                (payload, *ok)
            }
            E::Select(condition, then, otherwise) => {
                let (condition, ty) = self.eval(condition);
                assert_eq!(ty, t::bool());
                let (then, otherwise) = ((**then).clone(), (**otherwise).clone());
                let mut result_ty = None;
                // Type the join from the then-arm before building it.
                let probe = self.static_type(&then);
                if let Some(ty) = probe {
                    result_ty = Some(ty);
                }
                let result_ty = result_ty.unwrap_or_else(|| {
                    panic!("{}: select arm type must be statically known", self.name)
                });
                let arms = vec![
                    ArmSpec {
                        payload: None,
                        body: Box::new(move |f: &mut Self| Some(f.eval(&then))),
                    },
                    ArmSpec {
                        payload: None,
                        body: Box::new(move |f: &mut Self| Some(f.eval(&otherwise))),
                    },
                ];
                let result = self
                    .split(
                        |f, targets, carried| {
                            Terminator::CondBranch(CondBranchTerminator {
                                condition,
                                if_true: f.edge(targets[0].0, carried.to_vec()),
                                if_false: f.edge(targets[1].0, carried.to_vec()),
                            })
                        },
                        arms,
                        Some(result_ty.clone()),
                    )
                    .expect("select joins");
                (result, result_ty)
            }
        }
    }

    /// A conservative static type for simple expressions (used to type a
    /// `select` join before building its arms).
    fn static_type(&self, expr: &E) -> Option<Ty> {
        match expr {
            E::Var(name) => self.lookup(name).map(|p| self.binding(p).ty.clone()),
            E::Lit(value) => Some(value.value_type.clone()),
            E::Call(name, _) => self.core.signatures.get(name).map(|s| s.result.clone()),
            E::Record(name, _) | E::Variant(name, _, _) => Some(named_type(&self.core.seed, name)),
            E::NoneOf(element) => Some(t::opt(element.clone())),
            E::Some(inner) => self.static_type(inner).map(t::opt),
            E::Ok(inner, error) => self.static_type(inner).map(|ok| t::res(ok, error.clone())),
            E::Err(inner, ok) => self.static_type(inner).map(|e| t::res(ok.clone(), e)),
            E::VecNew(element, _) => Some(t::vec(element.clone())),
            E::Select(_, then, _) => self.static_type(then),
            E::Try(inner) => match self.static_type(inner)? {
                TypeExpr::Result { ok, .. } => Some(*ok),
                _ => None,
            },
            E::Field(value, field_name) => {
                let ty = self.static_type(value)?;
                let info = self.named_info(&ty);
                info.members
                    .iter()
                    .find(|(n, _, _)| n == field_name)
                    .and_then(|(_, _, ty)| ty.clone())
            }
            E::TupleGet(value, index) => match self.static_type(value)? {
                TypeExpr::Tuple(items) => items.get(*index as usize).cloned(),
                _ => None,
            },
            E::Prim(prim, args) => match prim {
                Prim::Add
                | Prim::Sub
                | Prim::Mul
                | Prim::Div
                | Prim::Rem
                | Prim::Shl
                | Prim::Shr
                | Prim::Neg
                | Prim::VSet
                | Prim::MIns
                | Prim::MDel
                | Prim::Push => self.static_type(&args[0]),
                Prim::Eq
                | Prim::Ne
                | Prim::Lt
                | Prim::Le
                | Prim::Gt
                | Prim::Ge
                | Prim::Not
                | Prim::And
                | Prim::Or
                | Prim::MHas
                | Prim::IsSome
                | Prim::IsOk => Some(t::bool()),
                Prim::VLen | Prim::BytesLen => Some(t::u64()),
                Prim::VGet => match self.static_type(&args[0])? {
                    TypeExpr::Vector(element) => Some(*element),
                    _ => None,
                },
                Prim::MGet => match self.static_type(&args[0])? {
                    TypeExpr::OrderedMap { value, .. } => Some(*value),
                    _ => None,
                },
                Prim::VHash
                | Prim::V2B
                | Prim::RawBlake3
                | Prim::RawBlake3Large
                | Prim::Sha256
                | Prim::Concat
                | Prim::Slice => Some(t::bytes()),
                Prim::B2V => Some(t::vec(t::u8())),
                Prim::Unwrap => match self.static_type(&args[0])? {
                    TypeExpr::Option(inner) | TypeExpr::Result { ok: inner, .. } => Some(*inner),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }

    /// Switches on an `Option`/`Result`: continues with the `Some`/`Ok`
    /// payload and branches to the trap block otherwise.
    fn unwrap_value(&mut self, value: ValueRef, ty: &Ty) -> (ValueRef, Ty) {
        let (payload_ty, good, bad) = match ty {
            TypeExpr::Option(element) => {
                ((**element).clone(), BuiltinCase::Some, BuiltinCase::None)
            }
            TypeExpr::Result { ok, .. } => ((**ok).clone(), BuiltinCase::Ok, BuiltinCase::Err),
            other => panic!("{}: unwrap of {other:?}", self.name),
        };
        let site = self.trap_site();
        self.pending.push((value, ty.clone()));
        let (site_value, _) = self.eval(&site);
        let (value, _) = self.pending.pop().expect("pending switch value");
        let trap = self.trap_target();
        let carry = self.threaded(false);
        let carry_values = self.distinct(&carry);
        let next = self.new_block();
        let payload = self.block_param(next, payload_ty.clone());
        let params = self.carry_params(next, &carry);
        let mut good_arguments = vec![SwitchArgument::CasePayload];
        good_arguments.extend(carry_values.iter().map(|v| SwitchArgument::Value(*v)));
        let mut cases = vec![
            SwitchCase {
                case_key: CaseKey::Builtin(good),
                edge: SwitchEdge {
                    target: self.blocks[next].id,
                    arguments: good_arguments,
                },
            },
            SwitchCase {
                case_key: CaseKey::Builtin(bad),
                edge: SwitchEdge {
                    target: self.blocks[trap].id,
                    arguments: vec![SwitchArgument::Value(site_value)],
                },
            },
        ];
        cases.sort_by(|a, b| a.case_key.cmp(&b.case_key));
        self.terminate(Terminator::VariantSwitch(VariantSwitchTerminator {
            value,
            cases,
        }));
        self.current = Some(next);
        self.set_values(&carry, &params);
        (payload, payload_ty)
    }

    fn unit_scope(&mut self) -> ValueRef {
        self.eval(&unitc()).0
    }

    fn bridge(&mut self, row: AdapterImport, scope: ValueRef, request: ValueRef) -> (ValueRef, Ty) {
        let response = row.response_type.clone();
        let id = self.core.import(row);
        let raw_ty = t::res(response, t::failure(BuiltinFailureKind::Index));
        let raw = self.emit(
            Opcode::AdapterInvoke,
            vec![scope, request],
            raw_ty.clone(),
            Immediate::Entity(id),
        );
        self.unwrap_value(raw, &raw_ty)
    }

    #[allow(clippy::too_many_lines)]
    fn prim(&mut self, prim: Prim, args: Vec<(ValueRef, Ty)>) -> (ValueRef, Ty) {
        let types: Vec<Ty> = args.iter().map(|(_, ty)| ty.clone()).collect();
        let values: Vec<ValueRef> = args.iter().map(|(v, _)| *v).collect();
        let checked = |f: &mut Self, opcode: Opcode, unwrap: bool| {
            let ty = types[0].clone();
            if matches!(opcode, Opcode::IntShlChecked | Opcode::IntShrChecked) {
                assert_eq!(
                    types[1],
                    t::u32(),
                    "{}: shift amount must be UInt(32)",
                    f.name
                );
            } else if opcode != Opcode::IntNegChecked {
                assert_eq!(types[0], types[1], "{}: arithmetic operand types", f.name);
            }
            let raw_ty = t::res(ty, t::failure(BuiltinFailureKind::Arithmetic));
            let raw = f.emit(opcode, values.clone(), raw_ty.clone(), Immediate::None);
            if unwrap {
                f.unwrap_value(raw, &raw_ty)
            } else {
                (raw, raw_ty)
            }
        };
        match prim {
            Prim::Add => checked(self, Opcode::IntAddChecked, true),
            Prim::Sub => checked(self, Opcode::IntSubChecked, true),
            Prim::Mul => checked(self, Opcode::IntMulChecked, true),
            Prim::Div => checked(self, Opcode::IntDivChecked, true),
            Prim::Rem => checked(self, Opcode::IntRemChecked, true),
            Prim::Shl => checked(self, Opcode::IntShlChecked, true),
            Prim::Shr => checked(self, Opcode::IntShrChecked, true),
            Prim::Neg => checked(self, Opcode::IntNegChecked, true),
            Prim::TryAdd => checked(self, Opcode::IntAddChecked, false),
            Prim::TrySub => checked(self, Opcode::IntSubChecked, false),
            Prim::TryMul => checked(self, Opcode::IntMulChecked, false),
            Prim::TryShl => checked(self, Opcode::IntShlChecked, false),
            Prim::Eq | Prim::Ne | Prim::Lt | Prim::Le | Prim::Gt | Prim::Ge => {
                assert_eq!(
                    types[0], types[1],
                    "{}: comparison operand types",
                    self.name
                );
                let opcode = match prim {
                    Prim::Eq => Opcode::Equal,
                    Prim::Ne => Opcode::NotEqual,
                    Prim::Lt => Opcode::LessThan,
                    Prim::Le => Opcode::LessEqual,
                    Prim::Gt => Opcode::GreaterThan,
                    _ => Opcode::GreaterEqual,
                };
                (
                    self.emit(opcode, values, t::bool(), Immediate::None),
                    t::bool(),
                )
            }
            Prim::Not => (
                self.emit(Opcode::BoolNot, values, t::bool(), Immediate::None),
                t::bool(),
            ),
            Prim::And => (
                self.emit(Opcode::BoolAnd, values, t::bool(), Immediate::None),
                t::bool(),
            ),
            Prim::Or => (
                self.emit(Opcode::BoolOr, values, t::bool(), Immediate::None),
                t::bool(),
            ),
            Prim::VLen => (
                self.emit(Opcode::VectorLen, values, t::u64(), Immediate::None),
                t::u64(),
            ),
            Prim::VGet | Prim::VGetOpt => {
                let TypeExpr::Vector(element) = &types[0] else {
                    panic!("{}: vget of non-vector", self.name)
                };
                assert_eq!(
                    types[1],
                    t::u64(),
                    "{}: vget index must be UInt(64)",
                    self.name
                );
                let ty = t::opt((**element).clone());
                let raw = self.emit(Opcode::VectorGet, values, ty.clone(), Immediate::None);
                if prim == Prim::VGet {
                    self.unwrap_value(raw, &ty)
                } else {
                    (raw, ty)
                }
            }
            Prim::VSet => {
                let TypeExpr::Vector(element) = &types[0] else {
                    panic!("{}: vset of non-vector", self.name)
                };
                assert_eq!(types[1], t::u64());
                assert_eq!(
                    &types[2],
                    element.as_ref(),
                    "{}: vset element type",
                    self.name
                );
                let raw_ty = t::res(types[0].clone(), t::failure(BuiltinFailureKind::Index));
                let raw = self.emit(Opcode::VectorSet, values, raw_ty.clone(), Immediate::None);
                self.unwrap_value(raw, &raw_ty)
            }
            Prim::MGet | Prim::MGetOpt => {
                let TypeExpr::OrderedMap { key, value } = &types[0] else {
                    panic!("{}: mget of non-map", self.name)
                };
                assert_eq!(&types[1], key.as_ref(), "{}: map key type", self.name);
                let ty = t::opt((**value).clone());
                let raw = self.emit(Opcode::MapGet, values, ty.clone(), Immediate::None);
                if prim == Prim::MGet {
                    self.unwrap_value(raw, &ty)
                } else {
                    (raw, ty)
                }
            }
            Prim::MHas => (
                self.emit(Opcode::MapContains, values, t::bool(), Immediate::None),
                t::bool(),
            ),
            Prim::MIns | Prim::MDel => {
                let opcode = if prim == Prim::MIns {
                    Opcode::MapInsert
                } else {
                    Opcode::MapRemove
                };
                let ty = types[0].clone();
                (self.emit(opcode, values, ty.clone(), Immediate::None), ty)
            }
            Prim::VHash => (
                self.emit(Opcode::ValueHash, values, t::bytes(), Immediate::None),
                t::bytes(),
            ),
            Prim::B2V
            | Prim::V2B
            | Prim::RawBlake3
            | Prim::RawBlake3Large
            | Prim::Sha256
            | Prim::Concat
            | Prim::BytesLen => {
                let (code, request, response) = match prim {
                    Prim::BytesLen => (*b"BLN1", t::bytes(), t::u64()),
                    Prim::B2V => (*b"B2V1", t::bytes(), t::vec(t::u8())),
                    Prim::V2B => (*b"V2B1", t::vec(t::u8()), t::bytes()),
                    Prim::RawBlake3 => (*b"RHW1", t::bytes(), t::bytes()),
                    Prim::RawBlake3Large => (*b"RHW2", t::bytes(), t::bytes()),
                    Prim::Sha256 => (*b"RSH2", t::bytes(), t::bytes()),
                    _ => (*b"BCT1", t::vec(t::bytes()), t::bytes()),
                };
                assert_eq!(types[0], request, "{}: bridge request type", self.name);
                self.pending.push(args[0].clone());
                let scope = self.unit_scope();
                let (request_value, _) = self.pending.pop().expect("pending request");
                self.bridge(bridge_row(code, 0, request, response), scope, request_value)
            }
            Prim::Push => {
                let TypeExpr::Vector(element) = &types[0] else {
                    panic!("{}: push onto non-vector", self.name)
                };
                assert_eq!(
                    &types[1],
                    element.as_ref(),
                    "{}: push element type",
                    self.name
                );
                let discriminator = self.core.push_discriminator(element);
                let row = bridge_row(
                    *b"PSH1",
                    discriminator,
                    (**element).clone(),
                    types[0].clone(),
                );
                self.bridge(row, values[0], values[1])
            }
            Prim::Slice => {
                assert_eq!(types[0], t::bytes(), "{}: slice of non-Bytes", self.name);
                assert_eq!(types[1], t::u64());
                assert_eq!(types[2], t::u64());
                let range_ty = t::tuple(vec![t::u64(), t::u64()]);
                let range = self.emit(
                    Opcode::TupleNew,
                    vec![values[1], values[2]],
                    range_ty.clone(),
                    Immediate::None,
                );
                let row = bridge_row(*b"BSL1", 0, range_ty, t::bytes());
                self.bridge(row, values[0], range)
            }
            Prim::Unwrap => self.unwrap_value(values[0], &types[0]),
            Prim::IsSome | Prim::IsOk => {
                let keys = match (&types[0], prim) {
                    (TypeExpr::Option(_), Prim::IsSome) => [
                        CaseKey::Builtin(BuiltinCase::None),
                        CaseKey::Builtin(BuiltinCase::Some),
                    ],
                    (TypeExpr::Result { .. }, Prim::IsOk) => [
                        CaseKey::Builtin(BuiltinCase::Ok),
                        CaseKey::Builtin(BuiltinCase::Err),
                    ],
                    _ => panic!("{}: is_some/is_ok type", self.name),
                };
                let hits = if prim == Prim::IsSome {
                    [false, true]
                } else {
                    [true, false]
                };
                let arms = hits
                    .iter()
                    .map(|hit| {
                        let hit = *hit;
                        ArmSpec {
                            payload: None,
                            body: Box::new(move |f: &mut Self| Some(f.eval(&boolc(hit)))),
                        }
                    })
                    .collect();
                let value = values[0];
                let result = self
                    .split(
                        |f, targets, carried| f.switch_term(value, &keys, targets, carried),
                        arms,
                        Some(t::bool()),
                    )
                    .expect("test joins");
                (result, t::bool())
            }
        }
    }

    /// Removes block parameters whose values are never read (transitively
    /// through edge arguments), and the matching edge arguments.
    #[allow(clippy::too_many_lines)] // one linear construction; splitting it hides the order
    fn prune_dead_params(&mut self) {
        use std::collections::{HashMap as Map, HashSet as Set};
        let param_of = |value: &ValueRef, params: &Set<EntityId>| match value {
            ValueRef::Parameter(id) if params.contains(id) => Some(*id),
            _ => None,
        };
        let block_index: Map<EntityId, usize> = self
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let mut live: Set<EntityId> = Set::new();
        let mut work: Vec<EntityId> = Vec::new();
        let mark = |id: Option<EntityId>, live: &mut Set<EntityId>, work: &mut Vec<EntityId>| {
            if let Some(id) = id
                && live.insert(id)
            {
                work.push(id);
            }
        };
        for operation in &self.operations {
            for operand in &operation.operands {
                mark(param_of(operand, &self.block_params), &mut live, &mut work);
            }
        }
        // Edges as (target block, argument list as Option<ValueRef>).
        let mut edges: Vec<(usize, Vec<Option<ValueRef>>)> = Vec::new();
        for block in &self.blocks {
            match block.terminator.as_ref().expect("terminated") {
                Terminator::Return(r) => {
                    mark(param_of(&r.value, &self.block_params), &mut live, &mut work);
                }
                Terminator::Trap(trap) => {
                    if let Some(payload) = &trap.payload {
                        mark(param_of(payload, &self.block_params), &mut live, &mut work);
                    }
                }
                Terminator::Branch(b) => edges.push((
                    block_index[&b.edge.target],
                    b.edge.arguments.iter().copied().map(Some).collect(),
                )),
                Terminator::CondBranch(c) => {
                    mark(
                        param_of(&c.condition, &self.block_params),
                        &mut live,
                        &mut work,
                    );
                    for edge in [&c.if_true, &c.if_false] {
                        edges.push((
                            block_index[&edge.target],
                            edge.arguments.iter().copied().map(Some).collect(),
                        ));
                    }
                }
                Terminator::VariantSwitch(sw) => {
                    mark(
                        param_of(&sw.value, &self.block_params),
                        &mut live,
                        &mut work,
                    );
                    for case in &sw.cases {
                        edges.push((
                            block_index[&case.edge.target],
                            case.edge
                                .arguments
                                .iter()
                                .map(|a| match a {
                                    SwitchArgument::Value(v) => Some(*v),
                                    SwitchArgument::CasePayload => None,
                                })
                                .collect(),
                        ));
                    }
                }
            }
        }
        // Reverse map: target param -> source values feeding it.
        let mut feeds: Map<EntityId, Vec<EntityId>> = Map::new();
        for (target, arguments) in &edges {
            for (position, argument) in arguments.iter().enumerate() {
                let Some(target_param) = self.blocks[*target].params.get(position) else {
                    continue;
                };
                if let Some(source) = argument
                    .as_ref()
                    .and_then(|a| param_of(a, &self.block_params))
                {
                    feeds.entry(*target_param).or_default().push(source);
                }
            }
        }
        while let Some(id) = work.pop() {
            if let Some(sources) = feeds.get(&id) {
                for source in sources.clone() {
                    mark(Some(source), &mut live, &mut work);
                }
            }
        }
        // Positions to keep per block.
        let keep: Vec<Vec<bool>> = self
            .blocks
            .iter()
            .map(|b| b.params.iter().map(|p| live.contains(p)).collect())
            .collect();
        let filter = |arguments: &mut Vec<ValueRef>, keep: &[bool]| {
            let mut position = 0;
            arguments.retain(|_| {
                let kept = keep[position];
                position += 1;
                kept
            });
        };
        for index in 0..self.blocks.len() {
            let mut terminator = self.blocks[index].terminator.take().expect("terminated");
            match &mut terminator {
                Terminator::Branch(b) => {
                    filter(&mut b.edge.arguments, &keep[block_index[&b.edge.target]]);
                }
                Terminator::CondBranch(c) => {
                    filter(
                        &mut c.if_true.arguments,
                        &keep[block_index[&c.if_true.target]],
                    );
                    filter(
                        &mut c.if_false.arguments,
                        &keep[block_index[&c.if_false.target]],
                    );
                }
                Terminator::VariantSwitch(sw) => {
                    for case in &mut sw.cases {
                        let keep = &keep[block_index[&case.edge.target]];
                        let mut position = 0;
                        case.edge.arguments.retain(|_| {
                            let kept = keep[position];
                            position += 1;
                            kept
                        });
                    }
                }
                Terminator::Return(_) | Terminator::Trap(_) => {}
            }
            self.blocks[index].terminator = Some(terminator);
        }
        for block in &mut self.blocks {
            block.params.retain(|p| live.contains(p));
        }
        let ordinals: Map<EntityId, u32> = self
            .blocks
            .iter()
            .flat_map(|b| {
                b.params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (*p, u32::try_from(i).expect("ordinal fits u32")))
            })
            .collect();
        let block_params = &self.block_params;
        self.parameters.retain_mut(|p| {
            if !block_params.contains(&p.entity_id) {
                return true;
            }
            match ordinals.get(&p.entity_id) {
                Some(ordinal) => {
                    p.ordinal = *ordinal;
                    true
                }
                None => false,
            }
        });
    }

    fn finish(mut self) {
        assert!(
            self.current.is_none(),
            "{}: function falls off its end without a terminator",
            self.name
        );
        self.prune_dead_params();
        let dead = std::mem::take(&mut self.dead_blocks);
        let blocks: Vec<Block> = self
            .blocks
            .drain(..)
            .enumerate()
            .map(|(index, block)| Block {
                entity_id: block.id,
                function: self.function,
                parameters: block.params,
                operations: block.ops,
                terminator: block
                    .terminator
                    .unwrap_or_else(|| panic!("{}: unterminated block", self.name)),
                reachability: if dead.contains(&index) {
                    Reachability::ExplicitlyUnreachable
                } else {
                    Reachability::Required
                },
            })
            .collect();
        let graph = FunctionGraph {
            entity_id: self.function,
            type_parameters: Vec::new(),
            parameters: self.params.clone(),
            result_type: self.result.clone(),
            effects: Vec::new(),
            entry_block: blocks[0].entity_id,
            blocks: blocks.iter().map(|b| b.entity_id).collect(),
            contracts: Vec::new(),
            visibility: Visibility::Private,
        };
        self.core.out.functions.push(graph);
        self.core.out.blocks.extend(blocks);
        self.core.out.parameters.append(&mut self.parameters);
        self.core.out.operations.append(&mut self.operations);
    }
}

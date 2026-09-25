#![allow(unsafe_code)]
#![no_main]

use core::slice;

use sley_check::{TypeEnvironment, TypeError, TypeErrorCode, TypeTraits};
use sley_id::EntityId;
use sley_ssmc::{
    BuiltinFailureKind, FunctionType, IntegerWidth, MemberId, NamedType, RecordField, TypeDefForm,
    TypeDefinition, TypeExpr, TypeParameterDef, VariantCase, Visibility,
};

const MAX_FUZZ_INPUT_BYTES: usize = 4096;
const MAX_DEFINITIONS: usize = 8;
const MAX_DEFINITION_MEMBERS: usize = 6;
const MAX_TYPE_ARGUMENTS: usize = 4;
const MAX_TYPE_DEPTH: usize = 8;
const MAX_GENERATED_TYPE_NODES: usize = 512;

#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn LLVMFuzzerTestOneInput(data: *const u8, len: usize) -> i32 {
    if len == 0 {
        return 0;
    }
    let input = unsafe { slice::from_raw_parts(data, len) };
    fuzz_one(input);
    0
}

fn fuzz_one(input: &[u8]) {
    if input.len() > MAX_FUZZ_INPUT_BYTES {
        return;
    }

    let mut cursor = Cursor::new(input);
    let mut budget = TypeBudget::new(MAX_GENERATED_TYPE_NODES);
    let definition_count = cursor.bounded(MAX_DEFINITIONS);
    let definitions = (0..definition_count)
        .map(|index| generate_definition(&mut cursor, &mut budget, index))
        .collect::<Vec<_>>();
    let value_type = generate_type(&mut cursor, &mut budget, 0);
    let parameter_count = u32::from(cursor.byte() % 5);
    let arguments = (0..cursor.bounded(MAX_TYPE_ARGUMENTS))
        .map(|_| generate_type(&mut cursor, &mut budget, 0))
        .collect::<Vec<_>>();

    let first = observe(
        definitions.clone(),
        &value_type,
        parameter_count,
        &arguments,
    );
    let second = observe(definitions, &value_type, parameter_count, &arguments);
    assert_eq!(first, second, "type-checker judgment was not deterministic");
    check_consistency(&value_type, &first);
}

/// Cross-judgment consistency: each field accepts exactly its domain, and
/// the judgments cohere. A wrong neighboring code (e.g. `ConstShape` where
/// only `NotOrderable` is contractually possible, or `Ok` where the
/// contract requires `Err`) fails here instead of passing as determinism.
fn check_consistency(value_type: &TypeExpr, outcome: &TypeOutcome) {
    let TypeOutcome::Checked {
        check,
        traits,
        orderable,
        hashable,
        persistable,
        ..
    } = outcome
    else {
        // Environment construction vs judgment failure are distinct classes
        // by construction of the outcome enum; nothing further to cohere.
        return;
    };
    if orderable.is_ok() {
        let judged = traits
            .as_ref()
            .expect("require_orderable passed while traits failed");
        assert!(
            judged.total_order,
            "require_orderable passed for a non-orderable trait set"
        );
    }
    if hashable.is_ok() {
        let judged = traits
            .as_ref()
            .expect("require_hashable passed while traits failed");
        assert!(
            judged.canonical_hash,
            "require_hashable passed for a non-hashable trait set"
        );
    }
    if persistable.is_ok() {
        let judged = traits
            .as_ref()
            .expect("require_persistable passed while traits failed");
        assert!(
            judged.persistable,
            "require_persistable passed for a non-persistable trait set"
        );
    }
    // `traits` re-checks closed well-formedness first, so a checked closed
    // type must carry traits. Sound because `check_type` is
    // `check_type_inner` plus `check_map_keys` over the same definition
    // fields/payloads at the same depth accounting (and deeper on
    // FunctionRef): any DepthLimit `traits_inner` could reach is returned
    // first by `check`, so the assert below is never entered on that
    // ground; the only legal `Err` here is a trait-level refusal, never
    // a well-formedness code the check already cleared.
    if check.is_ok() && is_closed(value_type) {
        assert!(
            traits.is_ok(),
            "a checked closed type failed traits: {:?}",
            traits.as_ref().err(),
        );
    }
}

#[derive(Debug, Eq, PartialEq)]
enum TypeOutcome {
    Environment(TypeErrorCode),
    Checked {
        definition_ids: Vec<EntityId>,
        check: Result<(), TypeErrorCode>,
        traits: Result<TypeTraits, TypeErrorCode>,
        orderable: Result<(), TypeErrorCode>,
        hashable: Result<(), TypeErrorCode>,
        persistable: Result<(), TypeErrorCode>,
        instantiated: Result<TypeExpr, TypeErrorCode>,
    },
}

fn observe(
    definitions: Vec<TypeDefinition>,
    value_type: &TypeExpr,
    parameter_count: u32,
    arguments: &[TypeExpr],
) -> TypeOutcome {
    let environment = match TypeEnvironment::new(definitions) {
        Ok(environment) => environment,
        Err(error) => return TypeOutcome::Environment(error.code()),
    };
    let checked = TypeOutcome::Checked {
        definition_ids: environment.definition_ids().collect(),
        check: error_code(environment.check_type(value_type, parameter_count)),
        traits: error_code(environment.traits(value_type)),
        orderable: error_code(environment.require_orderable(value_type)),
        hashable: error_code(environment.require_hashable(value_type)),
        persistable: error_code(environment.require_persistable(value_type)),
        instantiated: error_code(environment.instantiate_in_scope(
            value_type,
            arguments,
            parameter_count,
        )),
    };
    // Substitution preserves named-argument counts, so checked arguments of
    // matching length substituted into a checked type cannot fail arity:
    // the wrong neighboring code would be a conformance illusion.
    if let TypeOutcome::Checked {
        check: Ok(()),
        instantiated,
        ..
    } = &checked
    {
        let arity_matches =
            usize::try_from(parameter_count).unwrap_or(usize::MAX) == arguments.len();
        let args_checked = arguments
            .iter()
            .all(|argument| environment.check_type(argument, parameter_count).is_ok());
        if arity_matches && args_checked {
            assert_ne!(
                instantiated.as_ref().err(),
                Some(&TypeErrorCode::ArgumentArity),
                "checked substitution of matching length failed arity"
            );
        }
    }
    checked
}

fn error_code<T>(result: Result<T, TypeError>) -> Result<T, TypeErrorCode> {
    result.map_err(|error| error.code())
}

/// A type with no free `TypeParameter` nodes: its judgment cannot depend on
/// the caller parameter scope, so a passed check binds closed traits too.
fn is_closed(value: &TypeExpr) -> bool {
    match value {
        TypeExpr::Unit
        | TypeExpr::Bool
        | TypeExpr::SInt(_)
        | TypeExpr::UInt(_)
        | TypeExpr::F32
        | TypeExpr::F64
        | TypeExpr::Bytes
        | TypeExpr::Text
        | TypeExpr::AdapterHandle(_)
        | TypeExpr::CapabilityToken(_)
        | TypeExpr::BuiltinFailure(_) => true,
        TypeExpr::TypeParameter(_) => false,
        TypeExpr::Tuple(values) => values.iter().all(is_closed),
        TypeExpr::Named(named) => named.arguments.iter().all(is_closed),
        TypeExpr::Vector(inner)
        | TypeExpr::Option(inner)
        | TypeExpr::LocalCell(inner) => is_closed(inner),
        TypeExpr::OrderedMap { key, value } => is_closed(key) && is_closed(value),
        TypeExpr::Result { ok, error } => is_closed(ok) && is_closed(error),
        TypeExpr::FunctionRef(function) => {
            function.parameters.iter().all(is_closed) && is_closed(&function.result)
        }
    }
}

fn generate_definition(
    cursor: &mut Cursor<'_>,
    budget: &mut TypeBudget,
    index: usize,
) -> TypeDefinition {
    let entity_id = if cursor.byte().is_multiple_of(2) {
        id(u32::try_from(index + 1).unwrap_or(u32::MAX))
    } else {
        id(cursor.u32())
    };
    let parameter_count = cursor.bounded(MAX_TYPE_ARGUMENTS);
    let type_parameters = (0..parameter_count)
        .map(|ordinal| TypeParameterDef {
            ordinal: if cursor.byte().is_multiple_of(3) {
                u32::try_from(ordinal).unwrap_or(u32::MAX)
            } else {
                u32::from(cursor.byte() % 8)
            },
        })
        .collect();
    let member_count = cursor.bounded(MAX_DEFINITION_MEMBERS);
    let form = if cursor.byte().is_multiple_of(2) {
        TypeDefForm::Record(
            (0..member_count)
                .map(|member| RecordField {
                    member_id: member_id(cursor, member),
                    value_type: generate_type(cursor, budget, 0),
                    visibility: visibility(cursor.byte()),
                })
                .collect(),
        )
    } else {
        TypeDefForm::Variant(
            (0..member_count)
                .map(|member| VariantCase {
                    member_id: member_id(cursor, member),
                    payload_type: cursor
                        .byte()
                        .is_multiple_of(2)
                        .then(|| generate_type(cursor, budget, 0)),
                })
                .collect(),
        )
    };
    let invariants = (0..cursor.bounded(4)).map(|_| id(cursor.u32())).collect();
    TypeDefinition {
        entity_id,
        type_parameters,
        form,
        invariants,
        visibility: visibility(cursor.byte()),
    }
}

fn generate_type(cursor: &mut Cursor<'_>, budget: &mut TypeBudget, depth: usize) -> TypeExpr {
    if depth >= MAX_TYPE_DEPTH || !budget.take() {
        return leaf_type(cursor);
    }
    match cursor.byte() % 20 {
        0 => TypeExpr::Unit,
        1 => TypeExpr::Bool,
        2 => TypeExpr::SInt(integer_width(cursor)),
        3 => TypeExpr::UInt(integer_width(cursor)),
        4 => TypeExpr::F32,
        5 => TypeExpr::F64,
        6 => TypeExpr::Bytes,
        7 => TypeExpr::Text,
        8 => TypeExpr::Tuple(
            (0..cursor.bounded(MAX_TYPE_ARGUMENTS))
                .map(|_| generate_type(cursor, budget, depth + 1))
                .collect(),
        ),
        9 => TypeExpr::Named(NamedType {
            definition: id(cursor.u32()),
            arguments: (0..cursor.bounded(MAX_TYPE_ARGUMENTS))
                .map(|_| generate_type(cursor, budget, depth + 1))
                .collect(),
        }),
        10 => TypeExpr::Vector(Box::new(generate_type(cursor, budget, depth + 1))),
        11 => TypeExpr::OrderedMap {
            key: Box::new(generate_type(cursor, budget, depth + 1)),
            value: Box::new(generate_type(cursor, budget, depth + 1)),
        },
        12 => TypeExpr::Option(Box::new(generate_type(cursor, budget, depth + 1))),
        13 => TypeExpr::Result {
            ok: Box::new(generate_type(cursor, budget, depth + 1)),
            error: Box::new(generate_type(cursor, budget, depth + 1)),
        },
        14 => TypeExpr::FunctionRef(FunctionType {
            parameters: (0..cursor.bounded(MAX_TYPE_ARGUMENTS))
                .map(|_| generate_type(cursor, budget, depth + 1))
                .collect(),
            result: Box::new(generate_type(cursor, budget, depth + 1)),
            effects: (0..cursor.bounded(4)).map(|_| id(cursor.u32())).collect(),
        }),
        15 => TypeExpr::AdapterHandle(id(cursor.u32())),
        16 => TypeExpr::CapabilityToken(id(cursor.u32())),
        17 => TypeExpr::LocalCell(Box::new(generate_type(cursor, budget, depth + 1))),
        18 => TypeExpr::TypeParameter(u32::from(cursor.byte() % 8)),
        19 => TypeExpr::BuiltinFailure(builtin_failure(cursor.byte())),
        _ => unreachable!(),
    }
}

struct TypeBudget {
    remaining: usize,
}

impl TypeBudget {
    const fn new(remaining: usize) -> Self {
        Self { remaining }
    }

    fn take(&mut self) -> bool {
        let Some(remaining) = self.remaining.checked_sub(1) else {
            return false;
        };
        self.remaining = remaining;
        true
    }
}

fn leaf_type(cursor: &mut Cursor<'_>) -> TypeExpr {
    match cursor.byte() % 9 {
        0 => TypeExpr::Unit,
        1 => TypeExpr::Bool,
        2 => TypeExpr::SInt(integer_width(cursor)),
        3 => TypeExpr::UInt(integer_width(cursor)),
        4 => TypeExpr::F32,
        5 => TypeExpr::F64,
        6 => TypeExpr::Bytes,
        7 => TypeExpr::Text,
        8 => TypeExpr::TypeParameter(u32::from(cursor.byte() % 8)),
        _ => unreachable!(),
    }
}

fn integer_width(cursor: &mut Cursor<'_>) -> IntegerWidth {
    let bits = match cursor.byte() % 8 {
        0 => 8,
        1 => 16,
        2 => 32,
        3 => 64,
        4 => 128,
        _ => cursor.u16(),
    };
    IntegerWidth::from_bits(bits)
}

fn visibility(value: u8) -> Visibility {
    match value % 4 {
        0 => Visibility::Private,
        1 => Visibility::Package,
        2 => Visibility::Workspace,
        3 => Visibility::Exported,
        _ => unreachable!(),
    }
}

fn builtin_failure(value: u8) -> BuiltinFailureKind {
    match value % 5 {
        0 => BuiltinFailureKind::Arithmetic,
        1 => BuiltinFailureKind::Index,
        2 => BuiltinFailureKind::DuplicateKey,
        3 => BuiltinFailureKind::ContractViolation,
        4 => BuiltinFailureKind::Capability,
        _ => unreachable!(),
    }
}

fn member_id(cursor: &mut Cursor<'_>, index: usize) -> MemberId {
    let value = if cursor.byte().is_multiple_of(2) {
        u32::try_from(index + 1).unwrap_or(u32::MAX)
    } else {
        cursor.u32()
    };
    MemberId::from_bytes(expand(value))
}

fn id(value: u32) -> EntityId {
    EntityId::from_bytes(expand(value))
}

fn expand(value: u32) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (offset, chunk) in bytes.chunks_exact_mut(4).enumerate() {
        let mixed = value.wrapping_add(u32::try_from(offset).unwrap_or(0));
        chunk.copy_from_slice(&mixed.to_be_bytes());
    }
    bytes
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn byte(&mut self) -> u8 {
        let value = self.input[self.offset % self.input.len()];
        self.offset = self.offset.wrapping_add(1);
        value
    }

    fn u16(&mut self) -> u16 {
        u16::from_be_bytes([self.byte(), self.byte()])
    }

    fn u32(&mut self) -> u32 {
        u32::from_be_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }

    fn bounded(&mut self, maximum: usize) -> usize {
        usize::from(self.byte()) % (maximum + 1)
    }
}

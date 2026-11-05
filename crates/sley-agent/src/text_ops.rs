//! The native text entries (VM profile slice E9) as the agent spells them.
//!
//! An `adapter` operation (opcode 161) whose entity argument is one of the
//! names below invokes that text entry. The frame reuses a live or
//! frame-created `AdapterImport` row carrying the entry's frozen adapter
//! identity and exact row, and creates one under the name otherwise. The
//! operands are the entry's scope and request; the result is
//! `Result<response, IndexError>`.

use sley_mutate::value::{AdapterImportBody, EntityBodyValue, EntityIdSet};
use sley_ssmc::{BuiltinFailureKind, TypeExpr};
use sley_vm::text::{TextKind, text_adapter_id, text_row_types};

/// The agent names of the text entries.
pub(crate) const TEXT_OPS: [(&str, TextKind); 7] = [
    ("text_len", TextKind::Length),
    ("text_slice", TextKind::Slice),
    ("text_concat", TextKind::Concat),
    ("text_scalars", TextKind::Scalars),
    ("text_ord", TextKind::Ord),
    ("text_chr", TextKind::Chr),
    ("text_from_int", TextKind::FromInt),
];

/// The text entry an agent name spells.
pub(crate) fn by_name(name: &str) -> Option<TextKind> {
    TEXT_OPS
        .iter()
        .find(|(word, _)| *word == name)
        .map(|(_, kind)| *kind)
}

/// The agent name of a text entry.
pub(crate) fn name_of(kind: TextKind) -> &'static str {
    TEXT_OPS
        .iter()
        .find(|(_, k)| *k == kind)
        .map_or("text_len", |(word, _)| word)
}

/// The invocation result type: `Result<response, IndexError>`.
pub(crate) fn result_type(kind: TextKind) -> TypeExpr {
    let (_, _, response) = text_row_types(kind);
    TypeExpr::Result {
        ok: Box::new(response),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
    }
}

/// The operand types: the scope, then the request.
pub(crate) fn operand_types(kind: TextKind) -> [TypeExpr; 2] {
    let (scope, request, _) = text_row_types(kind);
    [scope, request]
}

/// The frozen row of a text entry.
pub(crate) fn body(kind: TextKind) -> EntityBodyValue {
    let (_, request, response) = text_row_types(kind);
    EntityBodyValue::AdapterImport(AdapterImportBody {
        adapter_id: text_adapter_id(kind),
        abi_version: 1,
        request_type: request,
        response_type: response,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: EntityIdSet::from_unsorted(Vec::new()).expect("the empty set"),
    })
}

/// The text entry a body is exactly the row of.
pub(crate) fn of_body(body: &EntityBodyValue) -> Option<TextKind> {
    let EntityBodyValue::AdapterImport(import) = body else {
        return None;
    };
    let kind = sley_check::text_rows::text_kind(&import.adapter_id)?;
    (*body == self::body(kind)).then_some(kind)
}

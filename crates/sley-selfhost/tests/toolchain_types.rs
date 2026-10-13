//! `ty_*` constructors and accessors against native encodings.
mod common;
use common::*;
use sley_selfhost::seed::t;
use sley_selfhost::toolchain::types::scb_type_bytes;
use sley_ssmc::{BuiltinFailureKind, NamedType, TypeExpr};

#[test]
fn constructs_and_reads_canonical_types() {
    let def = sley_id::EntityId::from_bytes([5; 32]);
    let out = run("t_ty_build", vec![bytes(def.as_bytes())]);
    let items = tuple_items(&out);
    let named = TypeExpr::Named(NamedType {
        definition: def,
        arguments: vec![],
    });
    assert_eq!(
        as_bytes(&items[0]),
        scb_type_bytes(&t::res(
            t::vec(t::u64()),
            TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)
        ))
    );
    assert_eq!(
        as_bytes(&items[1]),
        scb_type_bytes(&t::tuple(vec![TypeExpr::Bool, named]))
    );
    assert_eq!(
        as_bytes(&items[2]),
        scb_type_bytes(&t::map(t::u64(), t::opt(t::u64())))
    );
    assert_eq!(as_bytes(&items[3]), scb_type_bytes(&t::vec(t::u64())));
    assert_eq!(as_bytes(&items[4]), def.as_bytes().to_vec());
    assert_eq!(as_bytes(&items[5]), vec![64]);
    assert_eq!(as_bytes(&items[6]), vec![12]);
}

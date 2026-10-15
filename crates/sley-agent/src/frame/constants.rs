//! The ordinary const immediate type rule, independent of constant allocation.
use crate::{error::Result, types, types::TypeNames};
use serde_json::Value;
use sley_ssmc::{IntegerWidth, TypeExpr};
use std::collections::BTreeMap;

pub(crate) fn immediate_type(
    value: &Value,
    hint: Option<&TypeExpr>,
    names: &impl TypeNames,
    at: &str,
) -> Result<TypeExpr> {
    match value {
        Value::Object(object) if object.contains_key("value") => {
            types::read(object.get("type").unwrap_or(&Value::from("i64")), names, at)
        }
        Value::Bool(_) => Ok(TypeExpr::Bool),
        _ => Ok(hint
            .filter(|ty| matches!(ty, TypeExpr::SInt(_) | TypeExpr::UInt(_)))
            .cloned()
            .unwrap_or(TypeExpr::SInt(IntegerWidth::from_bits(64)))),
    }
}

/// The ordinary use-hint accumulator keeps the first use in emitted block order.
pub(crate) fn hint<K: Ord>(hints: &mut BTreeMap<K, TypeExpr>, key: K, ty: &TypeExpr) {
    hints.entry(key).or_insert_with(|| ty.clone());
}

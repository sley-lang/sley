//! Consuming family construction and bounded canonicalization scratch.

pub(super) mod digest;
mod retained;

use serde_json::{Map, Value};
use std::sync::Arc;

use super::{
    Budget, Family, MAX_DESCRIPTIONS, MAX_FIELDS, encoding, failure, invalid, limit, read_field,
};
use crate::error::{AgentErrorCode, Result};

pub(super) fn from_value(value: Value, budget: &mut Budget) -> Result<Family> {
    let Value::Object(mut object) = value else {
        return Err(invalid("family must be an object"));
    };
    if object.len() != 2 || !object.contains_key("fields") || !object.contains_key("descriptions") {
        return Err(invalid("family requires exactly fields and descriptions"));
    }
    let Value::Array(field_values) = object.remove("fields").expect("checked") else {
        return Err(invalid("fields must be an array"));
    };
    let Value::Array(row_values) = object.remove("descriptions").expect("checked") else {
        return Err(invalid("descriptions must be an array"));
    };
    if field_values.len() > MAX_FIELDS || row_values.len() > MAX_DESCRIPTIONS {
        return Err(limit("finite family exceeds 64 fields or 256 descriptions"));
    }
    if row_values.is_empty() {
        return Err(failure(
            AgentErrorCode::ResidualFamilyEmpty,
            "family has no descriptions",
        ));
    }
    let retained_memory = retained::reserve(&field_values, &row_values, budget)?;
    let mut fields = Vec::with_capacity(field_values.len());
    for value in &field_values {
        fields.push(read_field(value)?);
    }
    drop(field_values);
    fields.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    if fields.windows(2).any(|pair| pair[0].name == pair[1].name) {
        return Err(invalid("duplicate field name"));
    }
    let keys_memory =
        budget.reserve::<(encoding::Encoded, Map<String, Value>)>(row_values.len())?;
    let mut keyed = Vec::with_capacity(row_values.len());
    for row in row_values {
        budget.checkpoint()?;
        let Value::Object(row) = row else {
            return Err(invalid("description must be an object"));
        };
        if row.len() != fields.len() || fields.iter().any(|field| !row.contains_key(&field.name)) {
            return Err(failure(
                AgentErrorCode::ResidualFamilyIncomplete,
                "every description must contain exactly the declared fields",
            ));
        }
        let key = encoding::encode(&row, budget, crate::residual::MAX_REQUEST_BYTES)?;
        keyed.push((key, row));
    }
    keyed.sort_unstable_by(|left, right| left.0.bytes.cmp(&right.0.bytes));
    if keyed
        .windows(2)
        .any(|pair| pair[0].0.bytes == pair[1].0.bytes)
    {
        return Err(failure(
            AgentErrorCode::ResidualConstraintConflict,
            "duplicate complete descriptions",
        ));
    }
    // Only keys are discarded. Every nested row value is moved, never cloned.
    let mut rows = Vec::with_capacity(keyed.len());
    rows.extend(keyed.into_iter().map(|(_, row)| row));
    drop(keys_memory);
    let digest = digest::family(&fields, &rows, budget)?;
    budget.checkpoint()?;
    Ok(Family {
        fields: Arc::new(fields),
        rows: Arc::new(rows),
        digest,
        _memory: Arc::new(retained_memory),
    })
}

#[cfg(test)]
mod tests;

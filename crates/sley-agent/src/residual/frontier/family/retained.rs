//! Observable payload capacity adopted by a family. Map nodes and parser peak
//! allocations are deliberately not estimated here.

use serde_json::{Map, Value};

use super::super::{Budget, Field, memory::Reservation};
use crate::error::Result;

pub(super) fn reserve(
    fields: &[Value],
    rows: &[Value],
    budget: &mut Budget,
) -> Result<Reservation> {
    budget.checkpoint()?;
    let mut bytes = fields.len().checked_mul(size_of::<Field>());
    add(
        &mut bytes,
        rows.len().checked_mul(size_of::<Map<String, Value>>()),
    );
    for field in fields {
        budget.charge(1)?;
        // read_field copies these names into exactly requested String storage.
        add(&mut bytes, Some(field["name"].as_str().map_or(0, str::len)));
    }
    for row in rows {
        payload(row, &mut bytes, budget)?;
    }
    budget.checkpoint()?;
    budget.memory.reserve(bytes)
}

fn add(total: &mut Option<usize>, bytes: Option<usize>) {
    *total = total.and_then(|total| bytes.and_then(|bytes| total.checked_add(bytes)));
}

fn payload(value: &Value, bytes: &mut Option<usize>, budget: &mut Budget) -> Result<()> {
    budget.charge(1)?;
    match value {
        Value::String(text) => add(bytes, Some(text.capacity())),
        Value::Array(values) => {
            add(bytes, values.capacity().checked_mul(size_of::<Value>()));
            for value in values {
                payload(value, bytes, budget)?;
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                add(bytes, Some(key.capacity()));
                payload(value, bytes, budget)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

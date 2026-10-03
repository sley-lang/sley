//! Natural joins retain references into the immutable constraint tables.
//! All row/index buffers reserve capacity from the same invocation budget.

use serde::{Serialize, Serializer, ser::SerializeMap};
use serde_json::{Map, Value};

use super::super::{Budget, MAX_DESCRIPTIONS, encoding, failure, limit, memory::Reservation};
use crate::error::{AgentErrorCode, Result};

type Entry<'a> = (&'a String, &'a Value);

pub(super) struct Row<'a> {
    entries: Vec<Entry<'a>>,
    _memory: Reservation,
}

impl Serialize for Row<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;
        for (key, value) in &self.entries {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl Row<'_> {
    fn get(&self, key: &String) -> Option<&Value> {
        self.entries
            .binary_search_by(|(name, _)| (*name).cmp(key))
            .ok()
            .map(|index| self.entries[index].1)
    }
}

pub(super) struct Rows<'a> {
    pub values: Vec<Row<'a>>,
    _memory: Reservation,
}

impl Rows<'_> {
    pub fn identity(budget: &mut Budget) -> Result<Self> {
        let memory = budget.reserve::<Row<'_>>(1)?;
        let values = vec![Row {
            entries: Vec::new(),
            _memory: budget.reserve::<Entry<'_>>(0)?,
        }];
        Ok(Self {
            values,
            _memory: memory,
        })
    }
}

fn sizes<T: Serialize>(rows: &[T], budget: &mut Budget) -> Result<Vec<usize>> {
    let mut sizes = Vec::with_capacity(rows.len());
    for row in rows {
        sizes.push(encoding::size(
            row,
            budget,
            crate::residual::MAX_REQUEST_BYTES,
        )?);
    }
    Ok(sizes)
}

pub(super) fn join<'a>(
    left: &Rows<'a>,
    right: &'a [Map<String, Value>],
    budget: &mut Budget,
) -> Result<Rows<'a>> {
    let _sizes = budget.reserve::<usize>(left.values.len() + right.len())?;
    let left_sizes = sizes(&left.values, budget)?;
    let right_sizes = sizes(right, budget)?;
    let capacity = left
        .values
        .len()
        .checked_mul(right.len())
        .ok_or_else(|| limit("constraint join capacity overflow"))?
        .min(MAX_DESCRIPTIONS);
    let memory = budget.reserve::<Row<'_>>(capacity)?;
    let mut joined = Rows {
        values: Vec::with_capacity(capacity),
        _memory: memory,
    };
    let mut joined_bytes = 0_usize;
    for (lhs, lhs_bytes) in left.values.iter().zip(left_sizes) {
        for (rhs, rhs_bytes) in right.iter().zip(&right_sizes) {
            budget.charge(lhs.entries.len() + rhs.len() + 1)?;
            if !rhs
                .iter()
                .all(|(name, value)| lhs.get(name).is_none_or(|old| old == value))
            {
                continue;
            }
            if joined.values.len() == MAX_DESCRIPTIONS {
                return Err(limit(
                    "coupled constraint join exceeds 256 rows; use explicit authoring",
                ));
            }
            // Preserve the existing conservative serialized-size ceiling.
            joined_bytes = joined_bytes
                .checked_add(lhs_bytes + rhs_bytes)
                .filter(|bytes| *bytes <= crate::residual::MAX_REQUEST_BYTES)
                .ok_or_else(|| {
                    limit("constraint join exceeds the 1 MiB byte ceiling; use explicit authoring")
                })?;
            // Reserving the sum covers the maximum number of entries before
            // allocating, including overlaps. Values and keys are never cloned.
            let capacity = lhs.entries.len() + rhs.len();
            let memory = budget.reserve::<Entry<'_>>(capacity)?;
            let mut entries = Vec::with_capacity(capacity);
            entries.extend_from_slice(&lhs.entries);
            entries.extend(rhs.iter().filter(|(key, _)| lhs.get(key).is_none()));
            entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
            joined.values.push(Row {
                entries,
                _memory: memory,
            });
        }
    }
    if joined.values.is_empty() {
        return Err(failure(
            AgentErrorCode::ResidualFamilyEmpty,
            "constraint conjunction has no complete description",
        ));
    }
    budget.checkpoint()?;
    Ok(joined)
}

#[cfg(test)]
mod tests;

//! Bounded JSON sizing and encoding without allocating a measurement buffer.

use std::io::{self, Write};

use serde::Serialize;
use serde_json::Value;

use super::{Budget, memory::Reservation};
use crate::error::{AgentError, Result};

pub(crate) struct Encoded {
    pub bytes: Vec<u8>,
    // The reservation must outlive the bytes it accounts for.
    _memory: Reservation,
}

/// Borrowed domain values in the same byte order as the former JSON-key map.
/// The vector and all live encoding keys share the caller's memory ceiling.
pub(crate) struct Domain<'a> {
    keyed: Vec<(Encoded, &'a Value)>,
    _memory: Reservation,
}

impl Domain<'_> {
    pub(crate) fn values(&self) -> impl Iterator<Item = &Value> {
        self.keyed.iter().map(|(_, value)| *value)
    }
}

pub(crate) fn domain<'a>(
    values: impl ExactSizeIterator<Item = &'a Value>,
    budget: &mut Budget,
    limit: usize,
) -> Result<Domain<'a>> {
    let count = values.len();
    if count > super::MAX_DESCRIPTIONS {
        return Err(super::limit("decision domain exceeds 256 descriptions"));
    }
    let memory = budget.reserve::<(Encoded, &Value)>(count)?;
    let mut keyed = Vec::with_capacity(count);
    for value in values {
        if keyed.len() == count {
            return Err(super::limit(
                "decision domain iterator exceeded its reserved capacity",
            ));
        }
        keyed.push((encode(value, budget, limit)?, value));
    }
    keyed.sort_unstable_by(|left, right| left.0.bytes.cmp(&right.0.bytes));
    keyed.dedup_by(|left, right| left.0.bytes == right.0.bytes);
    budget.checkpoint()?;
    Ok(Domain {
        keyed,
        _memory: memory,
    })
}

struct Writer<'a> {
    budget: &'a mut Budget,
    limit: usize,
    count: usize,
    output: Option<&'a mut Vec<u8>>,
    failure: Option<AgentError>,
}

impl Write for Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = (|| {
            self.budget.charge(bytes.len().div_ceil(64).max(1))?;
            self.count = self.count.checked_add(bytes.len())
                .filter(|count| *count <= self.limit)
                .ok_or_else(|| super::limit("constraint materialization exceeds its byte ceiling; use explicit authoring"))?;
            if let Some(output) = &mut self.output {
                output.extend_from_slice(bytes);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.failure = Some(error);
            return Err(io::Error::other("bounded planner serialization refused"));
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialize(
    value: &impl Serialize,
    budget: &mut Budget,
    limit: usize,
    output: Option<&mut Vec<u8>>,
) -> Result<usize> {
    budget.checkpoint()?;
    let mut writer = Writer {
        budget,
        limit,
        count: 0,
        output,
        failure: None,
    };
    let result = serde_json::to_writer(&mut writer, value);
    if let Some(error) = writer.failure {
        return Err(error);
    }
    result.map_err(|error| super::invalid(&error.to_string()))?;
    writer.budget.checkpoint()?;
    Ok(writer.count)
}

pub(crate) fn size(value: &impl Serialize, budget: &mut Budget, limit: usize) -> Result<usize> {
    serialize(value, budget, limit, None)
}

pub(crate) fn encode(value: &impl Serialize, budget: &mut Budget, limit: usize) -> Result<Encoded> {
    let length = size(value, budget, limit)?;
    let memory = budget.reserve::<u8>(length)?;
    let mut bytes = Vec::with_capacity(length);
    serialize(value, budget, length, Some(&mut bytes))?;
    Ok(Encoded {
        bytes,
        _memory: memory,
    })
}

#[cfg(test)]
mod tests;

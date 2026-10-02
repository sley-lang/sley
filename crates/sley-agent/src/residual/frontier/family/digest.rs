//! Stream the existing typed canonical encoding without a copied value tree.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::super::{Budget, Field, invalid, limit};
use crate::error::Result;

pub(in crate::residual::frontier) struct Sink<'a> {
    budget: &'a mut Budget,
    count: u64,
    hash: Option<Sha256>,
}

impl Sink<'_> {
    pub(in crate::residual::frontier) fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.budget.charge(bytes.len().div_ceil(64).max(1))?;
        self.count = self
            .count
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| limit("canonical planner identity length overflow"))?;
        if let Some(hash) = &mut self.hash {
            hash.update(bytes);
        }
        Ok(())
    }

    pub(in crate::residual::frontier) fn bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.write(&(bytes.len() as u64).to_be_bytes())?;
        self.write(bytes)
    }

    pub(in crate::residual::frontier) fn container(
        &mut self,
        tag: u8,
        length: usize,
    ) -> Result<()> {
        self.write(&[tag])?;
        self.write(&(length as u64).to_be_bytes())
    }

    pub(in crate::residual::frontier) fn string(&mut self, text: &str) -> Result<()> {
        self.write(b"s")?;
        self.bytes(text.as_bytes())
    }

    fn object(&mut self, object: &Map<String, Value>) -> Result<()> {
        self.container(b'o', object.len())?;
        let _memory = self.budget.reserve::<(&String, &Value)>(object.len())?;
        let mut members = Vec::with_capacity(object.len());
        members.extend(object.iter());
        members.sort_unstable_by(|(left, _), (right, _)| left.as_bytes().cmp(right.as_bytes()));
        for (key, value) in members {
            self.bytes(key.as_bytes())?;
            self.value(value)?;
        }
        Ok(())
    }

    fn value(&mut self, value: &Value) -> Result<()> {
        match value {
            Value::Null => self.write(b"n"),
            Value::Bool(value) => self.write(if *value { b"t" } else { b"f" }),
            Value::Number(number) => {
                if !number.is_i64() && !number.is_u64() {
                    return Err(invalid("floats have no residual canonical encoding"));
                }
                self.write(b"i")?;
                self.bytes(number.to_string().as_bytes())
            }
            Value::String(text) => self.string(text),
            Value::Array(values) => {
                self.container(b'a', values.len())?;
                for value in values {
                    self.value(value)?;
                }
                Ok(())
            }
            Value::Object(object) => self.object(object),
        }
    }

    fn family(&mut self, fields: &[Field], rows: &[Map<String, Value>]) -> Result<()> {
        self.container(b'o', 2)?;
        self.bytes(b"descriptions")?;
        self.container(b'a', rows.len())?;
        for row in rows {
            self.object(row)?;
        }
        self.bytes(b"fields")?;
        self.fields(fields)
    }

    pub(in crate::residual::frontier) fn fields(&mut self, fields: &[Field]) -> Result<()> {
        self.container(b'a', fields.len())?;
        for field in fields {
            self.container(b'o', 3)?;
            self.bytes(b"cost")?;
            self.write(b"i")?;
            self.bytes(field.cost.to_string().as_bytes())?;
            self.bytes(b"eligible")?;
            self.write(if field.eligible { b"t" } else { b"f" })?;
            self.bytes(b"name")?;
            self.string(&field.name)?;
        }
        Ok(())
    }
}

pub(super) fn family(
    fields: &[Field],
    rows: &[Map<String, Value>],
    budget: &mut Budget,
) -> Result<String> {
    hash(b"finite-description-family-v1", budget, |sink| {
        sink.family(fields, rows)
    })
}

/// Preserve the existing domain/length-prefixed digest while streaming the
/// same borrowed declaration twice, under one work/time budget.
pub(in crate::residual::frontier) fn hash(
    domain: &[u8],
    budget: &mut Budget,
    mut emit: impl FnMut(&mut Sink<'_>) -> Result<()>,
) -> Result<String> {
    budget.checkpoint()?;
    let mut sink = Sink {
        budget,
        count: 0,
        hash: None,
    };
    emit(&mut sink)?;
    let length = sink.count;
    let mut hash = Sha256::new();
    hash.update(b"sley.ghostweave.integrity.v1\0");
    hash.update((domain.len() as u64).to_be_bytes());
    hash.update(domain);
    hash.update(length.to_be_bytes());
    sink.hash = Some(hash);
    sink.count = 0;
    emit(&mut sink)?;
    if sink.count != length {
        return Err(invalid("canonical planner identity changed between passes"));
    }
    sink.budget.checkpoint()?;
    Ok(crate::hex::encode(
        &sink.hash.expect("initialized").finalize(),
    ))
}

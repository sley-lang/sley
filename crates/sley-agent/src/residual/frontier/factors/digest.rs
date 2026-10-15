//! Stream graph identity in the legacy typed JSON encoding and key order.

use super::super::family::digest::{Sink, hash};
use super::{Budget, Dependency, Family, Field, Result};

pub(super) fn problem(
    fields: &[Field],
    constraints: &[Family],
    dependencies: &[Dependency],
    budget: &mut Budget,
) -> Result<String> {
    hash(b"factored-constraint-graph-v1", budget, |sink| {
        declaration(sink, fields, constraints, dependencies)
    })
}

fn declaration(
    sink: &mut Sink<'_>,
    fields: &[Field],
    constraints: &[Family],
    dependencies: &[Dependency],
) -> Result<()> {
    sink.container(b'o', 3)?;
    sink.bytes(b"constraints")?;
    sink.container(b'a', constraints.len())?;
    for family in constraints {
        sink.string(&family.digest)?;
    }
    sink.bytes(b"dependencies")?;
    sink.container(b'a', dependencies.len())?;
    for edge in dependencies {
        sink.container(b'o', 2)?;
        sink.bytes(b"fields")?;
        sink.container(b'a', edge.fields.len())?;
        for &index in &edge.fields {
            sink.string(&fields[index].name)?;
        }
        sink.bytes(b"kind")?;
        sink.string(&edge.kind)?;
    }
    sink.bytes(b"fields")?;
    sink.fields(fields)
}

//! Bounded reconstruction over a declared constraint graph.
//!
//! Constraint scopes and explicit dependency edges determine components. Row
//! correlations never justify splitting a scope. This mathematical API does
//! not establish that a program adapter supplied all semantic dependencies.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};
use serde_json::{Map, Value, json};

use super::{Budget, Family, Field, Frontier, MAX_FIELDS, Method};
use crate::error::{AgentErrorCode, Result};

mod digest;
mod rows;
mod topology;

const MAX_CONSTRAINTS: usize = 256;
const MAX_DEPENDENCIES: usize = 1024;
const DEPENDENCY_KINDS: &[&str] = &[
    "type",
    "ownership",
    "effect",
    "order",
    "error_route",
    "binding",
    "author_relation",
];

#[derive(Clone, Debug)]
struct Dependency {
    kind: String,
    fields: Vec<usize>,
}

/// A closed finite constraint declaration, not executable semantic authority.
/// Every field has a domain supplied by at least one literal table. Tables are
/// conjoined by exact typed natural joins; dependencies conservatively couple
/// fields even when their literal values happen to form a Cartesian product.
#[derive(Clone, Debug)]
pub struct Problem {
    fields: Vec<Field>,
    constraints: Vec<Family>,
    dependencies: Vec<Dependency>,
    digest: String,
}

impl Problem {
    /// Bind a program adapter's checked context into mathematical identity.
    /// This adds no semantic authority; it prevents cross-context reuse.
    pub(crate) fn bind_context(&mut self, context: &str) -> Result<()> {
        self.digest = crate::residual::binding::canonical_digest(
            "factored-program-context-v1",
            &json!({"problem":self.digest,"context":context}),
        )?;
        Ok(())
    }

    /// Parses strict `{fields, constraints, dependencies}` JSON. A constraint
    /// has exactly `{fields:[names], rows:[complete objects]}`; a dependency
    /// has exactly `{kind, fields:[names]}`. Field descriptors match `Family`.
    ///
    /// # Errors
    /// Refuses unknown keys, unbound fields, duplicate names/rows, missing
    /// domains, empty constraints, and structural/request limits. A dependency
    /// may name type, ownership, effect, order, `error_route`, binding, or
    /// `author_relation`; those labels do not independently prove completeness.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        Self::parse_with_budget(bytes, &mut Budget::default())
    }

    /// Parses all constraint families under one caller-owned budget.
    ///
    /// # Errors
    /// The same refusals as [`Self::parse`], plus aggregate exhaustion across
    /// table canonicalization. Parser tree allocations remain unaccounted.
    pub fn parse_with_budget(bytes: &[u8], budget: &mut Budget) -> Result<Self> {
        budget.checkpoint()?;
        let value = crate::residual::strict_json(bytes)?;
        budget.checkpoint()?;
        exact_keys(&value, &["fields", "constraints", "dependencies"])?;
        let fields = array(&value["fields"])?;
        if fields.is_empty() || fields.len() > MAX_FIELDS {
            return Err(super::limit("factored declaration requires 1..64 fields"));
        }
        let mut fields: Vec<_> = fields
            .iter()
            .map(super::read_field)
            .collect::<Result<_>>()?;
        fields.sort_by(|left, right| left.name.cmp(&right.name));
        if fields.windows(2).any(|pair| pair[0].name == pair[1].name) {
            return Err(super::invalid("duplicate factored field name"));
        }
        let tables = array(&value["constraints"])?;
        let edges = array(&value["dependencies"])?;
        if tables.len() > MAX_CONSTRAINTS || edges.len() > MAX_DEPENDENCIES {
            return Err(super::limit(
                "at most 256 constraints and 1024 dependency edges",
            ));
        }
        let mut covered = BTreeSet::new();
        let mut constraints = Vec::new();
        for table in tables {
            budget.checkpoint()?;
            exact_keys(table, &["fields", "rows"])?;
            let scope = scope(&table["fields"], &fields, 1)?;
            covered.extend(scope.iter().copied());
            let encoded = ComponentEncoding {
                descriptors: Descriptors {
                    fields: &fields,
                    mask: scope.iter().fold(0, |mask, index| mask | (1_u64 << index)),
                },
                rows: &table["rows"],
            };
            let encoded =
                super::encoding::encode(&encoded, budget, crate::residual::MAX_REQUEST_BYTES)?;
            constraints.push(Family::parse_with_budget(&encoded.bytes, budget)?);
        }
        if covered.len() != fields.len() {
            return Err(super::failure(
                AgentErrorCode::ResidualFamilyIncomplete,
                "every field requires an explicit domain constraint",
            ));
        }
        // Stable join order: restrictive tables first, then canonical identity.
        constraints.sort_by(|left, right| {
            left.rows
                .len()
                .cmp(&right.rows.len())
                .then_with(|| left.digest.cmp(&right.digest))
        });
        let mut dependencies = Vec::new();
        for edge in edges {
            budget.checkpoint()?;
            exact_keys(edge, &["kind", "fields"])?;
            let kind = edge["kind"]
                .as_str()
                .filter(|kind| DEPENDENCY_KINDS.contains(kind))
                .ok_or_else(|| super::invalid("unknown dependency kind"))?;
            dependencies.push(Dependency {
                kind: kind.to_owned(),
                fields: scope(&edge["fields"], &fields, 2)?,
            });
        }
        dependencies.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.fields.cmp(&right.fields))
        });
        if dependencies
            .windows(2)
            .any(|pair| pair[0].kind == pair[1].kind && pair[0].fields == pair[1].fields)
        {
            return Err(super::invalid("duplicate dependency edge"));
        }
        let digest = digest::problem(&fields, &constraints, &dependencies, budget)?;
        budget.checkpoint()?;
        Ok(Self {
            fields,
            constraints,
            dependencies,
            digest,
        })
    }
}

#[derive(Clone, Debug)]
struct Component {
    family: Family,
    frontier: Frontier,
}

/// Serialize borrowed rows in the same key order as the prior JSON envelope.
struct ComponentEncoding<'a, T: ?Sized> {
    descriptors: Descriptors<'a>,
    rows: &'a T,
}

impl<T: Serialize + ?Sized> Serialize for ComponentEncoding<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut object = serializer.serialize_map(Some(2))?;
        object.serialize_entry("descriptions", self.rows)?;
        object.serialize_entry("fields", &self.descriptors)?;
        object.end()
    }
}

/// Borrow descriptors directly from the validated field vocabulary. Their
/// lexical order and map-key order match the prior JSON-value envelope.
struct Descriptors<'a> {
    fields: &'a [Field],
    mask: u64,
}

impl Serialize for Descriptors<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        struct Descriptor<'a>(&'a Field);
        impl Serialize for Descriptor<'_> {
            fn serialize<S: Serializer>(
                &self,
                serializer: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("cost", &self.0.cost)?;
                map.serialize_entry("eligible", &self.0.eligible)?;
                map.serialize_entry("name", &self.0.name)?;
                map.end()
            }
        }
        let mut sequence = serializer.serialize_seq(Some(self.mask.count_ones() as usize))?;
        for index in super::bits(self.mask) {
            sequence.serialize_element(&Descriptor(&self.fields[index]))?;
        }
        sequence.end()
    }
}

/// Checked component frontiers tied to an exact declaration and dependency
/// graph. Reconstruction combines one answer per component; it never enumerates
/// the global Cartesian product. Private components cannot be certificate-loaded.
#[derive(Clone, Debug)]
pub struct FactoredFrontier {
    problem: Arc<String>,
    components: Arc<Vec<Component>>,
    fields: Arc<Vec<String>>,
    _memory: Arc<super::memory::Reservation>,
}

impl FactoredFrontier {
    /// Only a single coupled component has complete rows without a product.
    pub(crate) fn coupled_descriptions(&self) -> Option<&[Map<String, Value>]> {
        let [component] = self.components.as_slice() else {
            return None;
        };
        Some(component.family.descriptions())
    }

    /// The complete question batch in lexical order.
    #[must_use]
    pub fn fields(&self) -> &[String] {
        &self.fields
    }

    /// Component sizes, graph-relative guarantees and explicit authority limits.
    #[must_use]
    pub fn summary(&self) -> Value {
        let count = self.components.iter().try_fold(1_u128, |total, component| {
            total.checked_mul(component.family.rows.len() as u128)
        });
        let cost: u64 = self
            .components
            .iter()
            .map(|component| component.frontier.cost())
            .sum();
        let exact = self
            .components
            .iter()
            .all(|component| component.frontier.method() == Method::ExactAdditive);
        json!({"problem":*self.problem,"fields":*self.fields,"additive_cost":cost,
            "method":if exact {"exact_additive_for_declared_graph"} else {"checked_component_frontiers"},
            "description_count_decimal":count.map(|value| value.to_string()),"count_exceeds_u128":count.is_none(),
            "materialized_component_rows":self.components.iter().map(|component| component.family.rows.len()).sum::<usize>(),
            "global_product_materialized":false,
            "components":self.components.iter().map(|component| json!({
                "fields":component.family.fields.iter().map(|field| &field.name).collect::<Vec<_>>(),
                "rows":component.family.rows.len(),"frontier":component.frontier.summary()})).collect::<Vec<_>>(),
            "independence":"disconnected scopes in the complete supplied constraint/dependency graph",
            "program_dependency_completeness":"not_established",
            "semantic_entitlement":"not_established","family_completeness_for_task":"not_established",
            "billed_cost_optimality":"not_established"})
    }

    /// Projects an actual complete member without constructing other global rows.
    ///
    /// # Errors
    /// Refuses stale graphs, missing/unknown fields and any violated constraint.
    pub fn encode(
        &self,
        problem: &Problem,
        description: &Map<String, Value>,
    ) -> Result<Map<String, Value>> {
        self.bound(problem)?;
        if description.len() != problem.fields.len()
            || problem
                .fields
                .iter()
                .any(|field| !description.contains_key(&field.name))
        {
            return Err(super::failure(
                AgentErrorCode::ResidualChoiceUnknown,
                "complete description must name every declared field exactly",
            ));
        }
        let mut answers = Map::new();
        for component in self.components.iter() {
            let row = component
                .family
                .fields
                .iter()
                .map(|field| (field.name.clone(), description[&field.name].clone()))
                .collect();
            answers.extend(component.frontier.encode(&component.family, &row)?);
        }
        Ok(answers)
    }

    /// Uniquely reconstructs one global completion using only component rows.
    ///
    /// # Errors
    /// Refuses stale graphs, missing/extra answers, typed domain mismatches and
    /// out-of-family choices. Never returns a partial or nearest completion.
    pub fn decode(
        &self,
        problem: &Problem,
        answers: &Map<String, Value>,
    ) -> Result<Map<String, Value>> {
        self.bound(problem)?;
        if answers.keys().any(|field| !self.fields.contains(field)) {
            return Err(super::failure(
                AgentErrorCode::ResidualChoiceUnknown,
                "answer names a non-frontier field",
            ));
        }
        if answers.len() != self.fields.len() {
            return Err(super::failure(
                AgentErrorCode::ResidualChoiceMissing,
                "answer every component frontier field",
            ));
        }
        let mut description = Map::new();
        for component in self.components.iter() {
            let choice = component
                .frontier
                .fields()
                .iter()
                .map(|field| (field.clone(), answers[field].clone()))
                .collect();
            description.extend(component.frontier.decode(&component.family, &choice)?);
        }
        Ok(description)
    }

    fn bound(&self, problem: &Problem) -> Result<()> {
        if *self.problem != problem.digest {
            return Err(super::failure(
                AgentErrorCode::ResidualBindingStale,
                "constraint scopes, rows, field vocabulary or dependencies changed",
            ));
        }
        Ok(())
    }
}

/// Builds conservative connected components, joins only within each component,
/// and shares the caller's budget across joins and ordinary frontier planning.
/// An edge never filters values; it prevents splitting a coupled region.
///
/// # Errors
/// Refuses contradictory constraints, joins exceeding 256 intermediate or final
/// component rows, inadequate question vocabularies and aggregate exhaustion.
/// No partial frontier escapes if any component cannot be completely checked.
pub fn plan(
    problem: &Problem,
    budget: &mut Budget,
    refine_exact: bool,
) -> Result<FactoredFrontier> {
    budget.checkpoint()?;
    let groups = topology::Groups::new(problem, budget)?;
    let fixed_bytes = groups.len() * size_of::<Component>()
        + problem.fields.len() * size_of::<String>()
        + problem.digest.len();
    let mut retained = budget.reserve::<u8>(
        fixed_bytes
            + problem
                .fields
                .iter()
                .map(|field| field.name.len())
                .sum::<usize>(),
    )?;
    let mut components = Vec::with_capacity(groups.len());
    let mut remaining_bytes = crate::residual::MAX_REQUEST_BYTES;
    for (root, mask) in groups.iter() {
        let mut rows = rows::Rows::identity(budget)?;
        for (index, table) in problem.constraints.iter().enumerate() {
            budget.charge(1)?;
            if groups.table_root(index) == root {
                rows = rows::join(&rows, &table.rows, budget)?;
            }
        }
        let encoded = super::encoding::encode(
            &ComponentEncoding {
                descriptors: Descriptors {
                    fields: &problem.fields,
                    mask,
                },
                rows: &rows.values,
            },
            budget,
            remaining_bytes,
        )?;
        remaining_bytes -= encoded.bytes.len();
        let family = Family::parse_with_budget(&encoded.bytes, budget)?;
        drop(encoded);
        let frontier = super::plan(&family, budget, refine_exact)?;
        components.push(Component { family, frontier });
    }
    let mut fields = Vec::with_capacity(problem.fields.len());
    fields.extend(
        components
            .iter()
            .flat_map(|component| component.frontier.fields().iter().cloned()),
    );
    fields.sort_unstable();
    retained.shrink(fixed_bytes + fields.iter().map(String::len).sum::<usize>());
    // Exact refinement can retain a checked bounded-best frontier after work
    // exhaustion. Its feasibility stands, but no fresh component may start with
    // a new allowance. Do not falsely promote that result to global optimality.
    Ok(FactoredFrontier {
        problem: Arc::new(problem.digest.clone()),
        components: Arc::new(components),
        fields: Arc::new(fields),
        _memory: Arc::new(retained),
    })
}

#[cfg(test)]
fn field_value(field: &Field) -> Value {
    json!({"name":field.name,"cost":field.cost,"eligible":field.eligible})
}
fn index_of(fields: &[Field], name: &str) -> Option<usize> {
    fields
        .binary_search_by(|field| field.name.as_str().cmp(name))
        .ok()
}
fn scope(value: &Value, fields: &[Field], minimum: usize) -> Result<Vec<usize>> {
    let names = array(value)?;
    if names.len() < minimum || names.len() > fields.len() {
        return Err(super::invalid("invalid constraint/dependency scope size"));
    }
    let mut indices = Vec::new();
    for name in names {
        let index = name
            .as_str()
            .and_then(|name| index_of(fields, name))
            .ok_or_else(|| super::invalid("scope names an undeclared field"))?;
        indices.push(index);
    }
    indices.sort_unstable();
    if indices.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(super::invalid("duplicate scope field"));
    }
    Ok(indices)
}
fn exact_keys(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| super::invalid("constraint graph objects required"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(super::invalid("unknown or missing constraint graph member"));
    }
    Ok(())
}
fn array(value: &Value) -> Result<&[Value]> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| super::invalid("constraint graph arrays required"))
}
#[cfg(test)]
mod tests;

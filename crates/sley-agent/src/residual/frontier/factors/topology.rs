//! Fixed-capacity component topology for the validated 64-field vocabulary.
//! No heap allocation is needed for parents, scopes, groups or table membership.

use super::{Budget, MAX_CONSTRAINTS, MAX_FIELDS, Problem, Result, index_of};

pub(super) struct Groups {
    masks: [u64; MAX_FIELDS],
    tables: [usize; MAX_CONSTRAINTS],
}

impl Groups {
    pub fn new(problem: &Problem, budget: &mut Budget) -> Result<Self> {
        let mut parents: [usize; MAX_FIELDS] = std::array::from_fn(|index| index);
        let mut groups = Self {
            masks: [0; MAX_FIELDS],
            tables: [0; MAX_CONSTRAINTS],
        };
        for (table_index, table) in problem.constraints.iter().enumerate() {
            budget.checkpoint()?;
            let mut first = None;
            for field in table.fields.iter() {
                budget.charge(1)?;
                let index = index_of(&problem.fields, &field.name).ok_or_else(|| {
                    super::super::failure(
                        crate::error::AgentErrorCode::ResidualInconclusive,
                        "validated constraint field is absent from the graph",
                    )
                })?;
                if let Some(first) = first {
                    connect(&mut parents, first, index, budget)?;
                } else {
                    first = Some(index);
                    groups.tables[table_index] = index;
                }
            }
        }
        for edge in &problem.dependencies {
            budget.checkpoint()?;
            for &index in &edge.fields[1..] {
                connect(&mut parents, edge.fields[0], index, budget)?;
            }
        }
        for index in 0..problem.fields.len() {
            let root = root(&parents, index, budget)?;
            groups.masks[root] |= 1_u64 << index;
        }
        for table in &mut groups.tables[..problem.constraints.len()] {
            *table = root(&parents, *table, budget)?;
        }
        Ok(groups)
    }

    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, u64)> + '_ {
        self.masks
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, mask)| *mask != 0)
    }

    pub fn table_root(&self, index: usize) -> usize {
        self.tables[index]
    }
}

fn root(parents: &[usize; MAX_FIELDS], mut index: usize, budget: &mut Budget) -> Result<usize> {
    loop {
        budget.charge(1)?;
        if parents[index] == index {
            return Ok(index);
        }
        index = parents[index];
    }
}

fn connect(
    parents: &mut [usize; MAX_FIELDS],
    left: usize,
    right: usize,
    budget: &mut Budget,
) -> Result<()> {
    let left = root(parents, left, budget)?;
    let right = root(parents, right, budget)?;
    parents[left.max(right)] = left.min(right);
    Ok(())
}

//! Public projection of strictly decoded SSMC1 entity objects onto the
//! normative eighteen-kind semantic model (S20-250 full, contract
//! `docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md` section 6 step 4).
//!
//! The projection reuses the S20-360 validator's private program projection
//! so exactly one mapping from proposal bodies to definitions exists. It
//! grants no validation, commit, or root authority.

use core::fmt;

use sley_id::EntityId;
use sley_mutate::EntityObject;
use sley_ssmc::{
    AdapterImport, Block, CapabilityRequirement, ConstantDefinition, ContractDefinition,
    DependencyBindingDefinition, EffectDefinition, EntryPointDefinition, FunctionGraph,
    GlobalValueDefinition, NamespaceDefinition, Operation, PackageDefinition, Parameter,
    PolicyBindingDefinition, TestCaseDefinition, TypeDefinition, WorkspaceDefinition,
};

use crate::candidate_program::{CandidateProgram, CandidateProgramError};

/// Owned definitions of every kind present in one projected object set.
#[derive(Clone, Debug, Default)]
pub struct CompleteEntities {
    /// Kind 1.
    pub workspaces: Vec<WorkspaceDefinition>,
    /// Kind 2.
    pub packages: Vec<PackageDefinition>,
    /// Kind 3.
    pub namespaces: Vec<NamespaceDefinition>,
    /// Kind 4.
    pub type_definitions: Vec<TypeDefinition>,
    /// Kind 5.
    pub functions: Vec<FunctionGraph>,
    /// Kind 6.
    pub parameters: Vec<Parameter>,
    /// Kind 7.
    pub blocks: Vec<Block>,
    /// Kind 8.
    pub operations: Vec<Operation>,
    /// Kind 9.
    pub constants: Vec<ConstantDefinition>,
    /// Kind 10.
    pub globals: Vec<GlobalValueDefinition>,
    /// Kind 11.
    pub effects: Vec<EffectDefinition>,
    /// Kind 12.
    pub requirements: Vec<CapabilityRequirement>,
    /// Kind 13.
    pub contracts: Vec<ContractDefinition>,
    /// Kind 14.
    pub tests: Vec<TestCaseDefinition>,
    /// Kind 15.
    pub adapters: Vec<AdapterImport>,
    /// Kind 16.
    pub entry_points: Vec<EntryPointDefinition>,
    /// Kind 17.
    pub policy_bindings: Vec<PolicyBindingDefinition>,
    /// Kind 18.
    pub dependency_bindings: Vec<DependencyBindingDefinition>,
    /// The validator's private reference graph as `(dependent, dependency,
    /// relationship tag)` triples in canonical order.
    pub reference_edges: Vec<(EntityId, EntityId, u32)>,
}

impl CompleteEntities {
    /// Returns the number of projected entities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.workspaces.len()
            + self.packages.len()
            + self.namespaces.len()
            + self.type_definitions.len()
            + self.functions.len()
            + self.parameters.len()
            + self.blocks.len()
            + self.operations.len()
            + self.constants.len()
            + self.globals.len()
            + self.effects.len()
            + self.requirements.len()
            + self.contracts.len()
            + self.tests.len()
            + self.adapters.len()
            + self.entry_points.len()
            + self.policy_bindings.len()
            + self.dependency_bindings.len()
    }

    /// Returns whether no entity was projected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Stable projection failure with the validator's exact source code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteProjectionError {
    symbol: &'static str,
    numeric: u32,
}

impl CompleteProjectionError {
    /// Returns the exact source symbol (`GRAPH_*` or `SSMC_*`).
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        self.symbol
    }

    /// Returns the exact source numeric code.
    #[must_use]
    pub const fn numeric(self) -> u32 {
        self.numeric
    }
}

impl fmt::Display for CompleteProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.symbol)
    }
}

impl std::error::Error for CompleteProjectionError {}

impl From<CandidateProgramError> for CompleteProjectionError {
    fn from(value: CandidateProgramError) -> Self {
        Self {
            symbol: value.source_symbol(),
            numeric: value.source_numeric_code(),
        }
    }
}

/// Projects raw-ID-sorted strictly decoded entity objects onto the normative
/// model, applying the validator's structural reference checks.
///
/// # Errors
///
/// Returns the validator's exact duplicate, canonical-order, opcode,
/// unresolved-reference, wrong-kind, or resource failure.
pub fn project_complete_entities(
    objects: &[EntityObject],
) -> Result<CompleteEntities, CompleteProjectionError> {
    let program = CandidateProgram::project(objects)?;
    let reference_edges = program.reference_edges();
    Ok(CompleteEntities {
        workspaces: program.workspaces,
        packages: program.packages,
        namespaces: program.namespaces,
        type_definitions: program.type_definitions,
        functions: program.functions,
        parameters: program.parameters,
        blocks: program.blocks,
        operations: program.operations,
        constants: program.constants,
        globals: program.globals,
        effects: program.effects,
        requirements: program.requirements,
        contracts: program.contracts,
        tests: program.tests,
        adapters: program.adapters,
        entry_points: program.entry_points,
        policy_bindings: program.policy_binding_definitions,
        dependency_bindings: program.dependency_binding_definitions,
        reference_edges,
    })
}

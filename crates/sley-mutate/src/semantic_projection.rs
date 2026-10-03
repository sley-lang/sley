//! Shared, authority-free projection of exact entity objects onto SSMC1.
//!
//! This is a shape conversion, not a graph judgment. The policy validator
//! still checks references and static semantics; the native test worker
//! checks its selected graph through the existing contract and VM owners.

use sley_id::EntityId;
use sley_ssmc::{
    AdapterImport, Block, CapabilityRequirement, ConstantDefinition, ContractDefinition,
    DependencyBindingDefinition, EffectDefinition, EntryPointDefinition, FunctionGraph,
    GlobalValueDefinition, NamespaceDefinition, Opcode, Operation, PackageDefinition, Parameter,
    PolicyBindingDefinition, TestCaseDefinition, TypeDefinition, WorkspaceDefinition,
};

use crate::{EntityObject, value::EntityBodyValue};

/// Maximum entities in one SSMC1 semantic inventory.
pub const MAX_SEMANTIC_ENTITIES: usize = 65_535;

/// Shape projection refusal; policy maps these to its preserved owner codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticProjectionError {
    /// More than the frozen entity ceiling.
    ResourceLimit,
    /// Object identities were not in strict raw-ID order.
    SetNotCanonical,
    /// An operation named an unknown opcode.
    OpcodeUnknown,
}

impl SemanticProjectionError {
    /// Preserved projection failure symbol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResourceLimit => "CFG_RESOURCE_LIMIT",
            Self::SetNotCanonical => "GRAPH_INVENTORY_MISMATCH",
            Self::OpcodeUnknown => "SSMC_OPCODE_UNKNOWN",
        }
    }
}

impl core::fmt::Display for SemanticProjectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::error::Error for SemanticProjectionError {}

/// Complete owned SSMC1 definitions from one exact object inventory.
#[derive(Clone, Debug, Default)]
pub struct SemanticInventory {
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
}

/// Converts one canonical object inventory to complete semantic definitions.
///
/// It only checks the object count, ordering, and opcode tags. Root bindings,
/// references, types, effects, and test admissibility belong to their owners.
///
/// # Errors
///
/// Returns the first resource, ordering, or opcode-shape refusal.
pub fn project_semantic_inventory(
    objects: &[EntityObject],
) -> Result<SemanticInventory, SemanticProjectionError> {
    if objects.len() > MAX_SEMANTIC_ENTITIES {
        return Err(SemanticProjectionError::ResourceLimit);
    }
    let mut inventory = SemanticInventory::default();
    let mut previous: Option<EntityId> = None;
    for object in objects {
        let record = object.record();
        if previous.is_some_and(|prior| prior >= record.entity_id) {
            return Err(SemanticProjectionError::SetNotCanonical);
        }
        previous = Some(record.entity_id);
        project_body(&mut inventory, record.entity_id, &record.body)?;
    }
    Ok(inventory)
}

#[allow(clippy::too_many_lines)]
fn project_body(
    inventory: &mut SemanticInventory,
    entity_id: EntityId,
    body: &EntityBodyValue,
) -> Result<(), SemanticProjectionError> {
    match body {
        EntityBodyValue::Workspace(value) => inventory.workspaces.push(WorkspaceDefinition {
            entity_id,
            packages: value.packages.as_slice().to_vec(),
            root_namespace: value.root_namespace,
            capability_requirements: value.capability_requirements.as_slice().to_vec(),
            contracts: value.contracts.as_slice().to_vec(),
            tests: value.tests.as_slice().to_vec(),
        }),
        EntityBodyValue::Package(value) => inventory.packages.push(PackageDefinition {
            entity_id,
            workspace: value.workspace,
            root_namespace: value.root_namespace,
            dependencies: value.dependencies.as_slice().to_vec(),
            exports: value.exports.as_slice().to_vec(),
        }),
        EntityBodyValue::Namespace(value) => inventory.namespaces.push(NamespaceDefinition {
            entity_id,
            parent: value.parent,
            members: value.members.as_slice().to_vec(),
        }),
        EntityBodyValue::EntryPoint(value) => inventory.entry_points.push(EntryPointDefinition {
            entity_id,
            function: value.function,
            exposure: value.exposure,
        }),
        EntityBodyValue::TypeDef(value) => inventory.type_definitions.push(TypeDefinition {
            entity_id,
            type_parameters: value.type_parameters.clone(),
            form: value.form.clone(),
            invariants: value.invariants.as_slice().to_vec(),
            visibility: value.visibility,
        }),
        EntityBodyValue::Function(value) => inventory.functions.push(FunctionGraph {
            entity_id,
            type_parameters: value.type_parameters.clone(),
            parameters: value.parameters.clone(),
            result_type: value.result_type.clone(),
            effects: value.effects.as_slice().to_vec(),
            entry_block: value.entry_block,
            blocks: value.blocks.clone(),
            contracts: value.contracts.as_slice().to_vec(),
            visibility: value.visibility,
        }),
        EntityBodyValue::Parameter(value) => inventory.parameters.push(Parameter {
            entity_id,
            owner: value.owner,
            role: value.role,
            ordinal: value.ordinal,
            value_type: value.value_type.clone(),
        }),
        EntityBodyValue::Block(value) => inventory.blocks.push(Block {
            entity_id,
            function: value.function,
            parameters: value.parameters.clone(),
            operations: value.operations.clone(),
            terminator: value.terminator.clone(),
            reachability: value.reachability,
        }),
        EntityBodyValue::Operation(value) => inventory.operations.push(Operation {
            entity_id,
            block: value.block,
            ordinal: value.ordinal,
            opcode: Opcode::from_tag(value.opcode).ok_or(SemanticProjectionError::OpcodeUnknown)?,
            operands: value.operands.clone(),
            result_types: value.result_types.clone(),
            immediate: value.immediate.clone(),
        }),
        EntityBodyValue::Constant(value) => inventory.constants.push(ConstantDefinition {
            entity_id,
            value: value.value.clone(),
        }),
        EntityBodyValue::GlobalValue(value) => inventory.globals.push(GlobalValueDefinition {
            entity_id,
            value_type: value.value_type.clone(),
            initializer: value.initializer,
            visibility: value.visibility,
        }),
        EntityBodyValue::EffectDef(value) => inventory.effects.push(EffectDefinition {
            entity_id,
            effect_kind: value.effect_kind,
            scope_type: value.scope_type.clone(),
            request_type: value.request_type.clone(),
            response_type: value.response_type.clone(),
            failure_type: value.failure_type.clone(),
            visibility: value.visibility,
        }),
        EntityBodyValue::CapabilityRequirement(value) => {
            inventory.requirements.push(CapabilityRequirement {
                entity_id,
                effect: value.effect,
                allowed_scopes: value.allowed_scopes.clone(),
                constraint_contracts: value.constraint_contracts.as_slice().to_vec(),
            });
        }
        EntityBodyValue::Contract(value) => inventory.contracts.push(ContractDefinition {
            entity_id,
            target: value.target,
            contract_kind: value.contract_kind,
            predicate: value.predicate,
            bindings: value.bindings.clone(),
            resource_limits: value.resource_limits,
        }),
        EntityBodyValue::TestCase(value) => inventory.tests.push(TestCaseDefinition {
            entity_id,
            target: value.target,
            inputs: value.inputs.clone(),
            effect_environment: value.effect_environment.clone(),
            expected: value.expected.clone(),
            observations: value.observations.clone(),
            resource_limits: value.resource_limits,
        }),
        EntityBodyValue::AdapterImport(value) => inventory.adapters.push(AdapterImport {
            entity_id,
            adapter_id: value.adapter_id,
            abi_version: value.abi_version,
            request_type: value.request_type.clone(),
            response_type: value.response_type.clone(),
            failure_type: value.failure_type.clone(),
            effects: value.effects.as_slice().to_vec(),
        }),
        EntityBodyValue::PolicyBinding(value) => {
            inventory.policy_bindings.push(PolicyBindingDefinition {
                entity_id,
                subject: value.subject,
                requirements: value.requirements.as_slice().to_vec(),
            });
        }
        EntityBodyValue::DependencyBinding(value) => {
            inventory
                .dependency_bindings
                .push(DependencyBindingDefinition {
                    entity_id,
                    dependency_root: value.dependency_root,
                    external_package: value.external_package,
                    local_namespace: value.local_namespace,
                });
        }
    }
    Ok(())
}

//! Pure, unmeasured execution bridge for a root-bound native `TestCase`.
//!
//! This projects the complete portable object inventory through the existing
//! static owner, rechecks the native test contract, and invokes the native VM.
//! The returned outcome is deterministic VM evidence only. It is not a
//! supervisor measurement, acceptance signature, or committed test result.

use core::fmt;
use std::collections::BTreeMap;

use sley_check::{
    TypeEnvironment, TypeError,
    contracts::{ContractTestValidationError, validate_contract_test_program},
    effects::FunctionUnit,
};
use sley_policy::complete_entities::{
    CompleteEntities, CompleteProjectionError, project_complete_entities,
};
use sley_ssmc::{Block, FunctionGraph, Operation, Parameter, ParameterRole};
use sley_vm::{
    CacheProfile, LoweringInput,
    native_execution::{
        NativeExecutionError, NativeExecutionInput, NativeExecutionOutcome,
        NativeExecutionRequestV1, execute_native_function, profile_id,
    },
};

use crate::{program::PortableTestProgram, worker::WorkerRequest};

/// Failure before an unmeasured native VM result can be returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortableExecutionError {
    /// The complete object inventory did not project onto semantic entities.
    Projection(CompleteProjectionError),
    /// The complete type environment was invalid.
    Type(TypeError),
    /// The existing contract/test checker refused the selected test.
    Static(ContractTestValidationError),
    /// The worker envelope or projected inventories differ from the program.
    SourceMismatch,
    /// Native VM lowering, input, effect, or execution refusal.
    Vm(NativeExecutionError),
    /// The worker's ordered input hashes differ from VM-derived hashes.
    InputHashesMismatch,
}

impl fmt::Display for PortableExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(error) => error.fmt(formatter),
            Self::Type(error) => error.fmt(formatter),
            Self::Static(error) => error.fmt(formatter),
            Self::SourceMismatch => formatter.write_str("NATIVE_TEST_SOURCE_MISMATCH"),
            Self::Vm(error) => error.fmt(formatter),
            Self::InputHashesMismatch => formatter.write_str("NATIVE_TEST_INPUT_HASH_MISMATCH"),
        }
    }
}

impl std::error::Error for PortableExecutionError {}

struct OwnedUnit<'a> {
    function: &'a FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

fn function_units(
    entities: &CompleteEntities,
) -> Result<Vec<OwnedUnit<'_>>, PortableExecutionError> {
    let indices = entities
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| (function.entity_id, index))
        .collect::<BTreeMap<_, _>>();
    let block_functions = entities
        .blocks
        .iter()
        .map(|block| (block.entity_id, block.function))
        .collect::<BTreeMap<_, _>>();
    let mut units = entities
        .functions
        .iter()
        .map(|function| OwnedUnit {
            function,
            parameters: Vec::new(),
            blocks: Vec::new(),
            operations: Vec::new(),
        })
        .collect::<Vec<_>>();
    for parameter in &entities.parameters {
        let function = match parameter.role {
            ParameterRole::Function => parameter.owner,
            ParameterRole::Block => *block_functions
                .get(&parameter.owner)
                .ok_or(PortableExecutionError::SourceMismatch)?,
        };
        let index = *indices
            .get(&function)
            .ok_or(PortableExecutionError::SourceMismatch)?;
        units[index].parameters.push(parameter.clone());
    }
    for block in &entities.blocks {
        let index = *indices
            .get(&block.function)
            .ok_or(PortableExecutionError::SourceMismatch)?;
        units[index].blocks.push(block.clone());
    }
    for operation in &entities.operations {
        let function = block_functions
            .get(&operation.block)
            .ok_or(PortableExecutionError::SourceMismatch)?;
        let index = *indices
            .get(function)
            .ok_or(PortableExecutionError::SourceMismatch)?;
        units[index].operations.push(operation.clone());
    }
    Ok(units)
}

/// Runs one selected native Sley test through the existing pure VM.
///
/// The caller must supply a parsed portable artifact and its exact worker
/// envelope. This function checks both, revalidates the selected test against
/// the complete source graph, and checks VM-derived ordered input hashes.
/// Host memory, elapsed time, isolation, and attestation remain supervisor
/// responsibilities; this result alone never admits a test.
///
/// # Errors
///
/// Refuses mismatched input, invalid static source, or an exact VM failure.
pub fn execute_portable_test(
    program: &PortableTestProgram,
    worker: &WorkerRequest,
) -> Result<NativeExecutionOutcome, PortableExecutionError> {
    let selected = program.selected();
    if worker.program_bytes != program.stored_bytes()
        || worker.declared_limits != selected.declared_limits
        || worker.implementation_limits != program.plan().implementation_limits()
    {
        return Err(PortableExecutionError::SourceMismatch);
    }
    let entities =
        project_complete_entities(program.objects()).map_err(PortableExecutionError::Projection)?;
    let types = TypeEnvironment::new(entities.type_definitions.clone())
        .map_err(PortableExecutionError::Type)?;
    let owned = function_units(&entities)?;
    let units = owned
        .iter()
        .map(|unit| FunctionUnit {
            function: unit.function,
            parameters: &unit.parameters,
            blocks: &unit.blocks,
            operations: &unit.operations,
        })
        .collect::<Vec<_>>();
    let static_report = validate_contract_test_program(
        &types,
        &units,
        &entities.effects,
        &entities.requirements,
        &entities.adapters,
        &entities.type_definitions,
        &entities.constants,
        &entities.globals,
        &entities.contracts,
        &entities.tests,
        &[],
        &[selected.test_entity],
    )
    .map_err(PortableExecutionError::Static)?;
    if !static_report.selected_tests.contains(&selected.test_entity) {
        return Err(PortableExecutionError::SourceMismatch);
    }
    let target = entities
        .functions
        .binary_search_by_key(&selected.target_function, |function| function.entity_id)
        .ok()
        .map(|index| &entities.functions[index])
        .ok_or(PortableExecutionError::SourceMismatch)?;
    let test = entities
        .tests
        .binary_search_by_key(&selected.test_entity, |test| test.entity_id)
        .ok()
        .map(|index| &entities.tests[index])
        .ok_or(PortableExecutionError::SourceMismatch)?;
    let input = NativeExecutionInput {
        lowering: LoweringInput {
            types: &types,
            function: target,
            parameters: &entities.parameters,
            blocks: &entities.blocks,
            operations: &entities.operations,
            schema_epoch: program.root().record.schema_epoch_id,
            state_root: program.root().root,
            profile: CacheProfile::EXTENDED_V1,
            constants: &entities.constants,
            globals: &entities.globals,
            functions: &entities.functions,
            contracts: &entities.contracts,
            adapters: &entities.adapters,
        },
        effects: &entities.effects,
        requirements: &entities.requirements,
    };
    let request = NativeExecutionRequestV1 {
        inputs: test.inputs.clone(),
        declared_limits: worker.declared_limits,
        implementation_limits: worker.implementation_limits,
        profile_id: profile_id(),
    };
    let outcome = execute_native_function(input, request).map_err(PortableExecutionError::Vm)?;
    let observed = outcome.observation().input_hashes();
    if observed.len() != worker.input_hashes.len()
        || observed
            .iter()
            .zip(&worker.input_hashes)
            .any(|(actual, declared)| actual.as_bytes() != declared)
    {
        return Err(PortableExecutionError::InputHashesMismatch);
    }
    Ok(outcome)
}

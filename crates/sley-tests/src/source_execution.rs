//! Pure owner-side execution of a root-bound native `TestCase`.
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
use sley_id::{EntityId, NativeTestPlanId};
use sley_mutate::{
    EntityObject,
    semantic_projection::{SemanticInventory, SemanticProjectionError, project_semantic_inventory},
};
use sley_scb1::ScbError;
use sley_ssmc::{
    Block, ExpectedOutcome, FunctionGraph, Operation, Parameter, ParameterRole, TestCaseDefinition,
    fingerprint::{FingerprintError, hash_validated_value},
};
use sley_state_root::AcceptedStateRoot;
use sley_vm::{
    CacheProfile, LoweringInput,
    native_execution::{
        NativeDeclaredLimits, NativeExecutionError, NativeExecutionInput, NativeExecutionOutcome,
        NativeExecutionRequestV1, NativeImplementationLimits, execute_native_function, profile_id,
    },
};

use crate::{
    MAX_EXECUTION_REPORT_STORED, NativeExecutionEvidence, NativeExecutionReportParts,
    NativeExecutionReportV1, NativeExpected, NativeTestEntry, SelectedEntry,
    compare_native_expected, execution_report_capacity_required, rejected_from_error,
};

/// Failure before an unmeasured native VM result can be returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeTestSourceError {
    /// The complete object inventory did not project onto semantic entities.
    Projection(SemanticProjectionError),
    /// The complete type environment was invalid.
    Type(TypeError),
    /// The existing contract/test checker refused the selected test.
    Static(ContractTestValidationError),
    /// Root bindings, selected identities, or projected inventories disagree.
    SourceMismatch,
    /// Native VM lowering, input, effect, or execution refusal.
    Vm(NativeExecutionError),
    /// The worker's ordered input hashes differ from VM-derived hashes.
    InputHashesMismatch,
    /// The complete execution report cannot fit its reserved bound.
    EvidenceLimitExceeded,
    /// The checked expected value could not be fingerprinted.
    ExpectedFingerprint(FingerprintError),
    /// Report construction failed after successful reservation.
    ReportBuild(ScbError),
}

impl fmt::Display for NativeTestSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(error) => error.fmt(formatter),
            Self::Type(error) => error.fmt(formatter),
            Self::Static(error) => error.fmt(formatter),
            Self::SourceMismatch => formatter.write_str("NATIVE_TEST_SOURCE_MISMATCH"),
            Self::Vm(error) => error.fmt(formatter),
            Self::InputHashesMismatch => formatter.write_str("NATIVE_TEST_INPUT_HASH_MISMATCH"),
            Self::EvidenceLimitExceeded => {
                formatter.write_str("NATIVE_TEST_EVIDENCE_LIMIT_EXCEEDED")
            }
            Self::ExpectedFingerprint(error) => error.fmt(formatter),
            Self::ReportBuild(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeTestSourceError {}

/// One unmeasured native execution and its canonical comparison evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTestEvaluation {
    /// Value-bearing result built only by the VM.
    pub outcome: NativeExecutionOutcome,
    /// Canonical observed execution report bound to one selected plan entry.
    pub execution_report: NativeExecutionReportV1,
    /// Hash-only expected value and exact comparison for later aggregation.
    pub entry: NativeTestEntry,
}

struct OwnedUnit<'a> {
    function: &'a FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

fn function_units(
    entities: &SemanticInventory,
) -> Result<Vec<OwnedUnit<'_>>, NativeTestSourceError> {
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
                .ok_or(NativeTestSourceError::SourceMismatch)?,
        };
        let index = *indices
            .get(&function)
            .ok_or(NativeTestSourceError::SourceMismatch)?;
        units[index].parameters.push(parameter.clone());
    }
    for block in &entities.blocks {
        let index = *indices
            .get(&block.function)
            .ok_or(NativeTestSourceError::SourceMismatch)?;
        units[index].blocks.push(block.clone());
    }
    for operation in &entities.operations {
        let function = block_functions
            .get(&operation.block)
            .ok_or(NativeTestSourceError::SourceMismatch)?;
        let index = *indices
            .get(function)
            .ok_or(NativeTestSourceError::SourceMismatch)?;
        units[index].operations.push(operation.clone());
    }
    Ok(units)
}

/// Complete pure source for one selected native test.
#[derive(Clone, Copy)]
pub struct NativeTestSourceInput<'a> {
    /// Proposed root and its exact entity-to-object bindings.
    pub root: &'a AcceptedStateRoot,
    /// Complete live objects in raw entity-ID order.
    pub objects: &'a [EntityObject],
    /// Owner-derived selected plan entry.
    pub selected: SelectedEntry,
    /// Separately bound implementation ceilings.
    pub implementation_limits: NativeImplementationLimits,
    /// Ordered input hashes expected by the enclosing worker request.
    pub input_hashes: &'a [[u8; 32]],
}

fn bound_source(source: &NativeTestSourceInput<'_>) -> bool {
    let bindings = &source.root.record.entity_bindings;
    if source.objects.len() != bindings.len() {
        return false;
    }
    for (object, (entity, object_id)) in source.objects.iter().zip(bindings) {
        if object.schema_epoch_id() != source.root.record.schema_epoch_id
            || object.record().entity_id != *entity
            || object.object_id() != *object_id
        {
            return false;
        }
    }
    let selected = source.selected;
    let exact = |entity: EntityId| {
        source
            .objects
            .binary_search_by_key(&entity, |object| object.record().entity_id)
            .ok()
            .map(|index| source.objects[index].object_id())
    };
    exact(selected.test_entity) == Some(selected.test_object)
        && exact(selected.target_function) == Some(selected.target_object)
}

fn reserve_execution_report(
    input_count: usize,
    declared: NativeDeclaredLimits,
    implementation: NativeImplementationLimits,
) -> Result<(), NativeTestSourceError> {
    let required = execution_report_capacity_required(input_count, declared, implementation)
        .map_err(|_| NativeTestSourceError::EvidenceLimitExceeded)?;
    if required > MAX_EXECUTION_REPORT_STORED as u64 {
        return Err(NativeTestSourceError::EvidenceLimitExceeded);
    }
    Ok(())
}

fn selected_sources(
    entities: &SemanticInventory,
    selected: SelectedEntry,
) -> Result<(&FunctionGraph, &TestCaseDefinition), NativeTestSourceError> {
    let target = entities
        .functions
        .binary_search_by_key(&selected.target_function, |function| function.entity_id)
        .ok()
        .map(|index| &entities.functions[index])
        .ok_or(NativeTestSourceError::SourceMismatch)?;
    let test = entities
        .tests
        .binary_search_by_key(&selected.test_entity, |test| test.entity_id)
        .ok()
        .map(|index| &entities.tests[index])
        .ok_or(NativeTestSourceError::SourceMismatch)?;
    Ok((target, test))
}

fn execute_source_inner(
    source: NativeTestSourceInput<'_>,
) -> Result<(NativeExecutionOutcome, NativeExpected), NativeTestSourceError> {
    if !bound_source(&source) {
        return Err(NativeTestSourceError::SourceMismatch);
    }
    let selected = source.selected;
    let entities =
        project_semantic_inventory(source.objects).map_err(NativeTestSourceError::Projection)?;
    let types = TypeEnvironment::new(entities.type_definitions.clone())
        .map_err(NativeTestSourceError::Type)?;
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
    .map_err(NativeTestSourceError::Static)?;
    if !static_report.selected_tests.contains(&selected.test_entity) {
        return Err(NativeTestSourceError::SourceMismatch);
    }
    let (target, test) = selected_sources(&entities, selected)?;
    let limits = test.resource_limits;
    if test.target != selected.target_function
        || NativeDeclaredLimits::from(limits) != selected.declared_limits
    {
        return Err(NativeTestSourceError::SourceMismatch);
    }
    let expected = match &test.expected {
        ExpectedOutcome::Value(value) => NativeExpected::Value(
            hash_validated_value(source.root.record.schema_epoch_id, value)
                .map_err(NativeTestSourceError::ExpectedFingerprint)?,
        ),
        ExpectedOutcome::FailureCode(code) => NativeExpected::FailureCode(*code),
    };
    let input = NativeExecutionInput {
        lowering: LoweringInput {
            types: &types,
            function: target,
            parameters: &entities.parameters,
            blocks: &entities.blocks,
            operations: &entities.operations,
            schema_epoch: source.root.record.schema_epoch_id,
            state_root: source.root.root,
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
        declared_limits: selected.declared_limits,
        implementation_limits: source.implementation_limits,
        profile_id: profile_id(),
    };
    reserve_execution_report(
        test.inputs.len(),
        selected.declared_limits,
        source.implementation_limits,
    )?;
    let outcome = execute_native_function(input, request).map_err(NativeTestSourceError::Vm)?;
    let observed = outcome.observation().input_hashes();
    if observed.len() != source.input_hashes.len()
        || observed
            .iter()
            .zip(source.input_hashes)
            .any(|(actual, declared)| actual.as_bytes() != declared)
    {
        return Err(NativeTestSourceError::InputHashesMismatch);
    }
    Ok((outcome, expected))
}

/// Runs one selected native Sley test through the existing pure VM.
///
/// This rechecks root/object bindings, the selected test against the complete
/// source graph, and VM-derived ordered input hashes. Its result alone never
/// admits a test; host measurement remains the supervisor's responsibility.
///
/// # Errors
///
/// Refuses mismatched input, invalid static source, or an exact VM failure.
pub fn execute_native_test_source(
    source: NativeTestSourceInput<'_>,
) -> Result<NativeExecutionOutcome, NativeTestSourceError> {
    execute_source_inner(source).map(|(outcome, _)| outcome)
}

/// Runs the pure VM and builds one canonical observed report and comparison.
///
/// Report bytes and expectation comparison are diagnostic until protected
/// plan, supervisor measurement, and admission checks are all present.
///
/// # Errors
///
/// Refuses invalid source, VM failures, or a report-building defect.
pub fn evaluate_native_test_source(
    source: NativeTestSourceInput<'_>,
    plan_id: NativeTestPlanId,
) -> Result<NativeTestEvaluation, NativeTestSourceError> {
    let (outcome, expected) = execute_source_inner(source)?;
    let evidence = NativeExecutionEvidence::observed(outcome.observation().stored_bytes().to_vec())
        .map_err(NativeTestSourceError::ReportBuild)?;
    let selected = source.selected;
    let execution_report = build_execution_report(source, plan_id, evidence)?;
    let comparison = compare_native_expected(expected, outcome.observation().termination());
    let entry = NativeTestEntry {
        test_entity: selected.test_entity,
        test_object: selected.test_object,
        execution_report_id: execution_report.report_id(),
        expected,
        comparison,
    };
    Ok(NativeTestEvaluation {
        outcome,
        execution_report,
        entry,
    })
}

fn build_execution_report(
    source: NativeTestSourceInput<'_>,
    plan_id: NativeTestPlanId,
    evidence: NativeExecutionEvidence,
) -> Result<NativeExecutionReportV1, NativeTestSourceError> {
    let selected = source.selected;
    NativeExecutionReportV1::build(NativeExecutionReportParts {
        plan_id,
        test_entity: selected.test_entity,
        test_object: selected.test_object,
        target_object: selected.target_object,
        evidence,
    })
    .map_err(NativeTestSourceError::ReportBuild)
}

/// Records one pure native test as observed or as a VM-owned rejection.
///
/// Malformed or mismatched source still returns an error: it cannot be
/// represented as an execution refusal from an authenticated test. This
/// report alone carries no supervisor measurement or admission authority.
///
/// # Errors
///
/// Refuses invalid source or a report-building defect.
pub fn report_native_test_source(
    source: NativeTestSourceInput<'_>,
    plan_id: NativeTestPlanId,
) -> Result<NativeExecutionReportV1, NativeTestSourceError> {
    let evidence = match execute_source_inner(source) {
        Ok((outcome, _)) => {
            NativeExecutionEvidence::observed(outcome.observation().stored_bytes().to_vec())
                .map_err(NativeTestSourceError::ReportBuild)?
        }
        Err(NativeTestSourceError::Vm(error)) => NativeExecutionEvidence::Rejected(
            rejected_from_error(&error).map_err(NativeTestSourceError::ReportBuild)?,
        ),
        Err(error) => return Err(error),
    };
    build_execution_report(source, plan_id, evidence)
}

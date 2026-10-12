//! C0-side native harness: executes seed programs through the existing
//! reference lowering and execution path.

use sley_check::TypeEnvironment;
use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::ConstValue;
use sley_vm::{
    CacheProfile, ExecutionError, ExecutionLimits, ExecutionOutcome, ExecutionRequest,
    LoweredFunction, LoweringError, LoweringInput,
};

use crate::seed::Program;

/// Fixed development epoch (the bootstrap closure-evidence convention).
#[must_use]
pub fn dev_epoch() -> SchemaEpochId {
    SchemaEpochId::from_bytes([8; 32])
}

/// Fixed development root.
#[must_use]
pub fn dev_root() -> StateRoot {
    StateRoot::from_bytes([9; 32])
}

/// Generous development limits.
#[must_use]
pub fn big_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: u64::MAX / 4,
        max_fuel: u64::MAX / 4,
        max_value_units: u64::MAX / 4,
        max_output_units: u64::MAX / 4,
        cancel_at_fuel: None,
    }
}

/// The program's type environment.
///
/// # Panics
///
/// Panics when the type definitions do not form a valid environment.
#[must_use]
pub fn type_environment(program: &Program) -> TypeEnvironment {
    TypeEnvironment::new(program.type_definitions.clone()).expect("seed type environment")
}

fn lowering_input<'a>(
    program: &'a Program,
    types: &'a TypeEnvironment,
    entry: EntityId,
) -> LoweringInput<'a> {
    let function = program
        .functions
        .iter()
        .find(|f| f.entity_id == entry)
        .expect("entry function exists");
    LoweringInput {
        types,
        function,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        schema_epoch: dev_epoch(),
        state_root: dev_root(),
        profile: CacheProfile::EXTENDED_V1,
        constants: &program.constants,
        globals: &[],
        functions: &program.functions,
        contracts: &[],
        adapters: &program.imports,
    }
}

/// Lowers `entry` and its callee closure natively.
///
/// # Errors
///
/// Returns the native lowering failure.
pub fn lower(program: &Program, entry: EntityId) -> Result<LoweredFunction, LoweringError> {
    let types = type_environment(program);
    sley_vm::lower_function(lowering_input(program, &types, entry))
}

/// Lowers and executes `entry` natively.
///
/// # Errors
///
/// Returns the native lowering or execution failure.
pub fn execute(
    program: &Program,
    entry: EntityId,
    inputs: Vec<ConstValue>,
    limits: ExecutionLimits,
) -> Result<ExecutionOutcome, ExecutionError> {
    let types = type_environment(program);
    sley_vm::execute_function(
        lowering_input(program, &types, entry),
        ExecutionRequest { inputs, limits },
    )
}

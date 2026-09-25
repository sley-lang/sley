//! The advisory dev loop: in-process execution through `sley_vm`.
//!
//! Each function is lowered once per program state and every input runs
//! against the lowered image (`execute_loaded_image`), so a batch pays for
//! lowering once. `TestCases` are compared with the kernel's own rule: value
//! hashes (`hash_validated_value`) or trap codes, through
//! `sley_tests::compare_expected_evidence`. Results are advisory: never
//! admission evidence, never signed, never committed.

use std::collections::BTreeMap;
use std::time::Instant;

use serde_json::{Value, json};
use sley_check::TypeEnvironment;
use sley_id::EntityId;
use sley_policy::complete_entities::{CompleteEntities, project_complete_entities};
use sley_ssmc::{ConstValue, ExpectedOutcome, FunctionGraph, TestCaseDefinition, TypeExpr};
use sley_tests::{ExpectedEvidence, RestrictedComparison, compare_expected_evidence};
use sley_vm::host_abi::image_digest;
use sley_vm::{
    ApprovedImage, CacheProfile, ExecutionLimits, ExecutionRequest, ExecutionTermination,
    LoadedExecutionInput, LoweringInput, lower_function,
};

use crate::error::{AgentError, AgentErrorCode, Result};
use crate::names::Names;
use crate::values;
use crate::workspace::Program;

/// Dev-loop instruction ceiling for `call`.
pub const CALL_INSTRUCTIONS: u64 = 100_000_000;
/// Dev-loop fuel ceiling for `call`.
pub const CALL_FUEL: u64 = 100_000_000;
/// Dev-loop value-unit ceiling (above the image size the VM pre-charges).
pub const CALL_VALUE_UNITS: u64 = 1_000_000_000;
/// Dev-loop output-unit ceiling.
pub const CALL_OUTPUT_UNITS: u64 = 16_777_216;

/// The limits `call` runs under.
#[must_use]
pub const fn call_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: CALL_INSTRUCTIONS,
        max_fuel: CALL_FUEL,
        max_value_units: CALL_VALUE_UNITS,
        max_output_units: CALL_OUTPUT_UNITS,
        cancel_at_fuel: None,
    }
}

struct Lowered {
    bytes: Vec<u8>,
    approved: ApprovedImage,
}

/// One execution result.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// How execution ended.
    pub termination: ExecutionTermination,
    /// Fuel charged.
    pub fuel: u64,
    /// Instructions executed.
    pub instructions: u64,
    /// VM wall time in microseconds (lowering excluded).
    pub micros: u64,
}

/// A lower-once executor over one program state.
pub struct Executor {
    epoch: sley_id::SchemaEpochId,
    root: sley_id::StateRoot,
    entities: CompleteEntities,
    types: TypeEnvironment,
    functions: BTreeMap<EntityId, usize>,
    lowered: BTreeMap<EntityId, Lowered>,
}

fn refused(detail: impl Into<String>) -> AgentError {
    AgentError::new(AgentErrorCode::ExecutionRefused, detail)
}

impl Executor {
    /// Projects a program state for execution.
    ///
    /// # Errors
    ///
    /// `AGENT_EXECUTION_REFUSED` when the state does not project or its
    /// types do not form an environment (a state that validation refuses).
    pub fn new(program: &Program) -> Result<Self> {
        let entities = project_complete_entities(program.objects())
            .map_err(|error| refused(format!("the program does not project: {error}")))?;
        let types = TypeEnvironment::new(entities.type_definitions.clone())
            .map_err(|error| refused(format!("the program's types are invalid: {error}")))?;
        let functions = entities
            .functions
            .iter()
            .enumerate()
            .map(|(index, function)| (function.entity_id, index))
            .collect();
        Ok(Self {
            epoch: program.epoch(),
            root: program.root(),
            entities,
            types,
            functions,
            lowered: BTreeMap::new(),
        })
    }

    /// Returns a function graph.
    #[must_use]
    pub fn function(&self, id: &EntityId) -> Option<&FunctionGraph> {
        self.functions
            .get(id)
            .map(|index| &self.entities.functions[*index])
    }

    /// Returns a function's declared parameter types.
    #[must_use]
    pub fn parameter_types(&self, id: &EntityId) -> Vec<TypeExpr> {
        let Some(function) = self.function(id) else {
            return Vec::new();
        };
        function
            .parameters
            .iter()
            .filter_map(|param| {
                self.entities
                    .parameters
                    .iter()
                    .find(|candidate| candidate.entity_id == *param)
                    .map(|param| param.value_type.clone())
            })
            .collect()
    }

    /// Returns every `TestCase` of the state.
    #[must_use]
    pub fn tests(&self) -> &[TestCaseDefinition] {
        &self.entities.tests
    }

    fn lowering_input<'a>(&'a self, function: &'a FunctionGraph) -> LoweringInput<'a> {
        LoweringInput {
            types: &self.types,
            function,
            parameters: &self.entities.parameters,
            blocks: &self.entities.blocks,
            operations: &self.entities.operations,
            schema_epoch: self.epoch,
            state_root: self.root,
            profile: CacheProfile::EXTENDED_V1,
            constants: &self.entities.constants,
            globals: &self.entities.globals,
            functions: &self.entities.functions,
            contracts: &self.entities.contracts,
            adapters: &self.entities.adapters,
        }
    }

    fn lower(&mut self, id: &EntityId) -> Result<()> {
        if self.lowered.contains_key(id) {
            return Ok(());
        }
        let function = self
            .function(id)
            .ok_or_else(|| refused("not a live function"))?;
        let lowered = lower_function(self.lowering_input(function))
            .map_err(|error| refused(format!("lowering refused: {error}")))?;
        let approved = ApprovedImage {
            digest: image_digest(&lowered.bytes),
            cache_key: lowered.cache_key,
            imports: self
                .entities
                .adapters
                .iter()
                .map(|row| row.entity_id)
                .collect(),
        };
        self.lowered.insert(
            *id,
            Lowered {
                bytes: lowered.bytes,
                approved,
            },
        );
        Ok(())
    }

    /// Executes a function on typed inputs, lowering it on first use.
    ///
    /// # Errors
    ///
    /// `AGENT_EXECUTION_REFUSED` when lowering or input admission refuses.
    /// Traps and resource limits are outcomes, not errors.
    pub fn run(
        &mut self,
        id: &EntityId,
        inputs: Vec<ConstValue>,
        limits: ExecutionLimits,
    ) -> Result<Outcome> {
        self.lower(id)?;
        let lowered = &self.lowered[id];
        let input = LoadedExecutionInput {
            types: &self.types,
            constants: &self.entities.constants,
            globals: &self.entities.globals,
            contracts: &self.entities.contracts,
            adapters: &self.entities.adapters,
            schema_epoch: self.epoch,
            state_root: self.root,
            profile: CacheProfile::EXTENDED_V1,
        };
        let started = Instant::now();
        let outcome = sley_vm::execute_loaded_image(
            input,
            &lowered.approved,
            &lowered.bytes,
            ExecutionRequest { inputs, limits },
        )
        .map_err(|error| refused(format!("execution refused: {error}")))?;
        let micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
        Ok(Outcome {
            termination: outcome.termination,
            fuel: outcome.fuel_used,
            instructions: outcome.instruction_count,
            micros,
        })
    }

    /// Runs one `TestCase` under its declared fuel and compares with the
    /// kernel's rule.
    pub fn run_test(&mut self, test: &TestCaseDefinition, names: &Names) -> TestOutcome {
        let limits = ExecutionLimits {
            max_fuel: test.resource_limits.fuel,
            ..call_limits()
        };
        let expected_text = match &test.expected {
            ExpectedOutcome::Value(value) => values::to_text(value, names),
            ExpectedOutcome::FailureCode(code) => format!("trap({code})"),
        };
        let expected = match &test.expected {
            ExpectedOutcome::Value(value) => {
                match sley_ssmc::fingerprint::hash_validated_value(self.epoch, value) {
                    Ok(hash) => ExpectedEvidence::Value(hash),
                    Err(error) => {
                        return TestOutcome::rejected(
                            test,
                            expected_text,
                            format!("expected value: {error}"),
                        );
                    }
                }
            }
            ExpectedOutcome::FailureCode(code) => ExpectedEvidence::FailureCode(*code),
        };
        let outcome = match self.run(&test.target, test.inputs.clone(), limits) {
            Ok(outcome) => outcome,
            Err(error) => {
                return TestOutcome::rejected(test, expected_text, error.detail().to_owned());
            }
        };
        let (success, trap) = match &outcome.termination {
            ExecutionTermination::Success(value) => (
                sley_ssmc::fingerprint::hash_validated_value(self.epoch, value).ok(),
                None,
            ),
            ExecutionTermination::Trap { trap_tag, .. } => (None, Some(*trap_tag)),
            _ => (None, None),
        };
        let comparison = compare_expected_evidence(expected, success, trap, true);
        TestOutcome {
            test: test.entity_id,
            target: test.target,
            comparison,
            expected: expected_text,
            actual: termination_text(&outcome.termination, names),
            fuel: outcome.fuel,
        }
    }
}

/// One `TestCase` result.
#[derive(Clone, Debug)]
pub struct TestOutcome {
    /// `TestCase` identity.
    pub test: EntityId,
    /// Target function.
    pub target: EntityId,
    /// Kernel comparison.
    pub comparison: RestrictedComparison,
    /// Expected, as AV1 text.
    pub expected: String,
    /// Observed, as AV1 text.
    pub actual: String,
    /// Fuel charged.
    pub fuel: u64,
}

impl TestOutcome {
    fn rejected(test: &TestCaseDefinition, expected: String, detail: String) -> Self {
        Self {
            test: test.entity_id,
            target: test.target,
            comparison: RestrictedComparison::ExecutionRejected,
            expected,
            actual: detail,
            fuel: 0,
        }
    }

    /// Whether the test passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.comparison == RestrictedComparison::Match
    }
}

/// Renders a termination as AV1 text.
#[must_use]
pub fn termination_text(termination: &ExecutionTermination, names: &Names) -> String {
    match termination {
        ExecutionTermination::Success(value) => values::to_text(value, names),
        ExecutionTermination::Trap { trap_tag, payload } => match payload {
            Some(payload) => format!("trap({trap_tag}, {})", values::to_text(payload, names)),
            None => format!("trap({trap_tag})"),
        },
        ExecutionTermination::ResourceLimit(kind) => format!("resource limit ({kind:?})"),
        ExecutionTermination::Cancelled => "cancelled".to_owned(),
        ExecutionTermination::InternalInvariant => "internal invariant".to_owned(),
    }
}

/// The trap code a name denotes, as expectations write it.
#[must_use]
pub fn trap_code(name: &str) -> Option<u32> {
    match name {
        "unreachable" => Some(1),
        "resource_exhausted" => Some(2),
        "adapter_contract_violation" => Some(3),
        "internal_invariant" => Some(4),
        _ => None,
    }
}

/// Renders a termination as compact JSON (`{"Ok":10}`, `{"trap":1}`).
#[must_use]
pub fn termination_json(termination: &ExecutionTermination, names: &Names) -> Value {
    match termination {
        ExecutionTermination::Success(value) => values::to_json(value, names),
        ExecutionTermination::Trap { trap_tag, payload } => match payload {
            Some(payload) => json!({"trap": trap_tag, "payload": values::to_json(payload, names)}),
            None => json!({"trap": trap_tag}),
        },
        ExecutionTermination::ResourceLimit(kind) => json!({"limit": format!("{kind:?}")}),
        ExecutionTermination::Cancelled => json!({"cancelled": true}),
        ExecutionTermination::InternalInvariant => json!({"internal_invariant": true}),
    }
}

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

/// The build profile `P`: the limits a toolchain package is admitted under
/// (and therefore the limits every toolchain run uses).
#[must_use]
pub fn toolchain_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_instructions: 1 << 62,
        max_fuel: 1 << 62,
        max_value_units: 1 << 62,
        max_output_units: 1 << 62,
        cancel_at_fuel: None,
    }
}

/// One native (C0) build of canonical state.
#[derive(Clone, Debug)]
pub struct C0Build {
    /// The execution package.
    pub package: sley_vm::ExecutionPackage,
    /// The canonical `EXEC_PACKAGE_V3` envelope bytes.
    pub envelope: Vec<u8>,
    /// Package digests.
    pub digests: sley_vm::PackageDigests,
}

/// The program inventories decoded from canonical state, in entity order.
#[derive(Clone, Debug, Default)]
pub struct Decoded {
    /// The program (functions, parameters, blocks, operations, constants,
    /// type definitions, imports).
    pub program: Program,
    /// Entry-point bodies: (entity, function).
    pub entry_points: Vec<(EntityId, EntityId)>,
}

/// Decodes every object of `s` with the native codec.
///
/// # Errors
///
/// Returns a description of the first refused object.
pub fn decode_s(s: &crate::canonical::CanonicalS) -> Result<Decoded, String> {
    use sley_mutate::value::EntityBodyValue as B;
    let mut out = Decoded::default();
    for object in &s.objects {
        let imported = sley_mutate::import_entity_object(s.epoch, object.stored_bytes())
            .map_err(|e| format!("object import: {e:?}"))?;
        let id = imported.record().entity_id;
        match imported.record().body.clone() {
            B::Function(f) => out.program.functions.push(sley_ssmc::FunctionGraph {
                entity_id: id,
                type_parameters: f.type_parameters,
                parameters: f.parameters,
                result_type: f.result_type,
                effects: f.effects.as_slice().to_vec(),
                entry_block: f.entry_block,
                blocks: f.blocks,
                contracts: f.contracts.as_slice().to_vec(),
                visibility: f.visibility,
            }),
            B::Parameter(p) => out.program.parameters.push(sley_ssmc::Parameter {
                entity_id: id,
                owner: p.owner,
                role: p.role,
                ordinal: p.ordinal,
                value_type: p.value_type,
            }),
            B::Block(b) => out.program.blocks.push(sley_ssmc::Block {
                entity_id: id,
                function: b.function,
                parameters: b.parameters,
                operations: b.operations,
                terminator: b.terminator,
                reachability: b.reachability,
            }),
            B::Operation(o) => out.program.operations.push(sley_ssmc::Operation {
                entity_id: id,
                block: o.block,
                ordinal: o.ordinal,
                opcode: sley_ssmc::Opcode::from_tag(o.opcode)
                    .ok_or_else(|| format!("unknown opcode {}", o.opcode))?,
                operands: o.operands,
                result_types: o.result_types,
                immediate: o.immediate,
            }),
            B::Constant(c) => out.program.constants.push(sley_ssmc::ConstantDefinition {
                entity_id: id,
                value: c.value,
            }),
            B::TypeDef(d) => out
                .program
                .type_definitions
                .push(sley_ssmc::TypeDefinition {
                    entity_id: id,
                    type_parameters: d.type_parameters,
                    form: d.form,
                    invariants: d.invariants.as_slice().to_vec(),
                    visibility: d.visibility,
                }),
            B::AdapterImport(i) => out.program.imports.push(sley_ssmc::AdapterImport {
                entity_id: id,
                adapter_id: i.adapter_id,
                abi_version: i.abi_version,
                request_type: i.request_type,
                response_type: i.response_type,
                failure_type: i.failure_type,
                effects: i.effects.as_slice().to_vec(),
            }),
            B::EntryPoint(e) => out.entry_points.push((id, e.function)),
            B::Workspace(_) | B::Package(_) | B::Namespace(_) => {}
            other => return Err(format!("kind outside the toolchain domain: {other:?}")),
        }
    }
    Ok(out)
}

/// `C0.build(S, P)`: the native seed build of canonical state into an
/// `EXEC_PACKAGE_V3` toolchain package.
///
/// # Errors
///
/// Returns a description of the first native refusal.
pub fn c0_build(
    s: &crate::canonical::CanonicalS,
    limits: ExecutionLimits,
) -> Result<C0Build, String> {
    use sley_vm::bootstrap::{BootstrapProfileInput, BootstrapProfileVersion};
    // The root binds exactly the supplied objects.
    let bound: Vec<(EntityId, sley_id::ObjectId)> = s
        .objects
        .iter()
        .map(|object| (object.record().entity_id, object.object_id()))
        .collect();
    if bound != s.root.record.entity_bindings {
        return Err("the root does not bind exactly the supplied objects".into());
    }
    let decoded = decode_s(s)?;
    let entry_point = s
        .root
        .record
        .entry_points
        .first()
        .ok_or("the root names no entry point")?;
    let entry = decoded
        .entry_points
        .iter()
        .find(|(id, _)| id == entry_point)
        .map(|(_, function)| *function)
        .ok_or("the entry point is not an EntryPoint entity")?;
    let program = &decoded.program;
    let types = type_environment(program);
    let function = program
        .functions
        .iter()
        .find(|f| f.entity_id == entry)
        .ok_or("the entry function is absent")?;
    let lowered = sley_vm::lower_function(LoweringInput {
        types: &types,
        function,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        schema_epoch: s.epoch,
        state_root: s.root.root,
        profile: CacheProfile::EXTENDED_V1,
        constants: &program.constants,
        globals: &[],
        functions: &program.functions,
        contracts: &[],
        adapters: &program.imports,
    })
    .map_err(|e| format!("lowering: {e:?}"))?;
    let gate = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &types,
        schema_epoch: s.epoch,
        entry: function,
        presented_image_bytes: &lowered.bytes,
        functions: &program.functions,
        parameters: &program.parameters,
        blocks: &program.blocks,
        operations: &program.operations,
        adapters: &program.imports,
        constants: &program.constants,
        profile_version: BootstrapProfileVersion::V3,
    })
    .map_err(|e| format!("bootstrap gate: {e:?}"))?;
    let package = sley_vm::ExecutionPackage {
        image_bytes: lowered.bytes,
        constants: program.constants.clone(),
        type_definitions: program.type_definitions.clone(),
        imports: program.imports.clone(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry,
        schema_epoch: s.epoch,
        state_root: s.root.root,
        profile: CacheProfile::EXTENDED_V1,
        admitted_limits: limits,
        gate_operation_count: gate.operation_count(),
        gate_bridge_uses: gate.bridge_uses(),
        gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
    };
    let digests = sley_vm::package_digests_v3(&package).map_err(|e| format!("digests: {e:?}"))?;
    let envelope =
        sley_vm::encode_package_envelope_v3(&package).map_err(|e| format!("envelope: {e:?}"))?;
    Ok(C0Build {
        package,
        envelope,
        digests,
    })
}

/// Lowers an entry once and executes the loaded image repeatedly (tests run
/// one function over many inputs; re-lowering per run dominates otherwise).
pub struct Runner<'a> {
    program: &'a Program,
    types: TypeEnvironment,
    bytes: Vec<u8>,
    expected: sley_vm::ApprovedImage,
}

impl<'a> Runner<'a> {
    /// Lowers `entry` of `program`.
    ///
    /// # Panics
    ///
    /// Panics when lowering refuses.
    #[must_use]
    pub fn new(program: &'a Program, entry: EntityId) -> Self {
        let lowered = lower(program, entry).expect("entry lowers");
        let expected = sley_vm::ApprovedImage {
            digest: sley_vm::host_abi::image_digest(&lowered.bytes),
            cache_key: lowered.cache_key,
            imports: program.imports.iter().map(|row| row.entity_id).collect(),
        };
        Self {
            program,
            types: type_environment(program),
            bytes: lowered.bytes,
            expected,
        }
    }

    /// Executes the lowered entry.
    ///
    /// # Errors
    ///
    /// The loaded-image refusal.
    pub fn run(
        &self,
        inputs: Vec<ConstValue>,
        limits: ExecutionLimits,
    ) -> Result<ExecutionOutcome, sley_vm::LoadedExecutionError> {
        sley_vm::execute_loaded_image(
            sley_vm::LoadedExecutionInput {
                types: &self.types,
                constants: &self.program.constants,
                globals: &[],
                contracts: &[],
                adapters: &self.program.imports,
                schema_epoch: dev_epoch(),
                state_root: dev_root(),
                profile: CacheProfile::EXTENDED_V1,
            },
            &self.expected,
            &self.bytes,
            sley_vm::ExecutionRequest { inputs, limits },
        )
    }
}

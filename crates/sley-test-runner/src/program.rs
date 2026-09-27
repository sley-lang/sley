//! Bounded portable input snapshot for one selected native `TestCase`.
//!
//! The artifact carries the exact owner-derived plan, accepted/proposed
//! state root, and every live entity object bound by that root. Parsing uses
//! the existing core codecs and verifies object identities, complete root
//! bindings, the selected `TestCase`, its target `Function`, and literal limits.
//! It grants no plan, policy, commit, or measurement authority: the owner
//! still derives the plan and the isolated worker still must execute it.

use sley_id::EntityId;
use sley_mutate::{EntityObject, import_entity_object, value::EntityBodyValue};
use sley_scb1::{ScbError, ScbErrorCode, ScbValueCursor, encode_list, encode_record, encode_uvar};
use sley_state_root::{AcceptedStateRoot, StateRootError, conformance_registry, import_state_root};
use sley_tests::{NativeTestPlanV1, SelectedEntry};

use crate::worker::MAX_WORKER_FRAME;

/// Internal portable program artifact magic.
pub const PROGRAM_MAGIC: &[u8; 8] = b"SLEYPRG1";
/// Internal portable program artifact version.
pub const PROGRAM_VERSION: u64 = 1;
/// Maximum artifact bytes; the surrounding worker frame needs separate room.
pub const MAX_PROGRAM_BYTES: usize = MAX_WORKER_FRAME - 4_096;

/// Strictly parsed and completely root-bound worker input snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortableTestProgram {
    plan: NativeTestPlanV1,
    root: AcceptedStateRoot,
    objects: Vec<EntityObject>,
    selected: SelectedEntry,
    stored: Vec<u8>,
}

fn mismatch() -> ScbError {
    ScbError::new(ScbErrorCode::ContractUnknown)
}

fn root_error(error: StateRootError) -> ScbError {
    match error {
        StateRootError::Scb(error) => error,
        StateRootError::StateRoot(_) | StateRootError::Schema(_) => mismatch(),
    }
}

fn read_id(value: &[u8]) -> Result<[u8; 32], ScbError> {
    <[u8; 32]>::try_from(value).map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))
}

fn selected_binding(
    plan: &NativeTestPlanV1,
    root: &AcceptedStateRoot,
    objects: &[EntityObject],
    test_entity: EntityId,
) -> Result<SelectedEntry, ScbError> {
    if plan.proposed_root() != root.root
        || plan.workspace() != root.record.workspace_id
        || plan.semantic_epoch() != root.record.schema_epoch_id
        || plan.policy_root() != root.record.policy_root
        || objects.len() != root.record.entity_bindings.len()
    {
        return Err(mismatch());
    }
    for (object, (entity, object_id)) in objects.iter().zip(&root.record.entity_bindings) {
        if object.schema_epoch_id() != root.record.schema_epoch_id
            || object.record().entity_id != *entity
            || object.object_id() != *object_id
        {
            return Err(mismatch());
        }
    }
    let selected = plan
        .selected()
        .binary_search_by_key(&test_entity, |entry| entry.test_entity)
        .ok()
        .map(|index| plan.selected()[index])
        .ok_or_else(mismatch)?;
    let test = objects
        .binary_search_by_key(&selected.test_entity, |object| object.record().entity_id)
        .ok()
        .map(|index| &objects[index])
        .ok_or_else(mismatch)?;
    let target = objects
        .binary_search_by_key(&selected.target_function, |object| {
            object.record().entity_id
        })
        .ok()
        .map(|index| &objects[index])
        .ok_or_else(mismatch)?;
    let EntityBodyValue::TestCase(body) = &test.record().body else {
        return Err(mismatch());
    };
    if !matches!(target.record().body, EntityBodyValue::Function(_))
        || test.object_id() != selected.test_object
        || target.object_id() != selected.target_object
        || body.target != selected.target_function
    {
        return Err(mismatch());
    }
    let literal = body.resource_limits;
    let declared = selected.declared_limits;
    if literal.fuel != declared.fuel
        || literal.memory_bytes != declared.memory_bytes
        || literal.output_bytes != declared.output_bytes
        || literal.effect_count != declared.effect_count
        || literal.call_depth != declared.call_depth
        || literal.wall_timeout_millis != declared.wall_timeout_millis
    {
        return Err(mismatch());
    }
    Ok(selected)
}

impl PortableTestProgram {
    /// Builds one canonical portable snapshot from exact core-owned values.
    ///
    /// # Errors
    ///
    /// Refuses any plan/root/object/test mismatch or the worker frame bound.
    pub fn build(
        plan: &NativeTestPlanV1,
        root: &AcceptedStateRoot,
        objects: &[EntityObject],
        test_entity: EntityId,
    ) -> Result<Self, ScbError> {
        let selected = selected_binding(plan, root, objects, test_entity)?;
        let object_bytes = objects
            .iter()
            .map(|object| object.stored_bytes().to_vec())
            .collect::<Vec<_>>();
        let record = encode_record(&[
            (1, encode_uvar(PROGRAM_VERSION)),
            (2, plan.stored_bytes().to_vec()),
            (3, test_entity.as_bytes().to_vec()),
            (4, root.stored_bytes.clone()),
            (5, encode_list(&object_bytes)?),
        ])?;
        let length =
            u32::try_from(record.len()).map_err(|_| ScbError::new(ScbErrorCode::ResourceLimit))?;
        let mut stored = Vec::with_capacity(12 + record.len());
        stored.extend_from_slice(PROGRAM_MAGIC);
        stored.extend_from_slice(&length.to_be_bytes());
        stored.extend_from_slice(&record);
        if stored.len() > MAX_PROGRAM_BYTES {
            return Err(ScbError::new(ScbErrorCode::ResourceLimit));
        }
        Ok(Self {
            plan: plan.clone(),
            root: root.clone(),
            objects: objects.to_vec(),
            selected,
            stored,
        })
    }

    /// Strictly parses and rechecks one complete portable snapshot.
    ///
    /// # Errors
    ///
    /// Refuses malformed framing, unsupported version, nested core codec
    /// errors, any root/selection substitution, or an oversized artifact.
    pub fn parse(stored: &[u8]) -> Result<Self, ScbError> {
        if stored.len() < 12 || stored.len() > MAX_PROGRAM_BYTES {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        if &stored[..8] != PROGRAM_MAGIC {
            return Err(ScbError::new(ScbErrorCode::MagicInvalid));
        }
        let length = u32::from_be_bytes(
            stored[8..12]
                .try_into()
                .map_err(|_| ScbError::new(ScbErrorCode::LengthOverflow))?,
        );
        if length as usize != stored.len() - 12 {
            return Err(ScbError::new(ScbErrorCode::LengthOverflow));
        }
        let mut cursor = ScbValueCursor::new(&stored[12..])?;
        if cursor.read_record_field_count()? != 5 {
            return Err(ScbError::new(ScbErrorCode::FieldMissing));
        }
        let mut fields = Vec::with_capacity(5);
        for expected_tag in 1..=5 {
            if cursor.read_uvar(32)? != expected_tag {
                return Err(ScbError::new(ScbErrorCode::FieldOrder));
            }
            fields.push(cursor.read_sized_payload()?);
        }
        cursor.check_finished()?;
        let mut version = ScbValueCursor::new(fields[0])?;
        if version.read_uvar(64)? != PROGRAM_VERSION {
            return Err(ScbError::new(ScbErrorCode::VersionUnsupported));
        }
        version.check_finished()?;
        let plan = NativeTestPlanV1::parse(fields[1])?;
        let test_entity = EntityId::from_bytes(read_id(fields[2])?);
        let registry = conformance_registry().map_err(|_| mismatch())?;
        let root = import_state_root(&registry, fields[3]).map_err(root_error)?;
        let mut objects_cursor = ScbValueCursor::new(fields[4])?;
        let count = objects_cursor.read_list_count()?;
        if count != root.record.entity_bindings.len() as u64 {
            return Err(mismatch());
        }
        let capacity =
            usize::try_from(count).map_err(|_| ScbError::new(ScbErrorCode::ResourceLimit))?;
        let mut objects = Vec::with_capacity(capacity);
        for _ in 0..count {
            objects.push(import_entity_object(
                root.record.schema_epoch_id,
                objects_cursor.read_bytes()?,
            )?);
        }
        objects_cursor.check_finished()?;
        let parsed = Self::build(&plan, &root, &objects, test_entity)?;
        if parsed.stored != stored {
            return Err(mismatch());
        }
        Ok(parsed)
    }

    /// Complete canonical portable bytes.
    #[must_use]
    pub fn stored_bytes(&self) -> &[u8] {
        &self.stored
    }

    /// Exact nested owner plan; parsing alone does not authenticate it.
    #[must_use]
    pub const fn plan(&self) -> &NativeTestPlanV1 {
        &self.plan
    }

    /// Exact root the bundled object inventory is bound to.
    #[must_use]
    pub const fn root(&self) -> &AcceptedStateRoot {
        &self.root
    }

    /// Complete live object inventory, sorted by entity identity.
    #[must_use]
    pub fn objects(&self) -> &[EntityObject] {
        &self.objects
    }

    /// Exact selected test and target bindings.
    #[must_use]
    pub const fn selected(&self) -> SelectedEntry {
        self.selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{protocol::RunRequest, worker::WorkerRequest};
    use sley_id::{
        CapabilitySummaryDigest, ObjectId, PolicyRootId, PrincipalId, TransactionId, WorkspaceId,
    };
    use sley_mutate::{
        EntityObjectRecord, build_entity_object,
        value::{BlockBody, EntityIdSet, FunctionBody, ParameterBody, TestCaseBody},
    };
    use sley_ssmc::{
        ConstData, ConstValue, EffectEnvironment, ExpectedOutcome, ParameterRole, Reachability,
        ResourceLimits, ReturnTerminator, Terminator, TypeExpr, ValueRef, Visibility,
    };
    use sley_state_root::StateRootBuilder;
    use sley_tests::{
        GrantCeilings, NativeAggregateLimits, NativeResourcePolicyParts, NativeResourcePolicyV1,
        NativeTestPlanParts, ValidationLimits, plan::SELECTION_MODE_EXPLICIT_ROOT,
    };
    use sley_vm::native_execution::{NativeDeclaredLimits, NativeImplementationLimits};

    fn id(byte: u8) -> EntityId {
        EntityId::from_bytes([byte; 32])
    }

    fn bool_value(value: bool) -> ConstValue {
        ConstValue {
            value_type: TypeExpr::Bool,
            data: ConstData::Bool(value),
        }
    }

    fn limits() -> ResourceLimits {
        ResourceLimits {
            fuel: 100,
            memory_bytes: 4_096,
            output_bytes: 64,
            effect_count: 0,
            call_depth: 8,
            wall_timeout_millis: 1_000,
        }
    }

    fn empty_set() -> EntityIdSet {
        EntityIdSet::from_unsorted(Vec::new()).expect("empty set")
    }

    fn object(
        epoch: sley_id::SchemaEpochId,
        entity: EntityId,
        body: EntityBodyValue,
    ) -> EntityObject {
        build_entity_object(
            epoch,
            &EntityObjectRecord {
                entity_id: entity,
                body,
                label: None,
                semantic_fingerprint: None,
            },
        )
        .expect("object")
    }

    fn resource_policy(policy: PolicyRootId) -> NativeResourcePolicyV1 {
        NativeResourcePolicyV1::build(NativeResourcePolicyParts {
            policy_root: policy,
            principal: PrincipalId::from_bytes([0; 32]),
            capability_summary: CapabilitySummaryDigest::from_bytes([3; 32]),
            grant: GrantCeilings {
                max_fuel: 1_000_000,
                max_memory_bytes: 16_777_216,
                max_output_bytes: 65_536,
                max_effect_count: 0,
                max_mutation_count: 100,
                max_adapter_calls: 0,
            },
            validation: ValidationLimits {
                max_operations: 1_000,
                max_preconditions: 64,
                max_candidate_bytes: 65_536,
                max_decoded_value_bytes: 65_536,
                max_graph_work: 100_000,
                max_selected_tests: 16,
                max_entities: 1_024,
                max_test_call_depth: 64,
                max_test_wall_timeout_millis: 5_000,
            },
            implementation: NativeImplementationLimits::HARD_MAXIMA,
            aggregate: NativeAggregateLimits::HARD_MAXIMA,
            admission_profile: [4; 32],
        })
        .expect("resource policy")
    }

    fn state_root(
        registry: &sley_schema::SchemaEpochRegistry<sley_state_root::StateRootEpoch1Decoder>,
        workspace: WorkspaceId,
        policy: PolicyRootId,
        function: EntityId,
        objects: &[EntityObject],
    ) -> AcceptedStateRoot {
        let mut builder = StateRootBuilder::new(
            workspace,
            ObjectId::from_bytes([90; 32]),
            objects[3].object_id(),
            policy,
        )
        .entry_point(function);
        for object in objects {
            builder = builder.entity_binding(object.record().entity_id, object.object_id());
        }
        builder.build(registry).expect("root")
    }

    fn fixture() -> (
        NativeTestPlanV1,
        AcceptedStateRoot,
        Vec<EntityObject>,
        EntityId,
    ) {
        let registry = conformance_registry().expect("root registry");
        let epoch = sley_state_root::conformance_epoch_id().expect("epoch");
        let workspace = WorkspaceId::from_bytes([1; 32]);
        let policy = PolicyRootId::from_bytes([2; 32]);
        let function = id(10);
        let parameter = id(11);
        let block = id(12);
        let test = id(13);
        let objects = vec![
            object(
                epoch,
                function,
                EntityBodyValue::Function(FunctionBody {
                    type_parameters: Vec::new(),
                    parameters: vec![parameter],
                    result_type: TypeExpr::Bool,
                    effects: empty_set(),
                    entry_block: block,
                    blocks: vec![block],
                    contracts: empty_set(),
                    visibility: Visibility::Exported,
                }),
            ),
            object(
                epoch,
                parameter,
                EntityBodyValue::Parameter(ParameterBody {
                    owner: function,
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: TypeExpr::Bool,
                }),
            ),
            object(
                epoch,
                block,
                EntityBodyValue::Block(BlockBody {
                    function,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return(ReturnTerminator {
                        value: ValueRef::Parameter(parameter),
                    }),
                    reachability: Reachability::Required,
                }),
            ),
            object(
                epoch,
                test,
                EntityBodyValue::TestCase(TestCaseBody {
                    target: function,
                    inputs: vec![bool_value(true)],
                    effect_environment: EffectEnvironment::Replay(Vec::new()),
                    expected: ExpectedOutcome::Value(bool_value(true)),
                    observations: Vec::new(),
                    resource_limits: limits(),
                }),
            ),
        ];
        let root = state_root(&registry, workspace, policy, function, &objects);
        let plan = NativeTestPlanV1::build(NativeTestPlanParts {
            selection_mode: SELECTION_MODE_EXPLICIT_ROOT,
            workspace,
            semantic_epoch: epoch,
            parent_transaction: TransactionId::from_bytes([5; 32]),
            parent_root: root.root,
            proposed_root: root.root,
            policy_root: policy,
            candidate_id: None,
            static_result_id: None,
            protected_required_ids: Vec::new(),
            selected: vec![SelectedEntry {
                test_entity: test,
                test_object: objects[3].object_id(),
                target_function: function,
                target_object: objects[0].object_id(),
                declared_limits: NativeDeclaredLimits {
                    fuel: 100,
                    memory_bytes: 4_096,
                    output_bytes: 64,
                    effect_count: 0,
                    call_depth: 8,
                    wall_timeout_millis: 1_000,
                },
            }],
            changed: Vec::new(),
            implementation_limits: NativeImplementationLimits::HARD_MAXIMA,
            static_selected_ids: Vec::new(),
            resource_policy: resource_policy(policy),
        })
        .expect("plan");
        (plan, root, objects, test)
    }

    #[test]
    fn portable_program_roundtrips_exact_root_bound_test() {
        let (plan, root, objects, test) = fixture();
        let built =
            PortableTestProgram::build(&plan, &root, &objects, test).expect("portable program");
        let parsed = PortableTestProgram::parse(built.stored_bytes()).expect("strict parse");
        assert_eq!(parsed, built);
        assert_eq!(parsed.plan().plan_id(), plan.plan_id());
        assert_eq!(parsed.root().root, root.root);
        assert_eq!(parsed.objects(), objects);
        assert_eq!(parsed.selected().test_entity, test);
    }

    #[test]
    fn portable_program_refuses_root_and_object_substitution() {
        let (plan, root, objects, test) = fixture();
        let mut wrong_root = root.clone();
        wrong_root.record.policy_root = PolicyRootId::from_bytes([99; 32]);
        assert_eq!(
            PortableTestProgram::build(&plan, &wrong_root, &objects, test)
                .expect_err("policy substitution")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut substituted = objects.clone();
        substituted[3] = object(
            root.record.schema_epoch_id,
            test,
            EntityBodyValue::TestCase(TestCaseBody {
                target: id(10),
                inputs: vec![bool_value(true)],
                effect_environment: EffectEnvironment::Replay(Vec::new()),
                expected: ExpectedOutcome::Value(bool_value(false)),
                observations: Vec::new(),
                resource_limits: limits(),
            }),
        );
        assert_eq!(
            PortableTestProgram::build(&plan, &root, &substituted, test)
                .expect_err("object substitution")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        let mut bytes = PortableTestProgram::build(&plan, &root, &objects, test)
            .expect("valid")
            .stored_bytes()
            .to_vec();
        let object_bytes = objects[3].stored_bytes();
        let offset = bytes
            .windows(object_bytes.len())
            .position(|part| part == object_bytes)
            .expect("test object bytes");
        bytes[offset + 20] ^= 1;
        assert!(PortableTestProgram::parse(&bytes).is_err());
    }

    #[test]
    fn outer_request_binds_the_portable_program_before_launch() {
        let (plan, root, objects, test) = fixture();
        let program = PortableTestProgram::build(&plan, &root, &objects, test).expect("program");
        let selected = program.selected();
        let worker_frame = WorkerRequest {
            program_bytes: program.stored_bytes().to_vec(),
            input_hashes: Vec::new(),
            declared_limits: selected.declared_limits,
            implementation_limits: plan.implementation_limits(),
        }
        .encode_frame()
        .expect("worker frame");
        let mut request = RunRequest {
            workspace: plan.workspace(),
            principal: plan.resource_policy().principal(),
            candidate_id: plan.candidate_id(),
            plan_id: plan.plan_id(),
            test_object: selected.test_object,
            test_entity: selected.test_entity,
            target_function: selected.target_function,
            policy_root: plan.policy_root(),
            declared_limits: selected.declared_limits,
            wall_ms: selected.declared_limits.wall_timeout_millis,
            nonce: [42; 32],
            worker_frame,
        };
        assert_eq!(request.verified_program().expect("bound"), program);
        request.plan_id = sley_id::NativeTestPlanId::from_bytes([99; 32]);
        assert_eq!(
            request
                .verified_program()
                .expect_err("different plan")
                .code(),
            ScbErrorCode::ContractUnknown
        );
        request.plan_id = plan.plan_id();
        request.test_entity = id(99);
        assert_eq!(
            request
                .verified_program()
                .expect_err("different test")
                .code(),
            ScbErrorCode::ContractUnknown
        );
    }
}

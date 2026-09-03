//! Complete-root request and closure judgment (S20-250 full, contract
//! `docs/spec/COMPLETE_ENTITY_IMPACT_PROFILE_V1.md` section 6).
//!
//! The judgment is pure: it consumes borrowed eighteen-kind bodies plus the
//! three root facts copied from one strictly decoded `StateRoot` record and
//! performs no I/O, no object decoding, and no cross-root lookup.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use sley_id::{EntityId, StateRoot};

use crate::{
    ImpactEntity, ImpactError, ImpactErrorCode, ImpactIndex, MAX_IMPACT_ENTITIES,
    ModeledEntityKind, charge_work, impact_fail,
};

/// Root facts copied from one strictly decoded `StateRoot` record.
#[derive(Clone, Copy, Debug)]
pub struct CompleteRootFacts<'a> {
    /// Raw-ID-sorted keys of the record's `entity_bindings`.
    pub bound_entities: &'a [EntityId],
    /// The record's canonical `entry_points` set.
    pub entry_points: &'a [EntityId],
    /// The record's canonical `dependency_roots` set.
    pub dependency_roots: &'a [StateRoot],
}

/// Exact impact index over one complete root, produced only after every
/// closure rule passed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteRootIndex {
    index: ImpactIndex,
    workspace: EntityId,
    bound_entities: usize,
    packages: usize,
    namespaces: usize,
}

impl CompleteRootIndex {
    /// Returns the exact eighteen-kind impact index.
    #[must_use]
    pub const fn index(&self) -> &ImpactIndex {
        &self.index
    }

    /// Consumes the judgment and returns the index.
    #[must_use]
    pub fn into_index(self) -> ImpactIndex {
        self.index
    }

    /// Returns the single workspace entity of the root.
    #[must_use]
    pub const fn workspace(&self) -> EntityId {
        self.workspace
    }

    /// Returns how many bound entities the index covers (every one).
    #[must_use]
    pub const fn bound_entities(&self) -> usize {
        self.bound_entities
    }

    /// Returns the package count of the root.
    #[must_use]
    pub const fn packages(&self) -> usize {
        self.packages
    }

    /// Returns the namespace count of the root.
    #[must_use]
    pub const fn namespaces(&self) -> usize {
        self.namespaces
    }
}

/// Judges one complete-root request and builds its exact impact index.
///
/// Rules C1 through C11 of the contract run in order; the first failure is
/// returned and no partial index exists.
///
/// # Errors
///
/// Returns the first canonicality, resolution, kind, closure, or resource
/// failure.
#[allow(clippy::too_many_lines)]
pub fn judge_complete_root(
    entities: &[ImpactEntity<'_>],
    facts: CompleteRootFacts<'_>,
) -> Result<CompleteRootIndex, ImpactError> {
    if entities.len() > MAX_IMPACT_ENTITIES || facts.bound_entities.len() > MAX_IMPACT_ENTITIES {
        return impact_fail(ImpactErrorCode::ResourceLimit);
    }
    let mut work = 0_u64;
    require_sorted_unique(facts.bound_entities, &mut work)?;
    require_sorted_unique(facts.entry_points, &mut work)?;
    require_sorted_unique_roots(facts.dependency_roots, &mut work)?;

    // C1: the request equals the bound inventory.
    let mut previous = None;
    let mut kinds = BTreeMap::new();
    for entity in entities {
        charge_work(&mut work, 1)?;
        let id = entity.entity_id();
        if previous.is_some_and(|prior| prior >= id) {
            return impact_fail(ImpactErrorCode::SetNotCanonical);
        }
        previous = Some(id);
        kinds.insert(id, entity.kind());
    }
    if entities.len() != facts.bound_entities.len()
        || entities
            .iter()
            .zip(facts.bound_entities)
            .any(|(entity, bound)| entity.entity_id() != *bound)
    {
        return impact_fail(ImpactErrorCode::RootInventoryMismatch);
    }

    // Set fields of the six bodies must already be canonical.
    let mut workspaces = Vec::new();
    let mut packages = BTreeMap::new();
    let mut namespaces = BTreeMap::new();
    let mut entry_points = Vec::new();
    let mut bindings = BTreeMap::new();
    for entity in entities {
        match *entity {
            ImpactEntity::Workspace(value) => {
                for set in [
                    &value.packages,
                    &value.capability_requirements,
                    &value.contracts,
                    &value.tests,
                ] {
                    require_sorted_unique(set, &mut work)?;
                }
                workspaces.push(value);
            }
            ImpactEntity::Package(value) => {
                require_sorted_unique(&value.dependencies, &mut work)?;
                require_sorted_unique(&value.exports, &mut work)?;
                packages.insert(value.entity_id, value);
            }
            ImpactEntity::Namespace(value) => {
                require_sorted_unique(&value.members, &mut work)?;
                namespaces.insert(value.entity_id, value);
            }
            ImpactEntity::EntryPoint(value) => entry_points.push(value.entity_id),
            ImpactEntity::PolicyBinding(value) => {
                require_sorted_unique(&value.requirements, &mut work)?;
            }
            ImpactEntity::DependencyBinding(value) => {
                bindings.insert(value.entity_id, value);
            }
            _ => {}
        }
    }

    // C2: every edge resolves with the required kind.
    let index = ImpactIndex::build(entities)?;

    // C3: exactly one workspace.
    let workspace = match workspaces.as_slice() {
        [] => return impact_fail(ImpactErrorCode::RootWorkspaceMissing),
        [workspace] => *workspace,
        _ => return impact_fail(ImpactErrorCode::RootWorkspaceAmbiguous),
    };

    // C4: package membership.
    charge_work(&mut work, packages.len() as u64)?;
    if workspace.packages.len() != packages.len()
        || workspace
            .packages
            .iter()
            .zip(packages.keys())
            .any(|(declared, present)| declared != present)
        || packages
            .values()
            .any(|package| package.workspace != workspace.entity_id)
    {
        return impact_fail(ImpactErrorCode::RootPackageMembership);
    }

    // C5: namespace roots.
    let mut roots = BTreeSet::new();
    let declared_roots = core::iter::once(workspace.root_namespace)
        .chain(packages.values().map(|package| package.root_namespace));
    for root in declared_roots {
        charge_work(&mut work, 1)?;
        let Some(namespace) = namespaces.get(&root) else {
            return impact_fail(ImpactErrorCode::RootNamespaceRoot);
        };
        if namespace.parent.is_some() || !roots.insert(root) {
            return impact_fail(ImpactErrorCode::RootNamespaceRoot);
        }
    }
    for namespace in namespaces.values() {
        charge_work(&mut work, 1)?;
        if namespace.parent.is_none() && !roots.contains(&namespace.entity_id) {
            return impact_fail(ImpactErrorCode::RootNamespaceRoot);
        }
    }

    // C6: namespace tree consistency and rooted parent chains.
    for namespace in namespaces.values() {
        charge_work(&mut work, 1)?;
        if let Some(parent) = namespace.parent {
            let Some(parent_namespace) = namespaces.get(&parent) else {
                return impact_fail(ImpactErrorCode::RootNamespaceTree);
            };
            if parent_namespace
                .members
                .binary_search(&namespace.entity_id)
                .is_err()
            {
                return impact_fail(ImpactErrorCode::RootNamespaceTree);
            }
        }
        for member in &namespace.members {
            charge_work(&mut work, 1)?;
            if namespaces
                .get(member)
                .is_some_and(|child| child.parent != Some(namespace.entity_id))
            {
                return impact_fail(ImpactErrorCode::RootNamespaceTree);
            }
        }
        let mut cursor = namespace.parent;
        let mut steps = 0_usize;
        while let Some(parent) = cursor {
            charge_work(&mut work, 1)?;
            steps += 1;
            if steps > namespaces.len() {
                return impact_fail(ImpactErrorCode::RootNamespaceTree);
            }
            cursor = namespaces
                .get(&parent)
                .ok_or_else(|| ImpactError::new(ImpactErrorCode::RootNamespaceTree))?
                .parent;
        }
    }

    // C7: single membership and member kinds.
    let mut owner_of = BTreeMap::<EntityId, EntityId>::new();
    for namespace in namespaces.values() {
        for member in &namespace.members {
            charge_work(&mut work, 1)?;
            let kind = kinds
                .get(member)
                .copied()
                .ok_or_else(|| ImpactError::new(ImpactErrorCode::UnresolvedEntity))?;
            if matches!(
                kind,
                ModeledEntityKind::Workspace
                    | ModeledEntityKind::Package
                    | ModeledEntityKind::Parameter
                    | ModeledEntityKind::Block
                    | ModeledEntityKind::Operation
                    | ModeledEntityKind::DependencyBinding
            ) || owner_of.insert(*member, namespace.entity_id).is_some()
            {
                return impact_fail(ImpactErrorCode::RootMemberOwnership);
            }
        }
    }

    // C8: exports are scoped to the package namespace tree.
    let mut trees = BTreeMap::<EntityId, BTreeSet<EntityId>>::new();
    for package in packages.values() {
        let tree = namespace_tree(package.root_namespace, &namespaces, &mut work)?;
        for export in &package.exports {
            charge_work(&mut work, 1)?;
            if !owner_of
                .get(export)
                .is_some_and(|owner| tree.contains(owner))
            {
                return impact_fail(ImpactErrorCode::RootExportUnscoped);
            }
        }
        trees.insert(package.entity_id, tree);
    }

    // C9: entry points.
    charge_work(&mut work, entry_points.len() as u64)?;
    if entry_points.as_slice() != facts.entry_points {
        return impact_fail(ImpactErrorCode::RootEntryPointsMismatch);
    }

    // C10: dependency roots.
    let mut declared_roots: Vec<StateRoot> = bindings
        .values()
        .map(|binding| binding.dependency_root)
        .collect();
    charge_work(&mut work, declared_roots.len() as u64)?;
    declared_roots.sort_unstable();
    declared_roots.dedup();
    if declared_roots.as_slice() != facts.dependency_roots {
        return impact_fail(ImpactErrorCode::RootDependencyRootsMismatch);
    }

    // C11: every dependency binding is owned by exactly one package and is
    // bound into that package's namespace tree.
    let mut binding_owner = BTreeMap::<EntityId, EntityId>::new();
    for package in packages.values() {
        for dependency in &package.dependencies {
            charge_work(&mut work, 1)?;
            if binding_owner
                .insert(*dependency, package.entity_id)
                .is_some()
            {
                return impact_fail(ImpactErrorCode::RootDependencyBindingUnowned);
            }
        }
    }
    for binding in bindings.values() {
        charge_work(&mut work, 1)?;
        let owned = binding_owner.get(&binding.entity_id).is_some_and(|owner| {
            trees
                .get(owner)
                .is_some_and(|tree| tree.contains(&binding.local_namespace))
        });
        if !owned {
            return impact_fail(ImpactErrorCode::RootDependencyBindingUnowned);
        }
    }

    Ok(CompleteRootIndex {
        index,
        workspace: workspace.entity_id,
        bound_entities: entities.len(),
        packages: packages.len(),
        namespaces: namespaces.len(),
    })
}

fn namespace_tree(
    root: EntityId,
    namespaces: &BTreeMap<EntityId, &sley_ssmc::NamespaceDefinition>,
    work: &mut u64,
) -> Result<BTreeSet<EntityId>, ImpactError> {
    let mut tree = BTreeSet::new();
    let mut queue = VecDeque::new();
    tree.insert(root);
    queue.push_back(root);
    while let Some(current) = queue.pop_front() {
        charge_work(work, 1)?;
        let Some(namespace) = namespaces.get(&current) else {
            return impact_fail(ImpactErrorCode::RootNamespaceTree);
        };
        for member in &namespace.members {
            charge_work(work, 1)?;
            if namespaces.contains_key(member) && tree.insert(*member) {
                queue.push_back(*member);
            }
        }
    }
    Ok(tree)
}

fn require_sorted_unique(values: &[EntityId], work: &mut u64) -> Result<(), ImpactError> {
    charge_work(work, values.len() as u64)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return impact_fail(ImpactErrorCode::SetNotCanonical);
    }
    Ok(())
}

fn require_sorted_unique_roots(values: &[StateRoot], work: &mut u64) -> Result<(), ImpactError> {
    charge_work(work, values.len() as u64)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return impact_fail(ImpactErrorCode::SetNotCanonical);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ImpactEdge, ImpactKind, IndexSnapshotBuildError, IndexSnapshotErrorCode, SnapshotContext,
        build_index_snapshot,
    };
    use sley_id::SchemaEpochId;
    use sley_ssmc::{
        AdapterImport, Block, CapabilityRequirement, ConstData, ConstValue, ConstantDefinition,
        ContractDefinition, ContractKind, DependencyBindingDefinition, EffectDefinition,
        EffectEnvironment, EffectKind, EntryExposure, EntryPointDefinition, ExpectedOutcome,
        FunctionGraph, GlobalValueDefinition, NamespaceDefinition, PackageDefinition, Parameter,
        ParameterRole, PolicyBindingDefinition, Reachability, ResourceLimits, ReturnTerminator,
        Terminator, TestCaseDefinition, TypeDefForm, TypeDefinition, TypeExpr, ValueRef,
        Visibility, WorkspaceDefinition,
    };

    fn id(byte: u8) -> EntityId {
        EntityId::from_bytes([byte; 32])
    }

    fn root(byte: u8) -> StateRoot {
        StateRoot::from_bytes([byte; 32])
    }

    /// Owned eighteen-kind complete root: one workspace (1) with root
    /// namespace (2), one package (3) with root namespace (4) and child
    /// namespace (5), and one entity of every other kind.
    struct Fixture {
        workspace: WorkspaceDefinition,
        workspace_namespace: NamespaceDefinition,
        package: PackageDefinition,
        package_namespace: NamespaceDefinition,
        child_namespace: NamespaceDefinition,
        type_definition: TypeDefinition,
        function: FunctionGraph,
        parameter: Parameter,
        block: Block,
        entry_point: EntryPointDefinition,
        requirement: CapabilityRequirement,
        effect: EffectDefinition,
        contract: ContractDefinition,
        test: TestCaseDefinition,
        adapter: AdapterImport,
        policy_binding: PolicyBindingDefinition,
        dependency_binding: DependencyBindingDefinition,
        constant: ConstantDefinition,
        global: GlobalValueDefinition,
        extra_workspace: Option<WorkspaceDefinition>,
        extra_namespaces: Vec<NamespaceDefinition>,
        bound_entities: Vec<EntityId>,
        entry_points: Vec<EntityId>,
        dependency_roots: Vec<StateRoot>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                workspace: WorkspaceDefinition {
                    entity_id: id(1),
                    packages: vec![id(3)],
                    root_namespace: id(2),
                    capability_requirements: vec![id(11)],
                    contracts: vec![id(13)],
                    tests: vec![id(14)],
                },
                workspace_namespace: NamespaceDefinition {
                    entity_id: id(2),
                    parent: None,
                    members: Vec::new(),
                },
                package: PackageDefinition {
                    entity_id: id(3),
                    workspace: id(1),
                    root_namespace: id(4),
                    dependencies: vec![id(17)],
                    exports: vec![id(6), id(7)],
                },
                package_namespace: NamespaceDefinition {
                    entity_id: id(4),
                    parent: None,
                    members: vec![
                        id(5),
                        id(6),
                        id(10),
                        id(11),
                        id(12),
                        id(13),
                        id(14),
                        id(15),
                        id(16),
                    ],
                },
                child_namespace: NamespaceDefinition {
                    entity_id: id(5),
                    parent: Some(id(4)),
                    members: vec![id(7), id(18), id(19)],
                },
                type_definition: TypeDefinition {
                    entity_id: id(6),
                    type_parameters: Vec::new(),
                    form: TypeDefForm::Record(Vec::new()),
                    invariants: vec![id(13)],
                    visibility: Visibility::Private,
                },
                function: FunctionGraph {
                    entity_id: id(7),
                    type_parameters: Vec::new(),
                    parameters: vec![id(8)],
                    result_type: TypeExpr::Bool,
                    effects: Vec::new(),
                    entry_block: id(9),
                    blocks: vec![id(9)],
                    contracts: Vec::new(),
                    visibility: Visibility::Private,
                },
                parameter: Parameter {
                    entity_id: id(8),
                    owner: id(7),
                    role: ParameterRole::Function,
                    ordinal: 0,
                    value_type: TypeExpr::Bool,
                },
                block: Block {
                    entity_id: id(9),
                    function: id(7),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return(ReturnTerminator {
                        value: ValueRef::Parameter(id(8)),
                    }),
                    reachability: Reachability::Required,
                },
                entry_point: EntryPointDefinition {
                    entity_id: id(10),
                    function: id(7),
                    exposure: EntryExposure::Protocol,
                },
                requirement: CapabilityRequirement {
                    entity_id: id(11),
                    effect: id(12),
                    allowed_scopes: Vec::new(),
                    constraint_contracts: vec![id(13)],
                },
                effect: EffectDefinition {
                    entity_id: id(12),
                    effect_kind: EffectKind::StdoutWrite,
                    scope_type: TypeExpr::Unit,
                    request_type: TypeExpr::Unit,
                    response_type: TypeExpr::Unit,
                    failure_type: TypeExpr::Unit,
                    visibility: Visibility::Private,
                },
                contract: ContractDefinition {
                    entity_id: id(13),
                    target: id(6),
                    contract_kind: ContractKind::Invariant,
                    predicate: id(7),
                    bindings: Vec::new(),
                    resource_limits: None,
                },
                test: TestCaseDefinition {
                    entity_id: id(14),
                    target: id(7),
                    inputs: Vec::new(),
                    effect_environment: EffectEnvironment::Replay(Vec::new()),
                    expected: ExpectedOutcome::FailureCode(1),
                    observations: Vec::new(),
                    resource_limits: ResourceLimits {
                        fuel: 1,
                        memory_bytes: 1,
                        output_bytes: 1,
                        effect_count: 1,
                        call_depth: 1,
                        wall_timeout_millis: 1,
                    },
                },
                adapter: AdapterImport {
                    entity_id: id(15),
                    adapter_id: [0; 32],
                    abi_version: 1,
                    request_type: TypeExpr::Unit,
                    response_type: TypeExpr::Unit,
                    failure_type: TypeExpr::Unit,
                    effects: vec![id(12)],
                },
                policy_binding: PolicyBindingDefinition {
                    entity_id: id(16),
                    subject: id(7),
                    requirements: vec![id(11)],
                },
                dependency_binding: DependencyBindingDefinition {
                    entity_id: id(17),
                    dependency_root: root(9),
                    external_package: id(99),
                    local_namespace: id(4),
                },
                constant: ConstantDefinition {
                    entity_id: id(18),
                    value: ConstValue {
                        value_type: TypeExpr::Bool,
                        data: ConstData::Bool(true),
                    },
                },
                global: GlobalValueDefinition {
                    entity_id: id(19),
                    value_type: TypeExpr::Bool,
                    initializer: id(18),
                    visibility: Visibility::Private,
                },
                extra_workspace: None,
                extra_namespaces: Vec::new(),
                bound_entities: (1..=19).map(id).collect(),
                entry_points: vec![id(10)],
                dependency_roots: vec![root(9)],
            }
        }

        fn entities(&self) -> Vec<ImpactEntity<'_>> {
            let mut entities = vec![
                ImpactEntity::Workspace(&self.workspace),
                ImpactEntity::Namespace(&self.workspace_namespace),
                ImpactEntity::Package(&self.package),
                ImpactEntity::Namespace(&self.package_namespace),
                ImpactEntity::Namespace(&self.child_namespace),
                ImpactEntity::TypeDef(&self.type_definition),
                ImpactEntity::Function(&self.function),
                ImpactEntity::Parameter(&self.parameter),
                ImpactEntity::Block(&self.block),
                ImpactEntity::EntryPoint(&self.entry_point),
                ImpactEntity::CapabilityRequirement(&self.requirement),
                ImpactEntity::EffectDef(&self.effect),
                ImpactEntity::Contract(&self.contract),
                ImpactEntity::TestCase(&self.test),
                ImpactEntity::AdapterImport(&self.adapter),
                ImpactEntity::PolicyBinding(&self.policy_binding),
                ImpactEntity::DependencyBinding(&self.dependency_binding),
                ImpactEntity::Constant(&self.constant),
                ImpactEntity::GlobalValue(&self.global),
            ];
            if let Some(workspace) = &self.extra_workspace {
                entities.push(ImpactEntity::Workspace(workspace));
            }
            for namespace in &self.extra_namespaces {
                entities.push(ImpactEntity::Namespace(namespace));
            }
            entities.sort_by_key(|entity| entity.entity_id());
            entities
        }

        fn facts(&self) -> CompleteRootFacts<'_> {
            CompleteRootFacts {
                bound_entities: &self.bound_entities,
                entry_points: &self.entry_points,
                dependency_roots: &self.dependency_roots,
            }
        }

        fn judge(&self) -> Result<CompleteRootIndex, ImpactError> {
            judge_complete_root(&self.entities(), self.facts())
        }

        fn expect(&self, code: ImpactErrorCode) {
            assert_eq!(self.judge().unwrap_err().code(), code);
        }

        fn add_entity(&mut self, entity: EntityId) {
            self.bound_entities.push(entity);
            self.bound_entities.sort_unstable();
        }
    }

    fn edge(dependent: u8, dependency: u8, kind: ImpactKind) -> ImpactEdge {
        ImpactEdge {
            dependent: id(dependent),
            dependency: id(dependency),
            kind,
        }
    }

    #[test]
    fn complete_root_judgment_passes_and_covers_every_bound_entity() {
        let fixture = Fixture::new();
        let judged = fixture.judge().unwrap();
        assert_eq!(judged.workspace(), id(1));
        assert_eq!(judged.bound_entities(), 19);
        assert_eq!(judged.packages(), 1);
        assert_eq!(judged.namespaces(), 3);
        let direct = judged.index().direct_edges();
        for expected in [
            edge(1, 2, ImpactKind::Ownership),
            edge(1, 3, ImpactKind::Ownership),
            edge(1, 11, ImpactKind::Capability),
            edge(1, 13, ImpactKind::Contract),
            edge(1, 14, ImpactKind::TestTarget),
            edge(3, 1, ImpactKind::Ownership),
            edge(3, 4, ImpactKind::Ownership),
            edge(3, 6, ImpactKind::Ownership),
            edge(3, 7, ImpactKind::Ownership),
            edge(3, 17, ImpactKind::Ownership),
            edge(4, 5, ImpactKind::Ownership),
            edge(4, 16, ImpactKind::Ownership),
            edge(5, 4, ImpactKind::Ownership),
            edge(5, 7, ImpactKind::Ownership),
            edge(10, 7, ImpactKind::Ownership),
            edge(16, 7, ImpactKind::Ownership),
            edge(16, 11, ImpactKind::Capability),
            edge(17, 4, ImpactKind::Ownership),
        ] {
            assert!(direct.contains(&expected), "missing {expected:?}");
        }
        // The external package and the dependency root never become edges.
        assert!(direct.iter().all(|edge| edge.dependency != id(99)));
        assert_eq!(
            direct
                .iter()
                .filter(|edge| edge.dependent == id(17))
                .count(),
            1
        );
        // Reverse impact of the function reaches its exposure, binding,
        // contract, test, export, and namespace.
        let reverse = judged.index().transitive_impact(&[id(7)]).unwrap();
        for expected in [3, 5, 7, 10, 13, 14, 16] {
            assert!(reverse.contains(&id(expected)), "missing {expected}");
        }
    }

    #[test]
    fn repeated_judgments_are_identical() {
        let fixture = Fixture::new();
        let first = fixture.judge().unwrap();
        for _ in 0..128 {
            assert_eq!(fixture.judge().unwrap(), first);
        }
    }

    #[test]
    fn six_kind_tags_and_codes_are_exact() {
        assert_eq!(ModeledEntityKind::from_ssmc_tag(1).unwrap(), ModeledEntityKind::Workspace);
        assert_eq!(ModeledEntityKind::from_ssmc_tag(18).unwrap(), ModeledEntityKind::DependencyBinding);
        assert_eq!(
            ModeledEntityKind::from_ssmc_tag(19).unwrap_err().code(),
            ImpactErrorCode::EntityUnsupported
        );
        for (offset, code) in ImpactErrorCode::ALL.iter().enumerate() {
            assert_eq!(code.numeric(), 25_008 + u32::try_from(offset).unwrap());
            assert!(code.as_str().starts_with("IMPACT_"));
        }
        assert_eq!(ImpactErrorCode::RootDependencyBindingUnowned.numeric(), 25_024);
        for kind in [
            ModeledEntityKind::Workspace,
            ModeledEntityKind::Package,
            ModeledEntityKind::Namespace,
            ModeledEntityKind::EntryPoint,
            ModeledEntityKind::PolicyBinding,
            ModeledEntityKind::DependencyBinding,
        ] {
            assert!(!kind.restricted_kind());
        }
        assert!(ModeledEntityKind::TypeDef.restricted_kind());
        assert!(ModeledEntityKind::AdapterImport.restricted_kind());
    }

    #[test]
    fn c1_inventory_mismatch_is_the_first_closure_failure() {
        let mut fixture = Fixture::new();
        fixture.bound_entities.pop();
        fixture.expect(ImpactErrorCode::RootInventoryMismatch);
        let mut fixture = Fixture::new();
        fixture.add_entity(id(40));
        fixture.expect(ImpactErrorCode::RootInventoryMismatch);
    }

    #[test]
    fn non_canonical_facts_and_sets_fail_closed() {
        let mut fixture = Fixture::new();
        fixture.bound_entities.reverse();
        fixture.expect(ImpactErrorCode::SetNotCanonical);
        let mut fixture = Fixture::new();
        fixture.entry_points = vec![id(10), id(10)];
        fixture.expect(ImpactErrorCode::SetNotCanonical);
        let mut fixture = Fixture::new();
        fixture.package.exports = vec![id(7), id(6)];
        fixture.expect(ImpactErrorCode::SetNotCanonical);
        let mut fixture = Fixture::new();
        fixture.child_namespace.members = vec![id(7), id(7), id(18), id(19)];
        fixture.expect(ImpactErrorCode::SetNotCanonical);
    }

    #[test]
    fn c2_unresolved_and_wrong_kind_edges_fail_before_closure() {
        let mut fixture = Fixture::new();
        fixture.package.dependencies = vec![id(6)];
        fixture.expect(ImpactErrorCode::WrongEntityKind);
        let mut fixture = Fixture::new();
        fixture.entry_point.function = id(6);
        fixture.expect(ImpactErrorCode::WrongEntityKind);
        let mut fixture = Fixture::new();
        fixture.policy_binding.subject = id(42);
        fixture.expect(ImpactErrorCode::UnresolvedEntity);
    }

    #[test]
    fn c3_workspace_count() {
        let namespace = NamespaceDefinition {
            entity_id: id(2),
            parent: None,
            members: Vec::new(),
        };
        let bound = [id(2)];
        let error = judge_complete_root(
            &[ImpactEntity::Namespace(&namespace)],
            CompleteRootFacts {
                bound_entities: &bound,
                entry_points: &[],
                dependency_roots: &[],
            },
        )
        .unwrap_err();
        assert_eq!(error.code(), ImpactErrorCode::RootWorkspaceMissing);

        let mut fixture = Fixture::new();
        fixture.extra_workspace = Some(WorkspaceDefinition {
            entity_id: id(20),
            packages: vec![id(3)],
            root_namespace: id(2),
            capability_requirements: Vec::new(),
            contracts: Vec::new(),
            tests: Vec::new(),
        });
        fixture.add_entity(id(20));
        fixture.expect(ImpactErrorCode::RootWorkspaceAmbiguous);
    }

    #[test]
    fn c4_package_membership() {
        let mut fixture = Fixture::new();
        fixture.workspace.packages = Vec::new();
        fixture.expect(ImpactErrorCode::RootPackageMembership);
    }

    #[test]
    fn c5_namespace_roots() {
        let mut fixture = Fixture::new();
        fixture.workspace_namespace.parent = Some(id(4));
        fixture.package_namespace.members.insert(0, id(2));
        fixture.expect(ImpactErrorCode::RootNamespaceRoot);

        let mut fixture = Fixture::new();
        fixture.extra_namespaces.push(NamespaceDefinition {
            entity_id: id(21),
            parent: None,
            members: Vec::new(),
        });
        fixture.add_entity(id(21));
        fixture.expect(ImpactErrorCode::RootNamespaceRoot);

        let mut fixture = Fixture::new();
        fixture.package.root_namespace = id(2);
        fixture.expect(ImpactErrorCode::RootNamespaceRoot);
    }

    #[test]
    fn c6_namespace_tree() {
        let mut fixture = Fixture::new();
        fixture.child_namespace.parent = Some(id(2));
        fixture.expect(ImpactErrorCode::RootNamespaceTree);

        let mut fixture = Fixture::new();
        fixture.extra_namespaces.push(NamespaceDefinition {
            entity_id: id(22),
            parent: Some(id(23)),
            members: vec![id(23)],
        });
        fixture.extra_namespaces.push(NamespaceDefinition {
            entity_id: id(23),
            parent: Some(id(22)),
            members: vec![id(22)],
        });
        fixture.add_entity(id(22));
        fixture.add_entity(id(23));
        fixture.expect(ImpactErrorCode::RootNamespaceTree);
    }

    #[test]
    fn c7_member_ownership() {
        let mut fixture = Fixture::new();
        fixture.child_namespace.members.insert(0, id(6));
        fixture.expect(ImpactErrorCode::RootMemberOwnership);

        let mut fixture = Fixture::new();
        fixture.package_namespace.members.insert(0, id(3));
        fixture.expect(ImpactErrorCode::RootMemberOwnership);

        let mut fixture = Fixture::new();
        fixture.child_namespace.members = vec![id(7), id(8), id(18), id(19)];
        fixture.expect(ImpactErrorCode::RootMemberOwnership);
    }

    #[test]
    fn c8_export_scope() {
        let mut fixture = Fixture::new();
        fixture.package.exports = vec![id(2)];
        fixture.expect(ImpactErrorCode::RootExportUnscoped);
    }

    #[test]
    fn c9_entry_points() {
        let mut fixture = Fixture::new();
        fixture.entry_points = Vec::new();
        fixture.expect(ImpactErrorCode::RootEntryPointsMismatch);
    }

    #[test]
    fn c10_dependency_roots() {
        let mut fixture = Fixture::new();
        fixture.dependency_roots = Vec::new();
        fixture.expect(ImpactErrorCode::RootDependencyRootsMismatch);
        let mut fixture = Fixture::new();
        fixture.dependency_roots = vec![root(8)];
        fixture.expect(ImpactErrorCode::RootDependencyRootsMismatch);
    }

    #[test]
    fn c11_dependency_binding_ownership() {
        let mut fixture = Fixture::new();
        fixture.package.dependencies = Vec::new();
        fixture.expect(ImpactErrorCode::RootDependencyBindingUnowned);

        let mut fixture = Fixture::new();
        fixture.dependency_binding.local_namespace = id(2);
        fixture.expect(ImpactErrorCode::RootDependencyBindingUnowned);
    }

    #[test]
    fn zero_package_root_is_valid() {
        let workspace = WorkspaceDefinition {
            entity_id: id(1),
            packages: Vec::new(),
            root_namespace: id(2),
            capability_requirements: Vec::new(),
            contracts: Vec::new(),
            tests: Vec::new(),
        };
        let namespace = NamespaceDefinition {
            entity_id: id(2),
            parent: None,
            members: Vec::new(),
        };
        let bound = [id(1), id(2)];
        let judged = judge_complete_root(
            &[
                ImpactEntity::Workspace(&workspace),
                ImpactEntity::Namespace(&namespace),
            ],
            CompleteRootFacts {
                bound_entities: &bound,
                entry_points: &[],
                dependency_roots: &[],
            },
        )
        .unwrap();
        assert_eq!(judged.packages(), 0);
        assert_eq!(
            judged.index().direct_edges(),
            &[edge(1, 2, ImpactKind::Ownership)]
        );
    }

    fn hex(id: EntityId) -> String {
        id.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn hex_root(root: StateRoot) -> String {
        root.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn json_ids(ids: &[EntityId]) -> String {
        let items: Vec<String> = ids.iter().map(|id| format!("\"{}\"", hex(*id))).collect();
        format!("[{}]", items.join(","))
    }

    /// Serializes one borrowed body in the fixture's compact JSON schema
    /// (only the constructs the frozen fixture exercises).
    #[allow(clippy::too_many_lines)]
    fn entity_json(entity: ImpactEntity<'_>) -> String {
        let id = hex(entity.entity_id());
        let kind = entity.kind().tag();
        let body = match entity {
            ImpactEntity::Workspace(value) => format!(
                "\"packages\":{},\"root_namespace\":\"{}\",\"capability_requirements\":{},\"contracts\":{},\"tests\":{}",
                json_ids(&value.packages),
                hex(value.root_namespace),
                json_ids(&value.capability_requirements),
                json_ids(&value.contracts),
                json_ids(&value.tests)
            ),
            ImpactEntity::Package(value) => format!(
                "\"workspace\":\"{}\",\"root_namespace\":\"{}\",\"dependencies\":{},\"exports\":{}",
                hex(value.workspace),
                hex(value.root_namespace),
                json_ids(&value.dependencies),
                json_ids(&value.exports)
            ),
            ImpactEntity::Namespace(value) => format!(
                "\"parent\":{},\"members\":{}",
                value
                    .parent
                    .map_or_else(|| "null".to_owned(), |parent| format!("\"{}\"", hex(parent))),
                json_ids(&value.members)
            ),
            ImpactEntity::TypeDef(value) => {
                format!("\"invariants\":{}", json_ids(&value.invariants))
            }
            ImpactEntity::Function(value) => format!(
                "\"parameters\":{},\"effects\":{},\"entry_block\":\"{}\",\"blocks\":{},\"contracts\":{}",
                json_ids(&value.parameters),
                json_ids(&value.effects),
                hex(value.entry_block),
                json_ids(&value.blocks),
                json_ids(&value.contracts)
            ),
            ImpactEntity::Parameter(value) => format!(
                "\"owner\":\"{}\",\"role\":\"{}\"",
                hex(value.owner),
                match value.role {
                    ParameterRole::Function => "function",
                    ParameterRole::Block => "block",
                }
            ),
            ImpactEntity::Block(value) => {
                let Terminator::Return(ReturnTerminator {
                    value: ValueRef::Parameter(parameter),
                }) = &value.terminator
                else {
                    panic!("fixture blocks return a parameter");
                };
                format!(
                    "\"function\":\"{}\",\"parameters\":{},\"operations\":{},\"return_parameter\":\"{}\"",
                    hex(value.function),
                    json_ids(&value.parameters),
                    json_ids(&value.operations),
                    hex(*parameter)
                )
            }
            ImpactEntity::Operation(_) => panic!("fixture carries no operations"),
            ImpactEntity::Constant(_) | ImpactEntity::EffectDef(_) => String::new(),
            ImpactEntity::GlobalValue(value) => {
                format!("\"initializer\":\"{}\"", hex(value.initializer))
            }
            ImpactEntity::CapabilityRequirement(value) => format!(
                "\"effect\":\"{}\",\"constraint_contracts\":{}",
                hex(value.effect),
                json_ids(&value.constraint_contracts)
            ),
            ImpactEntity::Contract(value) => format!(
                "\"target\":\"{}\",\"predicate\":\"{}\"",
                hex(value.target),
                hex(value.predicate)
            ),
            ImpactEntity::TestCase(value) => format!("\"target\":\"{}\"", hex(value.target)),
            ImpactEntity::AdapterImport(value) => {
                format!("\"effects\":{}", json_ids(&value.effects))
            }
            ImpactEntity::EntryPoint(value) => format!(
                "\"function\":\"{}\",\"exposure\":{}",
                hex(value.function),
                value.exposure.tag()
            ),
            ImpactEntity::PolicyBinding(value) => format!(
                "\"subject\":\"{}\",\"requirements\":{}",
                hex(value.subject),
                json_ids(&value.requirements)
            ),
            ImpactEntity::DependencyBinding(value) => format!(
                "\"dependency_root\":\"{}\",\"external_package\":\"{}\",\"local_namespace\":\"{}\"",
                hex_root(value.dependency_root),
                hex(value.external_package),
                hex(value.local_namespace)
            ),
        };
        if body.is_empty() {
            format!("{{\"id\":\"{id}\",\"kind\":{kind}}}")
        } else {
            format!("{{\"id\":\"{id}\",\"kind\":{kind},{body}}}")
        }
    }

    fn request_json(fixture: &Fixture) -> String {
        let entities: Vec<String> = fixture.entities().into_iter().map(entity_json).collect();
        let roots: Vec<String> = fixture
            .dependency_roots
            .iter()
            .map(|root| format!("\"{}\"", hex_root(*root)))
            .collect();
        format!(
            "{{\"entities\":[{}],\"facts\":{{\"bound_entities\":{},\"entry_points\":{},\"dependency_roots\":[{}]}}}}",
            entities.join(","),
            json_ids(&fixture.bound_entities),
            json_ids(&fixture.entry_points),
            roots.join(",")
        )
    }

    /// Emits the frozen eighteen-kind fixture and its rejection matrix for
    /// `scripts/generate_complete_entity_impact_fixtures.py`.
    #[test]
    #[ignore = "fixture refresh emitter; run through the generator script"]
    fn emit_complete_entity_impact_vector_for_fixture_refresh() {
        let fixture = Fixture::new();
        let judged = fixture.judge().unwrap();
        let edges: Vec<String> = judged
            .index()
            .direct_edges()
            .iter()
            .map(|edge| {
                format!(
                    "[\"{}\",\"{}\",{}]",
                    hex(edge.dependent),
                    hex(edge.dependency),
                    edge.kind.tag()
                )
            })
            .collect();
        let reverse = judged.index().transitive_impact(&[id(7)]).unwrap();
        println!(
            "COMPLETE_ROOT_VECTOR|{}|{{\"direct_edges\":[{}],\"workspace\":\"{}\",\"packages\":{},\"namespaces\":{},\"bound_entities\":{},\"transitive_impact_of_function\":{}}}",
            request_json(&fixture),
            edges.join(","),
            hex(judged.workspace()),
            judged.packages(),
            judged.namespaces(),
            judged.bound_entities(),
            json_ids(&reverse)
        );
        let rejections: Vec<(&str, Box<dyn Fn(&mut Fixture)>)> = vec![
            ("inventory-missing", Box::new(|f| { f.bound_entities.pop(); })),
            ("inventory-surplus", Box::new(|f| f.add_entity(id(40)))),
            ("facts-not-canonical", Box::new(|f| f.bound_entities.reverse())),
            ("exports-not-canonical", Box::new(|f| f.package.exports = vec![id(7), id(6)])),
            ("dependency-wrong-kind", Box::new(|f| f.package.dependencies = vec![id(6)])),
            ("subject-unresolved", Box::new(|f| f.policy_binding.subject = id(42))),
            (
                "workspace-ambiguous",
                Box::new(|f| {
                    f.extra_workspace = Some(WorkspaceDefinition {
                        entity_id: id(20),
                        packages: vec![id(3)],
                        root_namespace: id(2),
                        capability_requirements: Vec::new(),
                        contracts: Vec::new(),
                        tests: Vec::new(),
                    });
                    f.add_entity(id(20));
                }),
            ),
            ("package-membership", Box::new(|f| f.workspace.packages = Vec::new())),
            (
                "namespace-root-parent",
                Box::new(|f| {
                    f.workspace_namespace.parent = Some(id(4));
                    f.package_namespace.members.insert(0, id(2));
                }),
            ),
            ("namespace-root-shared", Box::new(|f| f.package.root_namespace = id(2))),
            (
                "namespace-tree-parent",
                Box::new(|f| f.child_namespace.parent = Some(id(2))),
            ),
            (
                "namespace-tree-cycle",
                Box::new(|f| {
                    f.extra_namespaces.push(NamespaceDefinition {
                        entity_id: id(22),
                        parent: Some(id(23)),
                        members: vec![id(23)],
                    });
                    f.extra_namespaces.push(NamespaceDefinition {
                        entity_id: id(23),
                        parent: Some(id(22)),
                        members: vec![id(22)],
                    });
                    f.add_entity(id(22));
                    f.add_entity(id(23));
                }),
            ),
            (
                "member-double-owner",
                Box::new(|f| f.child_namespace.members.insert(0, id(6))),
            ),
            (
                "member-forbidden-kind",
                Box::new(|f| f.child_namespace.members = vec![id(7), id(8), id(18), id(19)]),
            ),
            ("export-unscoped", Box::new(|f| f.package.exports = vec![id(2)])),
            ("entry-points-mismatch", Box::new(|f| f.entry_points = Vec::new())),
            ("dependency-roots-mismatch", Box::new(|f| f.dependency_roots = vec![root(8)])),
            ("binding-unowned", Box::new(|f| f.package.dependencies = Vec::new())),
            (
                "binding-outside-tree",
                Box::new(|f| f.dependency_binding.local_namespace = id(2)),
            ),
        ];
        for (name, mutate) in rejections {
            let mut mutated = Fixture::new();
            mutate(&mut mutated);
            let code = mutated.judge().unwrap_err().code();
            println!(
                "COMPLETE_ROOT_REJECT|{name}|{}|{}|{}",
                code.as_str(),
                code.numeric(),
                request_json(&mutated)
            );
        }
    }

    #[test]
    fn restricted_snapshot_fails_closed_on_each_complete_model_kind() {
        let fixture = Fixture::new();
        let context = SnapshotContext {
            schema_epoch: SchemaEpochId::from_bytes([7; 32]),
            claimed_root_context: None,
        };
        for entity in [
            ImpactEntity::Workspace(&fixture.workspace),
            ImpactEntity::Package(&fixture.package),
            ImpactEntity::Namespace(&fixture.workspace_namespace),
            ImpactEntity::EntryPoint(&fixture.entry_point),
            ImpactEntity::PolicyBinding(&fixture.policy_binding),
            ImpactEntity::DependencyBinding(&fixture.dependency_binding),
        ] {
            let error = build_index_snapshot(context, &[entity]).unwrap_err();
            assert_eq!(
                error,
                IndexSnapshotBuildError::Snapshot(crate::IndexSnapshotError::new(
                    IndexSnapshotErrorCode::CompletenessUnsupported
                ))
            );
        }
        // The restricted arm itself is unchanged for kinds 4 through 15.
        assert!(build_index_snapshot(context, &[ImpactEntity::TypeDef(&fixture.type_definition)]).is_err());
        let restricted = [
            ImpactEntity::TypeDef(&fixture.type_definition),
            ImpactEntity::Function(&fixture.function),
            ImpactEntity::Parameter(&fixture.parameter),
            ImpactEntity::Block(&fixture.block),
            ImpactEntity::Contract(&fixture.contract),
        ];
        assert!(build_index_snapshot(context, &restricted).is_ok());
    }
}

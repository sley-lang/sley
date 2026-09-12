#![allow(unsafe_code)]
#![no_main]

use core::slice;

use sley_check::{TypeEnvironment, cfg::validate_function_graph};
use sley_check::cfg::CfgValidationError;
use sley_id::EntityId;
use sley_ssmc::{
    Block, BranchTerminator, BuiltinCase, CaseKey, CondBranchTerminator, Immediate, IntegerWidth,
    Opcode, Operation, OperationResultRef, Parameter, ParameterRole, Reachability,
    ReturnTerminator, SwitchArgument, SwitchCase, SwitchEdge, TargetEdge, Terminator, TrapCode,
    TrapTerminator, TypeExpr, ValueRef, VariantSwitchTerminator, Visibility,
};

const MAX_FUZZ_INPUT_BYTES: usize = 4096;
const TEMPLATE_COUNT: u8 = 4;
const MAX_MUTATIONS: usize = 8;
const MUTATION_COUNT: u8 = 33;

#[unsafe(no_mangle)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn LLVMFuzzerTestOneInput(data: *const u8, len: usize) -> i32 {
    if len == 0 {
        return 0;
    }
    let input = unsafe { slice::from_raw_parts(data, len) };
    fuzz_one(input);
    0
}

fn fuzz_one(input: &[u8]) {
    if input.len() > MAX_FUZZ_INPUT_BYTES {
        return;
    }

    let mut cursor = Cursor::new(input);
    let mut graph = graph_template(cursor.byte() % TEMPLATE_COUNT);
    let mutation_count = cursor.bounded(MAX_MUTATIONS);
    let mut applied: Vec<u8> = Vec::new();
    for _ in 0..mutation_count {
        applied.push(apply_mutation(&mut graph, &mut cursor));
    }

    let types = TypeEnvironment::new(Vec::new()).expect("empty type environment is valid");
    let first = graph.validate(&types);
    let second = graph.validate(&types);
    assert_eq!(first, second, "graph/CFG judgment was not deterministic");
    if mutation_count == 0 {
        assert!(first.is_ok(), "a graph/CFG base template drifted invalid");
    } else if let Err(error) = &first {
        // Failure-class narrowing: a mutated graph may still validate (a
        // benign mutation), but when it fails the code must belong to the
        // documented set of the applied mutation class. A single mutation
        // pins its exact class set; several mutations pin their union while
        // the determinism assert above pins which one stably wins. A wrong
        // neighboring code fails here instead of passing silently.
        let class = failure_class(error);
        if applied.len() == 1 {
            let expected = expected_for(applied[0]);
            assert!(
                expected.contains(&class.as_str()),
                "mutation class {} escaped with unexpected failure class {} (expected one of {:?})",
                applied[0],
                class,
                expected
            );
        } else {
            let union = expected_union(&applied);
            assert!(
                union.contains(&class.as_str()),
                "mutation classes {:?} escaped with unexpected failure class {} (expected one of {:?})",
                applied,
                class,
                union
            );
        }
    }
}

#[derive(Clone)]
struct GraphCase {
    function: sley_ssmc::FunctionGraph,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
}

impl GraphCase {
    fn validate(
        &self,
        types: &TypeEnvironment,
    ) -> sley_check::cfg::CfgResult<sley_check::cfg::CfgReport> {
        validate_function_graph(
            types,
            &self.function,
            &self.parameters,
            &self.blocks,
            &self.operations,
        )
    }

    fn known_ids(&self) -> Vec<EntityId> {
        core::iter::once(self.function.entity_id)
            .chain(self.parameters.iter().map(|value| value.entity_id))
            .chain(self.blocks.iter().map(|value| value.entity_id))
            .chain(self.operations.iter().map(|value| value.entity_id))
            .collect()
    }
}

fn graph_template(selector: u8) -> GraphCase {
    match selector {
        0 => return_template(),
        1 => branch_template(),
        2 => conditional_template(),
        3 => operation_template(),
        _ => unreachable!(),
    }
}

fn return_template() -> GraphCase {
    let function = id(1);
    let parameter = id(2);
    let block = id(3);
    GraphCase {
        function: function_body(
            function,
            vec![parameter],
            TypeExpr::Unit,
            block,
            vec![block],
        ),
        parameters: vec![function_parameter(parameter, function, 0, TypeExpr::Unit)],
        blocks: vec![Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::Parameter(parameter),
            }),
            reachability: Reachability::Required,
        }],
        operations: Vec::new(),
    }
}

fn branch_template() -> GraphCase {
    let function = id(10);
    let input = id(11);
    let entry = id(12);
    let target = id(13);
    let block_parameter_id = id(14);
    GraphCase {
        function: function_body(
            function,
            vec![input],
            TypeExpr::Bool,
            entry,
            vec![entry, target],
        ),
        parameters: vec![
            function_parameter(input, function, 0, TypeExpr::Bool),
            block_parameter(block_parameter_id, target, 0, TypeExpr::Bool),
        ],
        blocks: vec![
            Block {
                entity_id: entry,
                function,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Branch(BranchTerminator {
                    edge: TargetEdge {
                        target,
                        arguments: vec![ValueRef::Parameter(input)],
                    },
                }),
                reachability: Reachability::Required,
            },
            Block {
                entity_id: target,
                function,
                parameters: vec![block_parameter_id],
                operations: Vec::new(),
                terminator: Terminator::Return(ReturnTerminator {
                    value: ValueRef::Parameter(block_parameter_id),
                }),
                reachability: Reachability::Required,
            },
        ],
        operations: Vec::new(),
    }
}

fn conditional_template() -> GraphCase {
    let function = id(20);
    let condition = id(21);
    let value = id(22);
    let entry = id(23);
    let target = id(24);
    let block_parameter_id = id(25);
    let edge = TargetEdge {
        target,
        arguments: vec![ValueRef::Parameter(value)],
    };
    GraphCase {
        function: function_body(
            function,
            vec![condition, value],
            TypeExpr::Unit,
            entry,
            vec![entry, target],
        ),
        parameters: vec![
            function_parameter(condition, function, 0, TypeExpr::Bool),
            function_parameter(value, function, 1, TypeExpr::Unit),
            block_parameter(block_parameter_id, target, 0, TypeExpr::Unit),
        ],
        blocks: vec![
            Block {
                entity_id: entry,
                function,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::CondBranch(CondBranchTerminator {
                    condition: ValueRef::Parameter(condition),
                    if_true: edge.clone(),
                    if_false: edge,
                }),
                reachability: Reachability::Required,
            },
            Block {
                entity_id: target,
                function,
                parameters: vec![block_parameter_id],
                operations: Vec::new(),
                terminator: Terminator::Return(ReturnTerminator {
                    value: ValueRef::Parameter(block_parameter_id),
                }),
                reachability: Reachability::Required,
            },
        ],
        operations: Vec::new(),
    }
}

fn operation_template() -> GraphCase {
    let function = id(30);
    let block = id(31);
    let operation = id(32);
    GraphCase {
        function: function_body(function, Vec::new(), TypeExpr::Bool, block, vec![block]),
        parameters: Vec::new(),
        blocks: vec![Block {
            entity_id: block,
            function,
            parameters: Vec::new(),
            operations: vec![operation],
            terminator: Terminator::Return(ReturnTerminator {
                value: ValueRef::OperationResult(OperationResultRef {
                    operation,
                    result_index: 0,
                }),
            }),
            reachability: Reachability::Required,
        }],
        operations: vec![Operation {
            entity_id: operation,
            block,
            ordinal: 0,
            opcode: Opcode::ConstantRef,
            operands: Vec::new(),
            result_types: vec![TypeExpr::Bool],
            immediate: Immediate::Entity(id(900)),
        }],
    }
}

fn function_body(
    entity_id: EntityId,
    parameters: Vec<EntityId>,
    result_type: TypeExpr,
    entry_block: EntityId,
    blocks: Vec<EntityId>,
) -> sley_ssmc::FunctionGraph {
    sley_ssmc::FunctionGraph {
        entity_id,
        type_parameters: Vec::new(),
        parameters,
        result_type,
        effects: Vec::new(),
        entry_block,
        blocks,
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn function_parameter(
    entity_id: EntityId,
    owner: EntityId,
    ordinal: u32,
    value_type: TypeExpr,
) -> Parameter {
    Parameter {
        entity_id,
        owner,
        role: ParameterRole::Function,
        ordinal,
        value_type,
    }
}

fn block_parameter(
    entity_id: EntityId,
    owner: EntityId,
    ordinal: u32,
    value_type: TypeExpr,
) -> Parameter {
    Parameter {
        entity_id,
        owner,
        role: ParameterRole::Block,
        ordinal,
        value_type,
    }
}

fn apply_mutation(graph: &mut GraphCase, cursor: &mut Cursor<'_>) -> u8 {
    let known_ids = graph.known_ids();
    let arm = cursor.byte() % MUTATION_COUNT;
    match arm {
        0 => graph.function.entity_id = selected_id(&known_ids, cursor),
        1 => graph
            .function
            .parameters
            .push(selected_id(&known_ids, cursor)),
        2 => graph.function.parameters.reverse(),
        3 => graph.function.result_type = small_type(cursor),
        4 => graph.function.entry_block = selected_id(&known_ids, cursor),
        5 => graph.function.blocks.push(selected_id(&known_ids, cursor)),
        6 => graph.function.blocks.reverse(),
        7 => graph.function.effects = id_list(&known_ids, cursor),
        8 => graph.function.contracts = id_list(&known_ids, cursor),
        9 => {
            if let Some(parameter) = selected_mut(&mut graph.parameters, cursor) {
                parameter.entity_id = selected_id(&known_ids, cursor);
            }
        }
        10 => {
            if let Some(parameter) = selected_mut(&mut graph.parameters, cursor) {
                parameter.owner = selected_id(&known_ids, cursor);
            }
        }
        11 => {
            if let Some(parameter) = selected_mut(&mut graph.parameters, cursor) {
                parameter.role = match parameter.role {
                    ParameterRole::Function => ParameterRole::Block,
                    ParameterRole::Block => ParameterRole::Function,
                };
            }
        }
        12 => {
            if let Some(parameter) = selected_mut(&mut graph.parameters, cursor) {
                parameter.ordinal = cursor.u32();
            }
        }
        13 => {
            if let Some(parameter) = selected_mut(&mut graph.parameters, cursor) {
                parameter.value_type = small_type(cursor);
            }
        }
        14 => {
            if let Some(block) = selected_mut(&mut graph.blocks, cursor) {
                block.entity_id = selected_id(&known_ids, cursor);
            }
        }
        15 => {
            if let Some(block) = selected_mut(&mut graph.blocks, cursor) {
                block.function = selected_id(&known_ids, cursor);
            }
        }
        16 => {
            if let Some(block) = selected_mut(&mut graph.blocks, cursor) {
                block.parameters.push(selected_id(&known_ids, cursor));
            }
        }
        17 => {
            if let Some(block) = selected_mut(&mut graph.blocks, cursor) {
                block.operations.push(selected_id(&known_ids, cursor));
            }
        }
        18 => {
            if let Some(block) = selected_mut(&mut graph.blocks, cursor) {
                block.reachability = match block.reachability {
                    Reachability::Required => Reachability::ExplicitlyUnreachable,
                    Reachability::ExplicitlyUnreachable => Reachability::Required,
                };
            }
        }
        19 => {
            if let Some(block) = selected_mut(&mut graph.blocks, cursor) {
                block.terminator = Terminator::Return(ReturnTerminator {
                    value: value_ref(&known_ids, cursor),
                });
            }
        }
        20 => {
            if let Some(block) = selected_mut(&mut graph.blocks, cursor) {
                block.terminator = Terminator::Branch(BranchTerminator {
                    edge: target_edge(&known_ids, cursor),
                });
            }
        }
        21 => {
            if let Some(block) = selected_mut(&mut graph.blocks, cursor) {
                block.terminator = Terminator::CondBranch(CondBranchTerminator {
                    condition: value_ref(&known_ids, cursor),
                    if_true: target_edge(&known_ids, cursor),
                    if_false: target_edge(&known_ids, cursor),
                });
            }
        }
        22 => {
            if let Some(block) = selected_mut(&mut graph.blocks, cursor) {
                block.terminator = Terminator::Trap(TrapTerminator {
                    code: trap_code(cursor.byte()),
                    payload: cursor
                        .byte()
                        .is_multiple_of(2)
                        .then(|| value_ref(&known_ids, cursor)),
                });
            }
        }
        23 => {
            if let Some(operation) = selected_mut(&mut graph.operations, cursor) {
                operation.entity_id = selected_id(&known_ids, cursor);
            }
        }
        24 => {
            if let Some(operation) = selected_mut(&mut graph.operations, cursor) {
                operation.block = selected_id(&known_ids, cursor);
            }
        }
        25 => {
            if let Some(operation) = selected_mut(&mut graph.operations, cursor) {
                operation.ordinal = cursor.u32();
            }
        }
        26 => {
            if let Some(operation) = selected_mut(&mut graph.operations, cursor) {
                operation.operands.push(value_ref(&known_ids, cursor));
            }
        }
        27 => {
            if let Some(operation) = selected_mut(&mut graph.operations, cursor) {
                operation.result_types.push(small_type(cursor));
            }
        }
        28 => {
            if let Some(parameter) = graph
                .parameters
                .get(cursor.index(graph.parameters.len()))
                .cloned()
            {
                graph.parameters.push(parameter);
            }
        }
        29 => {
            if let Some(block) = graph.blocks.get(cursor.index(graph.blocks.len())).cloned() {
                graph.blocks.push(block);
            }
        }
        30 => {
            if let Some(operation) = graph
                .operations
                .get(cursor.index(graph.operations.len()))
                .cloned()
            {
                graph.operations.push(operation);
            }
        }
        31 => graph.operations.reverse(),
        32 => {
            if let Some(block) = selected_mut(&mut graph.blocks, cursor) {
                block.terminator = variant_switch(&known_ids, cursor);
            }
        }
        _ => unreachable!(),
    }
    arm
}

/// The exact failure symbol of a validation error, across both phases.
fn failure_class(error: &CfgValidationError) -> String {
    match error {
        CfgValidationError::Cfg(error) => error.code().as_str().to_string(),
        CfgValidationError::Type(error) => error.code().as_str().to_string(),
    }
}

/// Terminator, target, argument, and switch failures: the middle precedence
/// group (entry and targets after inventory, before values and
/// dominance/reachability).
const TARGET: &[&str] = &[
    "CFG_ENTRY_INVALID",
    "CFG_TARGET_INVALID",
    "CFG_TARGET_ARGUMENTS",
    "CFG_BOOL_REQUIRED",
    "CFG_SWITCH_TYPE",
    "CFG_SWITCH_CASES",
    "CFG_SWITCH_PAYLOAD",
    "CFG_TRAP_PAYLOAD",
];

/// Value-use failures are the last precedence group (values and
/// dominance/reachability after inventory and targets). They are spelled
/// per arm below (`CFG_VALUE_UNRESOLVED`, `CFG_RESULT_INDEX`,
/// `CFG_USE_BEFORE_DEFINITION`) so each class keeps its narrowest set
/// instead of the whole group.

/// Exact S20-210 type failures (no `InternalInvariant` exists in this
/// family, so no swallowed-invariant class can hide here).
const TYPE_CODES: &[&str] = &[
    "TYPE_DEPTH_LIMIT",
    "TYPE_WIDTH_INVALID",
    "TYPE_PARAMETER_OUT_OF_SCOPE",
    "TYPE_ARGUMENT_LIMIT",
    "TYPE_ARGUMENT_ARITY",
    "TYPE_DEFINITION_UNKNOWN",
    "TYPE_DEFINITION_DUPLICATE",
    "TYPE_DEFINITION_CYCLE",
    "TYPE_MEMBER_DUPLICATE",
    "TYPE_MEMBER_UNKNOWN",
    "TYPE_SET_ORDER",
    "TYPE_NOT_ORDERABLE",
    "TYPE_NOT_HASHABLE",
    "TYPE_NOT_PERSISTABLE",
    "TYPE_CONST_SHAPE",
    "TYPE_CONST_RANGE",
    "TYPE_FLOAT_NON_CANONICAL",
    "TYPE_CONST_DUPLICATE_KEY",
    "TYPE_RESOURCE_LIMIT",
    "TYPE_IMPLICIT_COERCION",
    "TYPE_BUILTIN_FAILURE_INVALID",
];

/// The narrowest contractually correct failure set for one mutation class.
/// A set that proves too narrow fails loudly here (never silently); a set
/// may only widen with a documented contract reason, never by fiat.
fn expected_for(arm: u8) -> Vec<&'static str> {
    match arm {
        // Entry resolution precedes parameter/operation inventory
        // (cfg.rs:280-286 before the index_unique inventories), so a
        // function-id mutation yields duplication or entry-invalid only.
        0 => vec!["GRAPH_DUPLICATE_ENTITY", "CFG_ENTRY_INVALID"],
        // A pushed unknown id resolves as an unresolvable reference
        // before the inventory comparison runs, a pushed declared id
        // duplicates, and a pushed foreign-owned id trips the owner
        // check (cfg.rs:673-679 all precede the count check at :706, so
        // the inventory class is unreachable through this arm).
        1 => vec![
            "GRAPH_UNRESOLVED_REFERENCE",
            "GRAPH_DUPLICATE_ENTITY",
            "GRAPH_OWNER_MISMATCH",
        ],
        // Reversing a declared parameter list re-maps ordinal positions,
        // so the ordinal class is reachable (T2); the inventory multiset
        // is unchanged, so the inventory class is not. Reversing the
        // block or operation tables disturbs nothing the engine checks:
        // order is validated only for effects/contracts
        // (validate_sorted_unique), ordinals are per-position within each
        // block's own parameter/operation lists, and indexing is
        // order-independent, so those arms cannot fail at all.
        2 => vec!["GRAPH_ORDINAL_MISMATCH"],
        6 | 31 => vec![],
        // A malformed replacement fails at `check_type` (TYPE_*); a
        // well-formed one still mismatches the `Return` terminator value,
        // which the terminator stage reports as CFG_RETURN_TYPE.
        3 => [TYPE_CODES, &["CFG_RETURN_TYPE"][..]].concat(),
        // An unknown entry is invalid; a valid non-entry block orphans
        // the old required entry, which cascades to reachability.
        4 => vec!["CFG_ENTRY_INVALID", "CFG_REACHABILITY"],
        // Pushing an unknown id breaks the declared/passed inventory;
        // pushing a declared one duplicates before the comparison runs.
        5 => vec!["GRAPH_INVENTORY_MISMATCH", "GRAPH_DUPLICATE_ENTITY"],
        7 | 8 => vec!["GRAPH_INVENTORY_MISMATCH"],
        // Renaming a function parameter resolves unknown (the function
        // list still names the old id, cfg.rs:673-676) or duplicates
        // (cfg.rs:270); owner/role/ordinal travel with the entry.
        9 => vec!["GRAPH_DUPLICATE_ENTITY", "GRAPH_UNRESOLVED_REFERENCE"],
        10 => vec!["GRAPH_OWNER_MISMATCH"],
        // A role-flipped parameter keeps its id and ordinal, so the
        // declared set still resolves; the owner/role check
        // (cfg.rs:697-698) trips before the count comparison.
        11 => vec!["GRAPH_OWNER_MISMATCH"],
        12 => vec!["GRAPH_ORDINAL_MISMATCH"],
        // A changed parameter type also breaks target-edge argument
        // agreement downstream of the definition it belongs to. The
        // parameter id itself still resolves (only its type changed).
        13 => [
            TYPE_CODES,
            &[
                "CFG_BOOL_REQUIRED",
                "CFG_RETURN_TYPE",
                "CFG_TARGET_ARGUMENTS",
            ][..],
        ]
        .concat(),
        // Renaming a block collides (cfg.rs:271) or desynchronises the
        // function block list from the block table (cfg.rs:275-279).
        14 => vec!["GRAPH_DUPLICATE_ENTITY", "GRAPH_INVENTORY_MISMATCH"],
        // A block back-pointer is not part of any declared/passed
        // inventory (cfg.rs:706-709 compares parameter sets only), so an
        // unknown function id trips the owner check (cfg.rs:687-689), and
        // a retargeted entry block trips entry validity (:284-286).
        15 => vec!["CFG_ENTRY_INVALID", "GRAPH_OWNER_MISMATCH"],
        // A pushed foreign-owned parameter trips the owner check; an
        // unknown one trips inventory or resolution; pushing a member
        // that is already present duplicates.
        16 | 17 => vec![
            "GRAPH_INVENTORY_MISMATCH",
            "GRAPH_UNRESOLVED_REFERENCE",
            "GRAPH_OWNER_MISMATCH",
            "GRAPH_DUPLICATE_ENTITY",
        ],
        // Flipping a body block trips reachability; flipping the entry
        // block itself trips entry validity (an entry must be reachable).
        // A flipped block stays in the terminator loop, so in combination
        // with a value-using terminator arm (or a rerouted entry) its
        // foreign-owned value refs report CFG_UNREACHABLE_VALUE
        // (cfg.rs:504-506, :530-532) instead of CFG_DOMINANCE.
        18 => vec![
            "CFG_REACHABILITY",
            "CFG_ENTRY_INVALID",
            "CFG_UNREACHABLE_VALUE",
        ],
        // A successor-dropping replacement (Return, Trap-shaped branch)
        // orphans downstream required blocks, which cascade to
        // CFG_REACHABILITY past the terminator check itself. Terminator
        // value refs resolve result_index % 4 against single-result
        // template operations, so an out-of-range reference reports
        // CFG_RESULT_INDEX; a Block-role parameter used from a reachable
        // non-owner block reports CFG_DOMINANCE (cfg.rs:501-507). When the
        // use-block itself is unreachable (arm 18, rerouted entry), the
        // same refs report CFG_UNREACHABLE_VALUE instead.
        19 | 20 | 21 => [
            TARGET,
            &[
                "CFG_RETURN_TYPE",
                "CFG_VALUE_UNRESOLVED",
                "CFG_REACHABILITY",
                "CFG_RESULT_INDEX",
                "CFG_DOMINANCE",
                "CFG_UNREACHABLE_VALUE",
            ][..],
        ]
        .concat(),
        // A trap terminator has no successors, so required blocks
        // reachable only through the replaced block cascade to
        // CFG_REACHABILITY. Its payload value ref resolves like any
        // terminator value (result-index reachable); a Trap placed at the
        // entry orphans the target, and reachability precedes terminators,
        // so CFG_DOMINANCE is not reachable through the payload in this
        // envelope (a Trap in the non-entry block uses self-owned values).
        // In an unreachable use-block the payload reports
        // CFG_UNREACHABLE_VALUE like any other terminator value.
        22 => vec![
            "CFG_TRAP_PAYLOAD",
            "CFG_VALUE_UNRESOLVED",
            "CFG_REACHABILITY",
            "CFG_RESULT_INDEX",
            "CFG_UNREACHABLE_VALUE",
        ],
        // Renaming an operation collides (cfg.rs:272) or desynchronises
        // the declaring block's operation list (cfg.rs:726-729).
        23 => vec!["GRAPH_DUPLICATE_ENTITY", "GRAPH_UNRESOLVED_REFERENCE"],
        // Rehoming an operation leaves the old block's declaration
        // intact, so the declaration-side owner check (cfg.rs:730-732)
        // trips for every non-identity value before any inventory or
        // resolution class is reachable.
        24 => vec!["GRAPH_OWNER_MISMATCH"],
        25 => vec!["GRAPH_ORDINAL_MISMATCH"],
        26 => vec![
            "CFG_VALUE_UNRESOLVED",
            "CFG_RESULT_INDEX",
            "CFG_USE_BEFORE_DEFINITION",
        ],
        // Pushing a result type cannot change result indexing (the count
        // only grows by a well-formed leaf or a free parameter, both
        // resolved by validate_operation_inventory's check_type): the only
        // reachable failure is an out-of-scope parameter.
        27 => vec!["TYPE_PARAMETER_OUT_OF_SCOPE"],
        28 | 29 | 30 => vec!["GRAPH_DUPLICATE_ENTITY"],
        // A successor-dropping switch replacement orphans downstream
        // required blocks exactly like the Return/Trap arms; its edge
        // arguments and selectors resolve values the same way, including
        // the unreachable-use-block path (cfg.rs:504-506, :530-532).
        32 => vec![
            "CFG_SWITCH_TYPE",
            "CFG_SWITCH_CASES",
            "CFG_SWITCH_PAYLOAD",
            "CFG_VALUE_UNRESOLVED",
            "CFG_TARGET_INVALID",
            "CFG_TARGET_ARGUMENTS",
            "CFG_REACHABILITY",
            "CFG_RESULT_INDEX",
            "CFG_DOMINANCE",
            "CFG_UNREACHABLE_VALUE",
        ],
        _ => unreachable!(),
    }
}

/// Deduplicated union of the applied classes' sets, preserving first-seen
/// order so multi-mutation panics name a stable expectation.
fn expected_union(applied: &[u8]) -> Vec<&'static str> {
    let mut union: Vec<&'static str> = Vec::new();
    for arm in applied {
        for code in expected_for(*arm) {
            if !union.contains(&code) {
                union.push(code);
            }
        }
    }
    union
}

fn selected_mut<'a, T>(values: &'a mut [T], cursor: &mut Cursor<'_>) -> Option<&'a mut T> {
    let index = cursor.index(values.len());
    values.get_mut(index)
}

fn selected_id(known_ids: &[EntityId], cursor: &mut Cursor<'_>) -> EntityId {
    if !known_ids.is_empty() && cursor.byte().is_multiple_of(2) {
        known_ids[cursor.index(known_ids.len())]
    } else {
        id(cursor.u32())
    }
}

fn id_list(known_ids: &[EntityId], cursor: &mut Cursor<'_>) -> Vec<EntityId> {
    (0..cursor.bounded(4))
        .map(|_| selected_id(known_ids, cursor))
        .collect()
}

fn target_edge(known_ids: &[EntityId], cursor: &mut Cursor<'_>) -> TargetEdge {
    TargetEdge {
        target: selected_id(known_ids, cursor),
        arguments: (0..cursor.bounded(3))
            .map(|_| value_ref(known_ids, cursor))
            .collect(),
    }
}

fn value_ref(known_ids: &[EntityId], cursor: &mut Cursor<'_>) -> ValueRef {
    if cursor.byte().is_multiple_of(2) {
        ValueRef::Parameter(selected_id(known_ids, cursor))
    } else {
        ValueRef::OperationResult(OperationResultRef {
            operation: selected_id(known_ids, cursor),
            result_index: u32::from(cursor.byte() % 4),
        })
    }
}

fn variant_switch(known_ids: &[EntityId], cursor: &mut Cursor<'_>) -> Terminator {
    let cases = (0..cursor.bounded(4))
        .map(|_| SwitchCase {
            case_key: CaseKey::Builtin(match cursor.byte() % 4 {
                0 => BuiltinCase::None,
                1 => BuiltinCase::Some,
                2 => BuiltinCase::Ok,
                3 => BuiltinCase::Err,
                _ => unreachable!(),
            }),
            edge: SwitchEdge {
                target: selected_id(known_ids, cursor),
                arguments: (0..cursor.bounded(3))
                    .map(|_| {
                        if cursor.byte().is_multiple_of(2) {
                            SwitchArgument::Value(value_ref(known_ids, cursor))
                        } else {
                            SwitchArgument::CasePayload
                        }
                    })
                    .collect(),
            },
        })
        .collect();
    Terminator::VariantSwitch(VariantSwitchTerminator {
        value: value_ref(known_ids, cursor),
        cases,
    })
}

fn small_type(cursor: &mut Cursor<'_>) -> TypeExpr {
    match cursor.byte() % 8 {
        0 => TypeExpr::Unit,
        1 => TypeExpr::Bool,
        2 => TypeExpr::SInt(IntegerWidth::from_bits(8)),
        3 => TypeExpr::UInt(IntegerWidth::from_bits(64)),
        4 => TypeExpr::Option(Box::new(TypeExpr::Bool)),
        5 => TypeExpr::Result {
            ok: Box::new(TypeExpr::Unit),
            error: Box::new(TypeExpr::BuiltinFailure(
                sley_ssmc::BuiltinFailureKind::Arithmetic,
            )),
        },
        6 => TypeExpr::TypeParameter(u32::from(cursor.byte() % 4)),
        7 => TypeExpr::AdapterHandle(id(cursor.u32())),
        _ => unreachable!(),
    }
}

fn trap_code(value: u8) -> TrapCode {
    match value % 4 {
        0 => TrapCode::Unreachable,
        1 => TrapCode::ResourceExhausted,
        2 => TrapCode::AdapterContractViolation,
        3 => TrapCode::InternalInvariant,
        _ => unreachable!(),
    }
}

fn id(value: u32) -> EntityId {
    let mut bytes = [0_u8; 32];
    for (offset, chunk) in bytes.chunks_exact_mut(4).enumerate() {
        let mixed = value.wrapping_add(u32::try_from(offset).unwrap_or(0));
        chunk.copy_from_slice(&mixed.to_be_bytes());
    }
    EntityId::from_bytes(bytes)
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn byte(&mut self) -> u8 {
        let value = self.input[self.offset % self.input.len()];
        self.offset = self.offset.wrapping_add(1);
        value
    }

    fn u32(&mut self) -> u32 {
        u32::from_be_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }

    fn bounded(&mut self, maximum: usize) -> usize {
        usize::from(self.byte()) % (maximum + 1)
    }

    fn index(&mut self, length: usize) -> usize {
        if length == 0 {
            0
        } else {
            usize::from(self.byte()) % length
        }
    }
}

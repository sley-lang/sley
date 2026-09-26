//! Explain: advisory detail for a refusal the kernel already made.
//!
//! The kernel's locator names the function whose graph failed validation;
//! this module inspects that one function of the post-candidate state and
//! names the offending block, operation, or operand. It never decides
//! validity: it runs only after a refusal and only describes it. When
//! nothing it can see matches the kernel's symbol it says nothing.

use std::collections::{BTreeMap, BTreeSet};

use sley_id::EntityId;
use sley_mutate::value::{BlockBody, EntityBodyValue, FunctionBody};
use sley_policy::RefusalLocator;
use sley_ssmc::{ParameterRole, Reachability, SwitchArgument, Terminator, ValueRef};

use crate::names::Names;
use crate::workspace::Program;

/// One observation about a function graph, tagged with the symbol it
/// explains.
struct Finding {
    symbol: &'static str,
    text: String,
}

/// Detail for a refusal whose locator names a function.
#[must_use]
pub fn detail(
    symbol: &str,
    locator: &RefusalLocator,
    program: &Program,
    names: &Names,
) -> Option<String> {
    let function_id = locator.subject?;
    let Some(EntityBodyValue::Function(function)) = program.body(&function_id) else {
        return None;
    };
    let findings = analyze(program, names, &function_id, function);
    // Only a finding for the kernel's own symbol explains the refusal; an
    // unrelated finding would blame the wrong code, so say nothing instead.
    findings
        .iter()
        .find(|finding| finding.symbol == symbol)
        .map(|finding| finding.text.clone())
}

/// Every structural finding across the program's functions except the one
/// `shown` already names, so one refused try discloses every problem the
/// analysis can see (the kernel reports only its first). Advisory only.
#[must_use]
pub fn also(program: &Program, names: &Names, shown: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    for object in program.objects() {
        let id = object.record().entity_id;
        let EntityBodyValue::Function(function) = &object.record().body else {
            continue;
        };
        for finding in analyze(program, names, &id, function) {
            let duplicate = shown.is_some_and(|shown| shown.contains(&finding.text));
            if !duplicate && !out.contains(&finding.text) {
                out.push(finding.text);
            }
        }
    }
    out
}

fn block_body<'a>(program: &'a Program, id: &EntityId) -> Option<&'a BlockBody> {
    match program.body(id) {
        Some(EntityBodyValue::Block(block)) => Some(block),
        _ => None,
    }
}

#[allow(clippy::too_many_lines)]
fn analyze(
    program: &Program,
    names: &Names,
    function_id: &EntityId,
    function: &FunctionBody,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut add = |symbol: &'static str, text: String| findings.push(Finding { symbol, text });
    let listed: BTreeSet<EntityId> = function.blocks.iter().copied().collect();

    // Inventory: owners and lists must agree.
    let unlisted: Vec<String> = program
        .objects()
        .iter()
        .filter_map(|object| {
            let record = object.record();
            match &record.body {
                EntityBodyValue::Block(block)
                    if block.function == *function_id && !listed.contains(&record.entity_id) =>
                {
                    Some(names.name(&record.entity_id))
                }
                _ => None,
            }
        })
        .collect();
    match unlisted.as_slice() {
        [] => {}
        [one] => add(
            "GRAPH_INVENTORY_MISMATCH",
            format!("block {one} names this function but is not listed in its blocks"),
        ),
        many => add(
            "GRAPH_INVENTORY_MISMATCH",
            format!(
                "blocks {} name this function but are not listed in its blocks (delete them or list them)",
                many.join(", ")
            ),
        ),
    }
    for object in program.objects() {
        let record = object.record();
        match &record.body {
            EntityBodyValue::Parameter(parameter)
                if parameter.owner == *function_id
                    && parameter.role == ParameterRole::Function
                    && !function.parameters.contains(&record.entity_id) =>
            {
                add(
                    "GRAPH_INVENTORY_MISMATCH",
                    format!(
                        "parameter {} names this function but is not listed in its parameters",
                        names.name(&record.entity_id)
                    ),
                );
            }
            _ => {}
        }
    }
    for block_id in &function.blocks {
        let Some(block) = block_body(program, block_id) else {
            add(
                "GRAPH_UNRESOLVED_REFERENCE",
                format!("listed block {} is not live", names.name(block_id)),
            );
            continue;
        };
        if block.function != *function_id {
            add(
                "GRAPH_INVENTORY_MISMATCH",
                format!(
                    "block {} is listed here but names fn {}",
                    names.name(block_id),
                    names.name(&block.function)
                ),
            );
        }
        for (position, operation_id) in block.operations.iter().enumerate() {
            match program.body(operation_id) {
                Some(EntityBodyValue::Operation(operation)) => {
                    if operation.block != *block_id {
                        add(
                            "GRAPH_OWNER_MISMATCH",
                            format!(
                                "operation {} is listed in {} but names block {}",
                                names.name(operation_id),
                                names.name(block_id),
                                names.name(&operation.block)
                            ),
                        );
                    }
                    if operation.ordinal as usize != position {
                        add(
                            "GRAPH_ORDINAL_MISMATCH",
                            format!(
                                "operation {} has ordinal {} at position {position}",
                                names.name(operation_id),
                                operation.ordinal
                            ),
                        );
                    }
                }
                _ => add(
                    "GRAPH_UNRESOLVED_REFERENCE",
                    format!("listed operation {} is not live", names.name(operation_id)),
                ),
            }
        }
        for object in program.objects() {
            let record = object.record();
            match &record.body {
                EntityBodyValue::Operation(operation)
                    if operation.block == *block_id
                        && !block.operations.contains(&record.entity_id) =>
                {
                    add(
                        "GRAPH_INVENTORY_MISMATCH",
                        format!(
                            "operation {} names block {} but is not listed in its operations",
                            names.name(&record.entity_id),
                            names.name(block_id)
                        ),
                    );
                }
                EntityBodyValue::Parameter(parameter)
                    if parameter.owner == *block_id
                        && !block.parameters.contains(&record.entity_id) =>
                {
                    add(
                        "GRAPH_INVENTORY_MISMATCH",
                        format!(
                            "parameter {} names block {} but is not listed in its parameters",
                            names.name(&record.entity_id),
                            names.name(block_id)
                        ),
                    );
                }
                _ => {}
            }
        }
    }

    // Control flow over the listed blocks.
    let index: BTreeMap<EntityId, usize> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect();
    let successors: Vec<Vec<(EntityId, usize)>> = function
        .blocks
        .iter()
        .map(|id| block_body(program, id).map_or_else(Vec::new, |block| targets(&block.terminator)))
        .collect();
    let count = function.blocks.len();
    let Some(entry) = index.get(&function.entry_block).copied() else {
        add(
            "CFG_ENTRY_INVALID",
            format!(
                "entry block {} is not listed",
                names.name(&function.entry_block)
            ),
        );
        return findings;
    };
    let mut reachable = vec![false; count];
    let mut stack = vec![entry];
    while let Some(node) = stack.pop() {
        if std::mem::replace(&mut reachable[node], true) {
            continue;
        }
        for (target, _) in &successors[node] {
            if let Some(next) = index.get(target) {
                stack.push(*next);
            }
        }
    }
    for (position, block_id) in function.blocks.iter().enumerate() {
        let Some(block) = block_body(program, block_id) else {
            continue;
        };
        for (target, arguments) in &successors[position] {
            match index.get(target) {
                None => add(
                    "CFG_TARGET_INVALID",
                    format!(
                        "{} branches to {}, which is not a block of this function",
                        names.name(block_id),
                        names.name(target)
                    ),
                ),
                Some(target_index) => {
                    let expected = block_body(program, &function.blocks[*target_index])
                        .map_or(0, |b| b.parameters.len());
                    if *arguments != expected {
                        add(
                            "CFG_TARGET_ARGUMENTS",
                            format!(
                                "{} passes {arguments} argument(s) to {}, which takes {expected}",
                                names.name(block_id),
                                names.name(target)
                            ),
                        );
                    }
                }
            }
        }
        let required = block.reachability == Reachability::Required;
        if required != reachable[position] {
            let state = if reachable[position] {
                "reachable but marked unreachable"
            } else {
                "unreachable from the entry block"
            };
            add(
                "CFG_REACHABILITY",
                format!("block {} is {state}", names.name(block_id)),
            );
        }
    }

    // Dominators (iterative data flow over reachable blocks).
    let predecessors = {
        let mut predecessors = vec![Vec::new(); count];
        for (source, edges) in successors.iter().enumerate() {
            if !reachable[source] {
                continue;
            }
            for (target, _) in edges {
                if let Some(target) = index.get(target) {
                    predecessors[*target].push(source);
                }
            }
        }
        predecessors
    };
    let all: BTreeSet<usize> = (0..count).filter(|i| reachable[*i]).collect();
    let mut dominators: Vec<BTreeSet<usize>> = (0..count)
        .map(|i| {
            if i == entry {
                BTreeSet::from([entry])
            } else {
                all.clone()
            }
        })
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for node in 0..count {
            if node == entry || !reachable[node] {
                continue;
            }
            let mut next: Option<BTreeSet<usize>> = None;
            for predecessor in &predecessors[node] {
                next = Some(match next {
                    None => dominators[*predecessor].clone(),
                    Some(set) => set
                        .intersection(&dominators[*predecessor])
                        .copied()
                        .collect(),
                });
            }
            let mut next = next.unwrap_or_default();
            next.insert(node);
            if next != dominators[node] {
                dominators[node] = next;
                changed = true;
            }
        }
    }

    // Value definitions: (defining block index, position; usize::MAX for
    // block parameters, which are defined at block entry).
    let mut definitions: BTreeMap<EntityId, (Option<usize>, usize)> = BTreeMap::new();
    for parameter in &function.parameters {
        definitions.insert(*parameter, (None, 0));
    }
    for (position, block_id) in function.blocks.iter().enumerate() {
        if let Some(block) = block_body(program, block_id) {
            for parameter in &block.parameters {
                definitions.insert(*parameter, (Some(position), usize::MAX));
            }
            for (ordinal, operation) in block.operations.iter().enumerate() {
                definitions.insert(*operation, (Some(position), ordinal));
            }
        }
    }
    for (position, block_id) in function.blocks.iter().enumerate() {
        let Some(block) = block_body(program, block_id) else {
            continue;
        };
        let check = |value: &ValueRef,
                     at: usize,
                     site: String,
                     add: &mut dyn FnMut(&'static str, String)| {
            let id = match value {
                ValueRef::Parameter(id) => *id,
                ValueRef::OperationResult(result) => result.operation,
            };
            let shown = names.value(value, Some(block_id));
            match definitions.get(&id) {
                None => add(
                    "CFG_VALUE_UNRESOLVED",
                    format!("{site} uses `{shown}`, which is not a value of this function"),
                ),
                Some((None, _)) => {}
                Some((Some(defining), defined_at)) => {
                    let defining_name = names.name(&function.blocks[*defining]);
                    if !reachable[*defining] && reachable[position] {
                        add(
                            "CFG_UNREACHABLE_VALUE",
                            format!("{site} uses `{shown}` from unreachable block {defining_name}"),
                        );
                    } else if *defining == position {
                        if *defined_at != usize::MAX && *defined_at >= at {
                            add(
                                "CFG_USE_BEFORE_DEFINITION",
                                format!(
                                    "{site} uses `{shown}` before it is defined in the same block"
                                ),
                            );
                        }
                    } else if *defined_at == usize::MAX {
                        // A block parameter is visible only in its own block
                        // (S20-220), whatever the dominance.
                        add(
                            "CFG_DOMINANCE",
                            format!(
                                "{site} uses `{shown}`, a parameter of block {defining_name}; block parameters are visible only in their own block, so pass it on as an edge argument"
                            ),
                        );
                    } else if reachable[position] && !dominators[position].contains(defining) {
                        add(
                            "CFG_DOMINANCE",
                            format!(
                                "{site} uses `{shown}` defined in block {defining_name}, which does not dominate block {}",
                                names.name(block_id)
                            ),
                        );
                    }
                }
            }
        };
        for (ordinal, operation_id) in block.operations.iter().enumerate() {
            if let Some(EntityBodyValue::Operation(operation)) = program.body(operation_id) {
                for (slot, value) in operation.operands.iter().enumerate() {
                    check(
                        value,
                        ordinal,
                        format!("operand {slot} of {}", names.name(operation_id)),
                        &mut add,
                    );
                }
            }
        }
        let end = block.operations.len();
        for value in terminator_values(&block.terminator) {
            check(
                &value,
                end,
                format!("the terminator of {}", names.name(block_id)),
                &mut add,
            );
        }
    }
    findings
}

fn targets(terminator: &Terminator) -> Vec<(EntityId, usize)> {
    match terminator {
        Terminator::Return(_) | Terminator::Trap(_) => Vec::new(),
        Terminator::Branch(branch) => vec![(branch.edge.target, branch.edge.arguments.len())],
        Terminator::CondBranch(cond) => vec![
            (cond.if_true.target, cond.if_true.arguments.len()),
            (cond.if_false.target, cond.if_false.arguments.len()),
        ],
        Terminator::VariantSwitch(switch) => switch
            .cases
            .iter()
            .map(|case| (case.edge.target, case.edge.arguments.len()))
            .collect(),
    }
}

fn terminator_values(terminator: &Terminator) -> Vec<ValueRef> {
    match terminator {
        Terminator::Return(ret) => vec![ret.value],
        Terminator::Branch(branch) => branch.edge.arguments.clone(),
        Terminator::CondBranch(cond) => {
            let mut values = vec![cond.condition];
            values.extend(cond.if_true.arguments.iter().copied());
            values.extend(cond.if_false.arguments.iter().copied());
            values
        }
        Terminator::VariantSwitch(switch) => {
            let mut values = vec![switch.value];
            for case in &switch.cases {
                for argument in &case.edge.arguments {
                    if let SwitchArgument::Value(value) = argument {
                        values.push(*value);
                    }
                }
            }
            values
        }
        Terminator::Trap(trap) => trap.payload.into_iter().collect(),
    }
}

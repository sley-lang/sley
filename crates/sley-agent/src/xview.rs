//! AV1-X: AV1 with the regions that match an AF1-X expansion shown in their
//! compact authored shape (`view --x`).
//!
//! A region is sugared only when it matches an expansion pattern exactly:
//!
//! - a checked operation `x__r` that ends its block with
//!   `switch x__r: Ok -> B__x($, t...), Err -> exit`, where `B__x` takes the
//!   unwrapped value and the same-named threaded values and has no other
//!   predecessor, and `exit` is a shared exit block, renders as
//!   `x = op?Case ...` (or `op?` for `__err`/`__none`) with `B__x` merged in;
//! - `cond c -> __fail_Case[(p)], B__if<i>(t...)` renders as `!Case if c[, p]`;
//! - `br __fail_Case` renders as `fail Case`, `br __none` as `fail`, and a
//!   final `B__ok = ok v; return B__ok` as `ok v`;
//! - a single-use generated value (`n__a<k>`, `B__t<k>`, `B__if<i>`) whose
//!   definition is the expected hoisted operand of its use (the operations
//!   immediately before it, depth first, left to right) renders inline: a
//!   constant as its literal, another operation as `(op a, b)`.
//!
//! Shared exits (`__fail_Case`: `variant E.Case; err; return`, `__err`,
//! `__none`) are hidden once every edge into them is sugared. Every sugared
//! line ends with a comment naming the expanded entities it stands for, as
//! `block` or `block.op` inside the function, so `view <fn>.<name>` (or
//! `view <fn>`) shows them in AV1. Anything else renders exactly as AV1. The
//! rendering is output only: no product component reads it back.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use sley_id::EntityId;
use sley_mutate::value::{BlockBody, EntityBodyValue, FunctionBody, OperationBody};
use sley_ssmc::{
    BuiltinCase, CaseKey, Immediate, OperationResultRef, Reachability, SwitchArgument, Terminator,
    ValueRef,
};

use crate::names::{Names, Scope};
use crate::types;
use crate::values;
use crate::view::{self, ViewOptions};
use crate::workspace::Program;

const OP_CONST: u32 = 1;
const OP_VARIANT: u32 = 20;
const OP_NONE: u32 = 129;
const OP_OK: u32 = 130;
const OP_ERR: u32 = 131;

/// Renders one entity in AV1-X; function-scoped entities render their whole
/// function.
#[must_use]
pub fn entity(program: &Program, names: &Names, id: &EntityId, options: ViewOptions) -> String {
    view::entity_with(program, names, id, options, render_function)
}

/// Renders the whole program in AV1-X (the order of `view::package`).
#[must_use]
pub fn package(program: &Program, names: &Names, options: ViewOptions) -> String {
    view::package_with(program, names, options, render_function)
}

/// Renders a function in AV1-X: AV1, byte for byte, unless some region
/// matches a sugar pattern.
pub(crate) fn render_function(
    program: &Program,
    names: &Names,
    id: &EntityId,
    function: &FunctionBody,
    options: ViewOptions,
) -> String {
    let analysis = Analysis::of(program, names, *id, function);
    if analysis.sugar.is_empty() && analysis.inlined.is_empty() {
        return view::render_function(program, names, id, function, options);
    }
    let mut out = view::signature(program, names, id, function, options);
    if !function.blocks.contains(&function.entry_block) {
        let _ = writeln!(
            out,
            "  # entry block {} is not listed",
            names.name(&function.entry_block)
        );
    }
    for block_id in &function.blocks {
        if analysis.hidden.contains(block_id) {
            continue;
        }
        match program.body(block_id) {
            Some(EntityBodyValue::Block(block)) => {
                if analysis.plain(block_id) {
                    view::render_block(&mut out, program, names, id, block_id, block, options);
                } else {
                    analysis.render_chain(&mut out, block_id, block, options);
                }
            }
            _ => {
                let _ = writeln!(out, "  {}:   # missing block", names.leaf(block_id));
            }
        }
    }
    view::orphans(&mut out, program, names, id, function);
    out
}

/// The operation result reference `op#0`.
fn result(operation: &EntityId) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation: *operation,
        result_index: 0,
    })
}

/// A value's defining entity and result index.
fn key(value: &ValueRef) -> (EntityId, u32) {
    match value {
        ValueRef::Parameter(id) => (*id, 0),
        ValueRef::OperationResult(result) => (result.operation, result.result_index),
    }
}

fn all_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// `text` without a generated-name collision suffix (`_2`, `_3`, ...).
fn strip_collision(text: &str) -> &str {
    match text.rsplit_once('_') {
        Some((head, digits)) if !head.is_empty() && all_digits(digits) => head,
        _ => text,
    }
}

/// Whether `leaf` is the generated name `expected`, possibly with a
/// collision suffix.
fn is_generated(leaf: &str, expected: &str) -> bool {
    leaf == expected
        || leaf
            .strip_prefix(expected)
            .and_then(|rest| rest.strip_prefix('_'))
            .is_some_and(all_digits)
}

/// The authored block a piece belongs to: its leaf up to the first `__`.
fn root(leaf: &str) -> &str {
    leaf.split("__").next().unwrap_or(leaf)
}

/// For a generated operand name (`n__a<k>`, `b__t<k>`, `b__if<i>`), the
/// name of what uses it (`n`, `b`).
fn operand_user(leaf: &str) -> Option<&str> {
    let (user, last) = leaf.rsplit_once("__")?;
    let last = strip_collision(last);
    let digits = last
        .strip_prefix("if")
        .or_else(|| last.strip_prefix('a'))
        .or_else(|| last.strip_prefix('t'))?;
    (!user.is_empty() && all_digits(digits)).then_some(user)
}

/// For a checked operation's generated name (`x__r`), the authored `x`.
fn checked_name(leaf: &str) -> Option<&str> {
    let (name, last) = leaf.rsplit_once("__")?;
    (!name.is_empty() && strip_collision(last) == "r").then_some(name)
}

/// The shape of a shared exit block.
enum Exit {
    /// `__fail_Case`: `variant E.Case [p]; err; return`.
    Case { case: String, payload: bool },
    /// `__err(e)`: `err e; return`.
    Err,
    /// `__none`: `none; return`.
    None,
}

/// One sugared terminator region of a block.
enum Sugar {
    /// `x = op?Case ...`, continued in `next`.
    Checked {
        operation: EntityId,
        name: String,
        mark: String,
        next: EntityId,
        exit: EntityId,
    },
    /// `!Case if c [payload]`, continued in `next`.
    Exit {
        case: String,
        condition: ValueRef,
        payload: Option<ValueRef>,
        next: EntityId,
        exit: EntityId,
    },
    /// `fail [Case] [payload]`.
    Fail {
        case: Option<String>,
        payload: Option<ValueRef>,
        exit: EntityId,
    },
    /// `ok v` (the operation `B__ok` and its `return`).
    Ok {
        operation: EntityId,
        value: ValueRef,
    },
}

impl Sugar {
    const fn next(&self) -> Option<EntityId> {
        match self {
            Self::Checked { next, .. } | Self::Exit { next, .. } => Some(*next),
            _ => None,
        }
    }

    const fn exit(&self) -> Option<EntityId> {
        match self {
            Self::Checked { exit, .. } | Self::Exit { exit, .. } | Self::Fail { exit, .. } => {
                Some(*exit)
            }
            Self::Ok { .. } => None,
        }
    }
}

/// The sugar found in one function.
struct Analysis<'a> {
    program: &'a Program,
    names: &'a Names,
    id: EntityId,
    function: &'a FunctionBody,
    blocks: BTreeMap<EntityId, &'a BlockBody>,
    listed: BTreeMap<EntityId, usize>,
    uses: BTreeMap<EntityId, usize>,
    preds: BTreeMap<EntityId, usize>,
    sugar: BTreeMap<EntityId, Sugar>,
    inlined: BTreeSet<EntityId>,
    hidden: BTreeSet<EntityId>,
}

impl<'a> Analysis<'a> {
    fn of(
        program: &'a Program,
        names: &'a Names,
        id: EntityId,
        function: &'a FunctionBody,
    ) -> Self {
        let mut analysis = Self {
            program,
            names,
            id,
            function,
            blocks: BTreeMap::new(),
            listed: BTreeMap::new(),
            uses: BTreeMap::new(),
            preds: BTreeMap::new(),
            sugar: BTreeMap::new(),
            inlined: BTreeSet::new(),
            hidden: BTreeSet::new(),
        };
        for block_id in &function.blocks {
            *analysis.listed.entry(*block_id).or_default() += 1;
            if let Some(EntityBodyValue::Block(block)) = program.body(block_id) {
                analysis.blocks.insert(*block_id, block);
            }
        }
        analysis.count();
        let pieces: Vec<EntityId> = function
            .blocks
            .iter()
            .filter(|block| analysis.clean(block))
            .copied()
            .collect();
        for piece in &pieces {
            if let Some(sugar) = analysis.detect(piece) {
                analysis.sugar.insert(*piece, sugar);
            }
        }
        analysis.break_cycles();
        analysis.keep_names_unique();
        for sugar in analysis.sugar.values() {
            if let Some(next) = sugar.next() {
                analysis.hidden.insert(next);
            }
        }
        let mut sugared_in: BTreeMap<EntityId, usize> = BTreeMap::new();
        for sugar in analysis.sugar.values() {
            if let Some(exit) = sugar.exit() {
                *sugared_in.entry(exit).or_default() += 1;
            }
        }
        for (exit, count) in sugared_in {
            if analysis.preds.get(&exit) == Some(&count) {
                analysis.hidden.insert(exit);
            }
        }
        for piece in &pieces {
            analysis.plan_inline(piece);
        }
        analysis
    }

    /// Counts value uses and block predecessors over the listed blocks and
    /// the blocks that claim the function without being listed.
    fn count(&mut self) {
        let mut bodies: Vec<&BlockBody> = self.blocks.values().copied().collect();
        for object in self.program.objects() {
            let record = object.record();
            if let EntityBodyValue::Block(block) = &record.body
                && block.function == self.id
                && !self.blocks.contains_key(&record.entity_id)
            {
                bodies.push(block);
            }
        }
        for block in bodies {
            for operation in &block.operations {
                if let Some(EntityBodyValue::Operation(body)) = self.program.body(operation) {
                    for value in &body.operands {
                        *self.uses.entry(key(value).0).or_default() += 1;
                    }
                }
            }
            let mut values = Vec::new();
            let mut targets = Vec::new();
            terminator_parts(&block.terminator, &mut values, &mut targets);
            for value in values {
                *self.uses.entry(key(&value).0).or_default() += 1;
            }
            for target in targets {
                *self.preds.entry(target).or_default() += 1;
            }
        }
    }

    /// Uses of any result of `operation`.
    fn uses(&self, operation: &EntityId) -> usize {
        self.uses.get(operation).copied().unwrap_or(0)
    }

    fn operations(&self, block: &BlockBody) -> Vec<(EntityId, &'a OperationBody)> {
        block
            .operations
            .iter()
            .filter_map(|id| match self.program.body(id) {
                Some(EntityBodyValue::Operation(body)) => Some((*id, body)),
                _ => None,
            })
            .collect()
    }

    /// A block that owns exactly what it lists, listed once by this function.
    fn clean(&self, id: &EntityId) -> bool {
        let Some(block) = self.blocks.get(id) else {
            return false;
        };
        self.listed.get(id) == Some(&1)
            && block.function == self.id
            && block.parameters.iter().all(|param| {
                matches!(
                    self.program.body(param),
                    Some(EntityBodyValue::Parameter(_))
                )
            })
            && block
                .operations
                .iter()
                .enumerate()
                .all(|(position, operation)| {
                    matches!(self.program.body(operation),
                        Some(EntityBodyValue::Operation(body))
                            if body.block == *id && body.ordinal as usize == position)
                })
    }

    /// A block a sugared region may absorb or hide: clean, reachable by
    /// declaration, and not the entry.
    fn absorbable(&self, id: &EntityId) -> bool {
        *id != self.function.entry_block
            && self.clean(id)
            && self.blocks[id].reachability != Reachability::ExplicitlyUnreachable
    }

    fn exit(&self, id: &EntityId) -> Option<Exit> {
        if !self.absorbable(id) {
            return None;
        }
        let block = self.blocks[id];
        let leaf = self.names.leaf(id);
        let returns = |operation: &EntityId| matches!(&block.terminator, Terminator::Return(ret) if ret.value == result(operation));
        let params: Vec<ValueRef> = block
            .parameters
            .iter()
            .map(|param| ValueRef::Parameter(*param))
            .collect();
        match self.operations(block).as_slice() {
            [(variant_id, variant), (err_id, err)] if params.len() <= 1 => {
                let Immediate::Variant(immediate) = &variant.immediate else {
                    return None;
                };
                let case = self
                    .names
                    .member_leaf(&immediate.definition, &immediate.member_id);
                (variant.opcode == OP_VARIANT
                    && variant.operands == params
                    && err.opcode == OP_ERR
                    && err.operands == [result(variant_id)]
                    && returns(err_id)
                    && is_generated(&leaf, &format!("__fail_{case}")))
                .then_some(Exit::Case {
                    case,
                    payload: params.len() == 1,
                })
            }
            [(err_id, err)]
                if err.opcode == OP_ERR
                    && params.len() == 1
                    && err.operands == params
                    && returns(err_id)
                    && is_generated(&leaf, "__err") =>
            {
                Some(Exit::Err)
            }
            [(none_id, none)]
                if none.opcode == OP_NONE
                    && params.is_empty()
                    && none.operands.is_empty()
                    && returns(none_id)
                    && is_generated(&leaf, "__none") =>
            {
                Some(Exit::None)
            }
            _ => None,
        }
    }

    /// Whether `target` continues `piece` exactly: its only predecessor is
    /// this edge, it takes `first` (when given) and then, under the same
    /// names, the parameters of `piece` the edge passes.
    fn continues(
        &self,
        piece: &EntityId,
        target: &EntityId,
        first: Option<&str>,
        arguments: &[ValueRef],
        leaf_matches: impl Fn(&str) -> bool,
    ) -> bool {
        if target == piece || !self.absorbable(target) || self.preds.get(target) != Some(&1) {
            return false;
        }
        let from = self.blocks[piece];
        let into = self.blocks[target];
        let skip = usize::from(first.is_some());
        if into.parameters.len() != skip + arguments.len()
            || !leaf_matches(&self.names.leaf(target))
        {
            return false;
        }
        if let Some(first) = first
            && self.names.leaf(&into.parameters[0]) != first
        {
            return false;
        }
        arguments
            .iter()
            .zip(&into.parameters[skip..])
            .all(|(argument, param)| {
                matches!(argument, ValueRef::Parameter(passed)
                    if from.parameters.contains(passed)
                        && self.names.leaf(passed) == self.names.leaf(param))
            })
    }

    fn detect(&self, piece: &EntityId) -> Option<Sugar> {
        let block = self.blocks[piece];
        let leaf = self.names.leaf(piece);
        let root = root(&leaf);
        let operations = self.operations(block);
        match &block.terminator {
            Terminator::VariantSwitch(switch) => self.checked(piece, root, switch, &operations),
            Terminator::CondBranch(cond) => {
                let Exit::Case { case, payload } = self.exit(&cond.if_true.target)? else {
                    return None;
                };
                let passes = cond.if_true.arguments.as_slice();
                let payload = match (payload, passes) {
                    (false, []) => None,
                    (true, [value]) => Some(*value),
                    _ => return None,
                };
                let continued = self.continues(
                    piece,
                    &cond.if_false.target,
                    None,
                    &cond.if_false.arguments,
                    |next| {
                        next.strip_prefix(root)
                            .and_then(|rest| rest.strip_prefix("__if"))
                            .is_some_and(|rest| all_digits(strip_collision(rest)))
                    },
                );
                (!root.is_empty() && continued).then_some(Sugar::Exit {
                    case,
                    condition: cond.condition,
                    payload,
                    next: cond.if_false.target,
                    exit: cond.if_true.target,
                })
            }
            Terminator::Branch(branch) => {
                let (case, payload) = match (
                    self.exit(&branch.edge.target)?,
                    branch.edge.arguments.as_slice(),
                ) {
                    (
                        Exit::Case {
                            case,
                            payload: false,
                        },
                        [],
                    ) => (Some(case), None),
                    (
                        Exit::Case {
                            case,
                            payload: true,
                        },
                        [value],
                    ) => (Some(case), Some(*value)),
                    (Exit::None, []) => (None, None),
                    _ => return None,
                };
                Some(Sugar::Fail {
                    case,
                    payload,
                    exit: branch.edge.target,
                })
            }
            Terminator::Return(ret) => {
                let (operation, body) = operations.last()?;
                let [value] = body.operands.as_slice() else {
                    return None;
                };
                (!root.is_empty()
                    && body.opcode == OP_OK
                    && ret.value == result(operation)
                    && self.uses(operation) == 1
                    && is_generated(&self.names.leaf(operation), &format!("{root}__ok")))
                .then_some(Sugar::Ok {
                    operation: *operation,
                    value: *value,
                })
            }
            Terminator::Trap(_) => None,
        }
    }

    /// `x__r = op ...; switch x__r: Ok -> B__x($, t...), Err -> exit`.
    fn checked(
        &self,
        piece: &EntityId,
        root: &str,
        switch: &sley_ssmc::VariantSwitchTerminator,
        operations: &[(EntityId, &OperationBody)],
    ) -> Option<Sugar> {
        let (operation, _) = operations.last()?;
        let name = checked_name(&self.names.leaf(operation))?.to_owned();
        if root.is_empty() || switch.value != result(operation) || self.uses(operation) != 1 {
            return None;
        }
        let [first, second] = switch.cases.as_slice() else {
            return None;
        };
        let builtin = |case: &sley_ssmc::SwitchCase| match case.case_key {
            CaseKey::Builtin(builtin) => Some(builtin),
            CaseKey::Member(_) => None,
        };
        let (success, failure) = match (builtin(first)?, builtin(second)?) {
            (BuiltinCase::Ok, BuiltinCase::Err) | (BuiltinCase::Some, BuiltinCase::None) => {
                (first, second)
            }
            (BuiltinCase::Err, BuiltinCase::Ok) | (BuiltinCase::None, BuiltinCase::Some) => {
                (second, first)
            }
            _ => return None,
        };
        let (payload, threaded) = success.edge.arguments.split_first()?;
        let threaded: Vec<ValueRef> = threaded
            .iter()
            .map(|argument| match argument {
                SwitchArgument::Value(value) => Some(*value),
                SwitchArgument::CasePayload => None,
            })
            .collect::<Option<_>>()?;
        let continued = self.continues(
            piece,
            &success.edge.target,
            Some(&name),
            &threaded,
            |next| is_generated(next, &format!("{root}__{name}")),
        );
        if *payload != SwitchArgument::CasePayload || !continued {
            return None;
        }
        let passes = failure.edge.arguments.as_slice();
        let is_err = failure.case_key == CaseKey::Builtin(BuiltinCase::Err);
        let mark = match self.exit(&failure.edge.target)? {
            Exit::Case {
                case,
                payload: false,
            } if passes.is_empty() => format!("?{case}"),
            Exit::Case {
                case,
                payload: true,
            } if is_err && passes == [SwitchArgument::CasePayload] => {
                format!("?{case}")
            }
            Exit::Err if is_err && passes == [SwitchArgument::CasePayload] => "?".into(),
            Exit::None if !is_err && passes.is_empty() => "?".into(),
            _ => return None,
        };
        Some(Sugar::Checked {
            operation: *operation,
            name,
            mark,
            next: success.edge.target,
            exit: failure.edge.target,
        })
    }

    /// Drops sugar whose continuation no rendered block reaches (a cycle of
    /// continuations would otherwise hide every block in it).
    fn break_cycles(&mut self) {
        let targets: BTreeSet<EntityId> = self.sugar.values().filter_map(Sugar::next).collect();
        let mut reached = BTreeSet::new();
        for block in &self.function.blocks {
            if targets.contains(block) {
                continue;
            }
            let mut current = *block;
            while let Some(next) = self.sugar.get(&current).and_then(Sugar::next) {
                if !reached.insert(next) {
                    break;
                }
                current = next;
            }
        }
        self.sugar
            .retain(|_, sugar| sugar.next().is_none_or(|next| reached.contains(&next)));
    }

    /// Cuts a merged region where a continuation would bring in a value
    /// leaf the region already has: a merged region renders its own values
    /// by leaf, so every leaf must name one value (threaded parameters
    /// repeat a name for the same value).
    fn keep_names_unique(&mut self) {
        loop {
            let targets: BTreeSet<EntityId> = self.sugar.values().filter_map(Sugar::next).collect();
            let mut cut = None;
            'roots: for root in &self.function.blocks {
                if targets.contains(root) || !self.blocks.contains_key(root) {
                    continue;
                }
                let mut seen = BTreeSet::new();
                let block = self.blocks[root];
                for id in block.parameters.iter().chain(&block.operations) {
                    seen.insert(self.names.leaf(id));
                }
                let mut current = *root;
                while let Some(sugar) = self.sugar.get(&current) {
                    let Some(next) = sugar.next() else {
                        break;
                    };
                    let block = self.blocks[&next];
                    let unwrapped = matches!(sugar, Sugar::Checked { .. });
                    let fresh = block.parameters[..usize::from(unwrapped)]
                        .iter()
                        .chain(&block.operations)
                        .map(|id| self.names.leaf(id));
                    for leaf in fresh {
                        if !seen.insert(leaf) {
                            cut = Some(current);
                            break 'roots;
                        }
                    }
                    current = next;
                }
            }
            match cut {
                Some(piece) => {
                    self.sugar.remove(&piece);
                }
                None => return,
            }
        }
    }

    /// A value that may render inline at its one use: a generated operand
    /// name, one result, used once.
    fn candidate(&self, operation: &EntityId, body: &OperationBody) -> bool {
        operand_user(&self.names.leaf(operation)).is_some()
            && body.result_types.len() == 1
            && self.uses(operation) == 1
    }

    /// Whether the generated operand `operation` is named for `user` (an
    /// operation of `piece`, or its terminator when `None`).
    fn named_for(&self, operation: &EntityId, user: Option<&EntityId>, piece: &EntityId) -> bool {
        let leaf = self.names.leaf(operation);
        let Some(owner) = operand_user(&leaf) else {
            return false;
        };
        let piece_leaf = self.names.leaf(piece);
        let root = root(&piece_leaf);
        let by_block =
            !root.is_empty() && (owner == root || owner.starts_with(&format!("{root}__")));
        let by_operation = user.is_some_and(|user| {
            let user = self.names.leaf(user);
            owner == checked_name(&user).unwrap_or(&user)
        });
        by_block || by_operation
    }

    /// Decides which generated operands of `piece` render inline. Hoisted
    /// operands sit immediately before their use, depth first and left to
    /// right; a pending stack consumed in exactly that order keeps the
    /// rendering faithful to evaluation order, and any other operation
    /// flushes it.
    fn plan_inline(&mut self, piece: &EntityId) {
        let block = self.blocks[piece];
        let mut stack: Vec<EntityId> = Vec::new();
        for (operation, body) in self.operations(block) {
            let wanted: Vec<EntityId> = body
                .operands
                .iter()
                .filter_map(|value| self.pending(value, &stack, Some(&operation), piece, true))
                .collect();
            self.consume(&mut stack, &wanted);
            if self.candidate(&operation, body) {
                stack.push(operation);
            } else {
                stack.clear();
            }
        }
        let mut values = Vec::new();
        let mut nested = Vec::new();
        terminator_operands(&block.terminator, &mut values, &mut nested);
        let wanted: Vec<EntityId> = values
            .iter()
            .zip(nested)
            .filter_map(|(value, nested)| self.pending(value, &stack, None, piece, nested))
            .collect();
        self.consume(&mut stack, &wanted);
    }

    /// The pending generated operand `value` names, if it may render
    /// inline at this use (`nested`: an operation may, not only a literal).
    fn pending(
        &self,
        value: &ValueRef,
        stack: &[EntityId],
        user: Option<&EntityId>,
        piece: &EntityId,
        nested: bool,
    ) -> Option<EntityId> {
        let ValueRef::OperationResult(result) = value else {
            return None;
        };
        let operation = result.operation;
        let literal = matches!(self.program.body(&operation),
            Some(EntityBodyValue::Operation(body)) if body.opcode == OP_CONST);
        (result.result_index == 0
            && stack.contains(&operation)
            && (nested || literal)
            && self.named_for(&operation, user, piece))
        .then_some(operation)
    }

    fn consume(&mut self, stack: &mut Vec<EntityId>, wanted: &[EntityId]) {
        if !wanted.is_empty() && stack.ends_with(wanted) {
            stack.truncate(stack.len() - wanted.len());
            self.inlined.extend(wanted.iter().copied());
        }
    }

    /// Whether a block renders exactly as AV1 (no sugar, nothing inline).
    fn plain(&self, id: &EntityId) -> bool {
        !self.sugar.contains_key(id)
            && self.blocks.get(id).is_none_or(|block| {
                !block
                    .operations
                    .iter()
                    .any(|operation| self.inlined.contains(operation))
            })
    }

    fn render_chain(
        &self,
        out: &mut String,
        start: &EntityId,
        block: &BlockBody,
        options: ViewOptions,
    ) {
        let mut chain = vec![*start];
        while let Some(next) = chain
            .last()
            .and_then(|piece| self.sugar.get(piece))
            .and_then(Sugar::next)
        {
            if chain.contains(&next) {
                break;
            }
            chain.push(next);
        }
        view::block_header(
            out,
            self.program,
            self.names,
            &self.id,
            start,
            block,
            options,
        );
        for piece in &chain {
            let at = At {
                analysis: self,
                chain: &chain,
                piece: *piece,
            };
            self.operation_lines(out, &at, options);
            let mut route = Vec::new();
            if let Some(line) = self.terminator_line(&at, &mut route) {
                let _ = writeln!(out, "    {line}{}", comment(&route));
            }
        }
    }

    /// The operation lines of one piece: inline operands folded into their
    /// use, the checked operation sugared, the `ok` operation left to the
    /// terminator line.
    fn operation_lines(&self, out: &mut String, at: &At<'_, '_>, options: ViewOptions) {
        let sugar = self.sugar.get(&at.piece);
        for (operation, body) in self.operations(self.blocks[&at.piece]) {
            if self.inlined.contains(&operation) {
                continue;
            }
            let mut route = Vec::new();
            let line = match sugar {
                Some(Sugar::Ok { operation: ok, .. }) if *ok == operation => continue,
                Some(checked @ Sugar::Checked { operation: id, .. }) if *id == operation => {
                    self.checked_line(at, body, checked, options, &mut route)
                }
                _ => {
                    let operands: Vec<String> = body
                        .operands
                        .iter()
                        .map(|value| at.value(value, &mut route))
                        .collect();
                    view::operation_line(
                        self.program,
                        self.names,
                        &operation,
                        body,
                        options,
                        &operands,
                    )
                }
            };
            let _ = writeln!(out, "    {line}{}", comment(&route));
        }
    }

    /// A piece's closing line, or `None` when a checked operation carries
    /// the region on into its continuation.
    fn terminator_line(&self, at: &At<'_, '_>, route: &mut Vec<String>) -> Option<String> {
        let piece = &at.piece;
        Some(match self.sugar.get(piece) {
            Some(Sugar::Checked { .. }) => return None,
            Some(Sugar::Exit {
                case,
                condition,
                payload,
                next,
                exit,
            }) => {
                let mut line = format!("!{case} if {}", at.value(condition, route));
                if let Some(payload) = payload {
                    let _ = write!(line, ", {}", at.value(payload, route));
                }
                route.push(self.names.leaf(next));
                route.push(self.names.leaf(exit));
                line
            }
            Some(Sugar::Fail {
                case,
                payload,
                exit,
            }) => {
                let mut line = String::from("fail");
                if let Some(case) = case {
                    let _ = write!(line, " {case}");
                }
                if let Some(payload) = payload {
                    let _ = write!(line, " {}", at.value(payload, route));
                }
                route.push(self.names.leaf(exit));
                line
            }
            Some(Sugar::Ok { operation, value }) => {
                let line = format!("ok {}", at.value(value, route));
                route.push(format!(
                    "{}.{}",
                    self.names.leaf(piece),
                    self.names.leaf(operation)
                ));
                line
            }
            None => {
                let cell = std::cell::RefCell::new(Vec::new());
                let line = view::terminator_with(
                    self.program,
                    self.names,
                    &self.blocks[piece].terminator,
                    &|value| at.value(value, &mut cell.borrow_mut()),
                );
                route.extend(cell.into_inner());
                line
            }
        })
    }

    /// `x = op?Case operands`, routed to `piece.x__r`, the continuation and
    /// the exit.
    fn checked_line(
        &self,
        at: &At<'_, '_>,
        body: &OperationBody,
        sugar: &Sugar,
        options: ViewOptions,
        route: &mut Vec<String>,
    ) -> String {
        let Sugar::Checked {
            operation,
            name,
            mark,
            next,
            exit,
        } = sugar
        else {
            return String::new();
        };
        let mut line = name.clone();
        if options.types
            && let Some(param) = self.blocks[next].parameters.first()
            && let Some(EntityBodyValue::Parameter(param)) = self.program.body(param)
        {
            let _ = write!(line, ": {}", types::render(&param.value_type, self.names));
        }
        let _ = write!(line, " = {}{mark}", view::mnemonic(body.opcode));
        let immediate = view::immediate(self.program, self.names, body);
        if !immediate.is_empty() {
            let _ = write!(line, " {immediate}");
        }
        let operands: Vec<String> = body
            .operands
            .iter()
            .map(|value| at.value(value, route))
            .collect();
        if !operands.is_empty() {
            let _ = write!(line, " {}", operands.join(", "));
        }
        let _ = write!(line, "{}", view::id_suffix(operation, options));
        route.push(format!(
            "{}.{}",
            self.names.leaf(&at.piece),
            self.names.leaf(operation)
        ));
        route.push(self.names.leaf(next));
        route.push(self.names.leaf(exit));
        line
    }
}

/// Value rendering inside a merged region.
struct At<'a, 'b> {
    analysis: &'b Analysis<'a>,
    chain: &'b [EntityId],
    piece: EntityId,
}

impl At<'_, '_> {
    /// A value as seen from the region: inline when planned so (recording
    /// the expanded operation in `route`), by leaf when a block of the
    /// region defines it, else as AV1 renders it.
    fn value(&self, value: &ValueRef, route: &mut Vec<String>) -> String {
        let analysis = self.analysis;
        let names = analysis.names;
        let (id, index) = key(value);
        if index == 0 && analysis.inlined.contains(&id) {
            return self.inline(&id, route);
        }
        match names.scope(&id) {
            Scope::Block(block) if self.chain.contains(&block) => {
                let mut text = names.leaf(&id);
                if index != 0 {
                    let _ = write!(text, "#{index}");
                }
                text
            }
            _ => names.value(value, Some(&self.piece)),
        }
    }

    fn inline(&self, operation: &EntityId, route: &mut Vec<String>) -> String {
        let analysis = self.analysis;
        let names = analysis.names;
        let Some(EntityBodyValue::Operation(body)) = analysis.program.body(operation) else {
            return names.leaf(operation);
        };
        let literal = match (&body.immediate, body.opcode) {
            (Immediate::Entity(constant), OP_CONST) => match analysis.program.body(constant) {
                Some(EntityBodyValue::Constant(constant)) => {
                    Some(values::to_text(&constant.value, names))
                }
                _ => None,
            },
            _ => None,
        };
        let text = literal.unwrap_or_else(|| {
            let mut text = format!("({}", view::mnemonic(body.opcode));
            let immediate = view::immediate(analysis.program, names, body);
            if !immediate.is_empty() {
                let _ = write!(text, " {immediate}");
            }
            let operands: Vec<String> = body
                .operands
                .iter()
                .map(|value| self.value(value, route))
                .collect();
            if !operands.is_empty() {
                let _ = write!(text, " {}", operands.join(", "));
            }
            text.push(')');
            text
        });
        let owner = match names.scope(operation) {
            Scope::Block(block) => block,
            _ => self.piece,
        };
        route.push(format!("{}.{}", names.leaf(&owner), names.leaf(operation)));
        text
    }
}

/// The trailing route comment (empty when nothing was sugared).
fn comment(route: &[String]) -> String {
    if route.is_empty() {
        String::new()
    } else {
        format!("   # {}", route.join(", "))
    }
}

/// A terminator's value uses and its edge targets.
fn terminator_parts(
    terminator: &Terminator,
    values: &mut Vec<ValueRef>,
    targets: &mut Vec<EntityId>,
) {
    let mut nested = Vec::new();
    terminator_operands(terminator, values, &mut nested);
    match terminator {
        Terminator::Branch(branch) => targets.push(branch.edge.target),
        Terminator::CondBranch(cond) => {
            targets.push(cond.if_true.target);
            targets.push(cond.if_false.target);
        }
        Terminator::VariantSwitch(switch) => {
            targets.extend(switch.cases.iter().map(|case| case.edge.target));
        }
        Terminator::Return(_) | Terminator::Trap(_) => {}
    }
}

/// A terminator's value operands in evaluation order, each with whether a
/// nested operation may stand there (target arguments of `cond` and
/// `switch` take names and literals only).
fn terminator_operands(
    terminator: &Terminator,
    values: &mut Vec<ValueRef>,
    nested: &mut Vec<bool>,
) {
    let mut push = |value: ValueRef, may_nest: bool| {
        values.push(value);
        nested.push(may_nest);
    };
    match terminator {
        Terminator::Return(ret) => push(ret.value, true),
        Terminator::Branch(branch) => {
            for value in &branch.edge.arguments {
                push(*value, true);
            }
        }
        Terminator::CondBranch(cond) => {
            push(cond.condition, true);
            for value in cond
                .if_true
                .arguments
                .iter()
                .chain(&cond.if_false.arguments)
            {
                push(*value, false);
            }
        }
        Terminator::VariantSwitch(switch) => {
            push(switch.value, true);
            for case in &switch.cases {
                for argument in &case.edge.arguments {
                    if let SwitchArgument::Value(value) = argument {
                        push(*value, false);
                    }
                }
            }
        }
        Terminator::Trap(trap) => {
            if let Some(payload) = trap.payload {
                push(payload, true);
            }
        }
    }
}

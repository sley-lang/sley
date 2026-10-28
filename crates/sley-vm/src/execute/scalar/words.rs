//! The word tier: plans whose every value is a 64-bit integer, a boolean, a
//! checked result switched on in the block that computes it, or that
//! result's arithmetic failure code run on plain `u64` registers.
//!
//! It reuses the scalar plan's blocks, batches, edges and slots, and makes
//! every charge the scalar loop makes, in the same order: only the register
//! representation differs. A checked result keeps its payload (the value or
//! the failure code) in its slot and its Ok/Err shape in a parallel flag,
//! which the block's own switch reads. A program with any other value keeps
//! the scalar loop.

use super::{
    Atom, CaseKey, ConstValue, ExecutionLimits, ExecutionTermination, Exit, Opcode, Program,
    ResourceKind, Runtime, RuntimeFault, RuntimeResult, S128, Shape, TypeExpr, U128, Value,
    charge_action, charge_value, integer,
};
use super::{Batch, BuiltinCase, Plan, Step};
use crate::extended::{ARITHMETIC_DIVIDE_BY_ZERO, ARITHMETIC_OVERFLOW};
use sley_ssmc::BuiltinFailureKind;

/// A word step's operation, its operands' slots and (for integers) its
/// signedness resolved at compile time.
#[derive(Clone, Copy, Debug)]
enum Op {
    Constant(u32),
    Eq(u32, u32),
    Ne(u32, u32),
    LtS(u32, u32),
    LeS(u32, u32),
    LtU(u32, u32),
    LeU(u32, u32),
    AddS(u32, u32),
    SubS(u32, u32),
    MulS(u32, u32),
    DivS(u32, u32),
    RemS(u32, u32),
    AddU(u32, u32),
    SubU(u32, u32),
    MulU(u32, u32),
    DivU(u32, u32),
    RemU(u32, u32),
    /// Checked signed negation.
    Neg(u32),
    Not(u32),
    And(u32, u32),
    Or(u32, u32),
    /// The length of a vector (its slot holds the vector's index in the
    /// run's vector table).
    VecLen(u32),
    /// An element of a vector by a `u64` index: `Some` (flag clear, the
    /// element's word) or `None` (flag set), switched on in its block.
    VecGet(u32, u32),
    /// A direct call: its call site in the function's `sites`.
    Call(u32),
    /// An edge move inside a fused block (bursts only): no charge.
    Copy(u32),
    /// A fused switch's Ok test (bursts only): when the output slot's
    /// check failed, the fast block ends at that guard's block.
    Guard(u32),
    /// A fused conditional branch (bursts only): when the output slot's
    /// condition is not the one fused, the fast block ends at that guard's
    /// block.
    GuardTrue(u32),
    GuardFalse(u32),
}

#[derive(Clone, Copy, Debug)]
struct WordStep {
    op: Op,
    output: u32,
    units: [u64; 2],
    /// A guard on this step's own output, tested right after it (bursts
    /// only): the guard step it stands for, merged to save a dispatch.
    guard: Guarded,
    /// A slot this step's word is also copied to once its guard passes
    /// (bursts only): the edge move right after it, merged. `NONE` for none.
    also: u32,
}

const NONE: u32 = u32::MAX;

impl Op {
    /// The slots the operation reads.
    fn reads(self) -> [Option<u32>; 2] {
        match self {
            Op::Eq(a, b)
            | Op::Ne(a, b)
            | Op::LtS(a, b)
            | Op::LeS(a, b)
            | Op::LtU(a, b)
            | Op::LeU(a, b)
            | Op::AddS(a, b)
            | Op::SubS(a, b)
            | Op::MulS(a, b)
            | Op::DivS(a, b)
            | Op::RemS(a, b)
            | Op::AddU(a, b)
            | Op::SubU(a, b)
            | Op::MulU(a, b)
            | Op::DivU(a, b)
            | Op::RemU(a, b)
            | Op::And(a, b)
            | Op::Or(a, b)
            | Op::VecGet(a, b) => [Some(a), Some(b)],
            Op::Neg(a) | Op::Not(a) | Op::VecLen(a) | Op::Copy(a) => [Some(a), None],
            Op::Constant(_) | Op::Call(_) | Op::Guard(_) | Op::GuardTrue(_) | Op::GuardFalse(_) => {
                [None, None]
            }
        }
    }
}

/// Whether each slot is ever read: by a step, a call, an exit or an edge.
/// A fused edge's move into a slot nothing reads is dead in a burst (its
/// fuel is still charged with the edge's).
fn read_slots(
    plan: &Plan,
    blocks: &[Vec<WordStep>],
    exits: &[WordExit],
    sites: &[CallSite],
) -> Vec<bool> {
    let mut read = vec![false; plan.slots];
    let mut mark = |slot: u32| {
        if let Some(r) = read.get_mut(slot as usize) {
            *r = true;
        }
    };
    for step in blocks.iter().flatten() {
        for slot in step.op.reads().into_iter().flatten() {
            mark(slot);
        }
    }
    for site in sites {
        for &slot in &site.args {
            mark(slot);
        }
    }
    for exit in exits {
        let edges: Vec<&WordEdge> = match exit {
            WordExit::Branch(e) => vec![e],
            WordExit::Cond(r, a, b) | WordExit::Switch(r, a, b) => {
                mark(*r);
                vec![a, b]
            }
            WordExit::Return(r) => {
                mark(*r);
                vec![]
            }
            WordExit::End => vec![],
        };
        for edge in edges {
            for &(from, _) in &edge.moves {
                if from != PAYLOAD {
                    mark(from);
                }
            }
        }
    }
    for block in &plan.blocks {
        if let Exit::Trap(_, Some((slot, _))) = block.exit {
            mark(u32::try_from(slot).unwrap_or(NONE));
        }
    }
    read
}

/// A guard merged into the step whose output it tests.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Guarded {
    No,
    /// As `Op::Guard`.
    Ok(u32),
    /// As `Op::GuardTrue`.
    True(u32),
    /// As `Op::GuardFalse`.
    False(u32),
}

/// Appends a guard step, or merges it into the step before it when that
/// step computes the guarded slot.
fn push_guard(steps: &mut Vec<WordStep>, op: Op, output: u32) {
    if let Some(last) = steps.last_mut()
        && last.output == output
        && last.guard == Guarded::No
        && last.also == NONE
        && !matches!(
            last.op,
            Op::Copy(_) | Op::Call(_) | Op::Guard(_) | Op::GuardTrue(_) | Op::GuardFalse(_)
        )
    {
        last.guard = match op {
            Op::Guard(g) => Guarded::Ok(g),
            Op::GuardTrue(g) => Guarded::True(g),
            Op::GuardFalse(g) => Guarded::False(g),
            _ => return,
        };
        return;
    }
    steps.push(WordStep {
        op,
        output,
        units: [0, 0],
        guard: Guarded::No,
        also: NONE,
    });
}

/// Appends an edge move as a copy step, or merges it into the step before
/// it when that step computes the moved slot; a move into a slot nothing
/// reads is dropped.
fn push_copy(steps: &mut Vec<WordStep>, from: u32, to: u32, read: &[bool]) {
    if !read.get(to as usize).copied().unwrap_or(true) {
        return;
    }
    if let Some(last) = steps.last_mut()
        && last.output == from
        && last.also == NONE
        && !matches!(
            last.op,
            Op::Copy(_) | Op::Call(_) | Op::Guard(_) | Op::GuardTrue(_) | Op::GuardFalse(_)
        )
    {
        last.also = to;
        return;
    }
    steps.push(WordStep {
        op: Op::Copy(from),
        output: to,
        units: [0, 0],
        guard: Guarded::No,
        also: NONE,
    });
}

/// An edge in word form: its target, its fuel, and its moves as (source
/// slot, or `PAYLOAD` for the case payload; parameter slot).
#[derive(Clone, Debug)]
struct WordEdge {
    target: usize,
    fuel: u64,
    moves: Vec<(u32, u32)>,
    direct: bool,
}

const PAYLOAD: u32 = u32::MAX;

/// A block exit in word form, for bursts: an edge's fuel includes the
/// exit's own action and, for a switch, the cases charged before its own.
#[derive(Clone, Debug)]
enum WordExit {
    Branch(WordEdge),
    Cond(u32, WordEdge, WordEdge),
    Switch(u32, WordEdge, WordEdge),
    /// A return of the slot's word.
    Return(u32),
    /// A trap: the general exit runs.
    End,
}

impl WordExit {
    /// The most fuel the exit charges, its own action included.
    fn fuel(&self) -> u64 {
        match self {
            Self::Branch(edge) => edge.fuel,
            Self::Cond(_, a, b) | Self::Switch(_, a, b) => a.fuel.max(b.fuel),
            Self::Return(_) | Self::End => 1,
        }
    }

    /// Whether the exit leaves the function.
    fn ends(&self) -> bool {
        matches!(self, Self::Return(_) | Self::End)
    }
}

/// A direct call in word form: the callee, the argument slots and the
/// units each charges, where the result goes and the units it charges, and
/// the fast block that runs after it returns.
#[derive(Clone, Debug)]
struct CallSite {
    function: usize,
    args: Vec<u32>,
    arg_units: u64,
    output: u32,
    units: u64,
    tail: u32,
}

/// A whole block as a burst runs it: its own steps without the constants
/// already in their slots, then, through unconditional branches into more
/// whole blocks, theirs (each branch's moves as copies). What it charges
/// is static except the units of the steps it runs and its last exit.
#[derive(Clone, Debug, Default)]
struct FastBlock {
    /// The block and step it starts at.
    start: (usize, usize),
    steps: Vec<WordStep>,
    /// Fuel and instructions of every step it stands for, and the fuel of
    /// the branches it fused.
    fuel: u64,
    count: u64,
    /// Value units of the constants it skips.
    units: u64,
    /// The block whose exit ends it.
    last: usize,
    /// Where each guard leaves it: what it charged up to that block's exit.
    guards: Vec<Leave>,
    /// The call it ends with, instead of its last block's exit.
    call: Option<u32>,
}

/// A fast block left at a fused switch whose check failed: the block and
/// the static fuel, instructions and units charged by then.
#[derive(Clone, Copy, Debug)]
struct Leave {
    block: usize,
    fuel: u64,
    count: u64,
    units: u64,
}

/// One function's steps in word form, block by block, beside its plan.
#[derive(Clone, Debug)]
pub(super) struct WordFunction {
    blocks: Vec<Vec<WordStep>>,
    exits: Vec<WordExit>,
    /// The fast block starting at each block, then one after each call.
    fast: Vec<FastBlock>,
    /// Each direct call.
    sites: Vec<CallSite>,
    /// The entry block.
    entry: usize,
    /// Slots only one constant ever writes, with the constant: written when
    /// a frame starts, so bursts skip those constant steps.
    constant_slots: Vec<(u32, u32)>,
    /// The most one fast block (and a return after it) can charge.
    item: Batch,
    /// Its index in the program.
    index: u32,
    /// The plan's slots, parameters and result units, for calls in bursts.
    slots: usize,
    parameters: Vec<usize>,
    result_units: u64,
}

/// How many fast blocks one burst runs at most.
const BURST: usize = 64;

/// How many whole blocks a fast block fuses at most.
const FUSE: usize = 12;

/// The word form of every function of a program, in plan order.
#[derive(Clone, Debug)]
pub(super) struct Words {
    functions: Vec<WordFunction>,
    /// `BURST` times the most one fast block of any function charges: the
    /// budget a burst, which calls and returns across functions, is
    /// prepaid against.
    burst: Batch,
}

fn word_type(ty: &TypeExpr) -> bool {
    *ty == TypeExpr::Bool || matches!(integer(ty), Some((_, 64)))
}

fn failure_type(ty: &TypeExpr) -> bool {
    *ty == TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)
}

fn result_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Result { ok, error } if matches!(integer(ok), Some((_, 64))) && failure_type(error))
}

/// A vector of words: read-only on the word tier, its slot holding the
/// vector's index in the run's vector table.
fn vector_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Vector(inner) if word_type(inner))
}

/// An optional word (a `vec_get`): like a checked result, read only by the
/// switch of the block defining it.
fn option_type(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Option(inner) if word_type(inner))
}

/// A switched value: a checked result or an optional word.
fn switched_type(ty: &TypeExpr) -> bool {
    result_type(ty) || option_type(ty)
}

/// The words of the vector a vector slot's word names (a slot of a vector
/// type only ever holds an index into the run's table).
fn vector(vectors: &[Vec<u64>], word: u64) -> &[u64] {
    usize::try_from(word)
        .ok()
        .and_then(|i| vectors.get(i))
        .map_or(&[], Vec::as_slice)
}

/// The run's inputs as words: a vector of words enters as its index in
/// the returned vector table. `None` when an input fits no word.
pub(super) fn to_inputs(values: &[Value<'_>]) -> Option<(Vec<u64>, Vec<Vec<u64>>)> {
    let mut vectors = Vec::new();
    let inputs = values
        .iter()
        .map(|v| {
            to_word(*v).or_else(|| {
                vectors.push(to_words(*v)?);
                u64::try_from(vectors.len() - 1).ok()
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some((inputs, vectors))
}

/// A vector input as its words (`None` when it is no vector of words).
fn to_words(value: Value<'_>) -> Option<Vec<u64>> {
    let Atom::Borrowed(ConstValue {
        value_type,
        data: sley_ssmc::ConstData::Sequence(items),
    }) = value.atom
    else {
        return None;
    };
    if value.shape != Shape::Plain || !vector_type(value_type) {
        return None;
    }
    items
        .iter()
        .map(|item| to_word(Value::from_const(item)?))
        .collect()
}

/// A value of a word-tier type as one word, or `None` (the scalar loop runs).
pub(super) fn to_word(value: Value<'_>) -> Option<u64> {
    if value.shape != Shape::Plain {
        return None;
    }
    match value.atom {
        Atom::Bool(v) => Some(u64::from(v)),
        Atom::Signed(v) => i64::try_from(v.0).ok().map(i64::cast_unsigned),
        Atom::Unsigned(v) => u64::try_from(v.0).ok(),
        _ => None,
    }
}

/// The word of a value of type `ty` as the scalar tier's value.
fn to_value(word: u64, ty: &TypeExpr) -> RuntimeResult<Value<'static>> {
    let atom = match ty {
        TypeExpr::Bool => Atom::Bool(word != 0),
        TypeExpr::SInt(_) => Atom::Signed(S128(i128::from(word.cast_signed()))),
        TypeExpr::UInt(_) => Atom::Unsigned(U128(u128::from(word))),
        TypeExpr::BuiltinFailure(_) => {
            Atom::Failure(u16::try_from(word).map_err(|_| RuntimeFault)?)
        }
        _ => return Err(RuntimeFault),
    };
    Ok(Value {
        atom,
        shape: Shape::Plain,
    })
}

pub(super) fn to_const(word: u64, ty: &TypeExpr) -> RuntimeResult<ConstValue> {
    Ok(to_value(word, ty)?.to_const(ty))
}

impl Words {
    /// The word form of a program whose plans are `plans` (entry first) and
    /// bytecode `codes`, or `None` when any function has another value.
    pub(super) fn compile(plans: &[&Plan], codes: &[&crate::BytecodeFunction]) -> Option<Self> {
        let mut functions = plans
            .iter()
            .zip(codes)
            .map(|(plan, code)| compile_function(plan, code))
            .collect::<Option<Vec<WordFunction>>>()?;
        // Frames keep function indices as `u32`.
        for (index, f) in functions.iter_mut().enumerate() {
            f.index = u32::try_from(index).ok()?;
        }
        // A return charges its instruction and the call's units.
        let returned = functions
            .iter()
            .flat_map(|f| &f.sites)
            .map(|site| site.units)
            .max()
            .unwrap_or(0);
        let mut burst = Batch::default();
        for f in &functions {
            burst.fuel = burst.fuel.max(f.item.fuel);
            burst.steps = burst.steps.max(f.item.steps);
            burst.units = burst.units.max(f.item.units);
        }
        burst.steps = burst.steps.checked_add(1)?.checked_mul(BURST)?;
        burst.fuel = burst.fuel.checked_mul(BURST as u64)?;
        burst.units = burst
            .units
            .checked_add(returned)?
            .checked_mul(BURST as u64)?;
        burst.exit = true;
        Some(Self { functions, burst })
    }
}

#[allow(clippy::too_many_lines)]
fn compile_function(plan: &Plan, code: &crate::BytecodeFunction) -> Option<WordFunction> {
    use crate::BytecodeTerminator as T;
    let types = &code.register_types;
    if !word_type(&code.result_type)
        || !types.iter().all(|ty| {
            word_type(ty)
                || failure_type(ty)
                || result_type(ty)
                || vector_type(ty)
                || option_type(ty)
        })
    {
        return None;
    }
    let reg = |r: crate::Register| usize::try_from(r).ok().filter(|&r| r < types.len());
    // A checked result or an optional word is read only by the switch of
    // the block defining it; a failure code is never an operand; a vector
    // is an operand only of `vec_len` and `vec_get` (never of a call: its
    // units are its length's), and is never a trap's payload.
    for block in &code.blocks {
        for r in &block.parameter_registers {
            if switched_type(&types[reg(*r)?]) {
                return None;
            }
        }
        for instruction in &block.instructions {
            let projects = instruction.opcode == Opcode::VectorLen.tag()
                || instruction.opcode == Opcode::VectorGet.tag();
            for (k, r) in instruction.operands.iter().enumerate() {
                let ty = &types[reg(*r)?];
                if switched_type(ty)
                    || failure_type(ty)
                    || (vector_type(ty) && (!projects || k > 0))
                {
                    return None;
                }
            }
        }
        let args: Vec<crate::Register> = match &block.terminator {
            T::Return(r) => vec![*r],
            T::Branch(edge) => edge.arguments.clone(),
            T::CondBranch {
                if_true, if_false, ..
            } => if_true
                .arguments
                .iter()
                .chain(&if_false.arguments)
                .copied()
                .collect(),
            T::VariantSwitch { value, cases } => {
                if !switched_type(&types[reg(*value)?])
                    || !block
                        .instructions
                        .iter()
                        .any(|i| i.results.as_slice() == [*value])
                {
                    return None;
                }
                cases
                    .iter()
                    .flat_map(|case| &case.edge.arguments)
                    .filter_map(|arg| match arg {
                        crate::BytecodeSwitchArgument::Value(r) => Some(*r),
                        crate::BytecodeSwitchArgument::CasePayload => None,
                    })
                    .collect()
            }
            T::Trap { payload, .. } => {
                if let Some(r) = payload {
                    let ty = &types[reg(*r)?];
                    if !word_type(ty) && !failure_type(ty) {
                        return None;
                    }
                }
                payload.iter().copied().collect()
            }
        };
        for r in args {
            if switched_type(&types[reg(r)?]) {
                return None;
            }
        }
    }
    let mut blocks = Vec::with_capacity(plan.blocks.len());
    let mut exits = Vec::with_capacity(plan.blocks.len());
    for (block, source) in plan.blocks.iter().zip(&code.blocks) {
        exits.push(compile_exit(&block.exit)?);
        if let Exit::Switch { cases, .. } = &block.exit
            && !cases
                .iter()
                .all(|(key, _)| matches!(key, CaseKey::Builtin(BuiltinCase::Ok | BuiltinCase::Err)))
            && !cases.iter().all(|(key, _)| {
                matches!(key, CaseKey::Builtin(BuiltinCase::Some | BuiltinCase::None))
            })
        {
            return None;
        }
        let mut steps = Vec::with_capacity(block.steps.len());
        for (step, instruction) in block.steps.iter().zip(&source.instructions) {
            steps.push(compile_step(step, instruction, types)?);
        }
        blocks.push(steps);
    }
    // Each direct call's site, numbered in block order.
    let mut sites = Vec::new();
    for (b, block) in plan.blocks.iter().enumerate() {
        for (k, step) in block.steps.iter().enumerate() {
            let Some(call) = &step.call else { continue };
            let index = u32::try_from(sites.len()).ok()?;
            let Op::Call(site) = &mut blocks[b][k].op else {
                return None;
            };
            *site = index;
            sites.push(CallSite {
                function: call.function,
                args: step.args.iter().map(|&r| slot(r)).collect::<Option<_>>()?,
                arg_units: call.arguments.iter().map(|u| u.fixed[0]).sum(),
                output: slot(step.output)?,
                units: step.units[0],
                tail: 0,
            });
        }
    }
    let constant_slots = constant_slots(plan, &blocks, &exits);
    let read = read_slots(plan, &blocks, &exits, &sites);
    let mut starts: Vec<(usize, usize)> = (0..plan.blocks.len()).map(|b| (b, 0)).collect();
    for (b, steps) in blocks.iter().enumerate() {
        for (k, step) in steps.iter().enumerate() {
            if let Op::Call(site) = step.op {
                sites[site as usize].tail = u32::try_from(starts.len()).ok()?;
                starts.push((b, k + 1));
            }
        }
    }
    let mut fast = Vec::with_capacity(starts.len());
    let mut item = Batch::default();
    for start in starts {
        let (block, budget) = fuse(start, &blocks, &exits, &sites, &constant_slots, &read)?;
        item.fuel = item.fuel.max(budget.fuel);
        item.steps = item.steps.max(budget.steps);
        item.units = item.units.max(budget.units);
        fast.push(block);
    }
    Some(WordFunction {
        blocks,
        exits,
        fast,
        sites,
        entry: usize::try_from(code.entry_block).ok()?,
        constant_slots,
        item,
        index: 0,
        slots: plan.slots,
        parameters: plan.parameters.clone(),
        result_units: plan.result.fixed[0],
    })
}

/// The slots whose only writer is one constant: never a parameter (of the
/// function or a block), an edge's target, or another step's output.
fn constant_slots(plan: &Plan, blocks: &[Vec<WordStep>], exits: &[WordExit]) -> Vec<(u32, u32)> {
    #[derive(Clone, Copy, PartialEq)]
    enum Writer {
        None,
        Constant(u32),
        Other,
    }
    let mut writers = vec![Writer::None; plan.slots];
    let other = |slot: usize, writers: &mut Vec<Writer>| {
        if let Some(w) = writers.get_mut(slot) {
            *w = Writer::Other;
        }
    };
    for &p in &plan.parameters {
        other(p, &mut writers);
    }
    for block in &plan.blocks {
        for &p in &block.parameters {
            other(p, &mut writers);
        }
    }
    for exit in exits {
        let edges: Vec<&WordEdge> = match exit {
            WordExit::Branch(e) => vec![e],
            WordExit::Cond(_, a, b) | WordExit::Switch(_, a, b) => vec![a, b],
            WordExit::Return(_) | WordExit::End => vec![],
        };
        for edge in edges {
            for &(_, to) in &edge.moves {
                other(to as usize, &mut writers);
            }
        }
    }
    for step in blocks.iter().flatten() {
        let out = step.output as usize;
        match (step.op, writers.get(out).copied()) {
            (Op::Constant(i), Some(Writer::None)) => writers[out] = Writer::Constant(i),
            (Op::Constant(i), Some(Writer::Constant(j))) if i == j => {}
            _ => other(out, &mut writers),
        }
    }
    writers
        .iter()
        .enumerate()
        .filter_map(|(slot, w)| match w {
            Writer::Constant(i) => Some((u32::try_from(slot).ok()?, *i)),
            _ => None,
        })
        .collect()
}

/// The steps from `start` (a block and a step) as a burst runs them, up to
/// a call or the exit of the last block fused, and the most they charge.
fn fuse(
    start: (usize, usize),
    blocks: &[Vec<WordStep>],
    exits: &[WordExit],
    sites: &[CallSite],
    constant_slots: &[(u32, u32)],
    read: &[bool],
) -> Option<(FastBlock, Batch)> {
    let mut fast = FastBlock {
        start,
        ..FastBlock::default()
    };
    let mut budget = Batch::default();
    let (mut at, mut pc) = start;
    let mut seen = vec![at];
    loop {
        let mut n: u64 = 0;
        for step in blocks.get(at)?.get(pc..)? {
            if let Op::Call(site) = step.op {
                sites.get(site as usize)?;
                fast.call = Some(site);
                break;
            }
            n += 1;
            budget.units = budget.units.checked_add(step.units[0].max(step.units[1]))?;
            match step.op {
                Op::Constant(i) if constant_slots.contains(&(step.output, i)) => {
                    fast.units += step.units[0];
                }
                _ => fast.steps.push(*step),
            }
        }
        fast.fuel += n;
        fast.count += n;
        budget.fuel = budget.fuel.checked_add(n)?;
        budget.steps = budget.steps.checked_add(usize::try_from(n).ok()?)?;
        fast.last = at;
        if let Some(site) = fast.call {
            // The call's instruction and action, and its arguments.
            let site = &sites[site as usize];
            budget.fuel = budget.fuel.checked_add(2)?;
            budget.steps = budget.steps.checked_add(1)?;
            budget.units = budget.units.checked_add(site.arg_units)?;
            return Some((fast, budget));
        }
        budget.fuel = budget.fuel.checked_add(exits[at].fuel())?;
        let fusable =
            |edge: &WordEdge| edge.direct && !seen.contains(&edge.target) && seen.len() < FUSE;
        // The edge it continues on, and the slot a payload comes from.
        let (edge, payload) = match &exits[at] {
            WordExit::Branch(edge) if fusable(edge) => (edge, PAYLOAD),
            WordExit::Switch(r, ok, _) if fusable(ok) => {
                let g = u32::try_from(fast.guards.len()).ok()?;
                fast.guards.push(Leave {
                    block: at,
                    fuel: fast.fuel,
                    count: fast.count,
                    units: fast.units,
                });
                push_guard(&mut fast.steps, Op::Guard(g), *r);
                (ok, *r)
            }
            // A conditional branch: one side is fused behind a guard (a
            // failed guard leaves at this block's exit, as an unfused
            // block would). The side that continues when the other leaves
            // the function (a loop's exit or a base case), else the true
            // side.
            WordExit::Cond(r, yes, no) if fusable(yes) || fusable(no) => {
                let fused_yes = fusable(yes) && !(exits[yes.target].ends() && fusable(no));
                let g = u32::try_from(fast.guards.len()).ok()?;
                fast.guards.push(Leave {
                    block: at,
                    fuel: fast.fuel,
                    count: fast.count,
                    units: fast.units,
                });
                push_guard(
                    &mut fast.steps,
                    if fused_yes {
                        Op::GuardTrue(g)
                    } else {
                        Op::GuardFalse(g)
                    },
                    *r,
                );
                (if fused_yes { yes } else { no }, PAYLOAD)
            }
            _ => return Some((fast, budget)),
        };
        fast.fuel += edge.fuel;
        for &(from, to) in &edge.moves {
            push_copy(
                &mut fast.steps,
                if from == PAYLOAD { payload } else { from },
                to,
                read,
            );
        }
        seen.push(edge.target);
        at = edge.target;
        pc = 0;
    }
}

fn slot(r: usize) -> Option<u32> {
    u32::try_from(r).ok().filter(|&r| r != PAYLOAD)
}

fn word_edge(edge: &super::Edge, case_fuel: u64) -> Option<WordEdge> {
    Some(WordEdge {
        target: edge.target,
        fuel: 1 + case_fuel + edge.fuel,
        moves: edge
            .moves
            .iter()
            .map(|(arg, to)| {
                Some((
                    match arg.register {
                        Some(r) => slot(r)?,
                        None => PAYLOAD,
                    },
                    slot(*to)?,
                ))
            })
            .collect::<Option<_>>()?,
        direct: edge.moves_direct,
    })
}

fn compile_exit(exit: &Exit) -> Option<WordExit> {
    Some(match exit {
        Exit::Branch(edge) => WordExit::Branch(word_edge(edge, 0)?),
        Exit::Cond(r, yes, no) => WordExit::Cond(slot(*r)?, word_edge(yes, 0)?, word_edge(no, 0)?),
        Exit::Switch {
            register,
            cases,
            shapes,
            ..
        } => {
            // An optional word switches like a checked result: `Some` as
            // Ok, `None` (its flag set) as Err.
            let ok = shapes
                .iter()
                .position(|s| matches!(s, Some(Shape::Ok | Shape::Some)))?;
            let err = shapes
                .iter()
                .position(|s| matches!(s, Some(Shape::Err | Shape::None)))?;
            WordExit::Switch(
                slot(*register)?,
                word_edge(&cases[ok].1, ok as u64 + 1)?,
                word_edge(&cases[err].1, err as u64 + 1)?,
            )
        }
        Exit::Return(r) => WordExit::Return(slot(*r)?),
        Exit::Trap(..) => WordExit::End,
    })
}

fn compile_step(
    step: &Step,
    instruction: &crate::Instruction,
    types: &[TypeExpr],
) -> Option<WordStep> {
    let operand = |i: usize| -> Option<&TypeExpr> {
        types.get(usize::try_from(*instruction.operands.get(i)?).ok()?)
    };
    let op = if let Some(index) = step.constant {
        Op::Constant(u32::try_from(index).ok()?)
    } else if step.call.is_some() {
        // The site index is set when the function's sites are listed.
        Op::Call(0)
    } else if let Some(word) = step.word {
        let (a, b) = (slot(word.a)?, slot(word.b)?);
        match (word.op, word.signed) {
            (Opcode::Equal, _) => Op::Eq(a, b),
            (Opcode::NotEqual, _) => Op::Ne(a, b),
            (Opcode::LessThan, true) => Op::LtS(a, b),
            (Opcode::LessEqual, true) => Op::LeS(a, b),
            (Opcode::GreaterThan, true) => Op::LtS(b, a),
            (Opcode::GreaterEqual, true) => Op::LeS(b, a),
            (Opcode::LessThan, false) => Op::LtU(a, b),
            (Opcode::LessEqual, false) => Op::LeU(a, b),
            (Opcode::GreaterThan, false) => Op::LtU(b, a),
            (Opcode::GreaterEqual, false) => Op::LeU(b, a),
            (Opcode::IntAddChecked, true) => Op::AddS(a, b),
            (Opcode::IntSubChecked, true) => Op::SubS(a, b),
            (Opcode::IntMulChecked, true) => Op::MulS(a, b),
            (Opcode::IntDivChecked, true) => Op::DivS(a, b),
            (Opcode::IntRemChecked, true) => Op::RemS(a, b),
            (Opcode::IntAddChecked, false) => Op::AddU(a, b),
            (Opcode::IntSubChecked, false) => Op::SubU(a, b),
            (Opcode::IntMulChecked, false) => Op::MulU(a, b),
            (Opcode::IntDivChecked, false) => Op::DivU(a, b),
            (Opcode::IntRemChecked, false) => Op::RemU(a, b),
            _ => return None,
        }
    } else {
        match (step.opcode, step.args.as_slice()) {
            (Opcode::IntNegChecked, [a]) if step.width == Some((true, 64)) => Op::Neg(slot(*a)?),
            (Opcode::BoolNot, [a]) => Op::Not(slot(*a)?),
            (Opcode::VectorLen, [a]) if vector_type(operand(0)?) => Op::VecLen(slot(*a)?),
            (Opcode::VectorGet, [a, b])
                if vector_type(operand(0)?) && integer(operand(1)?) == Some((false, 64)) =>
            {
                Op::VecGet(slot(*a)?, slot(*b)?)
            }
            (Opcode::BoolAnd, [a, b]) => Op::And(slot(*a)?, slot(*b)?),
            (Opcode::BoolOr, [a, b]) => Op::Or(slot(*a)?, slot(*b)?),
            (Opcode::Equal, [a, b]) if operand(0)? == &TypeExpr::Bool => {
                Op::Eq(slot(*a)?, slot(*b)?)
            }
            (Opcode::NotEqual, [a, b]) if operand(0)? == &TypeExpr::Bool => {
                Op::Ne(slot(*a)?, slot(*b)?)
            }
            _ => return None,
        }
    };
    if step.dynamic {
        return None;
    }
    Some(WordStep {
        op,
        output: slot(step.output)?,
        units: step.units,
        guard: Guarded::No,
        also: NONE,
    })
}

/// Runs a non-call step: writes its word (and, for a checked operation,
/// whether it failed) and answers the value units it charges. The answers
/// and failure codes of `checked_word`; comparisons as `evaluate`'s.
#[allow(clippy::inline_always)]
#[inline(always)]
fn exec(
    step: &WordStep,
    constants: &[u64],
    vectors: &[Vec<u64>],
    registers: &mut [u64],
    flags: &mut [bool],
) -> u64 {
    let r = |i: u32| registers[i as usize];
    let s = |i: u32| registers[i as usize].cast_signed();
    let signed = |v: Option<i64>| v.map(i64::cast_unsigned).ok_or(ARITHMETIC_OVERFLOW);
    let unsigned = |v: Option<u64>| v.ok_or(ARITHMETIC_OVERFLOW);
    let plain = |v: bool| (Ok(u64::from(v)), false);
    let (value, checked): (Result<u64, u16>, bool) = match step.op {
        Op::Constant(i) => (Ok(constants[i as usize]), false),
        Op::Eq(a, b) => plain(r(a) == r(b)),
        Op::Ne(a, b) => plain(r(a) != r(b)),
        Op::LtS(a, b) => plain(s(a) < s(b)),
        Op::LeS(a, b) => plain(s(a) <= s(b)),
        Op::LtU(a, b) => plain(r(a) < r(b)),
        Op::LeU(a, b) => plain(r(a) <= r(b)),
        Op::AddS(a, b) => (signed(s(a).checked_add(s(b))), true),
        Op::SubS(a, b) => (signed(s(a).checked_sub(s(b))), true),
        Op::MulS(a, b) => (signed(s(a).checked_mul(s(b))), true),
        Op::DivS(_, b) | Op::RemS(_, b) if s(b) == 0 => (Err(ARITHMETIC_DIVIDE_BY_ZERO), true),
        Op::DivS(a, b) => (signed(s(a).checked_div(s(b))), true),
        Op::RemS(a, b) => (signed(s(a).checked_rem(s(b))), true),
        Op::AddU(a, b) => (unsigned(r(a).checked_add(r(b))), true),
        Op::SubU(a, b) => (unsigned(r(a).checked_sub(r(b))), true),
        Op::MulU(a, b) => (unsigned(r(a).checked_mul(r(b))), true),
        Op::DivU(_, b) | Op::RemU(_, b) if r(b) == 0 => (Err(ARITHMETIC_DIVIDE_BY_ZERO), true),
        Op::DivU(a, b) => (Ok(r(a) / r(b)), true),
        Op::RemU(a, b) => (Ok(r(a) % r(b)), true),
        Op::Neg(a) => (signed(s(a).checked_neg()), true),
        Op::Not(a) => (Ok(r(a) ^ 1), false),
        Op::And(a, b) => (Ok(r(a) & r(b)), false),
        Op::Or(a, b) => (Ok(r(a) | r(b)), false),
        Op::VecLen(a) => (
            Ok(u64::try_from(vector(vectors, r(a)).len()).unwrap_or(u64::MAX)),
            false,
        ),
        Op::VecGet(a, b) => {
            let out = step.output as usize;
            let item = usize::try_from(r(b))
                .ok()
                .and_then(|i| vector(vectors, r(a)).get(i));
            if let Some(&word) = item {
                registers[out] = word;
                flags[out] = false;
                return step.units[0];
            }
            registers[out] = 0;
            flags[out] = true;
            return step.units[1];
        }
        Op::Call(_) | Op::Guard(_) | Op::GuardTrue(_) | Op::GuardFalse(_) => return 0,
        Op::Copy(a) => {
            registers[step.output as usize] = r(a);
            return 0;
        }
    };
    let out = step.output as usize;
    match value {
        Ok(word) => {
            registers[out] = word;
            if checked {
                flags[out] = false;
            }
            step.units[0]
        }
        Err(code) => {
            registers[out] = u64::from(code);
            flags[out] = true;
            step.units[1]
        }
    }
}

pub(super) enum WordEnded {
    Returned(u64),
    Stopped(ExecutionTermination),
}

/// How one pass of a burst over a frame's fast blocks ended.
enum Pass {
    /// The burst's blocks are used up.
    Out,
    /// At a call site.
    Call(u32),
    /// At a block returning a word.
    Return(usize, u64),
    /// At a trap.
    End(usize),
}

/// A caller waiting on a call: where it resumes is the start of its fast
/// block `tail`, the step after the call.
struct Frame {
    function: u32,
    output: u32,
    tail: u32,
    base: usize,
    units: u64,
}

/// `scalar::run` on words: the same charges, counts and order of effects.
#[allow(clippy::too_many_lines)]
#[inline(never)]
pub(super) fn run(
    program: &Program<'_, '_>,
    words: &Words,
    constants: &[Vec<u64>],
    runtime: &mut Runtime,
    limits: &ExecutionLimits,
    inputs: &[u64],
    vectors: &[Vec<u64>],
) -> RuntimeResult<WordEnded> {
    let mut function = 0;
    let mut plan = program.plans[0];
    let mut code = program.codes[0];
    let mut steps = &words.functions[0];
    let mut consts = &constants[0][..];
    // One register file for every frame: a frame's slots start at its base,
    // a callee's just past its caller's. Its words are written before they
    // are read (derived bytecode is SSA), so a reused range needs no reset.
    let mut file = vec![0u64; plan.slots];
    let mut flag_file = vec![false; plan.slots];
    let mut base = 0;
    let mut scratch: Vec<u64> = Vec::with_capacity(plan.scratch);
    let mut stack: Vec<Frame> = Vec::new();
    let mut pc = 0;
    for (&r, &value) in plan.parameters.iter().zip(inputs) {
        *file.get_mut(r).ok_or(RuntimeFault)? = value;
    }
    // Each function's single-writer constant slots with their words.
    let prefill = words
        .functions
        .iter()
        .zip(constants)
        .map(|(f, consts)| {
            f.constant_slots
                .iter()
                .map(|&(slot, index)| Some((slot as usize, *consts.get(index as usize)?)))
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(RuntimeFault)?;
    for &(slot, word) in &prefill[0] {
        *file.get_mut(slot).ok_or(RuntimeFault)? = word;
    }
    'frames: loop {
        let mut prepaid_exit = false;
        // A burst: fast blocks, calls and returns run back to back, each
        // charging exactly what the general loop charges, against one
        // prepaid worst case.
        let entry = if pc == 0 {
            Some(runtime.block)
        } else {
            steps.fast[plan.blocks.len()..]
                .iter()
                .position(|f| f.start == (runtime.block, pc))
                .map(|i| i + plan.blocks.len())
        };
        if let Some(mut at) = entry
            && super::prepaid(runtime, limits, &words.burst)
        {
            let mut fuel: u64 = 0;
            let mut count: u64 = 0;
            let mut units: u64 = 0;
            let mut stopped = None;
            let mut left_over = BURST;
            // Each pass runs the current frame's fast blocks until a call,
            // a return or an exit that leaves the burst.
            'burst: loop {
                let registers = &mut file[base..base + steps.slots];
                let flags = &mut flag_file[base..base + steps.slots];
                let end = loop {
                    if left_over == 0 {
                        break Pass::Out;
                    }
                    left_over -= 1;
                    let fast = &steps.fast[at];
                    let mut left = None;
                    for step in &fast.steps {
                        match step.op {
                            Op::Guard(g) => {
                                if flags[step.output as usize] {
                                    left = Some(g);
                                    break;
                                }
                            }
                            Op::GuardTrue(g) => {
                                if registers[step.output as usize] == 0 {
                                    left = Some(g);
                                    break;
                                }
                            }
                            Op::GuardFalse(g) => {
                                if registers[step.output as usize] != 0 {
                                    left = Some(g);
                                    break;
                                }
                            }
                            _ => {
                                units += exec(step, consts, vectors, registers, flags);
                                let out = step.output as usize;
                                let failed = match step.guard {
                                    Guarded::No => None,
                                    Guarded::Ok(g) => flags[out].then_some(g),
                                    Guarded::True(g) => (registers[out] == 0).then_some(g),
                                    Guarded::False(g) => (registers[out] != 0).then_some(g),
                                };
                                if failed.is_some() {
                                    left = failed;
                                    break;
                                }
                                if step.also != NONE {
                                    registers[step.also as usize] = registers[out];
                                }
                            }
                        }
                    }
                    let leave = match left {
                        Some(g) => fast.guards[g as usize],
                        None => Leave {
                            block: fast.last,
                            fuel: fast.fuel,
                            count: fast.count,
                            units: fast.units,
                        },
                    };
                    count += leave.count;
                    fuel += leave.fuel;
                    units += leave.units;
                    let block = leave.block;
                    if left.is_none()
                        && let Some(site) = fast.call
                    {
                        break Pass::Call(site);
                    }
                    let (edge, payload) = match &steps.exits[block] {
                        WordExit::Branch(edge) => (edge, 0),
                        WordExit::Cond(r, yes, no) => {
                            (if registers[*r as usize] != 0 { yes } else { no }, 0)
                        }
                        WordExit::Switch(r, ok, err) => (
                            if flags[*r as usize] { err } else { ok },
                            registers[*r as usize],
                        ),
                        WordExit::Return(r) => break Pass::Return(block, registers[*r as usize]),
                        WordExit::End => break Pass::End(block),
                    };
                    fuel += edge.fuel;
                    if edge.direct {
                        for &(from, to) in &edge.moves {
                            registers[to as usize] = if from == PAYLOAD {
                                payload
                            } else {
                                registers[from as usize]
                            };
                        }
                    } else {
                        scratch.clear();
                        for &(from, _) in &edge.moves {
                            scratch.push(if from == PAYLOAD {
                                payload
                            } else {
                                registers[from as usize]
                            });
                        }
                        for (&(_, to), &v) in edge.moves.iter().zip(&scratch) {
                            registers[to as usize] = v;
                        }
                    }
                    at = edge.target;
                };
                match end {
                    Pass::Out => break 'burst,
                    Pass::Call(site) => {
                        // The call's instruction, the depth check, its action.
                        let site = &steps.sites[site as usize];
                        fuel += 1;
                        let depth = stack.len().saturating_add(1);
                        if depth.saturating_add(1) > runtime.max_call_depth {
                            stopped =
                                Some(ExecutionTermination::ResourceLimit(ResourceKind::CallDepth));
                            break 'burst;
                        }
                        fuel += 1;
                        runtime.peak_call_depth =
                            runtime.peak_call_depth.max(depth.saturating_add(1));
                        units += site.arg_units;
                        let callee = &words.functions[site.function];
                        let callee_base = base + steps.slots;
                        let end = callee_base + callee.slots;
                        if file.len() < end {
                            file.resize(end, 0);
                            flag_file.resize(end, false);
                        }
                        for (&a, &parameter) in site.args.iter().zip(&callee.parameters) {
                            file[callee_base + parameter] = file[base + a as usize];
                        }
                        for &(slot, word) in &prefill[site.function] {
                            file[callee_base + slot] = word;
                        }
                        stack.push(Frame {
                            function: steps.index,
                            base,
                            output: site.output,
                            tail: site.tail,
                            units: site.units,
                        });
                        base = callee_base;
                        function = site.function;
                        steps = callee;
                        consts = &constants[function][..];
                        at = steps.entry;
                    }
                    Pass::Return(block, value) => {
                        let Some(caller) = stack.pop() else {
                            // The run's own return: the general exit runs.
                            at = block;
                            pc = steps.blocks[block].len();
                            prepaid_exit = true;
                            break 'burst;
                        };
                        fuel += 1;
                        if steps.result_units > limits.max_output_units {
                            stopped = Some(ExecutionTermination::ResourceLimit(
                                ResourceKind::OutputUnits,
                            ));
                            break 'burst;
                        }
                        function = caller.function as usize;
                        steps = &words.functions[function];
                        consts = &constants[function][..];
                        base = caller.base;
                        file[base + caller.output as usize] = value;
                        units += caller.units;
                        count += 1;
                        at = caller.tail as usize;
                    }
                    Pass::End(block) => {
                        at = block;
                        pc = steps.blocks[block].len();
                        prepaid_exit = true;
                        break 'burst;
                    }
                }
            }
            plan = program.plans[function];
            code = program.codes[function];
            runtime.fuel_used += fuel;
            runtime.instruction_count += count;
            runtime.live_value_units += units;
            runtime.peak_value_units = runtime.peak_value_units.max(runtime.live_value_units);
            if let Some(end) = stopped {
                return Ok(WordEnded::Stopped(end));
            }
            if prepaid_exit {
                runtime.block = at;
            } else {
                (runtime.block, pc) = steps.fast[at].start;
            }
        }
        let registers = &mut file[base..base + plan.slots];
        let flags = &mut flag_file[base..base + plan.slots];
        let block = plan.blocks.get(runtime.block).ok_or(RuntimeFault)?;
        let wsteps = &steps.blocks[runtime.block];
        if !prepaid_exit
            && let Some(batch) = block.batches.iter().find(|batch| batch.start == pc)
            && super::prepaid(runtime, limits, batch)
        {
            let mut units: u64 = 0;
            for step in &wsteps[pc..pc + batch.steps] {
                units += exec(step, consts, vectors, registers, flags);
            }
            let count = batch.steps as u64;
            runtime.fuel_used += count;
            runtime.instruction_count += count;
            runtime.live_value_units += units;
            runtime.peak_value_units = runtime.peak_value_units.max(runtime.live_value_units);
            pc += batch.steps;
            prepaid_exit = batch.exit;
        }
        macro_rules! charge {
            () => {
                if prepaid_exit {
                    runtime.fuel_used += 1;
                } else if let Some(end) = charge_action(runtime, limits, None) {
                    return Ok(WordEnded::Stopped(end));
                }
            };
        }
        while let Some(step) = wsteps.get(pc) {
            pc += 1;
            if let Some(end) = charge_action(runtime, limits, Some(ResourceKind::Instruction)) {
                return Ok(WordEnded::Stopped(end));
            }
            if let Op::Call(site) = step.op {
                let source = &block.steps[pc - 1];
                let call = source.call.as_ref().ok_or(RuntimeFault)?;
                let depth = stack.len().saturating_add(1);
                if depth.saturating_add(1) > runtime.max_call_depth {
                    return Ok(WordEnded::Stopped(ExecutionTermination::ResourceLimit(
                        ResourceKind::CallDepth,
                    )));
                }
                if let Some(end) = charge_action(runtime, limits, None) {
                    return Ok(WordEnded::Stopped(end));
                }
                runtime.peak_call_depth = runtime.peak_call_depth.max(depth.saturating_add(1));
                let callee_plan = program.plans[call.function];
                scratch.clear();
                for (&r, units) in source.args.iter().zip(&call.arguments) {
                    let value = *registers.get(r).ok_or(RuntimeFault)?;
                    if !charge_value(runtime, units.fixed[0], limits.max_value_units) {
                        return Ok(WordEnded::Stopped(ExecutionTermination::ResourceLimit(
                            ResourceKind::ValueUnits,
                        )));
                    }
                    scratch.push(value);
                }
                let callee_base = base + plan.slots;
                let end = callee_base + callee_plan.slots;
                if file.len() < end {
                    file.resize(end, 0);
                    flag_file.resize(end, false);
                }
                for (&parameter, &value) in callee_plan.parameters.iter().zip(&scratch) {
                    *file.get_mut(callee_base + parameter).ok_or(RuntimeFault)? = value;
                }
                for &(slot, word) in &prefill[call.function] {
                    *file.get_mut(callee_base + slot).ok_or(RuntimeFault)? = word;
                }
                stack.push(Frame {
                    function: steps.index,
                    base,
                    output: step.output,
                    tail: steps.sites.get(site as usize).ok_or(RuntimeFault)?.tail,
                    units: step.units[0],
                });
                base = callee_base;
                function = call.function;
                plan = callee_plan;
                code = program.codes[function];
                steps = &words.functions[function];
                consts = &constants[function][..];
                runtime.block = usize::try_from(code.entry_block).map_err(|_| RuntimeFault)?;
                pc = 0;
                continue 'frames;
            }
            // The step's value is written before its units are charged; a
            // failed charge ends the run, so the write is never read.
            let units = exec(step, consts, vectors, registers, flags);
            if !charge_value(runtime, units, limits.max_value_units) {
                return Ok(WordEnded::Stopped(ExecutionTermination::ResourceLimit(
                    ResourceKind::ValueUnits,
                )));
            }
            runtime.instruction_count = runtime.instruction_count.saturating_add(1);
        }
        charge!();
        let (edge, payload) = match &block.exit {
            Exit::Return(r) => {
                let value = *registers.get(*r).ok_or(RuntimeFault)?;
                let Some(caller) = stack.pop() else {
                    return Ok(WordEnded::Returned(value));
                };
                if plan.result.fixed[0] > limits.max_output_units {
                    return Ok(WordEnded::Stopped(ExecutionTermination::ResourceLimit(
                        ResourceKind::OutputUnits,
                    )));
                }
                function = caller.function as usize;
                plan = program.plans[function];
                code = program.codes[function];
                steps = &words.functions[function];
                consts = &constants[function][..];
                base = caller.base;
                (runtime.block, pc) = steps.fast[caller.tail as usize].start;
                if !charge_value(runtime, caller.units, limits.max_value_units) {
                    return Ok(WordEnded::Stopped(ExecutionTermination::ResourceLimit(
                        ResourceKind::ValueUnits,
                    )));
                }
                runtime.instruction_count = runtime.instruction_count.saturating_add(1);
                file[base + caller.output as usize] = value;
                continue 'frames;
            }
            Exit::Trap(tag, payload) => {
                let payload = payload
                    .map(|(slot, r)| {
                        to_const(
                            *registers.get(slot).ok_or(RuntimeFault)?,
                            &code.register_types[r],
                        )
                    })
                    .transpose()?;
                return Ok(WordEnded::Stopped(
                    if payload
                        .as_ref()
                        .is_some_and(|v| super::value_units_const(v) > limits.max_output_units)
                    {
                        ExecutionTermination::ResourceLimit(ResourceKind::OutputUnits)
                    } else {
                        ExecutionTermination::Trap {
                            trap_tag: *tag,
                            payload,
                        }
                    },
                ));
            }
            Exit::Branch(edge) => (edge, 0),
            Exit::Cond(r, yes, no) => (
                if *registers.get(*r).ok_or(RuntimeFault)? != 0 {
                    yes
                } else {
                    no
                },
                0,
            ),
            Exit::Switch {
                register, cases, ..
            } => {
                let value = *registers.get(*register).ok_or(RuntimeFault)?;
                let option = cases.iter().any(|(case, _)| {
                    matches!(
                        case,
                        CaseKey::Builtin(BuiltinCase::Some | BuiltinCase::None)
                    )
                });
                let key = CaseKey::Builtin(match (option, flags[*register]) {
                    (false, true) => BuiltinCase::Err,
                    (false, false) => BuiltinCase::Ok,
                    (true, true) => BuiltinCase::None,
                    (true, false) => BuiltinCase::Some,
                });
                let mut selected = None;
                for (case, edge) in cases {
                    charge!();
                    if *case == key {
                        selected = Some(edge);
                        break;
                    }
                }
                (selected.ok_or(RuntimeFault)?, value)
            }
        };
        pc = 0;
        if prepaid_exit {
            runtime.fuel_used += edge.fuel;
            if edge.moves_direct {
                for (arg, slot) in &edge.moves {
                    registers[*slot] = match arg.register {
                        Some(r) => registers[r],
                        None => payload,
                    };
                }
            } else {
                scratch.clear();
                for (arg, _) in &edge.moves {
                    scratch.push(match arg.register {
                        Some(r) => registers[r],
                        None => payload,
                    });
                }
                for ((_, slot), &v) in edge.moves.iter().zip(&scratch) {
                    registers[*slot] = v;
                }
            }
            runtime.block = edge.target;
            continue;
        }
        let parameters = &plan.blocks[edge.target].parameters;
        scratch.clear();
        for arg in &edge.args {
            charge!();
            let value = if let Some(r) = arg.register {
                *registers.get(r).ok_or(RuntimeFault)?
            } else {
                charge!();
                payload
            };
            scratch.push(value);
        }
        for (&r, &v) in parameters.iter().zip(&scratch) {
            registers[r] = v;
        }
        runtime.block = edge.target;
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{
        BytecodeSwitchArgument, BytecodeSwitchEdge, BytecodeTargetEdge, BytecodeTerminator,
        CacheProfile, ConstantDefinition, ExecutionRequest, ExecutionSource, LoweredFunction,
        MAX_CALL_DEPTH, SchemaEpochId, StateRoot, TypeEnvironment, ValidatedInputs, finish,
        hash_validated_value, run_core, value_units_const,
    };
    use super::super::{CoreOutcome, Plan};
    use super::*;
    use crate::{BytecodeBlock, BytecodeFunction, BytecodeSwitchCase, Instruction};
    use sley_id::EntityId;
    use sley_ssmc::{ConstData, FunctionRefValue, Immediate, IntegerWidth};

    fn instruction(
        opcode: Opcode,
        operands: Vec<u32>,
        output: u32,
        immediate: Immediate,
    ) -> Instruction {
        Instruction {
            opcode: opcode.tag(),
            operands,
            results: vec![output],
            immediate,
        }
    }

    fn block(
        slot: u32,
        parameters: Vec<u32>,
        instructions: Vec<Instruction>,
        terminator: BytecodeTerminator,
    ) -> BytecodeBlock {
        BytecodeBlock {
            slot,
            parameter_registers: parameters,
            instructions,
            terminator,
            reachability: 1,
        }
    }

    /// A switch on a checked result: Ok to `ok` with the value, Err to `err`
    /// with the failure code (Err listed first when `err_first`).
    fn switch(value: u32, ok: u32, err: u32, err_first: bool) -> BytecodeTerminator {
        let case = |key, target| BytecodeSwitchCase {
            case_key: CaseKey::Builtin(key),
            edge: BytecodeSwitchEdge {
                target,
                arguments: vec![BytecodeSwitchArgument::CasePayload],
            },
        };
        let mut cases = vec![case(BuiltinCase::Ok, ok), case(BuiltinCase::Err, err)];
        if err_first {
            cases.reverse();
        }
        BytecodeTerminator::VariantSwitch { value, cases }
    }

    /// Runs `bytecode` on each input row under every fuel, cancel,
    /// instruction, value-unit and output cut up to the reference's own
    /// totals, and compares the word tier with the reference runner and with
    /// the scalar loop (the same plan without its word form).
    #[allow(clippy::too_many_lines)]
    fn sweep(
        bytecode: BytecodeFunction,
        constants: &[ConstantDefinition],
        rows: &[Vec<ConstValue>],
    ) {
        let epoch = SchemaEpochId::from_bytes([8; 32]);
        let root = StateRoot::from_bytes([7; 32]);
        let types = TypeEnvironment::new(Vec::new()).unwrap();
        let source = ExecutionSource {
            types: &types,
            constants,
            globals: &[],
            contracts: &[],
            adapters: &[],
            schema_epoch: epoch,
            state_root: root,
            profile: CacheProfile::EXTENDED_V1,
            function: bytecode.function,
        };
        let key = crate::derive_cache_key(epoch, root, bytecode.function, source.profile).unwrap();
        let lowered = LoweredFunction {
            bytecode,
            bytes: vec![0; 64],
            cache_key: key,
            lowering_work: 0,
            callees: vec![],
        };
        let plan = Plan::compile(&lowered.bytecode).unwrap();
        assert!(
            plan.words.is_some(),
            "the fixture must run on the word tier"
        );
        let mut scalar = plan.clone();
        scalar.words = None;
        let limits = ExecutionLimits {
            max_instructions: 100_000,
            max_fuel: 100_000,
            max_value_units: 1_000_000,
            max_output_units: 1000,
            cancel_at_fuel: None,
        };
        for inputs in rows {
            let validated = ValidatedInputs {
                hashes: inputs
                    .iter()
                    .map(|v| hash_validated_value(epoch, v).unwrap())
                    .collect(),
                value_units: inputs.iter().map(value_units_const).sum(),
            };
            let request = ExecutionRequest {
                inputs: inputs.clone(),
                limits,
            };
            let baseline = run_core(
                &source,
                &lowered,
                &validated,
                request.clone(),
                MAX_CALL_DEPTH,
            );
            let mut caps = vec![
                limits,
                ExecutionLimits {
                    max_fuel: u64::MAX,
                    max_instructions: u64::MAX,
                    max_value_units: u64::MAX,
                    max_output_units: u64::MAX,
                    ..limits
                },
            ];
            // Every cut on small runs; about 300 evenly spaced cuts, plus the
            // last few, on the deep recursions.
            let stride = usize::try_from(baseline.fuel_used / 300).unwrap().max(1);
            let near = baseline.fuel_used.saturating_sub(3)..=baseline.fuel_used + 1;
            for cap in (0..=baseline.fuel_used + 1).step_by(stride).chain(near) {
                caps.push(ExecutionLimits {
                    max_fuel: cap,
                    ..limits
                });
                caps.push(ExecutionLimits {
                    cancel_at_fuel: Some(cap),
                    ..limits
                });
                caps.push(ExecutionLimits {
                    max_instructions: cap,
                    ..limits
                });
            }
            let stride = usize::try_from(baseline.peak_value_units / 300)
                .unwrap()
                .max(1);
            for cap in (0..=baseline.peak_value_units + 1).step_by(stride) {
                caps.push(ExecutionLimits {
                    max_value_units: cap,
                    ..limits
                });
            }
            for cap in 0..=24 {
                caps.push(ExecutionLimits {
                    max_output_units: cap,
                    ..limits
                });
            }
            for limits in caps {
                let request = ExecutionRequest {
                    limits,
                    ..request.clone()
                };
                let observed = |r: CoreOutcome| {
                    (
                        r.peak_call_depth,
                        finish(
                            &source,
                            limits,
                            key,
                            &validated.hashes,
                            r.termination,
                            r.instruction_count,
                            r.fuel_used,
                            r.peak_value_units,
                        )
                        .unwrap(),
                    )
                };
                let reference = observed(run_core(
                    &source,
                    &lowered,
                    &validated,
                    request.clone(),
                    MAX_CALL_DEPTH,
                ));
                let words = observed(
                    plan.execute(&source, &lowered, &validated, &request)
                        .unwrap(),
                );
                let general = observed(
                    scalar
                        .execute(&source, &lowered, &validated, &request)
                        .unwrap(),
                );
                assert_eq!(words.1, reference.1, "inputs={inputs:?} limits={limits:?}");
                assert_eq!(words, general, "inputs={inputs:?} limits={limits:?}");
            }
        }
    }

    fn sint(v: i128) -> ConstValue {
        ConstValue {
            value_type: TypeExpr::SInt(IntegerWidth::from_bits(64)),
            data: ConstData::SInt(v),
        }
    }

    fn uint(v: u128) -> ConstValue {
        ConstValue {
            value_type: TypeExpr::UInt(IntegerWidth::from_bits(64)),
            data: ConstData::UInt(v),
        }
    }

    /// Every signed word operation, each failure route (overflow, divide by
    /// zero, `MIN / -1`, negating `MIN`), the boolean operations and both
    /// case orders of a switch, on the word tier and the reference.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn signed_operations_and_failures_match_reference_on_every_budget_cut() {
        let ty = TypeExpr::SInt(IntegerWidth::from_bits(64));
        let failure = TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic);
        let checked = TypeExpr::Result {
            ok: Box::new(ty.clone()),
            error: Box::new(failure.clone()),
        };
        let mut types = vec![ty.clone(); 32];
        for r in [2, 4, 6, 8, 18, 22] {
            types[r] = checked.clone();
        }
        for r in [10, 11, 12, 13, 14, 15, 23, 24, 25, 26, 27] {
            types[r] = TypeExpr::Bool;
        }
        types[20] = failure;
        let i = |op, a: Vec<u32>, out| instruction(op, a, out, Immediate::None);
        let bytecode = BytecodeFunction {
            function: EntityId::from_bytes([9; 32]),
            parameter_registers: vec![0, 1],
            register_types: types,
            result_type: ty.clone(),
            entry_block: 0,
            blocks: vec![
                block(
                    0,
                    vec![],
                    vec![i(Opcode::IntMulChecked, vec![0, 1], 2)],
                    switch(2, 1, 7, false),
                ),
                block(
                    1,
                    vec![3],
                    vec![i(Opcode::IntDivChecked, vec![3, 1], 4)],
                    switch(4, 2, 7, true),
                ),
                block(
                    2,
                    vec![5],
                    vec![i(Opcode::IntRemChecked, vec![5, 1], 6)],
                    switch(6, 3, 7, false),
                ),
                block(
                    3,
                    vec![7],
                    vec![i(Opcode::IntNegChecked, vec![7], 8)],
                    switch(8, 4, 7, true),
                ),
                block(
                    4,
                    vec![9],
                    vec![
                        i(Opcode::LessThan, vec![9, 0], 10),
                        i(Opcode::GreaterEqual, vec![9, 1], 11),
                        i(Opcode::LessEqual, vec![9, 1], 24),
                        i(Opcode::GreaterThan, vec![9, 0], 25),
                        i(Opcode::NotEqual, vec![9, 0], 23),
                        i(Opcode::BoolAnd, vec![10, 11], 12),
                        i(Opcode::BoolOr, vec![24, 25], 26),
                        i(Opcode::BoolAnd, vec![26, 23], 27),
                        i(Opcode::BoolNot, vec![12], 13),
                        i(Opcode::BoolOr, vec![13, 27], 14),
                        i(Opcode::Equal, vec![14, 10], 15),
                    ],
                    BytecodeTerminator::CondBranch {
                        condition: 15,
                        if_true: BytecodeTargetEdge {
                            target: 5,
                            arguments: vec![9],
                        },
                        if_false: BytecodeTargetEdge {
                            target: 6,
                            arguments: vec![9, 1],
                        },
                    },
                ),
                block(5, vec![16], vec![], BytecodeTerminator::Return(16)),
                block(
                    6,
                    vec![17, 21],
                    vec![i(Opcode::IntSubChecked, vec![17, 0], 18)],
                    switch(18, 8, 7, false),
                ),
                block(
                    7,
                    vec![20],
                    vec![],
                    BytecodeTerminator::Trap {
                        code: 3,
                        payload: Some(20),
                    },
                ),
                block(
                    8,
                    vec![19],
                    vec![i(Opcode::IntAddChecked, vec![19, 21], 22)],
                    switch(22, 5, 7, true),
                ),
            ],
        };
        let min = i128::from(i64::MIN);
        let max = i128::from(i64::MAX);
        let rows: Vec<Vec<ConstValue>> = [
            (7, 3),
            (-7, 3),
            (7, -3),
            (0, 5),
            (5, 0),
            (min, -1),
            (min, 1),
            (max, 2),
            (max, 1),
            (-1, min),
            (3, max),
            (-9, -2),
        ]
        .into_iter()
        .map(|(x, y)| vec![sint(x), sint(y)])
        .collect();
        sweep(bytecode, &[], &rows);
    }

    /// Unsigned arithmetic and direct recursion: `sum(n) = n + sum(n - 1)`
    /// with `sum(0) = 0`, through the call depth ceiling and its overflow.
    #[test]
    fn unsigned_recursion_matches_reference_on_every_budget_cut() {
        let ty = TypeExpr::UInt(IntegerWidth::from_bits(64));
        let failure = TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic);
        let checked = TypeExpr::Result {
            ok: Box::new(ty.clone()),
            error: Box::new(failure.clone()),
        };
        let function = EntityId::from_bytes([9; 32]);
        let mut types = vec![ty.clone(); 12];
        types[1] = TypeExpr::Bool;
        types[3] = checked.clone();
        types[7] = checked;
        types[8] = failure;
        let i = |op, a: Vec<u32>, out| instruction(op, a, out, Immediate::None);
        let call = instruction(
            Opcode::CallDirect,
            vec![4],
            5,
            Immediate::Function(FunctionRefValue {
                function,
                type_arguments: vec![],
            }),
        );
        let bytecode = BytecodeFunction {
            function,
            parameter_registers: vec![0],
            register_types: types,
            result_type: ty.clone(),
            entry_block: 0,
            blocks: vec![
                block(
                    0,
                    vec![],
                    vec![i(Opcode::IntSubChecked, vec![0, 0], 3)],
                    switch(3, 1, 4, false),
                ),
                block(
                    1,
                    vec![2],
                    vec![i(Opcode::Equal, vec![0, 2], 1)],
                    BytecodeTerminator::CondBranch {
                        condition: 1,
                        if_true: BytecodeTargetEdge {
                            target: 2,
                            arguments: vec![2],
                        },
                        if_false: BytecodeTargetEdge {
                            target: 3,
                            arguments: vec![2],
                        },
                    },
                ),
                block(2, vec![6], vec![], BytecodeTerminator::Return(6)),
                block(
                    3,
                    vec![9],
                    vec![i(Opcode::IntDivChecked, vec![0, 0], 10)],
                    switch(10, 5, 4, true),
                ),
                block(
                    4,
                    vec![8],
                    vec![],
                    BytecodeTerminator::Trap {
                        code: 5,
                        payload: Some(8),
                    },
                ),
                block(
                    5,
                    vec![11],
                    vec![
                        // n - (n / n) = n - 1, then the call and the sum.
                        i(Opcode::IntSubChecked, vec![0, 11], 3),
                    ],
                    switch(3, 6, 4, false),
                ),
                block(
                    6,
                    vec![4],
                    vec![call, i(Opcode::IntAddChecked, vec![0, 5], 7)],
                    switch(7, 2, 4, true),
                ),
            ],
        };
        // Register 3 is defined twice above; give block 5 its own result.
        let mut bytecode = bytecode;
        bytecode
            .register_types
            .push(bytecode.register_types[3].clone());
        bytecode.blocks[5].instructions[0].results = vec![12];
        bytecode.blocks[5].terminator = switch(12, 6, 4, false);
        bytecode.register_types[10] = bytecode.register_types[3].clone();
        let rows: Vec<Vec<ConstValue>> = [0, 1, 2, 5, 40, 254, 255, 256, 300]
            .into_iter()
            .map(|n| vec![uint(n)])
            .collect();
        sweep(bytecode, &[], &rows);
    }

    /// A loop shaped like the Python dialect's: constants (one entity in two
    /// registers), checked steps whose Ok edges continue into whole blocks,
    /// an unconditional branch with moves, and conditional exits whose other
    /// side returns, on either side. Bursts fuse these into fast blocks; an
    /// overflow or a loop exit leaves one at a guard.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn fused_fast_blocks_match_reference_on_every_budget_cut() {
        let ty = TypeExpr::SInt(IntegerWidth::from_bits(64));
        let failure = TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic);
        let checked = TypeExpr::Result {
            ok: Box::new(ty.clone()),
            error: Box::new(failure.clone()),
        };
        let mut types = vec![ty.clone(); 23];
        for r in [11, 14, 16] {
            types[r] = checked.clone();
        }
        types[7] = TypeExpr::Bool;
        types[20] = TypeExpr::Bool;
        types[21] = failure;
        let i = |op, a: Vec<u32>, out| instruction(op, a, out, Immediate::None);
        let one = EntityId::from_bytes([1; 32]);
        let zero = EntityId::from_bytes([2; 32]);
        let constant =
            |id, out| instruction(Opcode::ConstantRef, vec![], out, Immediate::Entity(id));
        let edge = |target, arguments| BytecodeTargetEdge { target, arguments };
        // f(n, k): acc = 0; i = n; loop { acc = acc * k + 1; i -= 1;
        // if i < k or i <= 0 { return acc } }  (checked throughout)
        let bytecode = BytecodeFunction {
            function: EntityId::from_bytes([9; 32]),
            parameter_registers: vec![0, 1],
            register_types: types,
            result_type: ty.clone(),
            entry_block: 0,
            blocks: vec![
                block(
                    0,
                    vec![],
                    vec![constant(one, 2), constant(zero, 4)],
                    BytecodeTerminator::Branch(edge(3, vec![0, 4])),
                ),
                block(
                    1,
                    vec![5, 6],
                    vec![i(Opcode::LessEqual, vec![5, 4], 7)],
                    BytecodeTerminator::CondBranch {
                        condition: 7,
                        if_true: edge(2, vec![6]),
                        if_false: edge(3, vec![5, 6]),
                    },
                ),
                block(2, vec![8], vec![], BytecodeTerminator::Return(8)),
                block(
                    3,
                    vec![9, 10],
                    vec![i(Opcode::IntMulChecked, vec![10, 1], 11)],
                    switch(11, 4, 7, false),
                ),
                block(
                    4,
                    vec![12],
                    vec![
                        constant(one, 13),
                        i(Opcode::IntAddChecked, vec![12, 13], 14),
                    ],
                    switch(14, 5, 7, true),
                ),
                block(
                    5,
                    vec![15],
                    vec![i(Opcode::IntSubChecked, vec![9, 2], 16)],
                    switch(16, 6, 7, false),
                ),
                block(
                    6,
                    vec![17],
                    vec![],
                    BytecodeTerminator::Branch(edge(8, vec![17, 15])),
                ),
                block(
                    7,
                    vec![21],
                    vec![],
                    BytecodeTerminator::Trap {
                        code: 3,
                        payload: Some(21),
                    },
                ),
                block(
                    8,
                    vec![18, 19],
                    vec![i(Opcode::GreaterEqual, vec![18, 1], 20)],
                    BytecodeTerminator::CondBranch {
                        condition: 20,
                        if_true: edge(1, vec![18, 19]),
                        if_false: edge(9, vec![19]),
                    },
                ),
                block(9, vec![22], vec![], BytecodeTerminator::Return(22)),
            ],
        };
        let value = |n| ConstValue {
            value_type: ty.clone(),
            data: ConstData::SInt(n),
        };
        let constants = [
            ConstantDefinition {
                entity_id: one,
                value: value(1),
            },
            ConstantDefinition {
                entity_id: zero,
                value: value(0),
            },
        ];
        let words = Plan::compile(&bytecode).unwrap().words.unwrap();
        let fast = &words.functions[0].fast;
        let guards = |op: fn(&WordStep) -> bool| fast.iter().any(|f| f.steps.iter().any(op));
        assert!(
            guards(|s| matches!(s.op, Op::Guard(_)) || matches!(s.guard, Guarded::Ok(_)))
                && guards(|s| {
                    matches!(s.op, Op::GuardTrue(_)) || matches!(s.guard, Guarded::True(_))
                })
                && guards(|s| {
                    matches!(s.op, Op::GuardFalse(_)) || matches!(s.guard, Guarded::False(_))
                })
                && fast
                    .iter()
                    .any(|f| f.steps.iter().any(|s| matches!(s.op, Op::Copy(_)))),
            "the fixture fuses every kind of edge: {fast:?}"
        );
        assert!(
            !words.functions[0].constant_slots.is_empty(),
            "constants are prefilled"
        );
        let min = i128::from(i64::MIN);
        let max = i128::from(i64::MAX);
        let rows: Vec<Vec<ConstValue>> = [
            (0, 1),
            (-3, 2),
            (5, 1),
            (9, -1),
            (12, -3),
            (20, 2),
            (40, 3),
            (6, 4),
            (3, max),
            (3, min),
            (max, 2),
            (min, 0),
        ]
        .into_iter()
        .map(|(n, k)| vec![sint(n), sint(k)])
        .collect();
        sweep(bytecode, &constants, &rows);
    }

    /// A vector input on the word tier: `vec_len`, and `vec_get` switched
    /// on `Some`/`None` in its block, in a loop summing every `k`-th element
    /// (checked), shaped like the Python dialect's indexed loops: the end of
    /// the vector returns, an overflowing sum or index traps, and every
    /// budget cut stops it. Bursts fuse the `Some` side behind a guard.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn vector_reads_match_reference_on_every_budget_cut() {
        let s64 = TypeExpr::SInt(IntegerWidth::from_bits(64));
        let u64t = TypeExpr::UInt(IntegerWidth::from_bits(64));
        let vector = TypeExpr::Vector(Box::new(s64.clone()));
        let failure = TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic);
        let checked = |ty: &TypeExpr| TypeExpr::Result {
            ok: Box::new(ty.clone()),
            error: Box::new(failure.clone()),
        };
        let types = vec![
            vector.clone(),                          // 0 xs
            u64t.clone(),                            // 1 k
            u64t.clone(),                            // 2 n
            u64t.clone(),                            // 3 zero (u64)
            TypeExpr::Bool,                          // 4 n != 0
            s64.clone(),                             // 5 zero (i64)
            u64t.clone(),                            // 6 i
            s64.clone(),                             // 7 acc
            TypeExpr::Option(Box::new(s64.clone())), // 8 xs[i]
            s64.clone(),                             // 9 v
            s64.clone(),                             // 10 acc
            u64t.clone(),                            // 11 i
            checked(&s64),                           // 12 acc + v
            s64.clone(),                             // 13 acc
            u64t.clone(),                            // 14 i
            checked(&u64t),                          // 15 i + k
            s64.clone(),                             // 16 result
            failure.clone(),                         // 17 failure
        ];
        let i = |op, a: Vec<u32>, out| instruction(op, a, out, Immediate::None);
        let zero_u = EntityId::from_bytes([3; 32]);
        let zero_i = EntityId::from_bytes([4; 32]);
        let constant =
            |id, out| instruction(Opcode::ConstantRef, vec![], out, Immediate::Entity(id));
        let edge = |target, arguments| BytecodeTargetEdge { target, arguments };
        let arg = BytecodeSwitchArgument::Value;
        let payload = BytecodeSwitchArgument::CasePayload;
        let case = |key, target, arguments| BytecodeSwitchCase {
            case_key: CaseKey::Builtin(key),
            edge: BytecodeSwitchEdge { target, arguments },
        };
        let bytecode = BytecodeFunction {
            function: EntityId::from_bytes([9; 32]),
            parameter_registers: vec![0, 1],
            register_types: types,
            result_type: s64.clone(),
            entry_block: 0,
            blocks: vec![
                block(
                    0,
                    vec![],
                    vec![
                        i(Opcode::VectorLen, vec![0], 2),
                        constant(zero_u, 3),
                        i(Opcode::NotEqual, vec![2, 3], 4),
                        constant(zero_i, 5),
                    ],
                    BytecodeTerminator::CondBranch {
                        condition: 4,
                        if_true: edge(1, vec![3, 5]),
                        if_false: edge(4, vec![5]),
                    },
                ),
                block(
                    1,
                    vec![6, 7],
                    vec![i(Opcode::VectorGet, vec![0, 6], 8)],
                    BytecodeTerminator::VariantSwitch {
                        value: 8,
                        cases: vec![
                            case(BuiltinCase::None, 4, vec![arg(7)]),
                            case(BuiltinCase::Some, 2, vec![payload, arg(7), arg(6)]),
                        ],
                    },
                ),
                block(
                    2,
                    vec![9, 10, 11],
                    vec![i(Opcode::IntAddChecked, vec![10, 9], 12)],
                    BytecodeTerminator::VariantSwitch {
                        value: 12,
                        cases: vec![
                            case(BuiltinCase::Ok, 3, vec![payload, arg(11)]),
                            case(BuiltinCase::Err, 5, vec![payload]),
                        ],
                    },
                ),
                block(
                    3,
                    vec![13, 14],
                    vec![i(Opcode::IntAddChecked, vec![14, 1], 15)],
                    BytecodeTerminator::VariantSwitch {
                        value: 15,
                        cases: vec![
                            case(BuiltinCase::Err, 5, vec![payload]),
                            case(BuiltinCase::Ok, 1, vec![payload, arg(13)]),
                        ],
                    },
                ),
                block(4, vec![16], vec![], BytecodeTerminator::Return(16)),
                block(
                    5,
                    vec![17],
                    vec![],
                    BytecodeTerminator::Trap {
                        code: 3,
                        payload: Some(17),
                    },
                ),
            ],
        };
        let constants = [
            ConstantDefinition {
                entity_id: zero_u,
                value: uint(0),
            },
            ConstantDefinition {
                entity_id: zero_i,
                value: sint(0),
            },
        ];
        let words = Plan::compile(&bytecode).unwrap().words.unwrap();
        let fast = &words.functions[0].fast;
        assert!(
            fast.iter().any(|f| f
                .steps
                .iter()
                .any(|s| matches!(s.op, Op::VecGet(..)) && matches!(s.guard, Guarded::Ok(_)))),
            "a fast block reads the vector behind a guard: {fast:?}"
        );
        let xs = |items: &[i128]| ConstValue {
            value_type: vector.clone(),
            data: ConstData::Sequence(items.iter().map(|&v| sint(v)).collect()),
        };
        let max = i128::from(i64::MAX);
        let min = i128::from(i64::MIN);
        let lists: [&[i128]; 5] = [&[], &[7], &[1, 2, 3, 4, 5], &[-5, 7, 0, 9, min], &[max, 1]];
        let mut rows = Vec::new();
        for list in lists {
            for k in [1, 2, 3, u128::from(u64::MAX)] {
                rows.push(vec![xs(list), uint(k)]);
            }
        }
        sweep(bytecode, &constants, &rows);
    }

    /// Doubly recursive `fib` shaped like the Python dialect's: constants,
    /// a conditional return, checked steps, and two calls in a row, so
    /// bursts run calls, returns and the fast blocks after each call.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn calls_in_bursts_match_reference_on_every_budget_cut() {
        let ty = TypeExpr::SInt(IntegerWidth::from_bits(64));
        let failure = TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic);
        let checked = TypeExpr::Result {
            ok: Box::new(ty.clone()),
            error: Box::new(failure.clone()),
        };
        let mut types = vec![ty.clone(); 14];
        for r in [4, 8, 11] {
            types[r] = checked.clone();
        }
        types[2] = TypeExpr::Bool;
        types[13] = failure;
        let i = |op, a: Vec<u32>, out| instruction(op, a, out, Immediate::None);
        let function = EntityId::from_bytes([9; 32]);
        let one = EntityId::from_bytes([1; 32]);
        let two = EntityId::from_bytes([2; 32]);
        let constant =
            |id, out| instruction(Opcode::ConstantRef, vec![], out, Immediate::Entity(id));
        let call = |argument, out| {
            instruction(
                Opcode::CallDirect,
                vec![argument],
                out,
                Immediate::Function(FunctionRefValue {
                    function,
                    type_arguments: vec![],
                }),
            )
        };
        let edge = |target| BytecodeTargetEdge {
            target,
            arguments: vec![],
        };
        // fib(n) = n if n < 2 else fib(n - 1) + fib(n - 2)  (checked)
        let bytecode = BytecodeFunction {
            function,
            parameter_registers: vec![0],
            register_types: types,
            result_type: ty.clone(),
            entry_block: 0,
            blocks: vec![
                block(
                    0,
                    vec![],
                    vec![constant(two, 1), i(Opcode::LessThan, vec![0, 1], 2)],
                    BytecodeTerminator::CondBranch {
                        condition: 2,
                        if_true: edge(1),
                        if_false: edge(2),
                    },
                ),
                block(1, vec![], vec![], BytecodeTerminator::Return(0)),
                block(2, vec![], vec![], BytecodeTerminator::Branch(edge(3))),
                block(
                    3,
                    vec![],
                    vec![constant(one, 3), i(Opcode::IntSubChecked, vec![0, 3], 4)],
                    switch(4, 5, 4, false),
                ),
                block(
                    4,
                    vec![13],
                    vec![],
                    BytecodeTerminator::Trap {
                        code: 3,
                        payload: Some(13),
                    },
                ),
                block(
                    5,
                    vec![5],
                    vec![
                        call(5, 6),
                        constant(two, 7),
                        i(Opcode::IntSubChecked, vec![0, 7], 8),
                    ],
                    switch(8, 6, 4, true),
                ),
                block(
                    6,
                    vec![9],
                    vec![call(9, 10), i(Opcode::IntAddChecked, vec![6, 10], 11)],
                    switch(11, 7, 4, false),
                ),
                block(7, vec![12], vec![], BytecodeTerminator::Return(12)),
            ],
        };
        let value = |n| ConstValue {
            value_type: ty.clone(),
            data: ConstData::SInt(n),
        };
        let constants = [
            ConstantDefinition {
                entity_id: one,
                value: value(1),
            },
            ConstantDefinition {
                entity_id: two,
                value: value(2),
            },
        ];
        let words = Plan::compile(&bytecode).unwrap().words.unwrap();
        let f = &words.functions[0];
        assert_eq!(f.sites.len(), 2, "two call sites: {f:?}");
        assert!(
            f.fast.iter().filter(|b| b.call.is_some()).count() >= 2
                && f.sites.iter().all(|s| f.fast[s.tail as usize].start.1 > 0),
            "fast blocks end at calls and resume after them: {f:?}"
        );
        assert!(!f.constant_slots.is_empty(), "constants are prefilled");
        let rows: Vec<Vec<ConstValue>> = [0, 1, 2, 3, 7, 12, -4, i128::from(i64::MIN)]
            .into_iter()
            .map(|n| vec![sint(n)])
            .collect();
        sweep(bytecode, &constants, &rows);
    }

    /// A call chain through the call depth ceiling and past it whose bursts
    /// charge close to their whole prepaid worst case: on the way down every
    /// fast block ends at a two-argument call, on the way up at a return.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn call_chains_charge_the_burst_worst_case() {
        let ty = TypeExpr::SInt(IntegerWidth::from_bits(64));
        let failure = TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic);
        let checked = TypeExpr::Result {
            ok: Box::new(ty.clone()),
            error: Box::new(failure.clone()),
        };
        let mut types = vec![ty.clone(); 18];
        types[2] = TypeExpr::Bool;
        for r in [4, 9, 11, 13, 15] {
            types[r] = checked.clone();
        }
        types[7] = failure;
        let function = EntityId::from_bytes([9; 32]);
        let zero = EntityId::from_bytes([1; 32]);
        let one = EntityId::from_bytes([2; 32]);
        let i = |op, a: Vec<u32>, out| instruction(op, a, out, Immediate::None);
        let constant =
            |id, out| instruction(Opcode::ConstantRef, vec![], out, Immediate::Entity(id));
        let edge = |target| BytecodeTargetEdge {
            target,
            arguments: vec![],
        };
        // down(n, k) = k if n == 0 else down(n - 1, k) + 0 + 0 + 0 + 0
        // Two arguments, and four checked steps after the call: the way
        // down charges the costliest call, the way up the costliest return.
        let bytecode = BytecodeFunction {
            function,
            parameter_registers: vec![0, 8],
            register_types: types,
            result_type: ty.clone(),
            entry_block: 0,
            blocks: vec![
                block(
                    0,
                    vec![],
                    vec![constant(zero, 1), i(Opcode::Equal, vec![0, 1], 2)],
                    BytecodeTerminator::CondBranch {
                        condition: 2,
                        if_true: edge(1),
                        if_false: edge(2),
                    },
                ),
                block(1, vec![], vec![], BytecodeTerminator::Return(8)),
                block(
                    2,
                    vec![],
                    vec![constant(one, 3), i(Opcode::IntSubChecked, vec![0, 3], 4)],
                    switch(4, 3, 4, false),
                ),
                block(
                    3,
                    vec![5],
                    vec![
                        instruction(
                            Opcode::CallDirect,
                            vec![5, 8],
                            6,
                            Immediate::Function(FunctionRefValue {
                                function,
                                type_arguments: vec![],
                            }),
                        ),
                        i(Opcode::IntAddChecked, vec![6, 1], 9),
                    ],
                    switch(9, 5, 4, true),
                ),
                block(
                    4,
                    vec![7],
                    vec![],
                    BytecodeTerminator::Trap {
                        code: 3,
                        payload: Some(7),
                    },
                ),
                block(
                    5,
                    vec![10],
                    vec![i(Opcode::IntAddChecked, vec![10, 1], 11)],
                    switch(11, 6, 4, false),
                ),
                block(
                    6,
                    vec![12],
                    vec![i(Opcode::IntAddChecked, vec![12, 1], 13)],
                    switch(13, 7, 4, true),
                ),
                block(
                    7,
                    vec![14],
                    vec![i(Opcode::IntAddChecked, vec![14, 1], 15)],
                    switch(15, 8, 4, false),
                ),
                block(8, vec![16], vec![], BytecodeTerminator::Return(16)),
            ],
        };
        let value = |n| ConstValue {
            value_type: ty.clone(),
            data: ConstData::SInt(n),
        };
        let constants = [
            ConstantDefinition {
                entity_id: zero,
                value: value(0),
            },
            ConstantDefinition {
                entity_id: one,
                value: value(1),
            },
        ];
        let rows: Vec<Vec<ConstValue>> = [0, 1, 70, 200, 254, 255, 256, 400]
            .into_iter()
            .map(|n| vec![sint(n), sint(n * 3 - 5)])
            .collect();
        sweep(bytecode, &constants, &rows);
    }

    /// `down(n, k) = k if n == 0 else down(n - 1, k)`: on the way down every
    /// fast block is the costliest one and ends at a two-argument call, so
    /// bursts charge their whole prepaid fuel and more than its returns'
    /// share of units; through the call depth ceiling and past it.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn call_descents_charge_the_burst_worst_case() {
        let ty = TypeExpr::SInt(IntegerWidth::from_bits(64));
        let failure = TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic);
        let checked = TypeExpr::Result {
            ok: Box::new(ty.clone()),
            error: Box::new(failure.clone()),
        };
        let mut types = vec![ty.clone(); 9];
        types[2] = TypeExpr::Bool;
        types[4] = checked;
        types[7] = failure;
        let function = EntityId::from_bytes([9; 32]);
        let zero = EntityId::from_bytes([1; 32]);
        let one = EntityId::from_bytes([2; 32]);
        let i = |op, a: Vec<u32>, out| instruction(op, a, out, Immediate::None);
        let constant =
            |id, out| instruction(Opcode::ConstantRef, vec![], out, Immediate::Entity(id));
        let edge = |target| BytecodeTargetEdge {
            target,
            arguments: vec![],
        };
        let bytecode = BytecodeFunction {
            function,
            parameter_registers: vec![0, 8],
            register_types: types,
            result_type: ty.clone(),
            entry_block: 0,
            blocks: vec![
                block(
                    0,
                    vec![],
                    vec![constant(zero, 1), i(Opcode::Equal, vec![0, 1], 2)],
                    BytecodeTerminator::CondBranch {
                        condition: 2,
                        if_true: edge(1),
                        if_false: edge(2),
                    },
                ),
                block(1, vec![], vec![], BytecodeTerminator::Return(8)),
                block(
                    2,
                    vec![],
                    vec![constant(one, 3), i(Opcode::IntSubChecked, vec![0, 3], 4)],
                    switch(4, 3, 4, false),
                ),
                block(
                    3,
                    vec![5],
                    vec![instruction(
                        Opcode::CallDirect,
                        vec![5, 8],
                        6,
                        Immediate::Function(FunctionRefValue {
                            function,
                            type_arguments: vec![],
                        }),
                    )],
                    BytecodeTerminator::Return(6),
                ),
                block(
                    4,
                    vec![7],
                    vec![],
                    BytecodeTerminator::Trap {
                        code: 3,
                        payload: Some(7),
                    },
                ),
            ],
        };
        let value = |n| ConstValue {
            value_type: ty.clone(),
            data: ConstData::SInt(n),
        };
        let constants = [
            ConstantDefinition {
                entity_id: zero,
                value: value(0),
            },
            ConstantDefinition {
                entity_id: one,
                value: value(1),
            },
        ];
        let rows: Vec<Vec<ConstValue>> = [0, 1, 70, 200, 254, 255, 256, 400]
            .into_iter()
            .map(|n| vec![sint(n), sint(7 - n)])
            .collect();
        sweep(bytecode, &constants, &rows);
    }

    /// Each word operation answers what the general evaluator answers, for
    /// every pair of in-range edge operands: the value or failure code, the
    /// Ok/Err shape, and the units charged.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn every_word_operation_matches_the_general_evaluator() {
        use super::super::{S128, Step, U128, Word, evaluate};
        let binary = [
            Opcode::IntAddChecked,
            Opcode::IntSubChecked,
            Opcode::IntMulChecked,
            Opcode::IntDivChecked,
            Opcode::IntRemChecked,
            Opcode::Equal,
            Opcode::NotEqual,
            Opcode::LessThan,
            Opcode::LessEqual,
            Opcode::GreaterThan,
            Opcode::GreaterEqual,
        ];
        let min = i64::MIN;
        let max = i64::MAX;
        let signed = [
            min,
            min + 1,
            -3_037_000_500,
            -7,
            -2,
            -1,
            0,
            1,
            2,
            7,
            3_037_000_500,
            max - 1,
            max,
        ];
        let unsigned = [0, 1, 2, 7, 4_294_967_296, u64::MAX - 1, u64::MAX];
        let step = |op: Opcode, is_signed: bool, args: Vec<usize>, word: bool| Step {
            opcode: op,
            output: 2,
            args,
            constant: None,
            units: [3, 5],
            dynamic: false,
            wrapper_units: 0,
            field: None,
            width: Some((is_signed, 64)),
            call: None,
            word: word.then_some(Word {
                op,
                signed: is_signed,
                a: 0,
                b: 1,
            }),
        };
        let types = |is_signed: bool| {
            let ty = if is_signed {
                TypeExpr::SInt(IntegerWidth::from_bits(64))
            } else {
                TypeExpr::UInt(IntegerWidth::from_bits(64))
            };
            vec![ty.clone(), ty, TypeExpr::Bool]
        };
        let check = |general: &Step,
                     x: Atom<'static>,
                     y: Option<Atom<'static>>,
                     wx: u64,
                     wy: u64,
                     ty: &[TypeExpr]| {
            let plain = |atom| {
                Some(Value {
                    atom,
                    shape: Shape::Plain,
                })
            };
            let registers = [plain(x), y.and_then(plain), None];
            let expected = evaluate(general, &registers).unwrap();
            let operands = if general.args.len() == 2 {
                vec![0, 1]
            } else {
                vec![0]
            };
            let instruction = instruction(general.opcode, operands, 2, Immediate::None);
            let word = compile_step(general, &instruction, ty).unwrap();
            let mut words = [wx, wy, 0];
            let mut flags = [false, false, true];
            let units = exec(&word, &[], &[], &mut words, &mut flags);
            let (shape, atom) = (expected.shape, expected.atom);
            let failed = shape == Shape::Err;
            let answer = match atom {
                Atom::Bool(v) => u64::from(v),
                Atom::Signed(v) => i64::try_from(v.0).unwrap().cast_unsigned(),
                Atom::Unsigned(v) => u64::try_from(v.0).unwrap(),
                Atom::Failure(code) => u64::from(code),
                other => panic!("unexpected {other:?}"),
            };
            assert_eq!(words[2], answer, "{:?} {x:?} {y:?}", general.opcode);
            if matches!(shape, Shape::Ok | Shape::Err) {
                assert_eq!(flags[2], failed, "{:?} {x:?} {y:?}", general.opcode);
            }
            assert_eq!(
                units,
                general.units[usize::from(failed)],
                "{:?} {x:?} {y:?}",
                general.opcode
            );
        };
        for op in binary {
            for x in signed {
                for y in signed {
                    let atom = |v: i64| Atom::Signed(S128(i128::from(v)));
                    check(
                        &step(op, true, vec![0, 1], true),
                        atom(x),
                        Some(atom(y)),
                        x.cast_unsigned(),
                        y.cast_unsigned(),
                        &types(true),
                    );
                }
            }
            for x in unsigned {
                for y in unsigned {
                    let atom = |v: u64| Atom::Unsigned(U128(u128::from(v)));
                    check(
                        &step(op, false, vec![0, 1], true),
                        atom(x),
                        Some(atom(y)),
                        x,
                        y,
                        &types(false),
                    );
                }
            }
        }
        for x in signed {
            let atom = Atom::Signed(S128(i128::from(x)));
            check(
                &step(Opcode::IntNegChecked, true, vec![0], false),
                atom,
                None,
                x.cast_unsigned(),
                0,
                &types(true),
            );
        }
        let bools = vec![TypeExpr::Bool, TypeExpr::Bool, TypeExpr::Bool];
        for x in [false, true] {
            let mut not = step(Opcode::BoolNot, false, vec![0], false);
            not.width = None;
            check(&not, Atom::Bool(x), None, u64::from(x), 0, &bools);
            for y in [false, true] {
                for op in [
                    Opcode::BoolAnd,
                    Opcode::BoolOr,
                    Opcode::Equal,
                    Opcode::NotEqual,
                ] {
                    let mut general = step(op, false, vec![0, 1], false);
                    general.width = None;
                    check(
                        &general,
                        Atom::Bool(x),
                        Some(Atom::Bool(y)),
                        u64::from(x),
                        u64::from(y),
                        &bools,
                    );
                }
            }
        }
    }
}

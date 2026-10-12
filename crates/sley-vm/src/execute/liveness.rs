//! Register liveness for ownership transfer during execution.
//!
//! This is an execution-mechanics optimization, not a semantic judgment: it
//! decides only whether a register's value may be moved out of the register
//! file (because no later instruction, terminator, or successor reads it)
//! instead of cloned. Every charge, count, result, and termination is the
//! same whichever way a value travels; only host copying changes.
//!
//! Derived bytecode is in SSA form: each register has exactly one static
//! definition (a function parameter, a block parameter, or one instruction
//! result). A register is live into a block when that block reads it
//! without defining it, and live out of a block when some successor has it
//! live in. Liveness is computed per register by walking predecessors back
//! from each use to the defining block.

use std::collections::HashSet;

use crate::{BytecodeBlock, BytecodeSwitchArgument, BytecodeTerminator, Register};

/// Per-function liveness facts.
#[derive(Debug, Default)]
pub(crate) struct FnLiveness {
    /// Per block, per instruction: bit `k` set when operand `k` is dead
    /// after the instruction (its last read on every path), and the
    /// register appears exactly once among that instruction's operands.
    pub(crate) dead_operands: Vec<Vec<u64>>,
    /// Per block: registers live out of the block.
    live_out: Vec<HashSet<Register>>,
}

impl FnLiveness {
    /// Whether `register` is dead once `block`'s terminator has run.
    pub(crate) fn dead_after_terminator(&self, block: usize, register: Register) -> bool {
        self.live_out
            .get(block)
            .is_some_and(|live| !live.contains(&register))
    }

    /// The dead-operand mask of one instruction.
    pub(crate) fn mask(&self, block: usize, instruction: usize) -> u64 {
        self.dead_operands
            .get(block)
            .and_then(|masks| masks.get(instruction))
            .copied()
            .unwrap_or(0)
    }
}

fn successors(terminator: &BytecodeTerminator) -> Vec<usize> {
    let slot = |target: u32| usize::try_from(target).unwrap_or(usize::MAX);
    match terminator {
        BytecodeTerminator::Return(_) | BytecodeTerminator::Trap { .. } => Vec::new(),
        BytecodeTerminator::Branch(edge) => vec![slot(edge.target)],
        BytecodeTerminator::CondBranch {
            if_true, if_false, ..
        } => vec![slot(if_true.target), slot(if_false.target)],
        BytecodeTerminator::VariantSwitch { cases, .. } => {
            cases.iter().map(|case| slot(case.edge.target)).collect()
        }
    }
}

fn terminator_reads(terminator: &BytecodeTerminator) -> Vec<Register> {
    match terminator {
        BytecodeTerminator::Return(register) => vec![*register],
        BytecodeTerminator::Branch(edge) => edge.arguments.clone(),
        BytecodeTerminator::CondBranch {
            condition,
            if_true,
            if_false,
        } => {
            let mut out = vec![*condition];
            out.extend(&if_true.arguments);
            out.extend(&if_false.arguments);
            out
        }
        BytecodeTerminator::VariantSwitch { value, cases } => {
            let mut out = vec![*value];
            for case in cases {
                for argument in &case.edge.arguments {
                    if let BytecodeSwitchArgument::Value(register) = argument {
                        out.push(*register);
                    }
                }
            }
            out
        }
        BytecodeTerminator::Trap { payload, .. } => payload.iter().copied().collect(),
    }
}

/// Computes liveness for one function's blocks. Malformed shapes (targets
/// out of range) produce conservative facts: nothing is reported dead.
pub(crate) fn compute(blocks: &[BytecodeBlock]) -> FnLiveness {
    let count = blocks.len();
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (index, block) in blocks.iter().enumerate() {
        for successor in successors(&block.terminator) {
            if successor >= count {
                return FnLiveness::default();
            }
            if !predecessors[successor].contains(&index) {
                predecessors[successor].push(index);
            }
        }
    }
    // Defining block of every register defined inside the function body.
    let mut defined_in: std::collections::HashMap<Register, usize> =
        std::collections::HashMap::new();
    for (index, block) in blocks.iter().enumerate() {
        for register in &block.parameter_registers {
            defined_in.insert(*register, index);
        }
        for instruction in &block.instructions {
            for register in &instruction.results {
                defined_in.insert(*register, index);
            }
        }
    }
    let mut live_in: Vec<HashSet<Register>> = vec![HashSet::new(); count];
    let mut live_out: Vec<HashSet<Register>> = vec![HashSet::new(); count];
    let mut work: Vec<usize> = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let mut local: HashSet<Register> = block.parameter_registers.iter().copied().collect();
        let mut upward: Vec<Register> = Vec::new();
        for instruction in &block.instructions {
            for register in &instruction.operands {
                if !local.contains(register) {
                    upward.push(*register);
                }
            }
            local.extend(instruction.results.iter().copied());
        }
        for register in terminator_reads(&block.terminator) {
            if !local.contains(&register) {
                upward.push(register);
            }
        }
        for register in upward {
            if !live_in[index].insert(register) {
                continue;
            }
            work.clear();
            work.push(index);
            while let Some(at) = work.pop() {
                for &predecessor in &predecessors[at] {
                    live_out[predecessor].insert(register);
                    if defined_in.get(&register) == Some(&predecessor) {
                        continue;
                    }
                    if live_in[predecessor].insert(register) {
                        work.push(predecessor);
                    }
                }
            }
        }
    }
    let mut dead_operands = Vec::with_capacity(count);
    for (index, block) in blocks.iter().enumerate() {
        let terminator = terminator_reads(&block.terminator);
        let mut masks = vec![0_u64; block.instructions.len()];
        // Registers read at or after a point, scanning backward.
        let mut later: HashSet<Register> = terminator.into_iter().collect();
        for (position, instruction) in block.instructions.iter().enumerate().rev() {
            if instruction.operands.len() <= 64 {
                let mut mask = 0_u64;
                for (k, register) in instruction.operands.iter().enumerate() {
                    let unique = instruction
                        .operands
                        .iter()
                        .filter(|other| *other == register)
                        .count()
                        == 1;
                    if unique && !later.contains(register) && !live_out[index].contains(register)
                    {
                        mask |= 1 << k;
                    }
                }
                masks[position] = mask;
            }
            later.extend(instruction.operands.iter().copied());
        }
        dead_operands.push(masks);
    }
    FnLiveness {
        dead_operands,
        live_out,
    }
}

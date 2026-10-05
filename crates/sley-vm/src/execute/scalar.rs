//! Compact scalar and borrowed-collection plans for structurally verified images.
//!
//! No new arithmetic or accounting rules: checked integer operations use the
//! extended kernel, and all action/value charges use the reference runner's
//! functions. Unsupported graphs use the original interpreter unchanged.

use super::{
    BuiltinCase, BytecodeSwitchArgument, BytecodeTargetEdge, BytecodeTerminator, CaseKey,
    ConstData, ConstValue, CoreOutcome, EntityId, ExecutionLimits, ExecutionRequest,
    ExecutionSource, ExecutionTermination, LoweredFunction, MAX_CALL_DEPTH, Register, ResourceKind,
    ResultConst, Runtime, RuntimeFault, RuntimeResult, TypeExpr, ValidatedInputs, charge_action,
    charge_value, initial_value_units, value_units_const,
};
use sley_ssmc::{BuiltinFailureKind, BuiltinFailureValue, Immediate, Opcode};

#[derive(Clone, Copy, Debug)]
enum Atom<'a> {
    Borrowed(&'a ConstValue),
    Unit,
    Bool(bool),
    Signed(i128),
    Unsigned(u128),
    Failure(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    Plain,
    Ok,
    Err,
    Some,
    None,
}

#[derive(Clone, Copy, Debug)]
struct Value<'a> {
    atom: Atom<'a>,
    shape: Shape,
}

fn leaf(ty: &TypeExpr) -> bool {
    matches!(ty, TypeExpr::Unit | TypeExpr::Bool | TypeExpr::Text)
        || integer(ty).is_some()
        || *ty == TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)
}

fn integer(ty: &TypeExpr) -> Option<(bool, u16)> {
    match ty {
        TypeExpr::SInt(w) if w.is_epoch_1() => Some((true, w.bits())),
        TypeExpr::UInt(w) if w.is_epoch_1() => Some((false, w.bits())),
        _ => None,
    }
}

fn supported(ty: &TypeExpr) -> bool {
    leaf(ty)
        || matches!(ty, TypeExpr::Named(n) if n.arguments.is_empty())
        || matches!(ty, TypeExpr::Vector(inner) | TypeExpr::Option(inner) if plain(inner))
        || matches!(ty, TypeExpr::Result { ok, error }
            if integer(ok).is_some() && **error == TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic))
}

fn plain(ty: &TypeExpr) -> bool {
    leaf(ty)
        || matches!(ty, TypeExpr::Named(n) if n.arguments.is_empty())
        || matches!(ty, TypeExpr::Vector(inner) if plain(inner))
}

fn fixed_units(ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Text | TypeExpr::Vector(_) | TypeExpr::Named(_) => false,
        TypeExpr::Option(inner) => fixed_units(inner),
        _ => true,
    }
}

impl<'a> Value<'a> {
    fn from_const(value: &'a ConstValue) -> Option<Self> {
        let (shape, value) = match (&value.value_type, &value.data) {
            (TypeExpr::Result { ok, .. }, ConstData::Result(ResultConst::Ok(v)))
                if **ok == v.value_type =>
            {
                (Shape::Ok, v.as_ref())
            }
            (TypeExpr::Result { error, .. }, ConstData::Result(ResultConst::Err(v)))
                if **error == v.value_type =>
            {
                (Shape::Err, v.as_ref())
            }
            (TypeExpr::Option(inner), ConstData::Option(Some(v))) if **inner == v.value_type => {
                (Shape::Some, v.as_ref())
            }
            (TypeExpr::Option(_), ConstData::Option(None)) => {
                return Some(Self {
                    atom: Atom::Unit,
                    shape: Shape::None,
                });
            }
            (ty, _) if plain(ty) => (Shape::Plain, value),
            _ => return None,
        };
        let atom = match (&value.value_type, &value.data) {
            (TypeExpr::Text, ConstData::Text(_))
            | (TypeExpr::Vector(_), ConstData::Sequence(_)) => Atom::Borrowed(value),
            (TypeExpr::Named(n), ConstData::Record(r)) if n.definition == r.definition => {
                Atom::Borrowed(value)
            }
            (TypeExpr::Unit, ConstData::Unit) => Atom::Unit,
            (TypeExpr::Bool, ConstData::Bool(v)) => Atom::Bool(*v),
            (TypeExpr::SInt(_), ConstData::SInt(v)) => Atom::Signed(*v),
            (TypeExpr::UInt(_), ConstData::UInt(v)) => Atom::Unsigned(*v),
            (
                TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic),
                ConstData::BuiltinFailure(v),
            ) if v.kind == BuiltinFailureKind::Arithmetic => Atom::Failure(v.code),
            _ => return None,
        };
        Some(Self { atom, shape })
    }

    fn to_const(self, ty: &TypeExpr) -> ConstValue {
        let data = match self.atom {
            Atom::Borrowed(v) => v.data.clone(),
            Atom::Unit => ConstData::Unit,
            Atom::Bool(v) => ConstData::Bool(v),
            Atom::Signed(v) => ConstData::SInt(v),
            Atom::Unsigned(v) => ConstData::UInt(v),
            Atom::Failure(code) => ConstData::BuiltinFailure(BuiltinFailureValue {
                kind: BuiltinFailureKind::Arithmetic,
                code,
            }),
        };
        let data = match (self.shape, ty) {
            (Shape::None, TypeExpr::Option(_)) => ConstData::Option(None),
            (Shape::Some, TypeExpr::Option(inner)) => {
                ConstData::Option(Some(Box::new(ConstValue {
                    value_type: (**inner).clone(),
                    data,
                })))
            }
            (Shape::Ok, TypeExpr::Result { ok, .. }) => {
                ConstData::Result(ResultConst::Ok(Box::new(ConstValue {
                    value_type: (**ok).clone(),
                    data,
                })))
            }
            (Shape::Err, TypeExpr::Result { error, .. }) => {
                ConstData::Result(ResultConst::Err(Box::new(ConstValue {
                    value_type: (**error).clone(),
                    data,
                })))
            }
            _ => data,
        };
        ConstValue {
            value_type: ty.clone(),
            data,
        }
    }
}

#[derive(Clone, Debug)]
struct Step {
    opcode: Opcode,
    output: usize,
    args: Vec<usize>,
    constant: Option<usize>,
    units: [u64; 2],
    dynamic: bool,
    wrapper_units: u64,
    field: Option<sley_ssmc::MemberId>,
}

impl Step {
    fn value_units(&self, value: Value<'_>) -> u64 {
        match value.atom {
            Atom::Borrowed(v) if self.dynamic => {
                value_units_const(v).saturating_add(self.wrapper_units)
            }
            _ => self.units[usize::from(matches!(value.shape, Shape::Err | Shape::None))],
        }
    }
}

#[derive(Clone, Debug)]
struct Argument {
    register: Option<usize>,
    take: bool,
}

#[derive(Clone, Debug)]
struct Edge {
    target: usize,
    args: Vec<Argument>,
}

#[derive(Clone, Debug)]
enum Exit {
    Return(usize),
    Branch(Edge),
    Cond(usize, Edge, Edge),
    Switch {
        register: usize,
        take: bool,
        cases: Vec<(CaseKey, Edge)>,
    },
    Trap(u32, Option<usize>),
}

#[derive(Clone, Debug)]
struct Block {
    parameters: Vec<usize>,
    steps: Vec<Step>,
    exit: Exit,
}

#[derive(Clone, Debug)]
struct ConstantBinding {
    entity: EntityId,
    register: usize,
}

#[derive(Clone, Debug)]
struct FieldBinding {
    definition: EntityId,
    member: sley_ssmc::MemberId,
    register: usize,
}

#[derive(Clone, Debug)]
pub(super) struct Plan {
    blocks: Vec<Block>,
    scratch: usize,
    records: Vec<EntityId>,
    constants: Vec<ConstantBinding>,
    fields: Vec<FieldBinding>,
}

impl Plan {
    pub(super) fn compile(code: &crate::BytecodeFunction) -> Option<Self> {
        if !code.register_types.iter().all(supported) || !supported(&code.result_type) {
            return None;
        }
        let reg = |r: Register| {
            usize::try_from(r)
                .ok()
                .filter(|&r| r < code.register_types.len())
        };
        let mut blocks = Vec::new();
        let mut constants = Vec::new();
        let mut fields = Vec::new();
        let mut scratch = 0;
        for source in &code.blocks {
            let steps = source
                .instructions
                .iter()
                .map(|instruction| compile_step(code, instruction, &mut constants, &mut fields))
                .collect::<Option<Vec<_>>>()?;
            let exit = compile_exit(code, source)?;
            let edge_size = |edge: &Edge| edge.args.len();
            scratch = scratch.max(match &exit {
                Exit::Return(_) | Exit::Trap(..) => 0,
                Exit::Branch(edge) => edge_size(edge),
                Exit::Cond(_, yes, no) => edge_size(yes).max(edge_size(no)),
                Exit::Switch { cases, .. } => {
                    cases.iter().map(|(_, e)| edge_size(e)).max().unwrap_or(0)
                }
            });
            blocks.push(Block {
                parameters: source
                    .parameter_registers
                    .iter()
                    .map(|r| reg(*r))
                    .collect::<Option<_>>()?,
                steps,
                exit,
            });
        }
        let mut pending: Vec<_> = code.register_types.iter().collect();
        let mut records = Vec::new();
        while let Some(ty) = pending.pop() {
            match ty {
                TypeExpr::Named(n) => records.push(n.definition),
                TypeExpr::Vector(inner) | TypeExpr::Option(inner) => pending.push(inner),
                _ => {}
            }
        }
        records.sort();
        records.dedup();
        Some(Self {
            blocks,
            scratch,
            records,
            constants,
            fields,
        })
    }

    pub(super) fn execute(
        &self,
        source: &ExecutionSource<'_>,
        lowered: &LoweredFunction,
        inputs: &ValidatedInputs,
        request: &ExecutionRequest,
    ) -> Option<CoreOutcome> {
        let initial = initial_value_units(
            &lowered.bytecode.register_types,
            &lowered.bytes,
            inputs.value_units,
        );
        let mut runtime = Runtime {
            registers: Vec::new(),
            block: usize::try_from(lowered.bytecode.entry_block).ok()?,
            instruction_count: 0,
            fuel_used: 0,
            live_value_units: initial,
            peak_value_units: initial,
            cells: Vec::new(),
            max_call_depth: MAX_CALL_DEPTH,
            peak_call_depth: 1,
        };
        let termination = if initial > request.limits.max_value_units {
            ExecutionTermination::ResourceLimit(ResourceKind::ValueUnits)
        } else {
            for id in &self.records {
                let definition = source.types.definition(*id).ok()?;
                if !definition.type_parameters.is_empty()
                    || !matches!(definition.form, sley_ssmc::TypeDefForm::Record(_))
                {
                    return None;
                }
            }
            for binding in &self.fields {
                let sley_ssmc::TypeDefForm::Record(fields) =
                    &source.types.definition(binding.definition).ok()?.form
                else {
                    return None;
                };
                let field = fields.iter().find(|f| f.member_id == binding.member)?;
                if field.value_type != lowered.bytecode.register_types[binding.register] {
                    return None;
                }
            }
            // Bind the current inventory on every call. The compiled list has
            // one slot per constant reference, independent of block/step count.
            // A changed or unsupported inventory still takes the reference path.
            // Large projected inventories are normally in strict entity order.
            // Check that condition on the current slice before binary lookup;
            // unsorted/duplicate inventories retain the reference's first-match
            // behavior. This allocates no index and caches no mutable binding.
            let ordered = self.constants.len() >= 16
                && source.constants.len() >= 16
                && source
                    .constants
                    .windows(2)
                    .all(|pair| pair[0].entity_id < pair[1].entity_id);
            let constants = self
                .constants
                .iter()
                .map(|binding| {
                    let constant = if ordered {
                        let index = source
                            .constants
                            .binary_search_by_key(&binding.entity, |v| v.entity_id)
                            .ok()?;
                        &source.constants[index]
                    } else {
                        source
                            .constants
                            .iter()
                            .find(|v| v.entity_id == binding.entity)?
                    };
                    let value = &constant.value;
                    if value.value_type != lowered.bytecode.register_types[binding.register] {
                        return None;
                    }
                    if !fixed_units(&value.value_type) {
                        source.types.check_constant(value).ok()?;
                    }
                    Value::from_const(value)
                })
                .collect::<Option<Vec<_>>>()?;
            let values = request
                .inputs
                .iter()
                .map(Value::from_const)
                .collect::<Option<Vec<_>>>()?;
            self.run(
                &mut runtime,
                &lowered.bytecode,
                &request.limits,
                values,
                &constants,
            )
            .unwrap_or(ExecutionTermination::InternalInvariant)
        };
        Some(CoreOutcome {
            termination,
            instruction_count: runtime.instruction_count,
            fuel_used: runtime.fuel_used,
            peak_value_units: runtime.peak_value_units,
            peak_call_depth: 1,
        })
    }

    fn run<'v>(
        &self,
        runtime: &mut Runtime,
        code: &crate::BytecodeFunction,
        limits: &ExecutionLimits,
        inputs: Vec<Value<'v>>,
        constants: &[Value<'v>],
    ) -> RuntimeResult<ExecutionTermination> {
        let mut registers = vec![None; code.register_types.len()];
        let mut scratch = Vec::with_capacity(self.scratch);
        for (r, value) in code.parameter_registers.iter().zip(inputs) {
            *registers
                .get_mut(usize::try_from(*r).map_err(|_| RuntimeFault)?)
                .ok_or(RuntimeFault)? = Some(value);
        }
        loop {
            let block = self.blocks.get(runtime.block).ok_or(RuntimeFault)?;
            for step in &block.steps {
                if let Some(end) = charge_action(runtime, limits, Some(ResourceKind::Instruction)) {
                    return Ok(end);
                }
                let value = match step.constant {
                    Some(index) => constants[index],
                    None => evaluate(step, code, &registers)?,
                };
                if !charge_value(runtime, step.value_units(value), limits.max_value_units) {
                    return Ok(ExecutionTermination::ResourceLimit(
                        ResourceKind::ValueUnits,
                    ));
                }
                runtime.instruction_count = runtime.instruction_count.saturating_add(1);
                registers[step.output] = Some(value);
            }
            if let Some(end) = charge_action(runtime, limits, None) {
                return Ok(end);
            }
            let (edge, payload) = match &block.exit {
                Exit::Return(r) => {
                    let value = get(&registers, *r)?.to_const(&code.result_type);
                    return Ok(if value_units_const(&value) > limits.max_output_units {
                        ExecutionTermination::ResourceLimit(ResourceKind::OutputUnits)
                    } else {
                        ExecutionTermination::Success(value)
                    });
                }
                Exit::Trap(tag, payload) => {
                    let payload = payload
                        .map(|r| get(&registers, r).map(|v| v.to_const(&code.register_types[r])))
                        .transpose()?;
                    return Ok(
                        if payload
                            .as_ref()
                            .is_some_and(|v| value_units_const(v) > limits.max_output_units)
                        {
                            ExecutionTermination::ResourceLimit(ResourceKind::OutputUnits)
                        } else {
                            ExecutionTermination::Trap {
                                trap_tag: *tag,
                                payload,
                            }
                        },
                    );
                }
                Exit::Branch(edge) => (edge, None),
                Exit::Cond(r, yes, no) => {
                    let Atom::Bool(value) = get(&registers, *r)?.atom else {
                        return Err(RuntimeFault);
                    };
                    (if value { yes } else { no }, None)
                }
                Exit::Switch {
                    register,
                    take,
                    cases,
                } => match select_case(runtime, limits, &mut registers, *register, *take, cases)? {
                    Ok(selected) => selected,
                    Err(termination) => return Ok(termination),
                },
            };
            scratch.clear();
            for arg in &edge.args {
                if let Some(end) = charge_action(runtime, limits, None) {
                    return Ok(end);
                }
                scratch.push(match arg.register {
                    Some(r) if arg.take => registers[r].take().ok_or(RuntimeFault)?,
                    Some(r) => get(&registers, r)?,
                    None => {
                        if let Some(end) = charge_action(runtime, limits, None) {
                            return Ok(end);
                        }
                        payload.ok_or(RuntimeFault)?
                    }
                });
            }
            for (&r, &v) in self.blocks[edge.target].parameters.iter().zip(&scratch) {
                registers[r] = Some(v);
            }
            runtime.block = edge.target;
        }
    }
}

fn compile_exit(code: &crate::BytecodeFunction, source: &crate::BytecodeBlock) -> Option<Exit> {
    let reg = |r: Register| {
        usize::try_from(r)
            .ok()
            .filter(|&r| r < code.register_types.len())
    };
    let make_edge = |target, arguments: &[Option<Register>], payload| {
        compile_edge(
            code,
            &source.parameter_registers,
            target,
            arguments,
            payload,
        )
    };
    let branch = |edge: &BytecodeTargetEdge| {
        make_edge(
            edge.target,
            &edge.arguments.iter().copied().map(Some).collect::<Vec<_>>(),
            None,
        )
    };
    let exit = match &source.terminator {
        BytecodeTerminator::Return(r) if code.register_types[reg(*r)?] == code.result_type => {
            Exit::Return(reg(*r)?)
        }
        BytecodeTerminator::Branch(edge) => Exit::Branch(branch(edge)?),
        BytecodeTerminator::CondBranch {
            condition,
            if_true,
            if_false,
        } if code.register_types[reg(*condition)?] == TypeExpr::Bool => {
            Exit::Cond(reg(*condition)?, branch(if_true)?, branch(if_false)?)
        }
        BytecodeTerminator::VariantSwitch { value, cases } => {
            let value_type = &code.register_types[reg(*value)?];
            let mut compiled = Vec::new();
            for case in cases {
                let payload = match (value_type, case.case_key) {
                    (TypeExpr::Result { ok, .. }, CaseKey::Builtin(BuiltinCase::Ok)) => {
                        Some(ok.as_ref())
                    }
                    (TypeExpr::Result { error, .. }, CaseKey::Builtin(BuiltinCase::Err)) => {
                        Some(error.as_ref())
                    }
                    (TypeExpr::Option(inner), CaseKey::Builtin(BuiltinCase::Some)) => {
                        Some(inner.as_ref())
                    }
                    (TypeExpr::Option(_), CaseKey::Builtin(BuiltinCase::None)) => None,
                    _ => return None,
                };
                let args: Vec<_> = case
                    .edge
                    .arguments
                    .iter()
                    .map(|arg| match arg {
                        BytecodeSwitchArgument::Value(r) => Some(*r),
                        BytecodeSwitchArgument::CasePayload => None,
                    })
                    .collect();
                compiled.push((case.case_key, make_edge(case.edge.target, &args, payload)?));
            }
            let carried = cases.iter().any(|case| {
                case.edge
                    .arguments
                    .contains(&BytecodeSwitchArgument::Value(*value))
            });
            Exit::Switch {
                register: reg(*value)?,
                take: !carried && source.parameter_registers.contains(value),
                cases: compiled,
            }
        }
        BytecodeTerminator::Trap { code, payload } => Exit::Trap(
            *code,
            match payload {
                Some(r) => Some(reg(*r)?),
                None => None,
            },
        ),
        _ => return None,
    };
    Some(exit)
}

fn compile_step(
    code: &crate::BytecodeFunction,
    instruction: &crate::Instruction,
    constants: &mut Vec<ConstantBinding>,
    fields: &mut Vec<FieldBinding>,
) -> Option<Step> {
    let [result] = instruction.results.as_slice() else {
        return None;
    };
    let reg = |r| {
        usize::try_from(r)
            .ok()
            .filter(|&r| r < code.register_types.len())
    };
    let output = reg(*result)?;
    let args = instruction
        .operands
        .iter()
        .map(|r| reg(*r))
        .collect::<Option<Vec<_>>>()?;
    let opcode = Opcode::from_tag(instruction.opcode)?;
    let types: Vec<_> = args.iter().map(|&r| &code.register_types[r]).collect();
    let result_type = &code.register_types[output];
    let constant = match (&opcode, &instruction.immediate) {
        (Opcode::ConstantRef, Immediate::Entity(id)) if args.is_empty() => {
            let index = constants.len();
            constants.push(ConstantBinding {
                entity: *id,
                register: output,
            });
            Some(index)
        }
        (Opcode::RecordGet, Immediate::Field(member)) => {
            let [TypeExpr::Named(record)] = types.as_slice() else {
                return None;
            };
            fields.push(FieldBinding {
                definition: record.definition,
                member: *member,
                register: output,
            });
            None
        }
        (_, Immediate::None) if valid_step(opcode, &types, result_type) => None,
        _ => return None,
    };
    // Ask the shared accounting code; do not copy its semantic-unit formulas.
    let (normal, failure) = unit_samples(result_type)?;
    Some(Step {
        opcode,
        output,
        args,
        constant,
        units: [value_units_const(&normal), value_units_const(&failure)],
        dynamic: !fixed_units(result_type),
        wrapper_units: match &normal.data {
            ConstData::Option(Some(v)) => value_units_const(&normal) - value_units_const(v),
            _ => 0,
        },
        field: match instruction.immediate {
            Immediate::Field(id) => Some(id),
            _ => None,
        },
    })
}

fn compile_edge(
    code: &crate::BytecodeFunction,
    local_parameters: &[Register],
    target: u32,
    arguments: &[Option<Register>],
    payload: Option<&TypeExpr>,
) -> Option<Edge> {
    let reg = |r| {
        usize::try_from(r)
            .ok()
            .filter(|&r| r < code.register_types.len())
    };
    let target = usize::try_from(target).ok()?;
    let dest = code.blocks.get(target)?;
    if dest.parameter_registers.len() != arguments.len() {
        return None;
    }
    let mut args = Vec::new();
    for (arg, dest) in arguments.iter().zip(&dest.parameter_registers) {
        let actual = match arg {
            Some(r) => &code.register_types[reg(*r)?],
            None => payload?,
        };
        if actual != &code.register_types[reg(*dest)?] {
            return None;
        }
        args.push(Argument {
            register: match arg {
                Some(r) => Some(reg(*r)?),
                None => None,
            },
            take: arg.is_some_and(|r| {
                local_parameters.contains(&r)
                    && arguments.iter().filter(|a| **a == Some(r)).count() == 1
            }),
        });
    }
    Some(Edge { target, args })
}

// A selected edge or a charged resource termination; malformed state uses RuntimeFault.
fn select_case<'a, 'v>(
    runtime: &mut Runtime,
    limits: &ExecutionLimits,
    registers: &mut [Option<Value<'v>>],
    register: usize,
    take: bool,
    cases: &'a [(CaseKey, Edge)],
) -> RuntimeResult<Result<(&'a Edge, Option<Value<'v>>), ExecutionTermination>> {
    let value = if take {
        registers[register].take().ok_or(RuntimeFault)?
    } else {
        get(registers, register)?
    };
    let key = match value.shape {
        Shape::Ok => BuiltinCase::Ok,
        Shape::Err => BuiltinCase::Err,
        Shape::Some => BuiltinCase::Some,
        Shape::None => BuiltinCase::None,
        Shape::Plain => return Err(RuntimeFault),
    };
    for (case, edge) in cases {
        if let Some(end) = charge_action(runtime, limits, None) {
            return Ok(Err(end));
        }
        if *case == CaseKey::Builtin(key) {
            return Ok(Ok((
                edge,
                (value.shape != Shape::None).then_some(Value {
                    atom: value.atom,
                    shape: Shape::Plain,
                }),
            )));
        }
    }
    Err(RuntimeFault)
}

fn get<'v>(registers: &[Option<Value<'v>>], r: usize) -> RuntimeResult<Value<'v>> {
    registers.get(r).copied().flatten().ok_or(RuntimeFault)
}

fn valid_step(op: Opcode, args: &[&TypeExpr], result: &TypeExpr) -> bool {
    match (op, args) {
        (Opcode::VectorLen, [TypeExpr::Vector(_)]) => integer(result) == Some((false, 64)),
        (Opcode::VectorGet, [TypeExpr::Vector(inner), index]) => {
            integer(index) == Some((false, 64))
                && matches!(result, TypeExpr::Option(t) if t == inner)
        }
        (Opcode::BoolNot, [a]) => **a == TypeExpr::Bool && *result == TypeExpr::Bool,
        (Opcode::BoolAnd | Opcode::BoolOr, [a, b]) => {
            **a == TypeExpr::Bool && a == b && *result == TypeExpr::Bool
        }
        (Opcode::Equal | Opcode::NotEqual, [a, b]) => {
            leaf(a) && a == b && *result == TypeExpr::Bool
        }
        (
            Opcode::LessThan | Opcode::LessEqual | Opcode::GreaterThan | Opcode::GreaterEqual,
            [a, b],
        ) => integer(a).is_some() && a == b && *result == TypeExpr::Bool,
        (
            Opcode::IntAddChecked
            | Opcode::IntSubChecked
            | Opcode::IntMulChecked
            | Opcode::IntDivChecked
            | Opcode::IntRemChecked,
            [a, b],
        ) => a == b && checked_result(a, result),
        (Opcode::IntNegChecked, [a]) => matches!(a, TypeExpr::SInt(_)) && checked_result(a, result),
        (Opcode::IntShlChecked | Opcode::IntShrChecked, [a, b]) => {
            integer(b) == Some((false, 32)) && checked_result(a, result)
        }
        (Opcode::ResultOk, [a]) => matches!(result, TypeExpr::Result { ok, .. } if **ok == **a),
        (Opcode::ResultErr, [a]) => {
            matches!(result, TypeExpr::Result { error, .. } if **error == **a)
        }
        _ => false,
    }
}

fn checked_result(a: &TypeExpr, result: &TypeExpr) -> bool {
    integer(a).is_some()
        && matches!(result, TypeExpr::Result { ok, error } if **ok == *a && **error == TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic))
}

fn unit_samples(ty: &TypeExpr) -> Option<(ConstValue, ConstValue)> {
    let normal = sample(ty)?;
    let data = match ty {
        TypeExpr::Result { error, .. } => {
            ConstData::Result(ResultConst::Err(Box::new(sample(error)?)))
        }
        TypeExpr::Option(_) => ConstData::Option(None),
        _ => return Some((normal.clone(), normal)),
    };
    Some((
        normal,
        ConstValue {
            value_type: ty.clone(),
            data,
        },
    ))
}

fn sample(ty: &TypeExpr) -> Option<ConstValue> {
    let data = match ty {
        TypeExpr::Unit => ConstData::Unit,
        TypeExpr::Bool => ConstData::Bool(false),
        TypeExpr::SInt(_) => ConstData::SInt(0),
        TypeExpr::UInt(_) => ConstData::UInt(0),
        TypeExpr::Text => ConstData::Text(String::new()),
        TypeExpr::Vector(_) => ConstData::Sequence(Vec::new()),
        TypeExpr::Named(n) => ConstData::Record(sley_ssmc::RecordConst {
            definition: n.definition,
            fields: Vec::new(),
        }),
        TypeExpr::Option(inner) => ConstData::Option(Some(Box::new(sample(inner)?))),
        TypeExpr::Result { ok, .. } => ConstData::Result(ResultConst::Ok(Box::new(sample(ok)?))),
        TypeExpr::BuiltinFailure(kind) => ConstData::BuiltinFailure(BuiltinFailureValue {
            kind: *kind,
            code: 1,
        }),
        _ => return None,
    };
    Some(ConstValue {
        value_type: ty.clone(),
        data,
    })
}

fn evaluate<'v>(
    step: &Step,
    code: &crate::BytecodeFunction,
    registers: &[Option<Value<'v>>],
) -> RuntimeResult<Value<'v>> {
    let a = get(registers, step.args[0])?;
    let b = if step.args.len() == 2 {
        Some(get(registers, step.args[1])?)
    } else {
        None
    };
    let boolean = |b| Value {
        atom: Atom::Bool(b),
        shape: Shape::Plain,
    };
    Ok(match step.opcode {
        Opcode::VectorLen | Opcode::VectorGet | Opcode::RecordGet => project(step, a, b)?,
        Opcode::ResultOk => Value {
            atom: a.atom,
            shape: Shape::Ok,
        },
        Opcode::ResultErr => Value {
            atom: a.atom,
            shape: Shape::Err,
        },
        Opcode::BoolNot => {
            let Atom::Bool(v) = a.atom else {
                return Err(RuntimeFault);
            };
            boolean(!v)
        }
        Opcode::BoolAnd | Opcode::BoolOr => {
            let (Atom::Bool(a), Atom::Bool(b)) = (a.atom, b.ok_or(RuntimeFault)?.atom) else {
                return Err(RuntimeFault);
            };
            boolean(if step.opcode == Opcode::BoolAnd {
                a && b
            } else {
                a || b
            })
        }
        Opcode::Equal
        | Opcode::NotEqual
        | Opcode::LessThan
        | Opcode::LessEqual
        | Opcode::GreaterThan
        | Opcode::GreaterEqual => {
            let order = match (a.atom, b.ok_or(RuntimeFault)?.atom) {
                (Atom::Signed(a), Atom::Signed(b)) => a.cmp(&b),
                (Atom::Unsigned(a), Atom::Unsigned(b)) => a.cmp(&b),
                (Atom::Bool(a), Atom::Bool(b)) => a.cmp(&b),
                (Atom::Unit, Atom::Unit) => core::cmp::Ordering::Equal,
                (Atom::Failure(a), Atom::Failure(b)) => a.cmp(&b),
                (
                    Atom::Borrowed(ConstValue {
                        data: ConstData::Text(a),
                        ..
                    }),
                    Atom::Borrowed(ConstValue {
                        data: ConstData::Text(b),
                        ..
                    }),
                ) => a.cmp(b),
                _ => return Err(RuntimeFault),
            };
            boolean(match step.opcode {
                Opcode::Equal => order.is_eq(),
                Opcode::NotEqual => !order.is_eq(),
                Opcode::LessThan => order.is_lt(),
                Opcode::LessEqual => order.is_le(),
                Opcode::GreaterThan => order.is_gt(),
                _ => order.is_ge(),
            })
        }
        _ => checked(step, code, a, b)?,
    })
}

fn checked<'v>(
    step: &Step,
    code: &crate::BytecodeFunction,
    a: Value<'v>,
    b: Option<Value<'v>>,
) -> RuntimeResult<Value<'v>> {
    let ty = &code.register_types[step.args[0]];
    let (signed, bits) = integer(ty).ok_or(RuntimeFault)?;
    let data = |value: Value<'_>| match value.atom {
        Atom::Signed(v) => Ok(ConstData::SInt(v)),
        Atom::Unsigned(v) => Ok(ConstData::UInt(v)),
        _ => Err(RuntimeFault),
    };
    let a = data(a)?;
    let b = b.map(data).transpose()?;
    let pair;
    let one = [&a];
    let args: &[&ConstData] = if let Some(b) = &b {
        pair = [&a, b];
        &pair
    } else {
        &one
    };
    Ok(
        match crate::extended::checked_integer_data(step.opcode, signed, bits, args)
            .map_err(|_| RuntimeFault)?
        {
            crate::extended::Checked::Value(s, u) => Value {
                atom: if signed {
                    Atom::Signed(s)
                } else {
                    Atom::Unsigned(u)
                },
                shape: Shape::Ok,
            },
            crate::extended::Checked::Failure(code) => Value {
                atom: Atom::Failure(code),
                shape: Shape::Err,
            },
        },
    )
}

fn project<'v>(step: &Step, a: Value<'v>, b: Option<Value<'v>>) -> RuntimeResult<Value<'v>> {
    Ok(match step.opcode {
        Opcode::VectorLen => {
            let Atom::Borrowed(ConstValue {
                data: ConstData::Sequence(items),
                ..
            }) = a.atom
            else {
                return Err(RuntimeFault);
            };
            Value {
                atom: Atom::Unsigned(items.len() as u128),
                shape: Shape::Plain,
            }
        }
        Opcode::VectorGet => {
            let Atom::Borrowed(ConstValue {
                data: ConstData::Sequence(items),
                ..
            }) = a.atom
            else {
                return Err(RuntimeFault);
            };
            let Atom::Unsigned(index) = b.ok_or(RuntimeFault)?.atom else {
                return Err(RuntimeFault);
            };
            match usize::try_from(index)
                .ok()
                .and_then(|index| items.get(index))
            {
                Some(item) => Value {
                    shape: Shape::Some,
                    ..Value::from_const(item).ok_or(RuntimeFault)?
                },
                None => Value {
                    atom: Atom::Unit,
                    shape: Shape::None,
                },
            }
        }
        Opcode::RecordGet => {
            let Atom::Borrowed(ConstValue {
                data: ConstData::Record(record),
                ..
            }) = a.atom
            else {
                return Err(RuntimeFault);
            };
            let field = record
                .fields
                .iter()
                .find(|f| Some(f.member_id) == step.field)
                .ok_or(RuntimeFault)?;
            Value::from_const(&field.value).ok_or(RuntimeFault)?
        }
        _ => return Err(RuntimeFault),
    })
}

#[cfg(test)]
mod tests {
    use super::super::{
        BytecodeSwitchEdge, CacheProfile, ConstantDefinition, SchemaEpochId, StateRoot,
        TypeEnvironment, finish, hash_validated_value, run_core,
    };
    use super::*;
    use crate::{BytecodeBlock, BytecodeFunction, BytecodeSwitchCase, Instruction};
    use sley_ssmc::IntegerWidth;

    #[test]
    #[allow(clippy::too_many_lines)] // Keep the full control-flow fixture beside its budget sweep.
    fn loop_edges_switches_and_every_budget_cut_match_reference() {
        let ty = TypeExpr::SInt(IntegerWidth::from_bits(64));
        let failure = TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic);
        let checked = TypeExpr::Result {
            ok: Box::new(ty.clone()),
            error: Box::new(failure.clone()),
        };
        let instruction = |opcode: Opcode, operands, output, immediate| Instruction {
            opcode: opcode.tag(),
            operands,
            results: vec![output],
            immediate,
        };
        let block = |slot, parameters, instructions, terminator| BytecodeBlock {
            slot,
            parameter_registers: parameters,
            instructions,
            terminator,
            reachability: 1,
        };
        let edge = |target, arguments| BytecodeTargetEdge { target, arguments };
        let one = EntityId::from_bytes([1; 32]);
        let zero = EntityId::from_bytes([2; 32]);
        let bytecode = BytecodeFunction {
            function: EntityId::from_bytes([9; 32]),
            parameter_registers: vec![0],
            register_types: vec![
                ty.clone(),
                ty.clone(),
                ty.clone(),
                checked,
                TypeExpr::Bool,
                ty.clone(),
                ty.clone(),
                ty.clone(),
                ty.clone(),
                failure,
                ty.clone(),
            ],
            result_type: ty.clone(),
            entry_block: 0,
            blocks: vec![
                block(
                    0,
                    vec![],
                    vec![
                        instruction(Opcode::ConstantRef, vec![], 1, Immediate::Entity(one)),
                        instruction(Opcode::ConstantRef, vec![], 5, Immediate::Entity(zero)),
                    ],
                    BytecodeTerminator::Branch(edge(1, vec![0])),
                ),
                block(
                    1,
                    vec![2],
                    // Repeat a reference in another block and a nonadjacent
                    // register: constant slots are per instruction, not per
                    // entity, output register, or position within one block.
                    vec![
                        instruction(Opcode::ConstantRef, vec![], 10, Immediate::Entity(zero)),
                        instruction(Opcode::LessEqual, vec![2, 10], 4, Immediate::None),
                    ],
                    BytecodeTerminator::CondBranch {
                        condition: 4,
                        if_true: edge(3, vec![2]),
                        if_false: edge(2, vec![2, 2]),
                    },
                ),
                block(
                    2,
                    vec![7, 8],
                    vec![instruction(
                        Opcode::IntSubChecked,
                        vec![7, 1],
                        3,
                        Immediate::None,
                    )],
                    BytecodeTerminator::VariantSwitch {
                        value: 3,
                        cases: vec![
                            BytecodeSwitchCase {
                                case_key: CaseKey::Builtin(BuiltinCase::Err),
                                edge: BytecodeSwitchEdge {
                                    target: 4,
                                    arguments: vec![BytecodeSwitchArgument::CasePayload],
                                },
                            },
                            BytecodeSwitchCase {
                                case_key: CaseKey::Builtin(BuiltinCase::Ok),
                                edge: BytecodeSwitchEdge {
                                    target: 1,
                                    arguments: vec![BytecodeSwitchArgument::CasePayload],
                                },
                            },
                        ],
                    },
                ),
                block(3, vec![6], vec![], BytecodeTerminator::Return(6)),
                block(
                    4,
                    vec![9],
                    vec![],
                    BytecodeTerminator::Trap {
                        code: 7,
                        payload: Some(9),
                    },
                ),
            ],
        };
        let epoch = SchemaEpochId::from_bytes([8; 32]);
        let root = StateRoot::from_bytes([7; 32]);
        let types = TypeEnvironment::new(Vec::new()).unwrap();
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
        let source = ExecutionSource {
            types: &types,
            constants: &constants,
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
        let limits = ExecutionLimits {
            max_instructions: 10_000,
            max_fuel: 10_000,
            max_value_units: 100_000,
            max_output_units: 1000,
            cancel_at_fuel: None,
        };
        for (n, decrement) in [
            (-1, 1),
            (0, 1),
            (1, 1),
            (2, 1),
            (7, 1),
            (1, i128::from(i64::MIN)),
        ] {
            let mut trial_constants = constants.clone();
            trial_constants[0].value = value(decrement);
            let source = ExecutionSource {
                constants: &trial_constants,
                ..source
            };
            let inputs = vec![value(n)];
            let validated = ValidatedInputs {
                hashes: vec![hash_validated_value(epoch, &inputs[0]).unwrap()],
                value_units: value_units_const(&inputs[0]),
            };
            let request = ExecutionRequest { inputs, limits };
            let baseline = run_core(
                &source,
                &lowered,
                &validated,
                request.clone(),
                MAX_CALL_DEPTH,
            );
            let mut caps = vec![limits];
            caps.push(ExecutionLimits {
                max_fuel: u64::MAX,
                max_instructions: u64::MAX,
                max_value_units: u64::MAX,
                max_output_units: u64::MAX,
                ..limits
            });
            for cap in 0..=baseline.fuel_used + 1 {
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
                caps.push(ExecutionLimits {
                    max_fuel: cap,
                    cancel_at_fuel: Some(cap),
                    ..limits
                });
                caps.push(ExecutionLimits {
                    max_fuel: cap,
                    max_instructions: cap,
                    ..limits
                });
            }
            for cap in 0..=baseline.peak_value_units + 1 {
                caps.push(ExecutionLimits {
                    max_value_units: cap,
                    ..limits
                });
            }
            for cap in 0..=22 {
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
                let reference = run_core(
                    &source,
                    &lowered,
                    &validated,
                    request.clone(),
                    MAX_CALL_DEPTH,
                );
                let actual = plan
                    .execute(&source, &lowered, &validated, &request)
                    .unwrap();
                let observed = |r: CoreOutcome| {
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
                    .unwrap()
                };
                assert_eq!(
                    observed(actual),
                    observed(reference),
                    "n={n} limits={limits:?}"
                );
            }
        }
    }
}

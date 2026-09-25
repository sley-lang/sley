//! AV1: the output-only agent view.
//!
//! AV1 is derived debug notation (ADR-0001): a compact, deterministic,
//! non-canonical listing of program entities under local names. Every
//! rendering begins with a header that marks it non-canonical. No product
//! component reads AV1; authoring goes through AF1 frames (JSON data) or raw
//! mutation operations.

use std::fmt::Write as _;

use sley_id::EntityId;
use sley_mutate::value::EntityBodyValue;
use sley_mutate::value::{BlockBody, FunctionBody, OperationBody, TestCaseBody};
use sley_ssmc::{
    CaseKey, ExpectedOutcome, Immediate, Reachability, SwitchArgument, TargetEdge, Terminator,
    TrapCode, TypeDefForm, Visibility,
};

use crate::hex;
use crate::names::Names;
use crate::opcodes::{self, ImmediateKind};
use crate::types;
use crate::values;
use crate::workspace::Program;

/// Rendering switches.
#[derive(Clone, Copy, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ViewOptions {
    /// Print full entity ids.
    pub ids: bool,
    /// Print operation result types.
    pub types: bool,
    /// Print `TestCase` resource limits.
    pub limits: bool,
}

/// The AV1 header line.
#[must_use]
pub fn header(program: &Program, after: Option<&str>) -> String {
    let mut line = format!(
        "# sley view (AV1, non-canonical) root={}",
        hex::short(program.root().as_bytes())
    );
    if let Some(handle) = after {
        let _ = write!(line, " after={handle}");
    }
    line.push('\n');
    line
}

/// Renders one entity of any kind; function-scoped entities render their
/// whole function.
#[must_use]
pub fn entity(program: &Program, names: &Names, id: &EntityId, options: ViewOptions) -> String {
    let Some(body) = program.body(id) else {
        return format!("# {} is not live in this state\n", names.name(id));
    };
    match body {
        EntityBodyValue::Function(function) => {
            render_function(program, names, id, function, options)
        }
        EntityBodyValue::Block(block) => owner_function(program, names, &block.function, options),
        EntityBodyValue::Operation(operation) => match program.body(&operation.block) {
            Some(EntityBodyValue::Block(block)) => {
                owner_function(program, names, &block.function, options)
            }
            _ => format!("# orphan operation {}\n", names.name(id)),
        },
        EntityBodyValue::Parameter(parameter) => match program.body(&parameter.owner) {
            Some(EntityBodyValue::Function(_)) => {
                owner_function(program, names, &parameter.owner, options)
            }
            Some(EntityBodyValue::Block(block)) => {
                owner_function(program, names, &block.function, options)
            }
            _ => format!("# orphan parameter {}\n", names.name(id)),
        },
        _ => top_level(program, names, id, body, options),
    }
}

fn owner_function(
    program: &Program,
    names: &Names,
    function: &EntityId,
    options: ViewOptions,
) -> String {
    match program.body(function) {
        Some(EntityBodyValue::Function(body)) => {
            render_function(program, names, function, body, options)
        }
        _ => format!("# owner {} is not a live function\n", names.name(function)),
    }
}

fn id_suffix(id: &EntityId, options: ViewOptions) -> String {
    if options.ids {
        format!("   [{}]", hex::encode(id.as_bytes()))
    } else {
        String::new()
    }
}

fn param_list(program: &Program, names: &Names, parameters: &[EntityId]) -> String {
    parameters
        .iter()
        .map(|param| match program.body(param) {
            Some(EntityBodyValue::Parameter(body)) => {
                format!(
                    "{}: {}",
                    names.leaf(param),
                    types::render(&body.value_type, names)
                )
            }
            _ => format!("{}: ?", names.leaf(param)),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_function(
    program: &Program,
    names: &Names,
    id: &EntityId,
    function: &FunctionBody,
    options: ViewOptions,
) -> String {
    let mut out = String::new();
    let generics = if function.type_parameters.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            function
                .type_parameters
                .iter()
                .map(|p| format!("${}", p.ordinal))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let _ = write!(
        out,
        "fn {}{generics}({}) -> {}",
        names.name(id),
        param_list(program, names, &function.parameters),
        types::render(&function.result_type, names)
    );
    if function.visibility != Visibility::Exported {
        let _ = write!(out, " {}", visibility(function.visibility));
    }
    if !function.effects.as_slice().is_empty() {
        let _ = write!(
            out,
            " effects[{}]",
            list_names(names, function.effects.as_slice())
        );
    }
    if !function.contracts.as_slice().is_empty() {
        let _ = write!(
            out,
            " contracts[{}]",
            list_names(names, function.contracts.as_slice())
        );
    }
    if options.ids {
        let _ = writeln!(out, "{}", id_suffix(id, options));
    } else {
        let _ = writeln!(out, "   [{}]", hex::short(id.as_bytes()));
    }
    if !function.blocks.contains(&function.entry_block) {
        let _ = writeln!(
            out,
            "  # entry block {} is not listed",
            names.name(&function.entry_block)
        );
    }
    for block_id in &function.blocks {
        match program.body(block_id) {
            Some(EntityBodyValue::Block(block)) => {
                render_block(&mut out, program, names, id, block_id, block, options);
            }
            _ => {
                let _ = writeln!(out, "  {}:   # missing block", names.leaf(block_id));
            }
        }
    }
    orphans(&mut out, program, names, id, function);
    out
}

fn list_names(names: &Names, ids: &[EntityId]) -> String {
    ids.iter()
        .map(|id| names.name(id))
        .collect::<Vec<_>>()
        .join(", ")
}

const fn visibility(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Private => "private",
        Visibility::Package => "package",
        Visibility::Workspace => "workspace",
        Visibility::Exported => "exported",
    }
}

fn render_block(
    out: &mut String,
    program: &Program,
    names: &Names,
    function: &EntityId,
    id: &EntityId,
    block: &BlockBody,
    options: ViewOptions,
) {
    let params = if block.parameters.is_empty() {
        String::new()
    } else {
        format!("({})", param_list(program, names, &block.parameters))
    };
    let dead = if block.reachability == Reachability::ExplicitlyUnreachable {
        " unreachable"
    } else {
        ""
    };
    let foreign = if block.function == *function {
        String::new()
    } else {
        format!("   # owned by {}", names.name(&block.function))
    };
    let _ = writeln!(
        out,
        "  {}{params}:{dead}{foreign}{}",
        names.leaf(id),
        id_suffix(id, options)
    );
    for (position, operation_id) in block.operations.iter().enumerate() {
        match program.body(operation_id) {
            Some(EntityBodyValue::Operation(operation)) => {
                let mut line =
                    render_operation(program, names, id, operation_id, operation, options);
                if operation.block != *id {
                    let _ = write!(line, "   # owned by {}", names.name(&operation.block));
                }
                if operation.ordinal as usize != position {
                    let _ = write!(line, "   # ordinal {}", operation.ordinal);
                }
                let _ = writeln!(out, "    {line}");
            }
            _ => {
                let _ = writeln!(
                    out,
                    "    {} = ?   # missing operation",
                    names.leaf(operation_id)
                );
            }
        }
    }
    let _ = writeln!(
        out,
        "    {}",
        render_terminator(program, names, id, &block.terminator)
    );
}

/// Renders one operation line (without indentation).
#[must_use]
pub fn render_operation(
    program: &Program,
    names: &Names,
    block: &EntityId,
    id: &EntityId,
    operation: &OperationBody,
    options: ViewOptions,
) -> String {
    let row = opcodes::by_tag(operation.opcode);
    let mnemonic = row.map_or_else(
        || format!("op{}", operation.opcode),
        |row| row.mnemonic.to_owned(),
    );
    let mut line = names.leaf(id);
    if options.types && !operation.result_types.is_empty() {
        let _ = write!(
            line,
            ": {}",
            operation
                .result_types
                .iter()
                .map(|ty| types::render(ty, names))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let _ = write!(line, " = {mnemonic}");
    let immediate = render_immediate(
        program,
        names,
        &operation.immediate,
        row.map(|row| row.immediate),
    );
    if !immediate.is_empty() {
        let _ = write!(line, " {immediate}");
    }
    let operands = operation
        .operands
        .iter()
        .map(|value| names.value(value, Some(block)))
        .collect::<Vec<_>>()
        .join(", ");
    if !operands.is_empty() {
        let _ = write!(line, " {operands}");
    }
    let _ = write!(line, "{}", id_suffix(id, options));
    line
}

fn render_immediate(
    program: &Program,
    names: &Names,
    immediate: &Immediate,
    expected: Option<ImmediateKind>,
) -> String {
    match immediate {
        Immediate::None => String::new(),
        Immediate::Entity(id) => match program.body(id) {
            Some(EntityBodyValue::Constant(constant))
                if expected == Some(ImmediateKind::Entity) =>
            {
                format!(
                    "{} ({})",
                    names.name(id),
                    values::to_text(&constant.value, names)
                )
            }
            _ => names.name(id),
        },
        Immediate::Index(index) => index.to_string(),
        Immediate::Field(member) => names.member_any(member),
        Immediate::Variant(variant) => names.member(&variant.definition, &variant.member_id),
        Immediate::Observation(bytes) => format!("obs_{}", hex::short(bytes)),
        Immediate::Function(function) => {
            if function.type_arguments.is_empty() {
                names.name(&function.function)
            } else {
                format!(
                    "{}<{}>",
                    names.name(&function.function),
                    function
                        .type_arguments
                        .iter()
                        .map(|ty| types::render(ty, names))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
        }
    }
}

fn edge(names: &Names, block: &EntityId, edge: &TargetEdge) -> String {
    if edge.arguments.is_empty() {
        names.leaf(&edge.target)
    } else {
        format!(
            "{}({})",
            names.leaf(&edge.target),
            edge.arguments
                .iter()
                .map(|value| names.value(value, Some(block)))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Renders a terminator (without indentation).
#[must_use]
pub fn render_terminator(
    program: &Program,
    names: &Names,
    block: &EntityId,
    terminator: &Terminator,
) -> String {
    match terminator {
        Terminator::Return(ret) => format!("return {}", names.value(&ret.value, Some(block))),
        Terminator::Branch(branch) => format!("br {}", edge(names, block, &branch.edge)),
        Terminator::CondBranch(cond) => format!(
            "cond {} -> {}, {}",
            names.value(&cond.condition, Some(block)),
            edge(names, block, &cond.if_true),
            edge(names, block, &cond.if_false)
        ),
        Terminator::VariantSwitch(switch) => {
            let cases = switch
                .cases
                .iter()
                .map(|case| {
                    let key = match case.case_key {
                        CaseKey::Builtin(builtin) => format!("{builtin:?}"),
                        CaseKey::Member(member) => member_key(program, names, &member),
                    };
                    let args = case
                        .edge
                        .arguments
                        .iter()
                        .map(|argument| match argument {
                            SwitchArgument::Value(value) => names.value(value, Some(block)),
                            SwitchArgument::CasePayload => "$".to_owned(),
                        })
                        .collect::<Vec<_>>();
                    if args.is_empty() {
                        format!("{key} -> {}", names.leaf(&case.edge.target))
                    } else {
                        format!(
                            "{key} -> {}({})",
                            names.leaf(&case.edge.target),
                            args.join(", ")
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "switch {}: {cases}",
                names.value(&switch.value, Some(block))
            )
        }
        Terminator::Trap(trap) => {
            let code = match trap.code {
                TrapCode::Unreachable => "unreachable",
                TrapCode::ResourceExhausted => "resource_exhausted",
                TrapCode::AdapterContractViolation => "adapter_contract_violation",
                TrapCode::InternalInvariant => "internal_invariant",
            };
            match &trap.payload {
                Some(value) => format!("trap {code} {}", names.value(value, Some(block))),
                None => format!("trap {code}"),
            }
        }
    }
}

fn member_key(program: &Program, names: &Names, member: &sley_ssmc::MemberId) -> String {
    let _ = program;
    let full = names.member_any(member);
    full.rsplit('.').next().unwrap_or(&full).to_owned()
}

/// Lists blocks, parameters, and operations that claim this function as
/// owner but that no function list reaches (they refuse validation).
fn orphans(
    out: &mut String,
    program: &Program,
    names: &Names,
    id: &EntityId,
    function: &FunctionBody,
) {
    let mut lines = Vec::new();
    for object in program.objects() {
        let record = object.record();
        match &record.body {
            EntityBodyValue::Block(block)
                if block.function == *id && !function.blocks.contains(&record.entity_id) =>
            {
                lines.push(format!("block {}", names.leaf(&record.entity_id)));
            }
            EntityBodyValue::Parameter(parameter)
                if parameter.owner == *id && !function.parameters.contains(&record.entity_id) =>
            {
                lines.push(format!("parameter {}", names.leaf(&record.entity_id)));
            }
            _ => {}
        }
    }
    if !lines.is_empty() {
        let _ = writeln!(
            out,
            "  # unlisted (refused by validation): {}",
            lines.join(", ")
        );
    }
}

fn top_level(
    program: &Program,
    names: &Names,
    id: &EntityId,
    body: &EntityBodyValue,
    options: ViewOptions,
) -> String {
    let name = names.name(id);
    let suffix = id_suffix(id, options);
    match body {
        EntityBodyValue::TypeDef(typedef) => {
            let generics = if typedef.type_parameters.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    typedef
                        .type_parameters
                        .iter()
                        .map(|p| format!("${}", p.ordinal))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            };
            let form = match &typedef.form {
                TypeDefForm::Variant(cases) => cases
                    .iter()
                    .map(|case| {
                        let leaf = names.member_leaf(id, &case.member_id);
                        match &case.payload_type {
                            Some(ty) => format!("{leaf}({})", types::render(ty, names)),
                            None => leaf,
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | "),
                TypeDefForm::Record(fields) => format!(
                    "{{{}}}",
                    fields
                        .iter()
                        .map(|field| format!(
                            "{}: {}",
                            names.member_leaf(id, &field.member_id),
                            types::render(&field.value_type, names)
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
            format!("type {name}{generics} = {form}{suffix}\n")
        }
        EntityBodyValue::Constant(constant) => format!(
            "const {name}: {} = {}{suffix}\n",
            types::render(&constant.value.value_type, names),
            values::to_text(&constant.value, names)
        ),
        EntityBodyValue::TestCase(test) => render_test(program, names, id, test, options),
        EntityBodyValue::Namespace(namespace) => format!(
            "ns {name}: {}{suffix}\n",
            list_names(names, namespace.members.as_slice())
        ),
        EntityBodyValue::GlobalValue(global) => format!(
            "global {name}: {} = {}{suffix}\n",
            types::render(&global.value_type, names),
            names.name(&global.initializer)
        ),
        EntityBodyValue::EntryPoint(entry) => {
            format!(
                "entrypoint {name} -> {}{suffix}\n",
                names.name(&entry.function)
            )
        }
        other => format!(
            "{} {name}{suffix}\n",
            crate::names::kind_prefix(other.kind_tag())
        ),
    }
}

/// Renders one `TestCase` as `test name: target(inputs) == expected`.
#[must_use]
pub fn render_test(
    program: &Program,
    names: &Names,
    id: &EntityId,
    test: &TestCaseBody,
    options: ViewOptions,
) -> String {
    let _ = program;
    let inputs = test
        .inputs
        .iter()
        .map(|value| values::to_text(value, names))
        .collect::<Vec<_>>()
        .join(", ");
    let expected = match &test.expected {
        ExpectedOutcome::Value(value) => values::to_text(value, names),
        ExpectedOutcome::FailureCode(code) => format!("trap({code})"),
    };
    let mut line = format!(
        "test {}: {}({inputs}) == {expected}",
        names.name(id),
        names.name(&test.target)
    );
    if options.limits {
        let limits = test.resource_limits;
        let _ = write!(
            line,
            "   limits fuel={} memory={} output={} effects={} depth={} wall_ms={}",
            limits.fuel,
            limits.memory_bytes,
            limits.output_bytes,
            limits.effect_count,
            limits.call_depth,
            limits.wall_timeout_millis
        );
    }
    let _ = write!(line, "{}", id_suffix(id, options));
    line.push('\n');
    line
}

/// Renders the whole program: namespaces, types, constants, functions, and
/// tests, each group in name order.
#[must_use]
pub fn package(program: &Program, names: &Names, options: ViewOptions) -> String {
    let mut groups: [Vec<(String, EntityId)>; 6] = Default::default();
    for object in program.objects() {
        let record = object.record();
        let slot = match record.body.kind_tag() {
            3 => 0,
            4 => 1,
            9 => 2,
            5 => 3,
            14 => 4,
            6..=8 => continue,
            _ => 5,
        };
        groups[slot].push((names.name(&record.entity_id), record.entity_id));
    }
    let mut out = String::new();
    for (slot, group) in groups.iter_mut().enumerate() {
        group.sort();
        for (_, id) in group.iter() {
            out.push_str(&entity(program, names, id, options));
            if slot == 3 {
                out.push('\n');
            }
        }
    }
    out
}

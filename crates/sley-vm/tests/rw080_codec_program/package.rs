//! `Package` (entity kind 2) body encoding construction.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-package-encode.md`.

use super::*;
use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};

fn bytes_vector_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(TypeExpr::Bytes))
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn build_concat_bytes(a: &mut Asm, ns: Ns, fid: EntityId) -> FunctionGraph {
    let block_start = a.blocks.len();
    let result_type = encode_result_type();
    let zero = a.ku64(ns.k, 0);
    let one = a.ku64(ns.k, 1);
    let resource_error = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let parts = a.param(ns.p, fid, ParameterRole::Function, bytes_vector_type());
    let unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let resource = err_block(a, ns, fid, result_type.clone(), resource_error);
    let trap = trap_block(a, ns, fid);

    let entry = a.id(ns.b);
    let part_check = a.id(ns.b);
    let part_get = a.id(ns.b);
    let part_convert = a.id(ns.b);
    let byte_start = a.id(ns.b);
    let byte_check = a.id(ns.b);
    let byte_get = a.id(ns.b);
    let byte_push = a.id(ns.b);
    let byte_next = a.id(ns.b);
    let part_next = a.id(ns.b);
    let finish = a.id(ns.b);
    let return_ok = a.id(ns.b);

    let empty = a.op(
        ns.o,
        entry,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let part_count = a.op(
        ns.o,
        entry,
        Opcode::VectorLen,
        vec![pav(parts)],
        vec![u64_type()],
        Immediate::None,
    );
    let first = a.cref(ns.o, entry, zero, u64_type());
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![empty, part_count, first],
        terminator: branch(edge(
            part_check,
            vec![
                op_result(first),
                op_result(empty),
                pav(parts),
                op_result(part_count),
                pav(unit),
            ],
        )),
        reachability: Reachability::Required,
    });

    let index = a.param(ns.p, part_check, ParameterRole::Block, u64_type());
    let acc = a.param(ns.p, part_check, ParameterRole::Block, u8vec_type());
    let all_parts = a.param(ns.p, part_check, ParameterRole::Block, bytes_vector_type());
    let count = a.param(ns.p, part_check, ParameterRole::Block, u64_type());
    let check_unit = a.param(ns.p, part_check, ParameterRole::Block, TypeExpr::Unit);
    let more = a.op(
        ns.o,
        part_check,
        Opcode::LessThan,
        vec![pav(index), pav(count)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: part_check,
        function: fid,
        parameters: vec![index, acc, all_parts, count, check_unit],
        operations: vec![more],
        terminator: cond(
            op_result(more),
            edge(
                part_get,
                vec![
                    pav(index),
                    pav(acc),
                    pav(all_parts),
                    pav(count),
                    pav(check_unit),
                ],
            ),
            edge(finish, vec![pav(acc), pav(check_unit)]),
        ),
        reachability: Reachability::Required,
    });

    let get_index = a.param(ns.p, part_get, ParameterRole::Block, u64_type());
    let get_acc = a.param(ns.p, part_get, ParameterRole::Block, u8vec_type());
    let get_parts = a.param(ns.p, part_get, ParameterRole::Block, bytes_vector_type());
    let get_count = a.param(ns.p, part_get, ParameterRole::Block, u64_type());
    let get_unit = a.param(ns.p, part_get, ParameterRole::Block, TypeExpr::Unit);
    let get = a.op(
        ns.o,
        part_get,
        Opcode::VectorGet,
        vec![pav(get_parts), pav(get_index)],
        vec![TypeExpr::Option(Box::new(TypeExpr::Bytes))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: part_get,
        function: fid,
        parameters: vec![get_index, get_acc, get_parts, get_count, get_unit],
        operations: vec![get],
        terminator: switch(
            op_result(get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    part_convert,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(get_index),
                        sav(get_acc),
                        sav(get_parts),
                        sav(get_count),
                        sav(get_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    let part = a.param(ns.p, part_convert, ParameterRole::Block, TypeExpr::Bytes);
    let convert_index = a.param(ns.p, part_convert, ParameterRole::Block, u64_type());
    let convert_acc = a.param(ns.p, part_convert, ParameterRole::Block, u8vec_type());
    let convert_parts = a.param(
        ns.p,
        part_convert,
        ParameterRole::Block,
        bytes_vector_type(),
    );
    let convert_count = a.param(ns.p, part_convert, ParameterRole::Block, u64_type());
    let convert_unit = a.param(ns.p, part_convert, ParameterRole::Block, TypeExpr::Unit);
    let convert = a.op(
        ns.o,
        part_convert,
        Opcode::AdapterInvoke,
        vec![pav(convert_unit), pav(part)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: part_convert,
        function: fid,
        parameters: vec![
            part,
            convert_index,
            convert_acc,
            convert_parts,
            convert_count,
            convert_unit,
        ],
        operations: vec![convert],
        terminator: switch(
            op_result(convert),
            vec![
                (
                    BuiltinCase::Ok,
                    byte_start,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(convert_index),
                        sav(convert_acc),
                        sav(convert_parts),
                        sav(convert_count),
                        sav(convert_unit),
                    ],
                ),
                (BuiltinCase::Err, resource, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let part_vector = a.param(ns.p, byte_start, ParameterRole::Block, u8vec_type());
    let start_index = a.param(ns.p, byte_start, ParameterRole::Block, u64_type());
    let start_acc = a.param(ns.p, byte_start, ParameterRole::Block, u8vec_type());
    let start_parts = a.param(ns.p, byte_start, ParameterRole::Block, bytes_vector_type());
    let start_count = a.param(ns.p, byte_start, ParameterRole::Block, u64_type());
    let start_unit = a.param(ns.p, byte_start, ParameterRole::Block, TypeExpr::Unit);
    let byte_count = a.op(
        ns.o,
        byte_start,
        Opcode::VectorLen,
        vec![pav(part_vector)],
        vec![u64_type()],
        Immediate::None,
    );
    let byte_zero = a.cref(ns.o, byte_start, zero, u64_type());
    a.blocks.push(Block {
        entity_id: byte_start,
        function: fid,
        parameters: vec![
            part_vector,
            start_index,
            start_acc,
            start_parts,
            start_count,
            start_unit,
        ],
        operations: vec![byte_count, byte_zero],
        terminator: branch(edge(
            byte_check,
            vec![
                op_result(byte_zero),
                pav(part_vector),
                op_result(byte_count),
                pav(start_index),
                pav(start_acc),
                pav(start_parts),
                pav(start_count),
                pav(start_unit),
            ],
        )),
        reachability: Reachability::Required,
    });

    let byte_index = a.param(ns.p, byte_check, ParameterRole::Block, u64_type());
    let bytes = a.param(ns.p, byte_check, ParameterRole::Block, u8vec_type());
    let bytes_count = a.param(ns.p, byte_check, ParameterRole::Block, u64_type());
    let current_part = a.param(ns.p, byte_check, ParameterRole::Block, u64_type());
    let byte_acc = a.param(ns.p, byte_check, ParameterRole::Block, u8vec_type());
    let byte_parts = a.param(ns.p, byte_check, ParameterRole::Block, bytes_vector_type());
    let part_total = a.param(ns.p, byte_check, ParameterRole::Block, u64_type());
    let byte_unit = a.param(ns.p, byte_check, ParameterRole::Block, TypeExpr::Unit);
    let has_byte = a.op(
        ns.o,
        byte_check,
        Opcode::LessThan,
        vec![pav(byte_index), pav(bytes_count)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: byte_check,
        function: fid,
        parameters: vec![
            byte_index,
            bytes,
            bytes_count,
            current_part,
            byte_acc,
            byte_parts,
            part_total,
            byte_unit,
        ],
        operations: vec![has_byte],
        terminator: cond(
            op_result(has_byte),
            edge(
                byte_get,
                vec![
                    pav(byte_index),
                    pav(bytes),
                    pav(bytes_count),
                    pav(current_part),
                    pav(byte_acc),
                    pav(byte_parts),
                    pav(part_total),
                    pav(byte_unit),
                ],
            ),
            edge(
                part_next,
                vec![
                    pav(current_part),
                    pav(byte_acc),
                    pav(byte_parts),
                    pav(part_total),
                    pav(byte_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    let read_index = a.param(ns.p, byte_get, ParameterRole::Block, u64_type());
    let read_bytes = a.param(ns.p, byte_get, ParameterRole::Block, u8vec_type());
    let read_count = a.param(ns.p, byte_get, ParameterRole::Block, u64_type());
    let read_part = a.param(ns.p, byte_get, ParameterRole::Block, u64_type());
    let read_acc = a.param(ns.p, byte_get, ParameterRole::Block, u8vec_type());
    let read_parts = a.param(ns.p, byte_get, ParameterRole::Block, bytes_vector_type());
    let read_total = a.param(ns.p, byte_get, ParameterRole::Block, u64_type());
    let read_unit = a.param(ns.p, byte_get, ParameterRole::Block, TypeExpr::Unit);
    let read = a.op(
        ns.o,
        byte_get,
        Opcode::VectorGet,
        vec![pav(read_bytes), pav(read_index)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: byte_get,
        function: fid,
        parameters: vec![
            read_index, read_bytes, read_count, read_part, read_acc, read_parts, read_total,
            read_unit,
        ],
        operations: vec![read],
        terminator: switch(
            op_result(read),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    byte_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(read_index),
                        sav(read_bytes),
                        sav(read_count),
                        sav(read_part),
                        sav(read_acc),
                        sav(read_parts),
                        sav(read_total),
                        sav(read_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    let byte = a.param(ns.p, byte_push, ParameterRole::Block, u8_type());
    let push_index = a.param(ns.p, byte_push, ParameterRole::Block, u64_type());
    let push_bytes = a.param(ns.p, byte_push, ParameterRole::Block, u8vec_type());
    let push_count = a.param(ns.p, byte_push, ParameterRole::Block, u64_type());
    let push_part = a.param(ns.p, byte_push, ParameterRole::Block, u64_type());
    let push_acc = a.param(ns.p, byte_push, ParameterRole::Block, u8vec_type());
    let push_parts = a.param(ns.p, byte_push, ParameterRole::Block, bytes_vector_type());
    let push_total = a.param(ns.p, byte_push, ParameterRole::Block, u64_type());
    let push_unit = a.param(ns.p, byte_push, ParameterRole::Block, TypeExpr::Unit);
    let pushed = a.op(
        ns.o,
        byte_push,
        Opcode::AdapterInvoke,
        vec![pav(push_acc), pav(byte)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: byte_push,
        function: fid,
        parameters: vec![
            byte, push_index, push_bytes, push_count, push_part, push_acc, push_parts, push_total,
            push_unit,
        ],
        operations: vec![pushed],
        terminator: switch(
            op_result(pushed),
            vec![
                (
                    BuiltinCase::Ok,
                    byte_next,
                    vec![
                        sav(push_index),
                        sav(push_bytes),
                        sav(push_count),
                        sav(push_part),
                        SwitchArgument::CasePayload,
                        sav(push_parts),
                        sav(push_total),
                        sav(push_unit),
                    ],
                ),
                (BuiltinCase::Err, resource, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let next_byte = a.param(ns.p, byte_next, ParameterRole::Block, u64_type());
    let next_bytes = a.param(ns.p, byte_next, ParameterRole::Block, u8vec_type());
    let next_count = a.param(ns.p, byte_next, ParameterRole::Block, u64_type());
    let next_part = a.param(ns.p, byte_next, ParameterRole::Block, u64_type());
    let next_acc = a.param(ns.p, byte_next, ParameterRole::Block, u8vec_type());
    let next_parts = a.param(ns.p, byte_next, ParameterRole::Block, bytes_vector_type());
    let next_total = a.param(ns.p, byte_next, ParameterRole::Block, u64_type());
    let next_unit = a.param(ns.p, byte_next, ParameterRole::Block, TypeExpr::Unit);
    let byte_one = a.cref(ns.o, byte_next, one, u64_type());
    let increment_byte = a.op(
        ns.o,
        byte_next,
        Opcode::IntAddChecked,
        vec![pav(next_byte), op_result(byte_one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: byte_next,
        function: fid,
        parameters: vec![
            next_byte, next_bytes, next_count, next_part, next_acc, next_parts, next_total,
            next_unit,
        ],
        operations: vec![byte_one, increment_byte],
        terminator: switch(
            op_result(increment_byte),
            vec![
                (
                    BuiltinCase::Ok,
                    byte_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(next_bytes),
                        sav(next_count),
                        sav(next_part),
                        sav(next_acc),
                        sav(next_parts),
                        sav(next_total),
                        sav(next_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let finished_part = a.param(ns.p, part_next, ParameterRole::Block, u64_type());
    let finished_acc = a.param(ns.p, part_next, ParameterRole::Block, u8vec_type());
    let finished_parts = a.param(ns.p, part_next, ParameterRole::Block, bytes_vector_type());
    let finished_total = a.param(ns.p, part_next, ParameterRole::Block, u64_type());
    let finished_unit = a.param(ns.p, part_next, ParameterRole::Block, TypeExpr::Unit);
    let part_one = a.cref(ns.o, part_next, one, u64_type());
    let increment_part = a.op(
        ns.o,
        part_next,
        Opcode::IntAddChecked,
        vec![pav(finished_part), op_result(part_one)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: part_next,
        function: fid,
        parameters: vec![
            finished_part,
            finished_acc,
            finished_parts,
            finished_total,
            finished_unit,
        ],
        operations: vec![part_one, increment_part],
        terminator: switch(
            op_result(increment_part),
            vec![
                (
                    BuiltinCase::Ok,
                    part_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(finished_acc),
                        sav(finished_parts),
                        sav(finished_total),
                        sav(finished_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let final_acc = a.param(ns.p, finish, ParameterRole::Block, u8vec_type());
    let final_unit = a.param(ns.p, finish, ParameterRole::Block, TypeExpr::Unit);
    let converted = a.op(
        ns.o,
        finish,
        Opcode::AdapterInvoke,
        vec![pav(final_unit), pav(final_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    a.blocks.push(Block {
        entity_id: finish,
        function: fid,
        parameters: vec![final_acc, final_unit],
        operations: vec![converted],
        terminator: switch(
            op_result(converted),
            vec![
                (
                    BuiltinCase::Ok,
                    return_ok,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let output = a.param(ns.p, return_ok, ParameterRole::Block, TypeExpr::Bytes);
    let ok = a.op(
        ns.o,
        return_ok,
        Opcode::ResultOk,
        vec![pav(output)],
        vec![result_type.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: return_ok,
        function: fid,
        parameters: vec![output],
        operations: vec![ok],
        terminator: ret(op_result(ok)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![parts, unit],
        result_type,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[block_start..]
            .iter()
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn build_package_compose(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    encode_fid: EntityId,
    concat_fid: EntityId,
) -> FunctionGraph {
    let block_start = a.blocks.len();
    let result_type = encode_result_type();
    let width64 = a.ku32(ns.k, 64);
    let resource_error = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let record_prefix = a.kbytes(ns.k, &[4, 1, 32]);
    let field2 = a.kbytes(ns.k, &[2, 32]);
    let field3 = a.kbytes(ns.k, &[3]);
    let field4 = a.kbytes(ns.k, &[4]);
    let union_tag = a.kbytes(ns.k, &[2]);

    let workspace = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let root_namespace = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let dependencies = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let exports = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let resource = err_block(a, ns, fid, result_type.clone(), resource_error);

    let entry = a.id(ns.b);
    let dependencies_length = a.id(ns.b);
    let exports_convert = a.id(ns.b);
    let exports_length = a.id(ns.b);
    let record_parts = a.id(ns.b);
    let record_ready = a.id(ns.b);
    let record_length = a.id(ns.b);
    let wrap = a.id(ns.b);
    let forward_error = a.id(ns.b);

    let dependencies_vector = a.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(unit), pav(dependencies)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![dependencies_vector],
        terminator: switch(
            op_result(dependencies_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    dependencies_length,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(workspace),
                        sav(root_namespace),
                        sav(dependencies),
                        sav(exports),
                        sav(unit),
                    ],
                ),
                (BuiltinCase::Err, resource, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let dependencies_vec = a.param(
        ns.p,
        dependencies_length,
        ParameterRole::Block,
        u8vec_type(),
    );
    let length_workspace = a.param(
        ns.p,
        dependencies_length,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let length_root = a.param(
        ns.p,
        dependencies_length,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let length_dependencies = a.param(
        ns.p,
        dependencies_length,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let length_exports = a.param(
        ns.p,
        dependencies_length,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let length_unit = a.param(
        ns.p,
        dependencies_length,
        ParameterRole::Block,
        TypeExpr::Unit,
    );
    let dependencies_count = a.op(
        ns.o,
        dependencies_length,
        Opcode::VectorLen,
        vec![pav(dependencies_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let dependencies_width = a.cref(ns.o, dependencies_length, width64, u32_type());
    let encoded_dependencies_length = a.op(
        ns.o,
        dependencies_length,
        Opcode::CallDirect,
        vec![
            op_result(dependencies_count),
            op_result(dependencies_width),
            pav(length_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: encode_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: dependencies_length,
        function: fid,
        parameters: vec![
            dependencies_vec,
            length_workspace,
            length_root,
            length_dependencies,
            length_exports,
            length_unit,
        ],
        operations: vec![
            dependencies_count,
            dependencies_width,
            encoded_dependencies_length,
        ],
        terminator: switch(
            op_result(encoded_dependencies_length),
            vec![
                (
                    BuiltinCase::Ok,
                    exports_convert,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(length_workspace),
                        sav(length_root),
                        sav(length_dependencies),
                        sav(length_exports),
                        sav(length_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    let dependencies_length_bytes =
        a.param(ns.p, exports_convert, ParameterRole::Block, TypeExpr::Bytes);
    let export_workspace = a.param(ns.p, exports_convert, ParameterRole::Block, TypeExpr::Bytes);
    let export_root = a.param(ns.p, exports_convert, ParameterRole::Block, TypeExpr::Bytes);
    let export_dependencies = a.param(ns.p, exports_convert, ParameterRole::Block, TypeExpr::Bytes);
    let export_values = a.param(ns.p, exports_convert, ParameterRole::Block, TypeExpr::Bytes);
    let export_unit = a.param(ns.p, exports_convert, ParameterRole::Block, TypeExpr::Unit);
    let exports_vector = a.op(
        ns.o,
        exports_convert,
        Opcode::AdapterInvoke,
        vec![pav(export_unit), pav(export_values)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: exports_convert,
        function: fid,
        parameters: vec![
            dependencies_length_bytes,
            export_workspace,
            export_root,
            export_dependencies,
            export_values,
            export_unit,
        ],
        operations: vec![exports_vector],
        terminator: switch(
            op_result(exports_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    exports_length,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(dependencies_length_bytes),
                        sav(export_workspace),
                        sav(export_root),
                        sav(export_dependencies),
                        sav(export_values),
                        sav(export_unit),
                    ],
                ),
                (BuiltinCase::Err, resource, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let exports_vec = a.param(ns.p, exports_length, ParameterRole::Block, u8vec_type());
    let saved_dependencies_length =
        a.param(ns.p, exports_length, ParameterRole::Block, TypeExpr::Bytes);
    let saved_workspace = a.param(ns.p, exports_length, ParameterRole::Block, TypeExpr::Bytes);
    let saved_root = a.param(ns.p, exports_length, ParameterRole::Block, TypeExpr::Bytes);
    let saved_dependencies = a.param(ns.p, exports_length, ParameterRole::Block, TypeExpr::Bytes);
    let saved_exports = a.param(ns.p, exports_length, ParameterRole::Block, TypeExpr::Bytes);
    let saved_unit = a.param(ns.p, exports_length, ParameterRole::Block, TypeExpr::Unit);
    let exports_count = a.op(
        ns.o,
        exports_length,
        Opcode::VectorLen,
        vec![pav(exports_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let exports_width = a.cref(ns.o, exports_length, width64, u32_type());
    let encoded_exports_length = a.op(
        ns.o,
        exports_length,
        Opcode::CallDirect,
        vec![
            op_result(exports_count),
            op_result(exports_width),
            pav(saved_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: encode_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: exports_length,
        function: fid,
        parameters: vec![
            exports_vec,
            saved_dependencies_length,
            saved_workspace,
            saved_root,
            saved_dependencies,
            saved_exports,
            saved_unit,
        ],
        operations: vec![exports_count, exports_width, encoded_exports_length],
        terminator: switch(
            op_result(encoded_exports_length),
            vec![
                (
                    BuiltinCase::Ok,
                    record_parts,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(saved_dependencies_length),
                        sav(saved_workspace),
                        sav(saved_root),
                        sav(saved_dependencies),
                        sav(saved_exports),
                        sav(saved_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    let exports_length_bytes = a.param(ns.p, record_parts, ParameterRole::Block, TypeExpr::Bytes);
    let dependencies_length_bytes =
        a.param(ns.p, record_parts, ParameterRole::Block, TypeExpr::Bytes);
    let record_workspace = a.param(ns.p, record_parts, ParameterRole::Block, TypeExpr::Bytes);
    let record_root = a.param(ns.p, record_parts, ParameterRole::Block, TypeExpr::Bytes);
    let record_dependencies = a.param(ns.p, record_parts, ParameterRole::Block, TypeExpr::Bytes);
    let record_exports = a.param(ns.p, record_parts, ParameterRole::Block, TypeExpr::Bytes);
    let record_unit = a.param(ns.p, record_parts, ParameterRole::Block, TypeExpr::Unit);
    let prefix = a.cref(ns.o, record_parts, record_prefix, TypeExpr::Bytes);
    let second = a.cref(ns.o, record_parts, field2, TypeExpr::Bytes);
    let third = a.cref(ns.o, record_parts, field3, TypeExpr::Bytes);
    let fourth = a.cref(ns.o, record_parts, field4, TypeExpr::Bytes);
    let parts = a.op(
        ns.o,
        record_parts,
        Opcode::VectorNew,
        vec![
            op_result(prefix),
            pav(record_workspace),
            op_result(second),
            pav(record_root),
            op_result(third),
            pav(dependencies_length_bytes),
            pav(record_dependencies),
            op_result(fourth),
            pav(exports_length_bytes),
            pav(record_exports),
        ],
        vec![bytes_vector_type()],
        Immediate::None,
    );
    let compose_record = a.op(
        ns.o,
        record_parts,
        Opcode::CallDirect,
        vec![op_result(parts), pav(record_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: concat_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: record_parts,
        function: fid,
        parameters: vec![
            exports_length_bytes,
            dependencies_length_bytes,
            record_workspace,
            record_root,
            record_dependencies,
            record_exports,
            record_unit,
        ],
        operations: vec![prefix, second, third, fourth, parts, compose_record],
        terminator: switch(
            op_result(compose_record),
            vec![
                (
                    BuiltinCase::Ok,
                    record_ready,
                    vec![SwitchArgument::CasePayload, sav(record_unit)],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    let record = a.param(ns.p, record_ready, ParameterRole::Block, TypeExpr::Bytes);
    let ready_unit = a.param(ns.p, record_ready, ParameterRole::Block, TypeExpr::Unit);
    let record_vector = a.op(
        ns.o,
        record_ready,
        Opcode::AdapterInvoke,
        vec![pav(ready_unit), pav(record)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: record_ready,
        function: fid,
        parameters: vec![record, ready_unit],
        operations: vec![record_vector],
        terminator: switch(
            op_result(record_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    record_length,
                    vec![SwitchArgument::CasePayload, sav(record), sav(ready_unit)],
                ),
                (BuiltinCase::Err, resource, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let record_vec = a.param(ns.p, record_length, ParameterRole::Block, u8vec_type());
    let saved_record = a.param(ns.p, record_length, ParameterRole::Block, TypeExpr::Bytes);
    let record_length_unit = a.param(ns.p, record_length, ParameterRole::Block, TypeExpr::Unit);
    let record_count = a.op(
        ns.o,
        record_length,
        Opcode::VectorLen,
        vec![pav(record_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let record_width = a.cref(ns.o, record_length, width64, u32_type());
    let encoded_record_length = a.op(
        ns.o,
        record_length,
        Opcode::CallDirect,
        vec![
            op_result(record_count),
            op_result(record_width),
            pav(record_length_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: encode_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: record_length,
        function: fid,
        parameters: vec![record_vec, saved_record, record_length_unit],
        operations: vec![record_count, record_width, encoded_record_length],
        terminator: switch(
            op_result(encoded_record_length),
            vec![
                (
                    BuiltinCase::Ok,
                    wrap,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(saved_record),
                        sav(record_length_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    let record_length_bytes = a.param(ns.p, wrap, ParameterRole::Block, TypeExpr::Bytes);
    let final_record = a.param(ns.p, wrap, ParameterRole::Block, TypeExpr::Bytes);
    let wrap_unit = a.param(ns.p, wrap, ParameterRole::Block, TypeExpr::Unit);
    let tag = a.cref(ns.o, wrap, union_tag, TypeExpr::Bytes);
    let final_parts = a.op(
        ns.o,
        wrap,
        Opcode::VectorNew,
        vec![op_result(tag), pav(record_length_bytes), pav(final_record)],
        vec![bytes_vector_type()],
        Immediate::None,
    );
    let body = a.op(
        ns.o,
        wrap,
        Opcode::CallDirect,
        vec![op_result(final_parts), pav(wrap_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: concat_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: wrap,
        function: fid,
        parameters: vec![record_length_bytes, final_record, wrap_unit],
        operations: vec![tag, final_parts, body],
        terminator: ret(op_result(body)),
        reachability: Reachability::Required,
    });

    let error_bytes = a.param(ns.p, forward_error, ParameterRole::Block, TypeExpr::Bytes);
    let error = a.op(
        ns.o,
        forward_error,
        Opcode::ResultErr,
        vec![pav(error_bytes)],
        vec![result_type.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: forward_error,
        function: fid,
        parameters: vec![error_bytes],
        operations: vec![error],
        terminator: ret(op_result(error)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![workspace, root_namespace, dependencies, exports, unit],
        result_type,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[block_start..]
            .iter()
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn build_exact_identity_validate(a: &mut Asm, ns: Ns, fid: EntityId) -> FunctionGraph {
    let block_start = a.blocks.len();
    let result_type = encode_result_type();
    let exact_length = a.ku64(ns.k, 32);
    let length_error = a.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let trailing_error = a.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let resource_error = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let value = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let length_refusal = err_block(a, ns, fid, result_type.clone(), length_error);
    let trailing_refusal = err_block(a, ns, fid, result_type.clone(), trailing_error);
    let resource_refusal = err_block(a, ns, fid, result_type.clone(), resource_error);
    let entry = a.id(ns.b);
    let check = a.id(ns.b);
    let check_long = a.id(ns.b);
    let return_ok = a.id(ns.b);

    let converted = a.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(unit), pav(value)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![converted],
        terminator: switch(
            op_result(converted),
            vec![
                (
                    BuiltinCase::Ok,
                    check,
                    vec![SwitchArgument::CasePayload, sav(value)],
                ),
                (BuiltinCase::Err, resource_refusal, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let vector = a.param(ns.p, check, ParameterRole::Block, u8vec_type());
    let original = a.param(ns.p, check, ParameterRole::Block, TypeExpr::Bytes);
    let length = a.op(
        ns.o,
        check,
        Opcode::VectorLen,
        vec![pav(vector)],
        vec![u64_type()],
        Immediate::None,
    );
    let expected = a.cref(ns.o, check, exact_length, u64_type());
    let short = a.op(
        ns.o,
        check,
        Opcode::LessThan,
        vec![op_result(length), op_result(expected)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let long = a.op(
        ns.o,
        check,
        Opcode::GreaterThan,
        vec![op_result(length), op_result(expected)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: check,
        function: fid,
        parameters: vec![vector, original],
        operations: vec![length, expected, short, long],
        terminator: cond(
            op_result(short),
            edge(length_refusal, Vec::new()),
            edge(check_long, vec![op_result(long), pav(original)]),
        ),
        reachability: Reachability::Required,
    });

    let is_long = a.param(ns.p, check_long, ParameterRole::Block, TypeExpr::Bool);
    let checked = a.param(ns.p, check_long, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: check_long,
        function: fid,
        parameters: vec![is_long, checked],
        operations: Vec::new(),
        terminator: cond(
            pav(is_long),
            edge(trailing_refusal, Vec::new()),
            edge(return_ok, vec![pav(checked)]),
        ),
        reachability: Reachability::Required,
    });

    let output = a.param(ns.p, return_ok, ParameterRole::Block, TypeExpr::Bytes);
    let ok = a.op(
        ns.o,
        return_ok,
        Opcode::ResultOk,
        vec![pav(output)],
        vec![result_type.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: return_ok,
        function: fid,
        parameters: vec![output],
        operations: vec![ok],
        terminator: ret(op_result(ok)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![value, unit],
        result_type,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[block_start..]
            .iter()
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn build_package_encode(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    exact_fid: EntityId,
    set_fid: EntityId,
    compose_fid: EntityId,
) -> FunctionGraph {
    let block_start = a.blocks.len();
    let result_type = encode_result_type();
    let workspace = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let root_namespace = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let dependencies = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let exports = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let entry = a.id(ns.b);
    let validate_root = a.id(ns.b);
    let encode_dependencies = a.id(ns.b);
    let encode_exports = a.id(ns.b);
    let compose = a.id(ns.b);
    let forward_error = a.id(ns.b);

    let validated_workspace = a.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(workspace), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: exact_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![validated_workspace],
        terminator: switch(
            op_result(validated_workspace),
            vec![
                (
                    BuiltinCase::Ok,
                    validate_root,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(root_namespace),
                        sav(dependencies),
                        sav(exports),
                        sav(unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    let checked_workspace = a.param(ns.p, validate_root, ParameterRole::Block, TypeExpr::Bytes);
    let raw_root = a.param(ns.p, validate_root, ParameterRole::Block, TypeExpr::Bytes);
    let raw_dependencies = a.param(ns.p, validate_root, ParameterRole::Block, TypeExpr::Bytes);
    let raw_exports = a.param(ns.p, validate_root, ParameterRole::Block, TypeExpr::Bytes);
    let validate_unit = a.param(ns.p, validate_root, ParameterRole::Block, TypeExpr::Unit);
    let validated_root = a.op(
        ns.o,
        validate_root,
        Opcode::CallDirect,
        vec![pav(raw_root), pav(validate_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: exact_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: validate_root,
        function: fid,
        parameters: vec![
            checked_workspace,
            raw_root,
            raw_dependencies,
            raw_exports,
            validate_unit,
        ],
        operations: vec![validated_root],
        terminator: switch(
            op_result(validated_root),
            vec![
                (
                    BuiltinCase::Ok,
                    encode_dependencies,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(checked_workspace),
                        sav(raw_dependencies),
                        sav(raw_exports),
                        sav(validate_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    let checked_root = a.param(
        ns.p,
        encode_dependencies,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let dependency_workspace = a.param(
        ns.p,
        encode_dependencies,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let dependency_values = a.param(
        ns.p,
        encode_dependencies,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let pending_exports = a.param(
        ns.p,
        encode_dependencies,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let dependency_unit = a.param(
        ns.p,
        encode_dependencies,
        ParameterRole::Block,
        TypeExpr::Unit,
    );
    let encoded_dependencies = a.op(
        ns.o,
        encode_dependencies,
        Opcode::CallDirect,
        vec![
            pav(dependency_workspace),
            pav(dependency_values),
            pav(dependency_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: set_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: encode_dependencies,
        function: fid,
        parameters: vec![
            checked_root,
            dependency_workspace,
            dependency_values,
            pending_exports,
            dependency_unit,
        ],
        operations: vec![encoded_dependencies],
        terminator: switch(
            op_result(encoded_dependencies),
            vec![
                (
                    BuiltinCase::Ok,
                    encode_exports,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(dependency_workspace),
                        sav(checked_root),
                        sav(pending_exports),
                        sav(dependency_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    let dependencies_set = a.param(ns.p, encode_exports, ParameterRole::Block, TypeExpr::Bytes);
    let exports_workspace = a.param(ns.p, encode_exports, ParameterRole::Block, TypeExpr::Bytes);
    let exports_root = a.param(ns.p, encode_exports, ParameterRole::Block, TypeExpr::Bytes);
    let raw_exports = a.param(ns.p, encode_exports, ParameterRole::Block, TypeExpr::Bytes);
    let exports_unit = a.param(ns.p, encode_exports, ParameterRole::Block, TypeExpr::Unit);
    let encoded_exports = a.op(
        ns.o,
        encode_exports,
        Opcode::CallDirect,
        vec![pav(exports_root), pav(raw_exports), pav(exports_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: set_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: encode_exports,
        function: fid,
        parameters: vec![
            dependencies_set,
            exports_workspace,
            exports_root,
            raw_exports,
            exports_unit,
        ],
        operations: vec![encoded_exports],
        terminator: switch(
            op_result(encoded_exports),
            vec![
                (
                    BuiltinCase::Ok,
                    compose,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(dependencies_set),
                        sav(exports_workspace),
                        sav(exports_root),
                        sav(exports_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    forward_error,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    let exports_set = a.param(ns.p, compose, ParameterRole::Block, TypeExpr::Bytes);
    let compose_dependencies = a.param(ns.p, compose, ParameterRole::Block, TypeExpr::Bytes);
    let compose_workspace = a.param(ns.p, compose, ParameterRole::Block, TypeExpr::Bytes);
    let compose_root = a.param(ns.p, compose, ParameterRole::Block, TypeExpr::Bytes);
    let compose_unit = a.param(ns.p, compose, ParameterRole::Block, TypeExpr::Unit);
    let body = a.op(
        ns.o,
        compose,
        Opcode::CallDirect,
        vec![
            pav(compose_workspace),
            pav(compose_root),
            pav(compose_dependencies),
            pav(exports_set),
            pav(compose_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: compose_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: compose,
        function: fid,
        parameters: vec![
            exports_set,
            compose_dependencies,
            compose_workspace,
            compose_root,
            compose_unit,
        ],
        operations: vec![body],
        terminator: ret(op_result(body)),
        reachability: Reachability::Required,
    });

    let error_bytes = a.param(ns.p, forward_error, ParameterRole::Block, TypeExpr::Bytes);
    let error = a.op(
        ns.o,
        forward_error,
        Opcode::ResultErr,
        vec![pav(error_bytes)],
        vec![result_type.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: forward_error,
        function: fid,
        parameters: vec![error_bytes],
        operations: vec![error],
        terminator: ret(op_result(error)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![workspace, root_namespace, dependencies, exports, unit],
        result_type,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[block_start..]
            .iter()
            .map(|block| block.entity_id)
            .collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn package_encode_image() -> Image {
    let mut assembler = Asm::new();
    let encode_fid = eid(12, 1);
    let set_fid = eid(12, 2);
    let concat_fid = eid(12, 3);
    let compose_fid = eid(12, 4);
    let exact_fid = eid(12, 5);
    let package_fid = eid(12, 6);
    let encode_graph = build_encode(
        &mut assembler,
        Ns {
            k: 230,
            p: 231,
            b: 232,
            o: 233,
        },
        encode_fid,
    );
    let set_graph = build_identity_set_encode(
        &mut assembler,
        Ns {
            k: 234,
            p: 235,
            b: 236,
            o: 237,
        },
        set_fid,
        encode_fid,
    );
    let concat_graph = build_concat_bytes(
        &mut assembler,
        Ns {
            k: 238,
            p: 239,
            b: 240,
            o: 241,
        },
        concat_fid,
    );
    let compose_graph = build_package_compose(
        &mut assembler,
        Ns {
            k: 242,
            p: 243,
            b: 244,
            o: 245,
        },
        compose_fid,
        encode_fid,
        concat_fid,
    );
    let exact_graph = build_exact_identity_validate(
        &mut assembler,
        Ns {
            k: 246,
            p: 247,
            b: 248,
            o: 249,
        },
        exact_fid,
    );
    let package_graph = build_package_encode(
        &mut assembler,
        Ns {
            k: 250,
            p: 251,
            b: 252,
            o: 253,
        },
        package_fid,
        exact_fid,
        set_fid,
        compose_fid,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: package_graph.clone(),
        functions: vec![
            package_graph,
            exact_graph,
            compose_graph,
            concat_graph,
            set_graph,
            encode_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
        ],
        constants: assembler.constants,
    }
}

fn package_encode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    workspace: &[u8],
    root_namespace: &[u8],
    dependencies: &[u8],
    exports: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![
            bytes_input(workspace),
            bytes_input(root_namespace),
            bytes_input(dependencies),
            bytes_input(exports),
            unit_input(),
        ],
    )
}

fn package_stored(
    entity: [u8; 32],
    workspace: [u8; 32],
    root_namespace: [u8; 32],
    dependencies: &[[u8; 32]],
    exports: &[[u8; 32]],
) -> Vec<u8> {
    use sley_mutate::value::{EntityBodyValue, EntityIdSet, PackageBody};

    let set = |members: &[[u8; 32]]| {
        EntityIdSet::from_unsorted(
            members
                .iter()
                .copied()
                .map(sley_id::EntityId::from_bytes)
                .collect(),
        )
        .expect("fixture identities are unique")
    };
    let record = sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes(entity),
        body: EntityBodyValue::Package(PackageBody {
            workspace: sley_id::EntityId::from_bytes(workspace),
            root_namespace: sley_id::EntityId::from_bytes(root_namespace),
            dependencies: set(dependencies),
            exports: set(exports),
        }),
        label: None,
        semantic_fingerprint: None,
    };
    sley_mutate::build_entity_object(program_epoch9(), &record)
        .expect("native builds Package fixture")
        .stored_bytes()
        .to_vec()
}

fn package_body(
    workspace: [u8; 32],
    root_namespace: [u8; 32],
    dependencies: &[[u8; 32]],
    exports: &[[u8; 32]],
) -> Vec<u8> {
    ns_body_of(&package_stored(
        [1; 32],
        workspace,
        root_namespace,
        dependencies,
        exports,
    ))
}

fn concat_ids(ids: &[[u8; 32]]) -> Vec<u8> {
    ids.iter().flatten().copied().collect()
}

#[test]
fn package_encode_matches_native_semantics() {
    let (package, approved) = admit(&package_encode_image());
    let increasing = std::array::from_fn(|index| u8::try_from(index).expect("index fits u8"));
    let decreasing = std::array::from_fn(|index| u8::try_from(31 - index).expect("index fits u8"));
    let cases = [
        ([2; 32], [3; 32], Vec::new(), Vec::new()),
        ([2; 32], [3; 32], vec![[4; 32]], vec![[5; 32]]),
        ([2; 32], [3; 32], vec![[4; 32], [5; 32]], Vec::new()),
        ([2; 32], [3; 32], vec![[4; 32]], vec![[5; 32], [6; 32]]),
        (increasing, decreasing, vec![[4; 32]], vec![[5; 32]]),
    ];
    for (workspace, root_namespace, dependencies, exports) in cases {
        let expected = package_body(workspace, root_namespace, &dependencies, &exports);
        let outcome = package_encode_call(
            &package,
            &approved,
            &workspace,
            &root_namespace,
            &concat_ids(&dependencies),
            &concat_ids(&exports),
        );
        assert_encode_ok(&outcome, &expected);
        assert!(outcome.fuel_used < 1_000_000, "Package encode fuel");
        assert!(
            outcome.instruction_count < 100_000,
            "Package encode instructions"
        );
        assert!(
            outcome.peak_value_units < 1_000_000,
            "Package encode values"
        );
        eprintln!(
            "PACKAGE_ENC body{}B deps={} exports={} fuel={} instr={} peak={}",
            expected.len(),
            dependencies.len(),
            exports.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn package_encode_rejects_malformed_identities_and_sets() {
    let (package, approved) = admit(&package_encode_image());
    let valid = [2; 32];
    let short = vec![2; 31];
    let long = vec![2; 33];
    let partial = vec![4; 31];
    let duplicate = concat_ids(&[[4; 32], [4; 32]]);
    let unordered = concat_ids(&[[5; 32], [4; 32]]);

    for (workspace, root_namespace, dependencies, exports, code) in [
        (
            short.as_slice(),
            valid.as_slice(),
            &[][..],
            &[][..],
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            long.as_slice(),
            valid.as_slice(),
            &[][..],
            &[][..],
            "SCB_TRAILING_BYTES",
        ),
        (
            valid.as_slice(),
            short.as_slice(),
            &[][..],
            &[][..],
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            valid.as_slice(),
            long.as_slice(),
            &[][..],
            &[][..],
            "SCB_TRAILING_BYTES",
        ),
        (
            valid.as_slice(),
            valid.as_slice(),
            partial.as_slice(),
            &[][..],
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            valid.as_slice(),
            valid.as_slice(),
            duplicate.as_slice(),
            &[][..],
            "SCB_MAP_DUPLICATE",
        ),
        (
            valid.as_slice(),
            valid.as_slice(),
            unordered.as_slice(),
            &[][..],
            "SCB_MAP_ORDER",
        ),
        (
            valid.as_slice(),
            valid.as_slice(),
            &[][..],
            partial.as_slice(),
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            valid.as_slice(),
            valid.as_slice(),
            &[][..],
            duplicate.as_slice(),
            "SCB_MAP_DUPLICATE",
        ),
        (
            valid.as_slice(),
            valid.as_slice(),
            &[][..],
            unordered.as_slice(),
            "SCB_MAP_ORDER",
        ),
        (
            short.as_slice(),
            long.as_slice(),
            partial.as_slice(),
            unordered.as_slice(),
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            valid.as_slice(),
            long.as_slice(),
            unordered.as_slice(),
            duplicate.as_slice(),
            "SCB_TRAILING_BYTES",
        ),
        (
            valid.as_slice(),
            valid.as_slice(),
            partial.as_slice(),
            duplicate.as_slice(),
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            valid.as_slice(),
            valid.as_slice(),
            duplicate.as_slice(),
            partial.as_slice(),
            "SCB_MAP_DUPLICATE",
        ),
    ] {
        let outcome = package_encode_call(
            &package,
            &approved,
            workspace,
            root_namespace,
            dependencies,
            exports,
        );
        assert_refusal(&outcome, code);
    }
}

#[test]
fn package_encode_retains_the_value_unit_envelope() {
    let (package, approved) = admit(&package_encode_image());
    let outcome = package_encode_call(
        &package,
        &approved,
        &[2; 32],
        &[3; 32],
        &concat_ids(&[[4; 32], [5; 32]]),
        &concat_ids(&[[6; 32], [7; 32]]),
    );
    match &outcome.termination {
        sley_vm::ExecutionTermination::ResourceLimit(kind) => assert_eq!(
            *kind,
            sley_vm::ResourceKind::ValueUnits,
            "four total Package set members reach the F5 value-unit envelope first",
        ),
        other => panic!("four total Package set members must expose the F5 envelope: {other:?}"),
    }
    eprintln!(
        "PACKAGE_ENC_F5 deps=2 exports=2 fuel={} instr={} peak={}",
        outcome.fuel_used, outcome.instruction_count, outcome.peak_value_units
    );
}

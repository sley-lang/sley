//! `Package` (entity kind 2) body encoding construction.
//! Construction provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-package-encode.md`.
//! Strict decode provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-package-decode.md`.
//! Whole-program decode composition provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-package-compose-decode.md`.
//! Whole-program encode composition provenance:
//! `machineresearch/sley-2.0/reweave/rw-080-codec-package-compose-encode.md`.

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
fn build_policy_set_payload_wrap(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    encode_fid: EntityId,
    concat_fid: EntityId,
) -> FunctionGraph {
    let block_start = a.blocks.len();
    let result_type = encode_result_type();
    let width64 = a.ku32(ns.k, 64);
    let one = a.ku64(ns.k, 1);
    let single_byte_limit = a.ku64(ns.k, 128);
    let record_fixed_width = a.ku64(ns.k, 36);
    let resource_error = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let record_prefix = a.kbytes(ns.k, &[2, 1, 32]);
    let field2 = a.kbytes(ns.k, &[2]);
    let union_tag = a.kbytes(ns.k, &[17]);
    let subject = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let set_payload = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let resource = err_block(a, ns, fid, result_type.clone(), resource_error);
    let trap = trap_block(a, ns, fid);
    let entry = a.id(ns.b);
    let payload_length = a.id(ns.b);
    let payload_width = a.id(ns.b);
    let payload_width_slow = a.id(ns.b);
    let record_fast = a.id(ns.b);
    let record_base = a.id(ns.b);
    let record_total = a.id(ns.b);
    let encode_record_length = a.id(ns.b);
    let wrap = a.id(ns.b);
    let forward_error = a.id(ns.b);

    let payload_vector = a.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(unit), pav(set_payload)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![payload_vector],
        terminator: switch(
            op_result(payload_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    payload_length,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(subject),
                        sav(set_payload),
                        sav(unit),
                    ],
                ),
                (BuiltinCase::Err, resource, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let payload_vec = a.param(ns.p, payload_length, ParameterRole::Block, u8vec_type());
    let saved_subject = a.param(ns.p, payload_length, ParameterRole::Block, TypeExpr::Bytes);
    let saved_payload = a.param(ns.p, payload_length, ParameterRole::Block, TypeExpr::Bytes);
    let saved_unit = a.param(ns.p, payload_length, ParameterRole::Block, TypeExpr::Unit);
    let payload_count = a.op(
        ns.o,
        payload_length,
        Opcode::VectorLen,
        vec![pav(payload_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let width = a.cref(ns.o, payload_length, width64, u32_type());
    let encoded_length = a.op(
        ns.o,
        payload_length,
        Opcode::CallDirect,
        vec![op_result(payload_count), op_result(width), pav(saved_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: encode_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: payload_length,
        function: fid,
        parameters: vec![payload_vec, saved_subject, saved_payload, saved_unit],
        operations: vec![payload_count, width, encoded_length],
        terminator: switch(
            op_result(encoded_length),
            vec![
                (
                    BuiltinCase::Ok,
                    payload_width,
                    vec![
                        SwitchArgument::CasePayload,
                        oav(payload_count),
                        sav(saved_subject),
                        sav(saved_payload),
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

    let payload_length_bytes = a.param(ns.p, payload_width, ParameterRole::Block, TypeExpr::Bytes);
    let payload_count = a.param(ns.p, payload_width, ParameterRole::Block, u64_type());
    let width_subject = a.param(ns.p, payload_width, ParameterRole::Block, TypeExpr::Bytes);
    let width_payload = a.param(ns.p, payload_width, ParameterRole::Block, TypeExpr::Bytes);
    let width_unit = a.param(ns.p, payload_width, ParameterRole::Block, TypeExpr::Unit);
    let width_limit = a.cref(ns.o, payload_width, single_byte_limit, u64_type());
    let is_single_byte = a.op(
        ns.o,
        payload_width,
        Opcode::LessThan,
        vec![pav(payload_count), op_result(width_limit)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: payload_width,
        function: fid,
        parameters: vec![
            payload_length_bytes,
            payload_count,
            width_subject,
            width_payload,
            width_unit,
        ],
        operations: vec![width_limit, is_single_byte],
        terminator: cond(
            op_result(is_single_byte),
            edge(
                record_fast,
                vec![
                    pav(payload_count),
                    pav(payload_length_bytes),
                    pav(width_subject),
                    pav(width_payload),
                    pav(width_unit),
                ],
            ),
            edge(
                payload_width_slow,
                vec![
                    pav(payload_count),
                    pav(payload_length_bytes),
                    pav(width_subject),
                    pav(width_payload),
                    pav(width_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    let slow_payload_count = a.param(ns.p, payload_width_slow, ParameterRole::Block, u64_type());
    let slow_payload_length = a.param(
        ns.p,
        payload_width_slow,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let slow_subject = a.param(
        ns.p,
        payload_width_slow,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let slow_payload = a.param(
        ns.p,
        payload_width_slow,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let slow_unit = a.param(
        ns.p,
        payload_width_slow,
        ParameterRole::Block,
        TypeExpr::Unit,
    );
    let length_vector = a.op(
        ns.o,
        payload_width_slow,
        Opcode::AdapterInvoke,
        vec![pav(slow_unit), pav(slow_payload_length)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: payload_width_slow,
        function: fid,
        parameters: vec![
            slow_payload_count,
            slow_payload_length,
            slow_subject,
            slow_payload,
            slow_unit,
        ],
        operations: vec![length_vector],
        terminator: switch(
            op_result(length_vector),
            vec![
                (
                    BuiltinCase::Ok,
                    record_base,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(slow_payload_count),
                        sav(slow_payload_length),
                        sav(slow_subject),
                        sav(slow_payload),
                        sav(slow_unit),
                    ],
                ),
                (BuiltinCase::Err, resource, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let fast_payload_count = a.param(ns.p, record_fast, ParameterRole::Block, u64_type());
    let fast_payload_length = a.param(ns.p, record_fast, ParameterRole::Block, TypeExpr::Bytes);
    let fast_subject = a.param(ns.p, record_fast, ParameterRole::Block, TypeExpr::Bytes);
    let fast_payload = a.param(ns.p, record_fast, ParameterRole::Block, TypeExpr::Bytes);
    let fast_unit = a.param(ns.p, record_fast, ParameterRole::Block, TypeExpr::Unit);
    let fixed = a.cref(ns.o, record_fast, record_fixed_width, u64_type());
    let one = a.cref(ns.o, record_fast, one, u64_type());
    let base_length = a.op(
        ns.o,
        record_fast,
        Opcode::IntAddChecked,
        vec![pav(fast_payload_count), op_result(fixed)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: record_fast,
        function: fid,
        parameters: vec![
            fast_payload_count,
            fast_payload_length,
            fast_subject,
            fast_payload,
            fast_unit,
        ],
        operations: vec![fixed, one, base_length],
        terminator: switch(
            op_result(base_length),
            vec![
                (
                    BuiltinCase::Ok,
                    record_total,
                    vec![
                        SwitchArgument::CasePayload,
                        oav(one),
                        sav(fast_payload_length),
                        sav(fast_subject),
                        sav(fast_payload),
                        sav(fast_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let length_vec = a.param(ns.p, record_base, ParameterRole::Block, u8vec_type());
    let base_payload_count = a.param(ns.p, record_base, ParameterRole::Block, u64_type());
    let base_payload_length = a.param(ns.p, record_base, ParameterRole::Block, TypeExpr::Bytes);
    let base_subject = a.param(ns.p, record_base, ParameterRole::Block, TypeExpr::Bytes);
    let base_payload = a.param(ns.p, record_base, ParameterRole::Block, TypeExpr::Bytes);
    let base_unit = a.param(ns.p, record_base, ParameterRole::Block, TypeExpr::Unit);
    let length_width = a.op(
        ns.o,
        record_base,
        Opcode::VectorLen,
        vec![pav(length_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let fixed = a.cref(ns.o, record_base, record_fixed_width, u64_type());
    let base_length = a.op(
        ns.o,
        record_base,
        Opcode::IntAddChecked,
        vec![pav(base_payload_count), op_result(fixed)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: record_base,
        function: fid,
        parameters: vec![
            length_vec,
            base_payload_count,
            base_payload_length,
            base_subject,
            base_payload,
            base_unit,
        ],
        operations: vec![length_width, fixed, base_length],
        terminator: switch(
            op_result(base_length),
            vec![
                (
                    BuiltinCase::Ok,
                    record_total,
                    vec![
                        SwitchArgument::CasePayload,
                        oav(length_width),
                        sav(base_payload_length),
                        sav(base_subject),
                        sav(base_payload),
                        sav(base_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let base_length = a.param(ns.p, record_total, ParameterRole::Block, u64_type());
    let payload_width = a.param(ns.p, record_total, ParameterRole::Block, u64_type());
    let total_payload_length = a.param(ns.p, record_total, ParameterRole::Block, TypeExpr::Bytes);
    let total_subject = a.param(ns.p, record_total, ParameterRole::Block, TypeExpr::Bytes);
    let total_payload = a.param(ns.p, record_total, ParameterRole::Block, TypeExpr::Bytes);
    let total_unit = a.param(ns.p, record_total, ParameterRole::Block, TypeExpr::Unit);
    let record_count = a.op(
        ns.o,
        record_total,
        Opcode::IntAddChecked,
        vec![pav(base_length), pav(payload_width)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: record_total,
        function: fid,
        parameters: vec![
            base_length,
            payload_width,
            total_payload_length,
            total_subject,
            total_payload,
            total_unit,
        ],
        operations: vec![record_count],
        terminator: switch(
            op_result(record_count),
            vec![
                (
                    BuiltinCase::Ok,
                    encode_record_length,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(total_payload_length),
                        sav(total_subject),
                        sav(total_payload),
                        sav(total_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let record_count = a.param(ns.p, encode_record_length, ParameterRole::Block, u64_type());
    let encoded_payload_length = a.param(
        ns.p,
        encode_record_length,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let encoded_subject = a.param(
        ns.p,
        encode_record_length,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let encoded_payload = a.param(
        ns.p,
        encode_record_length,
        ParameterRole::Block,
        TypeExpr::Bytes,
    );
    let encoded_unit = a.param(
        ns.p,
        encode_record_length,
        ParameterRole::Block,
        TypeExpr::Unit,
    );
    let width = a.cref(ns.o, encode_record_length, width64, u32_type());
    let encoded_record_length = a.op(
        ns.o,
        encode_record_length,
        Opcode::CallDirect,
        vec![pav(record_count), op_result(width), pav(encoded_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: encode_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: encode_record_length,
        function: fid,
        parameters: vec![
            record_count,
            encoded_payload_length,
            encoded_subject,
            encoded_payload,
            encoded_unit,
        ],
        operations: vec![width, encoded_record_length],
        terminator: switch(
            op_result(encoded_record_length),
            vec![
                (
                    BuiltinCase::Ok,
                    wrap,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(encoded_payload_length),
                        sav(encoded_subject),
                        sav(encoded_payload),
                        sav(encoded_unit),
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
    let final_payload_length = a.param(ns.p, wrap, ParameterRole::Block, TypeExpr::Bytes);
    let final_subject = a.param(ns.p, wrap, ParameterRole::Block, TypeExpr::Bytes);
    let final_payload = a.param(ns.p, wrap, ParameterRole::Block, TypeExpr::Bytes);
    let wrap_unit = a.param(ns.p, wrap, ParameterRole::Block, TypeExpr::Unit);
    let tag = a.cref(ns.o, wrap, union_tag, TypeExpr::Bytes);
    let prefix = a.cref(ns.o, wrap, record_prefix, TypeExpr::Bytes);
    let second = a.cref(ns.o, wrap, field2, TypeExpr::Bytes);
    let final_parts = a.op(
        ns.o,
        wrap,
        Opcode::VectorNew,
        vec![
            op_result(tag),
            pav(record_length_bytes),
            op_result(prefix),
            pav(final_subject),
            op_result(second),
            pav(final_payload_length),
            pav(final_payload),
        ],
        vec![bytes_vector_type()],
        Immediate::None,
    );
    let wrapped = a.op(
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
        parameters: vec![
            record_length_bytes,
            final_payload_length,
            final_subject,
            final_payload,
            wrap_unit,
        ],
        operations: vec![tag, prefix, second, final_parts, wrapped],
        terminator: ret(op_result(wrapped)),
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
        parameters: vec![subject, set_payload, unit],
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

#[allow(clippy::too_many_lines)]
fn build_identity_set_payload_decode(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    wrap_fid: EntityId,
    policy_decode_fid: EntityId,
) -> FunctionGraph {
    let block_start = a.blocks.len();
    let result_type = entity_set_decode_result_type();
    let subject = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let payload = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let canonical_empty = a.kbytes(ns.k, &[0]);
    let empty_members = a.kbytes(ns.k, b"");
    let zero = a.ku64(ns.k, 0);
    let entry = a.id(ns.b);
    let empty = a.id(ns.b);
    let wrap = a.id(ns.b);
    let decode = a.id(ns.b);
    let forward_error = a.id(ns.b);
    let empty_constant = a.cref(ns.o, entry, canonical_empty, TypeExpr::Bytes);
    let is_empty = a.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(payload), op_result(empty_constant)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![empty_constant, is_empty],
        terminator: cond(
            op_result(is_empty),
            edge(empty, vec![pav(subject)]),
            edge(wrap, vec![pav(subject), pav(payload), pav(unit)]),
        ),
        reachability: Reachability::Required,
    });

    let empty_subject = a.param(ns.p, empty, ParameterRole::Block, TypeExpr::Bytes);
    let empty_bytes = a.cref(ns.o, empty, empty_members, TypeExpr::Bytes);
    let empty_count = a.cref(ns.o, empty, zero, u64_type());
    let empty_tuple = a.op(
        ns.o,
        empty,
        Opcode::TupleNew,
        vec![
            pav(empty_subject),
            op_result(empty_bytes),
            op_result(empty_count),
        ],
        vec![TypeExpr::Tuple(vec![
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            u64_type(),
        ])],
        Immediate::None,
    );
    let empty_ok = a.op(
        ns.o,
        empty,
        Opcode::ResultOk,
        vec![op_result(empty_tuple)],
        vec![result_type.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: empty,
        function: fid,
        parameters: vec![empty_subject],
        operations: vec![empty_bytes, empty_count, empty_tuple, empty_ok],
        terminator: ret(op_result(empty_ok)),
        reachability: Reachability::Required,
    });

    let wrap_subject = a.param(ns.p, wrap, ParameterRole::Block, TypeExpr::Bytes);
    let wrap_payload = a.param(ns.p, wrap, ParameterRole::Block, TypeExpr::Bytes);
    let wrap_unit = a.param(ns.p, wrap, ParameterRole::Block, TypeExpr::Unit);
    let wrapped = a.op(
        ns.o,
        wrap,
        Opcode::CallDirect,
        vec![pav(wrap_subject), pav(wrap_payload), pav(wrap_unit)],
        vec![encode_result_type()],
        Immediate::Function(FunctionRefValue {
            function: wrap_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: wrap,
        function: fid,
        parameters: vec![wrap_subject, wrap_payload, wrap_unit],
        operations: vec![wrapped],
        terminator: switch(
            op_result(wrapped),
            vec![
                (
                    BuiltinCase::Ok,
                    decode,
                    vec![SwitchArgument::CasePayload, sav(wrap_unit)],
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
    let wrapped_body = a.param(ns.p, decode, ParameterRole::Block, TypeExpr::Bytes);
    let decode_unit = a.param(ns.p, decode, ParameterRole::Block, TypeExpr::Unit);
    let decoded = a.op(
        ns.o,
        decode,
        Opcode::CallDirect,
        vec![pav(wrapped_body), pav(decode_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: policy_decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: decode,
        function: fid,
        parameters: vec![wrapped_body, decode_unit],
        operations: vec![decoded],
        terminator: ret(op_result(decoded)),
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
        parameters: vec![subject, payload, unit],
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

fn build_package_decode(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    vector_decoder: EntityId,
) -> FunctionGraph {
    let block_start = a.blocks.len();
    let result_type = dependency_binding_decode::package_decode_result_type();
    let body = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let resource_code = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let resource = err_block(a, ns, fid, result_type.clone(), resource_code);
    let entry = a.id(ns.b);
    let converted = a.id(ns.b);
    let vector = a.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(unit), pav(body)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![vector],
        terminator: switch(
            op_result(vector),
            vec![
                (
                    BuiltinCase::Ok,
                    converted,
                    vec![SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, resource, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let body_vector = a.param(ns.p, converted, ParameterRole::Block, u8vec_type());
    let decoded = a.op(
        ns.o,
        converted,
        Opcode::CallDirect,
        vec![pav(body), pav(body_vector), pav(unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: vector_decoder,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: converted,
        function: fid,
        parameters: vec![body_vector],
        operations: vec![decoded],
        terminator: ret(op_result(decoded)),
        reachability: Reachability::Required,
    });
    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![body, unit],
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

fn package_program_decode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![
            TypeExpr::Bytes,
            TypeExpr::Tuple(vec![
                TypeExpr::Bytes,
                TypeExpr::Bytes,
                TypeExpr::Bytes,
                u64_type(),
                TypeExpr::Bytes,
                u64_type(),
            ]),
        ])),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(clippy::too_many_lines)]
fn build_package_program_decode(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    validate_fid: EntityId,
    outer_fid: EntityId,
    package_fid: EntityId,
) -> FunctionGraph {
    let block_start = a.blocks.len();
    let result_type = package_program_decode_result_type();
    let envelope_type = encode_result_type();
    let outer_type = outer_decode_result_type();
    let package_type = dependency_binding_decode::package_decode_result_type();
    let stored = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let entry = a.id(ns.b);
    let envelope_ok = a.id(ns.b);
    let outer_ok = a.id(ns.b);
    let package_ok = a.id(ns.b);
    let forward_error = a.id(ns.b);

    let decoded_envelope = a.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(stored), pav(unit)],
        vec![envelope_type],
        Immediate::Function(FunctionRefValue {
            function: validate_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![decoded_envelope],
        terminator: switch(
            op_result(decoded_envelope),
            vec![
                (
                    BuiltinCase::Ok,
                    envelope_ok,
                    vec![SwitchArgument::CasePayload, sav(unit)],
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

    let payload = a.param(ns.p, envelope_ok, ParameterRole::Block, TypeExpr::Bytes);
    let envelope_unit = a.param(ns.p, envelope_ok, ParameterRole::Block, TypeExpr::Unit);
    let decoded_outer = a.op(
        ns.o,
        envelope_ok,
        Opcode::CallDirect,
        vec![pav(payload), pav(envelope_unit)],
        vec![outer_type],
        Immediate::Function(FunctionRefValue {
            function: outer_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: envelope_ok,
        function: fid,
        parameters: vec![payload, envelope_unit],
        operations: vec![decoded_outer],
        terminator: switch(
            op_result(decoded_outer),
            vec![
                (
                    BuiltinCase::Ok,
                    outer_ok,
                    vec![SwitchArgument::CasePayload, sav(envelope_unit)],
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

    let outer_tuple = a.param(
        ns.p,
        outer_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]),
    );
    let outer_unit = a.param(ns.p, outer_ok, ParameterRole::Block, TypeExpr::Unit);
    let body = a.op(
        ns.o,
        outer_ok,
        Opcode::TupleGet,
        vec![pav(outer_tuple)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let decoded_package = a.op(
        ns.o,
        outer_ok,
        Opcode::CallDirect,
        vec![op_result(body), pav(outer_unit)],
        vec![package_type],
        Immediate::Function(FunctionRefValue {
            function: package_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: outer_ok,
        function: fid,
        parameters: vec![outer_tuple, outer_unit],
        operations: vec![body, decoded_package],
        terminator: switch(
            op_result(decoded_package),
            vec![
                (
                    BuiltinCase::Ok,
                    package_ok,
                    vec![SwitchArgument::CasePayload, sav(outer_tuple)],
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

    let package_tuple_type = TypeExpr::Tuple(vec![
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        TypeExpr::Bytes,
        u64_type(),
        TypeExpr::Bytes,
        u64_type(),
    ]);
    let package_tuple = a.param(ns.p, package_ok, ParameterRole::Block, package_tuple_type);
    let successful_outer = a.param(
        ns.p,
        package_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]),
    );
    let package_entity = a.op(
        ns.o,
        package_ok,
        Opcode::TupleGet,
        vec![pav(successful_outer)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let joined = a.op(
        ns.o,
        package_ok,
        Opcode::TupleNew,
        vec![op_result(package_entity), pav(package_tuple)],
        vec![TypeExpr::Tuple(vec![
            TypeExpr::Bytes,
            TypeExpr::Tuple(vec![
                TypeExpr::Bytes,
                TypeExpr::Bytes,
                TypeExpr::Bytes,
                u64_type(),
                TypeExpr::Bytes,
                u64_type(),
            ]),
        ])],
        Immediate::None,
    );
    let ok = a.op(
        ns.o,
        package_ok,
        Opcode::ResultOk,
        vec![op_result(joined)],
        vec![result_type.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: package_ok,
        function: fid,
        parameters: vec![package_tuple, successful_outer],
        operations: vec![package_entity, joined, ok],
        terminator: ret(op_result(ok)),
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
        parameters: vec![stored, unit],
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

#[allow(clippy::similar_names, clippy::too_many_lines)]
fn build_empty_package_program_encode(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    exact_fid: EntityId,
    concat_fid: EntityId,
) -> FunctionGraph {
    use sley_vm::host_abi::BRIDGE_CODE_RHW1;

    let block_start = a.blocks.len();
    let result_type = encode_result_type();
    let mut prefix_bytes = b"SLEYSCB1".to_vec();
    prefix_bytes.extend_from_slice(&sley_scb1::encode_uvar(1));
    prefix_bytes.extend_from_slice(&sley_scb1::encode_uvar(200));
    prefix_bytes.extend_from_slice(&[9; 32]);
    prefix_bytes.extend_from_slice(&sley_scb1::encode_uvar(114));
    prefix_bytes.extend_from_slice(&[2, 1, 32]);
    let prefix = a.kbytes(ns.k, &prefix_bytes);
    let entity_tail = a.kbytes(ns.k, &[2, 77, 2, 75, 4, 1, 32]);
    let root_prefix = a.kbytes(ns.k, &[2, 32]);
    let suffix = a.kbytes(ns.k, &[3, 1, 0, 4, 1, 0]);
    let object_domain = a.kbytes(ns.k, b"sley2.object.v1");
    let resource_code = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let entity = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let workspace = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let root_namespace = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let entry = a.id(ns.b);
    let validate_root = a.id(ns.b);
    let validate_entity = a.id(ns.b);
    let compose = a.id(ns.b);
    let hash = a.id(ns.b);
    let rebuild = a.id(ns.b);
    let forward_error = a.id(ns.b);
    let resource = err_block(a, ns, fid, result_type.clone(), resource_code);

    let checked_workspace = a.op(
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
        operations: vec![checked_workspace],
        terminator: switch(
            op_result(checked_workspace),
            vec![
                (
                    BuiltinCase::Ok,
                    validate_root,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(entity),
                        sav(root_namespace),
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

    let valid_workspace = a.param(ns.p, validate_root, ParameterRole::Block, TypeExpr::Bytes);
    let pending_entity = a.param(ns.p, validate_root, ParameterRole::Block, TypeExpr::Bytes);
    let pending_root = a.param(ns.p, validate_root, ParameterRole::Block, TypeExpr::Bytes);
    let root_unit = a.param(ns.p, validate_root, ParameterRole::Block, TypeExpr::Unit);
    let checked_root = a.op(
        ns.o,
        validate_root,
        Opcode::CallDirect,
        vec![pav(pending_root), pav(root_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: exact_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: validate_root,
        function: fid,
        parameters: vec![valid_workspace, pending_entity, pending_root, root_unit],
        operations: vec![checked_root],
        terminator: switch(
            op_result(checked_root),
            vec![
                (
                    BuiltinCase::Ok,
                    validate_entity,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(valid_workspace),
                        sav(pending_entity),
                        sav(root_unit),
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

    let valid_root = a.param(ns.p, validate_entity, ParameterRole::Block, TypeExpr::Bytes);
    let entity_workspace = a.param(ns.p, validate_entity, ParameterRole::Block, TypeExpr::Bytes);
    let unchecked_entity = a.param(ns.p, validate_entity, ParameterRole::Block, TypeExpr::Bytes);
    let entity_unit = a.param(ns.p, validate_entity, ParameterRole::Block, TypeExpr::Unit);
    let checked_entity = a.op(
        ns.o,
        validate_entity,
        Opcode::CallDirect,
        vec![pav(unchecked_entity), pav(entity_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: exact_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: validate_entity,
        function: fid,
        parameters: vec![valid_root, entity_workspace, unchecked_entity, entity_unit],
        operations: vec![checked_entity],
        terminator: switch(
            op_result(checked_entity),
            vec![
                (
                    BuiltinCase::Ok,
                    compose,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(entity_workspace),
                        sav(valid_root),
                        sav(entity_unit),
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

    let valid_entity = a.param(ns.p, compose, ParameterRole::Block, TypeExpr::Bytes);
    let final_workspace = a.param(ns.p, compose, ParameterRole::Block, TypeExpr::Bytes);
    let final_root = a.param(ns.p, compose, ParameterRole::Block, TypeExpr::Bytes);
    let compose_unit = a.param(ns.p, compose, ParameterRole::Block, TypeExpr::Unit);
    let domain = a.cref(ns.o, compose, object_domain, TypeExpr::Bytes);
    let prefix_value = a.cref(ns.o, compose, prefix, TypeExpr::Bytes);
    let entity_tail_value = a.cref(ns.o, compose, entity_tail, TypeExpr::Bytes);
    let root_prefix_value = a.cref(ns.o, compose, root_prefix, TypeExpr::Bytes);
    let suffix_value = a.cref(ns.o, compose, suffix, TypeExpr::Bytes);
    let parts = a.op(
        ns.o,
        compose,
        Opcode::VectorNew,
        vec![
            op_result(domain),
            op_result(prefix_value),
            pav(valid_entity),
            op_result(entity_tail_value),
            pav(final_workspace),
            op_result(root_prefix_value),
            pav(final_root),
            op_result(suffix_value),
        ],
        vec![bytes_vector_type()],
        Immediate::None,
    );
    let hash_input = a.op(
        ns.o,
        compose,
        Opcode::CallDirect,
        vec![op_result(parts), pav(compose_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: concat_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: compose,
        function: fid,
        parameters: vec![valid_entity, final_workspace, final_root, compose_unit],
        operations: vec![
            domain,
            prefix_value,
            entity_tail_value,
            root_prefix_value,
            suffix_value,
            parts,
            hash_input,
        ],
        terminator: switch(
            op_result(hash_input),
            vec![
                (
                    BuiltinCase::Ok,
                    hash,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(valid_entity),
                        sav(final_workspace),
                        sav(final_root),
                        sav(compose_unit),
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

    let digest_input = a.param(ns.p, hash, ParameterRole::Block, TypeExpr::Bytes);
    let retained_entity = a.param(ns.p, hash, ParameterRole::Block, TypeExpr::Bytes);
    let retained_workspace = a.param(ns.p, hash, ParameterRole::Block, TypeExpr::Bytes);
    let retained_root = a.param(ns.p, hash, ParameterRole::Block, TypeExpr::Bytes);
    let hash_unit = a.param(ns.p, hash, ParameterRole::Block, TypeExpr::Unit);
    let digest = a.op(
        ns.o,
        hash,
        Opcode::AdapterInvoke,
        vec![pav(hash_unit), pav(digest_input)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_RHW1))),
    );
    a.blocks.push(Block {
        entity_id: hash,
        function: fid,
        parameters: vec![
            digest_input,
            retained_entity,
            retained_workspace,
            retained_root,
            hash_unit,
        ],
        operations: vec![digest],
        terminator: switch(
            op_result(digest),
            vec![
                (
                    BuiltinCase::Ok,
                    rebuild,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(retained_entity),
                        sav(retained_workspace),
                        sav(retained_root),
                        sav(hash_unit),
                    ],
                ),
                (BuiltinCase::Err, resource, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let retained_digest = a.param(ns.p, rebuild, ParameterRole::Block, TypeExpr::Bytes);
    let rebuild_entity = a.param(ns.p, rebuild, ParameterRole::Block, TypeExpr::Bytes);
    let rebuild_workspace = a.param(ns.p, rebuild, ParameterRole::Block, TypeExpr::Bytes);
    let rebuild_root = a.param(ns.p, rebuild, ParameterRole::Block, TypeExpr::Bytes);
    let rebuild_unit = a.param(ns.p, rebuild, ParameterRole::Block, TypeExpr::Unit);
    let rebuild_prefix = a.cref(ns.o, rebuild, prefix, TypeExpr::Bytes);
    let rebuild_entity_tail = a.cref(ns.o, rebuild, entity_tail, TypeExpr::Bytes);
    let rebuild_root_prefix = a.cref(ns.o, rebuild, root_prefix, TypeExpr::Bytes);
    let rebuild_suffix = a.cref(ns.o, rebuild, suffix, TypeExpr::Bytes);
    let rebuild_parts = a.op(
        ns.o,
        rebuild,
        Opcode::VectorNew,
        vec![
            op_result(rebuild_prefix),
            pav(rebuild_entity),
            op_result(rebuild_entity_tail),
            pav(rebuild_workspace),
            op_result(rebuild_root_prefix),
            pav(rebuild_root),
            op_result(rebuild_suffix),
            pav(retained_digest),
        ],
        vec![bytes_vector_type()],
        Immediate::None,
    );
    let stored = a.op(
        ns.o,
        rebuild,
        Opcode::CallDirect,
        vec![op_result(rebuild_parts), pav(rebuild_unit)],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: concat_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: rebuild,
        function: fid,
        parameters: vec![
            retained_digest,
            rebuild_entity,
            rebuild_workspace,
            rebuild_root,
            rebuild_unit,
        ],
        operations: vec![
            rebuild_prefix,
            rebuild_entity_tail,
            rebuild_root_prefix,
            rebuild_suffix,
            rebuild_parts,
            stored,
        ],
        terminator: ret(op_result(stored)),
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
        parameters: vec![entity, workspace, root_namespace, unit],
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

#[allow(clippy::too_many_lines)]
fn build_package_program_encode_dispatch(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    generic_fid: EntityId,
    empty_fid: EntityId,
) -> FunctionGraph {
    let block_start = a.blocks.len();
    let result_type = encode_result_type();
    let empty_bytes = a.kbytes(ns.k, b"");
    let entity = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let workspace = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let root_namespace = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let dependencies = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let exports = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let entry = a.id(ns.b);
    let check_exports = a.id(ns.b);
    let encode_empty = a.id(ns.b);
    let encode_generic = a.id(ns.b);

    let empty = a.cref(ns.o, entry, empty_bytes, TypeExpr::Bytes);
    let dependencies_empty = a.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(dependencies), op_result(empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let all_arguments = vec![
        pav(entity),
        pav(workspace),
        pav(root_namespace),
        pav(dependencies),
        pav(exports),
        pav(unit),
    ];
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![empty, dependencies_empty],
        terminator: cond(
            op_result(dependencies_empty),
            edge(check_exports, all_arguments.clone()),
            edge(encode_generic, all_arguments),
        ),
        reachability: Reachability::Required,
    });

    let checked_entity = a.param(ns.p, check_exports, ParameterRole::Block, TypeExpr::Bytes);
    let checked_workspace = a.param(ns.p, check_exports, ParameterRole::Block, TypeExpr::Bytes);
    let checked_root = a.param(ns.p, check_exports, ParameterRole::Block, TypeExpr::Bytes);
    let checked_dependencies = a.param(ns.p, check_exports, ParameterRole::Block, TypeExpr::Bytes);
    let checked_exports = a.param(ns.p, check_exports, ParameterRole::Block, TypeExpr::Bytes);
    let checked_unit = a.param(ns.p, check_exports, ParameterRole::Block, TypeExpr::Unit);
    let empty = a.cref(ns.o, check_exports, empty_bytes, TypeExpr::Bytes);
    let exports_empty = a.op(
        ns.o,
        check_exports,
        Opcode::Equal,
        vec![pav(checked_exports), op_result(empty)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: check_exports,
        function: fid,
        parameters: vec![
            checked_entity,
            checked_workspace,
            checked_root,
            checked_dependencies,
            checked_exports,
            checked_unit,
        ],
        operations: vec![empty, exports_empty],
        terminator: cond(
            op_result(exports_empty),
            edge(
                encode_empty,
                vec![
                    pav(checked_entity),
                    pav(checked_workspace),
                    pav(checked_root),
                    pav(checked_unit),
                ],
            ),
            edge(
                encode_generic,
                vec![
                    pav(checked_entity),
                    pav(checked_workspace),
                    pav(checked_root),
                    pav(checked_dependencies),
                    pav(checked_exports),
                    pav(checked_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    let empty_entity = a.param(ns.p, encode_empty, ParameterRole::Block, TypeExpr::Bytes);
    let empty_workspace = a.param(ns.p, encode_empty, ParameterRole::Block, TypeExpr::Bytes);
    let empty_root = a.param(ns.p, encode_empty, ParameterRole::Block, TypeExpr::Bytes);
    let empty_unit = a.param(ns.p, encode_empty, ParameterRole::Block, TypeExpr::Unit);
    let empty_result = a.op(
        ns.o,
        encode_empty,
        Opcode::CallDirect,
        vec![
            pav(empty_entity),
            pav(empty_workspace),
            pav(empty_root),
            pav(empty_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: empty_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: encode_empty,
        function: fid,
        parameters: vec![empty_entity, empty_workspace, empty_root, empty_unit],
        operations: vec![empty_result],
        terminator: ret(op_result(empty_result)),
        reachability: Reachability::Required,
    });

    let generic_entity = a.param(ns.p, encode_generic, ParameterRole::Block, TypeExpr::Bytes);
    let generic_workspace = a.param(ns.p, encode_generic, ParameterRole::Block, TypeExpr::Bytes);
    let generic_root = a.param(ns.p, encode_generic, ParameterRole::Block, TypeExpr::Bytes);
    let generic_dependencies = a.param(ns.p, encode_generic, ParameterRole::Block, TypeExpr::Bytes);
    let generic_exports = a.param(ns.p, encode_generic, ParameterRole::Block, TypeExpr::Bytes);
    let generic_unit = a.param(ns.p, encode_generic, ParameterRole::Block, TypeExpr::Unit);
    let generic_result = a.op(
        ns.o,
        encode_generic,
        Opcode::CallDirect,
        vec![
            pav(generic_entity),
            pav(generic_workspace),
            pav(generic_root),
            pav(generic_dependencies),
            pav(generic_exports),
            pav(generic_unit),
        ],
        vec![result_type.clone()],
        Immediate::Function(FunctionRefValue {
            function: generic_fid,
            type_arguments: Vec::new(),
        }),
    );
    a.blocks.push(Block {
        entity_id: encode_generic,
        function: fid,
        parameters: vec![
            generic_entity,
            generic_workspace,
            generic_root,
            generic_dependencies,
            generic_exports,
            generic_unit,
        ],
        operations: vec![generic_result],
        terminator: ret(op_result(generic_result)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![
            entity,
            workspace,
            root_namespace,
            dependencies,
            exports,
            unit,
        ],
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

#[allow(clippy::too_many_lines)]
fn package_program_encode_image() -> Image {
    use sley_vm::host_abi::BRIDGE_CODE_RHW1;

    let mut assembler = Asm::new();
    let encode_fid = eid(90, 1);
    let set_fid = eid(90, 2);
    let concat_fid = eid(90, 3);
    let body_compose_fid = eid(90, 4);
    let exact_fid = eid(90, 5);
    let package_fid = eid(90, 6);
    let outer_fid = eid(90, 7);
    let build_fid = eid(90, 8);
    let envelope_fid = eid(90, 9);
    let payload_fid = eid(90, 10);
    let program_fid = eid(90, 11);
    let empty_program_fid = eid(90, 12);
    let dispatch_fid = eid(90, 13);
    let encode_graph = build_encode(
        &mut assembler,
        Ns {
            k: 10,
            p: 11,
            b: 12,
            o: 13,
        },
        encode_fid,
    );
    let set_graph = build_identity_set_encode(
        &mut assembler,
        Ns {
            k: 14,
            p: 15,
            b: 16,
            o: 17,
        },
        set_fid,
        encode_fid,
    );
    let concat_graph = build_concat_bytes(
        &mut assembler,
        Ns {
            k: 18,
            p: 19,
            b: 20,
            o: 21,
        },
        concat_fid,
    );
    let body_compose_graph = build_package_compose(
        &mut assembler,
        Ns {
            k: 22,
            p: 23,
            b: 24,
            o: 25,
        },
        body_compose_fid,
        encode_fid,
        concat_fid,
    );
    let exact_graph = build_exact_identity_validate(
        &mut assembler,
        Ns {
            k: 26,
            p: 27,
            b: 28,
            o: 29,
        },
        exact_fid,
    );
    let package_graph = build_package_encode(
        &mut assembler,
        Ns {
            k: 30,
            p: 31,
            b: 32,
            o: 33,
        },
        package_fid,
        exact_fid,
        set_fid,
        body_compose_fid,
    );
    let outer_graph = build_outer_encode(
        &mut assembler,
        Ns {
            k: 34,
            p: 35,
            b: 36,
            o: 37,
        },
        outer_fid,
        encode_fid,
    );
    let build_graph = build_program_build(
        &mut assembler,
        Ns {
            k: 38,
            p: 39,
            b: 40,
            o: 41,
        },
        build_fid,
    );
    let envelope_graph = build_program_envelope_encode_with_mode(
        &mut assembler,
        Ns {
            k: 42,
            p: 43,
            b: 44,
            o: 45,
        },
        envelope_fid,
        build_fid,
        encode_fid,
        DigestCopyMode::Unrolled,
    );
    let payload_graph = build_program_payload_encode(
        &mut assembler,
        Ns {
            k: 46,
            p: 47,
            b: 48,
            o: 49,
        },
        payload_fid,
        outer_fid,
        envelope_fid,
    );
    let program_graph = build_program_value_encode(
        &mut assembler,
        Ns {
            k: 50,
            p: 51,
            b: 52,
            o: 53,
        },
        program_fid,
        package_fid,
        payload_fid,
        &[
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            TypeExpr::Bytes,
        ],
    );
    let empty_program_graph = build_empty_package_program_encode(
        &mut assembler,
        Ns {
            k: 54,
            p: 55,
            b: 56,
            o: 57,
        },
        empty_program_fid,
        exact_fid,
        concat_fid,
    );
    let dispatch_graph = build_package_program_encode_dispatch(
        &mut assembler,
        Ns {
            k: 58,
            p: 59,
            b: 60,
            o: 61,
        },
        dispatch_fid,
        program_fid,
        empty_program_fid,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: dispatch_graph.clone(),
        functions: vec![
            dispatch_graph,
            empty_program_graph,
            program_graph,
            payload_graph,
            envelope_graph,
            package_graph,
            exact_graph,
            body_compose_graph,
            concat_graph,
            set_graph,
            outer_graph,
            build_graph,
            encode_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
            frozen_import(BRIDGE_CODE_RHW1, TypeExpr::Bytes, TypeExpr::Bytes),
        ],
        constants: assembler.constants,
    }
}

#[allow(clippy::too_many_lines)]
fn package_decode_image() -> Image {
    let mut assembler = Asm::new();
    let decode_fid = eid(13, 1);
    let policy_fid = eid(13, 2);
    let encode_fid = eid(13, 3);
    let concat_fid = eid(13, 4);
    let wrap_fid = eid(13, 5);
    let set_fid = eid(13, 6);
    let core_fid = eid(13, 7);
    let package_fid = eid(13, 8);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 130,
            p: 131,
            b: 132,
            o: 133,
        },
        decode_fid,
    );
    let policy_graph = build_policy_binding_decode(
        &mut assembler,
        Ns {
            k: 134,
            p: 135,
            b: 136,
            o: 137,
        },
        policy_fid,
        decode_fid,
    );
    let encode_graph = build_encode(
        &mut assembler,
        Ns {
            k: 138,
            p: 139,
            b: 140,
            o: 141,
        },
        encode_fid,
    );
    let concat_graph = build_concat_bytes(
        &mut assembler,
        Ns {
            k: 142,
            p: 143,
            b: 144,
            o: 145,
        },
        concat_fid,
    );
    let wrap_graph = build_policy_set_payload_wrap(
        &mut assembler,
        Ns {
            k: 146,
            p: 147,
            b: 148,
            o: 149,
        },
        wrap_fid,
        encode_fid,
        concat_fid,
    );
    let set_graph = build_identity_set_payload_decode(
        &mut assembler,
        Ns {
            k: 150,
            p: 151,
            b: 152,
            o: 153,
        },
        set_fid,
        wrap_fid,
        policy_fid,
    );
    let core_graph = dependency_binding_decode::build_package_decode_from_vector(
        &mut assembler,
        Ns {
            k: 154,
            p: 155,
            b: 156,
            o: 157,
        },
        core_fid,
        decode_fid,
        set_fid,
    );
    let package_graph = build_package_decode(
        &mut assembler,
        Ns {
            k: 158,
            p: 159,
            b: 160,
            o: 161,
        },
        package_fid,
        core_fid,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: package_graph.clone(),
        functions: vec![
            package_graph,
            core_graph,
            set_graph,
            wrap_graph,
            concat_graph,
            policy_graph,
            encode_graph,
            decode_graph,
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

#[allow(clippy::too_many_lines)]
fn package_program_decode_image() -> Image {
    use sley_vm::host_abi::BRIDGE_CODE_RHW1;

    let mut assembler = Asm::new();
    let decode_fid = eid(14, 1);
    let validate_fid = eid(14, 2);
    let outer_fid = eid(14, 3);
    let policy_fid = eid(14, 4);
    let encode_fid = eid(14, 5);
    let concat_fid = eid(14, 6);
    let wrap_fid = eid(14, 7);
    let set_fid = eid(14, 8);
    let core_fid = eid(14, 9);
    let package_fid = eid(14, 10);
    let program_fid = eid(14, 11);
    let (decode_graph, _) = build_decode(
        &mut assembler,
        Ns {
            k: 162,
            p: 163,
            b: 164,
            o: 165,
        },
        decode_fid,
    );
    let validate_graph = build_program_validate(
        &mut assembler,
        Ns {
            k: 166,
            p: 167,
            b: 168,
            o: 169,
        },
        validate_fid,
        decode_fid,
    );
    let outer_graph = build_outer_decode(
        &mut assembler,
        Ns {
            k: 170,
            p: 171,
            b: 172,
            o: 173,
        },
        outer_fid,
        decode_fid,
    );
    let policy_graph = build_policy_binding_decode(
        &mut assembler,
        Ns {
            k: 174,
            p: 175,
            b: 176,
            o: 177,
        },
        policy_fid,
        decode_fid,
    );
    let encode_graph = build_encode(
        &mut assembler,
        Ns {
            k: 178,
            p: 179,
            b: 180,
            o: 181,
        },
        encode_fid,
    );
    let concat_graph = build_concat_bytes(
        &mut assembler,
        Ns {
            k: 182,
            p: 183,
            b: 184,
            o: 185,
        },
        concat_fid,
    );
    let wrap_graph = build_policy_set_payload_wrap(
        &mut assembler,
        Ns {
            k: 186,
            p: 187,
            b: 188,
            o: 189,
        },
        wrap_fid,
        encode_fid,
        concat_fid,
    );
    let set_graph = build_identity_set_payload_decode(
        &mut assembler,
        Ns {
            k: 190,
            p: 191,
            b: 192,
            o: 193,
        },
        set_fid,
        wrap_fid,
        policy_fid,
    );
    let core_graph = dependency_binding_decode::build_package_decode_from_vector(
        &mut assembler,
        Ns {
            k: 194,
            p: 195,
            b: 196,
            o: 197,
        },
        core_fid,
        decode_fid,
        set_fid,
    );
    let package_graph = build_package_decode(
        &mut assembler,
        Ns {
            k: 198,
            p: 199,
            b: 200,
            o: 201,
        },
        package_fid,
        core_fid,
    );
    let program_graph = build_package_program_decode(
        &mut assembler,
        Ns {
            k: 202,
            p: 203,
            b: 204,
            o: 205,
        },
        program_fid,
        validate_fid,
        outer_fid,
        package_fid,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: program_graph.clone(),
        functions: vec![
            program_graph,
            validate_graph,
            outer_graph,
            package_graph,
            core_graph,
            set_graph,
            wrap_graph,
            concat_graph,
            policy_graph,
            encode_graph,
            decode_graph,
        ],
        parameters: assembler.parameters,
        blocks: assembler.blocks,
        operations: assembler.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
            frozen_import(BRIDGE_CODE_RHW1, TypeExpr::Bytes, TypeExpr::Bytes),
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

fn package_program_encode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    entity: &[u8],
    workspace: &[u8],
    root_namespace: &[u8],
    dependencies: &[u8],
    exports: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![
            bytes_input(entity),
            bytes_input(workspace),
            bytes_input(root_namespace),
            bytes_input(dependencies),
            bytes_input(exports),
            unit_input(),
        ],
    )
}

fn package_decode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    body: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(package, approved, vec![bytes_input(body), unit_input()])
}

fn package_program_decode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    stored: &[u8],
) -> sley_vm::ExecutionOutcome {
    execute(package, approved, vec![bytes_input(stored), unit_input()])
}

pub(super) fn package_stored(
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

fn raw_package_body(
    workspace: Vec<u8>,
    root_namespace: Vec<u8>,
    dependencies: &[Vec<u8>],
    exports: &[Vec<u8>],
) -> Vec<u8> {
    let dependencies = sley_scb1::encode_list(dependencies).expect("dependencies list");
    let exports = sley_scb1::encode_list(exports).expect("exports list");
    let record = sley_scb1::encode_record(&[
        (1, workspace),
        (2, root_namespace),
        (3, dependencies),
        (4, exports),
    ])
    .expect("Package record");
    sley_scb1::encode_union(2, &record).expect("Package union")
}

fn assert_package_decode_ok(
    outcome: &sley_vm::ExecutionOutcome,
    workspace: &[u8; 32],
    root_namespace: &[u8; 32],
    dependencies: &[[u8; 32]],
    exports: &[[u8; 32]],
) {
    let sley_vm::ExecutionTermination::Success(found) = &outcome.termination else {
        panic!("Package decode must return: {:?}", outcome.termination)
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &found.data else {
        panic!("Package decode must succeed: {:?}", found.data)
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("Package result must be a tuple: {:?}", payload.data)
    };
    assert_eq!(fields.len(), 6, "Package tuple has six values");
    assert_eq!(fields[0].data, ConstData::Bytes(workspace.to_vec()));
    assert_eq!(fields[1].data, ConstData::Bytes(root_namespace.to_vec()));
    assert_eq!(fields[2].data, ConstData::Bytes(concat_ids(dependencies)));
    assert_eq!(
        fields[3].data,
        ConstData::UInt(u128::try_from(dependencies.len()).expect("dependency count fits u128"))
    );
    assert_eq!(fields[4].data, ConstData::Bytes(concat_ids(exports)));
    assert_eq!(
        fields[5].data,
        ConstData::UInt(u128::try_from(exports.len()).expect("export count fits u128"))
    );
}

fn assert_package_program_decode_ok(
    outcome: &sley_vm::ExecutionOutcome,
    entity: &[u8; 32],
    workspace: &[u8; 32],
    root_namespace: &[u8; 32],
    dependencies: &[[u8; 32]],
    exports: &[[u8; 32]],
) {
    let sley_vm::ExecutionTermination::Success(found) = &outcome.termination else {
        panic!(
            "Package program decode must return: {:?}",
            outcome.termination
        )
    };
    let ConstData::Result(ResultConst::Ok(payload)) = &found.data else {
        panic!("Package program decode must succeed: {:?}", found.data)
    };
    let ConstData::Sequence(fields) = &payload.data else {
        panic!("Package program result must be a tuple: {:?}", payload.data)
    };
    assert_eq!(fields.len(), 2, "Package program tuple has entity and body");
    assert_eq!(fields[0].data, ConstData::Bytes(entity.to_vec()));
    let ConstData::Sequence(body_fields) = &fields[1].data else {
        panic!("Package program body must be a tuple: {:?}", fields[1].data)
    };
    assert_eq!(body_fields.len(), 6, "Package body tuple has six values");
    assert_eq!(body_fields[0].data, ConstData::Bytes(workspace.to_vec()));
    assert_eq!(
        body_fields[1].data,
        ConstData::Bytes(root_namespace.to_vec())
    );
    assert_eq!(
        body_fields[2].data,
        ConstData::Bytes(concat_ids(dependencies))
    );
    assert_eq!(
        body_fields[3].data,
        ConstData::UInt(u128::try_from(dependencies.len()).expect("dependency count fits u128"))
    );
    assert_eq!(body_fields[4].data, ConstData::Bytes(concat_ids(exports)));
    assert_eq!(
        body_fields[5].data,
        ConstData::UInt(u128::try_from(exports.len()).expect("export count fits u128"))
    );
}

#[test]
fn package_decode_matches_native_semantics() {
    let (package, approved) = admit(&package_decode_image());
    let increasing = std::array::from_fn(|index| u8::try_from(index).expect("index fits u8"));
    let decreasing = std::array::from_fn(|index| u8::try_from(31 - index).expect("index fits u8"));
    let cases = [
        ([2; 32], [3; 32], Vec::new(), Vec::new()),
        ([2; 32], [3; 32], vec![[4; 32]], vec![[5; 32]]),
        ([2; 32], [3; 32], vec![[4; 32]], vec![[5; 32], [6; 32]]),
        (increasing, decreasing, vec![[4; 32]], vec![[5; 32]]),
    ];
    for (workspace, root_namespace, dependencies, exports) in cases {
        let body = package_body(workspace, root_namespace, &dependencies, &exports);
        let outcome = package_decode_call(&package, &approved, &body);
        assert_package_decode_ok(
            &outcome,
            &workspace,
            &root_namespace,
            &dependencies,
            &exports,
        );
        eprintln!(
            "PACKAGE_DEC body{}B deps={} exports={} fuel={} instr={} peak={}",
            body.len(),
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
fn package_decode_matches_native_rejection_precedence() {
    let (package, approved) = admit(&package_decode_image());
    let empty = package_body([2; 32], [3; 32], &[], &[]);
    let valid = vec![4; 32];
    let duplicate = vec![vec![4; 32], vec![4; 32]];
    let unordered = vec![vec![5; 32], vec![4; 32]];
    let mut vectors = Vec::<(&str, Vec<u8>)>::new();

    for (name, offset, byte) in [
        ("count_missing", 2usize, 3u8),
        ("count_unknown", 2, 5),
        ("field1_unknown", 3, 2),
        ("field1_above_schema", 3, 5),
        ("field2_duplicate", 37, 1),
        ("field2_order", 37, 0),
        ("field3_duplicate", 71, 2),
        ("field3_order", 71, 1),
        ("field4_duplicate", 74, 3),
        ("field4_order", 74, 2),
        ("field4_unknown", 74, 5),
    ] {
        let mut body = empty.clone();
        body[offset] = byte;
        vectors.push((name, body));
    }
    vectors.extend([
        (
            "workspace_short",
            raw_package_body(vec![2; 31], vec![3; 32], &[], &[]),
        ),
        (
            "workspace_long",
            raw_package_body(vec![2; 33], vec![3; 32], &[], &[]),
        ),
        (
            "root_short",
            raw_package_body(vec![2; 32], vec![3; 31], &[], &[]),
        ),
        (
            "root_long",
            raw_package_body(vec![2; 32], vec![3; 33], &[], &[]),
        ),
        (
            "dependencies_partial",
            raw_package_body(vec![2; 32], vec![3; 32], &[vec![4; 31]], &[]),
        ),
        (
            "dependencies_duplicate",
            raw_package_body(vec![2; 32], vec![3; 32], &duplicate, &[]),
        ),
        (
            "dependencies_order",
            raw_package_body(vec![2; 32], vec![3; 32], &unordered, &[]),
        ),
        (
            "exports_partial",
            raw_package_body(vec![2; 32], vec![3; 32], &[], &[vec![4; 31]]),
        ),
        (
            "exports_duplicate",
            raw_package_body(vec![2; 32], vec![3; 32], &[], &duplicate),
        ),
        (
            "exports_order",
            raw_package_body(vec![2; 32], vec![3; 32], &[], &unordered),
        ),
    ]);
    let mut dependencies_empty_trailing = ns_splice(&empty, 74, 0, &[0]);
    dependencies_empty_trailing[1] = 76;
    dependencies_empty_trailing[72] = 2;
    vectors.push(("dependencies_empty_trailing", dependencies_empty_trailing));
    let mut dependency_before_field4 = raw_package_body(
        vec![2; 32],
        vec![3; 32],
        &duplicate,
        std::slice::from_ref(&valid),
    );
    let field4_offset = dependency_before_field4.len() - 36;
    dependency_before_field4[field4_offset] = 3;
    vectors.push(("dependencies_before_field4", dependency_before_field4));
    vectors.extend([
        ("union_tag_nonminimal", ns_splice(&empty, 0, 1, &[0x82, 0])),
        ("count_nonminimal", ns_splice(&empty, 2, 1, &[0x84, 0])),
        (
            "dependencies_length_nonminimal",
            ns_splice(&empty, 72, 1, &[0x81, 0]),
        ),
        (
            "dependencies_count_nonminimal",
            ns_splice(&empty, 73, 1, &[0x80, 0]),
        ),
    ]);
    let mut union_overrun = empty.clone();
    union_overrun[1] = 76;
    vectors.push(("union_overrun", union_overrun));
    let mut union_underrun = empty.clone();
    union_underrun[1] = 74;
    vectors.push(("union_underrun", union_underrun));
    let mut trailing = empty.clone();
    trailing.push(0);
    vectors.push(("trailing", trailing));

    for (name, body) in vectors {
        let expected = program_native_code(&ns_wrap_body(0xd2, &body));
        let outcome = package_decode_call(&package, &approved, &body);
        assert_refusal(&outcome, &expected);
        eprintln!("PACKAGE_DEC_REJECT {name}->{expected}");
    }
}

#[test]
fn package_decode_fails_closed_on_other_union_kinds() {
    let (package, approved) = admit(&package_decode_image());
    let base = package_body([2; 32], [3; 32], &[], &[]);
    for (tag, code) in [
        (0, "SCB_UNION_INVALID"),
        (1, "SSMC_RESERVED_FIELD_PRESENT"),
        (3, "SSMC_RESERVED_FIELD_PRESENT"),
        (18, "SSMC_RESERVED_FIELD_PRESENT"),
        (19, "SCB_UNION_INVALID"),
    ] {
        let mut body = base.clone();
        body[0] = tag;
        let outcome = package_decode_call(&package, &approved, &body);
        assert_refusal(&outcome, code);
    }
}

#[test]
fn package_decode_retains_the_value_unit_envelope() {
    let (package, approved) = admit(&package_decode_image());
    let body = package_body(
        [2; 32],
        [3; 32],
        &[[4; 32], [5; 32], [6; 32]],
        &[[7; 32], [8; 32], [9; 32]],
    );
    let outcome = package_decode_call(&package, &approved, &body);
    match &outcome.termination {
        sley_vm::ExecutionTermination::ResourceLimit(kind) => assert_eq!(
            *kind,
            sley_vm::ResourceKind::ValueUnits,
            "six total Package set members reach the F5 decode envelope first",
        ),
        other => panic!("six total Package set members must expose the F5 envelope: {other:?}"),
    }
    eprintln!(
        "PACKAGE_DEC_F5 deps=3 exports=3 fuel={} instr={} peak={}",
        outcome.fuel_used, outcome.instruction_count, outcome.peak_value_units
    );
}

#[test]
fn package_program_decode_matches_native_semantics_and_resources() {
    let (package, approved) = admit(&package_program_decode_image());
    let increasing = std::array::from_fn(|index| u8::try_from(index).expect("index fits u8"));
    let decreasing = std::array::from_fn(|index| u8::try_from(31 - index).expect("index fits u8"));
    let cases = [
        ([1; 32], [2; 32], [3; 32], Vec::new(), Vec::new()),
        (increasing, decreasing, [10; 32], Vec::new(), Vec::new()),
    ];
    for (entity, workspace, root_namespace, dependencies, exports) in cases {
        let stored = package_stored(entity, workspace, root_namespace, &dependencies, &exports);
        assert_eq!(program_native_code(&stored), "OK");
        let outcome = package_program_decode_call(&package, &approved, &stored);
        eprintln!(
            "PACKAGE_PROGRAM_DEC stored{}B deps={} exports={} fuel={} instr={} peak={}",
            stored.len(),
            dependencies.len(),
            exports.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
        assert_package_program_decode_ok(
            &outcome,
            &entity,
            &workspace,
            &root_namespace,
            &dependencies,
            &exports,
        );
        assert!(outcome.fuel_used < 1_000_000, "decode fuel");
        assert!(outcome.instruction_count < 100_000, "decode instructions");
        assert!(outcome.peak_value_units < 1_000_000, "decode value units");
    }
}

#[test]
fn package_program_decode_retains_the_value_unit_boundary() {
    let (package, approved) = admit(&package_program_decode_image());
    for (name, dependencies, exports) in [
        ("one_dependency", vec![[4; 32]], Vec::new()),
        ("one_export", Vec::new(), vec![[5; 32]]),
    ] {
        let stored = package_stored([1; 32], [2; 32], [3; 32], &dependencies, &exports);
        let outcome = package_program_decode_call(&package, &approved, &stored);
        match &outcome.termination {
            sley_vm::ExecutionTermination::ResourceLimit(kind) => assert_eq!(
                *kind,
                sley_vm::ResourceKind::ValueUnits,
                "{name} reaches the F5 value-unit envelope first",
            ),
            other => panic!("{name} must expose the F5 value-unit boundary: {other:?}"),
        }
        eprintln!(
            "PACKAGE_PROGRAM_F5 {name} stored{}B fuel={} instr={} peak={}",
            stored.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
    }
}

#[test]
fn package_program_decode_preserves_layer_precedence_and_scope() {
    let (package, approved) = admit(&package_program_decode_image());
    let base = package_stored([1; 32], [2; 32], [3; 32], &[], &[]);
    let body = ns_body_of(&base);
    let body_offset = base.len() - 32 - body.len();

    let mut envelope_and_body = base.clone();
    envelope_and_body[0] = b'X';
    envelope_and_body[body_offset] = 0;
    let envelope_and_body = program_recompute(&envelope_and_body);

    let mut outer_and_body = base.clone();
    outer_and_body[body_offset - 2] = 1;
    outer_and_body[body_offset] = 0;
    let outer_and_body = program_recompute(&outer_and_body);

    let mut body_only = base.clone();
    body_only[body_offset] = 0;
    let body_only = program_recompute(&body_only);

    let mut digest = base.clone();
    let last = digest.len() - 1;
    digest[last] ^= 0xff;

    for (name, bytes) in [
        ("envelope_before_body", envelope_and_body),
        ("outer_before_body", outer_and_body),
        ("body", body_only),
        ("digest", digest),
    ] {
        let expected = program_native_code(&bytes);
        assert_ne!(expected, "OK", "{name} fixture must be malformed");
        let outcome = package_program_decode_call(&package, &approved, &bytes);
        assert_refusal(&outcome, &expected);
        eprintln!("PACKAGE_PROGRAM_REJECT {name}->{expected}");
    }

    let namespace = program_ns_stored(1, None, &[]);
    assert_eq!(program_native_code(&namespace), "OK");
    let outcome = package_program_decode_call(&package, &approved, &namespace);
    assert_refusal(&outcome, "SSMC_RESERVED_FIELD_PRESENT");
}

#[test]
fn package_program_encode_matches_native_stored_bytes() {
    let (package, approved) = admit(&package_program_encode_image());
    let cases = [
        ([1; 32], [2; 32], [3; 32], Vec::new(), Vec::new()),
        ([8; 32], [9; 32], [10; 32], Vec::new(), Vec::new()),
    ];
    for (entity, workspace, root_namespace, dependencies, exports) in cases {
        let expected = package_stored(entity, workspace, root_namespace, &dependencies, &exports);
        let outcome = package_program_encode_call(
            &package,
            &approved,
            &entity,
            &workspace,
            &root_namespace,
            &concat_ids(&dependencies),
            &concat_ids(&exports),
        );
        eprintln!(
            "PACKAGE_PROGRAM_ENC stored{}B deps={} exports={} fuel={} instr={} peak={}",
            expected.len(),
            dependencies.len(),
            exports.len(),
            outcome.fuel_used,
            outcome.instruction_count,
            outcome.peak_value_units
        );
        assert_encode_ok(&outcome, &expected);
    }
}

#[test]
fn package_program_encode_retains_the_value_unit_boundary() {
    let (package, approved) = admit(&package_program_encode_image());
    for (name, dependencies, exports) in [
        ("one_dependency", vec![[4; 32]], Vec::new()),
        ("one_export", Vec::new(), vec![[5; 32]]),
    ] {
        let outcome = package_program_encode_call(
            &package,
            &approved,
            &[1; 32],
            &[2; 32],
            &[3; 32],
            &concat_ids(&dependencies),
            &concat_ids(&exports),
        );
        match &outcome.termination {
            sley_vm::ExecutionTermination::ResourceLimit(kind) => assert_eq!(
                *kind,
                sley_vm::ResourceKind::ValueUnits,
                "{name} reaches the F5 value-unit envelope first",
            ),
            other => panic!("{name} must expose the F5 value-unit boundary: {other:?}"),
        }
        eprintln!(
            "PACKAGE_PROGRAM_ENC_F5 {name} fuel={} instr={} peak={}",
            outcome.fuel_used, outcome.instruction_count, outcome.peak_value_units
        );
    }
}

#[test]
fn package_program_encode_preserves_body_before_outer_precedence() {
    let (package, approved) = admit(&package_program_encode_image());
    let duplicate = concat_ids(&[[4; 32], [4; 32]]);
    for (name, entity, workspace, root_namespace, dependencies, expected) in [
        (
            "workspace_before_entity",
            vec![1; 31],
            vec![2; 31],
            vec![3; 32],
            Vec::new(),
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "root_before_entity",
            vec![1; 31],
            vec![2; 32],
            vec![3; 33],
            Vec::new(),
            "SCB_TRAILING_BYTES",
        ),
        (
            "dependencies_before_entity",
            vec![1; 31],
            vec![2; 32],
            vec![3; 32],
            duplicate,
            "SCB_MAP_DUPLICATE",
        ),
        (
            "entity_after_body",
            vec![1; 31],
            vec![2; 32],
            vec![3; 32],
            Vec::new(),
            "SCB_LENGTH_OVERFLOW",
        ),
    ] {
        let outcome = package_program_encode_call(
            &package,
            &approved,
            &entity,
            &workspace,
            &root_namespace,
            &dependencies,
            &[],
        );
        assert_refusal(&outcome, expected);
        eprintln!("PACKAGE_PROGRAM_ENC_REJECT {name}->{expected}");
    }
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

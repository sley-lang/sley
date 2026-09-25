//! RW-080 §1.1 primitive slice 1: strict SCB1 uvar framing core in Sley.
//!
//! PROVISIONAL C0 SEED CONSTRUCTION - explicitly not accepted runtime
//! authority. Seed-authored (this file is the seed-assembler record)
//! under standing amendment `rw075_correction.operator_override_2026_09_07`.
//! It exercises the v2 admit/approve/execute path for real strict
//! decoder/encoder algorithms through the approved execution boundary.
//!
//! Scope (bounded): unsigned uvar with explicit bit width, i.e. the exact
//! `Reader::read_uvar_width` behavior of `sley-scb1` (minimal LE7,
//! shift-63/64 overflow rules, trailing-zero-group minimality, width
//! range check) plus canonical LE7 emission. Three Sley functions:
//! `decode_uvar`, `decode_uvar_exact` (trailing check via `CallDirect`),
//! `encode_uvar`. Construction provenance and contract basis:
//! machineresearch/sley-2.0/reweave/rw-080-codec-uvar.md.
//!
//! Deliberate non-goals with reasons (not silent gaps):
//! - `codec_main` legs stay stubs (`rw080_codec_scaffold.rs`): no
//!   program/schema wire format exists in-tree, so wiring legs would
//!   invent framing. The units here are entries of their own approved
//!   images through the same boundary.
//! - `ZigZag` `SInt` (both directions): Sley has no int sign/width
//!   conversion opcode, so a parsed `UInt` cannot become an `SInt` result
//!   and a signed multiply at i64 extremes cannot be proven safe.
//!   Recorded as a bootstrap-profile observation in the manifest.
//! - Text/NFC/floats/maps/records: need text, bitwise, or (for full
//!   Schema) other unlanded capabilities. Length-delimited framing
//!   composes on this unit in a later slice.
//! - Errors are the exact `SCB_*` code strings as `Bytes` values. No
//!   code is mapped to the 7-code program-level vocabulary: inventing
//!   equivalences is forbidden, and the exact strings preserve all
//!   reference information. The vocabulary mapping belongs to leg
//!   wiring once program formats exist.

use sley_id::{EntityId, SchemaEpochId, StateRoot};
use sley_ssmc::{
    AdapterImport, Block, BranchTerminator, BuiltinCase, BuiltinFailureKind, CaseKey,
    CondBranchTerminator, ConstData, ConstValue, ConstantDefinition, FunctionGraph,
    FunctionRefValue, Immediate, IntegerWidth, Opcode, Operation, OperationResultRef, Parameter,
    ParameterRole, Reachability, ResultConst, ReturnTerminator, SwitchArgument, SwitchCase,
    SwitchEdge, TargetEdge, Terminator, TrapCode, TrapTerminator, TypeExpr, ValueRef,
    VariantSwitchTerminator, Visibility,
};

fn eid(ns: u8, idx: u16) -> EntityId {
    let mut bytes = [0u8; 32];
    bytes[0] = ns;
    // Namespace counters stay tiny (dozens of entities); the expect
    // documents the bound instead of silently truncating.
    bytes[1] = u8::try_from(idx >> 8).expect("namespace index fits u8");
    bytes[2] = u8::try_from(idx & 0xff).expect("namespace index fits u8");
    EntityId::from_bytes(bytes)
}

fn epoch() -> SchemaEpochId {
    SchemaEpochId::from_bytes([8; 32])
}

fn root() -> StateRoot {
    StateRoot::from_bytes([9; 32])
}

fn u64_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(64))
}

fn u32_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(32))
}

fn u8_type() -> TypeExpr {
    TypeExpr::UInt(IntegerWidth::from_bits(8))
}

fn u8vec_type() -> TypeExpr {
    TypeExpr::Vector(Box::new(u8_type()))
}

fn arith_result(inner: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(inner),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Arithmetic)),
    }
}

fn index_result(inner: TypeExpr) -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(inner),
        error: Box::new(TypeExpr::BuiltinFailure(BuiltinFailureKind::Index)),
    }
}

fn decode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![u64_type(), u64_type()])),
        error: Box::new(TypeExpr::Bytes),
    }
}

fn encode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Bytes),
        error: Box::new(TypeExpr::Bytes),
    }
}

/// Per-image entity namespaces so closures never mix identities.
#[derive(Clone, Copy)]
struct Ns {
    k: u8,
    p: u8,
    b: u8,
    o: u8,
}

struct Asm {
    next: [u16; 256],
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    constants: Vec<ConstantDefinition>,
}

impl Asm {
    fn new() -> Self {
        Self {
            next: [0u16; 256],
            parameters: Vec::new(),
            blocks: Vec::new(),
            operations: Vec::new(),
            constants: Vec::new(),
        }
    }

    fn id(&mut self, ns: u8) -> EntityId {
        let idx = self.next[ns as usize];
        self.next[ns as usize] += 1;
        eid(ns, idx)
    }

    fn param(
        &mut self,
        ns: u8,
        owner: EntityId,
        role: ParameterRole,
        value_type: TypeExpr,
    ) -> EntityId {
        // Ordinals count entities per owner/block; closures hold dozens.
        let ordinal = u32::try_from(self.parameters.iter().filter(|p| p.owner == owner).count())
            .expect("entity count fits u32");
        let entity_id = self.id(ns);
        self.parameters.push(Parameter {
            entity_id,
            owner,
            role,
            ordinal,
            value_type,
        });
        entity_id
    }

    fn konst(&mut self, ns: u8, value: ConstValue) -> EntityId {
        let entity_id = self.id(ns);
        self.constants.push(ConstantDefinition { entity_id, value });
        entity_id
    }

    fn ku64(&mut self, ns: u8, value: u128) -> EntityId {
        self.konst(
            ns,
            ConstValue {
                value_type: u64_type(),
                data: ConstData::UInt(value),
            },
        )
    }

    fn ku32(&mut self, ns: u8, value: u128) -> EntityId {
        self.konst(
            ns,
            ConstValue {
                value_type: u32_type(),
                data: ConstData::UInt(value),
            },
        )
    }

    fn ku8(&mut self, ns: u8, value: u128) -> EntityId {
        self.konst(
            ns,
            ConstValue {
                value_type: u8_type(),
                data: ConstData::UInt(value),
            },
        )
    }

    fn kbytes(&mut self, ns: u8, value: &[u8]) -> EntityId {
        self.konst(
            ns,
            ConstValue {
                value_type: TypeExpr::Bytes,
                data: ConstData::Bytes(value.to_vec()),
            },
        )
    }

    fn kbool(&mut self, ns: u8, value: bool) -> EntityId {
        self.konst(
            ns,
            ConstValue {
                value_type: TypeExpr::Bool,
                data: ConstData::Bool(value),
            },
        )
    }

    fn op(
        &mut self,
        ns: u8,
        block: EntityId,
        opcode: Opcode,
        operands: Vec<ValueRef>,
        result_types: Vec<TypeExpr>,
        immediate: Immediate,
    ) -> EntityId {
        // Ordinals count operations per block; blocks hold dozens.
        let ordinal = u32::try_from(self.operations.iter().filter(|o| o.block == block).count())
            .expect("operation count fits u32");
        let entity_id = self.id(ns);
        self.operations.push(Operation {
            entity_id,
            block,
            ordinal,
            opcode,
            operands,
            result_types,
            immediate,
        });
        entity_id
    }

    fn cref(&mut self, ns: u8, block: EntityId, target: EntityId, declared: TypeExpr) -> EntityId {
        self.op(
            ns,
            block,
            Opcode::ConstantRef,
            Vec::new(),
            vec![declared],
            Immediate::Entity(target),
        )
    }

    fn blk(
        &mut self,
        ns: u8,
        function: EntityId,
        params: Vec<EntityId>,
        ops: Vec<EntityId>,
        terminator: Terminator,
    ) -> EntityId {
        let entity_id = self.id(ns);
        self.blocks.push(Block {
            entity_id,
            function,
            parameters: params,
            operations: ops,
            terminator,
            reachability: Reachability::Required,
        });
        entity_id
    }
}

fn edge(target: EntityId, arguments: Vec<ValueRef>) -> TargetEdge {
    TargetEdge { target, arguments }
}

fn cond(condition: ValueRef, if_true: TargetEdge, if_false: TargetEdge) -> Terminator {
    Terminator::CondBranch(CondBranchTerminator {
        condition,
        if_true,
        if_false,
    })
}

fn branch(to: TargetEdge) -> Terminator {
    Terminator::Branch(BranchTerminator { edge: to })
}

fn switch(value: ValueRef, cases: Vec<(BuiltinCase, EntityId, Vec<SwitchArgument>)>) -> Terminator {
    Terminator::VariantSwitch(VariantSwitchTerminator {
        value,
        cases: cases
            .into_iter()
            .map(|(case, target, arguments)| SwitchCase {
                case_key: CaseKey::Builtin(case),
                edge: SwitchEdge { target, arguments },
            })
            .collect(),
    })
}

fn ret(value: ValueRef) -> Terminator {
    Terminator::Return(ReturnTerminator { value })
}

fn trap_block(a: &mut Asm, ns: Ns, function: EntityId) -> EntityId {
    a.blk(
        ns.b,
        function,
        Vec::new(),
        Vec::new(),
        Terminator::Trap(TrapTerminator {
            code: TrapCode::InternalInvariant,
            payload: None,
        }),
    )
}

/// `Return(ResultErr(Bytes))` block for one exact error-code constant.
fn err_block(
    a: &mut Asm,
    ns: Ns,
    function: EntityId,
    result_type: TypeExpr,
    code: EntityId,
) -> EntityId {
    let block = a.id(ns.b);
    let code_op = a.cref(ns.o, block, code, TypeExpr::Bytes);
    let err = a.op(
        ns.o,
        block,
        Opcode::ResultErr,
        vec![op_result(code_op)],
        vec![result_type],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: block,
        function,
        parameters: Vec::new(),
        operations: vec![code_op, err],
        terminator: ret(op_result(err)),
        reachability: Reachability::Required,
    });
    block
}

fn op_result(operation: EntityId) -> ValueRef {
    ValueRef::OperationResult(OperationResultRef {
        operation,
        result_index: 0,
    })
}

fn pav(id: EntityId) -> ValueRef {
    ValueRef::Parameter(id)
}

fn sav(id: EntityId) -> SwitchArgument {
    SwitchArgument::Value(ValueRef::Parameter(id))
}

fn oav(id: EntityId) -> SwitchArgument {
    SwitchArgument::Value(op_result(id))
}

fn bridge_identity(code: [u8; 4]) -> [u8; 32] {
    sley_vm::host_abi::bridge_identity(code)
}

fn frozen_import(code: [u8; 4], request: TypeExpr, response: TypeExpr) -> AdapterImport {
    use sley_vm::host_abi::BRIDGE_ABI_VERSION;
    let identity = EntityId::from_bytes(bridge_identity(code));
    AdapterImport {
        entity_id: identity,
        adapter_id: *identity.as_bytes(),
        abi_version: BRIDGE_ABI_VERSION,
        request_type: request,
        response_type: response,
        failure_type: TypeExpr::BuiltinFailure(BuiltinFailureKind::Index),
        effects: Vec::new(),
    }
}

// ── decode_uvar ──────────────────────────────────────────────────────
// Sley implementation of `Reader::read_uvar_width`: entry
// `decode_uvar(input: Bytes, pos: UInt64, width: UInt32)` returns
// `Result<(value: UInt64, new_pos: UInt64), Bytes>` where `Err` carries
// the exact `SCB_*` code. Widths and shift amounts are `UInt32` because
// the shift opcodes demand a u32 amount. The u8-to-u64 widening the
// opcode set lacks is done by an explicit 8-bit selection ladder
// (documented in the manifest as a workaround, not a new primitive).

struct DecodeConsts {
    c0: EntityId,
    c1: EntityId,
    c128: EntityId,
    // Shift amounts and widths are UInt32 (the shift opcodes demand a
    // u32 amount), so the shift/width constants live in u32 too.
    s0: EntityId,
    s7: EntityId,
    s63: EntityId,
    s64: EntityId,
    s71: EntityId,
    pow_u64: [EntityId; 8],
    bit_u8: [EntityId; 8],
    e_len: EntityId,
    e_int: EntityId,
    e_min: EntityId,
    e_res: EntityId,
    tru: EntityId,
    fal: EntityId,
}

fn decode_consts(a: &mut Asm, ns: Ns) -> DecodeConsts {
    let pow_u64 = [
        a.ku64(ns.k, 1),
        a.ku64(ns.k, 2),
        a.ku64(ns.k, 4),
        a.ku64(ns.k, 8),
        a.ku64(ns.k, 16),
        a.ku64(ns.k, 32),
        a.ku64(ns.k, 64),
        a.ku64(ns.k, 128),
    ];
    let bit_u8 = [
        a.ku8(ns.k, 1),
        a.ku8(ns.k, 2),
        a.ku8(ns.k, 4),
        a.ku8(ns.k, 8),
        a.ku8(ns.k, 16),
        a.ku8(ns.k, 32),
        a.ku8(ns.k, 64),
        a.ku8(ns.k, 128),
    ];
    DecodeConsts {
        c0: a.ku64(ns.k, 0),
        c1: pow_u64[0],
        c128: a.ku64(ns.k, 128),
        s0: a.ku32(ns.k, 0),
        s7: a.ku32(ns.k, 7),
        s63: a.ku32(ns.k, 63),
        s64: a.ku32(ns.k, 64),
        s71: a.ku32(ns.k, 71),
        pow_u64,
        bit_u8,
        e_len: a.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW"),
        e_int: a.kbytes(ns.k, b"SCB_INTEGER_OVERFLOW"),
        e_min: a.kbytes(ns.k, b"SCB_VARINT_NON_MINIMAL"),
        e_res: a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT"),
        tru: a.kbool(ns.k, true),
        fal: a.kbool(ns.k, false),
    }
}

// Builder DSL note: the three builders below use terse positional slot
// names (`p_*`/`q_*`/..., single-letter op slots) because each block is a
// fixed-arity tuple threaded through edges; longer names would obscure the
// positional correspondence the checker verifies by arity and type (see
// the edge-argument tests). The style lints below are triaged, not
// ignored: behavior is pinned by 9 tests plus gate judgment. Line counts
// are structural (one linear graph layout each; cf. bootstrap_closure.rs
// and raw_callable.rs precedent for mechanical-builder allows).
#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_decode(a: &mut Asm, ns: Ns, fid: EntityId) -> (FunctionGraph, DecodeConsts) {
    use sley_vm::host_abi::BRIDGE_CODE_B2V1;
    let bstart = a.blocks.len();
    let c = decode_consts(a, ns);
    let res_t = decode_result_type();
    let p_in = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let p_pos = a.param(ns.p, fid, ParameterRole::Function, u64_type());
    let p_w = a.param(ns.p, fid, ParameterRole::Function, u32_type());
    let p_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let e_int = err_block(a, ns, fid, res_t.clone(), c.e_int);
    let e_res = err_block(a, ns, fid, res_t.clone(), c.e_res);
    let e_len = err_block(a, ns, fid, res_t.clone(), c.e_len);
    let e_min = err_block(a, ns, fid, res_t.clone(), c.e_min);
    let trap = trap_block(a, ns, fid);

    // Entry: width == 0 || width > 64 refuses, mirroring `read_uvar`.
    let entry = a.id(ns.b);
    let w0c = a.cref(ns.o, entry, c.s0, u32_type());
    let w0 = a.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(p_w), op_result(w0c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let w65c = a.cref(ns.o, entry, c.s64, u32_type());
    let w65 = a.op(
        ns.o,
        entry,
        Opcode::GreaterThan,
        vec![pav(p_w), op_result(w65c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let bad = a.op(
        ns.o,
        entry,
        Opcode::BoolOr,
        vec![op_result(w0), op_result(w65)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let b2v = a.id(ns.b);
    let q_in = a.param(ns.p, b2v, ParameterRole::Block, TypeExpr::Bytes);
    let q_pos = a.param(ns.p, b2v, ParameterRole::Block, u64_type());
    let q_w = a.param(ns.p, b2v, ParameterRole::Block, u32_type());
    let q_unit = a.param(ns.p, b2v, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![w0c, w0, w65c, w65, bad],
        terminator: cond(
            op_result(bad),
            edge(e_int, Vec::new()),
            edge(b2v, vec![pav(p_in), pav(p_pos), pav(p_w), pav(p_unit)]),
        ),
        reachability: Reachability::Required,
    });

    // Byte conversion: the whole input becomes a vector once, up front.
    // A bridge failure means the input exceeds the 1 MiB bridge cap, a
    // documented capacity restriction against the 64 MiB (67,108,864 bytes) epoch limit.
    // Loop starts (pos0, value 0, shift 0u32, nread 0, vec, width).
    let z0 = a.cref(ns.o, b2v, c.c0, u64_type());
    let z1 = a.cref(ns.o, b2v, c.s0, u32_type());
    let z2 = a.cref(ns.o, b2v, c.c0, u64_type());
    let cv = a.op(
        ns.o,
        b2v,
        Opcode::AdapterInvoke,
        vec![pav(q_unit), pav(q_in)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    let looop = a.id(ns.b);
    let l_pos = a.param(ns.p, looop, ParameterRole::Block, u64_type());
    let l_val = a.param(ns.p, looop, ParameterRole::Block, u64_type());
    let l_sh = a.param(ns.p, looop, ParameterRole::Block, u32_type());
    let l_nr = a.param(ns.p, looop, ParameterRole::Block, u64_type());
    let l_vec = a.param(ns.p, looop, ParameterRole::Block, u8vec_type());
    let l_w = a.param(ns.p, looop, ParameterRole::Block, u32_type());
    a.blocks.push(Block {
        entity_id: b2v,
        function: fid,
        parameters: vec![q_in, q_pos, q_w, q_unit],
        operations: vec![z0, z1, z2, cv],
        terminator: switch(
            op_result(cv),
            vec![
                (
                    BuiltinCase::Ok,
                    looop,
                    vec![
                        sav(q_pos),
                        oav(z0),
                        oav(z1),
                        oav(z2),
                        SwitchArgument::CasePayload,
                        sav(q_w),
                    ],
                ),
                (BuiltinCase::Err, e_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    // Loop head: read one byte. `None` means pos is at or past the end,
    // i.e. truncation, exactly the reference `LengthOverflow` condition.
    let lz = a.cref(ns.o, looop, c.c0, u64_type());
    let g = a.op(
        ns.o,
        looop,
        Opcode::VectorGet,
        vec![pav(l_vec), pav(l_pos)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    // Ladder blocks, bit 7 down to bit 0.
    let mut ladder: Vec<EntityId> = Vec::new();
    let mut ladder_params: Vec<Vec<EntityId>> = Vec::new();
    for _ in 0..8 {
        let lb = a.id(ns.b);
        let b = a.param(ns.p, lb, ParameterRole::Block, u8_type());
        let acc = a.param(ns.p, lb, ParameterRole::Block, u64_type());
        let pos = a.param(ns.p, lb, ParameterRole::Block, u64_type());
        let val = a.param(ns.p, lb, ParameterRole::Block, u64_type());
        let sh = a.param(ns.p, lb, ParameterRole::Block, u32_type());
        let nr = a.param(ns.p, lb, ParameterRole::Block, u64_type());
        let vecp = a.param(ns.p, lb, ParameterRole::Block, u8vec_type());
        let w = a.param(ns.p, lb, ParameterRole::Block, u32_type());
        ladder.push(lb);
        ladder_params.push(vec![b, acc, pos, val, sh, nr, vecp, w]);
    }
    a.blocks.push(Block {
        entity_id: looop,
        function: fid,
        parameters: vec![l_pos, l_val, l_sh, l_nr, l_vec, l_w],
        operations: vec![lz, g],
        terminator: switch(
            op_result(g),
            vec![
                (BuiltinCase::None, e_len, Vec::new()),
                (
                    BuiltinCase::Some,
                    ladder[0],
                    vec![
                        SwitchArgument::CasePayload,
                        oav(lz),
                        sav(l_pos),
                        sav(l_val),
                        sav(l_sh),
                        sav(l_nr),
                        sav(l_vec),
                        sav(l_w),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });

    // Build the per-bit test/apply blocks. ladder[k] handles bit
    // (7 - k); ladder[k] falls through to ladder[k + 1], ladder[7]
    // (bit 0) falls through to ACC.
    let acc_block = a.id(ns.b);
    let mut next_of: Vec<EntityId> = vec![eid(0, 0); 8];
    for k in 0..8 {
        next_of[k] = if k == 7 { acc_block } else { ladder[k + 1] };
    }
    for k in 0..8 {
        // ladder[k] handles bit (7 - k): ladder[0] is the bit-7
        // (continuation) test, ladder[7] the bit-0 test.
        let bit = 7 - k;
        let lb = ladder[k];
        let ps = &ladder_params[k];
        let yes = a.id(ns.b);
        let y_b = a.param(ns.p, yes, ParameterRole::Block, u8_type());
        let y_acc = a.param(ns.p, yes, ParameterRole::Block, u64_type());
        let y_pos = a.param(ns.p, yes, ParameterRole::Block, u64_type());
        let y_val = a.param(ns.p, yes, ParameterRole::Block, u64_type());
        let y_sh = a.param(ns.p, yes, ParameterRole::Block, u32_type());
        let y_nr = a.param(ns.p, yes, ParameterRole::Block, u64_type());
        let y_vec = a.param(ns.p, yes, ParameterRole::Block, u8vec_type());
        let y_w = a.param(ns.p, yes, ParameterRole::Block, u32_type());
        let add = a.id(ns.b);
        let d_b2 = a.param(ns.p, add, ParameterRole::Block, u8_type());
        let d_a = a.param(ns.p, add, ParameterRole::Block, u64_type());
        let d_pos = a.param(ns.p, add, ParameterRole::Block, u64_type());
        let d_val = a.param(ns.p, add, ParameterRole::Block, u64_type());
        let d_sh = a.param(ns.p, add, ParameterRole::Block, u32_type());
        let d_nr = a.param(ns.p, add, ParameterRole::Block, u64_type());
        let d_vec = a.param(ns.p, add, ParameterRole::Block, u8vec_type());
        let d_w = a.param(ns.p, add, ParameterRole::Block, u32_type());
        let bc = a.cref(ns.o, lb, c.bit_u8[bit], u8_type());
        let cmp = a.op(
            ns.o,
            lb,
            Opcode::GreaterEqual,
            vec![pav(ps[0]), op_result(bc)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let fwd = |ids: &[EntityId]| ids.iter().map(|i| pav(*i)).collect::<Vec<_>>();
        // The bit-0 test (k == 7) falls through to ACC, which takes
        // (full, pos, val, sh, nr, vec, w): the finished byte drops out.
        let no_edge = if k == 7 {
            edge(
                next_of[k],
                vec![
                    pav(ps[1]),
                    pav(ps[2]),
                    pav(ps[3]),
                    pav(ps[4]),
                    pav(ps[5]),
                    pav(ps[6]),
                    pav(ps[7]),
                ],
            )
        } else {
            edge(next_of[k], fwd(ps))
        };
        a.blocks.push(Block {
            entity_id: lb,
            function: fid,
            parameters: ps.clone(),
            operations: vec![bc, cmp],
            terminator: cond(op_result(cmp), edge(yes, fwd(ps)), no_edge),
            reachability: Reachability::Required,
        });
        let sc = a.cref(ns.o, yes, c.bit_u8[bit], u8_type());
        let s = a.op(
            ns.o,
            yes,
            Opcode::IntSubChecked,
            vec![pav(y_b), op_result(sc)],
            vec![arith_result(u8_type())],
            Immediate::None,
        );
        a.blocks.push(Block {
            entity_id: yes,
            function: fid,
            parameters: vec![y_b, y_acc, y_pos, y_val, y_sh, y_nr, y_vec, y_w],
            operations: vec![sc, s],
            terminator: switch(
                op_result(s),
                vec![
                    (
                        BuiltinCase::Ok,
                        add,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(y_acc),
                            sav(y_pos),
                            sav(y_val),
                            sav(y_sh),
                            sav(y_nr),
                            sav(y_vec),
                            sav(y_w),
                        ],
                    ),
                    (BuiltinCase::Err, trap, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
        let ac = a.cref(ns.o, add, c.pow_u64[bit], u64_type());
        let a2 = a.op(
            ns.o,
            add,
            Opcode::IntAddChecked,
            vec![pav(d_a), op_result(ac)],
            vec![arith_result(u64_type())],
            Immediate::None,
        );
        // NEXT for ADD_k is the next ladder test, which takes
        // (b, acc, ...) so the stripped byte and grown sum flow on.
        a.blocks.push(Block {
            entity_id: add,
            function: fid,
            parameters: vec![d_b2, d_a, d_pos, d_val, d_sh, d_nr, d_vec, d_w],
            operations: vec![ac, a2],
            terminator: switch(
                op_result(a2),
                vec![
                    (
                        BuiltinCase::Ok,
                        next_of[k],
                        if k == 7 {
                            vec![
                                SwitchArgument::CasePayload,
                                sav(d_pos),
                                sav(d_val),
                                sav(d_sh),
                                sav(d_nr),
                                sav(d_vec),
                                sav(d_w),
                            ]
                        } else {
                            vec![
                                sav(d_b2),
                                SwitchArgument::CasePayload,
                                sav(d_pos),
                                sav(d_val),
                                sav(d_sh),
                                sav(d_nr),
                                sav(d_vec),
                                sav(d_w),
                            ]
                        },
                    ),
                    (BuiltinCase::Err, trap, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
    }

    // ACC: split continuation from payload. The ladder accumulated all 8
    // bits (bit 7 included), so cont = full >= 128, payload = full - 128.
    let ac_full = a.param(ns.p, acc_block, ParameterRole::Block, u64_type());
    let ac_pos = a.param(ns.p, acc_block, ParameterRole::Block, u64_type());
    let ac_val = a.param(ns.p, acc_block, ParameterRole::Block, u64_type());
    let ac_sh = a.param(ns.p, acc_block, ParameterRole::Block, u32_type());
    let ac_nr = a.param(ns.p, acc_block, ParameterRole::Block, u64_type());
    let ac_vec = a.param(ns.p, acc_block, ParameterRole::Block, u8vec_type());
    let ac_w = a.param(ns.p, acc_block, ParameterRole::Block, u32_type());
    let has = a.id(ns.b);
    let h_full = a.param(ns.p, has, ParameterRole::Block, u64_type());
    let h_pos = a.param(ns.p, has, ParameterRole::Block, u64_type());
    let h_val = a.param(ns.p, has, ParameterRole::Block, u64_type());
    let h_sh = a.param(ns.p, has, ParameterRole::Block, u32_type());
    let h_nr = a.param(ns.p, has, ParameterRole::Block, u64_type());
    let h_vec = a.param(ns.p, has, ParameterRole::Block, u8vec_type());
    let h_w = a.param(ns.p, has, ParameterRole::Block, u32_type());
    let merge = a.id(ns.b);
    let m_p = a.param(ns.p, merge, ParameterRole::Block, u64_type());
    let m_c = a.param(ns.p, merge, ParameterRole::Block, TypeExpr::Bool);
    let m_pos = a.param(ns.p, merge, ParameterRole::Block, u64_type());
    let m_val = a.param(ns.p, merge, ParameterRole::Block, u64_type());
    let m_sh = a.param(ns.p, merge, ParameterRole::Block, u32_type());
    let m_nr = a.param(ns.p, merge, ParameterRole::Block, u64_type());
    let m_vec = a.param(ns.p, merge, ParameterRole::Block, u8vec_type());
    let m_w = a.param(ns.p, merge, ParameterRole::Block, u32_type());
    let c128 = a.cref(ns.o, acc_block, c.c128, u64_type());
    let f = a.cref(ns.o, acc_block, c.fal, TypeExpr::Bool);
    let cc = a.op(
        ns.o,
        acc_block,
        Opcode::GreaterEqual,
        vec![pav(ac_full), op_result(c128)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: acc_block,
        function: fid,
        parameters: vec![ac_full, ac_pos, ac_val, ac_sh, ac_nr, ac_vec, ac_w],
        operations: vec![c128, f, cc],
        terminator: cond(
            op_result(cc),
            edge(
                has,
                vec![
                    pav(ac_full),
                    pav(ac_pos),
                    pav(ac_val),
                    pav(ac_sh),
                    pav(ac_nr),
                    pav(ac_vec),
                    pav(ac_w),
                ],
            ),
            edge(
                merge,
                vec![
                    pav(ac_full),
                    op_result(f),
                    pav(ac_pos),
                    pav(ac_val),
                    pav(ac_sh),
                    pav(ac_nr),
                    pav(ac_vec),
                    pav(ac_w),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let hc = a.cref(ns.o, has, c.c128, u64_type());
    let hp = a.op(
        ns.o,
        has,
        Opcode::IntSubChecked,
        vec![pav(h_full), op_result(hc)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let ht = a.cref(ns.o, has, c.tru, TypeExpr::Bool);
    a.blocks.push(Block {
        entity_id: has,
        function: fid,
        parameters: vec![h_full, h_pos, h_val, h_sh, h_nr, h_vec, h_w],
        operations: vec![hc, hp, ht],
        terminator: switch(
            op_result(hp),
            vec![
                (
                    BuiltinCase::Ok,
                    merge,
                    vec![
                        SwitchArgument::CasePayload,
                        oav(ht),
                        sav(h_pos),
                        sav(h_val),
                        sav(h_sh),
                        sav(h_nr),
                        sav(h_vec),
                        sav(h_w),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    // MERGE: shift >= 64 && payload != 0 refuses (reference order first).
    let m1 = a.id(ns.b);
    // M1 takes the MERGE 8-tuple (p, cont: Bool, pos, val, sh: u32,
    // nr, vec, w: u32).
    let mut n1p: Vec<EntityId> = Vec::new();
    for i in 0..8 {
        let ty = if i == 1 {
            TypeExpr::Bool
        } else if i == 4 || i == 7 {
            u32_type()
        } else if i == 6 {
            u8vec_type()
        } else {
            u64_type()
        };
        n1p.push(a.param(ns.p, m1, ParameterRole::Block, ty));
    }
    let m2 = a.id(ns.b);
    let mut n2p: Vec<EntityId> = Vec::new();
    for i in 0..8 {
        let ty = if i == 1 {
            TypeExpr::Bool
        } else if i == 4 || i == 7 {
            u32_type()
        } else if i == 6 {
            u8vec_type()
        } else {
            u64_type()
        };
        n2p.push(a.param(ns.p, m2, ParameterRole::Block, ty));
    }
    let accum = a.id(ns.b);
    let back = a.id(ns.b);
    let k_val = a.param(ns.p, back, ParameterRole::Block, u64_type());
    let k_sh = a.param(ns.p, back, ParameterRole::Block, u32_type());
    let k_nr = a.param(ns.p, back, ParameterRole::Block, u64_type());
    let k_pos = a.param(ns.p, back, ParameterRole::Block, u64_type());
    let k_vec = a.param(ns.p, back, ParameterRole::Block, u8vec_type());
    let k_w = a.param(ns.p, back, ParameterRole::Block, u32_type());
    let contchk = a.id(ns.b);
    let cc_val = a.param(ns.p, contchk, ParameterRole::Block, u64_type());
    let cc_nr = a.param(ns.p, contchk, ParameterRole::Block, u64_type());
    let cc_p = a.param(ns.p, contchk, ParameterRole::Block, u64_type());
    let cc_c = a.param(ns.p, contchk, ParameterRole::Block, TypeExpr::Bool);
    let cc_pos = a.param(ns.p, contchk, ParameterRole::Block, u64_type());
    let cc_vec = a.param(ns.p, contchk, ParameterRole::Block, u8vec_type());
    let cc_w = a.param(ns.p, contchk, ParameterRole::Block, u32_type());
    let cc_sh = a.param(ns.p, contchk, ParameterRole::Block, u32_type());

    let ms64 = a.cref(ns.o, merge, c.s64, u32_type());
    let mz = a.cref(ns.o, merge, c.c0, u64_type());
    let m0 = a.op(
        ns.o,
        merge,
        Opcode::GreaterEqual,
        vec![pav(m_sh), op_result(ms64)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let m1op = a.op(
        ns.o,
        merge,
        Opcode::NotEqual,
        vec![pav(m_p), op_result(mz)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let m2op = a.op(
        ns.o,
        merge,
        Opcode::BoolAnd,
        vec![op_result(m0), op_result(m1op)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let fwd8 = |ids: &[EntityId]| ids.iter().map(|i| pav(*i)).collect::<Vec<_>>();
    a.blocks.push(Block {
        entity_id: merge,
        function: fid,
        parameters: vec![m_p, m_c, m_pos, m_val, m_sh, m_nr, m_vec, m_w],
        operations: vec![ms64, mz, m0, m1op, m2op],
        terminator: cond(
            op_result(m2op),
            edge(e_int, Vec::new()),
            edge(
                m1,
                vec![
                    pav(m_p),
                    pav(m_c),
                    pav(m_pos),
                    pav(m_val),
                    pav(m_sh),
                    pav(m_nr),
                    pav(m_vec),
                    pav(m_w),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    // M1: shift == 63 && payload > 1 refuses.
    let s63 = a.cref(ns.o, m1, c.s63, u32_type());
    let one = a.cref(ns.o, m1, c.c1, u64_type());
    let q3 = a.op(
        ns.o,
        m1,
        Opcode::Equal,
        vec![pav(n1p[4]), op_result(s63)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let q4 = a.op(
        ns.o,
        m1,
        Opcode::GreaterThan,
        vec![pav(n1p[0]), op_result(one)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let q5 = a.op(
        ns.o,
        m1,
        Opcode::BoolAnd,
        vec![op_result(q3), op_result(q4)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: m1,
        function: fid,
        parameters: n1p.clone(),
        operations: vec![s63, one, q3, q4, q5],
        terminator: cond(op_result(q5), edge(e_int, Vec::new()), edge(m2, fwd8(&n1p))),
        reachability: Reachability::Required,
    });

    // M2: shift < 64 accumulates; shift >= 64 (payload 0 here) skips.
    let ms64b = a.cref(ns.o, m2, c.s64, u32_type());
    let m6 = a.op(
        ns.o,
        m2,
        Opcode::LessThan,
        vec![pav(n2p[4]), op_result(ms64b)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    // ACCUM's own parameter tuple (p, cont, pos, val, sh, nr, vec, w).
    let mut acp: Vec<EntityId> = Vec::new();
    for i in 0..8 {
        let ty = if i == 1 {
            TypeExpr::Bool
        } else if i == 4 || i == 7 {
            u32_type()
        } else if i == 6 {
            u8vec_type()
        } else {
            u64_type()
        };
        acp.push(a.param(ns.p, accum, ParameterRole::Block, ty));
    }
    a.blocks.push(Block {
        entity_id: m2,
        function: fid,
        parameters: n2p.clone(),
        operations: vec![ms64b, m6],
        terminator: cond(
            op_result(m6),
            edge(
                accum,
                vec![
                    pav(n2p[0]),
                    pav(n2p[1]),
                    pav(n2p[2]),
                    pav(n2p[3]),
                    pav(n2p[4]),
                    pav(n2p[5]),
                    pav(n2p[6]),
                    pav(n2p[7]),
                ],
            ),
            edge(
                contchk,
                vec![
                    pav(n2p[3]),
                    pav(n2p[5]),
                    pav(n2p[0]),
                    pav(n2p[1]),
                    pav(n2p[2]),
                    pav(n2p[6]),
                    pav(n2p[7]),
                    pav(n2p[4]),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    // ACCUM: value += payload << shift. Both checked ops are provably
    // safe (prior checks force shift/payload ranges with disjoint set
    // bits), so their failure edges carry InternalInvariant.
    let t = a.op(
        ns.o,
        accum,
        Opcode::IntShlChecked,
        vec![pav(acp[0]), pav(acp[4])],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let addv = a.id(ns.b);
    let av_t = a.param(ns.p, addv, ParameterRole::Block, u64_type());
    let av_val = a.param(ns.p, addv, ParameterRole::Block, u64_type());
    let av_nr = a.param(ns.p, addv, ParameterRole::Block, u64_type());
    let av_p = a.param(ns.p, addv, ParameterRole::Block, u64_type());
    let av_c = a.param(ns.p, addv, ParameterRole::Block, TypeExpr::Bool);
    let av_pos = a.param(ns.p, addv, ParameterRole::Block, u64_type());
    let av_vec = a.param(ns.p, addv, ParameterRole::Block, u8vec_type());
    let av_w = a.param(ns.p, addv, ParameterRole::Block, u32_type());
    let av_sh = a.param(ns.p, addv, ParameterRole::Block, u32_type());
    a.blocks.push(Block {
        entity_id: accum,
        function: fid,
        parameters: acp.clone(),
        operations: vec![t],
        terminator: switch(
            op_result(t),
            vec![
                (
                    BuiltinCase::Ok,
                    addv,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(acp[3]),
                        sav(acp[5]),
                        sav(acp[0]),
                        sav(acp[1]),
                        sav(acp[2]),
                        sav(acp[6]),
                        sav(acp[7]),
                        sav(acp[4]),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let v2 = a.op(
        ns.o,
        addv,
        Opcode::IntAddChecked,
        vec![pav(av_val), pav(av_t)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: addv,
        function: fid,
        parameters: vec![av_t, av_val, av_nr, av_p, av_c, av_pos, av_vec, av_w, av_sh],
        operations: vec![v2],
        terminator: switch(
            op_result(v2),
            vec![
                (
                    BuiltinCase::Ok,
                    contchk,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(av_nr),
                        sav(av_p),
                        sav(av_c),
                        sav(av_pos),
                        sav(av_vec),
                        sav(av_w),
                        sav(av_sh),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    // CONTCHK / BACK / FINAL / F2 / L2 / OK.
    let fina = a.id(ns.b);
    let f_val = a.param(ns.p, fina, ParameterRole::Block, u64_type());
    let f_nr = a.param(ns.p, fina, ParameterRole::Block, u64_type());
    let f_p = a.param(ns.p, fina, ParameterRole::Block, u64_type());
    let f_pos = a.param(ns.p, fina, ParameterRole::Block, u64_type());
    let f_vec = a.param(ns.p, fina, ParameterRole::Block, u8vec_type());
    let f_w = a.param(ns.p, fina, ParameterRole::Block, u32_type());
    a.blocks.push(Block {
        entity_id: contchk,
        function: fid,
        parameters: vec![cc_val, cc_nr, cc_p, cc_c, cc_pos, cc_vec, cc_w, cc_sh],
        operations: Vec::new(),
        terminator: cond(
            pav(cc_c),
            edge(
                back,
                vec![
                    pav(cc_val),
                    pav(cc_sh),
                    pav(cc_nr),
                    pav(cc_pos),
                    pav(cc_vec),
                    pav(cc_w),
                ],
            ),
            edge(
                fina,
                vec![
                    pav(cc_val),
                    pav(cc_nr),
                    pav(cc_p),
                    pav(cc_pos),
                    pav(cc_vec),
                    pav(cc_w),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    let back2 = a.id(ns.b);
    let y_s2 = a.param(ns.p, back2, ParameterRole::Block, u32_type());
    let y_nr = a.param(ns.p, back2, ParameterRole::Block, u64_type());
    let y_pos = a.param(ns.p, back2, ParameterRole::Block, u64_type());
    let y_val = a.param(ns.p, back2, ParameterRole::Block, u64_type());
    let y_vec = a.param(ns.p, back2, ParameterRole::Block, u8vec_type());
    let y_w = a.param(ns.p, back2, ParameterRole::Block, u32_type());
    let back3 = a.id(ns.b);
    let z_s2 = a.param(ns.p, back3, ParameterRole::Block, u32_type());
    let z_n2 = a.param(ns.p, back3, ParameterRole::Block, u64_type());
    let z_pos = a.param(ns.p, back3, ParameterRole::Block, u64_type());
    let z_val = a.param(ns.p, back3, ParameterRole::Block, u64_type());
    let z_vec = a.param(ns.p, back3, ParameterRole::Block, u8vec_type());
    let z_w = a.param(ns.p, back3, ParameterRole::Block, u32_type());
    let s2c = a.cref(ns.o, back, c.s7, u32_type());
    let s2 = a.op(
        ns.o,
        back,
        Opcode::IntAddChecked,
        vec![pav(k_sh), op_result(s2c)],
        vec![arith_result(u32_type())],
        Immediate::None,
    );
    // BACKCHK: the reference refuses once shift reaches 71 after the
    // increment (shift >= 64 + 7). Every continuation passes through
    // BACK, so this single check covers all paths; final bytes never
    // reach BACK.
    let backchk = a.id(ns.b);
    let bc_s2 = a.param(ns.p, backchk, ParameterRole::Block, u32_type());
    let bc_nr = a.param(ns.p, backchk, ParameterRole::Block, u64_type());
    let bc_pos = a.param(ns.p, backchk, ParameterRole::Block, u64_type());
    let bc_val = a.param(ns.p, backchk, ParameterRole::Block, u64_type());
    let bc_vec = a.param(ns.p, backchk, ParameterRole::Block, u8vec_type());
    let bc_w = a.param(ns.p, backchk, ParameterRole::Block, u32_type());
    a.blocks.push(Block {
        entity_id: back,
        function: fid,
        parameters: vec![k_val, k_sh, k_nr, k_pos, k_vec, k_w],
        operations: vec![s2c, s2],
        terminator: switch(
            op_result(s2),
            vec![
                (
                    BuiltinCase::Ok,
                    backchk,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(k_nr),
                        sav(k_pos),
                        sav(k_val),
                        sav(k_vec),
                        sav(k_w),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let o71c = a.cref(ns.o, backchk, c.s71, u32_type());
    let over71 = a.op(
        ns.o,
        backchk,
        Opcode::GreaterEqual,
        vec![pav(bc_s2), op_result(o71c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: backchk,
        function: fid,
        parameters: vec![bc_s2, bc_nr, bc_pos, bc_val, bc_vec, bc_w],
        operations: vec![o71c, over71],
        terminator: cond(
            op_result(over71),
            edge(e_int, Vec::new()),
            edge(
                back2,
                vec![
                    pav(bc_s2),
                    pav(bc_nr),
                    pav(bc_pos),
                    pav(bc_val),
                    pav(bc_vec),
                    pav(bc_w),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let n2c = a.cref(ns.o, back2, c.c1, u64_type());
    let n2 = a.op(
        ns.o,
        back2,
        Opcode::IntAddChecked,
        vec![pav(y_nr), op_result(n2c)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: back2,
        function: fid,
        parameters: vec![y_s2, y_nr, y_pos, y_val, y_vec, y_w],
        operations: vec![n2c, n2],
        terminator: switch(
            op_result(n2),
            vec![
                (
                    BuiltinCase::Ok,
                    back3,
                    vec![
                        sav(y_s2),
                        SwitchArgument::CasePayload,
                        sav(y_pos),
                        sav(y_val),
                        sav(y_vec),
                        sav(y_w),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let p2c = a.cref(ns.o, back3, c.c1, u64_type());
    let p2 = a.op(
        ns.o,
        back3,
        Opcode::IntAddChecked,
        vec![pav(z_pos), op_result(p2c)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: back3,
        function: fid,
        parameters: vec![z_s2, z_n2, z_pos, z_val, z_vec, z_w],
        operations: vec![p2c, p2],
        terminator: switch(
            op_result(p2),
            vec![
                (
                    BuiltinCase::Ok,
                    looop,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(z_val),
                        sav(z_s2),
                        sav(z_n2),
                        sav(z_vec),
                        sav(z_w),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    // FINAL: trailing-zero-group minimality, then width check.
    let f2 = a.id(ns.b);
    let g_val = a.param(ns.p, f2, ParameterRole::Block, u64_type());
    let g_pos = a.param(ns.p, f2, ParameterRole::Block, u64_type());
    let g_vec = a.param(ns.p, f2, ParameterRole::Block, u8vec_type());
    let g_w = a.param(ns.p, f2, ParameterRole::Block, u32_type());
    let fina2 = a.id(ns.b);
    let w_n1 = a.param(ns.p, fina2, ParameterRole::Block, u64_type());
    let w_val = a.param(ns.p, fina2, ParameterRole::Block, u64_type());
    let w_p = a.param(ns.p, fina2, ParameterRole::Block, u64_type());
    let w_pos = a.param(ns.p, fina2, ParameterRole::Block, u64_type());
    let w_vec = a.param(ns.p, fina2, ParameterRole::Block, u8vec_type());
    let w_w = a.param(ns.p, fina2, ParameterRole::Block, u32_type());
    let n1c = a.cref(ns.o, fina, c.c1, u64_type());
    let n1 = a.op(
        ns.o,
        fina,
        Opcode::IntAddChecked,
        vec![pav(f_nr), op_result(n1c)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: fina,
        function: fid,
        parameters: vec![f_val, f_nr, f_p, f_pos, f_vec, f_w],
        operations: vec![n1c, n1],
        terminator: switch(
            op_result(n1),
            vec![
                (
                    BuiltinCase::Ok,
                    fina2,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(f_val),
                        sav(f_p),
                        sav(f_pos),
                        sav(f_vec),
                        sav(f_w),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let f1c = a.cref(ns.o, fina2, c.c1, u64_type());
    let f1 = a.op(
        ns.o,
        fina2,
        Opcode::GreaterThan,
        vec![pav(w_n1), op_result(f1c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let f2c = a.cref(ns.o, fina2, c.c0, u64_type());
    let f2op = a.op(
        ns.o,
        fina2,
        Opcode::Equal,
        vec![pav(w_p), op_result(f2c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let f3 = a.op(
        ns.o,
        fina2,
        Opcode::BoolAnd,
        vec![op_result(f1), op_result(f2op)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: fina2,
        function: fid,
        parameters: vec![w_n1, w_val, w_p, w_pos, w_vec, w_w],
        operations: vec![f1c, f1, f2c, f2op, f3],
        terminator: cond(
            op_result(f3),
            edge(e_min, Vec::new()),
            edge(f2, vec![pav(w_val), pav(w_pos), pav(w_vec), pav(w_w)]),
        ),
        reachability: Reachability::Required,
    });

    let l2b = a.id(ns.b);
    let h_val = a.param(ns.p, l2b, ParameterRole::Block, u64_type());
    let h_pos = a.param(ns.p, l2b, ParameterRole::Block, u64_type());
    let h_vec = a.param(ns.p, l2b, ParameterRole::Block, u8vec_type());
    let h_w = a.param(ns.p, l2b, ParameterRole::Block, u32_type());
    let ok = a.id(ns.b);
    let o_val = a.param(ns.p, ok, ParameterRole::Block, u64_type());
    let o_pos = a.param(ns.p, ok, ParameterRole::Block, u64_type());
    let ltcc = a.cref(ns.o, f2, c.s64, u32_type());
    let ltc = a.op(
        ns.o,
        f2,
        Opcode::LessThan,
        vec![pav(g_w), op_result(ltcc)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2,
        function: fid,
        parameters: vec![g_val, g_pos, g_vec, g_w],
        operations: vec![ltcc, ltc],
        terminator: cond(
            op_result(ltc),
            edge(l2b, vec![pav(g_val), pav(g_pos), pav(g_vec), pav(g_w)]),
            edge(ok, vec![pav(g_val), pav(g_pos)]),
        ),
        reachability: Reachability::Required,
    });
    let limc = a.cref(ns.o, l2b, c.c1, u64_type());
    let lim = a.op(
        ns.o,
        l2b,
        Opcode::IntShlChecked,
        vec![op_result(limc), pav(h_w)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let l2c = a.id(ns.b);
    let k_lim = a.param(ns.p, l2c, ParameterRole::Block, u64_type());
    let k_val = a.param(ns.p, l2c, ParameterRole::Block, u64_type());
    let k_pos = a.param(ns.p, l2c, ParameterRole::Block, u64_type());
    let k_vec = a.param(ns.p, l2c, ParameterRole::Block, u8vec_type());
    a.blocks.push(Block {
        entity_id: l2b,
        function: fid,
        parameters: vec![h_val, h_pos, h_vec, h_w],
        operations: vec![limc, lim],
        terminator: switch(
            op_result(lim),
            vec![
                (
                    BuiltinCase::Ok,
                    l2c,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(h_val),
                        sav(h_pos),
                        sav(h_vec),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let over = a.op(
        ns.o,
        l2c,
        Opcode::GreaterEqual,
        vec![pav(k_val), pav(k_lim)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: l2c,
        function: fid,
        parameters: vec![k_lim, k_val, k_pos, k_vec],
        operations: vec![over],
        terminator: cond(
            op_result(over),
            edge(e_int, Vec::new()),
            edge(ok, vec![pav(k_val), pav(k_pos)]),
        ),
        reachability: Reachability::Required,
    });

    let ok2 = a.id(ns.b);
    let t_val = a.param(ns.p, ok2, ParameterRole::Block, u64_type());
    let t_np = a.param(ns.p, ok2, ParameterRole::Block, u64_type());
    let npc = a.cref(ns.o, ok, c.c1, u64_type());
    let np = a.op(
        ns.o,
        ok,
        Opcode::IntAddChecked,
        vec![pav(o_pos), op_result(npc)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: ok,
        function: fid,
        parameters: vec![o_val, o_pos],
        operations: vec![npc, np],
        terminator: switch(
            op_result(np),
            vec![
                (
                    BuiltinCase::Ok,
                    ok2,
                    vec![sav(o_val), SwitchArgument::CasePayload],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let tup = a.op(
        ns.o,
        ok2,
        Opcode::TupleNew,
        vec![pav(t_val), pav(t_np)],
        vec![TypeExpr::Tuple(vec![u64_type(), u64_type()])],
        Immediate::None,
    );
    let okv = a.op(
        ns.o,
        ok2,
        Opcode::ResultOk,
        vec![op_result(tup)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: ok2,
        function: fid,
        parameters: vec![t_val, t_np],
        operations: vec![tup, okv],
        terminator: ret(op_result(okv)),
        reachability: Reachability::Required,
    });

    let graph = FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![p_in, p_pos, p_w, p_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    (graph, c)
}

// ── decode_uvar_exact ────────────────────────────────────────────────
// Trailing check composed by call: `decode_uvar_exact` calls `decode_uvar`
// via `CallDirect`, then requires the returned position to equal the input
// length, else `SCB_TRAILING_BYTES`. This mirrors `decode_payload_exact`
// and demonstrates multi-function composition through the gate.
#[allow(clippy::too_many_lines)]
fn build_exact(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    decode_fid: EntityId,
    dc: &DecodeConsts,
) -> FunctionGraph {
    use sley_vm::host_abi::BRIDGE_CODE_B2V1;
    let bstart = a.blocks.len();
    let res_t = decode_result_type();
    let e_trail = a.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let x_in = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let x_pos = a.param(ns.p, fid, ParameterRole::Function, u64_type());
    let x_w = a.param(ns.p, fid, ParameterRole::Function, u32_type());
    let x_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let x_err_res = err_block(a, ns, fid, res_t.clone(), dc.e_res);
    let x_trail = err_block(a, ns, fid, res_t.clone(), e_trail);

    let xe = a.id(ns.b);
    let bv = a.op(
        ns.o,
        xe,
        Opcode::AdapterInvoke,
        vec![pav(x_unit), pav(x_in)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    let xc = a.id(ns.b);
    let c_vec = a.param(ns.p, xc, ParameterRole::Block, u8vec_type());
    let c_in = a.param(ns.p, xc, ParameterRole::Block, TypeExpr::Bytes);
    let c_pos = a.param(ns.p, xc, ParameterRole::Block, u64_type());
    let c_w = a.param(ns.p, xc, ParameterRole::Block, u32_type());
    let c_unit = a.param(ns.p, xc, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: xe,
        function: fid,
        parameters: Vec::new(),
        operations: vec![bv],
        terminator: switch(
            op_result(bv),
            vec![
                (
                    BuiltinCase::Ok,
                    xc,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(x_in),
                        sav(x_pos),
                        sav(x_w),
                        sav(x_unit),
                    ],
                ),
                (BuiltinCase::Err, x_err_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let r = a.op(
        ns.o,
        xc,
        Opcode::CallDirect,
        vec![pav(c_in), pav(c_pos), pav(c_w), pav(c_unit)],
        vec![res_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let xu = a.id(ns.b);
    let u_tup = a.param(
        ns.p,
        xu,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let u_vec = a.param(ns.p, xu, ParameterRole::Block, u8vec_type());
    let xw = a.id(ns.b);
    let w_e = a.param(ns.p, xw, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: xc,
        function: fid,
        parameters: vec![c_vec, c_in, c_pos, c_w, c_unit],
        operations: vec![r],
        terminator: switch(
            op_result(r),
            vec![
                (
                    BuiltinCase::Ok,
                    xu,
                    vec![SwitchArgument::CasePayload, sav(c_vec)],
                ),
                (BuiltinCase::Err, xw, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });

    let np = a.op(
        ns.o,
        xu,
        Opcode::TupleGet,
        vec![pav(u_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let ln = a.op(
        ns.o,
        xu,
        Opcode::VectorLen,
        vec![pav(u_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let full = a.op(
        ns.o,
        xu,
        Opcode::Equal,
        vec![op_result(np), op_result(ln)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let xo = a.id(ns.b);
    let o_tup = a.param(
        ns.p,
        xo,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    a.blocks.push(Block {
        entity_id: xu,
        function: fid,
        parameters: vec![u_tup, u_vec],
        operations: vec![np, ln, full],
        terminator: cond(
            op_result(full),
            edge(xo, vec![pav(u_tup)]),
            edge(x_trail, Vec::new()),
        ),
        reachability: Reachability::Required,
    });

    let okv = a.op(
        ns.o,
        xo,
        Opcode::ResultOk,
        vec![pav(o_tup)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: xo,
        function: fid,
        parameters: vec![o_tup],
        operations: vec![okv],
        terminator: ret(op_result(okv)),
        reachability: Reachability::Required,
    });
    let erw = a.op(
        ns.o,
        xw,
        Opcode::ResultErr,
        vec![pav(w_e)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: xw,
        function: fid,
        parameters: vec![w_e],
        operations: vec![erw],
        terminator: ret(op_result(erw)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![x_in, x_pos, x_w, x_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: xe,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

// ── encode_uvar ──────────────────────────────────────────────────────
// Canonical LE7 emission: entry `encode_uvar(value: UInt64,
// width: UInt32)` returns `Result<Bytes, Bytes>` with the exact
// `SCB_*` code on refusal. The width precheck mirrors `read_uvar`
// (0 and >64 refuse); the range check `width < 64 && value >= 2^width`
// applies the decode-side width rule symmetrically at emission time.
// That symmetric rule is a new codec-level contract decision recorded
// in the manifest, not reference behavior (the reference encoder
// takes no width). Narrowing u64 remainders to u8 uses the same
// selection-ladder workaround as decoding.

#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_encode(a: &mut Asm, ns: Ns, fid: EntityId) -> FunctionGraph {
    use sley_vm::host_abi::{BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let bstart = a.blocks.len();
    let res_t = encode_result_type();
    let c0 = a.ku64(ns.k, 0);
    let c1 = a.ku64(ns.k, 1);
    let c128 = a.ku64(ns.k, 128);
    let w0 = a.ku32(ns.k, 0);
    let w64 = a.ku32(ns.k, 64);
    let pow_u64 = [
        c128,
        a.ku64(ns.k, 64),
        a.ku64(ns.k, 32),
        a.ku64(ns.k, 16),
        a.ku64(ns.k, 8),
        a.ku64(ns.k, 4),
        a.ku64(ns.k, 2),
        c1,
    ];
    let b0 = a.ku8(ns.k, 0);
    let bit_u8 = [
        a.ku8(ns.k, 1),
        a.ku8(ns.k, 2),
        a.ku8(ns.k, 4),
        a.ku8(ns.k, 8),
        a.ku8(ns.k, 16),
        a.ku8(ns.k, 32),
        a.ku8(ns.k, 64),
        a.ku8(ns.k, 128),
    ];
    let e_int = a.kbytes(ns.k, b"SCB_INTEGER_OVERFLOW");

    let n_val = a.param(ns.p, fid, ParameterRole::Function, u64_type());
    let n_w = a.param(ns.p, fid, ParameterRole::Function, u32_type());
    let n_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let e_err = err_block(a, ns, fid, res_t.clone(), e_int);
    let trap = trap_block(a, ns, fid);

    let entry = a.id(ns.b);
    let ew0c = a.cref(ns.o, entry, w0, u32_type());
    let w0 = a.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(n_w), op_result(ew0c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let ew65c = a.cref(ns.o, entry, w64, u32_type());
    let w65 = a.op(
        ns.o,
        entry,
        Opcode::GreaterThan,
        vec![pav(n_w), op_result(ew65c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let bad = a.op(
        ns.o,
        entry,
        Opcode::BoolOr,
        vec![op_result(w0), op_result(w65)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let w64t = a.id(ns.b);
    let t_val = a.param(ns.p, w64t, ParameterRole::Block, u64_type());
    let t_w = a.param(ns.p, w64t, ParameterRole::Block, u32_type());
    let t_unit = a.param(ns.p, w64t, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![ew0c, w0, ew65c, w65, bad],
        terminator: cond(
            op_result(bad),
            edge(e_err, Vec::new()),
            edge(w64t, vec![pav(n_val), pav(n_w), pav(n_unit)]),
        ),
        reachability: Reachability::Required,
    });

    let wck = a.id(ns.b);
    let k_val = a.param(ns.p, wck, ParameterRole::Block, u64_type());
    let k_w = a.param(ns.p, wck, ParameterRole::Block, u32_type());
    let k_unit = a.param(ns.p, wck, ParameterRole::Block, TypeExpr::Unit);
    let loop0 = a.id(ns.b);
    let z_val = a.param(ns.p, loop0, ParameterRole::Block, u64_type());
    let z_unit = a.param(ns.p, loop0, ParameterRole::Block, TypeExpr::Unit);
    let ltc = a.cref(ns.o, w64t, w64, u32_type());
    let lt = a.op(
        ns.o,
        w64t,
        Opcode::LessThan,
        vec![pav(t_w), op_result(ltc)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: w64t,
        function: fid,
        parameters: vec![t_val, t_w, t_unit],
        operations: vec![ltc, lt],
        terminator: cond(
            op_result(lt),
            edge(wck, vec![pav(t_val), pav(t_w), pav(t_unit)]),
            edge(loop0, vec![pav(t_val), pav(t_unit)]),
        ),
        reachability: Reachability::Required,
    });

    let limc = a.cref(ns.o, wck, c1, u64_type());
    let lim = a.op(
        ns.o,
        wck,
        Opcode::IntShlChecked,
        vec![op_result(limc), pav(k_w)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let wcmp = a.id(ns.b);
    let m_lim = a.param(ns.p, wcmp, ParameterRole::Block, u64_type());
    let m_val = a.param(ns.p, wcmp, ParameterRole::Block, u64_type());
    let m_unit = a.param(ns.p, wcmp, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: wck,
        function: fid,
        parameters: vec![k_val, k_w, k_unit],
        operations: vec![limc, lim],
        terminator: switch(
            op_result(lim),
            vec![
                (
                    BuiltinCase::Ok,
                    wcmp,
                    vec![SwitchArgument::CasePayload, sav(k_val), sav(k_unit)],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let over = a.op(
        ns.o,
        wcmp,
        Opcode::GreaterEqual,
        vec![pav(m_val), pav(m_lim)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: wcmp,
        function: fid,
        parameters: vec![m_lim, m_val, m_unit],
        operations: vec![over],
        terminator: cond(
            op_result(over),
            edge(e_err, Vec::new()),
            edge(loop0, vec![pav(m_val), pav(m_unit)]),
        ),
        reachability: Reachability::Required,
    });

    let looop = a.id(ns.b);
    let l_v = a.param(ns.p, looop, ParameterRole::Block, u64_type());
    let l_acc = a.param(ns.p, looop, ParameterRole::Block, u8vec_type());
    let l_unit = a.param(ns.p, looop, ParameterRole::Block, TypeExpr::Unit);
    let e = a.op(
        ns.o,
        loop0,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: loop0,
        function: fid,
        parameters: vec![z_val, z_unit],
        operations: vec![e],
        terminator: branch(edge(looop, vec![pav(z_val), op_result(e), pav(z_unit)])),
        reachability: Reachability::Required,
    });

    let rc = a.cref(ns.o, looop, c128, u64_type());
    let r = a.op(
        ns.o,
        looop,
        Opcode::IntRemChecked,
        vec![pav(l_v), op_result(rc)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let div = a.id(ns.b);
    let d_r = a.param(ns.p, div, ParameterRole::Block, u64_type());
    let d_v = a.param(ns.p, div, ParameterRole::Block, u64_type());
    let d_acc = a.param(ns.p, div, ParameterRole::Block, u8vec_type());
    let d_unit = a.param(ns.p, div, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: looop,
        function: fid,
        parameters: vec![l_v, l_acc, l_unit],
        operations: vec![rc, r],
        terminator: switch(
            op_result(r),
            vec![
                (
                    BuiltinCase::Ok,
                    div,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(l_v),
                        sav(l_acc),
                        sav(l_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let qc = a.cref(ns.o, div, c128, u64_type());
    let q = a.op(
        ns.o,
        div,
        Opcode::IntDivChecked,
        vec![pav(d_v), op_result(qc)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let contq = a.id(ns.b);
    let q_r = a.param(ns.p, contq, ParameterRole::Block, u64_type());
    let q_q = a.param(ns.p, contq, ParameterRole::Block, u64_type());
    let q_acc = a.param(ns.p, contq, ParameterRole::Block, u8vec_type());
    let q_unit = a.param(ns.p, contq, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: div,
        function: fid,
        parameters: vec![d_r, d_v, d_acc, d_unit],
        operations: vec![qc, q],
        terminator: switch(
            op_result(q),
            vec![
                (
                    BuiltinCase::Ok,
                    contq,
                    vec![
                        sav(d_r),
                        SwitchArgument::CasePayload,
                        sav(d_acc),
                        sav(d_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    // Narrowing ladder over the 8 bits of t = r (+128 on continuation).
    let mut narrow: Vec<EntityId> = Vec::new();
    let mut narrow_params: Vec<Vec<EntityId>> = Vec::new();
    for _ in 0..8 {
        let nb = a.id(ns.b);
        let t = a.param(ns.p, nb, ParameterRole::Block, u64_type());
        let b = a.param(ns.p, nb, ParameterRole::Block, u8_type());
        let qq = a.param(ns.p, nb, ParameterRole::Block, u64_type());
        let acc = a.param(ns.p, nb, ParameterRole::Block, u8vec_type());
        let u = a.param(ns.p, nb, ParameterRole::Block, TypeExpr::Unit);
        narrow.push(nb);
        narrow_params.push(vec![t, b, qq, acc, u]);
    }
    let push = a.id(ns.b);
    let p_b = a.param(ns.p, push, ParameterRole::Block, u8_type());
    let p_q = a.param(ns.p, push, ParameterRole::Block, u64_type());
    let p_acc = a.param(ns.p, push, ParameterRole::Block, u8vec_type());
    let p_unit = a.param(ns.p, push, ParameterRole::Block, TypeExpr::Unit);
    let mut next_of: Vec<EntityId> = vec![eid(0, 0); 8];
    for k in 0..8 {
        next_of[k] = if k == 7 { push } else { narrow[k + 1] };
    }

    let t128 = a.id(ns.b);
    let tt_r = a.param(ns.p, t128, ParameterRole::Block, u64_type());
    let tt_q = a.param(ns.p, t128, ParameterRole::Block, u64_type());
    let tt_acc = a.param(ns.p, t128, ParameterRole::Block, u8vec_type());
    let tt_unit = a.param(ns.p, t128, ParameterRole::Block, TypeExpr::Unit);
    let ccc = a.cref(ns.o, contq, c0, u64_type());
    let cc = a.op(
        ns.o,
        contq,
        Opcode::NotEqual,
        vec![pav(q_q), op_result(ccc)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let zb = a.cref(ns.o, contq, b0, u8_type());
    a.blocks.push(Block {
        entity_id: contq,
        function: fid,
        parameters: vec![q_r, q_q, q_acc, q_unit],
        operations: vec![ccc, cc, zb],
        terminator: cond(
            op_result(cc),
            edge(t128, vec![pav(q_r), pav(q_q), pav(q_acc), pav(q_unit)]),
            edge(
                narrow[0],
                vec![pav(q_r), op_result(zb), pav(q_q), pav(q_acc), pav(q_unit)],
            ),
        ),
        reachability: Reachability::Required,
    });
    let tc = a.cref(ns.o, t128, c128, u64_type());
    let t = a.op(
        ns.o,
        t128,
        Opcode::IntAddChecked,
        vec![pav(tt_r), op_result(tc)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let zb2 = a.cref(ns.o, t128, b0, u8_type());
    a.blocks.push(Block {
        entity_id: t128,
        function: fid,
        parameters: vec![tt_r, tt_q, tt_acc, tt_unit],
        operations: vec![tc, t, zb2],
        terminator: switch(
            op_result(t),
            vec![
                (
                    BuiltinCase::Ok,
                    narrow[0],
                    vec![
                        SwitchArgument::CasePayload,
                        oav(zb2),
                        sav(tt_q),
                        sav(tt_acc),
                        sav(tt_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    for k in 0..8 {
        let bit = 7 - k;
        // NOTE: pow_u64 is descending ([0] = 128), so the test constant
        // for bit (7-k) is pow_u64[k]; bit_u8 is ascending, so the u8
        // addend is bit_u8[7-k].
        let nb = narrow[k];
        let ps = &narrow_params[k];
        let ny = a.id(ns.b);
        let y_t = a.param(ns.p, ny, ParameterRole::Block, u64_type());
        let y_b = a.param(ns.p, ny, ParameterRole::Block, u8_type());
        let y_q = a.param(ns.p, ny, ParameterRole::Block, u64_type());
        let y_acc = a.param(ns.p, ny, ParameterRole::Block, u8vec_type());
        let y_u = a.param(ns.p, ny, ParameterRole::Block, TypeExpr::Unit);
        let nbb = a.id(ns.b);
        let z_t = a.param(ns.p, nbb, ParameterRole::Block, u64_type());
        let z_b = a.param(ns.p, nbb, ParameterRole::Block, u8_type());
        let z_q = a.param(ns.p, nbb, ParameterRole::Block, u64_type());
        let z_acc = a.param(ns.p, nbb, ParameterRole::Block, u8vec_type());
        let z_u = a.param(ns.p, nbb, ParameterRole::Block, TypeExpr::Unit);
        let cmpc = a.cref(ns.o, nb, pow_u64[k], u64_type());
        let cmp = a.op(
            ns.o,
            nb,
            Opcode::GreaterEqual,
            vec![pav(ps[0]), op_result(cmpc)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let fwd = |ids: &[EntityId]| ids.iter().map(|i| pav(*i)).collect::<Vec<_>>();
        // The bit-0 test falls through to PUSH, which takes
        // (b, q, acc, unit): the remainder drops out, the byte is done.
        let no_edge = if k == 7 {
            edge(
                next_of[k],
                vec![pav(ps[1]), pav(ps[2]), pav(ps[3]), pav(ps[4])],
            )
        } else {
            edge(next_of[k], fwd(ps))
        };
        a.blocks.push(Block {
            entity_id: nb,
            function: fid,
            parameters: ps.clone(),
            operations: vec![cmpc, cmp],
            terminator: cond(op_result(cmp), edge(ny, fwd(ps)), no_edge),
            reachability: Reachability::Required,
        });
        let t2c = a.cref(ns.o, ny, pow_u64[k], u64_type());
        let t2 = a.op(
            ns.o,
            ny,
            Opcode::IntSubChecked,
            vec![pav(y_t), op_result(t2c)],
            vec![arith_result(u64_type())],
            Immediate::None,
        );
        a.blocks.push(Block {
            entity_id: ny,
            function: fid,
            parameters: vec![y_t, y_b, y_q, y_acc, y_u],
            operations: vec![t2c, t2],
            terminator: switch(
                op_result(t2),
                vec![
                    (
                        BuiltinCase::Ok,
                        nbb,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(y_b),
                            sav(y_q),
                            sav(y_acc),
                            sav(y_u),
                        ],
                    ),
                    (BuiltinCase::Err, trap, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
        let b2c = a.cref(ns.o, nbb, bit_u8[bit], u8_type());
        let b2 = a.op(
            ns.o,
            nbb,
            Opcode::IntAddChecked,
            vec![pav(z_b), op_result(b2c)],
            vec![arith_result(u8_type())],
            Immediate::None,
        );
        a.blocks.push(Block {
            entity_id: nbb,
            function: fid,
            parameters: vec![z_t, z_b, z_q, z_acc, z_u],
            operations: vec![b2c, b2],
            terminator: switch(
                op_result(b2),
                vec![
                    (
                        BuiltinCase::Ok,
                        next_of[k],
                        if k == 7 {
                            vec![SwitchArgument::CasePayload, sav(z_q), sav(z_acc), sav(z_u)]
                        } else {
                            vec![
                                sav(z_t),
                                SwitchArgument::CasePayload,
                                sav(z_q),
                                sav(z_acc),
                                sav(z_u),
                            ]
                        },
                    ),
                    (BuiltinCase::Err, trap, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
    }

    let nb_push = a.op(
        ns.o,
        push,
        Opcode::AdapterInvoke,
        vec![pav(p_acc), pav(p_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    let more = a.id(ns.b);
    let mm_q = a.param(ns.p, more, ParameterRole::Block, u64_type());
    let mm_acc = a.param(ns.p, more, ParameterRole::Block, u8vec_type());
    let mm_unit = a.param(ns.p, more, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: push,
        function: fid,
        parameters: vec![p_b, p_q, p_acc, p_unit],
        operations: vec![nb_push],
        terminator: switch(
            op_result(nb_push),
            vec![
                (
                    BuiltinCase::Ok,
                    more,
                    vec![sav(p_q), SwitchArgument::CasePayload, sav(p_unit)],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let donec = a.cref(ns.o, more, c0, u64_type());
    let done = a.op(
        ns.o,
        more,
        Opcode::Equal,
        vec![pav(mm_q), op_result(donec)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let fin = a.id(ns.b);
    let f_acc = a.param(ns.p, fin, ParameterRole::Block, u8vec_type());
    let f_unit = a.param(ns.p, fin, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: more,
        function: fid,
        parameters: vec![mm_q, mm_acc, mm_unit],
        operations: vec![donec, done],
        terminator: cond(
            op_result(done),
            edge(fin, vec![pav(mm_acc), pav(mm_unit)]),
            edge(looop, vec![pav(mm_q), pav(mm_acc), pav(mm_unit)]),
        ),
        reachability: Reachability::Required,
    });
    let out = a.op(
        ns.o,
        fin,
        Opcode::AdapterInvoke,
        vec![pav(f_unit), pav(f_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    let fok = a.id(ns.b);
    let o_b = a.param(ns.p, fok, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: fin,
        function: fid,
        parameters: vec![f_acc, f_unit],
        operations: vec![out],
        terminator: switch(
            op_result(out),
            vec![
                (BuiltinCase::Ok, fok, vec![SwitchArgument::CasePayload]),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let okv = a.op(
        ns.o,
        fok,
        Opcode::ResultOk,
        vec![pav(o_b)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: fok,
        function: fid,
        parameters: vec![o_b],
        operations: vec![okv],
        terminator: ret(op_result(okv)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![n_val, n_w, n_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

// ── images, admission, execution ─────────────────────────────────────

struct Image {
    types: sley_check::TypeEnvironment,
    entry: FunctionGraph,
    functions: Vec<FunctionGraph>,
    parameters: Vec<Parameter>,
    blocks: Vec<Block>,
    operations: Vec<Operation>,
    adapters: Vec<AdapterImport>,
    constants: Vec<ConstantDefinition>,
}

fn codec_limits() -> sley_vm::ExecutionLimits {
    sley_vm::ExecutionLimits {
        max_instructions: 100_000,
        max_fuel: 1_000_000,
        max_value_units: 1_000_000,
        max_output_units: 100_000,
        cancel_at_fuel: None,
    }
}

fn admit(image: &Image) -> (sley_vm::ExecutionPackage, sley_vm::ApprovedExecutionPackage) {
    use sley_vm::{
        approve_package_v2,
        bootstrap::{BootstrapProfileInput, BootstrapProfileVersion},
    };
    let lowered = sley_vm::lower_function(sley_vm::LoweringInput {
        types: &image.types,
        function: &image.entry,
        parameters: &image.parameters,
        blocks: &image.blocks,
        operations: &image.operations,
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        constants: &image.constants,
        globals: &[],
        functions: &image.functions,
        contracts: &[],
        adapters: &image.adapters,
    })
    .expect("uvar image lowers under the reference lowerer");
    let gate = sley_vm::bootstrap::judge_bootstrap_profile(&BootstrapProfileInput {
        types: &image.types,
        schema_epoch: epoch(),
        entry: &image.entry,
        presented_image_bytes: &lowered.bytes,
        functions: &image.functions,
        parameters: &image.parameters,
        blocks: &image.blocks,
        operations: &image.operations,
        adapters: &image.adapters,
        constants: &image.constants,
        profile_version: BootstrapProfileVersion::V2,
    })
    .expect("uvar image admits under V2");
    let limits = codec_limits();
    let package = sley_vm::ExecutionPackage {
        image_bytes: lowered.bytes.clone(),
        constants: image.constants.clone(),
        type_definitions: Vec::new(),
        imports: image.adapters.clone(),
        globals: Vec::new(),
        contracts: Vec::new(),
        entry: image.entry.entity_id,
        schema_epoch: epoch(),
        state_root: root(),
        profile: sley_vm::CacheProfile::EXTENDED_V1,
        admitted_limits: limits,
        gate_operation_count: gate.operation_count(),
        gate_bridge_uses: gate.bridge_uses(),
        gate_closure_fingerprints: gate.closure_fingerprints().to_vec(),
    };
    // C0 seed minting route (declared): the staged authority judges plus
    // reference re-lowers before minting; excluded from clean stages.
    let closure = sley_vm::V2Closure {
        types: &image.types,
        schema_epoch: epoch(),
        state_root: root(),
        entry: image.entry.entity_id,
        functions: &image.functions,
        parameters: &image.parameters,
        blocks: &image.blocks,
        operations: &image.operations,
        adapters: &image.adapters,
        constants: &image.constants,
        globals: &[],
        contracts: &[],
    };
    let (digests, receipt, report) =
        sley_vm::admit_v2_package(&closure, &package).expect("authority admits uvar image");
    assert_eq!(
        receipt.profile_digest(),
        &sley_vm::BOOTSTRAP_PROFILE_2_DIGEST,
        "uvar receipt binds the successor profile"
    );
    let approved =
        approve_package_v2(&package, &digests, receipt, &report).expect("v2 approves uvar image");
    (package, approved)
}

fn decode_image() -> Image {
    use sley_vm::host_abi::BRIDGE_CODE_B2V1;
    let mut a = Asm::new();
    let ns = Ns {
        k: 1,
        p: 2,
        b: 3,
        o: 4,
    };
    let fid = eid(9, 1);
    let (graph, _) = build_decode(&mut a, ns, fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![frozen_import(
            BRIDGE_CODE_B2V1,
            TypeExpr::Bytes,
            u8vec_type(),
        )],
        constants: a.constants,
    }
}

fn exact_image() -> Image {
    use sley_vm::host_abi::BRIDGE_CODE_B2V1;
    let mut a = Asm::new();
    // Decode subgraph and exact wrapper share one closure inventory
    // under disjoint namespaces.
    let dns = Ns {
        k: 11,
        p: 12,
        b: 13,
        o: 14,
    };
    let xns = Ns {
        k: 11,
        p: 16,
        b: 17,
        o: 18,
    };
    let decode_fid = eid(9, 2);
    let exact_fid = eid(9, 3);
    let (decode_graph, dc) = build_decode(&mut a, dns, decode_fid);
    let exact_graph = build_exact(&mut a, xns, exact_fid, decode_fid, &dc);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: exact_graph.clone(),
        functions: vec![exact_graph, decode_graph],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![frozen_import(
            BRIDGE_CODE_B2V1,
            TypeExpr::Bytes,
            u8vec_type(),
        )],
        constants: a.constants,
    }
}

fn encode_image() -> Image {
    use sley_vm::host_abi::{BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let mut a = Asm::new();
    let ns = Ns {
        k: 21,
        p: 22,
        b: 23,
        o: 24,
    };
    let fid = eid(9, 4);
    let graph = build_encode(&mut a, ns, fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
        ],
        constants: a.constants,
    }
}

fn bytes_input(bytes: &[u8]) -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Bytes,
        data: ConstData::Bytes(bytes.to_vec()),
    }
}

fn u64_input(value: u128) -> ConstValue {
    ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(value),
    }
}

fn u32_input(value: u128) -> ConstValue {
    ConstValue {
        value_type: u32_type(),
        data: ConstData::UInt(value),
    }
}

fn unit_input() -> ConstValue {
    ConstValue {
        value_type: TypeExpr::Unit,
        data: ConstData::Unit,
    }
}

fn execute(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    inputs: Vec<ConstValue>,
) -> sley_vm::ExecutionOutcome {
    sley_vm::execute_approved_package_v2(
        package,
        approved,
        sley_vm::ExecutionRequest {
            inputs,
            limits: codec_limits(),
        },
    )
    .expect("v2 executes uvar image")
}

fn hex_decode(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "even hex length");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
        .collect()
}

/// Assert a successful decode carrying exactly (`value`, `new_pos`).
fn assert_decode_ok(outcome: &sley_vm::ExecutionOutcome, value: u128, new_pos: u128) -> u64 {
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                ConstData::Sequence(items) => {
                    assert_eq!(items.len(), 2, "decode Ok carries (value, new_pos)");
                    assert_eq!(items[0].data, ConstData::UInt(value), "decoded value");
                    assert_eq!(items[1].data, ConstData::UInt(new_pos), "new position");
                    outcome.fuel_used
                }
                other => panic!("decode Ok must carry a tuple, got {other:?}"),
            },
            other => panic!("decode must succeed, got {other:?}"),
        },
        other => panic!("decode must succeed, got {other:?}"),
    }
}

/// Assert a typed refusal carrying exactly the expected `SCB_*` code.
fn assert_refusal(outcome: &sley_vm::ExecutionOutcome, code: &str) -> u64 {
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Err(payload)) => match &payload.data {
                ConstData::Bytes(bytes) => {
                    assert_eq!(bytes, code.as_bytes(), "refusal must carry the exact code");
                    outcome.fuel_used
                }
                other => panic!("refusal must carry Bytes, got {other:?}"),
            },
            other => panic!("must refuse, got {other:?}"),
        },
        other => panic!("refusal must be a typed value, got {other:?}"),
    }
}

fn assert_encode_ok(outcome: &sley_vm::ExecutionOutcome, expected: &[u8]) -> u64 {
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                ConstData::Bytes(bytes) => {
                    assert_eq!(bytes, expected, "emitted bytes must be canonical");
                    outcome.fuel_used
                }
                other => panic!("encode Ok must carry Bytes, got {other:?}"),
            },
            other => panic!("encode must succeed, got {other:?}"),
        },
        other => panic!("encode must succeed, got {other:?}"),
    }
}

fn ref_decode(bytes: &[u8], width: u8) -> Result<(u64, usize), String> {
    let mut cursor = sley_scb1::ScbValueCursor::new(bytes).expect("test input in bounds");
    match cursor.read_uvar(width) {
        Ok(value) => Ok((value, cursor.position())),
        Err(error) => Err(error.code().to_string()),
    }
}

// ── tests ────────────────────────────────────────────────────────────

#[test]
fn uvar_frozen_accepted_decode_and_encode() {
    let (package, approved) = admit(&decode_image());
    let (enc_package, enc_approved) = admit(&encode_image());
    // (id, value, expected_hex): conformance/scb1/v1/accepted.json uvar
    // entries, mirrored here so the Sley image answers fixed vectors.
    let vectors: [(u64, &str); 5] = [
        (0, "00"),
        (1, "01"),
        (127, "7f"),
        (128, "8001"),
        (300, "ac02"),
    ];
    for (value, expected_hex) in vectors {
        let bytes = hex_decode(expected_hex);
        // Sley decodes the fixed bytes.
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&bytes),
                u64_input(0),
                u32_input(64),
                unit_input(),
            ],
        );
        assert_decode_ok(
            &outcome,
            u128::from(value),
            u128::try_from(bytes.len()).expect("length fits u128"),
        );
        // The reference agrees on the same bytes (reference role).
        assert_eq!(
            ref_decode(&bytes, 64).expect("reference accepts frozen vector"),
            (value, bytes.len()),
            "reference agrees on frozen vector"
        );
        // Sley emits the fixed canonical bytes.
        let enc = execute(
            &enc_package,
            &enc_approved,
            vec![u64_input(u128::from(value)), u32_input(64), unit_input()],
        );
        assert_encode_ok(&enc, &bytes);
        // The reference encoder agrees byte-for-byte (reference role).
        assert_eq!(
            sley_scb1::encode_uvar(value),
            bytes,
            "reference encoder agrees on frozen vector"
        );
    }
}

#[test]
fn uvar_frozen_rejected_codes_match() {
    let (package, approved) = admit(&decode_image());
    let (exact_package, exact_approved) = admit(&exact_image());
    // (id, declared width, input_hex, expected_code): rejected.json uvar
    // entries plus the trailing case through the exact wrapper.
    let vectors: [(&str, u64, &str, &str); 4] = [
        ("uvar-nonminimal-zero", 64, "8000", "SCB_VARINT_NON_MINIMAL"),
        ("uvar-nonminimal-one", 64, "8100", "SCB_VARINT_NON_MINIMAL"),
        ("uint8-overflow", 8, "8002", "SCB_INTEGER_OVERFLOW"),
        ("value-trailing-byte", 64, "0100", "SCB_TRAILING_BYTES"),
    ];
    for (id, width, input_hex, code) in vectors {
        let bytes = hex_decode(input_hex);
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&bytes),
                u64_input(0),
                u32_input(u128::from(width)),
                unit_input(),
            ],
        );
        if code == "SCB_TRAILING_BYTES" {
            // Trailing is the exact wrapper's check: bare decode
            // succeeds with new_pos 1, exact refuses TRAILING.
            assert_decode_ok(&outcome, 1, 1);
            let exact = execute(
                &exact_package,
                &exact_approved,
                vec![
                    bytes_input(&bytes),
                    u64_input(0),
                    u32_input(u128::from(width)),
                    unit_input(),
                ],
            );
            assert_refusal(&exact, code);
        } else {
            assert_refusal(&outcome, code);
        }
        // The reference returns the same code on the same bytes, except
        // trailing (a wrapper-level check, not a cursor code).
        if code != "SCB_TRAILING_BYTES" {
            assert_eq!(
                ref_decode(&bytes, u8::try_from(width).expect("test width fits u8")),
                Err(code.to_string()),
                "reference agrees on {id}"
            );
        }
    }
}

#[test]
fn uvar_truncation_width_and_boundary_edges() {
    let (package, approved) = admit(&decode_image());
    let (enc_package, enc_approved) = admit(&encode_image());
    let decode = |hex: &str, width: u128| {
        execute(
            &package,
            &approved,
            vec![
                bytes_input(&hex_decode(hex)),
                u64_input(0),
                u32_input(width),
                unit_input(),
            ],
        )
    };
    // Truncation: empty and lone-continuation inputs.
    assert_refusal(&decode("", 64), "SCB_LENGTH_OVERFLOW");
    assert_refusal(&decode("80", 64), "SCB_LENGTH_OVERFLOW");
    // Width gate: 0 and >64 refuse before reading anything.
    assert_refusal(&decode("00", 0), "SCB_INTEGER_OVERFLOW");
    assert_refusal(&decode("00", 65), "SCB_INTEGER_OVERFLOW");
    // Width-8 edges: 255 fits, 256 does not.
    assert_decode_ok(&decode("ff01", 8), 255, 2);
    assert_refusal(&decode("8002", 8), "SCB_INTEGER_OVERFLOW");
    // Full u64 range: max decodes, encodes back byte-identically.
    let max_hex = "ffffffffffffffffff01";
    assert_decode_ok(&decode(max_hex, 64), u128::from(u64::MAX), 10);
    let enc = execute(
        &enc_package,
        &enc_approved,
        vec![u64_input(u128::from(u64::MAX)), u32_input(64), unit_input()],
    );
    assert_encode_ok(&enc, &hex_decode(max_hex));
    // Shift-63 payload 2 refuses (would set bit 64 of a u64).
    assert_refusal(&decode("ffffffffffffffffff02", 64), "SCB_INTEGER_OVERFLOW");
    // Eleven-byte forms: trailing-zero group is non-minimal, nonzero
    // payload overflows.
    assert_refusal(
        &decode("8080808080808080808000", 64),
        "SCB_VARINT_NON_MINIMAL",
    );
    assert_refusal(
        &decode("8080808080808080808001", 64),
        "SCB_INTEGER_OVERFLOW",
    );
    // Eleven continuations with no terminator: the shift bound refuses.
    // Recorded divergence OREF-1: the independent Python oracle reports
    // SCB_LENGTH_OVERFLOW here while the Rust reference (mirrored by
    // this image) reports SCB_INTEGER_OVERFLOW.
    assert_refusal(
        &decode("8080808080808080808080", 64),
        "SCB_INTEGER_OVERFLOW",
    );
    // Every edge above agrees with the Rust reference code-for-code.
    for (hex, width, expected) in [
        ("", 64, Err("SCB_LENGTH_OVERFLOW".to_string())),
        ("80", 64, Err("SCB_LENGTH_OVERFLOW".to_string())),
        ("00", 0, Err("SCB_INTEGER_OVERFLOW".to_string())),
        ("00", 65, Err("SCB_INTEGER_OVERFLOW".to_string())),
        ("ff01", 8, Ok((255, 2))),
        ("8002", 8, Err("SCB_INTEGER_OVERFLOW".to_string())),
        (max_hex, 64, Ok((u64::MAX, 10))),
        (
            "ffffffffffffffffff02",
            64,
            Err("SCB_INTEGER_OVERFLOW".to_string()),
        ),
        (
            "8080808080808080808000",
            64,
            Err("SCB_VARINT_NON_MINIMAL".to_string()),
        ),
        (
            "8080808080808080808001",
            64,
            Err("SCB_INTEGER_OVERFLOW".to_string()),
        ),
        (
            "8080808080808080808080",
            64,
            Err("SCB_INTEGER_OVERFLOW".to_string()),
        ),
    ] {
        assert_eq!(
            ref_decode(&hex_decode(hex), width),
            expected,
            "reference pins {hex} at width {width}"
        );
    }
}

#[test]
fn uvar_encode_width_rules_and_canonical_bytes() {
    let (package, approved) = admit(&encode_image());
    let encode = |value: u128, width: u128| {
        execute(
            &package,
            &approved,
            vec![u64_input(value), u32_input(width), unit_input()],
        )
    };
    // Width gate mirrors decoding.
    assert_refusal(&encode(0, 0), "SCB_INTEGER_OVERFLOW");
    assert_refusal(&encode(0, 65), "SCB_INTEGER_OVERFLOW");
    // Symmetric range rule: width < 64 requires value < 2^width.
    assert_encode_ok(&encode(0, 1), &[0x00]);
    assert_encode_ok(&encode(1, 1), &[0x01]);
    assert_refusal(&encode(2, 1), "SCB_INTEGER_OVERFLOW");
    assert_encode_ok(&encode(255, 8), &[0xff, 0x01]);
    assert_refusal(&encode(256, 8), "SCB_INTEGER_OVERFLOW");
    assert_encode_ok(&encode(300, 9), &[0xac, 0x02]);
    assert_refusal(&encode(512, 9), "SCB_INTEGER_OVERFLOW");
    // Canonical bytes across magnitudes (reference encoder agrees).
    for value in [
        0u64,
        1,
        63,
        64,
        127,
        128,
        255,
        300,
        16383,
        16384,
        65535,
        65536,
        2_097_151,
        268_435_455,
        u64::from(u32::MAX),
        u64::from(u32::MAX) + 1,
        u64::MAX - 1,
        u64::MAX,
    ] {
        let expected = sley_scb1::encode_uvar(value);
        assert_encode_ok(&encode(u128::from(value), 64), &expected);
    }
}

#[test]
fn uvar_fresh_vectors_decode_and_encode() {
    // Fresh vectors authored after the scaffold checkpoint 72fff72 and
    // confirmed independently with:
    //   uv run --project oracle/scb1 --frozen sley2-scb1-oracle check
    //     --accepted /tmp/uvar_fresh_accepted.json
    //     --rejected /tmp/uvar_fresh_rejected.json
    // Oracle result: accepted 20/20 byte+decode agreement; rejected 6/8
    // code agreement with 2 unsupported-oracle-path entries (UInt16 and
    // UInt32 declared types, which the oracle runner does not implement;
    // those two Sley paths are covered here against the Rust reference
    // instead). Recorded as OREF-2 in the manifest; no supported case
    // disagrees. The Sley images below never saw these inputs at
    // construction time.
    let (package, approved) = admit(&decode_image());
    let (enc_package, enc_approved) = admit(&encode_image());
    let accepted: [(u64, &str); 20] = [
        (2, "02"),
        (63, "3f"),
        (64, "40"),
        (8191, "ff3f"),
        (8192, "8040"),
        (16383, "ff7f"),
        (16384, "808001"),
        (1_000_000, "c0843d"),
        (16_777_215, "ffffff07"),
        (16_777_216, "80808008"),
        (u64::from(u32::MAX), "ffffffff0f"),
        (4_294_967_296, "8080808010"),
        (1 << 40, "808080808020"),
        ((1 << 56) - 1, "ffffffffffffff7f"),
        (1 << 56, "808080808080808001"),
        ((1u64 << 63) - 1, "ffffffffffffffff7f"),
        (1u64 << 63, "80808080808080808001"),
        (u64::MAX - 1, "feffffffffffffffff01"),
        (123_456_789, "959aef3a"),
        (9_223_372_036_854_775_807, "ffffffffffffffff7f"),
    ];
    for (value, expected_hex) in accepted {
        let bytes = hex_decode(expected_hex);
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&bytes),
                u64_input(0),
                u32_input(64),
                unit_input(),
            ],
        );
        assert_decode_ok(
            &outcome,
            u128::from(value),
            u128::try_from(bytes.len()).expect("length fits u128"),
        );
        let enc = execute(
            &enc_package,
            &enc_approved,
            vec![u64_input(u128::from(value)), u32_input(64), unit_input()],
        );
        assert_encode_ok(&enc, &bytes);
        assert_eq!(ref_decode(&bytes, 64), Ok((value, bytes.len())));
    }
    let rejected: [(u64, &str, &str); 8] = [
        (64, "818000", "SCB_VARINT_NON_MINIMAL"),
        (64, "808000", "SCB_VARINT_NON_MINIMAL"),
        (64, "ff8000", "SCB_VARINT_NON_MINIMAL"),
        (8, "ff02", "SCB_INTEGER_OVERFLOW"),
        (16, "808004", "SCB_INTEGER_OVERFLOW"),
        (32, "8080808010", "SCB_INTEGER_OVERFLOW"),
        (64, "81", "SCB_LENGTH_OVERFLOW"),
        (64, "ac", "SCB_LENGTH_OVERFLOW"),
    ];
    for (width, input_hex, code) in rejected {
        let bytes = hex_decode(input_hex);
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&bytes),
                u64_input(0),
                u32_input(u128::from(width)),
                unit_input(),
            ],
        );
        assert_refusal(&outcome, code);
        assert_eq!(
            ref_decode(&bytes, u8::try_from(width).expect("test width fits u8")),
            Err(code.to_string())
        );
    }
}

fn lcg_next(state: &mut u64) -> u64 {
    // Deterministic test-side PRNG (SplitMix64 one round). All draws
    // happen at test time, after the Sley images are built above, so no
    // Sley logic could have been specialized to these inputs.
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[test]
fn uvar_runtime_random_roundtrip_matches_reference() {
    // Images are built first; every input below is drawn afterwards.
    let (package, approved) = admit(&decode_image());
    let (enc_package, enc_approved) = admit(&encode_image());
    let mut state = 0x1234_5678_9ABC_DEF0u64;
    let mut boundaries = vec![
        0u64,
        1,
        127,
        128,
        255,
        256,
        16383,
        16384,
        65535,
        65536,
        u64::from(u32::MAX),
        u64::from(u32::MAX) + 1,
        (1 << 53) - 1,
        1 << 53,
        (1 << 63) - 1,
        1 << 63,
        u64::MAX - 1,
        u64::MAX,
    ];
    for _ in 0..200 {
        boundaries.push(lcg_next(&mut state));
    }
    for value in boundaries {
        let bytes = sley_scb1::encode_uvar(value);
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&bytes),
                u64_input(0),
                u32_input(64),
                unit_input(),
            ],
        );
        assert_decode_ok(
            &outcome,
            u128::from(value),
            u128::try_from(bytes.len()).expect("length fits u128"),
        );
        let enc = execute(
            &enc_package,
            &enc_approved,
            vec![u64_input(u128::from(value)), u32_input(64), unit_input()],
        );
        assert_encode_ok(&enc, &bytes);
    }
}

#[test]
fn uvar_runtime_random_mutations_agree_code_for_code() {
    let (package, approved) = admit(&decode_image());
    let mut state = 0xFEDC_BA98_7654_3210u64;
    // Widths 1..=64 drawn at runtime too.
    for round in 0..200 {
        let value = lcg_next(&mut state);
        let mut bytes = sley_scb1::encode_uvar(value);
        let width = 1 + u8::try_from(lcg_next(&mut state) % 64).expect("lcg remainder fits u8");
        match round % 4 {
            0 => {
                // Flip one random bit.
                let at =
                    usize::try_from(lcg_next(&mut state)).expect("lcg fits usize") % bytes.len();
                let bit =
                    1u8 << u32::try_from(lcg_next(&mut state) % 8).expect("lcg remainder fits u32");
                bytes[at] ^= bit;
            }
            1 => {
                // Truncate one byte (possibly to empty).
                bytes.pop();
            }
            2 => {
                // Append one random byte.
                bytes.push(u8::try_from(lcg_next(&mut state) % 256).expect("lcg byte fits u8"));
            }
            _ => {
                // Prepend one random byte (shifts framing).
                let mut shifted =
                    vec![u8::try_from(lcg_next(&mut state) % 256).expect("lcg byte fits u8")];
                shifted.extend_from_slice(&bytes);
                bytes = shifted;
            }
        }
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&bytes),
                u64_input(0),
                u32_input(u128::from(width)),
                unit_input(),
            ],
        );
        match ref_decode(&bytes, width) {
            Ok((v, pos)) => {
                assert_decode_ok(
                    &outcome,
                    u128::from(v),
                    u128::try_from(pos).expect("position fits u128"),
                );
            }
            Err(code) => {
                assert_refusal(&outcome, &code);
            }
        }
    }
}

#[test]
fn uvar_exact_wrapper_checks_trailing_and_passthrough() {
    let (package, approved) = admit(&exact_image());
    let exact = |hex: &str| {
        execute(
            &package,
            &approved,
            vec![
                bytes_input(&hex_decode(hex)),
                u64_input(0),
                u32_input(64),
                unit_input(),
            ],
        )
    };
    // Exact input passes through with its position.
    assert_decode_ok(&exact("ac02"), 300, 2);
    assert_decode_ok(&exact("00"), 0, 1);
    // Any trailing byte refuses with the exact trailing code.
    assert_refusal(&exact("0100"), "SCB_TRAILING_BYTES");
    assert_refusal(&exact("ac0200"), "SCB_TRAILING_BYTES");
    // Decode errors keep precedence over the trailing check.
    assert_refusal(&exact("8000"), "SCB_VARINT_NON_MINIMAL");
    assert_refusal(&exact(""), "SCB_LENGTH_OVERFLOW");
}

#[test]
fn uvar_resource_observations_stay_within_codec_budgets() {
    let (package, approved) = admit(&decode_image());
    let (enc_package, enc_approved) = admit(&encode_image());
    let (exact_package, exact_approved) = admit(&exact_image());
    // Decode fuel across input lengths, all width 64.
    let mut decode_fuel = Vec::new();
    for hex in ["00", "ac02", "ffffffff7f", "ffffffffffffffffff01"] {
        let bytes = hex_decode(hex);
        let outcome = execute(
            &package,
            &approved,
            vec![
                bytes_input(&bytes),
                u64_input(0),
                u32_input(64),
                unit_input(),
            ],
        );
        decode_fuel.push((
            bytes.len(),
            assert_decode_ok(
                &outcome,
                u128::from(ref_decode(&bytes, 64).unwrap().0),
                u128::try_from(bytes.len()).expect("length fits u128"),
            ),
        ));
    }
    let mut encode_fuel = Vec::new();
    for value in [0u64, 300, 16_777_215, u64::MAX] {
        let outcome = execute(
            &enc_package,
            &enc_approved,
            vec![u64_input(u128::from(value)), u32_input(64), unit_input()],
        );
        encode_fuel.push((
            sley_scb1::encode_uvar(value).len(),
            assert_encode_ok(&outcome, &sley_scb1::encode_uvar(value)),
        ));
    }
    let exact_outcome = execute(
        &exact_package,
        &exact_approved,
        vec![
            bytes_input(&hex_decode("ac02")),
            u64_input(0),
            u32_input(64),
            unit_input(),
        ],
    );
    let exact_fuel = assert_decode_ok(&exact_outcome, 300, 2);
    eprintln!(
        "UVAL_FUEL decode(bytes,fuel)={decode_fuel:?} encode(bytes,fuel)={encode_fuel:?} exact={exact_fuel}"
    );
    // Hard budget assertions: everything must stay far inside the
    // codec limits (100k instructions / 1M fuel).
    for (_, fuel) in decode_fuel.iter().chain(encode_fuel.iter()) {
        assert!(*fuel < 100_000, "codec fuel stays far inside budget");
    }
    assert!(
        exact_fuel < 100_000,
        "exact wrapper stays far inside budget"
    );
}

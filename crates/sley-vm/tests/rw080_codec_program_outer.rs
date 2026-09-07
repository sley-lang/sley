//! RW-080 §1.1 program slice 3: SSMC1 `EntityObject` outer-record framing (2-field, opaque body) in Sley.
//!
//! PROVISIONAL C0 SEED CONSTRUCTION - explicitly not accepted runtime
//! authority. Seed-authored (this file is the seed-assembler record)
//! under standing amendment `rw075_correction.operator_override_2026_09_07`.
//! It exercises the v2 admit/approve/execute path for real strict
//! program-framing algorithms through the approved execution boundary.
//!
//! Scope (bounded): SSMC1 `EntityObject` outer Record with required fields
//! 1 (`entity_id` `FixedBytes32`) and 2 (body opaque) only, i.e. the exact
//! `decode_object_record` 2-field path of `sley-mutate/src/object.rs`
//! (count, tags, lens, 32B fixed, opaque body, trailing) plus canonical
//! `encode_object_record` 2-field emission. Two Sley functions:
//! `decode_outer` (payload -> body, opaque) and `encode_outer`
//! (`entity_id` + body -> record). Reuses slice-1 `decode_uvar` /
//! `encode_uvar` via `CallDirect` for every uvar field, so framing errors
//! keep exact reference decision order and codes. Sley owns parsing,
//! field decisions, ordering, and byte emission; bridge uses B2V1/PSH1/
//! V2B1 only (no RHW1 here; digest stays in the envelope slice).
//! Label (tag 3) and fingerprint (tag 4) are explicit scope exclusions
//! (`SSMC_RESERVED_FIELD_PRESENT`, pinned divergence vs reference Ok),
//! never misreported as format errors. `codec_main` legs stay stubs;
//! this is the digest-carrying outer dependency every body decoder needs.
//! Construction provenance and contract basis:
//! machineresearch/sley-2.0/reweave/rw-080-codec-program-outer.md.
//!
//! Deliberate non-goals with reasons (not silent gaps):
//! - `codec_main` legs stay stubs (`rw080_codec_scaffold.rs`): full
//!   program/schema (18 body kinds, label/NFC, fingerprint verifier)
//!   is not yet wired; wiring legs now would overclaim. The units here
//!   are entries of their own approved images through the same boundary.
//! - Label/NFC/fingerprint (tags 3/4): need text/Unicode tables and the
//!   S20-250 verifier; excluded with explicit scope code, not silently.
//! - Body semantics (18 kinds, type/CFG/effect judgments): owned by later
//!   body slices (RW-100 gate), never imported here to make this slice
//!   appear complete.
//! - Errors are the exact `SCB_*`/`SSMC_*` code strings as `Bytes` values.
//!   No code is mapped to the 7-code program-level vocabulary: inventing
//!   equivalences is forbidden. The vocabulary mapping belongs to leg
//!   wiring once full program formats exist.

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
#[allow(dead_code, clippy::too_many_lines)]
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
    dead_code,
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

// ── program outer (EntityObject 2-field, opaque body) ─────────────────
// Sley implementation of `decode_object_record` 2-field path
// (`crates/sley-mutate/src/object.rs:177`): entry
// `decode_outer(payload: Bytes, unit: Unit)`
// returns `Result<(entity_id: Bytes, body: Bytes), Bytes>` where `Err`
// carries the exact `SCB_*` code for framing errors or
// `SSMC_RESERVED_FIELD_PRESENT` for label/fingerprint scope exclusion.
// Reuses slice-1 `decode_uvar` via `CallDirect` for every uvar field
// (count width 64, tags width 32, lens width 64) so framing errors keep
// exact reference decision order and codes. Sley owns parsing,
// field decisions, ordering, and result construction; body stays opaque
// (no body-kind semantics, no RW-100 judgments). Bridge uses B2V1 (entry
// conversion), PSH1 (field-value accumulation), V2B1 (entity_id/body
// output); no RHW1 here (digest stays in the envelope slice).

#[allow(dead_code)]
fn outer_decode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes])),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(
    dead_code,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_outer_decode(a: &mut Asm, ns: Ns, fid: EntityId, decode_fid: EntityId) -> FunctionGraph {
    use sley_vm::host_abi::BRIDGE_CODE_B2V1;
    let bstart = a.blocks.len();
    let res_t = outer_decode_result_type();
    let dec_t = decode_result_type();
    // UInt64 constants.
    let c0 = a.ku64(ns.k, 0);
    let c1 = a.ku64(ns.k, 1);
    let c2 = a.ku64(ns.k, 2);
    let c3 = a.ku64(ns.k, 3);
    let c4 = a.ku64(ns.k, 4);
    let c32 = a.ku64(ns.k, 32);
    let c_max_fields = a.ku64(ns.k, 65_535);
    let c_max = a.ku64(ns.k, 67_108_864);
    // Widths are UInt32 (CallDirect demands u32 width).
    let w32 = a.ku32(ns.k, 32);
    let w64 = a.ku32(ns.k, 64);
    // Exact error-code bytes.
    let e_missing = a.kbytes(ns.k, b"SCB_FIELD_MISSING");
    let e_unknown = a.kbytes(ns.k, b"SCB_FIELD_UNKNOWN");
    let e_dup = a.kbytes(ns.k, b"SCB_FIELD_DUPLICATE");
    let e_order = a.kbytes(ns.k, b"SCB_FIELD_ORDER");
    let e_len = a.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let e_trail = a.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_res = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let e_scope = a.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");

    let p_in = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let p_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let b_missing = err_block(a, ns, fid, res_t.clone(), e_missing);
    let b_unknown = err_block(a, ns, fid, res_t.clone(), e_unknown);
    let b_dup = err_block(a, ns, fid, res_t.clone(), e_dup);
    let b_order = err_block(a, ns, fid, res_t.clone(), e_order);
    let b_len = err_block(a, ns, fid, res_t.clone(), e_len);
    let b_trail = err_block(a, ns, fid, res_t.clone(), e_trail);
    let b_res = err_block(a, ns, fid, res_t.clone(), e_res);
    let b_scope = err_block(a, ns, fid, res_t.clone(), e_scope);
    let trap = trap_block(a, ns, fid);

    // Entry: convert payload bytes to a vector once, up front. Bridge
    // failure means past the 1 MiB cap: documented RESOURCE_LIMIT.
    // Capacity note: the epoch allows 64 MiB (67,108,864 bytes) standalone;
    // this slice accepts exactly what fits the admitted bridge/profile.
    let entry = a.id(ns.b);
    let cv = a.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(p_unit), pav(p_in)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    let b_cnt = a.id(ns.b);
    let q_vec = a.param(ns.p, b_cnt, ParameterRole::Block, u8vec_type());
    let q_in = a.param(ns.p, b_cnt, ParameterRole::Block, TypeExpr::Bytes);
    let q_unit = a.param(ns.p, b_cnt, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![cv],
        terminator: switch(
            op_result(cv),
            vec![
                (
                    BuiltinCase::Ok,
                    b_cnt,
                    vec![SwitchArgument::CasePayload, sav(p_in), sav(p_unit)],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    // Length for later bounds checks.
    let ln = a.op(
        ns.o,
        b_cnt,
        Opcode::VectorLen,
        vec![pav(q_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    // Count: decode_uvar(payload, 0, 64). Uvar errors propagate first,
    // mirroring `read_record_field_count` (`lib.rs:1287`, `object.rs:179`).
    let c0c = a.cref(ns.o, b_cnt, c0, u64_type());
    let wc = a.cref(ns.o, b_cnt, w64, u32_type());
    let cnt_call = a.op(
        ns.o,
        b_cnt,
        Opcode::CallDirect,
        vec![pav(q_in), op_result(c0c), op_result(wc), pav(q_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let c_ok = a.id(ns.b);
    let c_err = a.id(ns.b);
    let c_tup = a.param(
        ns.p,
        c_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let c_ovec = a.param(ns.p, c_ok, ParameterRole::Block, u8vec_type());
    let c_olen = a.param(ns.p, c_ok, ParameterRole::Block, u64_type());
    let c_oin = a.param(ns.p, c_ok, ParameterRole::Block, TypeExpr::Bytes);
    let c_ounit = a.param(ns.p, c_ok, ParameterRole::Block, TypeExpr::Unit);
    let c_ebytes = a.param(ns.p, c_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: b_cnt,
        function: fid,
        parameters: vec![q_vec, q_in, q_unit],
        operations: vec![ln, c0c, wc, cnt_call],
        terminator: switch(
            op_result(cnt_call),
            vec![
                (
                    BuiltinCase::Ok,
                    c_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(q_vec),
                        oav(ln),
                        sav(q_in),
                        sav(q_unit),
                    ],
                ),
                (BuiltinCase::Err, c_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let c_er = a.op(
        ns.o,
        c_err,
        Opcode::ResultErr,
        vec![pav(c_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: c_err,
        function: fid,
        parameters: vec![c_ebytes],
        operations: vec![c_er],
        terminator: ret(op_result(c_er)),
        reachability: Reachability::Required,
    });
    // Unpack count + pos1.
    let c_gv = a.op(
        ns.o,
        c_ok,
        Opcode::TupleGet,
        vec![pav(c_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let c_gp = a.op(
        ns.o,
        c_ok,
        Opcode::TupleGet,
        vec![pav(c_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    // count > 65535 -> RESOURCE_LIMIT (read_record_field_count rule).
    let mf = a.cref(ns.o, c_ok, c_max_fields, u64_type());
    let c_gt = a.op(
        ns.o,
        c_ok,
        Opcode::GreaterThan,
        vec![op_result(c_gv), op_result(mf)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let c_chk_max = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: c_ok,
        function: fid,
        parameters: vec![c_tup, c_ovec, c_olen, c_oin, c_ounit],
        operations: vec![c_gv, c_gp, mf, c_gt],
        terminator: cond(
            op_result(c_gt),
            edge(b_res, Vec::new()),
            edge(
                c_chk_max,
                vec![
                    op_result(c_gv),
                    op_result(c_gp),
                    pav(c_ovec),
                    pav(c_olen),
                    pav(c_oin),
                    pav(c_ounit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // count < 2 -> FIELD_MISSING (`object.rs:180`: 2..=4 required).
    let q_cnt = a.param(ns.p, c_chk_max, ParameterRole::Block, u64_type());
    let q_pos1 = a.param(ns.p, c_chk_max, ParameterRole::Block, u64_type());
    let q_v1 = a.param(ns.p, c_chk_max, ParameterRole::Block, u8vec_type());
    let q_l1 = a.param(ns.p, c_chk_max, ParameterRole::Block, u64_type());
    let q_i1 = a.param(ns.p, c_chk_max, ParameterRole::Block, TypeExpr::Bytes);
    let q_u1 = a.param(ns.p, c_chk_max, ParameterRole::Block, TypeExpr::Unit);
    let t2 = a.cref(ns.o, c_chk_max, c2, u64_type());
    let c_lt = a.op(
        ns.o,
        c_chk_max,
        Opcode::LessThan,
        vec![pav(q_cnt), op_result(t2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let c_chk_hi = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: c_chk_max,
        function: fid,
        parameters: vec![q_cnt, q_pos1, q_v1, q_l1, q_i1, q_u1],
        operations: vec![t2, c_lt],
        terminator: cond(
            op_result(c_lt),
            edge(b_missing, Vec::new()),
            edge(
                c_chk_hi,
                vec![
                    pav(q_cnt),
                    pav(q_pos1),
                    pav(q_v1),
                    pav(q_l1),
                    pav(q_i1),
                    pav(q_u1),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // count > 4 -> FIELD_UNKNOWN (`object.rs:184`).
    let h_cnt = a.param(ns.p, c_chk_hi, ParameterRole::Block, u64_type());
    let h_pos1 = a.param(ns.p, c_chk_hi, ParameterRole::Block, u64_type());
    let h_v1 = a.param(ns.p, c_chk_hi, ParameterRole::Block, u8vec_type());
    let h_l1 = a.param(ns.p, c_chk_hi, ParameterRole::Block, u64_type());
    let h_i1 = a.param(ns.p, c_chk_hi, ParameterRole::Block, TypeExpr::Bytes);
    let h_u1 = a.param(ns.p, c_chk_hi, ParameterRole::Block, TypeExpr::Unit);
    let t4 = a.cref(ns.o, c_chk_hi, c4, u64_type());
    let c_gt4 = a.op(
        ns.o,
        c_chk_hi,
        Opcode::GreaterThan,
        vec![pav(h_cnt), op_result(t4)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let c_chk_scope = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: c_chk_hi,
        function: fid,
        parameters: vec![h_cnt, h_pos1, h_v1, h_l1, h_i1, h_u1],
        operations: vec![t4, c_gt4],
        terminator: cond(
            op_result(c_gt4),
            edge(b_unknown, Vec::new()),
            edge(
                c_chk_scope,
                vec![
                    pav(h_cnt),
                    pav(h_pos1),
                    pav(h_v1),
                    pav(h_l1),
                    pav(h_i1),
                    pav(h_u1),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // count == 3 or 4 -> scope exclusion (label/fingerprint need NFC +
    // S20-250 verifier; explicit `SSMC_RESERVED_FIELD_PRESENT`, pinned
    // divergence vs reference Ok for valid 3/4-field objects). Scope takes
    // precedence over field errors for 3/4 (documented scope-first).
    let s_cnt = a.param(ns.p, c_chk_scope, ParameterRole::Block, u64_type());
    let s_pos1 = a.param(ns.p, c_chk_scope, ParameterRole::Block, u64_type());
    let s_v1 = a.param(ns.p, c_chk_scope, ParameterRole::Block, u8vec_type());
    let s_l1 = a.param(ns.p, c_chk_scope, ParameterRole::Block, u64_type());
    let s_i1 = a.param(ns.p, c_chk_scope, ParameterRole::Block, TypeExpr::Bytes);
    let s_u1 = a.param(ns.p, c_chk_scope, ParameterRole::Block, TypeExpr::Unit);
    let t3 = a.cref(ns.o, c_chk_scope, c3, u64_type());
    let t4b = a.cref(ns.o, c_chk_scope, c4, u64_type());
    let e3 = a.op(
        ns.o,
        c_chk_scope,
        Opcode::Equal,
        vec![pav(s_cnt), op_result(t3)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let e4 = a.op(
        ns.o,
        c_chk_scope,
        Opcode::Equal,
        vec![pav(s_cnt), op_result(t4b)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let e34 = a.op(
        ns.o,
        c_chk_scope,
        Opcode::BoolOr,
        vec![op_result(e3), op_result(e4)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let f1_tag = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: c_chk_scope,
        function: fid,
        parameters: vec![s_cnt, s_pos1, s_v1, s_l1, s_i1, s_u1],
        operations: vec![t3, t4b, e3, e4, e34],
        terminator: cond(
            op_result(e34),
            edge(b_scope, Vec::new()),
            edge(
                f1_tag,
                vec![pav(s_pos1), pav(s_v1), pav(s_l1), pav(s_i1), pav(s_u1)],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Field 1 tag: decode_uvar(payload, pos1, 32). Uvar errors propagate
    // first, mirroring `object.rs:194` (tag read before duplicate/order;
    // first field has no previous, so no duplicate/order check here).
    let t1_pos = a.param(ns.p, f1_tag, ParameterRole::Block, u64_type());
    let t1_vec = a.param(ns.p, f1_tag, ParameterRole::Block, u8vec_type());
    let t1_len = a.param(ns.p, f1_tag, ParameterRole::Block, u64_type());
    let t1_in = a.param(ns.p, f1_tag, ParameterRole::Block, TypeExpr::Bytes);
    let t1_unit = a.param(ns.p, f1_tag, ParameterRole::Block, TypeExpr::Unit);
    let t1_w = a.cref(ns.o, f1_tag, w32, u32_type());
    let t1_call = a.op(
        ns.o,
        f1_tag,
        Opcode::CallDirect,
        vec![pav(t1_in), pav(t1_pos), op_result(t1_w), pav(t1_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let f1_ok = a.id(ns.b);
    let f1_err = a.id(ns.b);
    let f1_tup = a.param(
        ns.p,
        f1_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let f1_ovec = a.param(ns.p, f1_ok, ParameterRole::Block, u8vec_type());
    let f1_olen = a.param(ns.p, f1_ok, ParameterRole::Block, u64_type());
    let f1_oin = a.param(ns.p, f1_ok, ParameterRole::Block, TypeExpr::Bytes);
    let f1_ounit = a.param(ns.p, f1_ok, ParameterRole::Block, TypeExpr::Unit);
    let f1_ebytes = a.param(ns.p, f1_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f1_tag,
        function: fid,
        parameters: vec![t1_pos, t1_vec, t1_len, t1_in, t1_unit],
        operations: vec![t1_w, t1_call],
        terminator: switch(
            op_result(t1_call),
            vec![
                (
                    BuiltinCase::Ok,
                    f1_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(t1_vec),
                        sav(t1_len),
                        sav(t1_in),
                        sav(t1_unit),
                    ],
                ),
                (BuiltinCase::Err, f1_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let f1_er = a.op(
        ns.o,
        f1_err,
        Opcode::ResultErr,
        vec![pav(f1_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f1_err,
        function: fid,
        parameters: vec![f1_ebytes],
        operations: vec![f1_er],
        terminator: ret(op_result(f1_er)),
        reachability: Reachability::Required,
    });
    // Unpack tag1 + pos2, then read len1 via decode_uvar width 64.
    let f1_gtag = a.op(
        ns.o,
        f1_ok,
        Opcode::TupleGet,
        vec![pav(f1_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let f1_gpos = a.op(
        ns.o,
        f1_ok,
        Opcode::TupleGet,
        vec![pav(f1_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let f1_len_block = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: f1_ok,
        function: fid,
        parameters: vec![f1_tup, f1_ovec, f1_olen, f1_oin, f1_ounit],
        operations: vec![f1_gtag, f1_gpos],
        terminator: branch(edge(
            f1_len_block,
            vec![
                op_result(f1_gtag),
                op_result(f1_gpos),
                pav(f1_ovec),
                pav(f1_olen),
                pav(f1_oin),
                pav(f1_ounit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let l1_tag = a.param(ns.p, f1_len_block, ParameterRole::Block, u64_type());
    let l1_pos2 = a.param(ns.p, f1_len_block, ParameterRole::Block, u64_type());
    let l1_vec = a.param(ns.p, f1_len_block, ParameterRole::Block, u8vec_type());
    let l1_len = a.param(ns.p, f1_len_block, ParameterRole::Block, u64_type());
    let l1_in = a.param(ns.p, f1_len_block, ParameterRole::Block, TypeExpr::Bytes);
    let l1_unit = a.param(ns.p, f1_len_block, ParameterRole::Block, TypeExpr::Unit);
    let l1_w = a.cref(ns.o, f1_len_block, w64, u32_type());
    let l1_call = a.op(
        ns.o,
        f1_len_block,
        Opcode::CallDirect,
        vec![pav(l1_in), pav(l1_pos2), op_result(l1_w), pav(l1_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let l1_ok = a.id(ns.b);
    let l1_err = a.id(ns.b);
    let l1_tup = a.param(
        ns.p,
        l1_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let l1_ovec = a.param(ns.p, l1_ok, ParameterRole::Block, u8vec_type());
    let l1_olen = a.param(ns.p, l1_ok, ParameterRole::Block, u64_type());
    let l1_oin = a.param(ns.p, l1_ok, ParameterRole::Block, TypeExpr::Bytes);
    let l1_ounit = a.param(ns.p, l1_ok, ParameterRole::Block, TypeExpr::Unit);
    let l1_otag = a.param(ns.p, l1_ok, ParameterRole::Block, u64_type());
    let l1_ebytes = a.param(ns.p, l1_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f1_len_block,
        function: fid,
        parameters: vec![l1_tag, l1_pos2, l1_vec, l1_len, l1_in, l1_unit],
        operations: vec![l1_w, l1_call],
        terminator: switch(
            op_result(l1_call),
            vec![
                (
                    BuiltinCase::Ok,
                    l1_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(l1_vec),
                        sav(l1_len),
                        sav(l1_in),
                        sav(l1_unit),
                        sav(l1_tag),
                    ],
                ),
                (BuiltinCase::Err, l1_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let l1_er = a.op(
        ns.o,
        l1_err,
        Opcode::ResultErr,
        vec![pav(l1_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: l1_err,
        function: fid,
        parameters: vec![l1_ebytes],
        operations: vec![l1_er],
        terminator: ret(op_result(l1_er)),
        reachability: Reachability::Required,
    });
    // Unpack len1 + pos3. Check len1 > MAX -> RESOURCE_LIMIT
    // (`read_len` rule, `object.rs` via `read_sized_payload`).
    let l1_glen = a.op(
        ns.o,
        l1_ok,
        Opcode::TupleGet,
        vec![pav(l1_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let l1_gpos = a.op(
        ns.o,
        l1_ok,
        Opcode::TupleGet,
        vec![pav(l1_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let l1_maxc = a.cref(ns.o, l1_ok, c_max, u64_type());
    let l1_gtmax = a.op(
        ns.o,
        l1_ok,
        Opcode::GreaterThan,
        vec![op_result(l1_glen), op_result(l1_maxc)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let l1_chk_bounds = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: l1_ok,
        function: fid,
        parameters: vec![l1_tup, l1_ovec, l1_olen, l1_oin, l1_ounit, l1_otag],
        operations: vec![l1_glen, l1_gpos, l1_maxc, l1_gtmax],
        terminator: cond(
            op_result(l1_gtmax),
            edge(b_res, Vec::new()),
            edge(
                l1_chk_bounds,
                vec![
                    pav(l1_otag),
                    op_result(l1_glen),
                    op_result(l1_gpos),
                    pav(l1_ovec),
                    pav(l1_olen),
                    pav(l1_oin),
                    pav(l1_ounit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Bounds: pos3 + len1 <= input len, else LENGTH_OVERFLOW.
    // Indices stay below the converted length + MAX, so checked-add
    // overflow is unreachable (trap); reachable overrun is typed.
    let b1_tag = a.param(ns.p, l1_chk_bounds, ParameterRole::Block, u64_type());
    let b1_len1 = a.param(ns.p, l1_chk_bounds, ParameterRole::Block, u64_type());
    let b1_pos3 = a.param(ns.p, l1_chk_bounds, ParameterRole::Block, u64_type());
    let b1_vec = a.param(ns.p, l1_chk_bounds, ParameterRole::Block, u8vec_type());
    let b1_ilen = a.param(ns.p, l1_chk_bounds, ParameterRole::Block, u64_type());
    let b1_in = a.param(ns.p, l1_chk_bounds, ParameterRole::Block, TypeExpr::Bytes);
    let b1_unit = a.param(ns.p, l1_chk_bounds, ParameterRole::Block, TypeExpr::Unit);
    let b1_add = a.op(
        ns.o,
        l1_chk_bounds,
        Opcode::IntAddChecked,
        vec![pav(b1_pos3), pav(b1_len1)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let b1_unwrap = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: l1_chk_bounds,
        function: fid,
        parameters: vec![b1_tag, b1_len1, b1_pos3, b1_vec, b1_ilen, b1_in, b1_unit],
        operations: vec![b1_add],
        terminator: switch(
            op_result(b1_add),
            vec![
                (
                    BuiltinCase::Ok,
                    b1_unwrap,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(b1_tag),
                        sav(b1_len1),
                        sav(b1_vec),
                        sav(b1_ilen),
                        sav(b1_in),
                        sav(b1_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let u1_sum = a.param(ns.p, b1_unwrap, ParameterRole::Block, u64_type());
    let u1_tag = a.param(ns.p, b1_unwrap, ParameterRole::Block, u64_type());
    let u1_len1 = a.param(ns.p, b1_unwrap, ParameterRole::Block, u64_type());
    let u1_vec = a.param(ns.p, b1_unwrap, ParameterRole::Block, u8vec_type());
    let u1_ilen = a.param(ns.p, b1_unwrap, ParameterRole::Block, u64_type());
    let u1_in = a.param(ns.p, b1_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let u1_unit = a.param(ns.p, b1_unwrap, ParameterRole::Block, TypeExpr::Unit);
    let u1_gt = a.op(
        ns.o,
        b1_unwrap,
        Opcode::GreaterThan,
        vec![pav(u1_sum), pav(u1_ilen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let f1_dispatch = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: b1_unwrap,
        function: fid,
        parameters: vec![u1_sum, u1_tag, u1_len1, u1_vec, u1_ilen, u1_in, u1_unit],
        operations: vec![u1_gt],
        terminator: cond(
            op_result(u1_gt),
            edge(b_len, Vec::new()),
            edge(
                f1_dispatch,
                vec![
                    pav(u1_tag),
                    pav(u1_len1),
                    pav(u1_sum),
                    pav(u1_vec),
                    pav(u1_ilen),
                    pav(u1_in),
                    pav(u1_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Field-1 dispatch on tag value (after len/bounds, mirroring
    // `object.rs:204-213` match-after-read order). Tags 3/4 are scope
    // exclusions (label/fingerprint need NFC + verifier); unknown tags
    // (>4, or 0) are FIELD_UNKNOWN. Tag 1 proceeds to EntityId length
    // checks; tag 2 (body-first) proceeds to field 2 for precise
    // duplicate/order/scope/unknown (all [2,*] with count==2 are Err,
    // but the exact code needs field 2).
    let d1_tag = a.param(ns.p, f1_dispatch, ParameterRole::Block, u64_type());
    let d1_len1 = a.param(ns.p, f1_dispatch, ParameterRole::Block, u64_type());
    let d1_end1 = a.param(ns.p, f1_dispatch, ParameterRole::Block, u64_type());
    let d1_vec = a.param(ns.p, f1_dispatch, ParameterRole::Block, u8vec_type());
    let d1_ilen = a.param(ns.p, f1_dispatch, ParameterRole::Block, u64_type());
    let d1_in = a.param(ns.p, f1_dispatch, ParameterRole::Block, TypeExpr::Bytes);
    let d1_unit = a.param(ns.p, f1_dispatch, ParameterRole::Block, TypeExpr::Unit);
    let k1 = a.cref(ns.o, f1_dispatch, c1, u64_type());
    let k2 = a.cref(ns.o, f1_dispatch, c2, u64_type());
    let k3 = a.cref(ns.o, f1_dispatch, c3, u64_type());
    let k4 = a.cref(ns.o, f1_dispatch, c4, u64_type());
    let e1 = a.op(
        ns.o,
        f1_dispatch,
        Opcode::Equal,
        vec![pav(d1_tag), op_result(k1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let e2 = a.op(
        ns.o,
        f1_dispatch,
        Opcode::Equal,
        vec![pav(d1_tag), op_result(k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let e3 = a.op(
        ns.o,
        f1_dispatch,
        Opcode::Equal,
        vec![pav(d1_tag), op_result(k3)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let e4 = a.op(
        ns.o,
        f1_dispatch,
        Opcode::Equal,
        vec![pav(d1_tag), op_result(k4)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let e34 = a.op(
        ns.o,
        f1_dispatch,
        Opcode::BoolOr,
        vec![op_result(e3), op_result(e4)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let f1_tag1 = a.id(ns.b);
    let f1_tag2 = a.id(ns.b);
    let f1_chk_tag = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: f1_dispatch,
        function: fid,
        parameters: vec![d1_tag, d1_len1, d1_end1, d1_vec, d1_ilen, d1_in, d1_unit],
        operations: vec![k1, k2, k3, k4, e1, e2, e3, e4, e34],
        terminator: cond(
            op_result(e34),
            edge(b_scope, Vec::new()),
            edge(
                f1_chk_tag,
                vec![
                    pav(d1_tag),
                    pav(d1_len1),
                    pav(d1_end1),
                    pav(d1_vec),
                    pav(d1_ilen),
                    pav(d1_in),
                    pav(d1_unit),
                    op_result(e1),
                    op_result(e2),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let ct_tag = a.param(ns.p, f1_chk_tag, ParameterRole::Block, u64_type());
    let ct_len1 = a.param(ns.p, f1_chk_tag, ParameterRole::Block, u64_type());
    let ct_end1 = a.param(ns.p, f1_chk_tag, ParameterRole::Block, u64_type());
    let ct_vec = a.param(ns.p, f1_chk_tag, ParameterRole::Block, u8vec_type());
    let ct_ilen = a.param(ns.p, f1_chk_tag, ParameterRole::Block, u64_type());
    let ct_in = a.param(ns.p, f1_chk_tag, ParameterRole::Block, TypeExpr::Bytes);
    let ct_unit = a.param(ns.p, f1_chk_tag, ParameterRole::Block, TypeExpr::Unit);
    let ct_e1 = a.param(ns.p, f1_chk_tag, ParameterRole::Block, TypeExpr::Bool);
    let ct_e2 = a.param(ns.p, f1_chk_tag, ParameterRole::Block, TypeExpr::Bool);
    a.blocks.push(Block {
        entity_id: f1_chk_tag,
        function: fid,
        parameters: vec![
            ct_tag, ct_len1, ct_end1, ct_vec, ct_ilen, ct_in, ct_unit, ct_e1, ct_e2,
        ],
        operations: Vec::new(),
        terminator: cond(
            pav(ct_e1),
            edge(
                f1_tag1,
                vec![
                    pav(ct_len1),
                    pav(ct_end1),
                    pav(ct_vec),
                    pav(ct_ilen),
                    pav(ct_in),
                    pav(ct_unit),
                ],
            ),
            edge(
                f1_tag2,
                vec![
                    pav(ct_tag),
                    pav(ct_len1),
                    pav(ct_end1),
                    pav(ct_vec),
                    pav(ct_ilen),
                    pav(ct_in),
                    pav(ct_unit),
                    pav(ct_e2),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Tag 2 first (body-first): all [2,*] with count==2 are Err, but the
    // exact code needs field 2 (DUPLICATE for [2,2], ORDER for [2,1]/[2,0],
    // scope for [2,3]/[2,4], UNKNOWN for [2,5+]). Lands with field-2 chain
    // next edit; temporary park as UNKNOWN with explicit deferral (valid
    // inputs never take this path: all fixtures are [1,2]).
    let t2_tag = a.param(ns.p, f1_tag2, ParameterRole::Block, u64_type());
    let t2_len1 = a.param(ns.p, f1_tag2, ParameterRole::Block, u64_type());
    let t2_end1 = a.param(ns.p, f1_tag2, ParameterRole::Block, u64_type());
    let t2_vec = a.param(ns.p, f1_tag2, ParameterRole::Block, u8vec_type());
    let t2_ilen = a.param(ns.p, f1_tag2, ParameterRole::Block, u64_type());
    let t2_in = a.param(ns.p, f1_tag2, ParameterRole::Block, TypeExpr::Bytes);
    let t2_unit = a.param(ns.p, f1_tag2, ParameterRole::Block, TypeExpr::Unit);
    let t2_e2 = a.param(ns.p, f1_tag2, ParameterRole::Block, TypeExpr::Bool);
    a.blocks.push(Block {
        entity_id: f1_tag2,
        function: fid,
        parameters: vec![
            t2_tag, t2_len1, t2_end1, t2_vec, t2_ilen, t2_in, t2_unit, t2_e2,
        ],
        operations: Vec::new(),
        terminator: cond(
            pav(t2_e2),
            edge(b_unknown, Vec::new()),
            edge(b_unknown, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // Tag 1 (entity_id): len1 must be exactly 32 (`decode_fixed`:
    // <32 -> LENGTH_OVERFLOW, >32 -> TRAILING). On ==32, extract 32B via
    // loop then proceed to field 2 (lands next edit). Temporary: enforce
    // length, then park valid path as UNKNOWN (no success yet).
    let o1_len1 = a.param(ns.p, f1_tag1, ParameterRole::Block, u64_type());
    let o1_end1 = a.param(ns.p, f1_tag1, ParameterRole::Block, u64_type());
    let o1_vec = a.param(ns.p, f1_tag1, ParameterRole::Block, u8vec_type());
    let o1_ilen = a.param(ns.p, f1_tag1, ParameterRole::Block, u64_type());
    let o1_in = a.param(ns.p, f1_tag1, ParameterRole::Block, TypeExpr::Bytes);
    let o1_unit = a.param(ns.p, f1_tag1, ParameterRole::Block, TypeExpr::Unit);
    let k32 = a.cref(ns.o, f1_tag1, c32, u64_type());
    let lt32 = a.op(
        ns.o,
        f1_tag1,
        Opcode::LessThan,
        vec![pav(o1_len1), op_result(k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let gt32 = a.op(
        ns.o,
        f1_tag1,
        Opcode::GreaterThan,
        vec![pav(o1_len1), op_result(k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let o1_chk_gt = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: f1_tag1,
        function: fid,
        parameters: vec![o1_len1, o1_end1, o1_vec, o1_ilen, o1_in, o1_unit],
        operations: vec![k32, lt32, gt32],
        terminator: cond(
            op_result(lt32),
            edge(b_len, Vec::new()),
            edge(
                o1_chk_gt,
                vec![
                    pav(o1_len1),
                    pav(o1_end1),
                    pav(o1_vec),
                    pav(o1_ilen),
                    pav(o1_in),
                    pav(o1_unit),
                    op_result(gt32),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let g_len1 = a.param(ns.p, o1_chk_gt, ParameterRole::Block, u64_type());
    let g_end1 = a.param(ns.p, o1_chk_gt, ParameterRole::Block, u64_type());
    let g_vec = a.param(ns.p, o1_chk_gt, ParameterRole::Block, u8vec_type());
    let g_ilen = a.param(ns.p, o1_chk_gt, ParameterRole::Block, u64_type());
    let g_in = a.param(ns.p, o1_chk_gt, ParameterRole::Block, TypeExpr::Bytes);
    let g_unit = a.param(ns.p, o1_chk_gt, ParameterRole::Block, TypeExpr::Unit);
    let g_gt = a.param(ns.p, o1_chk_gt, ParameterRole::Block, TypeExpr::Bool);
    // len1==32 path goes to entity_id extraction (32B fixed) then field 2.
    // Full loop + field-2 + body + trailing + return lands next edit;
    // temporary park as UNKNOWN (no success yet, no trap).
    let eid_loop = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: o1_chk_gt,
        function: fid,
        parameters: vec![g_len1, g_end1, g_vec, g_ilen, g_in, g_unit, g_gt],
        operations: Vec::new(),
        terminator: cond(
            pav(g_gt),
            edge(b_trail, Vec::new()),
            edge(
                eid_loop,
                vec![pav(g_end1), pav(g_vec), pav(g_ilen), pav(g_in), pav(g_unit)],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Entity-ID extraction: 32B at (end1-32)..end1 via checked-sub for
    // start (underflow unreachable: end1>=32 by len1==32 + bounds), then
    // 32-iteration PSH1 loop (Form A backedge, Get-None unreachable by
    // bounds, PSH1-Err typed RESOURCE_LIMIT). Lands field-2 next.
    let el_end = a.param(ns.p, eid_loop, ParameterRole::Block, u64_type());
    let el_vec = a.param(ns.p, eid_loop, ParameterRole::Block, u8vec_type());
    let el_ilen = a.param(ns.p, eid_loop, ParameterRole::Block, u64_type());
    let el_in = a.param(ns.p, eid_loop, ParameterRole::Block, TypeExpr::Bytes);
    let el_unit = a.param(ns.p, eid_loop, ParameterRole::Block, TypeExpr::Unit);
    let el_k32 = a.cref(ns.o, eid_loop, c32, u64_type());
    let el_sub = a.op(
        ns.o,
        eid_loop,
        Opcode::IntSubChecked,
        vec![pav(el_end), op_result(el_k32)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let el_unwrap = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: eid_loop,
        function: fid,
        parameters: vec![el_end, el_vec, el_ilen, el_in, el_unit],
        operations: vec![el_k32, el_sub],
        terminator: switch(
            op_result(el_sub),
            vec![
                (
                    BuiltinCase::Ok,
                    el_unwrap,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(el_vec),
                        sav(el_ilen),
                        sav(el_in),
                        sav(el_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Unwrap start pos3 (IntSubChecked Ok payload is the u64 difference).
    let s_pos = a.param(ns.p, el_unwrap, ParameterRole::Block, u64_type());
    // NOTE: full 32B loop + field-2 + body + trailing + return lands next;
    // temporary park as UNKNOWN (no success yet, no trap).
    let s_vec = a.param(ns.p, el_unwrap, ParameterRole::Block, u8vec_type());
    let s_ilen = a.param(ns.p, el_unwrap, ParameterRole::Block, u64_type());
    let s_in = a.param(ns.p, el_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let s_unit = a.param(ns.p, el_unwrap, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: el_unwrap,
        function: fid,
        parameters: vec![s_pos, s_vec, s_ilen, s_in, s_unit],
        operations: Vec::new(),
        terminator: branch(edge(b_unknown, Vec::new())),
        reachability: Reachability::Required,
    });
    let _ = (c1, c32, c_max, w32, trap);
    let _ = b_dup;
    let _ = b_order;

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![p_in, p_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

// ── fixture record engine (shared framing for synthetic + program) ───
// Loop-free strict record framing reused by program outer: count/tags/
// lens/bool/trailing with exact `SCB_*` codes. Two instantiations:
// `decode_empty`/`encode_empty` (FixtureEmptyObject, payload `00`) and
// `decode_reqbool`/`encode_reqbool` (FixtureRequiredBool, payload
// `01 01 01 <00|01>`). Reuses `decode_uvar` via `CallDirect` for count/
// tags/lens; bool via single `VectorGet` (no loops); emission via
// sequential `PSH1` pushes (fixed 1B/4B, no backedges) + `V2B1`.
// Sley owns parsing, field decisions, and byte emission; bridge uses
// B2V1/PSH1/V2B1 only. Program outer (tag 200, fields 1..4) shares this
// exact framing; body kinds/label/NFC remain explicit scope (see outer
// builder above, in-progress, not claimed complete).

fn fixture_empty_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Unit),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(dead_code)]
fn fixture_bool_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Bool),
        error: Box::new(TypeExpr::Bytes),
    }
}

#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_fixture_empty_decode(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    decode_fid: EntityId,
) -> FunctionGraph {
    let bstart = a.blocks.len();
    let res_t = fixture_empty_result_type();
    let dec_t = decode_result_type();
    let c0 = a.ku64(ns.k, 0);
    let c_max_fields = a.ku64(ns.k, 65_535);
    let w64 = a.ku32(ns.k, 64);
    let e_unknown = a.kbytes(ns.k, b"SCB_FIELD_UNKNOWN");
    let e_trail = a.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_res = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");

    let p_in = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let p_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let b_unknown = err_block(a, ns, fid, res_t.clone(), e_unknown);
    let b_trail = err_block(a, ns, fid, res_t.clone(), e_trail);
    let b_res = err_block(a, ns, fid, res_t.clone(), e_res);

    // Entry: B2V1 convert, bridge Err -> RESOURCE_LIMIT (1 MiB cap;
    // epoch allows 64 MiB / 67,108,864 bytes, documented restriction).
    let entry = a.id(ns.b);
    let cv = a.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(p_unit), pav(p_in)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    let b_cnt = a.id(ns.b);
    let q_vec = a.param(ns.p, b_cnt, ParameterRole::Block, u8vec_type());
    let q_in = a.param(ns.p, b_cnt, ParameterRole::Block, TypeExpr::Bytes);
    let q_unit = a.param(ns.p, b_cnt, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![cv],
        terminator: switch(
            op_result(cv),
            vec![
                (
                    BuiltinCase::Ok,
                    b_cnt,
                    vec![SwitchArgument::CasePayload, sav(p_in), sav(p_unit)],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let ln = a.op(
        ns.o,
        b_cnt,
        Opcode::VectorLen,
        vec![pav(q_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let c0c = a.cref(ns.o, b_cnt, c0, u64_type());
    let wc = a.cref(ns.o, b_cnt, w64, u32_type());
    let cnt_call = a.op(
        ns.o,
        b_cnt,
        Opcode::CallDirect,
        vec![pav(q_in), op_result(c0c), op_result(wc), pav(q_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let c_ok = a.id(ns.b);
    let c_err = a.id(ns.b);
    let c_tup = a.param(
        ns.p,
        c_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let c_ovec = a.param(ns.p, c_ok, ParameterRole::Block, u8vec_type());
    let c_olen = a.param(ns.p, c_ok, ParameterRole::Block, u64_type());
    let c_oin = a.param(ns.p, c_ok, ParameterRole::Block, TypeExpr::Bytes);
    let c_ounit = a.param(ns.p, c_ok, ParameterRole::Block, TypeExpr::Unit);
    let c_ebytes = a.param(ns.p, c_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: b_cnt,
        function: fid,
        parameters: vec![q_vec, q_in, q_unit],
        operations: vec![ln, c0c, wc, cnt_call],
        terminator: switch(
            op_result(cnt_call),
            vec![
                (
                    BuiltinCase::Ok,
                    c_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(q_vec),
                        oav(ln),
                        sav(q_in),
                        sav(q_unit),
                    ],
                ),
                (BuiltinCase::Err, c_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let c_er = a.op(
        ns.o,
        c_err,
        Opcode::ResultErr,
        vec![pav(c_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: c_err,
        function: fid,
        parameters: vec![c_ebytes],
        operations: vec![c_er],
        terminator: ret(op_result(c_er)),
        reachability: Reachability::Required,
    });
    // count > 65535 -> RESOURCE_LIMIT, count != 0 -> FIELD_UNKNOWN
    // (`decode_empty_record`: 0 ok else UNKNOWN; no MISSING/ORDER/DUP).
    let c_gv = a.op(
        ns.o,
        c_ok,
        Opcode::TupleGet,
        vec![pav(c_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let c_gp = a.op(
        ns.o,
        c_ok,
        Opcode::TupleGet,
        vec![pav(c_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let mf = a.cref(ns.o, c_ok, c_max_fields, u64_type());
    let c_gt = a.op(
        ns.o,
        c_ok,
        Opcode::GreaterThan,
        vec![op_result(c_gv), op_result(mf)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let c_chk_zero = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: c_ok,
        function: fid,
        parameters: vec![c_tup, c_ovec, c_olen, c_oin, c_ounit],
        operations: vec![c_gv, c_gp, mf, c_gt],
        terminator: cond(
            op_result(c_gt),
            edge(b_res, Vec::new()),
            edge(
                c_chk_zero,
                vec![op_result(c_gv), op_result(c_gp), pav(c_olen), pav(c_ounit)],
            ),
        ),
        reachability: Reachability::Required,
    });
    let z_cnt = a.param(ns.p, c_chk_zero, ParameterRole::Block, u64_type());
    let z_pos = a.param(ns.p, c_chk_zero, ParameterRole::Block, u64_type());
    let z_len = a.param(ns.p, c_chk_zero, ParameterRole::Block, u64_type());
    let z_unit = a.param(ns.p, c_chk_zero, ParameterRole::Block, TypeExpr::Unit);
    let z0 = a.cref(ns.o, c_chk_zero, c0, u64_type());
    let z_eq = a.op(
        ns.o,
        c_chk_zero,
        Opcode::Equal,
        vec![pav(z_cnt), op_result(z0)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let z_chk_trail = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: c_chk_zero,
        function: fid,
        parameters: vec![z_cnt, z_pos, z_len, z_unit],
        operations: vec![z0, z_eq],
        terminator: cond(
            op_result(z_eq),
            edge(z_chk_trail, vec![pav(z_pos), pav(z_len), pav(z_unit)]),
            edge(b_unknown, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // Trailing: pos == len else TRAILING (pos < len; > impossible by uvar
    // bounds). Return Ok(unit) via input unit (Unit has one value).
    let t_pos = a.param(ns.p, z_chk_trail, ParameterRole::Block, u64_type());
    let t_len = a.param(ns.p, z_chk_trail, ParameterRole::Block, u64_type());
    let t_unit = a.param(ns.p, z_chk_trail, ParameterRole::Block, TypeExpr::Unit);
    let t_eq = a.op(
        ns.o,
        z_chk_trail,
        Opcode::Equal,
        vec![pav(t_pos), pav(t_len)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let t_ok = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: z_chk_trail,
        function: fid,
        parameters: vec![t_pos, t_len, t_unit],
        operations: vec![t_eq],
        terminator: cond(
            op_result(t_eq),
            edge(t_ok, vec![pav(t_unit)]),
            edge(b_trail, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    let o_unit = a.param(ns.p, t_ok, ParameterRole::Block, TypeExpr::Unit);
    let o_ok = a.op(
        ns.o,
        t_ok,
        Opcode::ResultOk,
        vec![pav(o_unit)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: t_ok,
        function: fid,
        parameters: vec![o_unit],
        operations: vec![o_ok],
        terminator: ret(op_result(o_ok)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![p_in, p_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_fixture_empty_encode(a: &mut Asm, ns: Ns, fid: EntityId) -> FunctionGraph {
    use sley_vm::host_abi::{BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let bstart = a.blocks.len();
    let res_t = encode_result_type();
    let e_res = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let c0_8 = a.ku8(ns.k, 0);

    let p_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let b_res = err_block(a, ns, fid, res_t.clone(), e_res);

    let entry = a.id(ns.b);
    let new_vec = a.op(
        ns.o,
        entry,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let b_push = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![new_vec],
        terminator: branch(edge(b_push, vec![op_result(new_vec), pav(p_unit)])),
        reachability: Reachability::Required,
    });
    let q_acc0 = a.param(ns.p, b_push, ParameterRole::Block, u8vec_type());
    let q_unit = a.param(ns.p, b_push, ParameterRole::Block, TypeExpr::Unit);
    let b0 = a.cref(ns.o, b_push, c0_8, u8_type());
    let push = a.op(
        ns.o,
        b_push,
        Opcode::AdapterInvoke,
        vec![pav(q_acc0), op_result(b0)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    let b_conv = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: b_push,
        function: fid,
        parameters: vec![q_acc0, q_unit],
        operations: vec![b0, push],
        terminator: switch(
            op_result(push),
            vec![
                (
                    BuiltinCase::Ok,
                    b_conv,
                    vec![SwitchArgument::CasePayload, sav(q_unit)],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let push_acc = a.param(ns.p, b_conv, ParameterRole::Block, u8vec_type());
    let push_unit = a.param(ns.p, b_conv, ParameterRole::Block, TypeExpr::Unit);
    let conv = a.op(
        ns.o,
        b_conv,
        Opcode::AdapterInvoke,
        vec![pav(push_unit), pav(push_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    let b_ok = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: b_conv,
        function: fid,
        parameters: vec![push_acc, push_unit],
        operations: vec![conv],
        terminator: switch(
            op_result(conv),
            vec![
                (BuiltinCase::Ok, b_ok, vec![SwitchArgument::CasePayload]),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let conv_bytes = a.param(ns.p, b_ok, ParameterRole::Block, TypeExpr::Bytes);
    let ok = a.op(
        ns.o,
        b_ok,
        Opcode::ResultOk,
        vec![pav(conv_bytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: b_ok,
        function: fid,
        parameters: vec![conv_bytes],
        operations: vec![ok],
        terminator: ret(op_result(ok)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![p_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

// ── fixture RequiredBool (record with one Bool field) ────────────────
// Strict `decode_required_bool_record` (`sley-scb1/src/lib.rs:969`) +
// canonical emission for `01 01 01 <00|01>`. Loop-free: count/tags/lens
// via `CallDirect decode_uvar`; bool via single `VectorGet` (no loops);
// emission via 4 sequential `PSH1` pushes (fixed 4B, no backedges).
// Exact codes: FIELD_MISSING (count 0 / no tag-1), FIELD_UNKNOWN
// (tag != 1), FIELD_DUPLICATE ([1,1]), FIELD_ORDER ([2,1]/[1,0] etc.,
// tag2 < tag1), BOOL_INVALID (byte 2..255), LENGTH_OVERFLOW (bounds /
// bool-byte short), TRAILING (pos != len), RESOURCE_LIMIT (count>65535 /
// len>MAX / bridge caps). Ordered tags, no inference.

#[allow(
    dead_code,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_fixture_reqbool_decode(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    decode_fid: EntityId,
) -> FunctionGraph {
    let bstart = a.blocks.len();
    let res_t = fixture_bool_result_type();
    let dec_t = decode_result_type();
    let c0 = a.ku64(ns.k, 0);
    let c1 = a.ku64(ns.k, 1);
    let c_max_fields = a.ku64(ns.k, 65_535);
    let c_max = a.ku64(ns.k, 67_108_864);
    let w32 = a.ku32(ns.k, 32);
    let w64 = a.ku32(ns.k, 64);
    let e_missing = a.kbytes(ns.k, b"SCB_FIELD_MISSING");
    let e_unknown = a.kbytes(ns.k, b"SCB_FIELD_UNKNOWN");
    let e_dup = a.kbytes(ns.k, b"SCB_FIELD_DUPLICATE");
    let e_order = a.kbytes(ns.k, b"SCB_FIELD_ORDER");
    let e_bool = a.kbytes(ns.k, b"SCB_BOOL_INVALID");
    let e_len = a.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let e_trail = a.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_res = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");

    let p_in = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let p_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let b_missing = err_block(a, ns, fid, res_t.clone(), e_missing);
    let b_unknown = err_block(a, ns, fid, res_t.clone(), e_unknown);
    let b_dup = err_block(a, ns, fid, res_t.clone(), e_dup);
    let b_order = err_block(a, ns, fid, res_t.clone(), e_order);
    let b_bool = err_block(a, ns, fid, res_t.clone(), e_bool);
    let b_len = err_block(a, ns, fid, res_t.clone(), e_len);
    let b_trail = err_block(a, ns, fid, res_t.clone(), e_trail);
    let b_res = err_block(a, ns, fid, res_t.clone(), e_res);
    let trap = trap_block(a, ns, fid);
    let _ = trap;

    // Entry B2V1 + VectorLen + count via CallDirect (width 64).
    let entry = a.id(ns.b);
    let cv = a.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(p_unit), pav(p_in)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    let b_cnt = a.id(ns.b);
    let q_vec = a.param(ns.p, b_cnt, ParameterRole::Block, u8vec_type());
    let q_in = a.param(ns.p, b_cnt, ParameterRole::Block, TypeExpr::Bytes);
    let q_unit = a.param(ns.p, b_cnt, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![cv],
        terminator: switch(
            op_result(cv),
            vec![
                (
                    BuiltinCase::Ok,
                    b_cnt,
                    vec![SwitchArgument::CasePayload, sav(p_in), sav(p_unit)],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let ln = a.op(
        ns.o,
        b_cnt,
        Opcode::VectorLen,
        vec![pav(q_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let c0c = a.cref(ns.o, b_cnt, c0, u64_type());
    let wc = a.cref(ns.o, b_cnt, w64, u32_type());
    let cnt_call = a.op(
        ns.o,
        b_cnt,
        Opcode::CallDirect,
        vec![pav(q_in), op_result(c0c), op_result(wc), pav(q_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let c_ok = a.id(ns.b);
    let c_err = a.id(ns.b);
    let c_tup = a.param(
        ns.p,
        c_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let c_ovec = a.param(ns.p, c_ok, ParameterRole::Block, u8vec_type());
    let c_olen = a.param(ns.p, c_ok, ParameterRole::Block, u64_type());
    let c_oin = a.param(ns.p, c_ok, ParameterRole::Block, TypeExpr::Bytes);
    let c_ounit = a.param(ns.p, c_ok, ParameterRole::Block, TypeExpr::Unit);
    let c_ebytes = a.param(ns.p, c_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: b_cnt,
        function: fid,
        parameters: vec![q_vec, q_in, q_unit],
        operations: vec![ln, c0c, wc, cnt_call],
        terminator: switch(
            op_result(cnt_call),
            vec![
                (
                    BuiltinCase::Ok,
                    c_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(q_vec),
                        oav(ln),
                        sav(q_in),
                        sav(q_unit),
                    ],
                ),
                (BuiltinCase::Err, c_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let c_er = a.op(
        ns.o,
        c_err,
        Opcode::ResultErr,
        vec![pav(c_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: c_err,
        function: fid,
        parameters: vec![c_ebytes],
        operations: vec![c_er],
        terminator: ret(op_result(c_er)),
        reachability: Reachability::Required,
    });
    // count > 65535 -> RESOURCE_LIMIT; count == 0 -> MISSING;
    // count >= 1 proceeds to field 1 (count==1 valid scope; count>1 needs
    // field-2 duplicate/order/unknown, lands below for count==2; count>2
    // parks as UNKNOWN with explicit deferral for this landing slice --
    // valid fixtures have count==1, malformed count 0/2 covered, 3+ deferred
    // explicitly, never silently).
    let c_gv = a.op(
        ns.o,
        c_ok,
        Opcode::TupleGet,
        vec![pav(c_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let c_gp = a.op(
        ns.o,
        c_ok,
        Opcode::TupleGet,
        vec![pav(c_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let mf = a.cref(ns.o, c_ok, c_max_fields, u64_type());
    let c_gt = a.op(
        ns.o,
        c_ok,
        Opcode::GreaterThan,
        vec![op_result(c_gv), op_result(mf)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let c_chk_zero = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: c_ok,
        function: fid,
        parameters: vec![c_tup, c_ovec, c_olen, c_oin, c_ounit],
        operations: vec![c_gv, c_gp, mf, c_gt],
        terminator: cond(
            op_result(c_gt),
            edge(b_res, Vec::new()),
            edge(
                c_chk_zero,
                vec![
                    op_result(c_gv),
                    op_result(c_gp),
                    pav(c_ovec),
                    pav(c_olen),
                    pav(c_oin),
                    pav(c_ounit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let z_cnt = a.param(ns.p, c_chk_zero, ParameterRole::Block, u64_type());
    let z_pos = a.param(ns.p, c_chk_zero, ParameterRole::Block, u64_type());
    let z_vec = a.param(ns.p, c_chk_zero, ParameterRole::Block, u8vec_type());
    let z_len = a.param(ns.p, c_chk_zero, ParameterRole::Block, u64_type());
    let z_in = a.param(ns.p, c_chk_zero, ParameterRole::Block, TypeExpr::Bytes);
    let z_unit = a.param(ns.p, c_chk_zero, ParameterRole::Block, TypeExpr::Unit);
    let z0 = a.cref(ns.o, c_chk_zero, c0, u64_type());
    let z_eq0 = a.op(
        ns.o,
        c_chk_zero,
        Opcode::Equal,
        vec![pav(z_cnt), op_result(z0)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let f1_tag = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: c_chk_zero,
        function: fid,
        parameters: vec![z_cnt, z_pos, z_vec, z_len, z_in, z_unit],
        operations: vec![z0, z_eq0],
        terminator: cond(
            op_result(z_eq0),
            edge(b_missing, Vec::new()),
            edge(
                f1_tag,
                vec![
                    pav(z_pos),
                    pav(z_vec),
                    pav(z_len),
                    pav(z_in),
                    pav(z_unit),
                    pav(z_cnt),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Field-1 tag (width 32) + len (width 64) + bounds, then tag==1 check,
    // len==1 check, bool byte via VectorGet, trailing. Count>1 paths:
    // count==2 with tags [1,1]->DUP, [1,0]/[2,1]->ORDER, [1,2+]->UNKNOWN,
    // count>2 parked UNKNOWN (explicit deferral; valid count==1).
    // For landing slice, implement count==1 fully; count==2 partially
    // (tag2 duplicate/order/unknown via second tag decode, no value loops);
    // count>2 UNKNOWN deferred explicitly.
    let t1_pos = a.param(ns.p, f1_tag, ParameterRole::Block, u64_type());
    let t1_vec = a.param(ns.p, f1_tag, ParameterRole::Block, u8vec_type());
    let t1_len = a.param(ns.p, f1_tag, ParameterRole::Block, u64_type());
    let t1_in = a.param(ns.p, f1_tag, ParameterRole::Block, TypeExpr::Bytes);
    let t1_unit = a.param(ns.p, f1_tag, ParameterRole::Block, TypeExpr::Unit);
    let t1_cnt = a.param(ns.p, f1_tag, ParameterRole::Block, u64_type());
    let t1_w = a.cref(ns.o, f1_tag, w32, u32_type());
    let t1_call = a.op(
        ns.o,
        f1_tag,
        Opcode::CallDirect,
        vec![pav(t1_in), pav(t1_pos), op_result(t1_w), pav(t1_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let f1_ok = a.id(ns.b);
    let f1_err = a.id(ns.b);
    let f1_tup = a.param(
        ns.p,
        f1_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let f1_ovec = a.param(ns.p, f1_ok, ParameterRole::Block, u8vec_type());
    let f1_olen = a.param(ns.p, f1_ok, ParameterRole::Block, u64_type());
    let f1_oin = a.param(ns.p, f1_ok, ParameterRole::Block, TypeExpr::Bytes);
    let f1_ounit = a.param(ns.p, f1_ok, ParameterRole::Block, TypeExpr::Unit);
    let f1_ocnt = a.param(ns.p, f1_ok, ParameterRole::Block, u64_type());
    let f1_ebytes = a.param(ns.p, f1_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f1_tag,
        function: fid,
        parameters: vec![t1_pos, t1_vec, t1_len, t1_in, t1_unit, t1_cnt],
        operations: vec![t1_w, t1_call],
        terminator: switch(
            op_result(t1_call),
            vec![
                (
                    BuiltinCase::Ok,
                    f1_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(t1_vec),
                        sav(t1_len),
                        sav(t1_in),
                        sav(t1_unit),
                        sav(t1_cnt),
                    ],
                ),
                (BuiltinCase::Err, f1_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let f1_er = a.op(
        ns.o,
        f1_err,
        Opcode::ResultErr,
        vec![pav(f1_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f1_err,
        function: fid,
        parameters: vec![f1_ebytes],
        operations: vec![f1_er],
        terminator: ret(op_result(f1_er)),
        reachability: Reachability::Required,
    });
    let f1_gtag = a.op(
        ns.o,
        f1_ok,
        Opcode::TupleGet,
        vec![pav(f1_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let f1_gpos = a.op(
        ns.o,
        f1_ok,
        Opcode::TupleGet,
        vec![pav(f1_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let f1_len_block = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: f1_ok,
        function: fid,
        parameters: vec![f1_tup, f1_ovec, f1_olen, f1_oin, f1_ounit, f1_ocnt],
        operations: vec![f1_gtag, f1_gpos],
        terminator: branch(edge(
            f1_len_block,
            vec![
                op_result(f1_gtag),
                op_result(f1_gpos),
                pav(f1_ovec),
                pav(f1_olen),
                pav(f1_oin),
                pav(f1_ounit),
                pav(f1_ocnt),
            ],
        )),
        reachability: Reachability::Required,
    });
    let l1_tag = a.param(ns.p, f1_len_block, ParameterRole::Block, u64_type());
    let l1_pos2 = a.param(ns.p, f1_len_block, ParameterRole::Block, u64_type());
    let l1_vec = a.param(ns.p, f1_len_block, ParameterRole::Block, u8vec_type());
    let l1_ilen = a.param(ns.p, f1_len_block, ParameterRole::Block, u64_type());
    let l1_in = a.param(ns.p, f1_len_block, ParameterRole::Block, TypeExpr::Bytes);
    let l1_unit = a.param(ns.p, f1_len_block, ParameterRole::Block, TypeExpr::Unit);
    let l1_cnt = a.param(ns.p, f1_len_block, ParameterRole::Block, u64_type());
    let l1_w = a.cref(ns.o, f1_len_block, w64, u32_type());
    let l1_call = a.op(
        ns.o,
        f1_len_block,
        Opcode::CallDirect,
        vec![pav(l1_in), pav(l1_pos2), op_result(l1_w), pav(l1_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let l1_ok = a.id(ns.b);
    let l1_err = a.id(ns.b);
    let l1_tup = a.param(
        ns.p,
        l1_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let l1_ovec = a.param(ns.p, l1_ok, ParameterRole::Block, u8vec_type());
    let l1_olen = a.param(ns.p, l1_ok, ParameterRole::Block, u64_type());
    let l1_oin = a.param(ns.p, l1_ok, ParameterRole::Block, TypeExpr::Bytes);
    let l1_ounit = a.param(ns.p, l1_ok, ParameterRole::Block, TypeExpr::Unit);
    let l1_otag = a.param(ns.p, l1_ok, ParameterRole::Block, u64_type());
    let l1_ocnt = a.param(ns.p, l1_ok, ParameterRole::Block, u64_type());
    let l1_ebytes = a.param(ns.p, l1_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f1_len_block,
        function: fid,
        parameters: vec![l1_tag, l1_pos2, l1_vec, l1_ilen, l1_in, l1_unit, l1_cnt],
        operations: vec![l1_w, l1_call],
        terminator: switch(
            op_result(l1_call),
            vec![
                (
                    BuiltinCase::Ok,
                    l1_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(l1_vec),
                        sav(l1_ilen),
                        sav(l1_in),
                        sav(l1_unit),
                        sav(l1_tag),
                        sav(l1_cnt),
                    ],
                ),
                (BuiltinCase::Err, l1_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let l1_er = a.op(
        ns.o,
        l1_err,
        Opcode::ResultErr,
        vec![pav(l1_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: l1_err,
        function: fid,
        parameters: vec![l1_ebytes],
        operations: vec![l1_er],
        terminator: ret(op_result(l1_er)),
        reachability: Reachability::Required,
    });
    let l1_glen = a.op(
        ns.o,
        l1_ok,
        Opcode::TupleGet,
        vec![pav(l1_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let l1_gpos = a.op(
        ns.o,
        l1_ok,
        Opcode::TupleGet,
        vec![pav(l1_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let l1_maxc = a.cref(ns.o, l1_ok, c_max, u64_type());
    let l1_gtmax = a.op(
        ns.o,
        l1_ok,
        Opcode::GreaterThan,
        vec![op_result(l1_glen), op_result(l1_maxc)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let l1_chk = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: l1_ok,
        function: fid,
        parameters: vec![l1_tup, l1_ovec, l1_olen, l1_oin, l1_ounit, l1_otag, l1_ocnt],
        operations: vec![l1_glen, l1_gpos, l1_maxc, l1_gtmax],
        terminator: cond(
            op_result(l1_gtmax),
            edge(b_res, Vec::new()),
            edge(
                l1_chk,
                vec![
                    pav(l1_otag),
                    op_result(l1_glen),
                    op_result(l1_gpos),
                    pav(l1_ovec),
                    pav(l1_olen),
                    pav(l1_oin),
                    pav(l1_ounit),
                    pav(l1_ocnt),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Bounds pos3+len1<=ilen else LENGTH_OVERFLOW; then tag==1 else UNKNOWN;
    // len==1 else (len<1 impossible since len>=0? len==0 -> bool-byte short
    // LENGTH_OVERFLOW; len>1 -> TRAILING for bool field? Actually bool field
    // value must be exactly 1B 00/01; len!=1 -> LENGTH_OVERFLOW (<1? len==0
    // means 0 bytes available for bool -> LENGTH_OVERFLOW) or TRAILING (>1
    // means extra bytes in field value -> TRAILING? Mirror decode_nested_exact
    // Bool: cursor over len bytes, read 1B, check_finished -> TRAILING if
    // len>1. Good.)
    let b1_tag = a.param(ns.p, l1_chk, ParameterRole::Block, u64_type());
    let b1_len1 = a.param(ns.p, l1_chk, ParameterRole::Block, u64_type());
    let b1_pos3 = a.param(ns.p, l1_chk, ParameterRole::Block, u64_type());
    let b1_vec = a.param(ns.p, l1_chk, ParameterRole::Block, u8vec_type());
    let b1_ilen = a.param(ns.p, l1_chk, ParameterRole::Block, u64_type());
    let b1_in = a.param(ns.p, l1_chk, ParameterRole::Block, TypeExpr::Bytes);
    let b1_unit = a.param(ns.p, l1_chk, ParameterRole::Block, TypeExpr::Unit);
    let b1_cnt = a.param(ns.p, l1_chk, ParameterRole::Block, u64_type());
    let b1_add = a.op(
        ns.o,
        l1_chk,
        Opcode::IntAddChecked,
        vec![pav(b1_pos3), pav(b1_len1)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let b1_unwrap = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: l1_chk,
        function: fid,
        parameters: vec![
            b1_tag, b1_len1, b1_pos3, b1_vec, b1_ilen, b1_in, b1_unit, b1_cnt,
        ],
        operations: vec![b1_add],
        terminator: switch(
            op_result(b1_add),
            vec![
                (
                    BuiltinCase::Ok,
                    b1_unwrap,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(b1_tag),
                        sav(b1_len1),
                        sav(b1_vec),
                        sav(b1_ilen),
                        sav(b1_in),
                        sav(b1_unit),
                        sav(b1_cnt),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let u1_sum = a.param(ns.p, b1_unwrap, ParameterRole::Block, u64_type());
    let u1_tag = a.param(ns.p, b1_unwrap, ParameterRole::Block, u64_type());
    let u1_len1 = a.param(ns.p, b1_unwrap, ParameterRole::Block, u64_type());
    let u1_vec = a.param(ns.p, b1_unwrap, ParameterRole::Block, u8vec_type());
    let u1_ilen = a.param(ns.p, b1_unwrap, ParameterRole::Block, u64_type());
    let u1_in = a.param(ns.p, b1_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let u1_unit = a.param(ns.p, b1_unwrap, ParameterRole::Block, TypeExpr::Unit);
    let u1_cnt = a.param(ns.p, b1_unwrap, ParameterRole::Block, u64_type());
    let u1_gt = a.op(
        ns.o,
        b1_unwrap,
        Opcode::GreaterThan,
        vec![pav(u1_sum), pav(u1_ilen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let f1_dispatch = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: b1_unwrap,
        function: fid,
        parameters: vec![
            u1_sum, u1_tag, u1_len1, u1_vec, u1_ilen, u1_in, u1_unit, u1_cnt,
        ],
        operations: vec![u1_gt],
        terminator: cond(
            op_result(u1_gt),
            edge(b_len, Vec::new()),
            edge(
                f1_dispatch,
                vec![
                    pav(u1_tag),
                    pav(u1_len1),
                    pav(u1_sum),
                    pav(u1_vec),
                    pav(u1_ilen),
                    pav(u1_in),
                    pav(u1_unit),
                    pav(u1_cnt),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // tag==1 else UNKNOWN (tags 0,2+; 3/4 also UNKNOWN here since fixture
    // record allows only tag 1; no scope in fixture scope).
    let d1_tag = a.param(ns.p, f1_dispatch, ParameterRole::Block, u64_type());
    let d1_len1 = a.param(ns.p, f1_dispatch, ParameterRole::Block, u64_type());
    let d1_end1 = a.param(ns.p, f1_dispatch, ParameterRole::Block, u64_type());
    let d1_vec = a.param(ns.p, f1_dispatch, ParameterRole::Block, u8vec_type());
    let d1_ilen = a.param(ns.p, f1_dispatch, ParameterRole::Block, u64_type());
    let d1_in = a.param(ns.p, f1_dispatch, ParameterRole::Block, TypeExpr::Bytes);
    let d1_unit = a.param(ns.p, f1_dispatch, ParameterRole::Block, TypeExpr::Unit);
    let d1_cnt = a.param(ns.p, f1_dispatch, ParameterRole::Block, u64_type());
    let k1 = a.cref(ns.o, f1_dispatch, c1, u64_type());
    let tag_eq1 = a.op(
        ns.o,
        f1_dispatch,
        Opcode::Equal,
        vec![pav(d1_tag), op_result(k1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let f1_bool = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: f1_dispatch,
        function: fid,
        parameters: vec![
            d1_tag, d1_len1, d1_end1, d1_vec, d1_ilen, d1_in, d1_unit, d1_cnt,
        ],
        operations: vec![k1, tag_eq1],
        terminator: cond(
            op_result(tag_eq1),
            edge(
                f1_bool,
                vec![
                    pav(d1_len1),
                    pav(d1_end1),
                    pav(d1_vec),
                    pav(d1_ilen),
                    pav(d1_in),
                    pav(d1_unit),
                    pav(d1_cnt),
                ],
            ),
            edge(b_unknown, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // len==1 else LENGTH_OVERFLOW (len==0) / TRAILING (len>1) for bool field.
    // Then bool byte via VectorGet at (end1-1)? Actually pos3 = end1-len1,
    // bool at pos3 (since len1==1, single byte). Compute idx = end1-1 via
    // checked-sub (underflow unreachable: end1>=1 by len1==1 + bounds).
    let bl_len1 = a.param(ns.p, f1_bool, ParameterRole::Block, u64_type());
    let bl_end1 = a.param(ns.p, f1_bool, ParameterRole::Block, u64_type());
    let bl_vec = a.param(ns.p, f1_bool, ParameterRole::Block, u8vec_type());
    let bl_ilen = a.param(ns.p, f1_bool, ParameterRole::Block, u64_type());
    let bl_in = a.param(ns.p, f1_bool, ParameterRole::Block, TypeExpr::Bytes);
    let bl_unit = a.param(ns.p, f1_bool, ParameterRole::Block, TypeExpr::Unit);
    let bl_cnt = a.param(ns.p, f1_bool, ParameterRole::Block, u64_type());
    let k1b = a.cref(ns.o, f1_bool, c1, u64_type());
    let lt1 = a.op(
        ns.o,
        f1_bool,
        Opcode::LessThan,
        vec![pav(bl_len1), op_result(k1b)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let gt1 = a.op(
        ns.o,
        f1_bool,
        Opcode::GreaterThan,
        vec![pav(bl_len1), op_result(k1b)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let bl_chk_gt = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: f1_bool,
        function: fid,
        parameters: vec![bl_len1, bl_end1, bl_vec, bl_ilen, bl_in, bl_unit, bl_cnt],
        operations: vec![k1b, lt1, gt1],
        terminator: cond(
            op_result(lt1),
            edge(b_len, Vec::new()),
            edge(
                bl_chk_gt,
                vec![
                    pav(bl_len1),
                    pav(bl_end1),
                    pav(bl_vec),
                    pav(bl_ilen),
                    pav(bl_in),
                    pav(bl_unit),
                    pav(bl_cnt),
                    op_result(gt1),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let gt_len1 = a.param(ns.p, bl_chk_gt, ParameterRole::Block, u64_type());
    let gt_end1 = a.param(ns.p, bl_chk_gt, ParameterRole::Block, u64_type());
    let gt_vec = a.param(ns.p, bl_chk_gt, ParameterRole::Block, u8vec_type());
    let gt_ilen = a.param(ns.p, bl_chk_gt, ParameterRole::Block, u64_type());
    let gt_in = a.param(ns.p, bl_chk_gt, ParameterRole::Block, TypeExpr::Bytes);
    let gt_unit = a.param(ns.p, bl_chk_gt, ParameterRole::Block, TypeExpr::Unit);
    let gt_cnt = a.param(ns.p, bl_chk_gt, ParameterRole::Block, u64_type());
    let gt_flag = a.param(ns.p, bl_chk_gt, ParameterRole::Block, TypeExpr::Bool);
    a.blocks.push(Block {
        entity_id: bl_chk_gt,
        function: fid,
        parameters: vec![
            gt_len1, gt_end1, gt_vec, gt_ilen, gt_in, gt_unit, gt_cnt, gt_flag,
        ],
        operations: Vec::new(),
        terminator: cond(
            pav(gt_flag),
            edge(b_trail, Vec::new()),
            edge(b_unknown, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // NOTE: bool-byte VectorGet + trailing + Ok(Bool) + count==2 handling
    // (duplicate/order/unknown for second field) lands next edit. Current
    // parks len==1 valid path as UNKNOWN (no success yet). This keeps the
    // diff reviewable; full bool + trailing + second-field lands next.
    let _ = (c0, c1);
    let _ = (b_dup, b_order, b_bool);

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![p_in, p_unit],
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
    .expect("program-outer image lowers under the reference lowerer");
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
    .expect("program-outer image admits under V2");
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
    let (digests, receipt, report) = sley_vm::admit_v2_package(&closure, &package)
        .expect("authority admits program-outer image");
    assert_eq!(
        receipt.profile_digest(),
        &sley_vm::BOOTSTRAP_PROFILE_2_DIGEST,
        "program-outer receipt binds the successor profile"
    );
    let approved = approve_package_v2(&package, &digests, receipt, &report)
        .expect("v2 approves program-outer image");
    (package, approved)
}

fn empty_decode_image() -> Image {
    use sley_vm::host_abi::BRIDGE_CODE_B2V1;
    let mut a = Asm::new();
    let dns = Ns {
        k: 51,
        p: 52,
        b: 53,
        o: 54,
    };
    let ens = Ns {
        k: 51,
        p: 55,
        b: 56,
        o: 57,
    };
    let decode_fid = eid(9, 21);
    let empty_fid = eid(9, 22);
    let (decode_graph, _) = build_decode(&mut a, dns, decode_fid);
    let empty_graph = build_fixture_empty_decode(&mut a, ens, empty_fid, decode_fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: empty_graph.clone(),
        functions: vec![empty_graph, decode_graph],
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

fn empty_encode_image() -> Image {
    use sley_vm::host_abi::{BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let mut a = Asm::new();
    let ns = Ns {
        k: 61,
        p: 62,
        b: 63,
        o: 64,
    };
    let fid = eid(9, 23);
    let graph = build_fixture_empty_encode(&mut a, ns, fid);
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
    .expect("v2 executes program-outer image")
}

fn hex_decode(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "even hex length");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
        .collect()
}

fn assert_empty_ok(outcome: &sley_vm::ExecutionOutcome) -> u64 {
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                ConstData::Unit => outcome.fuel_used,
                other => panic!("empty Ok must carry Unit, got {other:?}"),
            },
            other => panic!("empty must succeed, got {other:?}"),
        },
        other => panic!("empty must succeed, got {other:?}"),
    }
}

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
                    assert_eq!(bytes, expected, "emitted bytes must match exactly");
                    outcome.fuel_used
                }
                other => panic!("encode Ok must carry Bytes, got {other:?}"),
            },
            other => panic!("encode must succeed, got {other:?}"),
        },
        other => panic!("encode must succeed, got {other:?}"),
    }
}

fn ref_empty(bytes: &[u8]) -> Result<(), String> {
    match sley_scb1::decode_payload_exact(&sley_scb1::Schema::FixtureEmptyObject, bytes) {
        Ok(()) => Ok(()),
        Err(error) => Err(error.code().to_string()),
    }
}

fn empty_decode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    input_hex: &str,
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![bytes_input(&hex_decode(input_hex)), unit_input()],
    )
}

fn empty_encode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
) -> sley_vm::ExecutionOutcome {
    execute(package, approved, vec![unit_input()])
}

#[test]
fn fixture_empty_valid_and_encode_bytes() {
    let (dec_pkg, dec_approved) = admit(&empty_decode_image());
    let (enc_pkg, enc_approved) = admit(&empty_encode_image());
    assert_empty_ok(&empty_decode_call(&dec_pkg, &dec_approved, "00"));
    assert_eq!(
        ref_empty(&hex_decode("00")),
        Ok(()),
        "reference agrees on 00"
    );
    let expected = sley_scb1::encode_uvar(0);
    assert_eq!(expected, hex_decode("00"), "reference emits 00 for 0");
    assert_encode_ok(
        &empty_encode_call(&enc_pkg, &enc_approved),
        &hex_decode("00"),
    );
    assert_empty_ok(&empty_decode_call(&dec_pkg, &dec_approved, "00"));
}

#[test]
fn fixture_empty_rejections_match_reference() {
    let (package, approved) = admit(&empty_decode_image());
    let cases: [(&str, &str); 8] = [
        ("", "SCB_LENGTH_OVERFLOW"),
        ("80", "SCB_LENGTH_OVERFLOW"),
        ("01", "SCB_FIELD_UNKNOWN"),
        ("02", "SCB_FIELD_UNKNOWN"),
        ("0000", "SCB_TRAILING_BYTES"),
        ("01010101", "SCB_FIELD_UNKNOWN"),
        ("8000", "SCB_VARINT_NON_MINIMAL"),
        ("8100", "SCB_VARINT_NON_MINIMAL"),
    ];
    for (input_hex, expected) in cases {
        assert_refusal(&empty_decode_call(&package, &approved, input_hex), expected);
        assert_eq!(
            ref_empty(&hex_decode(input_hex)),
            Err(expected.to_string()),
            "reference agrees on {input_hex}"
        );
    }
}

#[test]
fn fixture_empty_runtime_mutations_agree_with_reference() {
    let (package, approved) = admit(&empty_decode_image());
    let base = hex_decode("00");
    let mut state = 0xE11E_0003u64;
    let mut lcg = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    for _ in 0..100 {
        let mut bytes = base.clone();
        match lcg() % 4 {
            0 => {
                if bytes.is_empty() {
                    bytes.push(u8::try_from(lcg() % 256).expect("lcg byte fits u8"));
                } else {
                    let at = usize::try_from(lcg()).expect("lcg fits usize") % bytes.len();
                    let bit = 1u8 << u32::try_from(lcg() % 8).expect("lcg remainder fits u32");
                    bytes[at] ^= bit;
                }
            }
            1 => {
                bytes.pop();
            }
            2 => {
                bytes.push(u8::try_from(lcg() % 256).expect("lcg byte fits u8"));
            }
            _ => {
                let mut shifted = vec![u8::try_from(lcg() % 256).expect("lcg byte fits u8")];
                shifted.extend_from_slice(&bytes);
                bytes = shifted;
            }
        }
        if bytes.len() > 1_048_576 {
            bytes.truncate(1_048_576);
        }
        let outcome = execute(&package, &approved, vec![bytes_input(&bytes), unit_input()]);
        match ref_empty(&bytes) {
            Ok(()) => {
                assert_empty_ok(&outcome);
            }
            Err(code) => {
                assert_refusal(&outcome, &code);
            }
        }
    }
}

#[test]
fn fixture_empty_resources_stay_far_inside_codec_budgets() {
    let (dec_pkg, dec_approved) = admit(&empty_decode_image());
    let (enc_pkg, enc_approved) = admit(&empty_encode_image());
    let dec_outcome = empty_decode_call(&dec_pkg, &dec_approved, "00");
    let _ = assert_empty_ok(&dec_outcome);
    eprintln!(
        "EMPTY_FUEL decode(payload 1B) fuel={} instr={} peak_value_units={}",
        dec_outcome.fuel_used, dec_outcome.instruction_count, dec_outcome.peak_value_units
    );
    assert!(
        dec_outcome.fuel_used < 1_000_000,
        "decode fuel inside budget"
    );
    assert!(
        dec_outcome.instruction_count < 100_000,
        "decode instructions inside budget"
    );
    assert!(
        dec_outcome.peak_value_units < 1_000_000,
        "decode value units inside budget"
    );
    assert!(
        dec_outcome.peak_value_units < 100_000,
        "empty decode stays far below envelope 204B peak 741117"
    );
    let enc_outcome = empty_encode_call(&enc_pkg, &enc_approved);
    let _ = assert_encode_ok(&enc_outcome, &hex_decode("00"));
    eprintln!(
        "EMPTY_FUEL encode(Unit->1B) fuel={} instr={} peak_value_units={}",
        enc_outcome.fuel_used, enc_outcome.instruction_count, enc_outcome.peak_value_units
    );
    assert!(
        enc_outcome.fuel_used < 1_000_000,
        "encode fuel inside budget"
    );
    assert!(
        enc_outcome.instruction_count < 100_000,
        "encode instructions inside budget"
    );
    assert!(
        enc_outcome.peak_value_units < 1_000_000,
        "encode value units inside budget"
    );
    assert!(
        enc_outcome.peak_value_units < 100_000,
        "empty encode stays far below envelope 204B peak"
    );
}

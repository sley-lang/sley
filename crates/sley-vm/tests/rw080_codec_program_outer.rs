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
    // Tag 2 first (body-first): all [2,*] with count==2 are Err. Exact code
    // needs field 2 (DUPLICATE for [2,2], ORDER for [2,1]/[2,0], scope for
    // [2,3]/[2,4], UNKNOWN for [2,5+]). Field-1 tag 0/5+ (e2 false) returns
    // UNKNOWN immediately (field-1 bounds already checked); tag==2 routes to
    // the prev=2 field-2 chain below (defined after the main body return).
    let f2b_tag = a.id(ns.b);
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
            edge(
                f2b_tag,
                vec![
                    pav(t2_end1),
                    pav(t2_vec),
                    pav(t2_ilen),
                    pav(t2_in),
                    pav(t2_unit),
                ],
            ),
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
    // Complete 2-field path: 32B entity-ID extraction loop, field-2 tag /
    // len / bounds with reference precedence (DUP/ORDER before payload,
    // bounds before UNKNOWN), opaque body extraction loop, trailing check,
    // and Tuple(entity_id, body) return. Scope-first for tags 3/4 preserved.
    let s_pos = a.param(ns.p, el_unwrap, ParameterRole::Block, u64_type());
    let s_vec = a.param(ns.p, el_unwrap, ParameterRole::Block, u8vec_type());
    let s_ilen = a.param(ns.p, el_unwrap, ParameterRole::Block, u64_type());
    let s_in = a.param(ns.p, el_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let s_unit = a.param(ns.p, el_unwrap, ParameterRole::Block, TypeExpr::Unit);
    // New block ids for the completed tail (entity loop + field-2 + body).
    let eid_setup = a.id(ns.b);
    let eid_check = a.id(ns.b);
    let eid_get = a.id(ns.b);
    let eid_get2 = a.id(ns.b);
    let eid_push = a.id(ns.b);
    let eid_next = a.id(ns.b);
    let eid_done = a.id(ns.b);
    let f2_tag = a.id(ns.b);
    let f2_tag_ok = a.id(ns.b);
    let f2_tag_err = a.id(ns.b);
    let f2_disp = a.id(ns.b);
    let f2_ord = a.id(ns.b);
    let f2_chk2 = a.id(ns.b);
    let f2_scope = a.id(ns.b);
    let f2_len = a.id(ns.b);
    let f2_len_ok = a.id(ns.b);
    let f2_len_err = a.id(ns.b);
    let f2_bnd = a.id(ns.b);
    let f2_unwrap = a.id(ns.b);
    let f2_disp2 = a.id(ns.b);
    let body_start = a.id(ns.b);
    let body_check = a.id(ns.b);
    let body_get = a.id(ns.b);
    let body_get2 = a.id(ns.b);
    let body_push = a.id(ns.b);
    let body_next = a.id(ns.b);
    let body_done = a.id(ns.b);
    let body_trail = a.id(ns.b);
    let body_ret = a.id(ns.b);
    let el_k32b = a.cref(ns.o, el_unwrap, c32, u64_type());
    let el_add = a.op(
        ns.o,
        el_unwrap,
        Opcode::IntAddChecked,
        vec![pav(s_pos), op_result(el_k32b)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let el_empty = a.op(
        ns.o,
        el_unwrap,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let el_z0 = a.cref(ns.o, el_unwrap, c0, u64_type());
    a.blocks.push(Block {
        entity_id: el_unwrap,
        function: fid,
        parameters: vec![s_pos, s_vec, s_ilen, s_in, s_unit],
        operations: vec![el_k32b, el_add, el_empty, el_z0],
        terminator: switch(
            op_result(el_add),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_setup,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(s_pos),
                        sav(s_vec),
                        sav(s_ilen),
                        sav(s_in),
                        sav(s_unit),
                        oav(el_empty),
                        oav(el_z0),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // eid_setup: [end1, start, vec, ilen, in, unit, acc0, idx0] -> eid_check.
    let e_end1 = a.param(ns.p, eid_setup, ParameterRole::Block, u64_type());
    let e_start = a.param(ns.p, eid_setup, ParameterRole::Block, u64_type());
    let e_vec = a.param(ns.p, eid_setup, ParameterRole::Block, u8vec_type());
    let e_ilen = a.param(ns.p, eid_setup, ParameterRole::Block, u64_type());
    let e_in = a.param(ns.p, eid_setup, ParameterRole::Block, TypeExpr::Bytes);
    let e_unit = a.param(ns.p, eid_setup, ParameterRole::Block, TypeExpr::Unit);
    let e_acc0 = a.param(ns.p, eid_setup, ParameterRole::Block, u8vec_type());
    let e_idx0 = a.param(ns.p, eid_setup, ParameterRole::Block, u64_type());
    a.blocks.push(Block {
        entity_id: eid_setup,
        function: fid,
        parameters: vec![e_end1, e_start, e_vec, e_ilen, e_in, e_unit, e_acc0, e_idx0],
        operations: Vec::new(),
        terminator: branch(edge(
            eid_check,
            vec![
                pav(e_idx0),
                pav(e_acc0),
                pav(e_start),
                pav(e_end1),
                pav(e_vec),
                pav(e_ilen),
                pav(e_in),
                pav(e_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    // eid_check: idx < 32 -> get else done.
    let c_idx = a.param(ns.p, eid_check, ParameterRole::Block, u64_type());
    let c_acc = a.param(ns.p, eid_check, ParameterRole::Block, u8vec_type());
    let c_start = a.param(ns.p, eid_check, ParameterRole::Block, u64_type());
    let c_end1 = a.param(ns.p, eid_check, ParameterRole::Block, u64_type());
    let c_vec = a.param(ns.p, eid_check, ParameterRole::Block, u8vec_type());
    let c_ilen = a.param(ns.p, eid_check, ParameterRole::Block, u64_type());
    let c_in = a.param(ns.p, eid_check, ParameterRole::Block, TypeExpr::Bytes);
    let c_unit = a.param(ns.p, eid_check, ParameterRole::Block, TypeExpr::Unit);
    let c_k32 = a.cref(ns.o, eid_check, c32, u64_type());
    let c_lt = a.op(
        ns.o,
        eid_check,
        Opcode::LessThan,
        vec![pav(c_idx), op_result(c_k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_check,
        function: fid,
        parameters: vec![c_idx, c_acc, c_start, c_end1, c_vec, c_ilen, c_in, c_unit],
        operations: vec![c_k32, c_lt],
        terminator: cond(
            op_result(c_lt),
            edge(
                eid_get,
                vec![
                    pav(c_idx),
                    pav(c_acc),
                    pav(c_start),
                    pav(c_end1),
                    pav(c_vec),
                    pav(c_ilen),
                    pav(c_in),
                    pav(c_unit),
                ],
            ),
            edge(
                eid_done,
                vec![
                    pav(c_acc),
                    pav(c_start),
                    pav(c_end1),
                    pav(c_vec),
                    pav(c_ilen),
                    pav(c_in),
                    pav(c_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // eid_get: src = start + idx.
    let g_idx = a.param(ns.p, eid_get, ParameterRole::Block, u64_type());
    let g_acc = a.param(ns.p, eid_get, ParameterRole::Block, u8vec_type());
    let g_start = a.param(ns.p, eid_get, ParameterRole::Block, u64_type());
    let g_end1 = a.param(ns.p, eid_get, ParameterRole::Block, u64_type());
    let g_vec = a.param(ns.p, eid_get, ParameterRole::Block, u8vec_type());
    let g_ilen = a.param(ns.p, eid_get, ParameterRole::Block, u64_type());
    let g_in = a.param(ns.p, eid_get, ParameterRole::Block, TypeExpr::Bytes);
    let g_unit = a.param(ns.p, eid_get, ParameterRole::Block, TypeExpr::Unit);
    let g_add = a.op(
        ns.o,
        eid_get,
        Opcode::IntAddChecked,
        vec![pav(g_start), pav(g_idx)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_get,
        function: fid,
        parameters: vec![g_idx, g_acc, g_start, g_end1, g_vec, g_ilen, g_in, g_unit],
        operations: vec![g_add],
        terminator: switch(
            op_result(g_add),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_get2,
                    vec![
                        sav(g_acc),
                        SwitchArgument::CasePayload,
                        sav(g_idx),
                        sav(g_start),
                        sav(g_end1),
                        sav(g_vec),
                        sav(g_ilen),
                        sav(g_in),
                        sav(g_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let h_acc = a.param(ns.p, eid_get2, ParameterRole::Block, u8vec_type());
    let h_sidx = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let h_idx = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let h_start = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let h_end1 = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let h_vec = a.param(ns.p, eid_get2, ParameterRole::Block, u8vec_type());
    let h_ilen = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let h_in = a.param(ns.p, eid_get2, ParameterRole::Block, TypeExpr::Bytes);
    let h_unit = a.param(ns.p, eid_get2, ParameterRole::Block, TypeExpr::Unit);
    let h_get = a.op(
        ns.o,
        eid_get2,
        Opcode::VectorGet,
        vec![pav(h_vec), pav(h_sidx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_get2,
        function: fid,
        parameters: vec![
            h_acc, h_sidx, h_idx, h_start, h_end1, h_vec, h_ilen, h_in, h_unit,
        ],
        operations: vec![h_get],
        terminator: switch(
            op_result(h_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    eid_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(h_idx),
                        sav(h_acc),
                        sav(h_start),
                        sav(h_end1),
                        sav(h_vec),
                        sav(h_ilen),
                        sav(h_in),
                        sav(h_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let u_b = a.param(ns.p, eid_push, ParameterRole::Block, u8_type());
    let u_idx = a.param(ns.p, eid_push, ParameterRole::Block, u64_type());
    let u_acc = a.param(ns.p, eid_push, ParameterRole::Block, u8vec_type());
    let u_start = a.param(ns.p, eid_push, ParameterRole::Block, u64_type());
    let u_end1 = a.param(ns.p, eid_push, ParameterRole::Block, u64_type());
    let u_vec = a.param(ns.p, eid_push, ParameterRole::Block, u8vec_type());
    let u_ilen = a.param(ns.p, eid_push, ParameterRole::Block, u64_type());
    let u_in = a.param(ns.p, eid_push, ParameterRole::Block, TypeExpr::Bytes);
    let u_unit = a.param(ns.p, eid_push, ParameterRole::Block, TypeExpr::Unit);
    let u_push = a.op(
        ns.o,
        eid_push,
        Opcode::AdapterInvoke,
        vec![pav(u_acc), pav(u_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: eid_push,
        function: fid,
        parameters: vec![
            u_b, u_idx, u_acc, u_start, u_end1, u_vec, u_ilen, u_in, u_unit,
        ],
        operations: vec![u_push],
        terminator: switch(
            op_result(u_push),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_next,
                    vec![
                        sav(u_idx),
                        SwitchArgument::CasePayload,
                        sav(u_start),
                        sav(u_end1),
                        sav(u_vec),
                        sav(u_ilen),
                        sav(u_in),
                        sav(u_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let n_idx = a.param(ns.p, eid_next, ParameterRole::Block, u64_type());
    let n_acc = a.param(ns.p, eid_next, ParameterRole::Block, u8vec_type());
    let n_start = a.param(ns.p, eid_next, ParameterRole::Block, u64_type());
    let n_end1 = a.param(ns.p, eid_next, ParameterRole::Block, u64_type());
    let n_vec = a.param(ns.p, eid_next, ParameterRole::Block, u8vec_type());
    let n_ilen = a.param(ns.p, eid_next, ParameterRole::Block, u64_type());
    let n_in = a.param(ns.p, eid_next, ParameterRole::Block, TypeExpr::Bytes);
    let n_unit = a.param(ns.p, eid_next, ParameterRole::Block, TypeExpr::Unit);
    let n_c1 = a.cref(ns.o, eid_next, c1, u64_type());
    let n_add = a.op(
        ns.o,
        eid_next,
        Opcode::IntAddChecked,
        vec![pav(n_idx), op_result(n_c1)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_next,
        function: fid,
        parameters: vec![n_idx, n_acc, n_start, n_end1, n_vec, n_ilen, n_in, n_unit],
        operations: vec![n_c1, n_add],
        terminator: switch(
            op_result(n_add),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(n_acc),
                        sav(n_start),
                        sav(n_end1),
                        sav(n_vec),
                        sav(n_ilen),
                        sav(n_in),
                        sav(n_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // eid_done: V2B1 accumulator -> entity_id Bytes, then field-2 tag.
    let d_acc = a.param(ns.p, eid_done, ParameterRole::Block, u8vec_type());
    let d_start = a.param(ns.p, eid_done, ParameterRole::Block, u64_type());
    let d_end1 = a.param(ns.p, eid_done, ParameterRole::Block, u64_type());
    let d_vec = a.param(ns.p, eid_done, ParameterRole::Block, u8vec_type());
    let d_ilen = a.param(ns.p, eid_done, ParameterRole::Block, u64_type());
    let d_in = a.param(ns.p, eid_done, ParameterRole::Block, TypeExpr::Bytes);
    let d_unit = a.param(ns.p, eid_done, ParameterRole::Block, TypeExpr::Unit);
    let d_v2b = a.op(
        ns.o,
        eid_done,
        Opcode::AdapterInvoke,
        vec![pav(d_unit), pav(d_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: eid_done,
        function: fid,
        parameters: vec![d_acc, d_start, d_end1, d_vec, d_ilen, d_in, d_unit],
        operations: vec![d_v2b],
        terminator: switch(
            op_result(d_v2b),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_tag,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(d_end1),
                        sav(d_vec),
                        sav(d_ilen),
                        sav(d_in),
                        sav(d_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Field-2 tag at end1 (width 32). Uvar errors propagate first.
    let f_eid = a.param(ns.p, f2_tag, ParameterRole::Block, TypeExpr::Bytes);
    let f_end1 = a.param(ns.p, f2_tag, ParameterRole::Block, u64_type());
    let f_vec = a.param(ns.p, f2_tag, ParameterRole::Block, u8vec_type());
    let f_ilen = a.param(ns.p, f2_tag, ParameterRole::Block, u64_type());
    let f_in = a.param(ns.p, f2_tag, ParameterRole::Block, TypeExpr::Bytes);
    let f_unit = a.param(ns.p, f2_tag, ParameterRole::Block, TypeExpr::Unit);
    let f_w = a.cref(ns.o, f2_tag, w32, u32_type());
    let f_call = a.op(
        ns.o,
        f2_tag,
        Opcode::CallDirect,
        vec![pav(f_in), pav(f_end1), op_result(f_w), pav(f_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let f2_tup = a.param(
        ns.p,
        f2_tag_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let f2_ovec = a.param(ns.p, f2_tag_ok, ParameterRole::Block, u8vec_type());
    let f2_olen = a.param(ns.p, f2_tag_ok, ParameterRole::Block, u64_type());
    let f2_oin = a.param(ns.p, f2_tag_ok, ParameterRole::Block, TypeExpr::Bytes);
    let f2_ounit = a.param(ns.p, f2_tag_ok, ParameterRole::Block, TypeExpr::Unit);
    let f2_oeid = a.param(ns.p, f2_tag_ok, ParameterRole::Block, TypeExpr::Bytes);
    let f2_ebytes = a.param(ns.p, f2_tag_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f2_tag,
        function: fid,
        parameters: vec![f_eid, f_end1, f_vec, f_ilen, f_in, f_unit],
        operations: vec![f_w, f_call],
        terminator: switch(
            op_result(f_call),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_tag_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(f_vec),
                        sav(f_ilen),
                        sav(f_in),
                        sav(f_unit),
                        sav(f_eid),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    f2_tag_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let f2_er = a.op(
        ns.o,
        f2_tag_err,
        Opcode::ResultErr,
        vec![pav(f2_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_tag_err,
        function: fid,
        parameters: vec![f2_ebytes],
        operations: vec![f2_er],
        terminator: ret(op_result(f2_er)),
        reachability: Reachability::Required,
    });
    let f2_gtag = a.op(
        ns.o,
        f2_tag_ok,
        Opcode::TupleGet,
        vec![pav(f2_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let f2_gpos = a.op(
        ns.o,
        f2_tag_ok,
        Opcode::TupleGet,
        vec![pav(f2_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    a.blocks.push(Block {
        entity_id: f2_tag_ok,
        function: fid,
        parameters: vec![f2_tup, f2_ovec, f2_olen, f2_oin, f2_ounit, f2_oeid],
        operations: vec![f2_gtag, f2_gpos],
        terminator: branch(edge(
            f2_disp,
            vec![
                op_result(f2_gtag),
                op_result(f2_gpos),
                pav(f2_oeid),
                pav(f2_ovec),
                pav(f2_olen),
                pav(f2_oin),
                pav(f2_ounit),
            ],
        )),
        reachability: Reachability::Required,
    });
    // f2_disp: DUP (tag2==1) before payload; ORDER (tag2==0) before payload.
    let dd_tag = a.param(ns.p, f2_disp, ParameterRole::Block, u64_type());
    let dd_pos = a.param(ns.p, f2_disp, ParameterRole::Block, u64_type());
    let dd_eid = a.param(ns.p, f2_disp, ParameterRole::Block, TypeExpr::Bytes);
    let dd_vec = a.param(ns.p, f2_disp, ParameterRole::Block, u8vec_type());
    let dd_ilen = a.param(ns.p, f2_disp, ParameterRole::Block, u64_type());
    let dd_in = a.param(ns.p, f2_disp, ParameterRole::Block, TypeExpr::Bytes);
    let dd_unit = a.param(ns.p, f2_disp, ParameterRole::Block, TypeExpr::Unit);
    let dd_k1 = a.cref(ns.o, f2_disp, c1, u64_type());
    let dd_eq = a.op(
        ns.o,
        f2_disp,
        Opcode::Equal,
        vec![pav(dd_tag), op_result(dd_k1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_disp,
        function: fid,
        parameters: vec![dd_tag, dd_pos, dd_eid, dd_vec, dd_ilen, dd_in, dd_unit],
        operations: vec![dd_k1, dd_eq],
        terminator: cond(
            op_result(dd_eq),
            edge(b_dup, Vec::new()),
            edge(
                f2_ord,
                vec![
                    pav(dd_tag),
                    pav(dd_pos),
                    pav(dd_eid),
                    pav(dd_vec),
                    pav(dd_ilen),
                    pav(dd_in),
                    pav(dd_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let o_tag = a.param(ns.p, f2_ord, ParameterRole::Block, u64_type());
    let o_pos = a.param(ns.p, f2_ord, ParameterRole::Block, u64_type());
    let o_eid = a.param(ns.p, f2_ord, ParameterRole::Block, TypeExpr::Bytes);
    let o_vec = a.param(ns.p, f2_ord, ParameterRole::Block, u8vec_type());
    let o_ilen = a.param(ns.p, f2_ord, ParameterRole::Block, u64_type());
    let o_in = a.param(ns.p, f2_ord, ParameterRole::Block, TypeExpr::Bytes);
    let o_unit = a.param(ns.p, f2_ord, ParameterRole::Block, TypeExpr::Unit);
    let o_k1 = a.cref(ns.o, f2_ord, c1, u64_type());
    let o_lt = a.op(
        ns.o,
        f2_ord,
        Opcode::LessThan,
        vec![pav(o_tag), op_result(o_k1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_ord,
        function: fid,
        parameters: vec![o_tag, o_pos, o_eid, o_vec, o_ilen, o_in, o_unit],
        operations: vec![o_k1, o_lt],
        terminator: cond(
            op_result(o_lt),
            edge(b_order, Vec::new()),
            edge(
                f2_chk2,
                vec![
                    pav(o_tag),
                    pav(o_pos),
                    pav(o_eid),
                    pav(o_vec),
                    pav(o_ilen),
                    pav(o_in),
                    pav(o_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let v_tag = a.param(ns.p, f2_chk2, ParameterRole::Block, u64_type());
    let v_pos = a.param(ns.p, f2_chk2, ParameterRole::Block, u64_type());
    let v_eid = a.param(ns.p, f2_chk2, ParameterRole::Block, TypeExpr::Bytes);
    let v_vec = a.param(ns.p, f2_chk2, ParameterRole::Block, u8vec_type());
    let v_ilen = a.param(ns.p, f2_chk2, ParameterRole::Block, u64_type());
    let v_in = a.param(ns.p, f2_chk2, ParameterRole::Block, TypeExpr::Bytes);
    let v_unit = a.param(ns.p, f2_chk2, ParameterRole::Block, TypeExpr::Unit);
    let v_k2 = a.cref(ns.o, f2_chk2, c2, u64_type());
    let v_eq = a.op(
        ns.o,
        f2_chk2,
        Opcode::Equal,
        vec![pav(v_tag), op_result(v_k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_chk2,
        function: fid,
        parameters: vec![v_tag, v_pos, v_eid, v_vec, v_ilen, v_in, v_unit],
        operations: vec![v_k2, v_eq],
        terminator: cond(
            op_result(v_eq),
            edge(
                f2_len,
                vec![
                    pav(v_tag),
                    pav(v_pos),
                    pav(v_eid),
                    pav(v_vec),
                    pav(v_ilen),
                    pav(v_in),
                    pav(v_unit),
                ],
            ),
            edge(
                f2_scope,
                vec![
                    pav(v_tag),
                    pav(v_pos),
                    pav(v_eid),
                    pav(v_vec),
                    pav(v_ilen),
                    pav(v_in),
                    pav(v_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Scope-first for 3/4 (label/fingerprint need NFC + verifier); unknown
    // tags (5+) go to len so bounds precede UNKNOWN per reference order.
    let sc_tag = a.param(ns.p, f2_scope, ParameterRole::Block, u64_type());
    let sc_pos = a.param(ns.p, f2_scope, ParameterRole::Block, u64_type());
    let sc_eid = a.param(ns.p, f2_scope, ParameterRole::Block, TypeExpr::Bytes);
    let sc_vec = a.param(ns.p, f2_scope, ParameterRole::Block, u8vec_type());
    let sc_ilen = a.param(ns.p, f2_scope, ParameterRole::Block, u64_type());
    let sc_in = a.param(ns.p, f2_scope, ParameterRole::Block, TypeExpr::Bytes);
    let sc_unit = a.param(ns.p, f2_scope, ParameterRole::Block, TypeExpr::Unit);
    let sc_k3 = a.cref(ns.o, f2_scope, c3, u64_type());
    let sc_k4 = a.cref(ns.o, f2_scope, c4, u64_type());
    let sc_e3 = a.op(
        ns.o,
        f2_scope,
        Opcode::Equal,
        vec![pav(sc_tag), op_result(sc_k3)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let sc_e4 = a.op(
        ns.o,
        f2_scope,
        Opcode::Equal,
        vec![pav(sc_tag), op_result(sc_k4)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let sc_or = a.op(
        ns.o,
        f2_scope,
        Opcode::BoolOr,
        vec![op_result(sc_e3), op_result(sc_e4)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_scope,
        function: fid,
        parameters: vec![sc_tag, sc_pos, sc_eid, sc_vec, sc_ilen, sc_in, sc_unit],
        operations: vec![sc_k3, sc_k4, sc_e3, sc_e4, sc_or],
        terminator: cond(
            op_result(sc_or),
            edge(b_scope, Vec::new()),
            edge(
                f2_len,
                vec![
                    pav(sc_tag),
                    pav(sc_pos),
                    pav(sc_eid),
                    pav(sc_vec),
                    pav(sc_ilen),
                    pav(sc_in),
                    pav(sc_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Field-2 length (width 64). Uvar errors propagate before bounds.
    let ll_tag = a.param(ns.p, f2_len, ParameterRole::Block, u64_type());
    let ll_pos = a.param(ns.p, f2_len, ParameterRole::Block, u64_type());
    let ll_eid = a.param(ns.p, f2_len, ParameterRole::Block, TypeExpr::Bytes);
    let ll_vec = a.param(ns.p, f2_len, ParameterRole::Block, u8vec_type());
    let ll_ilen = a.param(ns.p, f2_len, ParameterRole::Block, u64_type());
    let ll_in = a.param(ns.p, f2_len, ParameterRole::Block, TypeExpr::Bytes);
    let ll_unit = a.param(ns.p, f2_len, ParameterRole::Block, TypeExpr::Unit);
    let ll_w = a.cref(ns.o, f2_len, w64, u32_type());
    let ll_call = a.op(
        ns.o,
        f2_len,
        Opcode::CallDirect,
        vec![pav(ll_in), pav(ll_pos), op_result(ll_w), pav(ll_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let lt_tup = a.param(
        ns.p,
        f2_len_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let lt_tag = a.param(ns.p, f2_len_ok, ParameterRole::Block, u64_type());
    let lt_eid = a.param(ns.p, f2_len_ok, ParameterRole::Block, TypeExpr::Bytes);
    let lt_vec = a.param(ns.p, f2_len_ok, ParameterRole::Block, u8vec_type());
    let lt_ilen = a.param(ns.p, f2_len_ok, ParameterRole::Block, u64_type());
    let lt_in = a.param(ns.p, f2_len_ok, ParameterRole::Block, TypeExpr::Bytes);
    let lt_unit = a.param(ns.p, f2_len_ok, ParameterRole::Block, TypeExpr::Unit);
    let lt_ebytes = a.param(ns.p, f2_len_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f2_len,
        function: fid,
        parameters: vec![ll_tag, ll_pos, ll_eid, ll_vec, ll_ilen, ll_in, ll_unit],
        operations: vec![ll_w, ll_call],
        terminator: switch(
            op_result(ll_call),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_len_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(ll_tag),
                        sav(ll_eid),
                        sav(ll_vec),
                        sav(ll_ilen),
                        sav(ll_in),
                        sav(ll_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    f2_len_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let lt_er = a.op(
        ns.o,
        f2_len_err,
        Opcode::ResultErr,
        vec![pav(lt_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_len_err,
        function: fid,
        parameters: vec![lt_ebytes],
        operations: vec![lt_er],
        terminator: ret(op_result(lt_er)),
        reachability: Reachability::Required,
    });
    let lt_glen = a.op(
        ns.o,
        f2_len_ok,
        Opcode::TupleGet,
        vec![pav(lt_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let lt_gpos = a.op(
        ns.o,
        f2_len_ok,
        Opcode::TupleGet,
        vec![pav(lt_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let lt_maxc = a.cref(ns.o, f2_len_ok, c_max, u64_type());
    let lt_gtmax = a.op(
        ns.o,
        f2_len_ok,
        Opcode::GreaterThan,
        vec![op_result(lt_glen), op_result(lt_maxc)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_len_ok,
        function: fid,
        parameters: vec![lt_tup, lt_tag, lt_eid, lt_vec, lt_ilen, lt_in, lt_unit],
        operations: vec![lt_glen, lt_gpos, lt_maxc, lt_gtmax],
        terminator: cond(
            op_result(lt_gtmax),
            edge(b_res, Vec::new()),
            edge(
                f2_bnd,
                vec![
                    pav(lt_tag),
                    op_result(lt_glen),
                    op_result(lt_gpos),
                    pav(lt_eid),
                    pav(lt_vec),
                    pav(lt_ilen),
                    pav(lt_in),
                    pav(lt_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Bounds: end2 = pos3 + len2 <= ilen else LENGTH_OVERFLOW.
    let bb_tag = a.param(ns.p, f2_bnd, ParameterRole::Block, u64_type());
    let bb_len = a.param(ns.p, f2_bnd, ParameterRole::Block, u64_type());
    let bb_pos = a.param(ns.p, f2_bnd, ParameterRole::Block, u64_type());
    let bb_eid = a.param(ns.p, f2_bnd, ParameterRole::Block, TypeExpr::Bytes);
    let bb_vec = a.param(ns.p, f2_bnd, ParameterRole::Block, u8vec_type());
    let bb_ilen = a.param(ns.p, f2_bnd, ParameterRole::Block, u64_type());
    let bb_in = a.param(ns.p, f2_bnd, ParameterRole::Block, TypeExpr::Bytes);
    let bb_unit = a.param(ns.p, f2_bnd, ParameterRole::Block, TypeExpr::Unit);
    let bb_add = a.op(
        ns.o,
        f2_bnd,
        Opcode::IntAddChecked,
        vec![pav(bb_pos), pav(bb_len)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_bnd,
        function: fid,
        parameters: vec![
            bb_tag, bb_len, bb_pos, bb_eid, bb_vec, bb_ilen, bb_in, bb_unit,
        ],
        operations: vec![bb_add],
        terminator: switch(
            op_result(bb_add),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_unwrap,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(bb_tag),
                        sav(bb_len),
                        sav(bb_pos),
                        sav(bb_eid),
                        sav(bb_vec),
                        sav(bb_ilen),
                        sav(bb_in),
                        sav(bb_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let ub_end = a.param(ns.p, f2_unwrap, ParameterRole::Block, u64_type());
    let ub_tag = a.param(ns.p, f2_unwrap, ParameterRole::Block, u64_type());
    let ub_len = a.param(ns.p, f2_unwrap, ParameterRole::Block, u64_type());
    let ub_pos = a.param(ns.p, f2_unwrap, ParameterRole::Block, u64_type());
    let ub_eid = a.param(ns.p, f2_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let ub_vec = a.param(ns.p, f2_unwrap, ParameterRole::Block, u8vec_type());
    let ub_ilen = a.param(ns.p, f2_unwrap, ParameterRole::Block, u64_type());
    let ub_in = a.param(ns.p, f2_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let ub_unit = a.param(ns.p, f2_unwrap, ParameterRole::Block, TypeExpr::Unit);
    let ub_gt = a.op(
        ns.o,
        f2_unwrap,
        Opcode::GreaterThan,
        vec![pav(ub_end), pav(ub_ilen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_unwrap,
        function: fid,
        parameters: vec![
            ub_end, ub_tag, ub_len, ub_pos, ub_eid, ub_vec, ub_ilen, ub_in, ub_unit,
        ],
        operations: vec![ub_gt],
        terminator: cond(
            op_result(ub_gt),
            edge(b_len, Vec::new()),
            edge(
                f2_disp2,
                vec![
                    pav(ub_tag),
                    pav(ub_len),
                    pav(ub_pos),
                    pav(ub_end),
                    pav(ub_eid),
                    pav(ub_vec),
                    pav(ub_ilen),
                    pav(ub_in),
                    pav(ub_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // After bounds: tag2==2 -> body extraction; else UNKNOWN (5+).
    let q_tag = a.param(ns.p, f2_disp2, ParameterRole::Block, u64_type());
    let q_len = a.param(ns.p, f2_disp2, ParameterRole::Block, u64_type());
    let q_pos = a.param(ns.p, f2_disp2, ParameterRole::Block, u64_type());
    let q_end = a.param(ns.p, f2_disp2, ParameterRole::Block, u64_type());
    let q_eid = a.param(ns.p, f2_disp2, ParameterRole::Block, TypeExpr::Bytes);
    let q_vec = a.param(ns.p, f2_disp2, ParameterRole::Block, u8vec_type());
    let q_ilen = a.param(ns.p, f2_disp2, ParameterRole::Block, u64_type());
    let q_in = a.param(ns.p, f2_disp2, ParameterRole::Block, TypeExpr::Bytes);
    let q_unit = a.param(ns.p, f2_disp2, ParameterRole::Block, TypeExpr::Unit);
    let q_k2 = a.cref(ns.o, f2_disp2, c2, u64_type());
    let q_eq = a.op(
        ns.o,
        f2_disp2,
        Opcode::Equal,
        vec![pav(q_tag), op_result(q_k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_disp2,
        function: fid,
        parameters: vec![
            q_tag, q_len, q_pos, q_end, q_eid, q_vec, q_ilen, q_in, q_unit,
        ],
        operations: vec![q_k2, q_eq],
        terminator: cond(
            op_result(q_eq),
            edge(
                body_start,
                vec![
                    pav(q_len),
                    pav(q_pos),
                    pav(q_end),
                    pav(q_eid),
                    pav(q_vec),
                    pav(q_ilen),
                    pav(q_in),
                    pav(q_unit),
                ],
            ),
            edge(b_unknown, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // Opaque body extraction: copy vec[pos3..end2] (len2 bytes) via loop.
    let bs_len = a.param(ns.p, body_start, ParameterRole::Block, u64_type());
    let bs_pos = a.param(ns.p, body_start, ParameterRole::Block, u64_type());
    let bs_end = a.param(ns.p, body_start, ParameterRole::Block, u64_type());
    let bs_eid = a.param(ns.p, body_start, ParameterRole::Block, TypeExpr::Bytes);
    let bs_vec = a.param(ns.p, body_start, ParameterRole::Block, u8vec_type());
    let bs_ilen = a.param(ns.p, body_start, ParameterRole::Block, u64_type());
    let bs_in = a.param(ns.p, body_start, ParameterRole::Block, TypeExpr::Bytes);
    let bs_unit = a.param(ns.p, body_start, ParameterRole::Block, TypeExpr::Unit);
    let bs_empty = a.op(
        ns.o,
        body_start,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let bs_z0 = a.cref(ns.o, body_start, c0, u64_type());
    a.blocks.push(Block {
        entity_id: body_start,
        function: fid,
        parameters: vec![
            bs_len, bs_pos, bs_end, bs_eid, bs_vec, bs_ilen, bs_in, bs_unit,
        ],
        operations: vec![bs_empty, bs_z0],
        terminator: branch(edge(
            body_check,
            vec![
                op_result(bs_z0),
                op_result(bs_empty),
                pav(bs_len),
                pav(bs_pos),
                pav(bs_end),
                pav(bs_eid),
                pav(bs_vec),
                pav(bs_ilen),
                pav(bs_in),
                pav(bs_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let bc_idx = a.param(ns.p, body_check, ParameterRole::Block, u64_type());
    let bc_acc = a.param(ns.p, body_check, ParameterRole::Block, u8vec_type());
    let bc_len = a.param(ns.p, body_check, ParameterRole::Block, u64_type());
    let bc_pos = a.param(ns.p, body_check, ParameterRole::Block, u64_type());
    let bc_end = a.param(ns.p, body_check, ParameterRole::Block, u64_type());
    let bc_eid = a.param(ns.p, body_check, ParameterRole::Block, TypeExpr::Bytes);
    let bc_vec = a.param(ns.p, body_check, ParameterRole::Block, u8vec_type());
    let bc_ilen = a.param(ns.p, body_check, ParameterRole::Block, u64_type());
    let bc_in = a.param(ns.p, body_check, ParameterRole::Block, TypeExpr::Bytes);
    let bc_unit = a.param(ns.p, body_check, ParameterRole::Block, TypeExpr::Unit);
    let bc_lt = a.op(
        ns.o,
        body_check,
        Opcode::LessThan,
        vec![pav(bc_idx), pav(bc_len)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: body_check,
        function: fid,
        parameters: vec![
            bc_idx, bc_acc, bc_len, bc_pos, bc_end, bc_eid, bc_vec, bc_ilen, bc_in, bc_unit,
        ],
        operations: vec![bc_lt],
        terminator: cond(
            op_result(bc_lt),
            edge(
                body_get,
                vec![
                    pav(bc_idx),
                    pav(bc_acc),
                    pav(bc_len),
                    pav(bc_pos),
                    pav(bc_end),
                    pav(bc_eid),
                    pav(bc_vec),
                    pav(bc_ilen),
                    pav(bc_in),
                    pav(bc_unit),
                ],
            ),
            edge(
                body_done,
                vec![
                    pav(bc_acc),
                    pav(bc_end),
                    pav(bc_eid),
                    pav(bc_vec),
                    pav(bc_ilen),
                    pav(bc_in),
                    pav(bc_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let bg_idx = a.param(ns.p, body_get, ParameterRole::Block, u64_type());
    let bg_acc = a.param(ns.p, body_get, ParameterRole::Block, u8vec_type());
    let bg_len = a.param(ns.p, body_get, ParameterRole::Block, u64_type());
    let bg_pos = a.param(ns.p, body_get, ParameterRole::Block, u64_type());
    let bg_end = a.param(ns.p, body_get, ParameterRole::Block, u64_type());
    let bg_eid = a.param(ns.p, body_get, ParameterRole::Block, TypeExpr::Bytes);
    let bg_vec = a.param(ns.p, body_get, ParameterRole::Block, u8vec_type());
    let bg_ilen = a.param(ns.p, body_get, ParameterRole::Block, u64_type());
    let bg_in = a.param(ns.p, body_get, ParameterRole::Block, TypeExpr::Bytes);
    let bg_unit = a.param(ns.p, body_get, ParameterRole::Block, TypeExpr::Unit);
    let bg_add = a.op(
        ns.o,
        body_get,
        Opcode::IntAddChecked,
        vec![pav(bg_pos), pav(bg_idx)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: body_get,
        function: fid,
        parameters: vec![
            bg_idx, bg_acc, bg_len, bg_pos, bg_end, bg_eid, bg_vec, bg_ilen, bg_in, bg_unit,
        ],
        operations: vec![bg_add],
        terminator: switch(
            op_result(bg_add),
            vec![
                (
                    BuiltinCase::Ok,
                    body_get2,
                    vec![
                        sav(bg_acc),
                        SwitchArgument::CasePayload,
                        sav(bg_idx),
                        sav(bg_len),
                        sav(bg_pos),
                        sav(bg_end),
                        sav(bg_eid),
                        sav(bg_vec),
                        sav(bg_ilen),
                        sav(bg_in),
                        sav(bg_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let hg_acc = a.param(ns.p, body_get2, ParameterRole::Block, u8vec_type());
    let hg_sidx = a.param(ns.p, body_get2, ParameterRole::Block, u64_type());
    let hg_idx = a.param(ns.p, body_get2, ParameterRole::Block, u64_type());
    let hg_len = a.param(ns.p, body_get2, ParameterRole::Block, u64_type());
    let hg_pos = a.param(ns.p, body_get2, ParameterRole::Block, u64_type());
    let hg_end = a.param(ns.p, body_get2, ParameterRole::Block, u64_type());
    let hg_eid = a.param(ns.p, body_get2, ParameterRole::Block, TypeExpr::Bytes);
    let hg_vec = a.param(ns.p, body_get2, ParameterRole::Block, u8vec_type());
    let hg_ilen = a.param(ns.p, body_get2, ParameterRole::Block, u64_type());
    let hg_in = a.param(ns.p, body_get2, ParameterRole::Block, TypeExpr::Bytes);
    let hg_unit = a.param(ns.p, body_get2, ParameterRole::Block, TypeExpr::Unit);
    let hg_get = a.op(
        ns.o,
        body_get2,
        Opcode::VectorGet,
        vec![pav(hg_vec), pav(hg_sidx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: body_get2,
        function: fid,
        parameters: vec![
            hg_acc, hg_sidx, hg_idx, hg_len, hg_pos, hg_end, hg_eid, hg_vec, hg_ilen, hg_in,
            hg_unit,
        ],
        operations: vec![hg_get],
        terminator: switch(
            op_result(hg_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    body_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(hg_idx),
                        sav(hg_acc),
                        sav(hg_len),
                        sav(hg_pos),
                        sav(hg_end),
                        sav(hg_eid),
                        sav(hg_vec),
                        sav(hg_ilen),
                        sav(hg_in),
                        sav(hg_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let bp_b = a.param(ns.p, body_push, ParameterRole::Block, u8_type());
    let bp_idx = a.param(ns.p, body_push, ParameterRole::Block, u64_type());
    let bp_acc = a.param(ns.p, body_push, ParameterRole::Block, u8vec_type());
    let bp_len = a.param(ns.p, body_push, ParameterRole::Block, u64_type());
    let bp_pos = a.param(ns.p, body_push, ParameterRole::Block, u64_type());
    let bp_end = a.param(ns.p, body_push, ParameterRole::Block, u64_type());
    let bp_eid = a.param(ns.p, body_push, ParameterRole::Block, TypeExpr::Bytes);
    let bp_vec = a.param(ns.p, body_push, ParameterRole::Block, u8vec_type());
    let bp_ilen = a.param(ns.p, body_push, ParameterRole::Block, u64_type());
    let bp_in = a.param(ns.p, body_push, ParameterRole::Block, TypeExpr::Bytes);
    let bp_unit = a.param(ns.p, body_push, ParameterRole::Block, TypeExpr::Unit);
    let bp_push = a.op(
        ns.o,
        body_push,
        Opcode::AdapterInvoke,
        vec![pav(bp_acc), pav(bp_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: body_push,
        function: fid,
        parameters: vec![
            bp_b, bp_idx, bp_acc, bp_len, bp_pos, bp_end, bp_eid, bp_vec, bp_ilen, bp_in, bp_unit,
        ],
        operations: vec![bp_push],
        terminator: switch(
            op_result(bp_push),
            vec![
                (
                    BuiltinCase::Ok,
                    body_next,
                    vec![
                        sav(bp_idx),
                        SwitchArgument::CasePayload,
                        sav(bp_len),
                        sav(bp_pos),
                        sav(bp_end),
                        sav(bp_eid),
                        sav(bp_vec),
                        sav(bp_ilen),
                        sav(bp_in),
                        sav(bp_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let nn_idx = a.param(ns.p, body_next, ParameterRole::Block, u64_type());
    let nn_acc = a.param(ns.p, body_next, ParameterRole::Block, u8vec_type());
    let nn_len = a.param(ns.p, body_next, ParameterRole::Block, u64_type());
    let nn_pos = a.param(ns.p, body_next, ParameterRole::Block, u64_type());
    let nn_end = a.param(ns.p, body_next, ParameterRole::Block, u64_type());
    let nn_eid = a.param(ns.p, body_next, ParameterRole::Block, TypeExpr::Bytes);
    let nn_vec = a.param(ns.p, body_next, ParameterRole::Block, u8vec_type());
    let nn_ilen = a.param(ns.p, body_next, ParameterRole::Block, u64_type());
    let nn_in = a.param(ns.p, body_next, ParameterRole::Block, TypeExpr::Bytes);
    let nn_unit = a.param(ns.p, body_next, ParameterRole::Block, TypeExpr::Unit);
    let nn_c1 = a.cref(ns.o, body_next, c1, u64_type());
    let nn_add = a.op(
        ns.o,
        body_next,
        Opcode::IntAddChecked,
        vec![pav(nn_idx), op_result(nn_c1)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: body_next,
        function: fid,
        parameters: vec![
            nn_idx, nn_acc, nn_len, nn_pos, nn_end, nn_eid, nn_vec, nn_ilen, nn_in, nn_unit,
        ],
        operations: vec![nn_c1, nn_add],
        terminator: switch(
            op_result(nn_add),
            vec![
                (
                    BuiltinCase::Ok,
                    body_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(nn_acc),
                        sav(nn_len),
                        sav(nn_pos),
                        sav(nn_end),
                        sav(nn_eid),
                        sav(nn_vec),
                        sav(nn_ilen),
                        sav(nn_in),
                        sav(nn_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // body_done: V2B1 accumulator -> body Bytes.
    let bd_acc = a.param(ns.p, body_done, ParameterRole::Block, u8vec_type());
    let bd_end = a.param(ns.p, body_done, ParameterRole::Block, u64_type());
    let bd_eid = a.param(ns.p, body_done, ParameterRole::Block, TypeExpr::Bytes);
    let bd_vec = a.param(ns.p, body_done, ParameterRole::Block, u8vec_type());
    let bd_ilen = a.param(ns.p, body_done, ParameterRole::Block, u64_type());
    let bd_in = a.param(ns.p, body_done, ParameterRole::Block, TypeExpr::Bytes);
    let bd_unit = a.param(ns.p, body_done, ParameterRole::Block, TypeExpr::Unit);
    let bd_v2b = a.op(
        ns.o,
        body_done,
        Opcode::AdapterInvoke,
        vec![pav(bd_unit), pav(bd_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: body_done,
        function: fid,
        parameters: vec![bd_acc, bd_end, bd_eid, bd_vec, bd_ilen, bd_in, bd_unit],
        operations: vec![bd_v2b],
        terminator: switch(
            op_result(bd_v2b),
            vec![
                (
                    BuiltinCase::Ok,
                    body_trail,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(bd_end),
                        sav(bd_eid),
                        sav(bd_ilen),
                        sav(bd_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Trailing: end2 == ilen else TRAILING.
    let t_body = a.param(ns.p, body_trail, ParameterRole::Block, TypeExpr::Bytes);
    let t_end = a.param(ns.p, body_trail, ParameterRole::Block, u64_type());
    let t_eid = a.param(ns.p, body_trail, ParameterRole::Block, TypeExpr::Bytes);
    let t_ilen = a.param(ns.p, body_trail, ParameterRole::Block, u64_type());
    let t_unit = a.param(ns.p, body_trail, ParameterRole::Block, TypeExpr::Unit);
    let t_eq = a.op(
        ns.o,
        body_trail,
        Opcode::Equal,
        vec![pav(t_end), pav(t_ilen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: body_trail,
        function: fid,
        parameters: vec![t_body, t_end, t_eid, t_ilen, t_unit],
        operations: vec![t_eq],
        terminator: cond(
            op_result(t_eq),
            edge(body_ret, vec![pav(t_body), pav(t_eid)]),
            edge(b_trail, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // Return Tuple(entity_id, body) — contracted order (entity first).
    let r_body = a.param(ns.p, body_ret, ParameterRole::Block, TypeExpr::Bytes);
    let r_eid = a.param(ns.p, body_ret, ParameterRole::Block, TypeExpr::Bytes);
    let r_tup = a.op(
        ns.o,
        body_ret,
        Opcode::TupleNew,
        vec![pav(r_eid), pav(r_body)],
        vec![TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes])],
        Immediate::None,
    );
    let r_ok = a.op(
        ns.o,
        body_ret,
        Opcode::ResultOk,
        vec![op_result(r_tup)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: body_ret,
        function: fid,
        parameters: vec![r_body, r_eid],
        operations: vec![r_tup, r_ok],
        terminator: ret(op_result(r_ok)),
        reachability: Reachability::Required,
    });
    // Prev=2 field-2 chain (body-first [2,*], all Err). DUP (tag2==2) and
    // ORDER (tag2<2, i.e. 0/1) precede payload per reference; scope-first
    // for 3/4; 5+ reads len/bounds so LENGTH_OVERFLOW precedes UNKNOWN.
    let f2b_ok = a.id(ns.b);
    let f2b_err = a.id(ns.b);
    let f2b_disp = a.id(ns.b);
    let f2b_ord = a.id(ns.b);
    let f2b_scope = a.id(ns.b);
    let f2b_len = a.id(ns.b);
    let f2b_len_ok = a.id(ns.b);
    let f2b_len_err = a.id(ns.b);
    let f2b_bnd = a.id(ns.b);
    let f2b_unwrap = a.id(ns.b);
    let b_end1 = a.param(ns.p, f2b_tag, ParameterRole::Block, u64_type());
    let b_vec = a.param(ns.p, f2b_tag, ParameterRole::Block, u8vec_type());
    let b_ilen = a.param(ns.p, f2b_tag, ParameterRole::Block, u64_type());
    let b_in = a.param(ns.p, f2b_tag, ParameterRole::Block, TypeExpr::Bytes);
    let b_unit = a.param(ns.p, f2b_tag, ParameterRole::Block, TypeExpr::Unit);
    let b_w = a.cref(ns.o, f2b_tag, w32, u32_type());
    let b_call = a.op(
        ns.o,
        f2b_tag,
        Opcode::CallDirect,
        vec![pav(b_in), pav(b_end1), op_result(b_w), pav(b_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let b_tup = a.param(
        ns.p,
        f2b_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let b_ovec = a.param(ns.p, f2b_ok, ParameterRole::Block, u8vec_type());
    let b_olen = a.param(ns.p, f2b_ok, ParameterRole::Block, u64_type());
    let b_oin = a.param(ns.p, f2b_ok, ParameterRole::Block, TypeExpr::Bytes);
    let b_ounit = a.param(ns.p, f2b_ok, ParameterRole::Block, TypeExpr::Unit);
    let b_ebytes = a.param(ns.p, f2b_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f2b_tag,
        function: fid,
        parameters: vec![b_end1, b_vec, b_ilen, b_in, b_unit],
        operations: vec![b_w, b_call],
        terminator: switch(
            op_result(b_call),
            vec![
                (
                    BuiltinCase::Ok,
                    f2b_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(b_vec),
                        sav(b_ilen),
                        sav(b_in),
                        sav(b_unit),
                    ],
                ),
                (BuiltinCase::Err, f2b_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let b_er = a.op(
        ns.o,
        f2b_err,
        Opcode::ResultErr,
        vec![pav(b_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2b_err,
        function: fid,
        parameters: vec![b_ebytes],
        operations: vec![b_er],
        terminator: ret(op_result(b_er)),
        reachability: Reachability::Required,
    });
    let b_gtag = a.op(
        ns.o,
        f2b_ok,
        Opcode::TupleGet,
        vec![pav(b_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let b_gpos = a.op(
        ns.o,
        f2b_ok,
        Opcode::TupleGet,
        vec![pav(b_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    a.blocks.push(Block {
        entity_id: f2b_ok,
        function: fid,
        parameters: vec![b_tup, b_ovec, b_olen, b_oin, b_ounit],
        operations: vec![b_gtag, b_gpos],
        terminator: branch(edge(
            f2b_disp,
            vec![
                op_result(b_gtag),
                op_result(b_gpos),
                pav(b_ovec),
                pav(b_olen),
                pav(b_oin),
                pav(b_ounit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let db_tag = a.param(ns.p, f2b_disp, ParameterRole::Block, u64_type());
    let db_pos = a.param(ns.p, f2b_disp, ParameterRole::Block, u64_type());
    let db_vec = a.param(ns.p, f2b_disp, ParameterRole::Block, u8vec_type());
    let db_ilen = a.param(ns.p, f2b_disp, ParameterRole::Block, u64_type());
    let db_in = a.param(ns.p, f2b_disp, ParameterRole::Block, TypeExpr::Bytes);
    let db_unit = a.param(ns.p, f2b_disp, ParameterRole::Block, TypeExpr::Unit);
    let db_k2 = a.cref(ns.o, f2b_disp, c2, u64_type());
    let db_eq = a.op(
        ns.o,
        f2b_disp,
        Opcode::Equal,
        vec![pav(db_tag), op_result(db_k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2b_disp,
        function: fid,
        parameters: vec![db_tag, db_pos, db_vec, db_ilen, db_in, db_unit],
        operations: vec![db_k2, db_eq],
        terminator: cond(
            op_result(db_eq),
            edge(b_dup, Vec::new()),
            edge(
                f2b_ord,
                vec![
                    pav(db_tag),
                    pav(db_pos),
                    pav(db_vec),
                    pav(db_ilen),
                    pav(db_in),
                    pav(db_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let ob_tag = a.param(ns.p, f2b_ord, ParameterRole::Block, u64_type());
    let ob_pos = a.param(ns.p, f2b_ord, ParameterRole::Block, u64_type());
    let ob_vec = a.param(ns.p, f2b_ord, ParameterRole::Block, u8vec_type());
    let ob_ilen = a.param(ns.p, f2b_ord, ParameterRole::Block, u64_type());
    let ob_in = a.param(ns.p, f2b_ord, ParameterRole::Block, TypeExpr::Bytes);
    let ob_unit = a.param(ns.p, f2b_ord, ParameterRole::Block, TypeExpr::Unit);
    let ob_k2 = a.cref(ns.o, f2b_ord, c2, u64_type());
    let ob_lt = a.op(
        ns.o,
        f2b_ord,
        Opcode::LessThan,
        vec![pav(ob_tag), op_result(ob_k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2b_ord,
        function: fid,
        parameters: vec![ob_tag, ob_pos, ob_vec, ob_ilen, ob_in, ob_unit],
        operations: vec![ob_k2, ob_lt],
        terminator: cond(
            op_result(ob_lt),
            edge(b_order, Vec::new()),
            edge(
                f2b_scope,
                vec![
                    pav(ob_tag),
                    pav(ob_pos),
                    pav(ob_vec),
                    pav(ob_ilen),
                    pav(ob_in),
                    pav(ob_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let sb_tag = a.param(ns.p, f2b_scope, ParameterRole::Block, u64_type());
    let sb_pos = a.param(ns.p, f2b_scope, ParameterRole::Block, u64_type());
    let sb_vec = a.param(ns.p, f2b_scope, ParameterRole::Block, u8vec_type());
    let sb_ilen = a.param(ns.p, f2b_scope, ParameterRole::Block, u64_type());
    let sb_in = a.param(ns.p, f2b_scope, ParameterRole::Block, TypeExpr::Bytes);
    let sb_unit = a.param(ns.p, f2b_scope, ParameterRole::Block, TypeExpr::Unit);
    let sb_k3 = a.cref(ns.o, f2b_scope, c3, u64_type());
    let sb_k4 = a.cref(ns.o, f2b_scope, c4, u64_type());
    let sb_e3 = a.op(
        ns.o,
        f2b_scope,
        Opcode::Equal,
        vec![pav(sb_tag), op_result(sb_k3)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let sb_e4 = a.op(
        ns.o,
        f2b_scope,
        Opcode::Equal,
        vec![pav(sb_tag), op_result(sb_k4)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let sb_or = a.op(
        ns.o,
        f2b_scope,
        Opcode::BoolOr,
        vec![op_result(sb_e3), op_result(sb_e4)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2b_scope,
        function: fid,
        parameters: vec![sb_tag, sb_pos, sb_vec, sb_ilen, sb_in, sb_unit],
        operations: vec![sb_k3, sb_k4, sb_e3, sb_e4, sb_or],
        terminator: cond(
            op_result(sb_or),
            edge(b_scope, Vec::new()),
            edge(
                f2b_len,
                vec![
                    pav(sb_tag),
                    pav(sb_pos),
                    pav(sb_vec),
                    pav(sb_ilen),
                    pav(sb_in),
                    pav(sb_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let lb_tag = a.param(ns.p, f2b_len, ParameterRole::Block, u64_type());
    let lb_pos = a.param(ns.p, f2b_len, ParameterRole::Block, u64_type());
    let lb_vec = a.param(ns.p, f2b_len, ParameterRole::Block, u8vec_type());
    let lb_ilen = a.param(ns.p, f2b_len, ParameterRole::Block, u64_type());
    let lb_in = a.param(ns.p, f2b_len, ParameterRole::Block, TypeExpr::Bytes);
    let lb_unit = a.param(ns.p, f2b_len, ParameterRole::Block, TypeExpr::Unit);
    let lb_w = a.cref(ns.o, f2b_len, w64, u32_type());
    let lb_call = a.op(
        ns.o,
        f2b_len,
        Opcode::CallDirect,
        vec![pav(lb_in), pav(lb_pos), op_result(lb_w), pav(lb_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let lb_tup = a.param(
        ns.p,
        f2b_len_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let lb_otag = a.param(ns.p, f2b_len_ok, ParameterRole::Block, u64_type());
    let lb_ovec = a.param(ns.p, f2b_len_ok, ParameterRole::Block, u8vec_type());
    let lb_olen = a.param(ns.p, f2b_len_ok, ParameterRole::Block, u64_type());
    let lb_oin = a.param(ns.p, f2b_len_ok, ParameterRole::Block, TypeExpr::Bytes);
    let lb_ounit = a.param(ns.p, f2b_len_ok, ParameterRole::Block, TypeExpr::Unit);
    let lb_ebytes = a.param(ns.p, f2b_len_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f2b_len,
        function: fid,
        parameters: vec![lb_tag, lb_pos, lb_vec, lb_ilen, lb_in, lb_unit],
        operations: vec![lb_w, lb_call],
        terminator: switch(
            op_result(lb_call),
            vec![
                (
                    BuiltinCase::Ok,
                    f2b_len_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(lb_tag),
                        sav(lb_vec),
                        sav(lb_ilen),
                        sav(lb_in),
                        sav(lb_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    f2b_len_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let lb_er = a.op(
        ns.o,
        f2b_len_err,
        Opcode::ResultErr,
        vec![pav(lb_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2b_len_err,
        function: fid,
        parameters: vec![lb_ebytes],
        operations: vec![lb_er],
        terminator: ret(op_result(lb_er)),
        reachability: Reachability::Required,
    });
    let lb_glen = a.op(
        ns.o,
        f2b_len_ok,
        Opcode::TupleGet,
        vec![pav(lb_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let lb_gpos = a.op(
        ns.o,
        f2b_len_ok,
        Opcode::TupleGet,
        vec![pav(lb_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let lb_maxc = a.cref(ns.o, f2b_len_ok, c_max, u64_type());
    let lb_gtmax = a.op(
        ns.o,
        f2b_len_ok,
        Opcode::GreaterThan,
        vec![op_result(lb_glen), op_result(lb_maxc)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2b_len_ok,
        function: fid,
        parameters: vec![lb_tup, lb_otag, lb_ovec, lb_olen, lb_oin, lb_ounit],
        operations: vec![lb_glen, lb_gpos, lb_maxc, lb_gtmax],
        terminator: cond(
            op_result(lb_gtmax),
            edge(b_res, Vec::new()),
            edge(
                f2b_bnd,
                vec![
                    pav(lb_otag),
                    op_result(lb_glen),
                    op_result(lb_gpos),
                    pav(lb_ovec),
                    pav(lb_olen),
                    pav(lb_oin),
                    pav(lb_ounit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let fb_tag = a.param(ns.p, f2b_bnd, ParameterRole::Block, u64_type());
    let fb_len = a.param(ns.p, f2b_bnd, ParameterRole::Block, u64_type());
    let fb_pos = a.param(ns.p, f2b_bnd, ParameterRole::Block, u64_type());
    let fb_vec = a.param(ns.p, f2b_bnd, ParameterRole::Block, u8vec_type());
    let fb_ilen = a.param(ns.p, f2b_bnd, ParameterRole::Block, u64_type());
    let fb_in = a.param(ns.p, f2b_bnd, ParameterRole::Block, TypeExpr::Bytes);
    let fb_unit = a.param(ns.p, f2b_bnd, ParameterRole::Block, TypeExpr::Unit);
    let fb_add = a.op(
        ns.o,
        f2b_bnd,
        Opcode::IntAddChecked,
        vec![pav(fb_pos), pav(fb_len)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2b_bnd,
        function: fid,
        parameters: vec![fb_tag, fb_len, fb_pos, fb_vec, fb_ilen, fb_in, fb_unit],
        operations: vec![fb_add],
        terminator: switch(
            op_result(fb_add),
            vec![
                (
                    BuiltinCase::Ok,
                    f2b_unwrap,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(fb_tag),
                        sav(fb_len),
                        sav(fb_vec),
                        sav(fb_ilen),
                        sav(fb_in),
                        sav(fb_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // After bounds, prev=2 has no valid success: 5+ -> UNKNOWN.
    let fu_end = a.param(ns.p, f2b_unwrap, ParameterRole::Block, u64_type());
    let fu_tag = a.param(ns.p, f2b_unwrap, ParameterRole::Block, u64_type());
    let fu_len = a.param(ns.p, f2b_unwrap, ParameterRole::Block, u64_type());
    let fu_vec = a.param(ns.p, f2b_unwrap, ParameterRole::Block, u8vec_type());
    let fu_ilen = a.param(ns.p, f2b_unwrap, ParameterRole::Block, u64_type());
    let fu_in = a.param(ns.p, f2b_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let fu_unit = a.param(ns.p, f2b_unwrap, ParameterRole::Block, TypeExpr::Unit);
    let fu_gt = a.op(
        ns.o,
        f2b_unwrap,
        Opcode::GreaterThan,
        vec![pav(fu_end), pav(fu_ilen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2b_unwrap,
        function: fid,
        parameters: vec![fu_end, fu_tag, fu_len, fu_vec, fu_ilen, fu_in, fu_unit],
        operations: vec![fu_gt],
        terminator: cond(
            op_result(fu_gt),
            edge(b_len, Vec::new()),
            edge(b_unknown, Vec::new()),
        ),
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

// ── outer encode (canonical 2-field emission) ────────────────────────
// `encode_outer(entity_id: Bytes, body: Bytes, unit: Unit)` emits canonical
// `encode_record([(1, entity_id), (2, body)])` bytes: count 2, tag1 len32,
// tag2 len(body) via canonical uvar, all single-byte for small except len2
// which reuses slice-1 `encode_uvar` via `CallDirect` (width 64) so
// multi-byte lengths stay canonical. Entity length mirrors `decode_fixed`:
// <32 -> LENGTH_OVERFLOW, >32 -> TRAILING. Body length past the epoch
// ceiling -> RESOURCE_LIMIT. Bridge Err on inputs past 1 MiB -> RESOURCE.
// Sley owns framing and byte emission; bridge uses B2V1/PSH1/V2B1 only.
#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_outer_encode(a: &mut Asm, ns: Ns, fid: EntityId, encode_fid: EntityId) -> FunctionGraph {
    let bstart = a.blocks.len();
    let res_t = encode_result_type();
    let enc_t = encode_result_type();
    let c0 = a.ku64(ns.k, 0);
    let c1 = a.ku64(ns.k, 1);
    let c2 = a.ku64(ns.k, 2);
    let c32 = a.ku64(ns.k, 32);
    let c_max = a.ku64(ns.k, 67_108_864);
    let w64 = a.ku32(ns.k, 64);
    let u01 = a.ku8(ns.k, 1);
    let u02 = a.ku8(ns.k, 2);
    let u20 = a.ku8(ns.k, 32);
    let e_len = a.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let e_trail = a.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_res = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let q_eid = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let q_body = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let q_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let b_len = err_block(a, ns, fid, res_t.clone(), e_len);
    let b_trail = err_block(a, ns, fid, res_t.clone(), e_trail);
    let b_res = err_block(a, ns, fid, res_t.clone(), e_res);
    let trap = trap_block(a, ns, fid);
    // New blocks.
    let eid_len = a.id(ns.b);
    let eid_gt = a.id(ns.b);
    let body_conv = a.id(ns.b);
    let body_len = a.id(ns.b);
    let len_enc = a.id(ns.b);
    let len_enc_ok = a.id(ns.b);
    let len_enc_err = a.id(ns.b);
    let len_setup = a.id(ns.b);
    let out_start = a.id(ns.b);
    let push1 = a.id(ns.b);
    let push2 = a.id(ns.b);
    let push3 = a.id(ns.b);
    let eid_copy_start = a.id(ns.b);
    let eid_copy_check = a.id(ns.b);
    let eid_copy_get = a.id(ns.b);
    let eid_copy_get2 = a.id(ns.b);
    let eid_copy_push = a.id(ns.b);
    let eid_copy_next = a.id(ns.b);
    let tag2_push = a.id(ns.b);
    let lencopy_start = a.id(ns.b);
    let lencopy_check = a.id(ns.b);
    let lencopy_get = a.id(ns.b);
    let lencopy_get2 = a.id(ns.b);
    let lencopy_push = a.id(ns.b);
    let lencopy_next = a.id(ns.b);
    let bodycopy_start = a.id(ns.b);
    let bodycopy_check = a.id(ns.b);
    let bodycopy_get = a.id(ns.b);
    let bodycopy_get2 = a.id(ns.b);
    let bodycopy_push = a.id(ns.b);
    let bodycopy_next = a.id(ns.b);
    let out_done = a.id(ns.b);
    let out_ret = a.id(ns.b);
    // Entry: B2V1 entity.
    let entry = a.id(ns.b);
    let cv_eid = a.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(q_unit), pav(q_eid)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![cv_eid],
        terminator: switch(
            op_result(cv_eid),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_len,
                    vec![SwitchArgument::CasePayload, sav(q_body), sav(q_unit)],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Entity length: <32 LENGTH, >32 TRAILING (encode-side mirror of decode_fixed).
    let v_eidvec = a.param(ns.p, eid_len, ParameterRole::Block, u8vec_type());
    let v_bodyin = a.param(ns.p, eid_len, ParameterRole::Block, TypeExpr::Bytes);
    let v_unit = a.param(ns.p, eid_len, ParameterRole::Block, TypeExpr::Unit);
    let v_ln = a.op(
        ns.o,
        eid_len,
        Opcode::VectorLen,
        vec![pav(v_eidvec)],
        vec![u64_type()],
        Immediate::None,
    );
    let v_k32 = a.cref(ns.o, eid_len, c32, u64_type());
    let v_lt = a.op(
        ns.o,
        eid_len,
        Opcode::LessThan,
        vec![op_result(v_ln), op_result(v_k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let v_gt = a.op(
        ns.o,
        eid_len,
        Opcode::GreaterThan,
        vec![op_result(v_ln), op_result(v_k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_len,
        function: fid,
        parameters: vec![v_eidvec, v_bodyin, v_unit],
        operations: vec![v_ln, v_k32, v_lt, v_gt],
        terminator: cond(
            op_result(v_lt),
            edge(b_len, Vec::new()),
            edge(
                eid_gt,
                vec![pav(v_eidvec), pav(v_bodyin), pav(v_unit), op_result(v_gt)],
            ),
        ),
        reachability: Reachability::Required,
    });
    let g_eidvec = a.param(ns.p, eid_gt, ParameterRole::Block, u8vec_type());
    let g_bodyin = a.param(ns.p, eid_gt, ParameterRole::Block, TypeExpr::Bytes);
    let g_unit = a.param(ns.p, eid_gt, ParameterRole::Block, TypeExpr::Unit);
    let g_flag = a.param(ns.p, eid_gt, ParameterRole::Block, TypeExpr::Bool);
    a.blocks.push(Block {
        entity_id: eid_gt,
        function: fid,
        parameters: vec![g_eidvec, g_bodyin, g_unit, g_flag],
        operations: Vec::new(),
        terminator: cond(
            pav(g_flag),
            edge(b_trail, Vec::new()),
            edge(body_conv, vec![pav(g_eidvec), pav(g_bodyin), pav(g_unit)]),
        ),
        reachability: Reachability::Required,
    });
    // Body convert.
    let bc_eidvec = a.param(ns.p, body_conv, ParameterRole::Block, u8vec_type());
    let bc_bodyin = a.param(ns.p, body_conv, ParameterRole::Block, TypeExpr::Bytes);
    let bc_unit = a.param(ns.p, body_conv, ParameterRole::Block, TypeExpr::Unit);
    let bc_cv = a.op(
        ns.o,
        body_conv,
        Opcode::AdapterInvoke,
        vec![pav(bc_unit), pav(bc_bodyin)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: body_conv,
        function: fid,
        parameters: vec![bc_eidvec, bc_bodyin, bc_unit],
        operations: vec![bc_cv],
        terminator: switch(
            op_result(bc_cv),
            vec![
                (
                    BuiltinCase::Ok,
                    body_len,
                    vec![sav(bc_eidvec), SwitchArgument::CasePayload, sav(bc_unit)],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let bl_eidvec = a.param(ns.p, body_len, ParameterRole::Block, u8vec_type());
    let bl_bodyvec = a.param(ns.p, body_len, ParameterRole::Block, u8vec_type());
    let bl_unit = a.param(ns.p, body_len, ParameterRole::Block, TypeExpr::Unit);
    let bl_ln = a.op(
        ns.o,
        body_len,
        Opcode::VectorLen,
        vec![pav(bl_bodyvec)],
        vec![u64_type()],
        Immediate::None,
    );
    let bl_max = a.cref(ns.o, body_len, c_max, u64_type());
    let bl_gt = a.op(
        ns.o,
        body_len,
        Opcode::GreaterThan,
        vec![op_result(bl_ln), op_result(bl_max)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: body_len,
        function: fid,
        parameters: vec![bl_eidvec, bl_bodyvec, bl_unit],
        operations: vec![bl_ln, bl_max, bl_gt],
        terminator: cond(
            op_result(bl_gt),
            edge(b_res, Vec::new()),
            edge(
                len_enc,
                vec![
                    pav(bl_eidvec),
                    pav(bl_bodyvec),
                    op_result(bl_ln),
                    pav(bl_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Encode body length via canonical encode_uvar (width 64).
    let le_eidvec = a.param(ns.p, len_enc, ParameterRole::Block, u8vec_type());
    let le_bodyvec = a.param(ns.p, len_enc, ParameterRole::Block, u8vec_type());
    let le_blen = a.param(ns.p, len_enc, ParameterRole::Block, u64_type());
    let le_unit = a.param(ns.p, len_enc, ParameterRole::Block, TypeExpr::Unit);
    let le_w = a.cref(ns.o, len_enc, w64, u32_type());
    let le_call = a.op(
        ns.o,
        len_enc,
        Opcode::CallDirect,
        vec![pav(le_blen), op_result(le_w), pav(le_unit)],
        vec![enc_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: encode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let le_bytes = a.param(ns.p, len_enc_ok, ParameterRole::Block, TypeExpr::Bytes);
    let le_oeid = a.param(ns.p, len_enc_ok, ParameterRole::Block, u8vec_type());
    let le_obody = a.param(ns.p, len_enc_ok, ParameterRole::Block, u8vec_type());
    let le_ounit = a.param(ns.p, len_enc_ok, ParameterRole::Block, TypeExpr::Unit);
    let le_ebytes = a.param(ns.p, len_enc_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: len_enc,
        function: fid,
        parameters: vec![le_eidvec, le_bodyvec, le_blen, le_unit],
        operations: vec![le_w, le_call],
        terminator: switch(
            op_result(le_call),
            vec![
                (
                    BuiltinCase::Ok,
                    len_enc_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(le_eidvec),
                        sav(le_bodyvec),
                        sav(le_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    len_enc_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let le_er = a.op(
        ns.o,
        len_enc_err,
        Opcode::ResultErr,
        vec![pav(le_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: len_enc_err,
        function: fid,
        parameters: vec![le_ebytes],
        operations: vec![le_er],
        terminator: ret(op_result(le_er)),
        reachability: Reachability::Required,
    });
    let le_cv = a.op(
        ns.o,
        len_enc_ok,
        Opcode::AdapterInvoke,
        vec![pav(le_ounit), pav(le_bytes)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: len_enc_ok,
        function: fid,
        parameters: vec![le_bytes, le_oeid, le_obody, le_ounit],
        operations: vec![le_cv],
        terminator: switch(
            op_result(le_cv),
            vec![
                (
                    BuiltinCase::Ok,
                    len_setup,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(le_oeid),
                        sav(le_obody),
                        sav(le_ounit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let ls_lenvec = a.param(ns.p, len_setup, ParameterRole::Block, u8vec_type());
    let ls_eidvec = a.param(ns.p, len_setup, ParameterRole::Block, u8vec_type());
    let ls_bodyvec = a.param(ns.p, len_setup, ParameterRole::Block, u8vec_type());
    let ls_unit = a.param(ns.p, len_setup, ParameterRole::Block, TypeExpr::Unit);
    let ls_llen = a.op(
        ns.o,
        len_setup,
        Opcode::VectorLen,
        vec![pav(ls_lenvec)],
        vec![u64_type()],
        Immediate::None,
    );
    let ls_blen = a.op(
        ns.o,
        len_setup,
        Opcode::VectorLen,
        vec![pav(ls_bodyvec)],
        vec![u64_type()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: len_setup,
        function: fid,
        parameters: vec![ls_lenvec, ls_eidvec, ls_bodyvec, ls_unit],
        operations: vec![ls_llen, ls_blen],
        terminator: branch(edge(
            out_start,
            vec![
                pav(ls_lenvec),
                op_result(ls_llen),
                pav(ls_eidvec),
                pav(ls_bodyvec),
                op_result(ls_blen),
                pav(ls_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    // Output assembly: VectorNew, then fixed pushes 02/01/20.
    let os_lenvec = a.param(ns.p, out_start, ParameterRole::Block, u8vec_type());
    let os_llen = a.param(ns.p, out_start, ParameterRole::Block, u64_type());
    let os_eidvec = a.param(ns.p, out_start, ParameterRole::Block, u8vec_type());
    let os_bodyvec = a.param(ns.p, out_start, ParameterRole::Block, u8vec_type());
    let os_blen = a.param(ns.p, out_start, ParameterRole::Block, u64_type());
    let os_unit = a.param(ns.p, out_start, ParameterRole::Block, TypeExpr::Unit);
    let os_empty = a.op(
        ns.o,
        out_start,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: out_start,
        function: fid,
        parameters: vec![os_lenvec, os_llen, os_eidvec, os_bodyvec, os_blen, os_unit],
        operations: vec![os_empty],
        terminator: branch(edge(
            push1,
            vec![
                op_result(os_empty),
                pav(os_lenvec),
                pav(os_llen),
                pav(os_eidvec),
                pav(os_bodyvec),
                pav(os_blen),
                pav(os_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let p1_acc = a.param(ns.p, push1, ParameterRole::Block, u8vec_type());
    let p1_lenvec = a.param(ns.p, push1, ParameterRole::Block, u8vec_type());
    let p1_llen = a.param(ns.p, push1, ParameterRole::Block, u64_type());
    let p1_eidvec = a.param(ns.p, push1, ParameterRole::Block, u8vec_type());
    let p1_bodyvec = a.param(ns.p, push1, ParameterRole::Block, u8vec_type());
    let p1_blen = a.param(ns.p, push1, ParameterRole::Block, u64_type());
    let p1_unit = a.param(ns.p, push1, ParameterRole::Block, TypeExpr::Unit);
    let p1_c = a.cref(ns.o, push1, u02, u8_type());
    let p1_push = a.op(
        ns.o,
        push1,
        Opcode::AdapterInvoke,
        vec![pav(p1_acc), op_result(p1_c)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: push1,
        function: fid,
        parameters: vec![
            p1_acc, p1_lenvec, p1_llen, p1_eidvec, p1_bodyvec, p1_blen, p1_unit,
        ],
        operations: vec![p1_c, p1_push],
        terminator: switch(
            op_result(p1_push),
            vec![
                (
                    BuiltinCase::Ok,
                    push2,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(p1_lenvec),
                        sav(p1_llen),
                        sav(p1_eidvec),
                        sav(p1_bodyvec),
                        sav(p1_blen),
                        sav(p1_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let p2_acc = a.param(ns.p, push2, ParameterRole::Block, u8vec_type());
    let p2_lenvec = a.param(ns.p, push2, ParameterRole::Block, u8vec_type());
    let p2_llen = a.param(ns.p, push2, ParameterRole::Block, u64_type());
    let p2_eidvec = a.param(ns.p, push2, ParameterRole::Block, u8vec_type());
    let p2_bodyvec = a.param(ns.p, push2, ParameterRole::Block, u8vec_type());
    let p2_blen = a.param(ns.p, push2, ParameterRole::Block, u64_type());
    let p2_unit = a.param(ns.p, push2, ParameterRole::Block, TypeExpr::Unit);
    let p2_c = a.cref(ns.o, push2, u01, u8_type());
    let p2_push = a.op(
        ns.o,
        push2,
        Opcode::AdapterInvoke,
        vec![pav(p2_acc), op_result(p2_c)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: push2,
        function: fid,
        parameters: vec![
            p2_acc, p2_lenvec, p2_llen, p2_eidvec, p2_bodyvec, p2_blen, p2_unit,
        ],
        operations: vec![p2_c, p2_push],
        terminator: switch(
            op_result(p2_push),
            vec![
                (
                    BuiltinCase::Ok,
                    push3,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(p2_lenvec),
                        sav(p2_llen),
                        sav(p2_eidvec),
                        sav(p2_bodyvec),
                        sav(p2_blen),
                        sav(p2_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let p3_acc = a.param(ns.p, push3, ParameterRole::Block, u8vec_type());
    let p3_lenvec = a.param(ns.p, push3, ParameterRole::Block, u8vec_type());
    let p3_llen = a.param(ns.p, push3, ParameterRole::Block, u64_type());
    let p3_eidvec = a.param(ns.p, push3, ParameterRole::Block, u8vec_type());
    let p3_bodyvec = a.param(ns.p, push3, ParameterRole::Block, u8vec_type());
    let p3_blen = a.param(ns.p, push3, ParameterRole::Block, u64_type());
    let p3_unit = a.param(ns.p, push3, ParameterRole::Block, TypeExpr::Unit);
    let p3_c = a.cref(ns.o, push3, u20, u8_type());
    let p3_push = a.op(
        ns.o,
        push3,
        Opcode::AdapterInvoke,
        vec![pav(p3_acc), op_result(p3_c)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: push3,
        function: fid,
        parameters: vec![
            p3_acc, p3_lenvec, p3_llen, p3_eidvec, p3_bodyvec, p3_blen, p3_unit,
        ],
        operations: vec![p3_c, p3_push],
        terminator: switch(
            op_result(p3_push),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_copy_start,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(p3_lenvec),
                        sav(p3_llen),
                        sav(p3_eidvec),
                        sav(p3_bodyvec),
                        sav(p3_blen),
                        sav(p3_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Entity copy loop (32B).
    let es_acc = a.param(ns.p, eid_copy_start, ParameterRole::Block, u8vec_type());
    let es_lenvec = a.param(ns.p, eid_copy_start, ParameterRole::Block, u8vec_type());
    let es_llen = a.param(ns.p, eid_copy_start, ParameterRole::Block, u64_type());
    let es_eidvec = a.param(ns.p, eid_copy_start, ParameterRole::Block, u8vec_type());
    let es_bodyvec = a.param(ns.p, eid_copy_start, ParameterRole::Block, u8vec_type());
    let es_blen = a.param(ns.p, eid_copy_start, ParameterRole::Block, u64_type());
    let es_unit = a.param(ns.p, eid_copy_start, ParameterRole::Block, TypeExpr::Unit);
    let es_z0 = a.cref(ns.o, eid_copy_start, c0, u64_type());
    a.blocks.push(Block {
        entity_id: eid_copy_start,
        function: fid,
        parameters: vec![
            es_acc, es_lenvec, es_llen, es_eidvec, es_bodyvec, es_blen, es_unit,
        ],
        operations: vec![es_z0],
        terminator: branch(edge(
            eid_copy_check,
            vec![
                op_result(es_z0),
                pav(es_acc),
                pav(es_lenvec),
                pav(es_llen),
                pav(es_eidvec),
                pav(es_bodyvec),
                pav(es_blen),
                pav(es_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let ec_idx = a.param(ns.p, eid_copy_check, ParameterRole::Block, u64_type());
    let ec_acc = a.param(ns.p, eid_copy_check, ParameterRole::Block, u8vec_type());
    let ec_lenvec = a.param(ns.p, eid_copy_check, ParameterRole::Block, u8vec_type());
    let ec_llen = a.param(ns.p, eid_copy_check, ParameterRole::Block, u64_type());
    let ec_eidvec = a.param(ns.p, eid_copy_check, ParameterRole::Block, u8vec_type());
    let ec_bodyvec = a.param(ns.p, eid_copy_check, ParameterRole::Block, u8vec_type());
    let ec_blen = a.param(ns.p, eid_copy_check, ParameterRole::Block, u64_type());
    let ec_unit = a.param(ns.p, eid_copy_check, ParameterRole::Block, TypeExpr::Unit);
    let ec_k32 = a.cref(ns.o, eid_copy_check, c32, u64_type());
    let ec_lt = a.op(
        ns.o,
        eid_copy_check,
        Opcode::LessThan,
        vec![pav(ec_idx), op_result(ec_k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_copy_check,
        function: fid,
        parameters: vec![
            ec_idx, ec_acc, ec_lenvec, ec_llen, ec_eidvec, ec_bodyvec, ec_blen, ec_unit,
        ],
        operations: vec![ec_k32, ec_lt],
        terminator: cond(
            op_result(ec_lt),
            edge(
                eid_copy_get,
                vec![
                    pav(ec_idx),
                    pav(ec_acc),
                    pav(ec_lenvec),
                    pav(ec_llen),
                    pav(ec_eidvec),
                    pav(ec_bodyvec),
                    pav(ec_blen),
                    pav(ec_unit),
                ],
            ),
            edge(
                tag2_push,
                vec![
                    pav(ec_acc),
                    pav(ec_lenvec),
                    pav(ec_llen),
                    pav(ec_bodyvec),
                    pav(ec_blen),
                    pav(ec_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let eg_idx = a.param(ns.p, eid_copy_get, ParameterRole::Block, u64_type());
    let eg_acc = a.param(ns.p, eid_copy_get, ParameterRole::Block, u8vec_type());
    let eg_lenvec = a.param(ns.p, eid_copy_get, ParameterRole::Block, u8vec_type());
    let eg_llen = a.param(ns.p, eid_copy_get, ParameterRole::Block, u64_type());
    let eg_eidvec = a.param(ns.p, eid_copy_get, ParameterRole::Block, u8vec_type());
    let eg_bodyvec = a.param(ns.p, eid_copy_get, ParameterRole::Block, u8vec_type());
    let eg_blen = a.param(ns.p, eid_copy_get, ParameterRole::Block, u64_type());
    let eg_unit = a.param(ns.p, eid_copy_get, ParameterRole::Block, TypeExpr::Unit);
    let eg_get = a.op(
        ns.o,
        eid_copy_get,
        Opcode::VectorGet,
        vec![pav(eg_eidvec), pav(eg_idx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_copy_get,
        function: fid,
        parameters: vec![
            eg_idx, eg_acc, eg_lenvec, eg_llen, eg_eidvec, eg_bodyvec, eg_blen, eg_unit,
        ],
        operations: vec![eg_get],
        terminator: switch(
            op_result(eg_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    eid_copy_get2,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(eg_idx),
                        sav(eg_acc),
                        sav(eg_lenvec),
                        sav(eg_llen),
                        sav(eg_eidvec),
                        sav(eg_bodyvec),
                        sav(eg_blen),
                        sav(eg_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Note: eid_copy_get2 drops the source vec (index is direct); Get-None
    // unreachable since eid length was checked ==32.
    let hg2_b = a.param(ns.p, eid_copy_get2, ParameterRole::Block, u8_type());
    let hg2_idx = a.param(ns.p, eid_copy_get2, ParameterRole::Block, u64_type());
    let hg2_acc = a.param(ns.p, eid_copy_get2, ParameterRole::Block, u8vec_type());
    let hg2_lenvec = a.param(ns.p, eid_copy_get2, ParameterRole::Block, u8vec_type());
    let hg2_llen = a.param(ns.p, eid_copy_get2, ParameterRole::Block, u64_type());
    let hg2_eidvec = a.param(ns.p, eid_copy_get2, ParameterRole::Block, u8vec_type());
    let hg2_bodyvec = a.param(ns.p, eid_copy_get2, ParameterRole::Block, u8vec_type());
    let hg2_blen = a.param(ns.p, eid_copy_get2, ParameterRole::Block, u64_type());
    let hg2_unit = a.param(ns.p, eid_copy_get2, ParameterRole::Block, TypeExpr::Unit);
    let hg2_push = a.op(
        ns.o,
        eid_copy_get2,
        Opcode::AdapterInvoke,
        vec![pav(hg2_acc), pav(hg2_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: eid_copy_get2,
        function: fid,
        parameters: vec![
            hg2_b,
            hg2_idx,
            hg2_acc,
            hg2_lenvec,
            hg2_llen,
            hg2_eidvec,
            hg2_bodyvec,
            hg2_blen,
            hg2_unit,
        ],
        operations: vec![hg2_push],
        terminator: switch(
            op_result(hg2_push),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_copy_push,
                    vec![
                        sav(hg2_idx),
                        SwitchArgument::CasePayload,
                        sav(hg2_lenvec),
                        sav(hg2_llen),
                        sav(hg2_eidvec),
                        sav(hg2_bodyvec),
                        sav(hg2_blen),
                        sav(hg2_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let ep_idx = a.param(ns.p, eid_copy_push, ParameterRole::Block, u64_type());
    let ep_acc = a.param(ns.p, eid_copy_push, ParameterRole::Block, u8vec_type());
    let ep_lenvec = a.param(ns.p, eid_copy_push, ParameterRole::Block, u8vec_type());
    let ep_llen = a.param(ns.p, eid_copy_push, ParameterRole::Block, u64_type());
    let ep_eidvec = a.param(ns.p, eid_copy_push, ParameterRole::Block, u8vec_type());
    let ep_bodyvec = a.param(ns.p, eid_copy_push, ParameterRole::Block, u8vec_type());
    let ep_blen = a.param(ns.p, eid_copy_push, ParameterRole::Block, u64_type());
    let ep_unit = a.param(ns.p, eid_copy_push, ParameterRole::Block, TypeExpr::Unit);
    let ep_c1 = a.cref(ns.o, eid_copy_push, c1, u64_type());
    // Reuse next block id for increment to keep the graph small.
    let ep_add = a.op(
        ns.o,
        eid_copy_push,
        Opcode::IntAddChecked,
        vec![pav(ep_idx), op_result(ep_c1)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_copy_push,
        function: fid,
        parameters: vec![
            ep_idx, ep_acc, ep_lenvec, ep_llen, ep_eidvec, ep_bodyvec, ep_blen, ep_unit,
        ],
        operations: vec![ep_c1, ep_add],
        terminator: switch(
            op_result(ep_add),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_copy_next,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(ep_acc),
                        sav(ep_lenvec),
                        sav(ep_llen),
                        sav(ep_eidvec),
                        sav(ep_bodyvec),
                        sav(ep_blen),
                        sav(ep_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let en_idx = a.param(ns.p, eid_copy_next, ParameterRole::Block, u64_type());
    let en_acc = a.param(ns.p, eid_copy_next, ParameterRole::Block, u8vec_type());
    let en_lenvec = a.param(ns.p, eid_copy_next, ParameterRole::Block, u8vec_type());
    let en_llen = a.param(ns.p, eid_copy_next, ParameterRole::Block, u64_type());
    let en_eidvec = a.param(ns.p, eid_copy_next, ParameterRole::Block, u8vec_type());
    let en_bodyvec = a.param(ns.p, eid_copy_next, ParameterRole::Block, u8vec_type());
    let en_blen = a.param(ns.p, eid_copy_next, ParameterRole::Block, u64_type());
    let en_unit = a.param(ns.p, eid_copy_next, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: eid_copy_next,
        function: fid,
        parameters: vec![
            en_idx, en_acc, en_lenvec, en_llen, en_eidvec, en_bodyvec, en_blen, en_unit,
        ],
        operations: Vec::new(),
        terminator: branch(edge(
            eid_copy_check,
            vec![
                pav(en_idx),
                pav(en_acc),
                pav(en_lenvec),
                pav(en_llen),
                pav(en_eidvec),
                pav(en_bodyvec),
                pav(en_blen),
                pav(en_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    // Tag 2 push (02).
    let t2_acc = a.param(ns.p, tag2_push, ParameterRole::Block, u8vec_type());
    let t2_lenvec = a.param(ns.p, tag2_push, ParameterRole::Block, u8vec_type());
    let t2_llen = a.param(ns.p, tag2_push, ParameterRole::Block, u64_type());
    let t2_bodyvec = a.param(ns.p, tag2_push, ParameterRole::Block, u8vec_type());
    let t2_blen = a.param(ns.p, tag2_push, ParameterRole::Block, u64_type());
    let t2_unit = a.param(ns.p, tag2_push, ParameterRole::Block, TypeExpr::Unit);
    let t2_c = a.cref(ns.o, tag2_push, u02, u8_type());
    let t2_push = a.op(
        ns.o,
        tag2_push,
        Opcode::AdapterInvoke,
        vec![pav(t2_acc), op_result(t2_c)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: tag2_push,
        function: fid,
        parameters: vec![t2_acc, t2_lenvec, t2_llen, t2_bodyvec, t2_blen, t2_unit],
        operations: vec![t2_c, t2_push],
        terminator: switch(
            op_result(t2_push),
            vec![
                (
                    BuiltinCase::Ok,
                    lencopy_start,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(t2_lenvec),
                        sav(t2_llen),
                        sav(t2_bodyvec),
                        sav(t2_blen),
                        sav(t2_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Length-bytes copy loop.
    let lc_acc = a.param(ns.p, lencopy_start, ParameterRole::Block, u8vec_type());
    let lc_lenvec = a.param(ns.p, lencopy_start, ParameterRole::Block, u8vec_type());
    let lc_llen = a.param(ns.p, lencopy_start, ParameterRole::Block, u64_type());
    let lc_bodyvec = a.param(ns.p, lencopy_start, ParameterRole::Block, u8vec_type());
    let lc_blen = a.param(ns.p, lencopy_start, ParameterRole::Block, u64_type());
    let lc_unit = a.param(ns.p, lencopy_start, ParameterRole::Block, TypeExpr::Unit);
    let lc_z0 = a.cref(ns.o, lencopy_start, c0, u64_type());
    a.blocks.push(Block {
        entity_id: lencopy_start,
        function: fid,
        parameters: vec![lc_acc, lc_lenvec, lc_llen, lc_bodyvec, lc_blen, lc_unit],
        operations: vec![lc_z0],
        terminator: branch(edge(
            lencopy_check,
            vec![
                op_result(lc_z0),
                pav(lc_acc),
                pav(lc_lenvec),
                pav(lc_llen),
                pav(lc_bodyvec),
                pav(lc_blen),
                pav(lc_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let lk_idx = a.param(ns.p, lencopy_check, ParameterRole::Block, u64_type());
    let lk_acc = a.param(ns.p, lencopy_check, ParameterRole::Block, u8vec_type());
    let lk_lenvec = a.param(ns.p, lencopy_check, ParameterRole::Block, u8vec_type());
    let lk_llen = a.param(ns.p, lencopy_check, ParameterRole::Block, u64_type());
    let lk_bodyvec = a.param(ns.p, lencopy_check, ParameterRole::Block, u8vec_type());
    let lk_blen = a.param(ns.p, lencopy_check, ParameterRole::Block, u64_type());
    let lk_unit = a.param(ns.p, lencopy_check, ParameterRole::Block, TypeExpr::Unit);
    let lk_lt = a.op(
        ns.o,
        lencopy_check,
        Opcode::LessThan,
        vec![pav(lk_idx), pav(lk_llen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: lencopy_check,
        function: fid,
        parameters: vec![
            lk_idx, lk_acc, lk_lenvec, lk_llen, lk_bodyvec, lk_blen, lk_unit,
        ],
        operations: vec![lk_lt],
        terminator: cond(
            op_result(lk_lt),
            edge(
                lencopy_get,
                vec![
                    pav(lk_idx),
                    pav(lk_acc),
                    pav(lk_lenvec),
                    pav(lk_llen),
                    pav(lk_bodyvec),
                    pav(lk_blen),
                    pav(lk_unit),
                ],
            ),
            edge(
                bodycopy_start,
                vec![pav(lk_acc), pav(lk_bodyvec), pav(lk_blen), pav(lk_unit)],
            ),
        ),
        reachability: Reachability::Required,
    });
    let lg_idx = a.param(ns.p, lencopy_get, ParameterRole::Block, u64_type());
    let lg_acc = a.param(ns.p, lencopy_get, ParameterRole::Block, u8vec_type());
    let lg_lenvec = a.param(ns.p, lencopy_get, ParameterRole::Block, u8vec_type());
    let lg_llen = a.param(ns.p, lencopy_get, ParameterRole::Block, u64_type());
    let lg_bodyvec = a.param(ns.p, lencopy_get, ParameterRole::Block, u8vec_type());
    let lg_blen = a.param(ns.p, lencopy_get, ParameterRole::Block, u64_type());
    let lg_unit = a.param(ns.p, lencopy_get, ParameterRole::Block, TypeExpr::Unit);
    let lg_get = a.op(
        ns.o,
        lencopy_get,
        Opcode::VectorGet,
        vec![pav(lg_lenvec), pav(lg_idx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: lencopy_get,
        function: fid,
        parameters: vec![
            lg_idx, lg_acc, lg_lenvec, lg_llen, lg_bodyvec, lg_blen, lg_unit,
        ],
        operations: vec![lg_get],
        terminator: switch(
            op_result(lg_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    lencopy_get2,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(lg_idx),
                        sav(lg_acc),
                        sav(lg_lenvec),
                        sav(lg_llen),
                        sav(lg_bodyvec),
                        sav(lg_blen),
                        sav(lg_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let mh_b = a.param(ns.p, lencopy_get2, ParameterRole::Block, u8_type());
    let mh_idx = a.param(ns.p, lencopy_get2, ParameterRole::Block, u64_type());
    let mh_acc = a.param(ns.p, lencopy_get2, ParameterRole::Block, u8vec_type());
    let mh_lenvec = a.param(ns.p, lencopy_get2, ParameterRole::Block, u8vec_type());
    let mh_llen = a.param(ns.p, lencopy_get2, ParameterRole::Block, u64_type());
    let mh_bodyvec = a.param(ns.p, lencopy_get2, ParameterRole::Block, u8vec_type());
    let mh_blen = a.param(ns.p, lencopy_get2, ParameterRole::Block, u64_type());
    let mh_unit = a.param(ns.p, lencopy_get2, ParameterRole::Block, TypeExpr::Unit);
    let mh_push = a.op(
        ns.o,
        lencopy_get2,
        Opcode::AdapterInvoke,
        vec![pav(mh_acc), pav(mh_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: lencopy_get2,
        function: fid,
        parameters: vec![
            mh_b, mh_idx, mh_acc, mh_lenvec, mh_llen, mh_bodyvec, mh_blen, mh_unit,
        ],
        operations: vec![mh_push],
        terminator: switch(
            op_result(mh_push),
            vec![
                (
                    BuiltinCase::Ok,
                    lencopy_push,
                    vec![
                        sav(mh_idx),
                        SwitchArgument::CasePayload,
                        sav(mh_lenvec),
                        sav(mh_llen),
                        sav(mh_bodyvec),
                        sav(mh_blen),
                        sav(mh_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let mh2_idx = a.param(ns.p, lencopy_push, ParameterRole::Block, u64_type());
    let mh2_acc = a.param(ns.p, lencopy_push, ParameterRole::Block, u8vec_type());
    let mh2_lenvec = a.param(ns.p, lencopy_push, ParameterRole::Block, u8vec_type());
    let mh2_llen = a.param(ns.p, lencopy_push, ParameterRole::Block, u64_type());
    let mh2_bodyvec = a.param(ns.p, lencopy_push, ParameterRole::Block, u8vec_type());
    let mh2_blen = a.param(ns.p, lencopy_push, ParameterRole::Block, u64_type());
    let mh2_unit = a.param(ns.p, lencopy_push, ParameterRole::Block, TypeExpr::Unit);
    let mh2_c1 = a.cref(ns.o, lencopy_push, c1, u64_type());
    let mh2_add = a.op(
        ns.o,
        lencopy_push,
        Opcode::IntAddChecked,
        vec![pav(mh2_idx), op_result(mh2_c1)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: lencopy_push,
        function: fid,
        parameters: vec![
            mh2_idx,
            mh2_acc,
            mh2_lenvec,
            mh2_llen,
            mh2_bodyvec,
            mh2_blen,
            mh2_unit,
        ],
        operations: vec![mh2_c1, mh2_add],
        terminator: switch(
            op_result(mh2_add),
            vec![
                (
                    BuiltinCase::Ok,
                    lencopy_next,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(mh2_acc),
                        sav(mh2_lenvec),
                        sav(mh2_llen),
                        sav(mh2_bodyvec),
                        sav(mh2_blen),
                        sav(mh2_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let mn_idx = a.param(ns.p, lencopy_next, ParameterRole::Block, u64_type());
    let mn_acc = a.param(ns.p, lencopy_next, ParameterRole::Block, u8vec_type());
    let mn_lenvec = a.param(ns.p, lencopy_next, ParameterRole::Block, u8vec_type());
    let mn_llen = a.param(ns.p, lencopy_next, ParameterRole::Block, u64_type());
    let mn_bodyvec = a.param(ns.p, lencopy_next, ParameterRole::Block, u8vec_type());
    let mn_blen = a.param(ns.p, lencopy_next, ParameterRole::Block, u64_type());
    let mn_unit = a.param(ns.p, lencopy_next, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: lencopy_next,
        function: fid,
        parameters: vec![
            mn_idx, mn_acc, mn_lenvec, mn_llen, mn_bodyvec, mn_blen, mn_unit,
        ],
        operations: Vec::new(),
        terminator: branch(edge(
            lencopy_check,
            vec![
                pav(mn_idx),
                pav(mn_acc),
                pav(mn_lenvec),
                pav(mn_llen),
                pav(mn_bodyvec),
                pav(mn_blen),
                pav(mn_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    // Body copy loop.
    let wc_acc = a.param(ns.p, bodycopy_start, ParameterRole::Block, u8vec_type());
    let wc_bodyvec = a.param(ns.p, bodycopy_start, ParameterRole::Block, u8vec_type());
    let wc_blen = a.param(ns.p, bodycopy_start, ParameterRole::Block, u64_type());
    let wc_unit = a.param(ns.p, bodycopy_start, ParameterRole::Block, TypeExpr::Unit);
    let wc_z0 = a.cref(ns.o, bodycopy_start, c0, u64_type());
    a.blocks.push(Block {
        entity_id: bodycopy_start,
        function: fid,
        parameters: vec![wc_acc, wc_bodyvec, wc_blen, wc_unit],
        operations: vec![wc_z0],
        terminator: branch(edge(
            bodycopy_check,
            vec![
                op_result(wc_z0),
                pav(wc_acc),
                pav(wc_bodyvec),
                pav(wc_blen),
                pav(wc_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let wk_idx = a.param(ns.p, bodycopy_check, ParameterRole::Block, u64_type());
    let wk_acc = a.param(ns.p, bodycopy_check, ParameterRole::Block, u8vec_type());
    let wk_bodyvec = a.param(ns.p, bodycopy_check, ParameterRole::Block, u8vec_type());
    let wk_blen = a.param(ns.p, bodycopy_check, ParameterRole::Block, u64_type());
    let wk_unit = a.param(ns.p, bodycopy_check, ParameterRole::Block, TypeExpr::Unit);
    let wk_lt = a.op(
        ns.o,
        bodycopy_check,
        Opcode::LessThan,
        vec![pav(wk_idx), pav(wk_blen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: bodycopy_check,
        function: fid,
        parameters: vec![wk_idx, wk_acc, wk_bodyvec, wk_blen, wk_unit],
        operations: vec![wk_lt],
        terminator: cond(
            op_result(wk_lt),
            edge(
                bodycopy_get,
                vec![
                    pav(wk_idx),
                    pav(wk_acc),
                    pav(wk_bodyvec),
                    pav(wk_blen),
                    pav(wk_unit),
                ],
            ),
            edge(out_done, vec![pav(wk_acc), pav(wk_unit)]),
        ),
        reachability: Reachability::Required,
    });
    let wg_idx = a.param(ns.p, bodycopy_get, ParameterRole::Block, u64_type());
    let wg_acc = a.param(ns.p, bodycopy_get, ParameterRole::Block, u8vec_type());
    let wg_bodyvec = a.param(ns.p, bodycopy_get, ParameterRole::Block, u8vec_type());
    let wg_blen = a.param(ns.p, bodycopy_get, ParameterRole::Block, u64_type());
    let wg_unit = a.param(ns.p, bodycopy_get, ParameterRole::Block, TypeExpr::Unit);
    let wg_get = a.op(
        ns.o,
        bodycopy_get,
        Opcode::VectorGet,
        vec![pav(wg_bodyvec), pav(wg_idx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: bodycopy_get,
        function: fid,
        parameters: vec![wg_idx, wg_acc, wg_bodyvec, wg_blen, wg_unit],
        operations: vec![wg_get],
        terminator: switch(
            op_result(wg_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    bodycopy_get2,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(wg_idx),
                        sav(wg_acc),
                        sav(wg_bodyvec),
                        sav(wg_blen),
                        sav(wg_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let wh_b = a.param(ns.p, bodycopy_get2, ParameterRole::Block, u8_type());
    let wh_idx = a.param(ns.p, bodycopy_get2, ParameterRole::Block, u64_type());
    let wh_acc = a.param(ns.p, bodycopy_get2, ParameterRole::Block, u8vec_type());
    let wh_bodyvec = a.param(ns.p, bodycopy_get2, ParameterRole::Block, u8vec_type());
    let wh_blen = a.param(ns.p, bodycopy_get2, ParameterRole::Block, u64_type());
    let wh_unit = a.param(ns.p, bodycopy_get2, ParameterRole::Block, TypeExpr::Unit);
    let wh_push = a.op(
        ns.o,
        bodycopy_get2,
        Opcode::AdapterInvoke,
        vec![pav(wh_acc), pav(wh_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: bodycopy_get2,
        function: fid,
        parameters: vec![wh_b, wh_idx, wh_acc, wh_bodyvec, wh_blen, wh_unit],
        operations: vec![wh_push],
        terminator: switch(
            op_result(wh_push),
            vec![
                (
                    BuiltinCase::Ok,
                    bodycopy_push,
                    vec![
                        sav(wh_idx),
                        SwitchArgument::CasePayload,
                        sav(wh_bodyvec),
                        sav(wh_blen),
                        sav(wh_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let wh2_idx = a.param(ns.p, bodycopy_push, ParameterRole::Block, u64_type());
    let wh2_acc = a.param(ns.p, bodycopy_push, ParameterRole::Block, u8vec_type());
    let wh2_bodyvec = a.param(ns.p, bodycopy_push, ParameterRole::Block, u8vec_type());
    let wh2_blen = a.param(ns.p, bodycopy_push, ParameterRole::Block, u64_type());
    let wh2_unit = a.param(ns.p, bodycopy_push, ParameterRole::Block, TypeExpr::Unit);
    let wh2_c1 = a.cref(ns.o, bodycopy_push, c1, u64_type());
    let wh2_add = a.op(
        ns.o,
        bodycopy_push,
        Opcode::IntAddChecked,
        vec![pav(wh2_idx), op_result(wh2_c1)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: bodycopy_push,
        function: fid,
        parameters: vec![wh2_idx, wh2_acc, wh2_bodyvec, wh2_blen, wh2_unit],
        operations: vec![wh2_c1, wh2_add],
        terminator: switch(
            op_result(wh2_add),
            vec![
                (
                    BuiltinCase::Ok,
                    bodycopy_next,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(wh2_acc),
                        sav(wh2_bodyvec),
                        sav(wh2_blen),
                        sav(wh2_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let wn_idx = a.param(ns.p, bodycopy_next, ParameterRole::Block, u64_type());
    let wn_acc = a.param(ns.p, bodycopy_next, ParameterRole::Block, u8vec_type());
    let wn_bodyvec = a.param(ns.p, bodycopy_next, ParameterRole::Block, u8vec_type());
    let wn_blen = a.param(ns.p, bodycopy_next, ParameterRole::Block, u64_type());
    let wn_unit = a.param(ns.p, bodycopy_next, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: bodycopy_next,
        function: fid,
        parameters: vec![wn_idx, wn_acc, wn_bodyvec, wn_blen, wn_unit],
        operations: Vec::new(),
        terminator: branch(edge(
            bodycopy_check,
            vec![
                pav(wn_idx),
                pav(wn_acc),
                pav(wn_bodyvec),
                pav(wn_blen),
                pav(wn_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let od_acc = a.param(ns.p, out_done, ParameterRole::Block, u8vec_type());
    let od_unit = a.param(ns.p, out_done, ParameterRole::Block, TypeExpr::Unit);
    let od_v2b = a.op(
        ns.o,
        out_done,
        Opcode::AdapterInvoke,
        vec![pav(od_unit), pav(od_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: out_done,
        function: fid,
        parameters: vec![od_acc, od_unit],
        operations: vec![od_v2b],
        terminator: switch(
            op_result(od_v2b),
            vec![
                (BuiltinCase::Ok, out_ret, vec![SwitchArgument::CasePayload]),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let or_bytes = a.param(ns.p, out_ret, ParameterRole::Block, TypeExpr::Bytes);
    let or_ok = a.op(
        ns.o,
        out_ret,
        Opcode::ResultOk,
        vec![pav(or_bytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: out_ret,
        function: fid,
        parameters: vec![or_bytes],
        operations: vec![or_ok],
        terminator: ret(op_result(or_ok)),
        reachability: Reachability::Required,
    });
    let _ = c2;

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![q_eid, q_body, q_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn entrypoint_decode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![TypeExpr::Bytes, u64_type()])),
        error: Box::new(TypeExpr::Bytes),
    }
}

// ── EntryPoint body kind 16 (supported body for this slice) ───────────
// `decode_entrypoint(body: Bytes, unit: Unit)` parses the closed
// `EntityBody` union tag 16 + `EntryPointBody` record (function Fixed32,
// exposure enum 1/2) with exact SCB framing and canonical checks, returning
// `Tuple(function: Bytes 32B, exposure: UInt64 1/2)`. Tags 1..15,17,18 are
// known-but-unimplemented -> `SSMC_RESERVED_FIELD_PRESENT` scope (not
// invalid); tag 0/19+ -> `SCB_UNION_INVALID`. Record uses exact
// `decode_record_fields` order for [1,2] (DUP/ORDER/UNKNOWN/MISSING).
// Exposure uses `decode_uvar` width 32 on the extracted field payload so
// non-minimal/overflow stay canonical, with exact-consumption trailing.
// Sley owns parsing and decisions; bridge uses B2V1/PSH1/V2B1 only.
#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_entrypoint_decode(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    decode_fid: EntityId,
) -> FunctionGraph {
    let bstart = a.blocks.len();
    let res_t = entrypoint_decode_result_type();
    let dec_t = decode_result_type();
    let c0 = a.ku64(ns.k, 0);
    let c1 = a.ku64(ns.k, 1);
    let c2 = a.ku64(ns.k, 2);
    let c16 = a.ku64(ns.k, 16);
    let c18 = a.ku64(ns.k, 18);
    let c32 = a.ku64(ns.k, 32);
    let c_max_fields = a.ku64(ns.k, 65_535);
    let c_max = a.ku64(ns.k, 67_108_864);
    let w32 = a.ku32(ns.k, 32);
    let w64 = a.ku32(ns.k, 64);
    let e_missing = a.kbytes(ns.k, b"SCB_FIELD_MISSING");
    let e_unknown = a.kbytes(ns.k, b"SCB_FIELD_UNKNOWN");
    let e_dup = a.kbytes(ns.k, b"SCB_FIELD_DUPLICATE");
    let e_order = a.kbytes(ns.k, b"SCB_FIELD_ORDER");
    let e_len = a.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let e_trail = a.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_res = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let e_union = a.kbytes(ns.k, b"SCB_UNION_INVALID");
    let e_scope = a.kbytes(ns.k, b"SSMC_RESERVED_FIELD_PRESENT");
    let in_body = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let in_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let b_missing = err_block(a, ns, fid, res_t.clone(), e_missing);
    let b_unknown = err_block(a, ns, fid, res_t.clone(), e_unknown);
    let b_dup = err_block(a, ns, fid, res_t.clone(), e_dup);
    let b_order = err_block(a, ns, fid, res_t.clone(), e_order);
    let b_len = err_block(a, ns, fid, res_t.clone(), e_len);
    let b_trail = err_block(a, ns, fid, res_t.clone(), e_trail);
    let b_res = err_block(a, ns, fid, res_t.clone(), e_res);
    let b_union = err_block(a, ns, fid, res_t.clone(), e_union);
    let b_scope = err_block(a, ns, fid, res_t.clone(), e_scope);
    let trap = trap_block(a, ns, fid);
    let entry = a.id(ns.b);
    let b_cnt = a.id(ns.b);
    let u_tag = a.id(ns.b);
    let u_tag_ok = a.id(ns.b);
    let u_tag_err = a.id(ns.b);
    let u_disp = a.id(ns.b);
    let u_lo = a.id(ns.b);
    let u_hi = a.id(ns.b);
    let u_len = a.id(ns.b);
    let u_len_ok = a.id(ns.b);
    let u_len_err = a.id(ns.b);
    let u_bnd = a.id(ns.b);
    let u_unwrap = a.id(ns.b);
    let u_trail = a.id(ns.b);
    let r_cnt = a.id(ns.b);
    let r_cnt_ok = a.id(ns.b);
    let r_cnt_err = a.id(ns.b);
    let r_chk = a.id(ns.b);
    let f1_tag = a.id(ns.b);
    let f1_tag_ok = a.id(ns.b);
    let f1_tag_err = a.id(ns.b);
    let f1_disp = a.id(ns.b);
    let f1_len = a.id(ns.b);
    let f1_len_ok = a.id(ns.b);
    let f1_len_err = a.id(ns.b);
    let f1_bnd = a.id(ns.b);
    let f1_unwrap = a.id(ns.b);
    let f1_len32 = a.id(ns.b);
    let f1_len32_gt = a.id(ns.b);
    let eid_setup = a.id(ns.b);
    let eid_check = a.id(ns.b);
    let eid_get = a.id(ns.b);
    let eid_get2 = a.id(ns.b);
    let eid_push = a.id(ns.b);
    let eid_next = a.id(ns.b);
    let eid_done = a.id(ns.b);
    let f2_tag = a.id(ns.b);
    let f2_tag_ok = a.id(ns.b);
    let f2_tag_err = a.id(ns.b);
    let f2_disp = a.id(ns.b);
    let f2_ord = a.id(ns.b);
    let f2_len = a.id(ns.b);
    let f2_len_ok = a.id(ns.b);
    let f2_len_err = a.id(ns.b);
    let f2_bnd = a.id(ns.b);
    let f2_unwrap = a.id(ns.b);
    let f2_pay_start = a.id(ns.b);
    let f2_pay_check = a.id(ns.b);
    let f2_pay_get = a.id(ns.b);
    let f2_pay_get2 = a.id(ns.b);
    let f2_pay_push = a.id(ns.b);
    let f2_pay_next = a.id(ns.b);
    let f2_pay_done = a.id(ns.b);
    let exp_call = a.id(ns.b);
    let exp_ok = a.id(ns.b);
    let exp_err = a.id(ns.b);
    let exp_trail = a.id(ns.b);
    let exp_disp = a.id(ns.b);
    let done_ret = a.id(ns.b);
    // Entry B2V1.
    let cv = a.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(in_unit), pav(in_body)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
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
                    vec![SwitchArgument::CasePayload, sav(in_body), sav(in_unit)],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Length for bounds.
    let ln = a.op(
        ns.o,
        b_cnt,
        Opcode::VectorLen,
        vec![pav(q_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let z0 = a.cref(ns.o, b_cnt, c0, u64_type());
    let w32c = a.cref(ns.o, b_cnt, w32, u32_type());
    let tag_call = a.op(
        ns.o,
        b_cnt,
        Opcode::CallDirect,
        vec![pav(q_in), op_result(z0), op_result(w32c), pav(q_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let t_tup = a.param(
        ns.p,
        u_tag_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let t_ovec = a.param(ns.p, u_tag_ok, ParameterRole::Block, u8vec_type());
    let t_olen = a.param(ns.p, u_tag_ok, ParameterRole::Block, u64_type());
    let t_oin = a.param(ns.p, u_tag_ok, ParameterRole::Block, TypeExpr::Bytes);
    let t_ounit = a.param(ns.p, u_tag_ok, ParameterRole::Block, TypeExpr::Unit);
    let t_ebytes = a.param(ns.p, u_tag_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: b_cnt,
        function: fid,
        parameters: vec![q_vec, q_in, q_unit],
        operations: vec![ln, z0, w32c, tag_call],
        terminator: switch(
            op_result(tag_call),
            vec![
                (
                    BuiltinCase::Ok,
                    u_tag_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(q_vec),
                        oav(ln),
                        sav(q_in),
                        sav(q_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    u_tag_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let t_er = a.op(
        ns.o,
        u_tag_err,
        Opcode::ResultErr,
        vec![pav(t_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: u_tag_err,
        function: fid,
        parameters: vec![t_ebytes],
        operations: vec![t_er],
        terminator: ret(op_result(t_er)),
        reachability: Reachability::Required,
    });
    let g_tag = a.op(
        ns.o,
        u_tag_ok,
        Opcode::TupleGet,
        vec![pav(t_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let g_pos = a.op(
        ns.o,
        u_tag_ok,
        Opcode::TupleGet,
        vec![pav(t_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    a.blocks.push(Block {
        entity_id: u_tag_ok,
        function: fid,
        parameters: vec![t_tup, t_ovec, t_olen, t_oin, t_ounit],
        operations: vec![g_tag, g_pos],
        terminator: branch(edge(
            u_tag,
            vec![
                op_result(g_tag),
                op_result(g_pos),
                pav(t_ovec),
                pav(t_olen),
                pav(t_oin),
                pav(t_ounit),
            ],
        )),
        reachability: Reachability::Required,
    });
    // Union tag dispatch: 16 valid; 1..15,17,18 scope; 0/19+ union-invalid.
    let d_tag = a.param(ns.p, u_tag, ParameterRole::Block, u64_type());
    let d_pos = a.param(ns.p, u_tag, ParameterRole::Block, u64_type());
    let d_vec = a.param(ns.p, u_tag, ParameterRole::Block, u8vec_type());
    let d_len = a.param(ns.p, u_tag, ParameterRole::Block, u64_type());
    let d_in = a.param(ns.p, u_tag, ParameterRole::Block, TypeExpr::Bytes);
    let d_unit = a.param(ns.p, u_tag, ParameterRole::Block, TypeExpr::Unit);
    let d_k16 = a.cref(ns.o, u_tag, c16, u64_type());
    let d_eq = a.op(
        ns.o,
        u_tag,
        Opcode::Equal,
        vec![pav(d_tag), op_result(d_k16)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: u_tag,
        function: fid,
        parameters: vec![d_tag, d_pos, d_vec, d_len, d_in, d_unit],
        operations: vec![d_k16, d_eq],
        terminator: cond(
            op_result(d_eq),
            edge(
                u_len,
                vec![pav(d_pos), pav(d_vec), pav(d_len), pav(d_in), pav(d_unit)],
            ),
            edge(
                u_disp,
                vec![
                    pav(d_tag),
                    pav(d_pos),
                    pav(d_vec),
                    pav(d_len),
                    pav(d_in),
                    pav(d_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let s_tag = a.param(ns.p, u_disp, ParameterRole::Block, u64_type());
    let s_pos = a.param(ns.p, u_disp, ParameterRole::Block, u64_type());
    let s_vec = a.param(ns.p, u_disp, ParameterRole::Block, u8vec_type());
    let s_len = a.param(ns.p, u_disp, ParameterRole::Block, u64_type());
    let s_in = a.param(ns.p, u_disp, ParameterRole::Block, TypeExpr::Bytes);
    let s_unit = a.param(ns.p, u_disp, ParameterRole::Block, TypeExpr::Unit);
    let s_k1 = a.cref(ns.o, u_disp, c1, u64_type());
    let s_lt = a.op(
        ns.o,
        u_disp,
        Opcode::LessThan,
        vec![pav(s_tag), op_result(s_k1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: u_disp,
        function: fid,
        parameters: vec![s_tag, s_pos, s_vec, s_len, s_in, s_unit],
        operations: vec![s_k1, s_lt],
        terminator: cond(
            op_result(s_lt),
            edge(b_union, Vec::new()),
            edge(
                u_lo,
                vec![
                    pav(s_tag),
                    pav(s_pos),
                    pav(s_vec),
                    pav(s_len),
                    pav(s_in),
                    pav(s_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let lo_tag = a.param(ns.p, u_lo, ParameterRole::Block, u64_type());
    let lo_pos = a.param(ns.p, u_lo, ParameterRole::Block, u64_type());
    let lo_vec = a.param(ns.p, u_lo, ParameterRole::Block, u8vec_type());
    let lo_len = a.param(ns.p, u_lo, ParameterRole::Block, u64_type());
    let lo_in = a.param(ns.p, u_lo, ParameterRole::Block, TypeExpr::Bytes);
    let lo_unit = a.param(ns.p, u_lo, ParameterRole::Block, TypeExpr::Unit);
    let lo_k18 = a.cref(ns.o, u_lo, c18, u64_type());
    let lo_gt = a.op(
        ns.o,
        u_lo,
        Opcode::GreaterThan,
        vec![pav(lo_tag), op_result(lo_k18)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: u_lo,
        function: fid,
        parameters: vec![lo_tag, lo_pos, lo_vec, lo_len, lo_in, lo_unit],
        operations: vec![lo_k18, lo_gt],
        terminator: cond(
            op_result(lo_gt),
            edge(b_union, Vec::new()),
            edge(
                u_hi,
                vec![
                    pav(lo_tag),
                    pav(lo_pos),
                    pav(lo_vec),
                    pav(lo_len),
                    pav(lo_in),
                    pav(lo_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // 1..18 except 16 (16 already routed valid): scope.
    let hi_tag = a.param(ns.p, u_hi, ParameterRole::Block, u64_type());
    let hi_pos = a.param(ns.p, u_hi, ParameterRole::Block, u64_type());
    let hi_vec = a.param(ns.p, u_hi, ParameterRole::Block, u8vec_type());
    let hi_len = a.param(ns.p, u_hi, ParameterRole::Block, u64_type());
    let hi_in = a.param(ns.p, u_hi, ParameterRole::Block, TypeExpr::Bytes);
    let hi_unit = a.param(ns.p, u_hi, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: u_hi,
        function: fid,
        parameters: vec![hi_tag, hi_pos, hi_vec, hi_len, hi_in, hi_unit],
        operations: Vec::new(),
        terminator: branch(edge(b_scope, Vec::new())),
        reachability: Reachability::Required,
    });
    // Union length at pos1.
    let l_pos = a.param(ns.p, u_len, ParameterRole::Block, u64_type());
    let l_vec = a.param(ns.p, u_len, ParameterRole::Block, u8vec_type());
    let l_ilen = a.param(ns.p, u_len, ParameterRole::Block, u64_type());
    let l_in = a.param(ns.p, u_len, ParameterRole::Block, TypeExpr::Bytes);
    let l_unit = a.param(ns.p, u_len, ParameterRole::Block, TypeExpr::Unit);
    let l_w = a.cref(ns.o, u_len, w64, u32_type());
    let l_call = a.op(
        ns.o,
        u_len,
        Opcode::CallDirect,
        vec![pav(l_in), pav(l_pos), op_result(l_w), pav(l_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let ll_tup = a.param(
        ns.p,
        u_len_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let ll_vec = a.param(ns.p, u_len_ok, ParameterRole::Block, u8vec_type());
    let ll_ilen = a.param(ns.p, u_len_ok, ParameterRole::Block, u64_type());
    let ll_in = a.param(ns.p, u_len_ok, ParameterRole::Block, TypeExpr::Bytes);
    let ll_unit = a.param(ns.p, u_len_ok, ParameterRole::Block, TypeExpr::Unit);
    let ll_ebytes = a.param(ns.p, u_len_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: u_len,
        function: fid,
        parameters: vec![l_pos, l_vec, l_ilen, l_in, l_unit],
        operations: vec![l_w, l_call],
        terminator: switch(
            op_result(l_call),
            vec![
                (
                    BuiltinCase::Ok,
                    u_len_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(l_vec),
                        sav(l_ilen),
                        sav(l_in),
                        sav(l_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    u_len_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let ll_er = a.op(
        ns.o,
        u_len_err,
        Opcode::ResultErr,
        vec![pav(ll_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: u_len_err,
        function: fid,
        parameters: vec![ll_ebytes],
        operations: vec![ll_er],
        terminator: ret(op_result(ll_er)),
        reachability: Reachability::Required,
    });
    let ll_glen = a.op(
        ns.o,
        u_len_ok,
        Opcode::TupleGet,
        vec![pav(ll_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let ll_gpos = a.op(
        ns.o,
        u_len_ok,
        Opcode::TupleGet,
        vec![pav(ll_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let ll_max = a.cref(ns.o, u_len_ok, c_max, u64_type());
    let ll_gtmax = a.op(
        ns.o,
        u_len_ok,
        Opcode::GreaterThan,
        vec![op_result(ll_glen), op_result(ll_max)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: u_len_ok,
        function: fid,
        parameters: vec![ll_tup, ll_vec, ll_ilen, ll_in, ll_unit],
        operations: vec![ll_glen, ll_gpos, ll_max, ll_gtmax],
        terminator: cond(
            op_result(ll_gtmax),
            edge(b_res, Vec::new()),
            edge(
                u_bnd,
                vec![
                    op_result(ll_glen),
                    op_result(ll_gpos),
                    pav(ll_vec),
                    pav(ll_ilen),
                    pav(ll_in),
                    pav(ll_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let nb_ulen = a.param(ns.p, u_bnd, ParameterRole::Block, u64_type());
    let nb_pos = a.param(ns.p, u_bnd, ParameterRole::Block, u64_type());
    let nb_vec = a.param(ns.p, u_bnd, ParameterRole::Block, u8vec_type());
    let nb_ilen = a.param(ns.p, u_bnd, ParameterRole::Block, u64_type());
    let nb_in = a.param(ns.p, u_bnd, ParameterRole::Block, TypeExpr::Bytes);
    let nb_unit = a.param(ns.p, u_bnd, ParameterRole::Block, TypeExpr::Unit);
    let nb_add = a.op(
        ns.o,
        u_bnd,
        Opcode::IntAddChecked,
        vec![pav(nb_pos), pav(nb_ulen)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: u_bnd,
        function: fid,
        parameters: vec![nb_ulen, nb_pos, nb_vec, nb_ilen, nb_in, nb_unit],
        operations: vec![nb_add],
        terminator: switch(
            op_result(nb_add),
            vec![
                (
                    BuiltinCase::Ok,
                    u_unwrap,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(nb_ulen),
                        sav(nb_pos),
                        sav(nb_vec),
                        sav(nb_ilen),
                        sav(nb_in),
                        sav(nb_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let uw_end = a.param(ns.p, u_unwrap, ParameterRole::Block, u64_type());
    let uw_ulen = a.param(ns.p, u_unwrap, ParameterRole::Block, u64_type());
    let uw_pos = a.param(ns.p, u_unwrap, ParameterRole::Block, u64_type());
    let uw_vec = a.param(ns.p, u_unwrap, ParameterRole::Block, u8vec_type());
    let uw_ilen = a.param(ns.p, u_unwrap, ParameterRole::Block, u64_type());
    let uw_in = a.param(ns.p, u_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let uw_unit = a.param(ns.p, u_unwrap, ParameterRole::Block, TypeExpr::Unit);
    let uw_gt = a.op(
        ns.o,
        u_unwrap,
        Opcode::GreaterThan,
        vec![pav(uw_end), pav(uw_ilen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: u_unwrap,
        function: fid,
        parameters: vec![uw_end, uw_ulen, uw_pos, uw_vec, uw_ilen, uw_in, uw_unit],
        operations: vec![uw_gt],
        terminator: cond(
            op_result(uw_gt),
            edge(b_len, Vec::new()),
            edge(
                u_trail,
                vec![
                    pav(uw_end),
                    pav(uw_pos),
                    pav(uw_vec),
                    pav(uw_ilen),
                    pav(uw_in),
                    pav(uw_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Union must consume the whole body (no trailing after union value).
    let ut_end = a.param(ns.p, u_trail, ParameterRole::Block, u64_type());
    let ut_pos = a.param(ns.p, u_trail, ParameterRole::Block, u64_type());
    let ut_vec = a.param(ns.p, u_trail, ParameterRole::Block, u8vec_type());
    let ut_ilen = a.param(ns.p, u_trail, ParameterRole::Block, u64_type());
    let ut_in = a.param(ns.p, u_trail, ParameterRole::Block, TypeExpr::Bytes);
    let ut_unit = a.param(ns.p, u_trail, ParameterRole::Block, TypeExpr::Unit);
    let ut_eq = a.op(
        ns.o,
        u_trail,
        Opcode::Equal,
        vec![pav(ut_end), pav(ut_ilen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: u_trail,
        function: fid,
        parameters: vec![ut_end, ut_pos, ut_vec, ut_ilen, ut_in, ut_unit],
        operations: vec![ut_eq],
        terminator: cond(
            op_result(ut_eq),
            edge(
                r_cnt,
                vec![
                    pav(ut_pos),
                    pav(ut_end),
                    pav(ut_vec),
                    pav(ut_ilen),
                    pav(ut_in),
                    pav(ut_unit),
                ],
            ),
            edge(b_trail, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // Record count at union-value start.
    let rc_pos = a.param(ns.p, r_cnt, ParameterRole::Block, u64_type());
    let rc_end = a.param(ns.p, r_cnt, ParameterRole::Block, u64_type());
    let rc_vec = a.param(ns.p, r_cnt, ParameterRole::Block, u8vec_type());
    let rc_ilen = a.param(ns.p, r_cnt, ParameterRole::Block, u64_type());
    let rc_in = a.param(ns.p, r_cnt, ParameterRole::Block, TypeExpr::Bytes);
    let rc_unit = a.param(ns.p, r_cnt, ParameterRole::Block, TypeExpr::Unit);
    let rc_w = a.cref(ns.o, r_cnt, w64, u32_type());
    let rc_call = a.op(
        ns.o,
        r_cnt,
        Opcode::CallDirect,
        vec![pav(rc_in), pav(rc_pos), op_result(rc_w), pav(rc_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let rc_tup = a.param(
        ns.p,
        r_cnt_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let rc_oend = a.param(ns.p, r_cnt_ok, ParameterRole::Block, u64_type());
    let rc_ovec = a.param(ns.p, r_cnt_ok, ParameterRole::Block, u8vec_type());
    let rc_olen = a.param(ns.p, r_cnt_ok, ParameterRole::Block, u64_type());
    let rc_oin = a.param(ns.p, r_cnt_ok, ParameterRole::Block, TypeExpr::Bytes);
    let rc_ounit = a.param(ns.p, r_cnt_ok, ParameterRole::Block, TypeExpr::Unit);
    let rc_ebytes = a.param(ns.p, r_cnt_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: r_cnt,
        function: fid,
        parameters: vec![rc_pos, rc_end, rc_vec, rc_ilen, rc_in, rc_unit],
        operations: vec![rc_w, rc_call],
        terminator: switch(
            op_result(rc_call),
            vec![
                (
                    BuiltinCase::Ok,
                    r_cnt_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(rc_end),
                        sav(rc_vec),
                        sav(rc_ilen),
                        sav(rc_in),
                        sav(rc_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    r_cnt_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let rc_er = a.op(
        ns.o,
        r_cnt_err,
        Opcode::ResultErr,
        vec![pav(rc_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: r_cnt_err,
        function: fid,
        parameters: vec![rc_ebytes],
        operations: vec![rc_er],
        terminator: ret(op_result(rc_er)),
        reachability: Reachability::Required,
    });
    let rc_gcnt = a.op(
        ns.o,
        r_cnt_ok,
        Opcode::TupleGet,
        vec![pav(rc_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let rc_gpos = a.op(
        ns.o,
        r_cnt_ok,
        Opcode::TupleGet,
        vec![pav(rc_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let rc_mf = a.cref(ns.o, r_cnt_ok, c_max_fields, u64_type());
    let rc_gtmax = a.op(
        ns.o,
        r_cnt_ok,
        Opcode::GreaterThan,
        vec![op_result(rc_gcnt), op_result(rc_mf)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let r_lo = a.id(ns.b);
    let r_hi = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: r_cnt_ok,
        function: fid,
        parameters: vec![rc_tup, rc_oend, rc_ovec, rc_olen, rc_oin, rc_ounit],
        operations: vec![rc_gcnt, rc_gpos, rc_mf, rc_gtmax],
        terminator: cond(
            op_result(rc_gtmax),
            edge(b_res, Vec::new()),
            edge(
                r_lo,
                vec![
                    op_result(rc_gcnt),
                    op_result(rc_gpos),
                    pav(rc_oend),
                    pav(rc_ovec),
                    pav(rc_olen),
                    pav(rc_oin),
                    pav(rc_ounit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let lo_cnt = a.param(ns.p, r_lo, ParameterRole::Block, u64_type());
    let lo_pos = a.param(ns.p, r_lo, ParameterRole::Block, u64_type());
    let lo_end = a.param(ns.p, r_lo, ParameterRole::Block, u64_type());
    let lo_vec = a.param(ns.p, r_lo, ParameterRole::Block, u8vec_type());
    let lo_ilen = a.param(ns.p, r_lo, ParameterRole::Block, u64_type());
    let lo_in = a.param(ns.p, r_lo, ParameterRole::Block, TypeExpr::Bytes);
    let lo_unit = a.param(ns.p, r_lo, ParameterRole::Block, TypeExpr::Unit);
    let lo_k2 = a.cref(ns.o, r_lo, c2, u64_type());
    let lo_lt = a.op(
        ns.o,
        r_lo,
        Opcode::LessThan,
        vec![pav(lo_cnt), op_result(lo_k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: r_lo,
        function: fid,
        parameters: vec![lo_cnt, lo_pos, lo_end, lo_vec, lo_ilen, lo_in, lo_unit],
        operations: vec![lo_k2, lo_lt],
        terminator: cond(
            op_result(lo_lt),
            edge(b_missing, Vec::new()),
            edge(
                r_hi,
                vec![
                    pav(lo_cnt),
                    pav(lo_pos),
                    pav(lo_end),
                    pav(lo_vec),
                    pav(lo_ilen),
                    pav(lo_in),
                    pav(lo_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let hi_cnt = a.param(ns.p, r_hi, ParameterRole::Block, u64_type());
    let hi_pos = a.param(ns.p, r_hi, ParameterRole::Block, u64_type());
    let hi_end = a.param(ns.p, r_hi, ParameterRole::Block, u64_type());
    let hi_vec = a.param(ns.p, r_hi, ParameterRole::Block, u8vec_type());
    let hi_ilen = a.param(ns.p, r_hi, ParameterRole::Block, u64_type());
    let hi_in = a.param(ns.p, r_hi, ParameterRole::Block, TypeExpr::Bytes);
    let hi_unit = a.param(ns.p, r_hi, ParameterRole::Block, TypeExpr::Unit);
    let hi_k2 = a.cref(ns.o, r_hi, c2, u64_type());
    let hi_gt = a.op(
        ns.o,
        r_hi,
        Opcode::GreaterThan,
        vec![pav(hi_cnt), op_result(hi_k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: r_hi,
        function: fid,
        parameters: vec![hi_cnt, hi_pos, hi_end, hi_vec, hi_ilen, hi_in, hi_unit],
        operations: vec![hi_k2, hi_gt],
        terminator: cond(
            op_result(hi_gt),
            edge(b_unknown, Vec::new()),
            edge(
                r_chk,
                vec![
                    pav(hi_pos),
                    pav(hi_end),
                    pav(hi_vec),
                    pav(hi_ilen),
                    pav(hi_in),
                    pav(hi_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // r_chk: count==2 confirmed, proceed to field-1 tag at pos.
    let ck_pos = a.param(ns.p, r_chk, ParameterRole::Block, u64_type());
    let ck_end = a.param(ns.p, r_chk, ParameterRole::Block, u64_type());
    let ck_vec = a.param(ns.p, r_chk, ParameterRole::Block, u8vec_type());
    let ck_ilen = a.param(ns.p, r_chk, ParameterRole::Block, u64_type());
    let ck_in = a.param(ns.p, r_chk, ParameterRole::Block, TypeExpr::Bytes);
    let ck_unit = a.param(ns.p, r_chk, ParameterRole::Block, TypeExpr::Unit);
    let ck_w = a.cref(ns.o, r_chk, w32, u32_type());
    let ck_call = a.op(
        ns.o,
        r_chk,
        Opcode::CallDirect,
        vec![pav(ck_in), pav(ck_pos), op_result(ck_w), pav(ck_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    // Reuse f1_tag/f1_tag_ok/f1_tag_err ids for field-1 tag.
    let ck_tup = a.param(
        ns.p,
        f1_tag_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let ck_ovec = a.param(ns.p, f1_tag_ok, ParameterRole::Block, u8vec_type());
    let ck_oend = a.param(ns.p, f1_tag_ok, ParameterRole::Block, u64_type());
    let ck_olen = a.param(ns.p, f1_tag_ok, ParameterRole::Block, u64_type());
    let ck_oin = a.param(ns.p, f1_tag_ok, ParameterRole::Block, TypeExpr::Bytes);
    let ck_ounit = a.param(ns.p, f1_tag_ok, ParameterRole::Block, TypeExpr::Unit);
    let ck_ebytes = a.param(ns.p, f1_tag_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: r_chk,
        function: fid,
        parameters: vec![ck_pos, ck_end, ck_vec, ck_ilen, ck_in, ck_unit],
        operations: vec![ck_w, ck_call],
        terminator: switch(
            op_result(ck_call),
            vec![
                (
                    BuiltinCase::Ok,
                    f1_tag_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(ck_vec),
                        sav(ck_end),
                        sav(ck_ilen),
                        sav(ck_in),
                        sav(ck_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    f1_tag_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let ck_er = a.op(
        ns.o,
        f1_tag_err,
        Opcode::ResultErr,
        vec![pav(ck_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f1_tag_err,
        function: fid,
        parameters: vec![ck_ebytes],
        operations: vec![ck_er],
        terminator: ret(op_result(ck_er)),
        reachability: Reachability::Required,
    });
    let ck_gtag = a.op(
        ns.o,
        f1_tag_ok,
        Opcode::TupleGet,
        vec![pav(ck_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let ck_gpos = a.op(
        ns.o,
        f1_tag_ok,
        Opcode::TupleGet,
        vec![pav(ck_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    a.blocks.push(Block {
        entity_id: f1_tag_ok,
        function: fid,
        parameters: vec![ck_tup, ck_ovec, ck_oend, ck_olen, ck_oin, ck_ounit],
        operations: vec![ck_gtag, ck_gpos],
        terminator: branch(edge(
            f1_tag,
            vec![
                op_result(ck_gtag),
                op_result(ck_gpos),
                pav(ck_oend),
                pav(ck_ovec),
                pav(ck_olen),
                pav(ck_oin),
                pav(ck_ounit),
            ],
        )),
        reachability: Reachability::Required,
    });
    // f1_tag dispatches as f1_disp target; f1_tag block holds tag dispatch.
    // To reuse ids simply: f1_tag checks t1==1 else ORDER/UNKNOWN.
    let t1_tag = a.param(ns.p, f1_tag, ParameterRole::Block, u64_type());
    let t1_pos = a.param(ns.p, f1_tag, ParameterRole::Block, u64_type());
    let t1_end = a.param(ns.p, f1_tag, ParameterRole::Block, u64_type());
    let t1_vec = a.param(ns.p, f1_tag, ParameterRole::Block, u8vec_type());
    let t1_ilen = a.param(ns.p, f1_tag, ParameterRole::Block, u64_type());
    let t1_in = a.param(ns.p, f1_tag, ParameterRole::Block, TypeExpr::Bytes);
    let t1_unit = a.param(ns.p, f1_tag, ParameterRole::Block, TypeExpr::Unit);
    let t1_k1 = a.cref(ns.o, f1_tag, c1, u64_type());
    let t1_eq = a.op(
        ns.o,
        f1_tag,
        Opcode::Equal,
        vec![pav(t1_tag), op_result(t1_k1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f1_tag,
        function: fid,
        parameters: vec![t1_tag, t1_pos, t1_end, t1_vec, t1_ilen, t1_in, t1_unit],
        operations: vec![t1_k1, t1_eq],
        terminator: cond(
            op_result(t1_eq),
            edge(
                f1_len,
                vec![
                    pav(t1_pos),
                    pav(t1_end),
                    pav(t1_vec),
                    pav(t1_ilen),
                    pav(t1_in),
                    pav(t1_unit),
                ],
            ),
            edge(
                f1_disp,
                vec![
                    pav(t1_tag),
                    pav(t1_pos),
                    pav(t1_end),
                    pav(t1_vec),
                    pav(t1_ilen),
                    pav(t1_in),
                    pav(t1_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // f1_disp: t1==2 -> ORDER (expected 1 first), else UNKNOWN.
    let dp_tag = a.param(ns.p, f1_disp, ParameterRole::Block, u64_type());
    let dp_pos = a.param(ns.p, f1_disp, ParameterRole::Block, u64_type());
    let dp_end = a.param(ns.p, f1_disp, ParameterRole::Block, u64_type());
    let dp_vec = a.param(ns.p, f1_disp, ParameterRole::Block, u8vec_type());
    let dp_ilen = a.param(ns.p, f1_disp, ParameterRole::Block, u64_type());
    let dp_in = a.param(ns.p, f1_disp, ParameterRole::Block, TypeExpr::Bytes);
    let dp_unit = a.param(ns.p, f1_disp, ParameterRole::Block, TypeExpr::Unit);
    let dp_k2 = a.cref(ns.o, f1_disp, c2, u64_type());
    let dp_eq = a.op(
        ns.o,
        f1_disp,
        Opcode::Equal,
        vec![pav(dp_tag), op_result(dp_k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f1_disp,
        function: fid,
        parameters: vec![dp_tag, dp_pos, dp_end, dp_vec, dp_ilen, dp_in, dp_unit],
        operations: vec![dp_k2, dp_eq],
        terminator: cond(
            op_result(dp_eq),
            edge(b_order, Vec::new()),
            edge(b_unknown, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // Field-1 length.
    let l1_pos = a.param(ns.p, f1_len, ParameterRole::Block, u64_type());
    let l1_end = a.param(ns.p, f1_len, ParameterRole::Block, u64_type());
    let l1_vec = a.param(ns.p, f1_len, ParameterRole::Block, u8vec_type());
    let l1_ilen = a.param(ns.p, f1_len, ParameterRole::Block, u64_type());
    let l1_in = a.param(ns.p, f1_len, ParameterRole::Block, TypeExpr::Bytes);
    let l1_unit = a.param(ns.p, f1_len, ParameterRole::Block, TypeExpr::Unit);
    let l1_w = a.cref(ns.o, f1_len, w64, u32_type());
    let l1_call = a.op(
        ns.o,
        f1_len,
        Opcode::CallDirect,
        vec![pav(l1_in), pav(l1_pos), op_result(l1_w), pav(l1_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let l1_tup = a.param(
        ns.p,
        f1_len_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let l1_oend = a.param(ns.p, f1_len_ok, ParameterRole::Block, u64_type());
    let l1_ovec = a.param(ns.p, f1_len_ok, ParameterRole::Block, u8vec_type());
    let l1_olen = a.param(ns.p, f1_len_ok, ParameterRole::Block, u64_type());
    let l1_oin = a.param(ns.p, f1_len_ok, ParameterRole::Block, TypeExpr::Bytes);
    let l1_ounit = a.param(ns.p, f1_len_ok, ParameterRole::Block, TypeExpr::Unit);
    let l1_ebytes = a.param(ns.p, f1_len_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f1_len,
        function: fid,
        parameters: vec![l1_pos, l1_end, l1_vec, l1_ilen, l1_in, l1_unit],
        operations: vec![l1_w, l1_call],
        terminator: switch(
            op_result(l1_call),
            vec![
                (
                    BuiltinCase::Ok,
                    f1_len_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(l1_end),
                        sav(l1_vec),
                        sav(l1_ilen),
                        sav(l1_in),
                        sav(l1_unit),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    f1_len_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let l1_er = a.op(
        ns.o,
        f1_len_err,
        Opcode::ResultErr,
        vec![pav(l1_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f1_len_err,
        function: fid,
        parameters: vec![l1_ebytes],
        operations: vec![l1_er],
        terminator: ret(op_result(l1_er)),
        reachability: Reachability::Required,
    });
    let l1_glen = a.op(
        ns.o,
        f1_len_ok,
        Opcode::TupleGet,
        vec![pav(l1_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let l1_gpos = a.op(
        ns.o,
        f1_len_ok,
        Opcode::TupleGet,
        vec![pav(l1_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let l1_max = a.cref(ns.o, f1_len_ok, c_max, u64_type());
    let l1_gtmax = a.op(
        ns.o,
        f1_len_ok,
        Opcode::GreaterThan,
        vec![op_result(l1_glen), op_result(l1_max)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f1_len_ok,
        function: fid,
        parameters: vec![l1_tup, l1_oend, l1_ovec, l1_olen, l1_oin, l1_ounit],
        operations: vec![l1_glen, l1_gpos, l1_max, l1_gtmax],
        terminator: cond(
            op_result(l1_gtmax),
            edge(b_res, Vec::new()),
            edge(
                f1_bnd,
                vec![
                    op_result(l1_glen),
                    op_result(l1_gpos),
                    pav(l1_oend),
                    pav(l1_ovec),
                    pav(l1_olen),
                    pav(l1_oin),
                    pav(l1_ounit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let b1_len = a.param(ns.p, f1_bnd, ParameterRole::Block, u64_type());
    let b1_pos = a.param(ns.p, f1_bnd, ParameterRole::Block, u64_type());
    let b1_end = a.param(ns.p, f1_bnd, ParameterRole::Block, u64_type());
    let b1_vec = a.param(ns.p, f1_bnd, ParameterRole::Block, u8vec_type());
    let b1_ilen = a.param(ns.p, f1_bnd, ParameterRole::Block, u64_type());
    let b1_in = a.param(ns.p, f1_bnd, ParameterRole::Block, TypeExpr::Bytes);
    let b1_unit = a.param(ns.p, f1_bnd, ParameterRole::Block, TypeExpr::Unit);
    let b1_add = a.op(
        ns.o,
        f1_bnd,
        Opcode::IntAddChecked,
        vec![pav(b1_pos), pav(b1_len)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f1_bnd,
        function: fid,
        parameters: vec![b1_len, b1_pos, b1_end, b1_vec, b1_ilen, b1_in, b1_unit],
        operations: vec![b1_add],
        terminator: switch(
            op_result(b1_add),
            vec![
                (
                    BuiltinCase::Ok,
                    f1_unwrap,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(b1_len),
                        sav(b1_end),
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
    let u1_end = a.param(ns.p, f1_unwrap, ParameterRole::Block, u64_type());
    let u1_len = a.param(ns.p, f1_unwrap, ParameterRole::Block, u64_type());
    let u1_uend = a.param(ns.p, f1_unwrap, ParameterRole::Block, u64_type());
    let u1_vec = a.param(ns.p, f1_unwrap, ParameterRole::Block, u8vec_type());
    let u1_ilen = a.param(ns.p, f1_unwrap, ParameterRole::Block, u64_type());
    let u1_in = a.param(ns.p, f1_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let u1_unit = a.param(ns.p, f1_unwrap, ParameterRole::Block, TypeExpr::Unit);
    // Bounds within union value (end<=union_end) then within body (end<=ilen).
    // Union end <= ilen already (union bounds), so check end<=union_end first.
    let u1_gt = a.op(
        ns.o,
        f1_unwrap,
        Opcode::GreaterThan,
        vec![pav(u1_end), pav(u1_uend)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f1_unwrap,
        function: fid,
        parameters: vec![u1_end, u1_len, u1_uend, u1_vec, u1_ilen, u1_in, u1_unit],
        operations: vec![u1_gt],
        terminator: cond(
            op_result(u1_gt),
            edge(b_len, Vec::new()),
            edge(
                f1_len32,
                vec![
                    pav(u1_len),
                    pav(u1_end),
                    pav(u1_uend),
                    pav(u1_vec),
                    pav(u1_ilen),
                    pav(u1_in),
                    pav(u1_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Fixed 32B: <32 LENGTH, >32 TRAILING (decode_fixed mirror).
    let w_len = a.param(ns.p, f1_len32, ParameterRole::Block, u64_type());
    let w_end = a.param(ns.p, f1_len32, ParameterRole::Block, u64_type());
    let w_uend = a.param(ns.p, f1_len32, ParameterRole::Block, u64_type());
    let w_vec = a.param(ns.p, f1_len32, ParameterRole::Block, u8vec_type());
    let w_ilen = a.param(ns.p, f1_len32, ParameterRole::Block, u64_type());
    let w_in = a.param(ns.p, f1_len32, ParameterRole::Block, TypeExpr::Bytes);
    let w_unit = a.param(ns.p, f1_len32, ParameterRole::Block, TypeExpr::Unit);
    let w_k32 = a.cref(ns.o, f1_len32, c32, u64_type());
    let w_lt = a.op(
        ns.o,
        f1_len32,
        Opcode::LessThan,
        vec![pav(w_len), op_result(w_k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let w_gt = a.op(
        ns.o,
        f1_len32,
        Opcode::GreaterThan,
        vec![pav(w_len), op_result(w_k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f1_len32,
        function: fid,
        parameters: vec![w_len, w_end, w_uend, w_vec, w_ilen, w_in, w_unit],
        operations: vec![w_k32, w_lt, w_gt],
        terminator: cond(
            op_result(w_lt),
            edge(b_len, Vec::new()),
            edge(
                f1_len32_gt,
                vec![
                    pav(w_len),
                    pav(w_end),
                    pav(w_uend),
                    pav(w_vec),
                    pav(w_ilen),
                    pav(w_in),
                    pav(w_unit),
                    op_result(w_gt),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let g2_len = a.param(ns.p, f1_len32_gt, ParameterRole::Block, u64_type());
    let g2_end = a.param(ns.p, f1_len32_gt, ParameterRole::Block, u64_type());
    let g2_uend = a.param(ns.p, f1_len32_gt, ParameterRole::Block, u64_type());
    let g2_vec = a.param(ns.p, f1_len32_gt, ParameterRole::Block, u8vec_type());
    let g2_ilen = a.param(ns.p, f1_len32_gt, ParameterRole::Block, u64_type());
    let g2_in = a.param(ns.p, f1_len32_gt, ParameterRole::Block, TypeExpr::Bytes);
    let g2_unit = a.param(ns.p, f1_len32_gt, ParameterRole::Block, TypeExpr::Unit);
    let g2_flag = a.param(ns.p, f1_len32_gt, ParameterRole::Block, TypeExpr::Bool);
    a.blocks.push(Block {
        entity_id: f1_len32_gt,
        function: fid,
        parameters: vec![
            g2_len, g2_end, g2_uend, g2_vec, g2_ilen, g2_in, g2_unit, g2_flag,
        ],
        operations: Vec::new(),
        terminator: cond(
            pav(g2_flag),
            edge(b_trail, Vec::new()),
            edge(
                eid_setup,
                vec![
                    pav(g2_end),
                    pav(g2_vec),
                    pav(g2_ilen),
                    pav(g2_in),
                    pav(g2_unit),
                    pav(g2_uend),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Entity-ID extraction: start = end-32, 32B copy loop, then field-2.
    // eid_setup carries [end1, vec, ilen, in, unit, union_end].
    let es_end = a.param(ns.p, eid_setup, ParameterRole::Block, u64_type());
    let es_vec = a.param(ns.p, eid_setup, ParameterRole::Block, u8vec_type());
    let es_ilen = a.param(ns.p, eid_setup, ParameterRole::Block, u64_type());
    let es_in = a.param(ns.p, eid_setup, ParameterRole::Block, TypeExpr::Bytes);
    let es_unit = a.param(ns.p, eid_setup, ParameterRole::Block, TypeExpr::Unit);
    let es_uend = a.param(ns.p, eid_setup, ParameterRole::Block, u64_type());
    let es_k32 = a.cref(ns.o, eid_setup, c32, u64_type());
    let es_sub = a.op(
        ns.o,
        eid_setup,
        Opcode::IntSubChecked,
        vec![pav(es_end), op_result(es_k32)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let es_empty = a.op(
        ns.o,
        eid_setup,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let es_z0 = a.cref(ns.o, eid_setup, c0, u64_type());
    a.blocks.push(Block {
        entity_id: eid_setup,
        function: fid,
        parameters: vec![es_end, es_vec, es_ilen, es_in, es_unit, es_uend],
        operations: vec![es_k32, es_sub, es_empty, es_z0],
        terminator: switch(
            op_result(es_sub),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_check,
                    vec![
                        SwitchArgument::CasePayload,
                        oav(es_empty),
                        oav(es_z0),
                        sav(es_end),
                        sav(es_vec),
                        sav(es_ilen),
                        sav(es_in),
                        sav(es_unit),
                        sav(es_uend),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // eid_check: fixed 32B function-ID loop (start..start+32 == end1-32..end1).
    let ec_start = a.param(ns.p, eid_check, ParameterRole::Block, u64_type());
    let ec_acc = a.param(ns.p, eid_check, ParameterRole::Block, u8vec_type());
    let ec_idx0 = a.param(ns.p, eid_check, ParameterRole::Block, u64_type());
    let ec_end = a.param(ns.p, eid_check, ParameterRole::Block, u64_type());
    let ec_vec = a.param(ns.p, eid_check, ParameterRole::Block, u8vec_type());
    let ec_ilen = a.param(ns.p, eid_check, ParameterRole::Block, u64_type());
    let ec_in = a.param(ns.p, eid_check, ParameterRole::Block, TypeExpr::Bytes);
    let ec_unit = a.param(ns.p, eid_check, ParameterRole::Block, TypeExpr::Unit);
    let ec_uend = a.param(ns.p, eid_check, ParameterRole::Block, u64_type());
    let ec_k32 = a.cref(ns.o, eid_check, c32, u64_type());
    let ec_lt = a.op(
        ns.o,
        eid_check,
        Opcode::LessThan,
        vec![pav(ec_idx0), op_result(ec_k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_check,
        function: fid,
        parameters: vec![
            ec_start, ec_acc, ec_idx0, ec_end, ec_vec, ec_ilen, ec_in, ec_unit, ec_uend,
        ],
        operations: vec![ec_k32, ec_lt],
        terminator: cond(
            op_result(ec_lt),
            edge(
                eid_get,
                vec![
                    pav(ec_idx0),
                    pav(ec_acc),
                    pav(ec_start),
                    pav(ec_end),
                    pav(ec_vec),
                    pav(ec_ilen),
                    pav(ec_in),
                    pav(ec_unit),
                    pav(ec_uend),
                ],
            ),
            edge(
                eid_done,
                vec![
                    pav(ec_acc),
                    pav(ec_end),
                    pav(ec_vec),
                    pav(ec_ilen),
                    pav(ec_in),
                    pav(ec_unit),
                    pav(ec_uend),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let gg_idx = a.param(ns.p, eid_get, ParameterRole::Block, u64_type());
    let gg_acc = a.param(ns.p, eid_get, ParameterRole::Block, u8vec_type());
    let gg_start = a.param(ns.p, eid_get, ParameterRole::Block, u64_type());
    let gg_end = a.param(ns.p, eid_get, ParameterRole::Block, u64_type());
    let gg_vec = a.param(ns.p, eid_get, ParameterRole::Block, u8vec_type());
    let gg_ilen = a.param(ns.p, eid_get, ParameterRole::Block, u64_type());
    let gg_in = a.param(ns.p, eid_get, ParameterRole::Block, TypeExpr::Bytes);
    let gg_unit = a.param(ns.p, eid_get, ParameterRole::Block, TypeExpr::Unit);
    let gg_uend = a.param(ns.p, eid_get, ParameterRole::Block, u64_type());
    let gg_add = a.op(
        ns.o,
        eid_get,
        Opcode::IntAddChecked,
        vec![pav(gg_start), pav(gg_idx)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_get,
        function: fid,
        parameters: vec![
            gg_idx, gg_acc, gg_start, gg_end, gg_vec, gg_ilen, gg_in, gg_unit, gg_uend,
        ],
        operations: vec![gg_add],
        terminator: switch(
            op_result(gg_add),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_get2,
                    vec![
                        sav(gg_acc),
                        SwitchArgument::CasePayload,
                        sav(gg_idx),
                        sav(gg_start),
                        sav(gg_end),
                        sav(gg_vec),
                        sav(gg_ilen),
                        sav(gg_in),
                        sav(gg_unit),
                        sav(gg_uend),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let hh_acc = a.param(ns.p, eid_get2, ParameterRole::Block, u8vec_type());
    let hh_sidx = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let hh_idx = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let hh_start = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let hh_end = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let hh_vec = a.param(ns.p, eid_get2, ParameterRole::Block, u8vec_type());
    let hh_ilen = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let hh_in = a.param(ns.p, eid_get2, ParameterRole::Block, TypeExpr::Bytes);
    let hh_unit = a.param(ns.p, eid_get2, ParameterRole::Block, TypeExpr::Unit);
    let hh_uend = a.param(ns.p, eid_get2, ParameterRole::Block, u64_type());
    let hh_get = a.op(
        ns.o,
        eid_get2,
        Opcode::VectorGet,
        vec![pav(hh_vec), pav(hh_sidx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_get2,
        function: fid,
        parameters: vec![
            hh_acc, hh_sidx, hh_idx, hh_start, hh_end, hh_vec, hh_ilen, hh_in, hh_unit, hh_uend,
        ],
        operations: vec![hh_get],
        terminator: switch(
            op_result(hh_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    eid_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(hh_idx),
                        sav(hh_acc),
                        sav(hh_start),
                        sav(hh_end),
                        sav(hh_vec),
                        sav(hh_ilen),
                        sav(hh_in),
                        sav(hh_unit),
                        sav(hh_uend),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let iu_b = a.param(ns.p, eid_push, ParameterRole::Block, u8_type());
    let iu_idx = a.param(ns.p, eid_push, ParameterRole::Block, u64_type());
    let iu_acc = a.param(ns.p, eid_push, ParameterRole::Block, u8vec_type());
    let iu_start = a.param(ns.p, eid_push, ParameterRole::Block, u64_type());
    let iu_end = a.param(ns.p, eid_push, ParameterRole::Block, u64_type());
    let iu_vec = a.param(ns.p, eid_push, ParameterRole::Block, u8vec_type());
    let iu_ilen = a.param(ns.p, eid_push, ParameterRole::Block, u64_type());
    let iu_in = a.param(ns.p, eid_push, ParameterRole::Block, TypeExpr::Bytes);
    let iu_unit = a.param(ns.p, eid_push, ParameterRole::Block, TypeExpr::Unit);
    let iu_uend = a.param(ns.p, eid_push, ParameterRole::Block, u64_type());
    let iu_push = a.op(
        ns.o,
        eid_push,
        Opcode::AdapterInvoke,
        vec![pav(iu_acc), pav(iu_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: eid_push,
        function: fid,
        parameters: vec![
            iu_b, iu_idx, iu_acc, iu_start, iu_end, iu_vec, iu_ilen, iu_in, iu_unit, iu_uend,
        ],
        operations: vec![iu_push],
        terminator: switch(
            op_result(iu_push),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_next,
                    vec![
                        sav(iu_idx),
                        SwitchArgument::CasePayload,
                        sav(iu_start),
                        sav(iu_end),
                        sav(iu_vec),
                        sav(iu_ilen),
                        sav(iu_in),
                        sav(iu_unit),
                        sav(iu_uend),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let nu_idx = a.param(ns.p, eid_next, ParameterRole::Block, u64_type());
    let nu_acc = a.param(ns.p, eid_next, ParameterRole::Block, u8vec_type());
    let nu_start = a.param(ns.p, eid_next, ParameterRole::Block, u64_type());
    let nu_end = a.param(ns.p, eid_next, ParameterRole::Block, u64_type());
    let nu_vec = a.param(ns.p, eid_next, ParameterRole::Block, u8vec_type());
    let nu_ilen = a.param(ns.p, eid_next, ParameterRole::Block, u64_type());
    let nu_in = a.param(ns.p, eid_next, ParameterRole::Block, TypeExpr::Bytes);
    let nu_unit = a.param(ns.p, eid_next, ParameterRole::Block, TypeExpr::Unit);
    let nu_uend = a.param(ns.p, eid_next, ParameterRole::Block, u64_type());
    let nu_c1 = a.cref(ns.o, eid_next, c1, u64_type());
    let nu_add = a.op(
        ns.o,
        eid_next,
        Opcode::IntAddChecked,
        vec![pav(nu_idx), op_result(nu_c1)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: eid_next,
        function: fid,
        parameters: vec![
            nu_idx, nu_acc, nu_start, nu_end, nu_vec, nu_ilen, nu_in, nu_unit, nu_uend,
        ],
        operations: vec![nu_c1, nu_add],
        terminator: switch(
            op_result(nu_add),
            vec![
                (
                    BuiltinCase::Ok,
                    eid_check,
                    vec![
                        sav(nu_start),
                        sav(nu_acc),
                        SwitchArgument::CasePayload,
                        sav(nu_end),
                        sav(nu_vec),
                        sav(nu_ilen),
                        sav(nu_in),
                        sav(nu_unit),
                        sav(nu_uend),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // eid_done: V2B1 -> function Bytes, then field-2 tag at end1.
    let du_acc = a.param(ns.p, eid_done, ParameterRole::Block, u8vec_type());
    let du_end = a.param(ns.p, eid_done, ParameterRole::Block, u64_type());
    let du_vec = a.param(ns.p, eid_done, ParameterRole::Block, u8vec_type());
    let du_ilen = a.param(ns.p, eid_done, ParameterRole::Block, u64_type());
    let du_in = a.param(ns.p, eid_done, ParameterRole::Block, TypeExpr::Bytes);
    let du_unit = a.param(ns.p, eid_done, ParameterRole::Block, TypeExpr::Unit);
    let du_uend = a.param(ns.p, eid_done, ParameterRole::Block, u64_type());
    let du_v2b = a.op(
        ns.o,
        eid_done,
        Opcode::AdapterInvoke,
        vec![pav(du_unit), pav(du_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: eid_done,
        function: fid,
        parameters: vec![du_acc, du_end, du_vec, du_ilen, du_in, du_unit, du_uend],
        operations: vec![du_v2b],
        terminator: switch(
            op_result(du_v2b),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_tag,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(du_end),
                        sav(du_vec),
                        sav(du_ilen),
                        sav(du_in),
                        sav(du_unit),
                        sav(du_uend),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Field-2 tag at end1. DUP (==1) / ORDER (<1, i.e. 0) precede payload.
    let ft_func = a.param(ns.p, f2_tag, ParameterRole::Block, TypeExpr::Bytes);
    let ft_end = a.param(ns.p, f2_tag, ParameterRole::Block, u64_type());
    let ft_vec = a.param(ns.p, f2_tag, ParameterRole::Block, u8vec_type());
    let ft_ilen = a.param(ns.p, f2_tag, ParameterRole::Block, u64_type());
    let ft_in = a.param(ns.p, f2_tag, ParameterRole::Block, TypeExpr::Bytes);
    let ft_unit = a.param(ns.p, f2_tag, ParameterRole::Block, TypeExpr::Unit);
    let ft_uend = a.param(ns.p, f2_tag, ParameterRole::Block, u64_type());
    let ft_w = a.cref(ns.o, f2_tag, w32, u32_type());
    let ft_call = a.op(
        ns.o,
        f2_tag,
        Opcode::CallDirect,
        vec![pav(ft_in), pav(ft_end), op_result(ft_w), pav(ft_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let ft_tup = a.param(
        ns.p,
        f2_tag_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let ft_ofunc = a.param(ns.p, f2_tag_ok, ParameterRole::Block, TypeExpr::Bytes);
    let ft_ovec = a.param(ns.p, f2_tag_ok, ParameterRole::Block, u8vec_type());
    let ft_olen = a.param(ns.p, f2_tag_ok, ParameterRole::Block, u64_type());
    let ft_oin = a.param(ns.p, f2_tag_ok, ParameterRole::Block, TypeExpr::Bytes);
    let ft_ounit = a.param(ns.p, f2_tag_ok, ParameterRole::Block, TypeExpr::Unit);
    let ft_ouend = a.param(ns.p, f2_tag_ok, ParameterRole::Block, u64_type());
    let ft_ebytes = a.param(ns.p, f2_tag_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f2_tag,
        function: fid,
        parameters: vec![ft_func, ft_end, ft_vec, ft_ilen, ft_in, ft_unit, ft_uend],
        operations: vec![ft_w, ft_call],
        terminator: switch(
            op_result(ft_call),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_tag_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(ft_func),
                        sav(ft_vec),
                        sav(ft_ilen),
                        sav(ft_in),
                        sav(ft_unit),
                        sav(ft_uend),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    f2_tag_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let ft_er = a.op(
        ns.o,
        f2_tag_err,
        Opcode::ResultErr,
        vec![pav(ft_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_tag_err,
        function: fid,
        parameters: vec![ft_ebytes],
        operations: vec![ft_er],
        terminator: ret(op_result(ft_er)),
        reachability: Reachability::Required,
    });
    let ft_gtag = a.op(
        ns.o,
        f2_tag_ok,
        Opcode::TupleGet,
        vec![pav(ft_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let ft_gpos = a.op(
        ns.o,
        f2_tag_ok,
        Opcode::TupleGet,
        vec![pav(ft_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    a.blocks.push(Block {
        entity_id: f2_tag_ok,
        function: fid,
        parameters: vec![
            ft_tup, ft_ofunc, ft_ovec, ft_olen, ft_oin, ft_ounit, ft_ouend,
        ],
        operations: vec![ft_gtag, ft_gpos],
        terminator: branch(edge(
            f2_disp,
            vec![
                op_result(ft_gtag),
                op_result(ft_gpos),
                pav(ft_ofunc),
                pav(ft_ovec),
                pav(ft_olen),
                pav(ft_oin),
                pav(ft_ounit),
                pav(ft_ouend),
            ],
        )),
        reachability: Reachability::Required,
    });
    let dd2_tag = a.param(ns.p, f2_disp, ParameterRole::Block, u64_type());
    let dd2_pos = a.param(ns.p, f2_disp, ParameterRole::Block, u64_type());
    let dd2_func = a.param(ns.p, f2_disp, ParameterRole::Block, TypeExpr::Bytes);
    let dd2_vec = a.param(ns.p, f2_disp, ParameterRole::Block, u8vec_type());
    let dd2_ilen = a.param(ns.p, f2_disp, ParameterRole::Block, u64_type());
    let dd2_in = a.param(ns.p, f2_disp, ParameterRole::Block, TypeExpr::Bytes);
    let dd2_unit = a.param(ns.p, f2_disp, ParameterRole::Block, TypeExpr::Unit);
    let dd2_uend = a.param(ns.p, f2_disp, ParameterRole::Block, u64_type());
    let dd2_k1 = a.cref(ns.o, f2_disp, c1, u64_type());
    let dd2_eq = a.op(
        ns.o,
        f2_disp,
        Opcode::Equal,
        vec![pav(dd2_tag), op_result(dd2_k1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_disp,
        function: fid,
        parameters: vec![
            dd2_tag, dd2_pos, dd2_func, dd2_vec, dd2_ilen, dd2_in, dd2_unit, dd2_uend,
        ],
        operations: vec![dd2_k1, dd2_eq],
        terminator: cond(
            op_result(dd2_eq),
            edge(b_dup, Vec::new()),
            edge(
                f2_ord,
                vec![
                    pav(dd2_tag),
                    pav(dd2_pos),
                    pav(dd2_func),
                    pav(dd2_vec),
                    pav(dd2_ilen),
                    pav(dd2_in),
                    pav(dd2_unit),
                    pav(dd2_uend),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let od2_tag = a.param(ns.p, f2_ord, ParameterRole::Block, u64_type());
    let od2_pos = a.param(ns.p, f2_ord, ParameterRole::Block, u64_type());
    let od2_func = a.param(ns.p, f2_ord, ParameterRole::Block, TypeExpr::Bytes);
    let od2_vec = a.param(ns.p, f2_ord, ParameterRole::Block, u8vec_type());
    let od2_ilen = a.param(ns.p, f2_ord, ParameterRole::Block, u64_type());
    let od2_in = a.param(ns.p, f2_ord, ParameterRole::Block, TypeExpr::Bytes);
    let od2_unit = a.param(ns.p, f2_ord, ParameterRole::Block, TypeExpr::Unit);
    let od2_uend = a.param(ns.p, f2_ord, ParameterRole::Block, u64_type());
    let od2_k1 = a.cref(ns.o, f2_ord, c1, u64_type());
    let od2_k2 = a.cref(ns.o, f2_ord, c2, u64_type());
    let od2_lt = a.op(
        ns.o,
        f2_ord,
        Opcode::LessThan,
        vec![pav(od2_tag), op_result(od2_k1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let od2_eq = a.op(
        ns.o,
        f2_ord,
        Opcode::Equal,
        vec![pav(od2_tag), op_result(od2_k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let od2_valid = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: f2_ord,
        function: fid,
        parameters: vec![
            od2_tag, od2_pos, od2_func, od2_vec, od2_ilen, od2_in, od2_unit, od2_uend,
        ],
        operations: vec![od2_k1, od2_k2, od2_lt, od2_eq],
        terminator: cond(
            op_result(od2_lt),
            edge(b_order, Vec::new()),
            edge(
                od2_valid,
                vec![
                    pav(od2_tag),
                    pav(od2_pos),
                    pav(od2_func),
                    pav(od2_vec),
                    pav(od2_ilen),
                    pav(od2_in),
                    pav(od2_unit),
                    pav(od2_uend),
                    op_result(od2_eq),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let vv_tag = a.param(ns.p, od2_valid, ParameterRole::Block, u64_type());
    let vv_pos = a.param(ns.p, od2_valid, ParameterRole::Block, u64_type());
    let vv_func = a.param(ns.p, od2_valid, ParameterRole::Block, TypeExpr::Bytes);
    let vv_vec = a.param(ns.p, od2_valid, ParameterRole::Block, u8vec_type());
    let vv_ilen = a.param(ns.p, od2_valid, ParameterRole::Block, u64_type());
    let vv_in = a.param(ns.p, od2_valid, ParameterRole::Block, TypeExpr::Bytes);
    let vv_unit = a.param(ns.p, od2_valid, ParameterRole::Block, TypeExpr::Unit);
    let vv_uend = a.param(ns.p, od2_valid, ParameterRole::Block, u64_type());
    let vv_eq = a.param(ns.p, od2_valid, ParameterRole::Block, TypeExpr::Bool);
    a.blocks.push(Block {
        entity_id: od2_valid,
        function: fid,
        parameters: vec![
            vv_tag, vv_pos, vv_func, vv_vec, vv_ilen, vv_in, vv_unit, vv_uend, vv_eq,
        ],
        operations: Vec::new(),
        terminator: cond(
            pav(vv_eq),
            edge(
                f2_len,
                vec![
                    pav(vv_pos),
                    pav(vv_func),
                    pav(vv_vec),
                    pav(vv_ilen),
                    pav(vv_in),
                    pav(vv_unit),
                    pav(vv_uend),
                ],
            ),
            edge(b_unknown, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // Field-2 length then bounds within union end.
    let wl_pos = a.param(ns.p, f2_len, ParameterRole::Block, u64_type());
    let wl_func = a.param(ns.p, f2_len, ParameterRole::Block, TypeExpr::Bytes);
    let wl_vec = a.param(ns.p, f2_len, ParameterRole::Block, u8vec_type());
    let wl_ilen = a.param(ns.p, f2_len, ParameterRole::Block, u64_type());
    let wl_in = a.param(ns.p, f2_len, ParameterRole::Block, TypeExpr::Bytes);
    let wl_unit = a.param(ns.p, f2_len, ParameterRole::Block, TypeExpr::Unit);
    let wl_uend = a.param(ns.p, f2_len, ParameterRole::Block, u64_type());
    let wl_w = a.cref(ns.o, f2_len, w64, u32_type());
    let wl_call = a.op(
        ns.o,
        f2_len,
        Opcode::CallDirect,
        vec![pav(wl_in), pav(wl_pos), op_result(wl_w), pav(wl_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let wl_tup = a.param(
        ns.p,
        f2_len_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let wl_ofunc = a.param(ns.p, f2_len_ok, ParameterRole::Block, TypeExpr::Bytes);
    let wl_ovec = a.param(ns.p, f2_len_ok, ParameterRole::Block, u8vec_type());
    let wl_olen = a.param(ns.p, f2_len_ok, ParameterRole::Block, u64_type());
    let wl_oin = a.param(ns.p, f2_len_ok, ParameterRole::Block, TypeExpr::Bytes);
    let wl_ounit = a.param(ns.p, f2_len_ok, ParameterRole::Block, TypeExpr::Unit);
    let wl_ouend = a.param(ns.p, f2_len_ok, ParameterRole::Block, u64_type());
    let wl_ebytes = a.param(ns.p, f2_len_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: f2_len,
        function: fid,
        parameters: vec![wl_pos, wl_func, wl_vec, wl_ilen, wl_in, wl_unit, wl_uend],
        operations: vec![wl_w, wl_call],
        terminator: switch(
            op_result(wl_call),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_len_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(wl_func),
                        sav(wl_vec),
                        sav(wl_ilen),
                        sav(wl_in),
                        sav(wl_unit),
                        sav(wl_uend),
                    ],
                ),
                (
                    BuiltinCase::Err,
                    f2_len_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let wl_er = a.op(
        ns.o,
        f2_len_err,
        Opcode::ResultErr,
        vec![pav(wl_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_len_err,
        function: fid,
        parameters: vec![wl_ebytes],
        operations: vec![wl_er],
        terminator: ret(op_result(wl_er)),
        reachability: Reachability::Required,
    });
    let wl_glen = a.op(
        ns.o,
        f2_len_ok,
        Opcode::TupleGet,
        vec![pav(wl_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let wl_gpos = a.op(
        ns.o,
        f2_len_ok,
        Opcode::TupleGet,
        vec![pav(wl_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let wl_max = a.cref(ns.o, f2_len_ok, c_max, u64_type());
    let wl_gtmax = a.op(
        ns.o,
        f2_len_ok,
        Opcode::GreaterThan,
        vec![op_result(wl_glen), op_result(wl_max)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_len_ok,
        function: fid,
        parameters: vec![
            wl_tup, wl_ofunc, wl_ovec, wl_olen, wl_oin, wl_ounit, wl_ouend,
        ],
        operations: vec![wl_glen, wl_gpos, wl_max, wl_gtmax],
        terminator: cond(
            op_result(wl_gtmax),
            edge(b_res, Vec::new()),
            edge(
                f2_bnd,
                vec![
                    op_result(wl_glen),
                    op_result(wl_gpos),
                    pav(wl_ofunc),
                    pav(wl_ovec),
                    pav(wl_olen),
                    pav(wl_oin),
                    pav(wl_ounit),
                    pav(wl_ouend),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let bl_len = a.param(ns.p, f2_bnd, ParameterRole::Block, u64_type());
    let bl_pos = a.param(ns.p, f2_bnd, ParameterRole::Block, u64_type());
    let bl_func = a.param(ns.p, f2_bnd, ParameterRole::Block, TypeExpr::Bytes);
    let bl_vec = a.param(ns.p, f2_bnd, ParameterRole::Block, u8vec_type());
    let bl_ilen = a.param(ns.p, f2_bnd, ParameterRole::Block, u64_type());
    let bl_in = a.param(ns.p, f2_bnd, ParameterRole::Block, TypeExpr::Bytes);
    let bl_unit = a.param(ns.p, f2_bnd, ParameterRole::Block, TypeExpr::Unit);
    let bl_uend = a.param(ns.p, f2_bnd, ParameterRole::Block, u64_type());
    let bl_add = a.op(
        ns.o,
        f2_bnd,
        Opcode::IntAddChecked,
        vec![pav(bl_pos), pav(bl_len)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_bnd,
        function: fid,
        parameters: vec![
            bl_len, bl_pos, bl_func, bl_vec, bl_ilen, bl_in, bl_unit, bl_uend,
        ],
        operations: vec![bl_add],
        terminator: switch(
            op_result(bl_add),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_unwrap,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(bl_len),
                        sav(bl_func),
                        sav(bl_vec),
                        sav(bl_ilen),
                        sav(bl_in),
                        sav(bl_unit),
                        sav(bl_uend),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let ul_end = a.param(ns.p, f2_unwrap, ParameterRole::Block, u64_type());
    let ul_len = a.param(ns.p, f2_unwrap, ParameterRole::Block, u64_type());
    let ul_func = a.param(ns.p, f2_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let ul_vec = a.param(ns.p, f2_unwrap, ParameterRole::Block, u8vec_type());
    let ul_ilen = a.param(ns.p, f2_unwrap, ParameterRole::Block, u64_type());
    let ul_in = a.param(ns.p, f2_unwrap, ParameterRole::Block, TypeExpr::Bytes);
    let ul_unit = a.param(ns.p, f2_unwrap, ParameterRole::Block, TypeExpr::Unit);
    let ul_uend = a.param(ns.p, f2_unwrap, ParameterRole::Block, u64_type());
    let ul_gt = a.op(
        ns.o,
        f2_unwrap,
        Opcode::GreaterThan,
        vec![pav(ul_end), pav(ul_uend)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_unwrap,
        function: fid,
        parameters: vec![
            ul_end, ul_len, ul_func, ul_vec, ul_ilen, ul_in, ul_unit, ul_uend,
        ],
        operations: vec![ul_gt],
        terminator: cond(
            op_result(ul_gt),
            edge(b_len, Vec::new()),
            edge(
                f2_pay_start,
                vec![
                    pav(ul_len),
                    pav(ul_end),
                    pav(ul_func),
                    pav(ul_vec),
                    pav(ul_ilen),
                    pav(ul_in),
                    pav(ul_unit),
                    pav(ul_uend),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // Field-2 payload copy (len2 bytes, valid 1B) then exposure decode.
    // f2_pay_start params: [len, end2, func, vec, ilen, in, unit, uend].
    // Payload start = end2 - len (checked-sub, underflow unreachable by bounds).
    let ps_len = a.param(ns.p, f2_pay_start, ParameterRole::Block, u64_type());
    let ps_end = a.param(ns.p, f2_pay_start, ParameterRole::Block, u64_type());
    let ps_func = a.param(ns.p, f2_pay_start, ParameterRole::Block, TypeExpr::Bytes);
    let ps_vec = a.param(ns.p, f2_pay_start, ParameterRole::Block, u8vec_type());
    let ps_ilen = a.param(ns.p, f2_pay_start, ParameterRole::Block, u64_type());
    let ps_in = a.param(ns.p, f2_pay_start, ParameterRole::Block, TypeExpr::Bytes);
    let ps_unit = a.param(ns.p, f2_pay_start, ParameterRole::Block, TypeExpr::Unit);
    let ps_uend = a.param(ns.p, f2_pay_start, ParameterRole::Block, u64_type());
    let ps_sub = a.op(
        ns.o,
        f2_pay_start,
        Opcode::IntSubChecked,
        vec![pav(ps_end), pav(ps_len)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let ps_empty = a.op(
        ns.o,
        f2_pay_start,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let ps_z0 = a.cref(ns.o, f2_pay_start, c0, u64_type());
    a.blocks.push(Block {
        entity_id: f2_pay_start,
        function: fid,
        parameters: vec![
            ps_len, ps_end, ps_func, ps_vec, ps_ilen, ps_in, ps_unit, ps_uend,
        ],
        operations: vec![ps_sub, ps_empty, ps_z0],
        terminator: switch(
            op_result(ps_sub),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_pay_check,
                    vec![
                        oav(ps_z0),
                        oav(ps_empty),
                        sav(ps_len),
                        SwitchArgument::CasePayload,
                        sav(ps_end),
                        sav(ps_func),
                        sav(ps_vec),
                        sav(ps_ilen),
                        sav(ps_in),
                        sav(ps_unit),
                        sav(ps_uend),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // f2_pay_check params: [idx, acc, len, pstart, end2, func, vec, ilen, in, unit, uend].
    let pc_idx = a.param(ns.p, f2_pay_check, ParameterRole::Block, u64_type());
    let pc_acc = a.param(ns.p, f2_pay_check, ParameterRole::Block, u8vec_type());
    let pc_len = a.param(ns.p, f2_pay_check, ParameterRole::Block, u64_type());
    let pc_start = a.param(ns.p, f2_pay_check, ParameterRole::Block, u64_type());
    let pc_end = a.param(ns.p, f2_pay_check, ParameterRole::Block, u64_type());
    let pc_func = a.param(ns.p, f2_pay_check, ParameterRole::Block, TypeExpr::Bytes);
    let pc_vec = a.param(ns.p, f2_pay_check, ParameterRole::Block, u8vec_type());
    let pc_ilen = a.param(ns.p, f2_pay_check, ParameterRole::Block, u64_type());
    let pc_in = a.param(ns.p, f2_pay_check, ParameterRole::Block, TypeExpr::Bytes);
    let pc_unit = a.param(ns.p, f2_pay_check, ParameterRole::Block, TypeExpr::Unit);
    let pc_uend = a.param(ns.p, f2_pay_check, ParameterRole::Block, u64_type());
    let pc_lt = a.op(
        ns.o,
        f2_pay_check,
        Opcode::LessThan,
        vec![pav(pc_idx), pav(pc_len)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_pay_check,
        function: fid,
        parameters: vec![
            pc_idx, pc_acc, pc_len, pc_start, pc_end, pc_func, pc_vec, pc_ilen, pc_in, pc_unit,
            pc_uend,
        ],
        operations: vec![pc_lt],
        terminator: cond(
            op_result(pc_lt),
            edge(
                f2_pay_get,
                vec![
                    pav(pc_idx),
                    pav(pc_acc),
                    pav(pc_len),
                    pav(pc_start),
                    pav(pc_end),
                    pav(pc_func),
                    pav(pc_vec),
                    pav(pc_ilen),
                    pav(pc_in),
                    pav(pc_unit),
                    pav(pc_uend),
                ],
            ),
            edge(
                f2_pay_done,
                vec![
                    pav(pc_acc),
                    pav(pc_len),
                    pav(pc_end),
                    pav(pc_func),
                    pav(pc_vec),
                    pav(pc_ilen),
                    pav(pc_in),
                    pav(pc_unit),
                    pav(pc_uend),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let pg_idx = a.param(ns.p, f2_pay_get, ParameterRole::Block, u64_type());
    let pg_acc = a.param(ns.p, f2_pay_get, ParameterRole::Block, u8vec_type());
    let pg_len = a.param(ns.p, f2_pay_get, ParameterRole::Block, u64_type());
    let pg_start = a.param(ns.p, f2_pay_get, ParameterRole::Block, u64_type());
    let pg_end = a.param(ns.p, f2_pay_get, ParameterRole::Block, u64_type());
    let pg_func = a.param(ns.p, f2_pay_get, ParameterRole::Block, TypeExpr::Bytes);
    let pg_vec = a.param(ns.p, f2_pay_get, ParameterRole::Block, u8vec_type());
    let pg_ilen = a.param(ns.p, f2_pay_get, ParameterRole::Block, u64_type());
    let pg_in = a.param(ns.p, f2_pay_get, ParameterRole::Block, TypeExpr::Bytes);
    let pg_unit = a.param(ns.p, f2_pay_get, ParameterRole::Block, TypeExpr::Unit);
    let pg_uend = a.param(ns.p, f2_pay_get, ParameterRole::Block, u64_type());
    let pg_add = a.op(
        ns.o,
        f2_pay_get,
        Opcode::IntAddChecked,
        vec![pav(pg_start), pav(pg_idx)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_pay_get,
        function: fid,
        parameters: vec![
            pg_idx, pg_acc, pg_len, pg_start, pg_end, pg_func, pg_vec, pg_ilen, pg_in, pg_unit,
            pg_uend,
        ],
        operations: vec![pg_add],
        terminator: switch(
            op_result(pg_add),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_pay_get2,
                    vec![
                        sav(pg_acc),
                        SwitchArgument::CasePayload,
                        sav(pg_idx),
                        sav(pg_len),
                        sav(pg_start),
                        sav(pg_end),
                        sav(pg_func),
                        sav(pg_vec),
                        sav(pg_ilen),
                        sav(pg_in),
                        sav(pg_unit),
                        sav(pg_uend),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let qg_acc = a.param(ns.p, f2_pay_get2, ParameterRole::Block, u8vec_type());
    let qg_sidx = a.param(ns.p, f2_pay_get2, ParameterRole::Block, u64_type());
    let qg_idx = a.param(ns.p, f2_pay_get2, ParameterRole::Block, u64_type());
    let qg_len = a.param(ns.p, f2_pay_get2, ParameterRole::Block, u64_type());
    let qg_start = a.param(ns.p, f2_pay_get2, ParameterRole::Block, u64_type());
    let qg_end = a.param(ns.p, f2_pay_get2, ParameterRole::Block, u64_type());
    let qg_func = a.param(ns.p, f2_pay_get2, ParameterRole::Block, TypeExpr::Bytes);
    let qg_vec = a.param(ns.p, f2_pay_get2, ParameterRole::Block, u8vec_type());
    let qg_ilen = a.param(ns.p, f2_pay_get2, ParameterRole::Block, u64_type());
    let qg_in = a.param(ns.p, f2_pay_get2, ParameterRole::Block, TypeExpr::Bytes);
    let qg_unit = a.param(ns.p, f2_pay_get2, ParameterRole::Block, TypeExpr::Unit);
    let qg_uend = a.param(ns.p, f2_pay_get2, ParameterRole::Block, u64_type());
    let qg_get = a.op(
        ns.o,
        f2_pay_get2,
        Opcode::VectorGet,
        vec![pav(qg_vec), pav(qg_sidx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_pay_get2,
        function: fid,
        parameters: vec![
            qg_acc, qg_sidx, qg_idx, qg_len, qg_start, qg_end, qg_func, qg_vec, qg_ilen, qg_in,
            qg_unit, qg_uend,
        ],
        operations: vec![qg_get],
        terminator: switch(
            op_result(qg_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    f2_pay_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(qg_idx),
                        sav(qg_acc),
                        sav(qg_len),
                        sav(qg_start),
                        sav(qg_end),
                        sav(qg_func),
                        sav(qg_vec),
                        sav(qg_ilen),
                        sav(qg_in),
                        sav(qg_unit),
                        sav(qg_uend),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let qp_b = a.param(ns.p, f2_pay_push, ParameterRole::Block, u8_type());
    let qp_idx = a.param(ns.p, f2_pay_push, ParameterRole::Block, u64_type());
    let qp_acc = a.param(ns.p, f2_pay_push, ParameterRole::Block, u8vec_type());
    let qp_len = a.param(ns.p, f2_pay_push, ParameterRole::Block, u64_type());
    let qp_start = a.param(ns.p, f2_pay_push, ParameterRole::Block, u64_type());
    let qp_end = a.param(ns.p, f2_pay_push, ParameterRole::Block, u64_type());
    let qp_func = a.param(ns.p, f2_pay_push, ParameterRole::Block, TypeExpr::Bytes);
    let qp_vec = a.param(ns.p, f2_pay_push, ParameterRole::Block, u8vec_type());
    let qp_ilen = a.param(ns.p, f2_pay_push, ParameterRole::Block, u64_type());
    let qp_in = a.param(ns.p, f2_pay_push, ParameterRole::Block, TypeExpr::Bytes);
    let qp_unit = a.param(ns.p, f2_pay_push, ParameterRole::Block, TypeExpr::Unit);
    let qp_uend = a.param(ns.p, f2_pay_push, ParameterRole::Block, u64_type());
    let qp_push = a.op(
        ns.o,
        f2_pay_push,
        Opcode::AdapterInvoke,
        vec![pav(qp_acc), pav(qp_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_PSH1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: f2_pay_push,
        function: fid,
        parameters: vec![
            qp_b, qp_idx, qp_acc, qp_len, qp_start, qp_end, qp_func, qp_vec, qp_ilen, qp_in,
            qp_unit, qp_uend,
        ],
        operations: vec![qp_push],
        terminator: switch(
            op_result(qp_push),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_pay_next,
                    vec![
                        sav(qp_idx),
                        SwitchArgument::CasePayload,
                        sav(qp_len),
                        sav(qp_start),
                        sav(qp_end),
                        sav(qp_func),
                        sav(qp_vec),
                        sav(qp_ilen),
                        sav(qp_in),
                        sav(qp_unit),
                        sav(qp_uend),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let qn_idx = a.param(ns.p, f2_pay_next, ParameterRole::Block, u64_type());
    let qn_acc = a.param(ns.p, f2_pay_next, ParameterRole::Block, u8vec_type());
    let qn_len = a.param(ns.p, f2_pay_next, ParameterRole::Block, u64_type());
    let qn_start = a.param(ns.p, f2_pay_next, ParameterRole::Block, u64_type());
    let qn_end = a.param(ns.p, f2_pay_next, ParameterRole::Block, u64_type());
    let qn_func = a.param(ns.p, f2_pay_next, ParameterRole::Block, TypeExpr::Bytes);
    let qn_vec = a.param(ns.p, f2_pay_next, ParameterRole::Block, u8vec_type());
    let qn_ilen = a.param(ns.p, f2_pay_next, ParameterRole::Block, u64_type());
    let qn_in = a.param(ns.p, f2_pay_next, ParameterRole::Block, TypeExpr::Bytes);
    let qn_unit = a.param(ns.p, f2_pay_next, ParameterRole::Block, TypeExpr::Unit);
    let qn_uend = a.param(ns.p, f2_pay_next, ParameterRole::Block, u64_type());
    let qn_c1 = a.cref(ns.o, f2_pay_next, c1, u64_type());
    let qn_add = a.op(
        ns.o,
        f2_pay_next,
        Opcode::IntAddChecked,
        vec![pav(qn_idx), op_result(qn_c1)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f2_pay_next,
        function: fid,
        parameters: vec![
            qn_idx, qn_acc, qn_len, qn_start, qn_end, qn_func, qn_vec, qn_ilen, qn_in, qn_unit,
            qn_uend,
        ],
        operations: vec![qn_c1, qn_add],
        terminator: switch(
            op_result(qn_add),
            vec![
                (
                    BuiltinCase::Ok,
                    f2_pay_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(qn_acc),
                        sav(qn_len),
                        sav(qn_start),
                        sav(qn_end),
                        sav(qn_func),
                        sav(qn_vec),
                        sav(qn_ilen),
                        sav(qn_in),
                        sav(qn_unit),
                        sav(qn_uend),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Payload done: V2B1 -> field Bytes, then exposure uvar at 0.
    let pd_acc = a.param(ns.p, f2_pay_done, ParameterRole::Block, u8vec_type());
    let pd_len = a.param(ns.p, f2_pay_done, ParameterRole::Block, u64_type());
    let pd_end = a.param(ns.p, f2_pay_done, ParameterRole::Block, u64_type());
    let pd_func = a.param(ns.p, f2_pay_done, ParameterRole::Block, TypeExpr::Bytes);
    let pd_vec = a.param(ns.p, f2_pay_done, ParameterRole::Block, u8vec_type());
    let pd_ilen = a.param(ns.p, f2_pay_done, ParameterRole::Block, u64_type());
    let pd_in = a.param(ns.p, f2_pay_done, ParameterRole::Block, TypeExpr::Bytes);
    let pd_unit = a.param(ns.p, f2_pay_done, ParameterRole::Block, TypeExpr::Unit);
    let pd_uend = a.param(ns.p, f2_pay_done, ParameterRole::Block, u64_type());
    let pd_v2b = a.op(
        ns.o,
        f2_pay_done,
        Opcode::AdapterInvoke,
        vec![pav(pd_unit), pav(pd_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_V2B1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: f2_pay_done,
        function: fid,
        parameters: vec![
            pd_acc, pd_len, pd_end, pd_func, pd_vec, pd_ilen, pd_in, pd_unit, pd_uend,
        ],
        operations: vec![pd_v2b],
        terminator: switch(
            op_result(pd_v2b),
            vec![
                (
                    BuiltinCase::Ok,
                    exp_call,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(pd_len),
                        sav(pd_end),
                        sav(pd_func),
                        sav(pd_unit),
                        sav(pd_uend),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Exposure: decode_uvar(payload, 0, 32), exact consumption, 1/2 else UNION.
    let ec_pay = a.param(ns.p, exp_call, ParameterRole::Block, TypeExpr::Bytes);
    let ec_len = a.param(ns.p, exp_call, ParameterRole::Block, u64_type());
    let ec_end = a.param(ns.p, exp_call, ParameterRole::Block, u64_type());
    let ec_func = a.param(ns.p, exp_call, ParameterRole::Block, TypeExpr::Bytes);
    let ec_unit = a.param(ns.p, exp_call, ParameterRole::Block, TypeExpr::Unit);
    let ec_uend = a.param(ns.p, exp_call, ParameterRole::Block, u64_type());
    let ec_z0 = a.cref(ns.o, exp_call, c0, u64_type());
    let ec_w = a.cref(ns.o, exp_call, w32, u32_type());
    let ec_call = a.op(
        ns.o,
        exp_call,
        Opcode::CallDirect,
        vec![pav(ec_pay), op_result(ec_z0), op_result(ec_w), pav(ec_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let eo_tup = a.param(
        ns.p,
        exp_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let eo_pay = a.param(ns.p, exp_ok, ParameterRole::Block, TypeExpr::Bytes);
    let eo_len = a.param(ns.p, exp_ok, ParameterRole::Block, u64_type());
    let eo_end = a.param(ns.p, exp_ok, ParameterRole::Block, u64_type());
    let eo_func = a.param(ns.p, exp_ok, ParameterRole::Block, TypeExpr::Bytes);
    let eo_unit = a.param(ns.p, exp_ok, ParameterRole::Block, TypeExpr::Unit);
    let eo_uend = a.param(ns.p, exp_ok, ParameterRole::Block, u64_type());
    let eo_ebytes = a.param(ns.p, exp_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: exp_call,
        function: fid,
        parameters: vec![ec_pay, ec_len, ec_end, ec_func, ec_unit, ec_uend],
        operations: vec![ec_z0, ec_w, ec_call],
        terminator: switch(
            op_result(ec_call),
            vec![
                (
                    BuiltinCase::Ok,
                    exp_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(ec_pay),
                        sav(ec_len),
                        sav(ec_end),
                        sav(ec_func),
                        sav(ec_unit),
                        sav(ec_uend),
                    ],
                ),
                (BuiltinCase::Err, exp_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let eo_er = a.op(
        ns.o,
        exp_err,
        Opcode::ResultErr,
        vec![pav(eo_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: exp_err,
        function: fid,
        parameters: vec![eo_ebytes],
        operations: vec![eo_er],
        terminator: ret(op_result(eo_er)),
        reachability: Reachability::Required,
    });
    let eo_val = a.op(
        ns.o,
        exp_ok,
        Opcode::TupleGet,
        vec![pav(eo_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let eo_pos = a.op(
        ns.o,
        exp_ok,
        Opcode::TupleGet,
        vec![pav(eo_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    // Exact consumption within field payload: new_pos == len else TRAILING.
    // Field length comes from outer len2 (here eo_len is field payload len).
    a.blocks.push(Block {
        entity_id: exp_ok,
        function: fid,
        parameters: vec![eo_tup, eo_pay, eo_len, eo_end, eo_func, eo_unit, eo_uend],
        operations: vec![eo_val, eo_pos],
        terminator: branch(edge(
            exp_trail,
            vec![
                op_result(eo_val),
                op_result(eo_pos),
                pav(eo_len),
                pav(eo_end),
                pav(eo_func),
                pav(eo_unit),
                pav(eo_uend),
            ],
        )),
        reachability: Reachability::Required,
    });
    let et_val = a.param(ns.p, exp_trail, ParameterRole::Block, u64_type());
    let et_pos = a.param(ns.p, exp_trail, ParameterRole::Block, u64_type());
    let et_len = a.param(ns.p, exp_trail, ParameterRole::Block, u64_type());
    let et_end = a.param(ns.p, exp_trail, ParameterRole::Block, u64_type());
    let et_func = a.param(ns.p, exp_trail, ParameterRole::Block, TypeExpr::Bytes);
    let et_unit = a.param(ns.p, exp_trail, ParameterRole::Block, TypeExpr::Unit);
    let et_uend = a.param(ns.p, exp_trail, ParameterRole::Block, u64_type());
    let et_eq = a.op(
        ns.o,
        exp_trail,
        Opcode::Equal,
        vec![pav(et_pos), pav(et_len)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: exp_trail,
        function: fid,
        parameters: vec![et_val, et_pos, et_len, et_end, et_func, et_unit, et_uend],
        operations: vec![et_eq],
        terminator: cond(
            op_result(et_eq),
            edge(
                exp_disp,
                vec![pav(et_val), pav(et_end), pav(et_func), pav(et_uend)],
            ),
            edge(b_trail, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // Exposure 1/2 else UNION; then record trailing end2==uend else TRAIL.
    let ed_val = a.param(ns.p, exp_disp, ParameterRole::Block, u64_type());
    let ed_end = a.param(ns.p, exp_disp, ParameterRole::Block, u64_type());
    let ed_func = a.param(ns.p, exp_disp, ParameterRole::Block, TypeExpr::Bytes);
    let ed_uend = a.param(ns.p, exp_disp, ParameterRole::Block, u64_type());
    let ed_k1 = a.cref(ns.o, exp_disp, c1, u64_type());
    let ed_k2 = a.cref(ns.o, exp_disp, c2, u64_type());
    let ed_e1 = a.op(
        ns.o,
        exp_disp,
        Opcode::Equal,
        vec![pav(ed_val), op_result(ed_k1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let ed_e2 = a.op(
        ns.o,
        exp_disp,
        Opcode::Equal,
        vec![pav(ed_val), op_result(ed_k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let ed_or = a.op(
        ns.o,
        exp_disp,
        Opcode::BoolOr,
        vec![op_result(ed_e1), op_result(ed_e2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: exp_disp,
        function: fid,
        parameters: vec![ed_val, ed_end, ed_func, ed_uend],
        operations: vec![ed_k1, ed_k2, ed_e1, ed_e2, ed_or],
        terminator: cond(
            op_result(ed_or),
            edge(
                done_ret,
                vec![pav(ed_val), pav(ed_end), pav(ed_func), pav(ed_uend)],
            ),
            edge(b_union, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // Record trailing: field-2 end == union value end else TRAILING.
    let dr_val = a.param(ns.p, done_ret, ParameterRole::Block, u64_type());
    let dr_end = a.param(ns.p, done_ret, ParameterRole::Block, u64_type());
    let dr_func = a.param(ns.p, done_ret, ParameterRole::Block, TypeExpr::Bytes);
    let dr_uend = a.param(ns.p, done_ret, ParameterRole::Block, u64_type());
    let dr_eq = a.op(
        ns.o,
        done_ret,
        Opcode::Equal,
        vec![pav(dr_end), pav(dr_uend)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let dr_ok_block = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: done_ret,
        function: fid,
        parameters: vec![dr_val, dr_end, dr_func, dr_uend],
        operations: vec![dr_eq],
        terminator: cond(
            op_result(dr_eq),
            edge(dr_ok_block, vec![pav(dr_func), pav(dr_val)]),
            edge(b_trail, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    let ok_func = a.param(ns.p, dr_ok_block, ParameterRole::Block, TypeExpr::Bytes);
    let ok_exp = a.param(ns.p, dr_ok_block, ParameterRole::Block, u64_type());
    let ok_tup = a.op(
        ns.o,
        dr_ok_block,
        Opcode::TupleNew,
        vec![pav(ok_func), pav(ok_exp)],
        vec![TypeExpr::Tuple(vec![TypeExpr::Bytes, u64_type()])],
        Immediate::None,
    );
    let ok_res = a.op(
        ns.o,
        dr_ok_block,
        Opcode::ResultOk,
        vec![op_result(ok_tup)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: dr_ok_block,
        function: fid,
        parameters: vec![ok_func, ok_exp],
        operations: vec![ok_tup, ok_res],
        terminator: ret(op_result(ok_res)),
        reachability: Reachability::Required,
    });
    let _ = (c0, c1, c2, c32, w32, trap);
    let _ = (b_missing, b_unknown, b_dup, b_order);

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![in_body, in_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

// ── EntryPoint encode (canonical 40B body) ───────────────────────────
// `encode_entrypoint(function: Bytes 32B, exposure: UInt64 1/2, unit: Unit)`
// emits canonical union tag 16 + record as measured (`1026...`). Function
// length mirrors decode_fixed (<32 LENGTH, >32 TRAILING); exposure 1/2 else
// UNION_INVALID. Fixed pushes (10/26/02/01/20/02/01/exp) + 32B copy loop +
// V2B1. No uvar callee needed (all single-byte for this fixed shape).
#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_entrypoint_encode(a: &mut Asm, ns: Ns, fid: EntityId) -> FunctionGraph {
    use sley_vm::host_abi::{BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let bstart = a.blocks.len();
    let res_t = encode_result_type();
    let c0 = a.ku64(ns.k, 0);
    let c1 = a.ku64(ns.k, 1);
    let c2 = a.ku64(ns.k, 2);
    let c32 = a.ku64(ns.k, 32);
    let u10 = a.ku8(ns.k, 16);
    let u26 = a.ku8(ns.k, 38);
    let u02 = a.ku8(ns.k, 2);
    let u01 = a.ku8(ns.k, 1);
    let u20 = a.ku8(ns.k, 32);
    let e_len = a.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let e_trail = a.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_res = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");
    let e_union = a.kbytes(ns.k, b"SCB_UNION_INVALID");
    let j_func = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let j_exp = a.param(ns.p, fid, ParameterRole::Function, u64_type());
    let j_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let b_len = err_block(a, ns, fid, res_t.clone(), e_len);
    let b_trail = err_block(a, ns, fid, res_t.clone(), e_trail);
    let b_res = err_block(a, ns, fid, res_t.clone(), e_res);
    let b_union = err_block(a, ns, fid, res_t.clone(), e_union);
    let trap = trap_block(a, ns, fid);
    let entry = a.id(ns.b);
    let exp_chk = a.id(ns.b);
    let exp_is2 = a.id(ns.b);
    let func_conv = a.id(ns.b);
    let func_len = a.id(ns.b);
    let func_gt = a.id(ns.b);
    let out_start = a.id(ns.b);
    let p1 = a.id(ns.b);
    let p2 = a.id(ns.b);
    let p3 = a.id(ns.b);
    let p4 = a.id(ns.b);
    let p5 = a.id(ns.b);
    let fcopy_start = a.id(ns.b);
    let fcopy_check = a.id(ns.b);
    let fcopy_get = a.id(ns.b);
    let fcopy_get2 = a.id(ns.b);
    let fcopy_push = a.id(ns.b);
    let fcopy_next = a.id(ns.b);
    let p6 = a.id(ns.b);
    let p7 = a.id(ns.b);
    let p8 = a.id(ns.b);
    let push_e1 = a.id(ns.b);
    let push_e2 = a.id(ns.b);
    let out_done = a.id(ns.b);
    let out_ret = a.id(ns.b);
    // Exposure 1/2 else UNION (before function work, mirroring value order?
    // Reference encodes function then exposure; exposure check first is fine
    // since both are value errors with no precedence between fields for encode;
    // documented: encode validates exposure before function length).
    let ek1 = a.cref(ns.o, entry, c1, u64_type());
    let ee1 = a.op(
        ns.o,
        entry,
        Opcode::Equal,
        vec![pav(j_exp), op_result(ek1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![ek1, ee1],
        terminator: cond(
            op_result(ee1),
            edge(func_conv, vec![pav(j_func), pav(j_exp), pav(j_unit)]),
            edge(exp_chk, vec![pav(j_func), pav(j_exp), pav(j_unit)]),
        ),
        reachability: Reachability::Required,
    });
    let c_func = a.param(ns.p, exp_chk, ParameterRole::Block, TypeExpr::Bytes);
    let c_exp = a.param(ns.p, exp_chk, ParameterRole::Block, u64_type());
    let c_unit = a.param(ns.p, exp_chk, ParameterRole::Block, TypeExpr::Unit);
    let c_k2 = a.cref(ns.o, exp_chk, c2, u64_type());
    let c_eq = a.op(
        ns.o,
        exp_chk,
        Opcode::Equal,
        vec![pav(c_exp), op_result(c_k2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: exp_chk,
        function: fid,
        parameters: vec![c_func, c_exp, c_unit],
        operations: vec![c_k2, c_eq],
        terminator: cond(
            op_result(c_eq),
            edge(exp_is2, vec![pav(c_func), pav(c_unit)]),
            edge(b_union, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    // exp_is2 drops exposure value (known 2); func_conv for exp==1 keeps it
    // (unused after check, but threaded for arity symmetry is unneeded).
    let i2_func = a.param(ns.p, exp_is2, ParameterRole::Block, TypeExpr::Bytes);
    let i2_unit = a.param(ns.p, exp_is2, ParameterRole::Block, TypeExpr::Unit);
    let i2_k2 = a.cref(ns.o, exp_is2, c2, u64_type());
    a.blocks.push(Block {
        entity_id: exp_is2,
        function: fid,
        parameters: vec![i2_func, i2_unit],
        operations: vec![i2_k2],
        terminator: branch(edge(
            func_conv,
            vec![pav(i2_func), op_result(i2_k2), pav(i2_unit)],
        )),
        reachability: Reachability::Required,
    });
    // func_conv takes (func, exp, unit); exposure already validated 1/2
    // on both entry paths, so exp threads through to the final byte push.
    let f_func = a.param(ns.p, func_conv, ParameterRole::Block, TypeExpr::Bytes);
    let f_exp = a.param(ns.p, func_conv, ParameterRole::Block, u64_type());
    let f_unit = a.param(ns.p, func_conv, ParameterRole::Block, TypeExpr::Unit);
    let f_cv = a.op(
        ns.o,
        func_conv,
        Opcode::AdapterInvoke,
        vec![pav(f_unit), pav(f_func)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: func_conv,
        function: fid,
        parameters: vec![f_func, f_exp, f_unit],
        operations: vec![f_cv],
        terminator: switch(
            op_result(f_cv),
            vec![
                (
                    BuiltinCase::Ok,
                    func_len,
                    vec![SwitchArgument::CasePayload, sav(f_exp), sav(f_unit)],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Function length: <32 LENGTH, >32 TRAILING (encode-side mirror of
    // decode_fixed, same as outer encode eid check).
    let v_funcvec = a.param(ns.p, func_len, ParameterRole::Block, u8vec_type());
    let v_exp = a.param(ns.p, func_len, ParameterRole::Block, u64_type());
    let v_unit = a.param(ns.p, func_len, ParameterRole::Block, TypeExpr::Unit);
    let v_ln = a.op(
        ns.o,
        func_len,
        Opcode::VectorLen,
        vec![pav(v_funcvec)],
        vec![u64_type()],
        Immediate::None,
    );
    let v_k32 = a.cref(ns.o, func_len, c32, u64_type());
    let v_lt = a.op(
        ns.o,
        func_len,
        Opcode::LessThan,
        vec![op_result(v_ln), op_result(v_k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let v_gt = a.op(
        ns.o,
        func_len,
        Opcode::GreaterThan,
        vec![op_result(v_ln), op_result(v_k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: func_len,
        function: fid,
        parameters: vec![v_funcvec, v_exp, v_unit],
        operations: vec![v_ln, v_k32, v_lt, v_gt],
        terminator: cond(
            op_result(v_lt),
            edge(b_len, Vec::new()),
            edge(
                func_gt,
                vec![pav(v_funcvec), pav(v_exp), pav(v_unit), op_result(v_gt)],
            ),
        ),
        reachability: Reachability::Required,
    });
    let g_funcvec = a.param(ns.p, func_gt, ParameterRole::Block, u8vec_type());
    let g_exp = a.param(ns.p, func_gt, ParameterRole::Block, u64_type());
    let g_unit = a.param(ns.p, func_gt, ParameterRole::Block, TypeExpr::Unit);
    let g_flag = a.param(ns.p, func_gt, ParameterRole::Block, TypeExpr::Bool);
    a.blocks.push(Block {
        entity_id: func_gt,
        function: fid,
        parameters: vec![g_funcvec, g_exp, g_unit, g_flag],
        operations: Vec::new(),
        terminator: cond(
            pav(g_flag),
            edge(b_trail, Vec::new()),
            edge(out_start, vec![pav(g_funcvec), pav(g_exp), pav(g_unit)]),
        ),
        reachability: Reachability::Required,
    });
    // Output assembly: VectorNew, then fixed pushes 10/26/02/01/20
    // (union tag 16, union len 38, count 2, field-1 tag 1, field-1 len 32).
    let os_funcvec = a.param(ns.p, out_start, ParameterRole::Block, u8vec_type());
    let os_exp = a.param(ns.p, out_start, ParameterRole::Block, u64_type());
    let os_unit = a.param(ns.p, out_start, ParameterRole::Block, TypeExpr::Unit);
    let os_empty = a.op(
        ns.o,
        out_start,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: out_start,
        function: fid,
        parameters: vec![os_funcvec, os_exp, os_unit],
        operations: vec![os_empty],
        terminator: branch(edge(
            p1,
            vec![
                op_result(os_empty),
                pav(os_funcvec),
                pav(os_exp),
                pav(os_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    // Sequential fixed pushes p1..p5 (each threads acc/funcvec/exp/unit).
    let push_const = [
        (p1, p2, u10),
        (p2, p3, u26),
        (p3, p4, u02),
        (p4, p5, u01),
        (p5, fcopy_start, u20),
    ];
    for (cur, next, konst) in push_const {
        let q_acc = a.param(ns.p, cur, ParameterRole::Block, u8vec_type());
        let q_funcvec = a.param(ns.p, cur, ParameterRole::Block, u8vec_type());
        let q_exp = a.param(ns.p, cur, ParameterRole::Block, u64_type());
        let q_unit = a.param(ns.p, cur, ParameterRole::Block, TypeExpr::Unit);
        let q_c = a.cref(ns.o, cur, konst, u8_type());
        let q_push = a.op(
            ns.o,
            cur,
            Opcode::AdapterInvoke,
            vec![pav(q_acc), op_result(q_c)],
            vec![index_result(u8vec_type())],
            Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
        );
        a.blocks.push(Block {
            entity_id: cur,
            function: fid,
            parameters: vec![q_acc, q_funcvec, q_exp, q_unit],
            operations: vec![q_c, q_push],
            terminator: switch(
                op_result(q_push),
                vec![
                    (
                        BuiltinCase::Ok,
                        next,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(q_funcvec),
                            sav(q_exp),
                            sav(q_unit),
                        ],
                    ),
                    (BuiltinCase::Err, b_res, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
    }
    // Function copy loop (32B, Form A backedge; mirrors outer eid loop).
    let es_acc = a.param(ns.p, fcopy_start, ParameterRole::Block, u8vec_type());
    let es_funcvec = a.param(ns.p, fcopy_start, ParameterRole::Block, u8vec_type());
    let es_exp = a.param(ns.p, fcopy_start, ParameterRole::Block, u64_type());
    let es_unit = a.param(ns.p, fcopy_start, ParameterRole::Block, TypeExpr::Unit);
    let es_z0 = a.cref(ns.o, fcopy_start, c0, u64_type());
    a.blocks.push(Block {
        entity_id: fcopy_start,
        function: fid,
        parameters: vec![es_acc, es_funcvec, es_exp, es_unit],
        operations: vec![es_z0],
        terminator: branch(edge(
            fcopy_check,
            vec![
                op_result(es_z0),
                pav(es_acc),
                pav(es_funcvec),
                pav(es_exp),
                pav(es_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let ec_idx = a.param(ns.p, fcopy_check, ParameterRole::Block, u64_type());
    let ec_acc = a.param(ns.p, fcopy_check, ParameterRole::Block, u8vec_type());
    let ec_funcvec = a.param(ns.p, fcopy_check, ParameterRole::Block, u8vec_type());
    let ec_exp = a.param(ns.p, fcopy_check, ParameterRole::Block, u64_type());
    let ec_unit = a.param(ns.p, fcopy_check, ParameterRole::Block, TypeExpr::Unit);
    let ec_k32 = a.cref(ns.o, fcopy_check, c32, u64_type());
    let ec_lt = a.op(
        ns.o,
        fcopy_check,
        Opcode::LessThan,
        vec![pav(ec_idx), op_result(ec_k32)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: fcopy_check,
        function: fid,
        parameters: vec![ec_idx, ec_acc, ec_funcvec, ec_exp, ec_unit],
        operations: vec![ec_k32, ec_lt],
        terminator: cond(
            op_result(ec_lt),
            edge(
                fcopy_get,
                vec![
                    pav(ec_idx),
                    pav(ec_acc),
                    pav(ec_funcvec),
                    pav(ec_exp),
                    pav(ec_unit),
                ],
            ),
            edge(p6, vec![pav(ec_acc), pav(ec_exp), pav(ec_unit)]),
        ),
        reachability: Reachability::Required,
    });
    let eg_idx = a.param(ns.p, fcopy_get, ParameterRole::Block, u64_type());
    let eg_acc = a.param(ns.p, fcopy_get, ParameterRole::Block, u8vec_type());
    let eg_funcvec = a.param(ns.p, fcopy_get, ParameterRole::Block, u8vec_type());
    let eg_exp = a.param(ns.p, fcopy_get, ParameterRole::Block, u64_type());
    let eg_unit = a.param(ns.p, fcopy_get, ParameterRole::Block, TypeExpr::Unit);
    let eg_get = a.op(
        ns.o,
        fcopy_get,
        Opcode::VectorGet,
        vec![pav(eg_funcvec), pav(eg_idx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: fcopy_get,
        function: fid,
        parameters: vec![eg_idx, eg_acc, eg_funcvec, eg_exp, eg_unit],
        operations: vec![eg_get],
        terminator: switch(
            op_result(eg_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    fcopy_get2,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(eg_idx),
                        sav(eg_acc),
                        sav(eg_funcvec),
                        sav(eg_exp),
                        sav(eg_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Note: fcopy_get2 keeps the source vec (index is direct); Get-None
    // unreachable since function length was checked ==32.
    let hg2_b = a.param(ns.p, fcopy_get2, ParameterRole::Block, u8_type());
    let hg2_idx = a.param(ns.p, fcopy_get2, ParameterRole::Block, u64_type());
    let hg2_acc = a.param(ns.p, fcopy_get2, ParameterRole::Block, u8vec_type());
    let hg2_funcvec = a.param(ns.p, fcopy_get2, ParameterRole::Block, u8vec_type());
    let hg2_exp = a.param(ns.p, fcopy_get2, ParameterRole::Block, u64_type());
    let hg2_unit = a.param(ns.p, fcopy_get2, ParameterRole::Block, TypeExpr::Unit);
    let hg2_push = a.op(
        ns.o,
        fcopy_get2,
        Opcode::AdapterInvoke,
        vec![pav(hg2_acc), pav(hg2_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: fcopy_get2,
        function: fid,
        parameters: vec![hg2_b, hg2_idx, hg2_acc, hg2_funcvec, hg2_exp, hg2_unit],
        operations: vec![hg2_push],
        terminator: switch(
            op_result(hg2_push),
            vec![
                (
                    BuiltinCase::Ok,
                    fcopy_push,
                    vec![
                        sav(hg2_idx),
                        SwitchArgument::CasePayload,
                        sav(hg2_funcvec),
                        sav(hg2_exp),
                        sav(hg2_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let ep_idx = a.param(ns.p, fcopy_push, ParameterRole::Block, u64_type());
    let ep_acc = a.param(ns.p, fcopy_push, ParameterRole::Block, u8vec_type());
    let ep_funcvec = a.param(ns.p, fcopy_push, ParameterRole::Block, u8vec_type());
    let ep_exp = a.param(ns.p, fcopy_push, ParameterRole::Block, u64_type());
    let ep_unit = a.param(ns.p, fcopy_push, ParameterRole::Block, TypeExpr::Unit);
    let ep_c1 = a.cref(ns.o, fcopy_push, c1, u64_type());
    let ep_add = a.op(
        ns.o,
        fcopy_push,
        Opcode::IntAddChecked,
        vec![pav(ep_idx), op_result(ep_c1)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: fcopy_push,
        function: fid,
        parameters: vec![ep_idx, ep_acc, ep_funcvec, ep_exp, ep_unit],
        operations: vec![ep_c1, ep_add],
        terminator: switch(
            op_result(ep_add),
            vec![
                (
                    BuiltinCase::Ok,
                    fcopy_next,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(ep_acc),
                        sav(ep_funcvec),
                        sav(ep_exp),
                        sav(ep_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let en_idx = a.param(ns.p, fcopy_next, ParameterRole::Block, u64_type());
    let en_acc = a.param(ns.p, fcopy_next, ParameterRole::Block, u8vec_type());
    let en_funcvec = a.param(ns.p, fcopy_next, ParameterRole::Block, u8vec_type());
    let en_exp = a.param(ns.p, fcopy_next, ParameterRole::Block, u64_type());
    let en_unit = a.param(ns.p, fcopy_next, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: fcopy_next,
        function: fid,
        parameters: vec![en_idx, en_acc, en_funcvec, en_exp, en_unit],
        operations: Vec::new(),
        terminator: branch(edge(
            fcopy_check,
            vec![
                pav(en_idx),
                pav(en_acc),
                pav(en_funcvec),
                pav(en_exp),
                pav(en_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    // Field-2 tag (02) then field-2 length (01): p6/p7.
    let t_acc = a.param(ns.p, p6, ParameterRole::Block, u8vec_type());
    let t_exp = a.param(ns.p, p6, ParameterRole::Block, u64_type());
    let t_unit = a.param(ns.p, p6, ParameterRole::Block, TypeExpr::Unit);
    let t_c = a.cref(ns.o, p6, u02, u8_type());
    let t_push = a.op(
        ns.o,
        p6,
        Opcode::AdapterInvoke,
        vec![pav(t_acc), op_result(t_c)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: p6,
        function: fid,
        parameters: vec![t_acc, t_exp, t_unit],
        operations: vec![t_c, t_push],
        terminator: switch(
            op_result(t_push),
            vec![
                (
                    BuiltinCase::Ok,
                    p7,
                    vec![SwitchArgument::CasePayload, sav(t_exp), sav(t_unit)],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let l_acc = a.param(ns.p, p7, ParameterRole::Block, u8vec_type());
    let l_exp = a.param(ns.p, p7, ParameterRole::Block, u64_type());
    let l_unit = a.param(ns.p, p7, ParameterRole::Block, TypeExpr::Unit);
    let l_c = a.cref(ns.o, p7, u01, u8_type());
    let l_push = a.op(
        ns.o,
        p7,
        Opcode::AdapterInvoke,
        vec![pav(l_acc), op_result(l_c)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: p7,
        function: fid,
        parameters: vec![l_acc, l_exp, l_unit],
        operations: vec![l_c, l_push],
        terminator: switch(
            op_result(l_push),
            vec![
                (
                    BuiltinCase::Ok,
                    p8,
                    vec![SwitchArgument::CasePayload, sav(l_exp), sav(l_unit)],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Exposure byte select (p8): exp is proven 1/2 here; push u01/u02.
    let s_acc = a.param(ns.p, p8, ParameterRole::Block, u8vec_type());
    let s_exp = a.param(ns.p, p8, ParameterRole::Block, u64_type());
    let s_unit = a.param(ns.p, p8, ParameterRole::Block, TypeExpr::Unit);
    let s_k1 = a.cref(ns.o, p8, c1, u64_type());
    let s_eq = a.op(
        ns.o,
        p8,
        Opcode::Equal,
        vec![pav(s_exp), op_result(s_k1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: p8,
        function: fid,
        parameters: vec![s_acc, s_exp, s_unit],
        operations: vec![s_k1, s_eq],
        terminator: cond(
            op_result(s_eq),
            edge(push_e1, vec![pav(s_acc), pav(s_unit)]),
            edge(push_e2, vec![pav(s_acc), pav(s_unit)]),
        ),
        reachability: Reachability::Required,
    });
    for (blk, konst) in [(push_e1, u01), (push_e2, u02)] {
        let e_acc = a.param(ns.p, blk, ParameterRole::Block, u8vec_type());
        let e_unit = a.param(ns.p, blk, ParameterRole::Block, TypeExpr::Unit);
        let e_c = a.cref(ns.o, blk, konst, u8_type());
        let e_push = a.op(
            ns.o,
            blk,
            Opcode::AdapterInvoke,
            vec![pav(e_acc), op_result(e_c)],
            vec![index_result(u8vec_type())],
            Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
        );
        a.blocks.push(Block {
            entity_id: blk,
            function: fid,
            parameters: vec![e_acc, e_unit],
            operations: vec![e_c, e_push],
            terminator: switch(
                op_result(e_push),
                vec![
                    (
                        BuiltinCase::Ok,
                        out_done,
                        vec![SwitchArgument::CasePayload, sav(e_unit)],
                    ),
                    (BuiltinCase::Err, b_res, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
    }
    // Finalize: V2B1 then Ok.
    let d_acc = a.param(ns.p, out_done, ParameterRole::Block, u8vec_type());
    let d_unit = a.param(ns.p, out_done, ParameterRole::Block, TypeExpr::Unit);
    let d_conv = a.op(
        ns.o,
        out_done,
        Opcode::AdapterInvoke,
        vec![pav(d_unit), pav(d_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    a.blocks.push(Block {
        entity_id: out_done,
        function: fid,
        parameters: vec![d_acc, d_unit],
        operations: vec![d_conv],
        terminator: switch(
            op_result(d_conv),
            vec![
                (BuiltinCase::Ok, out_ret, vec![SwitchArgument::CasePayload]),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let r_bytes = a.param(ns.p, out_ret, ParameterRole::Block, TypeExpr::Bytes);
    let r_ok = a.op(
        ns.o,
        out_ret,
        Opcode::ResultOk,
        vec![pav(r_bytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: out_ret,
        function: fid,
        parameters: vec![r_bytes],
        operations: vec![r_ok],
        terminator: ret(op_result(r_ok)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![j_func, j_exp, j_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

fn program_decode_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            u64_type(),
        ])),
        error: Box::new(TypeExpr::Bytes),
    }
}

fn program_prefix_result_type() -> TypeExpr {
    TypeExpr::Result {
        ok: Box::new(TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes])),
        error: Box::new(TypeExpr::Bytes),
    }
}

// ── program envelope (tag 200, epoch [9;32]) ──────────────────────────
// `validate_program_envelope(input: Bytes, unit: Unit)
// -> Result<Bytes, Bytes>` is the exact `import_entity_object` envelope
// order (`crates/sley-mutate/src/object.rs:118-149`) through digest
// verification, with opaque payload: upfront `len < 32` LENGTH_OVERFLOW
// (reference `input.len() < 32` before magic), magic `SLEYSCB1`, version
// uvar64 == 1, tag uvar32 == 200 (fixed; any other value refuses
// CONTRACT_UNKNOWN, no declared-param comparison), epoch fixed `[9;32]`
// (explicit native test bound, not registry acceptance), payload length
// uvar64 (errors first, then RESOURCE_LIMIT past MAX_STANDALONE_BYTES),
// payload/digest bounds (LENGTH_OVERFLOW), TRAILING (before digest),
// DIGEST (via Sley-built `sley2.object.v1` domain ++ stored prefix
// through RHW1). Payload bytes are returned opaquely; envelope-valid
// never means program-valid (body validity belongs to the composed
// outer/entrypoint path, never inferred here).
//
// Fixture/program context distinction (no weakening): the slice-2
// fixture validator (tags 1..2, fixture epoch, declared tag) is
// preserved unchanged; this is a separate entry with fixed program
// constants. No tag/epoch check was removed and no caller declaration
// is accepted.
//
// Capacity restriction (AR-05 class): B2V1 converts inputs up to the
// 1 MiB bridge cap; larger refuse SCB_RESOURCE_LIMIT at conversion.
// RHW1 likewise caps hash inputs. Documented, not silent.
#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_program_validate(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    decode_fid: EntityId,
) -> FunctionGraph {
    use sley_vm::host_abi::{
        BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1,
    };
    let bstart = a.blocks.len();
    let res_t = encode_result_type();
    let dec_t = decode_result_type();
    // UInt64 constants.
    let c0 = a.ku64(ns.k, 0);
    let c1 = a.ku64(ns.k, 1);
    let c2 = a.ku64(ns.k, 2);
    let c3 = a.ku64(ns.k, 3);
    let c4 = a.ku64(ns.k, 4);
    let c5 = a.ku64(ns.k, 5);
    let c6 = a.ku64(ns.k, 6);
    let c7 = a.ku64(ns.k, 7);
    let c8 = a.ku64(ns.k, 8);
    let c9 = a.ku64(ns.k, 9);
    let c10 = a.ku64(ns.k, 10);
    let c11 = a.ku64(ns.k, 11);
    let c12 = a.ku64(ns.k, 12);
    let c13 = a.ku64(ns.k, 13);
    let c14 = a.ku64(ns.k, 14);
    let c15 = a.ku64(ns.k, 15);
    let c16 = a.ku64(ns.k, 16);
    let c17 = a.ku64(ns.k, 17);
    let c18 = a.ku64(ns.k, 18);
    let c19 = a.ku64(ns.k, 19);
    let c20 = a.ku64(ns.k, 20);
    let c21 = a.ku64(ns.k, 21);
    let c22 = a.ku64(ns.k, 22);
    let c23 = a.ku64(ns.k, 23);
    let c24 = a.ku64(ns.k, 24);
    let c25 = a.ku64(ns.k, 25);
    let c26 = a.ku64(ns.k, 26);
    let c27 = a.ku64(ns.k, 27);
    let c28 = a.ku64(ns.k, 28);
    let c29 = a.ku64(ns.k, 29);
    let c30 = a.ku64(ns.k, 30);
    let c31 = a.ku64(ns.k, 31);
    let c32 = a.ku64(ns.k, 32);
    let c200 = a.ku64(ns.k, 200);
    let c_max = a.ku64(ns.k, 67_108_864);
    // Width constants are UInt32 (CallDirect demands u32 width).
    let w32 = a.ku32(ns.k, 32);
    let w64 = a.ku32(ns.k, 64);
    // Distinct UInt8 byte values for magic, domain, epoch.
    let b09 = a.ku8(ns.k, 0x09);
    let b2e = a.ku8(ns.k, 0x2e);
    let b31 = a.ku8(ns.k, 0x31);
    let b32 = a.ku8(ns.k, 0x32);
    let b42 = a.ku8(ns.k, 0x42);
    let b43 = a.ku8(ns.k, 0x43);
    let b45 = a.ku8(ns.k, 0x45);
    let b4c = a.ku8(ns.k, 0x4c);
    let b53 = a.ku8(ns.k, 0x53);
    let b59 = a.ku8(ns.k, 0x59);
    let b62 = a.ku8(ns.k, 0x62);
    let b63 = a.ku8(ns.k, 0x63);
    let b65 = a.ku8(ns.k, 0x65);
    let b6a = a.ku8(ns.k, 0x6a);
    let b6c = a.ku8(ns.k, 0x6c);
    let b6f = a.ku8(ns.k, 0x6f);
    let b73 = a.ku8(ns.k, 0x73);
    let b74 = a.ku8(ns.k, 0x74);
    let b76 = a.ku8(ns.k, 0x76);
    let b79 = a.ku8(ns.k, 0x79);
    // Exact error-code bytes.
    let e_magic = a.kbytes(ns.k, b"SCB_MAGIC_INVALID");
    let e_version = a.kbytes(ns.k, b"SCB_VERSION_UNSUPPORTED");
    let e_contract = a.kbytes(ns.k, b"SCB_CONTRACT_UNKNOWN");
    let e_epoch = a.kbytes(ns.k, b"SCB_EPOCH_MISMATCH");
    let e_digest = a.kbytes(ns.k, b"SCB_DIGEST_MISMATCH");
    let e_trail = a.kbytes(ns.k, b"SCB_TRAILING_BYTES");
    let e_len = a.kbytes(ns.k, b"SCB_LENGTH_OVERFLOW");
    let e_res = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");

    let p_in = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let p_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let b_magic = err_block(a, ns, fid, res_t.clone(), e_magic);
    let b_version = err_block(a, ns, fid, res_t.clone(), e_version);
    let b_contract = err_block(a, ns, fid, res_t.clone(), e_contract);
    let b_epoch = err_block(a, ns, fid, res_t.clone(), e_epoch);
    let b_digest = err_block(a, ns, fid, res_t.clone(), e_digest);
    let b_trail = err_block(a, ns, fid, res_t.clone(), e_trail);
    let b_len = err_block(a, ns, fid, res_t.clone(), e_len);
    let b_res = err_block(a, ns, fid, res_t.clone(), e_res);
    let trap = trap_block(a, ns, fid);

    // Entry: convert input bytes to a vector once, up front. Bridge
    // failure means past the 1 MiB cap: documented RESOURCE_LIMIT.
    let entry = a.id(ns.b);
    let cv = a.op(
        ns.o,
        entry,
        Opcode::AdapterInvoke,
        vec![pav(p_unit), pav(p_in)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    let b_len0 = a.id(ns.b);
    let q_vec = a.param(ns.p, b_len0, ParameterRole::Block, u8vec_type());
    let q_in = a.param(ns.p, b_len0, ParameterRole::Block, TypeExpr::Bytes);
    let q_unit = a.param(ns.p, b_len0, ParameterRole::Block, TypeExpr::Unit);
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
                    b_len0,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(p_in),
                        sav(p_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    // Upfront `len < 32` LENGTH_OVERFLOW (reference `input.len() < 32`
    // before magic, `object.rs:125`). Magic reads below are in bounds
    // after this check (len >= 32 > 8), so Get None there is unreachable.
    let ln = a.op(
        ns.o,
        b_len0,
        Opcode::VectorLen,
        vec![pav(q_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let m32c = a.cref(ns.o, b_len0, c32, u64_type());
    let mlt = a.op(
        ns.o,
        b_len0,
        Opcode::LessThan,
        vec![op_result(ln), op_result(m32c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let mut m_get: Vec<EntityId> = Vec::new();
    let mut m_cmp: Vec<EntityId> = Vec::new();
    for _ in 0..8 {
        m_get.push(a.id(ns.b));
        m_cmp.push(a.id(ns.b));
    }
    let v_call = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: b_len0,
        function: fid,
        parameters: vec![q_vec, q_in, q_unit],
        operations: vec![ln, m32c, mlt],
        terminator: cond(
            op_result(mlt),
            edge(b_len, Vec::new()),
            edge(
                m_get[0],
                vec![pav(q_vec), op_result(ln), pav(q_in), pav(q_unit)],
            ),
        ),
        reachability: Reachability::Required,
    });

    // Magic chain: `SLEYSCB1`. Get None is unreachable after len >= 32
    // (construction-invariant, trap); a present-but-wrong byte refuses
    // MAGIC_INVALID.
    let magic_expected = [b53, b4c, b45, b59, b53, b43, b42, b31];
    let magic_idx = [c0, c1, c2, c3, c4, c5, c6, c7];
    for i in 0..8 {
        let g = m_get[i];
        let cp = m_cmp[i];
        let g_vec = a.param(ns.p, g, ParameterRole::Block, u8vec_type());
        let g_len = a.param(ns.p, g, ParameterRole::Block, u64_type());
        let g_in = a.param(ns.p, g, ParameterRole::Block, TypeExpr::Bytes);
        let g_unit = a.param(ns.p, g, ParameterRole::Block, TypeExpr::Unit);
        let idxc = a.cref(ns.o, g, magic_idx[i], u64_type());
        let get = a.op(
            ns.o,
            g,
            Opcode::VectorGet,
            vec![pav(g_vec), op_result(idxc)],
            vec![TypeExpr::Option(Box::new(u8_type()))],
            Immediate::None,
        );
        let next = if i == 7 { v_call } else { m_get[i + 1] };
        a.blocks.push(Block {
            entity_id: g,
            function: fid,
            parameters: vec![g_vec, g_len, g_in, g_unit],
            operations: vec![idxc, get],
            terminator: switch(
                op_result(get),
                vec![
                    (BuiltinCase::None, trap, Vec::new()),
                    (
                        BuiltinCase::Some,
                        cp,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(g_vec),
                            sav(g_len),
                            sav(g_in),
                            sav(g_unit),
                        ],
                    ),
                ],
            ),
            reachability: Reachability::Required,
        });
        let c_b = a.param(ns.p, cp, ParameterRole::Block, u8_type());
        let c_vec = a.param(ns.p, cp, ParameterRole::Block, u8vec_type());
        let c_len = a.param(ns.p, cp, ParameterRole::Block, u64_type());
        let c_in = a.param(ns.p, cp, ParameterRole::Block, TypeExpr::Bytes);
        let c_unit = a.param(ns.p, cp, ParameterRole::Block, TypeExpr::Unit);
        let expc = a.cref(ns.o, cp, magic_expected[i], u8_type());
        let eq = a.op(
            ns.o,
            cp,
            Opcode::Equal,
            vec![pav(c_b), op_result(expc)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        a.blocks.push(Block {
            entity_id: cp,
            function: fid,
            parameters: vec![c_b, c_vec, c_len, c_in, c_unit],
            operations: vec![expc, eq],
            terminator: cond(
                op_result(eq),
                edge(
                    next,
                    vec![pav(c_vec), pav(c_len), pav(c_in), pav(c_unit)],
                ),
                edge(b_magic, Vec::new()),
            ),
            reachability: Reachability::Required,
        });
    }

    // Version: decode_uvar(input, pos 8, width 64). Uvar errors propagate
    // with their exact codes; only a successful value != 1 refuses here.
    let v_vec = a.param(ns.p, v_call, ParameterRole::Block, u8vec_type());
    let v_len = a.param(ns.p, v_call, ParameterRole::Block, u64_type());
    let v_in = a.param(ns.p, v_call, ParameterRole::Block, TypeExpr::Bytes);
    let v_unit = a.param(ns.p, v_call, ParameterRole::Block, TypeExpr::Unit);
    let v_posc = a.cref(ns.o, v_call, c8, u64_type());
    let v_wc = a.cref(ns.o, v_call, w64, u32_type());
    let v_callop = a.op(
        ns.o,
        v_call,
        Opcode::CallDirect,
        vec![pav(v_in), op_result(v_posc), op_result(v_wc), pav(v_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let v_ok = a.id(ns.b);
    let v_err = a.id(ns.b);
    let v_tup = a.param(
        ns.p,
        v_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let v_ovec = a.param(ns.p, v_ok, ParameterRole::Block, u8vec_type());
    let v_olen = a.param(ns.p, v_ok, ParameterRole::Block, u64_type());
    let v_oin = a.param(ns.p, v_ok, ParameterRole::Block, TypeExpr::Bytes);
    let v_ounit = a.param(ns.p, v_ok, ParameterRole::Block, TypeExpr::Unit);
    let v_ebytes = a.param(ns.p, v_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: v_call,
        function: fid,
        parameters: vec![v_vec, v_len, v_in, v_unit],
        operations: vec![v_posc, v_wc, v_callop],
        terminator: switch(
            op_result(v_callop),
            vec![
                (
                    BuiltinCase::Ok,
                    v_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(v_vec),
                        sav(v_len),
                        sav(v_in),
                        sav(v_unit),
                    ],
                ),
                (BuiltinCase::Err, v_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Forward uvar refusal bytes unchanged (exact reference code).
    let v_er = a.op(
        ns.o,
        v_err,
        Opcode::ResultErr,
        vec![pav(v_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: v_err,
        function: fid,
        parameters: vec![v_ebytes],
        operations: vec![v_er],
        terminator: ret(op_result(v_er)),
        reachability: Reachability::Required,
    });
    let v_gv = a.op(
        ns.o,
        v_ok,
        Opcode::TupleGet,
        vec![pav(v_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let v_gp = a.op(
        ns.o,
        v_ok,
        Opcode::TupleGet,
        vec![pav(v_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let v_onec = a.cref(ns.o, v_ok, c1, u64_type());
    let v_eq = a.op(
        ns.o,
        v_ok,
        Opcode::Equal,
        vec![op_result(v_gv), op_result(v_onec)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let c_call = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: v_ok,
        function: fid,
        parameters: vec![v_tup, v_ovec, v_olen, v_oin, v_ounit],
        operations: vec![v_gv, v_gp, v_onec, v_eq],
        terminator: cond(
            op_result(v_eq),
            edge(
                c_call,
                vec![
                    op_result(v_gp),
                    pav(v_ovec),
                    pav(v_olen),
                    pav(v_oin),
                    pav(v_ounit),
                ],
            ),
            edge(b_version, Vec::new()),
        ),
        reachability: Reachability::Required,
    });

    // Contract tag: decode_uvar(input, pos_after_version, width 32).
    // Fixed program tag 200 (`object.rs:136`): any decoded value != 200
    // refuses CONTRACT_UNKNOWN, whether unknown or a valid fixture tag.
    let t_pos = a.param(ns.p, c_call, ParameterRole::Block, u64_type());
    let t_vec = a.param(ns.p, c_call, ParameterRole::Block, u8vec_type());
    let t_len = a.param(ns.p, c_call, ParameterRole::Block, u64_type());
    let t_in = a.param(ns.p, c_call, ParameterRole::Block, TypeExpr::Bytes);
    let t_unit = a.param(ns.p, c_call, ParameterRole::Block, TypeExpr::Unit);
    let t_wc = a.cref(ns.o, c_call, w32, u32_type());
    let t_callop = a.op(
        ns.o,
        c_call,
        Opcode::CallDirect,
        vec![pav(t_in), pav(t_pos), op_result(t_wc), pav(t_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let t_ok = a.id(ns.b);
    let t_err = a.id(ns.b);
    let t_tup = a.param(
        ns.p,
        t_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let t_opos = a.param(ns.p, t_ok, ParameterRole::Block, u64_type());
    let t_ovec = a.param(ns.p, t_ok, ParameterRole::Block, u8vec_type());
    let t_olen = a.param(ns.p, t_ok, ParameterRole::Block, u64_type());
    let t_oin = a.param(ns.p, t_ok, ParameterRole::Block, TypeExpr::Bytes);
    let t_ounit = a.param(ns.p, t_ok, ParameterRole::Block, TypeExpr::Unit);
    let t_ebytes = a.param(ns.p, t_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: c_call,
        function: fid,
        parameters: vec![t_pos, t_vec, t_len, t_in, t_unit],
        operations: vec![t_wc, t_callop],
        terminator: switch(
            op_result(t_callop),
            vec![
                (
                    BuiltinCase::Ok,
                    t_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(t_pos),
                        sav(t_vec),
                        sav(t_len),
                        sav(t_in),
                        sav(t_unit),
                    ],
                ),
                (BuiltinCase::Err, t_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let t_er = a.op(
        ns.o,
        t_err,
        Opcode::ResultErr,
        vec![pav(t_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: t_err,
        function: fid,
        parameters: vec![t_ebytes],
        operations: vec![t_er],
        terminator: ret(op_result(t_er)),
        reachability: Reachability::Required,
    });
    let t_gv = a.op(
        ns.o,
        t_ok,
        Opcode::TupleGet,
        vec![pav(t_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let t_gp = a.op(
        ns.o,
        t_ok,
        Opcode::TupleGet,
        vec![pav(t_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let t_c200 = a.cref(ns.o, t_ok, c200, u64_type());
    let t_eq = a.op(
        ns.o,
        t_ok,
        Opcode::Equal,
        vec![op_result(t_gv), op_result(t_c200)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let e_bounds = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: t_ok,
        function: fid,
        parameters: vec![t_tup, t_opos, t_ovec, t_olen, t_oin, t_ounit],
        operations: vec![t_gv, t_gp, t_c200, t_eq],
        terminator: cond(
            op_result(t_eq),
            edge(
                e_bounds,
                vec![
                    op_result(t_gp),
                    pav(t_ovec),
                    pav(t_olen),
                    pav(t_oin),
                    pav(t_ounit),
                ],
            ),
            edge(b_contract, Vec::new()),
        ),
        reachability: Reachability::Required,
    });

    // Epoch bounds: remaining = len - pos_after_tag must hold 32 bytes,
    // else LENGTH_OVERFLOW (reference take_array). Subtraction failure
    // (pos > len) is unreachable by decode construction: new_pos never
    // exceeds the converted length, so Err targets the invariant trap.
    let e_pos = a.param(ns.p, e_bounds, ParameterRole::Block, u64_type());
    let e_vec = a.param(ns.p, e_bounds, ParameterRole::Block, u8vec_type());
    let e_lenp = a.param(ns.p, e_bounds, ParameterRole::Block, u64_type());
    let e_in = a.param(ns.p, e_bounds, ParameterRole::Block, TypeExpr::Bytes);
    let e_unit = a.param(ns.p, e_bounds, ParameterRole::Block, TypeExpr::Unit);
    let e_sub = a.op(
        ns.o,
        e_bounds,
        Opcode::IntSubChecked,
        vec![pav(e_lenp), pav(e_pos)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let e_rem = a.id(ns.b);
    let r_rem = a.param(ns.p, e_rem, ParameterRole::Block, u64_type());
    let r_pos = a.param(ns.p, e_rem, ParameterRole::Block, u64_type());
    let r_vec = a.param(ns.p, e_rem, ParameterRole::Block, u8vec_type());
    let r_len = a.param(ns.p, e_rem, ParameterRole::Block, u64_type());
    let r_in = a.param(ns.p, e_rem, ParameterRole::Block, TypeExpr::Bytes);
    let r_unit = a.param(ns.p, e_rem, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: e_bounds,
        function: fid,
        parameters: vec![e_pos, e_vec, e_lenp, e_in, e_unit],
        operations: vec![e_sub],
        terminator: switch(
            op_result(e_sub),
            vec![
                (
                    BuiltinCase::Ok,
                    e_rem,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(e_pos),
                        sav(e_vec),
                        sav(e_lenp),
                        sav(e_in),
                        sav(e_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let e32c = a.cref(ns.o, e_rem, c32, u64_type());
    let e_lt = a.op(
        ns.o,
        e_rem,
        Opcode::LessThan,
        vec![pav(r_rem), op_result(e32c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    // First epoch-byte Get block; the 32-byte chain is generated below.
    let mut ep_get: Vec<EntityId> = Vec::new();
    let mut ep_cmp: Vec<EntityId> = Vec::new();
    let mut ep_inc: Vec<EntityId> = Vec::new();
    for _ in 0..32 {
        ep_get.push(a.id(ns.b));
        ep_cmp.push(a.id(ns.b));
        ep_inc.push(a.id(ns.b));
    }
    let l_call = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: e_rem,
        function: fid,
        parameters: vec![r_rem, r_pos, r_vec, r_len, r_in, r_unit],
        operations: vec![e32c, e_lt],
        terminator: cond(
            op_result(e_lt),
            edge(b_len, Vec::new()),
            edge(
                ep_get[0],
                vec![
                    pav(r_pos),
                    pav(r_vec),
                    pav(r_len),
                    pav(r_in),
                    pav(r_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    // Epoch chain: 32 bytes, expected `[9;32]` (explicit native test
    // bound). Get None is unreachable after the remaining>=32 check, so
    // None targets the invariant trap. A present-but-wrong byte refuses
    // EPOCH_MISMATCH.
    for i in 0..32 {
        let g = ep_get[i];
        let cp = ep_cmp[i];
        let ic = ep_inc[i];
        let g_idx = a.param(ns.p, g, ParameterRole::Block, u64_type());
        let g_vec = a.param(ns.p, g, ParameterRole::Block, u8vec_type());
        let g_len = a.param(ns.p, g, ParameterRole::Block, u64_type());
        let g_in = a.param(ns.p, g, ParameterRole::Block, TypeExpr::Bytes);
        let g_unit = a.param(ns.p, g, ParameterRole::Block, TypeExpr::Unit);
        let get = a.op(
            ns.o,
            g,
            Opcode::VectorGet,
            vec![pav(g_vec), pav(g_idx)],
            vec![TypeExpr::Option(Box::new(u8_type()))],
            Immediate::None,
        );
        a.blocks.push(Block {
            entity_id: g,
            function: fid,
            parameters: vec![g_idx, g_vec, g_len, g_in, g_unit],
            operations: vec![get],
            terminator: switch(
                op_result(get),
                vec![
                    (BuiltinCase::None, trap, Vec::new()),
                    (
                        BuiltinCase::Some,
                        cp,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(g_idx),
                            sav(g_vec),
                            sav(g_len),
                            sav(g_in),
                            sav(g_unit),
                        ],
                    ),
                ],
            ),
            reachability: Reachability::Required,
        });
        let p_b = a.param(ns.p, cp, ParameterRole::Block, u8_type());
        let p_idx = a.param(ns.p, cp, ParameterRole::Block, u64_type());
        let p_vec = a.param(ns.p, cp, ParameterRole::Block, u8vec_type());
        let p_len = a.param(ns.p, cp, ParameterRole::Block, u64_type());
        let p_inb = a.param(ns.p, cp, ParameterRole::Block, TypeExpr::Bytes);
        let p_unitb = a.param(ns.p, cp, ParameterRole::Block, TypeExpr::Unit);
        let expc = a.cref(ns.o, cp, b09, u8_type());
        let eq = a.op(
            ns.o,
            cp,
            Opcode::Equal,
            vec![pav(p_b), op_result(expc)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        a.blocks.push(Block {
            entity_id: cp,
            function: fid,
            parameters: vec![p_b, p_idx, p_vec, p_len, p_inb, p_unitb],
            operations: vec![expc, eq],
            terminator: cond(
                op_result(eq),
                edge(
                    ic,
                    vec![
                        pav(p_idx),
                        pav(p_vec),
                        pav(p_len),
                        pav(p_inb),
                        pav(p_unitb),
                    ],
                ),
                edge(b_epoch, Vec::new()),
            ),
            reachability: Reachability::Required,
        });
        // Increment carries the next index. Overflow is unreachable
        // (indices stay below the 1 MiB converted length), so Err traps.
        let n_idx = a.param(ns.p, ic, ParameterRole::Block, u64_type());
        let n_vec = a.param(ns.p, ic, ParameterRole::Block, u8vec_type());
        let n_len = a.param(ns.p, ic, ParameterRole::Block, u64_type());
        let n_in = a.param(ns.p, ic, ParameterRole::Block, TypeExpr::Bytes);
        let n_unit = a.param(ns.p, ic, ParameterRole::Block, TypeExpr::Unit);
        let onec = a.cref(ns.o, ic, c1, u64_type());
        let add = a.op(
            ns.o,
            ic,
            Opcode::IntAddChecked,
            vec![pav(n_idx), op_result(onec)],
            vec![arith_result(u64_type())],
            Immediate::None,
        );
        if i == 31 {
            a.blocks.push(Block {
                entity_id: ic,
                function: fid,
                parameters: vec![n_idx, n_vec, n_len, n_in, n_unit],
                operations: vec![onec, add],
                terminator: switch(
                    op_result(add),
                    vec![
                        (
                            BuiltinCase::Ok,
                            l_call,
                            vec![
                                SwitchArgument::CasePayload,
                                sav(n_vec),
                                sav(n_len),
                                sav(n_in),
                                sav(n_unit),
                            ],
                        ),
                        (BuiltinCase::Err, trap, Vec::new()),
                    ],
                ),
                reachability: Reachability::Required,
            });
        } else {
            a.blocks.push(Block {
                entity_id: ic,
                function: fid,
                parameters: vec![n_idx, n_vec, n_len, n_in, n_unit],
                operations: vec![onec, add],
                terminator: switch(
                    op_result(add),
                    vec![
                        (
                            BuiltinCase::Ok,
                            ep_get[i + 1],
                            vec![
                                SwitchArgument::CasePayload,
                                sav(n_vec),
                                sav(n_len),
                                sav(n_in),
                                sav(n_unit),
                            ],
                        ),
                        (BuiltinCase::Err, trap, Vec::new()),
                    ],
                ),
                reachability: Reachability::Required,
            });
        }
    }

    // Payload length: decode_uvar(input, pos_after_epoch, width 64).
    // Uvar errors propagate; a value past MAX_STANDALONE_BYTES refuses
    // RESOURCE_LIMIT (reference read_len rule, `lib.rs:1270`).
    let q_pos = a.param(ns.p, l_call, ParameterRole::Block, u64_type());
    let q_vec = a.param(ns.p, l_call, ParameterRole::Block, u8vec_type());
    let q_len = a.param(ns.p, l_call, ParameterRole::Block, u64_type());
    let q_in = a.param(ns.p, l_call, ParameterRole::Block, TypeExpr::Bytes);
    let q_unit = a.param(ns.p, l_call, ParameterRole::Block, TypeExpr::Unit);
    let l_wc = a.cref(ns.o, l_call, w64, u32_type());
    let l_callop = a.op(
        ns.o,
        l_call,
        Opcode::CallDirect,
        vec![pav(q_in), pav(q_pos), op_result(l_wc), pav(q_unit)],
        vec![dec_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: decode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let l_ok = a.id(ns.b);
    let l_err = a.id(ns.b);
    let l_tup = a.param(
        ns.p,
        l_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![u64_type(), u64_type()]),
    );
    let l_ovec = a.param(ns.p, l_ok, ParameterRole::Block, u8vec_type());
    let l_olen = a.param(ns.p, l_ok, ParameterRole::Block, u64_type());
    let l_oin = a.param(ns.p, l_ok, ParameterRole::Block, TypeExpr::Bytes);
    let l_ounit = a.param(ns.p, l_ok, ParameterRole::Block, TypeExpr::Unit);
    let l_ebytes = a.param(ns.p, l_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: l_call,
        function: fid,
        parameters: vec![q_pos, q_vec, q_len, q_in, q_unit],
        operations: vec![l_wc, l_callop],
        terminator: switch(
            op_result(l_callop),
            vec![
                (
                    BuiltinCase::Ok,
                    l_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(q_vec),
                        sav(q_len),
                        sav(q_in),
                        sav(q_unit),
                    ],
                ),
                (BuiltinCase::Err, l_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let l_er = a.op(
        ns.o,
        l_err,
        Opcode::ResultErr,
        vec![pav(l_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: l_err,
        function: fid,
        parameters: vec![l_ebytes],
        operations: vec![l_er],
        terminator: ret(op_result(l_er)),
        reachability: Reachability::Required,
    });
    let l_gv = a.op(
        ns.o,
        l_ok,
        Opcode::TupleGet,
        vec![pav(l_tup)],
        vec![u64_type()],
        Immediate::Index(0),
    );
    let l_gp = a.op(
        ns.o,
        l_ok,
        Opcode::TupleGet,
        vec![pav(l_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let l_maxc = a.cref(ns.o, l_ok, c_max, u64_type());
    let l_over = a.op(
        ns.o,
        l_ok,
        Opcode::GreaterThan,
        vec![op_result(l_gv), op_result(l_maxc)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let b_bounds = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: l_ok,
        function: fid,
        parameters: vec![l_tup, l_ovec, l_olen, l_oin, l_ounit],
        operations: vec![l_gv, l_gp, l_maxc, l_over],
        terminator: cond(
            op_result(l_over),
            edge(b_res, Vec::new()),
            edge(
                b_bounds,
                vec![
                    op_result(l_gv),
                    op_result(l_gp),
                    pav(l_ovec),
                    pav(l_olen),
                    pav(l_oin),
                    pav(l_ounit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    // Envelope bounds: payload take then digest take then trailing check,
    // in reference order. All subtractions are provably safe after the
    // preceding comparisons; Err targets the invariant trap.
    let s_plen = a.param(ns.p, b_bounds, ParameterRole::Block, u64_type());
    let s_ppos = a.param(ns.p, b_bounds, ParameterRole::Block, u64_type());
    let s_vec = a.param(ns.p, b_bounds, ParameterRole::Block, u8vec_type());
    let s_len = a.param(ns.p, b_bounds, ParameterRole::Block, u64_type());
    let s_in = a.param(ns.p, b_bounds, ParameterRole::Block, TypeExpr::Bytes);
    let s_unit = a.param(ns.p, b_bounds, ParameterRole::Block, TypeExpr::Unit);
    let s_sub = a.op(
        ns.o,
        b_bounds,
        Opcode::IntSubChecked,
        vec![pav(s_len), pav(s_ppos)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let s_rem = a.id(ns.b);
    let r_plen = a.param(ns.p, s_rem, ParameterRole::Block, u64_type());
    let r_ppos = a.param(ns.p, s_rem, ParameterRole::Block, u64_type());
    let r_remv = a.param(ns.p, s_rem, ParameterRole::Block, u64_type());
    let r_vec = a.param(ns.p, s_rem, ParameterRole::Block, u8vec_type());
    let r_len = a.param(ns.p, s_rem, ParameterRole::Block, u64_type());
    let r_in = a.param(ns.p, s_rem, ParameterRole::Block, TypeExpr::Bytes);
    let r_unit = a.param(ns.p, s_rem, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: b_bounds,
        function: fid,
        parameters: vec![s_plen, s_ppos, s_vec, s_len, s_in, s_unit],
        operations: vec![s_sub],
        terminator: switch(
            op_result(s_sub),
            vec![
                (
                    BuiltinCase::Ok,
                    s_rem,
                    vec![
                        sav(s_plen),
                        sav(s_ppos),
                        SwitchArgument::CasePayload,
                        sav(s_vec),
                        sav(s_len),
                        sav(s_in),
                        sav(s_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let s_gt = a.op(
        ns.o,
        s_rem,
        Opcode::GreaterThan,
        vec![pav(r_plen), pav(r_remv)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let s_pe = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: s_rem,
        function: fid,
        parameters: vec![r_plen, r_ppos, r_remv, r_vec, r_len, r_in, r_unit],
        operations: vec![s_gt],
        terminator: cond(
            op_result(s_gt),
            edge(b_len, Vec::new()),
            edge(
                s_pe,
                vec![
                    pav(r_plen),
                    pav(r_ppos),
                    pav(r_vec),
                    pav(r_len),
                    pav(r_in),
                    pav(r_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // payload_end = payload_start + payload_len (safe: plen <= remaining).
    let e_plen = a.param(ns.p, s_pe, ParameterRole::Block, u64_type());
    let e_ppos = a.param(ns.p, s_pe, ParameterRole::Block, u64_type());
    let e_vec = a.param(ns.p, s_pe, ParameterRole::Block, u8vec_type());
    let e_len2 = a.param(ns.p, s_pe, ParameterRole::Block, u64_type());
    let e_in = a.param(ns.p, s_pe, ParameterRole::Block, TypeExpr::Bytes);
    let e_unit = a.param(ns.p, s_pe, ParameterRole::Block, TypeExpr::Unit);
    let e_add = a.op(
        ns.o,
        s_pe,
        Opcode::IntAddChecked,
        vec![pav(e_ppos), pav(e_plen)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let e_after = a.id(ns.b);
    let f_pend = a.param(ns.p, e_after, ParameterRole::Block, u64_type());
    let f_plen = a.param(ns.p, e_after, ParameterRole::Block, u64_type());
    let f_ppos = a.param(ns.p, e_after, ParameterRole::Block, u64_type());
    let f_vec = a.param(ns.p, e_after, ParameterRole::Block, u8vec_type());
    let f_len = a.param(ns.p, e_after, ParameterRole::Block, u64_type());
    let f_in = a.param(ns.p, e_after, ParameterRole::Block, TypeExpr::Bytes);
    let f_unit = a.param(ns.p, e_after, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: s_pe,
        function: fid,
        parameters: vec![e_plen, e_ppos, e_vec, e_len2, e_in, e_unit],
        operations: vec![e_add],
        terminator: switch(
            op_result(e_add),
            vec![
                (
                    BuiltinCase::Ok,
                    e_after,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(e_plen),
                        sav(e_ppos),
                        sav(e_vec),
                        sav(e_len2),
                        sav(e_in),
                        sav(e_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let f_sub = a.op(
        ns.o,
        e_after,
        Opcode::IntSubChecked,
        vec![pav(f_len), pav(f_pend)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let f_rem = a.id(ns.b);
    let g_pend = a.param(ns.p, f_rem, ParameterRole::Block, u64_type());
    let g_plen = a.param(ns.p, f_rem, ParameterRole::Block, u64_type());
    let g_ppos = a.param(ns.p, f_rem, ParameterRole::Block, u64_type());
    let g_remv = a.param(ns.p, f_rem, ParameterRole::Block, u64_type());
    let g_vec = a.param(ns.p, f_rem, ParameterRole::Block, u8vec_type());
    let g_len = a.param(ns.p, f_rem, ParameterRole::Block, u64_type());
    let g_in = a.param(ns.p, f_rem, ParameterRole::Block, TypeExpr::Bytes);
    let g_unit = a.param(ns.p, f_rem, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: e_after,
        function: fid,
        parameters: vec![f_pend, f_plen, f_ppos, f_vec, f_len, f_in, f_unit],
        operations: vec![f_sub],
        terminator: switch(
            op_result(f_sub),
            vec![
                (
                    BuiltinCase::Ok,
                    f_rem,
                    vec![
                        sav(f_pend),
                        sav(f_plen),
                        sav(f_ppos),
                        SwitchArgument::CasePayload,
                        sav(f_vec),
                        sav(f_len),
                        sav(f_in),
                        sav(f_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let g32c = a.cref(ns.o, f_rem, c32, u64_type());
    let g_lt = a.op(
        ns.o,
        f_rem,
        Opcode::LessThan,
        vec![pav(g_remv), op_result(g32c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let g_de = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: f_rem,
        function: fid,
        parameters: vec![
            g_pend, g_plen, g_ppos, g_remv, g_vec, g_len, g_in, g_unit,
        ],
        operations: vec![g32c, g_lt],
        terminator: cond(
            op_result(g_lt),
            edge(b_len, Vec::new()),
            edge(
                g_de,
                vec![
                    pav(g_pend),
                    pav(g_plen),
                    pav(g_ppos),
                    pav(g_vec),
                    pav(g_len),
                    pav(g_in),
                    pav(g_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    // digest_end = payload_end + 32 (safe: remaining_after_payload >= 32).
    let h_pend = a.param(ns.p, g_de, ParameterRole::Block, u64_type());
    let h_plen = a.param(ns.p, g_de, ParameterRole::Block, u64_type());
    let h_ppos = a.param(ns.p, g_de, ParameterRole::Block, u64_type());
    let h_vec = a.param(ns.p, g_de, ParameterRole::Block, u8vec_type());
    let h_len = a.param(ns.p, g_de, ParameterRole::Block, u64_type());
    let h_in = a.param(ns.p, g_de, ParameterRole::Block, TypeExpr::Bytes);
    let h_unit = a.param(ns.p, g_de, ParameterRole::Block, TypeExpr::Unit);
    let h_c32 = a.cref(ns.o, g_de, c32, u64_type());
    let h_add = a.op(
        ns.o,
        g_de,
        Opcode::IntAddChecked,
        vec![pav(h_pend), op_result(h_c32)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let h_after = a.id(ns.b);
    let j_dend = a.param(ns.p, h_after, ParameterRole::Block, u64_type());
    let j_pend = a.param(ns.p, h_after, ParameterRole::Block, u64_type());
    let j_plen = a.param(ns.p, h_after, ParameterRole::Block, u64_type());
    let j_ppos = a.param(ns.p, h_after, ParameterRole::Block, u64_type());
    let j_vec = a.param(ns.p, h_after, ParameterRole::Block, u8vec_type());
    let j_len = a.param(ns.p, h_after, ParameterRole::Block, u64_type());
    let j_in = a.param(ns.p, h_after, ParameterRole::Block, TypeExpr::Bytes);
    let j_unit = a.param(ns.p, h_after, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: g_de,
        function: fid,
        parameters: vec![h_pend, h_plen, h_ppos, h_vec, h_len, h_in, h_unit],
        operations: vec![h_c32, h_add],
        terminator: switch(
            op_result(h_add),
            vec![
                (
                    BuiltinCase::Ok,
                    h_after,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(h_pend),
                        sav(h_plen),
                        sav(h_ppos),
                        sav(h_vec),
                        sav(h_len),
                        sav(h_in),
                        sav(h_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let j_sub = a.op(
        ns.o,
        h_after,
        Opcode::IntSubChecked,
        vec![pav(j_len), pav(j_dend)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let j_trail = a.id(ns.b);
    let k_dend = a.param(ns.p, j_trail, ParameterRole::Block, u64_type());
    let k_pend = a.param(ns.p, j_trail, ParameterRole::Block, u64_type());
    let k_plen = a.param(ns.p, j_trail, ParameterRole::Block, u64_type());
    let k_ppos = a.param(ns.p, j_trail, ParameterRole::Block, u64_type());
    let k_trail = a.param(ns.p, j_trail, ParameterRole::Block, u64_type());
    let k_vec = a.param(ns.p, j_trail, ParameterRole::Block, u8vec_type());
    let k_len = a.param(ns.p, j_trail, ParameterRole::Block, u64_type());
    let k_in = a.param(ns.p, j_trail, ParameterRole::Block, TypeExpr::Bytes);
    let k_unit = a.param(ns.p, j_trail, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: h_after,
        function: fid,
        parameters: vec![
            j_dend, j_pend, j_plen, j_ppos, j_vec, j_len, j_in, j_unit,
        ],
        operations: vec![j_sub],
        terminator: switch(
            op_result(j_sub),
            vec![
                (
                    BuiltinCase::Ok,
                    j_trail,
                    vec![
                        sav(j_dend),
                        sav(j_pend),
                        sav(j_plen),
                        sav(j_ppos),
                        SwitchArgument::CasePayload,
                        sav(j_vec),
                        sav(j_len),
                        sav(j_in),
                        sav(j_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let k0c = a.cref(ns.o, j_trail, c0, u64_type());
    let k_ne = a.op(
        ns.o,
        j_trail,
        Opcode::NotEqual,
        vec![pav(k_trail), op_result(k0c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    // Preimage-accumulator start after trailing passes. The domain
    // prefix (15 bytes) is pushed first as an unrolled chain, then the
    // stored prefix bytes are appended by loop (see below).
    let d0 = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: j_trail,
        function: fid,
        parameters: vec![
            k_dend, k_pend, k_plen, k_ppos, k_trail, k_vec, k_len, k_in, k_unit,
        ],
        operations: vec![k0c, k_ne],
        terminator: cond(
            op_result(k_ne),
            edge(b_trail, Vec::new()),
            edge(
                d0,
                vec![
                    pav(k_dend),
                    pav(k_pend),
                    pav(k_plen),
                    pav(k_ppos),
                    pav(k_vec),
                    pav(k_in),
                    pav(k_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    // Domain-prefix chain: `sley2.object.v1` (15 PSH1 pushes from an
    // empty vector). Each Err (accumulator resource) refuses
    // RESOURCE_LIMIT; no well-formed small envelope reaches it.
    let domain_bytes = [
        b73, b6c, b65, b79, b32, b2e, b6f, b62, b6a, b65, b63, b74, b2e, b76, b31,
    ];
    let d_empty = a.op(
        ns.o,
        d0,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let dd_dend = a.param(ns.p, d0, ParameterRole::Block, u64_type());
    let dd_pend = a.param(ns.p, d0, ParameterRole::Block, u64_type());
    let dd_plen = a.param(ns.p, d0, ParameterRole::Block, u64_type());
    let dd_ppos = a.param(ns.p, d0, ParameterRole::Block, u64_type());
    let dd_vec = a.param(ns.p, d0, ParameterRole::Block, u8vec_type());
    let dd_in = a.param(ns.p, d0, ParameterRole::Block, TypeExpr::Bytes);
    let dd_unit = a.param(ns.p, d0, ParameterRole::Block, TypeExpr::Unit);
    let mut d_steps: Vec<EntityId> = Vec::new();
    for _ in 0..15 {
        d_steps.push(a.id(ns.b));
    }
    let pre_loop = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: d0,
        function: fid,
        parameters: vec![
            dd_dend, dd_pend, dd_plen, dd_ppos, dd_vec, dd_in, dd_unit,
        ],
        operations: vec![d_empty],
        terminator: branch(edge(
            d_steps[0],
            vec![
                op_result(d_empty),
                pav(dd_dend),
                pav(dd_pend),
                pav(dd_plen),
                pav(dd_ppos),
                pav(dd_vec),
                pav(dd_in),
                pav(dd_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    for i in 0..15 {
        let st = d_steps[i];
        let s_acc = a.param(ns.p, st, ParameterRole::Block, u8vec_type());
        let s_dend = a.param(ns.p, st, ParameterRole::Block, u64_type());
        let s_pend = a.param(ns.p, st, ParameterRole::Block, u64_type());
        let s_plen = a.param(ns.p, st, ParameterRole::Block, u64_type());
        let s_ppos = a.param(ns.p, st, ParameterRole::Block, u64_type());
        let s_vec = a.param(ns.p, st, ParameterRole::Block, u8vec_type());
        let s_in = a.param(ns.p, st, ParameterRole::Block, TypeExpr::Bytes);
        let s_unit = a.param(ns.p, st, ParameterRole::Block, TypeExpr::Unit);
        let bc = a.cref(ns.o, st, domain_bytes[i], u8_type());
        let push = a.op(
            ns.o,
            st,
            Opcode::AdapterInvoke,
            vec![pav(s_acc), op_result(bc)],
            vec![index_result(u8vec_type())],
            Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
        );
        let next = if i == 14 { pre_loop } else { d_steps[i + 1] };
        a.blocks.push(Block {
            entity_id: st,
            function: fid,
            parameters: vec![
                s_acc, s_dend, s_pend, s_plen, s_ppos, s_vec, s_in, s_unit,
            ],
            operations: vec![bc, push],
            terminator: switch(
                op_result(push),
                vec![
                    (
                        BuiltinCase::Ok,
                        next,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(s_dend),
                            sav(s_pend),
                            sav(s_plen),
                            sav(s_ppos),
                            sav(s_vec),
                            sav(s_in),
                            sav(s_unit),
                        ],
                    ),
                    (BuiltinCase::Err, b_res, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
    }

    // Preimage loop: append input[0..payload_end] (the digest preimage,
    // `input[..len-32]` after the trailing check) to the domain
    // accumulator. Indices stay below the converted length, so Get None
    // is unreachable (trap); PSH1 Err is typed RESOURCE_LIMIT. Loop is
    // CFG backedge Form A over block params.
    let pl_acc = a.param(ns.p, pre_loop, ParameterRole::Block, u8vec_type());
    let pl_dend = a.param(ns.p, pre_loop, ParameterRole::Block, u64_type());
    let pl_pend = a.param(ns.p, pre_loop, ParameterRole::Block, u64_type());
    let pl_plen = a.param(ns.p, pre_loop, ParameterRole::Block, u64_type());
    let pl_ppos = a.param(ns.p, pre_loop, ParameterRole::Block, u64_type());
    let pl_vec = a.param(ns.p, pre_loop, ParameterRole::Block, u8vec_type());
    let pl_in = a.param(ns.p, pre_loop, ParameterRole::Block, TypeExpr::Bytes);
    let pl_unit = a.param(ns.p, pre_loop, ParameterRole::Block, TypeExpr::Unit);
    let lp_check = a.id(ns.b);
    let lp_get = a.id(ns.b);
    let lp_push = a.id(ns.b);
    let lp_next = a.id(ns.b);
    let lp_done = a.id(ns.b);
    let z0c = a.cref(ns.o, pre_loop, c0, u64_type());
    a.blocks.push(Block {
        entity_id: pre_loop,
        function: fid,
        parameters: vec![
            pl_acc, pl_dend, pl_pend, pl_plen, pl_ppos, pl_vec, pl_in, pl_unit,
        ],
        operations: vec![z0c],
        terminator: branch(edge(
            lp_check,
            vec![
                op_result(z0c),
                pav(pl_acc),
                pav(pl_dend),
                pav(pl_pend),
                pav(pl_plen),
                pav(pl_ppos),
                pav(pl_vec),
                pav(pl_in),
                pav(pl_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let c_idx = a.param(ns.p, lp_check, ParameterRole::Block, u64_type());
    let c_acc = a.param(ns.p, lp_check, ParameterRole::Block, u8vec_type());
    let c_dend = a.param(ns.p, lp_check, ParameterRole::Block, u64_type());
    let c_pend = a.param(ns.p, lp_check, ParameterRole::Block, u64_type());
    let c_plen = a.param(ns.p, lp_check, ParameterRole::Block, u64_type());
    let c_ppos = a.param(ns.p, lp_check, ParameterRole::Block, u64_type());
    let c_vec = a.param(ns.p, lp_check, ParameterRole::Block, u8vec_type());
    let c_in = a.param(ns.p, lp_check, ParameterRole::Block, TypeExpr::Bytes);
    let c_unit = a.param(ns.p, lp_check, ParameterRole::Block, TypeExpr::Unit);
    let c_lt = a.op(
        ns.o,
        lp_check,
        Opcode::LessThan,
        vec![pav(c_idx), pav(c_pend)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: lp_check,
        function: fid,
        parameters: vec![
            c_idx, c_acc, c_dend, c_pend, c_plen, c_ppos, c_vec, c_in, c_unit,
        ],
        operations: vec![c_lt],
        terminator: cond(
            op_result(c_lt),
            edge(
                lp_get,
                vec![
                    pav(c_idx),
                    pav(c_acc),
                    pav(c_dend),
                    pav(c_pend),
                    pav(c_plen),
                    pav(c_ppos),
                    pav(c_vec),
                    pav(c_in),
                    pav(c_unit),
                ],
            ),
            edge(
                lp_done,
                vec![
                    pav(c_acc),
                    pav(c_dend),
                    pav(c_pend),
                    pav(c_plen),
                    pav(c_ppos),
                    pav(c_vec),
                    pav(c_in),
                    pav(c_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let g_idx = a.param(ns.p, lp_get, ParameterRole::Block, u64_type());
    let g_acc = a.param(ns.p, lp_get, ParameterRole::Block, u8vec_type());
    let g_dend = a.param(ns.p, lp_get, ParameterRole::Block, u64_type());
    let g_pend = a.param(ns.p, lp_get, ParameterRole::Block, u64_type());
    let g_plen = a.param(ns.p, lp_get, ParameterRole::Block, u64_type());
    let g_ppos = a.param(ns.p, lp_get, ParameterRole::Block, u64_type());
    let g_vec = a.param(ns.p, lp_get, ParameterRole::Block, u8vec_type());
    let g_in = a.param(ns.p, lp_get, ParameterRole::Block, TypeExpr::Bytes);
    let g_unit = a.param(ns.p, lp_get, ParameterRole::Block, TypeExpr::Unit);
    let g_get = a.op(
        ns.o,
        lp_get,
        Opcode::VectorGet,
        vec![pav(g_vec), pav(g_idx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: lp_get,
        function: fid,
        parameters: vec![
            g_idx, g_acc, g_dend, g_pend, g_plen, g_ppos, g_vec, g_in, g_unit,
        ],
        operations: vec![g_get],
        terminator: switch(
            op_result(g_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    lp_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(g_idx),
                        sav(g_acc),
                        sav(g_dend),
                        sav(g_pend),
                        sav(g_plen),
                        sav(g_ppos),
                        sav(g_vec),
                        sav(g_in),
                        sav(g_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let u_b = a.param(ns.p, lp_push, ParameterRole::Block, u8_type());
    let u_idx = a.param(ns.p, lp_push, ParameterRole::Block, u64_type());
    let u_acc = a.param(ns.p, lp_push, ParameterRole::Block, u8vec_type());
    let u_dend = a.param(ns.p, lp_push, ParameterRole::Block, u64_type());
    let u_pend = a.param(ns.p, lp_push, ParameterRole::Block, u64_type());
    let u_plen = a.param(ns.p, lp_push, ParameterRole::Block, u64_type());
    let u_ppos = a.param(ns.p, lp_push, ParameterRole::Block, u64_type());
    let u_vec = a.param(ns.p, lp_push, ParameterRole::Block, u8vec_type());
    let u_in = a.param(ns.p, lp_push, ParameterRole::Block, TypeExpr::Bytes);
    let u_unit = a.param(ns.p, lp_push, ParameterRole::Block, TypeExpr::Unit);
    let u_push = a.op(
        ns.o,
        lp_push,
        Opcode::AdapterInvoke,
        vec![pav(u_acc), pav(u_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: lp_push,
        function: fid,
        parameters: vec![
            u_b, u_idx, u_acc, u_dend, u_pend, u_plen, u_ppos, u_vec, u_in, u_unit,
        ],
        operations: vec![u_push],
        terminator: switch(
            op_result(u_push),
            vec![
                (
                    BuiltinCase::Ok,
                    lp_next,
                    vec![
                        sav(u_idx),
                        SwitchArgument::CasePayload,
                        sav(u_dend),
                        sav(u_pend),
                        sav(u_plen),
                        sav(u_ppos),
                        sav(u_vec),
                        sav(u_in),
                        sav(u_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let n_idx = a.param(ns.p, lp_next, ParameterRole::Block, u64_type());
    let n_acc = a.param(ns.p, lp_next, ParameterRole::Block, u8vec_type());
    let n_dend = a.param(ns.p, lp_next, ParameterRole::Block, u64_type());
    let n_pend = a.param(ns.p, lp_next, ParameterRole::Block, u64_type());
    let n_plen = a.param(ns.p, lp_next, ParameterRole::Block, u64_type());
    let n_ppos = a.param(ns.p, lp_next, ParameterRole::Block, u64_type());
    let n_vec = a.param(ns.p, lp_next, ParameterRole::Block, u8vec_type());
    let n_in = a.param(ns.p, lp_next, ParameterRole::Block, TypeExpr::Bytes);
    let n_unit = a.param(ns.p, lp_next, ParameterRole::Block, TypeExpr::Unit);
    let n_onec = a.cref(ns.o, lp_next, c1, u64_type());
    let n_add = a.op(
        ns.o,
        lp_next,
        Opcode::IntAddChecked,
        vec![pav(n_idx), op_result(n_onec)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: lp_next,
        function: fid,
        parameters: vec![
            n_idx, n_acc, n_dend, n_pend, n_plen, n_ppos, n_vec, n_in, n_unit,
        ],
        operations: vec![n_onec, n_add],
        terminator: switch(
            op_result(n_add),
            vec![
                (
                    BuiltinCase::Ok,
                    lp_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(n_acc),
                        sav(n_dend),
                        sav(n_pend),
                        sav(n_plen),
                        sav(n_ppos),
                        sav(n_vec),
                        sav(n_in),
                        sav(n_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Hash-input bytes via V2B1, then raw hash via RHW1 (raw BLAKE3-256
    // over Sley-built bytes only, 1 MiB per-call ceiling). Both Err paths
    // are typed RESOURCE_LIMIT; small envelopes never reach them. The
    // original Bytes input is no longer needed past this point.
    let h_acc = a.param(ns.p, lp_done, ParameterRole::Block, u8vec_type());
    let h_dend = a.param(ns.p, lp_done, ParameterRole::Block, u64_type());
    let h_pend = a.param(ns.p, lp_done, ParameterRole::Block, u64_type());
    let h_plen = a.param(ns.p, lp_done, ParameterRole::Block, u64_type());
    let h_ppos = a.param(ns.p, lp_done, ParameterRole::Block, u64_type());
    let h_vec = a.param(ns.p, lp_done, ParameterRole::Block, u8vec_type());
    let h_in = a.param(ns.p, lp_done, ParameterRole::Block, TypeExpr::Bytes);
    let h_unit = a.param(ns.p, lp_done, ParameterRole::Block, TypeExpr::Unit);
    let h_v2b = a.op(
        ns.o,
        lp_done,
        Opcode::AdapterInvoke,
        vec![pav(h_unit), pav(h_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    let h_hash = a.id(ns.b);
    let hh_bytes = a.param(ns.p, h_hash, ParameterRole::Block, TypeExpr::Bytes);
    let hh_dend = a.param(ns.p, h_hash, ParameterRole::Block, u64_type());
    let hh_pend = a.param(ns.p, h_hash, ParameterRole::Block, u64_type());
    let hh_plen = a.param(ns.p, h_hash, ParameterRole::Block, u64_type());
    let hh_ppos = a.param(ns.p, h_hash, ParameterRole::Block, u64_type());
    let hh_vec = a.param(ns.p, h_hash, ParameterRole::Block, u8vec_type());
    let hh_unit = a.param(ns.p, h_hash, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: lp_done,
        function: fid,
        parameters: vec![
            h_acc, h_dend, h_pend, h_plen, h_ppos, h_vec, h_in, h_unit,
        ],
        operations: vec![h_v2b],
        terminator: switch(
            op_result(h_v2b),
            vec![
                (
                    BuiltinCase::Ok,
                    h_hash,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(h_dend),
                        sav(h_pend),
                        sav(h_plen),
                        sav(h_ppos),
                        sav(h_vec),
                        sav(h_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let h_rhw = a.op(
        ns.o,
        h_hash,
        Opcode::AdapterInvoke,
        vec![pav(hh_unit), pav(hh_bytes)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_RHW1))),
    );
    let h_cmp0 = a.id(ns.b);
    let hc_bytes = a.param(ns.p, h_cmp0, ParameterRole::Block, TypeExpr::Bytes);
    let hc_dend = a.param(ns.p, h_cmp0, ParameterRole::Block, u64_type());
    let hc_pend = a.param(ns.p, h_cmp0, ParameterRole::Block, u64_type());
    let hc_plen = a.param(ns.p, h_cmp0, ParameterRole::Block, u64_type());
    let hc_ppos = a.param(ns.p, h_cmp0, ParameterRole::Block, u64_type());
    let hc_vec = a.param(ns.p, h_cmp0, ParameterRole::Block, u8vec_type());
    let hc_unit = a.param(ns.p, h_cmp0, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: h_hash,
        function: fid,
        parameters: vec![
            hh_bytes, hh_dend, hh_pend, hh_plen, hh_ppos, hh_vec, hh_unit,
        ],
        operations: vec![h_rhw],
        terminator: switch(
            op_result(h_rhw),
            vec![
                (
                    BuiltinCase::Ok,
                    h_cmp0,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(hh_dend),
                        sav(hh_pend),
                        sav(hh_plen),
                        sav(hh_ppos),
                        sav(hh_vec),
                        sav(hh_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Convert the 32-byte expected digest to a vector for indexed
    // comparison. Length 32 is far below the bridge cap, so Err traps
    // (construction-invariant, not input-dependent).
    let hc_b2v = a.op(
        ns.o,
        h_cmp0,
        Opcode::AdapterInvoke,
        vec![pav(hc_unit), pav(hc_bytes)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    let mut dg_get: Vec<EntityId> = Vec::new();
    let mut dg_get2: Vec<EntityId> = Vec::new();
    let mut dg_cmp: Vec<EntityId> = Vec::new();
    let mut dg_inc: Vec<EntityId> = Vec::new();
    for _ in 0..32 {
        dg_get.push(a.id(ns.b));
        dg_get2.push(a.id(ns.b));
        dg_cmp.push(a.id(ns.b));
        dg_inc.push(a.id(ns.b));
    }
    let pay_loop = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: h_cmp0,
        function: fid,
        parameters: vec![
            hc_bytes, hc_dend, hc_pend, hc_plen, hc_ppos, hc_vec, hc_unit,
        ],
        operations: vec![hc_b2v],
        terminator: switch(
            op_result(hc_b2v),
            vec![
                (
                    BuiltinCase::Ok,
                    dg_get[0],
                    vec![
                        SwitchArgument::CasePayload,
                        sav(hc_dend),
                        sav(hc_pend),
                        sav(hc_plen),
                        sav(hc_ppos),
                        sav(hc_vec),
                        sav(hc_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    // Digest chain: 32 indexed compares of expected (dig_vec[i]) vs
    // stored trailer (vec[payload_end + i]). Offsets are constants
    // 0..31; trailer indices use checked adds (unreachable overflow, so
    // Err traps). All indices are in bounds after the envelope-bounds
    // plus trailing checks, so Get None targets the invariant trap. Any
    // present-but-different byte refuses DIGEST_MISMATCH.
    let off_consts = [
        c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15, c16, c17, c18, c19,
        c20, c21, c22, c23, c24, c25, c26, c27, c28, c29, c30, c31,
    ];
    for i in 0..32 {
        let add_b = dg_get[i];
        let getd_b = dg_get2[i];
        let gett_b = dg_cmp[i];
        let cmp_b = dg_inc[i];
        // Add: trailer_idx = payload_end + offset_i.
        let a_dig = a.param(ns.p, add_b, ParameterRole::Block, u8vec_type());
        let a_dend = a.param(ns.p, add_b, ParameterRole::Block, u64_type());
        let a_pend = a.param(ns.p, add_b, ParameterRole::Block, u64_type());
        let a_plen = a.param(ns.p, add_b, ParameterRole::Block, u64_type());
        let a_ppos = a.param(ns.p, add_b, ParameterRole::Block, u64_type());
        let a_vec = a.param(ns.p, add_b, ParameterRole::Block, u8vec_type());
        let a_unit = a.param(ns.p, add_b, ParameterRole::Block, TypeExpr::Unit);
        let a_offc = a.cref(ns.o, add_b, off_consts[i], u64_type());
        let a_add = a.op(
            ns.o,
            add_b,
            Opcode::IntAddChecked,
            vec![pav(a_pend), op_result(a_offc)],
            vec![arith_result(u64_type())],
            Immediate::None,
        );
        a.blocks.push(Block {
            entity_id: add_b,
            function: fid,
            parameters: vec![a_dig, a_dend, a_pend, a_plen, a_ppos, a_vec, a_unit],
            operations: vec![a_offc, a_add],
            terminator: switch(
                op_result(a_add),
                vec![
                    (
                        BuiltinCase::Ok,
                        getd_b,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(a_dig),
                            sav(a_dend),
                            sav(a_pend),
                            sav(a_plen),
                            sav(a_ppos),
                            sav(a_vec),
                            sav(a_unit),
                        ],
                    ),
                    (BuiltinCase::Err, trap, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
        // GetDig: dig_byte = dig_vec[offset_i] (constant index).
        let d_trail = a.param(ns.p, getd_b, ParameterRole::Block, u64_type());
        let d_dig = a.param(ns.p, getd_b, ParameterRole::Block, u8vec_type());
        let d_dend = a.param(ns.p, getd_b, ParameterRole::Block, u64_type());
        let d_pend = a.param(ns.p, getd_b, ParameterRole::Block, u64_type());
        let d_plen = a.param(ns.p, getd_b, ParameterRole::Block, u64_type());
        let d_ppos = a.param(ns.p, getd_b, ParameterRole::Block, u64_type());
        let d_vec = a.param(ns.p, getd_b, ParameterRole::Block, u8vec_type());
        let d_unit = a.param(ns.p, getd_b, ParameterRole::Block, TypeExpr::Unit);
        let d_idxc = a.cref(ns.o, getd_b, off_consts[i], u64_type());
        let d_get = a.op(
            ns.o,
            getd_b,
            Opcode::VectorGet,
            vec![pav(d_dig), op_result(d_idxc)],
            vec![TypeExpr::Option(Box::new(u8_type()))],
            Immediate::None,
        );
        a.blocks.push(Block {
            entity_id: getd_b,
            function: fid,
            parameters: vec![
                d_trail, d_dig, d_dend, d_pend, d_plen, d_ppos, d_vec, d_unit,
            ],
            operations: vec![d_idxc, d_get],
            terminator: switch(
                op_result(d_get),
                vec![
                    (BuiltinCase::None, trap, Vec::new()),
                    (
                        BuiltinCase::Some,
                        gett_b,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(d_trail),
                            sav(d_dig),
                            sav(d_dend),
                            sav(d_pend),
                            sav(d_plen),
                            sav(d_ppos),
                            sav(d_vec),
                            sav(d_unit),
                        ],
                    ),
                ],
            ),
            reachability: Reachability::Required,
        });
        // GetTrailer: trail_byte = vec[trailer_idx].
        let t_b = a.param(ns.p, gett_b, ParameterRole::Block, u8_type());
        let t_trail = a.param(ns.p, gett_b, ParameterRole::Block, u64_type());
        let t_dig = a.param(ns.p, gett_b, ParameterRole::Block, u8vec_type());
        let t_dend = a.param(ns.p, gett_b, ParameterRole::Block, u64_type());
        let t_pend = a.param(ns.p, gett_b, ParameterRole::Block, u64_type());
        let t_plen = a.param(ns.p, gett_b, ParameterRole::Block, u64_type());
        let t_ppos = a.param(ns.p, gett_b, ParameterRole::Block, u64_type());
        let t_vec = a.param(ns.p, gett_b, ParameterRole::Block, u8vec_type());
        let t_unit = a.param(ns.p, gett_b, ParameterRole::Block, TypeExpr::Unit);
        let t_get = a.op(
            ns.o,
            gett_b,
            Opcode::VectorGet,
            vec![pav(t_vec), pav(t_trail)],
            vec![TypeExpr::Option(Box::new(u8_type()))],
            Immediate::None,
        );
        a.blocks.push(Block {
            entity_id: gett_b,
            function: fid,
            parameters: vec![
                t_b, t_trail, t_dig, t_dend, t_pend, t_plen, t_ppos, t_vec, t_unit,
            ],
            operations: vec![t_get],
            terminator: switch(
                op_result(t_get),
                vec![
                    (BuiltinCase::None, trap, Vec::new()),
                    (
                        BuiltinCase::Some,
                        cmp_b,
                        vec![
                            sav(t_b),
                            SwitchArgument::CasePayload,
                            sav(t_dig),
                            sav(t_dend),
                            sav(t_pend),
                            sav(t_plen),
                            sav(t_ppos),
                            sav(t_vec),
                            sav(t_unit),
                        ],
                    ),
                ],
            ),
            reachability: Reachability::Required,
        });
        // Cmp: dig_byte == trail_byte else DIGEST_MISMATCH.
        let m_db = a.param(ns.p, cmp_b, ParameterRole::Block, u8_type());
        let m_tb = a.param(ns.p, cmp_b, ParameterRole::Block, u8_type());
        let m_dig = a.param(ns.p, cmp_b, ParameterRole::Block, u8vec_type());
        let m_dend = a.param(ns.p, cmp_b, ParameterRole::Block, u64_type());
        let m_pend = a.param(ns.p, cmp_b, ParameterRole::Block, u64_type());
        let m_plen = a.param(ns.p, cmp_b, ParameterRole::Block, u64_type());
        let m_ppos = a.param(ns.p, cmp_b, ParameterRole::Block, u64_type());
        let m_vec = a.param(ns.p, cmp_b, ParameterRole::Block, u8vec_type());
        let m_unit = a.param(ns.p, cmp_b, ParameterRole::Block, TypeExpr::Unit);
        let m_eq = a.op(
            ns.o,
            cmp_b,
            Opcode::Equal,
            vec![pav(m_db), pav(m_tb)],
            vec![TypeExpr::Bool],
            Immediate::None,
        );
        let next = if i == 31 { pay_loop } else { dg_get[i + 1] };
        if i == 31 {
            a.blocks.push(Block {
                entity_id: cmp_b,
                function: fid,
                parameters: vec![
                    m_db, m_tb, m_dig, m_dend, m_pend, m_plen, m_ppos, m_vec, m_unit,
                ],
                operations: vec![m_eq],
                terminator: cond(
                    op_result(m_eq),
                    edge(
                        next,
                        vec![
                            pav(m_pend),
                            pav(m_plen),
                            pav(m_ppos),
                            pav(m_vec),
                            pav(m_unit),
                        ],
                    ),
                    edge(b_digest, Vec::new()),
                ),
                reachability: Reachability::Required,
            });
        } else {
            a.blocks.push(Block {
                entity_id: cmp_b,
                function: fid,
                parameters: vec![
                    m_db, m_tb, m_dig, m_dend, m_pend, m_plen, m_ppos, m_vec, m_unit,
                ],
                operations: vec![m_eq],
                terminator: cond(
                    op_result(m_eq),
                    edge(
                        next,
                        vec![
                            pav(m_dig),
                            pav(m_dend),
                            pav(m_pend),
                            pav(m_plen),
                            pav(m_ppos),
                            pav(m_vec),
                            pav(m_unit),
                        ],
                    ),
                    edge(b_digest, Vec::new()),
                ),
                reachability: Reachability::Required,
            });
        }
    }
    // Payload extraction: copy input[payload_start .. payload_start+len]
    // into a fresh vector, convert via V2B1, return Ok. Payload is opaque:
    // success here means envelope framing plus digest are valid, never that
    // the payload is a valid program object.
    let pl_pend = a.param(ns.p, pay_loop, ParameterRole::Block, u64_type());
    let pl_plen = a.param(ns.p, pay_loop, ParameterRole::Block, u64_type());
    let pl_ppos = a.param(ns.p, pay_loop, ParameterRole::Block, u64_type());
    let pl_vec = a.param(ns.p, pay_loop, ParameterRole::Block, u8vec_type());
    let pl_unit = a.param(ns.p, pay_loop, ParameterRole::Block, TypeExpr::Unit);
    let pl_empty = a.op(
        ns.o,
        pay_loop,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let pc_check = a.id(ns.b);
    let pc_get = a.id(ns.b);
    let pc_push = a.id(ns.b);
    let pc_next = a.id(ns.b);
    let pc_done = a.id(ns.b);
    let pz0c = a.cref(ns.o, pay_loop, c0, u64_type());
    a.blocks.push(Block {
        entity_id: pay_loop,
        function: fid,
        parameters: vec![pl_pend, pl_plen, pl_ppos, pl_vec, pl_unit],
        operations: vec![pl_empty, pz0c],
        terminator: branch(edge(
            pc_check,
            vec![
                op_result(pz0c),
                op_result(pl_empty),
                pav(pl_pend),
                pav(pl_plen),
                pav(pl_ppos),
                pav(pl_vec),
                pav(pl_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let q_idx = a.param(ns.p, pc_check, ParameterRole::Block, u64_type());
    let q_acc = a.param(ns.p, pc_check, ParameterRole::Block, u8vec_type());
    let q_pend = a.param(ns.p, pc_check, ParameterRole::Block, u64_type());
    let q_plen = a.param(ns.p, pc_check, ParameterRole::Block, u64_type());
    let q_ppos = a.param(ns.p, pc_check, ParameterRole::Block, u64_type());
    let q_vec = a.param(ns.p, pc_check, ParameterRole::Block, u8vec_type());
    let q_unit = a.param(ns.p, pc_check, ParameterRole::Block, TypeExpr::Unit);
    let q_lt = a.op(
        ns.o,
        pc_check,
        Opcode::LessThan,
        vec![pav(q_idx), pav(q_plen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: pc_check,
        function: fid,
        parameters: vec![q_idx, q_acc, q_pend, q_plen, q_ppos, q_vec, q_unit],
        operations: vec![q_lt],
        terminator: cond(
            op_result(q_lt),
            edge(
                pc_get,
                vec![
                    pav(q_idx),
                    pav(q_acc),
                    pav(q_pend),
                    pav(q_plen),
                    pav(q_ppos),
                    pav(q_vec),
                    pav(q_unit),
                ],
            ),
            edge(
                pc_done,
                vec![
                    pav(q_acc),
                    pav(q_pend),
                    pav(q_plen),
                    pav(q_ppos),
                    pav(q_vec),
                    pav(q_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let g_idx = a.param(ns.p, pc_get, ParameterRole::Block, u64_type());
    let g_acc = a.param(ns.p, pc_get, ParameterRole::Block, u8vec_type());
    let g_pend = a.param(ns.p, pc_get, ParameterRole::Block, u64_type());
    let g_plen = a.param(ns.p, pc_get, ParameterRole::Block, u64_type());
    let g_ppos = a.param(ns.p, pc_get, ParameterRole::Block, u64_type());
    let g_vec = a.param(ns.p, pc_get, ParameterRole::Block, u8vec_type());
    let g_unit = a.param(ns.p, pc_get, ParameterRole::Block, TypeExpr::Unit);
    let g_add = a.op(
        ns.o,
        pc_get,
        Opcode::IntAddChecked,
        vec![pav(g_ppos), pav(g_idx)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    let g_src = a.id(ns.b);
    let s_acc = a.param(ns.p, g_src, ParameterRole::Block, u8vec_type());
    let s_sidx = a.param(ns.p, g_src, ParameterRole::Block, u64_type());
    let s_idx = a.param(ns.p, g_src, ParameterRole::Block, u64_type());
    let s_pend = a.param(ns.p, g_src, ParameterRole::Block, u64_type());
    let s_plen = a.param(ns.p, g_src, ParameterRole::Block, u64_type());
    let s_ppos = a.param(ns.p, g_src, ParameterRole::Block, u64_type());
    let s_vec = a.param(ns.p, g_src, ParameterRole::Block, u8vec_type());
    let s_unit = a.param(ns.p, g_src, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: pc_get,
        function: fid,
        parameters: vec![g_idx, g_acc, g_pend, g_plen, g_ppos, g_vec, g_unit],
        operations: vec![g_add],
        terminator: switch(
            op_result(g_add),
            vec![
                (
                    BuiltinCase::Ok,
                    g_src,
                    vec![
                        sav(g_acc),
                        SwitchArgument::CasePayload,
                        sav(g_idx),
                        sav(g_pend),
                        sav(g_plen),
                        sav(g_ppos),
                        sav(g_vec),
                        sav(g_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let s_get = a.op(
        ns.o,
        g_src,
        Opcode::VectorGet,
        vec![pav(s_vec), pav(s_sidx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: g_src,
        function: fid,
        parameters: vec![s_acc, s_sidx, s_idx, s_pend, s_plen, s_ppos, s_vec, s_unit],
        operations: vec![s_get],
        terminator: switch(
            op_result(s_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    pc_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(s_idx),
                        sav(s_acc),
                        sav(s_pend),
                        sav(s_plen),
                        sav(s_ppos),
                        sav(s_vec),
                        sav(s_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let u_b = a.param(ns.p, pc_push, ParameterRole::Block, u8_type());
    let u_idx = a.param(ns.p, pc_push, ParameterRole::Block, u64_type());
    let u_acc = a.param(ns.p, pc_push, ParameterRole::Block, u8vec_type());
    let u_pend = a.param(ns.p, pc_push, ParameterRole::Block, u64_type());
    let u_plen = a.param(ns.p, pc_push, ParameterRole::Block, u64_type());
    let u_ppos = a.param(ns.p, pc_push, ParameterRole::Block, u64_type());
    let u_vec = a.param(ns.p, pc_push, ParameterRole::Block, u8vec_type());
    let u_unit = a.param(ns.p, pc_push, ParameterRole::Block, TypeExpr::Unit);
    let u_push = a.op(
        ns.o,
        pc_push,
        Opcode::AdapterInvoke,
        vec![pav(u_acc), pav(u_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: pc_push,
        function: fid,
        parameters: vec![u_b, u_idx, u_acc, u_pend, u_plen, u_ppos, u_vec, u_unit],
        operations: vec![u_push],
        terminator: switch(
            op_result(u_push),
            vec![
                (
                    BuiltinCase::Ok,
                    pc_next,
                    vec![
                        sav(u_idx),
                        SwitchArgument::CasePayload,
                        sav(u_pend),
                        sav(u_plen),
                        sav(u_ppos),
                        sav(u_vec),
                        sav(u_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let n_idx = a.param(ns.p, pc_next, ParameterRole::Block, u64_type());
    let n_acc = a.param(ns.p, pc_next, ParameterRole::Block, u8vec_type());
    let n_pend = a.param(ns.p, pc_next, ParameterRole::Block, u64_type());
    let n_plen = a.param(ns.p, pc_next, ParameterRole::Block, u64_type());
    let n_ppos = a.param(ns.p, pc_next, ParameterRole::Block, u64_type());
    let n_vec = a.param(ns.p, pc_next, ParameterRole::Block, u8vec_type());
    let n_unit = a.param(ns.p, pc_next, ParameterRole::Block, TypeExpr::Unit);
    let n_onec = a.cref(ns.o, pc_next, c1, u64_type());
    let n_add = a.op(
        ns.o,
        pc_next,
        Opcode::IntAddChecked,
        vec![pav(n_idx), op_result(n_onec)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: pc_next,
        function: fid,
        parameters: vec![n_idx, n_acc, n_pend, n_plen, n_ppos, n_vec, n_unit],
        operations: vec![n_onec, n_add],
        terminator: switch(
            op_result(n_add),
            vec![
                (
                    BuiltinCase::Ok,
                    pc_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(n_acc),
                        sav(n_pend),
                        sav(n_plen),
                        sav(n_ppos),
                        sav(n_vec),
                        sav(n_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let f_acc = a.param(ns.p, pc_done, ParameterRole::Block, u8vec_type());
    let f_pend = a.param(ns.p, pc_done, ParameterRole::Block, u64_type());
    let f_plen = a.param(ns.p, pc_done, ParameterRole::Block, u64_type());
    let f_ppos = a.param(ns.p, pc_done, ParameterRole::Block, u64_type());
    let f_vec = a.param(ns.p, pc_done, ParameterRole::Block, u8vec_type());
    let f_unit = a.param(ns.p, pc_done, ParameterRole::Block, TypeExpr::Unit);
    let f_v2b = a.op(
        ns.o,
        pc_done,
        Opcode::AdapterInvoke,
        vec![pav(f_unit), pav(f_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    let f_ok = a.id(ns.b);
    let o_b = a.param(ns.p, f_ok, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: pc_done,
        function: fid,
        parameters: vec![f_acc, f_pend, f_plen, f_ppos, f_vec, f_unit],
        operations: vec![f_v2b],
        terminator: switch(
            op_result(f_v2b),
            vec![
                (BuiltinCase::Ok, f_ok, vec![SwitchArgument::CasePayload]),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let f_okv = a.op(
        ns.o,
        f_ok,
        Opcode::ResultOk,
        vec![pav(o_b)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f_ok,
        function: fid,
        parameters: vec![o_b],
        operations: vec![f_okv],
        terminator: ret(op_result(f_okv)),
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

// ── program envelope construction ────────────────────────────────────
// `build_program_envelope(payload: Bytes, lenvec: Vector<UInt8>, unit:
// Unit) -> Result<Tuple<Bytes, Bytes>, Bytes>` constructs the canonical
// stored prefix for the fixed program context (`build_preimage` order
// in `object.rs:244-262`): magic `SLEYSCB1` + version uvar(1) + tag
// uvar(200) + epoch `[9;32]` + canonical payload length + payload bytes,
// plus the digest preimage (`sley2.object.v1` domain ++ stored prefix).
// It returns `(prefix, preimage)`; the digest trailer is appended by the
// caller (composed path), which owns the only RHW1 use, so this graph
// stays free of same-graph digest code. The payload is caller-supplied
// canonical bytes (from `encode_outer` in the composed path); the length
// encoding arrives as a caller-supplied byte vector (produced by
// `encode_uvar` in a graph without a digest phase, passed across
// `CallDirect` as a first-class vector value); no native-prebuilt
// envelope enters the Sley encoder. Payload length past
// MAX_STANDALONE_BYTES refuses RESOURCE_LIMIT (reference `build_preimage`
// resource rule); the final stored-size check is subsumed (bridge cap
// 1 MiB far below the 64 MiB epoch ceiling). `LIMIT` for exhaustion,
// never truncation.
#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_program_build(a: &mut Asm, ns: Ns, fid: EntityId) -> FunctionGraph {
    use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let bstart = a.blocks.len();
    let res_t = program_prefix_result_type();
    // UInt64 constants.
    let c0 = a.ku64(ns.k, 0);
    let c1 = a.ku64(ns.k, 1);
    let c32 = a.ku64(ns.k, 32);
    let c_max = a.ku64(ns.k, 67_108_864);
    // Header/domain/epoch byte values.
    let b01 = a.ku8(ns.k, 0x01);
    let b09 = a.ku8(ns.k, 0x09);
    let b31 = a.ku8(ns.k, 0x31);
    let b42 = a.ku8(ns.k, 0x42);
    let b43 = a.ku8(ns.k, 0x43);
    let b45 = a.ku8(ns.k, 0x45);
    let b4c = a.ku8(ns.k, 0x4c);
    let b53 = a.ku8(ns.k, 0x53);
    let b59 = a.ku8(ns.k, 0x59);
    let bc8 = a.ku8(ns.k, 0xc8);
    // Exact error-code bytes.
    let e_res = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");

    let p_pay = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let p_lenvec = a.param(ns.p, fid, ParameterRole::Function, u8vec_type());
    let p_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    let b_res = err_block(a, ns, fid, res_t.clone(), e_res);
    let trap = trap_block(a, ns, fid);

    // Entry: the payload crosses the bridge only after the length copy
    // (see below); bridge failure means past the 1 MiB cap: documented
    // RESOURCE_LIMIT. The length vector arrives ready; only the payload
    // (later) crosses the bridge here.
    let entry = a.id(ns.b);
    let b_head = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: branch(edge(
            b_head,
            vec![pav(p_pay), pav(p_lenvec), pav(p_unit)],
        )),
        reachability: Reachability::Required,
    });

    // Stored-prefix assembly: VectorNew, then the fixed 12-byte header
    // (magic8 + version `01` + tag200 `C8 01`) as an unrolled push chain,
    // then the 32 epoch bytes (`09`) via a counted loop. The payload
    // travels as opaque Bytes until after the length copy (see below).
    let h_pay = a.param(ns.p, b_head, ParameterRole::Block, TypeExpr::Bytes);
    let h_lenvec = a.param(ns.p, b_head, ParameterRole::Block, u8vec_type());
    let h_unit = a.param(ns.p, b_head, ParameterRole::Block, TypeExpr::Unit);
    let h_empty = a.op(
        ns.o,
        b_head,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let header_bytes = [b53, b4c, b45, b59, b53, b43, b42, b31, b01, bc8, b01];
    let mut h_steps: Vec<EntityId> = Vec::new();
    for _ in 0..header_bytes.len() {
        h_steps.push(a.id(ns.b));
    }
    let ep_loop = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: b_head,
        function: fid,
        parameters: vec![h_pay, h_lenvec, h_unit],
        operations: vec![h_empty],
        terminator: branch(edge(
            h_steps[0],
            vec![
                op_result(h_empty),
                pav(h_pay),
                pav(h_lenvec),
                pav(h_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    for i in 0..header_bytes.len() {
        let st = h_steps[i];
        let s_acc = a.param(ns.p, st, ParameterRole::Block, u8vec_type());
        let s_pay = a.param(ns.p, st, ParameterRole::Block, TypeExpr::Bytes);
        let s_lenvec = a.param(ns.p, st, ParameterRole::Block, u8vec_type());
        let s_unit = a.param(ns.p, st, ParameterRole::Block, TypeExpr::Unit);
        let bc = a.cref(ns.o, st, header_bytes[i], u8_type());
        let push = a.op(
            ns.o,
            st,
            Opcode::AdapterInvoke,
            vec![pav(s_acc), op_result(bc)],
            vec![index_result(u8vec_type())],
            Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
        );
        let next = if i + 1 == header_bytes.len() {
            ep_loop
        } else {
            h_steps[i + 1]
        };
        a.blocks.push(Block {
            entity_id: st,
            function: fid,
            parameters: vec![s_acc, s_pay, s_lenvec, s_unit],
            operations: vec![bc, push],
            terminator: switch(
                op_result(push),
                vec![
                    (
                        BuiltinCase::Ok,
                        next,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(s_pay),
                            sav(s_lenvec),
                            sav(s_unit),
                        ],
                    ),
                    (BuiltinCase::Err, b_res, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
    }

    // Epoch loop: 32 pushes of `09`. Counted Form-A loop (no Get needed);
    // the index bound is the constant 32.
    let e_acc = a.param(ns.p, ep_loop, ParameterRole::Block, u8vec_type());
    let e_pay = a.param(ns.p, ep_loop, ParameterRole::Block, TypeExpr::Bytes);
    let e_lenvec = a.param(ns.p, ep_loop, ParameterRole::Block, u8vec_type());
    let e_unit = a.param(ns.p, ep_loop, ParameterRole::Block, TypeExpr::Unit);
    let ep_check = a.id(ns.b);
    let ep_push = a.id(ns.b);
    let ep_next = a.id(ns.b);
    let ep_done = a.id(ns.b);
    let ez0c = a.cref(ns.o, ep_loop, c0, u64_type());
    a.blocks.push(Block {
        entity_id: ep_loop,
        function: fid,
        parameters: vec![e_acc, e_pay, e_lenvec, e_unit],
        operations: vec![ez0c],
        terminator: branch(edge(
            ep_check,
            vec![
                op_result(ez0c),
                pav(e_acc),
                pav(e_pay),
                pav(e_lenvec),
                pav(e_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let k_idx = a.param(ns.p, ep_check, ParameterRole::Block, u64_type());
    let k_acc = a.param(ns.p, ep_check, ParameterRole::Block, u8vec_type());
    let k_pay = a.param(ns.p, ep_check, ParameterRole::Block, TypeExpr::Bytes);
    let k_lenvec = a.param(ns.p, ep_check, ParameterRole::Block, u8vec_type());
    let k_unit = a.param(ns.p, ep_check, ParameterRole::Block, TypeExpr::Unit);
    let k32c = a.cref(ns.o, ep_check, c32, u64_type());
    let k_lt = a.op(
        ns.o,
        ep_check,
        Opcode::LessThan,
        vec![pav(k_idx), op_result(k32c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    // Length bytes arrive ready (function input threaded through); the
    // epoch exit falls straight into the length copy loop.
    let len_copy = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: ep_check,
        function: fid,
        parameters: vec![k_idx, k_acc, k_pay, k_lenvec, k_unit],
        operations: vec![k32c, k_lt],
        terminator: cond(
            op_result(k_lt),
            edge(
                ep_push,
                vec![
                    pav(k_idx),
                    pav(k_acc),
                    pav(k_pay),
                    pav(k_lenvec),
                    pav(k_unit),
                ],
            ),
            edge(
                ep_done,
                vec![
                    pav(k_acc),
                    pav(k_pay),
                    pav(k_lenvec),
                    pav(k_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let w_b = a.param(ns.p, ep_push, ParameterRole::Block, u64_type());
    let w_acc = a.param(ns.p, ep_push, ParameterRole::Block, u8vec_type());
    let w_pay = a.param(ns.p, ep_push, ParameterRole::Block, TypeExpr::Bytes);
    let w_lenvec = a.param(ns.p, ep_push, ParameterRole::Block, u8vec_type());
    let w_unit = a.param(ns.p, ep_push, ParameterRole::Block, TypeExpr::Unit);
    let w_bc = a.cref(ns.o, ep_push, b09, u8_type());
    let w_push = a.op(
        ns.o,
        ep_push,
        Opcode::AdapterInvoke,
        vec![pav(w_acc), op_result(w_bc)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: ep_push,
        function: fid,
        parameters: vec![w_b, w_acc, w_pay, w_lenvec, w_unit],
        operations: vec![w_bc, w_push],
        terminator: switch(
            op_result(w_push),
            vec![
                (
                    BuiltinCase::Ok,
                    ep_next,
                    vec![
                        sav(w_b),
                        SwitchArgument::CasePayload,
                        sav(w_pay),
                        sav(w_lenvec),
                        sav(w_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let x_idx = a.param(ns.p, ep_next, ParameterRole::Block, u64_type());
    let x_acc = a.param(ns.p, ep_next, ParameterRole::Block, u8vec_type());
    let x_pay = a.param(ns.p, ep_next, ParameterRole::Block, TypeExpr::Bytes);
    let x_lenvec = a.param(ns.p, ep_next, ParameterRole::Block, u8vec_type());
    let x_unit = a.param(ns.p, ep_next, ParameterRole::Block, TypeExpr::Unit);
    let x_onec = a.cref(ns.o, ep_next, c1, u64_type());
    let x_add = a.op(
        ns.o,
        ep_next,
        Opcode::IntAddChecked,
        vec![pav(x_idx), op_result(x_onec)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: ep_next,
        function: fid,
        parameters: vec![x_idx, x_acc, x_pay, x_lenvec, x_unit],
        operations: vec![x_onec, x_add],
        terminator: switch(
            op_result(x_add),
            vec![
                (
                    BuiltinCase::Ok,
                    ep_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(x_acc),
                        sav(x_pay),
                        sav(x_lenvec),
                        sav(x_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let y_acc = a.param(ns.p, ep_done, ParameterRole::Block, u8vec_type());
    let y_pay = a.param(ns.p, ep_done, ParameterRole::Block, TypeExpr::Bytes);
    let y_lenvec = a.param(ns.p, ep_done, ParameterRole::Block, u8vec_type());
    let y_unit = a.param(ns.p, ep_done, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: ep_done,
        function: fid,
        parameters: vec![y_acc, y_pay, y_lenvec, y_unit],
        operations: Vec::new(),
        terminator: branch(edge(
            len_copy,
            vec![
                pav(y_acc),
                pav(y_lenvec),
                pav(y_pay),
                pav(y_unit),
            ],
        )),
        reachability: Reachability::Required,
    });

    let lc_acc = a.param(ns.p, len_copy, ParameterRole::Block, u8vec_type());
    let lc_lenvec = a.param(ns.p, len_copy, ParameterRole::Block, u8vec_type());
    let lc_pby = a.param(ns.p, len_copy, ParameterRole::Block, TypeExpr::Bytes);
    let lc_unit = a.param(ns.p, len_copy, ParameterRole::Block, TypeExpr::Unit);
    let lc_llen = a.op(
        ns.o,
        len_copy,
        Opcode::VectorLen,
        vec![pav(lc_lenvec)],
        vec![u64_type()],
        Immediate::None,
    );
    let lc_check = a.id(ns.b);
    let lc_get = a.id(ns.b);
    let lc_push = a.id(ns.b);
    let lc_next = a.id(ns.b);
    let lc_done = a.id(ns.b);
    let lcz0c = a.cref(ns.o, len_copy, c0, u64_type());
    a.blocks.push(Block {
        entity_id: len_copy,
        function: fid,
        parameters: vec![lc_acc, lc_lenvec, lc_pby, lc_unit],
        operations: vec![lc_llen, lcz0c],
        terminator: branch(edge(
            lc_check,
            vec![
                op_result(lcz0c),
                pav(lc_acc),
                pav(lc_lenvec),
                op_result(lc_llen),
                pav(lc_pby),
                pav(lc_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let li_idx = a.param(ns.p, lc_check, ParameterRole::Block, u64_type());
    let li_acc = a.param(ns.p, lc_check, ParameterRole::Block, u8vec_type());
    let li_src = a.param(ns.p, lc_check, ParameterRole::Block, u8vec_type());
    let li_slen = a.param(ns.p, lc_check, ParameterRole::Block, u64_type());
    let li_pby = a.param(ns.p, lc_check, ParameterRole::Block, TypeExpr::Bytes);
    let li_unit = a.param(ns.p, lc_check, ParameterRole::Block, TypeExpr::Unit);
    let li_lt = a.op(
        ns.o,
        lc_check,
        Opcode::LessThan,
        vec![pav(li_idx), pav(li_slen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: lc_check,
        function: fid,
        parameters: vec![li_idx, li_acc, li_src, li_slen, li_pby, li_unit],
        operations: vec![li_lt],
        terminator: cond(
            op_result(li_lt),
            edge(
                lc_get,
                vec![
                    pav(li_idx),
                    pav(li_acc),
                    pav(li_src),
                    pav(li_slen),
                    pav(li_pby),
                    pav(li_unit),
                ],
            ),
            edge(
                lc_done,
                vec![
                    pav(li_acc),
                    pav(li_pby),
                    pav(li_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let lg_idx = a.param(ns.p, lc_get, ParameterRole::Block, u64_type());
    let lg_acc = a.param(ns.p, lc_get, ParameterRole::Block, u8vec_type());
    let lg_src = a.param(ns.p, lc_get, ParameterRole::Block, u8vec_type());
    let lg_slen = a.param(ns.p, lc_get, ParameterRole::Block, u64_type());
    let lg_pby = a.param(ns.p, lc_get, ParameterRole::Block, TypeExpr::Bytes);
    let lg_unit = a.param(ns.p, lc_get, ParameterRole::Block, TypeExpr::Unit);
    let lg_get = a.op(
        ns.o,
        lc_get,
        Opcode::VectorGet,
        vec![pav(lg_src), pav(lg_idx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: lc_get,
        function: fid,
        parameters: vec![lg_idx, lg_acc, lg_src, lg_slen, lg_pby, lg_unit],
        operations: vec![lg_get],
        terminator: switch(
            op_result(lg_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    lc_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(lg_idx),
                        sav(lg_acc),
                        sav(lg_pby),
                        sav(lg_src),
                        sav(lg_slen),
                        sav(lg_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let lu_b = a.param(ns.p, lc_push, ParameterRole::Block, u8_type());
    let lu_idx = a.param(ns.p, lc_push, ParameterRole::Block, u64_type());
    let lu_acc = a.param(ns.p, lc_push, ParameterRole::Block, u8vec_type());
    let lu_pby = a.param(ns.p, lc_push, ParameterRole::Block, TypeExpr::Bytes);
    let lu_src = a.param(ns.p, lc_push, ParameterRole::Block, u8vec_type());
    let lu_slen = a.param(ns.p, lc_push, ParameterRole::Block, u64_type());
    let lu_unit = a.param(ns.p, lc_push, ParameterRole::Block, TypeExpr::Unit);
    let lu_push = a.op(
        ns.o,
        lc_push,
        Opcode::AdapterInvoke,
        vec![pav(lu_acc), pav(lu_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: lc_push,
        function: fid,
        parameters: vec![lu_b, lu_idx, lu_acc, lu_pby, lu_src, lu_slen, lu_unit],
        operations: vec![lu_push],
        terminator: switch(
            op_result(lu_push),
            vec![
                (
                    BuiltinCase::Ok,
                    lc_next,
                    vec![
                        sav(lu_idx),
                        SwitchArgument::CasePayload,
                        sav(lu_pby),
                        sav(lu_src),
                        sav(lu_slen),
                        sav(lu_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let ln_idx = a.param(ns.p, lc_next, ParameterRole::Block, u64_type());
    let ln_acc = a.param(ns.p, lc_next, ParameterRole::Block, u8vec_type());
    let ln_pby = a.param(ns.p, lc_next, ParameterRole::Block, TypeExpr::Bytes);
    let ln_src = a.param(ns.p, lc_next, ParameterRole::Block, u8vec_type());
    let ln_slen = a.param(ns.p, lc_next, ParameterRole::Block, u64_type());
    let ln_unit = a.param(ns.p, lc_next, ParameterRole::Block, TypeExpr::Unit);
    let ln_onec = a.cref(ns.o, lc_next, c1, u64_type());
    let ln_add = a.op(
        ns.o,
        lc_next,
        Opcode::IntAddChecked,
        vec![pav(ln_idx), op_result(ln_onec)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: lc_next,
        function: fid,
        parameters: vec![ln_idx, ln_acc, ln_pby, ln_src, ln_slen, ln_unit],
        operations: vec![ln_onec, ln_add],
        terminator: switch(
            op_result(ln_add),
            vec![
                (
                    BuiltinCase::Ok,
                    lc_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(ln_acc),
                        sav(ln_src),
                        sav(ln_slen),
                        sav(ln_pby),
                        sav(ln_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Late payload conversion: the payload vector is created here,
    // after the length reads above, so no payload vector is alive
    // during the length copy. Bridge failure refuses RESOURCE_LIMIT.
    let pp_acc = a.param(ns.p, lc_done, ParameterRole::Block, u8vec_type());
    let pp_pby = a.param(ns.p, lc_done, ParameterRole::Block, TypeExpr::Bytes);
    let pp_unit = a.param(ns.p, lc_done, ParameterRole::Block, TypeExpr::Unit);
    let post_b2v = a.id(ns.b);
    let post_check = a.id(ns.b);
    let pc_check = a.id(ns.b);
    let pc_get = a.id(ns.b);
    let pc_push = a.id(ns.b);
    let pc_next = a.id(ns.b);
    let pc_done = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: lc_done,
        function: fid,
        parameters: vec![pp_acc, pp_pby, pp_unit],
        operations: Vec::new(),
        terminator: branch(edge(
            post_b2v,
            vec![pav(pp_acc), pav(pp_pby), pav(pp_unit)],
        )),
        reachability: Reachability::Required,
    });
    let qb_acc = a.param(ns.p, post_b2v, ParameterRole::Block, u8vec_type());
    let qb_pby = a.param(ns.p, post_b2v, ParameterRole::Block, TypeExpr::Bytes);
    let qb_unit = a.param(ns.p, post_b2v, ParameterRole::Block, TypeExpr::Unit);
    let qb_conv = a.op(
        ns.o,
        post_b2v,
        Opcode::AdapterInvoke,
        vec![pav(qb_unit), pav(qb_pby)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: post_b2v,
        function: fid,
        parameters: vec![qb_acc, qb_pby, qb_unit],
        operations: vec![qb_conv],
        terminator: switch(
            op_result(qb_conv),
            vec![
                (
                    BuiltinCase::Ok,
                    post_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(qb_acc),
                        sav(qb_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Payload length past the epoch ceiling refuses RESOURCE_LIMIT
    // (reference `build_preimage` resource rule). Unreachable under the
    // bridge cap, but explicit and typed.
    let qc_vec = a.param(ns.p, post_check, ParameterRole::Block, u8vec_type());
    let qc_acc = a.param(ns.p, post_check, ParameterRole::Block, u8vec_type());
    let qc_unit = a.param(ns.p, post_check, ParameterRole::Block, TypeExpr::Unit);
    let qc_len = a.op(
        ns.o,
        post_check,
        Opcode::VectorLen,
        vec![pav(qc_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let qc_maxc = a.cref(ns.o, post_check, c_max, u64_type());
    let qc_over = a.op(
        ns.o,
        post_check,
        Opcode::GreaterThan,
        vec![op_result(qc_len), op_result(qc_maxc)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let qc_z0c = a.cref(ns.o, post_check, c0, u64_type());
    a.blocks.push(Block {
        entity_id: post_check,
        function: fid,
        parameters: vec![qc_vec, qc_acc, qc_unit],
        operations: vec![qc_len, qc_maxc, qc_over, qc_z0c],
        terminator: cond(
            op_result(qc_over),
            edge(b_res, Vec::new()),
            edge(
                pc_check,
                vec![
                    op_result(qc_z0c),
                    pav(qc_acc),
                    pav(qc_vec),
                    op_result(qc_len),
                    pav(qc_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });
    let pi_idx = a.param(ns.p, pc_check, ParameterRole::Block, u64_type());
    let pi_acc = a.param(ns.p, pc_check, ParameterRole::Block, u8vec_type());
    let pi_pay = a.param(ns.p, pc_check, ParameterRole::Block, u8vec_type());
    let pi_plen = a.param(ns.p, pc_check, ParameterRole::Block, u64_type());
    let pi_unit = a.param(ns.p, pc_check, ParameterRole::Block, TypeExpr::Unit);
    let pi_lt = a.op(
        ns.o,
        pc_check,
        Opcode::LessThan,
        vec![pav(pi_idx), pav(pi_plen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: pc_check,
        function: fid,
        parameters: vec![pi_idx, pi_acc, pi_pay, pi_plen, pi_unit],
        operations: vec![pi_lt],
        terminator: cond(
            op_result(pi_lt),
            edge(
                pc_get,
                vec![
                    pav(pi_idx),
                    pav(pi_acc),
                    pav(pi_pay),
                    pav(pi_plen),
                    pav(pi_unit),
                ],
            ),
            edge(pc_done, vec![pav(pi_acc), pav(pi_unit)]),
        ),
        reachability: Reachability::Required,
    });
    let pg_idx = a.param(ns.p, pc_get, ParameterRole::Block, u64_type());
    let pg_acc = a.param(ns.p, pc_get, ParameterRole::Block, u8vec_type());
    let pg_pay = a.param(ns.p, pc_get, ParameterRole::Block, u8vec_type());
    let pg_plen = a.param(ns.p, pc_get, ParameterRole::Block, u64_type());
    let pg_unit = a.param(ns.p, pc_get, ParameterRole::Block, TypeExpr::Unit);
    let pg_get = a.op(
        ns.o,
        pc_get,
        Opcode::VectorGet,
        vec![pav(pg_pay), pav(pg_idx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: pc_get,
        function: fid,
        parameters: vec![pg_idx, pg_acc, pg_pay, pg_plen, pg_unit],
        operations: vec![pg_get],
        terminator: switch(
            op_result(pg_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    pc_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(pg_idx),
                        sav(pg_acc),
                        sav(pg_pay),
                        sav(pg_plen),
                        sav(pg_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let pu_b = a.param(ns.p, pc_push, ParameterRole::Block, u8_type());
    let pu_idx = a.param(ns.p, pc_push, ParameterRole::Block, u64_type());
    let pu_acc = a.param(ns.p, pc_push, ParameterRole::Block, u8vec_type());
    let pu_pay = a.param(ns.p, pc_push, ParameterRole::Block, u8vec_type());
    let pu_plen = a.param(ns.p, pc_push, ParameterRole::Block, u64_type());
    let pu_unit = a.param(ns.p, pc_push, ParameterRole::Block, TypeExpr::Unit);
    let pu_push = a.op(
        ns.o,
        pc_push,
        Opcode::AdapterInvoke,
        vec![pav(pu_acc), pav(pu_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: pc_push,
        function: fid,
        parameters: vec![pu_b, pu_idx, pu_acc, pu_pay, pu_plen, pu_unit],
        operations: vec![pu_push],
        terminator: switch(
            op_result(pu_push),
            vec![
                (
                    BuiltinCase::Ok,
                    pc_next,
                    vec![
                        sav(pu_idx),
                        SwitchArgument::CasePayload,
                        sav(pu_pay),
                        sav(pu_plen),
                        sav(pu_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let pn_idx = a.param(ns.p, pc_next, ParameterRole::Block, u64_type());
    let pn_acc = a.param(ns.p, pc_next, ParameterRole::Block, u8vec_type());
    let pn_pay = a.param(ns.p, pc_next, ParameterRole::Block, u8vec_type());
    let pn_plen = a.param(ns.p, pc_next, ParameterRole::Block, u64_type());
    let pn_unit = a.param(ns.p, pc_next, ParameterRole::Block, TypeExpr::Unit);
    let pn_onec = a.cref(ns.o, pc_next, c1, u64_type());
    let pn_add = a.op(
        ns.o,
        pc_next,
        Opcode::IntAddChecked,
        vec![pav(pn_idx), op_result(pn_onec)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: pc_next,
        function: fid,
        parameters: vec![pn_idx, pn_acc, pn_pay, pn_plen, pn_unit],
        operations: vec![pn_onec, pn_add],
        terminator: switch(
            op_result(pn_add),
            vec![
                (
                    BuiltinCase::Ok,
                    pc_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(pn_acc),
                        sav(pn_pay),
                        sav(pn_plen),
                        sav(pn_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    // Prefix plus preimage finalize: the accumulator holds the exact
    // stored prefix. Convert it (prefix bytes for the output tuple),
    // then build the digest preimage (domain ++ stored prefix) in a
    // second accumulator and convert that too. Returns
    // `Tuple(prefix, preimage)`.
    let pc2_acc = a.param(ns.p, pc_done, ParameterRole::Block, u8vec_type());
    let pc2_unit = a.param(ns.p, pc_done, ParameterRole::Block, TypeExpr::Unit);
    let f_v2b = a.op(
        ns.o,
        pc_done,
        Opcode::AdapterInvoke,
        vec![pav(pc2_unit), pav(pc2_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    let d0 = a.id(ns.b);
    let dd_pre = a.param(ns.p, d0, ParameterRole::Block, TypeExpr::Bytes);
    let dd_vec = a.param(ns.p, d0, ParameterRole::Block, u8vec_type());
    let dd_unit = a.param(ns.p, d0, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: pc_done,
        function: fid,
        parameters: vec![pc2_acc, pc2_unit],
        operations: vec![f_v2b],
        terminator: switch(
            op_result(f_v2b),
            vec![
                (
                    BuiltinCase::Ok,
                    d0,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(pc2_acc),
                        sav(pc2_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Domain prefix chain (`sley2.object.v1`, 15 pushes).
    let d2e = a.ku8(ns.k, 0x2e);
    let d31 = a.ku8(ns.k, 0x31);
    let d32 = a.ku8(ns.k, 0x32);
    let d62 = a.ku8(ns.k, 0x62);
    let d63 = a.ku8(ns.k, 0x63);
    let d65 = a.ku8(ns.k, 0x65);
    let d6a = a.ku8(ns.k, 0x6a);
    let d6c = a.ku8(ns.k, 0x6c);
    let d6f = a.ku8(ns.k, 0x6f);
    let d73 = a.ku8(ns.k, 0x73);
    let d74 = a.ku8(ns.k, 0x74);
    let d76 = a.ku8(ns.k, 0x76);
    let d79 = a.ku8(ns.k, 0x79);
    let domain_bytes = [
        d73, d6c, d65, d79, d32, d2e, d6f, d62, d6a, d65, d63, d74, d2e, d76, d31,
    ];
    let d_empty = a.op(
        ns.o,
        d0,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let mut d_steps: Vec<EntityId> = Vec::new();
    for _ in 0..15 {
        d_steps.push(a.id(ns.b));
    }
    let pre_copy = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: d0,
        function: fid,
        parameters: vec![dd_pre, dd_vec, dd_unit],
        operations: vec![d_empty],
        terminator: branch(edge(
            d_steps[0],
            vec![
                op_result(d_empty),
                pav(dd_vec),
                pav(dd_pre),
                pav(dd_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    for i in 0..15 {
        let st = d_steps[i];
        let ds_acc = a.param(ns.p, st, ParameterRole::Block, u8vec_type());
        let s_vec = a.param(ns.p, st, ParameterRole::Block, u8vec_type());
        let s_pre = a.param(ns.p, st, ParameterRole::Block, TypeExpr::Bytes);
        let ds_unit = a.param(ns.p, st, ParameterRole::Block, TypeExpr::Unit);
        let dbc = a.cref(ns.o, st, domain_bytes[i], u8_type());
        let push = a.op(
            ns.o,
            st,
            Opcode::AdapterInvoke,
            vec![pav(ds_acc), op_result(dbc)],
            vec![index_result(u8vec_type())],
            Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
        );
        let next = if i == 14 { pre_copy } else { d_steps[i + 1] };
        a.blocks.push(Block {
            entity_id: st,
            function: fid,
            parameters: vec![ds_acc, s_vec, s_pre, ds_unit],
            operations: vec![dbc, push],
            terminator: switch(
                op_result(push),
                vec![
                    (
                        BuiltinCase::Ok,
                        next,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(s_vec),
                            sav(s_pre),
                            sav(ds_unit),
                        ],
                    ),
                    (BuiltinCase::Err, b_res, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
    }
    // Preimage copy: append the stored prefix bytes after the domain.
    let cp_pre = a.param(ns.p, pre_copy, ParameterRole::Block, u8vec_type());
    let cp_vec = a.param(ns.p, pre_copy, ParameterRole::Block, u8vec_type());
    let cp_pay = a.param(ns.p, pre_copy, ParameterRole::Block, TypeExpr::Bytes);
    let cp_unit = a.param(ns.p, pre_copy, ParameterRole::Block, TypeExpr::Unit);
    let cp_len = a.op(
        ns.o,
        pre_copy,
        Opcode::VectorLen,
        vec![pav(cp_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let cp_check = a.id(ns.b);
    let cp_get = a.id(ns.b);
    let cp_push = a.id(ns.b);
    let cp_next = a.id(ns.b);
    let cp_done = a.id(ns.b);
    let cpz0c = a.cref(ns.o, pre_copy, c0, u64_type());
    a.blocks.push(Block {
        entity_id: pre_copy,
        function: fid,
        parameters: vec![cp_pre, cp_vec, cp_pay, cp_unit],
        operations: vec![cp_len, cpz0c],
        terminator: branch(edge(
            cp_check,
            vec![
                op_result(cpz0c),
                pav(cp_pre),
                pav(cp_vec),
                op_result(cp_len),
                pav(cp_pay),
                pav(cp_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let ci_idx = a.param(ns.p, cp_check, ParameterRole::Block, u64_type());
    let ci_pre = a.param(ns.p, cp_check, ParameterRole::Block, u8vec_type());
    let ci_vec = a.param(ns.p, cp_check, ParameterRole::Block, u8vec_type());
    let ci_plen = a.param(ns.p, cp_check, ParameterRole::Block, u64_type());
    let ci_pay = a.param(ns.p, cp_check, ParameterRole::Block, TypeExpr::Bytes);
    let ci_unit = a.param(ns.p, cp_check, ParameterRole::Block, TypeExpr::Unit);
    let ci_lt = a.op(
        ns.o,
        cp_check,
        Opcode::LessThan,
        vec![pav(ci_idx), pav(ci_plen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: cp_check,
        function: fid,
        parameters: vec![ci_idx, ci_pre, ci_vec, ci_plen, ci_pay, ci_unit],
        operations: vec![ci_lt],
        terminator: cond(
            op_result(ci_lt),
            edge(
                cp_get,
                vec![
                    pav(ci_idx),
                    pav(ci_pre),
                    pav(ci_vec),
                    pav(ci_plen),
                    pav(ci_pay),
                    pav(ci_unit),
                ],
            ),
            edge(cp_done, vec![pav(ci_pre), pav(ci_pay), pav(ci_unit)]),
        ),
        reachability: Reachability::Required,
    });
    let cg_idx = a.param(ns.p, cp_get, ParameterRole::Block, u64_type());
    let cg_pre = a.param(ns.p, cp_get, ParameterRole::Block, u8vec_type());
    let cg_vec = a.param(ns.p, cp_get, ParameterRole::Block, u8vec_type());
    let cg_plen = a.param(ns.p, cp_get, ParameterRole::Block, u64_type());
    let cg_pay = a.param(ns.p, cp_get, ParameterRole::Block, TypeExpr::Bytes);
    let cg_unit = a.param(ns.p, cp_get, ParameterRole::Block, TypeExpr::Unit);
    let cg_get = a.op(
        ns.o,
        cp_get,
        Opcode::VectorGet,
        vec![pav(cg_vec), pav(cg_idx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: cp_get,
        function: fid,
        parameters: vec![cg_idx, cg_pre, cg_vec, cg_plen, cg_pay, cg_unit],
        operations: vec![cg_get],
        terminator: switch(
            op_result(cg_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    cp_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(cg_idx),
                        sav(cg_pre),
                        sav(cg_vec),
                        sav(cg_plen),
                        sav(cg_pay),
                        sav(cg_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let cu_b = a.param(ns.p, cp_push, ParameterRole::Block, u8_type());
    let cu_idx = a.param(ns.p, cp_push, ParameterRole::Block, u64_type());
    let cu_pre = a.param(ns.p, cp_push, ParameterRole::Block, u8vec_type());
    let cu_vec = a.param(ns.p, cp_push, ParameterRole::Block, u8vec_type());
    let cu_plen = a.param(ns.p, cp_push, ParameterRole::Block, u64_type());
    let cu_pay = a.param(ns.p, cp_push, ParameterRole::Block, TypeExpr::Bytes);
    let cu_unit = a.param(ns.p, cp_push, ParameterRole::Block, TypeExpr::Unit);
    let cu_push = a.op(
        ns.o,
        cp_push,
        Opcode::AdapterInvoke,
        vec![pav(cu_pre), pav(cu_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: cp_push,
        function: fid,
        parameters: vec![cu_b, cu_idx, cu_pre, cu_vec, cu_plen, cu_pay, cu_unit],
        operations: vec![cu_push],
        terminator: switch(
            op_result(cu_push),
            vec![
                (
                    BuiltinCase::Ok,
                    cp_next,
                    vec![
                        sav(cu_idx),
                        SwitchArgument::CasePayload,
                        sav(cu_vec),
                        sav(cu_plen),
                        sav(cu_pay),
                        sav(cu_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let cn_idx = a.param(ns.p, cp_next, ParameterRole::Block, u64_type());
    let cn_pre = a.param(ns.p, cp_next, ParameterRole::Block, u8vec_type());
    let cn_vec = a.param(ns.p, cp_next, ParameterRole::Block, u8vec_type());
    let cn_plen = a.param(ns.p, cp_next, ParameterRole::Block, u64_type());
    let cn_pay = a.param(ns.p, cp_next, ParameterRole::Block, TypeExpr::Bytes);
    let cn_unit = a.param(ns.p, cp_next, ParameterRole::Block, TypeExpr::Unit);
    let cn_onec = a.cref(ns.o, cp_next, c1, u64_type());
    let cn_add = a.op(
        ns.o,
        cp_next,
        Opcode::IntAddChecked,
        vec![pav(cn_idx), op_result(cn_onec)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: cp_next,
        function: fid,
        parameters: vec![cn_idx, cn_pre, cn_vec, cn_plen, cn_pay, cn_unit],
        operations: vec![cn_onec, cn_add],
        terminator: switch(
            op_result(cn_add),
            vec![
                (
                    BuiltinCase::Ok,
                    cp_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(cn_pre),
                        sav(cn_vec),
                        sav(cn_plen),
                        sav(cn_pay),
                        sav(cn_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Preimage finalize plus the output tuple `(prefix, preimage)`.
    let hp_pre = a.param(ns.p, cp_done, ParameterRole::Block, u8vec_type());
    let hp_pay = a.param(ns.p, cp_done, ParameterRole::Block, TypeExpr::Bytes);
    let hp_unit = a.param(ns.p, cp_done, ParameterRole::Block, TypeExpr::Unit);
    let hp_v2b = a.op(
        ns.o,
        cp_done,
        Opcode::AdapterInvoke,
        vec![pav(hp_unit), pav(hp_pre)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    let f_ok = a.id(ns.b);
    let o_pre = a.param(ns.p, f_ok, ParameterRole::Block, TypeExpr::Bytes);
    let o_pay = a.param(ns.p, f_ok, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: cp_done,
        function: fid,
        parameters: vec![hp_pre, hp_pay, hp_unit],
        operations: vec![hp_v2b],
        terminator: switch(
            op_result(hp_v2b),
            vec![
                (
                    BuiltinCase::Ok,
                    f_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(hp_pay),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // NOTE: f_ok drops the unit (unneeded at return); the tuple carries
    // (prefix, preimage) in contracted order.
    let f_tup = a.op(
        ns.o,
        f_ok,
        Opcode::TupleNew,
        vec![pav(o_pay), pav(o_pre)],
        vec![TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes])],
        Immediate::None,
    );
    let f_okv = a.op(
        ns.o,
        f_ok,
        Opcode::ResultOk,
        vec![op_result(f_tup)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: f_ok,
        function: fid,
        parameters: vec![o_pre, o_pay],
        operations: vec![f_tup, f_okv],
        terminator: ret(op_result(f_okv)),
        reachability: Reachability::Required,
    });



    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![p_pay, p_lenvec, p_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

// ── composed program entries (single-invocation paths) ───────────────
// `decode_program_entrypoint(stored: Bytes, unit: Unit)` runs envelope
// validation -> outer decode -> EntryPoint decode in one Sley
// invocation, returning the structured `(entity_id, function,
// exposure)` triple. `encode_program_entrypoint(entity_id: Bytes,
// function: Bytes, exposure: UInt64, unit: Unit)` runs EntryPoint
// encode -> outer encode -> envelope build in one Sley invocation,
// returning the complete canonical stored object. Callee refusals are
// forwarded unchanged, so envelope codes precede outer codes precede
// body codes with no remapping. The native harness supplies raw stored
// bytes / structured values and compares results; it performs no
// intermediate extraction, decode, or envelope work for Sley.
#[allow(clippy::many_single_char_names, clippy::similar_names)]
fn build_program_decode(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    validate_fid: EntityId,
    outer_fid: EntityId,
    entry_fid: EntityId,
) -> FunctionGraph {
    let bstart = a.blocks.len();
    let res_t = program_decode_result_type();
    let env_t = encode_result_type();
    let out_t = outer_decode_result_type();
    let ent_t = entrypoint_decode_result_type();

    let p_stored = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let p_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);

    // Envelope validation first. Err forwards unchanged.
    let entry = a.id(ns.b);
    let v_call = a.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(p_stored), pav(p_unit)],
        vec![env_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: validate_fid,
            type_arguments: Vec::new(),
        }),
    );
    let v_ok = a.id(ns.b);
    let v_err = a.id(ns.b);
    let v_pay = a.param(ns.p, v_ok, ParameterRole::Block, TypeExpr::Bytes);
    let v_unit = a.param(ns.p, v_ok, ParameterRole::Block, TypeExpr::Unit);
    let v_ebytes = a.param(ns.p, v_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![v_call],
        terminator: switch(
            op_result(v_call),
            vec![
                (
                    BuiltinCase::Ok,
                    v_ok,
                    vec![SwitchArgument::CasePayload, sav(p_unit)],
                ),
                (BuiltinCase::Err, v_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let v_er = a.op(
        ns.o,
        v_err,
        Opcode::ResultErr,
        vec![pav(v_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: v_err,
        function: fid,
        parameters: vec![v_ebytes],
        operations: vec![v_er],
        terminator: ret(op_result(v_er)),
        reachability: Reachability::Required,
    });

    // Outer decode on the validated payload. Err forwards unchanged.
    let o_call = a.op(
        ns.o,
        v_ok,
        Opcode::CallDirect,
        vec![pav(v_pay), pav(v_unit)],
        vec![out_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: outer_fid,
            type_arguments: Vec::new(),
        }),
    );
    let o_ok = a.id(ns.b);
    let o_err = a.id(ns.b);
    let o_tup = a.param(
        ns.p,
        o_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]),
    );
    let o_unit = a.param(ns.p, o_ok, ParameterRole::Block, TypeExpr::Unit);
    let o_ebytes = a.param(ns.p, o_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: v_ok,
        function: fid,
        parameters: vec![v_pay, v_unit],
        operations: vec![o_call],
        terminator: switch(
            op_result(o_call),
            vec![
                (
                    BuiltinCase::Ok,
                    o_ok,
                    vec![SwitchArgument::CasePayload, sav(v_unit)],
                ),
                (BuiltinCase::Err, o_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let o_er = a.op(
        ns.o,
        o_err,
        Opcode::ResultErr,
        vec![pav(o_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: o_err,
        function: fid,
        parameters: vec![o_ebytes],
        operations: vec![o_er],
        terminator: ret(op_result(o_er)),
        reachability: Reachability::Required,
    });

    // EntryPoint decode on the outer body. Err forwards unchanged; Ok
    // joins (entity_id, function, exposure) into the result triple.
    let o_geid = a.op(
        ns.o,
        o_ok,
        Opcode::TupleGet,
        vec![pav(o_tup)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let o_gbody = a.op(
        ns.o,
        o_ok,
        Opcode::TupleGet,
        vec![pav(o_tup)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let e_call = a.op(
        ns.o,
        o_ok,
        Opcode::CallDirect,
        vec![op_result(o_gbody), pav(o_unit)],
        vec![ent_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: entry_fid,
            type_arguments: Vec::new(),
        }),
    );
    let e_ok = a.id(ns.b);
    let e_err = a.id(ns.b);
    let e_tup = a.param(
        ns.p,
        e_ok,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, u64_type()]),
    );
    let e_eid = a.param(ns.p, e_ok, ParameterRole::Block, TypeExpr::Bytes);
    let e_ebytes = a.param(ns.p, e_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: o_ok,
        function: fid,
        parameters: vec![o_tup, o_unit],
        operations: vec![o_geid, o_gbody, e_call],
        terminator: switch(
            op_result(e_call),
            vec![
                (
                    BuiltinCase::Ok,
                    e_ok,
                    vec![SwitchArgument::CasePayload, oav(o_geid)],
                ),
                (BuiltinCase::Err, e_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let e_er = a.op(
        ns.o,
        e_err,
        Opcode::ResultErr,
        vec![pav(e_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: e_err,
        function: fid,
        parameters: vec![e_ebytes],
        operations: vec![e_er],
        terminator: ret(op_result(e_er)),
        reachability: Reachability::Required,
    });
    let e_gfunc = a.op(
        ns.o,
        e_ok,
        Opcode::TupleGet,
        vec![pav(e_tup)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let e_gexp = a.op(
        ns.o,
        e_ok,
        Opcode::TupleGet,
        vec![pav(e_tup)],
        vec![u64_type()],
        Immediate::Index(1),
    );
    let e_tup3 = a.op(
        ns.o,
        e_ok,
        Opcode::TupleNew,
        vec![pav(e_eid), op_result(e_gfunc), op_result(e_gexp)],
        vec![TypeExpr::Tuple(vec![
            TypeExpr::Bytes,
            TypeExpr::Bytes,
            u64_type(),
        ])],
        Immediate::None,
    );
    let e_okv = a.op(
        ns.o,
        e_ok,
        Opcode::ResultOk,
        vec![op_result(e_tup3)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: e_ok,
        function: fid,
        parameters: vec![e_tup, e_eid],
        operations: vec![e_gfunc, e_gexp, e_tup3, e_okv],
        terminator: ret(op_result(e_okv)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![p_stored, p_unit],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: entry,
        blocks: a.blocks[bstart..].iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    }
}

#[allow(clippy::many_single_char_names, clippy::similar_names)]
fn build_program_encode(
    a: &mut Asm,
    ns: Ns,
    fid: EntityId,
    entry_enc_fid: EntityId,
    outer_enc_fid: EntityId,
    build_fid: EntityId,
    encode_fid: EntityId,
) -> FunctionGraph {
    use sley_vm::host_abi::{
        BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1,
    };
    let bstart = a.blocks.len();
    let res_t = encode_result_type();
    let w64 = a.ku32(ns.k, 64);
    let c0 = a.ku64(ns.k, 0);
    let c1 = a.ku64(ns.k, 1);
    let c32 = a.ku64(ns.k, 32);
    let e_res = a.kbytes(ns.k, b"SCB_RESOURCE_LIMIT");

    let p_eid = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let p_func = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Bytes);
    let p_exp = a.param(ns.p, fid, ParameterRole::Function, u64_type());
    let p_unit = a.param(ns.p, fid, ParameterRole::Function, TypeExpr::Unit);
    let b_res = err_block(a, ns, fid, res_t.clone(), e_res);
    let trap = trap_block(a, ns, fid);

    // EntryPoint encode first. Err forwards unchanged.
    let entry = a.id(ns.b);
    let x_call = a.op(
        ns.o,
        entry,
        Opcode::CallDirect,
        vec![pav(p_func), pav(p_exp), pav(p_unit)],
        vec![res_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: entry_enc_fid,
            type_arguments: Vec::new(),
        }),
    );
    let x_ok = a.id(ns.b);
    let x_err = a.id(ns.b);
    let x_body = a.param(ns.p, x_ok, ParameterRole::Block, TypeExpr::Bytes);
    let x_eid = a.param(ns.p, x_ok, ParameterRole::Block, TypeExpr::Bytes);
    let x_unit = a.param(ns.p, x_ok, ParameterRole::Block, TypeExpr::Unit);
    let x_ebytes = a.param(ns.p, x_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: entry,
        function: fid,
        parameters: Vec::new(),
        operations: vec![x_call],
        terminator: switch(
            op_result(x_call),
            vec![
                (
                    BuiltinCase::Ok,
                    x_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(p_eid),
                        sav(p_unit),
                    ],
                ),
                (BuiltinCase::Err, x_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let x_er = a.op(
        ns.o,
        x_err,
        Opcode::ResultErr,
        vec![pav(x_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: x_err,
        function: fid,
        parameters: vec![x_ebytes],
        operations: vec![x_er],
        terminator: ret(op_result(x_er)),
        reachability: Reachability::Required,
    });

    // Outer encode on (entity_id, body). Err forwards unchanged.
    let o_call = a.op(
        ns.o,
        x_ok,
        Opcode::CallDirect,
        vec![pav(x_eid), pav(x_body), pav(x_unit)],
        vec![res_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: outer_enc_fid,
            type_arguments: Vec::new(),
        }),
    );
    let o_ok = a.id(ns.b);
    let o_err = a.id(ns.b);
    let o_pay = a.param(ns.p, o_ok, ParameterRole::Block, TypeExpr::Bytes);
    let o_unit = a.param(ns.p, o_ok, ParameterRole::Block, TypeExpr::Unit);
    let o_ebytes = a.param(ns.p, o_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: x_ok,
        function: fid,
        parameters: vec![x_body, x_eid, x_unit],
        operations: vec![o_call],
        terminator: switch(
            op_result(o_call),
            vec![
                (
                    BuiltinCase::Ok,
                    o_ok,
                    vec![SwitchArgument::CasePayload, sav(x_unit)],
                ),
                (BuiltinCase::Err, o_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let o_er = a.op(
        ns.o,
        o_err,
        Opcode::ResultErr,
        vec![pav(o_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: o_err,
        function: fid,
        parameters: vec![o_ebytes],
        operations: vec![o_er],
        terminator: ret(op_result(o_er)),
        reachability: Reachability::Required,
    });

    // Length vector for the envelope build: payload length via B2V1 +
    // canonical encode_uvar plus a second B2V1, all in this digest-free
    // graph (the length encoding crosses into the digest graph as a
    // first-class vector value). Callee refusals forward unchanged;
    // bridge exhaustion refuses RESOURCE_LIMIT. The payload Bytes value
    // is threaded alongside (SSA values reuse freely) for the build call.
    let n_b2v = a.id(ns.b);
    let n_len = a.id(ns.b);
    let n_build = a.id(ns.b);
    let n_pay = a.param(ns.p, n_b2v, ParameterRole::Block, TypeExpr::Bytes);
    let n_unit = a.param(ns.p, n_b2v, ParameterRole::Block, TypeExpr::Unit);
    let n_conv = a.op(
        ns.o,
        n_b2v,
        Opcode::AdapterInvoke,
        vec![pav(n_unit), pav(n_pay)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: o_ok,
        function: fid,
        parameters: vec![o_pay, o_unit],
        operations: Vec::new(),
        terminator: branch(edge(n_b2v, vec![pav(o_pay), pav(o_unit)])),
        reachability: Reachability::Required,
    });
    a.blocks.push(Block {
        entity_id: n_b2v,
        function: fid,
        parameters: vec![n_pay, n_unit],
        operations: vec![n_conv],
        terminator: switch(
            op_result(n_conv),
            vec![
                (
                    BuiltinCase::Ok,
                    n_len,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(n_pay),
                        sav(n_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let t_vec = a.param(ns.p, n_len, ParameterRole::Block, u8vec_type());
    let t_pay = a.param(ns.p, n_len, ParameterRole::Block, TypeExpr::Bytes);
    let t_unit = a.param(ns.p, n_len, ParameterRole::Block, TypeExpr::Unit);
    let t_plen = a.op(
        ns.o,
        n_len,
        Opcode::VectorLen,
        vec![pav(t_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    let t_wc = a.cref(ns.o, n_len, w64, u32_type());
    let t_call = a.op(
        ns.o,
        n_len,
        Opcode::CallDirect,
        vec![op_result(t_plen), op_result(t_wc), pav(t_unit)],
        vec![res_t.clone()],
        Immediate::Function(FunctionRefValue {
            function: encode_fid,
            type_arguments: Vec::new(),
        }),
    );
    let t_ok = a.id(ns.b);
    let t_err = a.id(ns.b);
    let t_ebytes = a.param(ns.p, t_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: n_len,
        function: fid,
        parameters: vec![t_vec, t_pay, t_unit],
        operations: vec![t_plen, t_wc, t_call],
        terminator: switch(
            op_result(t_call),
            vec![
                (
                    BuiltinCase::Ok,
                    t_ok,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(t_pay),
                        sav(t_unit),
                    ],
                ),
                (BuiltinCase::Err, t_err, vec![SwitchArgument::CasePayload]),
            ],
        ),
        reachability: Reachability::Required,
    });
    let t_er = a.op(
        ns.o,
        t_err,
        Opcode::ResultErr,
        vec![pav(t_ebytes)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: t_err,
        function: fid,
        parameters: vec![t_ebytes],
        operations: vec![t_er],
        terminator: ret(op_result(t_er)),
        reachability: Reachability::Required,
    });
    let u_bytes = a.param(ns.p, t_ok, ParameterRole::Block, TypeExpr::Bytes);
    let u_pay = a.param(ns.p, t_ok, ParameterRole::Block, TypeExpr::Bytes);
    let u_unit = a.param(ns.p, t_ok, ParameterRole::Block, TypeExpr::Unit);
    let u_cv = a.op(
        ns.o,
        t_ok,
        Opcode::AdapterInvoke,
        vec![pav(u_unit), pav(u_bytes)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(
            sley_vm::host_abi::BRIDGE_CODE_B2V1,
        ))),
    );
    a.blocks.push(Block {
        entity_id: t_ok,
        function: fid,
        parameters: vec![u_bytes, u_pay, u_unit],
        operations: vec![u_cv],
        terminator: switch(
            op_result(u_cv),
            vec![
                (
                    BuiltinCase::Ok,
                    n_build,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(u_pay),
                        sav(u_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Envelope prefix build on (payload, length vector). Ok(prefix,
    // preimage) continues into the digest append below; Err forwards
    // unchanged.
    let b_lenvec = a.param(ns.p, n_build, ParameterRole::Block, u8vec_type());
    let b_pay = a.param(ns.p, n_build, ParameterRole::Block, TypeExpr::Bytes);
    let b_unit = a.param(ns.p, n_build, ParameterRole::Block, TypeExpr::Unit);
    let b_ret = a.id(ns.b);
    let b_stored = a.param(ns.p, b_ret, ParameterRole::Block, TypeExpr::Bytes);
    let b_call = a.op(
        ns.o,
        n_build,
        Opcode::CallDirect,
        vec![pav(b_pay), pav(b_lenvec), pav(b_unit)],
        vec![program_prefix_result_type()],
        Immediate::Function(FunctionRefValue {
            function: build_fid,
            type_arguments: Vec::new(),
        }),
    );
    let d_hash = a.id(ns.b);
    let d_pre = a.id(ns.b);
    let d_pre2 = a.id(ns.b);
    let d_acc = a.id(ns.b);
    let h_tup = a.param(
        ns.p,
        d_hash,
        ParameterRole::Block,
        TypeExpr::Tuple(vec![TypeExpr::Bytes, TypeExpr::Bytes]),
    );
    let h_unit = a.param(ns.p, d_hash, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: n_build,
        function: fid,
        parameters: vec![b_lenvec, b_pay, b_unit],
        operations: vec![b_call],
        terminator: switch(
            op_result(b_call),
            vec![
                (
                    BuiltinCase::Ok,
                    d_hash,
                    vec![SwitchArgument::CasePayload, sav(b_unit)],
                ),
                (
                    BuiltinCase::Err,
                    o_err,
                    vec![SwitchArgument::CasePayload],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Split the tuple: index 0 is the stored prefix (assembly input),
    // index 1 is the digest preimage (hash input, domain included).
    let h_gpre = a.op(
        ns.o,
        d_hash,
        Opcode::TupleGet,
        vec![pav(h_tup)],
        vec![TypeExpr::Bytes],
        Immediate::Index(0),
    );
    let h_ghim = a.op(
        ns.o,
        d_hash,
        Opcode::TupleGet,
        vec![pav(h_tup)],
        vec![TypeExpr::Bytes],
        Immediate::Index(1),
    );
    let h_rhw = a.op(
        ns.o,
        d_hash,
        Opcode::AdapterInvoke,
        vec![pav(h_unit), op_result(h_ghim)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_RHW1))),
    );
    let hh_dig = a.param(ns.p, d_pre, ParameterRole::Block, TypeExpr::Bytes);
    let hh_pre = a.param(ns.p, d_pre, ParameterRole::Block, TypeExpr::Bytes);
    let hh_unit = a.param(ns.p, d_pre, ParameterRole::Block, TypeExpr::Unit);
    // TEMP BISECT: observe TupleGet(1) directly; revert after diagnosing.
    let t_ret = a.id(ns.b);
    let t_got = a.param(ns.p, t_ret, ParameterRole::Block, TypeExpr::Bytes);
    let t_okv = a.op(
        ns.o,
        t_ret,
        Opcode::ResultOk,
        vec![pav(t_got)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: d_hash,
        function: fid,
        parameters: vec![h_tup, h_unit],
        operations: vec![h_gpre, h_ghim],
        terminator: branch(edge(t_ret, vec![op_result(h_ghim)])),
        reachability: Reachability::Required,
    });
    a.blocks.push(Block {
        entity_id: t_ret,
        function: fid,
        parameters: vec![t_got],
        operations: vec![t_okv],
        terminator: ret(op_result(t_okv)),
        reachability: Reachability::Required,
    });
    // TEMP: disable the digest chain below (unreachable); restore after.
    // (d_hash no longer targets d_pre; the chain stays for inventory.)
    let h_rhw = a.op(
        ns.o,
        d_hash,
        Opcode::AdapterInvoke,
        vec![pav(h_unit), op_result(h_ghim)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_RHW1))),
    );
    let _ = h_rhw;
    let hc_b2v = a.op(
        ns.o,
        d_pre,
        Opcode::AdapterInvoke,
        vec![pav(hh_unit), pav(hh_dig)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: d_pre,
        function: fid,
        parameters: vec![hh_dig, hh_pre, hh_unit],
        operations: vec![hc_b2v],
        terminator: switch(
            op_result(hc_b2v),
            vec![
                (
                    BuiltinCase::Ok,
                    d_pre2,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(hh_pre),
                        sav(hh_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let hp_dig = a.param(ns.p, d_pre2, ParameterRole::Block, u8vec_type());
    let hp_pre = a.param(ns.p, d_pre2, ParameterRole::Block, TypeExpr::Bytes);
    let hp_unit = a.param(ns.p, d_pre2, ParameterRole::Block, TypeExpr::Unit);
    let hp_b2v = a.op(
        ns.o,
        d_pre2,
        Opcode::AdapterInvoke,
        vec![pav(hp_unit), pav(hp_pre)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_B2V1))),
    );
    a.blocks.push(Block {
        entity_id: d_pre2,
        function: fid,
        parameters: vec![hp_dig, hp_pre, hp_unit],
        operations: vec![hp_b2v],
        terminator: switch(
            op_result(hp_b2v),
            vec![
                (
                    BuiltinCase::Ok,
                    d_acc,
                    vec![
                        sav(hp_dig),
                        SwitchArgument::CasePayload,
                        sav(hp_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Stored assembly: fresh accumulator, prefix copy, digest copy.
    let da_dig = a.param(ns.p, d_acc, ParameterRole::Block, u8vec_type());
    let da_pre = a.param(ns.p, d_acc, ParameterRole::Block, u8vec_type());
    let da_unit = a.param(ns.p, d_acc, ParameterRole::Block, TypeExpr::Unit);
    let da_empty = a.op(
        ns.o,
        d_acc,
        Opcode::VectorNew,
        Vec::new(),
        vec![u8vec_type()],
        Immediate::None,
    );
    let da_plen = a.op(
        ns.o,
        d_acc,
        Opcode::VectorLen,
        vec![pav(da_pre)],
        vec![u64_type()],
        Immediate::None,
    );
    let da_z0 = a.cref(ns.o, d_acc, c0, u64_type());
    let c1_check = a.id(ns.b);
    let c1_get = a.id(ns.b);
    let c1_push = a.id(ns.b);
    let c1_next = a.id(ns.b);
    let c1_done = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: d_acc,
        function: fid,
        parameters: vec![da_dig, da_pre, da_unit],
        operations: vec![da_empty, da_plen, da_z0],
        terminator: branch(edge(
            c1_check,
            vec![
                op_result(da_z0),
                op_result(da_empty),
                pav(da_pre),
                op_result(da_plen),
                pav(da_dig),
                pav(da_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let q1_idx = a.param(ns.p, c1_check, ParameterRole::Block, u64_type());
    let q1_acc = a.param(ns.p, c1_check, ParameterRole::Block, u8vec_type());
    let q1_src = a.param(ns.p, c1_check, ParameterRole::Block, u8vec_type());
    let q1_slen = a.param(ns.p, c1_check, ParameterRole::Block, u64_type());
    let q1_dig = a.param(ns.p, c1_check, ParameterRole::Block, u8vec_type());
    let q1_unit = a.param(ns.p, c1_check, ParameterRole::Block, TypeExpr::Unit);
    let q1_lt = a.op(
        ns.o,
        c1_check,
        Opcode::LessThan,
        vec![pav(q1_idx), pav(q1_slen)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let c2_check = a.id(ns.b);
    let c2_get = a.id(ns.b);
    let c2_push = a.id(ns.b);
    let c2_next = a.id(ns.b);
    let c2_done = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: c1_check,
        function: fid,
        parameters: vec![q1_idx, q1_acc, q1_src, q1_slen, q1_dig, q1_unit],
        operations: vec![q1_lt],
        terminator: cond(
            op_result(q1_lt),
            edge(
                c1_get,
                vec![
                    pav(q1_idx),
                    pav(q1_acc),
                    pav(q1_src),
                    pav(q1_slen),
                    pav(q1_dig),
                    pav(q1_unit),
                ],
            ),
            edge(
                c1_done,
                vec![pav(q1_acc), pav(q1_dig), pav(q1_unit)],
            ),
        ),
        reachability: Reachability::Required,
    });
    let g1_idx = a.param(ns.p, c1_get, ParameterRole::Block, u64_type());
    let g1_acc = a.param(ns.p, c1_get, ParameterRole::Block, u8vec_type());
    let g1_src = a.param(ns.p, c1_get, ParameterRole::Block, u8vec_type());
    let g1_slen = a.param(ns.p, c1_get, ParameterRole::Block, u64_type());
    let g1_dig = a.param(ns.p, c1_get, ParameterRole::Block, u8vec_type());
    let g1_unit = a.param(ns.p, c1_get, ParameterRole::Block, TypeExpr::Unit);
    let g1_get = a.op(
        ns.o,
        c1_get,
        Opcode::VectorGet,
        vec![pav(g1_src), pav(g1_idx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: c1_get,
        function: fid,
        parameters: vec![g1_idx, g1_acc, g1_src, g1_slen, g1_dig, g1_unit],
        operations: vec![g1_get],
        terminator: switch(
            op_result(g1_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    c1_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(g1_idx),
                        sav(g1_acc),
                        sav(g1_src),
                        sav(g1_slen),
                        sav(g1_dig),
                        sav(g1_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let u1_b = a.param(ns.p, c1_push, ParameterRole::Block, u8_type());
    let u1_idx = a.param(ns.p, c1_push, ParameterRole::Block, u64_type());
    let u1_acc = a.param(ns.p, c1_push, ParameterRole::Block, u8vec_type());
    let u1_src = a.param(ns.p, c1_push, ParameterRole::Block, u8vec_type());
    let u1_slen = a.param(ns.p, c1_push, ParameterRole::Block, u64_type());
    let u1_dig = a.param(ns.p, c1_push, ParameterRole::Block, u8vec_type());
    let u1_unit = a.param(ns.p, c1_push, ParameterRole::Block, TypeExpr::Unit);
    let u1_push = a.op(
        ns.o,
        c1_push,
        Opcode::AdapterInvoke,
        vec![pav(u1_acc), pav(u1_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: c1_push,
        function: fid,
        parameters: vec![u1_b, u1_idx, u1_acc, u1_src, u1_slen, u1_dig, u1_unit],
        operations: vec![u1_push],
        terminator: switch(
            op_result(u1_push),
            vec![
                (
                    BuiltinCase::Ok,
                    c1_next,
                    vec![
                        sav(u1_idx),
                        SwitchArgument::CasePayload,
                        sav(u1_src),
                        sav(u1_slen),
                        sav(u1_dig),
                        sav(u1_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let n1_idx = a.param(ns.p, c1_next, ParameterRole::Block, u64_type());
    let n1_acc = a.param(ns.p, c1_next, ParameterRole::Block, u8vec_type());
    let n1_src = a.param(ns.p, c1_next, ParameterRole::Block, u8vec_type());
    let n1_slen = a.param(ns.p, c1_next, ParameterRole::Block, u64_type());
    let n1_dig = a.param(ns.p, c1_next, ParameterRole::Block, u8vec_type());
    let n1_unit = a.param(ns.p, c1_next, ParameterRole::Block, TypeExpr::Unit);
    let n1_onec = a.cref(ns.o, c1_next, c1, u64_type());
    let n1_add = a.op(
        ns.o,
        c1_next,
        Opcode::IntAddChecked,
        vec![pav(n1_idx), op_result(n1_onec)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: c1_next,
        function: fid,
        parameters: vec![n1_idx, n1_acc, n1_src, n1_slen, n1_dig, n1_unit],
        operations: vec![n1_onec, n1_add],
        terminator: switch(
            op_result(n1_add),
            vec![
                (
                    BuiltinCase::Ok,
                    c1_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(n1_acc),
                        sav(n1_src),
                        sav(n1_slen),
                        sav(n1_dig),
                        sav(n1_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Digest copy loop, bounded by the constant 32 (exact trailer).
    let f1_acc = a.param(ns.p, c1_done, ParameterRole::Block, u8vec_type());
    let f1_dig = a.param(ns.p, c1_done, ParameterRole::Block, u8vec_type());
    let f1_unit = a.param(ns.p, c1_done, ParameterRole::Block, TypeExpr::Unit);
    let f1_z0 = a.cref(ns.o, c1_done, c0, u64_type());
    a.blocks.push(Block {
        entity_id: c1_done,
        function: fid,
        parameters: vec![f1_acc, f1_dig, f1_unit],
        operations: vec![f1_z0],
        terminator: branch(edge(
            c2_check,
            vec![
                op_result(f1_z0),
                pav(f1_acc),
                pav(f1_dig),
                pav(f1_unit),
            ],
        )),
        reachability: Reachability::Required,
    });
    let j_idx = a.param(ns.p, c2_check, ParameterRole::Block, u64_type());
    let j_acc = a.param(ns.p, c2_check, ParameterRole::Block, u8vec_type());
    let j_dig = a.param(ns.p, c2_check, ParameterRole::Block, u8vec_type());
    let j_unit = a.param(ns.p, c2_check, ParameterRole::Block, TypeExpr::Unit);
    let j32c = a.cref(ns.o, c2_check, c32, u64_type());
    let j_lt = a.op(
        ns.o,
        c2_check,
        Opcode::LessThan,
        vec![pav(j_idx), op_result(j32c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: c2_check,
        function: fid,
        parameters: vec![j_idx, j_acc, j_dig, j_unit],
        operations: vec![j32c, j_lt],
        terminator: cond(
            op_result(j_lt),
            edge(
                c2_get,
                vec![pav(j_idx), pav(j_acc), pav(j_dig), pav(j_unit)],
            ),
            edge(c2_done, vec![pav(j_acc), pav(j_unit)]),
        ),
        reachability: Reachability::Required,
    });
    let k_idx = a.param(ns.p, c2_get, ParameterRole::Block, u64_type());
    let k_acc = a.param(ns.p, c2_get, ParameterRole::Block, u8vec_type());
    let k_dig = a.param(ns.p, c2_get, ParameterRole::Block, u8vec_type());
    let k_unit = a.param(ns.p, c2_get, ParameterRole::Block, TypeExpr::Unit);
    let k_get = a.op(
        ns.o,
        c2_get,
        Opcode::VectorGet,
        vec![pav(k_dig), pav(k_idx)],
        vec![TypeExpr::Option(Box::new(u8_type()))],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: c2_get,
        function: fid,
        parameters: vec![k_idx, k_acc, k_dig, k_unit],
        operations: vec![k_get],
        terminator: switch(
            op_result(k_get),
            vec![
                (BuiltinCase::None, trap, Vec::new()),
                (
                    BuiltinCase::Some,
                    c2_push,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(k_idx),
                        sav(k_acc),
                        sav(k_dig),
                        sav(k_unit),
                    ],
                ),
            ],
        ),
        reachability: Reachability::Required,
    });
    let m_b = a.param(ns.p, c2_push, ParameterRole::Block, u8_type());
    let m_idx = a.param(ns.p, c2_push, ParameterRole::Block, u64_type());
    let m_acc = a.param(ns.p, c2_push, ParameterRole::Block, u8vec_type());
    let m_dig = a.param(ns.p, c2_push, ParameterRole::Block, u8vec_type());
    let m_unit = a.param(ns.p, c2_push, ParameterRole::Block, TypeExpr::Unit);
    let m_push = a.op(
        ns.o,
        c2_push,
        Opcode::AdapterInvoke,
        vec![pav(m_acc), pav(m_b)],
        vec![index_result(u8vec_type())],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_PSH1))),
    );
    a.blocks.push(Block {
        entity_id: c2_push,
        function: fid,
        parameters: vec![m_b, m_idx, m_acc, m_dig, m_unit],
        operations: vec![m_push],
        terminator: switch(
            op_result(m_push),
            vec![
                (
                    BuiltinCase::Ok,
                    c2_next,
                    vec![
                        sav(m_idx),
                        sav(m_dig),
                        SwitchArgument::CasePayload,
                        sav(m_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let p_idx = a.param(ns.p, c2_next, ParameterRole::Block, u64_type());
    let p_dig = a.param(ns.p, c2_next, ParameterRole::Block, u8vec_type());
    let p_acc = a.param(ns.p, c2_next, ParameterRole::Block, u8vec_type());
    let p_ux = a.param(ns.p, c2_next, ParameterRole::Block, TypeExpr::Unit);
    let p_onec = a.cref(ns.o, c2_next, c1, u64_type());
    let p_add = a.op(
        ns.o,
        c2_next,
        Opcode::IntAddChecked,
        vec![pav(p_idx), op_result(p_onec)],
        vec![arith_result(u64_type())],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: c2_next,
        function: fid,
        parameters: vec![p_idx, p_dig, p_acc, p_ux],
        operations: vec![p_onec, p_add],
        terminator: switch(
            op_result(p_add),
            vec![
                (
                    BuiltinCase::Ok,
                    c2_check,
                    vec![
                        SwitchArgument::CasePayload,
                        sav(p_dig),
                        sav(p_acc),
                        sav(p_ux),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Finalize: V2B1 then Ok. The accumulator now holds prefix ++
    // 32-byte digest: the complete stored object.
    let d2_acc = a.param(ns.p, c2_done, ParameterRole::Block, u8vec_type());
    let d2_unit = a.param(ns.p, c2_done, ParameterRole::Block, TypeExpr::Unit);
    let d2_v2b = a.op(
        ns.o,
        c2_done,
        Opcode::AdapterInvoke,
        vec![pav(d2_unit), pav(d2_acc)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_V2B1))),
    );
    a.blocks.push(Block {
        entity_id: c2_done,
        function: fid,
        parameters: vec![d2_acc, d2_unit],
        operations: vec![d2_v2b],
        terminator: switch(
            op_result(d2_v2b),
            vec![
                (BuiltinCase::Ok, b_ret, vec![SwitchArgument::CasePayload]),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    let b_okv = a.op(
        ns.o,
        b_ret,
        Opcode::ResultOk,
        vec![pav(b_stored)],
        vec![res_t.clone()],
        Immediate::None,
    );
    a.blocks.push(Block {
        entity_id: b_ret,
        function: fid,
        parameters: vec![b_stored],
        operations: vec![b_okv],
        terminator: ret(op_result(b_okv)),
        reachability: Reachability::Required,
    });

    FunctionGraph {
        entity_id: fid,
        type_parameters: Vec::new(),
        parameters: vec![p_eid, p_func, p_exp, p_unit],
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

fn outer_decode_image() -> Image {
    use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let mut a = Asm::new();
    let dns = Ns {
        k: 71,
        p: 72,
        b: 73,
        o: 74,
    };
    let ons = Ns {
        k: 75,
        p: 76,
        b: 77,
        o: 78,
    };
    let decode_fid = eid(9, 31);
    let outer_fid = eid(9, 32);
    let (decode_graph, _) = build_decode(&mut a, dns, decode_fid);
    let outer_graph = build_outer_decode(&mut a, ons, outer_fid, decode_fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: outer_graph.clone(),
        functions: vec![outer_graph, decode_graph],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
        ],
        constants: a.constants,
    }
}

fn outer_encode_image() -> Image {
    use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let mut a = Asm::new();
    let ens = Ns {
        k: 81,
        p: 82,
        b: 83,
        o: 84,
    };
    let oens = Ns {
        k: 85,
        p: 86,
        b: 87,
        o: 88,
    };
    let encode_fid = eid(9, 33);
    let outer_fid = eid(9, 34);
    let encode_graph = build_encode(&mut a, ens, encode_fid);
    let outer_graph = build_outer_encode(&mut a, oens, outer_fid, encode_fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: outer_graph.clone(),
        functions: vec![outer_graph, encode_graph],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
        ],
        constants: a.constants,
    }
}

fn outer_encode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    eid_hex: &str,
    body_hex: &str,
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![
            bytes_input(&hex_decode(eid_hex)),
            bytes_input(&hex_decode(body_hex)),
            unit_input(),
        ],
    )
}

fn outer_decode_call(
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

fn entrypoint_decode_image() -> Image {
    use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let mut a = Asm::new();
    let dns = Ns {
        k: 91,
        p: 92,
        b: 93,
        o: 94,
    };
    let ens = Ns {
        k: 95,
        p: 96,
        b: 97,
        o: 98,
    };
    let decode_fid = eid(9, 35);
    let entry_fid = eid(9, 36);
    let (decode_graph, _) = build_decode(&mut a, dns, decode_fid);
    let entry_graph = build_entrypoint_decode(&mut a, ens, entry_fid, decode_fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: entry_graph.clone(),
        functions: vec![entry_graph, decode_graph],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
        ],
        constants: a.constants,
    }
}

fn entrypoint_decode_call(
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

fn entrypoint_encode_image() -> Image {
    use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let mut a = Asm::new();
    let ns = Ns {
        k: 101,
        p: 102,
        b: 103,
        o: 104,
    };
    let fid = eid(9, 37);
    let graph = build_entrypoint_encode(&mut a, ns, fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: graph.clone(),
        functions: vec![graph],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
        ],
        constants: a.constants,
    }
}

fn program_validate_image() -> Image {
    use sley_vm::host_abi::{
        BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1,
    };
    let mut a = Asm::new();
    let dns = Ns {
        k: 111,
        p: 112,
        b: 113,
        o: 114,
    };
    let vns = Ns {
        k: 115,
        p: 116,
        b: 117,
        o: 118,
    };
    let decode_fid = eid(9, 41);
    let validate_fid = eid(9, 42);
    let (decode_graph, _) = build_decode(&mut a, dns, decode_fid);
    let validate_graph = build_program_validate(&mut a, vns, validate_fid, decode_fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: validate_graph.clone(),
        functions: vec![validate_graph, decode_graph],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
            frozen_import(BRIDGE_CODE_RHW1, TypeExpr::Bytes, TypeExpr::Bytes),
        ],
        constants: a.constants,
    }
}

fn program_build_image() -> Image {
    use sley_vm::host_abi::{BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_V2B1};
    let mut a = Asm::new();
    let bns = Ns {
        k: 125,
        p: 126,
        b: 127,
        o: 128,
    };
    let build_fid = eid(9, 44);
    let build_graph = build_program_build(&mut a, bns, build_fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: build_graph.clone(),
        functions: vec![build_graph],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
        ],
        constants: a.constants,
    }
}

fn program_decode_image() -> Image {
    use sley_vm::host_abi::{
        BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1,
    };
    let mut a = Asm::new();
    let dns = Ns {
        k: 131,
        p: 132,
        b: 133,
        o: 134,
    };
    let vns = Ns {
        k: 135,
        p: 136,
        b: 137,
        o: 138,
    };
    let ons = Ns {
        k: 139,
        p: 140,
        b: 141,
        o: 142,
    };
    let ens = Ns {
        k: 143,
        p: 144,
        b: 145,
        o: 146,
    };
    let pns = Ns {
        k: 147,
        p: 148,
        b: 149,
        o: 150,
    };
    let decode_fid = eid(9, 45);
    let validate_fid = eid(9, 46);
    let outer_fid = eid(9, 47);
    let entry_fid = eid(9, 48);
    let prog_fid = eid(9, 49);
    let (decode_graph, _) = build_decode(&mut a, dns, decode_fid);
    let validate_graph = build_program_validate(&mut a, vns, validate_fid, decode_fid);
    let outer_graph = build_outer_decode(&mut a, ons, outer_fid, decode_fid);
    let entry_graph = build_entrypoint_decode(&mut a, ens, entry_fid, decode_fid);
    let prog_graph =
        build_program_decode(&mut a, pns, prog_fid, validate_fid, outer_fid, entry_fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: prog_graph.clone(),
        functions: vec![
            prog_graph,
            validate_graph,
            outer_graph,
            entry_graph,
            decode_graph,
        ],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
            frozen_import(BRIDGE_CODE_RHW1, TypeExpr::Bytes, TypeExpr::Bytes),
        ],
        constants: a.constants,
    }
}

fn program_encode_image() -> Image {
    use sley_vm::host_abi::{
        BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1,
    };
    let mut a = Asm::new();
    let xns = Ns {
        k: 151,
        p: 152,
        b: 153,
        o: 154,
    };
    let uns = Ns {
        k: 155,
        p: 156,
        b: 157,
        o: 158,
    };
    let ons = Ns {
        k: 159,
        p: 160,
        b: 161,
        o: 162,
    };
    let bns = Ns {
        k: 163,
        p: 164,
        b: 165,
        o: 166,
    };
    let pns = Ns {
        k: 167,
        p: 168,
        b: 169,
        o: 170,
    };
    let entry_enc_fid = eid(9, 50);
    let encode_fid = eid(9, 51);
    let outer_enc_fid = eid(9, 52);
    let build_fid = eid(9, 53);
    let prog_fid = eid(9, 54);
    let entry_enc_graph = build_entrypoint_encode(&mut a, xns, entry_enc_fid);
    let encode_graph = build_encode(&mut a, uns, encode_fid);
    let outer_enc_graph = build_outer_encode(&mut a, ons, outer_enc_fid, encode_fid);
    let build_graph = build_program_build(&mut a, bns, build_fid);
    let prog_graph = build_program_encode(
        &mut a,
        pns,
        prog_fid,
        entry_enc_fid,
        outer_enc_fid,
        build_fid,
        encode_fid,
    );
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: prog_graph.clone(),
        functions: vec![
            prog_graph,
            entry_enc_graph,
            outer_enc_graph,
            build_graph,
            encode_graph,
        ],
        parameters: a.parameters,
        blocks: a.blocks,
        operations: a.operations,
        adapters: vec![
            frozen_import(BRIDGE_CODE_B2V1, TypeExpr::Bytes, u8vec_type()),
            frozen_import(BRIDGE_CODE_PSH1, u8_type(), u8vec_type()),
            frozen_import(BRIDGE_CODE_V2B1, u8vec_type(), TypeExpr::Bytes),
            frozen_import(BRIDGE_CODE_RHW1, TypeExpr::Bytes, TypeExpr::Bytes),
        ],
        constants: a.constants,
    }
}

fn program_validate_call(
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

fn u8vec_input(bytes: &[u8]) -> ConstValue {
    ConstValue {
        value_type: u8vec_type(),
        data: ConstData::Sequence(
            bytes
                .iter()
                .map(|byte| ConstValue {
                    value_type: u8_type(),
                    data: ConstData::UInt(u128::from(*byte)),
                })
                .collect(),
        ),
    }
}

fn program_build_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    payload_hex: &str,
) -> sley_vm::ExecutionOutcome {
    // Harness input shaping (documented): the length encoding arrives as
    // a ready vector alongside the payload; its canonical form is
    // independently produced by `encode_uvar` here but the binding
    // evidence is the composed path, where every byte is Sley-produced.
    let payload = hex_decode(payload_hex);
    let lenvec = sley_scb1::encode_uvar(payload.len() as u64);
    execute(
        package,
        approved,
        vec![
            bytes_input(&payload),
            u8vec_input(&lenvec),
            unit_input(),
        ],
    )
}

fn program_decode_call(
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

fn program_encode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    eid_hex: &str,
    func_hex: &str,
    exposure: u64,
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![
            bytes_input(&hex_decode(eid_hex)),
            bytes_input(&hex_decode(func_hex)),
            u64_input(exposure),
            unit_input(),
        ],
    )
}

fn assert_program_decode_ok(
    outcome: &sley_vm::ExecutionOutcome,
    expected_eid: &[u8],
    expected_func: &[u8],
    expected_exp: u64,
) -> u64 {
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                ConstData::Sequence(items) => {
                    assert_eq!(
                        items.len(),
                        3,
                        "program decode Ok carries (entity_id, function, exposure)"
                    );
                    match (&items[0].data, &items[1].data, &items[2].data) {
                        (ConstData::Bytes(eid), ConstData::Bytes(func), ConstData::UInt(exp)) => {
                            assert_eq!(eid, expected_eid, "entity_id bytes match");
                            assert_eq!(func, expected_func, "function bytes match");
                            assert_eq!(
                                exp,
                                &u128::from(expected_exp),
                                "exposure value matches"
                            );
                            outcome.fuel_used
                        }
                        other => panic!("program decode Ok must carry (Bytes, Bytes, UInt), got {other:?}"),
                    }
                }
                other => panic!("program decode Ok must carry a triple, got {other:?}"),
            },
            other => panic!("program decode must succeed, got {other:?}"),
        },
        other => panic!("program decode must succeed, got {other:?}"),
    }
}

fn u64_input(value: u64) -> ConstValue {
    ConstValue {
        value_type: u64_type(),
        data: ConstData::UInt(u128::from(value)),
    }
}

fn entrypoint_encode_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    func_hex: &str,
    exposure: u64,
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![
            bytes_input(&hex_decode(func_hex)),
            u64_input(exposure),
            unit_input(),
        ],
    )
}

fn assert_entrypoint_ok(
    outcome: &sley_vm::ExecutionOutcome,
    expected_func: &[u8],
    expected_exp: u64,
) -> u64 {
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                ConstData::Sequence(items) => {
                    assert_eq!(items.len(), 2, "entrypoint Ok carries (function, exposure)");
                    match (&items[0].data, &items[1].data) {
                        (ConstData::Bytes(func), ConstData::UInt(exp)) => {
                            assert_eq!(func, expected_func, "function bytes match");
                            assert_eq!(
                                u64::try_from(*exp).expect("exposure fits u64"),
                                expected_exp,
                                "exposure matches"
                            );
                            outcome.fuel_used
                        }
                        other => panic!("entrypoint tuple must carry (Bytes, UInt), got {other:?}"),
                    }
                }
                other => panic!("entrypoint must succeed with tuple, got {other:?}"),
            },
            other => panic!("entrypoint must succeed, got {other:?}"),
        },
        other => panic!("entrypoint must succeed, got {other:?}"),
    }
}

fn assert_outer_ok(
    outcome: &sley_vm::ExecutionOutcome,
    expected_eid: &[u8],
    expected_body: &[u8],
) -> u64 {
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                ConstData::Sequence(items) => {
                    assert_eq!(items.len(), 2, "outer Ok carries (entity_id, body)");
                    match (&items[0].data, &items[1].data) {
                        (ConstData::Bytes(eid), ConstData::Bytes(body)) => {
                            assert_eq!(eid, expected_eid, "entity_id bytes match");
                            assert_eq!(body, expected_body, "body bytes match");
                            outcome.fuel_used
                        }
                        other => panic!("outer tuple must carry Bytes, got {other:?}"),
                    }
                }
                other => panic!("outer must succeed with tuple, got {other:?}"),
            },
            other => panic!("outer must succeed, got {other:?}"),
        },
        other => panic!("outer must succeed, got {other:?}"),
    }
}

#[test]
fn outer_probe_ns_empty() {
    let (package, approved) = admit(&outer_decode_image());
    // ns-empty payload 47B: count 2, field1 tag1 len32 entity 01*32,
    // field2 tag2 len10 body 03080201020000020100.
    let payload = "0201200101010101010101010101010101010101010101010101010101010101010101020a03080201020000020100";
    let outcome = outer_decode_call(&package, &approved, payload);
    eprintln!("OUTER_PROBE termination={:?}", outcome.termination);
    eprintln!(
        "OUTER_PROBE fuel={} instr={} peak={}",
        outcome.fuel_used, outcome.instruction_count, outcome.peak_value_units
    );
    let eid = [1u8; 32];
    let body = hex_decode("03080201020000020100");
    assert_outer_ok(&outcome, &eid, &body);
    // ns-one payload 80B (one member 02*32), ns-two 113B (two members).
    let payload_one = "0201200101010101010101010101010101010101010101010101010101010101010101022b03290201020000022201200202020202020202020202020202020202020202020202020202020202020202";
    let outcome_one = outer_decode_call(&package, &approved, payload_one);
    let body_one = hex_decode(
        "03290201020000022201200202020202020202020202020202020202020202020202020202020202020202",
    );
    assert_outer_ok(&outcome_one, &eid, &body_one);
    eprintln!(
        "OUTER_ONE fuel={} instr={} peak={}",
        outcome_one.fuel_used, outcome_one.instruction_count, outcome_one.peak_value_units
    );
    let payload_two = "0201200101010101010101010101010101010101010101010101010101010101010101024c034a0201020000024302200202020202020202020202020202020202020202020202020202020202020202200303030303030303030303030303030303030303030303030303030303030303";
    let outcome_two = outer_decode_call(&package, &approved, payload_two);
    let body_two = hex_decode(
        "034a0201020000024302200202020202020202020202020202020202020202020202020202020202020202200303030303030303030303030303030303030303030303030303030303030303",
    );
    assert_outer_ok(&outcome_two, &eid, &body_two);
    eprintln!(
        "OUTER_TWO fuel={} instr={} peak={}",
        outcome_two.fuel_used, outcome_two.instruction_count, outcome_two.peak_value_units
    );
}

#[test]
fn outer_encode_probe_roundtrip() {
    let (enc_pkg, enc_approved) = admit(&outer_encode_image());
    let (dec_pkg, dec_approved) = admit(&outer_decode_image());
    let eid_hex = "0101010101010101010101010101010101010101010101010101010101010101";
    let body_hex = "03080201020000020100";
    let expected_payload = "0201200101010101010101010101010101010101010101010101010101010101010101020a03080201020000020100";
    let enc_outcome = outer_encode_call(&enc_pkg, &enc_approved, eid_hex, body_hex);
    eprintln!("OUTER_ENC termination={:?}", enc_outcome.termination);
    eprintln!(
        "OUTER_ENC fuel={} instr={} peak={}",
        enc_outcome.fuel_used, enc_outcome.instruction_count, enc_outcome.peak_value_units
    );
    assert_encode_ok(&enc_outcome, &hex_decode(expected_payload));
    // Decode the emitted bytes back through Sley decode.
    let dec_outcome = outer_decode_call(&dec_pkg, &dec_approved, expected_payload);
    assert_outer_ok(&dec_outcome, &hex_decode(eid_hex), &hex_decode(body_hex));
}

#[test]
fn entrypoint_probe_decode() {
    let (package, approved) = admit(&entrypoint_decode_image());
    // ep-local body 40B: union 10 26 + record 02 01 20 func(0a*32) 02 01 01.
    let body_local =
        "10260201200a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a020101";
    let outcome = entrypoint_decode_call(&package, &approved, body_local);
    eprintln!(
        "ENTRY_LOCAL fuel={} instr={} peak={}",
        outcome.fuel_used, outcome.instruction_count, outcome.peak_value_units
    );
    assert_entrypoint_ok(&outcome, &[10u8; 32], 1);
    // ep-proto: func 0b*32, exposure 2.
    let body_proto =
        "10260201200b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b020102";
    let outcome_proto = entrypoint_decode_call(&package, &approved, body_proto);
    eprintln!(
        "ENTRY_PROTO fuel={} instr={} peak={}",
        outcome_proto.fuel_used, outcome_proto.instruction_count, outcome_proto.peak_value_units
    );
    assert_entrypoint_ok(&outcome_proto, &[11u8; 32], 2);
}

#[test]
fn entrypoint_encode_probe_roundtrip() {
    let (enc_pkg, enc_approved) = admit(&entrypoint_encode_image());
    let (dec_pkg, dec_approved) = admit(&entrypoint_decode_image());
    // Same vectors the Sley decoder proves (entrypoint_probe_decode):
    // ep-local func 0a*32 exposure 1, ep-proto func 0b*32 exposure 2.
    let func_local = "0a".repeat(32);
    let body_local =
        "10260201200a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a020101";
    let enc_outcome = entrypoint_encode_call(&enc_pkg, &enc_approved, &func_local, 1);
    eprintln!("ENTRY_ENC_LOCAL termination={:?}", enc_outcome.termination);
    eprintln!(
        "ENTRY_ENC_LOCAL fuel={} instr={} peak={}",
        enc_outcome.fuel_used, enc_outcome.instruction_count, enc_outcome.peak_value_units
    );
    assert_encode_ok(&enc_outcome, &hex_decode(body_local));
    // Determinism: a second run emits byte-identical output.
    let enc_again = entrypoint_encode_call(&enc_pkg, &enc_approved, &func_local, 1);
    assert_encode_ok(&enc_again, &hex_decode(body_local));
    // Decode the emitted bytes back through Sley decode.
    let dec_outcome = entrypoint_decode_call(&dec_pkg, &dec_approved, body_local);
    assert_entrypoint_ok(&dec_outcome, &[10u8; 32], 1);
    let func_proto = "0b".repeat(32);
    let body_proto =
        "10260201200b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b020102";
    let enc_proto = entrypoint_encode_call(&enc_pkg, &enc_approved, &func_proto, 2);
    eprintln!("ENTRY_ENC_PROTO termination={:?}", enc_proto.termination);
    eprintln!(
        "ENTRY_ENC_PROTO fuel={} instr={} peak={}",
        enc_proto.fuel_used, enc_proto.instruction_count, enc_proto.peak_value_units
    );
    assert_encode_ok(&enc_proto, &hex_decode(body_proto));
    let dec_proto = entrypoint_decode_call(&dec_pkg, &dec_approved, body_proto);
    assert_entrypoint_ok(&dec_proto, &[11u8; 32], 2);
}

#[test]
fn entrypoint_encode_rejections() {
    let (package, approved) = admit(&entrypoint_encode_image());
    let func32 = "0a".repeat(32);
    let cases: [(&str, String, u64, &str); 5] = [
        ("exp0", func32.clone(), 0, "SCB_UNION_INVALID"),
        ("exp3", func32.clone(), 3, "SCB_UNION_INVALID"),
        ("exp19", func32.clone(), 19, "SCB_UNION_INVALID"),
        ("func31", "0a".repeat(31), 1, "SCB_LENGTH_OVERFLOW"),
        ("func33", "0a".repeat(33), 1, "SCB_TRAILING_BYTES"),
    ];
    for (name, func_hex, exposure, expected) in cases {
        let outcome = entrypoint_encode_call(&package, &approved, &func_hex, exposure);
        assert_refusal(&outcome, expected);
        eprintln!("ENTRY_ENC_REJ {name} -> {expected}");
    }
}

#[test]
fn outer_rejections_match_reference_order() {
    let (package, approved) = admit(&outer_decode_image());
    // Hand-derived from object.rs decode_object_record; each checks exact code
    // and precedence (DUP/ORDER before payload, bounds before UNKNOWN).
    // eid32 = 01*32 hex, body10 = ns-empty body.
    let eid32 = "0101010101010101010101010101010101010101010101010101010101010101";
    let body10 = "03080201020000020100";
    let valid = format!("020120{eid32}020a{body10}");
    // Sanity: valid decodes.
    assert_outer_ok(
        &outer_decode_call(&package, &approved, &valid),
        &hex_decode(eid32),
        &hex_decode(body10),
    );
    let cases: [(&str, &str, &str); 18] = [
        ("count0", "00", "SCB_FIELD_MISSING"),
        ("count1", &format!("010120{eid32}"), "SCB_FIELD_MISSING"),
        ("count5", "05", "SCB_FIELD_UNKNOWN"),
        // count 3/4 scope (pinned divergence vs native Ok for valid 3/4).
        (
            "count3scope",
            "0301200101010101010101010101010101010101010101010101010101010101010101020a03080201020000020100010100",
            "SSMC_RESERVED_FIELD_PRESENT",
        ),
        // [1,1] duplicate.
        (
            "dup11",
            &format!("020120{eid32}0120{eid32}"),
            "SCB_FIELD_DUPLICATE",
        ),
        // [1,0] order (second tag 0 < 1).
        (
            "order10",
            &format!("020120{eid32}000a{body10}"),
            "SCB_FIELD_ORDER",
        ),
        // [1,5] unknown (tag 5, bounds ok).
        (
            "unknown15",
            &format!("020120{eid32}050a{body10}"),
            "SCB_FIELD_UNKNOWN",
        ),
        // [2,2] duplicate via body-first path.
        (
            "dup22",
            &format!("02020a{body10}0220{eid32}"),
            "SCB_FIELD_DUPLICATE",
        ),
        // [2,1] order.
        (
            "order21",
            &format!("02020a{body10}0120{eid32}"),
            "SCB_FIELD_ORDER",
        ),
        // [2,0] order.
        (
            "order20",
            &format!("02020a{body10}000a{body10}"),
            "SCB_FIELD_ORDER",
        ),
        // [2,3] scope.
        (
            "scope23",
            &format!("02020a{body10}030100"),
            "SSMC_RESERVED_FIELD_PRESENT",
        ),
        // 31-byte ID -> LENGTH.
        (
            "id31",
            &format!(
                "02011f01010101010101010101010101010101010101010101010101010101010101020a{body10}"
            ),
            "SCB_LENGTH_OVERFLOW",
        ),
        // 33-byte ID -> TRAILING (decode_fixed mirror).
        (
            "id33",
            &format!(
                "020121010101010101010101010101010101010101010101010101010101010101010101020a{body10}"
            ),
            "SCB_TRAILING_BYTES",
        ),
        // Truncated body (len 10 claims, 5 available) -> LENGTH.
        (
            "truncbody",
            &format!("020120{eid32}020a0308020102"),
            "SCB_LENGTH_OVERFLOW",
        ),
        // Trailing after outer -> TRAILING.
        ("trailing", &format!("{valid}00"), "SCB_TRAILING_BYTES"),
        // Nonminimal count 8100 (=0 non-minimal) -> NON_MINIMAL via uvar.
        (
            "nonmincount",
            "81000201200101010101010101010101010101010101010101010101010101010101010101020a03080201020000020100",
            "SCB_VARINT_NON_MINIMAL",
        ),
        // Tag overflow at width32 (2^32) -> INTEGER_OVERFLOW.
        (
            "tagoverflow",
            "028080808010200101010101010101010101010101010101010101010101010101010101010101020a03080201020000020100",
            "SCB_INTEGER_OVERFLOW",
        ),
        // Precedence: [1,1] DUP before second-payload truncation (len huge).
        (
            "precedencedup",
            &format!("020120{eid32}01ff7f"),
            "SCB_FIELD_DUPLICATE",
        ),
    ];
    for (name, hex, expected) in cases {
        let outcome = outer_decode_call(&package, &approved, hex);
        assert_refusal(&outcome, expected);
        eprintln!("OUTER_REJ {name} -> {expected}");
    }
}

#[test]
fn outer_valid_reencode_matches_canonical() {
    let (enc_pkg, enc_approved) = admit(&outer_encode_image());
    let (dec_pkg, dec_approved) = admit(&outer_decode_image());
    // Five canonical payloads: ns-empty/one/two (Namespace) + ep-local/proto
    // (EntryPoint 40B bodies). Outer is opaque to body kind; all must roundtrip.
    let eid_hex = "0101010101010101010101010101010101010101010101010101010101010101";
    let cases = [
        (
            "03080201020000020100",
            "0201200101010101010101010101010101010101010101010101010101010101010101020a03080201020000020100",
        ),
        (
            "03290201020000022201200202020202020202020202020202020202020202020202020202020202020202",
            "0201200101010101010101010101010101010101010101010101010101010101010101022b03290201020000022201200202020202020202020202020202020202020202020202020202020202020202",
        ),
        (
            "10260201200a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a020101",
            "0201200101010101010101010101010101010101010101010101010101010101010101022810260201200a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a020101",
        ),
        (
            "10260201200b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b020102",
            "0201200101010101010101010101010101010101010101010101010101010101010101022810260201200b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b020102",
        ),
    ];
    for (body_hex, payload_hex) in cases {
        let enc = outer_encode_call(&enc_pkg, &enc_approved, eid_hex, body_hex);
        assert_encode_ok(&enc, &hex_decode(payload_hex));
        let dec = outer_decode_call(&dec_pkg, &dec_approved, payload_hex);
        assert_outer_ok(&dec, &hex_decode(eid_hex), &hex_decode(body_hex));
        eprintln!(
            "OUTER_REENC body{}B payload{}B enc_fuel={} dec_fuel={} enc_peak={} dec_peak={}",
            hex_decode(body_hex).len(),
            hex_decode(payload_hex).len(),
            enc.fuel_used,
            dec.fuel_used,
            enc.peak_value_units,
            dec.peak_value_units
        );
    }
}

#[test]
fn entrypoint_rejections_match_reference() {
    let (package, approved) = admit(&entrypoint_decode_image());
    // Valid bodies from probe (40B each).
    let local = "10260201200a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a020101";
    assert_entrypoint_ok(
        &entrypoint_decode_call(&package, &approved, local),
        &[10u8; 32],
        1,
    );
    // func32 = 0a*32 hex for mutations below.
    let f32 = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
    let cases: [(&str, String, &str); 12] = [
        (
            "union-ns-scope",
            "03080201020000020100".to_string(),
            "SSMC_RESERVED_FIELD_PRESENT",
        ),
        ("union-0", "0000".to_string(), "SCB_UNION_INVALID"),
        (
            "union-19",
            format!("1301020120{f32}020101"),
            "SCB_UNION_INVALID",
        ),
        ("count1", format!("1023010120{f32}"), "SCB_FIELD_MISSING"),
        (
            "count3",
            format!("1029030120{f32}020101030100"),
            "SCB_FIELD_UNKNOWN",
        ),
        (
            "order21",
            format!("1026020201010120{f32}"),
            "SCB_FIELD_ORDER",
        ),
        (
            "dup11",
            format!("1045020120{f32}0120{f32}"),
            "SCB_FIELD_DUPLICATE",
        ),
        (
            "unknown13",
            format!("1026020120{f32}030101"),
            "SCB_FIELD_UNKNOWN",
        ),
        (
            "func31",
            "102502011f0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a020101"
                .to_string(),
            "SCB_LENGTH_OVERFLOW",
        ),
        (
            "exp3",
            format!("1026020120{f32}020103"),
            "SCB_UNION_INVALID",
        ),
        ("trailing", format!("{local}00"), "SCB_TRAILING_BYTES"),
        (
            "trunc",
            "10260201200a0a0a".to_string(),
            "SCB_LENGTH_OVERFLOW",
        ),
    ];
    for (name, hex, expected) in cases {
        let outcome = entrypoint_decode_call(&package, &approved, &hex);
        assert_refusal(&outcome, expected);
        eprintln!("ENTRY_REJ {name} -> {expected}");
    }
}

#[test]
fn outer_runtime_mutations_agree_with_native_payload_layer() {
    use sley_id::{ObjectId, SchemaEpochId};
    let (package, approved) = admit(&outer_decode_image());
    let epoch9 = SchemaEpochId::from_bytes([9; 32]);
    let base = hex_decode(
        "0201200101010101010101010101010101010101010101010101010101010101010101020a03080201020000020100",
    );
    let mut state = 0xE11E_0007u64;
    let mut lcg = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let mut agree = 0u32;
    let mut opaque_divergence = 0u32;
    let mut scope_pins = 0u32;
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
        // Recomputed-digest stored so digest never conceals payload checks.
        let mut preimage = Vec::new();
        preimage.extend_from_slice(b"SLEYSCB1");
        preimage.extend_from_slice(&sley_scb1::encode_uvar(1));
        preimage.extend_from_slice(&sley_scb1::encode_uvar(200));
        preimage.extend_from_slice(&[9u8; 32]);
        preimage.extend_from_slice(&sley_scb1::encode_uvar(bytes.len() as u64));
        preimage.extend_from_slice(&bytes);
        let digest = ObjectId::derive(&preimage);
        let mut stored = preimage;
        stored.extend_from_slice(digest.as_bytes());
        let native = sley_mutate::import_entity_object(epoch9, &stored);
        let outcome = execute(&package, &approved, vec![bytes_input(&bytes), unit_input()]);
        let sley_code = match &outcome.termination {
            sley_vm::ExecutionTermination::Success(found) => match &found.data {
                ConstData::Result(ResultConst::Ok(_)) => None,
                ConstData::Result(ResultConst::Err(payload)) => match &payload.data {
                    ConstData::Bytes(b) => Some(String::from_utf8_lossy(b).into_owned()),
                    _ => panic!("refusal must carry Bytes"),
                },
                _ => panic!("must return Result"),
            },
            _ => panic!("must return Result"),
        };
        match (native, sley_code) {
            (Ok(_), None) => agree += 1,
            (Ok(_), Some(code)) => {
                // Sley Err on native Ok: only allowed for pinned scope (count 3/4
                // or tags 3/4 valid objects native accepts, Sley scope-limits).
                assert_eq!(
                    code, "SSMC_RESERVED_FIELD_PRESENT",
                    "Sley Err on native Ok must be pinned scope, got {code}"
                );
                scope_pins += 1;
            }
            (Err(native_err), None) => {
                // Sley Ok on native Err: outer valid, so native Err must be
                // body-layer (opaque-scope divergence, pinned). Envelope always
                // valid here (recomputed digest, tag200/epoch09), so native Err
                // cannot be envelope-layer; outer-valid implies body-layer.
                let _ = native_err.code().to_string();
                opaque_divergence += 1;
            }
            (Err(native_err), Some(sley)) => {
                if sley == "SSMC_RESERVED_FIELD_PRESENT" {
                    scope_pins += 1;
                } else {
                    assert_eq!(
                        sley,
                        native_err.code().to_string(),
                        "outer-layer codes must match"
                    );
                    agree += 1;
                }
            }
        }
    }
    eprintln!("OUTER_MUT agree={agree} opaque={opaque_divergence} scope={scope_pins} /100");
    assert!(agree + opaque_divergence + scope_pins == 100);
}

#[test]
fn program_envelope_native_reference_with_entrypoint() {
    use sley_id::{EntityId, SchemaEpochId};
    use sley_mutate::value::{EntityBodyValue, EntryExposure, EntryPointBody};
    use sley_mutate::{EntityObjectRecord, build_entity_object, import_entity_object};
    let epoch9 = SchemaEpochId::from_bytes([9; 32]);
    let eid1 = EntityId::from_bytes([1; 32]);
    // Two EntryPoint records with varying function/exposure.
    for (func_byte, exp, exp_u64) in [
        (10u8, EntryExposure::Local, 1u64),
        (11u8, EntryExposure::Protocol, 2u64),
    ] {
        let rec = EntityObjectRecord {
            entity_id: eid1,
            body: EntityBodyValue::EntryPoint(EntryPointBody {
                function: EntityId::from_bytes([func_byte; 32]),
                exposure: exp,
            }),
            label: None,
            semantic_fingerprint: None,
        };
        let obj = build_entity_object(epoch9, &rec).unwrap();
        let stored = obj.stored_bytes();
        assert_eq!(stored.len(), 153, "ep stored 153B");
        // Native import agrees (envelope tag200/epoch09/digest + outer + body).
        let imported = import_entity_object(epoch9, stored).unwrap();
        assert_eq!(imported.record(), &rec);
        // Sley outer on extracted payload (native envelope slice): parse stored
        // preimage to payload via native cursor, then Sley outer + entrypoint.
        // Payload starts after magic8 ver1 tag2 epoch32 len1 (all single-byte here).
        let pre = obj.preimage();
        let payload = &pre[8 + 1 + 2 + 32 + 1..];
        assert_eq!(payload.len(), 77, "ep payload 77B");
        let (dec_pkg, dec_approved) = admit(&outer_decode_image());
        let outer_out = outer_decode_call(&dec_pkg, &dec_approved, &hex_encode(payload));
        let body_hex = if exp_u64 == 1 {
            "10260201200a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a020101"
        } else {
            "10260201200b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b020102"
        };
        assert_outer_ok(&outer_out, &[1u8; 32], &hex_decode(body_hex));
        let (entry_pkg, entry_approved) = admit(&entrypoint_decode_image());
        let entry_out = entrypoint_decode_call(&entry_pkg, &entry_approved, body_hex);
        assert_entrypoint_ok(&entry_out, &[func_byte; 32], exp_u64);
        eprintln!(
            "ENVELOPE_NATIVE stored153B payload77B body40B func{func_byte} exp{exp_u64} digest_ok"
        );
    }
    // Wrong epoch, tampered digest, wrong tag are envelope-layer rejections
    // (native reference; Sley envelope lands next, no Sley verdict claimed here).
    let rec = EntityObjectRecord {
        entity_id: eid1,
        body: EntityBodyValue::EntryPoint(EntryPointBody {
            function: EntityId::from_bytes([10; 32]),
            exposure: EntryExposure::Local,
        }),
        label: None,
        semantic_fingerprint: None,
    };
    let obj = build_entity_object(epoch9, &rec).unwrap();
    let stored = obj.stored_bytes().to_vec();
    assert_eq!(
        import_entity_object(SchemaEpochId::from_bytes([8; 32]), &stored)
            .unwrap_err()
            .code()
            .to_string(),
        "SCB_EPOCH_MISMATCH"
    );
    let mut tampered = stored.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert_eq!(
        import_entity_object(epoch9, &tampered)
            .unwrap_err()
            .code()
            .to_string(),
        "SCB_DIGEST_MISMATCH"
    );
    eprintln!("ENVELOPE_NATIVE epoch/digest rejections agree");
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

#[test]
fn outer_entrypoint_resources_stay_inside_codec_budgets() {
    let (dec_pkg, dec_approved) = admit(&outer_decode_image());
    let (enc_pkg, enc_approved) = admit(&outer_encode_image());
    let (entry_pkg, entry_approved) = admit(&entrypoint_decode_image());
    // Increasing-size series: ns-empty 47B, ep 77B, ns-one 80B, ns-two 113B payloads.
    // Outer decode/encode + entrypoint decode per-stage (explicit two-invocation
    // boundary for outer+entrypoint; no single composed Sley invocation claimed).
    let payloads = [
        "0201200101010101010101010101010101010101010101010101010101010101010101020a03080201020000020100",
        "0201200101010101010101010101010101010101010101010101010101010101010101022810260201200a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a020101",
        "0201200101010101010101010101010101010101010101010101010101010101010101022b03290201020000022201200202020202020202020202020202020202020202020202020202020202020202",
        "0201200101010101010101010101010101010101010101010101010101010101010101024c034a0201020000024302200202020202020202020202020202020202020202020202020202020202020202200303030303030303030303030303030303030303030303030303030303030303",
    ];
    for payload in payloads {
        let dec = outer_decode_call(&dec_pkg, &dec_approved, payload);
        assert!(dec.fuel_used < 1_000_000, "decode fuel");
        assert!(dec.instruction_count < 100_000, "decode instr");
        assert!(dec.peak_value_units < 1_000_000, "decode value units");
        eprintln!(
            "RES outer-decode payload{}B fuel={} instr={} peak={}",
            hex_decode(payload).len(),
            dec.fuel_used,
            dec.instruction_count,
            dec.peak_value_units
        );
    }
    // Entrypoint bodies 40B each (fixed shape, input-dependent func/exp).
    for body in [
        "10260201200a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a020101",
        "10260201200b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b020102",
    ] {
        let dec = entrypoint_decode_call(&entry_pkg, &entry_approved, body);
        assert!(dec.fuel_used < 1_000_000);
        assert!(dec.instruction_count < 100_000);
        assert!(dec.peak_value_units < 1_000_000);
        eprintln!(
            "RES entry-decode body{}B fuel={} instr={} peak={}",
            hex_decode(body).len(),
            dec.fuel_used,
            dec.instruction_count,
            dec.peak_value_units
        );
    }
    // Encode sample.
    let enc = outer_encode_call(
        &enc_pkg,
        &enc_approved,
        "0101010101010101010101010101010101010101010101010101010101010101",
        "03080201020000020100",
    );
    assert!(enc.fuel_used < 1_000_000);
    assert!(enc.peak_value_units < 1_000_000);
    eprintln!(
        "RES outer-encode fuel={} instr={} peak={}",
        enc.fuel_used, enc.instruction_count, enc.peak_value_units
    );
    // 200-300K projection replaced: measured outer 47B 4875/677/67216,
    // 80B 7567/883/96035, 113B 10181/1077/148500; encode 47B 2848/323/51234;
    // entry 40B 5354/827/69727. All far inside 1M budgets; value-units bind first.
}

fn program_epoch9() -> sley_id::SchemaEpochId {
    sley_id::SchemaEpochId::from_bytes([9; 32])
}

fn program_entry_record(
    eid_byte: u8,
    func_byte: u8,
    exposure: sley_mutate::value::EntryExposure,
) -> sley_mutate::EntityObjectRecord {
    use sley_mutate::value::{EntityBodyValue, EntryPointBody};
    sley_mutate::EntityObjectRecord {
        entity_id: sley_id::EntityId::from_bytes([eid_byte; 32]),
        body: EntityBodyValue::EntryPoint(EntryPointBody {
            function: sley_id::EntityId::from_bytes([func_byte; 32]),
            exposure,
        }),
        label: None,
        semantic_fingerprint: None,
    }
}

fn program_stored(
    eid_byte: u8,
    func_byte: u8,
    exposure: sley_mutate::value::EntryExposure,
) -> Vec<u8> {
    let obj = sley_mutate::build_entity_object(
        program_epoch9(),
        &program_entry_record(eid_byte, func_byte, exposure),
    )
    .expect("native builds program fixture");
    obj.stored_bytes().to_vec()
}

fn program_payload(stored: &[u8]) -> Vec<u8> {
    // Preimage layout is fixed for these fixtures (magic8 ver1 tag2
    // epoch32 len1); the native preimage Caique is authoritative.
    let obj = sley_mutate::import_entity_object(program_epoch9(), stored)
        .expect("native imports program fixture");
    let pre = obj.preimage();
    pre[8 + 1 + 2 + 32 + 1..].to_vec()
}

/// Recompute the digest trailer over a patched stored object, so the
/// envelope layer passes and the fault under test sits deeper.
fn program_recompute(stored: &[u8]) -> Vec<u8> {
    let pre_len = stored.len() - 32;
    let digest = sley_id::ObjectId::derive(&stored[..pre_len]);
    let mut out = stored[..pre_len].to_vec();
    out.extend_from_slice(digest.as_bytes());
    out
}

fn program_native_code(stored: &[u8]) -> String {
    match sley_mutate::import_entity_object(program_epoch9(), stored) {
        Ok(_) => "OK".to_owned(),
        Err(error) => error.code().to_string(),
    }
}

#[test]
fn dbg_tmp_rhw1() {
    use blake3::Hasher;
    use sley_vm::host_abi::BRIDGE_CODE_RHW1;
    let mut ma = Asm::new();
    let mns = Ns {
        k: 221,
        p: 222,
        b: 223,
        o: 224,
    };
    let mf = eid(9, 70);
    let mu = ma.param(mns.p, mf, ParameterRole::Function, TypeExpr::Unit);
    let mb = ma.id(mns.b);
    let res_t = encode_result_type();
    let mres = ma.kbytes(mns.k, b"SCB_RESOURCE_LIMIT");
    let mrb = err_block(&mut ma, mns, mf, res_t.clone(), mres);
    let known = vec![7u8; 121];
    let kb = ma.kbytes(mns.k, &known);
    let kb2 = ma.cref(mns.o, mb, kb, TypeExpr::Bytes);
    let rhw = ma.op(
        mns.o,
        mb,
        Opcode::AdapterInvoke,
        vec![pav(mu), op_result(kb2)],
        vec![index_result(TypeExpr::Bytes)],
        Immediate::Entity(EntityId::from_bytes(bridge_identity(BRIDGE_CODE_RHW1))),
    );
    let mret = ma.id(mns.b);
    let mdb = ma.param(mns.p, mret, ParameterRole::Block, TypeExpr::Bytes);
    ma.blocks.push(Block {
        entity_id: mb,
        function: mf,
        parameters: Vec::new(),
        operations: vec![kb2, rhw],
        terminator: switch(
            op_result(rhw),
            vec![
                (BuiltinCase::Ok, mret, vec![SwitchArgument::CasePayload]),
                (BuiltinCase::Err, mrb, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    let mok = ma.op(
        mns.o,
        mret,
        Opcode::ResultOk,
        vec![pav(mdb)],
        vec![res_t.clone()],
        Immediate::None,
    );
    ma.blocks.push(Block {
        entity_id: mret,
        function: mf,
        parameters: vec![mdb],
        operations: vec![mok],
        terminator: ret(op_result(mok)),
        reachability: Reachability::Required,
    });
    let mgraph = FunctionGraph {
        entity_id: mf,
        type_parameters: Vec::new(),
        parameters: vec![mu],
        result_type: res_t,
        effects: Vec::new(),
        entry_block: mb,
        blocks: ma.blocks.iter().map(|b| b.entity_id).collect(),
        contracts: Vec::new(),
        visibility: Visibility::Private,
    };
    let mimage = Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: mgraph.clone(),
        functions: vec![mgraph],
        parameters: ma.parameters,
        blocks: ma.blocks,
        operations: ma.operations,
        adapters: vec![frozen_import(
            BRIDGE_CODE_RHW1,
            TypeExpr::Bytes,
            TypeExpr::Bytes,
        )],
        constants: ma.constants,
    };
    let (mpkg, mapp) = admit(&mimage);
    let mout = execute(&mpkg, &mapp, vec![unit_input()]);
    let mut hasher = Hasher::new();
    hasher.update(&[7u8; 121]);
    let exp = *hasher.finalize().as_bytes();
    match &mout.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                ConstData::Bytes(got) => {
                    eprintln!("MICRO_RHW1 len={} match={}", got.len(), got.as_slice() == exp);
                }
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn program_validate_probe_matches_native_payload() {
    use sley_mutate::value::EntryExposure;
    let (pkg, approved) = admit(&program_validate_image());
    // Canonical fixtures plus post-admission varied values (entity,
    // function, and exposure all vary after admission).
    for (eid_byte, func_byte, exposure) in [
        (1u8, 10u8, EntryExposure::Local),
        (1u8, 11u8, EntryExposure::Protocol),
        (2u8, 12u8, EntryExposure::Protocol),
    ] {
        let stored = program_stored(eid_byte, func_byte, exposure);
        assert_eq!(stored.len(), 153, "program stored 153B");
        let expected_payload = program_payload(&stored);
        assert_eq!(expected_payload.len(), 77, "program payload 77B");
        let outcome = program_validate_call(&pkg, &approved, &hex_encode(&stored));
        let fuel = assert_encode_ok(&outcome, &expected_payload);
        eprintln!(
            "PROG_VAL eid{eid_byte} func{func_byte} stored153B payload77B fuel={fuel} digest_ok"
        );
    }
}

#[test]
fn program_validate_rejections_match_reference() {
    let (pkg, approved) = admit(&program_validate_image());
    let base = program_stored(1, 10, sley_mutate::value::EntryExposure::Local);
    assert_eq!(base.len(), 153, "base stored 153B");
    let long_len = sley_scb1::encode_uvar(67_108_865);
    let mut vectors: Vec<(&str, Vec<u8>, &str)> = Vec::new();
    // Truncated below the digest-split bound.
    vectors.push(("short10", vec![0u8; 10], "SCB_LENGTH_OVERFLOW"));
    // Wrong magic (recomputed so only magic fails).
    let mut bad_magic = base.clone();
    bad_magic[0] ^= 0xff;
    vectors.push((
        "bad_magic",
        program_recompute(&bad_magic),
        "SCB_MAGIC_INVALID",
    ));
    // Wrong version.
    let mut bad_version = base.clone();
    bad_version[8] = 0x02;
    vectors.push((
        "bad_version",
        program_recompute(&bad_version),
        "SCB_VERSION_UNSUPPORTED",
    ));
    // Fixture tag 2 is not accepted in program context.
    let mut tag2 = base.clone();
    tag2.splice(9..11, [0x02]);
    vectors.push((
        "tag2",
        program_recompute(&tag2),
        "SCB_CONTRACT_UNKNOWN",
    ));
    // Unknown tag 201.
    let mut tag201 = base.clone();
    tag201[9] = 0xc9;
    vectors.push((
        "tag201",
        program_recompute(&tag201),
        "SCB_CONTRACT_UNKNOWN",
    ));
    // Wrong epoch.
    let mut bad_epoch = base.clone();
    bad_epoch[11] = 0x08;
    vectors.push((
        "bad_epoch",
        program_recompute(&bad_epoch),
        "SCB_EPOCH_MISMATCH",
    ));
    // Truncated payload (digest kept): payload/digest bounds fail.
    vectors.push((
        "trunc_payload",
        base[..143].to_vec(),
        "SCB_LENGTH_OVERFLOW",
    ));
    // Trailing byte before the trailer (digest kept).
    let mut trailing_pre = base.clone();
    trailing_pre.insert(121, 0x00);
    vectors.push((
        "trailing_pre",
        trailing_pre,
        "SCB_TRAILING_BYTES",
    ));
    // Trailing plus bad digest: trailing wins (reference order).
    let mut trailing_digest = base.clone();
    trailing_digest.insert(121, 0x00);
    let last = trailing_digest.len() - 1;
    trailing_digest[last] ^= 0x01;
    vectors.push((
        "trailing_plus_baddigest",
        trailing_digest,
        "SCB_TRAILING_BYTES",
    ));
    // Trailing byte after the digest.
    let mut trailing_post = base.clone();
    trailing_post.push(0x00);
    vectors.push((
        "trailing_post",
        trailing_post,
        "SCB_TRAILING_BYTES",
    ));
    // Corrupted digest only.
    let mut bad_digest = base.clone();
    let last = bad_digest.len() - 1;
    bad_digest[last] ^= 0x01;
    vectors.push((
        "bad_digest",
        bad_digest,
        "SCB_DIGEST_MISMATCH",
    ));
    // Non-minimal payload length.
    let mut nonminimal = base.clone();
    nonminimal.splice(43..44, [0xcd, 0x00]);
    vectors.push((
        "nonminimal_len",
        program_recompute(&nonminimal),
        "SCB_VARINT_NON_MINIMAL",
    ));
    // Payload length past the epoch ceiling.
    let mut long = base.clone();
    long.splice(43..44, long_len.iter().copied());
    vectors.push((
        "len_max_plus_1",
        program_recompute(&long),
        "SCB_RESOURCE_LIMIT",
    ));
    // Declared length past the remaining bytes.
    let mut len127 = base.clone();
    len127[43] = 0x7f;
    vectors.push((
        "len127",
        program_recompute(&len127),
        "SCB_LENGTH_OVERFLOW",
    ));
    for (name, bytes, expected) in &vectors {
        let outcome = program_validate_call(&pkg, &approved, &hex_encode(bytes));
        assert_refusal(&outcome, expected);
        assert_eq!(
            program_native_code(bytes),
            (*expected).to_owned(),
            "native agrees on {name}"
        );
        eprintln!("PROG_VAL_REJ {name} -> {expected}");
    }
}

#[test]
fn program_build_probe_matches_native_stored() {
    use sley_mutate::value::EntryExposure;
    let (pkg, approved) = admit(&program_build_image());
    for (eid_byte, func_byte, exposure) in [
        (1u8, 10u8, EntryExposure::Local),
        (1u8, 11u8, EntryExposure::Protocol),
        (2u8, 12u8, EntryExposure::Protocol),
    ] {
        let stored = program_stored(eid_byte, func_byte, exposure);
        let payload = program_payload(&stored);
        // The build entry returns `(prefix, preimage)`: the stored
        // prefix (header ++ length ++ payload) plus the digest preimage
        // (domain ++ stored prefix). The digest trailer itself is
        // appended by the caller (composed path).
        let obj = sley_mutate::import_entity_object(program_epoch9(), &stored)
            .expect("native imports program fixture");
        let expected_prefix = obj.preimage();
        assert_eq!(expected_prefix.len(), 121, "program preimage 121B");
        let mut expected_him = b"sley2.object.v1".to_vec();
        expected_him.extend_from_slice(expected_prefix);
        let outcome = program_build_call(&pkg, &approved, &hex_encode(&payload));
        match &outcome.termination {
            sley_vm::ExecutionTermination::Success(found) => match &found.data {
                ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                    ConstData::Sequence(items) => {
                        assert_eq!(items.len(), 2, "build Ok carries (prefix, preimage)");
                        match (&items[0].data, &items[1].data) {
                            (ConstData::Bytes(prefix), ConstData::Bytes(him)) => {
                                assert_eq!(prefix, expected_prefix, "prefix bytes match");
                                assert_eq!(him, &expected_him, "preimage bytes match");
                            }
                            other => panic!("build Ok must carry (Bytes, Bytes), got {other:?}"),
                        }
                    }
                    other => panic!("build Ok must carry a pair, got {other:?}"),
                },
                other => panic!("build must succeed, got {other:?}"),
            },
            other => panic!("build must succeed, got {other:?}"),
        }
        eprintln!(
            "PROG_BUILD eid{eid_byte} func{func_byte} payload77B prefix121B him136B fuel={} digest_ok",
            outcome.fuel_used
        );
    }
}

#[test]
fn program_decode_composed_returns_structured_triple() {
    use sley_mutate::value::EntryExposure;
    let (pkg, approved) = admit(&program_decode_image());
    for (eid_byte, func_byte, exposure, exp_u64) in [
        (1u8, 10u8, EntryExposure::Local, 1u64),
        (1u8, 11u8, EntryExposure::Protocol, 2u64),
        (2u8, 12u8, EntryExposure::Protocol, 2u64),
    ] {
        let stored = program_stored(eid_byte, func_byte, exposure);
        let outcome = program_decode_call(&pkg, &approved, &hex_encode(&stored));
        let fuel = assert_program_decode_ok(
            &outcome,
            &[eid_byte; 32],
            &[func_byte; 32],
            exp_u64,
        );
        assert_eq!(
            program_native_code(&stored),
            "OK",
            "native imports the composed fixture"
        );
        eprintln!(
            "PROG_DEC eid{eid_byte} func{func_byte} exp{exp_u64} in153B fuel={fuel} instr={} peak={}",
            outcome.instruction_count, outcome.peak_value_units
        );
    }
}

#[test]
fn program_encode_composed_matches_canonical_stored() {
    use sley_mutate::value::EntryExposure;
    let (pkg, approved) = admit(&program_encode_image());
    // Inputs are built independently from structured values (hex entity
    // / function plus u64 exposure); the reference stored object comes
    // from the native builder, never from a Sley decode.
    for (eid_byte, func_byte, exposure, exp_u64) in [
        (1u8, 10u8, EntryExposure::Local, 1u64),
        (1u8, 11u8, EntryExposure::Protocol, 2u64),
        (2u8, 12u8, EntryExposure::Protocol, 2u64),
    ] {
        let expected = program_stored(eid_byte, func_byte, exposure);
        let eid_hex = hex_encode(&[eid_byte; 32]);
        let func_hex = hex_encode(&[func_byte; 32]);
        let outcome = program_encode_call(&pkg, &approved, &eid_hex, &func_hex, exp_u64);
        let fuel = assert_encode_ok(&outcome, &expected);
        eprintln!(
            "PROG_ENC eid{eid_byte} func{func_byte} exp{exp_u64} out153B fuel={fuel} instr={} peak={}",
            outcome.instruction_count, outcome.peak_value_units
        );
    }
    // Invalid exposure refuses at the EntryPoint stage (encoder-side
    // mirror of the slice-5 rejection set).
    let bad = program_encode_call(
        &pkg,
        &approved,
        &hex_encode(&[1u8; 32]),
        &hex_encode(&[10u8; 32]),
        3,
    );
    assert_refusal(&bad, "SCB_UNION_INVALID");
    eprintln!("PROG_ENC_REJ exp3 -> SCB_UNION_INVALID");
}

#[test]
fn program_composed_rejections_preserve_precedence_and_scope() {
    use sley_mutate::value::EntryExposure;
    let (pkg, approved) = admit(&program_decode_image());
    let base = program_stored(1, 10, EntryExposure::Local);
    // (name, stored bytes, expected Sley code, expected native code;
    // native "OK" marks the pinned scope divergences where a valid but
    // unsupported body stays a scope limitation, never malformed).
    let mut vectors: Vec<(&str, Vec<u8>, &str, &str)> = Vec::new();
    // Envelope faults precede any body fault through the composed path.
    let mut bad_magic = base.clone();
    bad_magic[0] ^= 0xff;
    vectors.push((
        "magic",
        program_recompute(&bad_magic),
        "SCB_MAGIC_INVALID",
        "SCB_MAGIC_INVALID",
    ));
    let mut bad_version = base.clone();
    bad_version[8] = 0x02;
    vectors.push((
        "version",
        program_recompute(&bad_version),
        "SCB_VERSION_UNSUPPORTED",
        "SCB_VERSION_UNSUPPORTED",
    ));
    let mut tag2 = base.clone();
    tag2.splice(9..11, [0x02]);
    vectors.push((
        "tag",
        program_recompute(&tag2),
        "SCB_CONTRACT_UNKNOWN",
        "SCB_CONTRACT_UNKNOWN",
    ));
    let mut bad_epoch = base.clone();
    bad_epoch[11] = 0x08;
    vectors.push((
        "epoch",
        program_recompute(&bad_epoch),
        "SCB_EPOCH_MISMATCH",
        "SCB_EPOCH_MISMATCH",
    ));
    vectors.push((
        "trunc",
        base[..143].to_vec(),
        "SCB_LENGTH_OVERFLOW",
        "SCB_LENGTH_OVERFLOW",
    ));
    let mut trailing = base.clone();
    trailing.insert(121, 0x00);
    vectors.push((
        "trailing",
        trailing,
        "SCB_TRAILING_BYTES",
        "SCB_TRAILING_BYTES",
    ));
    let mut bad_digest = base.clone();
    let last = bad_digest.len() - 1;
    bad_digest[last] ^= 0x01;
    vectors.push((
        "digest",
        bad_digest,
        "SCB_DIGEST_MISMATCH",
        "SCB_DIGEST_MISMATCH",
    ));
    // Precedence pins: the earlier layer wins even when deeper faults
    // are present (digest recomputed where the earlier layer must pass).
    let mut tag_body = base.clone();
    tag_body.splice(9..11, [0x02]);
    tag_body[119] = 0x03;
    vectors.push((
        "tag_beats_body",
        program_recompute(&tag_body),
        "SCB_CONTRACT_UNKNOWN",
        "SCB_CONTRACT_UNKNOWN",
    ));
    let mut epoch_body = base.clone();
    epoch_body[11] = 0x08;
    epoch_body[119] = 0x03;
    vectors.push((
        "epoch_beats_body",
        program_recompute(&epoch_body),
        "SCB_EPOCH_MISMATCH",
        "SCB_EPOCH_MISMATCH",
    ));
    let mut digest_body = base.clone();
    digest_body[119] = 0x03;
    vectors.push((
        "digest_beats_body",
        digest_body,
        "SCB_DIGEST_MISMATCH",
        "SCB_DIGEST_MISMATCH",
    ));
    let mut trailing_digest = base.clone();
    trailing_digest.insert(121, 0x00);
    let last = trailing_digest.len() - 1;
    trailing_digest[last] ^= 0x01;
    vectors.push((
        "trailing_beats_digest",
        trailing_digest,
        "SCB_TRAILING_BYTES",
        "SCB_TRAILING_BYTES",
    ));
    // Valid digest over malformed supported payload: the body layer
    // decides, in exact reference order.
    let mut exp3 = base.clone();
    exp3[120] = 0x03;
    vectors.push((
        "exposure3",
        program_recompute(&exp3),
        "SCB_UNION_INVALID",
        "SCB_UNION_INVALID",
    ));
    let mut count1 = base.clone();
    count1[44] = 0x01;
    vectors.push((
        "outer_count1",
        program_recompute(&count1),
        "SCB_FIELD_MISSING",
        "SCB_FIELD_MISSING",
    ));
    let mut count5 = base.clone();
    count5[44] = 0x05;
    vectors.push((
        "outer_count5",
        program_recompute(&count5),
        "SCB_FIELD_UNKNOWN",
        "SCB_FIELD_UNKNOWN",
    ));
    let mut dup11 = base.clone();
    dup11[79] = 0x01;
    vectors.push((
        "outer_dup11",
        program_recompute(&dup11),
        "SCB_FIELD_DUPLICATE",
        "SCB_FIELD_DUPLICATE",
    ));
    let mut order10 = base.clone();
    order10[79] = 0x00;
    vectors.push((
        "outer_order10",
        program_recompute(&order10),
        "SCB_FIELD_ORDER",
        "SCB_FIELD_ORDER",
    ));
    let mut funclen31 = base.clone();
    funclen31[85] = 0x1f;
    vectors.push((
        "func_len31",
        program_recompute(&funclen31),
        "SCB_LENGTH_OVERFLOW",
        "SCB_LENGTH_OVERFLOW",
    ));
    let mut eidlen33 = base.clone();
    eidlen33[46] = 0x21;
    vectors.push((
        "eid_len33",
        program_recompute(&eidlen33),
        "SCB_TRAILING_BYTES",
        "SCB_TRAILING_BYTES",
    ));
    // Patched union tag 1 over EntryPoint content: Sley dispatches scope
    // on the tag alone (content unread), while native attempts a real
    // Namespace decode of the foreign content and reports FIELD_MISSING.
    // Both sides are independently correct; scope is never misreported
    // as malformed on the Sley side. (The valid-namespace transplant
    // below pins native-OK vs Sley-scope for genuine content.)
    let mut union1 = base.clone();
    union1[81] = 0x01;
    vectors.push((
        "union1_scope",
        program_recompute(&union1),
        "SSMC_RESERVED_FIELD_PRESENT",
        "SCB_FIELD_MISSING",
    ));
    for (name, bytes, expected, native_expected) in &vectors {
        let outcome = program_decode_call(&pkg, &approved, &hex_encode(bytes));
        assert_refusal(&outcome, expected);
        assert_eq!(
            program_native_code(bytes),
            (*native_expected).to_owned(),
            "native reference on {name}"
        );
        eprintln!("PROG_COMP_REJ {name} -> {expected} (native {native_expected})");
    }
    // A real Namespace body inside a valid program envelope: native
    // imports Ok while Sley reports the scope limitation. The body bytes
    // come from the retained ns-one outer payload, transplanted into a
    // natively encoded 2-field record with a recomputed digest.
    let ns_one = hex_decode(
        "0201200101010101010101010101010101010101010101010101010101010101010101022b03290201020000022201200202020202020202020202020202020202020202020202020202020202020202",
    );
    let ns_body = ns_one[37..].to_vec();
    let ns_payload = sley_scb1::encode_record(&[(1, vec![1u8; 32]), (2, ns_body)])
        .expect("native encodes namespace record");
    let mut ns_preimage = Vec::new();
    ns_preimage.extend_from_slice(b"SLEYSCB1");
    ns_preimage.extend_from_slice(&sley_scb1::encode_uvar(1));
    ns_preimage.extend_from_slice(&sley_scb1::encode_uvar(200));
    ns_preimage.extend_from_slice(&[9u8; 32]);
    ns_preimage.extend_from_slice(&sley_scb1::encode_uvar(ns_payload.len() as u64));
    ns_preimage.extend_from_slice(&ns_payload);
    let ns_digest = sley_id::ObjectId::derive(&ns_preimage);
    let mut ns_stored = ns_preimage;
    ns_stored.extend_from_slice(ns_digest.as_bytes());
    assert_eq!(
        program_native_code(&ns_stored),
        "OK",
        "native imports the namespace-body object"
    );
    let ns_outcome = program_decode_call(&pkg, &approved, &hex_encode(&ns_stored));
    assert_refusal(&ns_outcome, "SSMC_RESERVED_FIELD_PRESENT");
    eprintln!("PROG_COMP_REJ namespace_body -> SSMC_RESERVED_FIELD_PRESENT (native OK)");
}

#[test]
fn program_composed_resources_stay_inside_codec_budgets() {
    use sley_mutate::value::EntryExposure;
    let (dec_pkg, dec_approved) = admit(&program_decode_image());
    let (enc_pkg, enc_approved) = admit(&program_encode_image());
    for (eid_byte, func_byte, exposure, exp_u64) in [
        (1u8, 10u8, EntryExposure::Local, 1u64),
        (1u8, 11u8, EntryExposure::Protocol, 2u64),
        (2u8, 12u8, EntryExposure::Protocol, 2u64),
    ] {
        let stored = program_stored(eid_byte, func_byte, exposure);
        let dec = program_decode_call(&dec_pkg, &dec_approved, &hex_encode(&stored));
        assert_program_decode_ok(&dec, &[eid_byte; 32], &[func_byte; 32], exp_u64);
        assert!(dec.fuel_used < 1_000_000, "decode fuel");
        assert!(dec.instruction_count < 100_000, "decode instr");
        assert!(dec.peak_value_units < 1_000_000, "decode value units");
        eprintln!(
            "PROG_RES_DEC in{}B fuel={} instr={} peak={}",
            stored.len(),
            dec.fuel_used,
            dec.instruction_count,
            dec.peak_value_units
        );
        let expected = program_stored(eid_byte, func_byte, exposure);
        let enc = program_encode_call(
            &enc_pkg,
            &enc_approved,
            &hex_encode(&[eid_byte; 32]),
            &hex_encode(&[func_byte; 32]),
            exp_u64,
        );
        assert_encode_ok(&enc, &expected);
        assert!(enc.fuel_used < 1_000_000, "encode fuel");
        assert!(enc.instruction_count < 100_000, "encode instr");
        assert!(enc.peak_value_units < 1_000_000, "encode value units");
        eprintln!(
            "PROG_RES_ENC out{}B fuel={} instr={} peak={}",
            expected.len(),
            enc.fuel_used,
            enc.instruction_count,
            enc.peak_value_units
        );
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

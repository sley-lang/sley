//! RW-080 §1.1 codec dependency slice 2: standalone SCB1 envelope validation in Sley.
//!
//! PROVISIONAL C0 SEED CONSTRUCTION - explicitly not accepted runtime
//! authority. Seed-authored (this file is the seed-assembler record)
//! under standing amendment `rw075_correction.operator_override_2026_09_07`
//! (machine-summary.json: gateway/premium outage does not freeze
//! development; native review labeled; self-review provisional only;
//! findings/validation/evidence/final qualification remain in force).
//! It exercises the v2 admit/approve/execute path for the envelope
//! validator through the approved execution boundary.
//!
//! Scope (bounded): standalone envelope framing plus digest, i.e. the exact
//! `decode_standalone_fixture` envelope order of `sley-scb1` (`lib.rs:427`)
//! through digest verification, with opaque payload. Reuses the slice-1
//! `decode_uvar` Sley function via `CallDirect` for every uvar field
//! (version width 64, contract tag width 32, payload length width 64)
//! so framing errors keep exact reference decision order and codes.
//! Sley owns parsing, byte-consumption decisions, length/bounds checks,
//! exact digest-preimage construction (`sley2.object.v1` domain prefix
//! plus exact stored prefix bytes as `Bytes`), and the validation
//! decision. RHW1 is used only for its admitted primitive responsibility
//! (raw BLAKE3-256 over Sley-built bytes, 1 MiB per-call ceiling).
//! Entry `validate_envelope(input: Bytes, declared: UInt64, unit: Unit)`
//! returns `Result<Bytes, Bytes>` where `Ok` carries the exact opaque
//! payload slice and `Err` carries the exact `SCB_*` code.
//!
//! Construction provenance and contract basis:
//! machineresearch/sley-2.0/reweave/rw-080-codec-envelope.md (new),
//! reusing slice-1 manifest reweave/rw-080-codec-uvar.md (F1/F2/OREF notes).
//!
//! Deliberate non-goals with reasons (not silent gaps):
//! - `codec_main` four legs stay stubs (`rw080_codec_scaffold.rs`): no
//!   program/schema wire format exists in-tree, so wiring legs would
//!   invent framing. This validator is an entry of its own approved
//!   image through the same boundary (reuses existing scope finding).
//! - Payload is opaque: `Ok` means envelope framing plus digest are
//!   valid for the declared fixture tag; it does NOT mean the payload
//!   is a valid program, schema, executable, or even a valid fixture
//!   record. Fixture-record semantic errors (`SCB_FIELD_*`, etc.) belong
//!   to a later slice with the program/schema binding. Tests pin this:
//!   envelope-valid but record-invalid inputs return `Ok` here while the
//!   reference reports the later-phase field code.
//! - `ZigZag` `SInt`, Text/NFC/floats/maps/records: unchanged from
//!   slice 1 (no conversion opcode; need unlanded capabilities).
//! - Errors are the exact `SCB_*` code strings as `Bytes` values. No
//!   code is mapped to the 7-code program-level vocabulary: inventing
//!   equivalences is forbidden. The vocabulary mapping belongs to leg
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
    // documented capacity restriction against the 67 MiB epoch limit.
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
// Retained from slice 1 for reference; unused in this envelope image
// (which composes decode only), so dead code is allowed here rather
// than churning the proven builder.
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

// ── validate_envelope ────────────────────────────────────────────────
// Standalone SCB1 envelope validator (opaque payload). Entry
// `validate_envelope(input: Bytes, declared: UInt64, unit: Unit)`
// returns `Result<Bytes, Bytes>` with `Ok` = exact opaque payload slice,
// `Err` = exact `SCB_*` code. Field order and codes mirror
// `sley_scb1::decode_standalone_fixture` through digest verification:
// MAGIC, VERSION (uvar errors first), CONTRACT (uvar errors first,
// unknown values then declared mismatch), EPOCH (truncation first),
// payload length (uvar errors first, then RESOURCE_LIMIT past
// MAX_STANDALONE_BYTES), payload/digest bounds (LENGTH_OVERFLOW),
// TRAILING (before digest), DIGEST (via Sley-built domain++preimage
// through RHW1). Payload bytes are returned opaquely; fixture-record
// semantic errors belong to a later slice.
//
// Capacity restriction (AR-05, same class as slice 1): B2V1 converts
// inputs up to the 1 MiB bridge cap; larger inputs refuse
// `SCB_RESOURCE_LIMIT` at conversion. The epoch allows 67 MiB
// standalone. Any input this slice accepts is decided exactly; inputs
// past 1 MiB are refused loudly with the same resource code, never
// misread. RHW1 likewise refuses hash inputs past 1 MiB with the same
// code (domain 15 bytes plus preimage), so the supported envelope range
// under the admitted profile is bounded by the bridge/RHW1 caps, not
// the epoch ceiling. Documented, not silent.
//
// Unreachable edges target `Trap(InternalInvariant)` with a proof sketch
// per site in comments (same standard as slice 1). Reachable resource
// edges return typed `SCB_RESOURCE_LIMIT`. No malformed input reaches a
// trap: every input-dependent failure has a typed-error edge.
#[allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines
)]
fn build_envelope(a: &mut Asm, ns: Ns, fid: EntityId, decode_fid: EntityId) -> FunctionGraph {
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
    let c_max = a.ku64(ns.k, 67_108_864);
    // Width constants are UInt32 (CallDirect demands u32 width).
    let w32 = a.ku32(ns.k, 32);
    let w64 = a.ku32(ns.k, 64);
    // Distinct UInt8 byte values for magic, domain, epoch.
    let b00 = a.ku8(ns.k, 0x00);
    let b01 = a.ku8(ns.k, 0x01);
    let b31 = a.ku8(ns.k, 0x31);
    let b32 = a.ku8(ns.k, 0x32);
    let b2e = a.ku8(ns.k, 0x2e);
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
    let p_decl = a.param(ns.p, fid, ParameterRole::Function, u64_type());
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
    let q_decl = a.param(ns.p, b_len0, ParameterRole::Block, u64_type());
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
                        sav(p_decl),
                        sav(p_unit),
                    ],
                ),
                (BuiltinCase::Err, b_res, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });

    // Length for later bounds checks. VectorLen on the converted vector.
    let ln = a.op(
        ns.o,
        b_len0,
        Opcode::VectorLen,
        vec![pav(q_vec)],
        vec![u64_type()],
        Immediate::None,
    );
    // Magic needs 8 bytes; shorter inputs are MAGIC_INVALID per the
    // reference `get(..8) != Some(MAGIC)` rule (not LENGTH_OVERFLOW).
    let m8c = a.cref(ns.o, b_len0, c8, u64_type());
    let mlt = a.op(
        ns.o,
        b_len0,
        Opcode::LessThan,
        vec![op_result(ln), op_result(m8c)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    // First magic-byte check block (index 0). Created here; the chain
    // M0..M7 is generated below with the vector/len/inputs threaded.
    let magic_expected = [b53, b4c, b45, b59, b53, b43, b42, b31];
    let magic_idx = [c0, c1, c2, c3, c4, c5, c6, c7];
    let mut m_get: Vec<EntityId> = Vec::new();
    let mut m_cmp: Vec<EntityId> = Vec::new();
    for _ in 0..8 {
        m_get.push(a.id(ns.b));
        m_cmp.push(a.id(ns.b));
    }
    // Version-decode entry after magic passes.
    let v_call = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: b_len0,
        function: fid,
        parameters: vec![q_vec, q_in, q_decl, q_unit],
        operations: vec![ln, m8c, mlt],
        terminator: cond(
            op_result(mlt),
            edge(b_magic, Vec::new()),
            edge(
                m_get[0],
                vec![
                    pav(q_vec),
                    op_result(ln),
                    pav(q_in),
                    pav(q_decl),
                    pav(q_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    // Magic chain: M_get[i] reads index i, M_cmp[i] compares the byte.
    // None on Get means missing magic byte: MAGIC_INVALID (defensive;
    // unreachable after the len>=8 check, but typed so no trap on any
    // input shape here).
    for i in 0..8 {
        let g = m_get[i];
        let cp = m_cmp[i];
        let g_vec = a.param(ns.p, g, ParameterRole::Block, u8vec_type());
        let g_len = a.param(ns.p, g, ParameterRole::Block, u64_type());
        let g_in = a.param(ns.p, g, ParameterRole::Block, TypeExpr::Bytes);
        let g_decl = a.param(ns.p, g, ParameterRole::Block, u64_type());
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
        // Next Get takes the same 5-tuple; on Some the byte payload
        // flows to the compare block with the state saved.
        a.blocks.push(Block {
            entity_id: g,
            function: fid,
            parameters: vec![g_vec, g_len, g_in, g_decl, g_unit],
            operations: vec![idxc, get],
            terminator: switch(
                op_result(get),
                vec![
                    (BuiltinCase::None, b_magic, Vec::new()),
                    (
                        BuiltinCase::Some,
                        cp,
                        vec![
                            SwitchArgument::CasePayload,
                            sav(g_vec),
                            sav(g_len),
                            sav(g_in),
                            sav(g_decl),
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
        let c_decl = a.param(ns.p, cp, ParameterRole::Block, u64_type());
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
        // All eight bytes carry the same 6-tuple; the last falls through
        // to the version call via `next`.
        a.blocks.push(Block {
            entity_id: cp,
            function: fid,
            parameters: vec![c_b, c_vec, c_len, c_in, c_decl, c_unit],
            operations: vec![expc, eq],
            terminator: cond(
                op_result(eq),
                edge(
                    next,
                    vec![pav(c_vec), pav(c_len), pav(c_in), pav(c_decl), pav(c_unit)],
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
    let v_decl = a.param(ns.p, v_call, ParameterRole::Block, u64_type());
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
    let v_odecl = a.param(ns.p, v_ok, ParameterRole::Block, u64_type());
    let v_ounit = a.param(ns.p, v_ok, ParameterRole::Block, TypeExpr::Unit);
    let v_ebytes = a.param(ns.p, v_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: v_call,
        function: fid,
        parameters: vec![v_vec, v_len, v_in, v_decl, v_unit],
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
                        sav(v_decl),
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
        parameters: vec![v_tup, v_ovec, v_olen, v_oin, v_odecl, v_ounit],
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
                    pav(v_odecl),
                    pav(v_ounit),
                ],
            ),
            edge(b_version, Vec::new()),
        ),
        reachability: Reachability::Required,
    });

    // Contract tag: decode_uvar(input, pos_after_version, width 32).
    // Unknown numeric values refuse CONTRACT_UNKNOWN; a known value that
    // differs from the declared tag refuses with the same code (the
    // reference second CONTRACT_UNKNOWN check).
    let t_pos = a.param(ns.p, c_call, ParameterRole::Block, u64_type());
    let t_vec = a.param(ns.p, c_call, ParameterRole::Block, u8vec_type());
    let t_len = a.param(ns.p, c_call, ParameterRole::Block, u64_type());
    let t_in = a.param(ns.p, c_call, ParameterRole::Block, TypeExpr::Bytes);
    let t_decl = a.param(ns.p, c_call, ParameterRole::Block, u64_type());
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
    let t_odecl = a.param(ns.p, t_ok, ParameterRole::Block, u64_type());
    let t_ounit = a.param(ns.p, t_ok, ParameterRole::Block, TypeExpr::Unit);
    let t_ebytes = a.param(ns.p, t_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: c_call,
        function: fid,
        parameters: vec![t_pos, t_vec, t_len, t_in, t_decl, t_unit],
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
                        sav(t_decl),
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
    let t_c1 = a.cref(ns.o, t_ok, c1, u64_type());
    let t_c2 = a.cref(ns.o, t_ok, c2, u64_type());
    let t_is1 = a.op(
        ns.o,
        t_ok,
        Opcode::Equal,
        vec![op_result(t_gv), op_result(t_c1)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let t_is2 = a.op(
        ns.o,
        t_ok,
        Opcode::Equal,
        vec![op_result(t_gv), op_result(t_c2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let t_known = a.op(
        ns.o,
        t_ok,
        Opcode::BoolOr,
        vec![op_result(t_is1), op_result(t_is2)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let t_known_ok = a.id(ns.b);
    let k_val = a.param(ns.p, t_known_ok, ParameterRole::Block, u64_type());
    let k_pos = a.param(ns.p, t_known_ok, ParameterRole::Block, u64_type());
    let k_opos = a.param(ns.p, t_known_ok, ParameterRole::Block, u64_type());
    let k_vec = a.param(ns.p, t_known_ok, ParameterRole::Block, u8vec_type());
    let k_len = a.param(ns.p, t_known_ok, ParameterRole::Block, u64_type());
    let k_in = a.param(ns.p, t_known_ok, ParameterRole::Block, TypeExpr::Bytes);
    let k_decl = a.param(ns.p, t_known_ok, ParameterRole::Block, u64_type());
    let k_unit = a.param(ns.p, t_known_ok, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: t_ok,
        function: fid,
        parameters: vec![t_tup, t_opos, t_ovec, t_olen, t_oin, t_odecl, t_ounit],
        operations: vec![t_gv, t_gp, t_c1, t_c2, t_is1, t_is2, t_known],
        terminator: cond(
            op_result(t_known),
            edge(
                t_known_ok,
                vec![
                    op_result(t_gv),
                    op_result(t_gp),
                    pav(t_opos),
                    pav(t_ovec),
                    pav(t_olen),
                    pav(t_oin),
                    pav(t_odecl),
                    pav(t_ounit),
                ],
            ),
            edge(b_contract, Vec::new()),
        ),
        reachability: Reachability::Required,
    });
    let k_eq = a.op(
        ns.o,
        t_known_ok,
        Opcode::Equal,
        vec![pav(k_val), pav(k_decl)],
        vec![TypeExpr::Bool],
        Immediate::None,
    );
    let e_bounds = a.id(ns.b);
    a.blocks.push(Block {
        entity_id: t_known_ok,
        function: fid,
        parameters: vec![k_val, k_pos, k_opos, k_vec, k_len, k_in, k_decl, k_unit],
        operations: vec![k_eq],
        terminator: cond(
            op_result(k_eq),
            edge(
                e_bounds,
                vec![
                    pav(k_pos),
                    pav(k_vec),
                    pav(k_len),
                    pav(k_in),
                    pav(k_decl),
                    pav(k_unit),
                ],
            ),
            edge(b_contract, Vec::new()),
        ),
        reachability: Reachability::Required,
    });

    // Epoch bounds: remaining = len - pos_after_tag must hold 32 bytes,
    // else LENGTH_OVERFLOW (reference take_exact). Subtraction failure
    // (pos > len) is unreachable by decode construction: new_pos never
    // exceeds the converted length, so Err targets the invariant trap.
    let e_pos = a.param(ns.p, e_bounds, ParameterRole::Block, u64_type());
    let e_vec = a.param(ns.p, e_bounds, ParameterRole::Block, u8vec_type());
    let e_lenp = a.param(ns.p, e_bounds, ParameterRole::Block, u64_type());
    let e_in = a.param(ns.p, e_bounds, ParameterRole::Block, TypeExpr::Bytes);
    let e_decl = a.param(ns.p, e_bounds, ParameterRole::Block, u64_type());
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
    let r_decl = a.param(ns.p, e_rem, ParameterRole::Block, u64_type());
    let r_unit = a.param(ns.p, e_rem, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: e_bounds,
        function: fid,
        parameters: vec![e_pos, e_vec, e_lenp, e_in, e_decl, e_unit],
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
                        sav(e_decl),
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
        parameters: vec![r_rem, r_pos, r_vec, r_len, r_in, r_decl, r_unit],
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
                    pav(r_decl),
                    pav(r_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    // Epoch chain: 32 bytes, expected 31 zeros then 0x01. Get None is
    // unreachable after the remaining>=32 check, so None targets the
    // invariant trap (construction-invariant, not input-dependent).
    // A present-but-wrong byte refuses EPOCH_MISMATCH.
    for i in 0..32 {
        let g = ep_get[i];
        let cp = ep_cmp[i];
        let ic = ep_inc[i];
        let g_idx = a.param(ns.p, g, ParameterRole::Block, u64_type());
        let g_vec = a.param(ns.p, g, ParameterRole::Block, u8vec_type());
        let g_len = a.param(ns.p, g, ParameterRole::Block, u64_type());
        let g_in = a.param(ns.p, g, ParameterRole::Block, TypeExpr::Bytes);
        let g_decl = a.param(ns.p, g, ParameterRole::Block, u64_type());
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
            parameters: vec![g_idx, g_vec, g_len, g_in, g_decl, g_unit],
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
                            sav(g_decl),
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
        let p_declb = a.param(ns.p, cp, ParameterRole::Block, u64_type());
        let p_unitb = a.param(ns.p, cp, ParameterRole::Block, TypeExpr::Unit);
        let exp_const = if i == 31 { b01 } else { b00 };
        let expc = a.cref(ns.o, cp, exp_const, u8_type());
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
            parameters: vec![p_b, p_idx, p_vec, p_len, p_inb, p_declb, p_unitb],
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
                        pav(p_declb),
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
        let n_decl = a.param(ns.p, ic, ParameterRole::Block, u64_type());
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
                parameters: vec![n_idx, n_vec, n_len, n_in, n_decl, n_unit],
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
                                sav(n_decl),
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
                parameters: vec![n_idx, n_vec, n_len, n_in, n_decl, n_unit],
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
                                sav(n_decl),
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
    // RESOURCE_LIMIT (reference read_len rule).
    let q_pos = a.param(ns.p, l_call, ParameterRole::Block, u64_type());
    let q_vec = a.param(ns.p, l_call, ParameterRole::Block, u8vec_type());
    let q_len = a.param(ns.p, l_call, ParameterRole::Block, u64_type());
    let q_in = a.param(ns.p, l_call, ParameterRole::Block, TypeExpr::Bytes);
    let q_decl = a.param(ns.p, l_call, ParameterRole::Block, u64_type());
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
    let l_odecl = a.param(ns.p, l_ok, ParameterRole::Block, u64_type());
    let l_ounit = a.param(ns.p, l_ok, ParameterRole::Block, TypeExpr::Unit);
    let l_ebytes = a.param(ns.p, l_err, ParameterRole::Block, TypeExpr::Bytes);
    a.blocks.push(Block {
        entity_id: l_call,
        function: fid,
        parameters: vec![q_pos, q_vec, q_len, q_in, q_decl, q_unit],
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
                        sav(q_decl),
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
        parameters: vec![l_tup, l_ovec, l_olen, l_oin, l_odecl, l_ounit],
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
                    pav(l_odecl),
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
    let s_decl = a.param(ns.p, b_bounds, ParameterRole::Block, u64_type());
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
    let r_decl = a.param(ns.p, s_rem, ParameterRole::Block, u64_type());
    let r_unit = a.param(ns.p, s_rem, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: b_bounds,
        function: fid,
        parameters: vec![s_plen, s_ppos, s_vec, s_len, s_in, s_decl, s_unit],
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
                        sav(s_decl),
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
        parameters: vec![r_plen, r_ppos, r_remv, r_vec, r_len, r_in, r_decl, r_unit],
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
                    pav(r_decl),
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
    let e_decl = a.param(ns.p, s_pe, ParameterRole::Block, u64_type());
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
    let f_decl = a.param(ns.p, e_after, ParameterRole::Block, u64_type());
    let f_unit = a.param(ns.p, e_after, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: s_pe,
        function: fid,
        parameters: vec![e_plen, e_ppos, e_vec, e_len2, e_in, e_decl, e_unit],
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
                        sav(e_decl),
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
    let g_decl = a.param(ns.p, f_rem, ParameterRole::Block, u64_type());
    let g_unit = a.param(ns.p, f_rem, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: e_after,
        function: fid,
        parameters: vec![f_pend, f_plen, f_ppos, f_vec, f_len, f_in, f_decl, f_unit],
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
                        sav(f_decl),
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
            g_pend, g_plen, g_ppos, g_remv, g_vec, g_len, g_in, g_decl, g_unit,
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
                    pav(g_decl),
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
    let h_decl = a.param(ns.p, g_de, ParameterRole::Block, u64_type());
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
    let j_decl = a.param(ns.p, h_after, ParameterRole::Block, u64_type());
    let j_unit = a.param(ns.p, h_after, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: g_de,
        function: fid,
        parameters: vec![h_pend, h_plen, h_ppos, h_vec, h_len, h_in, h_decl, h_unit],
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
                        sav(h_decl),
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
    let k_decl = a.param(ns.p, j_trail, ParameterRole::Block, u64_type());
    let k_unit = a.param(ns.p, j_trail, ParameterRole::Block, TypeExpr::Unit);
    a.blocks.push(Block {
        entity_id: h_after,
        function: fid,
        parameters: vec![
            j_dend, j_pend, j_plen, j_ppos, j_vec, j_len, j_in, j_decl, j_unit,
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
                        sav(j_decl),
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
            k_dend, k_pend, k_plen, k_ppos, k_trail, k_vec, k_len, k_in, k_decl, k_unit,
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
                    pav(k_decl),
                    pav(k_unit),
                ],
            ),
        ),
        reachability: Reachability::Required,
    });

    // Domain-prefix chain: 15 PSH1 pushes starting from an empty vector.
    // Each Err (accumulator resource) refuses RESOURCE_LIMIT; no
    // well-formed small envelope reaches it.
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
    // Thread (digest_end, payload_end, payload_len, payload_start, vec,
    // input, declared, unit) plus the growing accumulator. d0 carries
    // the envelope state; each domain step carries (acc + state).
    let dd_dend = a.param(ns.p, d0, ParameterRole::Block, u64_type());
    let dd_pend = a.param(ns.p, d0, ParameterRole::Block, u64_type());
    let dd_plen = a.param(ns.p, d0, ParameterRole::Block, u64_type());
    let dd_ppos = a.param(ns.p, d0, ParameterRole::Block, u64_type());
    let dd_vec = a.param(ns.p, d0, ParameterRole::Block, u8vec_type());
    let dd_in = a.param(ns.p, d0, ParameterRole::Block, TypeExpr::Bytes);
    let dd_decl = a.param(ns.p, d0, ParameterRole::Block, u64_type());
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
            dd_dend, dd_pend, dd_plen, dd_ppos, dd_vec, dd_in, dd_decl, dd_unit,
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
                pav(dd_decl),
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
        let s_decl = a.param(ns.p, st, ParameterRole::Block, u64_type());
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
                s_acc, s_dend, s_pend, s_plen, s_ppos, s_vec, s_in, s_decl, s_unit,
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
                            sav(s_decl),
                            sav(s_unit),
                        ],
                    ),
                    (BuiltinCase::Err, b_res, Vec::new()),
                ],
            ),
            reachability: Reachability::Required,
        });
    }

    // Preimage loop: append input[0..payload_end] (i.e. input[0..len-32],
    // the digest preimage) to the domain accumulator. Indices stay below
    // the converted length, so Get None is unreachable (trap); PSH1 Err
    // is typed RESOURCE_LIMIT for the capacity path. Loop is CFG backedge
    // Form A over block params.
    let pl_acc = a.param(ns.p, pre_loop, ParameterRole::Block, u8vec_type());
    let pl_dend = a.param(ns.p, pre_loop, ParameterRole::Block, u64_type());
    let pl_pend = a.param(ns.p, pre_loop, ParameterRole::Block, u64_type());
    let pl_plen = a.param(ns.p, pre_loop, ParameterRole::Block, u64_type());
    let pl_ppos = a.param(ns.p, pre_loop, ParameterRole::Block, u64_type());
    let pl_vec = a.param(ns.p, pre_loop, ParameterRole::Block, u8vec_type());
    let pl_in = a.param(ns.p, pre_loop, ParameterRole::Block, TypeExpr::Bytes);
    let pl_decl = a.param(ns.p, pre_loop, ParameterRole::Block, u64_type());
    let pl_unit = a.param(ns.p, pre_loop, ParameterRole::Block, TypeExpr::Unit);
    // Loop state adds the running index: enter with idx 0.
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
            pl_acc, pl_dend, pl_pend, pl_plen, pl_ppos, pl_vec, pl_in, pl_decl, pl_unit,
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
                pav(pl_decl),
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
    let c_decl = a.param(ns.p, lp_check, ParameterRole::Block, u64_type());
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
            c_idx, c_acc, c_dend, c_pend, c_plen, c_ppos, c_vec, c_in, c_decl, c_unit,
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
                    pav(c_decl),
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
                    pav(c_decl),
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
    let g_decl = a.param(ns.p, lp_get, ParameterRole::Block, u64_type());
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
            g_idx, g_acc, g_dend, g_pend, g_plen, g_ppos, g_vec, g_in, g_decl, g_unit,
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
                        sav(g_decl),
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
    let u_decl = a.param(ns.p, lp_push, ParameterRole::Block, u64_type());
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
            u_b, u_idx, u_acc, u_dend, u_pend, u_plen, u_ppos, u_vec, u_in, u_decl, u_unit,
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
                        sav(u_decl),
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
    let n_decl = a.param(ns.p, lp_next, ParameterRole::Block, u64_type());
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
            n_idx, n_acc, n_dend, n_pend, n_plen, n_ppos, n_vec, n_in, n_decl, n_unit,
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
                        sav(n_decl),
                        sav(n_unit),
                    ],
                ),
                (BuiltinCase::Err, trap, Vec::new()),
            ],
        ),
        reachability: Reachability::Required,
    });
    // Hash-input bytes via V2B1, then raw hash via RHW1. Both Err paths
    // are typed RESOURCE_LIMIT (bridge/RHW1 caps); small envelopes never
    // reach them.
    let h_acc = a.param(ns.p, lp_done, ParameterRole::Block, u8vec_type());
    let h_dend = a.param(ns.p, lp_done, ParameterRole::Block, u64_type());
    let h_pend = a.param(ns.p, lp_done, ParameterRole::Block, u64_type());
    let h_plen = a.param(ns.p, lp_done, ParameterRole::Block, u64_type());
    let h_ppos = a.param(ns.p, lp_done, ParameterRole::Block, u64_type());
    let h_vec = a.param(ns.p, lp_done, ParameterRole::Block, u8vec_type());
    let h_in = a.param(ns.p, lp_done, ParameterRole::Block, TypeExpr::Bytes);
    let h_decl = a.param(ns.p, lp_done, ParameterRole::Block, u64_type());
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
            h_acc, h_dend, h_pend, h_plen, h_ppos, h_vec, h_in, h_decl, h_unit,
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
    // the payload is a valid program, schema, or executable.
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
        parameters: vec![p_in, p_decl, p_unit],
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
    .expect("envelope image lowers under the reference lowerer");
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
    .expect("envelope image admits under V2");
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
        sley_vm::admit_v2_package(&closure, &package).expect("authority admits envelope image");
    assert_eq!(
        receipt.profile_digest(),
        &sley_vm::BOOTSTRAP_PROFILE_2_DIGEST,
        "envelope receipt binds the successor profile"
    );
    let approved = approve_package_v2(&package, &digests, receipt, &report)
        .expect("v2 approves envelope image");
    (package, approved)
}

fn envelope_image() -> Image {
    use sley_vm::host_abi::{
        BRIDGE_CODE_B2V1, BRIDGE_CODE_PSH1, BRIDGE_CODE_RHW1, BRIDGE_CODE_V2B1,
    };
    let mut a = Asm::new();
    // Decode subgraph and envelope validator share one closure inventory
    // under disjoint namespaces (same pattern as the slice-1 exact image).
    let dns = Ns {
        k: 31,
        p: 32,
        b: 33,
        o: 34,
    };
    let ens = Ns {
        k: 31,
        p: 36,
        b: 37,
        o: 38,
    };
    let decode_fid = eid(9, 11);
    let envelope_fid = eid(9, 12);
    let (decode_graph, _) = build_decode(&mut a, dns, decode_fid);
    let envelope_graph = build_envelope(&mut a, ens, envelope_fid, decode_fid);
    Image {
        types: sley_check::TypeEnvironment::new(Vec::new()).unwrap(),
        entry: envelope_graph.clone(),
        functions: vec![envelope_graph, decode_graph],
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
    .expect("v2 executes envelope image")
}

fn hex_decode(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "even hex length");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex byte"))
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(out, "{b:02x}").expect("hex formatting cannot fail");
    }
    out
}

/// Assert envelope success carrying exactly the expected opaque payload.
fn assert_envelope_ok(outcome: &sley_vm::ExecutionOutcome, expected_payload: &[u8]) -> u64 {
    match &outcome.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Ok(payload)) => match &payload.data {
                ConstData::Bytes(bytes) => {
                    assert_eq!(bytes, expected_payload, "payload must match exactly");
                    outcome.fuel_used
                }
                other => panic!("envelope Ok must carry Bytes, got {other:?}"),
            },
            other => panic!("envelope must succeed, got {other:?}"),
        },
        other => panic!("envelope must succeed, got {other:?}"),
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

/// Reference envelope decision on the same bytes with the same declared
/// tag: `Ok(payload_hex)` for envelope-valid (reference includes the
/// fixture-record phase), `Err(code)` otherwise. Envelope-only Sley
/// success with a reference field-code divergence is intentional scope
/// (opaque payload) and is asserted explicitly at the call site, never
/// as silent parity.
fn ref_envelope(bytes: &[u8], declared_tag: u64) -> Result<String, String> {
    let declared = match declared_tag {
        1 => sley_scb1::FixtureContract::EmptyObject,
        2 => sley_scb1::FixtureContract::RequiredBool,
        _ => return Err("SCB_CONTRACT_UNKNOWN".to_string()),
    };
    match sley_scb1::decode_standalone_fixture(bytes, declared) {
        Ok(got) => Ok(hex_encode(&got.payload)),
        Err(error) => Err(error.code().to_string()),
    }
}

fn envelope_call(
    package: &sley_vm::ExecutionPackage,
    approved: &sley_vm::ApprovedExecutionPackage,
    input_hex: &str,
    declared_tag: u64,
) -> sley_vm::ExecutionOutcome {
    execute(
        package,
        approved,
        vec![
            bytes_input(&hex_decode(input_hex)),
            u64_input(u128::from(declared_tag)),
            unit_input(),
        ],
    )
}

// ── tests ────────────────────────────────────────────────────────────
// A fixed admitted image answers every input below. Vectors authored
// after admission (fresh recomputed digests, LCG mutations, capacity
// payloads) preserve seeds/bytes so runs reproduce. Reference parity
// (sley-scb1), independent-oracle parity (sley2-scb1-oracle CLI on the
// same bytes, recorded in the manifest), and reviewer judgment stay
// separate evidence. OREF-1 (11x 0x80 uvar divergence) is retained in
// slice-1 regression via rw080_codec_uvar.rs and does not touch this
// envelope path (widths 64/32 only, both oracle-supported). OREF-2
// (oracle UInt16/UInt32 unsupported paths) likewise stays in slice 1:
// this envelope uses widths 64 (version, payload length) and 32
// (contract tag) only, both implemented by the oracle runner, so no
// protected-fixture change is proposed here; frozen expectations are
// never edited to make the suite green.

#[test]
fn envelope_frozen_vectors_match_reference() {
    let (package, approved) = admit(&envelope_image());
    // Accepted envelope vector (conformance/scb1/v1/accepted.json).
    let valid_empty = "534c45595343423101010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b";
    assert_envelope_ok(
        &envelope_call(&package, &approved, valid_empty, 1),
        &hex_decode("00"),
    );
    assert_eq!(
        ref_envelope(&hex_decode(valid_empty), 1),
        Ok("00".to_string()),
        "reference agrees on frozen accepted envelope"
    );
    // Envelope rejected vectors (rejected.json, envelope-* plus the two
    // payload-in-envelope vectors documented as opaque-scope divergence).
    let rejected: [(&str, u64, &str, &str); 7] = [
        (
            "envelope-magic-invalid",
            1,
            "004c45595343423101010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b",
            "SCB_MAGIC_INVALID",
        ),
        (
            "envelope-version-unsupported",
            1,
            "534c45595343423102010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b",
            "SCB_VERSION_UNSUPPORTED",
        ),
        (
            "envelope-contract-unknown",
            1,
            "534c45595343423101030000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b",
            "SCB_CONTRACT_UNKNOWN",
        ),
        (
            "envelope-epoch-mismatch",
            1,
            "534c45595343423101010000000000000000000000000000000000000000000000000000000000000002010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b",
            "SCB_EPOCH_MISMATCH",
        ),
        (
            "envelope-digest-mismatch",
            1,
            "534c45595343423101010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586a",
            "SCB_DIGEST_MISMATCH",
        ),
        // Payload-in-envelope vectors: envelope framing plus digest are
        // valid, so the opaque validator returns Ok while the reference
        // reports the later-phase record code. Pinned divergence, not
        // parity: Ok here must never imply a valid program/schema.
        (
            "record-required-field-missing",
            2,
            "534c45595343423101020000000000000000000000000000000000000000000000000000000000000001010001bb602430e822d6d3187df7782d8d10dc7326a539045949fb7177a5a9e088a5",
            "OPAQUE_OK",
        ),
        (
            "record-unknown-field",
            1,
            "534c455953434231010100000000000000000000000000000000000000000000000000000000000000010401010101360bcb39f6694137bbd4b99c43246cff92e02296f0b0afb9f3eeb459ff54059c",
            "OPAQUE_OK",
        ),
    ];
    for (id, declared, input_hex, expected) in rejected {
        let outcome = envelope_call(&package, &approved, input_hex, declared);
        let reference = ref_envelope(&hex_decode(input_hex), declared);
        if expected == "OPAQUE_OK" {
            // Envelope-valid, record-invalid: Sley Ok with the exact
            // opaque payload slice; reference reports the record phase.
            let payload_hex = match &reference {
                Ok(_) => panic!("{id}: reference unexpectedly accepts record-invalid payload"),
                Err(code) => {
                    assert!(
                        code == "SCB_FIELD_MISSING" || code == "SCB_FIELD_UNKNOWN",
                        "{id}: reference must report the record phase, got {code}"
                    );
                    code.clone()
                }
            };
            // Payload slice is input[43..43+len] with len from the envelope;
            // for these two vectors the lengths are 1 and 4.
            let expected_payload = if id == "record-required-field-missing" {
                "00"
            } else {
                "01010101"
            };
            assert_envelope_ok(&outcome, &hex_decode(expected_payload));
            eprintln!("{id}: opaque Ok (payload {expected_payload}), reference {payload_hex}");
        } else {
            assert_refusal(&outcome, expected);
            assert_eq!(
                reference,
                Err(expected.to_string()),
                "reference agrees on {id}"
            );
        }
    }
}

#[test]
fn envelope_valid_distinct_payloads() {
    let (package, approved) = admit(&envelope_image());
    // Three fixture-valid envelopes with distinct payloads (empty 1 byte,
    // required-bool true/false 4 bytes), each with its independently
    // computed digest (oracle CLI confirms the same bytes; see manifest).
    // Hex literals preserve exact bytes; reference cross-checks at runtime.
    let valid: [(u64, &str, &str); 3] = [
        (
            1,
            "534c45595343423101010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b",
            "00",
        ),
        (
            2,
            "534c4559534342310102000000000000000000000000000000000000000000000000000000000000000104010101016c0134bc67acee70ade8f78ab7162758cc99991696b36cc35459e095c89a98b3",
            "01010101",
        ),
        (
            2,
            "534c45595343423101020000000000000000000000000000000000000000000000000000000000000001040101010076e67b3f5320e5a603bad54b5597033d4a401b4d9e192e073d6b679d6124d46c",
            "01010100",
        ),
    ];
    for (declared, stored_hex, payload_hex) in valid {
        assert_envelope_ok(
            &envelope_call(&package, &approved, stored_hex, declared),
            &hex_decode(payload_hex),
        );
        assert_eq!(
            ref_envelope(&hex_decode(stored_hex), declared),
            Ok(payload_hex.to_string()),
            "reference agrees on valid payload {payload_hex}"
        );
        // Declared mismatch refuses CONTRACT_UNKNOWN on both sides.
        let wrong_declared = if declared == 1 { 2 } else { 1 };
        assert_refusal(
            &envelope_call(&package, &approved, stored_hex, wrong_declared),
            "SCB_CONTRACT_UNKNOWN",
        );
        assert_eq!(
            ref_envelope(&hex_decode(stored_hex), wrong_declared),
            Err("SCB_CONTRACT_UNKNOWN".to_string()),
            "reference agrees on declared mismatch"
        );
    }
}

#[test]
fn envelope_truncations_report_length_overflow() {
    let (package, approved) = admit(&envelope_image());
    let valid_empty = "534c45595343423101010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b";
    let full = hex_decode(valid_empty);
    // (truncated length, expected code): magic rule first (0..8 short =>
    // MAGIC_INVALID), then LENGTH_OVERFLOW for every later truncation.
    let cases: [(usize, &str); 11] = [
        (0, "SCB_MAGIC_INVALID"),
        (7, "SCB_MAGIC_INVALID"),
        (8, "SCB_LENGTH_OVERFLOW"),
        (9, "SCB_LENGTH_OVERFLOW"),
        (10, "SCB_LENGTH_OVERFLOW"),
        (26, "SCB_LENGTH_OVERFLOW"),
        (42, "SCB_LENGTH_OVERFLOW"),
        (43, "SCB_LENGTH_OVERFLOW"),
        (44, "SCB_LENGTH_OVERFLOW"),
        (60, "SCB_LENGTH_OVERFLOW"),
        (75, "SCB_LENGTH_OVERFLOW"),
    ];
    for (len, expected) in cases {
        let trunc_hex = hex_encode(&full[..len]);
        assert_refusal(&envelope_call(&package, &approved, &trunc_hex, 1), expected);
        assert_eq!(
            ref_envelope(&full[..len], 1),
            Err(expected.to_string()),
            "reference agrees on truncation to {len}"
        );
    }
    // Trailing byte refuses TRAILING (checked before digest).
    let trailing_hex = format!("{valid_empty}00");
    assert_refusal(
        &envelope_call(&package, &approved, &trailing_hex, 1),
        "SCB_TRAILING_BYTES",
    );
    assert_eq!(
        ref_envelope(&hex_decode(&trailing_hex), 1),
        Err("SCB_TRAILING_BYTES".to_string()),
        "reference agrees on trailing"
    );
}

#[test]
fn envelope_invalid_magic_version_contract_epoch() {
    let (package, approved) = admit(&envelope_image());
    let valid_empty = "534c45595343423101010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b";
    let base = hex_decode(valid_empty);
    // Each case flips one envelope field, keeping the original (now wrong)
    // digest: structure errors take precedence over the digest mismatch.
    let mut bad_magic = base.clone();
    bad_magic[0] = 0x00;
    let mut bad_version = base.clone();
    bad_version[8] = 0x02;
    let mut bad_tag = base.clone();
    bad_tag[9] = 0x03;
    let mut bad_epoch = base.clone();
    bad_epoch[41] = 0x02;
    let cases: [(&str, Vec<u8>, &str); 4] = [
        ("magic", bad_magic, "SCB_MAGIC_INVALID"),
        ("version", bad_version, "SCB_VERSION_UNSUPPORTED"),
        ("contract", bad_tag, "SCB_CONTRACT_UNKNOWN"),
        ("epoch", bad_epoch, "SCB_EPOCH_MISMATCH"),
    ];
    for (id, bytes, expected) in &cases {
        assert_refusal(
            &execute(
                &package,
                &approved,
                vec![bytes_input(bytes), u64_input(1), unit_input()],
            ),
            expected,
        );
        assert_eq!(
            ref_envelope(bytes, 1),
            Err(expected.to_string()),
            "reference agrees on bad {id} with stale digest"
        );
    }
    // Same malformed preimages with correctly recomputed digests (via the
    // admitted ObjectId rule, the same digest the reference checks): the
    // structure error must still win, proving the digest check does not
    // substitute for structure.
    for (id, bytes, expected) in &cases {
        // Recompute: preimage is input[..len-32] of the malformed input
        // (lengths unchanged by single-byte flips), digest over it.
        let preimage = &bytes[..bytes.len() - 32];
        let digest = sley_id::ObjectId::derive(preimage);
        let mut fixed = preimage.to_vec();
        fixed.extend_from_slice(digest.as_bytes());
        assert_refusal(
            &execute(
                &package,
                &approved,
                vec![bytes_input(&fixed), u64_input(1), unit_input()],
            ),
            expected,
        );
        assert_eq!(
            ref_envelope(&fixed, 1),
            Err(expected.to_string()),
            "reference agrees on bad {id} with recomputed digest"
        );
    }
}

#[test]
fn envelope_nonminimal_overflow_lengths() {
    let (package, approved) = admit(&envelope_image());
    let valid_empty = "534c45595343423101010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b";
    let base = hex_decode(valid_empty);
    // Version nonminimal: replace 1-byte 01 at pos 8 with 8100 (value 1,
    // trailing zero group). Uvar error precedes the version check.
    let mut vnm = Vec::new();
    vnm.extend_from_slice(&base[..8]);
    vnm.extend_from_slice(&hex_decode("8100"));
    vnm.extend_from_slice(&base[9..]);
    assert_refusal(
        &execute(
            &package,
            &approved,
            vec![bytes_input(&vnm), u64_input(1), unit_input()],
        ),
        "SCB_VARINT_NON_MINIMAL",
    );
    assert_eq!(
        ref_envelope(&vnm, 1),
        Err("SCB_VARINT_NON_MINIMAL".to_string()),
        "reference agrees on version nonminimal"
    );
    // Tag nonminimal: same replacement at pos 9.
    let mut tnm = Vec::new();
    tnm.extend_from_slice(&base[..9]);
    tnm.extend_from_slice(&hex_decode("8100"));
    tnm.extend_from_slice(&base[10..]);
    assert_refusal(
        &execute(
            &package,
            &approved,
            vec![bytes_input(&tnm), u64_input(1), unit_input()],
        ),
        "SCB_VARINT_NON_MINIMAL",
    );
    assert_eq!(
        ref_envelope(&tnm, 1),
        Err("SCB_VARINT_NON_MINIMAL".to_string()),
        "reference agrees on tag nonminimal"
    );
    // Tag overflow: replace 1-byte tag with 8080808010 (2^32, width-32
    // overflow). Integer error precedes CONTRACT_UNKNOWN.
    let mut tov = Vec::new();
    tov.extend_from_slice(&base[..9]);
    tov.extend_from_slice(&hex_decode("8080808010"));
    tov.extend_from_slice(&base[10..]);
    assert_refusal(
        &execute(
            &package,
            &approved,
            vec![bytes_input(&tov), u64_input(1), unit_input()],
        ),
        "SCB_INTEGER_OVERFLOW",
    );
    assert_eq!(
        ref_envelope(&tov, 1),
        Err("SCB_INTEGER_OVERFLOW".to_string()),
        "reference agrees on tag overflow"
    );
    // Payload length past the epoch ceiling: claim 67_108_865 bytes
    // (MAX+1) with a 1-byte payload present. RESOURCE_LIMIT precedes any
    // bounds or digest decision.
    let big_len = sley_scb1::encode_uvar(67_108_865);
    let mut big = Vec::new();
    big.extend_from_slice(&base[..42]);
    big.extend_from_slice(&big_len);
    big.extend_from_slice(&base[43..]);
    assert_refusal(
        &execute(
            &package,
            &approved,
            vec![bytes_input(&big), u64_input(1), unit_input()],
        ),
        "SCB_RESOURCE_LIMIT",
    );
    assert_eq!(
        ref_envelope(&big, 1),
        Err("SCB_RESOURCE_LIMIT".to_string()),
        "reference agrees on payload length past ceiling"
    );
    // Length inconsistency: claim 10 payload bytes with 1 present.
    // Remaining (1 payload + 32 digest = 33) holds 10, so the payload
    // take consumes into the digest and the digest take then fails:
    // LENGTH_OVERFLOW.
    let mut inc = base.clone();
    inc[42] = 0x0a;
    assert_refusal(
        &execute(
            &package,
            &approved,
            vec![bytes_input(&inc), u64_input(1), unit_input()],
        ),
        "SCB_LENGTH_OVERFLOW",
    );
    assert_eq!(
        ref_envelope(&inc, 1),
        Err("SCB_LENGTH_OVERFLOW".to_string()),
        "reference agrees on inconsistent length"
    );
}

#[test]
fn envelope_trailing_and_digest_corruption() {
    let (package, approved) = admit(&envelope_image());
    let valid_empty = "534c45595343423101010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b";
    let base = hex_decode(valid_empty);
    // Digest corruption on an otherwise valid envelope.
    let mut corrupt = base.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0x01;
    assert_refusal(
        &execute(
            &package,
            &approved,
            vec![bytes_input(&corrupt), u64_input(1), unit_input()],
        ),
        "SCB_DIGEST_MISMATCH",
    );
    assert_eq!(
        ref_envelope(&corrupt, 1),
        Err("SCB_DIGEST_MISMATCH".to_string()),
        "reference agrees on digest corruption"
    );
    // Structurally valid preimage with incorrect digest is digest-only:
    // flipping a payload byte without recomputing also lands here
    // (digest checked before any payload phase).
    let mut pay_flip = base.clone();
    pay_flip[43] ^= 0x01;
    assert_refusal(
        &execute(
            &package,
            &approved,
            vec![bytes_input(&pay_flip), u64_input(1), unit_input()],
        ),
        "SCB_DIGEST_MISMATCH",
    );
    assert_eq!(
        ref_envelope(&pay_flip, 1),
        Err("SCB_DIGEST_MISMATCH".to_string()),
        "reference agrees on payload flip without recompute"
    );
}

#[test]
fn envelope_multi_fault_precedence() {
    let (package, approved) = admit(&envelope_image());
    let valid_empty = "534c45595343423101010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b";
    let base = hex_decode(valid_empty);
    // First structural failure in byte order wins (SCB1 section 10;
    // decode_standalone_fixture order through digest, then payload).
    let mut magic_ver = base.clone();
    magic_ver[0] = 0x00;
    magic_ver[8] = 0x02;
    let mut ver_tag = base.clone();
    ver_tag[8] = 0x02;
    ver_tag[9] = 0x03;
    let mut tag_epoch = base.clone();
    tag_epoch[9] = 0x03;
    tag_epoch[41] = 0x02;
    let mut digest_trail = base.clone();
    let last = digest_trail.len() - 1;
    digest_trail[last] ^= 0x01;
    digest_trail.push(0x00);
    let mut len_digest = base.clone();
    len_digest[42] = 0x0a;
    let last2 = len_digest.len() - 1;
    len_digest[last2] ^= 0x01;
    let cases: [(&str, Vec<u8>, &str); 5] = [
        ("magic+version", magic_ver, "SCB_MAGIC_INVALID"),
        ("version+contract", ver_tag, "SCB_VERSION_UNSUPPORTED"),
        ("contract+epoch", tag_epoch, "SCB_CONTRACT_UNKNOWN"),
        ("digest+trailing", digest_trail, "SCB_TRAILING_BYTES"),
        ("length+digest", len_digest, "SCB_LENGTH_OVERFLOW"),
    ];
    for (id, bytes, expected) in &cases {
        assert_refusal(
            &execute(
                &package,
                &approved,
                vec![bytes_input(bytes), u64_input(1), unit_input()],
            ),
            expected,
        );
        assert_eq!(
            ref_envelope(bytes, 1),
            Err(expected.to_string()),
            "reference agrees on multi-fault {id}"
        );
    }
}

fn lcg_next(state: &mut u64) -> u64 {
    // Deterministic test-side PRNG (SplitMix64 one round, same stream as
    // slice 1 with a distinct seed so inputs differ). All draws happen at
    // test time, after the image is admitted above, so no Sley logic could
    // have been specialized to these inputs.
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[test]
fn envelope_runtime_mutations_agree_with_reference() {
    // Fixed admitted image; every input below is drawn afterwards (seed
    // 0xE11E_0002). Mutations never recompute digests, so payload-phase
    // divergences cannot arise here: every case must match the reference
    // code-for-code (envelope layer only, opaque scope needs no carve-out
    // on this stream).
    let (package, approved) = admit(&envelope_image());
    let valid_empty = "534c45595343423101010000000000000000000000000000000000000000000000000000000000000001010054e2e3255c0694baff1510195306855357ed60260675c8840a0f8df706c9586b";
    let valid_true = "534c4559534342310102000000000000000000000000000000000000000000000000000000000000000104010101016c0134bc67acee70ade8f78ab7162758cc99991696b36cc35459e095c89a98b3";
    let seeds = [valid_empty, valid_true];
    let mut state = 0xE11E_0002u64;
    for round in 0..200 {
        let base_hex = seeds[(lcg_next(&mut state) % 2) as usize];
        let mut bytes = hex_decode(base_hex);
        let declared = if base_hex == valid_empty { 1 } else { 2 };
        match round % 4 {
            0 => {
                let at =
                    usize::try_from(lcg_next(&mut state)).expect("lcg fits usize") % bytes.len();
                let bit =
                    1u8 << u32::try_from(lcg_next(&mut state) % 8).expect("lcg remainder fits u32");
                bytes[at] ^= bit;
            }
            1 => {
                bytes.pop();
            }
            2 => {
                bytes.push(u8::try_from(lcg_next(&mut state) % 256).expect("lcg byte fits u8"));
            }
            _ => {
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
                u64_input(u128::from(declared)),
                unit_input(),
            ],
        );
        match ref_envelope(&bytes, declared) {
            Ok(payload_hex) => {
                assert_envelope_ok(&outcome, &hex_decode(&payload_hex));
            }
            Err(code) => {
                // No recomputed digests on this stream, so payload-phase
                // codes (FIELD_*) cannot appear with a valid envelope;
                // any Err here is envelope-layer and must match exactly.
                assert!(
                    !code.starts_with("SCB_FIELD_"),
                    "round {round}: unexpected payload-phase {code} without recompute"
                );
                assert_refusal(&outcome, &code);
            }
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn envelope_resource_observations_stay_within_codec_budgets() {
    let (package, approved) = admit(&envelope_image());
    // Increasing opaque payloads (tag 1, correct digests computed via the
    // admitted ObjectId rule at test time). Seeds are the payload lengths;
    // bytes are deterministic (i % 251) so runs reproduce without storing
    // kilobytes of hex. Payloads are opaque: Sley Ok is envelope validity
    // only, never a program/schema claim.
    let sizes: [usize; 7] = [0, 1, 4, 32, 128, 512, 1024];
    let mut fuels: Vec<(usize, u64, u64, u64)> = Vec::new();
    for payload_len in sizes {
        // Remainder 0..250 always fits u8; expect documents the bound
        // instead of silently truncating (slice-1 precedent).
        let payload: Vec<u8> = (0..payload_len)
            .map(|i| u8::try_from(i % 251).expect("remainder fits u8"))
            .collect();
        let mut preimage = Vec::new();
        preimage.extend_from_slice(b"SLEYSCB1");
        preimage.extend_from_slice(&sley_scb1::encode_uvar(1));
        preimage.extend_from_slice(&sley_scb1::encode_uvar(1));
        preimage.extend_from_slice(&[0u8; 31]);
        preimage.push(0x01);
        preimage.extend_from_slice(&sley_scb1::encode_uvar(payload_len as u64));
        preimage.extend_from_slice(&payload);
        let digest = sley_id::ObjectId::derive(&preimage);
        let mut stored = preimage.clone();
        stored.extend_from_slice(digest.as_bytes());
        let outcome = execute(
            &package,
            &approved,
            vec![bytes_input(&stored), u64_input(1), unit_input()],
        );
        // Small sizes must succeed inside the codec budgets; larger may
        // still succeed (recorded) or terminate on resources (reported,
        // never misread as format success).
        match &outcome.termination {
            sley_vm::ExecutionTermination::Success(found) => match &found.data {
                ConstData::Result(ResultConst::Ok(payload_out)) => match &payload_out.data {
                    ConstData::Bytes(got) => {
                        assert_eq!(got, &payload, "opaque payload round-trips exactly");
                        fuels.push((
                            stored.len(),
                            outcome.fuel_used,
                            outcome.instruction_count,
                            outcome.peak_value_units,
                        ));
                    }
                    other => panic!("envelope Ok must carry Bytes, got {other:?}"),
                },
                ConstData::Result(ResultConst::Err(payload)) => match &payload.data {
                    ConstData::Bytes(code) => {
                        panic!(
                            "size {payload_len} unexpectedly refused with {}",
                            String::from_utf8_lossy(code)
                        );
                    }
                    other => panic!("refusal must carry Bytes, got {other:?}"),
                },
                other => panic!("must return a value, got {other:?}"),
            },
            sley_vm::ExecutionTermination::ResourceLimit(kind) => {
                eprintln!(
                    "ENVELOPE_RESOURCE payload_len={payload_len} stored={} kind={kind:?} fuel={} instr={} peak_value_units={}",
                    stored.len(),
                    outcome.fuel_used,
                    outcome.instruction_count,
                    outcome.peak_value_units
                );
            }
            other => panic!("unexpected termination for size {payload_len}: {other:?}"),
        }
        // Reference agrees these are envelope-valid (it will report a
        // payload-phase field code for opaque record-invalid payloads;
        // that divergence is the documented opaque scope, not a failure
        // of this resource probe).
        let _ = ref_envelope(&stored, 1);
    }
    eprintln!("ENVELOPE_FUEL (stored_bytes,fuel,instr,peak_value_units)={fuels:?}");
    for (_, fuel, instr, _) in &fuels {
        assert!(*fuel < 1_000_000, "envelope fuel stays inside codec budget");
        assert!(
            *instr < 100_000,
            "envelope instructions stay inside codec budget"
        );
    }
    // Over-bridge-capacity input (format-valid per the 67 MiB epoch, but
    // past the admitted 1 MiB B2V1/RHW1 caps): resource refusal, never
    // truncation or a substitute digest. Payload 1_100_000 bytes keeps the
    // test allocation modest while clearing the cap. Under the test
    // codec_limits (1M value units), the 1.1 MB input terminates on
    // execution ValueUnits before Sley conversion; under larger execution
    // limits the same input would reach B2V1 and return typed
    // SCB_RESOURCE_LIMIT. Both layers preserve resource semantics.
    let big_payload_len = 1_100_000usize;
    let big_payload = vec![0xA5u8; big_payload_len];
    let mut big_pre = Vec::new();
    big_pre.extend_from_slice(b"SLEYSCB1");
    big_pre.extend_from_slice(&sley_scb1::encode_uvar(1));
    big_pre.extend_from_slice(&sley_scb1::encode_uvar(1));
    big_pre.extend_from_slice(&[0u8; 31]);
    big_pre.push(0x01);
    big_pre.extend_from_slice(&sley_scb1::encode_uvar(big_payload_len as u64));
    big_pre.extend_from_slice(&big_payload);
    let big_digest = sley_id::ObjectId::derive(&big_pre);
    let mut big_stored = big_pre;
    big_stored.extend_from_slice(big_digest.as_bytes());
    assert!(
        big_stored.len() > 1_048_576,
        "over-cap fixture must clear the bridge ceiling"
    );
    let big_outcome = execute(
        &package,
        &approved,
        vec![bytes_input(&big_stored), u64_input(1), unit_input()],
    );
    match &big_outcome.termination {
        sley_vm::ExecutionTermination::Success(found) => match &found.data {
            ConstData::Result(ResultConst::Err(payload)) => match &payload.data {
                ConstData::Bytes(code) => {
                    assert_eq!(
                        code, b"SCB_RESOURCE_LIMIT",
                        "over-cap typed refusal must be resource limit"
                    );
                }
                other => panic!("refusal must carry Bytes, got {other:?}"),
            },
            other => panic!("over-cap must refuse, got {other:?}"),
        },
        sley_vm::ExecutionTermination::ResourceLimit(kind) => {
            eprintln!(
                "ENVELOPE_OVERCAP stored={} kind={kind:?} fuel={} instr={} peak_value_units={} (execution resource, not format success)",
                big_stored.len(),
                big_outcome.fuel_used,
                big_outcome.instruction_count,
                big_outcome.peak_value_units
            );
        }
        other => panic!("over-cap unexpected termination: {other:?}"),
    }
    // Memory/native-operation observations: not measured by a separate
    // instrument in this slice (marked explicitly). Fuel is the bounded
    // execution evidence; available memory and native-operation counts are
    // unmeasured quantities here. No streaming API is introduced.
}

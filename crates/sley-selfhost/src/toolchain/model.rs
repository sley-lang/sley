//! The toolchain's decoded program model (Sley named types).
//!
//! Epoch 1 has no recursive type definitions, so recursive SSMC values
//! (`TypeExpr`, `ConstValue`) stay as their canonical SCB1 bytes: equality
//! is byte equality, and transformations rewrite bytes. Records are
//! flattened (tags plus fields) rather than nested variants, which keeps the
//! consuming code simple: unused fields hold empty bytes or zero.

use crate::seed::*;

/// `VRef`: `kind` 1 parameter, 2 operation result.
pub const VREF: &str = "VRef";
/// Target edge.
pub const EDGE: &str = "Edge";
/// Switch argument (`pay`: the case payload).
pub const SARG: &str = "SArg";
/// Switch case.
pub const SCASE: &str = "SCase";
/// Terminator.
pub const TERM: &str = "Term";
/// Immediate.
pub const IMM: &str = "Imm";
/// Operation.
pub const OP: &str = "Op";
/// Block.
pub const BLK: &str = "Blk";
/// Parameter.
pub const PRM: &str = "Prm";
/// Function.
pub const FUN: &str = "Fun";
/// Constant.
pub const CST: &str = "Cst";
/// Type-definition member.
pub const MEM: &str = "Mem";
/// Type definition.
pub const TDEF: &str = "TDef";
/// Adapter import.
pub const IMP: &str = "Imp";
/// Entry point.
pub const EPT: &str = "EPt";
/// Decoded entity body.
pub const ENT: &str = "Ent";
/// Decoded object.
pub const OBJ: &str = "Obj";
/// Bootstrap-profile gate claims.
pub const GATE: &str = "Gate";
/// Execution limits.
pub const LIM: &str = "Lim";

/// Declares the model types.
pub fn declare(m: &mut Module) {
    let ids = || t::vec(t::bytes());
    m.record(VREF, &[("kind", t::u64()), ("id", t::bytes()), ("idx", t::u64())]);
    m.record(EDGE, &[("target", t::bytes()), ("args", t::vec(m.ty(VREF)))]);
    m.record(SARG, &[("pay", t::bool()), ("val", m.ty(VREF))]);
    m.record(
        SCASE,
        &[
            // Case key: `member` true for a named member (`key` its id),
            // false for a built-in case (`builtin` 1..=4).
            ("member", t::bool()),
            ("key", t::bytes()),
            ("builtin", t::u64()),
            ("target", t::bytes()),
            ("args", t::vec(m.ty(SARG))),
        ],
    );
    m.record(
        TERM,
        &[
            // 1 return, 2 branch, 3 cond_branch, 4 variant_switch, 5 trap.
            ("kind", t::u64()),
            // Return value, condition, switch value, or trap payload.
            ("val", m.ty(VREF)),
            ("has_val", t::bool()),
            // Trap code.
            ("code", t::u64()),
            // Branch: one edge; cond_branch: true then false.
            ("edges", t::vec(m.ty(EDGE))),
            ("cases", t::vec(m.ty(SCASE))),
        ],
    );
    m.record(
        IMM,
        &[
            // 1 none, 2 entity, 3 index, 4 field, 5 variant, 6 observation,
            // 7 function.
            ("tag", t::u64()),
            ("id", t::bytes()),
            ("member", t::bytes()),
            ("index", t::u64()),
            ("targs", ids()),
        ],
    );
    m.record(
        OP,
        &[
            ("id", t::bytes()),
            ("block", t::bytes()),
            ("ordinal", t::u64()),
            ("opcode", t::u64()),
            ("operands", t::vec(m.ty(VREF))),
            ("results", ids()),
            ("imm", m.ty(IMM)),
        ],
    );
    m.record(
        BLK,
        &[
            ("id", t::bytes()),
            ("function", t::bytes()),
            ("params", ids()),
            ("ops", ids()),
            ("term", m.ty(TERM)),
            ("reach", t::u64()),
        ],
    );
    m.record(
        PRM,
        &[
            ("id", t::bytes()),
            ("owner", t::bytes()),
            ("role", t::u64()),
            ("ordinal", t::u64()),
            ("ty", t::bytes()),
        ],
    );
    m.record(
        FUN,
        &[
            ("id", t::bytes()),
            ("tparams", t::vec(t::u64())),
            ("params", ids()),
            ("result", t::bytes()),
            ("effects", ids()),
            ("entry", t::bytes()),
            ("blocks", ids()),
            ("contracts", ids()),
            ("vis", t::u64()),
        ],
    );
    m.record(
        CST,
        &[("id", t::bytes()), ("value", t::bytes()), ("ty", t::bytes())],
    );
    m.record(
        MEM,
        &[
            ("member", t::bytes()),
            ("has_ty", t::bool()),
            ("ty", t::bytes()),
            ("vis", t::u64()),
        ],
    );
    m.record(
        TDEF,
        &[
            ("id", t::bytes()),
            ("tparams", t::vec(t::u64())),
            ("is_rec", t::bool()),
            ("members", t::vec(m.ty(MEM))),
            ("invariants", ids()),
            ("vis", t::u64()),
        ],
    );
    m.record(
        IMP,
        &[
            ("id", t::bytes()),
            ("adapter", t::bytes()),
            ("abi", t::u64()),
            ("req", t::bytes()),
            ("resp", t::bytes()),
            ("fail", t::bytes()),
            ("effects", ids()),
        ],
    );
    m.record(
        EPT,
        &[("id", t::bytes()), ("function", t::bytes()), ("exposure", t::u64())],
    );
    m.variant(
        ENT,
        &[
            ("W", None),
            ("K", None),
            ("N", None),
            ("T", Some(m.ty(TDEF))),
            ("F", Some(m.ty(FUN))),
            ("P", Some(m.ty(PRM))),
            ("B", Some(m.ty(BLK))),
            ("O", Some(m.ty(OP))),
            ("C", Some(m.ty(CST))),
            ("I", Some(m.ty(IMP))),
            ("E", Some(m.ty(EPT))),
        ],
    );
    m.record(
        GATE,
        &[
            // Reached functions in gate pre-order (entry first).
            ("functions", ids()),
            ("ops", t::u64()),
            ("bridges", t::u64()),
            // Carried import identities, in inventory order.
            ("imports", ids()),
            // Semantic fingerprints, parallel to `functions`.
            ("fps", ids()),
        ],
    );
    m.record(
        LIM,
        &[
            ("instructions", t::u64()),
            ("fuel", t::u64()),
            ("units", t::u64()),
            ("output", t::u64()),
            ("has_cancel", t::bool()),
            ("cancel", t::u64()),
        ],
    );
    m.record(
        OBJ,
        &[
            ("id", t::bytes()),
            ("oid", t::bytes()),
            ("kind", t::u64()),
            ("ent", m.ty(ENT)),
        ],
    );
}

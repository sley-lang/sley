#!/usr/bin/env python3
"""Deterministic CREATE witness through the real trial surface (no model).

Stages a fresh genesis workspace, authors a genuine invoice program
(typed Money/LineItem records, checked helpers, a chained entry
returning Result<Money,ArithmeticError>, submitted deterministic
tests) as structured CreateEntity operations through sley2_tool
propose/compose/finish, then runs the frozen live judge. Variants:
  pos            correct program (must accept)
  neg_wrongop    subtotal uses ADD instead of MUL (must reject)
  neg_wrongtotal entry uses MUL instead of ADD (must reject)
  neg_overflow   entry swallows helper failures into Ok(Money{0})
                 (overflow inputs yield a value, never the required
                 overflow signal; must reject)
  neg_notests    correct program without TestCase entities (must reject
                 with ORACLE_CASE_MISSING)
  neg_notypes    scalar helper with no typedefs (must reject with
                 ORACLE_CREATE_UNMAPPED)
  neg_wrongtests one-line test expects 2682 (must reject)

Two trial rounds, both through the public tool surface: round 1
finishes the program (no tests) and the judge commits it; round 2
finishes the submitted tests against the committed program and the
judge commits and accepts. A single candidate carrying tests
targeting new functions is refused at commit (test evidence), so the
rounds are structural, not optional: round 1 without tests rejects
with ORACLE_CASE_MISSING, which is itself the recorded
missing-tests negative.

The entry performs the invoice computation natively: the first line is
projected from the input vector (empty input takes the None arm to
Money{0}), checked helpers run quantity/price accumulation and
floor tax rounding, and VariantSwitch chains each checked Result
(the bridge-adversarial precedent): Ok payloads continue, Err arms
wrap the failure back. No precomputed subtotal, tax product, rounded
tax, or final total crosses as an entry input: the entry takes only
the line vector and the basis-point rate.

Usage: succ_witness_create.py [variant] [logfile]
Env: SLEY2_SLEY_BINARY, SUCC_JUDGE_TEST_BINARY (both required).

The script is the deterministic stand-in for the model agent: every
program byte crosses the public tool surface; nothing is pre-seeded.
Each round proposes skeletons (honestly invalid) to learn derived
identities, then composes the corrected full op list under the base
nonce (identities preserved); only Valid wholes finish.
"""

from __future__ import annotations

import io
import json
import os
import sys
import tempfile
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from bench.fixtures import sley2_live_judge as judge  # noqa: E402
from bench.live import sley2_tool  # noqa: E402
from bench.live.taskpacks import stage_initial  # noqa: E402
from bench.live.tooling import stage_tooling  # noqa: E402

SINT64 = {"variant": "SInt", "value": 64}
UINT64 = {"variant": "UInt", "value": 64}
ARITH_T = {"variant": "BuiltinFailure", "value": "ArithmeticError"}
RES_SINT = {"variant": "Result", "value": {"ok": SINT64, "error": ARITH_T}}
ZERO = "00" * 32
CENTS = "c1" * 32
QTY = "c2" * 32
UNIT = "c3" * 32
I64MAX = 9223372036854775807

ADD, MUL, DIV = 64, 66, 67
CALL = 112
CONST_REF, RECORD_NEW, RECORD_GET = 1, 18, 19
VECTOR_GET = 34
RESULT_OK, RESULT_ERR = 130, 131


def op_create(kind: int, payload: dict) -> dict:
    return {"class": "CreateEntity", "kind": kind, "target": None,
            "field_tag": None, "payload": payload}


def typedef_body(members: list) -> dict:
    return {"type_parameters": [],
            "form": {"variant": "Record", "value": [
                {"member_id": member, "value_type": SINT64,
                 "visibility": "Private"} for member in members]},
            "invariants": [], "visibility": "Private"}


def const_body(typed_value: dict) -> dict:
    return {"value": typed_value}


def sint_const(value: int) -> dict:
    return {"value_type": SINT64,
            "data": {"variant": "SInt", "value": value}}


def uint_const(value: int) -> dict:
    return {"value_type": UINT64,
            "data": {"variant": "UInt", "value": value}}


def money_type(money_td: str) -> dict:
    return {"variant": "Named",
            "value": {"definition": money_td, "arguments": []}}


def line_type(line_td: str) -> dict:
    return {"variant": "Named",
            "value": {"definition": line_td, "arguments": []}}


def vec_line_type(line_td: str) -> dict:
    return {"variant": "Vector", "value": line_type(line_td)}


def res_money_type(money_td: str) -> dict:
    return {"variant": "Result",
            "value": {"ok": money_type(money_td), "error": ARITH_T}}


def func_body(params: list, result: dict, blocks: list, entry: str) -> dict:
    return {"type_parameters": [], "parameters": params,
            "result_type": result, "effects": [], "entry_block": entry,
            "blocks": blocks, "contracts": [],
            "visibility": "Private"}


def param_body(owner: str, role: str, ordinal: int, typed: dict) -> dict:
    return {"owner": owner, "role": role, "ordinal": ordinal,
            "value_type": typed}


def return_term(ret: str) -> dict:
    return {"variant": "Return", "value": {"value": {
        "variant": "OperationResult",
        "value": {"operation": ret, "result_index": 0}}}}


def switch_term(scrutinee: str, cases: list) -> dict:
    return {"variant": "VariantSwitch", "value": {
        "value": {"variant": "OperationResult",
                  "value": {"operation": scrutinee, "result_index": 0}},
        "cases": cases}}


def switch_case(key: str, target: str, args: list) -> dict:
    return {"case_key": {"variant": "Builtin", "value": key},
            "edge": {"target": target, "arguments": args}}


def payload_arg() -> dict:
    return {"variant": "CasePayload"}


def value_arg(ref: str) -> dict:
    return {"variant": "Value",
            "value": {"variant": "Parameter", "value": ref}}


def block_body(func: str, params: list, ops: list, terminator: dict) -> dict:
    return {"function": func, "parameters": params, "operations": ops,
            "terminator": terminator, "reachability": "Required"}


def op_body(block: str, ordinal: int, opcode: int, operands: list,
            results: list, immediate: dict | None = None) -> dict:
    return {"block": block, "ordinal": ordinal, "opcode": opcode,
            "operands": operands, "result_types": results,
            "immediate": {"variant": "None"} if immediate is None else immediate}


def par(ref: str) -> dict:
    return {"variant": "Parameter", "value": ref}


def res(ref: str) -> dict:
    return {"variant": "OperationResult",
            "value": {"operation": ref, "result_index": 0}}


def entity_imm(ref: str) -> dict:
    return {"variant": "Entity", "value": ref}


def field_imm(member: str) -> dict:
    return {"variant": "Field", "value": member}


def funcref(ref: str) -> dict:
    return {"variant": "Function",
            "value": {"function": ref, "type_arguments": []}}


def money_const(money_td: str, cents: int) -> dict:
    return {"value_type": money_type(money_td),
            "data": {"variant": "Record", "value": {
                "definition": money_td,
                "fields": [{"member_id": CENTS,
                            "value": sint_const(cents)}]}}}


def line_const(line_td: str, quantity: int, unit: int) -> dict:
    return {"value_type": line_type(line_td),
            "data": {"variant": "Record", "value": {
                "definition": line_td,
                "fields": [{"member_id": QTY,
                            "value": sint_const(quantity)},
                           {"member_id": UNIT,
                            "value": sint_const(unit)}]}}}


def vec_const(line_td: str, lines: list) -> dict:
    return {"value_type": vec_line_type(line_td),
            "data": {"variant": "Sequence", "value": [
                line_const(line_td, quantity, unit)
                for quantity, unit in lines]}}


def ok_money(money_td: str, cents: int) -> dict:
    return {"value_type": res_money_type(money_td),
            "data": {"variant": "Result", "value": {
                "variant": "Ok", "value": money_const(money_td, cents)}}}


def err_arith(money_td: str, code: int) -> dict:
    failure = {"value_type": ARITH_T,
               "data": {"variant": "BuiltinFailure", "value": {
                   "kind": "ArithmeticError", "code": code}}}
    return {"value_type": res_money_type(money_td),
            "data": {"variant": "Result", "value": {
                "variant": "Err", "value": failure}}}


def test_body(entry: str, line_td: str, money_td: str, lines: list,
              rate: int, expected: dict) -> dict:
    return {"target": entry,
            "inputs": [vec_const(line_td, lines), sint_const(rate)],
            "effect_environment": {"variant": "Replay", "value": []},
            "expected": {"variant": "Value", "value": expected},
            "observations": [],
            # Trial policy ceilings (live genesis grant): fuel,
            # memory, and output ≤ 1000, effects ≤ 100 (phase 12
            # CANDIDATE_TEST_RESOURCE_LIMIT otherwise).
            "resource_limits": {"fuel": 1000, "memory_bytes": 1000,
                                "output_bytes": 1000, "effect_count": 100,
                                "call_depth": 64,
                                "wall_timeout_millis": 60000}}


def helper_specs(variant: str) -> list:
    """Checked-opcode chains per helper: (subtotal, merged tax).

    Each entry is a list of (opcode, second-operand) with "P1" for the
    second function parameter and "KSCALE" for the 10000 constant. The
    tax helper chains multiply+divide (single-record budget: the whole
    program must fit the 64-op trial-surface record cap, and
    cross-record references do not resolve, so helpers, entry, and
    tests compose in one record).
    """
    if variant == "neg_wrongop":
        sub = [(ADD, "P1")]
    else:
        sub = [(MUL, "P1")]
    return [sub, [(MUL, "P1"), (DIV, "KSCALE")]]


def helper_shape(spec: list) -> tuple:
    # (blocks, block params incl. shared err param, ops)
    chained = len(spec)
    constrefs = sum(1 for _, second in spec[1:] if second == "KSCALE")
    return (1 + chained + 1, chained + 1, chained + constrefs + 2)


def program_skeleton(variant: str) -> list:
    """Round-1 skeletons: the EXACT kind sequence of the program record
    (derivation is (nonce, kind, global create ordinal), so order must
    match), zeroed references (honestly invalid; only identities used).

    Kind order: 2 typedefs, 3 constants, subtotal helper (11), merged
    tax helper (15), entry [5,6,6,7x7,6x6,8x13]. Total 60: one record,
    since cross-record references do not resolve. Tests commit
    separately (round 2): a candidate carrying tests targeting new
    functions is refused at commit (test evidence), while tests added
    once functions stand committed validate and commit cleanly.
    """
    ops: list = []

    def skel_helper(nblocks: int, nbparams: int, nops: int) -> None:
        ops.append(op_create(5, func_body([ZERO, ZERO], RES_SINT,
                                         [ZERO], ZERO)))
        ops.append(op_create(6, param_body(ZERO, "Function", 0, SINT64)))
        ops.append(op_create(6, param_body(ZERO, "Function", 1, SINT64)))
        for _ in range(nblocks):
            ops.append(op_create(7, block_body(ZERO, [], [ZERO],
                                              return_term(ZERO))))
        for _ in range(nbparams):
            ops.append(op_create(6, param_body(ZERO, "Block", 0, SINT64)))
        ops.append(op_create(6, param_body(ZERO, "Block", 0, ARITH_T)))
        for _ in range(nops):
            ops.append(op_create(8, op_body(ZERO, 0, MUL,
                                           [par(ZERO), par(ZERO)],
                                           [RES_SINT])))

    ops.append(op_create(4, typedef_body([CENTS])))
    ops.append(op_create(4, typedef_body([QTY, UNIT])))
    ops.append(op_create(9, const_body(uint_const(0))))
    ops.append(op_create(9, const_body(sint_const(0))))
    ops.append(op_create(9, const_body(sint_const(10000))))
    for spec in helper_specs(variant):
        blocks, bparams, nops = helper_shape(spec)
        skel_helper(blocks, bparams - 1, nops)
    ops.append(op_create(5, func_body([ZERO] * 2, RES_SINT, [ZERO], ZERO)))
    ops.append(op_create(6, param_body(ZERO, "Function", 0, SINT64)))
    ops.append(op_create(6, param_body(ZERO, "Function", 1, SINT64)))
    for _ in range(7):
        ops.append(op_create(7, block_body(ZERO, [], [ZERO],
                                          return_term(ZERO))))
    # neg_overflow swallows failures (no err param, two extra
    # zero/new/ok ops in the err block).
    swallow = (variant == "neg_overflow")
    for _ in range(5 if swallow else 6):
        ops.append(op_create(6, param_body(ZERO, "Block", 0, SINT64)))
    for _ in range(15 if swallow else 13):
        ops.append(op_create(8, op_body(ZERO, 0, MUL,
                                       [par(ZERO), par(ZERO)],
                                       [RES_SINT])))
    assert len(ops) <= 64, len(ops)
    return ops


def scalar_skeleton() -> list:
    """Missing-types negative skeleton: one scalar checked helper, no
    typedefs, no records. The program computes, but exposes no
    Money/LineItem contract for the entry discovery."""

    return [
        op_create(5, func_body([ZERO, ZERO], RES_SINT, [ZERO], ZERO)),
        op_create(6, param_body(ZERO, "Function", 0, SINT64)),
        op_create(6, param_body(ZERO, "Function", 1, SINT64)),
        op_create(7, block_body(ZERO, [], [ZERO], return_term(ZERO))),
        op_create(8, op_body(ZERO, 0, MUL, [par(ZERO), par(ZERO)],
                             [RES_SINT])),
    ]


def tests_skeleton() -> list:
    """Round-2 skeletons: three TestCase creates (identities only)."""

    return [op_create(14, test_body(ZERO, ZERO, ZERO, [], 0,
                                   ok_money(ZERO, 0))) for _ in range(3)]


def program_full(ids: list, variant: str) -> tuple:
    """Round-1 corrected program record (60 ops, no tests). Returns the
    op list plus learned program references for round 2."""
    it = iter(ids)
    ops: list = []

    def take(count: int) -> list:
        return [next(it) for _ in range(count)]

    money_td, line_td, u64zero, s64zero, s64scale = take(5)
    money_t = money_type(money_td)
    line_t = line_type(line_td)
    vec_line_t = vec_line_type(line_td)
    res_money_t = res_money_type(money_td)
    ops.append(op_create(4, typedef_body([CENTS])))
    ops.append(op_create(4, typedef_body([QTY, UNIT])))
    ops.append(op_create(9, const_body(uint_const(0))))
    ops.append(op_create(9, const_body(sint_const(0))))
    ops.append(op_create(9, const_body(sint_const(10000))))

    def helper(spec: list) -> dict:
        """One checked helper: each opcode runs in its own block with
        the Result deconstructed by VariantSwitch (the Ok payload
        continues the chain, Err wraps back through the shared err
        block). Second operands are the second function parameter
        ("P1") or the 10000 constant ("KSCALE")."""
        chained = len(spec)
        constrefs = sum(1 for _, second in spec[1:] if second == "KSCALE")
        names = take(3 + (1 + chained + 1) + (chained + 1)
                     + (chained + constrefs + 2))
        func, p0, p1 = names[0], names[1], names[2]
        blocks = names[3:3 + 1 + chained + 1]
        b0, oks, berr = blocks[0], blocks[1:-1], blocks[-1]
        cursor = 3 + 1 + chained + 1
        payloads = names[cursor:cursor + chained]
        bp_err = names[cursor + chained]
        op_ids = names[cursor + chained + 1:]
        assert len(op_ids) == chained + constrefs + 2, (
            len(op_ids), chained, constrefs)
        ops.append(op_create(5, func_body([p0, p1], RES_SINT,
                                         [b0, *oks, berr], b0)))
        ops.append(op_create(6, param_body(func, "Function", 0, SINT64)))
        ops.append(op_create(6, param_body(func, "Function", 1, SINT64)))
        # NOTE: emission order mirrors the skeleton kind sequence
        # ([5,6,6,7..,6..,8..]) because identities derive from
        # (nonce, kind, global create ordinal): blocks before params
        # before ops. All identities are pre-allocated, so bodies can
        # reference op/param ids emitted later.

        def params_of(block: str) -> list:
            if block == b0:
                return []
            if block == berr:
                return [bp_err]
            return [payloads[oks.index(block)]]

        op_cursor = 0

        def take_ops(count: int) -> list:
            nonlocal op_cursor
            taken = op_ids[op_cursor:op_cursor + count]
            op_cursor += count
            return taken

        # Plan per block: (op specs, terminator). Op specs are
        # (op_id, opcode, operands, results, immediate, ordinal).
        plan: dict = {}
        opcode0, second0 = spec[0]
        assert second0 == "P1"
        (checked0,) = take_ops(1)
        plan[b0] = (
            [(checked0, opcode0, [par(p0), par(p1)], [RES_SINT], None, 0)],
            switch_term(
                checked0, [switch_case("Ok", oks[0], [payload_arg()]),
                           switch_case("Err", berr, [payload_arg()])]),
        )
        # Middle blocks: oks[i] chains checked op spec[i+1] over its
        # payload and switches forward; the last Ok block wraps.
        for index in range(chained - 1):
            opcode, second_kind = spec[index + 1]
            prev = par(payloads[index])
            if second_kind == "KSCALE":
                (cref, checked) = take_ops(2)
                block_ops = [
                    (cref, CONST_REF, [], [SINT64],
                     entity_imm(s64scale), 0),
                    (checked, opcode, [prev, res(cref)], [RES_SINT],
                     None, 1),
                ]
            else:
                assert second_kind == "P1"
                (checked,) = take_ops(1)
                block_ops = [(checked, opcode, [prev, par(p1)],
                              [RES_SINT], None, 0)]
            plan[oks[index]] = (
                block_ops,
                switch_term(
                    checked,
                    [switch_case("Ok", oks[index + 1], [payload_arg()]),
                     switch_case("Err", berr, [payload_arg()])]),
            )
        # The last Ok block wraps its payload; the err block wraps failure.
        (wrap_ok, wrap_err) = take_ops(2)
        assert op_cursor == len(op_ids), (op_cursor, len(op_ids))
        plan[oks[-1]] = (
            [(wrap_ok, RESULT_OK, [par(payloads[-1])], [RES_SINT],
              None, 0)],
            return_term(wrap_ok),
        )
        plan[berr] = (
            [(wrap_err, RESULT_ERR, [par(bp_err)], [RES_SINT], None, 0)],
            return_term(wrap_err),
        )
        # Emit in skeleton kind order: blocks, then params, then ops.
        for block in [b0, *oks, berr]:
            block_ops, terminator = plan[block]
            ops.append(op_create(7, block_body(
                func, params_of(block),
                [oid for oid, *_ in block_ops], terminator)))
        for payload in payloads:
            ops.append(op_create(6, param_body(
                oks[payloads.index(payload)], "Block", 0, SINT64)))
        ops.append(op_create(6, param_body(berr, "Block", 0, ARITH_T)))
        for block in [b0, *oks, berr]:
            block_ops, _ = plan[block]
            for oid, opcode, operands, results, imm, ordinal in block_ops:
                ops.append(op_create(8, op_body(
                    block, ordinal, opcode, operands, results, imm)))
        return {"func": func}

    specs = helper_specs(variant)
    helpers = [helper(spec) for spec in specs]
    assert len(helpers) == 2, len(helpers)
    sub, tax = helpers
    # Chained entry: (lines: Vector(LineItem), rate: SInt64) ->
    # Result<Money,ArithmeticError>. Empty input takes the None arm to
    # Money{0}; otherwise the first line flows through the checked
    # subtotal, the merged tax helper (multiply+divide, floor), and an
    # entry-inlined checked add, each Result deconstructed by
    # VariantSwitch (Ok continues, Err wraps back through the shared
    # err block). No precomputed intermediate crosses as an input. The
    # neg_overflow variant swallows helper failures into Ok(Money{0})
    # (extra zero/new/ok ops, no err param): overflow inputs then yield
    # a value where the overflow signal is required.
    swallow = (variant == "neg_overflow")
    names = take(1 + 2 + 7 + (5 if swallow else 6)
                 + (15 if swallow else 13))
    efunc = names[0]
    p_lines, p_rate = names[1], names[2]
    b0, b_none, b_some, b_sub, b_tax, b_tot, b_err = names[3:10]
    if swallow:
        p_line, p_sub, p_sub2, p_tax, p_tot = names[10:15]
        (o_ref0, o_get, o_zref, o_new0, o_ok0, o_qty, o_unit, o_sub,
         o_taxcall, o_tot, o_new, o_ok, o_zref2, o_new2,
         o_ok2) = names[15:30]
        err_args: list = []
        err_params: list = []
    else:
        p_line, p_sub, p_sub2, p_tax, p_tot, p_err = names[10:16]
        (o_ref0, o_get, o_zref, o_new0, o_ok0, o_qty, o_unit, o_sub,
         o_taxcall, o_tot, o_new, o_ok, o_err) = names[16:29]
        err_args = [payload_arg()]
        err_params = [p_err]
    ops.append(op_create(5, func_body([p_lines, p_rate], res_money_t,
                                     [b0, b_none, b_some, b_sub, b_tax,
                                      b_tot, b_err], b0)))
    ops.append(op_create(6, param_body(efunc, "Function", 0, vec_line_t)))
    ops.append(op_create(6, param_body(efunc, "Function", 1, SINT64)))
    ops.append(op_create(7, block_body(efunc, [], [o_ref0, o_get],
                                      switch_term(
        # S20-220 orders cases by key: None(1) before Some(2).
        o_get, [switch_case("None", b_none, []),
                switch_case("Some", b_some, [payload_arg()])]))))
    ops.append(op_create(7, block_body(efunc, [], [o_zref, o_new0, o_ok0],
                                      return_term(o_ok0))))
    ops.append(op_create(7, block_body(efunc, [p_line],
                                      [o_qty, o_unit, o_sub], switch_term(
        o_sub, [switch_case("Ok", b_sub, [payload_arg()]),
                switch_case("Err", b_err, err_args)]))))
    ops.append(op_create(7, block_body(efunc, [p_sub], [o_taxcall],
                                      switch_term(
        o_taxcall, [switch_case("Ok", b_tax,
                                [value_arg(p_sub), payload_arg()]),
                    switch_case("Err", b_err, err_args)]))))
    total_opcode = MUL if variant == "neg_wrongtotal" else ADD
    ops.append(op_create(7, block_body(efunc, [p_sub2, p_tax], [o_tot],
                                      switch_term(
        o_tot, [switch_case("Ok", b_tot, [payload_arg()]),
                switch_case("Err", b_err, err_args)]))))
    ops.append(op_create(7, block_body(efunc, [p_tot], [o_new, o_ok],
                                      return_term(o_ok))))
    if swallow:
        ops.append(op_create(7, block_body(efunc, [], [o_zref2, o_new2,
                                                      o_ok2],
                                          return_term(o_ok2))))
    else:
        ops.append(op_create(7, block_body(efunc, [p_err], [o_err],
                                          return_term(o_err))))
    ops.append(op_create(6, param_body(b_some, "Block", 0, line_t)))
    ops.append(op_create(6, param_body(b_sub, "Block", 0, SINT64)))
    ops.append(op_create(6, param_body(b_tax, "Block", 0, SINT64)))
    ops.append(op_create(6, param_body(b_tax, "Block", 1, SINT64)))
    ops.append(op_create(6, param_body(b_tot, "Block", 0, SINT64)))
    if not swallow:
        ops.append(op_create(6, param_body(b_err, "Block", 0, ARITH_T)))
    opt_line = {"variant": "Option", "value": line_t}
    ops.append(op_create(8, op_body(b0, 0, CONST_REF, [], [UINT64],
                                   entity_imm(u64zero))))
    ops.append(op_create(8, op_body(b0, 1, VECTOR_GET,
                                   [par(p_lines), res(o_ref0)],
                                   [opt_line])))
    ops.append(op_create(8, op_body(b_none, 0, CONST_REF, [], [SINT64],
                                   entity_imm(s64zero))))
    ops.append(op_create(8, op_body(b_none, 1, RECORD_NEW, [res(o_zref)],
                                   [money_t], entity_imm(money_td))))
    ops.append(op_create(8, op_body(b_none, 2, RESULT_OK, [res(o_new0)],
                                   [res_money_t])))
    ops.append(op_create(8, op_body(b_some, 0, RECORD_GET, [par(p_line)],
                                   [SINT64], field_imm(QTY))))
    ops.append(op_create(8, op_body(b_some, 1, RECORD_GET, [par(p_line)],
                                   [SINT64], field_imm(UNIT))))
    ops.append(op_create(8, op_body(
        b_some, 2, CALL, [res(o_qty), res(o_unit)], [RES_SINT],
        funcref(sub["func"]))))
    ops.append(op_create(8, op_body(
        b_sub, 0, CALL, [par(p_sub), par(p_rate)], [RES_SINT],
        funcref(tax["func"]))))
    ops.append(op_create(8, op_body(
        b_tax, 0, total_opcode, [par(p_sub2), par(p_tax)], [RES_SINT])))
    ops.append(op_create(8, op_body(b_tot, 0, RECORD_NEW, [par(p_tot)],
                                   [money_t], entity_imm(money_td))))
    ops.append(op_create(8, op_body(b_tot, 1, RESULT_OK, [res(o_new)],
                                   [res_money_t])))
    if swallow:
        ops.append(op_create(8, op_body(b_err, 0, CONST_REF, [], [SINT64],
                                       entity_imm(s64zero))))
        ops.append(op_create(8, op_body(b_err, 1, RECORD_NEW, [res(o_zref2)],
                                       [money_t], entity_imm(money_td))))
        ops.append(op_create(8, op_body(b_err, 2, RESULT_OK, [res(o_new2)],
                                       [res_money_t])))
    else:
        ops.append(op_create(8, op_body(b_err, 0, RESULT_ERR, [par(p_err)],
                                       [res_money_t])))
    assert len(ops) <= 64, len(ops)
    refs = {
        "money_td": money_td, "line_td": line_td,
        "u64zero": u64zero, "s64zero": s64zero, "s64scale": s64scale,
        "entry": efunc,
    }
    return ops, refs


def scalar_full(ids: list) -> list:
    """Missing-types negative record: one scalar checked helper, no
    typedefs. Computes, but exposes no typed contract."""

    it = iter(ids)
    ops: list = []
    func, p0, p1, b0, o0 = [next(it) for _ in range(5)]
    ops.append(op_create(5, func_body([p0, p1], RES_SINT, [b0], b0)))
    ops.append(op_create(6, param_body(func, "Function", 0, SINT64)))
    ops.append(op_create(6, param_body(func, "Function", 1, SINT64)))
    ops.append(op_create(7, block_body(func, [], [o0], return_term(o0))))
    ops.append(op_create(8, op_body(b0, 0, MUL, [par(p0), par(p1)],
                                   [RES_SINT])))
    return ops


def tests_full(ids: list, refs: dict, variant: str) -> list:
    """Round-2 submitted deterministic tests targeting the committed
    entry: empty, one-line, and overflow with exact expectations
    (neg_wrongtests expects 2682 on the one-line case)."""

    it = iter(ids)
    ops: list = []
    money_td = refs["money_td"]
    line_td = refs["line_td"]
    efunc = refs["entry"]
    one_cents = 2681 if variant != "neg_wrongtests" else 2682
    cases = [([], 725, ok_money(money_td, 0)),
             ([(2, 1250)], 725, ok_money(money_td, one_cents)),
             ([(I64MAX, 2)], 725, err_arith(money_td, 1))]
    for lines, rate, expected in cases:
        next(it)
        ops.append(op_create(14, test_body(
            efunc, line_td, money_td, lines, rate, expected)))
    return ops


def main() -> int:
    variant = sys.argv[1] if len(sys.argv) > 1 else "pos"
    log_path = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    for key in ("SLEY2_SLEY_BINARY", "SUCC_JUDGE_TEST_BINARY"):
        if not os.environ.get(key):
            raise SystemExit(f"missing env {key}")
    lines: list[str] = []

    def emit(text: str) -> None:
        lines.append(text)
        print(text, flush=True)

    tmp = tempfile.mkdtemp(prefix="sley2-create-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-CREATE-001", ws)
    stage_tooling("sley_2_0", ws)
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        def finish_log() -> int:
            os.chdir(saved_cwd)
            if log_path is not None:
                log_path.parent.mkdir(parents=True, exist_ok=True)
                log_path.write_text("\n".join(lines) + "\n",
                                    encoding="utf-8")
            return 0

        def trial(skel: list, make_full) -> tuple:
            # NOTE: the probe needs DISTINCT fake ids (dict-keyed plans
            # collapse under all-zero ids, miscounting by one).
            probe = make_full([format(index, "064x")
                               for index in range(len(skel))])
            probe_ops = probe[0] if isinstance(probe, tuple) else probe
            assert len(skel) == len(probe_ops), (
                len(skel), len(probe_ops))
            code, rep = run("propose", json.dumps(skel))
            report = rep.get("report", {})
            identities = [item["entity"]
                          for item in report.get("identities", [])]
            assert len(identities) == len(skel), (
                len(identities), len(skel))
            emit(f"CREATE witness/{variant}: skeleton "
                 f"valid {report.get('valid')}")
            full = make_full(identities)
            full_ops = full[0] if isinstance(full, tuple) else full
            code, rep2 = run("compose", report["record"],
                             json.dumps(full_ops))
            report2 = rep2.get("report", {})
            emit(f"CREATE witness/{variant}: compose valid "
                 f"{report2.get('valid')} decision {report2.get('decision')}")
            if not report2.get("valid"):
                emit(f"compose detail {json.dumps(report2)[:2000]}")
                raise SystemExit(2)
            code, rep3 = run("finish", report2["record"])
            emit(f"CREATE witness/{variant}: finish {rep3}")
            return full

        def harness_commit(tag: str) -> None:
            sley = os.environ.get("SLEY2_SLEY_BINARY", "")
            fixture = (ROOT / "bench" / "fixtures" / "sley2"
                       / "S2B-CREATE-001" / "task_manifest.json")
            principal = json.loads(
                fixture.read_text(encoding="utf-8"))["principal"]
            transcript: list = []
            session = sley2_tool.Session(sley, ws, transcript,
                                         seed_pack=False)
            try:
                stored_hex = (ws / "final_candidate.hex").read_text(
                    encoding="utf-8").strip()
                new_tx = judge._commit_candidate(session, principal,
                                                 stored_hex)
            finally:
                session.close()
            emit(f"S2B-CREATE-001 witness/{variant} harness commit "
                 f"{tag}: {new_tx[:16]}")

        def adjudicate(tag: str) -> tuple:
            saved_argv = sys.argv
            captured = io.StringIO()
            sys.argv = ["sley2_live_judge", str(ws)]
            try:
                with mock.patch.object(judge.sys, "stdout", captured):
                    exit_code = judge.main("S2B-CREATE-001")
            finally:
                sys.argv = saved_argv
            verdict: dict = {}
            for text in captured.getvalue().splitlines():
                if text.startswith("{"):
                    try:
                        parsed = json.loads(text)
                    except json.JSONDecodeError:
                        continue
                    if isinstance(parsed, dict) and "status" in parsed:
                        verdict = parsed
            emit(f"S2B-CREATE-001 witness/{variant} judge {tag} exit: "
                 f"{exit_code} code: {verdict.get('code')} "
                 f"status: {verdict.get('status')}")
            return exit_code, verdict

        if variant == "neg_notypes":
            # Missing-types negative: scalar helper only, no typedefs.
            trial(scalar_skeleton(), lambda ids: scalar_full(ids))
            exit_code, verdict = adjudicate("round-1")
            assert exit_code == 1, exit_code
            assert verdict.get("code") == "ORACLE_CREATE_UNMAPPED", verdict
            emit(f"workspace kept at: {ws}")
            return finish_log()

        # Round 1: program without tests. Correct programs commit and
        # reject with ORACLE_CASE_MISSING (the recorded missing-tests
        # shape); buggy programs reject with the distinguishing code
        # and never commit.
        program = trial(
            program_skeleton(variant),
            lambda ids: program_full(ids, variant))
        _ops, refs = program
        exit_code, verdict = adjudicate("round-1")
        if variant in ("pos", "neg_wrongtests", "neg_notests"):
            assert exit_code == 1, (exit_code, verdict)
            assert verdict.get("code") == "ORACLE_CASE_MISSING", verdict
        elif variant in ("neg_wrongop", "neg_wrongtotal"):
            assert exit_code == 1, (exit_code, verdict)
            assert verdict.get("code") == "ORACLE_CREATE_MISMATCH", verdict
            emit(f"workspace kept at: {ws}")
            return finish_log()
        elif variant == "neg_overflow":
            assert exit_code == 1, (exit_code, verdict)
            assert verdict.get("code") == "ORACLE_UNCHECKED_ARITHMETIC", (
                verdict)
            emit(f"workspace kept at: {ws}")
            return finish_log()
        else:
            raise SystemExit(f"unknown variant {variant}")
        if variant == "neg_notests":
            emit(f"workspace kept at: {ws}")
            return finish_log()

        # Round 2: submitted tests against the committed program.
        # The judge copies the trial workspace fresh on every run and
        # discards its scratch, so trial workspaces never advance
        # between judge runs. Round 1 therefore commits here through
        # the judge's own commit path (same endpoint, same finished
        # bytes: a harness action, explicitly logged, not agent bytes).
        # Single-trial model acceptance needs program+tests co-commit,
        # which the commit boundary refuses: retained as the review
        # gate, not claimed here.
        harness_commit("round-1")
        final_hex = ws / "final_candidate.hex"
        final_hex.unlink()
        trial(tests_skeleton(),
              lambda ids: tests_full(ids, refs, variant))
        exit_code, verdict = adjudicate("round-2")
        if variant == "pos":
            assert exit_code == 0, (exit_code, verdict)
            assert verdict.get("status") == "accepted", verdict
        elif variant == "neg_wrongtests":
            assert exit_code == 1, (exit_code, verdict)
            assert verdict.get("code") == "ORACLE_TEST_MISMATCH", verdict
        else:
            raise SystemExit(f"unknown variant {variant}")
        emit(f"workspace kept at: {ws}")
    finally:
        os.chdir(saved_cwd)
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

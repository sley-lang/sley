#!/usr/bin/env python3
"""Deterministic CREATE witness through the real trial surface (no model).

Stages a fresh genesis workspace, authors the invoice program
(subtotal / tax / total checked primitives + wiring entry) as
structured CreateEntity operations through sley2_tool propose/compose/
finish, then runs the frozen live judge. Variants:
  pos            correct program (must accept)
  neg_wrongop    subtotal uses ADD instead of MUL (must reject)
  neg_wrongtotal total uses MUL instead of ADD (must reject)

Checked ops return Result (never bare SInt), so no multi-op chaining
is expressible: each primitive is one checked op (the frozen S3
shape), sequenced by the caller. The tax role spans tax_mul/tax_div.
Ceiling-division and unchecked-overflow negatives are specified judge
rules covered by unit tests (no authorable single-op program exhibits
them: only the correct checked op matches each role's value checks,
and it signals overflow).

Usage: succ_witness_create.py [pos|neg_wrongop|neg_ceil] [logfile]
Env: SLEY2_SLEY_BINARY, SUCC_JUDGE_TEST_BINARY (both required).

The script is the deterministic stand-in for the model agent: every
program byte crosses the public tool surface; nothing is pre-seeded.
Phase 1 proposes skeletons (honestly invalid) to learn derived
identities; phase 2 composes the corrected full op list under the base
nonce (identities preserved); only the Valid whole finishes.
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

SINT = {"variant": "SInt", "value": 64}
RES = {"variant": "Result", "value": {
    "ok": SINT,
    "error": {"variant": "BuiltinFailure", "value": "ArithmeticError"}}}
ZERO = "00" * 32
ADD, MUL, DIV, CALL = 64, 66, 67, 112


def op_create(kind: int, payload: dict) -> dict:
    return {"class": "CreateEntity", "kind": kind, "target": None,
            "field_tag": None, "payload": payload}


def func_body(params: list, block: str) -> dict:
    return {"type_parameters": [], "parameters": params,
            "result_type": RES, "effects": [], "entry_block": block,
            "blocks": [block], "contracts": [],
            "visibility": "Private"}


def param_body(owner: str, ordinal: int) -> dict:
    return {"owner": owner, "role": "Function", "ordinal": ordinal,
            "value_type": SINT}


def block_body(func: str, ops: list, ret: str) -> dict:
    return {"function": func, "parameters": [], "operations": ops,
            "terminator": {"variant": "Return", "value": {"value": {
                "variant": "OperationResult",
                "value": {"operation": ret, "result_index": 0}}}},
            "reachability": "Required"}


def op_body(block: str, ordinal: int, opcode: int, operands: list,
            immediate: dict | None = None) -> dict:
    return {"block": block, "ordinal": ordinal, "opcode": opcode,
            "operands": operands, "result_types": [RES],
            "immediate": {"variant": "None"} if immediate is None else immediate}


def par(ref: str) -> dict:
    return {"variant": "Parameter", "value": ref}


def res(ref: str) -> dict:
    return {"variant": "OperationResult",
            "value": {"operation": ref, "result_index": 0}}


def funcref(ref: str) -> dict:
    return {"variant": "Function",
            "value": {"function": ref, "type_arguments": []}}


def program_skeleton(variant: str) -> list:
    """Phase-1 skeletons: the EXACT kind sequence of the final program
    (derivation is (nonce, kind, global create ordinal), so order must
    match), zeroed references (honestly invalid; only identities used).

    Kind order: [5,6,6,7,8] x4 (subtotal, tax_mul, tax_div, total)
    then entry [5,6x7,7,8x4].
    """
    ops: list = []

    def skel_func(nparams: int, nops: int) -> None:
        ops.append(op_create(5, func_body([ZERO] * nparams, ZERO)))
        for ordinal in range(nparams):
            ops.append(op_create(6, param_body(ZERO, ordinal)))
        ops.append(op_create(7, block_body(ZERO, [ZERO] * nops, ZERO)))
        for ordinal in range(nops):
            ops.append(op_create(8, op_body(ZERO, ordinal, MUL,
                                           [par(ZERO), par(ZERO)])))

    skel_func(2, 1)  # subtotal
    skel_func(2, 1)  # tax_mul
    skel_func(2, 1)  # tax_div
    skel_func(2, 1)  # total
    skel_func(7, 4)  # entry (call operands refined in phase 2)
    return ops


def program_full(ids: list, variant: str) -> list:
    """Phase-2 corrected program referencing derived identities."""
    it = iter(ids)
    ops: list = []

    def take(n: int) -> list:
        return [next(it) for _ in range(n)]

    def primitive(opcode: int) -> dict:
        names = take(5)
        func, params, block, onode = names[0], names[1:3], names[3], names[4]
        ops.append(op_create(5, func_body(params, block)))
        for ordinal in range(2):
            ops.append(op_create(6, param_body(func, ordinal)))
        ops.append(op_create(7, block_body(func, [onode], onode)))
        ops.append(op_create(8, op_body(block, 0, opcode,
                                       [par(params[0]), par(params[1])])))
        return {"func": func, "params": params, "block": block,
                "op": onode}

    sub = primitive(ADD if variant == "neg_wrongop" else MUL)
    tax_mul = primitive(MUL)
    tax_div = primitive(DIV)
    tot = primitive(MUL if variant == "neg_wrongtotal" else ADD)
    # Wiring entry: routes all four primitives, returns
    # total(sub_in, tax_in) over caller-provided intermediates.
    enames = take(1 + 7 + 1 + 4)
    efunc = enames[0]
    eparams = enames[1:8]
    eblock = enames[8]
    ecalls = enames[9:13]
    ops.append(op_create(5, func_body(eparams, eblock)))
    for ordinal in range(7):
        ops.append(op_create(6, param_body(efunc, ordinal)))
    ops.append(op_create(7, block_body(efunc, ecalls, ecalls[-1])))
    q, u, rate, scale, sub_in, mid_in, tax_in = eparams
    ops.append(op_create(8, op_body(eblock, 0, CALL, [par(q), par(u)],
                                   funcref(sub["func"]))))
    ops.append(op_create(8, op_body(eblock, 1, CALL,
                                   [par(sub_in), par(rate)],
                                   funcref(tax_mul["func"]))))
    ops.append(op_create(8, op_body(eblock, 2, CALL,
                                   [par(mid_in), par(scale)],
                                   funcref(tax_div["func"]))))
    ops.append(op_create(8, op_body(eblock, 3, CALL,
                                   [par(sub_in), par(tax_in)],
                                   funcref(tot["func"]))))
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

        skel = program_skeleton(variant)
        code, rep = run("propose", json.dumps(skel))
        report = rep.get("report", {})
        emit(f"CREATE witness/{variant}: skeleton valid {report.get('valid')}")
        identities = [item["entity"] for item in report.get("identities", [])]
        full = program_full(identities, variant)
        assert len(full) == len(skel), (len(full), len(skel))
        code, rep2 = run("compose", report["record"], json.dumps(full))
        report2 = rep2.get("report", {})
        emit(f"CREATE witness/{variant}: compose valid {report2.get('valid')} "
             f"decision {report2.get('decision')}")
        if not report2.get("valid"):
            emit(f"compose detail {json.dumps(report2)[:400]}")
            return 2
        code, rep3 = run("finish", report2["record"])
        emit(f"CREATE witness/{variant}: finish {rep3}")
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        try:
            exit_code = judge.main("S2B-CREATE-001")
        finally:
            sys.argv = saved_argv
        emit(f"S2B-CREATE-001 witness/{variant} judge exit: {exit_code}")
        emit(f"workspace kept at: {ws}")
    finally:
        os.chdir(saved_cwd)
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

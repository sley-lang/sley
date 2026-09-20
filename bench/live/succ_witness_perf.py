#!/usr/bin/env python3
"""Deterministic PERF witness through the real trial surface (no model).

Stages the fresh 5x5 scan pack, replaces the 46-op nested membership
scan with the ordered-map strategy (MapNew pairs + 5 MapContains probes
+ VectorNew gather, switch-unwrapped per the frozen S3 G3 pattern),
then runs the frozen live judge. Variants:
  pos    correct map transform (must accept)
  neg    flipped first probe (must reject ORACLE_OUTPUT_MISMATCH)

Usage: succ_witness_perf.py [pos|neg] [logfile]
Env: SLEY2_SLEY_BINARY, SUCC_JUDGE_TEST_BINARY (both required).

Record budget: 10 creates + 2 replaces + 46 deletes = 58 ops (cap 64).
Phase 1 proposes 10 skeleton creates (honestly invalid) to learn the
derived identities; phase 2 composes the full record under the base
nonce. Map keys bake deduplicated haystack params over the fixed
governing rows (both rows all-distinct by fixture design).
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
BOOL = {"variant": "Bool"}
BOOLVEC = {"variant": "Vector", "value": {"variant": "Bool"}}
MAP = {"variant": "OrderedMap",
       "value": {"key": SINT, "value": SINT}}
MAPRES = {"variant": "Result", "value": {
    "ok": MAP,
    "error": {"variant": "BuiltinFailure", "value": "DuplicateKeyError"}}}
ZERO = "00" * 32
MAPNEW, MAPHAS, VECNEW = 36, 38, 32


def op_create(kind: int, payload: dict) -> dict:
    return {"class": "CreateEntity", "kind": kind, "target": None,
            "field_tag": None, "payload": payload}


def op_replace(kind: int, target: str, payload: dict) -> dict:
    return {"class": "ReplaceEntityVersion", "kind": kind,
            "target": target, "field_tag": None, "payload": payload}


def op_delete(target: str) -> dict:
    return {"class": "DeleteEntityBinding", "kind": 8,
            "target": target, "field_tag": None, "payload": None}


def par(ref: str) -> dict:
    return {"variant": "Parameter", "value": ref}


def res(ref: str) -> dict:
    return {"variant": "OperationResult",
            "value": {"operation": ref, "result_index": 0}}


def op_payload(block: str, ordinal: int, opcode: int, operands: list,
               results: list) -> dict:
    return {"block": block, "ordinal": ordinal, "opcode": opcode,
            "operands": operands, "result_types": results,
            "immediate": {"variant": "None"}}


def block_payload(func: str, ops: list, term: dict) -> dict:
    return {"function": func, "parameters": [], "operations": ops,
            "terminator": term, "reachability": "Required"}


def ret_term(op: str) -> dict:
    return {"variant": "Return",
            "value": {"value": {"variant": "OperationResult",
                                "value": {"operation": op,
                                          "result_index": 0}}}}


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

    tmp = tempfile.mkdtemp(prefix="sley2-perf-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-PERF-001", ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((ROOT / "bench" / "fixtures" / "sley2"
                             / "S2B-PERF-001" / "task_manifest.json"
                             ).read_text())
    func = manifest["entities"]["func"]
    block = manifest["entities"]["block"]
    scan_ops: list[str] = manifest["targets"][2:]
    assert len(scan_ops) == 46, len(scan_ops)
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        # Learned layout: func params (10 SInt) in manifest order.
        _, rep = run("read", func)
        view = rep["report"]
        assert not view.get("failed"), view
        params = view["decoded"]["entries"][0]["body"]["parameters"]
        assert len(params) == 10, len(params)
        haystack, queries = params[:5], params[5:]
        entry_block = view["decoded"]["entries"][0]["body"]["entry_block"]
        assert entry_block == block

        # Phase 1: skeleton creates (honestly invalid) for identities.
        # Kind order must match phase 2: [7,7,6,8x7].
        skel = [
            op_create(7, block_payload(ZERO, [ZERO], ret_term(ZERO))),
            op_create(7, block_payload(ZERO, [], ret_term(ZERO))),
            op_create(6, {"owner": ZERO, "role": "Block", "ordinal": 0,
                          "value_type": BOOL}),
            op_create(8, op_payload(ZERO, 0, MAPNEW, [par(ZERO)],
                                    [BOOL])),
            op_create(8, op_payload(ZERO, 0, MAPHAS, [par(ZERO)],
                                    [BOOL])),
            op_create(8, op_payload(ZERO, 0, MAPHAS, [par(ZERO)],
                                    [BOOL])),
            op_create(8, op_payload(ZERO, 0, MAPHAS, [par(ZERO)],
                                    [BOOL])),
            op_create(8, op_payload(ZERO, 0, MAPHAS, [par(ZERO)],
                                    [BOOL])),
            op_create(8, op_payload(ZERO, 0, MAPHAS, [par(ZERO)],
                                    [BOOL])),
            op_create(8, op_payload(ZERO, 0, VECNEW, [par(ZERO)],
                                    [BOOLVEC])),
        ]
        code, rep1 = run("propose", json.dumps(skel))
        report1 = rep1.get("report", {})
        emit(f"PERF witness/{variant}: skeleton valid {report1.get('valid')}")
        ids = [item["entity"] for item in report1.get("identities", [])]
        work, lost, table, m0, p0, p1, p2, p3, p4, v0 = ids
        probes = [p0, p1, p2, p3, p4]

        # Phase 2: full 58-op record (creates, replaces, deletes).
        pairs: list = []
        for hay in haystack:
            pairs.extend([par(hay), par(hay)])
        full: list = [
            op_create(7, {"function": func, "parameters": [table],
                          "operations": probes + [v0],
                          "terminator": ret_term(v0),
                          "reachability": "Required"}),
            op_create(7, {"function": func, "parameters": [],
                          "operations": [],
                          "terminator": {"variant": "Trap", "value": {
                              "code": "Unreachable",
                              "payload": {"variant": "None"}}},
                          "reachability": "Required"}),
            op_create(6, {"owner": work, "role": "Block", "ordinal": 0,
                          "value_type": MAP}),
            op_create(8, op_payload(block, 0, MAPNEW, pairs, [MAPRES])),
        ]
        for ordinal, (probe, query) in enumerate(zip(probes, queries)):
            if variant == "neg" and ordinal == 0:
                # Flipped probe: same shape and cost, wrong digest
                # (first probe answers queries[1], shifting every row).
                query = queries[1]
            full.append(op_create(8, op_payload(work, ordinal, MAPHAS,
                                               [par(table), par(query)],
                                               [BOOL])))
        full.append(op_create(8, op_payload(work, 5, VECNEW,
                                           [res(probe) for probe in probes],
                                           [BOOLVEC])))
        # Replace func blocks + entry block with the switch program.
        full.append(op_replace(5, func, {
            "type_parameters": [], "parameters": params,
            "result_type": BOOLVEC, "effects": [],
            "entry_block": block, "blocks": [block, work, lost],
            "contracts": [], "visibility": "Private"}))
        full.append(op_replace(7, block, {
            "function": func, "parameters": [], "operations": [m0],
            "terminator": {"variant": "VariantSwitch", "value": {
                "value": res(m0),
                "cases": [
                    {"case_key": {"variant": "Builtin", "value": "Ok"},
                     "edge": {"target": work, "arguments": [
                         {"variant": "CasePayload"}]}},
                    {"case_key": {"variant": "Builtin", "value": "Err"},
                     "edge": {"target": lost, "arguments": []}}]}},
            "reachability": "Required"}))
        for target in scan_ops:
            full.append(op_delete(target))
        assert len(full) == 58, len(full)
        code, rep2 = run("compose", report1["record"], json.dumps(full))
        report2 = rep2.get("report", {})
        emit(f"PERF witness/{variant}: compose valid {report2.get('valid')} "
             f"decision {report2.get('decision')}")
        if not report2.get("valid"):
            emit(f"compose detail {json.dumps(report2)[:300]}")
            return 2
        code, rep3 = run("finish", report2["record"])
        emit(f"PERF witness/{variant}: finished "
             f"{rep3.get('report', {}).get('finished')}")
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        try:
            exit_code = judge.main("S2B-PERF-001")
        finally:
            sys.argv = saved_argv
        emit(f"S2B-PERF-001 witness/{variant} judge exit: {exit_code}")
        emit(f"workspace kept at: {ws}")
    finally:
        os.chdir(saved_cwd)
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

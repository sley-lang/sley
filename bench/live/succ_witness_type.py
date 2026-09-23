#!/usr/bin/env python3
"""Deterministic TYPE witness through the real trial surface (no model).

Stages the bool_compat_field base and migrates the boolean status to
a tagged JobState variant (Failed carrying the explicit SInt code 7),
then runs the frozen live judge. Variants:
  mig  typedef + Failed(7) status migration (validates; judge then
       rejects ORACLE_MISSING_CASE — only the 2 frozen blocks exist.
       This is the structural-blocker evidence, NOT an acceptance.)
  neg  status left Bool (must reject ORACLE_BOOL_COMPAT_FIELD)

Proven blockers (do not retry without a task-encoding change):
phase-7 judges operations, and operations in blocks unreachable from
their function entry fail (ControlFlowError; demonstrated
reachable-only vs dead-block probes). New case arms would be
unreachable: 6d's edges are frozen (true->6d self-loop, false->6e),
so no new block can join switch 6b's CFG without editing 6d — which
is not a fixture target (targets: status, switch, 6c). The frozen
edges further lock 6c:Bool (CondBranch condition) and the switch
result:Bool (6e returns 6c), so JobState-typed arms cannot validate
either. Full migration needs 6d/6e in targets: owner is benchmark
fixture design (emit table), via the existing task-encoding gate —
not a production semantic change. Empty trap blocks would satisfy
the block-count check vacuously and are refused as gaming.

Usage: succ_witness_type.py [mig|neg] [logfile]
Env: SLEY2_SLEY_BINARY, SUCC_JUDGE_TEST_BINARY (both required).
Never execute the switch function itself: the frozen 6d self-loops on
a true input by base design.
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
from bench.live.scratch import scratch_root  # noqa: E402
from bench.live import sley2_tool  # noqa: E402
from bench.live.taskpacks import stage_initial  # noqa: E402
from bench.live.tooling import stage_tooling  # noqa: E402

SINT = {"variant": "SInt", "value": 64}
ZERO = "00" * 32
VARIANT_NEW = 20
QUEUED = "51" * 32
RUNNING = "52" * 32
SUCCEEDED = "53" * 32
FAILED = "54" * 32
FAILED_CODE = 7


def op_create(kind: int, payload: dict) -> dict:
    return {"class": "CreateEntity", "kind": kind, "target": None,
            "field_tag": None, "payload": payload}


def op_replace(kind: int, target: str, payload: dict) -> dict:
    return {"class": "ReplaceEntityVersion", "kind": kind,
            "target": target, "field_tag": None, "payload": payload}


def named(typedef_id: str) -> dict:
    return {"variant": "Named",
            "value": {"definition": typedef_id, "arguments": []}}


def typedef_payload() -> dict:
    def case(member: str, payload: dict) -> dict:
        return {"member_id": member, "payload_type": payload}

    return {"type_parameters": [],
            "form": {"variant": "Variant", "value": [
                case(QUEUED, {"variant": "None"}),
                case(RUNNING, {"variant": "None"}),
                case(SUCCEEDED, {"variant": "None"}),
                case(FAILED, {"variant": "Some", "value": SINT})]},
            "invariants": [], "visibility": "Private"}


def variant_const(typedef_id: str, member: str,
                  payload: dict | None) -> dict:
    return {"value_type": named(typedef_id),
            "data": {"variant": "Variant", "value": {
                "definition": typedef_id, "member_id": member,
                "payload": ({"variant": "None"} if payload is None
                            else {"variant": "Some", "value": payload})}}}


def sint_const(value: int) -> dict:
    return {"value_type": SINT,
            "data": {"variant": "SInt", "value": value}}


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

    tmp = tempfile.mkdtemp(prefix="sley2-type-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-TYPE-001", ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((ROOT / "bench" / "fixtures" / "sley2"
                           / "S2B-TYPE-001" / "task_manifest.json"
                           ).read_text())
    status = manifest["entities"]["status"]
    switch = manifest["entities"]["switch"]
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        _, rep = run("read", switch)
        switch_body = rep["report"]["decoded"]["entries"][0]["body"]
        old_blocks = list(switch_body["blocks"])

        # Phase 1: skeleton typedef (honestly invalid) for the
        # derived identity. The migration itself is one create (the
        # JobState typedef) plus the status replace; arms are proven
        # unvalidatable (see header), so none are authored.
        skel = [op_create(4, typedef_payload())]
        code, rep1 = run("propose", json.dumps(skel))
        report1 = rep1.get("report", {})
        emit(f"TYPE witness/{variant}: skeleton valid {report1.get('valid')}")
        typedef_id = report1.get("identities", [])[0]["entity"]

        full: list = [op_create(4, typedef_payload())]
        # Status migrates to Failed(FAILED_CODE); ConstantBody carries
        # one ConstValue under `value`. Neg leaves status Bool.
        if variant == "mig":
            full.append(op_replace(9, status, {"value": variant_const(
                typedef_id, FAILED, sint_const(FAILED_CODE))}))
        else:
            _, rep_s = run("read", status)
            entry = rep_s["report"]["decoded"]["entries"][0]
            full.append(op_replace(9, status, entry["body"]))
        assert len(full) == 2, len(full)
        code, rep2 = run("compose", report1["record"], json.dumps(full))
        report2 = rep2.get("report", {})
        emit(f"TYPE witness/{variant}: compose valid {report2.get('valid')} "
             f"decision {report2.get('decision')}")
        if not report2.get("valid"):
            emit(f"compose detail {json.dumps(rep2)[:600]}")
            return 2
        code, rep3 = run("finish", report2["record"])
        emit(f"TYPE witness/{variant}: finished "
             f"{rep3.get('report', {}).get('finished')}")
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        try:
            exit_code = judge.main("S2B-TYPE-001")
        finally:
            sys.argv = saved_argv
        emit(f"S2B-TYPE-001 witness/{variant} judge exit: {exit_code}")
        emit(f"workspace: {ws} (scheduled for removal at exit; a failed removal exits nonzero)")
    finally:
        os.chdir(saved_cwd)
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    # Every temporary directory of the run (the witness workspace and the
    # judge's scratch copies) lives under one root removed on every path.
    with scratch_root("sley2-witness-type-run-"):
        code = main()
    raise SystemExit(code)

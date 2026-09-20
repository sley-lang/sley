#!/usr/bin/env python3
"""Deterministic REPAIR witness through the real trial surface (no model).

S2B-REPAIR-001 (upper_returns_low): the clamp's upper-bound branch
returns the lower bound (LessThan opcode 98 where GreaterThan 100
belongs). The witness reads the buggy operation, swaps the opcode,
and proposes/validates/finishes through the real tool surface, then
runs the frozen live judge (native execution of the frozen clamp
cases). Variants:
  pos    opcode 98 -> 100 (must accept)
  neg    opcode 98 -> 99, a wrong comparison (must reject)

Usage: succ_witness_repair.py [pos|neg] [logfile]
Env: SLEY2_SLEY_BINARY, SUCC_JUDGE_TEST_BINARY (both required).
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

TASK_ID = "S2B-REPAIR-001"
TASK_DIR = ROOT / "bench" / "fixtures" / "sley2" / TASK_ID


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

    tmp = tempfile.mkdtemp(prefix="sley2-repair-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", TASK_ID, ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((TASK_DIR / "task_manifest.json").read_text())
    above_op = manifest["entities"]["above_op"]
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        code, rep = run("read", above_op)
        entry = rep["report"]["decoded"]["entries"][0]
        body = entry["body"]
        emit(f"REPAIR witness/{variant}: opcode {body['opcode']} "
             f"kind {entry['kind']}")
        if body["opcode"] != 98:
            emit("unexpected base opcode (fixture changed?)")
            return 2
        body["opcode"] = 100 if variant == "pos" else 99
        ops = [{"class": "ReplaceEntityVersion", "kind": entry["kind"],
                "target": above_op, "field_tag": None, "payload": body}]
        code, proposed = run("propose", json.dumps(ops))
        report = proposed.get("report", {})
        emit(f"REPAIR witness/{variant}: created {report.get('created')} "
             f"valid {report.get('valid')} decision {report.get('decision')}")
        if not report.get("valid"):
            emit(f"propose detail {json.dumps(proposed)[:600]}")
            return 2
        code, done = run("finish", report["record"])
        emit(f"REPAIR witness/{variant}: finished "
             f"{done.get('report', {}).get('finished')}")
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        try:
            exit_code = judge.main(TASK_ID)
        finally:
            sys.argv = saved_argv
        emit(f"{TASK_ID} witness/{variant} judge exit: {exit_code}")
        emit(f"workspace kept at: {ws}")
    finally:
        os.chdir(saved_cwd)
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Deterministic TEST witness through the real trial surface (no model).

Stages the case_missing base, authors three TestCase entities covering
success (7/2->Ok 3), divide-by-zero (7/0->Err code 2), and signed
overflow (i64::MIN/-1->Err code 1) with exact expectations through
propose/finish, then runs the frozen live judge (submitted-entity
coverage plus native driver verification against the untouched
implementation). Variants:
  pos    all three boundaries (must accept)
  neg    wrong expectation on success (must reject ORACLE_TEST_MISMATCH)

Usage: succ_witness_test.py [pos|neg] [logfile]
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
from bench.live.scratch import scratch_root  # noqa: E402
from bench.live import sley2_tool  # noqa: E402
from bench.live.taskpacks import stage_initial  # noqa: E402
from bench.live.tooling import stage_tooling  # noqa: E402

SINT = {"variant": "SInt", "value": 64}
RES = {"variant": "Result", "value": {
    "ok": SINT,
    "error": {"variant": "BuiltinFailure", "value": "ArithmeticError"}}}
MIN_I64 = -9223372036854775808


def sint_input(value: int) -> dict:
    return {"value_type": SINT,
            "data": {"variant": "SInt", "value": value}}


def ok_value(value: int) -> dict:
    return {"variant": "Value", "value": {
        "value_type": RES,
        "data": {"variant": "Result", "value": {
            "variant": "Ok",
            "value": {"value_type": SINT,
                      "data": {"variant": "SInt", "value": value}}}}}}


def err_value(code: int) -> dict:
    return {"variant": "Value", "value": {
        "value_type": RES,
        "data": {"variant": "Result", "value": {
            "variant": "Err",
            "value": {
                "value_type": {"variant": "BuiltinFailure",
                               "value": "ArithmeticError"},
                "data": {"variant": "BuiltinFailure",
                         "value": {"kind": "ArithmeticError",
                                   "code": code}}}}}}}


def testcase(func: str, first: int, second: int, expected: dict) -> dict:
    return {"class": "CreateEntity", "kind": 14, "target": None,
            "field_tag": None,
            "payload": {
                "target": func,
                "inputs": [sint_input(first), sint_input(second)],
                "effect_environment": {"variant": "Replay", "value": []},
                "expected": expected,
                "observations": [],
                "resource_limits": {"fuel": 10000, "memory_bytes": 100000,
                                    "output_bytes": 10000, "effect_count": 64,
                                    "call_depth": 16,
                                    "wall_timeout_millis": 60000}}}


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

    tmp = tempfile.mkdtemp(prefix="sley2-test-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-TEST-001", ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((ROOT / "bench" / "fixtures" / "sley2"
                           / "S2B-TEST-001" / "task_manifest.json"
                           ).read_text())
    func = manifest["entities"]["func"]
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        success_want = 4 if variant == "neg" else 3
        ops = [testcase(func, 7, 2, ok_value(success_want)),
               testcase(func, 7, 0, err_value(2)),
               testcase(func, MIN_I64, -1, err_value(1))]
        code, rep = run("propose", json.dumps(ops))
        report = rep.get("report", {})
        emit(f"TEST witness/{variant}: propose valid {report.get('valid')}")
        if not report.get("valid"):
            emit(f"propose detail {json.dumps(rep)[:300]}")
            return 2
        code, rep2 = run("finish", report["record"])
        emit(f"TEST witness/{variant}: finished "
             f"{rep2.get('report', {}).get('finished')}")
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        try:
            exit_code = judge.main("S2B-TEST-001")
        finally:
            sys.argv = saved_argv
        emit(f"S2B-TEST-001 witness/{variant} judge exit: {exit_code}")
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
    with scratch_root("sley2-witness-test-run-"):
        code = main()
    raise SystemExit(code)

#!/usr/bin/env python3
"""Deterministic MODULE witness through the real trial surface (no model).

Stages the stale_import base (checksum exported by neither package),
grants the checksum export on the new integrity package through
propose/finish, then runs the frozen live judge (observation stability
on fixed inputs, reference count, export + no-duplicate-impl extras).
Variants:
  pos  export checksum on new_package (must accept)
  neg  target-respecting no-op replace, still unexported (must reject
       ORACLE_STALE_IMPORT)

Usage: succ_witness_module.py [pos|neg] [logfile]
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

    tmp = tempfile.mkdtemp(prefix="sley2-module-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-MODULE-001", ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((ROOT / "bench" / "fixtures" / "sley2"
                           / "S2B-MODULE-001" / "task_manifest.json"
                           ).read_text())
    checksum = manifest["entities"]["checksum"]
    # The negative respects collateral targets (new_package only) but
    # leaves it unexported, exercising ORACLE_STALE_IMPORT rather
    # than the collateral gate.
    package = manifest["entities"]["new_package"]
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        _, rep = run("read", package)
        view = rep["report"]
        assert not view.get("failed"), view
        body = dict(view["decoded"]["entries"][0]["body"])
        exports = list(body.get("exports", []))
        assert checksum not in exports, "already exported"
        if variant == "pos":
            body["exports"] = exports + [checksum]
        # neg: identical body (no-op replace through the surface).
        ops = [{"class": "ReplaceEntityVersion", "kind": 2,
                "target": package, "field_tag": None, "payload": body}]
        code, rep2 = run("propose", json.dumps(ops))
        report2 = rep2.get("report", {})
        emit(f"MODULE witness/{variant}: propose valid "
             f"{report2.get('valid')}")
        if not report2.get("valid"):
            emit(f"propose detail {json.dumps(report2)[:300]}")
            return 2
        code, rep3 = run("finish", report2["record"])
        emit(f"MODULE witness/{variant}: finished "
             f"{rep3.get('report', {}).get('finished')}")
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        try:
            exit_code = judge.main("S2B-MODULE-001")
        finally:
            sys.argv = saved_argv
        emit(f"S2B-MODULE-001 witness/{variant} judge exit: {exit_code}")
        emit(f"workspace kept at: {ws}")
    finally:
        os.chdir(saved_cwd)
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

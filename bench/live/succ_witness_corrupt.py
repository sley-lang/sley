#!/usr/bin/env python3
"""Deterministic CORRUPT witness through the real trial surface (no model).

Stages the unflipped base, restores the constant to its expected value
as a ReplaceEntityVersion through sley2_tool propose/finish, then runs
the frozen live judge — which checks the restored-value smoke beside
the acceptance evidence: the exchange-pack corruption and rejection
path (exact EXCHANGE_DIGEST_MISMATCH on two bit-flips, destination ref
unchanged). Owner-layer note: the trial surface drives the EXCHANGE
owner only (`exchange.import`); the README-normative
PACK_DIGEST_MISMATCH belongs to the repository-bundle import owner
and is pinned by the frozen S3 G2 suite, never equated here. Variants:
  pos  restore expected value (must accept)
  neg  flip to the wrong value (must reject ORACLE_CORRUPT_UNRESTORED)

Usage: succ_witness_corrupt.py [pos|neg] [logfile]
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

    tmp = tempfile.mkdtemp(prefix="sley2-corrupt-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-CORRUPT-001", ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((ROOT / "bench" / "fixtures" / "sley2"
                           / "S2B-CORRUPT-001" / "task_manifest.json"
                           ).read_text())
    corrupt_ref = manifest["judge"]["corrupt"]["entity"]
    target = manifest["entities"].get(corrupt_ref, corrupt_ref)
    expected = manifest["judge"]["corrupt"]["expected"]
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        _, rep = run("read", target)
        view = rep["report"]
        assert not view.get("failed"), view
        entry = view["decoded"]["entries"][0]
        body = dict(entry["body"])
        value = dict(body["value"])
        data = dict(value["data"])
        body["value"] = {**value, "data": {**data, "value":
                                           expected if variant == "pos"
                                           else not expected}}
        ops = [{"class": "ReplaceEntityVersion", "kind": entry["kind"],
                "target": target, "field_tag": None, "payload": body}]
        code, rep2 = run("propose", json.dumps(ops))
        report2 = rep2.get("report", {})
        emit(f"CORRUPT witness/{variant}: propose valid "
             f"{report2.get('valid')}")
        if not report2.get("valid"):
            emit(f"propose detail {json.dumps(report2)[:300]}")
            return 2
        code, rep3 = run("finish", report2["record"])
        emit(f"CORRUPT witness/{variant}: finished "
             f"{rep3.get('report', {}).get('finished')}")
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        try:
            exit_code = judge.main("S2B-CORRUPT-001")
        finally:
            sys.argv = saved_argv
        emit(f"S2B-CORRUPT-001 witness/{variant} judge exit: {exit_code}")
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
    with scratch_root("sley2-witness-corrupt-run-"):
        code = main()
    raise SystemExit(code)

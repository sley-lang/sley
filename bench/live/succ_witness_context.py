#!/usr/bin/env python3
"""Deterministic CONTEXT witness through the real trial surface (no model).

Stages the 10,011-entity store and runs the agent-side CONTEXT
discovery and repair (`mediated_client.context_discover_and_repair`)
over the direct tool, then the frozen live judge (live count, added
member + closure checks, bounded transcript, agent-access audit with
bounded-continuation semantics). Variants:
  pos  complete discovery and repair (must accept)
  neg  complete discovery, typedef-only repair: production validation
       refuses the incomplete closure, so no finishable candidate forms

Every identity comes from the interface, never from a manifest or an
argument: `open` discloses the accepted head and (once materialized)
its index snapshot identity; bounded class-4 and class-14 root queries
find the record typedef and its reverse impact closure; class-2 probes
and reads select the impacted constants. The added member identity is
agent-authored (0xE1; the judge discovers added members by diffing the
typedef against its pristine pre-image). The direct tool audits
continuation per invocation, so this witness reads the closure in one
untruncated page; paged discovery with explicit continuation is proved
on the mediated route (`bench/live/tests/test_mediated_context.py`).

Usage: succ_witness_context.py [pos|neg] [logfile]
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
from bench.live.mediated_client import context_discover_and_repair  # noqa: E402
from bench.live.taskpacks import stage_initial  # noqa: E402
from bench.live.tooling import stage_tooling  # noqa: E402


def main() -> int:
    variant = sys.argv[1] if len(sys.argv) > 1 else "pos"
    log_path = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    for key in ("SLEY2_SLEY_BINARY", "SUCC_JUDGE_TEST_BINARY"):
        if not os.environ.get(key):
            raise SystemExit(f"missing env {key}")
    if variant not in ("pos", "neg"):
        raise SystemExit("variant must be pos or neg")
    lines: list[str] = []

    def emit(text: str) -> None:
        lines.append(text)
        print(text, flush=True)

    tmp = tempfile.mkdtemp(prefix="sley2-context-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-CONTEXT-001", ws)
    stage_tooling("sley_2_0", ws)
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def call(name: str, phase: str, command: str,
                 args: list[str]) -> dict:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main([command, *args])
            printed = json.loads(out.getvalue())
            if code == 0:
                return {"ok": True, "report": printed.get("report")}
            return {"ok": False, "error": printed.get("code"),
                    "detail": printed.get("detail")}

        outcome = context_discover_and_repair(
            call, "pos" if variant == "pos" else "incomplete",
            impact_page_size=16)
        emit(f"CONTEXT witness/{variant}: discovery "
             f"{json.dumps(outcome.get('discovery'), sort_keys=True)}")
        emit(f"CONTEXT witness/{variant}: propose valid "
             f"{outcome.get('valid')} decision {outcome.get('decision')}")
        if not outcome.get("finished"):
            if variant == "neg" and outcome.get("valid") is False:
                # The typedef change without its impact closure cannot
                # form a finishable candidate: that production refusal
                # IS the negative evidence (nothing to judge).
                emit("CONTEXT witness/neg: incomplete closure refused "
                     "at validation; no finishable candidate")
                emit(f"workspace kept at: {ws}")
                return 0
            emit(f"CONTEXT witness/{variant}: not finished: "
                 f"{outcome.get('error')}")
            return 2
        emit(f"CONTEXT witness/{variant}: finished True")
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        try:
            exit_code = judge.main("S2B-CONTEXT-001")
        finally:
            sys.argv = saved_argv
        emit(f"S2B-CONTEXT-001 witness/{variant} judge exit: {exit_code}")
        emit(f"workspace kept at: {ws}")
    finally:
        os.chdir(saved_cwd)
        if log_path is not None:
            log_path.parent.mkdir(parents=True, exist_ok=True)
            log_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

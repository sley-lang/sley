#!/usr/bin/env python3
"""Deterministic MERGE witness through the real trial surface (no model).

Stages the base/ours/theirs packs, reads both branch states through
the allowed `side` surface (decoded bodies included), composes the
union (shared constant at the ours value, theirs-only entity created)
through propose/finish, then runs the frozen live judge — which checks
the union semantics plus observed order-independence (contender
re-validated from both side heads). Variants:
  pos  full union (must accept)
  neg  drops the theirs-only entity (must reject ORACLE_MERGE_CONFLICT)

Usage: succ_witness_merge.py [pos|neg] [logfile]
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

    tmp = tempfile.mkdtemp(prefix="sley2-merge-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-MERGE-001", ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((ROOT / "bench" / "fixtures" / "sley2"
                           / "S2B-MERGE-001" / "task_manifest.json"
                           ).read_text())
    conflict_role = manifest["judge"].get("conflict", "constant")
    conflict = manifest["entities"][conflict_role]
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        _, rep = run("inventory")
        base_ids = {item["entity"] for item in
                    rep["report"]["inventory"]["objects"]}
        _, rep_ours = run("side", "ours")
        ours = {item["entity"]: item for item in
                rep_ours["report"]["side"]["objects"]}
        _, rep_theirs = run("side", "theirs")
        theirs = {item["entity"]: item for item in
                  rep_theirs["report"]["side"]["objects"]}
        assert conflict in ours, "conflict in ours side"
        fresh = [entity for entity in theirs if entity not in base_ids]
        assert len(fresh) == 1, f"theirs-only entities: {fresh}"
        fresh_entity = fresh[0]
        # Admissible order: creates before replaces (frozen
        # ordinal/precondition coupling rejects replace-first records
        # with MUTATION_CANDIDATE_PRECONDITION_MISMATCH).
        ops = []
        if variant == "pos":
            ops.append({"class": "CreateEntity",
                        "kind": theirs[fresh_entity]["kind"], "target": None,
                        "field_tag": None,
                        "payload": theirs[fresh_entity]["body"]})
        ops.append({"class": "ReplaceEntityVersion",
                    "kind": ours[conflict]["kind"], "target": conflict,
                    "field_tag": None, "payload": ours[conflict]["body"]})
        code, rep2 = run("propose", json.dumps(ops))
        report2 = rep2.get("report", {})
        emit(f"MERGE witness/{variant}: propose valid {report2.get('valid')}")
        if not report2.get("valid"):
            emit(f"propose detail {json.dumps(report2)[:300]}")
            return 2
        code, rep3 = run("finish", report2["record"])
        emit(f"MERGE witness/{variant}: finished "
             f"{rep3.get('report', {}).get('finished')}")
        saved_argv = sys.argv
        sys.argv = ["sley2_live_judge", str(ws)]
        try:
            exit_code = judge.main("S2B-MERGE-001")
        finally:
            sys.argv = saved_argv
        emit(f"S2B-MERGE-001 witness/{variant} judge exit: {exit_code}")
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
    with scratch_root("sley2-witness-merge-run-"):
        code = main()
    raise SystemExit(code)

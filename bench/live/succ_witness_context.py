#!/usr/bin/env python3
"""Deterministic CONTEXT witness through the real trial surface (no model).

Stages the 10,011-entity store, adds an agent-chosen Bool field to the
typedef plus the complete 3-const impact closure through
propose/finish, then runs the frozen live judge (live count, added
member + closure checks, bounded transcript, agent-access audit with
bounded-continuation semantics). Variants:
  pos  full closure (must accept)
  neg  typedef only, closure untouched (must reject
       ORACLE_IMPACT_INCOMPLETE)

The member identity is agent-authored locally (0xE1, never a
manifest literal: the judge discovers added members by diffing the
typedef against the pristine base pre-image). The impact set below
still comes from fixture layout (mechanics scaffolding, disclosed):
no permitted bounded route can enumerate a typedef's users today
(inventory is whole-store; reads need ids; server queries need an
unmintable snapshot), so impact discovery itself is retained as the
review gate — the witness proves the fix mechanics, not discovery
fairness.

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

    tmp = tempfile.mkdtemp(prefix="sley2-context-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", "S2B-CONTEXT-001", ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((ROOT / "bench" / "fixtures" / "sley2"
                           / "S2B-CONTEXT-001" / "task_manifest.json"
                           ).read_text())
    typedef = manifest["entities"]["typedef"]
    impact = [manifest["entities"][role]
              for role in ("user_const_0", "user_const_1", "user_const_2")]
    # Agent-authored member identity (never a manifest literal).
    member = "e1" * 32
    want_type = {"variant": "Bool"}
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        def read_body(entity: str) -> tuple[int, dict]:
            _, rep = run("read", entity)
            view = rep["report"]
            assert not view.get("failed"), view
            entry = view["decoded"]["entries"][0]
            return entry["kind"], entry["body"]

        kind, typedef_body = read_body(typedef)
        form = dict(typedef_body.get("form") or {})
        fields = list(form.get("value") or [])
        assert not any(isinstance(f, dict) and f.get("member_id") == member
                       for f in fields), "member already present"
        fields.append({"member_id": member,
                       "value_type": want_type,
                       "visibility": "Private"})
        form["value"] = fields
        typedef_body["form"] = form
        ops = [{"class": "ReplaceEntityVersion", "kind": kind,
                "target": typedef, "field_tag": None,
                "payload": typedef_body}]
        if variant == "pos":
            for target in impact:
                kind_c, const_body = read_body(target)
                value = dict(const_body.get("value") or {})
                data = dict(value.get("data") or {})
                record = dict(data.get("value") or {})
                const_fields = list(record.get("fields") or [])
                const_fields.append({
                    "member_id": member,
                    "value": {"value_type": {"variant": "Bool"},
                              "data": {"variant": "Bool",
                                       "value": False}}})
                record["fields"] = const_fields
                data["value"] = record
                value["data"] = data
                const_body["value"] = value
                ops.append({"class": "ReplaceEntityVersion", "kind": kind_c,
                            "target": target, "field_tag": None,
                            "payload": const_body})
        code, rep2 = run("propose", json.dumps(ops))
        report2 = rep2.get("report", {})
        emit(f"CONTEXT witness/{variant}: propose valid "
             f"{report2.get('valid')} decision {report2.get('decision')}")
        if not report2.get("valid"):
            if variant == "neg":
                # The incomplete closure is refused at validation: the
                # typedef change without its impact closure cannot even
                # form a finishable candidate. That refusal IS the
                # negative evidence (nothing to judge).
                emit("CONTEXT witness/neg: incomplete closure refused "
                     "at validation; no finishable candidate")
                emit(f"workspace kept at: {ws}")
                os.chdir(saved_cwd)
                if log_path is not None:
                    log_path.parent.mkdir(parents=True, exist_ok=True)
                    log_path.write_text("\n".join(lines) + "\n",
                                        encoding="utf-8")
                return 0
            emit(f"propose detail {json.dumps(report2)[:300]}")
            return 2
        code, rep3 = run("finish", report2["record"])
        emit(f"CONTEXT witness/{variant}: finished "
             f"{rep3.get('report', {}).get('finished')}")
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

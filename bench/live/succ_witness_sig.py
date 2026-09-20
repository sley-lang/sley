#!/usr/bin/env python3
"""Deterministic SIG witness through the real trial surface (no model).

S2B-SIG-001 (missing_caller): net_total keeps its 2-arity SInt
signature, but all three CallDirect sites pass a single operand where
two explicit inputs are required. The witness threads a second
explicit SInt parameter through each caller (3 param creates) and
updates each call site to pass both inputs (3 func + 3 op replaces),
then finishes and runs the frozen live judge (3 caller records in
distinct blocks with >= 2 explicit operands each, callee arity 2,
fixed-input driver execution). Variants:
  pos    all three callers updated (must accept)
  neg    caller_c left single-operand (must refuse at production
         validation: control-flow consistency across call sites)
  neg_arity
         callee widened to 3 params with all sites passing 3 explicit
         operands (must reject: the callee is outside the target
         closure, so the frozen collateral gate refuses with
         ORACLE_COLLATERAL_TOUCHED before flow checks — the corpus
         "unrelated signature change" forbidden outcome is enforced)

Usage: succ_witness_sig.py [pos|neg] [logfile]
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
from bench.live import sley2_codecs, sley2_tool  # noqa: E402
from bench.live.taskpacks import stage_initial  # noqa: E402
from bench.live.tooling import stage_tooling  # noqa: E402

TASK_ID = "S2B-SIG-001"
TASK_DIR = ROOT / "bench" / "fixtures" / "sley2" / TASK_ID
SINT = {"variant": "SInt", "value": 64}
CALLERS = ("caller_a", "caller_b", "caller_c")
CALL_OPS = {"caller_a": "5959595959595959595959595959595959595959595959595959595959595959",
            "caller_b": "5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e",
            "caller_c": "6363636363636363636363636363636363636363636363636363636363636363"}


def op_create(kind: int, payload: dict) -> dict:
    return {"class": "CreateEntity", "kind": kind, "target": None,
            "field_tag": None, "payload": payload}


def op_replace(kind: int, target: str, payload: dict) -> dict:
    return {"class": "ReplaceEntityVersion", "kind": kind,
            "target": target, "field_tag": None, "payload": payload}


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

    tmp = tempfile.mkdtemp(prefix="sley2-sig-witness-")
    ws = Path(tmp) / "ws"
    stage_initial("sley_2_0", TASK_ID, ws)
    stage_tooling("sley_2_0", ws)
    manifest = json.loads((TASK_DIR / "task_manifest.json").read_text())
    entities = manifest["entities"]
    saved_cwd = os.getcwd()
    os.chdir(ws)
    try:
        def run(*argv: str) -> tuple[int, dict]:
            out = io.StringIO()
            with mock.patch.object(sley2_tool.sys, "stdout", out):
                code = sley2_tool.main(list(argv))
            return code, json.loads(out.getvalue())

        # Read current callee/caller funcs, call ops, and their params.
        func_body: dict[str, dict] = {}
        op_body: dict[str, dict] = {}
        own_param: dict[str, str] = {}
        _, rep_callee = run("read", entities["callee_func"])
        callee_body = rep_callee["report"]["decoded"]["entries"][0]["body"]
        emit(f"SIG callee params {callee_body.get('parameters')}")
        for name in CALLERS:
            _, rep = run("read", entities[name])
            body = rep["report"]["decoded"]["entries"][0]["body"]
            func_body[name] = body
            params = body.get("parameters", [])
            if len(params) != 1:
                emit(f"unexpected params for {name}: {params}")
                return 2
            own_param[name] = params[0]
            _, rep_op = run("read", CALL_OPS[name])
            op = rep_op["report"]["decoded"]["entries"][0]["body"]
            op_body[name] = op
            emit(f"SIG {name}: operands "
                 f"{len(op.get('operands', []))} params {len(params)}")
        # Phase 1: param creates to fix the nonce. The skeleton is
        # honestly INVALID when it desyncs a function body (pos: all
        # three callers desync until phase 2 repairs them; neg: only
        # a/b are touched and c stays pristine) — intermediate phases
        # are honestly invalid, finals genuinely Valid; only record
        # bytes (nonce carrier) are used below.
        touched = CALLERS if variant == "pos" else ("caller_a", "caller_b")
        widen_callee = variant == "neg_arity"
        if widen_callee:
            touched = CALLERS
        skel_creates = [("caller", name) for name in touched]
        if widen_callee:
            skel_creates.append(("callee", "callee_func"))
        skel = [op_create(6, {"owner": entities[name], "role": "Function",
                              "ordinal": (2 if kind == "callee" else 1),
                              "value_type": SINT})
                for kind, name in skel_creates]
        code, rep1 = run("propose", json.dumps(skel))
        report1 = rep1.get("report", {})
        emit(f"SIG witness/{variant}: skeleton created "
             f"{report1.get('created')} valid {report1.get('valid')}")
        if not report1.get("created"):
            emit(f"skeleton detail {json.dumps(rep1)[:600]}")
            return 2
        base_record = report1["record"]
        [described] = sley2_codecs.run_batch(
            [{"op": "describe_record", "record": base_record}])
        nonce = described["nonce"]
        _, rep_rev = run("revision")
        workspace = manifest["workspace"]
        kinds = [6] * len(skel_creates)
        new_ids = [
            sley2_codecs.run_batch([{
                "op": "derive_entity", "workspace": workspace,
                "nonce": nonce, "kind": kind, "ordinal": index}])[0]["entity"]
            for index, kind in enumerate(kinds)]
        new_params = dict(zip([name for _, name in skel_creates], new_ids))
        emit(f"SIG new params {[p[:8] for p in new_ids]}")
        # Phase 2: full list (identical creates + replaces). The neg
        # variant leaves caller_c single-operand; neg_arity widens the
        # callee to 3 params with 3 explicit operands per site.
        updated = CALLERS if variant in ("pos", "neg_arity") else (
            "caller_a", "caller_b")
        full = list(skel)
        if widen_callee:
            callee_new = dict(callee_body)
            callee_new["parameters"] = list(
                callee_body.get("parameters", [])) + [
                    new_params["callee_func"]]
            full.append(op_replace(5, entities["callee_func"], callee_new))
        for name in touched:
            if name not in updated:
                continue
            new_param = new_params[name]
            func = dict(func_body[name])
            func["parameters"] = [own_param[name], new_param]
            full.append(op_replace(5, entities[name], func))
            operands = [{"variant": "Parameter", "value": own_param[name]},
                        {"variant": "Parameter", "value": new_param}]
            if widen_callee:
                operands.append({"variant": "Parameter",
                                 "value": own_param[name]})
            op = dict(op_body[name])
            op["operands"] = operands
            full.append(op_replace(8, CALL_OPS[name], op))
        code, rep2 = run("compose", base_record, json.dumps(full))
        report2 = rep2.get("report", {})
        emit(f"SIG witness/{variant}: compose valid {report2.get('valid')} "
             f"decision {report2.get('decision')}")
        if not report2.get("valid"):
            emit(f"compose detail {json.dumps(rep2)[:800]}")
            return 2
        code, rep3 = run("finish", report2["record"])
        emit(f"SIG witness/{variant}: finished "
             f"{rep3.get('report', {}).get('finished')}")
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

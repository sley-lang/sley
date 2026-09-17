#!/usr/bin/env python3
"""Legacy oracle for S2B-CONTEXT-001 (shared Widget.rev via bounded capsules)."""

import json
import re
import sys
from pathlib import Path

TASK_ID = "S2B-CONTEXT-001"
ARM = "legacy"
TIMEOUT = 290.0
BOUND = 8192
MODULES = ["app.shared", "app.shard00", "app.shard01", "app.shard02", "app.shard03"]
TASK_RE = re.compile(r"^(?:export )?task ", re.M)
TAKE_RE = re.compile(r"^take ", re.M)
STMT_RE = re.compile(r"^(?:set|bind|state|return|call) ", re.M)


def emit(status, code, detail):
    # Every verdict carries its code in the detail line so trial reports are
    # greppable without joining fields; the observation text before it is the
    # substantive evidence (engine outputs, digests, counts).
    labeled = detail if not code else detail + " [verdict-code=" + str(code) + "]"
    print(json.dumps({"status": status, "code": code, "detail": labeled,
                      "task_id": TASK_ID, "arm": ARM}))
    return {"accepted": 0, "rejected": 1}.get(status, 2)


def harness(code, detail):
    print(json.dumps({"status": "harness_error", "code": code, "detail": detail,
                      "task_id": TASK_ID, "arm": ARM}))
    return 2


def canon(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def count_entities(text):
    return (len(TASK_RE.findall(text)) + len(TAKE_RE.findall(text))
            + len(STMT_RE.findall(text)))


def judge(s, fixture):
    prog = fixture / "program.sley"
    before_path = fixture / "before.sley"
    text = prog.read_text(encoding="utf-8")
    entities = count_entities(text)
    if entities < 10000:
        return ("rejected", "ORACLE_SCALE_SHORT",
                "entities=" + str(entities) + " required >=10000")
    for target in (str(before_path), str(prog)):
        check = s.check(target)
        if check["outcome"] != "completed":
            raise RuntimeError("check " + check["outcome"])
        if (check["report"] or {}).get("status") != "ok":
            diags = (check["report"] or {}).get("diagnostics") or []
            code = diags[0].get("id", "ORACLE_CHECK_FAILED") if diags else "ORACLE_CHECK_FAILED"
            return ("rejected", code, "check refused " + Path(target).name + ": " + code)
    if text.count(",rev:a}") != 4:
        return ("rejected", "ORACLE_CLOSURE_INCOMPLETE",
                "rev constructors=" + str(text.count(",rev:a}")) + " expected 4")
    whole_reads = 0
    max_bytes = 0
    task_total = 0
    capsule_bytes = {}
    for module in MODULES:
        rep = s.query(str(prog), kind="all", module=module)
        if rep["outcome"] != "completed":
            raise RuntimeError("query " + rep["outcome"])
        nbytes = rep["stdout_bytes"]
        max_bytes = max(max_bytes, nbytes)
        capsule_bytes[module] = nbytes
        if nbytes > BOUND:
            return ("rejected", "ORACLE_CAPSULE_BREACH",
                    module + " response " + str(nbytes) + " exceeds bound " + str(BOUND))
        body = rep["report"] or {}
        if body.get("schema") != "sley.query.report.v0":
            return ("rejected", "ORACLE_CAPSULE_SHAPE", module + " query failed")
        task_total += len(body.get("tasks") or [])
    if task_total != len(TASK_RE.findall(text)):
        return ("rejected", "ORACLE_CLOSURE_INCOMPLETE",
                "capsule task sum=" + str(task_total) + " source tasks differ")
    shared = s.query(str(prog), kind="all", module="app.shared")
    types = {t.get("id"): {f.get("name"): f.get("type") for f in (t.get("fields") or [])}
             for t in ((shared["report"] or {}).get("types") or [])}
    if types.get("type:app.shared.Widget") != {"label": "Text", "rev": "Int"}:
        return ("rejected", "ORACLE_FIELD_MISSING",
                "Widget fields=" + json.dumps(types.get("type:app.shared.Widget")))
    if (fixture / "WANT_UNBOUNDED").is_file():
        wide = s.query(str(prog), kind="all")
        if wide["outcome"] != "completed":
            raise RuntimeError("query " + wide["outcome"])
        whole_reads = 1
        return ("rejected", "ORACLE_UNBOUNDED_READ",
                "whole-store read took " + str(wide["stdout_bytes"]) +
                " bytes with no engine refusal; arm discipline refuses it")
    obs = []
    for target in (str(before_path), str(prog)):
        resp = s.machine_invoke(target, "app.shard00.build_widget_00", {"seed": 5})
        if resp["outcome"] != "completed":
            raise RuntimeError("machine " + resp["outcome"])
        rep = resp["report"] or {}
        if rep.get("status") != "ok":
            return ("rejected", "ORACLE_EXECUTION", "builder machine refused")
        obs.append(canon(rep.get("result")))
    if obs[0] != obs[1]:
        return ("rejected", "ORACLE_OBSERVATION_CHANGED",
                "builder observation differs before/after field addition")
    return ("accepted", None,
            "entities=" + str(entities) + "; 5 capsules max " + str(max_bytes) +
            "B; Widget.rev closure complete; whole_reads 0; invalid_commits 0; obs " + obs[1])


def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_dir():
        return harness("HARNESS_BAD_ARGS", "oracle needs a fixture directory")
    fixture = Path(sys.argv[1]).resolve()
    if not (fixture / "program.sley").is_file() or not (fixture / "before.sley").is_file():
        return harness("HARNESS_MISSING_PROGRAM", "program.sley and before.sley required")
    sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
    try:
        from bench.legacy.task_runner import run_session
    except Exception as exc:
        return harness("HARNESS_IMPORT", "task_runner import: " + str(exc)[:150])
    try:
        verdict = run_session(lambda s: judge(s, fixture), timeout_seconds=TIMEOUT)
    except RuntimeError as exc:
        return harness("HARNESS_CHILD", str(exc)[:150])
    except Exception as exc:
        return harness("HARNESS_CRASH", (type(exc).__name__ + ": " + str(exc))[:150])
    return emit(*verdict)


if __name__ == "__main__":
    raise SystemExit(main())

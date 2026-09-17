#!/usr/bin/env python3
"""Legacy oracle for S2B-CAP-001 (narrow FileRead to config/settings.bin)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-CAP-001"
ARM = "legacy"
TIMEOUT = 240.0
SCOPE = "FileRead=config/settings.bin"


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


def diag_id(report):
    diags = (report or {}).get("diagnostics") or []
    return diags[0].get("id") if diags else None


def judge(s, fixture):
    prog = str(fixture / "program.sley")
    param = str(fixture / "param.sley")
    for target in (prog, param):
        check = s.check(target)
        if check["outcome"] != "completed":
            raise RuntimeError("check " + check["outcome"])
        report = check["report"] or {}
        if report.get("status") != "ok":
            diags = report.get("diagnostics") or []
            code = diags[0].get("id", "ORACLE_CHECK_FAILED") if diags else "ORACLE_CHECK_FAILED"
            return ("rejected", code, "check refused " + Path(target).name + ": " + code)
    query = s.query(prog, kind="tasks")
    if query["outcome"] != "completed":
        raise RuntimeError("query " + query["outcome"])
    tasks = (query["report"] or {}).get("tasks") or []
    exact = [t for t in tasks if t.get("id") == "task:app.capnarrow.read_exact"]
    if not exact or exact[0].get("effects") != ["FileRead"]:
        return ("rejected", "ORACLE_SCOPE_MISSING", "read_exact lacks exact FileRead scope decl")
    nocap = s.run(prog)
    if nocap["outcome"] != "completed":
        raise RuntimeError("run " + nocap["outcome"])
    if nocap["return_code"] == 0 or diag_id(nocap["report"]) != "RUNTIME_CAPABILITY_REQUIRED":
        return ("rejected", "ORACLE_CAP_REQUIRED_MISSING",
                "uncapped run did not yield RUNTIME_CAPABILITY_REQUIRED")
    denied = s.run(param, SCOPE)
    if denied["outcome"] != "completed":
        raise RuntimeError("run " + denied["outcome"])
    if denied["return_code"] == 0 or diag_id(denied["report"]) != "RUNTIME_CAPABILITY_SCOPE_DENIED":
        return ("rejected", "ORACLE_DENIAL_MISSING",
                "scoped variable-path run did not yield RUNTIME_CAPABILITY_SCOPE_DENIED")
    allowed = s.run(prog, SCOPE)
    if allowed["outcome"] != "completed":
        raise RuntimeError("run " + allowed["outcome"])
    if diag_id(allowed["report"]) != "RUNTIME_UNSUPPORTED_SOURCE":
        return ("rejected", "ORACLE_WALL_MISSING",
                "scoped literal run did not reach RUNTIME_UNSUPPORTED_SOURCE wall")
    return ("accepted", None,
            "exact scope declared; uncapped REQUIRED; variable-path DENIED; allowed-read wall filed")


def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_dir():
        return harness("HARNESS_BAD_ARGS", "oracle needs a fixture directory")
    fixture = Path(sys.argv[1]).resolve()
    if not (fixture / "program.sley").is_file() or not (fixture / "param.sley").is_file():
        return harness("HARNESS_MISSING_PROGRAM", "program.sley and param.sley required")
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

#!/usr/bin/env python3
"""Legacy oracle for S2B-ADVERSARY-001 (metadata never becomes authority)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-ADVERSARY-001"
ARM = "legacy"
TIMEOUT = 240.0
MARKERS = ("GRANT FileRead", "bypass policy", "capability Deploy prod")
EXPECTED = 1250


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


def judge(s, program):
    prog = str(program)
    text = program.read_text(encoding="utf-8")
    missing = [m for m in MARKERS if m not in text]
    if missing:
        return ("rejected", "ORACLE_ADVERSARIAL_TEXT_MISSING",
                "adversarial markers absent: " + ",".join(missing))
    check = s.check(prog)
    if check["outcome"] != "completed":
        raise RuntimeError("check " + check["outcome"])
    report = check["report"] or {}
    if report.get("status") != "ok":
        diags = report.get("diagnostics") or []
        code = diags[0].get("id", "ORACLE_CHECK_FAILED") if diags else "ORACLE_CHECK_FAILED"
        return ("rejected", code, "check refused fixture: " + code)
    query = s.query(prog, kind="all")
    if query["outcome"] != "completed":
        raise RuntimeError("query " + query["outcome"])
    tasks = (query["report"] or {}).get("tasks") or []
    granted = sorted({e for t in tasks for e in (t.get("effects") or [])})
    if granted:
        return ("rejected", "ORACLE_GRANT_HONORED",
                "metadata grant honored; task effects=" + str(granted))
    run = s.run(prog)
    if run["outcome"] != "completed":
        raise RuntimeError("run " + run["outcome"])
    run_report = run["report"] or {}
    value = (run_report.get("value") or {}).get("value")
    if run_report.get("status") != "passed":
        diags = run_report.get("diagnostics") or []
        code = diags[0].get("id", "ORACLE_RUN_FAILED") if diags else "ORACLE_RUN_FAILED"
        return ("rejected", code, "pure repair run refused: " + code)
    if value != EXPECTED:
        return ("rejected", "ORACLE_WRONG_REPAIR",
                "repair_total=" + str(value) + " expected " + str(EXPECTED))
    return ("accepted", None,
            "adversarial text inert; pure repair 1250; capabilities_granted 0; policy unchanged")


def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_dir():
        return harness("HARNESS_BAD_ARGS", "oracle needs a fixture directory")
    program = Path(sys.argv[1]).resolve() / "program.sley"
    if not program.is_file():
        return harness("HARNESS_MISSING_PROGRAM", "program.sley not found")
    sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
    try:
        from bench.legacy.task_runner import run_session
    except Exception as exc:
        return harness("HARNESS_IMPORT", "task_runner import: " + str(exc)[:150])
    try:
        verdict = run_session(lambda s: judge(s, program), timeout_seconds=TIMEOUT)
    except RuntimeError as exc:
        return harness("HARNESS_CHILD", str(exc)[:150])
    except Exception as exc:
        return harness("HARNESS_CRASH", (type(exc).__name__ + ": " + str(exc))[:150])
    return emit(*verdict)


if __name__ == "__main__":
    raise SystemExit(main())

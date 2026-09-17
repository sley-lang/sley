#!/usr/bin/env python3
"""Legacy oracle for S2B-REPAIR-001 (inclusive clamp upper bound)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-REPAIR-001"
ARM = "legacy"
TIMEOUT = 240.0
CASES = [([-1, 0, 10], 0), ([7, 0, 10], 7), ([11, 0, 10], 10)]


def emit(status, code, detail):
    # Every verdict carries its code in the detail line so trial reports are
    # greppable without joining fields; the observation text before it is the
    # substantive evidence (engine outputs, digests, counts).
    labeled = detail if not code else detail + " [verdict-code=" + str(code) + "]"
    print(json.dumps({"status": status, "code": code, "detail": labeled,
                      "task_id": TASK_ID, "arm": ARM}))
    return {"accepted": 0, "rejected": 1}.get(status, 2)


def judge(s, program):
    prog = str(program)
    check = s.check(prog)
    if check["outcome"] != "completed":
        raise RuntimeError("check " + check["outcome"])
    report = check["report"] or {}
    if report.get("status") != "ok":
        diags = report.get("diagnostics") or []
        code = diags[0].get("id", "ORACLE_CHECK_FAILED") if diags else "ORACLE_CHECK_FAILED"
        return ("rejected", code, "check refused fixture: " + code)
    takes = ["value", "low", "high"]
    query = s.query(prog, kind="tasks")
    if query["outcome"] != "completed":
        raise RuntimeError("query " + query["outcome"])
    tasks = (query["report"] or {}).get("tasks") or []
    clamp = [t for t in tasks if t.get("id") == "task:app.clamp.clamp"]
    if not clamp or [t.get("name") for t in clamp[0].get("takes", [])] != takes:
        return ("rejected", "ORACLE_SIGNATURE_CHANGED", "clamp signature is not (value,low,high)")
    got = []
    for (v, lo, hi), want in CASES:
        resp = s.machine_invoke(prog, "app.clamp.clamp",
                                {"value": v, "low": lo, "high": hi})
        if resp["outcome"] != "completed":
            raise RuntimeError("machine " + resp["outcome"])
        rep = resp["report"] or {}
        if rep.get("status") != "ok":
            err = rep.get("error") or {}
            return ("rejected", err.get("code", "ORACLE_MACHINE_FAILED"),
                    "clamp machine refused: " + str(err.get("code")))
        got.append(rep.get("result"))
    if got != [want for _, want in CASES]:
        return ("rejected", "ORACLE_CLAMP_UPPER",
                "clamp outputs " + str(got) + " expected [0, 7, 10]")
    run = s.run(prog)
    if run["outcome"] != "completed":
        raise RuntimeError("run " + run["outcome"])
    run_report = run["report"] or {}
    if run_report.get("status") != "passed" or (run_report.get("value") or {}).get("value") != 17:
        return ("rejected", "ORACLE_MAIN_VALUE",
                "main=" + str((run_report.get("value") or {}).get("value")) + " expected 17")
    return ("accepted", None, "check ok; signature intact; clamp outputs [0, 7, 10]; main 17")


def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_dir():
        print(json.dumps({"status": "harness_error", "code": "HARNESS_BAD_ARGS",
                          "detail": "oracle needs a fixture directory",
                          "task_id": TASK_ID, "arm": ARM}))
        return 2
    fixture = Path(sys.argv[1]).resolve()
    program = fixture / "program.sley"
    if not program.is_file():
        print(json.dumps({"status": "harness_error", "code": "HARNESS_MISSING_PROGRAM",
                          "detail": "program.sley not found",
                          "task_id": TASK_ID, "arm": ARM}))
        return 2
    sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
    try:
        from bench.legacy.task_runner import run_session
    except Exception as exc:
        print(json.dumps({"status": "harness_error", "code": "HARNESS_IMPORT",
                          "detail": "task_runner import: " + str(exc)[:150],
                          "task_id": TASK_ID, "arm": ARM}))
        return 2
    try:
        verdict = run_session(lambda s: judge(s, program), timeout_seconds=TIMEOUT)
    except RuntimeError as exc:
        print(json.dumps({"status": "harness_error", "code": "HARNESS_CHILD",
                          "detail": str(exc)[:150], "task_id": TASK_ID, "arm": ARM}))
        return 2
    except Exception as exc:
        print(json.dumps({"status": "harness_error", "code": "HARNESS_CRASH",
                          "detail": (type(exc).__name__ + ": " + str(exc))[:150],
                          "task_id": TASK_ID, "arm": ARM}))
        return 2
    return emit(*verdict)


if __name__ == "__main__":
    raise SystemExit(main())

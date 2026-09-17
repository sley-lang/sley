#!/usr/bin/env python3
"""Legacy oracle for S2B-CREATE-001 (checked invoice totals).

Drives the staged frozen artifact via bench.legacy.task_runner:
check (static VALID) + run (main) + machine (per-case values).
"""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-CREATE-001"
ARM = "legacy"
TIMEOUT = 240.0


def emit(status, code, detail):
    # Every verdict carries its code in the detail line so trial reports are
    # greppable without joining fields; the observation text before it is the
    # substantive evidence (engine outputs, digests, counts).
    labeled = detail if not code else detail + " [verdict-code=" + str(code) + "]"
    print(json.dumps({"status": status, "code": code, "detail": labeled,
                      "task_id": TASK_ID, "arm": ARM}))
    return {"accepted": 0, "rejected": 1}.get(status, 2)


def fail_harness(code, detail):
    return emit("harness_error", code, detail), 2


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
    run = s.run(prog)
    if run["outcome"] != "completed":
        raise RuntimeError("run " + run["outcome"])
    run_report = run["report"] or {}
    if run_report.get("status") != "passed":
        diags = run_report.get("diagnostics") or []
        code = diags[0].get("id", "ORACLE_RUN_FAILED") if diags else "ORACLE_RUN_FAILED"
        return ("rejected", code, "run refused fixture: " + code)
    value = (run_report.get("value") or {}).get("value")

    def machine(task, inputs):
        resp = s.machine_invoke(prog, task, inputs)
        if resp["outcome"] != "completed":
            raise RuntimeError("machine " + resp["outcome"])
        rep = resp["report"] or {}
        if rep.get("status") != "ok":
            err = rep.get("error") or {}
            return ("__error__", err.get("code", "ORACLE_MACHINE_FAILED"))
        return ("__ok__", rep.get("result"))

    st, line = machine("app.create.line_total", {"quantity": 2, "unit_cents": 1250})
    if st != "__ok__":
        return ("rejected", line, "line_total machine refused: " + str(line))
    if line != 2500:
        return ("rejected", "ORACLE_UNCHECKED_ADD",
                "line_total(2,1250)=" + str(line) + " expected 2500")
    st, empty = machine("app.create.subtotal_pair",
                        {"q1": 0, "u1": 0, "q2": 0, "u2": 0})
    if st != "__ok__" or empty != 0:
        return ("rejected", "ORACLE_EMPTY_NONZERO",
                "empty subtotal=" + str(empty) + " expected 0")
    st, numer = machine("app.create.tax_numerator",
                        {"subtotal_cents": 2500, "basis_points": 725})
    if st != "__ok__":
        return ("rejected", numer, "tax_numerator machine refused: " + str(numer))
    if numer != 1812500:
        return ("rejected", "ORACLE_ROUND_UP_TAX",
                "tax_numerator(2500,725)=" + str(numer) + " expected 1812500")
    if value != -2:
        return ("rejected", "ORACLE_OVERFLOW_VALUE",
                "main=" + str(value) + " expected -2 wrapped MAX*2")
    return ("accepted", None,
            "check ok; line_total 2500; empty 0; numerator 1812500; wrapped MAX*2 -2")


def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_dir():
        _, code = fail_harness("HARNESS_BAD_ARGS", "oracle needs a fixture directory")
        return code
    fixture = Path(sys.argv[1]).resolve()
    program = fixture / "program.sley"
    if not program.is_file():
        _, code = fail_harness("HARNESS_MISSING_PROGRAM", "program.sley not found")
        return code
    sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
    try:
        from bench.legacy.task_runner import run_session
        from bench.legacy.task_runner import TaskRunnerError
    except Exception as exc:
        _, code = fail_harness("HARNESS_IMPORT", "task_runner import: " + str(exc)[:150])
        return code
    try:
        verdict = run_session(lambda s: judge(s, program), timeout_seconds=TIMEOUT)
    except RuntimeError as exc:
        _, code = fail_harness("HARNESS_CHILD", str(exc)[:150])
        return code
    except Exception as exc:
        _, code = fail_harness("HARNESS_CRASH", (type(exc).__name__ + ": " + str(exc))[:150])
        return code
    if verdict[0] not in ("accepted", "rejected"):
        _, code = fail_harness("HARNESS_VERDICT", "bad verdict")
        return code
    return emit(*verdict)


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Legacy oracle for S2B-TYPE-001 (JobState tagged state via record+discriminator)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-TYPE-001"
ARM = "legacy"
TIMEOUT = 240.0
CASES = [
    ({"state": "queued", "code": 0}, "queued"),
    ({"state": "running", "code": 0}, "running"),
    ({"state": "succeeded", "code": 0}, "succeeded"),
    ({"state": "failed", "code": 7}, "failed:7"),
]


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
    query = s.query(prog, kind="all")
    if query["outcome"] != "completed":
        raise RuntimeError("query " + query["outcome"])
    qrep = query["report"] or {}
    fields = {}
    for t in (qrep.get("types") or []):
        if t.get("id") == "type:app.jobs.JobState":
            fields = {f.get("name"): f.get("type") for f in (t.get("fields") or [])}
    if not fields:
        return ("rejected", "ORACLE_TYPE_MISSING", "type:app.jobs.JobState not found")
    if set(fields) != {"state", "code"} or fields.get("state") != "Text" or fields.get("code") != "Int":
        return ("rejected", "ORACLE_BOOL_COMPAT_FIELD",
                "JobState fields=" + json.dumps(fields) + " expected {state:Text,code:Int}")
    task_ids = {t.get("id") for t in (qrep.get("tasks") or [])}
    for name in ("queued", "running", "succeeded", "failed", "describe"):
        if "task:app.jobs." + name not in task_ids:
            return ("rejected", "ORACLE_CONSTRUCTOR_MISSING", "missing task:app.jobs." + name)
    for job, want in CASES:
        seen = []
        for _ in ("first", "second"):
            resp = s.machine_invoke(prog, "app.jobs.describe", {"job": job})
            if resp["outcome"] != "completed":
                raise RuntimeError("machine " + resp["outcome"])
            rep = resp["report"] or {}
            if rep.get("status") != "ok":
                return ("rejected", "ORACLE_EXECUTION", "describe machine refused")
            if rep.get("result") != want:
                return ("rejected", "ORACLE_MISSING_CASE",
                        "describe(" + job["state"] + ")=" + str(rep.get("result")) +
                        " expected " + want)
            seen.append(canon(rep.get("result")))
        if seen[0] != seen[1]:
            return ("rejected", "ORACLE_NONDETERMINISTIC", "repeat execution diverged")
    return ("accepted", None,
            "4 variants constructed+handled; Failed round-trips code; deterministic")


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

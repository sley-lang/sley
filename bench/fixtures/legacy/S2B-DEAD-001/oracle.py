#!/usr/bin/env python3
"""Legacy oracle for S2B-DEAD-001 (unreachable retry block + unused helper gone)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-DEAD-001"
ARM = "legacy"
TIMEOUT = 240.0
EXPECTED_MAIN = 42


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


def dead_findings(lint_report):
    ids = set()
    for f in (lint_report or {}).get("findings") or []:
        if f.get("id") in ("UNUSED_PRIVATE_TASK", "UNREACHABLE_STATEMENT"):
            ids.add(f.get("id"))
    return ids


def judge(s, fixture):
    prog = str(fixture / "program.sley")
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
    tasks = {t.get("id"): t for t in (qrep.get("tasks") or [])}
    scale = tasks.get("task:app.prune.scale")
    if scale is None or not scale.get("exported"):
        return ("rejected", "ORACLE_PUBLIC_DELETED", "exported task:app.prune.scale is gone")
    if [t.get("name") for t in scale.get("takes", [])] != ["value"]:
        return ("rejected", "ORACLE_SIGNATURE_CHANGED", "scale signature changed")
    lint = s.lint(prog)
    if lint["outcome"] != "completed":
        raise RuntimeError("lint " + lint["outcome"])
    bad = dead_findings(lint["report"])
    if bad:
        return ("rejected", "ORACLE_DEAD_REMAINS", "dead findings remain: " + ",".join(sorted(bad)))
    run = s.run(prog)
    if run["outcome"] != "completed":
        raise RuntimeError("run " + run["outcome"])
    run_report = run["report"] or {}
    value = (run_report.get("value") or {}).get("value")
    if run_report.get("status") != "passed" or value != EXPECTED_MAIN:
        return ("rejected", "ORACLE_DIGEST_CHANGED",
                "observation=" + str(value) + " expected " + str(EXPECTED_MAIN))
    effects = sorted({e for t in tasks.values() for e in (t.get("effects") or [])})
    before_path = fixture / "before.sley"
    if before_path.is_file():
        blint = s.lint(str(before_path))
        if blint["outcome"] != "completed":
            raise RuntimeError("lint " + blint["outcome"])
        bbad = dead_findings(blint["report"])
        if "UNUSED_PRIVATE_TASK" not in bbad or "UNREACHABLE_STATEMENT" not in bbad:
            return ("rejected", "ORACLE_BEFORE_NOT_DEAD",
                    "before.sley lacks the dead shapes: " + ",".join(sorted(bbad)))
        brun = s.run(str(before_path))
        if brun["outcome"] != "completed":
            raise RuntimeError("run " + brun["outcome"])
        brep = brun["report"] or {}
        if canon((brep.get("value") or {}).get("value")) != canon(value):
            return ("rejected", "ORACLE_DIGEST_CHANGED", "before/after observations differ")
        bquery = s.query(str(before_path), kind="all")
        beff = sorted({e for t in (bquery["report"] or {}).get("tasks", [])
                       for e in (t.get("effects") or [])})
        if beff != effects:
            return ("rejected", "ORACLE_EFFECTS_EXPANDED",
                    "effect closure changed: " + str(beff) + " vs " + str(effects))
    return ("accepted", None,
            "dead block+helper gone; scale intact; observation 42; effects " + str(effects))


def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_dir():
        return harness("HARNESS_BAD_ARGS", "oracle needs a fixture directory")
    fixture = Path(sys.argv[1]).resolve()
    if not (fixture / "program.sley").is_file():
        return harness("HARNESS_MISSING_PROGRAM", "program.sley not found")
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

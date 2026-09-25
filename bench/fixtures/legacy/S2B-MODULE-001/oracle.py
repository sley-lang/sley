#!/usr/bin/env python3
"""Legacy oracle for S2B-MODULE-001 (checksum moves to integrity namespace)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-MODULE-001"
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


def harness(code, detail):
    print(json.dumps({"status": "harness_error", "code": code, "detail": detail,
                      "task_id": TASK_ID, "arm": ARM}))
    return 2


def canon(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def checked(s, prog):
    rep = s.check(prog)
    if rep["outcome"] != "completed":
        raise RuntimeError("check " + rep["outcome"])
    return rep["report"] or {}


def judge(s, fixture):
    prog = str(fixture / "program.sley")
    report = checked(s, prog)
    if report.get("status") != "ok":
        diags = report.get("diagnostics") or []
        code = diags[0].get("id", "ORACLE_CHECK_FAILED") if diags else "ORACLE_CHECK_FAILED"
        return ("rejected", code, "check refused fixture: " + code)
    query = s.query(prog, kind="all")
    if query["outcome"] != "completed":
        raise RuntimeError("query " + query["outcome"])
    qrep = query["report"] or {}
    ids = [t.get("id") for t in (qrep.get("tasks") or [])]
    if "task:app.integrity.checksum" not in ids:
        return ("rejected", "ORACLE_NAMESPACE",
                "task:app.integrity.checksum missing; tasks=" + str(len(ids)))
    if "task:app.util.checksum" in ids:
        return ("rejected", "ORACLE_STALE_BINDING", "old app.util.checksum binding remains")
    inbound = [c for c in (qrep.get("calls") or [])
               if c.get("target") == "app.integrity.checksum"]
    if len(inbound) != 6:
        return ("rejected", "ORACLE_REFERENCE_COUNT",
                "integrity.checksum inbound=" + str(len(inbound)) + " expected 6")
    after = s.machine_invoke(prog, "app.shop.main", {})
    if after["outcome"] != "completed":
        raise RuntimeError("machine " + after["outcome"])
    arep = after["report"] or {}
    if arep.get("status") != "ok":
        return ("rejected", "ORACLE_EXECUTION", "after machine refused")
    before_path = fixture / "before.sley"
    if before_path.is_file():
        breport = checked(s, str(before_path))
        if breport.get("status") != "ok":
            return ("rejected", "ORACLE_BEFORE_CHECK", "before.sley does not check")
        before = s.machine_invoke(str(before_path), "app.shop.main", {})
        if before["outcome"] != "completed":
            raise RuntimeError("machine " + before["outcome"])
        brep = before["report"] or {}
        if brep.get("status") != "ok":
            return ("rejected", "ORACLE_EXECUTION", "before machine refused")
        if canon(brep.get("result")) != canon(arep.get("result")):
            return ("rejected", "ORACLE_DIGEST_CHANGED",
                    "observation digest differs before/after move")
        detail = "6 refs resolve; digest byte-equal before/after: " + canon(arep.get("result"))
    else:
        detail = "6 refs resolve; observation: " + canon(arep.get("result"))
    return ("accepted", None, detail)


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

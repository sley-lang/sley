#!/usr/bin/env python3
"""Legacy oracle for S2B-STALE-001 (stale second commit rejected)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-STALE-001"
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


def commit(s, prog, stored, base, value):
    resp = s.machine_invoke(prog, "app.stale.commit_if_fresh",
                            {"stored_version": stored, "base_version": base,
                             "value": value})
    if resp["outcome"] != "completed":
        raise RuntimeError("machine " + resp["outcome"])
    rep = resp["report"] or {}
    if rep.get("status") != "ok":
        err = rep.get("error") or {}
        return ("__refused__", err.get("code", "ORACLE_MACHINE_FAILED"))
    return ("__ok__", rep.get("result"))


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
    st, first = commit(s, prog, 1, 1, 100)
    if st != "__ok__" or first != {"ok": 100}:
        return ("rejected", "ORACLE_FIRST_COMMIT",
                "A commit=" + canon(first) + " expected {ok:100}")
    st, second = commit(s, prog, 2, 1, 200)
    if st != "__ok__":
        return ("rejected", second, "B commit machine refused: " + str(second))
    if second == {"ok": 200}:
        return ("rejected", "ORACLE_LAST_WRITE_WINS",
                "B stale commit visibly succeeded: " + canon(second))
    if second != {"err": "STALE"}:
        return ("rejected", "ORACLE_STALE_SHAPE",
                "B commit=" + canon(second) + " expected {err:STALE}")
    st, third = commit(s, prog, 2, 2, 300)
    if st != "__ok__" or third != {"ok": 300}:
        return ("rejected", "ORACLE_REBASE_INVALID",
                "re-based C=" + canon(third) + " expected {ok:300}")
    return ("accepted", None, "A ok; B stale rejected; re-based C validates")


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

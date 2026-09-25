#!/usr/bin/env python3
"""Legacy oracle for S2B-PERF-001 (nested scan -> range-guarded scan)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-PERF-001"
ARM = "legacy"
TIMEOUT = 240.0
HAY = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
NEEDLES = [1, 2, 3, 12, 4, 5]
MIN_REDUCTION = 30.0


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


def measured(s, source, task):
    resp = s.machine_invoke(source, task, {"hay": HAY, "needles": NEEDLES})
    if resp["outcome"] != "completed":
        raise RuntimeError("machine " + resp["outcome"])
    rep = resp["report"] or {}
    if rep.get("status") != "ok":
        err = rep.get("error") or {}
        return ("__refused__", err.get("code", "ORACLE_MACHINE_FAILED"), 0)
    counters = rep.get("counters") or {}
    return ("__ok__", rep.get("result"), int(counters.get("steps", 0)))


def judge(s, fixture):
    slow = str(fixture / "slow.sley")
    fast = str(fixture / "fast.sley")
    for target in (slow, fast):
        check = s.check(target)
        if check["outcome"] != "completed":
            raise RuntimeError("check " + check["outcome"])
        if (check["report"] or {}).get("status") != "ok":
            return ("rejected", "ORACLE_SIDE_CHECK", Path(target).name + " does not check")
    for target in (slow, fast):
        query = s.query(target, kind="all")
        if query["outcome"] != "completed":
            raise RuntimeError("query " + query["outcome"])
        eff = {e for t in (query["report"] or {}).get("tasks", [])
               for e in (t.get("effects") or [])}
        if eff:
            return ("rejected", "ORACLE_EFFECTS_CHANGED",
                    Path(target).name + " effects=" + str(sorted(eff)))
    st, slow_result, slow_steps = measured(s, slow, "app.slowmem.slow_count")
    if st != "__ok__":
        return ("rejected", slow_result, "slow machine refused: " + str(slow_result))
    st, fast_result, fast_steps = measured(s, fast, "app.fastmem.range_count")
    if st != "__ok__":
        return ("rejected", fast_result, "fast machine refused: " + str(fast_result))
    if canon(slow_result) != canon(fast_result):
        return ("rejected", "ORACLE_OUTPUT_CHANGED",
                "outputs differ slow=" + canon(slow_result) + " fast=" + canon(fast_result) +
                " steps slow/fast=" + str(slow_steps) + "/" + str(fast_steps))
    if slow_steps <= 0 or fast_steps < 0:
        return ("rejected", "ORACLE_COUNTERS", "machine step counters missing")
    reduction = (slow_steps - fast_steps) * 100.0 / slow_steps
    if reduction < MIN_REDUCTION:
        return ("rejected", "ORACLE_INSUFFICIENT_REDUCTION",
                "reduction=" + ("%.1f" % reduction) + "% steps slow/fast=" +
                str(slow_steps) + "/" + str(fast_steps))
    return ("accepted", None,
            "outputs identical " + canon(slow_result) + "; steps " + str(slow_steps) +
            "->" + str(fast_steps) + " reduction " + ("%.1f" % reduction) +
            "%; no ceiling breach")


def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_dir():
        return harness("HARNESS_BAD_ARGS", "oracle needs a fixture directory")
    fixture = Path(sys.argv[1]).resolve()
    if not (fixture / "slow.sley").is_file() or not (fixture / "fast.sley").is_file():
        return harness("HARNESS_MISSING_PROGRAM", "slow.sley and fast.sley required")
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

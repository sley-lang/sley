#!/usr/bin/env python3
"""Legacy oracle for S2B-MERGE-001 (proven-disjoint branch merge)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-MERGE-001"
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


def run_value(s, target):
    rep = s.run(str(target))
    if rep["outcome"] != "completed":
        raise RuntimeError("run " + rep["outcome"])
    body = rep["report"] or {}
    if body.get("status") != "passed":
        diags = body.get("diagnostics") or []
        code = diags[0].get("id", "ORACLE_RUN_FAILED") if diags else "ORACLE_RUN_FAILED"
        return ("__refused__", code)
    return ("__ok__", (body.get("value") or {}).get("value"))


def judge(s, fixture):
    base = str(fixture / "base.sley")
    ours = str(fixture / "ours.sley")
    theirs = str(fixture / "theirs.sley")
    for target in (base, ours, theirs):
        check = s.check(target)
        if check["outcome"] != "completed":
            raise RuntimeError("check " + check["outcome"])
        if (check["report"] or {}).get("status") != "ok":
            return ("rejected", "ORACLE_SIDE_CHECK",
                    Path(target).name + " does not check")
    diff = s.graph_diff(base, ours, theirs)
    if diff["outcome"] != "completed":
        raise RuntimeError("graph-diff " + diff["outcome"])
    drep = diff["report"] or {}
    summary = drep.get("summary") or {}
    if drep.get("status") == "conflicted" or summary.get("conflict_count", 0) != 0:
        conflicts = drep.get("conflicts") or []
        classes = sorted({c.get("class", "?") for c in conflicts})
        return ("rejected", "ORACLE_MERGE_CONFLICT",
                "conflicts=" + str(summary.get("conflict_count")) + " classes=" + str(classes))
    if drep.get("status") != "passed":
        return ("rejected", "ORACLE_DIFF_STATUS",
                "graph-diff status=" + str(drep.get("status")))
    st, base_v = run_value(s, base)
    st2, ours_v = run_value(s, ours)
    st3, theirs_v = run_value(s, theirs)
    if st != "__ok__" or st2 != "__ok__" or st3 != "__ok__":
        return ("rejected", "ORACLE_EXECUTION", "side execution refused")
    program = fixture / "program.sley"
    if program.is_file():
        st4, merged_v = run_value(s, program)
        if st4 != "__ok__":
            return ("rejected", "ORACLE_EXECUTION", "merged execution refused")
        if merged_v != 6310 or ours_v != 6308 or theirs_v != 6309 or base_v != 6307:
            return ("rejected", "ORACLE_CHANGE_LOST",
                    "base/ours/theirs/merged=" + str([base_v, ours_v, theirs_v, merged_v]))
        again = run_value(s, program)[1]
        if again != merged_v:
            return ("rejected", "ORACLE_NONDETERMINISTIC", "merged root not deterministic")
    swapped = s.graph_diff(base, theirs, ours)
    if swapped["outcome"] != "completed":
        raise RuntimeError("graph-diff " + swapped["outcome"])
    srep = swapped["report"] or {}
    if srep.get("status") != "passed" or \
            (srep.get("summary") or {}).get("conflict_count", 0) != 0:
        return ("rejected", "ORACLE_NOT_COMMUTATIVE", "swapped sides do not pass clean")
    if (srep.get("summary") or {}).get("node_change_count") != summary.get("node_change_count"):
        return ("rejected", "ORACLE_NOT_COMMUTATIVE", "swapped change set differs")
    return ("accepted", None,
            "disjoint merge passed; both changes preserved 6307->6310; order-independent")


def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_dir():
        return harness("HARNESS_BAD_ARGS", "oracle needs a fixture directory")
    fixture = Path(sys.argv[1]).resolve()
    for name in ("base.sley", "ours.sley", "theirs.sley"):
        if not (fixture / name).is_file():
            return harness("HARNESS_MISSING_PROGRAM", name + " not found")
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

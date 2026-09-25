#!/usr/bin/env python3
"""Legacy oracle for S2B-TEST-001 (checked division boundary tests)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-TEST-001"
ARM = "legacy"
TIMEOUT = 240.0
REQUIRED_CASES = ["divide_by_zero", "signed_overflow", "success"]


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


def check_ids(s, target):
    rep = s.check(str(target))
    if rep["outcome"] != "completed":
        raise RuntimeError("check " + rep["outcome"])
    body = rep["report"] or {}
    if body.get("status") != "error":
        return None
    return sorted(d.get("id") for d in (body.get("diagnostics") or []))


def judge(s, fixture):
    program = fixture / "program.sley"
    manifest_path = fixture / "sley.test.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as exc:
        return ("rejected", "ORACLE_MANIFEST_UNREADABLE", "manifest unreadable: " + str(exc)[:100])
    case_ids = sorted(c.get("id") for c in manifest.get("cases", []))
    if case_ids != REQUIRED_CASES:
        return ("rejected", "ORACLE_CASE_MISSING",
                "manifest cases=" + str(case_ids) + " required " + str(REQUIRED_CASES))
    kinds = {c.get("id"): c.get("kind") for c in manifest.get("cases", [])}
    if kinds != {"success": "table", "divide_by_zero": "diagnostic",
                 "signed_overflow": "diagnostic"}:
        return ("rejected", "ORACLE_CASE_KIND", "manifest case kinds drifted")
    test = s.test(str(fixture))
    if test["outcome"] != "completed":
        raise RuntimeError("test " + test["outcome"])
    if test["return_code"] != 2 or \
            "failed sley.test.manifest.v1 validation" not in test["stderr_text"]:
        return ("rejected", "ORACLE_TEST_DRIFT",
                "sley test refusal lifted rc=" + str(test["return_code"]))
    check = s.check(str(program))
    if check["outcome"] != "completed":
        raise RuntimeError("check " + check["outcome"])
    if (check["report"] or {}).get("status") != "ok":
        return ("rejected", "ORACLE_IMPL_CHECK", "implementation does not check")
    success = [c for c in manifest["cases"] if c.get("id") == "success"][0]
    for row in success.get("rows", []):
        resp = s.machine_invoke(str(program), "app.divider.add_one", row.get("input", {}))
        if resp["outcome"] != "completed":
            raise RuntimeError("machine " + resp["outcome"])
        rep = resp["report"] or {}
        if rep.get("status") != "ok" or rep.get("result") != row.get("expected_result"):
            return ("rejected", "ORACLE_IMPL_CHANGED",
                    "row " + str(row.get("id")) + "=" + str(rep.get("result")) +
                    " expected " + str(row.get("expected_result")))
    for name in ("divzero.sley", "overflow.sley"):
        ids = check_ids(s, fixture / name)
        if ids != ["UNSUPPORTED_RAW_EXPRESSION"]:
            return ("rejected", "ORACLE_DIVISION_SURFACE",
                    name + " diagnostics=" + str(ids))
    return ("accepted", None,
            "3 cases bound; success rows execute; division forms statically refused; test wall filed")


def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_dir():
        return harness("HARNESS_BAD_ARGS", "oracle needs a fixture directory")
    fixture = Path(sys.argv[1]).resolve()
    for name in ("program.sley", "sley.test.json", "divzero.sley", "overflow.sley"):
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

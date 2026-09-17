#!/usr/bin/env python3
"""Legacy oracle for S2B-EFFECT-001 (explicit FileRead effect closure)."""

import json
import sys
from pathlib import Path

TASK_ID = "S2B-EFFECT-001"
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
    eff = {t.get("id"): (t.get("effects") or []) for t in (qrep.get("tasks") or [])}
    if eff.get("task:app.cfgeffect.fetch_config") != ["FileRead"]:
        return ("rejected", "ORACLE_EFFECT_MISSING",
                "fetch_config effects=" + str(eff.get("task:app.cfgeffect.fetch_config")))
    if "FileRead" not in eff.get("task:app.cfgeffect.render_config", []):
        return ("rejected", "ORACLE_CLOSURE_MISSING",
                "render_config effects=" + str(eff.get("task:app.cfgeffect.render_config")))
    if eff.get("task:app.cfgeffect.pure_greet") != []:
        return ("rejected", "ORACLE_SIBLING_IMPURE",
                "pure_greet effects=" + str(eff.get("task:app.cfgeffect.pure_greet")))
    nocap = s.run(prog)
    if nocap["outcome"] != "completed":
        raise RuntimeError("run " + nocap["outcome"])
    nrep = nocap["report"] or {}
    ndiags = nrep.get("diagnostics") or []
    if nocap["return_code"] == 0 or nrep.get("schema") != "sley.diagnostics.report.v0" or \
            not ndiags or ndiags[0].get("id") != "RUNTIME_CAPABILITY_REQUIRED":
        return ("rejected", "ORACLE_CAP_REQUIRED_MISSING",
                "uncapped run did not yield RUNTIME_CAPABILITY_REQUIRED")
    if nocap["stderr_text"].strip() != "operation failed":
        return ("rejected", "ORACLE_STDERR_DRIFT",
                "uncapped stderr=" + repr(nocap["stderr_text"].strip()))
    capped = s.run(prog, "FileRead")
    if capped["outcome"] != "completed":
        raise RuntimeError("run " + capped["outcome"])
    crep = capped["report"] or {}
    cdiags = crep.get("diagnostics") or []
    if not cdiags or cdiags[0].get("id") != "RUNTIME_UNSUPPORTED_SOURCE":
        return ("rejected", "ORACLE_WALL_MISSING",
                "capped run did not hit RUNTIME_UNSUPPORTED_SOURCE wall")
    pure = s.machine_invoke(prog, "app.cfgeffect.pure_greet", {"name": "Ada"})
    if pure["outcome"] != "completed":
        raise RuntimeError("machine " + pure["outcome"])
    prep = pure["report"] or {}
    if prep.get("status") != "ok" or prep.get("result") != "hello, Ada":
        return ("rejected", "ORACLE_PURE_VALUE",
                "pure_greet=" + str(prep.get("result")) + " expected hello, Ada")
    return ("accepted", None,
            "FileRead declared; caller closure carries it; sibling pure; capped wall documented")


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

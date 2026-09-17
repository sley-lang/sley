#!/usr/bin/env python3
"""Legacy oracle for S2B-SIG-001 (net_total gains tax_basis_points)."""

import json
import re
import sys
from pathlib import Path

TASK_ID = "S2B-SIG-001"
ARM = "legacy"
TIMEOUT = 240.0
WANT = {"invoice_a": 1815000, "invoice_b": 501000, "invoice_c": 300, "main": 2316300}
INT_RE = re.compile(r"^-?[0-9]+$")


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
    text = program.read_text(encoding="utf-8")
    sites = re.findall(r"call\s+net_total\(([^)]*)\)", text)
    if len(sites) != 3:
        return ("rejected", "ORACLE_CALLER_COUNT",
                "net_total call sites=" + str(len(sites)) + " expected 3")
    for site in sites:
        args = [a.strip() for a in site.split(",")]
        if len(args) != 2 or not all(INT_RE.match(a) for a in args):
            return ("rejected", "ORACLE_AMBIENT_DEFAULT",
                    "non-explicit tax arg at call net_total(" + site.strip() + ")")
    query = s.query(prog, kind="all")
    if query["outcome"] != "completed":
        raise RuntimeError("query " + query["outcome"])
    qrep = query["report"] or {}
    nets = [t for t in (qrep.get("tasks") or []) if t.get("id") == "task:app.totals.net_total"]
    if not nets:
        return ("rejected", "ORACLE_TARGET_MISSING", "task:app.totals.net_total not found")
    takes = [(t.get("name"), t.get("type")) for t in nets[0].get("takes", [])]
    if takes != [("subtotal", "Int"), ("tax_basis_points", "Int")]:
        return ("rejected", "ORACLE_SIGNATURE",
                "net_total takes=" + json.dumps(takes) + " expected typed tax_basis_points")
    inbound = [c for c in (qrep.get("calls") or [])
               if c.get("target") == "app.totals.net_total"]
    if len(inbound) != 3:
        return ("rejected", "ORACLE_CALLER_COUNT",
                "query inbound to net_total=" + str(len(inbound)) + " expected 3")
    for name, want in (("invoice_a", WANT["invoice_a"]), ("invoice_b", WANT["invoice_b"]),
                       ("invoice_c", WANT["invoice_c"])):
        resp = s.machine_invoke(prog, "app.totals." + name, {})
        if resp["outcome"] != "completed":
            raise RuntimeError("machine " + resp["outcome"])
        rep = resp["report"] or {}
        if rep.get("status") != "ok" or rep.get("result") != want:
            return ("rejected", "ORACLE_CALLER_VALUE",
                    name + "=" + str(rep.get("result")) + " expected " + str(want))
    run = s.run(prog)
    if run["outcome"] != "completed":
        raise RuntimeError("run " + run["outcome"])
    run_report = run["report"] or {}
    main_value = (run_report.get("value") or {}).get("value")
    if run_report.get("status") != "passed" or main_value != WANT["main"]:
        return ("rejected", "ORACLE_MAIN_VALUE",
                "main=" + str(main_value) + " expected " + str(WANT["main"]))
    return ("accepted", None,
            "check ok; net_total(subtotal,tax_basis_points); 3 explicit callers; main 2316300")


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

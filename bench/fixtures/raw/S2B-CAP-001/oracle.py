#!/usr/bin/env python3
"""Raw-arm oracle. Stdlib only. See bench/fixtures/README.md."""
import json
import subprocess
import sys
from pathlib import Path

TASK_ID = "S2B-CAP-001"
ARM = "raw"
TIMEOUT_S = 60
POSITIVE_NOTE = ("exact-object scope config/settings.bin; sibling denied "
                 "CAP_SCOPE_MISMATCH")


def emit(status, code, detail, exit_code):
    detail_line = " ".join(str(detail).split())
    print(json.dumps({"status": status, "code": code,
                      "detail": detail_line[:500], "task_id": TASK_ID,
                      "arm": ARM}))
    sys.exit(exit_code)


def first_failure(text, want=None):
    lines = [l.strip() for l in text.splitlines() if l.strip()]
    tag = ""
    for l in lines:
        if l.startswith("FAIL:") or l.startswith("ERROR:"):
            tag = l[:160]
            break
    errs = [l for l in lines
            if l[:1].isupper() and ("Error:" in l or l.endswith("not raised"))
            and not l.startswith(("FAIL:", "ERROR:", "Traceback"))]
    if want and errs:
        low = str(want).lower().replace("_", "")
        for l in errs:
            if low in l.lower().replace("_", ""):
                return ((tag + " | " + l[:160]) if tag else l[:200])
    if errs:
        return ((tag + " | " + errs[0][:160]) if tag else errs[0][:200])
    return tag if tag else (lines[-1][:200] if lines else "no output")


def main():
    if len(sys.argv) != 2:
        emit("rejected", "HARNESS_ERROR", "usage: oracle.py <fixture-dir>", 2)
    base = Path(__file__).resolve().parent
    cand = Path(sys.argv[1])
    if not cand.is_absolute():
        maybe = Path.cwd() / cand
        fdir = maybe if maybe.exists() else base / cand
    else:
        fdir = cand
    fdir = fdir.resolve()
    try:
        rel = fdir.relative_to(base.resolve())
    except ValueError:
        emit("rejected", "HARNESS_ERROR", "fixture-dir outside task dir", 2)
    if rel.parts == ("fixture",):
        role, name = "positive", None
    elif len(rel.parts) == 2 and rel.parts[0] == "negative":
        role, name = "negative", rel.parts[1]
    else:
        emit("rejected", "HARNESS_ERROR",
             "fixture-dir must be fixture or negative/<name>", 2)
    for need in ("program.py", "test_program.py"):
        if not (fdir / need).is_file():
            emit("rejected", "HARNESS_ERROR", "missing file: " + need, 2)
    try:
        expect = json.loads((base / "expect.json").read_text())
    except Exception:
        emit("rejected", "HARNESS_ERROR", "expect.json unreadable", 2)
    try:
        proc = subprocess.run(
            [sys.executable, "-m", "unittest", "test_program"],
            cwd=str(fdir), capture_output=True, text=True,
            timeout=TIMEOUT_S)
    except subprocess.TimeoutExpired:
        emit("rejected", "HARNESS_ERROR",
             "unittest timeout over %ds" % TIMEOUT_S, 2)
    log = (proc.stderr or "") + "\n" + (proc.stdout or "")
    if role == "positive":
        if proc.returncode == 0:
            emit("accepted", None, POSITIVE_NOTE + " suite=PASS rc=0", 0)
        emit("rejected", "ORACLE_POSITIVE_FAILURE",
             "positive suite FAILED rc=%d evidence=%s"
             % (proc.returncode, first_failure(log)), 1)
    negatives = expect.get("negatives", {})
    if name not in negatives:
        emit("rejected", "HARNESS_ERROR",
             "negative not in expect.json: " + str(name), 2)
    if proc.returncode == 0:
        emit("accepted", None,
             "negative %s unexpectedly PASSED (builder error)" % name, 0)
    emit("rejected", negatives[name],
         "negative %s rejected expected=%s evidence=%s"
         % (name, negatives[name], first_failure(log, negatives[name])), 1)


main()

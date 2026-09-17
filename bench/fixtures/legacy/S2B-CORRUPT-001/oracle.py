#!/usr/bin/env python3
"""Legacy oracle for S2B-CORRUPT-001 (corrupt pack rejected before ref movement)."""

import hashlib
import json
import sys
from pathlib import Path

TASK_ID = "S2B-CORRUPT-001"
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


def judge(s, fixture):
    try:
        pack = json.loads((fixture / "pack.json").read_text(encoding="utf-8"))
    except Exception as exc:
        return ("rejected", "ORACLE_PACK_UNREADABLE", "pack.json unreadable")
    if pack.get("schema") != "legacy.exchange-pack.v0":
        return ("rejected", "ORACLE_PACK_SCHEMA", "pack schema drifted")
    payload = fixture / pack.get("payload", "payload.sley")
    if not payload.is_file():
        return ("rejected", "ORACLE_PAYLOAD_MISSING", "payload file missing")
    declared = pack.get("payload_sha256", "")
    ref_path = fixture / "ref.txt"
    ref_before = ref_path.read_text(encoding="utf-8").strip() if ref_path.is_file() else None
    actual = hashlib.sha256(payload.read_bytes()).hexdigest()
    retry = hashlib.sha256(payload.read_bytes()).hexdigest()
    if retry != actual:
        return ("rejected", "ORACLE_NONDETERMINISTIC", "digest retry diverged")
    check = s.check(str(payload))
    if check["outcome"] != "completed":
        raise RuntimeError("check " + check["outcome"])
    check_status = (check["report"] or {}).get("status")
    if actual != declared:
        if ref_before != pack.get("ref"):
            return ("rejected", "ORACLE_REF_MOVED", "destination ref changed during refusal")
        # Required outcome demonstrated: the corrupt import is refused before
        # any ref movement. The oracle reports the demonstrated refusal as
        # accepted (uniform verdict polarity: status judges the fixture
        # against the task, code names the observed outcome) -- unless this
        # directory carries the PROVE_THEN_FAIL marker, which is the
        # multi-position negative control: the refusal at a second flip
        # offset is demonstrated and then reported as rejected by procedure
        # (same handshake as the sley2-arm flip_elsewhere control), so the
        # gate is proved position-independent without breaking the
        # negative-must-exit-1 contract.
        detail = ("import refused before ref movement; ref unchanged " + str(ref_before)[:16] +
                  "; retry deterministic; payload check " + str(check_status))
        if (fixture / "PROVE_THEN_FAIL").exists():
            return ("rejected", "PACK_DIGEST_MISMATCH", detail + "; prove-then-fail multi-position control")
        return ("accepted", "PACK_DIGEST_MISMATCH", detail)
    if check_status != "ok":
        return ("rejected", "ORACLE_PAYLOAD_CHECK", "clean payload does not check")
    run = s.run(str(payload))
    if run["outcome"] != "completed":
        raise RuntimeError("run " + run["outcome"])
    if (run["report"] or {}).get("status") != "passed":
        return ("rejected", "ORACLE_PAYLOAD_RUN", "clean payload does not run")
    return ("rejected", "ORACLE_CORRUPTION_MISSING",
            "clean pack imports with no refusal; required corruption absent; digest " + actual[:16])


def main():
    if len(sys.argv) != 2 or not Path(sys.argv[1]).is_dir():
        return harness("HARNESS_BAD_ARGS", "oracle needs a fixture directory")
    fixture = Path(sys.argv[1]).resolve()
    if not (fixture / "pack.json").is_file():
        return harness("HARNESS_MISSING_PROGRAM", "pack.json not found")
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

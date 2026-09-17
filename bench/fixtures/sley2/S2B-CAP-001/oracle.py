#!/usr/bin/env python3
"""S3 G3 oracle: bench/fixtures/sley2/S2B-CAP-001 (excluded).

No execution fixture exists by contract. This oracle reproduces the pinned
refusal against the real engine: it attempts the capability-gated
adapter-invoke operation through the real restricted lowering path
(`s3_g3_cap_refusal_pin` in crates/sley-vm/tests/s3_g3_perf.rs) via
`cargo test --offline` and requires the refusal to be exactly
`VM_LOWER_OPCODE_UNSUPPORTED` — the same code recorded in excluded.json.
Verdict rejected/exit 1 only then.

If the engine ever widened (the operation lowered, or a different code
appeared), the pin test fails or the code mismatches and this oracle reports
a harness error (exit 2): the guard against silent widening. Stdlib only.
"""

import json
import subprocess
import sys
from pathlib import Path

TASK_ID = "S2B-CAP-001"
ARM = "sley2"
PIN_TEST = "s3_g3_cap_refusal_pin"
TIMEOUT_S = 300


def emit(status, code, detail):
    print(json.dumps({
        "status": status,
        "code": code,
        "detail": detail,
        "task_id": TASK_ID,
        "arm": ARM,
    }))
    sys.stdout.flush()


def fail_harness(detail):
    emit("harness_error", None, "HARNESS-ERROR " + detail)
    sys.exit(2)


def repo_root(start):
    node = Path(start).resolve()
    for _ in range(12):
        if (node / "Cargo.toml").is_file():
            return node
        node = node.parent
    return None


def main(argv):
    if len(argv) != 2:
        emit("harness_error", None, "HARNESS-ERROR usage: oracle.py <fixture-dir>")
        sys.exit(2)
    here = Path(__file__).resolve().parent
    root = repo_root(Path(argv[1]).resolve())
    if root is None:
        emit("harness_error", None, "HARNESS-ERROR no workspace root above fixture dir")
        sys.exit(2)
    try:
        excluded = json.loads((here / "excluded.json").read_text())
        expect = json.loads((here / "expect.json").read_text())
    except (OSError, ValueError) as exc:
        fail_harness("unreadable excluded/expect json: %s" % exc)
    if (excluded.get("refusal_code") != "VM_LOWER_OPCODE_UNSUPPORTED"
            or expect.get("positive") != "rejected"
            or expect.get("exclusion") is not True):
        fail_harness("exclusion pin mismatch in excluded/expect json")
    try:
        proc = subprocess.run(
            ["cargo", "test", "--offline", "-p", "sley-vm", "--test",
             "s3_g3_perf", PIN_TEST, "--", "--exact", "--nocapture"],
            cwd=str(root),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=TIMEOUT_S,
        )
    except (subprocess.TimeoutExpired, OSError) as exc:
        fail_harness("child failed: %s" % exc)
    if proc.returncode != 0:
        # The operation lowered (widened) or the harness broke: either way
        # the pinned refusal no longer reproduces — never call it accepted.
        fail_harness("refusal pin test exited %d" % proc.returncode)
    if "refusal_code=VM_LOWER_OPCODE_UNSUPPORTED" not in proc.stdout:
        fail_harness("refusal PIN lacks the pinned code")
    emit("rejected", "VM_LOWER_OPCODE_UNSUPPORTED",
         "E7 capability execution refused: VM_LOWER_OPCODE_UNSUPPORTED")
    sys.exit(1)


if __name__ == "__main__":
    main(sys.argv)

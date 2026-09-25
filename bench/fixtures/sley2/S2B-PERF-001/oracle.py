#!/usr/bin/env python3
"""S3 G3 oracle: bench/fixtures/sley2/S2B-PERF-001.

Positive (`fixture/`): loads fixture/fixture.json, re-runs the real
conformance test (`s3_g3_perf_fixture_conformance` in
crates/sley-vm/tests/s3_g3_perf.rs) via `cargo test --offline`, and requires
its PIN line to repeat the recorded instruction counts, fuel figures, and
output digest exactly. Verdict accepted/exit 0 only then.

Negative (`negative/<name>/`): loads negative/<name>/case.json, runs the
named Rust negative test, and requires exit 0 plus the expected code in its
PIN line. Verdict rejected/exit 1 with that code.

Anything else (missing files, child timeout/crash, PIN drift) is a harness
error: exit 2, never a verdict. Stdlib only.
"""

import json
import subprocess
import sys
from pathlib import Path

TASK_ID = "S2B-PERF-001"
ARM = "sley2"
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
    # NOTE: harness errors still print the verdict-shaped object (the runner
    # keys on exit code 2 to tell them apart); keep the object honest.
    sys.exit(2)


def repo_root(start):
    node = Path(start).resolve()
    for _ in range(12):
        if (node / "Cargo.toml").is_file():
            return node
        node = node.parent
    return None


def run_cargo(root, package, binary, test_name):
    try:
        proc = subprocess.run(
            ["cargo", "test", "--offline", "-p", package, "--test", binary,
             test_name, "--", "--exact", "--nocapture"],
            cwd=str(root),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=TIMEOUT_S,
        )
    except (subprocess.TimeoutExpired, OSError) as exc:
        return None, "child failed: %s" % exc
    return proc, None


def main(argv):
    if len(argv) != 2:
        emit("harness_error", None, "HARNESS-ERROR usage: oracle.py <fixture-dir>")
        sys.exit(2)
    target = Path(argv[1]).resolve()
    root = repo_root(target)
    if root is None:
        emit("harness_error", None, "HARNESS-ERROR no workspace root above fixture dir")
        sys.exit(2)
    here = Path(__file__).resolve().parent
    name = target.name
    parent = target.parent.name

    if name == "fixture" and target.is_dir():
        try:
            fixture = json.loads((target / "fixture.json").read_text())
        except (OSError, ValueError) as exc:
            fail_harness("unreadable fixture.json: %s" % exc)
        proc, err = run_cargo(root, "sley-vm", "s3_g3_perf",
                              "s3_g3_perf_fixture_conformance")
        if proc is None:
            fail_harness(err)
        if proc.returncode != 0:
            fail_harness("conformance test exited %d" % proc.returncode)
        out = proc.stdout
        want = [
            "before_instructions=%d" % fixture["before"]["instruction_count"],
            "before_fuel=%d" % fixture["before"]["fuel_used"],
            "after_instructions=%d" % fixture["after"]["instruction_count"],
            "after_fuel=%d" % fixture["after"]["fuel_used"],
            "output_digest=%s" % fixture["output_digest"],
        ]
        missing = [w for w in want if w not in out]
        if missing:
            fail_harness("PIN drift: %s" % ",".join(missing))
        emit("accepted", None,
             "nested-scan %d instr / map %d instr, digest %s" % (
                 fixture["before"]["instruction_count"],
                 fixture["after"]["instruction_count"],
                 fixture["output_digest"][:16]))
        sys.exit(0)

    if parent == "negative" and target.is_dir():
        try:
            case = json.loads((target / "case.json").read_text())
        except (OSError, ValueError) as exc:
            fail_harness("unreadable case.json: %s" % exc)
        try:
            expect = json.loads((here / "expect.json").read_text())
            pinned = expect["negatives"][name]
        except (OSError, ValueError, KeyError) as exc:
            fail_harness("expect.json lacks negative %s: %s" % (name, exc))
        if case.get("expected_code") != pinned or case.get("test") is None:
            fail_harness("case.json/expect.json handshake mismatch")
        proc, err = run_cargo(root, case.get("package", "sley-vm"),
                              case.get("binary", "s3_g3_perf"), case["test"])
        if proc is None:
            fail_harness(err)
        if proc.returncode != 0:
            fail_harness("negative test exited %d" % proc.returncode)
        if ("code=%s" % pinned) not in proc.stdout:
            fail_harness("negative PIN lacks code %s" % pinned)
        emit("rejected", pinned, "negative %s rejected with %s" % (name, pinned))
        sys.exit(1)

    fail_harness("unknown fixture dir: %s" % target)


if __name__ == "__main__":
    main(sys.argv)

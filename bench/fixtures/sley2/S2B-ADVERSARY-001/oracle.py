#!/usr/bin/env python3
"""S3 G3 oracle: bench/fixtures/sley2/S2B-ADVERSARY-001.

Positive (`fixture/`): loads fixture/fixture.json, re-runs the real
conformance test (`s3_g3_adversary_fixture_conformance` in
crates/sley-policy/tests/s3_g3_adversary.rs) via `cargo test --offline`,
and requires its PIN line to repeat the exact repair value, output digest,
and the before/after policy-root digests (read through the protected
`AcceptedPolicyRoot::root` binding). Verdict accepted/exit 0 only then.

Negatives (`negative/<name>/`): loads negative/<name>/case.json, runs the
named Rust negative test, and requires exit 0 plus the expected code in its
PIN line. Verdict rejected/exit 1 with that code.

Anything else (missing files, child timeout/crash, PIN drift) is a harness
error: exit 2, never a verdict. Stdlib only.
"""

import json
import subprocess
import sys
from pathlib import Path

TASK_ID = "S2B-ADVERSARY-001"
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
        proc, err = run_cargo(root, "sley-policy", "s3_g3_adversary",
                              "s3_g3_adversary_fixture_conformance")
        if proc is None:
            fail_harness(err)
        if proc.returncode != 0:
            fail_harness("conformance test exited %d" % proc.returncode)
        out = proc.stdout
        want = [
            "repair_value=%d" % fixture["repair"]["value"],
            "output_digest=%s" % fixture["output_digest"],
            "policy_root_before=%s" % fixture["policy_root_before"],
            "policy_root_after=%s" % fixture["policy_root_after"],
            "capabilities_granted=0",
        ]
        missing = [w for w in want if w not in out]
        if missing:
            fail_harness("PIN drift: %s" % ",".join(missing))
        emit("accepted", None,
             "repair %d exact, root %s unchanged, 0 grants" % (
                 fixture["repair"]["value"],
                 fixture["policy_root_before"][:16]))
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
        proc, err = run_cargo(root, case.get("package", "sley-policy"),
                              case.get("binary", "s3_g3_adversary"), case["test"])
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

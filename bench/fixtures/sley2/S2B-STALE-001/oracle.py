#!/usr/bin/env python3
"""S3 sley2 oracle (stdlib only, task-generic).

CLI contract: `python3 oracle.py <fixture-dir>` where <fixture-dir> is
`fixture` (positive) or `negative/<name>` (negative control).
Prints exactly one JSON object; exit 0 = accepted, 1 = rejected,
2 = harness error. Task identity comes from `expect.json`; the positive
test binary/name and evidence digests come from `fixture/fixture.json`;
each negative names its Rust test plus expected code in
`negative/<name>/case.json`.

Positive: re-verifies `fixture/fixture.json` digests (sha256 over the
canonical JSON of the `evidence` object) and re-runs the Rust conformance
test via `cargo test --offline` (bounded timeout 240 s).

Negatives handshake (see each `negative/<name>/HANDSHAKE.md`): the named
Rust test demonstrates the rejection against the real engine, prints
`S3_NEG_RESULT <CODE>` on success, then fails by design (nonzero cargo
exit). The oracle requires BOTH nonzero exit AND the exact code line to
record `rejected` with that code.
"""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

TIMEOUT_S = 240


def verdict(status, code, detail, task_id, arm):
    print(json.dumps({
        "status": status,
        "code": code,
        "detail": detail,
        "task_id": task_id,
        "arm": arm,
    }))
    sys.stdout.flush()


def find_workspace(start):
    current = start.resolve()
    for _ in range(8):
        cargo = current / "Cargo.toml"
        if cargo.is_file():
            try:
                text = cargo.read_text(encoding="utf-8")
            except OSError:
                text = ""
            if "sley-repo" in text and "[workspace]" in text:
                return current
        if current.parent == current:
            break
        current = current.parent
    return None


def run_cargo(workspace, test_binary, test_name, nocapture):
    cmd = ["cargo", "test", "--offline", "-p", "sley-repo",
           "--test", test_binary, test_name]
    if nocapture:
        cmd += ["--", "--ignored", "--nocapture"]
    try:
        proc = subprocess.run(
            cmd, cwd=str(workspace), capture_output=True, text=True,
            timeout=TIMEOUT_S,
        )
    except (subprocess.TimeoutExpired, OSError) as exc:
        return None, "cargo invocation failed: %s" % type(exc).__name__
    return proc, proc.stdout + proc.stderr


def check_positive(task_dir, workspace, expect, task_id, arm):
    fixture_path = task_dir / "fixture" / "fixture.json"
    try:
        fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return 2, None, "fixture.json unreadable: %s" % exc
    evidence = fixture.get("evidence")
    expected_digest = fixture.get("evidence_sha256")
    if not isinstance(evidence, dict) or not isinstance(expected_digest, str):
        return 2, None, "fixture.json missing evidence/digest"
    canonical = json.dumps(evidence, sort_keys=True, separators=(",", ":"))
    actual_digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
    if actual_digest != expected_digest:
        return 2, None, "fixture digest mismatch"
    if fixture.get("task_id") != task_id or fixture.get("arm") != arm:
        return 2, None, "fixture task/arm mismatch"
    if expect.get("positive") != "accepted":
        return 2, None, "expect.json positive is not accepted"
    binary = fixture.get("test_binary")
    conformance = fixture.get("conformance_test")
    if not binary or not conformance:
        return 2, None, "fixture missing test mapping"
    proc, output = run_cargo(workspace, binary, conformance, False)
    if proc is None:
        return 2, None, output
    tail = (output.strip().splitlines() or ["no output"])[-1][:160]
    if proc.returncode == 0:
        return 0, None, "positive conformance passed"
    if "test result: FAILED" in output or "panicked" in output:
        return 1, "ORACLE_CONFORMANCE_FAILED", "conformance failed: %s" % tail
    return 2, None, "cargo error: %s" % tail


def check_negative(task_dir, workspace, expect, name):
    expected_map = expect.get("negatives") or {}
    if name not in expected_map:
        return 2, None, "negative not listed in expect.json"
    case_path = task_dir / "negative" / name / "case.json"
    try:
        case = json.loads(case_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return 2, None, "case.json unreadable: %s" % exc
    test_name = case.get("test")
    expected = case.get("expected_code")
    if not test_name or not test_name.endswith("_neg_" + name):
        return 2, None, "case.json does not name this negative"
    if expected != expected_map[name]:
        return 2, None, "case.json code differs from expect.json"
    binary = (task_dir / "fixture" / "fixture.json")
    try:
        test_binary = json.loads(binary.read_text(encoding="utf-8")).get("test_binary")
    except (OSError, ValueError):
        test_binary = None
    if not test_binary:
        return 2, None, "fixture missing test mapping"
    proc, output = run_cargo(workspace, test_binary, test_name, True)
    if proc is None:
        return 2, None, output
    marker = "S3_NEG_RESULT %s" % expected
    if proc.returncode != 0 and marker in output:
        return 1, expected, "negative %s rejected with %s" % (name, expected)
    if proc.returncode == 0:
        return 2, None, "negative test unexpectedly passed"
    return 2, None, "negative handshake broken: code line missing"


def main(argv):
    task_dir = Path(__file__).resolve().parent
    try:
        expect = json.loads((task_dir / "expect.json").read_text(encoding="utf-8"))
        task_id = expect["task_id"]
        arm = expect["arm"]
    except (OSError, ValueError, KeyError) as exc:
        print(json.dumps({
            "status": "rejected", "code": None,
            "detail": "expect.json unreadable: %s" % exc,
            "task_id": "unknown", "arm": "sley2",
        }))
        return 2
    if len(argv) != 2:
        verdict("rejected", None, "usage: oracle.py <fixture-dir>", task_id, arm)
        return 2
    target = (Path(argv[1]).resolve() if Path(argv[1]).is_absolute()
              else (Path.cwd() / argv[1]).resolve())
    workspace = find_workspace(task_dir)
    if workspace is None:
        verdict("rejected", None, "workspace not found", task_id, arm)
        return 2
    if target == (task_dir / "fixture").resolve():
        code_out, code, detail = check_positive(task_dir, workspace, expect, task_id, arm)
    elif target.parent == (task_dir / "negative").resolve() and target.is_dir():
        code_out, code, detail = check_negative(task_dir, workspace, expect, target.name)
    else:
        verdict("rejected", None, "unknown fixture dir", task_id, arm)
        return 2
    if code_out == 2:
        # Harness errors are never verdicts: null code, exit 2.
        verdict("rejected", None, detail, task_id, arm)
        return 2
    verdict("accepted" if code_out == 0 else "rejected", code, detail, task_id, arm)
    return code_out


if __name__ == "__main__":
    sys.exit(main(sys.argv))

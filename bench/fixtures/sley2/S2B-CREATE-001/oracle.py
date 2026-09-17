#!/usr/bin/env python3
"""S3 sley2 oracle: re-verifies fixture digests + re-runs Rust conformance.

CLI contract: `python3 oracle.py <fixture-dir>` where <fixture-dir> is
`fixture` (positive) or `negative/<name>` (negative control).
Stdlib only. Prints exactly one JSON object; exit 0 accepted, 1 rejected,
2 harness error.
"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

TIMEOUT_S = 240


def fail(msg):
    print(json.dumps({"status": "harness_error", "code": None, "detail": msg[:200],
                      "task_id": "unknown", "arm": "sley2"}))
    return 2


def main():
    if len(sys.argv) != 2:
        return fail("usage: oracle.py <fixture-dir>")
    tdir = Path(__file__).resolve().parent
    try:
        expect = json.loads((tdir / "expect.json").read_text())
    except Exception as e:
        return fail(f"expect.json unreadable: {e}")
    task_id = expect.get("task_id", "?")
    arm = expect.get("arm", "sley2")
    fdir = Path(sys.argv[1])
    if not fdir.is_absolute():
        fdir = (Path.cwd() / fdir).resolve()
    try:
        rel = fdir.resolve().relative_to(tdir.resolve())
    except Exception:
        return fail("fixture-dir outside task dir")

    def out(status, code, detail, exit_code):
        print(json.dumps({"status": status, "code": code, "detail": detail[:200],
                          "task_id": task_id, "arm": arm}))
        return exit_code

    ws = tdir
    while not (ws / "Cargo.toml").exists():
        ws = ws.parent
        if ws == ws.parent:
            return out("harness_error", None, "workspace root not found", 2)

    if rel.parts[0] == "fixture":
        try:
            fx = json.loads((fdir / "fixture.json").read_text())
        except Exception as e:
            return out("harness_error", None, f"fixture.json unreadable: {e}", 2)
        try:
            raw = bytes.fromhex(fx["candidate_bytes_hex"])
        except Exception as e:
            return out("harness_error", None, f"candidate hex bad: {e}", 2)
        if hashlib.sha256(raw).hexdigest() != fx.get("candidate_bytes_sha256"):
            return out("harness_error", None, "candidate sha256 mismatch", 2)
        target = fx.get("test_target", "s3_g1_create")
        conf = fx.get("conformance_test", "")
        try:
            p = subprocess.run(
                ["cargo", "test", "--offline", "-q", "-p", "sley-repo",
                 "--test", target, conf],
                cwd=str(ws), capture_output=True, text=True, timeout=TIMEOUT_S)
        except subprocess.TimeoutExpired:
            return out("harness_error", None, "cargo test timeout", 2)
        except Exception as e:
            return out("harness_error", None, f"cargo spawn failed: {e}", 2)
        if p.returncode == 0:
            return out("accepted", None, f"conformance {conf} passed", 0)
        return out("harness_error", None,
                   f"conformance failed rc={p.returncode}", 2)
    elif rel.parts[0] == "negative" and len(rel.parts) == 2:
        name = rel.parts[1]
        expected = expect.get("negatives", {}).get(name)
        if expected is None:
            return out("harness_error", None, f"negative {name} not in expect.json", 2)
        try:
            case = json.loads((fdir / "case.json").read_text())
        except Exception as e:
            return out("harness_error", None, f"case.json unreadable: {e}", 2)
        target = case.get("test_target", "sley-repo")
        test_name = case.get("negative_test", "")
        code = case.get("expected_code", expected)
        if code != expected:
            return out("harness_error", None, "case.json code vs expect.json drift", 2)
        try:
            p = subprocess.run(
                ["cargo", "test", "--offline", "-q", "-p", "sley-repo",
                 "--test", target, test_name],
                cwd=str(ws), capture_output=True, text=True, timeout=TIMEOUT_S)
        except subprocess.TimeoutExpired:
            return out("harness_error", None, "cargo test timeout", 2)
        except Exception as e:
            return out("harness_error", None, f"cargo spawn failed: {e}", 2)
        if p.returncode == 0:
            return out("rejected", code,
                       f"negative {name} rejected with {code}", 1)
        return out("harness_error", None,
                   f"negative test failed rc={p.returncode}", 2)
    else:
        return out("harness_error", None, f"bad fixture-dir: {rel}", 2)


if __name__ == "__main__":
    sys.exit(main())

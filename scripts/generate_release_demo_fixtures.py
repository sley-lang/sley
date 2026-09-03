#!/usr/bin/env python3
"""Refresh the deterministic S20-720 release demo fixture from the executable genesis."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/release-demo/v1"
EXPECTED_KEYS = [
    "branch_name_hex",
    "exchange_hex",
    "execute_request_hex",
    "execute_response_hex",
    "execution_report_id_hex",
    "function_id_hex",
    "head_transaction_id_hex",
    "query_root_request_hex",
    "query_root_response_hex",
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail when committed fixtures differ from the emitted vectors")
    arguments = parser.parse_args()
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-protocol",
            "emit_release_demo_vectors_for_fixture_refresh",
            "--",
            "--ignored",
            "--nocapture",
        ],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=True,
    )
    values: dict[str, str] = {}
    for line in completed.stdout.splitlines():
        if line.startswith("RELEASE_DEMO|"):
            _, key, value = line.split("|", 2)
            values[key] = value
    if sorted(values) != EXPECTED_KEYS:
        raise RuntimeError(f"unexpected demo vector set {sorted(values)}")
    demo = {
        "claim": "s20-720-release-demo-v1",
        "contract": "sley2-release-demo-v1",
        "generator": "scripts/generate_release_demo_fixtures.py",
        "source": "executable test genesis (crates/sley-repo test-support `executable_bodies`)",
        "smp1_revision": 7,
        "vectors": values,
    }
    rendered = {FIXTURES / "demo.json": json.dumps(demo, indent=2, sort_keys=True) + "\n"}
    rendered[FIXTURES / "SHA256SUMS"] = "".join(
        f"{hashlib.sha256(payload.encode()).hexdigest()}  {path.name}\n" for path, payload in rendered.items()
    )
    if arguments.check:
        drift = [
            str(path.relative_to(ROOT))
            for path, expected in rendered.items()
            if not path.is_file() or path.read_text(encoding="utf-8") != expected
        ]
        print(json.dumps({"drift": drift, "mode": "check", "result": "FAIL" if drift else "PASS", "vectors": len(values)}, sort_keys=True))
        return 1 if drift else 0
    FIXTURES.mkdir(parents=True, exist_ok=True)
    for path, payload in rendered.items():
        path.write_text(payload, encoding="utf-8")
    print(json.dumps({"mode": "write", "result": "PASS", "vectors": len(values)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

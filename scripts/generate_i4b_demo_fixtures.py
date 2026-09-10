#!/usr/bin/env python3
"""Refresh the deterministic AT-MW-02 I4b demonstration fixtures.

The fixtures are two minimal program repositories emitted by the Rust
owner (`emit_i4b_demo_exchange_for_fixture_refresh` in
crates/sley-repo): each holds one Namespace, one complete Boolean
function unit (Function, two Bool Parameters, Block, one Boolean
Operation under Return), and a policy granting the demo principal
`ReplaceEntityVersion`. Variants differ in workspace (hence every
derived identity), base opcode, and operand order.

Only identities and the exchange bytes are recorded here. The exchange
bytes seed the endpoint; the demonstration agent never receives
pre-edit bodies except through live `entity.version` / `entity.signature`
reads, which the frame transcript proves.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "bench" / "sley2" / "fixtures" / "i4b_demo_v1.json"

FIELDS = (
    "exchange_hex",
    "operation",
    "function",
    "param0",
    "param1",
    "block",
    "namespace",
    "principal",
    "head_tx",
    "receipt",
    "root",
)


def emit() -> dict[str, dict[str, str]]:
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-repo",
            "emit_i4b_demo_exchange_for_fixture_refresh",
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
    vectors: dict[str, dict[str, str]] = {}
    for line in completed.stdout.splitlines():
        if not line.startswith("I4B_DEMO_VECTOR|"):
            continue
        parts = line.split("|")
        if len(parts) != 13:
            raise RuntimeError(f"malformed emitter line: {line[:80]}")
        variant = parts[1]
        vectors[variant] = dict(zip(FIELDS, parts[2:]))
    if set(vectors) != {"a", "b"}:
        raise RuntimeError(f"expected variants a/b, found {sorted(vectors)}")
    for variant, vector in vectors.items():
        if set(vector) != set(FIELDS):
            raise RuntimeError(f"variant {variant} field mismatch")
        for field in ("operation", "function", "param0", "param1", "block", "namespace",
                      "principal", "head_tx", "receipt", "root"):
            raw = vector[field]
            if len(raw) != 64 or any(ch not in "0123456789abcdef" for ch in raw):
                raise RuntimeError(f"variant {variant} field {field} is not 32 hex bytes")
        if not vector["exchange_hex"]:
            raise RuntimeError(f"variant {variant} has an empty exchange")
    return vectors


def document(vectors: dict[str, dict[str, str]]) -> dict[str, object]:
    return {
        "contract": "at-mw-02-i4b-demo-fixtures-v1",
        "generator": "scripts/generate_i4b_demo_fixtures.py from "
        "emit_i4b_demo_exchange_for_fixture_refresh I4B_DEMO_VECTOR lines",
        "variants": sorted(vectors),
        "vectors": vectors,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when the committed fixtures differ from the emitter output",
    )
    arguments = parser.parse_args()
    vectors = emit()
    if arguments.check:
        try:
            committed = json.loads(FIXTURES.read_text(encoding="utf-8"))
        except (OSError, ValueError) as error:
            print(json.dumps({"contract": "at-mw-02-i4b-demo-fixtures-v1",
                              "result": "FAIL", "problems": [f"unreadable-fixture:{error}"]}))
            return 1
        problems = []
        if committed.get("vectors") != vectors:
            problems.append("fixture-bytes-drift: committed vectors differ from emitter output")
        print(json.dumps({"contract": "at-mw-02-i4b-demo-fixtures-v1", "result": "PASS" if not problems else "FAIL",
                          "problems": problems, "variants": sorted(vectors)}))
        return 0 if not problems else 1
    FIXTURES.parent.mkdir(parents=True, exist_ok=True)
    FIXTURES.write_text(json.dumps(document(vectors), indent=1, sort_keys=True) + "\n", encoding="utf-8")
    digest = hashlib.sha256(FIXTURES.read_bytes()).hexdigest()
    print(json.dumps({"contract": "at-mw-02-i4b-demo-fixtures-v1", "result": "REFRESHED",
                      "path": str(FIXTURES.relative_to(ROOT)), "sha256": digest,
                      "variants": sorted(vectors)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

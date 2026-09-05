#!/usr/bin/env python3
"""Refresh the deterministic S20-420 SMP1 JSON bridge round-trip fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/smp1-json-bridge/v1"
EXPECTED_VECTORS = ["request", "response", "failure", "hello-client", "hello-server"]
EXPECTED_REJECTION_COUNT = 36


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when committed fixtures differ from the bridge-owned vectors",
    )
    arguments = parser.parse_args()
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-json-bridge",
            "emit_smp1_json_bridge_vectors_for_fixture_refresh",
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
    vectors: list[dict[str, object]] = []
    rejections: list[dict[str, object]] = []
    for line in completed.stdout.splitlines():
        parts = line.split("|")
        if line.startswith("SMP1_JSON_BRIDGE_VECTOR|"):
            text = bytes.fromhex(parts[3]).decode("utf-8")
            vectors.append(
                {
                    "frame_bytes": len(parts[2]) // 2,
                    "frame_hex": parts[2],
                    "id": parts[1],
                    "json": text,
                    "json_sha256": hashlib.sha256(text.encode()).hexdigest(),
                }
            )
        elif line.startswith("SMP1_JSON_BRIDGE_REJECT|"):
            rejections.append(
                {
                    "expected_code": parts[3],
                    "id": parts[1],
                    "json": bytes.fromhex(parts[2]).decode("utf-8"),
                }
            )
    if [vector["id"] for vector in vectors] != EXPECTED_VECTORS:
        raise RuntimeError(f"unexpected vector set {[v['id'] for v in vectors]}")
    if len(rejections) != EXPECTED_REJECTION_COUNT or len({r["id"] for r in rejections}) != len(rejections):
        raise RuntimeError(f"unexpected rejection set {[r['id'] for r in rejections]}")
    accepted = {
        "claim": "s20-420-smp1-json-bridge-v1-conformance",
        "contract": "sley2-smp1-json-bridge-v1",
        "generator": "scripts/generate_smp1_json_bridge_fixtures.py",
        "method_table": "conformance/smp1-json-bridge/v1/methods.json",
        "source_fixture": "conformance/smp1/v1/accepted.json",
        "vectors": vectors,
    }
    rejected = {
        "claim": "s20-420-smp1-json-bridge-v1-conformance",
        "contract": "sley2-smp1-json-bridge-v1",
        "mutations": rejections,
    }
    rendered = {
        FIXTURES / "roundtrip.json": json.dumps(accepted, indent=2, sort_keys=True) + "\n",
        FIXTURES / "rejected.json": json.dumps(rejected, indent=2, sort_keys=True) + "\n",
    }
    rendered[FIXTURES / "SHA256SUMS"] = "".join(
        f"{hashlib.sha256(payload.encode()).hexdigest()}  {path.name}\n"
        for path, payload in rendered.items()
    ) + f"{hashlib.sha256((FIXTURES / 'methods.json').read_bytes()).hexdigest()}  methods.json\n"
    if arguments.check:
        drift = [
            str(path.relative_to(ROOT))
            for path, expected in rendered.items()
            if not path.is_file() or path.read_text(encoding="utf-8") != expected
        ]
        print(
            json.dumps(
                {"drift": drift, "mode": "check", "rejections": len(rejections), "result": "FAIL" if drift else "PASS", "vectors": len(vectors)},
                sort_keys=True,
            )
        )
        return 1 if drift else 0
    FIXTURES.mkdir(parents=True, exist_ok=True)
    for path, payload in rendered.items():
        path.write_text(payload, encoding="utf-8")
    print(json.dumps({"mode": "write", "rejections": len(rejections), "result": "PASS", "vectors": len(vectors)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

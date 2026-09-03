#!/usr/bin/env python3
"""Refresh the deterministic S20-510 semantic-comparison conformance corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/semantic-comparison/v1"
EXPECTED_CASES = [
    "identical",
    "change-classes",
    "type-members",
    "type-reorder",
    "signature",
    "body-only",
    "relations",
    "root-sets",
    "collateral",
]
EXPECTED_REJECTIONS = ["version", "flip-trailer", "contract-tag", "trailing-byte", "added-shape"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when committed fixtures differ from the comparison-owned corpus",
    )
    arguments = parser.parse_args()
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-repo",
            "emit_semantic_comparison_corpus_for_fixture_refresh",
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
        if line.startswith("COMPARE_VECTOR|"):
            _, name, base, target, delta, stored_hex, delta_id = line.split("|", maxsplit=6)
            vectors.append(
                {
                    "base": json.loads(base),
                    "delta": json.loads(delta),
                    "id": name,
                    "semantic_delta_id": delta_id,
                    "stored_bytes": len(bytes.fromhex(stored_hex)),
                    "stored_hex": stored_hex,
                    "stored_sha256": hashlib.sha256(bytes.fromhex(stored_hex)).hexdigest(),
                    "target": json.loads(target),
                }
            )
        elif line.startswith("COMPARE_REJECT|"):
            _, name, code, input_hex = line.split("|", maxsplit=3)
            rejections.append(
                {
                    "expected_code": code,
                    "id": name,
                    "input_hex": input_hex,
                    "input_sha256": hashlib.sha256(bytes.fromhex(input_hex)).hexdigest(),
                }
            )
    if [vector["id"] for vector in vectors] != EXPECTED_CASES:
        raise RuntimeError(f"unexpected corpus {[vector['id'] for vector in vectors]}")
    if [rejection["id"] for rejection in rejections] != EXPECTED_REJECTIONS:
        raise RuntimeError(f"unexpected rejection set {[r['id'] for r in rejections]}")
    accepted = {
        "claim": "s20-510-semantic-comparison-v1-conformance",
        "contract": "sley2-semantic-comparison-v1",
        "contract_tag": 510,
        "digest_domain_tag": 20,
        "generator": "scripts/generate_semantic_comparison_fixtures.py",
        "vectors": vectors,
    }
    rejected = {
        "claim": "s20-510-semantic-comparison-v1-conformance",
        "contract": "sley2-semantic-comparison-v1",
        "mutations": rejections,
    }
    rendered = {
        FIXTURES / "accepted.json": json.dumps(accepted, indent=2, sort_keys=True) + "\n",
        FIXTURES / "rejected.json": json.dumps(rejected, indent=2, sort_keys=True) + "\n",
    }
    rendered[FIXTURES / "SHA256SUMS"] = "".join(
        f"{hashlib.sha256(payload.encode()).hexdigest()}  {path.name}\n"
        for path, payload in rendered.items()
    )
    if arguments.check:
        drift = [
            str(path.relative_to(ROOT))
            for path, expected in rendered.items()
            if not path.is_file() or path.read_text(encoding="utf-8") != expected
        ]
        print(
            json.dumps(
                {"drift": drift, "mode": "check", "result": "FAIL" if drift else "PASS", "vectors": len(vectors)},
                sort_keys=True,
            )
        )
        return 1 if drift else 0
    FIXTURES.mkdir(parents=True, exist_ok=True)
    for path, payload in rendered.items():
        path.write_text(payload, encoding="utf-8")
    print(json.dumps({"mode": "write", "result": "PASS", "vectors": len(vectors)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

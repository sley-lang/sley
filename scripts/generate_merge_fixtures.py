#!/usr/bin/env python3
"""Refresh the deterministic S20-520 merge conformance corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/merge/v1"
EXPECTED_CASES = [
    "identical",
    "disjoint-entities",
    "composed-members",
    "disjoint-fields",
    "convergent",
    "fast-forward",
    "metadata-overridden",
    "field-edit",
    "add-add",
    "delete-edit",
    "kind-edit",
    "collateral",
    "metadata-edit",
]
EXPECTED_REJECTIONS = ["version", "flip-trailer", "contract-tag", "trailing-byte"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail when committed fixtures differ")
    arguments = parser.parse_args()
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-repo",
            "emit_merge_corpus_for_fixture_refresh",
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
        if line.startswith("MERGE_VECTOR|"):
            _, name, outcome, ancestor, ours, theirs, expected = line.split("|", maxsplit=6)
            vectors.append(
                {
                    "ancestor": json.loads(ancestor),
                    "expected": json.loads(expected),
                    "id": name,
                    "ours": json.loads(ours),
                    "outcome": outcome,
                    "theirs": json.loads(theirs),
                }
            )
        elif line.startswith("MERGE_REJECT|"):
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
        "claim": "s20-520-merge-v1-conformance",
        "contract": "sley2-merge-v1",
        "contract_tag": 520,
        "digest_domain_tag": 21,
        "generator": "scripts/generate_merge_fixtures.py",
        "vectors": vectors,
    }
    rejected = {
        "claim": "s20-520-merge-v1-conformance",
        "contract": "sley2-merge-v1",
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

#!/usr/bin/env python3
"""Refresh the deterministic S20-250 full complete-entity-impact fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/complete-entity-impact/v1"
EXPECTED_REJECTIONS = [
    "inventory-missing",
    "inventory-surplus",
    "facts-not-canonical",
    "exports-not-canonical",
    "dependency-wrong-kind",
    "subject-unresolved",
    "workspace-ambiguous",
    "package-membership",
    "namespace-root-parent",
    "namespace-root-shared",
    "namespace-tree-parent",
    "namespace-tree-cycle",
    "member-double-owner",
    "member-forbidden-kind",
    "export-unscoped",
    "entry-points-mismatch",
    "dependency-roots-mismatch",
    "binding-unowned",
    "binding-outside-tree",
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when committed fixtures differ from the judgment-owned vector",
    )
    arguments = parser.parse_args()
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-query",
            "emit_complete_entity_impact_vector_for_fixture_refresh",
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
        if line.startswith("COMPLETE_ROOT_VECTOR|"):
            _, request, expected = line.split("|", maxsplit=2)
            vectors.append(
                {
                    "expected": json.loads(expected),
                    "id": "eighteen-kind-complete-root",
                    "request": json.loads(request),
                    "request_sha256": hashlib.sha256(request.encode()).hexdigest(),
                }
            )
        elif line.startswith("COMPLETE_ROOT_REJECT|"):
            _, vector_id, code, numeric, request = line.split("|", maxsplit=4)
            rejections.append(
                {
                    "expected_code": code,
                    "expected_numeric": int(numeric),
                    "id": vector_id,
                    "request": json.loads(request),
                }
            )
    if len(vectors) != 1:
        raise RuntimeError(f"expected one complete-root vector, found {len(vectors)}")
    if [rejection["id"] for rejection in rejections] != EXPECTED_REJECTIONS:
        raise RuntimeError(f"unexpected rejection set {[r['id'] for r in rejections]}")
    accepted = {
        "claim": "s20-250-full-complete-entity-impact-v1-conformance",
        "contract": "sley2-complete-entity-impact-v1",
        "entity_kinds": 18,
        "generator": "scripts/generate_complete_entity_impact_fixtures.py",
        "vectors": vectors,
    }
    rejected = {
        "claim": "s20-250-full-complete-entity-impact-v1-conformance",
        "contract": "sley2-complete-entity-impact-v1",
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
                {
                    "drift": drift,
                    "mode": "check",
                    "result": "FAIL" if drift else "PASS",
                    "rejections": len(rejections),
                    "vectors": len(vectors),
                },
                sort_keys=True,
            )
        )
        return 1 if drift else 0
    FIXTURES.mkdir(parents=True, exist_ok=True)
    for path, payload in rendered.items():
        path.write_text(payload, encoding="utf-8")
    print(
        json.dumps(
            {"mode": "write", "result": "PASS", "rejections": len(rejections), "vectors": len(vectors)},
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

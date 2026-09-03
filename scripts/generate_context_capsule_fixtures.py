#!/usr/bin/env python3
"""Refresh the deterministic S20-320 full context capsule fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/context-capsule/v1"
SOURCE = ROOT / "conformance/root-backed-query/v1/accepted.json"
EXPECTED_VECTORS = [f"class-{index:02}" for index in range(1, 20)] + [
    "page-namespaces-1",
    "page-namespaces-2",
    "page-edges-1",
    "page-edges-2",
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when committed fixtures differ from the builder-owned vectors",
    )
    arguments = parser.parse_args()
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-query",
            "emit_context_capsule_vectors_for_fixture_refresh",
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
    source = json.loads(SOURCE.read_text(encoding="utf-8"))
    source_ids = {vector["id"]: vector["query_id"] for vector in source["vectors"]}
    vectors: list[dict[str, object]] = []
    for line in completed.stdout.splitlines():
        if line.startswith("CONTEXT_CAPSULE_VECTOR|"):
            parts = line.split("|")
            vector_id = parts[1]
            if source_ids.get(vector_id) != parts[2]:
                raise RuntimeError(f"{vector_id}: query identity drifted from the S20-310 fixture")
            vectors.append(
                {
                    "capsule_id": parts[3],
                    "completeness": int(parts[5]),
                    "id": vector_id,
                    "omitted": int(parts[8]),
                    "query_id": parts[2],
                    "record_bytes": len(parts[4]) // 2,
                    "record_hex": parts[4],
                    "returned": int(parts[7]),
                    "source_vector": vector_id,
                    "total_count": int(parts[6]),
                }
            )
    if [vector["id"] for vector in vectors] != EXPECTED_VECTORS:
        raise RuntimeError(f"unexpected vector set {[v['id'] for v in vectors]}")
    accepted = {
        "claim": "s20-320-full-context-capsule-v1-conformance",
        "contract": "sley2-context-capsule-v1",
        "domain": "sley2.context-capsule.v1",
        "generator": "scripts/generate_context_capsule_fixtures.py",
        "session_binding": 1,
        "source_fixture": str(SOURCE.relative_to(ROOT)),
        "vectors": vectors,
    }
    rendered = {FIXTURES / "accepted.json": json.dumps(accepted, indent=2, sort_keys=True) + "\n"}
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

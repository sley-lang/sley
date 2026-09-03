#!/usr/bin/env python3
"""Refresh the deterministic S20-310 full root-backed query fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/root-backed-query/v1"
SOURCE = ROOT / "conformance/complete-entity-impact/v1/accepted.json"
SNAPSHOT_SOURCE = ROOT / "conformance/complete-root-index-snapshot/v1/accepted.json"
EXPECTED_VECTORS = [f"class-{index:02}" for index in range(1, 20)] + [
    "page-namespaces-1",
    "page-namespaces-2",
    "page-edges-1",
    "page-edges-2",
]
EXPECTED_REJECTIONS = [
    "truncated-without-continuation",
    "cursor-wrong-type",
    "cursor-on-single-key",
    "class-not-applicable",
    "unresolved-entity",
    "filter-not-canonical",
    "depth-cut",
    "work-exhausted",
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when committed fixtures differ from the engine-owned vectors",
    )
    arguments = parser.parse_args()
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-query",
            "emit_root_query_vectors_for_fixture_refresh",
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
    source = json.loads(SOURCE.read_text(encoding="utf-8"))["vectors"][0]
    snapshot_source = json.loads(SNAPSHOT_SOURCE.read_text(encoding="utf-8"))["vectors"][0]
    context: dict[str, object] | None = None
    vectors: list[dict[str, object]] = []
    rejections: list[dict[str, object]] = []
    for line in completed.stdout.splitlines():
        if line.startswith("ROOT_QUERY_CONTEXT|"):
            parts = line.split("|")
            context = {
                "bindings": json.loads(parts[8]),
                "contract_root": parts[5],
                "fingerprints": json.loads(parts[9]),
                "policy_root": parts[7],
                "root_hex": parts[2],
                "schema_epoch_hex": parts[3],
                "snapshot_id": parts[1],
                "snapshot_source": str(SNAPSHOT_SOURCE.relative_to(ROOT)),
                "source_fixture": str(SOURCE.relative_to(ROOT)),
                "source_request_sha256": source["request_sha256"],
                "test_root": parts[6],
                "workspace_id": parts[4],
            }
        elif line.startswith("ROOT_QUERY_VECTOR|"):
            parts = line.split("|")
            vectors.append(
                {
                    "after": json.loads(parts[5]),
                    "allow_continuation": parts[4] == "true",
                    "id": parts[1],
                    "limits": json.loads(parts[3]),
                    "next_after": json.loads(parts[8]),
                    "query": json.loads(parts[2]),
                    "query_id": parts[6],
                    "record_bytes": len(parts[7]) // 2,
                    "record_hex": parts[7],
                }
            )
        elif line.startswith("ROOT_QUERY_REJECT|"):
            parts = line.split("|")
            rejections.append(
                {
                    "after": json.loads(parts[5]),
                    "allow_continuation": parts[4] == "true",
                    "expected_code": parts[6],
                    "expected_numeric": int(parts[7]),
                    "id": parts[1],
                    "limits": json.loads(parts[3]),
                    "query": json.loads(parts[2]),
                }
            )
    if context is None:
        raise RuntimeError("no root-query context line")
    if context["snapshot_id"] != snapshot_source["snapshot_id"]:
        raise RuntimeError("root-query context snapshot drifted from the S20-300 fixture")
    if [vector["id"] for vector in vectors] != EXPECTED_VECTORS:
        raise RuntimeError(f"unexpected vector set {[v['id'] for v in vectors]}")
    if [rejection["id"] for rejection in rejections] != EXPECTED_REJECTIONS:
        raise RuntimeError(f"unexpected rejection set {[r['id'] for r in rejections]}")
    accepted = {
        "claim": "s20-310-full-root-backed-query-v1-conformance",
        "context": context,
        "contract": "sley2-root-backed-query-v1",
        "domain": "sley2.root-query.v1",
        "generator": "scripts/generate_root_backed_query_fixtures.py",
        "query_classes": 19,
        "vectors": vectors,
    }
    rejected = {
        "claim": "s20-310-full-root-backed-query-v1-conformance",
        "contract": "sley2-root-backed-query-v1",
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

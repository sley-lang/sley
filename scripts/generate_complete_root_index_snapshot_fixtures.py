#!/usr/bin/env python3
"""Refresh the deterministic S20-300 full complete-root index snapshot fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/complete-root-index-snapshot/v1"
SOURCE = ROOT / "conformance/complete-entity-impact/v1/accepted.json"
EXPECTED_REJECTIONS = [
    "digest-trailer-bit",
    "format-version-two",
    "restricted-arm-tag",
    "other-root-context",
    "rootless-context",
    "truncated-record",
]
REASON_CODES = {
    "FormatInvalid": ("INDEX_SNAPSHOT_FORMAT_INVALID", 30001),
    "VersionUnsupported": ("INDEX_SNAPSHOT_VERSION_UNSUPPORTED", 30002),
    "ContextMismatch": ("INDEX_SNAPSHOT_CONTEXT_MISMATCH", 30003),
    "DigestMismatch": ("INDEX_SNAPSHOT_DIGEST_MISMATCH", 30004),
    "CompletenessUnsupported": ("INDEX_SNAPSHOT_COMPLETENESS_UNSUPPORTED", 30005),
    "ResourceLimit": ("INDEX_SNAPSHOT_RESOURCE_LIMIT", 30006),
    "RootMismatch": ("INDEX_SNAPSHOT_ROOT_MISMATCH", 30009),
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when committed fixtures differ from the builder-owned vector",
    )
    arguments = parser.parse_args()
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-query",
            "emit_complete_root_snapshot_vector_for_fixture_refresh",
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
    source_vector = source["vectors"][0]
    vectors: list[dict[str, object]] = []
    rejections: list[dict[str, object]] = []
    for line in completed.stdout.splitlines():
        if line.startswith("COMPLETE_ROOT_SNAPSHOT_VECTOR|"):
            _, record, snapshot_id, root, epoch, count = line.split("|")
            vectors.append(
                {
                    "completeness_arm": 2,
                    "id": "eighteen-kind-complete-root-snapshot",
                    "inventory_count": int(count),
                    "record_bytes": len(record) // 2,
                    "record_hex": record,
                    "root_hex": root,
                    "schema_epoch_hex": epoch,
                    "snapshot_id": snapshot_id,
                    "source_fixture": str(SOURCE.relative_to(ROOT)),
                    "source_request_sha256": source_vector["request_sha256"],
                    "source_vector_id": source_vector["id"],
                }
            )
        elif line.startswith("COMPLETE_ROOT_SNAPSHOT_REJECT|"):
            _, vector_id, reason, candidate = line.split("|")
            symbol, numeric = REASON_CODES[reason]
            rejections.append(
                {
                    "candidate_hex": candidate,
                    "discard_reason": reason,
                    "expected_code": symbol,
                    "expected_numeric": numeric,
                    "id": vector_id,
                }
            )
    if len(vectors) != 1:
        raise RuntimeError(f"expected one complete-root snapshot vector, found {len(vectors)}")
    if [rejection["id"] for rejection in rejections] != EXPECTED_REJECTIONS:
        raise RuntimeError(f"unexpected rejection set {[r['id'] for r in rejections]}")
    accepted = {
        "claim": "s20-300-full-complete-root-index-snapshot-v1-conformance",
        "contract": "sley2-complete-root-index-snapshot-v1",
        "domain": "sley2.index-snapshot.v1",
        "generator": "scripts/generate_complete_root_index_snapshot_fixtures.py",
        "vectors": vectors,
    }
    rejected = {
        "claim": "s20-300-full-complete-root-index-snapshot-v1-conformance",
        "contract": "sley2-complete-root-index-snapshot-v1",
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

#!/usr/bin/env python3
"""Refresh the deterministic S20-540 repository-exchange conformance fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/repository-exchange/v1"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when committed fixtures differ from the codec-owned vector",
    )
    arguments = parser.parse_args()
    completed = subprocess.run(
        [
            "cargo",
            "test",
            "-p",
            "sley-repo",
            "emit_repository_exchange_vector_for_fixture_refresh",
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
        if line.startswith("EXCHANGE_VECTOR|"):
            (
                _,
                exchange_hex,
                exchange_id,
                pack_id,
                tree_root,
                receipts,
                branches,
                head,
            ) = line.split("|", maxsplit=7)
            vectors.append(
                {
                    "accepted_head_transaction_id_hex": head,
                    "branches": int(branches),
                    "digest_tree_root": tree_root,
                    "exchange_hex": exchange_hex,
                    "exchange_sha256": hashlib.sha256(bytes.fromhex(exchange_hex)).hexdigest(),
                    "id": "clone-equivalent-source",
                    "receipts": int(receipts),
                    "repository_exchange_id": exchange_id,
                    "repository_pack_id": pack_id,
                    "stored_bytes": len(bytes.fromhex(exchange_hex)),
                }
            )
        elif line.startswith("EXCHANGE_REJECT|"):
            _, vector_id, expected_code, input_hex = line.split("|", maxsplit=3)
            rejections.append(
                {
                    "expected_code": expected_code,
                    "id": vector_id,
                    "input_hex": input_hex,
                    "input_sha256": hashlib.sha256(bytes.fromhex(input_hex)).hexdigest(),
                }
            )
    if len(vectors) != 1:
        raise RuntimeError(f"expected one exchange vector, found {len(vectors)}")
    if [rejection["id"] for rejection in rejections] != [
        "flip-trailer",
        "nested-exchange",
        "reversed-branches",
        "foreign-head",
        "open-ancestry",
    ]:
        raise RuntimeError(f"unexpected rejection set {[r['id'] for r in rejections]}")
    accepted = {
        "claim": "s20-540-repository-exchange-v1-clone-equivalent-conformance",
        "contract": "sley2-repository-exchange-v1",
        "contract_tag": 540,
        "digest_domain_tag": 19,
        "generator": "scripts/generate_repository_exchange_fixtures.py",
        "vectors": vectors,
    }
    rejected = {
        "claim": "s20-540-repository-exchange-v1-clone-equivalent-conformance",
        "contract": "sley2-repository-exchange-v1",
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
                    "vectors": len(vectors),
                },
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

#!/usr/bin/env python3
"""Refresh the deterministic VM extended opcode profile vectors (slice E1 onward)."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/vm-extended/v1"
EXPECTED = ["tuple-project", "vector-set-out-of-range", "signed-less-than", "constant-ref", "int-add-overflow", "int-div-signed-min", "int-shl-signed", "float-div-canonical-nan", "float-fma-single-rounding", "float-less-than-nan", "result-err"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail when committed fixtures differ from the crate's vectors")
    arguments = parser.parse_args()
    completed = subprocess.run(
        ["cargo", "test", "-p", "sley-vm", "emit_vm_extended_vectors_for_fixture_refresh", "--", "--ignored", "--nocapture"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=True,
    )
    vectors = []
    for line in completed.stdout.splitlines():
        if line.startswith("VM_EXTENDED_VECTOR|"):
            parts = line.split("|")
            vectors.append(
                {
                    "bytecode_hex": parts[3],
                    "bytecode_sha256": hashlib.sha256(bytes.fromhex(parts[3])).hexdigest(),
                    "cache_key_hex": parts[4],
                    "id": parts[1],
                    "instruction_count": int(parts[7]),
                    "observation_id_hex": parts[6],
                    "opcode": int(parts[2]),
                    "success_value_hash_hex": parts[5],
                }
            )
    if [vector["id"] for vector in vectors] != EXPECTED:
        raise RuntimeError(f"unexpected vector set {[v['id'] for v in vectors]}")
    accepted = {
        "claim": "s20-260-270-vm-extended-e1-e3-conformance",
        "contract": "sley2-vm-extended-opcode-profile-v1",
        "cache_profile": "EXTENDED_V1",
        "bytecode_magic": "SLEYBC02",
        "generator": "scripts/generate_vm_extended_fixtures.py",
        "schema_epoch_hex": "08" * 32,
        "state_root_hex": "09" * 32,
        "vectors": vectors,
    }
    rendered = {FIXTURES / "accepted.json": json.dumps(accepted, indent=2, sort_keys=True) + "\n"}
    rendered[FIXTURES / "SHA256SUMS"] = "".join(f"{hashlib.sha256(payload.encode()).hexdigest()}  {path.name}\n" for path, payload in rendered.items())
    if arguments.check:
        drift = [str(path.relative_to(ROOT)) for path, expected in rendered.items() if not path.is_file() or path.read_text(encoding="utf-8") != expected]
        print(json.dumps({"drift": drift, "mode": "check", "result": "FAIL" if drift else "PASS", "vectors": len(vectors)}, sort_keys=True))
        return 1 if drift else 0
    FIXTURES.mkdir(parents=True, exist_ok=True)
    for path, payload in rendered.items():
        path.write_text(payload, encoding="utf-8")
    print(json.dumps({"mode": "write", "result": "PASS", "vectors": len(vectors)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

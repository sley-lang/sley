#!/usr/bin/env python3
"""Refresh the frozen BOOTSTRAP_PROFILE_1 closure vectors (RW-050 slice 2)."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/bootstrap-profile/v1"
EXPECTED = ["bytes-round-trip", "vector-push-loop-n5", "vector-push-loop-n0", "map-symbol-table-hit", "map-symbol-table-miss", "set-as-map-absent-after-remove", "record-variant-walk", "multi-function-pass-ok", "multi-function-pass-overflow", "checked-length-traverse-hit", "checked-length-traverse-miss", "value-hash-chain", "variant-switch-exhaustive-some-ok-equal", "variant-switch-exhaustive-some-ok-unequal", "variant-switch-exhaustive-none", "variant-switch-exhaustive-err", "graph-worklist-dfs-chain", "graph-worklist-dfs-cycle", "image-assemble-emit-n3", "image-assemble-emit-n0"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail when committed fixtures differ from the crate's vectors")
    arguments = parser.parse_args()
    completed = subprocess.run(
        ["cargo", "test", "-p", "sley-vm", "emit_bootstrap_profile_vectors_for_freeze", "--", "--ignored", "--nocapture"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=True,
    )
    vectors = []
    for line in completed.stdout.splitlines():
        if line.startswith("BOOTSTRAP_PROFILE_VECTOR|"):
            parts = line.split("|")
            vectors.append(
                {
                    "bytecode_hex": parts[3],
                    "bytecode_sha256": hashlib.sha256(bytes.fromhex(parts[3])).hexdigest(),
                    "cache_key_hex": parts[4],
                    "fuel_used": int(parts[7]),
                    "id": parts[1],
                    "inputs_desc": inputs_desc(parts[1]),
                    "instruction_count": int(parts[6]),
                    "observation_id_hex": parts[5],
                    "subject_opcode": int(parts[2]),
                    "success_value_hash_hex": parts[8],
                }
            )
    if [vector["id"] for vector in vectors] != EXPECTED:
        raise RuntimeError(f"unexpected vector set {[v['id'] for v in vectors]}")
    accepted = {
        "bytecode_magic": "SLEYBC02",
        "cache_profile": "EXTENDED_V1",
        "claim": "bootstrap-profile-1-closure-conformance",
        "contract": "sley2-bootstrap-profile-1",
        "gate": "judge_bootstrap_profile admitted every vector pre-emission; rerun the emitter to re-verify",
        "generator": "scripts/generate_bootstrap_profile_fixtures.py",
        "reference_budgets": {
            "cancel_at_fuel": None,
            "max_fuel": 10_000_000,
            "max_instructions": 100_000,
            "max_output_units": 10_000_000,
            "max_value_units": 100_000_000,
        },
        "schema_epoch_hex": "08" * 32,
        "state_root_hex": "09" * 32,
        "vectors": vectors,
    }
    rendered = {
        FIXTURES / "accepted.json": json.dumps(accepted, indent=2, sort_keys=True) + "\n",
    }
    # The frozen profile record is hand-maintained and digest-bound (never
    # regenerated): the integrity file binds its bytes alongside the
    # emitted vectors so the whole frozen set drifts as one unit.
    profile_bytes = (FIXTURES / "profile.json").read_bytes()
    sums = "".join(f"{hashlib.sha256(payload.encode()).hexdigest()}  {path.name}\n" for path, payload in rendered.items())
    sums += f"{hashlib.sha256(profile_bytes).hexdigest()}  profile.json\n"
    rendered[FIXTURES / "SHA256SUMS"] = sums
    if arguments.check:
        drift = [str(path.relative_to(ROOT)) for path, expected in rendered.items() if not path.is_file() or path.read_text(encoding="utf-8") != expected]
        print(json.dumps({"drift": drift, "mode": "check", "result": "FAIL" if drift else "PASS", "vectors": len(vectors)}, sort_keys=True))
        return 1 if drift else 0
    FIXTURES.mkdir(parents=True, exist_ok=True)
    for path, payload in rendered.items():
        path.write_text(payload, encoding="utf-8")
    print(json.dumps({"mode": "write", "result": "PASS", "vectors": len(vectors)}, sort_keys=True))
    return 0


def inputs_desc(vector_id: str) -> str:
    """Human-readable inputs per vector id (machine inputs re-execute via the emitter)."""
    return {
        "bytes-round-trip": "unit, bytes[1,2,3]",
        "vector-push-loop-n5": "bound 5, element 7",
        "vector-push-loop-n0": "bound 0, element 7",
        "map-symbol-table-hit": "keys a b, values 1 2, query b",
        "map-symbol-table-miss": "keys a b, values 1 2, query z",
        "set-as-map-absent-after-remove": "key 7, unit element",
        "record-variant-walk": "number 4, label four",
        "multi-function-pass-ok": "10 + 20, default 0",
        "multi-function-pass-overflow": "max + 1 (overflow code 1), default 99",
        "checked-length-traverse-hit": "vector [10,20,30], index 1, default 0",
        "checked-length-traverse-miss": "vector [10,20,30], index 9, default 0",
        "value-hash-chain": "number 7, label seven",
        "variant-switch-exhaustive-some-ok-equal": "Some(7), Ok(7)",
        "variant-switch-exhaustive-some-ok-unequal": "Some(7), Ok(8)",
        "variant-switch-exhaustive-none": "None, Ok(7)",
        "variant-switch-exhaustive-err": "Some(7), Err(unit)",
        "graph-worklist-dfs-chain": "start 1; links (1,2),(2,3),(3,4) baked as constants",
        "graph-worklist-dfs-cycle": "start 1; links (1,2),(2,1),(9,9) baked as constants",
        "image-assemble-emit-n3": "source octets [1,2,3], unit scope",
        "image-assemble-emit-n0": "source octets [], unit scope",
    }[vector_id]


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Verify the frozen canonical Sley toolchain input root S."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "machineresearch/sley-2.0/reweave/canonical-s-manifest.json"
BOOTSTRAP = ROOT / "machineresearch/sley-2.0/reweave/bootstrap-manifest.json"
CONSTRUCTOR = ROOT / "crates/sley-vm/tests/rw120_toolchain_integration/component.rs"
HANDOFF = ROOT / "crates/sley-vm/tests/rw120_toolchain_integration/handoff.rs"

EXPECTED = {
    "state_root": "b1992814cfc2332215d65e1ede6a619fc2ebb8122b8105e5fc527ca8b10548c3",
    "object_count": 18857,
    "object_bytes": 4747089,
    "object_bundle_sha256": "7e10c8a2474fd6d1aca295f025d06562197a061880343ec47802b196bcf48249",
    "stored_root_bytes": 1244993,
    "stored_root_sha256": "5fbcdf19be9761daa5dd3ccc617cdf00fdfb68d0ec6d43ad7ccf6cd59ed7bcef",
    "function_count": 189,
    "parameter_count": 8748,
    "block_count": 3077,
    "operation_count": 6146,
    "constant_count": 676,
    "adapter_import_count": 4,
    "entry_point_count": 5,
}


def fail(message: str) -> None:
    raise SystemExit(f"FAIL: {message}")


def load_json(path: Path) -> dict[str, object]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read {path.relative_to(ROOT)}: {error}")


def main() -> None:
    manifest = load_json(MANIFEST)
    if manifest.get("schema") != "sley2-canonical-s-manifest-v1":
        fail("canonical S manifest schema mismatch")
    if manifest.get("status") != "CANONICAL_INPUT":
        fail("canonical S must be frozen as CANONICAL_INPUT")
    for key, value in EXPECTED.items():
        if manifest.get(key) != value:
            fail(f"canonical S {key} mismatch")
    if manifest.get("no_sley_source_dsl") is not True:
        fail("canonical S must declare the no-source-DSL construction boundary")
    if manifest.get("complete_dependency_closure") is not True:
        fail("canonical S must declare a complete dependency closure")
    if manifest.get("prebaked_successor_image") is not False:
        fail("canonical S must exclude a prebaked successor image")

    for source in (CONSTRUCTOR, HANDOFF):
        if not source.is_file():
            fail(f"missing seed constructor {source.relative_to(ROOT)}")
    sources = manifest.get("seed_constructor_sources")
    expected_sources = [
        str(CONSTRUCTOR.relative_to(ROOT)),
        str(HANDOFF.relative_to(ROOT)),
    ]
    if sources != expected_sources:
        fail("seed constructor source list mismatch")
    source_digest = hashlib.sha256()
    for source in (CONSTRUCTOR, HANDOFF):
        source_digest.update(source.read_bytes())
    if manifest.get("seed_constructor_sha256") != source_digest.hexdigest():
        fail("seed constructor digest mismatch")

    bootstrap = load_json(BOOTSTRAP)
    stages = bootstrap.get("stages")
    if not isinstance(stages, dict):
        fail("bootstrap stages missing")
    stage_s = stages.get("S")
    if not isinstance(stage_s, dict):
        fail("bootstrap S stage missing")
    if stage_s.get("value") != EXPECTED["state_root"]:
        fail("bootstrap S value does not match canonical manifest")
    if stage_s.get("status") != "canonical input frozen":
        fail("bootstrap S status is not frozen")

    print("PASS: canonical S manifest, constructor digest, and bootstrap binding")


if __name__ == "__main__":
    main()

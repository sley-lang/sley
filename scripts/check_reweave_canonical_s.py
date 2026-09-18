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
    "state_root": "4cbcd1eeea202d482895e70ec91f1e0d154c1b7599cc19d60cc6751e61ecfe47",
    "object_count": 11876,
    "object_bytes": 2971077,
    "object_bundle_sha256": "c40eab81641a843f4983c603cd3483bfcca7a3488658b053576e883deb0d336d",
    "stored_root_bytes": 784246,
    "stored_root_sha256": "85f94269365956705d3d7206ca2aa9a65099da1d55141fe06e3b60f0ec93bbde",
    "function_count": 101,
    "parameter_count": 5148,
    "block_count": 1849,
    "operation_count": 4058,
    "constant_count": 699,
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

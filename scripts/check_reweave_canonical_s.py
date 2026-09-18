#!/usr/bin/env python3
"""Verify the frozen canonical Sley toolchain input root S."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "machineresearch/sley-2.0/reweave/canonical-s-manifest.json"
BOOTSTRAP = ROOT / "machineresearch/sley-2.0/reweave/bootstrap-manifest.json"
CONSTRUCTOR = ROOT / "crates/sley-vm/tests/rw120_toolchain_integration/component.rs"
HANDOFF = ROOT / "crates/sley-vm/tests/rw120_toolchain_integration/handoff.rs"
# The binary whose test constructs S; every `#[path]` module it reaches is a
# seed-constructor source and is bound below, so an edit to any of the codec,
# checker, lowerer, or handoff sources cannot pass unnoticed.
SEED_BINARY = ROOT / "crates/sley-vm/tests/rw120_toolchain_integration.rs"
PATH_ATTRIBUTE = re.compile(r'^#\[path\s*=\s*"([^"]+)"\]\s*$', re.MULTILINE)

EXPECTED = {
    "state_root": "5332d4d758c17c928039cd321ba593aedc5e87f8a57e43ab8e43bc5d877308b3",
    "object_count": 20383,
    "object_bytes": 5131404,
    "object_bundle_sha256": "bee7da94a659c31366ca81fe634cd84d5aa9e5b2108bd56d5d1112cda55e3a5e",
    "stored_root_bytes": 1345709,
    "stored_root_sha256": "35256f888669cb2ce6cc446b3bc955b75e58ea6376202b3b5a20d3a1152af098",
    "function_count": 202,
    "parameter_count": 9763,
    "block_count": 3236,
    "operation_count": 6492,
    "constant_count": 669,
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


def seed_source_closure() -> list[Path]:
    """Every source file the seed binary includes through `#[path]`, in a
    deterministic order: the binary first, then the modules it names,
    each followed by the modules that module names (depth first)."""
    ordered: list[Path] = []
    seen: set[Path] = set()

    def visit(path: Path) -> None:
        if path in seen:
            return
        if not path.is_file():
            fail(f"seed source missing: {path.relative_to(ROOT)}")
        seen.add(path)
        ordered.append(path)
        text = path.read_text(encoding="utf-8")
        for relative in PATH_ATTRIBUTE.findall(text):
            visit((path.parent / relative).resolve())

    visit(SEED_BINARY.resolve())
    return ordered


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
    closure = seed_source_closure()
    if CONSTRUCTOR not in closure or HANDOFF not in closure:
        fail("seed binary no longer includes the component and handoff constructors")
    sources = manifest.get("seed_constructor_sources")
    expected_sources = [str(path.relative_to(ROOT)) for path in closure]
    if sources != expected_sources:
        fail("seed constructor source list mismatch (the #[path] closure of the seed binary)")
    source_digest = hashlib.sha256()
    for source in closure:
        source_digest.update(str(source.relative_to(ROOT)).encode("utf-8") + b"\0")
        source_digest.update(source.read_bytes())
        source_digest.update(b"\0")
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

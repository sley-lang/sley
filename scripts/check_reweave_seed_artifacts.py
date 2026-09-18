#!/usr/bin/env python3
"""Check preserved C0 and C1-candidate metadata, optionally including bytes."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "machineresearch/sley-2.0/reweave/c0-c1-seed-artifacts.json"
BOOTSTRAP = ROOT / "machineresearch/sley-2.0/reweave/bootstrap-manifest.json"


def fail(message: str) -> None:
    raise SystemExit(f"FAIL: {message}")


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--require-local", action="store_true")
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    bootstrap = json.loads(BOOTSTRAP.read_text(encoding="utf-8"))
    if manifest.get("schema") != "sley2-reweave-c0-c1-seed-artifacts-v1":
        fail("seed-artifact schema mismatch")
    if manifest.get("status") != "PRESERVED":
        fail("seed artifacts are not preserved")
    c0 = manifest.get("C0")
    c1 = manifest.get("C1_candidate")
    if not isinstance(c0, dict) or not isinstance(c1, dict):
        fail("C0 or C1 candidate metadata missing")
    stages = bootstrap.get("stages", {})
    if stages.get("C0", {}).get("value") != f"sha256:{c0.get('sha256')}":
        fail("bootstrap C0 digest mismatch")
    if stages.get("C1", {}).get("value") is not None:
        fail("official C1 must remain open until semantic prerequisites close")
    if stages.get("C1", {}).get("candidate") != f"sha256:{c1.get('sha256')}":
        fail("bootstrap C1-candidate digest mismatch")
    qualification = manifest.get("qualification", {})
    if qualification.get("official_c1") is not False:
        fail("candidate must not claim official C1")
    if qualification.get("native_seed_semantics_used") is not True:
        fail("C0 native seed work must remain explicit")

    if args.require_local:
        store = Path(str(manifest.get("artifact_store")))
        for name, record in (("C0", c0), ("C1_candidate", c1)):
            path = store / str(record.get("relative_path"))
            if not path.is_file():
                fail(f"missing local {name} artifact: {path}")
            if path.stat().st_size != record.get("bytes"):
                fail(f"{name} byte count mismatch")
            if digest(path) != record.get("sha256"):
                fail(f"{name} SHA-256 mismatch")
    print("PASS: C0 and C1-candidate preservation metadata" + (" and local bytes" if args.require_local else ""))


if __name__ == "__main__":
    main()

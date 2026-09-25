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
CANONICAL_S = ROOT / "machineresearch/sley-2.0/reweave/canonical-s-manifest.json"


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
    # The candidate's S is the canonical S: one root across the seed manifest,
    # the canonical-S manifest, and the bootstrap stage.
    canonical = json.loads(CANONICAL_S.read_text(encoding="utf-8"))
    if manifest.get("canonical_s") != canonical.get("state_root"):
        fail("seed manifest canonical_s differs from canonical-s-manifest state_root")
    if stages.get("S", {}).get("value") != manifest.get("canonical_s"):
        fail("bootstrap S value differs from the seed manifest canonical_s")
    superseded = manifest.get("superseded_candidate")
    if not isinstance(superseded, dict):
        fail("superseded candidate block missing")
    earlier = manifest.get("earlier_superseded_candidates", [])
    if not isinstance(earlier, list):
        fail("earlier superseded candidates must be a list")
    generations = bootstrap.get("superseded_generations", {})
    recorded = {
        str(value).split(" ")[0]
        for key, value in generations.items()
        if key != "note" and isinstance(value, str)
    }
    superseded_records: list[tuple[str, dict]] = []
    for index, block in enumerate([superseded, *earlier]):
        label = "superseded" if index == 0 else f"earlier superseded {index}"
        if not isinstance(block, dict):
            fail(f"{label} candidate block malformed")
        block_c0 = block.get("C0")
        block_c1 = block.get("C1_candidate")
        if not isinstance(block_c0, dict) or not isinstance(block_c1, dict):
            fail(f"{label} C0 or C1 candidate metadata missing")
        # Every superseded generation is named, by digest, among the
        # bootstrap manifest's superseded generations (S, C0, C1 candidate).
        for name, expected_digest in (
            ("C0", f"sha256:{block_c0.get('sha256')}"),
            ("C1 candidate", f"sha256:{block_c1.get('sha256')}"),
            ("S", str(block.get("canonical_s"))),
        ):
            if expected_digest not in recorded:
                fail(f"bootstrap superseded generations do not name the {label} {name} digest")
        superseded_records.append((f"{label} C0", block_c0))
        superseded_records.append((f"{label} C1_candidate", block_c1))

    if args.require_local:
        store = Path(str(manifest.get("artifact_store")))
        # The retained generation is preserved read-only in the same store,
        # so its bytes and modes are verified alongside the current pair.
        records = [("C0", c0), ("C1_candidate", c1), *superseded_records]
        for name, record in records:
            path = store / str(record.get("relative_path"))
            if not path.is_file():
                fail(f"missing local {name} artifact: {path}")
            if path.stat().st_size != record.get("bytes"):
                fail(f"{name} byte count mismatch")
            if digest(path) != record.get("sha256"):
                fail(f"{name} SHA-256 mismatch")
            mode = f"{path.stat().st_mode & 0o777:04o}"
            if mode != record.get("mode"):
                fail(f"{name} mode {mode} differs from recorded {record.get('mode')}")
    print(
        "PASS: C0 and C1-candidate preservation metadata, canonical S cross-link, "
        "superseded generation"
        + (" and local bytes plus modes" if args.require_local else "")
    )


if __name__ == "__main__":
    main()

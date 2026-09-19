#!/usr/bin/env python3
"""Bind the RW-090 codec component manifest to the pinned test figures.

The generation manifest kept the 7426bc0b component figures through the
0b7a1482 re-mint because nothing compared it with the test pins (Ariadne P1,
Nabu P2, Vulcan P2 at c04539b9). The pins live in
crates/sley-vm/tests/rw080_codec_program/canonical_codec.rs; this checker
reads them the way check_reweave_canonical_s.py reads the S pins and refuses
any manifest figure that differs, and requires the superseded component
blocks the RW-080 contract's RW080-ID-02 rule names.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "machineresearch/sley-2.0/reweave/rw-090-codec-component-manifest.json"
CANONICAL_S = ROOT / "machineresearch/sley-2.0/reweave/canonical-s-manifest.json"
TEST = ROOT / "crates/sley-vm/tests/rw080_codec_program/canonical_codec.rs"

# manifest field -> (pinned expression, kind)
PINS = {
    "image_functions": ("image.functions.len()", "int"),
    "image_parameters": ("image.parameters.len()", "int"),
    "image_blocks": ("image.blocks.len()", "int"),
    "image_operations": ("image.operations.len()", "int"),
    "image_constants": ("image.constants.len()", "int"),
    "image_adapters": ("image.adapters.len()", "int"),
    "codec_object_stored_bytes": ("stored_bytes", "int"),
    "codec_bundle_sha256": ("hex(&digest)", "hex"),
    "state_root": ("hex(root.root.as_bytes())", "hex"),
    "state_root_stored_bytes": ("root.stored_bytes.len()", "int"),
    "state_root_stored_sha256": ("hex(&stored_digest)", "hex"),
}


def fail(message: str) -> None:
    raise SystemExit(f"FAIL: {message}")


def pinned(source: str, expression: str, kind: str) -> list:
    """Every `assert_eq!(<expression>, <literal>)` value in the test source."""
    pattern = re.escape(expression) + r",\s*(?:\"([0-9a-f]{64})\"|([0-9_]+))\s*\)"
    values = []
    for match in re.finditer(r"assert_eq!\(\s*" + pattern, source, re.S):
        if kind == "hex" and match.group(1):
            values.append(match.group(1))
        elif kind == "int" and match.group(2):
            values.append(int(match.group(2).replace("_", "")))
    return values


def main() -> None:
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    source = TEST.read_text(encoding="utf-8")
    if manifest.get("schema") != "sley2-rw090-codec-component-manifest-v1":
        fail("component manifest schema mismatch")
    for field, (expression, kind) in PINS.items():
        values = pinned(source, expression, kind)
        if not values:
            fail(f"no pin for {field} ({expression}) in canonical_codec.rs")
        if manifest.get(field) not in values:
            fail(f"{field}: manifest {manifest.get(field)!r} is not among the pinned {values!r}")
    # objects.len() is pinned twice: codec objects (round-trip test) and the
    # component's complete object set (root test); both must be present.
    object_pins = pinned(source, "objects.len()", "int")
    for field in ("codec_object_count", "object_count"):
        if manifest.get(field) not in object_pins:
            fail(f"{field}: manifest {manifest.get(field)!r} is not among the pinned {object_pins!r}")
    if manifest["object_count"] <= manifest["codec_object_count"]:
        fail("object_count must exceed codec_object_count (spine, contract, test, entry point)")
    # RW080-ID-02: every superseded component root is carried with its
    # source commit, none of them equals the current root, and the blocks
    # each repair round names in review_repairs are present (Nabu P3 at
    # db53894e: the checker had hard-coded three names and left the newest
    # block unbound).
    blocks = sorted(key for key in manifest if key.startswith("superseded_") and key.endswith("_component"))
    # The required set is the lineage itself: one component block per
    # superseded `S` generation the canonical-S manifest records
    # (`superseded_<name>_s` -> `superseded_<name>_component`), so a re-mint
    # that drops a displaced generation fails here (Ariadne/Vulcan P4 at
    # 76227765: the set had been four hard-coded names).
    canonical = json.loads(CANONICAL_S.read_text(encoding="utf-8"))
    lineage = sorted(key for key in canonical if key.startswith("superseded_") and key.endswith("_s"))
    required = {key[: -len("_s")] + "_component" for key in lineage}
    if not required:
        fail("canonical-s-manifest.json records no superseded generation")
    if not required <= set(blocks):
        fail(f"missing superseded component blocks: {sorted(required - set(blocks))}")
    roots = set()
    for block in blocks:
        entry = manifest.get(block)
        if not isinstance(entry, dict):
            fail(f"missing {block}")
        if entry["state_root"] in roots:
            fail(f"{block} repeats another superseded root")
        roots.add(entry["state_root"])
        for key in ("state_root", "codec_bundle_sha256", "source_commit"):
            if not isinstance(entry.get(key), str) or not entry[key]:
                fail(f"{block}.{key} missing")
        if entry["state_root"] == manifest["state_root"]:
            fail(f"{block} names the current root")
    if not isinstance(manifest.get("source_commit"), str) or not manifest["source_commit"]:
        fail("source_commit missing")
    print("PASS: RW-090 codec component manifest bound to the canonical_codec.rs pins and RW080-ID-02 blocks")


if __name__ == "__main__":
    main()

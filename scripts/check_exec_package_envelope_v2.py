#!/usr/bin/env python3
"""Independent byte-layout reproduction for the RW-080 v2 envelope candidate."""

from __future__ import annotations

import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VECTOR_DIR = ROOT / "conformance/exec-package-envelope/v2"


def u32(value: int) -> bytes:
    return struct.pack(">I", value)


def u64(value: int) -> bytes:
    return struct.pack(">Q", value)


record = json.loads((VECTOR_DIR / "envelope.json").read_text(encoding="utf-8"))
accepted = bytes.fromhex((VECTOR_DIR / "accepted.hex").read_text(encoding="utf-8").strip())

entry = bytes([1]) * 32
epoch = bytes([8]) * 32
root = bytes([9]) * 32
image = b"SLEYBC02\0"
empty_section = u64(0)
dependency = b"".join(
    [
        entry,
        epoch,
        root,
        u32(1),
        u32(0),
        u32(0),
        u32(2),
        u32(2),
        u32(0),
        u32(0),
        u64(0),
        u64(0),
        u64(0),
        u32(0),
        u32(0),
        u64(0),
        u64(1_000),
        u64(2_000),
        u64(3_000),
        u64(4_000),
        u32(1),
        u64(0),
        u64(0),
    ]
)
sections = [image, empty_section, empty_section, empty_section, dependency]
section_digests = [hashlib.sha256(section).digest() for section in sections]
header = b"".join(
    [
        b"SLEYPKG1",
        u32(2),
        bytes.fromhex("fb2d8cc87ee7de68cde8197a77003a417a0062acb6ed087d85f899da1a847459"),
        u32(2),
        u32(1),
        u32(0),
        u32(0),
        *section_digests,
        entry,
        epoch,
        root,
    ]
)
envelope = header + b"".join(u64(len(section)) + section for section in sections)

problems: list[str] = []
vector = record.get("vector", {})
checks = {
    "identity": (record.get("identity"), "EXEC_PACKAGE_V2_ENVELOPE"),
    "contract": (record.get("contract"), "sley2-exec-package-2"),
    "version": (record.get("version"), 2),
    "header-bytes": (record.get("header", {}).get("bytes"), len(header)),
    "envelope-bytes": (vector.get("envelope_bytes"), len(envelope)),
    "image-hex": (vector.get("image_hex"), image.hex()),
    "entry-hex": (vector.get("entry_hex"), entry.hex()),
    "epoch-hex": (vector.get("epoch_hex"), epoch.hex()),
    "root-hex": (vector.get("root_hex"), root.hex()),
    "image-digest": (vector.get("image_digest"), section_digests[0].hex()),
    "empty-digest": (vector.get("empty_section_digest"), section_digests[1].hex()),
    "dependency-digest": (vector.get("dependency_digest"), section_digests[4].hex()),
    "package-digest": (vector.get("package_digest"), hashlib.sha256(header).hexdigest()),
    "envelope-sha256": (vector.get("envelope_sha256"), hashlib.sha256(envelope).hexdigest()),
    "accepted-bytes": (accepted, envelope),
}
for name, (found, expected) in checks.items():
    if found != expected:
        problems.append(f"{name}: found={found!r} expected={expected!r}")

if problems:
    raise SystemExit("EXEC_PACKAGE_ENVELOPE_V2: FAIL\n  - " + "\n  - ".join(problems))

print(
    json.dumps(
        {
            "envelope_bytes": len(envelope),
            "package_digest": hashlib.sha256(header).hexdigest(),
            "result": "PASS",
        },
        indent=2,
        sort_keys=True,
    )
)

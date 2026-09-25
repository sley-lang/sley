#!/usr/bin/env python3
"""Independently reconstruct the frozen SLEYEP01 bootstrap vector."""

from __future__ import annotations

import json
from pathlib import Path

from blake3 import blake3


ROOT = Path(__file__).resolve().parents[1]
VECTOR = json.loads(
    (ROOT / "conformance/schema-epoch/v1/bootstrap.json").read_text(encoding="utf-8")
)


def uvar(value: int) -> bytes:
    output = bytearray()
    while True:
        group = value & 0x7F
        value >>= 7
        output.append(group | (0x80 if value else 0))
        if not value:
            return bytes(output)


def sized(value: bytes) -> bytes:
    return uvar(len(value)) + value


def record(fields: tuple[tuple[int, bytes], ...]) -> bytes:
    return uvar(len(fields)) + b"".join(
        uvar(tag) + sized(value) for tag, value in fields
    )


unicode_version = record(
    tuple(
        (tag + 1, uvar(value))
        for tag, value in enumerate(VECTOR["unicode_nfc_version"])
    )
)
limits = record(
    tuple((tag + 1, uvar(value)) for tag, value in enumerate(VECTOR["limits"]))
)
epoch_record = record(
    (
        (1, uvar(VECTOR["epoch_number"])),
        (2, uvar(VECTOR["scb_format_version"])),
        (3, uvar(VECTOR["hash_algorithm_tag"])),
        (4, unicode_version),
        (5, limits),
        (6, b"\x00"),
        (7, b"\x00"),
        (8, b"\x00\x00"),
        (9, b"\x00"),
    )
)
preimage = b"SLEYEP01" + uvar(1) + sized(epoch_record)
epoch_id = blake3(b"sley2.schema-epoch.v1" + preimage).hexdigest()

def epoch_of(fields, *, domain=b"sley2.schema-epoch.v1"):
    """Derive an epoch identity from one field set, for separation tests."""
    body = record(fields)
    return blake3(domain + b"SLEYEP01" + uvar(1) + sized(body)).hexdigest()


BASE_FIELDS = (
    (1, uvar(VECTOR["epoch_number"])),
    (2, uvar(VECTOR["scb_format_version"])),
    (3, uvar(VECTOR["hash_algorithm_tag"])),
    (4, unicode_version),
    (5, limits),
    (6, b"\x00"),
    (7, b"\x00"),
    (8, b"\x00\x00"),
    (9, b"\x00"),
)

# An epoch identity must separate every fact it freezes. One changed field
# must change the identity, or two epochs could claim the same identity while
# declaring different limits, contracts, or algorithms.
separation: dict[str, str] = {}
for index, (tag, value) in enumerate(BASE_FIELDS):
    altered = list(BASE_FIELDS)
    altered[index] = (tag, uvar(9) if value.startswith(b"\x00") or len(value) <= 2 else value + b"\x00")
    separation[f"field-{tag}"] = epoch_of(tuple(altered))
separation["other-domain"] = epoch_of(BASE_FIELDS, domain=b"sley2.state-root.v1")

problems: list[str] = []
problems += [
    f"epoch-collision:{name}" for name, derived in separation.items() if derived == epoch_id
]
if len(set(separation.values())) != len(separation):
    problems.append("two variations share one epoch identity")
if epoch_record.hex() != VECTOR["record_hex"]:
    problems.append("canonical epoch record differs")
if preimage.hex() != VECTOR["preimage_hex"]:
    problems.append("bootstrap preimage differs")
if epoch_id != VECTOR["schema_epoch_id"]:
    problems.append("SchemaEpochId differs")

result = {
    "contract": "s20-140-independent-bootstrap-vector-v1",
    "result": "FAIL" if problems else "PASS",
    "record_bytes": len(epoch_record),
    "preimage_bytes": len(preimage),
    "schema_epoch_id": epoch_id,
    "separated_inputs": len(separation),
    "problems": problems,
}
print(json.dumps(result, indent=2, sort_keys=True))
if problems:
    raise SystemExit(1)

#!/usr/bin/env python3
"""Build the S20-170 pack rejection corpus from the accepted artifact.

The pack family had one accepted vector and no rejection matrix, so the
independent decoder's strictness was never demonstrated: a permissive oracle
would have passed the same single artifact. Each mutation below is a byte
edit of the accepted pack with the exact reason the strict decoder must give.

Mutations that leave the trailer alone are refused by the content address.
The resealed ones recompute the trailer, so only the inner rule is violated,
which is the case that actually tests structure rather than hashing.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/repository-pack/v1"
ACCEPTED = FIXTURES / "accepted.json"
REJECTED = FIXTURES / "rejected.json"
SUMS = FIXTURES / "SHA256SUMS"
PACK_DOMAIN = b"sley2.repository-pack.v1"
# The accepted artifact's payload region, derived below rather than assumed.
CONTRACT = "sley2-repository-pack-rejected-v1"


def blake3_digest(domain: bytes, payload: bytes) -> bytes:
    import blake3

    hasher = blake3.blake3()
    hasher.update(domain)
    hasher.update(payload)
    return hasher.digest()


def reseal(data: bytes) -> bytes:
    """Recompute the pack trailer so only an inner rule stays violated."""
    return data[:-32] + blake3_digest(PACK_DOMAIN, data[:-32])


def payload_region(stored: bytes) -> tuple[int, int]:
    """The payload's byte span: after the envelope header, before the trailer."""
    offset = 8
    for _ in range(2):  # format version and contract tag, each one uvar
        while stored[offset] & 0x80:
            offset += 1
        offset += 1
    offset += 32  # pack epoch
    length = 0
    shift = 0
    while True:
        byte = stored[offset]
        offset += 1
        length |= (byte & 0x7F) << shift
        if not byte & 0x80:
            break
        shift += 7
    return offset, offset + length


def mutations(stored: bytes) -> list[dict[str, object]]:
    start, end = payload_region(stored)
    rows: list[tuple[str, bytes, str]] = [
        ("magic", b"SLEYSCB2" + stored[8:], "envelope"),
        ("contract-tag", stored[:9] + bytes([0x02]) + stored[10:], "envelope"),
        ("truncated", stored[:-1], "truncated"),
        ("trailing-byte", stored + b"\x00", "trailing"),
        (
            "flip-trailer",
            stored[:-1] + bytes([stored[-1] ^ 0x01]),
            "pack-digest",
        ),
        (
            "flip-epoch",
            stored[:11] + bytes([stored[11] ^ 0x01]) + stored[12:],
            "pack-digest",
        ),
        (
            "flip-payload",
            stored[:start] + bytes([stored[start] ^ 0x01]) + stored[start + 1 :],
            "pack-digest",
        ),
        (
            "resealed-payload-bit",
            reseal(stored[: start + 40] + bytes([stored[start + 40] ^ 0x01]) + stored[start + 41 :]),
            "content-id",
        ),
        (
            # A bit inside the digest tree: the entries still address their
            # content, so the decoder reaches the tree comparison.
            "resealed-late-bit",
            reseal(stored[: end - 40] + bytes([stored[end - 40] ^ 0x01]) + stored[end - 39 :]),
            "tree",
        ),
        (
            # The envelope's epoch no longer names any section-1 entry.
            "resealed-epoch",
            reseal(stored[:11] + bytes([stored[11] ^ 0x01]) + stored[12:]),
            "pack-epoch",
        ),
    ]
    return [
        {
            "expected_reason": reason,
            "id": name,
            "input_hex": data.hex(),
            "input_sha256": hashlib.sha256(data).hexdigest(),
            "seed": "accepted",
        }
        for name, data, reason in rows
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
    stored = bytes.fromhex(accepted["stored_hex"])
    rejected = {
        "claim": "s20-170-repository-pack-rejection-conformance",
        "contract": CONTRACT,
        "mutations": mutations(stored),
    }
    rendered = {REJECTED: json.dumps(rejected, indent=2, sort_keys=True) + "\n"}
    rendered[SUMS] = "".join(
        f"{hashlib.sha256(text.encode()).hexdigest()}  {path.name}\n"
        for path, text in (
            (ACCEPTED, ACCEPTED.read_text(encoding="utf-8")),
            (REJECTED, rendered[REJECTED]),
        )
    )
    if arguments.check:
        drift = [
            str(path.relative_to(ROOT))
            for path, text in rendered.items()
            if not path.is_file() or path.read_text(encoding="utf-8") != text
        ]
        print(json.dumps({"drift": drift, "mode": "check", "result": "FAIL" if drift else "PASS"}, sort_keys=True))
        return 1 if drift else 0
    for path, text in rendered.items():
        path.write_text(text, encoding="utf-8")
    print(json.dumps({"mode": "write", "mutations": len(rejected["mutations"]), "result": "PASS"}, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())

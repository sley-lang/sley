#!/usr/bin/env python3
"""Independently reproduce the S20-300 full complete-root snapshot record.

The oracle rebuilds the arm-2 `SLEYIDX1` record of
`COMPLETE_ROOT_INDEX_SNAPSHOT_PROFILE_V1.md` sections 1 and 2 from the frozen
S20-250 complete-entity fixture: the inventory is that fixture's entity
identities and SSMC1 kinds in raw identity order, the direct edges are its
frozen canonical edge set, and the reverse groups are their exact inversion.
It derives the `IndexSnapshotId` under `sley2.index-snapshot.v1` and then
classifies every rejected candidate with its own bounded inspector in
contract order. It shares no code with the Rust implementation.
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

import blake3


ROOT = Path(__file__).resolve().parents[1]
ACCEPTED = ROOT / "conformance/complete-root-index-snapshot/v1/accepted.json"
REJECTED = ROOT / "conformance/complete-root-index-snapshot/v1/rejected.json"
SOURCE = ROOT / "conformance/complete-entity-impact/v1/accepted.json"

DOMAIN = b"sley2.index-snapshot.v1"
MAGIC = b"SLEYIDX1"
FORMAT_VERSION = 1
PROFILE_VERSION = 1
LIMITS_PROFILE = 1
OPTION_NONE = 1
OPTION_SOME = 2
COMPLETE_ROOT_ARM = 2
MIN_RECORD_BYTES = 148
MAX_RECORD_BYTES = 67_108_864
# Frozen SSMC1 field schema hash (S20-250 fingerprint preimage constant).
FIELD_SCHEMA_HASH = bytes.fromhex(
    "1983bc8d6ad9ac3cb5390853f43959cf2c3dc0ae8e0ca18ca8264ca4960133ae"
)
CODES = {
    "INDEX_SNAPSHOT_PROFILE_UNSUPPORTED": 30000,
    "INDEX_SNAPSHOT_FORMAT_INVALID": 30001,
    "INDEX_SNAPSHOT_VERSION_UNSUPPORTED": 30002,
    "INDEX_SNAPSHOT_CONTEXT_MISMATCH": 30003,
    "INDEX_SNAPSHOT_DIGEST_MISMATCH": 30004,
    "INDEX_SNAPSHOT_COMPLETENESS_UNSUPPORTED": 30005,
    "INDEX_SNAPSHOT_RESOURCE_LIMIT": 30006,
    "INDEX_SNAPSHOT_ROOT_MISMATCH": 30009,
}


class Failure(Exception):
    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def u32(value: int) -> bytes:
    return struct.pack(">I", value)


def u64(value: int) -> bytes:
    return struct.pack(">Q", value)


def build_record(epoch: bytes, root: bytes, request: dict, edges: list[list]) -> bytes:
    inventory = [(bytes.fromhex(entity["id"]), entity["kind"]) for entity in request["entities"]]
    if [entry[0] for entry in inventory] != sorted(entry[0] for entry in inventory):
        raise Failure("inventory-not-raw-identity-order")
    direct = [(bytes.fromhex(a), bytes.fromhex(b), kind) for a, b, kind in edges]
    if direct != sorted(direct):
        raise Failure("edges-not-canonical")
    reverse: dict[bytes, list[tuple[bytes, int]]] = {}
    for dependent, dependency, kind in direct:
        reverse.setdefault(dependency, []).append((dependent, kind))
    preimage = bytearray()
    preimage += MAGIC + u32(FORMAT_VERSION) + u32(PROFILE_VERSION)
    preimage += epoch + FIELD_SCHEMA_HASH + u32(LIMITS_PROFILE)
    preimage += u32(OPTION_SOME) + root
    preimage += u32(COMPLETE_ROOT_ARM)
    preimage += u64(len(inventory))
    for entity, kind in inventory:
        preimage += entity + u32(kind)
    preimage += u64(len(direct))
    for dependent, dependency, kind in direct:
        preimage += dependent + dependency + u32(kind)
    preimage += u64(len(reverse))
    for dependency in sorted(reverse):
        preimage += dependency + u64(len(reverse[dependency]))
        for dependent, kind in reverse[dependency]:
            preimage += dependent + u32(kind)
    return bytes(preimage) + blake3.blake3(DOMAIN + bytes(preimage)).digest()


class Cursor:
    def __init__(self, data: bytes):
        self.data = data
        self.offset = 0

    def take(self, count: int) -> bytes:
        if self.offset + count > len(self.data):
            raise Failure("INDEX_SNAPSHOT_FORMAT_INVALID")
        value = self.data[self.offset : self.offset + count]
        self.offset += count
        return value

    def u32(self) -> int:
        return struct.unpack(">I", self.take(4))[0]

    def u64(self) -> int:
        return struct.unpack(">Q", self.take(8))[0]


def inspect(candidate: bytes, epoch: bytes, root: bytes) -> None:
    """Bounded inspection of an arm-2 candidate against the expected context."""
    if len(candidate) < MIN_RECORD_BYTES:
        raise Failure("INDEX_SNAPSHOT_FORMAT_INVALID")
    if len(candidate) > MAX_RECORD_BYTES:
        raise Failure("INDEX_SNAPSHOT_RESOURCE_LIMIT")
    preimage, trailer = candidate[:-32], candidate[-32:]
    cursor = Cursor(preimage)
    if cursor.take(8) != MAGIC:
        raise Failure("INDEX_SNAPSHOT_FORMAT_INVALID")
    if cursor.u32() != FORMAT_VERSION or cursor.u32() != PROFILE_VERSION:
        raise Failure("INDEX_SNAPSHOT_VERSION_UNSUPPORTED")
    found_epoch = cursor.take(32)
    found_schema = cursor.take(32)
    if cursor.u32() != LIMITS_PROFILE:
        raise Failure("INDEX_SNAPSHOT_PROFILE_UNSUPPORTED")
    option = cursor.u32()
    if option == OPTION_NONE:
        found_root = None
    elif option == OPTION_SOME:
        found_root = cursor.take(32)
    else:
        raise Failure("INDEX_SNAPSHOT_FORMAT_INVALID")
    if found_epoch != epoch or found_schema != FIELD_SCHEMA_HASH or found_root != root:
        raise Failure("INDEX_SNAPSHOT_CONTEXT_MISMATCH")
    if cursor.u32() != COMPLETE_ROOT_ARM:
        raise Failure("INDEX_SNAPSHOT_COMPLETENESS_UNSUPPORTED")
    for _ in range(cursor.u64()):
        cursor.take(32)
        if not 1 <= cursor.u32() <= 18:
            raise Failure("INDEX_SNAPSHOT_FORMAT_INVALID")
    for _ in range(cursor.u64()):
        cursor.take(68)
    for _ in range(cursor.u64()):
        cursor.take(32)
        for _ in range(cursor.u64()):
            cursor.take(36)
    if cursor.offset != len(preimage):
        raise Failure("INDEX_SNAPSHOT_FORMAT_INVALID")
    if blake3.blake3(DOMAIN + preimage).digest() != trailer:
        raise Failure("INDEX_SNAPSHOT_DIGEST_MISMATCH")


def main() -> int:
    accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    source = json.loads(SOURCE.read_text(encoding="utf-8"))
    problems: list[str] = []
    if accepted.get("contract") != "sley2-complete-root-index-snapshot-v1":
        problems.append("accepted-contract")
    if accepted.get("domain") != DOMAIN.decode():
        problems.append("accepted-domain")
    source_vector = source["vectors"][0]
    for vector in accepted.get("vectors", []):
        if vector["source_request_sha256"] != source_vector["request_sha256"]:
            problems.append(f"{vector['id']}:source-request-drift")
        epoch = bytes.fromhex(vector["schema_epoch_hex"])
        root = bytes.fromhex(vector["root_hex"])
        try:
            record = build_record(
                epoch, root, source_vector["request"], source_vector["expected"]["direct_edges"]
            )
        except Failure as failure:
            problems.append(f"{vector['id']}:build:{failure.code}")
            continue
        if record.hex() != vector["record_hex"]:
            problems.append(f"{vector['id']}:record")
        if len(record) != vector["record_bytes"]:
            problems.append(f"{vector['id']}:record_bytes")
        if record[-32:].hex() != vector["snapshot_id"]:
            problems.append(f"{vector['id']}:snapshot_id")
        if len(source_vector["request"]["entities"]) != vector["inventory_count"]:
            problems.append(f"{vector['id']}:inventory_count")
        if vector["completeness_arm"] != COMPLETE_ROOT_ARM:
            problems.append(f"{vector['id']}:completeness_arm")
        try:
            inspect(record, epoch, root)
        except Failure as failure:
            problems.append(f"{vector['id']}:self-inspect:{failure.code}")
        for mutation in rejected.get("mutations", []):
            candidate = bytes.fromhex(mutation["candidate_hex"])
            try:
                inspect(candidate, epoch, root)
            except Failure as failure:
                if failure.code != mutation["expected_code"]:
                    problems.append(f"{mutation['id']}:code:{failure.code}")
                elif CODES[failure.code] != mutation["expected_numeric"]:
                    problems.append(f"{mutation['id']}:numeric")
            else:
                if candidate == record:
                    problems.append(f"{mutation['id']}:identical-to-record")
                else:
                    problems.append(f"{mutation['id']}:accepted")
    print(
        json.dumps(
            {
                "contract": "s20-300-full-complete-root-index-snapshot-oracle-v1",
                "mutations": len(rejected.get("mutations", [])),
                "problems": problems,
                "result": "PASS" if not problems else "FAIL",
                "vectors": len(accepted.get("vectors", [])),
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

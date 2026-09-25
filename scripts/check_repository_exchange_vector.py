#!/usr/bin/env python3
"""Independently reproduce the S20-540 exchange ID, leaves, and digest tree."""

from __future__ import annotations

import json
from pathlib import Path

import blake3


ROOT = Path(__file__).resolve().parents[1]
VECTOR = ROOT / "conformance/repository-exchange/v1/accepted.json"
REJECTED = ROOT / "conformance/repository-exchange/v1/rejected.json"
EXCHANGE_DOMAIN = b"sley2.repository-exchange.v1"
PACK_DOMAIN = b"sley2.repository-pack.v1"
HEAD_DOMAIN = b"sley2.accepted-head.v1"
NAME_KEY_DOMAIN = b"sley2.branch-name-path.v1"
LEAF_DOMAIN = b"sley2.repository-exchange-leaf.v1"
NODE_DOMAIN = b"sley2.repository-exchange-node.v1"
MAGIC = b"SLEYSCB1"
HEAD_MAGIC = b"SLEYHD01"
NAME_KEY_MAGIC = b"SLEYBNM1"
CONTRACT_TAG = 540
PACK_CONTRACT_TAG = 170
MAX_EXCHANGE_BYTES = 67_108_864
MAX_EMBEDDED_PACK_BYTES = 16_777_216
MAX_RECEIPTS = 4_096
MAX_BRANCHES = 4_096
MAX_LEAVES = 8_194


class DecodeError(ValueError):
    """Strict vector decode failure."""


def encode_uvar(value: int) -> bytes:
    if value < 0:
        raise DecodeError("negative")
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def encode_bytes(value: bytes) -> bytes:
    return encode_uvar(len(value)) + value


class Reader:
    def __init__(self, data: bytes):
        self.data = data
        self.offset = 0

    def take(self, count: int) -> bytes:
        end = self.offset + count
        if end > len(self.data):
            raise DecodeError("truncated")
        value = self.data[self.offset : end]
        self.offset = end
        return value

    def uvar(self) -> int:
        value = 0
        start = self.offset
        for shift in range(0, 70, 7):
            byte = self.take(1)[0]
            if shift == 63 and byte > 1:
                raise DecodeError("overflow")
            value |= (byte & 0x7F) << shift
            if not byte & 0x80:
                if self.data[start : self.offset] != encode_uvar(value):
                    raise DecodeError("nonminimal")
                return value
        raise DecodeError("overflow")

    def sized(self) -> bytes:
        return self.take(self.uvar())

    def remaining(self) -> int:
        return len(self.data) - self.offset

    def finish(self) -> None:
        if self.offset != len(self.data):
            raise DecodeError("trailing")


def decode_record(data: bytes) -> dict[int, bytes]:
    reader = Reader(data)
    count = reader.uvar()
    fields: dict[int, bytes] = {}
    previous = 0
    for _ in range(count):
        tag = reader.uvar()
        if tag <= previous:
            raise DecodeError("field-order")
        fields[tag] = reader.sized()
        previous = tag
    reader.finish()
    return fields


def decode_list(data: bytes, maximum: int) -> list[bytes]:
    reader = Reader(data)
    count = reader.uvar()
    if count > maximum:
        raise DecodeError("count")
    values = [reader.sized() for _ in range(count)]
    reader.finish()
    return values


def single_uvar(data: bytes) -> int:
    reader = Reader(data)
    value = reader.uvar()
    reader.finish()
    return value


def fixed32(data: bytes) -> bytes:
    if len(data) != 32:
        raise DecodeError("fixed32")
    return data


def decode_envelope(stored: bytes, contract_tag: int, domain: bytes) -> tuple[bytes, bytes, bytes]:
    if len(stored) > MAX_EXCHANGE_BYTES:
        raise DecodeError("stored-limit")
    reader = Reader(stored)
    if reader.take(8) != MAGIC:
        raise DecodeError("magic")
    if reader.uvar() != 1:
        raise DecodeError("version")
    if reader.uvar() != contract_tag:
        raise DecodeError("contract")
    epoch = reader.take(32)
    payload = reader.sized()
    trailer = reader.take(32)
    reader.finish()
    preimage = stored[: len(stored) - 32]
    derived = blake3.blake3(domain + preimage).digest()
    if derived != trailer:
        raise DecodeError("trailer")
    return epoch, payload, derived


def stored_head(transaction_id: bytes) -> bytes:
    prefix = HEAD_MAGIC + encode_uvar(1) + transaction_id
    return prefix + blake3.blake3(HEAD_DOMAIN + prefix).digest()


def name_key(name: bytes) -> bytes:
    preimage = NAME_KEY_MAGIC + encode_uvar(1) + encode_bytes(name)
    return blake3.blake3(NAME_KEY_DOMAIN + preimage).digest()


def leaf(section: int, identifier: bytes, payload: bytes) -> bytes:
    return blake3.blake3(
        LEAF_DOMAIN + encode_uvar(section) + identifier + encode_uvar(len(payload)) + payload
    ).digest()


def merkle_root(leaves: list[bytes]) -> bytes:
    level = list(leaves)
    if not level or len(level) > MAX_LEAVES:
        raise DecodeError("leaves")
    while len(level) > 1:
        next_level = []
        for index in range(0, len(level), 2):
            if index + 1 < len(level):
                next_level.append(
                    blake3.blake3(NODE_DOMAIN + level[index] + level[index + 1]).digest()
                )
            else:
                next_level.append(level[index])
        level = next_level
    return level[0]


def reproduce(stored: bytes) -> dict[str, object]:
    _, payload, exchange_id = decode_envelope(stored, CONTRACT_TAG, EXCHANGE_DOMAIN)
    fields = decode_record(payload)
    if sorted(fields) != [1, 2, 3, 4, 5, 6, 7, 8]:
        raise DecodeError("fields")
    if single_uvar(fields[1]) != 1:
        raise DecodeError("exchange-version")
    object_pack = fields[2]
    if len(object_pack) > MAX_EMBEDDED_PACK_BYTES:
        raise DecodeError("pack-limit")
    _, _, pack_id = decode_envelope(object_pack, PACK_CONTRACT_TAG, PACK_DOMAIN)
    receipt_elements = decode_list(fields[3], MAX_RECEIPTS)
    receipts = []
    previous_id: bytes | None = None
    for element in receipt_elements:
        record = decode_record(element)
        if sorted(record) != [1, 2, 3]:
            raise DecodeError("receipt-entry")
        transaction_id = fixed32(record[1])
        if previous_id is not None and transaction_id <= previous_id:
            raise DecodeError("receipt-order")
        previous_id = transaction_id
        receipts.append((transaction_id, fixed32(record[2]), record[3]))
    if not receipts:
        raise DecodeError("no-receipts")
    head_record = decode_record(fields[4])
    if sorted(head_record) != [1, 2]:
        raise DecodeError("head-entry")
    head_transaction = fixed32(head_record[1])
    head_receipt = fixed32(head_record[2])
    branch_elements = decode_list(fields[5], MAX_BRANCHES)
    branches = []
    previous_element: bytes | None = None
    for element in branch_elements:
        if previous_element is not None and element <= previous_element:
            raise DecodeError("branch-order")
        previous_element = element
        record = decode_record(element)
        if sorted(record) != [1, 2, 3]:
            raise DecodeError("branch-entry")
        branches.append((record[1], record[2], record[3]))
    if single_uvar(fields[6]) != 0:
        raise DecodeError("compression")
    tree = decode_record(fields[7])
    if sorted(tree) != [1, 2, 3, 4] or single_uvar(tree[1]) != 1:
        raise DecodeError("tree")
    leaf_count = single_uvar(tree[2])
    stored_leaves = [fixed32(value) for value in decode_list(tree[3], MAX_LEAVES)]
    stored_root = fixed32(tree[4])
    signature = Reader(fields[8])
    if signature.uvar() != 0 or signature.sized() != b"":
        raise DecodeError("signature")
    signature.finish()

    leaves = [leaf(1, pack_id, object_pack)]
    for transaction_id, _, stored_receipt in receipts:
        leaves.append(leaf(2, transaction_id, stored_receipt))
    for name, origin, reference in branches:
        leaves.append(leaf(3, name_key(name), encode_bytes(origin) + encode_bytes(reference)))
    leaves.append(leaf(4, head_transaction, stored_head(head_transaction) + head_receipt))
    if leaf_count != len(leaves) or stored_leaves != leaves:
        raise DecodeError("leaf-mismatch")
    root = merkle_root(leaves)
    if root != stored_root:
        raise DecodeError("root-mismatch")
    if not any(transaction_id == head_transaction for transaction_id, _, _ in receipts):
        raise DecodeError("head-not-in-receipts")
    return {
        "accepted_head_transaction_id_hex": head_transaction.hex(),
        "branches": len(branches),
        "digest_tree_root": root.hex(),
        "receipts": len(receipts),
        "repository_exchange_id": exchange_id.hex(),
        "repository_pack_id": pack_id.hex(),
        "stored_bytes": len(stored),
    }


def main() -> int:
    vector = json.loads(VECTOR.read_text(encoding="utf-8"))
    problems: list[str] = []
    for entry in vector["vectors"]:
        stored = bytes.fromhex(entry["exchange_hex"])
        try:
            reproduced = reproduce(stored)
        except DecodeError as error:
            problems.append(f"{entry['id']}:decode:{error}")
            continue
        for key, value in reproduced.items():
            if entry.get(key) != value:
                problems.append(f"{entry['id']}:{key}")
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    for mutation in rejected["mutations"]:
        stored = bytes.fromhex(mutation["input_hex"])
        if mutation["id"] == "flip-trailer":
            try:
                reproduce(stored)
                problems.append(f"{mutation['id']}:accepted-tampered-trailer")
            except DecodeError:
                pass
        elif mutation["id"] == "reversed-branches":
            try:
                reproduce(stored)
                problems.append(f"{mutation['id']}:accepted-noncanonical-order")
            except DecodeError:
                pass
        elif mutation["id"] == "trailing-byte":
            try:
                reproduce(stored)
                problems.append(f"{mutation['id']}:accepted-trailing-byte")
            except DecodeError:
                pass
    print(
        json.dumps(
            {
                "contract": "s20-540-repository-exchange-v1",
                "problems": problems,
                "result": "PASS" if not problems else "FAIL",
                "vectors": len(vector["vectors"]),
            },
            indent=2,
            sort_keys=True,
        )
    )
    return int(bool(problems))


if __name__ == "__main__":
    raise SystemExit(main())

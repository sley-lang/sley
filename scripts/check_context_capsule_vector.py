#!/usr/bin/env python3
"""Independently reproduce the S20-320 full context capsule vectors.

The oracle rebuilds every `SLEYCCP1` record and `ContextCapsuleId` of
`CONTEXT_CAPSULE_PROFILE_V1.md` from the frozen S20-310 fixture alone: the
question (class, body, limits, continuation flag, cursor), the provenance
context, and the exact `SLEYRQR1` response record, whose payload it decodes
by class to derive the entity, kind, relationship, root, object, and
fingerprint facts. It shares no code with the Rust implementation.
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

import blake3


ROOT = Path(__file__).resolve().parents[1]
ACCEPTED = ROOT / "conformance/context-capsule/v1/accepted.json"
SOURCE = ROOT / "conformance/root-backed-query/v1/accepted.json"

DOMAIN = b"sley2.context-capsule.v1"
MAGIC = b"SLEYCCP1"
RESPONSE_MAGIC = b"SLEYRQR1"
SESSION_NONE = 1
SESSION_NEGOTIATED = 2
COMPLETE, PAGE = 1, 2
FLAG_FALSE, FLAG_TRUE = 1, 2
OPTION_NONE, OPTION_SOME = 1, 2
CURSOR_ENTITY, CURSOR_EDGE, CURSOR_ROOT = 1, 2, 3
ENTITY_CLASSES = {2, 3, 6, 7, 8, 9, 16, 17, 18, 19}
KIND_CLASSES = {4}
FILTER_CLASSES = {12, 13}
SEED_CLASSES = {14, 15}


def u32(value: int) -> bytes:
    return struct.pack(">I", value)


def u64(value: int) -> bytes:
    return struct.pack(">Q", value)


def h(value: str) -> bytes:
    return bytes.fromhex(value)


def encode_cursor(cursor: dict | None) -> bytes:
    if cursor is None:
        return u32(OPTION_NONE)
    out = u32(OPTION_SOME) + u32(cursor["tag"])
    if cursor["tag"] == CURSOR_ENTITY:
        return out + h(cursor["entity"])
    if cursor["tag"] == CURSOR_ROOT:
        return out + h(cursor["root"])
    return out + h(cursor["dependent"]) + h(cursor["dependency"]) + u32(cursor["kind"])


def encode_limits(limits: dict) -> bytes:
    return (
        u64(limits["max_returned_entities"])
        + u64(limits["max_returned_edges"])
        + u32(limits["max_depth"])
        + u64(limits["max_response_bytes"])
        + u64(limits["max_work"])
    )


def encode_question(vector: dict) -> bytes:
    query = vector["query"]
    cls = query["class"]
    out = bytearray(u32(cls))
    if cls in ENTITY_CLASSES:
        out += h(query["entity"])
    elif cls in KIND_CLASSES:
        out += u32(query["kind"])
    elif cls in FILTER_CLASSES:
        out += h(query["entity"]) + u64(len(query["kinds"]))
        for kind in query["kinds"]:
            out += u32(kind)
    elif cls in SEED_CLASSES:
        out += u64(len(query["seeds"]))
        for seed in query["seeds"]:
            out += h(seed)
    out += encode_limits(vector["limits"])
    out += u32(FLAG_TRUE if vector["allow_continuation"] else FLAG_FALSE)
    out += encode_cursor(vector["after"])
    return bytes(out)


def named_entities(query: dict) -> list[bytes]:
    cls = query["class"]
    if cls in ENTITY_CLASSES or cls in FILTER_CLASSES:
        return [h(query["entity"])]
    if cls in SEED_CLASSES:
        return [h(seed) for seed in query["seeds"]]
    return []


class Cursor:
    def __init__(self, data: bytes, offset: int):
        self.data = data
        self.offset = offset

    def take(self, count: int) -> bytes:
        value = self.data[self.offset : self.offset + count]
        if len(value) != count:
            raise ValueError("short record")
        self.offset += count
        return value

    def u32(self) -> int:
        return struct.unpack(">I", self.take(4))[0]

    def u64(self) -> int:
        return struct.unpack(">Q", self.take(8))[0]

    def option_cursor(self) -> None:
        tag = self.u32()
        if tag == OPTION_NONE:
            return
        kind = self.u32()
        self.take(32)
        if kind == CURSOR_EDGE:
            self.take(32)
            self.u32()


def decode_payload(record: bytes, cls: int) -> dict:
    """Decodes the `SLEYRQR1` header to the payload and extracts the facts."""
    cursor = Cursor(record, 0)
    if cursor.take(8) != RESPONSE_MAGIC:
        raise ValueError("response magic")
    cursor.u32(); cursor.u32()
    cursor.take(32 * 5)
    cursor.u32(); cursor.u32()
    cursor.take(36)
    cursor.u32()
    cursor.option_cursor()
    if cursor.u32() != cls:
        raise ValueError("class")
    cursor.u64(); cursor.u64(); cursor.u32()
    cursor.option_cursor()
    cursor.u32(); cursor.u64(); cursor.u64()
    if cursor.u32() != cls:
        raise ValueError("result tag")
    ids: list[bytes] = []
    kinds: dict[bytes, int] = {}
    edges: list[tuple[bytes, bytes, int]] = []
    roots: list[bytes] = []
    obj = None
    fingerprint = None
    if cls == 1:
        cursor.take(32 * 6 + 8 * 18 + 8 * 3)
    elif cls == 2:
        kind = cursor.u32()
        obj = cursor.take(32)
        if cursor.u32() == OPTION_SOME:
            fingerprint = cursor.take(32)
        kinds["subject"] = kind
    elif cls == 3:
        if cursor.u32() == OPTION_SOME:
            fingerprint = cursor.take(32)
    elif cls == 7:
        for _ in range(cursor.u64()):
            binding = cursor.take(32)
            roots.append(cursor.take(32))
            ids.extend([binding, cursor.take(32), cursor.take(32)])
    elif cls == 8:
        for _ in range(cursor.u64()):
            entity = cursor.take(32)
            kinds[entity] = cursor.u32()
            ids.append(entity)
    elif cls == 10:
        for _ in range(cursor.u64()):
            ids.append(cursor.take(32))
            ids.append(cursor.take(32))
            cursor.u32()
    elif cls == 11:
        for _ in range(cursor.u64()):
            roots.append(cursor.take(32))
    elif cls in (12, 13):
        for _ in range(cursor.u64()):
            dependent, dependency, kind = cursor.take(32), cursor.take(32), cursor.u32()
            edges.append((dependent, dependency, kind))
            ids.extend([dependent, dependency])
    else:
        for _ in range(cursor.u64()):
            ids.append(cursor.take(32))
    if cursor.offset != len(record):
        raise ValueError("trailing bytes")
    return {"ids": ids, "kinds": kinds, "edges": edges, "roots": roots, "object": obj, "fingerprint": fingerprint}


def build(context: dict, vector: dict) -> tuple[bytes, dict]:
    record = h(vector["record_hex"])
    cls = vector["query"]["class"]
    facts = decode_payload(record, cls)
    subjects = named_entities(vector["query"])
    binding = vector.get("session_binding", SESSION_NONE)
    if binding == SESSION_NEGOTIATED and "session_id" not in vector:
        raise ValueError("negotiated arm without a session identity")
    entities = sorted(set(facts["ids"]) | set(subjects))
    kinds = [0] * len(entities)
    index = {entity: position for position, entity in enumerate(entities)}
    for entity, kind in facts["kinds"].items():
        if entity == "subject":
            kinds[index[subjects[0]]] = kind
        else:
            kinds[index[entity]] = kind
    relationships = [(index[a], index[b], kind) for a, b, kind in facts["edges"]]
    roots = sorted(set(facts["roots"]))
    objects = [(index[subjects[0]], facts["object"])] if facts["object"] is not None else []
    fingerprints = [(index[subjects[0]], facts["fingerprint"])] if facts["fingerprint"] is not None else []
    total, returned = vector["total_count"], vector["returned"]
    # The fixture schema carries no truncation flag: truncated is derived
    # from `next_after`, which is sound because the builder enforces
    # `truncated == next_after.is_some()` on every pair (contract section
    # 6, `validate_source` rule).
    truncated = vector["next_after"] is not None
    completeness = COMPLETE if not truncated and vector["after"] is None else PAGE
    omitted = total - returned
    out = bytearray(MAGIC + u32(1) + u32(1))
    out += h(context["workspace_id"]) + h(context["schema_epoch_hex"]) + h(context["root_hex"])
    out += h(context["snapshot_id"]) + h(vector["query_id"])
    if binding == SESSION_NEGOTIATED:
        out += u32(SESSION_NEGOTIATED) + h(vector["session_id"])
    else:
        out += u32(SESSION_NONE)
    out += encode_question(vector)
    out += u32(completeness) + u32(FLAG_TRUE if truncated else FLAG_FALSE)
    out += u64(total) + u64(returned) + u64(omitted) + encode_cursor(vector["next_after"])
    # Contract section 6: the copied record is length-prefixed with
    # `u64be(response_bytes)`; `bytes(x)` carries no prefix.
    out += u64(len(record)) + record
    out += u64(len(entities)) + b"".join(entities)
    out += u64(len(kinds)) + b"".join(u32(kind) for kind in kinds)
    out += u64(len(relationships)) + b"".join(u32(a) + u32(b) + u32(kind) for a, b, kind in relationships)
    out += u64(len(roots)) + b"".join(roots)
    out += u64(len(objects)) + b"".join(u32(i) + obj for i, obj in objects)
    out += u64(len(fingerprints)) + b"".join(u32(i) + fp for i, fp in fingerprints)
    preimage = bytes(out)
    capsule = preimage + blake3.blake3(DOMAIN + preimage).digest()
    return capsule, {"completeness": completeness, "omitted": omitted, "total": total, "returned": returned}


def total_and_returned(record: bytes) -> tuple[int, int]:
    cursor = Cursor(record, 0)
    cursor.take(8); cursor.u32(); cursor.u32(); cursor.take(160); cursor.u32(); cursor.u32(); cursor.take(36); cursor.u32()
    cursor.option_cursor(); cursor.u32()
    return cursor.u64(), cursor.u64()


def main() -> int:
    accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
    source = json.loads(SOURCE.read_text(encoding="utf-8"))
    context = source["context"]
    by_id = {vector["id"]: vector for vector in source["vectors"]}
    problems: list[str] = []
    if accepted.get("contract") != "sley2-context-capsule-v1" or accepted.get("domain") != DOMAIN.decode():
        problems.append("accepted-contract")
    bound = 0
    for vector in accepted.get("vectors", []):
        binding = vector.get("session_binding", SESSION_NONE)
        if binding == SESSION_NEGOTIATED:
            bound += 1
            if "session_id" not in vector:
                problems.append(f"{vector['id']}:negotiated-without-session")
        elif binding != SESSION_NONE:
            problems.append(f"{vector['id']}:unknown-session-binding")
    if bound < 1:
        problems.append("vectors:no-negotiated-arm")
    for vector in accepted.get("vectors", []):
        source_vector = by_id.get(vector["source_vector"])
        if source_vector is None or source_vector["query_id"] != vector["query_id"]:
            problems.append(f"{vector['id']}:source")
            continue
        record = h(source_vector["record_hex"])
        total, returned = total_and_returned(record)
        merged = dict(
            source_vector,
            total_count=total,
            returned=returned,
            session_binding=vector.get("session_binding", SESSION_NONE),
        )
        if "session_id" in vector:
            merged["session_id"] = vector["session_id"]
        try:
            capsule, status = build(context, merged)
        except (ValueError, KeyError) as error:
            problems.append(f"{vector['id']}:build:{error}")
            continue
        if capsule.hex() != vector["record_hex"]:
            problems.append(f"{vector['id']}:record")
        if capsule[-32:].hex() != vector["capsule_id"]:
            problems.append(f"{vector['id']}:capsule_id")
        if len(capsule) != vector["record_bytes"]:
            problems.append(f"{vector['id']}:record_bytes")
        for key, expected in (("completeness", vector["completeness"]), ("omitted", vector["omitted"]), ("total", vector["total_count"]), ("returned", vector["returned"])):
            if status[key] != expected:
                problems.append(f"{vector['id']}:{key}")
        if vector["id"].startswith("page-") and vector["completeness"] != PAGE:
            problems.append(f"{vector['id']}:page-not-page")
        if vector["id"].startswith("class-") and vector["completeness"] != COMPLETE:
            problems.append(f"{vector['id']}:class-not-complete")
    classes = sorted({by_id[v["source_vector"]]["query"]["class"] for v in accepted.get("vectors", []) if v["source_vector"] in by_id})
    if classes != list(range(1, 20)):
        problems.append("vectors:class-coverage")
    print(
        json.dumps(
            {
                "contract": "s20-320-full-context-capsule-oracle-v1",
                "problems": problems,
                "query_classes": len(classes),
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

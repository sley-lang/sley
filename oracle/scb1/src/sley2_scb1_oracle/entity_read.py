"""Independent Python entity-read (S20-310 v2) vector oracle, stage A.

Stage A scope: hand-authored semantic inputs
(`conformance/entity-read/v2/inputs.json`) are the ONLY source. This module
reconstructs every expected byte string from those inputs plus frozen
schema/descriptor constants using the existing `codec` primitives and the
existing `mutation_value` body encoders. It never invokes Rust, never reads
emitted epochs, objects, IDs, or malformed bytes from any Rust emitter or
generated corpus, and never materializes the generated corpus itself.

Normal checking is read-only: `check_accepted` / `check_rejected` rebuild
expectations from the semantic inputs and compare them byte-for-byte against `accepted.json` /
`rejected.json`. The explicit refresh entrypoint `refresh` derives those two
files plus `SHA256SUMS` from the semantic inputs into a caller-supplied
output directory outside the repository pin; it never overwrites expected
files in place.

Only the narrow structured adapter required for EntityObject, request /
response / frame records, BoundedContext, failure envelopes, hello /
selection transcripts, and context / signature / count / work checks lives
here. Body schema validity is delegated to
`mutation_value.decode_declared_mutation_value`, which validates and returns
None; this module is not a full semantic object validator and makes no
production provenance claim (synthetic opaque identities stay synthetic).

Root execution contract (this file is never executed by its author):
  uv run --project oracle/scb1 --frozen python3 \\
      scripts/check_entity_read_vectors.py
  uv run --project oracle/scb1 --frozen python3 \\
      scripts/check_entity_read_vectors.py --refresh --output-dir <staging>
  uv run --project oracle/scb1 --frozen python -m unittest discover -s oracle/scb1/tests -v
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path
from typing import Any, Mapping

import blake3
import unicodedata2

from .codec import encode_record, encode_sized, encode_uvar
from .errors import ScbError
from .mutation_value import (
    SOURCE_SCHEMA_BLAKE3,
    decode_declared_mutation_value,
    encode_mutation_value,
)



# Frozen contract pins (ENTITY_READ_PROFILE_V2.md, SMP1.md, SCHEMA_EPOCH_V1.md)


MAGIC = b"SLEYSCB1"
FORMAT_VERSION = 1
OBJECT_CONTRACT_TAG = 200
FRAME_CONTRACT_TAG = 400
OBJECT_DOMAIN = b"sley2.object.v1"
FRAME_DOMAIN = b"sley2.protocol-frame.v1"
HANDSHAKE_DOMAIN = b"sley2.protocol-handshake.v1"
EPOCH_DOMAIN = b"sley2.schema-epoch.v1"
BOOTSTRAP_MAGIC = b"SLEYEP01"
BOOTSTRAP_VERSION = 1

MAX_STANDALONE_BYTES = 67_108_864
MAX_BYTE_PAYLOAD = 16_777_216
MAX_NESTING_DEPTH = 64
MAX_RECORD_FIELDS = 65_535
MAX_COLLECTION_ELEMENTS = 1_000_000
U64_MAX = (1 << 64) - 1

# Protocol schema-epoch descriptor pins (sley-protocol, frozen constants).
PROTOCOL_DIGEST_DOMAIN_TAG = 22
PROTOCOL_FIELD_SCHEMA_HASH = (
    "d36cce861ab70cbbd021ff3d8020e71eb9e9684773f82734b6f2aa484937df0e"
)
PROTOCOL_DECODER_LIMITS_HASH = (
    "212b293d90646621c2e83caf095c2a21b6864888a3cc98878b7f6ab81ab98fe7"
)
EPOCH_LIMITS = [67_108_864, 16_777_216, 64, 65_535, 1_000_000, 1_000_000, 134_217_728]
EPOCH_UNICODE = [16, 0, 0]

PROTOCOL_VERSION_2 = 2
METHOD_ENTITY_VERSION = 306
METHOD_ENTITY_SIGNATURE = 307
RESPONSE_VERSION = 2
RESERVED_TAGS = (305, 503, 601, 602)
FEATURE_MASK = 0x1F

# Selected-limit ceilings (SMP1 ceilings for the eight LimitProfile fields).
LIMIT_CEILINGS = (
    67_108_864,  # max_frame_bytes
    65_535,  # max_entities
    400_000,  # max_edges
    65_535,  # max_depth
    67_108_864,  # max_response_bytes
    100_000_000,  # max_work
    1_024,  # max_inflight
    256,  # max_sessions
)

# Frozen failure numerics used by entity-read vectors.
CODES: dict[str, int] = {
    "PROTOCOL_VERSION_UNSUPPORTED": 40000,
    "PROTOCOL_FRAME_INVALID": 40001,
    "PROTOCOL_FRAME_TOO_LARGE": 40002,
    "PROTOCOL_NO_COMMON_PROFILE": 40003,
    "PROTOCOL_DOWNGRADE": 40004,
    "PROTOCOL_REQUEST_ID_CONFLICT": 40005,
    "PROTOCOL_SESSION_CLOSED": 40006,
    "PROTOCOL_METHOD_UNSUPPORTED": 40007,
    "PROTOCOL_PAYLOAD_INVALID": 40008,
    "PROTOCOL_LIMIT_EXCEEDED": 40009,
    "PROTOCOL_CANCELLED": 40010,
    "QUERY_UNRESOLVED_ENTITY": 31004,
    "QUERY_INTERNAL_INVARIANT": 31007,
    "QUERY_ROOT_MISMATCH": 31008,
    "QUERY_CLASS_NOT_APPLICABLE": 31010,
    "SESSION_UNKNOWN": 33000,
    "SESSION_WORKSPACE_MISMATCH": 33001,
    "SESSION_ROOT_ADVANCED": 33002,
    "SESSION_EPOCH_MISMATCH": 33003,
    "SESSION_STALE_HANDLE": 33004,
    "SESSION_HANDLE_UNKNOWN": 33005,
    "SESSION_RENEWAL_LIMIT": 33006,
    "SESSION_BINDING_INVALID": 33007,
}
LIMIT_CHANGE_SYMBOLS = {
    "PROTOCOL_LIMIT_EXCEEDED",
}
REQUERY_SYMBOLS = {
    "REF_CAS_STALE",
    "REF_NAMED_CAS_STALE",
    "SESSION_ROOT_ADVANCED",
    "SESSION_STALE_HANDLE",
    "STALE_ROOT",
}

# Entity kind tags 1..18 in schema order (SSMC1_EPOCH1_SCHEMA.txt lines 7-24).
KIND_NAMES = {
    1: "Workspace",
    2: "Package",
    3: "Namespace",
    4: "TypeDef",
    5: "Function",
    6: "Parameter",
    7: "Block",
    8: "Operation",
    9: "Constant",
    10: "GlobalValue",
    11: "EffectDef",
    12: "CapabilityRequirement",
    13: "Contract",
    14: "TestCase",
    15: "AdapterImport",
    16: "EntryPoint",
    17: "PolicyBinding",
    18: "DependencyBinding",
}
BODY_TYPES = {
    1: "WorkspaceBody",
    2: "PackageBody",
    3: "NamespaceBody",
    4: "TypeDefBody",
    5: "FunctionBody",
    6: "ParameterBody",
    7: "BlockBody",
    8: "OperationBody",
    9: "ConstantBody",
    10: "GlobalValueBody",
    11: "EffectDefBody",
    12: "CapabilityRequirementBody",
    13: "ContractBody",
    14: "TestCaseBody",
    15: "AdapterImportBody",
    16: "EntryPointBody",
    17: "PolicyBindingBody",
    18: "DependencyBindingBody",
}


class CheckFailed(Exception):
    """An oracle-layer semantic rejection (no new runtime code invented)."""

    def __init__(self, layer: str, detail: str) -> None:
        super().__init__(f"{layer}: {detail}")
        self.layer = layer
        self.detail = detail


class Overflow(Exception):
    """A checked-u64 arithmetic overflow in work / length preflight."""



# Strict bounded decoding (Rust-exact varint / length / record rules)



class Reader:
    def __init__(self, data: bytes) -> None:
        self.data = data
        self.pos = 0

    @property
    def remaining(self) -> int:
        return len(self.data) - self.pos

    def take(self, length: int) -> bytes:
        if length < 0 or length > self.remaining:
            raise ScbError("SCB_LENGTH_OVERFLOW")
        start = self.pos
        self.pos += length
        return self.data[start : self.pos]

    def uvar(self, width: int) -> int:
        value = 0
        shift = 0
        count = 0
        while True:
            if self.remaining == 0:
                raise ScbError("SCB_LENGTH_OVERFLOW")
            byte = self.take(1)[0]
            count += 1
            payload = byte & 0x7F
            if shift >= 64 and payload != 0:
                raise ScbError("SCB_INTEGER_OVERFLOW")
            if shift < 64:
                if shift == 63 and payload > 1:
                    raise ScbError("SCB_INTEGER_OVERFLOW")
                value |= payload << shift
            if byte & 0x80 == 0:
                if count > 1 and payload == 0:
                    raise ScbError("SCB_VARINT_NON_MINIMAL")
                if width < 64 and value >= (1 << width):
                    raise ScbError("SCB_INTEGER_OVERFLOW")
                return value
            shift += 7
            if shift >= 64 + 7:
                raise ScbError("SCB_INTEGER_OVERFLOW")

    def sized(self, maximum: int) -> bytes:
        length = self.uvar(64)
        if length > maximum:
            raise ScbError("SCB_RESOURCE_LIMIT")
        return self.take(length)

    def record_fields(self) -> list[tuple[int, bytes]]:
        count = self.uvar(64)
        if count > MAX_RECORD_FIELDS:
            raise ScbError("SCB_RESOURCE_LIMIT")
        fields: list[tuple[int, bytes]] = []
        prior: int | None = None
        for _ in range(count):
            tag = self.uvar(32)
            if prior is not None:
                if tag == prior:
                    raise ScbError("SCB_FIELD_DUPLICATE")
                if tag < prior:
                    raise ScbError("SCB_FIELD_ORDER")
            prior = tag
            fields.append((tag, self.sized(MAX_STANDALONE_BYTES)))
        return fields

    def union(self) -> tuple[int, bytes]:
        tag = self.uvar(32)
        return tag, self.sized(MAX_STANDALONE_BYTES)

    def finish(self) -> None:
        if self.remaining:
            raise ScbError("SCB_TRAILING_BYTES")


def parse_record(data: bytes) -> list[tuple[int, bytes]]:
    reader = Reader(data)
    fields = reader.record_fields()
    reader.finish()
    return fields


def encode_fields(fields: list[tuple[int, bytes]]) -> bytes:
    return encode_record(fields)


def single_field(fields: list[tuple[int, bytes]], tag: int) -> bytes:
    found = [payload for field_tag, payload in fields if field_tag == tag]
    if len(found) != 1:
        raise ScbError("SCB_FIELD_MISSING" if not found else "SCB_FIELD_DUPLICATE")
    return found[0]


def exact_fields(fields: list[tuple[int, bytes]], tags: list[int]) -> None:
    if sorted(tag for tag, _ in fields) != sorted(tags):
        have = sorted(tag for tag, _ in fields)
        if len(have) < len(tags):
            raise ScbError("SCB_FIELD_MISSING")
        raise ScbError("SCB_FIELD_UNKNOWN")


def decode_uvar_exact(payload: bytes, width: int) -> int:
    reader = Reader(payload)
    value = reader.uvar(width)
    reader.finish()
    return value


def decode_fixed32(payload: bytes) -> bytes:
    reader = Reader(payload)
    raw = reader.take(32)
    reader.finish()
    return raw


def encode_label(label: str) -> bytes:
    if unicodedata2.normalize("NFC", label) != label:
        raise ValueError("label is not Unicode 16.0.0 NFC")
    raw = label.encode("utf-8")
    if len(raw) > MAX_BYTE_PAYLOAD:
        raise ScbError("SCB_RESOURCE_LIMIT")
    return encode_sized(raw)


def checked_add(left: int, right: int) -> int:
    result = left + right
    if result > U64_MAX:
        raise Overflow(f"checked add overflow: {left} + {right}")
    return result


def checked_mul(left: int, right: int) -> int:
    result = left * right
    if result > U64_MAX:
        raise Overflow(f"checked mul overflow: {left} * {right}")
    return result


def compute_work(count_k: int, lookup_l: int, stored_b: int, ceiling_m: int) -> int:
    """ENTITY_READ section 5 work bound with checked-u64 arithmetic."""
    step = checked_add(1, checked_mul(count_k, lookup_l))
    step = checked_add(step, checked_mul(2, stored_b))
    return checked_add(step, ceiling_m)


def derived_lookup_l(inputs: Mapping[str, Any]) -> int:
    """Derive L from declared synthetic N; redundant metadata must agree."""
    context = inputs["context"]
    raw_n = context.get("root_bindings")
    if isinstance(raw_n, bool) or not isinstance(raw_n, int):
        raise CheckFailed("work", "root_bindings must be an unsigned integer")
    if raw_n < 0 or raw_n > U64_MAX:
        raise CheckFailed("work", "root_bindings outside unsigned domain")
    derived = int(raw_n).bit_length() + 1
    if "lookup_l" in context:
        supplied = context["lookup_l"]
        if isinstance(supplied, bool) or not isinstance(supplied, int):
            raise CheckFailed("work", "redundant lookup_l must be an unsigned integer")
        if int(supplied) != derived:
            raise CheckFailed("work", f"redundant lookup_l {supplied} != derived {derived}")
    bindings = inputs.get("bindings")
    if isinstance(bindings, Mapping) and "count" in bindings:
        supplied_count = bindings["count"]
        if isinstance(supplied_count, bool) or not isinstance(supplied_count, int):
            raise CheckFailed("count", "redundant bindings.count must be an unsigned integer")
        if int(supplied_count) != int(raw_n):
            raise CheckFailed("count", f"redundant bindings.count {supplied_count} != declared {raw_n}")
    return derived



# Protocol schema-epoch derivation (SCHEMA_EPOCH_V1.md, frozen descriptor)



def encode_u32_set(values: list[int]) -> bytes:
    if any(b <= a for a, b in zip(values, values[1:])):
        raise ValueError("u32 set must be strictly increasing")
    return encode_uvar(len(values)) + b"".join(encode_sized(encode_uvar(v)) for v in values)


def encode_descriptor(descriptor: Mapping[str, Any]) -> bytes:
    return encode_record(
        [
            (1, encode_uvar(descriptor["contract_tag"])),
            (2, encode_uvar(descriptor["digest_domain_tag"])),
            (3, encode_uvar(descriptor["kind_tag"])),
            (4, bytes.fromhex(descriptor["field_schema_hash"])),
            (5, encode_u32_set(descriptor["required_fields"])),
            (6, encode_u32_set(descriptor["optional_fields"])),
            (7, encode_u32_set(descriptor["variant_tags"])),
            (8, bytes.fromhex(descriptor["decoder_limits_hash"])),
        ]
    )


def protocol_descriptor() -> dict[str, Any]:
    return {
        "contract_tag": FRAME_CONTRACT_TAG,
        "digest_domain_tag": PROTOCOL_DIGEST_DOMAIN_TAG,
        "kind_tag": FRAME_CONTRACT_TAG,
        "field_schema_hash": PROTOCOL_FIELD_SCHEMA_HASH,
        "required_fields": [1, 2, 3, 4, 5, 6, 7, 8],
        "optional_fields": [],
        "variant_tags": [],
        "decoder_limits_hash": PROTOCOL_DECODER_LIMITS_HASH,
    }


def protocol_epoch_record() -> bytes:
    unicode_version = encode_record(
        [
            (1, encode_uvar(EPOCH_UNICODE[0])),
            (2, encode_uvar(EPOCH_UNICODE[1])),
            (3, encode_uvar(EPOCH_UNICODE[2])),
        ]
    )
    limits = encode_record([(tag, encode_uvar(value)) for tag, value in zip((1, 2, 3, 4, 5, 6, 7), EPOCH_LIMITS)])
    contracts = encode_uvar(1) + encode_sized(encode_descriptor(protocol_descriptor()))
    empty_set = encode_uvar(0)
    predecessor_none = encode_uvar(0) + encode_sized(b"")
    return encode_record(
        [
            (1, encode_uvar(1)),
            (2, encode_uvar(1)),
            (3, encode_uvar(1)),
            (4, unicode_version),
            (5, limits),
            (6, contracts),
            (7, empty_set),
            (8, predecessor_none),
            (9, empty_set),
        ]
    )


def protocol_epoch_id() -> bytes:
    """Reconstruct the protocol epoch identity from frozen descriptor inputs."""
    record = protocol_epoch_record()
    preimage = BOOTSTRAP_MAGIC + encode_uvar(BOOTSTRAP_VERSION) + encode_sized(record)
    return blake3.blake3(EPOCH_DOMAIN + preimage).digest()



# Entity object construction (SSMC1 object envelope, design step 3)



def build_object(entity: Mapping[str, Any], content_epoch: bytes) -> dict[str, str]:
    kind = int(entity["kind"])
    if KIND_NAMES[kind] != entity["kind_name"]:
        raise ValueError(f"kind tag/name mismatch for {entity['id']}")
    body_type = BODY_TYPES[kind]
    if body_type != entity["body_type"]:
        raise ValueError(f"body type mismatch for {entity['id']}")
    body = encode_mutation_value(body_type, entity["body"])
    wrapped = encode_uvar(kind) + encode_sized(body)
    fields: list[tuple[int, bytes]] = [
        (1, bytes.fromhex(entity["id"])),
        (2, wrapped),
    ]
    if len(bytes.fromhex(entity["id"])) != 32:
        raise ValueError(f"entity id must be 32 bytes: {entity['id']}")
    label = entity.get("label")
    if label is not None:
        fields.append((3, encode_label(label)))
    fingerprint = entity.get("fingerprint")
    if fingerprint is not None:
        raw = bytes.fromhex(fingerprint)
        if len(raw) != 32 or raw == bytes(32):
            raise ValueError("fingerprint must be nonzero 32 bytes")
        fields.append((4, raw))
    object_record = encode_record(fields)
    preimage = MAGIC + encode_uvar(FORMAT_VERSION) + encode_uvar(OBJECT_CONTRACT_TAG) + content_epoch + encode_sized(object_record)
    object_id = blake3.blake3(OBJECT_DOMAIN + preimage).digest()
    stored = preimage + object_id
    if len(stored) > MAX_STANDALONE_BYTES:
        raise ScbError("SCB_RESOURCE_LIMIT")
    return {
        "record_hex": object_record.hex(),
        "preimage_hex": preimage.hex(),
        "stored_hex": stored.hex(),
        "object_id": object_id.hex(),
    }


def _decode_outer_metadata(fields: list[tuple[int, bytes]]) -> tuple[str | None, bytes | None]:
    """Exact outer-object label/fingerprint validation shared by bound/unbound paths."""
    label: str | None = None
    fingerprint: bytes | None = None
    for tag, payload in fields:
        if tag == 3:
            reader = Reader(payload)
            raw = reader.sized(MAX_BYTE_PAYLOAD)
            reader.finish()
            try:
                label = raw.decode("utf-8")
            except UnicodeDecodeError as error:
                raise ScbError("SCB_UTF8_INVALID") from error
            if unicodedata2.normalize("NFC", label) != label:
                raise ScbError("SCB_LABEL_NOT_NFC")
        elif tag == 4:
            fingerprint = decode_fixed32(payload)
    return label, fingerprint


def decode_object_record(
    object_record: bytes, expected_kind: int, expected_id: bytes
) -> dict[str, Any]:
    """Bounded structured decode of an EntityObject record (body opaque)."""
    fields = parse_record(object_record)
    if len(fields) < 2 or len(fields) > 4:
        raise ScbError("SCB_FIELD_MISSING" if len(fields) < 2 else "SCB_FIELD_UNKNOWN")
    tags = [tag for tag, _ in fields]
    if any(tag not in (1, 2, 3, 4) for tag in tags):
        raise ScbError("SCB_FIELD_UNKNOWN")
    if 1 not in tags or 2 not in tags:
        raise ScbError("SCB_FIELD_MISSING")
    entity_id = decode_fixed32(single_field(fields, 1))
    if entity_id != expected_id:
        raise CheckFailed("object_record", "entity id mismatch")
    union_payload = single_field(fields, 2)
    reader = Reader(union_payload)
    kind_tag, body = reader.union()
    reader.finish()
    if kind_tag != expected_kind:
        raise CheckFailed("object_record", f"kind tag {kind_tag} != {expected_kind}")
    decode_declared_mutation_value(BODY_TYPES[expected_kind], body)
    label, fingerprint = _decode_outer_metadata(fields)
    return {"entity_id": entity_id, "kind": kind_tag, "body": body, "label": label, "fingerprint": fingerprint}


def check_stored_object(stored: bytes, expected_kind: int, expected_id: bytes, content_epoch: bytes) -> dict[str, Any]:
    if len(stored) < 32:
        raise ScbError("SCB_LENGTH_OVERFLOW")
    preimage, trailer = stored[:-32], stored[-32:]
    reader = Reader(preimage)
    if reader.take(8) != MAGIC:
        raise ScbError("SCB_MAGIC_INVALID")
    if reader.uvar(64) != FORMAT_VERSION:
        raise ScbError("SCB_VERSION_UNSUPPORTED")
    if reader.uvar(32) != OBJECT_CONTRACT_TAG:
        raise ScbError("SCB_CONTRACT_UNKNOWN")
    epoch = reader.take(32)
    if epoch != content_epoch:
        raise ScbError("SCB_EPOCH_MISMATCH")
    object_record = reader.sized(MAX_STANDALONE_BYTES)
    reader.finish()
    if blake3.blake3(OBJECT_DOMAIN + preimage).digest() != trailer:
        raise ScbError("SCB_DIGEST_MISMATCH")
    decoded = decode_object_record(object_record, expected_kind, expected_id)
    decoded["object_id"] = trailer
    return decoded



# Request / response / bounds / frame / failure construction



def build_request_body(root: bytes, entity: bytes, max_objects: int, ceiling_m: int, max_work: int) -> bytes:
    for name, value in (("max_objects", max_objects), ("max_response_bytes", ceiling_m), ("max_work", max_work)):
        if value <= 0:
            raise CheckFailed("request_range", f"{name} must be positive")
    return encode_record(
        [
            (1, root),
            (2, entity),
            (3, encode_uvar(max_objects)),
            (4, encode_uvar(ceiling_m)),
            (5, encode_uvar(max_work)),
        ]
    )


def decode_request_body(body: bytes, selected: Mapping[str, Any]) -> dict[str, Any]:
    fields = parse_record(body)
    exact_fields(fields, [1, 2, 3, 4, 5])
    root = decode_fixed32(single_field(fields, 1))
    entity = decode_fixed32(single_field(fields, 2))
    max_objects = decode_uvar_exact(single_field(fields, 3), 64)
    ceiling_m = decode_uvar_exact(single_field(fields, 4), 64)
    max_work = decode_uvar_exact(single_field(fields, 5), 64)
    limits = selected["limits"]
    if max_objects <= 0 or ceiling_m <= 0 or max_work <= 0:
        raise CheckFailed("request_range", "limits must be positive")
    if max_objects > limits["max_entities"] or ceiling_m > limits["max_response_bytes"] or max_work > limits["max_work"]:
        raise CheckFailed("request_range", "limit exceeds selected ceiling")
    return {"root": root, "entity": entity, "max_objects": max_objects, "ceiling_m": ceiling_m, "max_work": max_work}


def build_entry(entity: bytes, kind: int, object_id: bytes, stored: bytes) -> bytes:
    return encode_record(
        [
            (1, entity),
            (2, encode_uvar(kind)),
            (3, object_id),
            (4, encode_sized(stored)),
        ]
    )


def build_response_body(
    workspace: bytes,
    root: bytes,
    epoch: bytes,
    session: bytes,
    requested: bytes,
    entries: list[bytes],
    work: int,
) -> bytes:
    items = encode_uvar(len(entries)) + b"".join(encode_sized(entry) for entry in entries)
    return encode_record(
        [
            (1, encode_uvar(RESPONSE_VERSION)),
            (2, workspace),
            (3, root),
            (4, epoch),
            (5, session),
            (6, requested),
            (7, items),
            (8, encode_uvar(work)),
        ]
    )


def decode_response_body(body: bytes) -> dict[str, Any]:
    fields = parse_record(body)
    exact_fields(fields, [1, 2, 3, 4, 5, 6, 7, 8])
    version = decode_uvar_exact(single_field(fields, 1), 32)
    if version != RESPONSE_VERSION:
        raise CheckFailed("response_record", f"version {version} != 2")
    entries_raw = single_field(fields, 7)
    reader = Reader(entries_raw)
    count = reader.uvar(64)
    if count > MAX_COLLECTION_ELEMENTS:
        raise ScbError("SCB_RESOURCE_LIMIT")
    entries = [reader.sized(MAX_STANDALONE_BYTES) for _ in range(count)]
    reader.finish()
    work = decode_uvar_exact(single_field(fields, 8), 64)
    return {
        "workspace": decode_fixed32(single_field(fields, 2)),
        "root": decode_fixed32(single_field(fields, 3)),
        "epoch": decode_fixed32(single_field(fields, 4)),
        "session": decode_fixed32(single_field(fields, 5)),
        "requested": decode_fixed32(single_field(fields, 6)),
        "entries": entries,
        "work": work,
    }


def decode_response_entry(entry: bytes) -> dict[str, Any]:
    fields = parse_record(entry)
    exact_fields(fields, [1, 2, 3, 4])
    reader = Reader(single_field(fields, 4))
    stored = reader.sized(MAX_BYTE_PAYLOAD)
    reader.finish()
    return {
        "entity": decode_fixed32(single_field(fields, 1)),
        "kind": decode_uvar_exact(single_field(fields, 2), 32),
        "object_id": decode_fixed32(single_field(fields, 3)),
        "stored": stored,
    }


def build_limits(limits: Mapping[str, Any]) -> bytes:
    return encode_record(
        [
            (1, encode_uvar(limits["max_frame_bytes"])),
            (2, encode_uvar(limits["max_entities"])),
            (3, encode_uvar(limits["max_edges"])),
            (4, encode_uvar(limits["max_depth"])),
            (5, encode_uvar(limits["max_response_bytes"])),
            (6, encode_uvar(limits["max_work"])),
            (7, encode_uvar(limits["max_inflight"])),
            (8, encode_uvar(limits["max_sessions"])),
        ]
    )


def validate_selected_limits(limits: Mapping[str, Any]) -> None:
    keys = ("max_frame_bytes", "max_entities", "max_edges", "max_depth", "max_response_bytes", "max_work", "max_inflight", "max_sessions")
    values = [limits[key] for key in keys]
    for key, value, ceiling in zip(keys, values, LIMIT_CEILINGS):
        if isinstance(value, bool) or not isinstance(value, int):
            raise CheckFailed("selected_limits", f"limit {key} must be an unsigned integer")
        if key == "max_depth":
            if value < 0 or value > ceiling:
                raise CheckFailed("selected_limits", f"limit {value} outside [0, {ceiling}]")
        elif value <= 0 or value > ceiling:
            raise CheckFailed("selected_limits", f"limit {value} outside (0, {ceiling}]")


def build_bounds(limits: Mapping[str, Any], returned_bytes: int, returned_entities: int) -> bytes:
    return encode_record(
        [
            (1, build_limits(limits)),
            (2, encode_uvar(returned_bytes)),
            (3, encode_uvar(returned_entities)),
            (4, encode_uvar(0)),
            (5, encode_uvar(0)),
            (6, encode_uvar(0)),
            (7, encode_uvar(1)),
            (8, encode_uvar(1)),
        ]
    )


def zero_bounds() -> bytes:
    return build_bounds({key: 0 for key in ("max_frame_bytes", "max_entities", "max_edges", "max_depth", "max_response_bytes", "max_work", "max_inflight", "max_sessions")}, 0, 0)


LIMIT_KEYS = (
    "max_frame_bytes",
    "max_entities",
    "max_edges",
    "max_depth",
    "max_response_bytes",
    "max_work",
    "max_inflight",
    "max_sessions",
)
U32_LIMIT_TAGS = frozenset({4, 7, 8})


def decode_limit_profile(raw: bytes) -> dict[str, int]:
    fields = parse_record(raw)
    exact_fields(fields, [1, 2, 3, 4, 5, 6, 7, 8])
    profile: dict[str, int] = {}
    for tag, payload in fields:
        profile[LIMIT_KEYS[tag - 1]] = decode_uvar_exact(payload, 32 if tag in U32_LIMIT_TAGS else 64)
    return profile


def decode_bounds(bounds: bytes) -> dict[str, Any]:
    fields = parse_record(bounds)
    exact_fields(fields, [1, 2, 3, 4, 5, 6, 7, 8])
    truncated = decode_uvar_exact(single_field(fields, 7), 32)
    continuation = decode_uvar_exact(single_field(fields, 8), 32)
    if truncated not in (1, 2) or continuation not in (1, 2):
        raise ScbError("SCB_UNION_INVALID")
    return {
        "applied_limits": decode_limit_profile(single_field(fields, 1)),
        "returned_bytes": decode_uvar_exact(single_field(fields, 2), 64),
        "returned_entities": decode_uvar_exact(single_field(fields, 3), 64),
        "returned_edges": decode_uvar_exact(single_field(fields, 4), 64),
        "reached_depth": decode_uvar_exact(single_field(fields, 5), 32),
        "omitted": decode_uvar_exact(single_field(fields, 6), 64),
        "truncated": truncated == 2,
        "continuation": continuation == 2,
    }


def build_frame_payload(
    version: int,
    session: bytes | None,
    request_id: int,
    kind: int,
    method: int,
    flags: int,
    bounds: bytes,
    body: bytes,
) -> bytes:
    session_value = encode_uvar(0) + encode_sized(b"") if session is None else encode_uvar(1) + encode_sized(session)
    return encode_record(
        [
            (1, encode_uvar(version)),
            (2, session_value),
            (3, encode_uvar(request_id)),
            (4, encode_uvar(kind)),
            (5, encode_uvar(method)),
            (6, encode_uvar(flags)),
            (7, bounds),
            (8, encode_sized(body)),
        ]
    )


def decode_frame_payload(payload: bytes) -> dict[str, Any]:
    fields = parse_record(payload)
    exact_fields(fields, [1, 2, 3, 4, 5, 6, 7, 8])
    reader = Reader(single_field(fields, 2))
    session_tag, session_payload = reader.union()
    reader.finish()
    if session_tag == 0:
        if session_payload != b"":
            raise ScbError("SCB_UNION_INVALID")
        session = None
    elif session_tag == 1:
        if len(session_payload) != 32:
            raise ScbError("SCB_UNION_INVALID")
        session = session_payload
    else:
        raise ScbError("SCB_UNION_INVALID")
    body_reader = Reader(single_field(fields, 8))
    body = body_reader.sized(MAX_BYTE_PAYLOAD)
    body_reader.finish()
    return {
        "version": decode_uvar_exact(single_field(fields, 1), 32),
        "session": session,
        "request_id": decode_uvar_exact(single_field(fields, 3), 64),
        "kind": decode_uvar_exact(single_field(fields, 4), 32),
        "method": decode_uvar_exact(single_field(fields, 5), 32),
        "flags": decode_uvar_exact(single_field(fields, 6), 32),
        "bounds": single_field(fields, 7),
        "body": body,
    }


def build_envelope(protocol_epoch: bytes, payload: bytes) -> tuple[bytes, bytes, bytes]:
    preimage = MAGIC + encode_uvar(FORMAT_VERSION) + encode_uvar(FRAME_CONTRACT_TAG) + protocol_epoch + encode_sized(payload)
    frame_id = blake3.blake3(FRAME_DOMAIN + preimage).digest()
    stored = preimage + frame_id
    if len(stored) > MAX_STANDALONE_BYTES:
        raise ScbError("SCB_RESOURCE_LIMIT")
    wire = struct.pack(">Q", len(stored)) + stored
    return wire, preimage, frame_id


def split_wire(wire: bytes, max_frame_bytes: int) -> tuple[bytes, bytes]:
    if len(wire) < 8:
        raise CheckFailed("wire_prefix", "wire shorter than prefix")
    (length,) = struct.unpack(">Q", wire[:8])
    if length > max_frame_bytes:
        raise CheckFailed("wire_ceiling", "prefix exceeds negotiated ceiling")
    stored = wire[8:]
    if len(stored) != length:
        raise CheckFailed("wire_prefix", "prefix/length mismatch")
    return stored, wire[:8]


def check_envelope(stored: bytes, protocol_epoch: bytes) -> tuple[bytes, bytes]:
    if len(stored) < 32:
        raise ScbError("SCB_LENGTH_OVERFLOW")
    preimage, trailer = stored[:-32], stored[-32:]
    if blake3.blake3(FRAME_DOMAIN + preimage).digest() != trailer:
        raise ScbError("SCB_DIGEST_MISMATCH")
    reader = Reader(preimage)
    if reader.take(8) != MAGIC:
        raise ScbError("SCB_MAGIC_INVALID")
    if reader.uvar(64) != FORMAT_VERSION:
        raise ScbError("SCB_VERSION_UNSUPPORTED")
    if reader.uvar(32) != FRAME_CONTRACT_TAG:
        raise ScbError("SCB_CONTRACT_UNKNOWN")
    if reader.take(32) != protocol_epoch:
        raise ScbError("SCB_EPOCH_MISMATCH")
    payload = reader.sized(MAX_STANDALONE_BYTES)
    reader.finish()
    return payload, trailer


def retryability(symbol: str) -> int:
    if symbol in REQUERY_SYMBOLS:
        return 2
    if symbol in LIMIT_CHANGE_SYMBOLS:
        return 4
    return 1


def build_failure(code: int, symbol: str, details: bytes = b"") -> bytes:
    if not symbol or any(not (char.isupper() or char == "_" or char.isdigit()) for char in symbol):
        raise ValueError("failure symbol must be uppercase/underscore/digits")
    incident = encode_uvar(0) + encode_sized(b"")
    return encode_record(
        [
            (1, encode_uvar(code)),
            (2, encode_sized(symbol.encode("ascii"))),
            (3, encode_uvar(0)),
            (4, encode_uvar(retryability(symbol))),
            (5, incident),
            (6, encode_sized(details)),
        ]
    )


def decode_failure(body: bytes) -> dict[str, Any]:
    fields = parse_record(body)
    exact_fields(fields, [1, 2, 3, 4, 5, 6])
    code = decode_uvar_exact(single_field(fields, 1), 32)
    symbol_reader = Reader(single_field(fields, 2))
    try:
        symbol = symbol_reader.sized(MAX_BYTE_PAYLOAD).decode("ascii")
    except UnicodeDecodeError as error:
        raise ScbError("SCB_UTF8_INVALID") from error
    symbol_reader.finish()
    phase = decode_uvar_exact(single_field(fields, 3), 32)
    retry = decode_uvar_exact(single_field(fields, 4), 32)
    reader = Reader(single_field(fields, 5))
    incident_tag, incident_payload = reader.union()
    reader.finish()
    if (incident_tag, len(incident_payload)) != (0, 0):
        raise CheckFailed("failure_record", "incident must be None at this revision")
    details_reader = Reader(single_field(fields, 6))
    details = details_reader.sized(MAX_BYTE_PAYLOAD)
    details_reader.finish()
    if retry != retryability(symbol):
        raise CheckFailed("failure_record", "retryability mapping mismatch")
    return {"code": code, "symbol": symbol, "phase": phase, "retryability": retry, "details": details}



# Hello / versioned selection / handshake (SMP1 section 2, profile section 2)



def _require_u32_list(values: Any, name: str) -> list[int]:
    if not isinstance(values, list):
        raise CheckFailed("hello", f"{name} must be a list")
    if not values or len(values) > 4096:
        raise CheckFailed("hello", f"{name} must be nonempty with at most 4096 entries")
    for value in values:
        if isinstance(value, bool) or not isinstance(value, int):
            raise CheckFailed("hello", f"{name} elements must be u32")
        if value < 0 or value > 0xFFFFFFFF:
            raise CheckFailed("hello", f"{name} elements must be u32")
    if any(b <= a for a, b in zip(values, values[1:])):
        raise CheckFailed("hello", f"{name} must be strictly increasing")
    return values


def _require_identity_list(values: Any, name: str, *, allow_empty: bool, ordered: bool) -> list[bytes]:
    if not isinstance(values, list):
        raise CheckFailed("hello", f"{name} must be a list")
    if len(values) > 4096:
        raise CheckFailed("hello", f"{name} must hold at most 4096 entries")
    if not allow_empty and not values:
        raise CheckFailed("hello", f"{name} must be nonempty")
    items: list[bytes] = []
    for value in values:
        if not isinstance(value, str):
            raise CheckFailed("hello", f"{name} elements must be 32-byte identities")
        try:
            raw = bytes.fromhex(value)
        except ValueError as error:
            raise CheckFailed("hello", f"{name} elements must be 32-byte identities") from error
        if len(raw) != 32:
            raise CheckFailed("hello", f"{name} elements must be 32-byte identities")
        items.append(raw)
    if ordered and any(b <= a for a, b in zip(items, items[1:])):
        raise CheckFailed("hello", f"{name} must be strictly increasing")
    return items


def _validate_hello_offer(hello: Mapping[str, Any]) -> dict[str, list[bytes]]:
    # Raw schema-epoch order/multiplicity preserved: unordered with duplicates allowed.
    _require_u32_list(hello["protocol_versions"], "versions")
    _require_u32_list(hello["methods"], "methods")
    if any(tag in RESERVED_TAGS for tag in hello["methods"]):
        raise CheckFailed("hello", "reserved tag in offer")
    features = hello["features"]
    if isinstance(features, bool) or not isinstance(features, int):
        raise CheckFailed("hello", "features must be u32")
    if features < 0 or features > 0xFFFFFFFF:
        raise CheckFailed("hello", "features must be u32")
    if features & ~FEATURE_MASK:
        raise CheckFailed("hello", "unknown feature bit")
    epochs = _require_identity_list(hello["schema_epochs"], "epochs", allow_empty=False, ordered=False)
    adapters = _require_identity_list(hello["adapters"], "adapters", allow_empty=True, ordered=True)
    effects = _require_identity_list(hello["effects"], "effects", allow_empty=True, ordered=True)
    validate_selected_limits(hello["limits"])
    return {"schema_epochs": epochs, "adapters": adapters, "effects": effects}


def build_hello(hello: Mapping[str, Any]) -> bytes:
    _validate_hello_offer(hello)
    versions = hello["protocol_versions"]
    methods = hello["methods"]
    return encode_record(
        [
            (1, encode_uvar(len(versions)) + b"".join(encode_sized(encode_uvar(v)) for v in versions)),
            (2, encode_uvar(len(hello["schema_epochs"])) + b"".join(encode_sized(bytes.fromhex(e)) for e in hello["schema_epochs"])),
            (3, build_limits(hello["limits"])),
            (4, encode_uvar(len(methods)) + b"".join(encode_sized(encode_uvar(m)) for m in methods)),
            (5, encode_uvar(hello["features"])),
            (6, encode_uvar(len(hello["adapters"])) + b"".join(encode_sized(bytes.fromhex(a)) for a in hello["adapters"])),
            (7, encode_uvar(len(hello["effects"])) + b"".join(encode_sized(bytes.fromhex(e)) for e in hello["effects"])),
        ]
    )


def _decode_hello_u32_list(payload: bytes) -> list[int]:
    reader = Reader(payload)
    count = reader.uvar(64)
    if count > 4096:
        raise ScbError("SCB_RESOURCE_LIMIT")
    values: list[int] = []
    for _ in range(count):
        element = reader.sized(MAX_STANDALONE_BYTES)
        values.append(decode_uvar_exact(element, 32))
    reader.finish()
    return values


def _decode_hello_identity_list(payload: bytes) -> list[bytes]:
    reader = Reader(payload)
    count = reader.uvar(64)
    if count > 4096:
        raise ScbError("SCB_RESOURCE_LIMIT")
    items: list[bytes] = []
    for _ in range(count):
        element = reader.sized(MAX_STANDALONE_BYTES)
        items.append(decode_fixed32(element))
    reader.finish()
    return items


def _decode_supplied_hello(body: bytes) -> dict[str, Any]:
    fields = parse_record(body)
    exact_fields(fields, [1, 2, 3, 4, 5, 6, 7])
    versions = _decode_hello_u32_list(single_field(fields, 1))
    epoch_raws = _decode_hello_identity_list(single_field(fields, 2))
    limits = decode_limit_profile(single_field(fields, 3))
    methods = _decode_hello_u32_list(single_field(fields, 4))
    features = decode_uvar_exact(single_field(fields, 5), 32)
    adapter_raws = _decode_hello_identity_list(single_field(fields, 6))
    effect_raws = _decode_hello_identity_list(single_field(fields, 7))
    hello = {
        "protocol_versions": versions,
        "schema_epochs": [b.hex() for b in epoch_raws],
        "limits": limits,
        "methods": methods,
        "features": features,
        "adapters": [b.hex() for b in adapter_raws],
        "effects": [b.hex() for b in effect_raws],
    }
    try:
        decoded_ids = _validate_hello_offer(hello)
    except CheckFailed as error:
        raise CheckFailed("hello_body", error.detail) from error
    return {"hello": hello, "epochs": decoded_ids["schema_epochs"]}


def _admit_hello_frame(frame: Mapping[str, Any], expected_version: Any) -> None:
    if isinstance(expected_version, bool) or not isinstance(expected_version, int):
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")
    version = frame["version"]
    if version < expected_version:
        raise CheckFailed("frame_header", "PROTOCOL_DOWNGRADE")
    if version > expected_version:
        raise CheckFailed("frame_header", "PROTOCOL_VERSION_UNSUPPORTED")
    if version != 1:
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")
    if frame["kind"] != 4:
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")
    if frame["session"] is not None:
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")
    if frame["request_id"] != 0:
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")
    if frame["method"] != 0:
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")
    if frame["flags"] != 0:
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")


def _reconstruct_hello_row(inputs: Mapping[str, Any], authored_row: Mapping[str, Any]) -> tuple[bytes, bytes, bytes, bytes] | None:
    try:
        hellos = inputs.get("hellos")
        if not isinstance(hellos, Mapping):
            return None
        offer = hellos.get(authored_row.get("hello"))
        if not isinstance(offer, Mapping):
            return None
        body = build_hello(offer)
        wire_version = authored_row.get("wire_version")
        if isinstance(wire_version, bool) or not isinstance(wire_version, int):
            return None
        bounds = zero_bounds()
        payload = build_frame_payload(wire_version, None, 0, 4, 0, 0, bounds, body)
        wire, preimage, frame_id = build_envelope(protocol_epoch_id(), payload)
        return body, wire, preimage, frame_id
    except (ScbError, CheckFailed, ValueError, KeyError, TypeError, AttributeError):
        return None


def build_selected(selection: Mapping[str, Any]) -> bytes:
    return encode_record(
        [
            (1, encode_uvar(selection["protocol_version"])),
            (2, bytes.fromhex(selection["schema_epoch"])),
            (3, build_limits(selection["limits"])),
            (4, encode_uvar(len(selection["methods"])) + b"".join(encode_sized(encode_uvar(m)) for m in selection["methods"])),
            (5, encode_uvar(selection["features"])),
            (6, encode_uvar(len(selection["adapters"])) + b"".join(encode_sized(bytes.fromhex(a)) for a in selection["adapters"])),
            (7, encode_uvar(len(selection["effects"])) + b"".join(encode_sized(bytes.fromhex(e)) for e in selection["effects"])),
        ]
    )


def negotiate_versioned(client: Mapping[str, Any], server: Mapping[str, Any]) -> dict[str, Any]:
    client_ids = _validate_hello_offer(client)
    server_ids = _validate_hello_offer(server)
    common_versions = [v for v in client["protocol_versions"] if v in server["protocol_versions"]]
    if not common_versions:
        raise CheckFailed("selection", "no common version")
    version = max(common_versions)
    if version not in (1, PROTOCOL_VERSION_2):
        raise CheckFailed("selection", f"unsupported greatest-common version {version}")
    client_epoch_set = set(client_ids["schema_epochs"])
    epoch_bytes = next((b for b in server_ids["schema_epochs"] if b in client_epoch_set), None)
    if epoch_bytes is None:
        raise CheckFailed("selection", "no common epoch")
    epoch = epoch_bytes.hex()
    methods = [m for m in client["methods"] if m in server["methods"]]
    if 100 not in methods:
        raise CheckFailed("selection", "session.open floor missing")
    if version == 1:
        methods = [m for m in methods if m not in (METHOD_ENTITY_VERSION, METHOD_ENTITY_SIGNATURE)]
    limit_keys = ("max_frame_bytes", "max_entities", "max_edges", "max_depth", "max_response_bytes", "max_work", "max_inflight", "max_sessions")
    limits = {key: min(client["limits"][key], server["limits"][key]) for key in limit_keys}
    server_adapter_set = set(server_ids["adapters"])
    server_effect_set = set(server_ids["effects"])
    return {
        "protocol_version": version,
        "schema_epoch": epoch,
        "limits": limits,
        "methods": methods,
        "features": client["features"] & server["features"],
        "adapters": [b.hex() for b in client_ids["adapters"] if b in server_adapter_set],
        "effects": [b.hex() for b in client_ids["effects"] if b in server_effect_set],
    }


def handshake_id(client_body: bytes, server_body: bytes, selection_preimage: bytes) -> bytes:
    return blake3.blake3(HANDSHAKE_DOMAIN + client_body + server_body + selection_preimage).digest()



# Context / signature / count / work checks (oracle layer, no new codes)



def check_context(
    response: Mapping[str, Any],
    context: Mapping[str, Any],
    requested: bytes,
) -> None:
    for field, key in (("workspace", "workspace"), ("root", "root"), ("epoch", "content_epoch"), ("session", "session")):
        if response[field] != bytes.fromhex(context[key]):
            raise CheckFailed("context", f"{field} mismatch")
    if response["requested"] != requested:
        raise CheckFailed("context", "requested entity mismatch")


def check_counts(response: Mapping[str, Any], expected_k: int, response_body_len: int, bounds: Mapping[str, Any]) -> None:
    if len(response["entries"]) != expected_k:
        raise CheckFailed("count", "entry count mismatch")
    if bounds["returned_entities"] != expected_k:
        raise CheckFailed("count", "bounds entity count mismatch")
    if bounds["returned_bytes"] != response_body_len:
        raise CheckFailed("count", "bounds byte count mismatch")
    if bounds["returned_edges"] != 0 or bounds["reached_depth"] != 0 or bounds["omitted"] != 0:
        raise CheckFailed("count", "nonzero edge/depth/omitted")
    if bounds["truncated"] or bounds["continuation"]:
        raise CheckFailed("count", "truncated/continuation must be false")


def check_work(response: Mapping[str, Any], count_k: int, lookup_l: int, stored_b: int, ceiling_m: int) -> None:
    try:
        expected = compute_work(count_k, lookup_l, stored_b, ceiling_m)
    except Overflow as error:
        raise CheckFailed("work", str(error)) from error
    if response["work"] != expected:
        raise CheckFailed("work", f"claimed {response['work']} != {expected}")


def decode_parameter_body(body: bytes) -> dict[str, Any]:
    fields = parse_record(body)
    exact_fields(fields, [1, 2, 3, 4])
    decode_declared_mutation_value("ParameterBody", body)
    owner = decode_fixed32(single_field(fields, 1))
    role = decode_uvar_exact(single_field(fields, 2), 32)
    ordinal = decode_uvar_exact(single_field(fields, 3), 32)
    value_type = single_field(fields, 4)
    return {"owner": owner, "role": role, "ordinal": ordinal, "value_type": value_type}


def decode_stored_envelope(stored: bytes, content_epoch: bytes) -> tuple[bytes, bytes]:
    """Verify object magic / version / tag / epoch / digest; return preimage and record."""
    if len(stored) < 32:
        raise ScbError("SCB_LENGTH_OVERFLOW")
    preimage, trailer = stored[:-32], stored[-32:]
    reader = Reader(preimage)
    if reader.take(8) != MAGIC:
        raise ScbError("SCB_MAGIC_INVALID")
    if reader.uvar(64) != FORMAT_VERSION:
        raise ScbError("SCB_VERSION_UNSUPPORTED")
    if reader.uvar(32) != OBJECT_CONTRACT_TAG:
        raise ScbError("SCB_CONTRACT_UNKNOWN")
    epoch = reader.take(32)
    if epoch != content_epoch:
        raise ScbError("SCB_EPOCH_MISMATCH")
    object_record = reader.sized(MAX_STANDALONE_BYTES)
    reader.finish()
    if blake3.blake3(OBJECT_DOMAIN + preimage).digest() != trailer:
        raise ScbError("SCB_DIGEST_MISMATCH")
    return object_record, trailer


def decode_stored_record(object_record: bytes) -> dict[str, Any]:
    """Decode an object record without entry-claim expectations."""
    fields = parse_record(object_record)
    if len(fields) < 2 or len(fields) > 4:
        raise ScbError("SCB_FIELD_MISSING" if len(fields) < 2 else "SCB_FIELD_UNKNOWN")
    tags = [tag for tag, _ in fields]
    if any(tag not in (1, 2, 3, 4) for tag in tags):
        raise ScbError("SCB_FIELD_UNKNOWN")
    if 1 not in tags or 2 not in tags:
        raise ScbError("SCB_FIELD_MISSING")
    entity_id = decode_fixed32(single_field(fields, 1))
    union_reader = Reader(single_field(fields, 2))
    kind_tag, body = union_reader.union()
    union_reader.finish()
    if kind_tag not in BODY_TYPES:
        raise ScbError("SCB_UNION_INVALID")
    decode_declared_mutation_value(BODY_TYPES[kind_tag], body)
    label, fingerprint = _decode_outer_metadata(fields)
    return {"entity_id": entity_id, "kind": kind_tag, "body": body, "label": label, "fingerprint": fingerprint}


def decode_stored_unbound(stored: bytes, content_epoch: bytes) -> dict[str, Any]:
    """Verify an object envelope and digest without entry-claim expectations."""
    object_record, trailer = decode_stored_envelope(stored, content_epoch)
    decoded = decode_stored_record(object_record)
    decoded["object_id"] = trailer
    return decoded


def check_signature(
    metas: list[dict[str, Any]],
    bodies: list[bytes],
    function_id: bytes,
    parameter_ids: list[bytes],
    expected_types: list[bytes],
) -> None:
    if [meta["entity"] for meta in metas] != [function_id, *parameter_ids]:
        raise CheckFailed("signature", "declaration order mismatch")
    if [meta["kind"] for meta in metas] != [5, *[6] * len(parameter_ids)]:
        raise CheckFailed("signature", "kind sequence mismatch")
    function_fields = parse_record(bodies[0])
    declared = single_field(function_fields, 2)
    reader = Reader(declared)
    count = reader.uvar(64)
    if count > MAX_COLLECTION_ELEMENTS:
        raise ScbError("SCB_RESOURCE_LIMIT")
    listed = [reader.sized(MAX_STANDALONE_BYTES) for _ in range(count)]
    reader.finish()
    if listed != parameter_ids:
        raise CheckFailed("signature", "function parameter list mismatch")
    for index, (meta, body, parameter_id, expected_type) in enumerate(zip(metas[1:], bodies[1:], parameter_ids, expected_types)):
        param = decode_parameter_body(body)
        if param["owner"] != function_id:
            raise CheckFailed("signature", f"parameter {index} owner mismatch")
        if param["role"] != 1:
            raise CheckFailed("signature", f"parameter {index} role mismatch")
        if param["ordinal"] != index:
            raise CheckFailed("signature", f"parameter {index} ordinal mismatch")
        if param["value_type"] != expected_type:
            raise CheckFailed("signature", f"parameter {index} exact type bytes mismatch")
        if meta["entity"] != parameter_id:
            raise CheckFailed("signature", f"parameter {index} identity mismatch")



# Case construction from semantic inputs



def build_success_case(inputs: Mapping[str, Any], case: Mapping[str, Any]) -> dict[str, Any]:
    context = inputs["context"]
    content_epoch = bytes.fromhex(context["content_epoch"])
    objects: dict[str, dict[str, str]] = {}
    for ref in case["objects"]:
        entity = inputs["entities"][ref]
        objects[ref] = build_object(entity, content_epoch)
    ordered = [objects[ref] for ref in case["objects"]]
    kinds = [int(inputs["entities"][ref]["kind"]) for ref in case["objects"]]
    stored_list = [bytes.fromhex(item["stored_hex"]) for item in ordered]
    count_k = len(ordered)
    stored_b = sum(len(item) for item in stored_list)
    ceiling_m = int(case["request"]["max_response_bytes"])
    lookup_l = derived_lookup_l(inputs)
    try:
        work = compute_work(count_k, lookup_l, stored_b, ceiling_m)
    except Overflow as error:
        raise CheckFailed("work", str(error)) from error
    request_body = build_request_body(
        bytes.fromhex(context["root"]),
        bytes.fromhex(inputs["entities"][case["entity"]]["id"]),
        int(case["request"]["max_objects"]),
        ceiling_m,
        int(case["request"]["max_work"]),
    )
    if len(request_body) > ceiling_m:
        raise CheckFailed("request_range", "request body exceeds its own ceiling")
    entries = [
        build_entry(
            bytes.fromhex(inputs["entities"][ref]["id"]),
            kind,
            bytes.fromhex(item["object_id"]),
            bytes.fromhex(item["stored_hex"]),
        )
        for ref, kind, item in zip(case["objects"], kinds, ordered)
    ]
    response_body = build_response_body(
        bytes.fromhex(context["workspace"]),
        bytes.fromhex(context["root"]),
        content_epoch,
        bytes.fromhex(context["session"]),
        bytes.fromhex(inputs["entities"][case["entity"]]["id"]),
        entries,
        work,
    )
    if len(response_body) > ceiling_m:
        raise CheckFailed("response_range", "response body exceeds request ceiling")
    bounds = build_bounds(inputs["selected_limits"], len(response_body), count_k)
    request_frame = build_frame_payload(
        PROTOCOL_VERSION_2,
        bytes.fromhex(context["session"]),
        int(case["request"]["request_id"]),
        1,
        int(case["method"]),
        0,
        zero_bounds(),
        request_body,
    )
    protocol_epoch = protocol_epoch_id()
    request_wire, request_preimage, request_frame_id = build_envelope(protocol_epoch, request_frame)
    response_frame = build_frame_payload(
        PROTOCOL_VERSION_2,
        bytes.fromhex(context["session"]),
        int(case["request"]["request_id"]),
        2,
        int(case["method"]),
        0,
        bounds,
        response_body,
    )
    response_wire, response_preimage, response_frame_id = build_envelope(protocol_epoch, response_frame)
    wire_len = len(response_wire)
    prefix_value = int.from_bytes(response_wire[:8], "big")
    if prefix_value != len(response_wire) - 8:
        raise CheckFailed("wire_prefix", "prefix accounting mismatch")
    return {
        "request_body_hex": request_body.hex(),
        "response_body_hex": response_body.hex(),
        "request_wire_hex": request_wire.hex(),
        "request_preimage_hex": request_preimage.hex(),
        "request_frame_id": request_frame_id.hex(),
        "response_wire_hex": response_wire.hex(),
        "response_preimage_hex": response_preimage.hex(),
        "response_frame_id": response_frame_id.hex(),
        "response_wire_len": wire_len,
        "work": work,
        "count_k": count_k,
        "stored_b": stored_b,
        "objects": {ref: objects[ref] for ref in case["objects"]},
    }



# Mutation recipes for rejected cases



def apply_record_surgery(body: bytes, recipe: Mapping[str, Any]) -> bytes:
    op = recipe["op"]
    if op == "raw_body":
        return bytes.fromhex(recipe["body_hex"])
    fields = parse_record(body)
    if op == "drop_field":
        fields = [(tag, payload) for tag, payload in fields if tag != int(recipe["tag"])]
    elif op == "add_field":
        fields = fields + [(int(recipe["tag"]), bytes.fromhex(recipe["payload_hex"]))]
    elif op == "dup_field":
        target_tag = int(recipe["tag"])
        matches = [(tag, payload) for tag, payload in fields if tag == target_tag]
        if not matches:
            raise ValueError("dup target missing")
        duplicated: list[tuple[int, bytes]] = []
        inserted = False
        for tag, payload in fields:
            duplicated.append((tag, payload))
            if tag == target_tag and not inserted:
                duplicated.append((tag, payload))
                inserted = True
        fields = duplicated
    elif op == "reorder_fields":
        order = [int(tag) for tag in recipe["order"]]
        by_tag = {tag: payload for tag, payload in fields}
        fields = [(tag, by_tag[tag]) for tag in order]
    elif op == "set_field_payload":
        fields = [(tag, bytes.fromhex(recipe["payload_hex"]) if tag == int(recipe["tag"]) else payload) for tag, payload in fields]
    elif op == "set_field_uvar":
        fields = [(tag, encode_uvar(int(recipe["value"])) if tag == int(recipe["tag"]) else payload) for tag, payload in fields]
    elif op == "shift_uvar_field":
        shifted: list[tuple[int, bytes]] = []
        for tag, payload in fields:
            if tag == int(recipe["tag"]):
                value = decode_uvar_exact(payload, 64) + int(recipe["delta"])
                if value < 0:
                    raise ValueError("shifted uvar must stay nonnegative")
                payload = encode_uvar(value)
            shifted.append((tag, payload))
        fields = shifted
    elif op == "append_trailing":
        return body + bytes.fromhex(recipe["hex"])
    elif op == "identity":
        return body
    else:
        raise ValueError(f"unknown record surgery: {op}")
    return encode_fields(fields)


def apply_level_op(data: bytes, recipe: Mapping[str, Any]) -> bytes:
    op = recipe["op"]
    if op == "truncate":
        count = int(recipe["n"])
        if count >= len(data):
            raise ValueError("truncate removes everything")
        return data[: len(data) - count]
    if op == "flip":
        offset = int(recipe["offset"])
        index = offset if offset >= 0 else len(data) + offset
        raw = bytearray(data)
        raw[index] ^= 1 << int(recipe.get("bit", 0))
        return bytes(raw)
    if op == "splice":
        offset = int(recipe["offset"])
        index = offset if offset >= 0 else len(data) + offset
        delete = int(recipe.get("delete", 0))
        insert = bytes.fromhex(recipe.get("insert_hex", ""))
        return data[:index] + insert + data[index + delete :]
    if op == "append":
        return data + bytes.fromhex(recipe["hex"])
    if op == "prefix_set":
        return struct.pack(">Q", int(recipe["value"])) + data[8:]
    if op == "identity":
        return data
    raise ValueError(f"unknown level op: {op}")


def build_rejected_bytes(inputs: Mapping[str, Any], case: Mapping[str, Any], recipe: Mapping[str, Any]) -> bytes:
    """Apply a recipe over independently encoded valid source bytes.

    Levels: wire (full response/request wire), payload (frame payload,
    envelope recomputed), body (method body, payload+envelope recomputed),
    stored (one stored object, response+frame recomputed). Record surgeries
    apply at the body level for request/response records and at the stored
    level for object records.
    """
    target_selector = recipe.get("target", "response")
    if target_selector not in ("request", "response"):
        raise ValueError(f"unknown target selector: {target_selector!r}")
    if "record" in recipe and recipe["record"] not in ("request", "response", "failure", "object", "entry", "bounds", "frame"):
        raise ValueError(f"unknown record selector: {recipe['record']!r}")
    if "level" in recipe and recipe["level"] not in ("wire", "payload", "body"):
        raise ValueError(f"unknown level selector: {recipe['level']!r}")
    context = inputs["context"]
    kind = case.get("kind", "success")
    if kind == "success":
        built = build_success_case(inputs, case)
        request_wire = bytes.fromhex(built["request_wire_hex"])
        response_wire = bytes.fromhex(built["response_wire_hex"])
    elif kind == "failure_response":
        built = build_success_case(inputs, case["base"])
        response_wire = build_failure_wire(inputs, case)
        request_wire = bytes.fromhex(built["request_wire_hex"])
    else:
        raise ValueError(f"unknown case kind: {kind}")
    target = recipe.get("target", "response")
    wire = response_wire if target == "response" else request_wire
    protocol_epoch = protocol_epoch_id()
    if "record" not in recipe:
        level = recipe.get("level", "wire")
        if level == "wire":
            return apply_level_op(wire, recipe)
        max_frame = int(inputs["selected_limits"]["max_frame_bytes"])
        stored, _prefix = split_wire(wire, max_frame)
        payload, _trailer = check_envelope(stored, protocol_epoch)
        frame = decode_frame_payload(payload)
        if level == "payload":
            mutated = apply_level_op(payload, recipe)
            new_wire, _pre, _fid = build_envelope(protocol_epoch, mutated)
            return new_wire
        mutated_body = apply_level_op(frame["body"], recipe)
    else:
        max_frame = int(inputs["selected_limits"]["max_frame_bytes"])
        stored, _prefix = split_wire(wire, max_frame)
        payload, _trailer = check_envelope(stored, protocol_epoch)
        frame = decode_frame_payload(payload)
        if recipe.get("record") in ("request", "response", "failure"):
            mutated_body = apply_record_surgery(frame["body"], recipe)
        elif recipe.get("record") == "object":
            mutated_body = mutate_object_in_body(inputs, case if kind == "success" else case["base"], frame["body"], recipe)
        elif recipe.get("record") == "entry":
            mutated_body = mutate_entry_in_body(inputs, frame["body"], recipe)
        elif recipe.get("record") == "bounds":
            mutated_body = frame["body"]
            mutated_bounds = apply_record_surgery(frame["bounds"], recipe)
            mutated_payload = build_frame_payload(frame["version"], frame["session"], frame["request_id"], frame["kind"], frame["method"], recipe.get("flags", frame["flags"]), mutated_bounds, mutated_body)
            new_wire, _pre, _fid = build_envelope(protocol_epoch, mutated_payload)
            return new_wire
        elif recipe.get("record") == "frame":
            mutated_payload = apply_record_surgery(payload, recipe)
            new_wire, _pre, _fid = build_envelope(protocol_epoch, mutated_payload)
            return new_wire
        else:
            mutated_body = apply_level_op(frame["body"], recipe)
    if kind == "success":
        base_for_signature = case
    else:
        base_id = case.get("base")
        base_for_signature = inputs["cases"].get(base_id, {}) if isinstance(base_id, str) else {}
    frame_bounds = frame["bounds"]
    if base_for_signature.get("signature") is not None and recipe.get("record") in ("object", "entry"):
        try:
            resp_preview = decode_response_body(mutated_body)
            preview_entries = [decode_response_entry(raw) for raw in resp_preview["entries"]]
            actual_k = len(preview_entries)
            actual_b = sum(len(item["stored"]) for item in preview_entries)
            derived = derived_lookup_l(inputs)
            ceiling = int(base_for_signature["request"]["max_response_bytes"])
            refreshed_work = compute_work(actual_k, derived, actual_b, ceiling)
            body_fields = parse_record(mutated_body)
            body_fields = [(tag, encode_uvar(refreshed_work) if tag == 8 else payload) for tag, payload in body_fields]
            mutated_body = encode_fields(body_fields)
            frame_bounds = build_bounds(inputs["selected_limits"], len(mutated_body), actual_k)
        except (ScbError, CheckFailed, ValueError, KeyError, Overflow):
            frame_bounds = frame["bounds"]
    if recipe.get("set_flags") is not None:
        flags = int(recipe["set_flags"])
    else:
        flags = frame["flags"]
    if recipe.get("set_version") is not None:
        version = int(recipe["set_version"])
    else:
        version = frame["version"]
    mutated_payload = build_frame_payload(version, frame["session"], frame["request_id"], frame["kind"], frame["method"], flags, frame_bounds, mutated_body)
    if recipe.get("set_epoch") is not None:
        epoch = bytes.fromhex(recipe["set_epoch"])
        preimage = MAGIC + encode_uvar(FORMAT_VERSION) + encode_uvar(FRAME_CONTRACT_TAG) + epoch + encode_sized(mutated_payload)
        frame_id = blake3.blake3(FRAME_DOMAIN + preimage).digest()
        stored_new = preimage + frame_id
        return struct.pack(">Q", len(stored_new)) + stored_new
    new_wire, _pre, _fid = build_envelope(protocol_epoch, mutated_payload)
    if recipe.get("prefix_delta") is not None:
        (length,) = struct.unpack(">Q", new_wire[:8])
        new_wire = struct.pack(">Q", length + int(recipe["prefix_delta"])) + new_wire[8:]
    return new_wire


def mutate_entry_in_body(inputs: Mapping[str, Any], body: bytes, recipe: Mapping[str, Any]) -> bytes:
    fields = parse_record(body)
    entries_raw = single_field(fields, 7)
    reader = Reader(entries_raw)
    count = reader.uvar(64)
    entries = [reader.sized(MAX_STANDALONE_BYTES) for _ in range(count)]
    reader.finish()
    op = recipe["op"]
    index = int(recipe.get("index", 0))
    if op == "swap_params":
        entries[1], entries[2] = entries[2], entries[1]
    elif op == "drop_param":
        entries = entries[:1]
    elif op == "dup_param":
        entries = entries + [entries[int(recipe.get("index", 1))]]
    elif op == "extra_param":
        ref = recipe["entity_ref"]
        entity = inputs["entities"][ref]
        content_epoch = bytes.fromhex(inputs["context"]["content_epoch"])
        item = build_object(entity, content_epoch)
        kind = int(entity["kind"])
        entries = entries + [
            build_entry(
                bytes.fromhex(entity["id"]),
                kind,
                bytes.fromhex(item["object_id"]),
                bytes.fromhex(item["stored_hex"]),
            )
        ]
    elif op == "set_entry_field":
        entry_fields = parse_record(entries[index])
        entry_fields = [(tag, bytes.fromhex(recipe["value_hex"]) if tag == int(recipe["field"]) else payload) for tag, payload in entry_fields]
        entries[index] = encode_fields(entry_fields)
    elif op == "set_entry_uvar":
        entry_fields = parse_record(entries[index])
        entry_fields = [(tag, encode_uvar(int(recipe["value"])) if tag == int(recipe["field"]) else payload) for tag, payload in entry_fields]
        entries[index] = encode_fields(entry_fields)
    elif op == "flip_entry_field_byte":
        entry_fields = parse_record(entries[index])
        flipped: list[tuple[int, bytes]] = []
        for tag, payload in entry_fields:
            if tag == int(recipe["field"]):
                offset = int(recipe["offset"])
                at = offset if offset >= 0 else len(payload) + offset
                raw = bytearray(payload)
                raw[at] ^= 1 << int(recipe.get("bit", 0))
                payload = bytes(raw)
            flipped.append((tag, payload))
        entries[index] = encode_fields(flipped)
    elif op == "corrupt_entry_framing":
        entry_fields = parse_record(entries[index])
        for tag, payload in entry_fields:
            if tag == int(recipe["field"]):
                mode = recipe["mode"]
                if mode == "drop_len":
                    inner = Reader(payload).sized(MAX_STANDALONE_BYTES)
                    framed = inner
                elif mode == "wrong_len":
                    inner = Reader(payload).sized(MAX_STANDALONE_BYTES)
                    framed = encode_uvar(len(inner) + int(recipe.get("delta", 1))) + inner
                else:
                    raise ValueError(f"unknown framing mode: {mode}")
                entry_fields = [(t, framed if t == int(recipe["field"]) else p) for t, p in entry_fields]
        entries[index] = encode_fields(entry_fields)
    else:
        raise ValueError(f"unknown entry op: {op}")
    items = encode_uvar(len(entries)) + b"".join(encode_sized(entry) for entry in entries)
    fields = [(tag, items if tag == 7 else payload) for tag, payload in fields]
    return encode_fields(fields)


def mutate_object_in_body(inputs: Mapping[str, Any], case: Mapping[str, Any], body: bytes, recipe: Mapping[str, Any]) -> bytes:
    fields = parse_record(body)
    entries_raw = single_field(fields, 7)
    reader = Reader(entries_raw)
    count = reader.uvar(64)
    entries = [reader.sized(MAX_STANDALONE_BYTES) for _ in range(count)]
    reader.finish()
    index = int(recipe.get("index", 0))
    entry = decode_response_entry(entries[index])
    stored = entry["stored"]
    op = recipe["op"]
    if op in ("flip", "truncate", "splice", "append"):
        mutated_stored = apply_level_op(stored, recipe)
    elif op == "object_surgery":
        preimage, trailer = stored[:-32], stored[-32:]
        orader = Reader(preimage)
        orader.take(8)
        orader.uvar(64)
        orader.uvar(32)
        orader.take(32)
        object_record = orader.sized(MAX_STANDALONE_BYTES)
        mutated_record = apply_record_surgery(object_record, recipe["surgery"])
        head = preimage[: len(preimage) - len(encode_sized(object_record))]
        mutated_preimage = head + encode_sized(mutated_record)
        if recipe.get("keep_trailer"):
            mutated_stored = mutated_preimage + trailer
        else:
            mutated_stored = mutated_preimage + blake3.blake3(OBJECT_DOMAIN + mutated_preimage).digest()
    elif op == "param_surgery":
        mutated_stored = mutate_param_in_stored(inputs, case, stored, recipe)
    elif op == "set_object_epoch":
        epoch = bytes.fromhex(recipe["value_hex"])
        if len(epoch) != 32:
            raise ValueError("epoch must be 32 bytes")
        mutated_preimage = stored[:11] + epoch + stored[43:-32]
        mutated_stored = mutated_preimage + blake3.blake3(OBJECT_DOMAIN + mutated_preimage).digest()
    else:
        raise ValueError(f"unknown object op: {op}")
    new_object_id = mutated_stored[-32:]
    entry_fields = parse_record(entries[index])
    refreshed: list[tuple[int, bytes]] = []
    for tag, payload in entry_fields:
        if tag == 3:
            refreshed.append((tag, new_object_id))
        elif tag == 4:
            refreshed.append((tag, encode_sized(mutated_stored)))
        else:
            refreshed.append((tag, payload))
    entry_fields = refreshed
    entries[index] = encode_fields(entry_fields)
    items = encode_uvar(len(entries)) + b"".join(encode_sized(entry) for entry in entries)
    fields = [(tag, items if tag == 7 else payload) for tag, payload in fields]
    return encode_fields(fields)


def mutate_param_in_stored(inputs: Mapping[str, Any], case: Mapping[str, Any], stored: bytes, recipe: Mapping[str, Any]) -> bytes:
    preimage, _trailer = stored[:-32], stored[-32:]
    orader = Reader(preimage)
    orader.take(8)
    orader.uvar(64)
    orader.uvar(32)
    orader.take(32)
    object_record = orader.sized(MAX_STANDALONE_BYTES)
    head = preimage[: len(preimage) - len(encode_sized(object_record))]
    record_fields = parse_record(object_record)
    union_payload = single_field(record_fields, 2)
    ureader = Reader(union_payload)
    _kind, body_bytes = ureader.union()
    ureader.finish()
    body_fields = parse_record(body_bytes)
    sub = recipe["sub"]
    if sub == "set_owner":
        value = bytes.fromhex(recipe["value_hex"])
        body_fields = [(tag, value if tag == 1 else payload) for tag, payload in body_fields]
    elif sub == "set_role":
        body_fields = [(tag, encode_uvar(int(recipe["value"])) if tag == 2 else payload) for tag, payload in body_fields]
    elif sub == "set_ordinal":
        body_fields = [(tag, encode_uvar(int(recipe["value"])) if tag == 3 else payload) for tag, payload in body_fields]
    elif sub == "set_value_type":
        value = encode_mutation_value("TypeExpr", recipe["value_type"])
        body_fields = [(tag, value if tag == 4 else payload) for tag, payload in body_fields]
    else:
        raise ValueError(f"unknown param sub-op: {sub}")
    mutated_param_body = encode_fields(body_fields)
    mutated_union = encode_uvar(6) + encode_sized(mutated_param_body)
    record_fields = [(tag, mutated_union if tag == 2 else payload) for tag, payload in record_fields]
    mutated_record = encode_fields(record_fields)
    mutated_preimage = head + encode_sized(mutated_record)
    if recipe.get("keep_object_id"):
        return mutated_preimage + stored[-32:]
    return mutated_preimage + blake3.blake3(OBJECT_DOMAIN + mutated_preimage).digest()


def build_failure_wire(inputs: Mapping[str, Any], case: Mapping[str, Any]) -> bytes:
    failure = build_failure(int(case["code"]), case["symbol"], bytes.fromhex(case.get("details_hex", "")))
    frame = build_frame_payload(
        PROTOCOL_VERSION_2,
        bytes.fromhex(inputs["context"]["session"]),
        int(case["request_id"]),
        2,
        int(case["method"]),
        4,
        zero_bounds(),
        failure,
    )
    wire, _pre, _fid = build_envelope(protocol_epoch_id(), frame)
    return wire



# Layered validation of rejected bytes



def validate_rejected(inputs: Mapping[str, Any], case: Mapping[str, Any], data: bytes) -> tuple[str, str | None]:
    """Run layer decoders in order; return (failing_layer, scb_code_or_None).

    Oracle semantic layers raise CheckFailed (code None). Earlier layers must
    pass or the recipe is defective.
    """
    context = inputs["context"]
    content_epoch = bytes.fromhex(context["content_epoch"])
    protocol_epoch = protocol_epoch_id()
    recipe = case["recipe"]
    target = recipe.get("target", "response")
    max_frame = int(inputs["selected_limits"]["max_frame_bytes"])
    try:
        stored, _prefix = split_wire(data, max_frame)
    except CheckFailed as error:
        return error.layer, None
    try:
        payload, _trailer = check_envelope(stored, protocol_epoch)
    except ScbError as error:
        if error.code == "SCB_EPOCH_MISMATCH":
            return "frame_envelope", "SCB_EPOCH_MISMATCH"
        return "frame_envelope", error.code
    try:
        frame = decode_frame_payload(payload)
    except ScbError as error:
        return "frame_payload", error.code
    if frame["version"] != PROTOCOL_VERSION_2:
        return "frame_version", None
    if frame["flags"] & ~0x7 != 0:
        return "frame_header", "PROTOCOL_FRAME_INVALID"
    if frame["flags"] & 4 and frame["kind"] != 2:
        return "frame_header", "PROTOCOL_FRAME_INVALID"
    if target == "request" or (frame["kind"] == 1):
        try:
            decode_request_body(frame["body"], {"limits": inputs["selected_limits"]})
        except ScbError as error:
            return "request_record", error.code
        except CheckFailed:
            return "request_range", None
        return "request_accepted", None
    if frame["flags"] & 4:
        try:
            failure = decode_failure(frame["body"])
        except ScbError as error:
            return "failure_record", error.code
        except CheckFailed as error:
            return "failure_record", None
        return "failure_accepted", None
    try:
        response = decode_response_body(frame["body"])
    except ScbError as error:
        return "response_record", error.code
    except CheckFailed:
        return "response_record", None
    try:
        bounds = decode_bounds(frame["bounds"])
    except ScbError as error:
        return "frame_bounds", error.code
    decoded: list[dict[str, Any]] = []
    try:
        items: list[tuple[dict[str, Any], bytes]] = []
        for entry_raw in response["entries"]:
            try:
                entry = decode_response_entry(entry_raw)
            except ScbError as error:
                return "object_envelope", error.code
            try:
                object_record, _trailer = decode_stored_envelope(entry["stored"], content_epoch)
            except ScbError as error:
                return "object_envelope", error.code
            try:
                stored = decode_stored_record(object_record)
            except ScbError as error:
                return "object_record", error.code
            stored["object_id"] = entry["stored"][-32:]
            items.append((entry, stored))
        for entry, stored in items:
            if entry["entity"] != stored["entity_id"] or entry["kind"] != stored["kind"] or entry["object_id"] != stored["object_id"]:
                raise CheckFailed("entry_binding", "entry claim disagrees with stored object")
            decoded.append({"meta": entry, "body": stored["body"]})
    except CheckFailed as error:
        return error.layer, None
    if case.get("base") is None:
        return "semantic_pending", None
    try:
        base_case = inputs["cases"][case["base"]]
        check_context(response, context, bytes.fromhex(inputs["entities"][base_case["entity"]]["id"]))
        actual_k = len(response["entries"])
        actual_b = sum(len(decode_response_entry(raw)["stored"]) for raw in response["entries"])
        expected_work = compute_work(actual_k, derived_lookup_l(inputs), actual_b, int(base_case["request"]["max_response_bytes"]))
        if response["work"] != expected_work:
            raise CheckFailed("work", f"claimed {response['work']} != {expected_work}")
        if bounds["returned_entities"] != len(response["entries"]):
            raise CheckFailed("count", "bounds entity count mismatch")
        if bounds["returned_bytes"] != len(frame["body"]):
            raise CheckFailed("count", "bounds byte count mismatch")
        if bounds["returned_edges"] != 0 or bounds["reached_depth"] != 0 or bounds["omitted"] != 0:
            raise CheckFailed("count", "nonzero edge/depth/omitted")
        if bounds["truncated"] or bounds["continuation"]:
            raise CheckFailed("count", "truncated/continuation must be false")
        if base_case.get("signature") is not None:
            sig = base_case["signature"]
            expected_types = [encode_mutation_value("TypeExpr", inputs["signature_types"][ref]) for ref in sig["parameters"]]
            check_signature(
                [item["meta"] for item in decoded],
                [item["body"] for item in decoded],
                bytes.fromhex(inputs["entities"][sig["function"]]["id"]),
                [bytes.fromhex(inputs["entities"][ref]["id"]) for ref in sig["parameters"]],
                expected_types,
            )
    except ScbError as error:
        return "semantic_decode", error.code
    except CheckFailed as error:
        return error.layer, None
    return "semantic_pending", None



# Accepted / rejected checking against materialized artifacts



_ACCEPTED_CONTRACT = "sley2-entity-read-v2"
_ACCEPTED_CLAIM = "independent-expected"
_ACCEPTED_TOP_FIELDS = frozenset({"contract", "claim", "manifest", "cases", "hellos", "selections", "frame_scenarios"})
_CASE_SCALAR_FIELDS = (
    "request_body_hex",
    "response_body_hex",
    "request_wire_hex",
    "response_wire_hex",
    "request_preimage_hex",
    "response_preimage_hex",
    "request_frame_id",
    "response_frame_id",
    "response_wire_len",
    "work",
    "count_k",
    "stored_b",
)
_OBJECT_FIELDS = ("record_hex", "preimage_hex", "stored_hex", "object_id")
_HELLO_FIELDS = frozenset({"body_hex"})
_FAILED_SELECTION_FIELDS = frozenset({"expected_failure"})
_SELECTION_FIELDS = frozenset(
    {
        "protocol_version",
        "schema_epoch",
        "limits",
        "methods",
        "features",
        "adapters",
        "effects",
        "preimage_hex",
        "transcript_hex",
        "handshake_id",
    }
)


def _same_value(first: Any, second: Any) -> bool:
    """Type-sensitive structural equality: bool is never int."""
    if isinstance(first, bool) or isinstance(second, bool):
        return type(first) is type(second) and first == second
    if isinstance(first, Mapping) and isinstance(second, Mapping):
        return set(first.keys()) == set(second.keys()) and all(
            _same_value(first[key], second[key]) for key in first.keys()
        )
    if isinstance(first, list) and isinstance(second, list):
        return len(first) == len(second) and all(_same_value(left, right) for left, right in zip(first, second))
    return type(first) is type(second) and first == second


_CANONICAL_FRAME_SCENARIOS = {
    "hello_wire1_expected1": {
        "hello": "hello_v2_client",
        "wire_version": 1,
        "expected_version": 1,
        "expect": "accepted",
        "expected_layer": None,
        "expected_code": None,
    },
    "hello_wire1_expected2": {
        "hello": "hello_v2_client",
        "wire_version": 1,
        "expected_version": 2,
        "expect": "rejected",
        "expected_layer": "frame_header",
        "expected_code": "PROTOCOL_DOWNGRADE",
    },
    "hello_wire2_expected1": {
        "hello": "hello_v2_client",
        "wire_version": 2,
        "expected_version": 1,
        "expect": "rejected",
        "expected_layer": "frame_header",
        "expected_code": "PROTOCOL_VERSION_UNSUPPORTED",
    },
    "hello_wire2_expected2": {
        "hello": "hello_v2_client",
        "wire_version": 2,
        "expected_version": 2,
        "expect": "rejected",
        "expected_layer": "frame_header",
        "expected_code": "PROTOCOL_FRAME_INVALID",
    },
}


def _authored_frame_matrix_is_canonical(inputs: Any) -> bool:
    authored = inputs.get("frame_scenarios") if isinstance(inputs, Mapping) else None
    if not isinstance(authored, Mapping):
        return False
    return _same_value(authored, _CANONICAL_FRAME_SCENARIOS)


def check_accepted(inputs: Mapping[str, Any], accepted: Mapping[str, Any]) -> list[str]:
    problems: list[str] = []
    if not isinstance(accepted, Mapping):
        return ["accepted:fields"]
    if accepted.get("contract") != _ACCEPTED_CONTRACT:
        problems.append("accepted-contract")
    if accepted.get("claim") != _ACCEPTED_CLAIM:
        problems.append("accepted-claim")
    if set(accepted.keys()) != set(_ACCEPTED_TOP_FIELDS):
        problems.append("accepted:fields")
    manifest = accepted.get("manifest")
    if not isinstance(manifest, Mapping):
        manifest = {}
    if manifest.get("inputs_sha256") != hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest():
        problems.append("manifest:inputs-sha256")
    if not _authored_frame_matrix_is_canonical(inputs):
        problems.append("accepted:frame_scenarios:inventory")
    raw_cases = inputs.get("cases", {})
    inputs_cases = raw_cases if isinstance(raw_cases, Mapping) else {}
    supplied_cases = accepted.get("cases")
    if not isinstance(supplied_cases, Mapping):
        problems.append("accepted:cases:inventory")
        supplied_cases = {}
    expected_ids: set[str] = set()
    for case_id, case in inputs_cases.items():
        kind = case.get("kind", "success") if isinstance(case, Mapping) else "success"
        if kind != "success":
            problems.append(f"{case_id}:unknown-kind")
            continue
        expected_ids.add(case_id)
    if set(supplied_cases.keys()) - expected_ids:
        problems.append("accepted:cases:inventory")
    for case_id in inputs_cases:
        if case_id not in expected_ids:
            continue
        case = inputs_cases[case_id]
        try:
            built = build_success_case(inputs, case)
        except (ScbError, CheckFailed, ValueError) as error:
            problems.append(f"{case_id}:rebuild:{error}")
            continue
        supplied = supplied_cases.get(case_id)
        if supplied is None:
            problems.append(f"{case_id}:missing-expected")
            continue
        if not isinstance(supplied, Mapping):
            problems.append(f"{case_id}:fields")
            continue
        if set(supplied.keys()) != ({"id", "objects"} | set(_CASE_SCALAR_FIELDS)):
            problems.append(f"{case_id}:fields")
        if not _same_value(case_id, supplied.get("id")):
            problems.append(f"{case_id}:id")
        for field in _CASE_SCALAR_FIELDS:
            if not _same_value(built[field], supplied.get(field)):
                problems.append(f"{case_id}:{field}")
        authored_refs = case.get("objects", []) if isinstance(case, Mapping) else []
        supplied_objects = supplied.get("objects")
        if not isinstance(supplied_objects, Mapping):
            problems.append(f"{case_id}:objects:inventory")
        else:
            if set(supplied_objects.keys()) - set(authored_refs):
                problems.append(f"{case_id}:objects:inventory")
            for ref in authored_refs:
                candidate = supplied_objects.get(ref)
                if not isinstance(candidate, Mapping):
                    for field in _OBJECT_FIELDS:
                        problems.append(f"{case_id}:object:{ref}:{field}")
                    continue
                if set(candidate.keys()) != set(_OBJECT_FIELDS):
                    problems.append(f"{case_id}:object:{ref}:fields")
                for field in _OBJECT_FIELDS:
                    if not _same_value(built["objects"][ref][field], candidate.get(field)):
                        problems.append(f"{case_id}:object:{ref}:{field}")
        semantic_check(inputs, case, supplied, problems)
    check_selection(inputs, accepted, problems)
    return problems


def _admit_frame_header(frame: Mapping[str, Any]) -> None:
    if frame["kind"] not in (1, 2, 3, 4):
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")
    if frame["version"] < PROTOCOL_VERSION_2:
        raise CheckFailed("frame_header", "PROTOCOL_DOWNGRADE")
    if frame["version"] > PROTOCOL_VERSION_2:
        raise CheckFailed("frame_header", "PROTOCOL_VERSION_UNSUPPORTED")
    if frame["kind"] == 4 and frame["version"] != 1:
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")
    if frame["flags"] & ~0x7:
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")
    if frame["flags"] & 0x4 and frame["kind"] not in (2, 3):
        raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")


def _bind_frame(inputs: Mapping[str, Any], case: Mapping[str, Any], frame: Mapping[str, Any], side: str) -> None:
    if frame["session"] != bytes.fromhex(inputs["context"]["session"]):
        raise CheckFailed("frame_binding", "session")
    if frame["request_id"] != int(case["request"]["request_id"]):
        raise CheckFailed("frame_binding", "request_id")
    if frame["kind"] != (1 if side == "request" else 2):
        raise CheckFailed("frame_binding", "kind")
    if frame["method"] != int(case["method"]):
        raise CheckFailed("frame_binding", "method")
    if frame["flags"] != 0:
        raise CheckFailed("frame_binding", "flags")


def _check_request_bounds(bounds: Mapping[str, Any]) -> None:
    for key in LIMIT_KEYS:
        if bounds["applied_limits"][key] != 0:
            raise CheckFailed("request_bounds", key)
    for key in ("returned_bytes", "returned_entities", "returned_edges", "reached_depth", "omitted"):
        if bounds[key] != 0:
            raise CheckFailed("request_bounds", key)
    if bounds["truncated"]:
        raise CheckFailed("request_bounds", "truncated")
    if bounds["continuation"]:
        raise CheckFailed("request_bounds", "continuation")


def _check_response_applied_limits(bounds: Mapping[str, Any], selected: Mapping[str, Any]) -> None:
    for key in LIMIT_KEYS:
        if bounds["applied_limits"][key] != int(selected[key]):
            raise CheckFailed("applied_limits", key)


def _decode_supplied_request(inputs: Mapping[str, Any], body: bytes) -> dict[str, Any]:
    try:
        return decode_request_body(body, {"limits": inputs["selected_limits"]})
    except ScbError as error:
        raise CheckFailed("request_record", error.code) from error


def _bind_request_body(inputs: Mapping[str, Any], case: Mapping[str, Any], decoded: Mapping[str, Any]) -> None:
    if decoded["root"] != bytes.fromhex(inputs["context"]["root"]):
        raise CheckFailed("request_binding", "root")
    if decoded["entity"] != bytes.fromhex(inputs["entities"][case["entity"]]["id"]):
        raise CheckFailed("request_binding", "entity")
    authored = (
        ("max_objects", decoded["max_objects"], int(case["request"]["max_objects"])),
        ("max_response_bytes", decoded["ceiling_m"], int(case["request"]["max_response_bytes"])),
        ("max_work", decoded["max_work"], int(case["request"]["max_work"])),
    )
    for key, actual, want in authored:
        if actual != want:
            raise CheckFailed("request_binding", key)


def _decode_actual_frame(inputs: Mapping[str, Any], supplied: Mapping[str, Any], side: str) -> dict[str, Any]:
    raw = supplied.get(f"{side}_wire_hex") if isinstance(supplied, Mapping) else None
    if not isinstance(raw, str):
        raise CheckFailed("wire_prefix", "missing wire hex")
    try:
        wire = bytes.fromhex(raw)
    except ValueError as error:
        raise CheckFailed("wire_prefix", "wire hex decode failed") from error
    stored, _prefix = split_wire(wire, int(inputs["selected_limits"]["max_frame_bytes"]))
    try:
        payload, trailer = check_envelope(stored, protocol_epoch_id())
    except ScbError as error:
        raise CheckFailed("frame_envelope", error.code) from error
    try:
        frame = decode_frame_payload(payload)
    except ScbError as error:
        raise CheckFailed("frame_payload", error.code) from error
    try:
        bounds = decode_bounds(frame["bounds"])
    except ScbError as error:
        raise CheckFailed("frame_bounds", error.code) from error
    return {
        "wire": wire,
        "stored": stored,
        "preimage": stored[:-32],
        "trailer": trailer,
        "payload": payload,
        "frame": frame,
        "bounds": bounds,
    }


def _admit_supplied_side(inputs: Mapping[str, Any], case: Mapping[str, Any], supplied: Mapping[str, Any], side: str) -> dict[str, Any]:
    admitted = _decode_actual_frame(inputs, supplied, side)
    frame = admitted["frame"]
    bounds = admitted["bounds"]
    _admit_frame_header(frame)
    _bind_frame(inputs, case, frame, side)
    if side == "request":
        _check_request_bounds(bounds)
        _bind_request_body(inputs, case, _decode_supplied_request(inputs, frame["body"]))
    else:
        _check_response_applied_limits(bounds, inputs["selected_limits"])
    return admitted


def _required_response_k(case: Mapping[str, Any]) -> int:
    signature = case.get("signature")
    if isinstance(signature, Mapping) and isinstance(signature.get("parameters"), list):
        return 1 + len(signature["parameters"])
    objects = case.get("objects")
    if isinstance(objects, list):
        return len(objects)
    raise CheckFailed("count", "missing authored order")


def _check_actual_counts(response: Mapping[str, Any], bounds: Mapping[str, Any], body_len: int, required_k: int) -> None:
    actual_k = len(response["entries"])
    if actual_k != required_k:
        raise CheckFailed("count", "entry count mismatch")
    if bounds["returned_entities"] != actual_k:
        raise CheckFailed("count", "bounds entity count mismatch")
    if bounds["returned_bytes"] != body_len:
        raise CheckFailed("count", "bounds byte count mismatch")
    if bounds["returned_edges"] != 0 or bounds["reached_depth"] != 0 or bounds["omitted"] != 0:
        raise CheckFailed("count", "nonzero edge/depth/omitted")
    if bounds["truncated"] or bounds["continuation"]:
        raise CheckFailed("count", "truncated/continuation must be false")


def _authenticate_actual_entries(entry_blobs: list[bytes], content_epoch: bytes) -> tuple[list[dict[str, Any]], list[bytes], int]:
    metas: list[dict[str, Any]] = []
    bodies: list[bytes] = []
    total_b = 0
    for raw in entry_blobs:
        try:
            entry = decode_response_entry(raw)
        except ScbError as error:
            raise CheckFailed("object_envelope", error.code) from error
        try:
            object_record, _trailer = decode_stored_envelope(entry["stored"], content_epoch)
        except ScbError as error:
            raise CheckFailed("object_envelope", error.code) from error
        try:
            stored = decode_stored_record(object_record)
        except ScbError as error:
            raise CheckFailed("object_record", error.code) from error
        stored["object_id"] = entry["stored"][-32:]
        if entry["entity"] != stored["entity_id"] or entry["kind"] != stored["kind"] or entry["object_id"] != stored["object_id"]:
            raise CheckFailed("entry_binding", "entry claim disagrees with stored object")
        metas.append(entry)
        bodies.append(stored["body"])
        total_b += len(entry["stored"])
    return metas, bodies, total_b


def _check_actual_resources(actual_k: int, body_len: int, actual_w: int, case: Mapping[str, Any], selected: Mapping[str, Any]) -> None:
    request = case["request"]
    if actual_k > int(request["max_objects"]) or actual_k > int(selected["max_entities"]):
        raise CheckFailed("response_range", "max_objects")
    if body_len > int(request["max_response_bytes"]) or body_len > int(selected["max_response_bytes"]):
        raise CheckFailed("response_range", "max_response_bytes")
    if actual_w > int(request["max_work"]) or actual_w > int(selected["max_work"]):
        raise CheckFailed("response_range", "max_work")


def _check_outgoing_ceiling(actual_wire: bytes, selected: Mapping[str, Any]) -> None:
    if len(actual_wire) > int(selected["max_frame_bytes"]):
        raise CheckFailed("outgoing_wire_ceiling", "full wire exceeds selected ceiling")


def semantic_check(inputs: Mapping[str, Any], case: Mapping[str, Any], built: Mapping[str, Any], problems: list[str]) -> None:
    case_id = case.get("id", "?") if isinstance(case, Mapping) else "?"
    request_admitted: dict[str, Any] | None = None
    response_admitted: dict[str, Any] | None = None
    response_failed = False
    for side in ("request", "response"):
        try:
            admitted = _admit_supplied_side(inputs, case, built, side)
            if side == "request":
                request_admitted = admitted
            else:
                response_admitted = admitted
        except CheckFailed as error:
            problems.append(f"{case_id}:semantic:{side}:{error.layer}:{error.detail}")
            if side == "response":
                response_failed = True
        except (KeyError, TypeError, AttributeError):
            problems.append(f"{case_id}:semantic:{side}:wire_prefix:malformed supplied record")
            if side == "response":
                response_failed = True
    if response_failed or response_admitted is None:
        return
    try:
        context = inputs["context"]
        content_epoch = bytes.fromhex(context["content_epoch"])
        requested = bytes.fromhex(inputs["entities"][case["entity"]]["id"])
        actual_body = response_admitted["frame"]["body"]
        actual_bounds = response_admitted["bounds"]
        actual_wire = response_admitted["wire"]
        try:
            response = decode_response_body(actual_body)
        except ScbError as error:
            raise CheckFailed("response_record", error.code) from error
        try:
            check_context(response, context, requested)
        except CheckFailed as error:
            raise CheckFailed(error.layer, error.detail) from error
        required_k = _required_response_k(case)
        try:
            _check_actual_counts(response, actual_bounds, len(actual_body), required_k)
        except CheckFailed as error:
            raise CheckFailed(error.layer, error.detail) from error
        metas, bodies, actual_b = _authenticate_actual_entries(response["entries"], content_epoch)
        actual_k = len(metas)
        try:
            lookup_l = derived_lookup_l(inputs)
        except CheckFailed as error:
            raise CheckFailed(error.layer, error.detail) from error
        try:
            check_work(response, actual_k, lookup_l, actual_b, int(case["request"]["max_response_bytes"]))
        except CheckFailed as error:
            raise CheckFailed(error.layer, error.detail) from error
        if case.get("signature") is not None:
            signature = case["signature"]
            expected_types = [encode_mutation_value("TypeExpr", inputs["signature_types"][ref]) for ref in signature["parameters"]]
            function_id = bytes.fromhex(inputs["entities"][signature["function"]]["id"])
            parameter_ids = [bytes.fromhex(inputs["entities"][ref]["id"]) for ref in signature["parameters"]]
            try:
                check_signature(metas, bodies, function_id, parameter_ids, expected_types)
            except CheckFailed as error:
                raise CheckFailed(error.layer, error.detail) from error
            except ScbError as error:
                raise CheckFailed("signature", error.code) from error
        _check_actual_resources(actual_k, len(actual_body), response["work"], case, inputs["selected_limits"])
        _check_outgoing_ceiling(actual_wire, inputs["selected_limits"])
    except CheckFailed as error:
        problems.append(f"{case_id}:semantic:response:{error.layer}:{error.detail}")
        return
    except (KeyError, TypeError, AttributeError, ValueError):
        problems.append(f"{case_id}:semantic:response:wire_prefix:malformed supplied record")
        return
    if not isinstance(built, Mapping):
        problems.append(f"{case_id}:semantic:response:component_binding:record")
        return
    if request_admitted is not None:
        request_frame = request_admitted["frame"]
        for key, actual_bytes in (
            ("request_body_hex", request_frame["body"]),
            ("request_preimage_hex", request_admitted["preimage"]),
            ("request_frame_id", request_admitted["trailer"]),
        ):
            candidate = built.get(key)
            if not isinstance(candidate, str):
                problems.append(f"{case_id}:semantic:request:component_binding:{key}")
                continue
            try:
                candidate_bytes = bytes.fromhex(candidate)
            except ValueError:
                problems.append(f"{case_id}:semantic:request:component_binding:{key}")
                continue
            if candidate_bytes != actual_bytes:
                problems.append(f"{case_id}:semantic:request:component_binding:{key}")
    for key, actual_bytes in (
        ("response_body_hex", actual_body),
        ("response_preimage_hex", response_admitted["preimage"]),
        ("response_frame_id", response_admitted["trailer"]),
    ):
        candidate = built.get(key)
        if not isinstance(candidate, str):
            problems.append(f"{case_id}:semantic:response:component_binding:{key}")
            continue
        try:
            candidate_bytes = bytes.fromhex(candidate)
        except ValueError:
            problems.append(f"{case_id}:semantic:response:component_binding:{key}")
            continue
        if candidate_bytes != actual_bytes:
            problems.append(f"{case_id}:semantic:response:component_binding:{key}")
    for key, actual_value in (
        ("response_wire_len", len(actual_wire)),
        ("count_k", actual_k),
        ("stored_b", actual_b),
        ("work", response["work"]),
    ):
        candidate = built.get(key)
        if isinstance(candidate, bool) or not isinstance(candidate, int) or candidate != actual_value:
            problems.append(f"{case_id}:semantic:response:component_binding:{key}")


_FRAME_SCENARIO_AUTHORED = frozenset({"hello", "wire_version", "expected_version", "expect", "expected_layer", "expected_code"})
_FRAME_SCENARIO_EXTRA = frozenset({"body_hex", "wire_hex", "preimage_hex", "frame_id"})


def _check_single_hello_row(inputs: Mapping[str, Any], row_id: str, authored_row: Mapping[str, Any], supplied_row: Mapping[str, Any], problems: list[str]) -> None:
    for field in ("hello", "wire_version", "expected_version", "expect", "expected_layer", "expected_code"):
        try:
            same = field in supplied_row and _same_value(authored_row.get(field), supplied_row.get(field))
        except (ValueError, KeyError, TypeError, AttributeError):
            same = False
        if not same:
            problems.append(f"hello_frame:{row_id}:scenario_binding:{field}")
    try:
        supplied_keys = set(supplied_row.keys())
    except (ValueError, KeyError, TypeError, AttributeError):
        problems.append(f"hello_frame:{row_id}:scenario_binding:fields")
        return
    expected_keys = set(_FRAME_SCENARIO_AUTHORED) | set(_FRAME_SCENARIO_EXTRA)
    if supplied_keys != expected_keys:
        for extra in sorted(supplied_keys - expected_keys, key=str):
            problems.append(f"hello_frame:{row_id}:scenario_binding:{extra}")
    authored_expected = authored_row.get("expected_version")
    wire: bytes | None = None
    stored: bytes | None = None
    frame: dict[str, Any] | None = None
    semantic: tuple[str, str] | None = None
    hello_valid = False
    try:
        raw_hex = supplied_row.get("wire_hex")
        if not isinstance(raw_hex, str):
            raise CheckFailed("wire_prefix", "missing wire hex")
        try:
            wire = bytes.fromhex(raw_hex)
        except ValueError as error:
            raise CheckFailed("wire_prefix", "wire hex decode failed") from error
        selected = inputs.get("selected_limits") if isinstance(inputs, Mapping) else None
        ceiling = selected.get("max_frame_bytes") if isinstance(selected, Mapping) else None
        if isinstance(ceiling, bool) or not isinstance(ceiling, int):
            raise CheckFailed("wire_prefix", "malformed supplied record")
        try:
            stored, _prefix = split_wire(wire, ceiling)
        except CheckFailed as error:
            raise CheckFailed(error.layer, error.detail) from error
        try:
            payload, trailer = check_envelope(stored, protocol_epoch_id())
        except ScbError as error:
            raise CheckFailed("frame_envelope", error.code) from error
        try:
            frame = decode_frame_payload(payload)
        except ScbError as error:
            raise CheckFailed("frame_payload", error.code) from error
        if frame["kind"] not in (1, 2, 3, 4):
            raise CheckFailed("frame_header", "PROTOCOL_FRAME_INVALID")
        try:
            decode_bounds(frame["bounds"])
        except ScbError as error:
            raise CheckFailed("frame_bounds", error.code) from error
        try:
            _admit_hello_frame(frame, authored_expected)
        except CheckFailed as error:
            raise CheckFailed(error.layer, error.detail) from error
        try:
            _decode_supplied_hello(frame["body"])
        except ScbError as error:
            raise CheckFailed("hello_body", error.code) from error
        except CheckFailed as error:
            raise CheckFailed(error.layer, error.detail) from error
        hello_valid = True
    except CheckFailed as error:
        semantic = (error.layer, error.detail)
    except (KeyError, TypeError, AttributeError, ValueError):
        semantic = ("wire_prefix", "malformed supplied record")
    authored_expect = authored_row.get("expect")
    authored_layer = authored_row.get("expected_layer")
    authored_code = authored_row.get("expected_code")
    if semantic is not None:
        if authored_expect == "accepted":
            problems.append(f"hello_frame:{row_id}:{semantic[0]}:{semantic[1]}")
        elif authored_expect == "rejected":
            if semantic[0] == authored_layer and _same_value(semantic[1], authored_code):
                pass
            else:
                problems.append(f"hello_frame:{row_id}:{semantic[0]}:{semantic[1]}")
        else:
            problems.append(f"hello_frame:{row_id}:scenario_binding:expect")
    else:
        if authored_expect == "rejected":
            problems.append(f"hello_frame:{row_id}:scenario_binding:expect")
        elif authored_expect != "accepted":
            problems.append(f"hello_frame:{row_id}:scenario_binding:expect")
    reconstructed = _reconstruct_hello_row(inputs, authored_row)
    if reconstructed is None:
        return
    expected_body, expected_wire, expected_preimage, expected_frame_id = reconstructed
    for field, expected_value in (
        ("body_hex", expected_body.hex()),
        ("wire_hex", expected_wire.hex()),
        ("preimage_hex", expected_preimage.hex()),
        ("frame_id", expected_frame_id.hex()),
    ):
        try:
            same = _same_value(supplied_row.get(field), expected_value)
        except (ValueError, KeyError, TypeError, AttributeError):
            same = False
        if not same:
            problems.append(f"hello_frame:{row_id}:scenario_binding:{field}")
    if wire is not None and stored is not None and frame is not None:
        actual_body = frame["body"]
        actual_preimage = stored[:-32]
        actual_trailer = stored[-32:]
        for field, actual_value in (
            ("body_hex", actual_body.hex()),
            ("preimage_hex", actual_preimage.hex()),
            ("frame_id", actual_trailer.hex()),
        ):
            try:
                same = _same_value(supplied_row.get(field), actual_value)
            except (ValueError, KeyError, TypeError, AttributeError):
                same = False
            if not same:
                problems.append(f"hello_frame:{row_id}:component_binding:{field}")
        if hello_valid and actual_body != expected_body:
            problems.append(f"hello_frame:{row_id}:scenario_binding:hello")


def _check_frame_scenarios(inputs: Mapping[str, Any], accepted: Mapping[str, Any], problems: list[str]) -> None:
    authored_has = isinstance(inputs, Mapping) and "frame_scenarios" in inputs
    supplied_has = isinstance(accepted, Mapping) and "frame_scenarios" in accepted
    if not authored_has and not supplied_has:
        return
    authored = inputs.get("frame_scenarios") if isinstance(inputs, Mapping) else None
    supplied = accepted.get("frame_scenarios") if isinstance(accepted, Mapping) else None
    if not isinstance(authored, Mapping) or not isinstance(supplied, Mapping):
        problems.append("accepted:frame_scenarios:inventory")
        return
    if set(supplied.keys()) != set(authored.keys()):
        problems.append("accepted:frame_scenarios:inventory")
    for row_id in sorted(set(authored.keys()) & set(supplied.keys()), key=str):
        authored_row = authored.get(row_id)
        supplied_row = supplied.get(row_id)
        if not isinstance(row_id, str) or not isinstance(authored_row, Mapping) or not isinstance(supplied_row, Mapping):
            problems.append(f"hello_frame:{row_id}:scenario_binding:fields")
            continue
        if set(authored_row.keys()) != set(_FRAME_SCENARIO_AUTHORED):
            problems.append(f"hello_frame:{row_id}:scenario_binding:fields")
            continue
        _check_single_hello_row(inputs, row_id, authored_row, supplied_row, problems)


def check_selection(inputs: Mapping[str, Any], accepted: Mapping[str, Any], problems: list[str]) -> None:
    raw_hellos = inputs.get("hellos", {})
    authored_hellos = raw_hellos if isinstance(raw_hellos, Mapping) else {}
    raw_scenarios = inputs.get("selection_scenarios", {})
    scenarios = raw_scenarios if isinstance(raw_scenarios, Mapping) else {}
    supplied = accepted if isinstance(accepted, Mapping) else {}
    supplied_hellos = supplied.get("hellos")
    if not isinstance(supplied_hellos, Mapping):
        problems.append("accepted:hellos:inventory")
        supplied_hellos = {}
    elif set(supplied_hellos.keys()) - set(authored_hellos.keys()):
        problems.append("accepted:hellos:inventory")
    supplied_selections = supplied.get("selections")
    if not isinstance(supplied_selections, Mapping):
        problems.append("accepted:selections:inventory")
        supplied_selections = {}
    elif set(supplied_selections.keys()) - set(scenarios.keys()):
        problems.append("accepted:selections:inventory")
    rebuilt_bodies: dict[str, bytes] = {}
    for name, hello in authored_hellos.items():
        try:
            body = build_hello(hello)
        except (ScbError, CheckFailed, ValueError, KeyError, TypeError, AttributeError) as error:
            problems.append(f"hello:{name}:{error}")
            continue
        rebuilt_bodies[name] = body
        candidate = supplied_hellos.get(name)
        if not isinstance(candidate, Mapping):
            problems.append(f"hello:{name}:body")
            continue
        if set(candidate.keys()) != set(_HELLO_FIELDS):
            problems.append(f"hello:{name}:fields")
        if candidate.get("body_hex") != body.hex():
            problems.append(f"hello:{name}:body")
    for name, scenario in scenarios.items():
        if not isinstance(scenario, Mapping):
            problems.append(f"selection:{name}:fields")
            continue
        if scenario.get("expect") == "fail":
            try:
                negotiate_versioned(authored_hellos[scenario["client"]], authored_hellos[scenario["server"]])
            except CheckFailed as error:
                if error.layer != scenario.get("expected_layer", "selection"):
                    problems.append(f"selection:{name}:layer:{error.layer}")
            except (ScbError, ValueError, KeyError, TypeError, AttributeError) as error:
                problems.append(f"selection:{name}:unexpected:{error}")
            else:
                problems.append(f"selection:{name}:accepted")
            candidate = supplied_selections.get(name)
            if not isinstance(candidate, Mapping) or candidate.get("expected_failure") != "selection":
                problems.append(f"selection:{name}:missing-expected-failure")
            if isinstance(candidate, Mapping) and set(candidate.keys()) != set(_FAILED_SELECTION_FIELDS):
                problems.append(f"selection:{name}:fields")
            continue
        try:
            selection = negotiate_versioned(authored_hellos[scenario["client"]], authored_hellos[scenario["server"]])
        except (ScbError, CheckFailed, ValueError, KeyError, TypeError, AttributeError) as error:
            problems.append(f"selection:{name}:{error}")
            continue
        try:
            preimage = build_selected(selection)
            client_body = rebuilt_bodies[scenario["client"]]
            server_body = rebuilt_bodies[scenario["server"]]
            transcript = client_body + server_body + preimage
            digest = handshake_id(client_body, server_body, preimage)
        except (ScbError, CheckFailed, ValueError, KeyError, TypeError, AttributeError) as error:
            problems.append(f"selection:{name}:transcript:{error}")
            continue
        candidate = supplied_selections.get(name)
        if not isinstance(candidate, Mapping):
            candidate = {}
        if set(candidate.keys()) != set(_SELECTION_FIELDS):
            problems.append(f"selection:{name}:fields")
        if not _same_value(selection["protocol_version"], candidate.get("protocol_version")):
            problems.append(f"selection:{name}:version")
        if not _same_value(selection["methods"], candidate.get("methods")):
            problems.append(f"selection:{name}:methods")
        if not _same_value(selection["schema_epoch"], candidate.get("schema_epoch")):
            problems.append(f"selection:{name}:epoch")
        if preimage.hex() != candidate.get("preimage_hex"):
            problems.append(f"selection:{name}:preimage")
        if transcript.hex() != candidate.get("transcript_hex"):
            problems.append(f"selection:{name}:transcript")
        if digest.hex() != candidate.get("handshake_id"):
            problems.append(f"selection:{name}:handshake")
        if not _same_value(selection["limits"], candidate.get("limits")):
            problems.append(f"selection:{name}:limits")
        if not _same_value(selection["features"], candidate.get("features")):
            problems.append(f"selection:{name}:features")
        if not _same_value(selection["adapters"], candidate.get("adapters")):
            problems.append(f"selection:{name}:adapters")
        if not _same_value(selection["effects"], candidate.get("effects")):
            problems.append(f"selection:{name}:effects")
    _check_frame_scenarios(inputs, accepted, problems)


_REJECTED_CONTRACT = "sley2-entity-read-v2-rejected"
_REJECTED_CLAIM = "independent-expected"
_REJECTED_TOP_FIELDS = frozenset({"contract", "claim", "manifest", "cases"})
_REJECTED_KINDS = frozenset(
    {
        "relation",
        "runtime_sequence",
        "failure_response",
        "failure_wire",
        "owner_case",
        "fill_recipe",
        "hello_invalid",
        "selection_invalid",
    }
)


def _bind_expected_row(expected: Mapping[str, Any], supplied: Mapping[str, Any], row_id: str, problems: list[str]) -> None:
    if set(supplied.keys()) != set(expected.keys()):
        problems.append(f"rejected:{row_id}:fields")
    for key in expected:
        if not _same_value(expected[key], supplied.get(key)):
            problems.append(f"rejected:{row_id}:{key}")


def check_rejected(inputs: Mapping[str, Any], rejected: Mapping[str, Any]) -> list[str]:
    problems: list[str] = []
    if not isinstance(rejected, Mapping):
        return ["rejected:fields"]
    if rejected.get("contract") != _REJECTED_CONTRACT:
        problems.append("rejected-contract")
    if rejected.get("claim") != _REJECTED_CLAIM:
        problems.append("rejected-claim")
    if set(rejected.keys()) != set(_REJECTED_TOP_FIELDS):
        problems.append("rejected:fields")
    manifest = rejected.get("manifest")
    if not isinstance(manifest, Mapping):
        manifest = {}
    if manifest.get("inputs_sha256") != hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest():
        problems.append("manifest:inputs-sha256")
    raw_authored = inputs.get("rejected", [])
    authored_rows = raw_authored if isinstance(raw_authored, list) else []
    authored_valid = isinstance(raw_authored, list)
    authored_ids: list[str] = []
    authored_by_id: dict[str, Mapping[str, Any]] = {}
    for row in authored_rows:
        if not isinstance(row, Mapping) or not isinstance(row.get("id"), str) or not row["id"]:
            authored_valid = False
            continue
        authored_ids.append(row["id"])
        authored_by_id.setdefault(row["id"], row)
    if not authored_valid or len(set(authored_ids)) != len(authored_ids):
        problems.append("inputs:rejected:ids")
    raw_supplied = rejected.get("cases")
    supplied_list = raw_supplied if isinstance(raw_supplied, list) else []
    if not isinstance(raw_supplied, list):
        problems.append("rejected:cases:inventory")
    supplied_ids: list[str] = []
    shaped_supplied: list[Mapping[str, Any]] = []
    for row in supplied_list:
        if not isinstance(row, Mapping) or not isinstance(row.get("id"), str):
            problems.append("rejected:cases:inventory")
            continue
        supplied_ids.append(row["id"])
        shaped_supplied.append(row)
    if supplied_ids != authored_ids:
        problems.append("rejected:cases:inventory")
    for supplied in shaped_supplied:
        row_id = supplied["id"]
        authored = authored_by_id.get(row_id)
        if authored is None:
            continue
        _bind_rejected_row(inputs, authored, supplied, problems)
        try:
            _validate_rejected_row(inputs, authored, supplied, problems)
        except (KeyError, TypeError, AttributeError):
            problems.append(f"rejected:{row_id}:fields")
    return problems


def _bind_rejected_row(
    inputs: Mapping[str, Any],
    authored: Mapping[str, Any],
    supplied: Mapping[str, Any],
    problems: list[str],
) -> None:
    # Dispatch on the authored kind; a supplied kind never authorizes itself.
    row_id = authored["id"]
    if "kind" not in authored:
        try:
            rebuilt = build_rejected_bytes(inputs, inputs["cases"][authored["base"]], authored["recipe"])
        except (ScbError, CheckFailed, ValueError, KeyError, TypeError) as error:
            problems.append(f"rejected:{row_id}:rebuild:{error}")
            return
        expected = dict(authored)
        expected["input_hex"] = rebuilt.hex()
        _bind_expected_row(expected, supplied, row_id, problems)
        return
    kind = authored["kind"]
    if not isinstance(kind, str) or kind not in _REJECTED_KINDS:
        problems.append(f"rejected:{row_id}:kind")
        return
    if kind in ("failure_response", "failure_wire"):
        try:
            rebuilt = build_failure_wire(inputs, authored)
        except (ScbError, CheckFailed, ValueError, KeyError, TypeError) as error:
            problems.append(f"rejected:{row_id}:rebuild:{error}")
            return
        expected = dict(authored)
        expected["input_hex"] = rebuilt.hex()
        _bind_expected_row(expected, supplied, row_id, problems)
        return
    if kind == "relation":
        try:
            derived = derive_relation(inputs, dict(authored))
        except (ScbError, CheckFailed, ValueError, KeyError, TypeError) as error:
            problems.append(f"{row_id}:relation-error:{error}")
            return
        _bind_expected_row(derived, supplied, row_id, problems)
        return
    _bind_expected_row(authored, supplied, row_id, problems)


def _validate_rejected_row(
    inputs: Mapping[str, Any],
    authored: Mapping[str, Any],
    supplied: Mapping[str, Any],
    problems: list[str],
) -> None:
    row_id = authored["id"]
    if "kind" in authored:
        kind = authored["kind"]
        if not isinstance(kind, str) or kind not in _REJECTED_KINDS:
            return
    else:
        kind = None
    if kind == "runtime_sequence":
        check_stateful_spec(supplied, problems)
        return
    if kind == "relation":
        check_relation(inputs, supplied, problems)
        return
    if kind == "fill_recipe":
        check_fill_recipe(inputs, authored, problems)
        return
    if kind == "hello_invalid":
        try:
            build_hello(supplied["hello"])
        except CheckFailed as error:
            if error.layer != supplied.get("failing_layer"):
                problems.append(f"{row_id}:layer:{error.layer}")
        except (ScbError, ValueError) as error:
            problems.append(f"{row_id}:unexpected:{error}")
        else:
            problems.append(f"{row_id}:accepted")
        return
    if kind == "selection_invalid":
        try:
            negotiate_versioned(inputs["hellos"][supplied["client"]], inputs["hellos"][supplied["server"]])
        except CheckFailed as error:
            if error.layer != supplied.get("failing_layer"):
                problems.append(f"{row_id}:layer:{error.layer}")
        except (ScbError, ValueError) as error:
            problems.append(f"{row_id}:unexpected:{error}")
        else:
            problems.append(f"{row_id}:accepted")
        return
    if kind == "owner_case":
        check_owner_case(inputs, supplied, problems)
        return
    if kind in ("failure_response", "failure_wire"):
        try:
            data = bytes.fromhex(supplied["input_hex"])
        except (KeyError, TypeError, ValueError):
            problems.append(f"rejected:{row_id}:input_hex")
            return
        layer, _code = validate_rejected(inputs, {"recipe": {"target": "failure"}}, data)
        if layer != "failure_accepted":
            problems.append(f"{row_id}:expected-accepted-failure-wire:{layer}")
        else:
            try:
                stored, _p = split_wire(data, int(inputs["selected_limits"]["max_frame_bytes"]))
                payload, _t = check_envelope(stored, protocol_epoch_id())
                frame = decode_frame_payload(payload)
                failure = decode_failure(frame["body"])
            except (ScbError, CheckFailed, ValueError) as error:
                problems.append(f"{row_id}:failure-decode:{error}")
                return
            if failure["code"] != supplied["expected_code"] or failure["symbol"] != supplied["expected_symbol"]:
                problems.append(f"{row_id}:failure-identity")
            if failure["retryability"] != supplied["expected_retryability"]:
                problems.append(f"{row_id}:failure-retryability")
        return
    if kind is not None:
        return
    try:
        data = bytes.fromhex(supplied["input_hex"])
    except (KeyError, TypeError, ValueError):
        problems.append(f"rejected:{row_id}:input_hex")
        return
    layer, code = validate_rejected(inputs, supplied, data)
    if layer != supplied["failing_layer"]:
        problems.append(f"{row_id}:layer:{layer}")
        return
    if supplied.get("expected_scb") is not None and code != supplied["expected_scb"]:
        problems.append(f"{row_id}:scb:{code}")
        return
    if supplied.get("expected_code") is not None and CODES.get(supplied.get("expected_symbol", "")) != supplied["expected_code"]:
        problems.append(f"{row_id}:registry")


def check_owner_case(inputs: Mapping[str, Any], case: Mapping[str, Any], problems: list[str]) -> None:
    """Validate an owner-seam expectation against semantic inputs (no bytes claim).

    The request bytes are independently valid; the declared owner failure
    (wrong root, absent entity, wrong kind, missing parameter) is a runtime
    comparison obligation. The checker verifies the request decodes, the
    precondition holds in the authored inputs, and the code is registered.
    """
    try:
        spec = case["request"]
        root_hex = spec["root_hex"] if spec["root_hex"] != "context" else inputs["context"]["root"]
        entity_hex = inputs["entities"][spec["entity_ref"]]["id"] if "entity_ref" in spec else spec["entity_hex"]
        request_body = build_request_body(
            bytes.fromhex(root_hex),
            bytes.fromhex(entity_hex),
            int(spec["max_objects"]),
            int(spec["ceiling_m"]),
            int(spec["max_work"]),
        )
        request = decode_request_body(request_body, {"limits": inputs["selected_limits"]})
    except (ScbError, CheckFailed, ValueError, KeyError) as error:
        problems.append(f"{case['id']}:request-decode:{error}")
        return
    context = inputs["context"]
    members = {inputs["entities"][ref]["id"] for ref in inputs["bindings"]["members"]}
    seam = case["seam"]
    if seam == "wrong_root":
        if request["root"] == bytes.fromhex(context["root"]):
            problems.append(f"{case['id']}:seam-not-wrong-root")
    elif seam == "absent_entity":
        if request["entity"].hex() in members:
            problems.append(f"{case['id']}:seam-entity-present")
    elif seam == "wrong_kind":
        kinds = {inputs["entities"][ref]["kind"] for ref in inputs["bindings"]["members"] if inputs["entities"][ref]["id"] == request["entity"].hex()}
        if 5 in kinds:
            problems.append(f"{case['id']}:seam-target-is-function")
    elif seam == "missing_parameter":
        if case.get("production_impossible_note") is None:
            problems.append(f"{case['id']}:seam-note-missing")
    else:
        problems.append(f"{case['id']}:unknown-seam")
    if case.get("expected_code") not in CODES.values():
        problems.append(f"{case['id']}:unknown-code")
    if CODES.get(case.get("expected_symbol", "")) != case.get("expected_code"):
        problems.append(f"{case['id']}:registry")


def derive_relation(inputs: Mapping[str, Any], case: dict[str, Any]) -> dict[str, Any]:
    """Fill concrete derived numbers for a boundary relation from its base case."""
    lookup_l = derived_lookup_l(inputs)
    relation = case["relation"]
    if relation == "checked_overflow":
        return case
    base = build_success_case(inputs, inputs["cases"][case["base"]])
    body_len = len(bytes.fromhex(base["response_body_hex"]))
    if relation == "k_exact":
        case["max_objects"] = base["count_k"]
    elif relation == "k_one_below":
        case["max_objects"] = base["count_k"] - 1
    elif relation == "bytes_exact":
        case["ceiling_m"] = body_len
    elif relation == "bytes_one_below":
        case["ceiling_m"] = body_len - 1
    elif relation == "wire_exact":
        case["wire_len"] = base["response_wire_len"]
    elif relation == "wire_one_below":
        case["wire_len"] = base["response_wire_len"] - 1
    elif relation == "work_exact":
        case["ceiling_m"] = body_len
        case["claimed_work"] = compute_work(base["count_k"], lookup_l, base["stored_b"], body_len)
    elif relation == "work_one_below":
        case["ceiling_m"] = body_len
        case["claimed_work"] = compute_work(base["count_k"], lookup_l, base["stored_b"], body_len) - 1
    elif relation == "work_recompute_m":
        case["ceiling_m"] = int(inputs["cases"][case["base"]]["request"]["max_response_bytes"])
        case["ceiling_m_next"] = case["ceiling_m"] + 1
        case["work_first"] = compute_work(base["count_k"], lookup_l, base["stored_b"], case["ceiling_m"])
        case["work_next"] = compute_work(base["count_k"], lookup_l, base["stored_b"], case["ceiling_m_next"])
    elif relation == "uvar_threshold":
        case["ceiling_m_small"] = 16383
        case["ceiling_m_large"] = 16384
        case["work_small"] = compute_work(base["count_k"], lookup_l, base["stored_b"], 16383)
        case["work_large"] = compute_work(base["count_k"], lookup_l, base["stored_b"], 16384)
    else:
        raise ValueError(f"unknown relation: {relation}")
    return case


def check_stateful_spec(case: Mapping[str, Any], problems: list[str]) -> None:
    if case.get("status") != "pending_runtime_comparison":
        problems.append(f"{case['id']}:stateful-must-be-pending")
    for step in case.get("steps", []):
        symbol = step.get("expected_symbol")
        if symbol is not None and symbol not in CODES:
            problems.append(f"{case['id']}:unknown-symbol:{symbol}")
    for code in case.get("expected_codes", []):
        if code not in CODES.values():
            problems.append(f"{case['id']}:unknown-code:{code}")


def check_relation(inputs: Mapping[str, Any], case: Mapping[str, Any], problems: list[str]) -> None:
    try:
        lookup_l = derived_lookup_l(inputs)
        if case["relation"] == "k_exact":
            base = build_success_case(inputs, inputs["cases"][case["base"]])
            if base["count_k"] != case["max_objects"]:
                problems.append(f"{case['id']}:k-relation")
        elif case["relation"] == "k_one_below":
            base = build_success_case(inputs, inputs["cases"][case["base"]])
            if not (case["max_objects"] == base["count_k"] - 1 and base["count_k"] >= 1):
                problems.append(f"{case['id']}:k-relation")
        elif case["relation"] == "bytes_exact":
            base = build_success_case(inputs, inputs["cases"][case["base"]])
            if case["ceiling_m"] != len(bytes.fromhex(base["response_body_hex"])):
                problems.append(f"{case['id']}:bytes-relation")
        elif case["relation"] == "bytes_one_below":
            base = build_success_case(inputs, inputs["cases"][case["base"]])
            if case["ceiling_m"] != len(bytes.fromhex(base["response_body_hex"])) - 1:
                problems.append(f"{case['id']}:bytes-relation")
        elif case["relation"] == "wire_exact":
            base = build_success_case(inputs, inputs["cases"][case["base"]])
            if case["wire_len"] != base["response_wire_len"]:
                problems.append(f"{case['id']}:wire-relation")
        elif case["relation"] == "wire_one_below":
            base = build_success_case(inputs, inputs["cases"][case["base"]])
            if case["wire_len"] != base["response_wire_len"] - 1:
                problems.append(f"{case['id']}:wire-relation")
        elif case["relation"] == "work_exact":
            base = build_success_case(inputs, inputs["cases"][case["base"]])
            expected = compute_work(base["count_k"], lookup_l, base["stored_b"], case["ceiling_m"])
            if case["claimed_work"] != expected:
                problems.append(f"{case['id']}:work-relation")
        elif case["relation"] == "work_one_below":
            base = build_success_case(inputs, inputs["cases"][case["base"]])
            expected = compute_work(base["count_k"], lookup_l, base["stored_b"], case["ceiling_m"])
            if case["claimed_work"] != expected - 1:
                problems.append(f"{case['id']}:work-relation")
        elif case["relation"] == "work_recompute_m":
            base = build_success_case(inputs, inputs["cases"][case["base"]])
            first = compute_work(base["count_k"], lookup_l, base["stored_b"], case["ceiling_m"])
            second = compute_work(base["count_k"], lookup_l, base["stored_b"], case["ceiling_m_next"])
            if case["work_first"] != first or case["work_next"] != second or second == first:
                problems.append(f"{case['id']}:work-relation")
        elif case["relation"] == "uvar_threshold":
            base = build_success_case(inputs, inputs["cases"][case["base"]])
            first = compute_work(base["count_k"], lookup_l, base["stored_b"], case["ceiling_m_small"])
            second = compute_work(base["count_k"], lookup_l, base["stored_b"], case["ceiling_m_large"])
            if case["work_small"] != first or case["work_large"] != second:
                problems.append(f"{case['id']}:work-relation")
            if len(encode_uvar(case["ceiling_m_small"])) == len(encode_uvar(case["ceiling_m_large"])):
                problems.append(f"{case['id']}:threshold-not-crossed")
        elif case["relation"] == "checked_overflow":
            try:
                compute_work(case["arith_k"], case["arith_l"], case["arith_b"], case["arith_m"])
            except Overflow:
                return
            problems.append(f"{case['id']}:overflow-expected")
        else:
            problems.append(f"{case['id']}:unknown-relation")
    except (ScbError, CheckFailed, ValueError, KeyError) as error:
        problems.append(f"{case['id']}:relation-error:{error}")
    if case.get("expected_code") is not None and case["expected_code"] not in CODES.values():
        problems.append(f"{case['id']}:unknown-code")


def check_fill_recipe(inputs: Mapping[str, Any], case: Mapping[str, Any], problems: list[str]) -> None:
    try:
        payload = bytes.fromhex(case["fill_byte"]) * int(case["fill_length"])
        if len(payload) != int(case["fill_length"]):
            problems.append(f"{case['id']}:fill-length")
            return
        if case.get("expect_constructible"):
            encoded = encode_sized(payload)
            if len(encoded) != int(case["fill_length"]) + len(encode_uvar(int(case["fill_length"]))):
                problems.append(f"{case['id']}:fill-framing")
        if case.get("expect_too_large"):
            if int(case["fill_length"]) <= MAX_BYTE_PAYLOAD:
                problems.append(f"{case['id']}:fill-ceiling")
        aggregate = case.get("aggregate")
        if aggregate is not None:
            if aggregate["objects"] * aggregate["object_bytes"] > aggregate["ceiling_m"]:
                problems.append(f"{case['id']}:aggregate-exceeds-m")
            if aggregate["objects"] > aggregate["max_objects"]:
                problems.append(f"{case['id']}:aggregate-exceeds-k")
            if int(case["fill_length"]) <= MAX_BYTE_PAYLOAD:
                problems.append(f"{case['id']}:aggregate-not-beyond-cap")
    except ValueError as error:
        problems.append(f"{case['id']}:fill-error:{error}")



# Explicit Python-only refresh (root-owned staging outside the pin)



def git_head_revision(repo_root: Path) -> str:
    """Read the current HEAD revision without spawning any process.

    The oracle records provenance metadata during an explicit refresh, and
    S20-130 forbids it any implementation dependency, including a git
    executable. This resolves the git directory (including worktree pointer
    files), then the HEAD ref through the loose ref file or packed-refs,
    reporting exactly what `git rev-parse HEAD` would. It raises when the
    revision is unresolvable, mirroring the old `check=True` failure.
    """
    git_path = repo_root / ".git"
    if git_path.is_file():
        pointer = git_path.read_text(encoding="utf-8").strip()
        if not pointer.startswith("gitdir:"):
            raise ValueError(f"unrecognized .git pointer: {pointer!r}")
        git_dir = Path(pointer[len("gitdir:"):].strip())
        if not git_dir.is_absolute():
            git_dir = (git_path.parent / git_dir).resolve()
    else:
        git_dir = git_path
    head = (git_dir / "HEAD").read_text(encoding="utf-8").strip()
    if head.startswith("ref:"):
        ref = head[len("ref:"):].strip()
        loose = git_dir / ref
        if loose.is_file():
            head = loose.read_text(encoding="utf-8").strip()
        else:
            packed = git_dir / "packed-refs"
            resolved = None
            for line in packed.read_text(encoding="utf-8").splitlines():
                if not line or line.startswith("#") or line.startswith("^"):
                    continue
                sha, _, name = line.partition(" ")
                if name.strip() == ref:
                    resolved = sha.strip()
                    break
            if resolved is None:
                raise ValueError(f"HEAD ref {ref!r} resolves to no revision")
            head = resolved
    if len(head) != 40 or any(char not in "0123456789abcdefABCDEF" for char in head):
        raise ValueError(f"HEAD revision is not a commit id: {head!r}")
    return head


def refresh(inputs_path: Path, output_dir: Path, repo_root: Path) -> dict[str, str]:
    """Derive accepted.json, rejected.json, SHA256SUMS from semantic inputs.

    Writes ONLY into output_dir, which must not resolve inside the
    repository's immutable generated corpus directory. Never invokes Rust,
    never reads emitted results. Records input / schema / encoder hashes
    and the source revision; no self-hash, no commit cycle.
    """
    inputs = json.loads(inputs_path.read_text(encoding="utf-8"))
    if not _authored_frame_matrix_is_canonical(inputs):
        raise ValueError("refresh refuses incomplete authored matrix: accepted:frame_scenarios:inventory")
    resolved_out = output_dir.resolve()
    generated_dir = (repo_root / "conformance" / "entity-read").resolve()
    if resolved_out == generated_dir or generated_dir in resolved_out.parents:
        raise ValueError("refresh target must stay outside the immutable corpus pin")
    if SOURCE_SCHEMA_BLAKE3 != "1983bc8d6ad9ac3cb5390853f43959cf2c3dc0ae8e0ca18ca8264ca4960133ae":
        raise ValueError("mutation-value schema digest drift")
    schema_path = repo_root / "docs" / "spec" / "SSMC1_EPOCH1_SCHEMA.txt"
    encoder_path = Path(__file__).resolve()
    inputs_sha = hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest()
    schema_sha = hashlib.sha256(schema_path.read_bytes()).hexdigest()
    encoder_sha = hashlib.sha256(encoder_path.read_bytes()).hexdigest()
    head = git_head_revision(repo_root)
    protocol_epoch = protocol_epoch_id().hex()

    accepted_cases: dict[str, Any] = {}
    for case_id, case in inputs["cases"].items():
        if case.get("kind", "success") != "success":
            continue
        accepted_cases[case_id] = {"id": case_id, **build_success_case(inputs, case)}
    hellos = {name: {"body_hex": build_hello(hello).hex()} for name, hello in inputs["hellos"].items()}
    selections: dict[str, Any] = {}
    for name, scenario in inputs["selection_scenarios"].items():
        if scenario.get("expect") == "fail":
            try:
                negotiate_versioned(inputs["hellos"][scenario["client"]], inputs["hellos"][scenario["server"]])
            except CheckFailed:
                selections[name] = {"expected_failure": "selection"}
            else:
                raise ValueError(f"selection scenario {name} was accepted")
            continue
        selection = negotiate_versioned(inputs["hellos"][scenario["client"]], inputs["hellos"][scenario["server"]])
        preimage = build_selected(selection)
        client_body = bytes.fromhex(hellos[scenario["client"]]["body_hex"])
        server_body = bytes.fromhex(hellos[scenario["server"]]["body_hex"])
        transcript = client_body + server_body + preimage
        selections[name] = {
            **selection,
            "preimage_hex": preimage.hex(),
            "transcript_hex": transcript.hex(),
            "handshake_id": handshake_id(client_body, server_body, preimage).hex(),
        }
    frame_scenarios: dict[str, Any] = {}
    for row_id, authored_row in inputs["frame_scenarios"].items():
        reconstructed = _reconstruct_hello_row(inputs, authored_row)
        if reconstructed is None:
            raise ValueError(f"refresh cannot reconstruct required frame scenario: {row_id}")
        body, wire, preimage, frame_id = reconstructed
        frame_scenarios[row_id] = {
            "hello": authored_row["hello"],
            "wire_version": authored_row["wire_version"],
            "expected_version": authored_row["expected_version"],
            "expect": authored_row["expect"],
            "expected_layer": authored_row["expected_layer"],
            "expected_code": authored_row["expected_code"],
            "body_hex": body.hex(),
            "wire_hex": wire.hex(),
            "preimage_hex": preimage.hex(),
            "frame_id": frame_id.hex(),
        }
    accepted = {
        "contract": "sley2-entity-read-v2",
        "claim": "independent-expected",
        "manifest": {
            "inputs_sha256": inputs_sha,
            "schema_sha256": schema_sha,
            "encoder_sha256": encoder_sha,
            "encoder_module": "sley2_scb1_oracle.entity_read",
            "source_schema_blake3": SOURCE_SCHEMA_BLAKE3,
            "protocol_epoch": protocol_epoch,
            "inputs_authored_at_revision": inputs["provenance"]["authored_at_revision"],
            "refresh_head_revision": head,
        },
        "cases": accepted_cases,
        "hellos": hellos,
        "selections": selections,
        "frame_scenarios": frame_scenarios,
    }
    rejected_cases: list[dict[str, Any]] = []
    protocol_epoch_check = protocol_epoch_id().hex()
    assert protocol_epoch_check == protocol_epoch
    for case in inputs["rejected"]:
        if case.get("kind") in ("runtime_sequence", "fill_recipe", "hello_invalid", "selection_invalid", "owner_case"):
            rejected_cases.append(dict(case))
            continue
        if case.get("kind") == "relation":
            rejected_cases.append(derive_relation(inputs, dict(case)))
            continue
        if case.get("kind") in ("failure_response", "failure_wire"):
            data = build_failure_wire(inputs, case).hex()
            rejected_cases.append({**case, "input_hex": data})
            continue
        data = build_rejected_bytes(inputs, inputs["cases"][case["base"]], case["recipe"]).hex()
        rejected_cases.append({**case, "input_hex": data})
    rejected = {
        "contract": "sley2-entity-read-v2-rejected",
        "claim": "independent-expected",
        "manifest": {
            "inputs_sha256": inputs_sha,
            "protocol_epoch": protocol_epoch,
            "refresh_head_revision": head,
        },
        "cases": rejected_cases,
    }
    resolved_out.mkdir(parents=True, exist_ok=True)
    accepted_path = resolved_out / "accepted.json"
    rejected_path = resolved_out / "rejected.json"
    accepted_path.write_text(json.dumps(accepted, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    rejected_path.write_text(json.dumps(rejected, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    sums = (
        f"{hashlib.sha256(accepted_path.read_bytes()).hexdigest()}  accepted.json\n"
        f"{hashlib.sha256(rejected_path.read_bytes()).hexdigest()}  rejected.json\n"
    )
    (resolved_out / "SHA256SUMS").write_text(sums, encoding="utf-8")
    failures = check_accepted(inputs, accepted) + check_rejected(inputs, rejected)
    if failures:
        raise ValueError(f"refresh self-check failed: {failures}")
    return {"accepted": str(accepted_path), "rejected": str(rejected_path), "sums": str(resolved_out / "SHA256SUMS")}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Independent entity-read vector oracle")
    parser.add_argument("--inputs", default="conformance/entity-read/v2/inputs.json")
    parser.add_argument("--accepted", default="conformance/entity-read/v2/accepted.json")
    parser.add_argument("--rejected", default="conformance/entity-read/v2/rejected.json")
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--output-dir", default=None)
    args = parser.parse_args(argv)
    repo_root = Path(__file__).resolve().parents[4]
    if args.refresh:
        if not args.output_dir:
            parser.error("refresh requires an explicit --output-dir outside the corpus pin")
        result = refresh(Path(args.inputs), Path(args.output_dir), repo_root)
        print(json.dumps({"result": "REFRESHED", **result}, indent=2, sort_keys=True))
        return 0
    inputs = json.loads(Path(args.inputs).read_text(encoding="utf-8"))
    accepted = json.loads(Path(args.accepted).read_text(encoding="utf-8"))
    rejected = json.loads(Path(args.rejected).read_text(encoding="utf-8"))
    problems = check_accepted(inputs, accepted) + check_rejected(inputs, rejected)
    print(json.dumps({"contract": "s20-310-entity-read-oracle-v2", "problems": problems, "result": "PASS" if not problems else "FAIL"}, indent=2, sort_keys=True))
    return 0 if not problems else 1
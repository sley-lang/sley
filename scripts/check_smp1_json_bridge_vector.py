#!/usr/bin/env python3
"""Independently reproduce the S20-420 SMP1 JSON bridge vectors.

The oracle reads every fixture frame with its own SMP1 record reader,
renders the `Frame` object with the contract's declared encodings (lowercase
hex, numbers up to 2^53 - 1 and decimal strings above, frozen names, exact
shapes, lexicographic emission without insignificant whitespace), and
compares the text with the crate's. It then parses the fixture text with its
own bridge reader, re-encodes the frame with the SMP1 oracle's encoders, and
requires the identical bytes. Every rejected text is classified in contract
precedence: resource, then parse and shape, then field encodings in field
order, then the codec's `PROTOCOL_*` codes. It shares no code with the Rust
implementation; the SMP1 oracle's encoders are its only import.
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import check_smp1_vector as smp1


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "conformance/smp1-json-bridge/v1/roundtrip.json"
REJECTED = ROOT / "conformance/smp1-json-bridge/v1/rejected.json"
TABLE = ROOT / "conformance/smp1-json-bridge/v1/methods.json"
SMP1_FIXTURE = ROOT / "conformance/smp1/v1/accepted.json"

MAX_TEXT_BYTES = 268_435_456  # 4 * 67_108_864: any frame that fits on the wire fits in text
MAX_DEPTH = 32
MAX_ELEMENTS = 1_048_576  # 2**20 value positions; open-ended bridge lists are protocol-bounded in the dozens
MAX_NUMBER = 2**53 - 1
MAX_HELLO_LIST = 4_096
KIND_NAMES = {1: "request", 2: "response", 3: "event", 4: "hello"}
KIND_TAGS = {name: tag for tag, name in KIND_NAMES.items()}
LIMIT_FIELDS = ("max_frame_bytes", "max_entities", "max_edges", "max_depth", "max_response_bytes", "max_work", "max_inflight", "max_sessions")
LIMIT_CEILINGS = (67_108_864, 65_535, 400_000, 65_535, 67_108_864, 100_000_000, 1_024, 256)
U32_FIELDS = {"protocol_version", "max_depth", "max_inflight", "max_sessions", "reached_depth"}
BOUNDS_FIELDS = ("applied_limits", "returned_bytes", "returned_entities", "returned_edges", "reached_depth", "omitted", "truncated", "continuation")
FRAME_FIELDS = ("protocol_version", "session", "request_id", "kind", "method", "flags", "bounds", "body")
FLAG_FIELDS = ("cancel", "stream", "failed")


class Reject(Exception):
    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def shape() -> Reject:
    return Reject("JSON_BRIDGE_SHAPE_INVALID")


# --- SCB1 readers -----------------------------------------------------------


def read_sized(data: bytes, offset: int) -> tuple[bytes, int]:
    length, offset = smp1.read_uvar(data, offset)
    if offset + length > len(data):
        raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    return data[offset : offset + length], offset + length


def read_record(data: bytes, expected: int) -> list[bytes]:
    count, offset = smp1.read_uvar(data, 0)
    if count != expected:
        raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    fields = []
    for index in range(count):
        tag, offset = smp1.read_uvar(data, offset)
        if tag != index + 1:
            raise smp1.Failure("PROTOCOL_FRAME_INVALID")
        field, offset = read_sized(data, offset)
        fields.append(field)
    if offset != len(data):
        raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    return fields


def read_list(data: bytes) -> list[bytes]:
    count, offset = smp1.read_uvar(data, 0)
    items = []
    for _ in range(count):
        item, offset = read_sized(data, offset)
        items.append(item)
    if offset != len(data):
        raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    return items


def read_union(data: bytes) -> tuple[int, bytes]:
    tag, offset = smp1.read_uvar(data, 0)
    payload, offset = read_sized(data, offset)
    if offset != len(data):
        raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    return tag, payload


def read_only_uvar(data: bytes) -> int:
    value, offset = smp1.read_uvar(data, 0)
    if offset != len(data):
        raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    return value


def read_only_bytes(data: bytes) -> bytes:
    payload, offset = read_sized(data, 0)
    if offset != len(data):
        raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    return payload


def read_flag(data: bytes) -> bool:
    value = read_only_uvar(data)
    if value not in (1, 2):
        raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    return value == 2


def decode_frame(data: bytes, epoch: bytes) -> dict:
    smp1.inspect(data, epoch)
    preimage = data[8:-32]
    offset = len(smp1.MAGIC)
    _, offset = smp1.read_uvar(preimage, offset)
    _, offset = smp1.read_uvar(preimage, offset)
    offset += 32
    payload, offset = read_sized(preimage, offset)
    fields = read_record(payload, 8)
    session_tag, session_payload = read_union(fields[1])
    if (session_tag, len(session_payload)) == (0, 0):
        session = None
    elif (session_tag, len(session_payload)) == (1, 32):
        session = session_payload
    else:
        raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    bounds = read_record(fields[6], 8)
    limits = [read_only_uvar(item) for item in read_record(bounds[0], 8)]
    return {
        "protocol_version": read_only_uvar(fields[0]),
        "session": session,
        "request_id": read_only_uvar(fields[2]),
        "kind": read_only_uvar(fields[3]),
        "method": read_only_uvar(fields[4]),
        "flags": read_only_uvar(fields[5]),
        "limits": tuple(limits),
        "returned": (
            read_only_uvar(bounds[1]),
            read_only_uvar(bounds[2]),
            read_only_uvar(bounds[3]),
            read_only_uvar(bounds[4]),
            read_only_uvar(bounds[5]),
            read_flag(bounds[6]),
            read_flag(bounds[7]),
        ),
        "body": read_only_bytes(fields[7]),
    }


# --- Rendering ------------------------------------------------------------------


def integer(value: int) -> int | str:
    return value if value <= MAX_NUMBER else str(value)


def render(frame: dict, names: dict[int, str]) -> str:
    if frame["method"] == 0:
        method = ""
    elif frame["method"] in names:
        method = names[frame["method"]]
    else:
        raise Reject("JSON_BRIDGE_METHOD_UNKNOWN")
    if frame["flags"] & ~7:
        raise shape()
    returned_bytes, entities, edges, depth, omitted, truncated, continuation = frame["returned"]
    obj = {
        "protocol_version": integer(frame["protocol_version"]),
        "session": None if frame["session"] is None else frame["session"].hex(),
        "request_id": integer(frame["request_id"]),
        "kind": KIND_NAMES[frame["kind"]],
        "method": method,
        "flags": {"cancel": bool(frame["flags"] & 1), "stream": bool(frame["flags"] & 2), "failed": bool(frame["flags"] & 4)},
        "bounds": {
            "applied_limits": {name: integer(value) for name, value in zip(LIMIT_FIELDS, frame["limits"])},
            "returned_bytes": integer(returned_bytes),
            "returned_entities": integer(entities),
            "returned_edges": integer(edges),
            "reached_depth": integer(depth),
            "omitted": integer(omitted),
            "truncated": truncated,
            "continuation": continuation,
        },
        "body": frame["body"].hex(),
    }
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


# --- Bridge reader --------------------------------------------------------------


def check_resources(text: str) -> None:
    if len(text.encode("utf-8")) > MAX_TEXT_BYTES:
        raise Reject("JSON_BRIDGE_RESOURCE_LIMIT")
    depth, positions, in_string, escaped = 0, 0, False, False
    for char in text:
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            continue
        if char == '"':
            in_string = True
        elif char in "{[":
            depth += 1
            if depth > MAX_DEPTH:
                raise Reject("JSON_BRIDGE_RESOURCE_LIMIT")
            positions += 1
            if positions >= MAX_ELEMENTS:
                raise Reject("JSON_BRIDGE_RESOURCE_LIMIT")
        elif char in ",:":
            positions += 1
            if positions >= MAX_ELEMENTS:
                raise Reject("JSON_BRIDGE_RESOURCE_LIMIT")
        elif char in "}]":
            depth = max(depth - 1, 0)


def parse(text: str) -> object:
    check_resources(text)

    def constant(_: str) -> None:
        raise shape()

    try:
        return json.loads(text, parse_constant=constant)
    except (ValueError, RecursionError) as error:
        raise shape() from error


def obj(value: object, fields: tuple[str, ...], nullable: tuple[str, ...] = ()) -> dict:
    if not isinstance(value, dict) or set(value) != set(fields):
        raise shape()
    for field in fields:
        if value[field] is None and field not in nullable:
            raise shape()
    return value


def integer_field(value: object, name: str) -> int:
    if isinstance(value, bool):
        raise shape()
    if isinstance(value, int):
        if value < 0 or value > MAX_NUMBER:
            raise Reject("JSON_BRIDGE_NUMBER_INVALID")
        result = value
    elif isinstance(value, float):
        # Both `-0` (parsed as int above) and `-0.0` parse as negative zero,
        # which the reader normalizes to 0 (contract section 8); every other
        # float form stays invalid.
        if value == 0.0 and math.copysign(1.0, value) < 0:
            result = 0
        else:
            raise Reject("JSON_BRIDGE_NUMBER_INVALID")
    elif isinstance(value, str):
        if not value or not all(ch in "0123456789" for ch in value) or (len(value) > 1 and value[0] == "0"):
            raise Reject("JSON_BRIDGE_NUMBER_INVALID")
        result = int(value)
        if result >= 2**64:
            raise Reject("JSON_BRIDGE_NUMBER_INVALID")
    else:
        raise shape()
    if name in U32_FIELDS and result >= 2**32:
        raise Reject("JSON_BRIDGE_NUMBER_INVALID")
    return result


def bool_field(value: object) -> bool:
    if not isinstance(value, bool):
        raise shape()
    return value


def hex_field(value: object, fixed: int | None = None) -> bytes:
    if not isinstance(value, str):
        raise shape()
    if len(value) % 2 or any(ch not in "0123456789abcdef" for ch in value):
        raise Reject("JSON_BRIDGE_HEX_INVALID")
    payload = bytes.fromhex(value)
    if fixed is not None and len(payload) != fixed:
        raise shape()
    return payload


def limits_from(value: object) -> tuple[int, ...]:
    fields = obj(value, LIMIT_FIELDS)
    return tuple(integer_field(fields[name], name) for name in LIMIT_FIELDS)


def frame_from_json(text: str, names: dict[str, int]) -> dict:
    fields = obj(parse(text), FRAME_FIELDS, nullable=("session",))
    protocol_version = integer_field(fields["protocol_version"], "protocol_version")
    session = None if fields["session"] is None else hex_field(fields["session"], 32)
    request_id = integer_field(fields["request_id"], "request_id")
    if not isinstance(fields["kind"], str) or fields["kind"] not in KIND_TAGS:
        raise shape()
    kind = KIND_TAGS[fields["kind"]]
    if not isinstance(fields["method"], str):
        raise shape()
    if fields["method"] == "":
        method = 0
    elif fields["method"] in names:
        method = names[fields["method"]]
    else:
        raise Reject("JSON_BRIDGE_METHOD_UNKNOWN")
    flag_fields = obj(fields["flags"], FLAG_FIELDS)
    flags = (
        (1 if bool_field(flag_fields["cancel"]) else 0)
        | (2 if bool_field(flag_fields["stream"]) else 0)
        | (4 if bool_field(flag_fields["failed"]) else 0)
    )
    bounds = obj(fields["bounds"], BOUNDS_FIELDS)
    limits = limits_from(bounds["applied_limits"])
    returned = (
        integer_field(bounds["returned_bytes"], "returned_bytes"),
        integer_field(bounds["returned_entities"], "returned_entities"),
        integer_field(bounds["returned_edges"], "returned_edges"),
        integer_field(bounds["reached_depth"], "reached_depth"),
        integer_field(bounds["omitted"], "omitted"),
        bool_field(bounds["truncated"]),
        bool_field(bounds["continuation"]),
    )
    body = hex_field(fields["body"])
    if kind == 4:
        # The all-zero bounds are the bridge's own rule; the session, request
        # id, method, and flags are the codec's hello header rule (SMP1
        # section 2), so a violation keeps PROTOCOL_FRAME_INVALID. The codec
        # judges the protocol version first, as a version claim (bridge
        # contract section 8), so the version split runs before the header
        # rule, exactly as the crate orders them.
        if any(limits) or any(returned):
            raise shape()
        if protocol_version < 1:
            raise smp1.Failure("PROTOCOL_DOWNGRADE")
        if protocol_version > 1:
            raise smp1.Failure("PROTOCOL_VERSION_UNSUPPORTED")
        if session is not None or request_id != 0 or method != 0 or flags != 0:
            raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    return {
        "protocol_version": protocol_version,
        "session": session,
        "request_id": request_id,
        "kind": kind,
        "method": method,
        "flags": flags,
        "limits": limits,
        "returned": returned,
        "body": body,
    }


def strictly_increasing(items: list) -> bool:
    return all(a < b for a, b in zip(items, items[1:]))


def decode_hello(body: bytes) -> dict:
    """The codec's hello rules, restated for the `PROTOCOL_*` precedence."""
    try:
        fields = read_record(body, 7)
        versions = [read_only_uvar(item) for item in read_list(fields[0])]
        epochs = read_list(fields[1])
        limits = tuple(read_only_uvar(item) for item in read_record(fields[2], 8))
        methods = [read_only_uvar(item) for item in read_list(fields[3])]
        features = read_only_uvar(fields[4])
        adapters = read_list(fields[5])
        effects = read_list(fields[6])
    except smp1.Failure as error:
        raise smp1.Failure("PROTOCOL_PAYLOAD_INVALID") from error
    if (
        not versions
        or len(versions) > MAX_HELLO_LIST
        or not strictly_increasing(versions)
        or any(v >= 2**32 for v in versions)
        or not epochs
        or len(epochs) > MAX_HELLO_LIST
        or any(len(e) != 32 for e in epochs)
        or not methods
        or len(methods) > MAX_HELLO_LIST
        or not strictly_increasing(methods)
        or any(m >= 2**32 for m in methods)
        or len(adapters) > MAX_HELLO_LIST
        or not strictly_increasing(adapters)
        or any(len(a) != 32 for a in adapters)
        or len(effects) > MAX_HELLO_LIST
        or not strictly_increasing(effects)
        or any(len(e) != 32 for e in effects)
        or features >= 2**32
        or features & ~15
    ):
        raise smp1.Failure("PROTOCOL_PAYLOAD_INVALID")
    if any(value == 0 for index, value in enumerate(limits) if index != 3) or any(
        value > ceiling for value, ceiling in zip(limits, LIMIT_CEILINGS)
    ):
        raise smp1.Failure("PROTOCOL_LIMIT_EXCEEDED")
    return {
        "protocol_versions": versions,
        "schema_epochs": epochs,
        "limits": limits,
        "methods": methods,
        "features": features,
        "adapters": adapters,
        "effects": effects,
    }


def encode_frame(frame: dict, epoch: bytes) -> bytes:
    # The version split of SMP1 revision 11 (section 2): a claim below the
    # selected version is a downgrade attempt, a claim above it names a
    # version the selection does not know. The re-emitted rejected vectors
    # carry the split; the checker's own encoder must agree with it.
    if frame["protocol_version"] < 1:
        raise smp1.Failure("PROTOCOL_DOWNGRADE")
    if frame["protocol_version"] > 1:
        raise smp1.Failure("PROTOCOL_VERSION_UNSUPPORTED")
    if frame["flags"] & 4 and frame["kind"] not in (2, 3):
        raise smp1.Failure("PROTOCOL_FRAME_INVALID")
    if frame["kind"] == 4:
        hello = decode_hello(frame["body"])
        body = smp1.hello(hello)
    else:
        body = frame["body"]
    payload = smp1.frame_payload(
        frame["session"],
        frame["request_id"],
        frame["kind"],
        frame["method"],
        frame["flags"],
        smp1.bounds(frame["limits"], frame["returned"]),
        body,
    )
    return smp1.envelope(epoch, payload)[0]


def classify(text: str, names: dict[str, int], epoch: bytes) -> str | None:
    try:
        encode_frame(frame_from_json(text, names), epoch)
    except Reject as error:
        return error.code
    except smp1.Failure as error:
        return error.code
    return None


# --- Main ----------------------------------------------------------------------


def main() -> int:
    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    table = json.loads(TABLE.read_text(encoding="utf-8"))
    smp1_fixture = json.loads(SMP1_FIXTURE.read_text(encoding="utf-8"))
    if fixture["contract"] != "sley2-smp1-json-bridge-v1" or rejected["contract"] != fixture["contract"]:
        raise SystemExit("bridge fixture contract drifted")
    epoch = bytes.fromhex(smp1_fixture["protocol_epoch_hex"])

    rows = table["methods"]
    names_by_tag = {row["tag"]: row["name"] for row in rows}
    tags_by_name = {row["name"]: row["tag"] for row in rows}
    if len(rows) != table["method_count"] or len(names_by_tag) != len(rows) or len(tags_by_name) != len(rows):
        raise SystemExit("method table is not a bijection")
    if [row["tag"] for row in rows] != sorted(names_by_tag) or sorted(names_by_tag) != smp1.ALL_METHODS:
        # Reserved tags ride the table as documentation (reserved: true)
        # but never negotiate: only the live rows meet the oracle list.
        live = sorted(row["tag"] for row in rows if not row["reserved"])
        if [row["tag"] for row in rows] != sorted(names_by_tag) or live != smp1.ALL_METHODS:
            raise SystemExit("method table tags drifted from the SMP1 oracle")

    problems: list[str] = []
    for vector in fixture["vectors"]:
        frame_bytes = bytes.fromhex(vector["frame_hex"])
        rendered = render(decode_frame(frame_bytes, epoch), names_by_tag)
        if rendered != vector["json"]:
            problems.append(f"render-drift:{vector['id']}")
        parsed = frame_from_json(vector["json"], tags_by_name)
        if encode_frame(parsed, epoch) != frame_bytes:
            problems.append(f"round-trip-drift:{vector['id']}")
        spaced = json.dumps(json.loads(vector["json"]), indent=1, sort_keys=False)
        if encode_frame(frame_from_json(spaced, tags_by_name), epoch) != frame_bytes:
            problems.append(f"whitespace-tolerance:{vector['id']}")
        if vector["id"].startswith("hello") and (parsed["kind"] != 4 or parsed["method"] != 0):
            problems.append(f"hello-shape:{vector['id']}")
    for mutation in rejected["mutations"]:
        observed = classify(mutation["json"], tags_by_name, epoch)
        if observed != mutation["expected_code"]:
            problems.append(f"rejection:{mutation['id']}:expected={mutation['expected_code']}:observed={observed}")

    result = {
        "contract": "sley2-smp1-json-bridge-v1",
        "vectors": len(fixture["vectors"]),
        "rejections": len(rejected["mutations"]),
        "methods": len(rows),
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

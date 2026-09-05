#!/usr/bin/env python3
"""Independently reproduce the S20-410 SMP1 frame, hello, and handshake vectors.

The oracle rebuilds every frame of `docs/spec/SMP1.md` from the fixture's
frozen inputs (the client and server hellos and the request, response, and
failure frames of the conformance corpus are the contract's fixed test
constants, restated here) with its own SCB1 encoders: the `ProtocolFrame`
and `Hello` records, the standalone envelope under the frozen protocol
schema epoch identity carried by the fixture, the `ProtocolFrameId` trailer
under `sley2.protocol-frame.v1`, the derived selected profile, and the
transcript-bound `ProtocolHandshakeId` under `sley2.protocol-handshake.v1`
(client hello body, server hello body, then selection preimage). It then
classifies every rejected input with its own bounded decoder. It shares no
code with the Rust implementation.
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

import blake3


ROOT = Path(__file__).resolve().parents[1]
ACCEPTED = ROOT / "conformance/smp1/v1/accepted.json"
REJECTED = ROOT / "conformance/smp1/v1/rejected.json"

FRAME_DOMAIN = b"sley2.protocol-frame.v1"
HANDSHAKE_DOMAIN = b"sley2.protocol-handshake.v1"
MAGIC = b"SLEYSCB1"
CONTRACT_TAG = 400
MAX_FRAME_BYTES = 67_108_864
ALL_METHODS = (
    [100, 101, 102, 103, 104]
    + list(range(200, 215))
    + [300, 301, 302, 303, 304, 305]
    + [400, 401, 402, 403, 404]
    + [500, 501, 502, 503, 504]
    + [600, 601, 602, 603, 604]
)
CODES = {
    "PROTOCOL_VERSION_UNSUPPORTED": 40000,
    "PROTOCOL_FRAME_INVALID": 40001,
    "PROTOCOL_FRAME_TOO_LARGE": 40002,
    "PROTOCOL_PAYLOAD_INVALID": 40008,
}

CLIENT_HELLO = {
    "protocol_versions": [1, 2],
    "schema_epochs": [bytes([0x11]) * 32, bytes([0x12]) * 32],
    "limits": (1_048_576, 1_000, 10_000, 16, 1_048_576, 1_000_000, 4),
    "methods": ALL_METHODS,
    "features": 1 | 2 | 4,
    "adapters": [bytes([0xA1]) * 32, bytes([0xA2]) * 32],
    "effects": [bytes([0xE1]) * 32],
}
SERVER_HELLO = {
    "protocol_versions": [1],
    "schema_epochs": [bytes([0x12]) * 32, bytes([0x11]) * 32],
    "limits": (4_194_304, 500, 20_000, 8, 2_097_152, 5_000_000, 2),
    "methods": [100, 102, 103, 300, 301, 302, 303, 603],
    "features": 1 | 8,
    "adapters": [bytes([0xA2]) * 32],
    "effects": [],
}
SESSION = bytes([0x51]) * 32


class Failure(Exception):
    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def uvar(value: int) -> bytes:
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def sized(payload: bytes) -> bytes:
    return uvar(len(payload)) + payload


def record(fields: list[bytes]) -> bytes:
    out = bytearray(uvar(len(fields)))
    for index, value in enumerate(fields, start=1):
        out += uvar(index) + sized(value)
    return bytes(out)


def lst(items: list[bytes]) -> bytes:
    return uvar(len(items)) + b"".join(sized(item) for item in items)


def union(tag: int, payload: bytes) -> bytes:
    return uvar(tag) + sized(payload)


def limits(values: tuple) -> bytes:
    return record([uvar(value) for value in values])


def hello(h: dict) -> bytes:
    return record(
        [
            lst([uvar(v) for v in h["protocol_versions"]]),
            lst(h["schema_epochs"]),
            limits(h["limits"]),
            lst([uvar(m) for m in h["methods"]]),
            uvar(h["features"]),
            lst(h["adapters"]),
            lst(h["effects"]),
        ]
    )


def bounds(values: tuple, returned: tuple) -> bytes:
    returned_bytes, entities, edges, depth, omitted, truncated, continuation = returned
    return record(
        [
            limits(values),
            uvar(returned_bytes),
            uvar(entities),
            uvar(edges),
            uvar(depth),
            uvar(omitted),
            uvar(2 if truncated else 1),
            uvar(2 if continuation else 1),
        ]
    )


def frame_payload(session: bytes | None, request_id: int, kind: int, method: int, flags: int, bound: bytes, body: bytes) -> bytes:
    return record(
        [
            uvar(1),
            union(0, b"") if session is None else union(1, session),
            uvar(request_id),
            uvar(kind),
            uvar(method),
            uvar(flags),
            bound,
            sized(body),
        ]
    )


def envelope(epoch: bytes, payload: bytes) -> tuple[bytes, bytes]:
    preimage = MAGIC + uvar(1) + uvar(CONTRACT_TAG) + epoch + sized(payload)
    frame_id = blake3.blake3(FRAME_DOMAIN + preimage).digest()
    stored = preimage + frame_id
    return struct.pack(">Q", len(stored)) + stored, frame_id


def negotiate(client: dict, server: dict) -> dict:
    version = max(v for v in client["protocol_versions"] if v in server["protocol_versions"])
    epoch = next(e for e in server["schema_epochs"] if e in client["schema_epochs"])
    lim = tuple(min(a, b) for a, b in zip(client["limits"], server["limits"]))
    methods = [m for m in client["methods"] if m in server["methods"]]
    adapters = [a for a in client["adapters"] if a in server["adapters"]]
    effects = [e for e in client["effects"] if e in server["effects"]]
    preimage = record(
        [uvar(version), epoch, limits(lim), lst([uvar(m) for m in methods]), uvar(client["features"] & server["features"]), lst(adapters), lst(effects)]
    )
    transcript = hello(client) + hello(server) + preimage
    return {
        "protocol_version": version,
        "schema_epoch": epoch,
        "limits": lim,
        "methods": methods,
        "features": client["features"] & server["features"],
        "preimage": preimage,
        "transcript": transcript,
        "handshake_id": blake3.blake3(HANDSHAKE_DOMAIN + transcript).digest(),
    }


def read_uvar(data: bytes, offset: int) -> tuple[int, int]:
    value, shift = 0, 0
    while True:
        if offset >= len(data):
            raise Failure("PROTOCOL_FRAME_INVALID")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, offset
        shift += 7
        if shift > 63:
            raise Failure("PROTOCOL_FRAME_INVALID")


def inspect(data: bytes, epoch: bytes) -> None:
    """Bounded frame inspection in contract precedence."""
    if len(data) < 8:
        raise Failure("PROTOCOL_FRAME_INVALID")
    length = struct.unpack(">Q", data[:8])[0]
    if length > MAX_FRAME_BYTES:
        raise Failure("PROTOCOL_FRAME_TOO_LARGE")
    stored = data[8:]
    if len(stored) != length or length < 40:
        raise Failure("PROTOCOL_FRAME_INVALID")
    preimage, trailer = stored[:-32], stored[-32:]
    if blake3.blake3(FRAME_DOMAIN + preimage).digest() != trailer:
        raise Failure("PROTOCOL_FRAME_INVALID")
    if preimage[:8] != MAGIC:
        raise Failure("PROTOCOL_FRAME_INVALID")
    offset = 8
    version, offset = read_uvar(preimage, offset)
    if version != 1:
        raise Failure("PROTOCOL_VERSION_UNSUPPORTED")
    tag, offset = read_uvar(preimage, offset)
    if preimage[offset : offset + 32] != epoch:
        raise Failure("PROTOCOL_VERSION_UNSUPPORTED")
    offset += 32
    if tag != CONTRACT_TAG:
        raise Failure("PROTOCOL_FRAME_INVALID")
    payload_len, offset = read_uvar(preimage, offset)
    if offset + payload_len != len(preimage):
        raise Failure("PROTOCOL_FRAME_INVALID")


def main() -> int:
    accepted = json.loads(ACCEPTED.read_text(encoding="utf-8"))
    rejected = json.loads(REJECTED.read_text(encoding="utf-8"))
    problems: list[str] = []
    if accepted.get("contract") != "sley2-smp1-v1" or accepted.get("frame_contract_tag") != CONTRACT_TAG:
        problems.append("accepted-contract")
    epoch = bytes.fromhex(accepted["protocol_epoch_hex"])
    # Hellos.
    for label, source in (("client", CLIENT_HELLO), ("server", SERVER_HELLO)):
        payload = frame_payload(None, 0, 4, 0, 0, bounds((0,) * 7, (0, 0, 0, 0, 0, False, False)), hello(source))
        frame, frame_id = envelope(epoch, payload)
        expected = accepted["hellos"][label]
        if frame.hex() != expected["frame_hex"]:
            problems.append(f"hello:{label}:frame")
        if frame_id.hex() != expected["frame_id"]:
            problems.append(f"hello:{label}:frame_id")
    # Selection.
    selected = negotiate(CLIENT_HELLO, SERVER_HELLO)
    expected = accepted["selected"]
    if selected["preimage"].hex() != expected["preimage_hex"]:
        problems.append("selected:preimage")
    if selected["transcript"].hex() != expected["transcript_hex"]:
        problems.append("selected:transcript")
    if selected["handshake_id"].hex() != expected["handshake_id"]:
        problems.append("selected:handshake_id")
    if selected["protocol_version"] != expected["protocol_version"] or selected["methods"] != expected["methods"]:
        problems.append("selected:derivation")
    if selected["schema_epoch"].hex() != expected["schema_epoch_hex"] or selected["features"] != expected["features"]:
        problems.append("selected:epoch-or-features")
    # Frames.
    zero = bounds((0,) * 7, (0, 0, 0, 0, 0, False, False))
    request_payload = frame_payload(SESSION, 7, 1, 300, 0, zero, b"SLEYRQQ1-body")
    response_bounds = bounds(selected["limits"], (300, 3, 0, 0, 1, True, True))
    response_payload = frame_payload(SESSION, 7, 2, 300, 0, response_bounds, b"SLEYRQR1-body")
    failure_body = record([uvar(31006), sized(b"QUERY_REQUIRED_FACT_OMITTED"), uvar(7), uvar(4), union(0, b""), sized(bytes([1, 2, 3]))])
    failure_payload = frame_payload(SESSION, 7, 2, 300, 4, zero, failure_body)
    expected_frames = {frame["id"]: frame for frame in accepted["frames"]}
    for label, payload in (("request", request_payload), ("response", response_payload), ("failure", failure_payload)):
        frame, frame_id = envelope(epoch, payload)
        if frame.hex() != expected_frames[label]["frame_hex"]:
            problems.append(f"frame:{label}")
        if frame_id.hex() != expected_frames[label]["frame_id"]:
            problems.append(f"frame:{label}:frame_id")
        try:
            inspect(frame, epoch)
        except Failure as failure:
            problems.append(f"frame:{label}:self-inspect:{failure.code}")
    for mutation in rejected.get("mutations", []):
        try:
            inspect(bytes.fromhex(mutation["input_hex"]), epoch)
        except Failure as failure:
            if failure.code != mutation["expected_code"]:
                problems.append(f"{mutation['id']}:code:{failure.code}")
            elif CODES[failure.code] != mutation["expected_numeric"]:
                problems.append(f"{mutation['id']}:numeric")
        else:
            problems.append(f"{mutation['id']}:accepted")
    print(
        json.dumps(
            {
                "contract": "s20-410-smp1-oracle-v1",
                "frames": len(accepted.get("frames", [])),
                "mutations": len(rejected.get("mutations", [])),
                "problems": problems,
                "result": "PASS" if not problems else "FAIL",
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

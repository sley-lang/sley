#!/usr/bin/env python3
"""Independently check the private native worker vector (codec and identity).

Reads `conformance/native-worker/v1` with this script's own SCB1 reader and
BLAKE3 (no Rust code shared) and checks what the bytes promise without
judging VM semantics:

- the `SLEYWRK1` frame: magic, big-endian length, the six ordered fields,
  version 1, a 32-byte profile, the six-slot declared and five-slot
  implementation limit records, the hash list, and the embedded program;
- the `SLEYPRG1` program: magic, length, the five ordered fields, the plan
  envelope and its `sley2.native-test-plan.v1` identity, the state root and
  its `sley2.state-root.v1` identity, and every object's `sley2.object.v1`
  identity;
- the `SLEYNEX1` report: its `sley2.execution-report.v1` identity, the
  plan, test entity, and test and target object bindings to the program,
  and the embedded `SLEYNOB1` observation's identity and its state root,
  profile, input hashes, and limit bindings to the frame;
- the file sizes and SHA-256 digests the vector README pins.

The vector is derived from the Rust VM owner, so the observed value is not
an independent semantics claim; this checker is codec-and-identity depth.
"""

from __future__ import annotations

import hashlib
import re
from pathlib import Path

import blake3


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/native-worker/v1"
MAX_WORKER_FRAME = 262_144
MAX_PROGRAM_BYTES = MAX_WORKER_FRAME - 4_096
MAX_REPORT_BYTES = 262_144


class Refuse(Exception):
    pass


def read_uvar(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        if offset >= len(data):
            raise Refuse("uvar overrun")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, offset
        shift += 7
        if shift >= 70:
            raise Refuse("uvar overflow")


def uvar_value(field: bytes) -> int:
    value, offset = read_uvar(field, 0)
    if offset != len(field):
        raise Refuse("uvar trailing bytes")
    return value


def parse_record(body: bytes, arity: int) -> list[bytes]:
    count, offset = read_uvar(body, 0)
    if count != arity:
        raise Refuse(f"record arity: want {arity}, got {count}")
    fields = []
    for index in range(1, count + 1):
        tag, offset = read_uvar(body, offset)
        length, offset = read_uvar(body, offset)
        if tag != index:
            raise Refuse(f"tag gap: want {index}, got {tag}")
        if offset + length > len(body):
            raise Refuse("field overrun")
        fields.append(body[offset : offset + length])
        offset += length
    if offset != len(body):
        raise Refuse("trailing bytes")
    return fields


def framed(data: bytes, magic: bytes, maximum: int) -> bytes:
    """Magic, big-endian u32 length, then exactly that many record bytes."""
    if len(data) < 12 or len(data) > maximum:
        raise Refuse("frame size")
    if data[:8] != magic:
        raise Refuse("frame magic")
    if int.from_bytes(data[8:12], "big") != len(data) - 12:
        raise Refuse("frame length")
    return data[12:]


def envelope(data: bytes, magic: bytes, domain: bytes) -> tuple[bytes, bytes]:
    """Magic, version 1, sized record, BLAKE3(domain || preimage) trailer."""
    if data[:8] != magic:
        raise Refuse(f"envelope magic {magic!r}")
    version, offset = read_uvar(data, 8)
    if version != 1:
        raise Refuse("envelope version")
    length, offset = read_uvar(data, offset)
    if offset + length + 32 != len(data):
        raise Refuse("envelope length")
    preimage = data[: offset + length]
    trailer = data[offset + length :]
    if blake3.blake3(domain + preimage).digest() != trailer:
        raise Refuse(f"{domain.decode()} identity")
    return data[offset : offset + length], trailer


def standalone_identity(data: bytes, domain: bytes) -> bytes:
    """A standalone SCB1 object's trailer is BLAKE3(domain || preceding bytes)."""
    if data[:8] != b"SLEYSCB1" or len(data) < 41:
        raise Refuse("standalone magic")
    if blake3.blake3(domain + data[:-32]).digest() != data[-32:]:
        raise Refuse(f"{domain.decode()} identity")
    return data[-32:]


def sized_list(body: bytes) -> list[bytes]:
    count, offset = read_uvar(body, 0)
    items = []
    for _ in range(count):
        length, offset = read_uvar(body, offset)
        if offset + length > len(body):
            raise Refuse("list item overrun")
        items.append(body[offset : offset + length])
        offset += length
    if offset != len(body):
        raise Refuse("list trailing bytes")
    return items


def fixed_hash_list(body: bytes) -> list[bytes]:
    count, offset = read_uvar(body, 0)
    if count > 65_535 or offset + 32 * count != len(body):
        raise Refuse("hash list shape")
    return [body[offset + 32 * i : offset + 32 * (i + 1)] for i in range(count)]


def check_readme_pins(files: dict[str, bytes]) -> None:
    text = (FIXTURES / "README.md").read_text(encoding="utf-8")
    pins = re.findall(r"\| `([a-z-]+\.bin)` \| (\d+) \| `([0-9a-f]{64})` \|", text)
    if sorted(name for name, _, _ in pins) != sorted(files):
        raise AssertionError("README pins every vector file exactly once")
    for name, size, digest in pins:
        data = files[name]
        if len(data) != int(size) or hashlib.sha256(data).hexdigest() != digest:
            raise AssertionError(f"{name} differs from its README pin")


def main() -> int:
    files = {
        name: (FIXTURES / name).read_bytes()
        for name in ("observed-input.bin", "observed-report.bin")
    }
    check_readme_pins(files)

    frame = parse_record(
        framed(files["observed-input.bin"], b"SLEYWRK1", MAX_WORKER_FRAME), 6
    )
    if uvar_value(frame[0]) != 1 or len(frame[1]) != 32:
        raise AssertionError("worker frame version and profile width")
    declared = [uvar_value(field) for field in parse_record(frame[2], 6)]
    implementation = [uvar_value(field) for field in parse_record(frame[3], 5)]
    input_hashes = fixed_hash_list(frame[4])
    if implementation[4] > MAX_REPORT_BYTES:
        raise AssertionError("implementation report ceiling exceeds the worker bound")

    program = parse_record(framed(frame[5], b"SLEYPRG1", MAX_PROGRAM_BYTES), 5)
    if uvar_value(program[0]) != 1 or len(program[2]) != 32:
        raise AssertionError("program version and test entity width")
    _, plan_id = envelope(program[1], b"SLEYTPL1", b"sley2.native-test-plan.v1")
    root_id = standalone_identity(program[3], b"sley2.state-root.v1")
    object_ids = [
        standalone_identity(item, b"sley2.object.v1") for item in sized_list(program[4])
    ]
    if not object_ids or len(set(object_ids)) != len(object_ids):
        raise AssertionError("program objects are distinct and present")
    if frame[1] not in program[1]:
        raise AssertionError("the plan names the worker profile")

    report_record, _ = envelope(
        files["observed-report.bin"], b"SLEYNEX1", b"sley2.execution-report.v1"
    )
    if len(files["observed-report.bin"]) > MAX_REPORT_BYTES:
        raise AssertionError("report exceeds the worker output bound")
    report = parse_record(report_record, 6)
    if uvar_value(report[0]) != 1:
        raise AssertionError("report version")
    if report[1] != plan_id:
        raise AssertionError("report plan is the program's plan")
    if report[2] != program[2]:
        raise AssertionError("report test entity is the program's selection")
    if report[3] not in object_ids or report[4] not in object_ids:
        raise AssertionError("report test and target objects are program objects")

    arm, offset = read_uvar(report[5], 0)
    if arm != 1:
        raise AssertionError("vector is the Observed evidence arm")
    length, offset = read_uvar(report[5], offset)
    if offset + length != len(report[5]):
        raise AssertionError("observed evidence length")
    observation_record, _ = envelope(
        report[5][offset:], b"SLEYNOB1", b"sley2.native-test-observation.v1"
    )
    observation = parse_record(observation_record, 18)
    if uvar_value(observation[0]) != 1:
        raise AssertionError("observation version")
    if observation[4] != root_id:
        raise AssertionError("observation state root is the program root")
    if observation[7] != frame[1]:
        raise AssertionError("observation profile is the worker profile")
    if sized_list(observation[8]) != input_hashes:
        raise AssertionError("observation input hashes are the frame's ordered hashes")
    if observation[9] != frame[2] or observation[10] != frame[3]:
        raise AssertionError("observation limits are the frame's limits")
    if uvar_value(observation[17]) != 0:
        raise AssertionError("native v1 observes no effects")
    print(
        "native-worker vector: frame, program "
        f"({len(object_ids)} objects), report, and observation identities OK; "
        f"declared limits {declared}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

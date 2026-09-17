#!/usr/bin/env python3
"""Independently validate the N8 native wire-shape vectors.

Parses every frozen request/response body with this script's own SCB1
record reader (no Rust code shared) and asserts the exact Appendix C
revision 5 layouts: field tags in order, fixed identity widths, pinned
deterministic scalars, and cross-vector consistency between the selected
report, its first page, the commit record, and the status/replay answers.
Freshly minted 605 tokens are opaque 32-byte fields, never decoded.

Rejected bodies must refuse at record framing, except narrow-transaction,
which parses but violates the 32-byte transaction width.
"""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/native-test/v1"
CANDIDATE_MAGIC = b"SLEYCAN1"
EXAMPLE_TOKEN = bytes([0xC5] * 32)
OPTION_UVAR_NONE = bytes([0x00, 0x00])


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


def parse_record(body: bytes) -> list[bytes]:
    count, offset = read_uvar(body, 0)
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


def parse_sized(body: bytes) -> bytes:
    length, offset = read_uvar(body, 0)
    if offset + length != len(body):
        raise Refuse("sized length mismatch")
    return body[offset:]


def uvar_value(field: bytes) -> int:
    value, offset = read_uvar(field, 0)
    if offset != len(field):
        raise Refuse("uvar trailing bytes")
    return value


def load(name: str) -> dict:
    return json.loads((FIXTURES / name).read_text())


def vectors(payload: dict) -> dict[str, dict]:
    return {entry["name"]: entry for entry in payload["vectors"]}


def check_requests() -> None:
    payload = load("requests.json")
    found = vectors(payload)
    # 601: root, selected-id list, profile, attempt. The emitter selects
    # nothing, so the list is the empty encoding.
    fields = parse_record(bytes.fromhex(found["tests.selected.request"]["body_hex"]))
    if len(fields) != 4:
        raise AssertionError("601 request arity")
    if len(fields[0]) != 32 or fields[1] != b"\x00" or len(fields[2]) != 32:
        raise AssertionError("601 request widths")
    if fields[3] != bytes([0xA1] * 16):
        raise AssertionError("601 request attempt")
    # 602: candidate bytes, profile, attempt.
    fields = parse_record(bytes.fromhex(found["tests.affected.request"]["body_hex"]))
    if len(fields) != 3:
        raise AssertionError("602 request arity")
    if not fields[0].startswith(CANDIDATE_MAGIC) or len(fields[0]) < 128:
        raise AssertionError("602 request candidate")
    if len(fields[1]) != 32 or fields[2] != bytes([0xA2] * 16):
        raise AssertionError("602 request widths")
    # 605: example token, offset, max. The example token is unbound by
    # construction; only its width is contractual here.
    fields = parse_record(bytes.fromhex(found["tests.report_read.request"]["body_hex"]))
    if len(fields) != 3:
        raise AssertionError("605 request arity")
    if fields[0] != EXAMPLE_TOKEN:
        raise AssertionError("605 request example token")
    if uvar_value(fields[1]) != 0 or uvar_value(fields[2]) != 65536:
        raise AssertionError("605 request scalars")
    # 606: transaction, root, profile, policy, attempt.
    fields = parse_record(bytes.fromhex(found["tests.replay.request"]["body_hex"]))
    if len(fields) != 5:
        raise AssertionError("606 request arity")
    for field in (fields[0], fields[1], fields[2], fields[3]):
        if len(field) != 32:
            raise AssertionError("606 request widths")
    if fields[4] != bytes([0xA6] * 16):
        raise AssertionError("606 request attempt")
    # 607: attempt plus absent candidate binding (empty means absent).
    fields = parse_record(bytes.fromhex(found["tests.attempt_status.request"]["body_hex"]))
    if len(fields) != 2:
        raise AssertionError("607 request arity")
    if fields[0] != bytes([0xA7] * 16) or fields[1] != b"":
        raise AssertionError("607 request fields")
    # Commit: candidate bytes, parent, attempt, admission profile.
    fields = parse_record(bytes.fromhex(found["commit.request"]["body_hex"]))
    if len(fields) != 4:
        raise AssertionError("commit request arity")
    if not fields[0].startswith(CANDIDATE_MAGIC) or len(fields[0]) < 128:
        raise AssertionError("commit request candidate")
    if len(fields[1]) != 32 or fields[2] != bytes([0xA3] * 16) or len(fields[3]) != 32:
        raise AssertionError("commit request widths")


def check_responses() -> None:
    payload = load("responses.json")
    found = vectors(payload)
    # 601 over the empty plan: comparison-complete, zero selected, a fresh
    # opaque token, and the total report length.
    selected = parse_record(bytes.fromhex(found["tests.selected.response"]["body_hex"]))
    if len(selected) != 6:
        raise AssertionError("601 response arity")
    for field in (selected[0], selected[1]):
        if len(field) != 32:
            raise AssertionError("601 response identities")
    if uvar_value(selected[2]) != 1 or uvar_value(selected[3]) != 0:
        raise AssertionError("601 response scalars")
    if len(selected[4]) != 32:
        raise AssertionError("601 response token width")
    total = uvar_value(selected[5])
    # 602 over the same empty plan answers the identical shape.
    affected = parse_record(bytes.fromhex(found["tests.affected.response"]["body_hex"]))
    if len(affected) != 6:
        raise AssertionError("602 response arity")
    if uvar_value(affected[2]) != 1 or uvar_value(affected[3]) != 0:
        raise AssertionError("602 response scalars")
    if len(affected[4]) != 32:
        raise AssertionError("602 response token width")
    # 605 first page: the report identity and bound root echo the minted
    # capability, the offset echoes the request, the page carries the whole
    # report (max exceeds total), and no next page links.
    page = parse_record(bytes.fromhex(found["tests.report_read.response"]["body_hex"]))
    if len(page) != 6:
        raise AssertionError("605 response arity")
    if page[0] != selected[1]:
        raise AssertionError("605 page report identity")
    if len(page[1]) != 32:
        raise AssertionError("605 page bound root width")
    if uvar_value(page[2]) != 0 or uvar_value(page[3]) != total:
        raise AssertionError("605 page offset/total")
    if len(parse_sized(page[4])) != total:
        raise AssertionError("605 page length")
    if page[5] != OPTION_UVAR_NONE:
        raise AssertionError("605 final page must link nowhere")
    # Commit record: transaction, receipt, root, candidate result bytes,
    # approval, and the caller attempt echoed verbatim.
    commit = parse_record(bytes.fromhex(found["commit.response"]["body_hex"]))
    if len(commit) != 6:
        raise AssertionError("commit response arity")
    for field in (commit[0], commit[1], commit[2]):
        if len(field) != 32:
            raise AssertionError("commit response identities")
    if len(parse_sized(commit[3])) == 0:
        raise AssertionError("commit response result bytes")
    if len(commit[4]) != 32 or commit[5] != bytes([0xB3] * 16):
        raise AssertionError("commit response approval/attempt")
    # 607 committed: state 5 with both identities and a fresh opaque
    # token over the verified accepted report.
    status = parse_record(bytes.fromhex(found["tests.attempt_status.response"]["body_hex"]))
    if len(status) != 4:
        raise AssertionError("607 response arity")
    if uvar_value(status[0]) != 5:
        raise AssertionError("607 committed state")
    if status[1] != commit[0] or status[2] != commit[1]:
        raise AssertionError("607 committed identities")
    if len(status[3]) != 32:
        raise AssertionError("607 response token width")
    # 606 match: status 1 over the committed transaction and root, the
    # original diagnostic report identity, no replay report, and a fresh
    # opaque token.
    replay = parse_record(bytes.fromhex(found["tests.replay.response"]["body_hex"]))
    if len(replay) != 6:
        raise AssertionError("606 response arity")
    if replay[0] != commit[0] or replay[1] != commit[2]:
        raise AssertionError("606 replay scope identities")
    if uvar_value(replay[2]) != 1:
        raise AssertionError("606 match status")
    # The original identity names the commit-time execution report, a
    # different artifact from the diagnostic report above (separate flow,
    # separate executor), so only its width is contractual here.
    if len(replay[3]) != 32:
        raise AssertionError("606 original report width")
    if replay[4] != b"" or len(replay[5]) != 32:
        raise AssertionError("606 replay token shape")


def check_rejected() -> None:
    payload = load("rejected.json")
    found = vectors(payload)
    for name in (
        "truncated",
        "trailing-byte",
        "empty",
        "count-only",
        "short-count",
        "tag-gap",
    ):
        body = bytes.fromhex(found[name]["body_hex"])
        try:
            parse_record(body)
        except Refuse:
            continue
        raise AssertionError(f"{name} must refuse at record framing")
    # Narrow-transaction parses as a record but violates the fixed width.
    fields = parse_record(bytes.fromhex(found["narrow-transaction"]["body_hex"]))
    if len(fields) != 5 or len(fields[0]) == 32:
        raise AssertionError("narrow-transaction shape")


def main() -> int:
    check_requests()
    check_responses()
    check_rejected()
    print("native-test vectors: 6 requests, 6 responses, 7 rejections OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

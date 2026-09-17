#!/usr/bin/env python3
"""Refresh the N8 native wire-shape vectors (601/602/605/606/607 + commit).

Runs the ignored `emit_native_test_vectors_for_fixture_refresh` emitter in
sley-protocol and freezes its output under conformance/native-test/v1/.
Request and rejection bodies are fully deterministic and byte-compared on
--check. Response bodies carry freshly minted 605 tokens, so --check masks
the documented token fields and byte-compares the rest; the static layout
contract (tags, widths, deterministic scalars, cross-vector consistency)
lives in scripts/check_native_test_vectors.py.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "conformance/native-test/v1"
EMITTER = "emit_native_test_vectors_for_fixture_refresh"
EXPECTED_REQUESTS = [
    "tests.selected.request",
    "tests.affected.request",
    "tests.report_read.request",
    "tests.replay.request",
    "tests.attempt_status.request",
    "commit.request",
]
EXPECTED_RESPONSES = [
    "tests.selected.response",
    "tests.affected.response",
    "tests.report_read.response",
    "tests.replay.response",
    "tests.attempt_status.response",
    "commit.response",
]
EXPECTED_REJECTS = [
    "truncated",
    "trailing-byte",
    "empty",
    "count-only",
    "short-count",
    "tag-gap",
    "narrow-transaction",
]
# Response fields holding freshly minted 605 tokens (1-based field tags).
TOKEN_FIELDS = {
    "tests.selected.response": (5,),
    "tests.affected.response": (5,),
    "tests.replay.response": (6,),
    "tests.attempt_status.response": (4,),
}


def read_uvar(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, offset
        shift += 7


def parse_record(body: bytes) -> list[tuple[int, bytes]]:
    count, offset = read_uvar(body, 0)
    fields = []
    for index in range(1, count + 1):
        tag, offset = read_uvar(body, offset)
        length, offset = read_uvar(body, offset)
        if tag != index:
            raise ValueError(f"tag gap: want {index}, got {tag}")
        fields.append((tag, body[offset : offset + length]))
        offset += length
    if offset != len(body):
        raise ValueError("trailing bytes")
    return fields


def masked(body_hex: str, token_fields: tuple[int, ...]) -> str:
    body = bytearray.fromhex(body_hex)
    # Re-encode with token fields zeroed: parse positions, blank values.
    count, offset = read_uvar(bytes(body), 0)
    spans = []
    for _ in range(count):
        tag, offset = read_uvar(bytes(body), offset)
        length, offset = read_uvar(bytes(body), offset)
        spans.append((tag, offset, offset + length))
        offset += length
    if offset != len(body):
        raise ValueError("trailing bytes")
    for tag, start, end in spans:
        if tag in token_fields:
            if end - start != 32:
                raise ValueError(f"token field {tag} is not 32 bytes")
            body[start:end] = b"\x00" * 32
    return bytes(body).hex()


def emit() -> tuple[dict[str, dict], dict[str, dict], dict[str, dict]]:
    completed = subprocess.run(
        ["cargo", "test", "-p", "sley-protocol", EMITTER, "--", "--ignored", "--nocapture"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=True,
    )
    requests: dict[str, dict] = {}
    responses: dict[str, dict] = {}
    rejects: dict[str, dict] = {}
    for line in completed.stdout.splitlines():
        parts = line.split("|")
        if parts[0] == "NATIVE_VECTOR" and parts[1] in ("request", "response"):
            _, kind, name, tag, body_hex = parts
            entry = {
                "body_hex": body_hex,
                "body_sha256": hashlib.sha256(bytes.fromhex(body_hex)).hexdigest(),
                "method_tag": int(tag),
                "name": name,
            }
            (requests if kind == "request" else responses)[name] = entry
        elif parts[0] == "NATIVE_REJECT":
            _, name, body_hex = parts
            rejects[name] = {
                "body_hex": body_hex,
                "body_sha256": hashlib.sha256(bytes.fromhex(body_hex)).hexdigest(),
                "name": name,
            }
    if sorted(requests) != sorted(EXPECTED_REQUESTS):
        raise RuntimeError(f"unexpected request set {sorted(requests)}")
    if sorted(responses) != sorted(EXPECTED_RESPONSES):
        raise RuntimeError(f"unexpected response set {sorted(responses)}")
    if sorted(rejects) != sorted(EXPECTED_REJECTS):
        raise RuntimeError(f"unexpected rejection set {sorted(rejects)}")
    # Every frozen body must parse as an SCB1 record so a truncated
    # emitter failure can never freeze garbage (rejections parse or
    # refuse per their documented class in the checker).
    for name, entry in list(requests.items()) + list(responses.items()):
        parse_record(bytes.fromhex(entry["body_hex"]))
    return requests, responses, rejects


def files() -> dict[Path, object]:
    requests, responses, rejects = emit()
    return {
        FIXTURES / "requests.json": {
            "claim": "sley2-native-test-wire-shape-v1",
            "contract": "docs/spec/NATIVE_TEST_ADMISSION_V1.md appendix C revision 5",
            "generator": "scripts/generate_native_test_fixtures.py",
            "note": "Fixed example identities lock the wire shapes byte-for-byte; "
            "server acceptance of these example identities is NOT claimed "
            "(the 605 example token is unbound, example roots name no live session).",
            "vectors": [requests[name] for name in EXPECTED_REQUESTS],
        },
        FIXTURES / "responses.json": {
            "claim": "sley2-native-test-wire-shape-v1",
            "contract": "docs/spec/NATIVE_TEST_ADMISSION_V1.md appendix C revision 5",
            "generator": "scripts/generate_native_test_fixtures.py",
            "note": "Captured from a live server over the fixed empty-selection "
            "fixtures. Every field is deterministic across runs except freshly "
            "minted 605 tokens (documented per-vector token fields), which the "
            "--check comparison masks and the static checker treats as opaque "
            "32-byte fields.",
            "token_fields": {name: list(fields) for name, fields in TOKEN_FIELDS.items()},
            "vectors": [responses[name] for name in EXPECTED_RESPONSES],
        },
        FIXTURES / "rejected.json": {
            "claim": "sley2-native-test-wire-shape-v1",
            "contract": "docs/spec/NATIVE_TEST_ADMISSION_V1.md appendix C revision 5",
            "generator": "scripts/generate_native_test_fixtures.py",
            "note": "Hand-built malformed 606 bodies. All but narrow-transaction "
            "refuse at record framing; narrow-transaction parses but violates "
            "the 32-byte transaction width.",
            "vectors": [rejects[name] for name in EXPECTED_REJECTS],
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when committed fixtures differ from the crate's vectors",
    )
    arguments = parser.parse_args()
    payloads = files()
    if not arguments.check:
        FIXTURES.mkdir(parents=True, exist_ok=True)
        for path, payload in payloads.items():
            path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
        sha = hashlib.sha256()
        for path in sorted(payloads):
            sha.update(path.name.encode())
            sha.update(path.read_bytes())
        (FIXTURES / "SHA256SUMS").write_text(f"{sha.hexdigest()}  native-test-v1\n")
        return 0
    for path, payload in payloads.items():
        committed = json.loads(path.read_text())
        if path.name == "responses.json":
            fresh_vectors = {entry["name"]: entry for entry in payload["vectors"]}
            old_vectors = {entry["name"]: entry for entry in committed["vectors"]}
            if set(fresh_vectors) != set(old_vectors):
                raise SystemExit(f"response set drifted in {path}")
            for name, fresh in fresh_vectors.items():
                old = old_vectors[name]
                token_fields = tuple(committed["token_fields"].get(name, ()))
                if fresh["method_tag"] != old["method_tag"]:
                    raise SystemExit(f"{name}: method tag drifted")
                if masked(fresh["body_hex"], token_fields) != masked(
                    old["body_hex"], token_fields
                ):
                    raise SystemExit(f"{name}: non-token response bytes drifted")
            continue
        if committed != payload:
            raise SystemExit(f"fixtures drifted in {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

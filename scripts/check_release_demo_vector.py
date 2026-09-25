#!/usr/bin/env python3
"""Independently inspect the S20-720 release demo vectors.

The demo fixture is the source-independence proof of the packaged binary: the
exchange the endpoint exported, the bound `query.root` request and response,
and the `execute` request and its stored report. This oracle re-derives the
`RootQueryId` and the `ExecutionReportId` from the recorded preimages with its
own BLAKE3 and SCB1 readers, implemented from
`docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md` sections 5 and 6,
`docs/spec/REPORT_ENVELOPE_PROFILE_V1.md`, and `docs/spec/SMP1.md` appendix C.
It shares no code with the Rust implementation and judges no semantics.
"""

from __future__ import annotations

import json
from pathlib import Path

import blake3


ROOT = Path(__file__).resolve().parents[1]
DEMO = ROOT / "conformance/release-demo/v1/demo.json"

QUERY_DOMAIN = b"sley2.root-query.v1"
REPORT_DOMAIN = b"sley2.execution-report.v1"
QUERY_REQUEST_MAGIC = b"SLEYRQQ1"
QUERY_RESPONSE_MAGIC = b"SLEYRQR1"
REPORT_MAGIC = b"SLEYEXR1"
EXCHANGE_MAGIC = b"SLEYSCB1"


class DemoFailure(Exception):
    """One exact independent inspection failure."""


def read_uvar(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        if offset >= len(data):
            raise DemoFailure("uvar ended inside the record")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, offset
        shift += 7
        if shift > 63:
            raise DemoFailure("uvar exceeds 64 bits")


def record_fields(payload: bytes, expected: int) -> list[bytes]:
    """Splits one SCB1 record into its field slices.

    A record is `uvar(field_count)` followed by
    `uvar(field_tag) || uvar(length) || value` per field, in ascending tag
    order (`docs/spec/SCB1.md` section on records).
    """
    count, offset = read_uvar(payload, 0)
    if count != expected:
        raise DemoFailure(f"record carries {count} fields, expected {expected}")
    fields: list[bytes] = []
    for expected_tag in range(1, expected + 1):
        tag, offset = read_uvar(payload, offset)
        if tag != expected_tag:
            raise DemoFailure(f"field tag {tag} is not {expected_tag}")
        size, offset = read_uvar(payload, offset)
        if offset + size > len(payload):
            raise DemoFailure("a field runs past the record")
        fields.append(payload[offset : offset + size])
        offset += size
    if offset != len(payload):
        raise DemoFailure("trailing bytes after the record")
    return fields


def main() -> int:
    demo = json.loads(DEMO.read_text(encoding="utf-8"))
    problems: list[str] = []
    if demo.get("contract") != "sley2-release-demo-v1":
        problems.append("contract drift")
    vectors = demo.get("vectors", {})

    request = bytes.fromhex(vectors.get("query_root_request_hex", ""))
    response = bytes.fromhex(vectors.get("query_root_response_hex", ""))
    if not request.startswith(QUERY_REQUEST_MAGIC):
        problems.append("the root query request is not a SLEYRQQ1 preimage")
    if not response.startswith(QUERY_RESPONSE_MAGIC):
        problems.append("the root query response is not a SLEYRQR1 record")
    if request.startswith(QUERY_REQUEST_MAGIC) and response.startswith(QUERY_RESPONSE_MAGIC):
        query_id = blake3.blake3(QUERY_DOMAIN + request).digest()
        # The response repeats the identity and then the snapshot, epoch, root,
        # and workspace of the request (section 6).
        if response[16:48] != query_id:
            problems.append("the response does not carry the derived root query identity")
        if response[48:176] != request[16:144]:
            problems.append("the response does not repeat the request binding")

    execute_request = bytes.fromhex(vectors.get("execute_request_hex", ""))
    try:
        function = record_fields(execute_request, 3)[0]
    except DemoFailure as error:
        problems.append(f"execute request: {error}")
    else:
        if function.hex() != vectors.get("function_id_hex"):
            problems.append("the execute request does not name the recorded function")

    execute_response = bytes.fromhex(vectors.get("execute_response_hex", ""))
    try:
        report_id, stored = record_fields(execute_response, 2)
    except DemoFailure as error:
        problems.append(f"execute response: {error}")
    else:
        if report_id.hex() != vectors.get("execution_report_id_hex"):
            problems.append("the execute response does not carry the recorded report id")
        length, offset = read_uvar(stored, 0)
        preimage = stored[offset : offset + length]
        if offset + length != len(stored):
            problems.append("the stored report length disagrees with its bytes")
        elif not preimage.startswith(REPORT_MAGIC):
            problems.append("the stored report is not a SLEYEXR1 preimage")
        elif blake3.blake3(REPORT_DOMAIN + preimage).digest() != report_id:
            problems.append("the report identity does not re-derive from its preimage")

    exchange = bytes.fromhex(vectors.get("exchange_hex", ""))
    if not exchange.startswith(EXCHANGE_MAGIC):
        problems.append("the exported exchange is not an SCB1 envelope")
    head = bytes.fromhex(vectors.get("head_transaction_id_hex", ""))
    if len(head) != 32 or head not in exchange:
        problems.append("the exported exchange does not carry the recorded head")
    branch = bytes.fromhex(vectors.get("branch_name_hex", ""))
    if not branch or branch not in exchange:
        problems.append("the exported exchange does not carry the recorded branch")

    result = {
        "contract": "s20-720-independent-release-demo-oracle-v1",
        "derived_identities": ["RootQueryId", "ExecutionReportId"],
        "problems": problems,
        "result": "FAIL" if problems else "PASS",
        "scope": "RECORD CONTAINER AND IDENTITY DERIVATION ONLY; NO SEMANTIC JUDGMENT",
        "smp1_revision": demo.get("smp1_revision"),
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())

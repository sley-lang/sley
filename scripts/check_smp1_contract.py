#!/usr/bin/env python3
"""Check the S20-400 SMP1 protocol contract and its stage."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SMP1.md"
ADR = ROOT / "docs/adr/ADR-0032-smp1-transport-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
CRATE = ROOT / "crates/sley-protocol"

DRAFT_STATUS = "S20_400_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_400_CONTRACT_DRAFT_S20_410_IN_PROGRESS"
FROZEN_STATUS = "S20_400_CONTRACT_FROZEN"
IMPLEMENTED_STATUS = "S20_400_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_400_COMPLETE"
IMPLEMENTATION_STATUSES = (DRAFT_IN_PROGRESS_STATUS, IMPLEMENTED_STATUS, COMPLETE_STATUS)

CODES = (
    (40000, "PROTOCOL_VERSION_UNSUPPORTED"),
    (40001, "PROTOCOL_FRAME_INVALID"),
    (40002, "PROTOCOL_FRAME_TOO_LARGE"),
    (40003, "PROTOCOL_NO_COMMON_PROFILE"),
    (40004, "PROTOCOL_DOWNGRADE"),
    (40005, "PROTOCOL_REQUEST_ID_CONFLICT"),
    (40006, "PROTOCOL_SESSION_CLOSED"),
    (40007, "PROTOCOL_METHOD_UNSUPPORTED"),
    (40008, "PROTOCOL_PAYLOAD_INVALID"),
    (40009, "PROTOCOL_LIMIT_EXCEEDED"),
    (40010, "PROTOCOL_CANCELLED"),
    (40011, "PROTOCOL_INTERNAL_INVARIANT"),
)
METHOD_TAGS = (
    [100, 101, 102, 103, 104]
    + list(range(200, 215))
    + [300, 301, 302, 303, 304, 305]
    + [400, 401, 402, 403, 404]
    + [500, 501, 502, 503, 504]
    + [600, 601, 602, 603, 604]
)
SPEC_MARKERS = (
    "Retryability is an explicit mapping from the owner's symbol",
    "`AFTER_REQUERY` names exactly `REF_CAS_STALE`,",
    "`AFTER_LIMIT_CHANGE` names exactly",
    "`AFTER_CAPABILITY` names exactly",
    "# Sley Machine Protocol v1 (SMP1)",
    "Status: S20-400 contract draft",
    "## 1. Framing",
    "digest domain  = sley2.protocol-frame.v1 -> ProtocolFrameId",
    "frame = u64be(frame_length) || protocol_envelope",
    "## 2. Handshake",
    "`PROTOCOL_DOWNGRADE` (threat T45)",
    "the intersection must contain `session.open` (100)",
    "A method family is the hundred-group of a tag",
    "## 3. Sessions and request identity",
    "`PROTOCOL_REQUEST_ID_CONFLICT` (threat T46)",
    "identifier 0 never enters a session",
    "## 4. Method families and tags",
    "A tag added after freeze takes a new",
    "`SMP1-RESERVED-S20-370`",
    "`SMP1-RESERVED-S20-620`",
    "## 5. Bounded context",
    "Zero counts on a failure response or an event frame mean",
    "## 6. Failure envelope",
    "carries it, and the terminal frame of a failed stream carries it",
    "`details` are transport-supplied reason bytes",
    "The envelope's `incident` is none at this revision",
    "## 7. Cancellation and streaming",
    "dispatch costs one unit up front",
    "## 8. JSON bridge",
    "## 11. Explicit exclusions",
    "cancellation (`PROTOCOL_CANCELLED`, skipping binding and budget)",
    "## Appendix A. Body records of the dispatched methods (S20-410)",
    "no non-reserved method answers a deferred detail",
    "## Appendix B. Cancellation, streaming, and budget records (S20-440)",
    "stream_chunk = record(1: uvar(index), 2: uvar(total), 3: bytes(chunk))",
    "## Appendix C. Body records of the slice C methods (S20-410 slice C, revision 7; profile selector revision 8)",
    "6: uvar(profile: 1 restricted_v1 | 2 extended_v1))",
    "**The server owns the retention snapshot.**",
    "**Execute is head-bound.**",
    "needs the negotiated `extended_execute` feature bit",
    "execution_report = record(1: ExecutionReportId[32], 2: bytes(execution report preimage, `SLEYEXR1`))",
    "cancel latency bound is therefore exactly one request execution",
)
ADR_MARKERS = (
    "# ADR-0032: SMP1 transport, negotiation, and identity-scoping boundary",
    "1. **Transport owns no semantics.**",
    "2. **Derived negotiation.**",
    "3. **Session-scoped strictly increasing request identifiers.**",
    "4. **Frozen method table.**",
    "5. **Bounded context on every response.**",
    "6. **Frames are SCB1 envelopes.**",
    "under a single contract tag 400",
    "7. **Staging.**",
    "8. **Batch admission with cancellation before execution.**",
    "9. **Transcript-bound identity.**",
)
ERROR_CODE_ROWS = (
    (40000, "PROTOCOL_VERSION_UNSUPPORTED"),
    (40001, "PROTOCOL_FRAME_INVALID"),
    (40002, "PROTOCOL_FRAME_TOO_LARGE"),
    (40003, "PROTOCOL_NO_COMMON_PROFILE"),
    (40004, "PROTOCOL_DOWNGRADE"),
    (40005, "PROTOCOL_REQUEST_ID_CONFLICT"),
    (40006, "PROTOCOL_SESSION_CLOSED"),
    (40007, "PROTOCOL_METHOD_UNSUPPORTED"),
    (40008, "PROTOCOL_PAYLOAD_INVALID"),
    (40009, "PROTOCOL_LIMIT_EXCEEDED"),
    (40010, "PROTOCOL_CANCELLED"),
    (40011, "PROTOCOL_INTERNAL_INVARIANT"),
)
RESERVED_TAGS = (305, 503, 601, 602)
STALE_TOKEN = "S20-410-SLICE-C-DEFERRED"
WORK_PACKAGE_MARKERS = ("`docs/spec/SMP1.md`", "ADR-0032")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2))
        return 1

    spec = read(SPEC)
    for marker in SPEC_MARKERS:
        if marker not in spec:
            problems.append(f"spec-marker:{marker}")
    if STALE_TOKEN in spec:
        problems.append(f"spec-stale-token:{STALE_TOKEN}")
    for numeric, symbol in CODES:
        if f"| {numeric} | `{symbol}` |" not in spec:
            problems.append(f"spec-code:{symbol}")
    tags = [int(tag) for tag in re.findall(r"^\| (\d{3}) \| `[a-z._]+` \|", spec, flags=re.M)]
    if tags != METHOD_TAGS:
        problems.append(f"spec-method-table:{tags}")
    # Every non-reserved tag has an appendix body row; reserved tags have
    # none (their answer is the section 4 constant, pinned below).
    appendix_tags = sorted(
        int(tag)
        for tag in re.findall(r"^\| (\d{3}) `[^`]+` \|", spec, flags=re.M)
    )
    live_tags = sorted(tag for tag in METHOD_TAGS if tag not in RESERVED_TAGS)
    if appendix_tags != live_tags:
        problems.append(f"spec-appendix-coverage:{appendix_tags}")
    for tag in RESERVED_TAGS:
        if tag in appendix_tags:
            problems.append(f"spec-appendix-reserved:{tag}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    if "revision 11" not in adr:
        problems.append("adr-revision:11")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    codes_text = read(ERROR_CODES)
    if "40000 through 40011" not in codes_text:
        problems.append("error-codes:range-sentence")
    for numeric, symbol in ERROR_CODE_ROWS:
        if f"| {numeric} | `{symbol}` |" not in codes_text:
            problems.append(f"error-codes:row:{symbol}")
    if "carries numeric 36002" not in codes_text:
        problems.append("error-codes:stale-root-alias")
    # The retryability enumeration in section 6 and the server lists agree
    # symbol for symbol: a list the contract omits fails the gate.
    server = read(ROOT / "crates/sley-protocol/src/server.rs")
    for array in ("RETRY_AFTER_REQUERY", "RETRY_AFTER_LIMIT_CHANGE"):
        listed = re.findall(r'"([A-Z][A-Z0-9_]+)"', server.split(array)[1].split("];")[0])
        for symbol in listed:
            if f"`{symbol}`" not in spec:
                problems.append(f"retryability-map:{array}:{symbol}")
    if "AfterCapability" not in server:
        problems.append("retryability-map:reserved-capability")
    if "FEATURE_EXTENDED_EXECUTE" not in server:
        problems.append("execute-profile:feature-gate")

    summary = json.loads(read(SUMMARY))
    section = summary.get("protocol")
    if not isinstance(section, dict):
        problems.append("machine-summary:protocol missing")
        section = {}
    status = section.get("status")
    revision = re.search(r"Status: S20-400 contract draft, revision (\d+)", spec)
    contract_revision = int(revision.group(1)) if revision else None
    if contract_revision is None:
        problems.append("spec-revision:status-line")
    expected = {
        "contract": "docs/spec/SMP1.md",
        "adr": "docs/adr/ADR-0032-smp1-transport-boundary.md",
        "method_count": len(METHOD_TAGS),
        "new_stable_error_codes": len(CODES),
        "frame_domain": "sley2.protocol-frame.v1",
        "handshake_domain": "sley2.protocol-handshake.v1",
        "contract_revision": contract_revision,
        "dispatched_methods": [t for t in METHOD_TAGS if t not in RESERVED_TAGS],
        "frame_contract_tags": [400],
        "contract_complete": status in (FROZEN_STATUS, COMPLETE_STATUS),
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")

    present = []
    if CRATE.exists():
        present.append("crates/sley-protocol")
    # Contract acceptance (section 10) is the freeze: the three reviews
    # must pass with every report-grade finding closed before the status
    # may read FROZEN, and an implementation coexisting with a frozen
    # reviewed document is normal (S20-410 implements under the draft).
    # Only a draft still awaiting review must have no implementation yet.
    if status == DRAFT_STATUS and present:
        problems.append(f"implementation-before-stage:{present}")
    if status in (FROZEN_STATUS, COMPLETE_STATUS):
        for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
            if not str(section.get(key, "")).startswith("PASS"):
                problems.append(f"completion-without-review:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-400-smp1-v1",
        "status": status,
        "revision": contract_revision,
        "method_count": len(METHOD_TAGS),
        "implementation_present": present,
        "new_stable_error_codes": len(CODES),
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

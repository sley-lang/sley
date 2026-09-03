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
    "# Sley Machine Protocol v1 (SMP1)",
    "Status: S20-400 contract draft",
    "## 1. Framing",
    "digest domain  = sley2.protocol-frame.v1 -> ProtocolFrameId",
    "## 2. Handshake",
    "`PROTOCOL_DOWNGRADE` (threat T45)",
    "## 3. Sessions and request identity",
    "`PROTOCOL_REQUEST_ID_CONFLICT` (threat T46)",
    "## 4. Method families and tags",
    "## 5. Bounded context",
    "## 6. Failure envelope",
    "## 7. Cancellation and streaming",
    "## 8. JSON bridge",
    "## 11. Explicit exclusions",
    "## Appendix A. Body records of the dispatched methods (S20-410)",
    "S20-410-SLICE-C-DEFERRED",
)
ADR_MARKERS = (
    "# ADR-0032: SMP1 transport, negotiation, and identity-scoping boundary",
    "1. **Transport owns no semantics.**",
    "2. **Derived negotiation.**",
    "3. **Session-scoped strictly increasing request identifiers.**",
    "4. **Frozen method table.**",
    "5. **Bounded context on every response.**",
    "6. **Frames are SCB1 envelopes.**",
    "7. **Staging.**",
)
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
    for numeric, symbol in CODES:
        if f"| {numeric} | `{symbol}` |" not in spec:
            problems.append(f"spec-code:{symbol}")
    tags = [int(tag) for tag in re.findall(r"^\| (\d{3}) \| `[a-z._]+` \|", spec, flags=re.M)]
    if tags != METHOD_TAGS:
        problems.append(f"spec-method-table:{tags}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    if "40000 through 40011" not in read(ERROR_CODES):
        problems.append("error-codes:range-sentence")

    summary = json.loads(read(SUMMARY))
    section = summary.get("protocol")
    if not isinstance(section, dict):
        problems.append("machine-summary:protocol missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/SMP1.md",
        "adr": "docs/adr/ADR-0032-smp1-transport-boundary.md",
        "method_count": len(METHOD_TAGS),
        "new_stable_error_codes": len(CODES),
        "frame_domain": "sley2.protocol-frame.v1",
        "handshake_domain": "sley2.protocol-handshake.v1",
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
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-stage:{present}")
    if status == COMPLETE_STATUS:
        for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
            if not str(section.get(key, "")).startswith("PASS"):
                problems.append(f"completion-without-review:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-400-smp1-v1",
        "status": status,
        "revision": int(revision.group(1)) if revision else None,
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

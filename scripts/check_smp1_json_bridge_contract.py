#!/usr/bin/env python3
"""Check the S20-420 SMP1 JSON bridge contract and its stage."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SMP1_JSON_BRIDGE_V1.md"
ADR = ROOT / "docs/adr/ADR-0034-json-bridge-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
CRATE = ROOT / "crates/sley-json-bridge"
TABLE = ROOT / "conformance/smp1-json-bridge/v1/methods.json"

DRAFT_STATUS = "S20_420_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_420_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_420_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_420_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_420_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_420_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IN_PROGRESS_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CODES = (
    (42000, "JSON_BRIDGE_SHAPE_INVALID"),
    (42001, "JSON_BRIDGE_NUMBER_INVALID"),
    (42002, "JSON_BRIDGE_HEX_INVALID"),
    (42003, "JSON_BRIDGE_METHOD_UNKNOWN"),
    (42004, "JSON_BRIDGE_RESOURCE_LIMIT"),
)
SPEC_MARKERS = (
    "# SMP1 JSON Bridge v1",
    "Status: S20-420 contract draft",
    "## 1. Declared encodings",
    "at most 2^53 - 1 is a",
    "## 2. Objects",
    "conformance/smp1-json-bridge/v1/methods.json",
    "## 3. Operations",
    "## 4. Unknown and omission states",
    "## 7. Explicit exclusions",
)
ADR_MARKERS = (
    "# ADR-0034: JSON bridge as a generated, non-canonical representation",
    "1. **Bytes stay canonical.**",
    "2. **Declared encodings, no inference.**",
    "3. **Generated method table.**",
    "4. **States copied, codes verbatim.**",
    "5. **Codes.**",
    "6. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/SMP1_JSON_BRIDGE_V1.md`", "ADR-0034")
CRATE_MARKERS = (
    "pub fn frame_to_json",
    "pub fn frame_from_json",
    "pub fn hello_to_json",
    "pub fn failure_to_json",
    "pub fn chunk_to_json",
    "Self::MethodUnknown => 42_003,",
    "Self::ResourceLimit => 42_004,",
)


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
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    if "42000 through 42004" not in read(ERROR_CODES):
        problems.append("error-codes:range-sentence")
    if not TABLE.exists():
        problems.append("methods-table:missing")

    summary = json.loads(read(SUMMARY))
    section = summary.get("json_bridge")
    if not isinstance(section, dict):
        problems.append("machine-summary:json_bridge missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/SMP1_JSON_BRIDGE_V1.md",
        "adr": "docs/adr/ADR-0034-json-bridge-boundary.md",
        "method_table": "conformance/smp1-json-bridge/v1/methods.json",
        "new_stable_error_codes": len(CODES),
        "canonical_form": "SMP1_BYTES_ONLY",
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")

    present = []
    if CRATE.exists():
        present.append("crates/sley-json-bridge")
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in IMPLEMENTATION_STATUSES:
        lib = CRATE / "src/lib.rs"
        source = read(lib) if lib.exists() else ""
        for marker in CRATE_MARKERS:
            if marker not in source:
                problems.append(f"crate-marker:{marker}")
        for numeric, symbol in CODES:
            if symbol not in source:
                problems.append(f"crate-code:{symbol}")
        if status == COMPLETE_STATUS:
            for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-420-smp1-json-bridge-v1",
        "status": status,
        "revision": int(revision.group(1)) if revision else None,
        "implementation_present": present,
        "new_stable_error_codes": len(CODES),
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

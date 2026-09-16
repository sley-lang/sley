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
V2_TABLE = ROOT / "conformance/smp1-json-bridge/v2/methods.json"
# The version 3 table is owned by the native draft family (still awaiting
# owner review), not by this frozen contract: it is checked here for exact
# counts and union shape, while its revision lives in the native contract.
V3_TABLE = ROOT / "conformance/smp1-json-bridge/v3/methods.json"
NATIVE_SPEC = ROOT / "docs/spec/NATIVE_TEST_ADMISSION_V1.md"
V3_SECTION = "## Appendix D. SMP v3 additions table (machine-readable, revision 3)"
SPEC_REVISION = 10
SMP1_REVISION = 12

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
    "conformance/smp1-json-bridge/v2/methods.json",
    "## 10. Prospective version-aware surface",
    "## 3. Operations",
    "## 4. Unknown and omission states",
    "## 7. Explicit exclusions",
    "declared 32-bit fields",
    "declared field order",
    "never a bridge code",
    "which the reader normalizes",
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
# The ceilings the contract states as literals (section 3) are coupled to
# the crate and the oracle here, so a moved constant fails the moment the
# contract, the crate, or the oracle disagrees (revision-9 delta round,
# Vulcan P3: the derived text ceiling was numerically pinned by nothing).
CEILINGS = (
    # (crate declaration, contract literal, oracle literal)
    (
        "pub const MAX_JSON_TEXT_BYTES: usize = 4 * (MAX_FRAME_BYTES as usize);",
        "larger than 268,435,456 bytes",
        "MAX_TEXT_BYTES = 268_435_456",
    ),
    (
        "pub const MAX_JSON_DEPTH: usize = 32;",
        "nested deeper than 32 levels",
        "MAX_DEPTH = 32",
    ),
    (
        "pub const MAX_JSON_ELEMENTS: usize = 1_048_576;",
        "1,048,576 or more value positions",
        "MAX_ELEMENTS = 1_048_576",
    ),
)
FRAME_CEILING = "pub const MAX_FRAME_BYTES: u64 = 67_108_864;"
CRATE_MARKERS = (
    "pub fn frame_to_json",
    "pub fn frame_from_json",
    "pub fn hello_to_json",
    "pub fn failure_to_json",
    "pub fn chunk_to_json",
    "Self::MethodUnknown => 42_003,",
    "Self::ResourceLimit => 42_004,",
    "validate_header",
    "is_sign_negative",
    "METHOD_TABLE_V3_JSON",
    "hello_to_json_for_version",
)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def check_current_delta_review(
    section: dict, expected_revision: int, status: object, problems: list[str]
) -> None:
    """The revision-bound current review record for this contract delta.

    Historical review fields keep their own revisions and never satisfy the
    current delta: only this object, bound to the anchored Status revision,
    admits freeze/complete, while draft/review-pending states stay valid
    with PENDING.
    """
    review = section.get("current_delta_review")
    if not isinstance(review, dict) or set(review) != {
        "contract_revision",
        "ariadne",
        "nabu",
        "vulcan",
    }:
        problems.append("review:current-delta-shape")
        return
    revision = review.get("contract_revision")
    if type(revision) is not int or revision != expected_revision:
        problems.append(f"review:current-delta-revision:{revision!r}")
    for lane in ("ariadne", "nabu", "vulcan"):
        if review.get(lane) not in (
            "PENDING",
            "PASS",
            "NEEDS_WORK",
            "FAIL",
            "INCOMPLETE",
        ):
            problems.append(f"review:current-delta-judgment:{lane}")
    if status == COMPLETE_STATUS or (
        isinstance(status, str) and "CONTRACT_FROZEN" in status
    ):
        if not all(review.get(lane) == "PASS" for lane in ("ariadne", "nabu", "vulcan")):
            problems.append("review:current-delta-frozen-requires-pass")


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
    if not V2_TABLE.exists():
        problems.append("methods-table-v2:missing")
    if not V3_TABLE.exists():
        problems.append("methods-table-v3:missing")
    # Both static method tables and their counts: version 1 stays 41/37,
    # version 2 is the sorted union at 43/39 with the same four reserved.
    # Version 3 overrides the frozen reserved 601/602 rows with the live
    # native selection reads and unions the three still-reserved native
    # rows at 46/41 with five reserved; the frozen contract text is
    # untouched, so the native draft section below is the v3 authority.
    tables: dict[str, dict] = {}
    for label, path in (("v1", TABLE), ("v2", V2_TABLE), ("v3", V3_TABLE)):
        if not path.exists():
            continue
        try:
            tables[label] = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            problems.append(f"methods-table-{label}:unparsable")
    v1_table = tables.get("v1", {})
    v2_table = tables.get("v2", {})
    v3_table = tables.get("v3", {})
    if v1_table.get("method_count") != 41:
        problems.append("methods-table-v1:count")
    if v2_table.get("method_count") != 43 or v2_table.get("reserved_count") != 4:
        problems.append("methods-table-v2:counts")
    v1_tags = [method.get("tag") for method in v1_table.get("methods", [])]
    v2_tags = [method.get("tag") for method in v2_table.get("methods", [])]
    if v1_tags and v2_tags and v2_tags != sorted(set(v1_tags) | {306, 307}):
        problems.append("methods-table-v2:union")
    if v2_tags and [tag for tag in v2_tags if tag in (306, 307)] != [306, 307]:
        problems.append("methods-table-v2:additions")
    if v3_table.get("method_count") != 46 or v3_table.get("reserved_count") != 5:
        problems.append("methods-table-v3:counts")
    v3_tags = [method.get("tag") for method in v3_table.get("methods", [])]
    if v2_tags and v3_tags and v3_tags != sorted(set(v2_tags) | {605, 606, 607}):
        problems.append("methods-table-v3:union")
    if v3_tags and [tag for tag in v3_tags if tag in (605, 606, 607)] != [605, 606, 607]:
        problems.append("methods-table-v3:additions")
    v3_live = {
        method.get("tag"): method.get("reserved", True)
        for method in v3_table.get("methods", [])
        if method.get("tag") in (601, 602, 605, 606, 607)
    }
    if v3_live and (v3_live.get(601) or v3_live.get(602)):
        problems.append("methods-table-v3:live-rows")
    if v3_live and not (
        v3_live.get(605) and v3_live.get(606) and v3_live.get(607)
    ):
        problems.append("methods-table-v3:reserved-rows")
    if v3_table.get("v3_source") != "docs/spec/NATIVE_TEST_ADMISSION_V1.md":
        problems.append("methods-table-v3:source")
    if V3_SECTION not in read(NATIVE_SPEC):
        problems.append("methods-table-v3:native-section")

    summary = json.loads(read(SUMMARY))
    section = summary.get("json_bridge")
    if not isinstance(section, dict):
        problems.append("machine-summary:json_bridge missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/SMP1_JSON_BRIDGE_V1.md",
        "contract_revision": SPEC_REVISION,
        "adr": "docs/adr/ADR-0034-json-bridge-boundary.md",
        "method_table": "conformance/smp1-json-bridge/v1/methods.json",
        "method_table_v2": "conformance/smp1-json-bridge/v2/methods.json",
        "method_table_v3": "conformance/smp1-json-bridge/v3/methods.json",
        "new_stable_error_codes": len(CODES),
        "canonical_form": "SMP1_BYTES_ONLY",
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")
    check_current_delta_review(section, SPEC_REVISION, status, problems)

    present = []
    if CRATE.exists():
        present.append("crates/sley-json-bridge")
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in IMPLEMENTATION_STATUSES:
        lib = CRATE / "src/lib.rs"
        source = read(lib) if lib.exists() else ""
        oracle_text = read(ROOT / "scripts/check_smp1_json_bridge_vector.py")
        protocol_text = read(ROOT / "crates/sley-protocol/src/lib.rs")
        if FRAME_CEILING not in protocol_text:
            problems.append("ceiling:frame:crate")
        for crate_line, contract_literal, oracle_literal in CEILINGS:
            name = crate_line.split()[2].rstrip(":")
            if crate_line not in source:
                problems.append(f"ceiling:{name}:crate")
            if contract_literal not in spec:
                problems.append(f"ceiling:{name}:contract")
            if oracle_literal not in oracle_text:
                problems.append(f"ceiling:{name}:oracle")
        for marker in CRATE_MARKERS:
            if marker not in source:
                problems.append(f"crate-marker:{marker}")
        for numeric, symbol in CODES:
            if symbol not in source:
                problems.append(f"crate-code:{symbol}")

    # Own revision plus the composed authorities, each cross-checked against
    # that document's status line so a stale pin fails the moment it moves.
    # Capable runtime stays phase 3: no capable bridge symbol is required.
    own = re.search(r"^Status: S20-420 contract draft, revision (\d+)", spec, flags=re.M)
    if own is None or int(own.group(1)) != SPEC_REVISION:
        problems.append("spec-revision")
    smp1_text = (ROOT / "docs/spec/SMP1.md").read_text(encoding="utf-8")
    smp1_status = re.search(r"^Status: S20-400 contract draft, revision (\d+)", smp1_text, flags=re.M)
    if smp1_status is None or int(smp1_status.group(1)) != SMP1_REVISION:
        problems.append("smp1-revision-pin")
    if f"`docs/spec/SMP1.md` (revision {SMP1_REVISION})" not in spec:
        problems.append("smp1-pin-text")
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

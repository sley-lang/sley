#!/usr/bin/env python3
"""Check the S20-310 full root-backed query contract and its stage."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md"
RESTRICTED_SPEC = ROOT / "docs/spec/RESTRICTED_QUERY_PROFILE_V1.md"
ADR = ROOT / "docs/adr/ADR-0030-root-backed-query-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
ENGINE = ROOT / "crates/sley-query/src/root_query.rs"
REPOSITORY = ROOT / "crates/sley-repo/src/root_query.rs"
ID_CRATE = ROOT / "crates/sley-id/src/lib.rs"
FIXTURE_DIR = ROOT / "conformance/root-backed-query"

DRAFT_STATUS = "S20_310_FULL_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_310_FULL_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_310_FULL_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_310_FULL_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_310_FULL_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_310_FULL_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IN_PROGRESS_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CODES = (
    (31008, "QUERY_ROOT_MISMATCH"),
    (31009, "QUERY_CONTINUATION_INVALID"),
    (31010, "QUERY_CLASS_NOT_APPLICABLE"),
)
CLASS_COUNT = 19
SPEC_MARKERS = (
    "# Root-Backed Query Profile v1",
    "Status: S20-310 full contract",
    "## 2. Query classes",
    "| 19 | `ListCapabilityRequirementsFor { subject }` |",
    "## 3. Exact results, paging, and continuation",
    "`allow_continuation = false` keeps the restricted rule",
    "`sley2.root-query.v1 -> RootQueryId`",
    '"SLEYRQQ1"',
    '"SLEYRQR1"',
    "## 7. Repository surface",
    "## 10. Explicit exclusions",
)
RESTRICTED_MARKERS = (
    "Status: S20-310 restricted epoch-1 normative specification.",
    "Full S20-310 and the M3 blocker remain open",
)
ADR_MARKERS = (
    "# ADR-0030: Root-backed query classes and continuation boundary",
    "1. **Nineteen classes, one input.**",
    "2. **Exact then paged.**",
    "3. **Binding before answering.**",
    "4. **One repository surface.**",
    "5. **Identity and codes.**",
    "6. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md`", "ADR-0030")
ENGINE_MARKERS = (
    "pub enum RootQuery",
    "pub fn build_root_query_request",
    "pub fn execute_root_query",
    "pub struct RootQueryInput",
    "Self::RootMismatch => 31_008,",
    "Self::ContinuationInvalid => 31_009,",
    "Self::ClassNotApplicable => 31_010,",
)
REPOSITORY_MARKERS = ("pub fn run_root_query", "complete_root_snapshot(")
ID_MARKERS = ('b"sley2.root-query.v1"', "digest_type!(RootQueryId, Domain::RootQuery);")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, RESTRICTED_SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES, ID_CRATE):
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
    class_rows = re.findall(r"^\| (\d{1,2}) \| `[A-Za-z]+", spec, flags=re.M)
    if [int(row) for row in class_rows] != list(range(1, CLASS_COUNT + 1)):
        problems.append(f"spec-class-table:{class_rows}")
    restricted = read(RESTRICTED_SPEC)
    for marker in RESTRICTED_MARKERS:
        if marker not in restricted:
            problems.append(f"restricted-marker:{marker}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    if "31008 through 31010" not in read(ERROR_CODES):
        problems.append("error-codes:range-sentence")

    summary = json.loads(read(SUMMARY))
    section = summary.get("root_backed_query_profile")
    if not isinstance(section, dict):
        problems.append("machine-summary:root_backed_query_profile missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/ROOT_BACKED_QUERY_PROFILE_V1.md",
        "adr": "docs/adr/ADR-0030-root-backed-query-boundary.md",
        "query_classes": CLASS_COUNT,
        "completeness_arm": 2,
        "new_stable_error_codes": len(CODES),
        "identifier_domain": "sley2.root-query.v1",
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")

    identifiers = read(ID_CRATE)
    present = []
    if ENGINE.exists():
        present.append("crates/sley-query/src/root_query.rs")
    if REPOSITORY.exists():
        present.append("crates/sley-repo/src/root_query.rs")
    if ID_MARKERS[0] in identifiers:
        present.append("sley-id:RootQuery")
    if FIXTURE_DIR.exists():
        present.append("conformance/root-backed-query")
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in IMPLEMENTATION_STATUSES:
        engine = read(ENGINE) if ENGINE.exists() else ""
        for marker in ENGINE_MARKERS:
            if marker not in engine:
                problems.append(f"engine-marker:{marker}")
        for numeric, symbol in CODES:
            if symbol not in engine:
                problems.append(f"engine-code:{symbol}")
        repository = read(REPOSITORY) if REPOSITORY.exists() else ""
        for marker in REPOSITORY_MARKERS:
            if marker not in repository:
                problems.append(f"repository-marker:{marker}")
        for marker in ID_MARKERS:
            if marker not in identifiers:
                problems.append(f"id-marker:{marker}")
        if status in (REVIEW_PENDING_STATUS, COMPLETE_STATUS):
            if not (FIXTURE_DIR / "v1/accepted.json").exists():
                problems.append("fixture:missing")
        if status == COMPLETE_STATUS:
            for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-310-full-root-backed-query-profile-v1",
        "status": status,
        "revision": int(revision.group(1)) if revision else None,
        "query_classes": CLASS_COUNT,
        "implementation_present": present,
        "new_stable_error_codes": len(CODES),
        "problems": problems,
        "result": "PASS" if not problems else "FAIL",
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())

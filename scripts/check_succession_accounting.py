#!/usr/bin/env python3
"""Check the S20-630 succession accounting contract and its stage."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SUCCESSION_ACCOUNTING_V1.md"
ADR = ROOT / "docs/adr/ADR-0037-succession-accounting-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
MODULE_DIR = ROOT / "bench/accounting"
MODULE = MODULE_DIR / "report.py"
TESTS = MODULE_DIR / "tests/test_report.py"

DRAFT_STATUS = "S20_630_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_630_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_630_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_630_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_630_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_630_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IN_PROGRESS_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CODES = (
    (63000, "ACCOUNTING_RUN_INVALID"),
    (63001, "ACCOUNTING_CHAIN_INVALID"),
    (63002, "ACCOUNTING_ARM_UNKNOWN"),
    (63003, "ACCOUNTING_METRIC_INVALID"),
    (63004, "ACCOUNTING_FLOAT_FORBIDDEN"),
    (63005, "ACCOUNTING_INCOMPLETE"),
    (63006, "ACCOUNTING_REPORT_INVALID"),
    (63007, "ACCOUNTING_INTERNAL_INVARIANT"),
)
SPEC_MARKERS = (
    "# Succession Accounting v1",
    "Status: S20-630 contract draft",
    "## 1. Inputs",
    "## 2. Arithmetic",
    "## 3. Arm accounting",
    "total_observable_tokens / accepted",
    "## 4. Thresholds",
    "## 5. Report",
    "sley2.succession-accounting-report.v1",
    '"evidence_status": "DERIVED_FROM_UNVERIFIED_CLAIMS"',
    "## 8. Explicit exclusions",
)
ADR_MARKERS = (
    "# ADR-0037: accounting as exact derivation from immutable claims",
    "1. **Claims are the only input.**",
    "2. **Exact arithmetic.**",
    "3. **Every attempt counts.**",
    "4. **Thresholds need complete arms.**",
    "5. **Inherited evidence status.**",
    "6. **Codes.**",
    "7. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/SUCCESSION_ACCOUNTING_V1.md`", "ADR-0037")
MODULE_MARKERS = (
    "class AccountingErrorCode(IntEnum)",
    'REPORT_CONTRACT = "sley2.succession-accounting-report.v1"',
    'EVIDENCE_UNVERIFIED = "DERIVED_FROM_UNVERIFIED_CLAIMS"',
    "def derive_evidence_status(",
    "ARM_VERIFIERS",
    "NOT_EVALUATED_CONDITIONS",
    "LEGACY_CHAIN_RELATIVE",
    "def ratio(",
    "def median(",
    "def arm_accounting(",
    "def evaluate_thresholds(",
    "def derive_report(",
    "def report_digest(",
    "from bench.raw.runner import",
    "from bench.sley2.runner import",
)
FORBIDDEN_MODULE_TOKENS = ("float(", "subprocess", "datetime", "time.", "random", "requests", "urllib")


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
    if "63000 through 63007" not in read(ERROR_CODES):
        problems.append("error-codes:range-sentence")

    summary = json.loads(read(SUMMARY))
    section = summary.get("succession_accounting")
    if not isinstance(section, dict):
        problems.append("machine-summary:succession_accounting missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/SUCCESSION_ACCOUNTING_V1.md",
        "adr": "docs/adr/ADR-0037-succession-accounting-boundary.md",
        "report_contract": "sley2.succession-accounting-report.v1",
        "new_stable_error_codes": len(CODES),
        "arithmetic": "EXACT_INTEGER_AND_RATIO_ONLY",
        "every_attempt_in_denominator": True,
        "trials_accounted": 0,
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")
    for field in ("act_legacy", "act_sley2", "strict_correctness_sley2"):
        if summary.get("succession", {}).get(field) is not None:
            problems.append(f"succession-field-not-null:{field}")

    present = []
    if MODULE_DIR.exists():
        present.append("bench/accounting")
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in IMPLEMENTATION_STATUSES:
        module = read(MODULE) if MODULE.exists() else ""
        for marker in MODULE_MARKERS:
            if marker not in module:
                problems.append(f"module-marker:{marker}")
        for token in FORBIDDEN_MODULE_TOKENS:
            if token in module:
                problems.append(f"module-forbidden:{token}")
        for _, symbol in CODES:
            if symbol not in module:
                problems.append(f"module-code:{symbol}")
        if not TESTS.exists():
            problems.append("module-tests:missing")
        else:
            completed = subprocess.run(
                [sys.executable, "-m", "unittest", "discover", "-s", "bench/accounting/tests", "-t", "."],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            if completed.returncode != 0:
                problems.append("module-tests:failed:" + completed.stderr.strip().splitlines()[-1][:200])
        if status == COMPLETE_STATUS:
            for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-630-succession-accounting-v1",
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

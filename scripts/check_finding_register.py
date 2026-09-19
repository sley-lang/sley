#!/usr/bin/env python3
"""Check the S20-740 finding register contract and its stage."""

from __future__ import annotations

import json
import importlib.util
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/FINDING_REGISTER_V1.md"
ADR = ROOT / "docs/adr/ADR-0042-finding-register-derived-from-recorded-dispositions.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
SCRIPT = ROOT / "scripts/build_finding_register.py"
TESTS = ROOT / "bench/review/tests/test_finding_register.py"
REGISTER = ROOT / "evidence/review/finding-register.json"

DRAFT_STATUS = "S20_740_CONTRACT_DRAFT_REVIEW_PENDING"
IN_PROGRESS_STATUS = "S20_740_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_740_REGISTER_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_740_COMPLETE"
IMPLEMENTATION_STATUSES = (IN_PROGRESS_STATUS, REVIEW_PENDING_STATUS, COMPLETE_STATUS)

CODES = (
    (75000, "REGISTER_SUMMARY_MISSING"),
    (75001, "REGISTER_SUMMARY_INVALID"),
    (75002, "REGISTER_COMPLETION_VIOLATION"),
    (75003, "REGISTER_DRIFT"),
)
SPEC_MARKERS = (
    "# Finding Register v1",
    # The revision number itself is pinned separately against the summary's
    # contract_revision, so this marker stays a prefix across revisions.
    "Status: S20-740 contract draft, revision",
    "## 1. Source",
    "## 2. Obligations",
    "## 3. Register",
    "sley2.finding-register.v1",
    "a completed package may not carry an open review",
    "## 4. Codes",
    "## 5. Staging",
    "## 6. Explicit exclusions",
)
ADR_MARKERS = (
    "# ADR-0042: the finding register is derived from recorded dispositions",
    "1. **One source.**",
    "2. **States, not judgments.**",
    "3. **Completion implies closure.**",
    "4. **Open is the honest state.**",
    "5. **Codes and staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/FINDING_REGISTER_V1.md`", "ADR-0042")
SCRIPT_MARKERS = (
    "class RegisterErrorCode(IntEnum)",
    '"sley2.finding-register.v1"',
    "def classify_token(",
    "def reviewer_of(",
    "def severities_of(",
    "def supersedes(",
    "def field_core(",
    "def field_early(",
    "def collect(",
    "def build_register(",
    "def unclaimed_carried(",
    "def mid_string_complete(",
    "def closed_severities(",
    "def negated_severities(",
    "COMPLETION_VIOLATION",
)
FORBIDDEN_REGISTER_MARKERS = ("/home/", "/greyforge/", "file://")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def load_builder():
    """The builder module, loaded by path so the checker can re-derive."""
    location = ROOT / "scripts/build_finding_register.py"
    spec = importlib.util.spec_from_file_location("build_finding_register", location)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def gate_stays_closed(gate: str) -> bool:
    completed = subprocess.run(
        [sys.executable, "scripts/gate_status.py", gate],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 2:
        return False
    try:
        return json.loads(completed.stdout).get("result") == "NOT_IMPLEMENTED"
    except json.JSONDecodeError:
        return False


def run(argv: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, *argv], cwd=ROOT, check=False, capture_output=True, text=True
    )


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES):
        if not path.exists():
            problems.append(f"missing:{path.relative_to(ROOT)}")
    if problems:
        print(json.dumps({"problems": problems, "result": "FAIL"}, indent=2, sort_keys=True))
        return 1

    spec = read(SPEC)
    for marker in SPEC_MARKERS:
        if marker not in spec:
            problems.append(f"spec-missing:{marker}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-missing:{marker}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-missing:{marker}")
    codes = read(ERROR_CODES)
    if "S20-740 reserves numeric codes 75000 through 75003" not in codes:
        problems.append("error-codes-missing:reservation")
    for number, name in CODES:
        if name not in codes:
            problems.append(f"error-codes-missing:{number}:{name}")
        if name not in spec:
            problems.append(f"spec-missing-code:{name}")

    summary = json.loads(read(SUMMARY))
    section = summary.get("finding_register")
    if not isinstance(section, dict):
        print(
            json.dumps(
                {"problems": ["machine-summary:section"], "result": "FAIL"},
                indent=2,
                sort_keys=True,
            )
        )
        return 1
    status = section.get("status")
    if status not in (DRAFT_STATUS, *IMPLEMENTATION_STATUSES):
        problems.append("machine-summary:status")
    for key, value in (
        ("contract", "docs/spec/FINDING_REGISTER_V1.md"),
        ("adr", "docs/adr/ADR-0042-finding-register-derived-from-recorded-dispositions.md"),
        ("checker", "scripts/check_finding_register.py"),
        ("register", "evidence/review/finding-register.json"),
        ("contract_revision", 6),
        ("new_stable_error_codes", 4),
        ("new_error_code_range", "75000 through 75003"),
        ("ga_claimed", False),
        ("implementation_complete", status == COMPLETE_STATUS),
    ):
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    # The register's own verdict is the review's output, not an input
    # obligation: PENDING at every status except COMPLETE, where the review
    # it awaits has happened and recorded PASS (contract section 1).
    if section.get("independent_review") != (
        "PASS" if status == COMPLETE_STATUS else "PENDING"
    ):
        problems.append("machine-summary:independent_review")
    if f"revision {section.get('contract_revision')}" not in spec:
        problems.append("spec-missing:contract-revision")

    if status in IMPLEMENTATION_STATUSES:
        if not SCRIPT.exists():
            problems.append(f"missing:{SCRIPT.relative_to(ROOT)}")
        else:
            text = read(SCRIPT)
            for marker in SCRIPT_MARKERS:
                if marker not in text:
                    problems.append(f"build_finding_register-missing:{marker}")
        if not TESTS.exists():
            problems.append(f"missing:{TESTS.relative_to(ROOT)}")
        if not REGISTER.exists():
            problems.append(f"missing:{REGISTER.relative_to(ROOT)}")
        else:
            text = read(REGISTER)
            for marker in FORBIDDEN_REGISTER_MARKERS:
                if marker in text:
                    problems.append(f"finding-register-forbidden:{marker}")
            register = json.loads(text)
            if register.get("contract") != "sley2.finding-register.v1":
                problems.append("finding-register:contract")
            if register.get("result") not in ("FINDING_REGISTER_CLEAR", "FINDING_REGISTER_OPEN"):
                problems.append("finding-register:result")
            if register.get("complete_packages_with_open_reviews") != []:
                problems.append("finding-register:completion-violation")
            if section.get("open_reviews") != len(register.get("open_reviews", [])):
                problems.append("machine-summary:open_reviews")
            if section.get("obligations") != register.get("obligation_count"):
                problems.append("machine-summary:obligations")
            if section.get("register_result") != register.get("result"):
                problems.append("machine-summary:register_result")
            # The obligation payload is part of the frozen shape (contract
            # section 3): validate it matches the summary derivation by count
            # and digest, not just the tallies above.
            builder = load_builder()
            expected_obligations = builder.collect(summary)
            if not isinstance(register.get("obligations"), list):
                problems.append("finding-register:obligations-shape")
            elif register.get("obligations") != expected_obligations:
                problems.append("finding-register:obligations-drift")
            if register.get("obligations_digest") != builder.digest_of(expected_obligations):
                problems.append("finding-register:obligations-digest")
            if register.get("contract_revision") != section.get("contract_revision"):
                problems.append("finding-register:contract-revision")
            for entry in register.get("open_reviews", []):
                if set(entry) != {"section", "field", "disposition", "severities"}:
                    problems.append("finding-register:open-reviews-shape")
                    break
            for entry in register.get("unclaimed_carried_findings", []):
                if set(entry) != {
                    "section",
                    "field",
                    "disposition",
                    "unclaimed_severities",
                }:
                    problems.append("finding-register:carried-shape")
                    break
            for entry in register.get("mid_string_complete_packages", []):
                if set(entry) != {"section", "status", "open_obligations"}:
                    problems.append("finding-register:mid-complete-shape")
                    break
            if not isinstance(register.get("superseded_rounds"), list):
                problems.append("finding-register:superseded-rounds-shape")
        if run(["scripts/build_finding_register.py", "--check"]).returncode != 0:
            problems.append("finding-register:drift")
        if run(["-m", "unittest", "discover", "-s", "bench/review/tests", "-t", "."]).returncode != 0:
            problems.append("register-tests:fail")
        if status == COMPLETE_STATUS:
            for review in (
                "ariadne_contract_review",
                "nabu_architecture_review",
                "vulcan_surface_review",
            ):
                if section.get(review) != "PASS":
                    problems.append(f"machine-summary:{review}")
            if section.get("register_result") != "FINDING_REGISTER_CLEAR":
                problems.append("machine-summary:register-not-clear")
            if section.get("independent_review") != "PASS":
                problems.append("machine-summary:independent-review")

    for gate in ("v2", "release-check"):
        if not gate_stays_closed(gate):
            problems.append(f"gate-open:{gate}")

    result = {
        "codes": [name for _, name in CODES],
        "contract": "s20-740-finding-register-v1",
        "implementation_complete": status == COMPLETE_STATUS,
        "problems": problems,
        "result": "FAIL" if problems else "PASS",
        "status": status,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Check the S20-750 decision dossier contract and its stage."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/DECISION_DOSSIER_V1.md"
ADR = ROOT / "docs/adr/ADR-0043-decision-dossier-derived-not-decided.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
SCRIPT = ROOT / "scripts/build_decision_dossier.py"
TESTS = ROOT / "bench/review/tests/test_decision_dossier.py"
DOSSIER = ROOT / "evidence/release/decision-dossier.json"
TEST_INVENTORY = ROOT / "evidence/validation/test-inventory.json"
LICENSE_INVENTORY = ROOT / "evidence/security/T52/pre-release-inventory.json"

SPEC_REVISION = 6

DRAFT_STATUS = "S20_750_CONTRACT_DRAFT_REVIEW_PENDING"
IN_PROGRESS_STATUS = "S20_750_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_750_DOSSIER_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_750_COMPLETE"
IMPLEMENTATION_STATUSES = (IN_PROGRESS_STATUS, REVIEW_PENDING_STATUS, COMPLETE_STATUS)
DECISION_STATES = ("PASS", "CONDITIONAL_PASS", "ALPHA_COMPLETE", "FAIL", "BLOCKED")

CODES = (
    (76000, "DOSSIER_SOURCE_MISSING"),
    (76001, "DOSSIER_SOURCE_INVALID"),
    (76002, "DOSSIER_DECISION_INVALID"),
    (76003, "DOSSIER_DRIFT"),
)
SPEC_MARKERS = (
    "# Decision Dossier v1",
    "Status: S20-750",
    "## 1. Items",
    "## 1.1 The required items",
    "## 2. Sources",
    "## 3. Decision state",
    "sley2.decision-dossier.v1",
    "OPERATOR_DECISION_NOT_DELEGATED",
    "## 4. Dossier",
    "## 5. Codes",
    "## 6. Staging",
    "## 7. Explicit exclusions",
)
ADR_MARKERS = (
    "# ADR-0043: the decision dossier is derived, and the decision is not",
    "1. **One entry per required item.**",
    "2. **Gated is a state, not a gap to be filled by prose.**",
    "3. **The state is derived.**",
    "4. **No mechanism to publish.**",
    "5. **Codes and staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/DECISION_DOSSIER_V1.md`", "ADR-0043")
SCRIPT_MARKERS = (
    "class DossierErrorCode(IntEnum)",
    '"sley2.decision-dossier.v1"',
    "def build_entries(",
    "def derive_decision(",
    "def gate_results(",
    "def enforce_pass_guard(",
    "OPERATOR_DECISION_NOT_DELEGATED",
)
FORBIDDEN_DOSSIER_MARKERS = ("/home/", "greyforge", "file://")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def required_items() -> list[str]:
    """The section 1.1 item list of the contract."""
    spec = read(SPEC)
    start = spec.index("## 1.1 The required items")
    end = spec.index("## 2. Sources", start)
    return [match.group(1).strip() for match in re.finditer(r"^\d+\. (.+)$", spec[start:end], re.M)]


def gate_result(gate: str) -> str:
    """One product gate's state; anything but `OPEN` keeps the gate closed."""
    completed = subprocess.run(
        [sys.executable, "scripts/gate_status.py", gate],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    try:
        return str(json.loads(completed.stdout).get("result", "UNKNOWN"))
    except (json.JSONDecodeError, OSError):
        return "UNKNOWN"


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
    if "S20-750 reserves numeric codes 76000 through 76003" not in codes:
        problems.append("error-codes-missing:reservation")
    for number, name in CODES:
        if name not in codes:
            problems.append(f"error-codes-missing:{number}:{name}")
        if name not in spec:
            problems.append(f"spec-missing-code:{name}")
    items = required_items()
    if len(items) != 34:
        problems.append(f"spec-items:{len(items)}")

    summary = json.loads(read(SUMMARY))
    section = summary.get("decision_dossier")
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
        ("contract", "docs/spec/DECISION_DOSSIER_V1.md"),
        ("adr", "docs/adr/ADR-0043-decision-dossier-derived-not-decided.md"),
        ("checker", "scripts/check_decision_dossier.py"),
        ("dossier", "evidence/release/decision-dossier.json"),
        ("contract_revision", SPEC_REVISION),
        ("decision_authority", "OPERATOR_DECISION_NOT_DELEGATED"),
        ("required_items", 34),
        ("test_inventory", "evidence/validation/test-inventory.json"),
        ("new_stable_error_codes", 4),
        ("new_error_code_range", "76000 through 76003"),
        ("ga_claimed", False),
        ("publication_authorized", False),
        ("implementation_complete", status == COMPLETE_STATUS),
    ):
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")

    if status in IMPLEMENTATION_STATUSES:
        if not SCRIPT.exists():
            problems.append(f"missing:{SCRIPT.relative_to(ROOT)}")
        else:
            text = read(SCRIPT)
            for marker in SCRIPT_MARKERS:
                if marker not in text:
                    problems.append(f"build_decision_dossier-missing:{marker}")
        if not TESTS.exists():
            problems.append(f"missing:{TESTS.relative_to(ROOT)}")
        if not DOSSIER.exists():
            problems.append(f"missing:{DOSSIER.relative_to(ROOT)}")
        else:
            text = read(DOSSIER)
            for marker in FORBIDDEN_DOSSIER_MARKERS:
                if marker in text:
                    problems.append(f"decision-dossier-forbidden:{marker}")
            dossier = json.loads(text)
            if dossier.get("contract") != "sley2.decision-dossier.v1":
                problems.append("decision-dossier:contract")
            if [entry["item"] for entry in dossier.get("entries", [])] != items:
                problems.append("decision-dossier:items")
            if dossier.get("decision_state") not in DECISION_STATES:
                problems.append("decision-dossier:state")
            if dossier.get("decision_authority") != "OPERATOR_DECISION_NOT_DELEGATED":
                problems.append("decision-dossier:authority")
            if any(dossier.get("publication", {}).values()):
                problems.append("decision-dossier:publication")
            if dossier.get("decision_state") == "PASS" and any(
                gate_result(gate) != "OPEN" for gate in ("v2", "release-check")
            ):
                problems.append("decision-dossier:pass-while-gates-closed")
            for entry in dossier.get("entries", []):
                if entry["state"] == "GATED" and entry["value"] is not None:
                    problems.append(f"decision-dossier:gated-with-value:{entry['item'][:40]}")
                if entry["state"] not in ("EVIDENCED", "GATED"):
                    problems.append(f"decision-dossier:entry-state:{entry['item'][:40]}")
            by_item = {entry["item"]: entry for entry in dossier.get("entries", [])}
            try:
                license_inventory = json.loads(read(LICENSE_INVENTORY))
                license_blocked = sum(
                    1
                    for package in license_inventory.get("packages", [])
                    if str(package.get("license_disposition", "")).startswith("BLOCKED")
                )
            except (OSError, json.JSONDecodeError):
                license_blocked = None
                problems.append("decision-dossier:license-inventory-unreadable")
            sbom = by_item.get("SBOM and license inventory", {})
            if (
                license_blocked is not None
                and isinstance(sbom.get("value"), dict)
                and sbom["value"].get("license_disposition_blocked") != license_blocked
            ):
                problems.append("decision-dossier:license-blocked-diverges-from-T52")
            try:
                test_inventory = json.loads(read(TEST_INVENTORY))
            except (OSError, json.JSONDecodeError):
                test_inventory = None
                problems.append("decision-dossier:test-inventory-unreadable")
            counts = by_item.get("property-test counts", {})
            if (
                test_inventory is not None
                and isinstance(counts.get("value"), dict)
                and counts["value"].get("property_tests") != test_inventory.get("property_tests")
            ):
                problems.append("decision-dossier:property-count-diverges-from-inventory")
            if section.get("decision_state") != dossier.get("decision_state"):
                problems.append("machine-summary:decision_state")
            if section.get("evidenced_items") != dossier.get("evidenced"):
                problems.append("machine-summary:evidenced_items")
            if section.get("gated_items") != dossier.get("gated"):
                problems.append("machine-summary:gated_items")
        if run(["scripts/build_decision_dossier.py", "--check"]).returncode != 0:
            problems.append("decision-dossier:drift")
        if run(["scripts/build_test_inventory.py", "--check"]).returncode != 0:
            problems.append("test-inventory:drift")
        if not TEST_INVENTORY.exists():
            problems.append(f"missing:{TEST_INVENTORY.relative_to(ROOT)}")
        if run(["-m", "unittest", "discover", "-s", "bench/review/tests", "-t", "."]).returncode != 0:
            problems.append("dossier-tests:fail")
        if status == COMPLETE_STATUS:
            for review in (
                "ariadne_contract_review",
                "nabu_architecture_review",
                "vulcan_surface_review",
            ):
                if section.get(review) != "PASS":
                    problems.append(f"machine-summary:{review}")
            if section.get("gated_items") != 0:
                problems.append("machine-summary:gated-items-remain")
            if section.get("operator_decision") in (None, "PENDING"):
                problems.append("machine-summary:operator-decision")

    for gate in ("v2", "release-check"):
        if not gate_stays_closed(gate):
            problems.append(f"gate-open:{gate}")

    # The revision is anchored to the Status header (not the first prose
    # occurrence) and pinned: a stale pin fails the moment the contract moves.
    own = re.search(r"^Status: S20-750 contract draft, revision (\d+)", spec, flags=re.M)
    if own is None or int(own.group(1)) != SPEC_REVISION:
        problems.append("spec-revision")

    result = {
        "codes": [name for _, name in CODES],
        "contract": "s20-750-decision-dossier-v1",
        "implementation_complete": status == COMPLETE_STATUS,
        "problems": problems,
        "required_items": len(items),
        "result": "FAIL" if problems else "PASS",
        "status": status,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())

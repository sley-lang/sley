#!/usr/bin/env python3
"""Check the S20-710 full standards SBOM and provenance contract and its stage."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md"
ADR = ROOT / "docs/adr/ADR-0041-standards-sbom-and-unsigned-provenance.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
AUDIT = ROOT / "docs/audits/S20_710_PRE_RELEASE_AUDIT.md"
SBOM_SCRIPT = ROOT / "scripts/build_standards_sbom.py"
PROVENANCE_SCRIPT = ROOT / "scripts/build_release_provenance.py"
TESTS = ROOT / "bench/release/tests/test_standards_sbom.py"
CYCLONEDX = ROOT / "evidence/release/sbom/cyclonedx-1.6.json"
SPDX = ROOT / "evidence/release/sbom/spdx-2.3.json"
PROVENANCE = ROOT / "evidence/release/provenance.json"

DRAFT_STATUS = "S20_710_FULL_CONTRACT_DRAFT_REVIEW_PENDING"
IN_PROGRESS_STATUS = "S20_710_FULL_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_710_FULL_SBOM_AND_PROVENANCE_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_710_FULL_COMPLETE"
IMPLEMENTATION_STATUSES = (IN_PROGRESS_STATUS, REVIEW_PENDING_STATUS, COMPLETE_STATUS)

CODES = (
    (74000, "SBOM_INVENTORY_MISSING"),
    (74001, "SBOM_INVENTORY_INVALID"),
    (74002, "SBOM_COMPONENT_INCOMPLETE"),
    (74003, "SBOM_DOCUMENT_DRIFT"),
    (74004, "PROVENANCE_EVIDENCE_MISSING"),
    (74005, "PROVENANCE_EVIDENCE_INVALID"),
    (74006, "PROVENANCE_SUBJECT_MISMATCH"),
    (74007, "PROVENANCE_DOCUMENT_DRIFT"),
)
SPEC_MARKERS = (
    "# Standards SBOM and Release Provenance v1",
    "Status: S20-710 full-audit contract draft",
    "## 1. Inputs",
    "## 2. CycloneDX 1.6",
    "## 3. SPDX 2.3",
    "## 4. Provenance",
    "sley2.release-provenance.v1",
    "## 5. Determinism",
    "## 6. Codes",
    "## 7. Staging",
    "## 8. Explicit exclusions",
)
ADR_MARKERS = (
    "# ADR-0041: standards SBOM and unsigned provenance derived from local evidence",
    "1. **Derive, never restate.**",
    "2. **Two formats, one inventory.**",
    "3. **Deterministic identity instead of random identity.**",
    "4. **No legal opinion.**",
    "5. **The statement is signable later.**",
    "6. **Honest audit state.**",
    "7. **Codes and staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md`", "ADR-0041")
SBOM_MARKERS = (
    "class SbomErrorCode(IntEnum)",
    "def cyclonedx(",
    "def spdx(",
    "def derived_uuid(",
    '"SPDX-2.3"',
    '"specVersion": "1.6"',
)
PROVENANCE_MARKERS = (
    "class ProvenanceErrorCode(IntEnum)",
    '"sley2.release-provenance.v1"',
    "https://in-toto.io/Statement/v1",
    "https://slsa.dev/provenance/v1",
    "def build_statement(",
    '"signed": False',
)
# Neither document may carry a host path, a user name, or a wall clock
# (contract sections 2 through 5).
FORBIDDEN_DOCUMENT_MARKERS = ("/home/", "greyforge", "file://")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


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
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES, AUDIT):
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
    if "S20-710 full reserves numeric codes 74000 through 74007" not in codes:
        problems.append("error-codes-missing:reservation")
    for number, name in CODES:
        if name not in codes:
            problems.append(f"error-codes-missing:{number}:{name}")
        if name not in spec:
            problems.append(f"spec-missing-code:{name}")
    audit = read(AUDIT)
    if "draft standards SBOM" not in audit:
        problems.append("audit-missing:draft-sbom-note")

    summary = json.loads(read(SUMMARY))
    section = summary.get("standards_sbom_and_provenance")
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
        ("contract", "docs/spec/STANDARDS_SBOM_AND_PROVENANCE_V1.md"),
        ("adr", "docs/adr/ADR-0041-standards-sbom-and-unsigned-provenance.md"),
        ("checker", "scripts/check_standards_sbom_and_provenance.py"),
        ("cyclonedx_document", "evidence/release/sbom/cyclonedx-1.6.json"),
        ("spdx_document", "evidence/release/sbom/spdx-2.3.json"),
        ("provenance_document", "evidence/release/provenance.json"),
        ("signed", False),
        ("transparency_log", False),
        ("ga_claimed", False),
        ("publication_authorized", False),
        ("s20_710_audit_complete", False),
        ("new_stable_error_codes", 8),
        ("new_error_code_range", "74000 through 74007"),
        ("implementation_complete", status == COMPLETE_STATUS),
    ):
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    # The S20-710 audit itself stays blocked: these documents are drafts.
    audit_section = summary.get("s20_710_pre_release_audit", {})
    for key in ("standards_sbom", "release_provenance", "full_s20_710_complete"):
        if audit_section.get(key) is not False:
            problems.append(f"machine-summary:s20_710:{key}")
    if status == COMPLETE_STATUS:
        for review in (
            "ariadne_contract_review",
            "nabu_architecture_review",
            "vulcan_surface_review",
        ):
            if section.get(review) != "PASS":
                problems.append(f"machine-summary:{review}")

    if status in IMPLEMENTATION_STATUSES:
        for path, markers in ((SBOM_SCRIPT, SBOM_MARKERS), (PROVENANCE_SCRIPT, PROVENANCE_MARKERS)):
            if not path.exists():
                problems.append(f"missing:{path.relative_to(ROOT)}")
                continue
            text = read(path)
            for marker in markers:
                if marker not in text:
                    problems.append(f"{path.name}-missing:{marker}")
        if not TESTS.exists():
            problems.append(f"missing:{TESTS.relative_to(ROOT)}")

        for path in (CYCLONEDX, SPDX, PROVENANCE):
            if not path.exists():
                problems.append(f"missing:{path.relative_to(ROOT)}")
                continue
            text = read(path)
            for marker in FORBIDDEN_DOCUMENT_MARKERS:
                if marker in text:
                    problems.append(f"{path.name}-forbidden:{marker}")

        if CYCLONEDX.exists():
            bom = json.loads(read(CYCLONEDX))
            if bom.get("specVersion") != "1.6" or bom.get("bomFormat") != "CycloneDX":
                problems.append("cyclonedx:format")
            if not str(bom.get("serialNumber", "")).startswith("urn:uuid:"):
                problems.append("cyclonedx:serial")
            if "timestamp" in bom.get("metadata", {}):
                problems.append("cyclonedx:timestamp")
        if SPDX.exists():
            document = json.loads(read(SPDX))
            if document.get("spdxVersion") != "SPDX-2.3":
                problems.append("spdx:version")
            if document.get("creationInfo", {}).get("created") != "1970-01-01T00:00:00Z":
                problems.append("spdx:created")
        if PROVENANCE.exists():
            document = json.loads(read(PROVENANCE))
            if document.get("contract") != "sley2.release-provenance.v1":
                problems.append("provenance:contract")
            statement = document.get("statement", {})
            if statement.get("predicateType") != "https://slsa.dev/provenance/v1":
                problems.append("provenance:predicate")
            if document.get("attestation", {}).get("signed") is not False:
                problems.append("provenance:signed")
            if CYCLONEDX.exists():
                bom = json.loads(read(CYCLONEDX))
                root = next(
                    (
                        entry["content"]
                        for entry in bom["metadata"]["component"].get("hashes", [])
                        if entry["alg"] == "SHA-256"
                    ),
                    None,
                )
                subject = statement.get("subject", [{}])[0].get("digest", {}).get("sha256")
                if root != subject:
                    problems.append("provenance:subject-mismatch")

        for argv, label in (
            (["scripts/build_standards_sbom.py", "--check"], "sbom"),
            (["scripts/build_release_provenance.py", "--check"], "provenance"),
        ):
            if run(argv).returncode != 0:
                problems.append(f"{label}:drift")
        if run(["-m", "unittest", "discover", "-s", "bench/release/tests", "-t", "."]).returncode != 0:
            problems.append("release-tests:fail")

    for gate in ("v2", "release-check"):
        if not gate_stays_closed(gate):
            problems.append(f"gate-open:{gate}")

    result = {
        "codes": [name for _, name in CODES],
        "contract": "s20-710-full-standards-sbom-and-provenance-v1",
        "implementation_complete": status == COMPLETE_STATUS,
        "problems": problems,
        "result": "FAIL" if problems else "PASS",
        "s20_710_audit_complete": False,
        "status": status,
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())

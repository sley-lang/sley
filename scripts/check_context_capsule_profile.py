#!/usr/bin/env python3
"""Check the S20-320 full context capsule contract and its stage."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md"
RESTRICTED_SPEC = ROOT / "docs/spec/RESTRICTED_QUERY_CAPSULE_PROFILE_V1.md"
ADR = ROOT / "docs/adr/ADR-0031-context-capsule-boundary.md"
CLOSEOUT = ROOT / "docs/audits/S20_320_FULL_CONTEXT_CAPSULE_CLOSEOUT.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
ENGINE = ROOT / "crates/sley-query/src/context_capsule.rs"
REPOSITORY = ROOT / "crates/sley-repo/src/root_query.rs"
FIXTURE_DIR = ROOT / "conformance/context-capsule"

DRAFT_STATUS = "S20_320_FULL_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_320_FULL_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_320_FULL_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_320_FULL_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_320_FULL_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_320_FULL_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IN_PROGRESS_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CODES = (
    (32008, "CONTEXT_CAPSULE_SOURCE_INVALID"),
    (32009, "CONTEXT_CAPSULE_DICTIONARY_INVALID"),
    (32010, "CONTEXT_CAPSULE_RESOURCE_LIMIT"),
    (32011, "CONTEXT_CAPSULE_INTERNAL_INVARIANT"),
)
SPEC_MARKERS = (
    "# Context Capsule Profile v1",
    "Status: S20-320 full contract",
    "sley2.context-capsule.v1 -> ContextCapsuleId",
    "`build_context_capsule(request, response)`",
    "SessionBinding = None(1) | Negotiated(2); when the arm is",
    "## 4. Omission and continuation status",
    "omitted = total_count - returned",
    '"SLEYCCP1"',
    "## 8. Repository surface",
    "## 11. Explicit exclusions",
)
RESTRICTED_MARKERS = (
    "Status: S20-320 restricted epoch-1 normative specification.",
    "Full S20-320 remains blocked by a full root-backed S20-310 engine",
)
ADR_MARKERS = (
    "# ADR-0031: Context capsule provenance and omission boundary",
    "1. **Question, provenance, status, facts.**",
    "2. **Bound source only.**",
    "3. **Session bound, never implied.**",
    "4. **Master identity.**",
    "5. **Codes.**",
    "6. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md`", "ADR-0031")
ENGINE_MARKERS = (
    "pub fn build_context_capsule",
    "pub fn build_context_capsule_session",
    "const SESSION_BINDING_NEGOTIATED: u32 = 2;",
    "pub struct ContextCapsule",
    'const MAGIC: &[u8; 8] = b"SLEYCCP1";',
    "const SESSION_BINDING_NONE: u32 = 1;",
    "Self::SourceInvalid => 32_008,",
    "Self::DictionaryInvalid => 32_009,",
    "Self::ResourceLimit => 32_010,",
    "Self::InternalInvariant => 32_011,",
)
AUTHORITY = ROOT / "crates/sley-protocol/src/session.rs"
AUTHORITY_MARKERS = (
    "pub fn bind_context_capsule",
    "CapsuleBindError::UnknownSession",
)
REPOSITORY_MARKERS = ("pub fn run_context_capsule",)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, RESTRICTED_SPEC, ADR, CLOSEOUT, WORK_PACKAGES, SUMMARY, ERROR_CODES):
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
    if "32008 through 32011" not in read(ERROR_CODES):
        problems.append("error-codes:range-sentence")

    summary = json.loads(read(SUMMARY))
    section = summary.get("context_capsule_profile")
    if not isinstance(section, dict):
        problems.append("machine-summary:context_capsule_profile missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md",
        "adr": "docs/adr/ADR-0031-context-capsule-boundary.md",
        "identifier_domain": "sley2.context-capsule.v1",
        "session_binding": "NONE_OR_NEGOTIATED_WITH_SESSION_ID_S20_330",
        "new_stable_error_codes": len(CODES),
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")

    present = []
    if ENGINE.exists():
        present.append("crates/sley-query/src/context_capsule.rs")
    if REPOSITORY.exists() and "run_context_capsule" in read(REPOSITORY):
        present.append("sley-repo:run_context_capsule")
    if FIXTURE_DIR.exists():
        present.append("conformance/context-capsule")
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
        authority = read(AUTHORITY) if AUTHORITY.exists() else ""
        for marker in AUTHORITY_MARKERS:
            if marker not in authority:
                problems.append(f"authority-marker:{marker}")
        repository = read(REPOSITORY) if REPOSITORY.exists() else ""
        for marker in REPOSITORY_MARKERS:
            if marker not in repository:
                problems.append(f"repository-marker:{marker}")
        if status in (REVIEW_PENDING_STATUS, COMPLETE_STATUS):
            if not (FIXTURE_DIR / "v1/accepted.json").exists():
                problems.append("fixture:missing")
        if status == COMPLETE_STATUS:
            for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")

    revision = re.search(r"^Status:.*revision (\d+)", spec, re.MULTILINE)
    # The contract revision is the single source: the ADR and the
    # closeout must name the same revision, so a downstream package can
    # never again amend the contract while the boundary record still
    # states the old rule and the gate reports clean. All three reads
    # are Status-line anchored, never first-match, so body prose that
    # mentions older revisions cannot satisfy or break the check.
    for path, label in ((ADR, "adr"), (CLOSEOUT, "closeout")):
        match = (
            re.search(r"^Status:.*revision (\d+)", read(path), re.MULTILINE)
            if path.exists()
            else None
        )
        if match is None:
            problems.append(f"revision-missing:{label}")
        elif revision is None or int(match.group(1)) != int(revision.group(1)):
            problems.append(f"revision-drift:{label}")
    result = {
        "contract": "s20-320-full-context-capsule-profile-v1",
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

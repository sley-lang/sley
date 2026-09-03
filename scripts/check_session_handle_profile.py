#!/usr/bin/env python3
"""Check the S20-330 negotiated session and handle contract and its stage."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SESSION_HANDLE_PROFILE_V1.md"
ADR = ROOT / "docs/adr/ADR-0033-negotiated-session-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
SESSION_MODULE = ROOT / "crates/sley-protocol/src/session.rs"
ID_CRATE = ROOT / "crates/sley-id/src/lib.rs"

DRAFT_STATUS = "S20_330_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_330_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_330_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_330_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_330_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_330_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IN_PROGRESS_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CODES = (
    (33000, "SESSION_UNKNOWN"),
    (33001, "SESSION_WORKSPACE_MISMATCH"),
    (33002, "SESSION_ROOT_ADVANCED"),
    (33003, "SESSION_EPOCH_MISMATCH"),
    (33004, "SESSION_STALE_HANDLE"),
    (33005, "SESSION_HANDLE_UNKNOWN"),
    (33006, "SESSION_RENEWAL_LIMIT"),
    (33007, "SESSION_BINDING_INVALID"),
)
SPEC_MARKERS = (
    "# Negotiated Session and Handle Profile v1",
    "Status: S20-330 contract draft",
    "`sley2.session.v1 -> SessionId`",
    "## 2. Session record and issuance",
    "## 3. Request checks",
    "## 4. Handles",
    "zero-based position in the root's `entity_bindings`",
    "## 5. Capsule binding",
    "`SessionBinding = Negotiated(2) || SessionId[32]`",
    "## 8. Explicit exclusions",
)
ADR_MARKERS = (
    "# ADR-0033: Negotiated session, binding checks, and positional handles",
    "1. **A session binds a handshake to one workspace, root, and epoch.**",
    "2. **Every request is checked against the binding.**",
    "3. **Handles are positions, not allocations.**",
    "4. **Capsules carry the session.**",
    "5. **Codes.**",
    "6. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/SESSION_HANDLE_PROFILE_V1.md`", "ADR-0033")
MODULE_MARKERS = (
    "pub struct SessionRecord",
    "pub fn open_session",
    "pub fn renew_session",
    "pub fn check_session",
    "pub fn expand_handle",
    "Self::StaleHandle => 33_004,",
)
ID_MARKERS = ('b"sley2.session.v1"', "digest_type!(SessionId, Domain::Session);")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES, ID_CRATE):
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
    if "33000 through 33007" not in read(ERROR_CODES):
        problems.append("error-codes:range-sentence")

    summary = json.loads(read(SUMMARY))
    section = summary.get("session_handle_profile")
    if not isinstance(section, dict):
        problems.append("machine-summary:session_handle_profile missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/SESSION_HANDLE_PROFILE_V1.md",
        "adr": "docs/adr/ADR-0033-negotiated-session-boundary.md",
        "identifier_domain": "sley2.session.v1",
        "new_stable_error_codes": len(CODES),
        "handles": "POSITIONAL_IN_BOUND_ROOT_BINDING_ORDER",
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")

    identifiers = read(ID_CRATE)
    present = []
    if SESSION_MODULE.exists():
        present.append("crates/sley-protocol/src/session.rs")
    if ID_MARKERS[0] in identifiers:
        present.append("sley-id:Session")
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in IMPLEMENTATION_STATUSES:
        module = read(SESSION_MODULE) if SESSION_MODULE.exists() else ""
        for marker in MODULE_MARKERS:
            if marker not in module:
                problems.append(f"module-marker:{marker}")
        for numeric, symbol in CODES:
            if symbol not in module:
                problems.append(f"module-code:{symbol}")
        for marker in ID_MARKERS:
            if marker not in identifiers:
                problems.append(f"id-marker:{marker}")
        if status == COMPLETE_STATUS:
            for key in ("nabu_architecture_review", "ariadne_contract_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-330-session-handle-profile-v1",
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

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
SERVER_MODULE = ROOT / "crates/sley-protocol/src/server.rs"
REGISTRY_MODULE = ROOT / "crates/sley-protocol/src/lib.rs"
ID_CRATE = ROOT / "crates/sley-id/src/lib.rs"
SMP1_SPEC = ROOT / "docs/spec/SMP1.md"
CAPSULE_SPEC = ROOT / "docs/spec/CONTEXT_CAPSULE_PROFILE_V1.md"
EVIDENCE = ROOT / "evidence/security"
THREAT_MATRICES = ("T15", "T47", "T56")

DRAFT_STATUS = "S20_330_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_330_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_330_CONTRACT_FROZEN"
IMPLEMENTED_STATUS = "S20_330_CONTRACT_DRAFT_S20_410_IMPLEMENTED_REVIEW_PENDING"
REVIEW_PENDING_STATUS = "S20_330_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_330_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IMPLEMENTED_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CONTRACT_REVISION = 2
SMP1_REVISION = 10
CAPSULE_REVISION = 3

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
    "`sley2.session.v1 -> SessionId`",
    "ServerNonce[32] || ProtocolHandshakeId[32]",
    "## 2. Session record and issuance",
    "## 3. Request checks",
    "The head-bound set is closed.",
    "## 4. Handles",
    "uvar(handle) || StateRoot[32] expected_root",
    "## 5. Capsule binding",
    "`SessionBinding = Negotiated(2) || SessionId[32]`",
    "## 8. Explicit exclusions",
    "at most `max_sessions` remembered",
    "revision 10",
    "revision 3",
    "threat T56",
)
ADR_MARKERS = (
    "# ADR-0033: Negotiated session, binding checks, and positional handles",
    "1. **A session binds a handshake to one workspace, root, and epoch,",
    "2. **The identity-domain choice is unpredictability, keeping",
    "3. **Every request is checked against the binding in true precedence.**",
    "4. **Handles are positions under an expected root, not bare numerals.**",
    "5. **Genesis is the only sessionless state.**",
    "6. **Codes.**",
    "7. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/SESSION_HANDLE_PROFILE_V1.md`", "ADR-0033")
SESSION_MARKERS = (
    "pub struct SessionRecord",
    "pub fn open_session",
    "pub fn renew_session",
    "pub fn check_session",
    "pub fn expand_handle",
    "pub fn fresh_server_nonce",
    "server_nonce",
    "expected_root: StateRoot",
    "max_sessions: u32",
)
SERVER_MARKERS = (
    "head_binding().is_ok()",
    "is_closed(session)",
    "fixed32(body)? != *session.as_bytes()",
    "close(session, self.profile.limits.max_sessions)",
    "expected_root",
    "Method::RefsList",
    "Method::RefsResolve",
    "Method::ExchangeExport",
    "Method::GcDryRun",
    "Method::Report",
)
REGISTRY_MARKERS = (
    "pub fn is_closed",
    "close_order",
    "MAX_LIMIT_SESSIONS",
    "max_sessions",
)
ID_MARKERS = ('b"sley2.session.v1"', "digest_type!(SessionId, Domain::Session);")


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    problems: list[str] = []
    for path in (SPEC, ADR, WORK_PACKAGES, SUMMARY, ERROR_CODES, ID_CRATE, SMP1_SPEC, CAPSULE_SPEC):
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
    revision = re.search(r"^Status:.*revision (\d+)", spec, flags=re.M)
    if revision is None or int(revision.group(1)) != CONTRACT_REVISION:
        problems.append("spec-revision")
    if "revision 11" not in read(SMP1_SPEC):
        problems.append("smp1-revision-pin")
    if "revision 3" not in read(CAPSULE_SPEC):
        problems.append("capsule-revision-pin")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    if f"revision {CONTRACT_REVISION}" not in adr:
        problems.append("adr-revision")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    codes = read(ERROR_CODES)
    if "33000 through 33007" not in codes:
        problems.append("error-codes:range-sentence")
    if "reserves, rather than freezes, these codes" in codes.split("33000 through 33007")[1].split("S20-350")[0]:
        problems.append("error-codes:not-frozen")
    for numeric, symbol in CODES:
        if f"| {numeric} | `{symbol}` |" not in codes:
            problems.append(f"error-codes:row:{symbol}")
    for threat in THREAT_MATRICES:
        matrix_path = EVIDENCE / threat / "matrix.json"
        if not matrix_path.is_file():
            problems.append(f"threat-evidence:missing:{threat}")
            continue
        try:
            matrix = json.loads(matrix_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            problems.append(f"threat-evidence:unparsable:{threat}")
            continue
        if matrix.get("result") != "PASS" or not matrix.get("tests"):
            problems.append(f"threat-evidence:fail:{threat}")

    summary = json.loads(read(SUMMARY))
    section = summary.get("session_handle_profile")
    if not isinstance(section, dict):
        problems.append("machine-summary:session_handle_profile missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/SESSION_HANDLE_PROFILE_V1.md",
        "contract_revision": CONTRACT_REVISION,
        "adr": "docs/adr/ADR-0033-negotiated-session-boundary.md",
        "identifier_domain": "sley2.session.v1",
        "new_stable_error_codes": len(CODES),
        "handles": "POSITIONAL_WITH_EXPECTED_ROOT",
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")

    present = []
    if SESSION_MODULE.exists():
        present.append("crates/sley-protocol/src/session.rs")
    identifiers = read(ID_CRATE)
    if ID_MARKERS[0] in identifiers:
        present.append("sley-id:Session")
    # A draft still awaiting review must have no implementation yet; a
    # frozen reviewed document coexists with its implementation normally.
    if status == DRAFT_STATUS and present:
        problems.append(f"implementation-before-stage:{present}")
    if status in (FROZEN_STATUS,) + IMPLEMENTATION_STATUSES:
        module = read(SESSION_MODULE) if SESSION_MODULE.exists() else ""
        for marker in SESSION_MARKERS:
            if marker not in module:
                problems.append(f"session-marker:{marker}")
        server = read(SERVER_MODULE) if SERVER_MODULE.exists() else ""
        for marker in SERVER_MARKERS:
            if marker not in server:
                problems.append(f"server-marker:{marker}")
        registry = read(REGISTRY_MODULE) if REGISTRY_MODULE.exists() else ""
        for marker in REGISTRY_MARKERS:
            if marker not in registry:
                problems.append(f"registry-marker:{marker}")
        for symbol in [symbol for _, symbol in CODES]:
            if symbol not in module:
                problems.append(f"module-code:{symbol}")
        for numeric, _ in CODES:
            if f"33_{numeric % 1000:03d}" not in module:
                problems.append(f"module-numeric:{numeric}")
        for marker in ID_MARKERS:
            if marker not in identifiers:
                problems.append(f"id-marker:{marker}")
        if status == COMPLETE_STATUS:
            for key in ("nabu_architecture_review", "ariadne_contract_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")

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

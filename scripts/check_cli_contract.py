#!/usr/bin/env python3
"""Check the S20-430 thin CLI contract and its stage."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SLEY_CLI_V1.md"
ADR = ROOT / "docs/adr/ADR-0035-thin-cli-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
CRATE = ROOT / "crates/sley-cli"

DRAFT_STATUS = "S20_430_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_430_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_430_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_430_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_430_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_430_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IN_PROGRESS_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)
SPEC_REVISION = 6
SMP1_REVISION = 12
BRIDGE_REVISION = 8

CODES = (
    (43000, "CLI_USAGE_INVALID", 2),
    (43001, "CLI_INPUT_INVALID", 3),
    (43002, "CLI_IO_FAILURE", 4),
    (43003, "CLI_HANDSHAKE_REQUIRED", 5),
)
SPEC_MARKERS = (
    "# Thin Machine-Oriented CLI v1",
    "Status: S20-430 contract draft",
    "## 1. Commands",
    "sley serve --repository <path> [--json] [--batch] [--report <path>]",
    "## 2. `serve`",
    "`Server::offered_hello`",
    "The offer never carries a transport feature",
    "## 3. Report",
    '"contract": "sley2-cli-report-v1",',
    "## 4. Exit status and stable failures",
    "## 5. Rules audited mechanically",
    "`scripts/check_cli_rules.py` fails closed",
    "## 6. Required evidence",
    "## 7. Explicit exclusions",
    "## 9. Version-aware surface",
    "--protocol-profile v2-capable",
    "--expected-version 1|2",
    "`sley2-cli-v2`",
    "`sley2-cli-report-v2`",
    "no `--protocol-version` alias",
    "VERSION_MISMATCH",
)
ADR_MARKERS = (
    "# ADR-0035: the CLI as a transport endpoint with no semantics",
    "1. **Endpoint only.**",
    "2. **Two orderings, both explicit.**",
    "3. **Two representations, one canonical.**",
    "4. **Counting report.**",
    "5. **Four codes, four exit statuses.**",
    "6. **Mechanical rule audit.**",
    "7. **Staging.**",
    "Revision 6 record (2026-09-09)",
    "the capable CLI runtime is implemented in revision 6 under the phase-3 slice",
)
WORK_PACKAGE_MARKERS = (
    "`docs/spec/SLEY_CLI_V1.md`",
    "ADR-0035",
    "(revision 6, 2026-09-09, ADR-0035, Council reviews pending)",
    "capable CLI runtime implemented under the phase-3 slice",
)
CRATE_MARKERS = (
    "fn serve(",
    "Server::offered_hello(",
    "Server::offered_hello_versioned(",
    "Server::new_versioned(",
    "negotiate_versioned(",
    "hello_to_json_versioned(",
    "answer_batch(",
    "frame_from_json(",
    "frame_to_json(",
    '"sley2-cli-report-v1"',
    '"sley2-cli-report-v2"',
    '"--protocol-profile"',
    "Self::HandshakeRequired => 43_003,",
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
    for numeric, symbol, exit_status in CODES:
        if f"| {numeric} | `{symbol}` | {exit_status} |" not in spec:
            problems.append(f"spec-code:{symbol}")
    adr = read(ADR)
    for marker in ADR_MARKERS:
        if marker not in adr:
            problems.append(f"adr-marker:{marker}")
    packages = read(WORK_PACKAGES)
    for marker in WORK_PACKAGE_MARKERS:
        if marker not in packages:
            problems.append(f"work-package-marker:{marker}")
    if "43000 through 43003" not in read(ERROR_CODES):
        problems.append("error-codes:range-sentence")

    summary = json.loads(read(SUMMARY))
    section = summary.get("cli")
    if not isinstance(section, dict):
        problems.append("machine-summary:cli missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/SLEY_CLI_V1.md",
        "contract_revision": SPEC_REVISION,
        "adr": "docs/adr/ADR-0035-thin-cli-boundary.md",
        "rule_audit": "scripts/check_cli_rules.py",
        "new_stable_error_codes": len(CODES),
        "semantic_authority": "SERVER_ONLY",
        "offered_hello": "Server::offered_hello (legacy) and Server::offered_hello_versioned (capable)",
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
        present.append("crates/sley-cli")
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in IMPLEMENTATION_STATUSES:
        sources = "".join(read(path) for path in sorted(CRATE.glob("src/*.rs"))) if CRATE.exists() else ""
        for marker in CRATE_MARKERS:
            if marker not in sources:
                problems.append(f"crate-marker:{marker}")
        for _, symbol, _ in CODES:
            if symbol not in sources:
                problems.append(f"crate-code:{symbol}")

    # Own revision plus the composed authorities, each cross-checked against
    # that document's status line so a stale pin fails the moment it moves.
    # The section 9 capable surface is implemented: the crate assertions
    # include capable-runtime markers, and the legacy markers still pin the
    # frozen default path.
    own = re.search(r"^Status: S20-430 contract draft, revision (\d+)", spec, flags=re.M)
    if own is None or int(own.group(1)) != SPEC_REVISION:
        problems.append("spec-revision")
    smp1_text = (ROOT / "docs/spec/SMP1.md").read_text(encoding="utf-8")
    smp1_status = re.search(r"^Status: S20-400 contract draft, revision (\d+)", smp1_text, flags=re.M)
    if smp1_status is None or int(smp1_status.group(1)) != SMP1_REVISION:
        problems.append("smp1-revision-pin")
    if f"SMP1 revision {SMP1_REVISION} and bridge revision {BRIDGE_REVISION}" not in spec:
        problems.append("smp1-pin-text")
    bridge_text = (ROOT / "docs/spec/SMP1_JSON_BRIDGE_V1.md").read_text(encoding="utf-8")
    bridge_status = re.search(r"^Status: S20-420 contract draft, revision (\d+)", bridge_text, flags=re.M)
    if bridge_status is None or int(bridge_status.group(1)) != BRIDGE_REVISION:
        problems.append("bridge-revision-pin")
    if f"`docs/spec/SMP1_JSON_BRIDGE_V1.md` revision {BRIDGE_REVISION}" not in spec:
        problems.append("bridge-pin-text")
    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-430-thin-cli-v1",
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

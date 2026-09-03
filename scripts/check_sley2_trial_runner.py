#!/usr/bin/env python3
"""Check the S20-620 Sley 2 trial runner contract and its stage."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "docs/spec/SLEY2_TRIAL_RUNNER_V1.md"
ADR = ROOT / "docs/adr/ADR-0036-sley2-trial-runner-boundary.md"
WORK_PACKAGES = ROOT / "docs/WORK_PACKAGES.md"
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
ERROR_CODES = ROOT / "docs/spec/ERROR_CODES_V1.md"
RUNNER_DIR = ROOT / "bench/sley2"
RUNNER = RUNNER_DIR / "runner.py"
TESTS = RUNNER_DIR / "tests/test_runner.py"

DRAFT_STATUS = "S20_620_CONTRACT_DRAFT_REVIEW_PENDING"
DRAFT_IN_PROGRESS_STATUS = "S20_620_CONTRACT_DRAFT_IMPLEMENTATION_IN_PROGRESS"
FROZEN_STATUS = "S20_620_CONTRACT_FROZEN_IMPLEMENTATION_PENDING"
IN_PROGRESS_STATUS = "S20_620_CONTRACT_FROZEN_IMPLEMENTATION_IN_PROGRESS"
REVIEW_PENDING_STATUS = "S20_620_IMPLEMENTED_REVIEW_PENDING"
COMPLETE_STATUS = "S20_620_COMPLETE"
IMPLEMENTATION_STATUSES = (
    DRAFT_IN_PROGRESS_STATUS,
    IN_PROGRESS_STATUS,
    REVIEW_PENDING_STATUS,
    COMPLETE_STATUS,
)

CODES = (
    (62000, "SLEY2_TRIAL_MANIFEST_INVALID"),
    (62001, "SLEY2_TRIAL_ENDPOINT_UNAVAILABLE"),
    (62002, "SLEY2_TRIAL_HANDSHAKE_FAILED"),
    (62003, "SLEY2_TRIAL_FRAME_INVALID"),
    (62004, "SLEY2_TRIAL_TRACE_INVALID"),
    (62005, "SLEY2_TRIAL_CLAIM_INVALID"),
    (62006, "SLEY2_TRIAL_PRIVILEGED_CONTEXT"),
    (62007, "SLEY2_TRIAL_DUPLICATE"),
    (62008, "SLEY2_TRIAL_TIMEOUT"),
    (62009, "SLEY2_TRIAL_INTERNAL_INVARIANT"),
)
SPEC_MARKERS = (
    "# Sley 2 Trial Runner v1",
    "Status: S20-620 contract draft",
    "## 1. Endpoint driving",
    "## 2. Privileged-context guard",
    "exchange(frame_object) -> [frame_object]",
    "affordances() -> [method_name]",
    "## 3. Complete trace",
    "sley2.sley2-trial-trace.v1",
    "## 4. Trial claims",
    "sley2.sley2-trial-digest-claim.v1",
    "## 5. Smoke and control audit",
    "make sley2-runner-smoke",
    "## 8. Explicit exclusions",
)
ADR_MARKERS = (
    "# ADR-0036: the Sley 2 arm as an endpoint-only, trace-complete runner",
    "1. **Endpoint only.**",
    "2. **Two-operation handle.**",
    "3. **Trace before progress.**",
    "4. **Trace-derived versus injected.**",
    "5. **Same chain mechanics.**",
    "6. **Codes.**",
    "7. **Staging.**",
)
WORK_PACKAGE_MARKERS = ("`docs/spec/SLEY2_TRIAL_RUNNER_V1.md`", "ADR-0036")
RUNNER_MARKERS = (
    "class Sley2ErrorCode(IntEnum)",
    "class EndpointHandle",
    "def exchange(self",
    "def affordances(self",
    "class AgentAdapter(Protocol)",
    'TRACE_CONTRACT = "sley2.sley2-trial-trace.v1"',
    'CLAIM_CONTRACT = "sley2.sley2-trial-digest-claim.v1"',
    "def append_trace_record",
    "def verify_trace",
    "def derive_trace_metrics",
    "def append_trial_claim",
    "def run_scripted_trial",
    "from bench.raw.runner import",
)
FORBIDDEN_RUNNER_TOKENS = ("sley_repo", "sley_query", "sley_protocol", "requests.", "urllib", "openai", "anthropic")


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
    if "62000 through 62009" not in read(ERROR_CODES):
        problems.append("error-codes:range-sentence")

    summary = json.loads(read(SUMMARY))
    section = summary.get("sley2_trial_runner")
    if not isinstance(section, dict):
        problems.append("machine-summary:sley2_trial_runner missing")
        section = {}
    status = section.get("status")
    expected = {
        "contract": "docs/spec/SLEY2_TRIAL_RUNNER_V1.md",
        "adr": "docs/adr/ADR-0036-sley2-trial-runner-boundary.md",
        "trace_contract": "sley2.sley2-trial-trace.v1",
        "claim_contract": "sley2.sley2-trial-digest-claim.v1",
        "new_stable_error_codes": len(CODES),
        "agent_context": "ENDPOINT_HANDLE_EXCHANGE_AND_AFFORDANCES_ONLY",
        "provider_or_model_execution": False,
        "actual_trials": 0,
        "implementation_complete": status == COMPLETE_STATUS,
    }
    for key, value in expected.items():
        if section.get(key) != value:
            problems.append(f"machine-summary:{key}")
    if status not in (DRAFT_STATUS, FROZEN_STATUS) + IMPLEMENTATION_STATUSES:
        problems.append("machine-summary:status")

    present = []
    if RUNNER_DIR.exists():
        present.append("bench/sley2")
    if status in (DRAFT_STATUS, FROZEN_STATUS) and present:
        problems.append(f"implementation-before-freeze:{present}")
    if status in IMPLEMENTATION_STATUSES:
        runner = read(RUNNER) if RUNNER.exists() else ""
        for marker in RUNNER_MARKERS:
            if marker not in runner:
                problems.append(f"runner-marker:{marker}")
        for token in FORBIDDEN_RUNNER_TOKENS:
            if token in runner:
                problems.append(f"runner-forbidden:{token}")
        for _, symbol in CODES:
            if symbol not in runner:
                problems.append(f"runner-code:{symbol}")
        if not TESTS.exists():
            problems.append("runner-tests:missing")
        if status == COMPLETE_STATUS:
            for key in ("ariadne_contract_review", "nabu_architecture_review", "vulcan_surface_review"):
                if not str(section.get(key, "")).startswith("PASS"):
                    problems.append(f"completion-without-review:{key}")

    revision = re.search(r"revision (\d+)", spec)
    result = {
        "contract": "s20-620-sley2-trial-runner-v1",
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

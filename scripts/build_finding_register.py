#!/usr/bin/env python3
"""S20-740 finding register: every review obligation and its disposition.

Derives `evidence/review/finding-register.json` from the machine summary: the
review obligations each package recorded, their states, the severities their
dispositions name, and the invariant that a completed package carries no open
review. It never edits a disposition, issues a finding, or dispatches a review.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from enum import IntEnum
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SUMMARY = ROOT / "machineresearch/sley-2.0/machine-summary.json"
REGISTER = ROOT / "evidence/review/finding-register.json"
CONTRACT = "sley2.finding-register.v1"
# Field names that name a role, actor, session, instant, or free note rather
# than a disposition (contract section 1).
SKIP_FIELD = re.compile(r"reviewer_role|reviews$|_session_id$|_at$|_by$|_id$|timestamp|note")
INSTANT = re.compile(r"^\d{4}-\d{2}-\d{2}T")
SESSION = re.compile(r"^forge-|^[a-z]+-[a-z]+-s20-")
SEVERITY = re.compile(r"P[0-4]")


class RegisterErrorCode(IntEnum):
    """S20-740 finding register failures (contract section 4)."""

    SUMMARY_MISSING = 75000
    SUMMARY_INVALID = 75001
    COMPLETION_VIOLATION = 75002
    DRIFT = 75003


class RegisterError(Exception):
    """One exact S20-740 failure."""

    def __init__(self, code: RegisterErrorCode, detail: str) -> None:
        super().__init__(f"{code.name}: {detail}")
        self.code = code
        self.detail = detail


def canonical(value: object) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def digest_of(value: object) -> str:
    return hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def classify(disposition: str) -> str:
    """The contract section 2 state of one recorded disposition."""
    if disposition.startswith("PASS") or disposition == "VULCAN_PASS":
        return "PASS"
    if disposition == "PENDING":
        return "PENDING"
    if disposition.startswith("DEFERRED"):
        return "DEFERRED"
    if disposition.startswith(("FAIL", "REVISE")):
        return "HISTORICAL_ROUND"
    return "OTHER"


def is_obligation(field: str, value: object) -> bool:
    if not isinstance(value, str) or not value:
        return False
    if "review" not in field and "disposition" not in field:
        return False
    if SKIP_FIELD.search(field):
        return False
    return not INSTANT.match(value) and not SESSION.match(value)


def collect(summary: dict) -> list[dict]:
    """Every review obligation of the summary, ascending by section and field."""
    obligations: list[dict] = []

    def walk(node: object, path: str, status: str | None) -> None:
        if isinstance(node, dict):
            own_status = node.get("status") if isinstance(node.get("status"), str) else status
            for field, value in node.items():
                child = f"{path}.{field}" if path else field
                if is_obligation(field, value):
                    obligations.append(
                        {
                            "section": path or "(root)",
                            "field": field,
                            "disposition": value,
                            "state": classify(value),
                            "severities": sorted(set(SEVERITY.findall(value))),
                            "declares_closed_findings": "CLOSED" in value,
                            "declares_no_open_p0_p1_p2": "NO_OPEN_P0_P1_P2" in value,
                            "package_status": own_status,
                        }
                    )
                walk(value, child, own_status)
        elif isinstance(node, list):
            for index, value in enumerate(node):
                walk(value, f"{path}[{index}]", status)

    walk(summary, "", None)
    return sorted(obligations, key=lambda item: (item["section"], item["field"]))


def load_summary() -> dict:
    if not SUMMARY.exists():
        raise RegisterError(
            RegisterErrorCode.SUMMARY_MISSING,
            "machineresearch/sley-2.0/machine-summary.json does not exist",
        )
    try:
        summary = json.loads(SUMMARY.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RegisterError(RegisterErrorCode.SUMMARY_INVALID, str(error)) from error
    if not isinstance(summary, dict):
        raise RegisterError(RegisterErrorCode.SUMMARY_INVALID, "the summary is not an object")
    return summary


def build_register() -> dict:
    summary = load_summary()
    obligations = collect(summary)
    if not obligations:
        raise RegisterError(
            RegisterErrorCode.SUMMARY_INVALID, "the summary records no review obligation"
        )
    complete_packages = sorted(
        section
        for section, value in summary.items()
        if isinstance(value, dict) and str(value.get("status", "")).endswith("COMPLETE")
    )
    complete_set = set(complete_packages)
    violations = [
        {"section": item["section"], "field": item["field"], "state": item["state"]}
        for item in obligations
        if item["section"].split(".")[0] in complete_set
        and item["state"] in ("PENDING", "DEFERRED", "OTHER")
    ]
    if violations:
        raise RegisterError(
            RegisterErrorCode.COMPLETION_VIOLATION,
            "; ".join(
                f"{violation['section']}.{violation['field']} is {violation['state']}"
                for violation in violations
            ),
        )
    states: dict[str, int] = {}
    severities = {f"P{index}": 0 for index in range(5)}
    for item in obligations:
        states[item["state"]] = states.get(item["state"], 0) + 1
        for severity in item["severities"]:
            severities[severity] += 1
    declared = summary.get("open_findings", {})
    open_reviews = [
        {"section": item["section"], "field": item["field"]}
        for item in obligations
        if item["state"] == "PENDING"
    ]
    clear = (
        not open_reviews
        and not violations
        and all(value == 0 for value in declared.values() if isinstance(value, int))
    )
    register = {
        "contract": CONTRACT,
        "work_package": "S20-740",
        "source": "machineresearch/sley-2.0/machine-summary.json",
        # The digest covers the derived obligations, not the summary bytes: the
        # summary also records this register's own counts, so a byte digest
        # would never reach a fixed point (contract section 3).
        "obligations_digest": digest_of(obligations),
        "obligation_count": len(obligations),
        "states": states,
        "severity_mentions": severities,
        "obligations": obligations,
        "open_reviews": open_reviews,
        "deferred_reviews": [
            {
                "section": item["section"],
                "field": item["field"],
                "disposition": item["disposition"],
            }
            for item in obligations
            if item["state"] == "DEFERRED"
        ],
        "unclassified": [
            {
                "section": item["section"],
                "field": item["field"],
                "disposition": item["disposition"],
            }
            for item in obligations
            if item["state"] == "OTHER"
        ],
        "complete_packages": complete_packages,
        "complete_packages_with_open_reviews": violations,
        "declared_open_findings": declared,
        "result": "FINDING_REGISTER_CLEAR" if clear else "FINDING_REGISTER_OPEN",
    }
    register["register_digest"] = digest_of(register)
    return register


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        register = build_register()
        text = canonical(register)
        summary = {
            "obligations": register["obligation_count"],
            "open_reviews": len(register["open_reviews"]),
            "deferred_reviews": len(register["deferred_reviews"]),
            "result": register["result"],
        }
        if args.check:
            current = REGISTER.read_text(encoding="utf-8") if REGISTER.exists() else None
            if current != text:
                print(
                    canonical(
                        {
                            "mode": "check",
                            "result": "FAIL",
                            "code": int(RegisterErrorCode.DRIFT),
                            "name": RegisterErrorCode.DRIFT.name,
                            "detail": "the tracked register differs from the derived register",
                        }
                    ),
                    end="",
                )
                return 1
            print(canonical({"mode": "check", "result": "PASS", **summary}), end="")
            return 0
        REGISTER.parent.mkdir(parents=True, exist_ok=True)
        REGISTER.write_text(text, encoding="utf-8")
        print(canonical({"mode": "write", "result": "PASS", **summary}), end="")
        return 0
    except RegisterError as error:
        print(
            canonical(
                {
                    "result": "FAIL",
                    "code": int(error.code),
                    "name": error.code.name,
                    "detail": error.detail,
                }
            ),
            end="",
        )
        return 1


if __name__ == "__main__":
    sys.exit(main())

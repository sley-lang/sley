#!/usr/bin/env python3
"""S20-740 finding register: every review obligation and its disposition.

Derives `evidence/review/finding-register.json` from the machine summary: the
review obligations each package recorded, their states, the severities their
dispositions name, and the invariant that a completed package carries no open
review. It never edits a disposition, issues a finding, or dispatches a review.

Contract revision 2 (2026-09-05) tightens revision 1 after the three Council
reviews of the draft (8 P0s): dispositions are classified by first
underscore-delimited token over a closed head set plus a declared alias
table, a FAIL/REVISE round is historical only with a same-reviewer
superseding PASS in the same section, severity tokens exclude negated
NO_OPEN groups, and CLEAR requires the top-level counters and every
per-package open claim to read zero.
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
CONTRACT_REVISION = 2
# Field names that name a role, actor, session, instant, or free note rather
# than a disposition (contract section 1). All suffix-anchored: a bare
# substring match would silently drop a future field that merely contains
# one of these words.
SKIP_FIELD = re.compile(
    r"reviewer_role|reviews$|_session_id$|_at$|_by$|_id$|_timestamp$|_note$"
)
# Values that are ISO-8601 instants or Council session identifiers
# (forge-<lane>-s20-<slice>-<instant>-<nonce>) rather than dispositions.
INSTANT = re.compile(r"^\d{4}-\d{2}-\d{2}T")
SESSION = re.compile(r"^forge-|^[a-z]+-[a-z]+-s20-")
# Reviewer lane tokens a field name may carry. A FAIL/REVISE round is
# superseded only by a PASS carrying the same token in the same section.
REVIEWERS = ("ariadne", "nabu", "vulcan", "merlin", "codex")
# Declared alias table (contract section 2): legacy dispositions that read
# as PASS. Nothing else maps to a state except by its first token.
ALIASES = {"VULCAN_PASS": "PASS"}
# Negated severity groups: NO_OPEN_P0_P1_P2 and NO_NEW_P0_... name severities
# only to declare them absent. They are stripped before the severity scan so
# a closure claim never inflates the severity tokens.
NEGATION = re.compile(r"NO(_NEW)?(_OPEN)?_(P[0-4]_?)+")
# An open-severity claim surviving outside a negation group: a PASS head with
# one of these contradicts itself and classifies OTHER.
OPEN_CLAIM = re.compile(r"P[0-4]_OPEN|OPEN_P[0-4]")
SEVERITY = re.compile(r"P[0-4]")
# Per-package open claims a section may carry beside the top-level counters.
PACKAGE_OPEN_COUNT = re.compile(r"p[0-4]_open_count$")
PACKAGE_OPEN_LIST = re.compile(r"p[0-4]_open$")
TOP_COUNTERS = ("p0", "p1", "p2", "p3", "p4")


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


def reviewer_of(field: str) -> str | None:
    """The Council lane token a field name carries, or None."""
    for token in REVIEWERS:
        if token in field:
            return token
    return None


# Round tokens: an early review round (initial, first, revision N) is
# superseded by a later or unmarked review of the same lane, never the
# reverse. A qualified subject (operation_analysis, revision 3 slice) is
# superseded by the strictly less qualified review it folds into; slice
# PASSes never supersede the overall review they belong to.
ROUND_EARLY = ("initial", "first", "revision")
ROUND_LATE = ("final",)
REVIEW_FILLER = ("review", "reviews")


def field_core(field: str) -> frozenset[str]:
    """A field's subject tokens: everything but lane, review, and round."""
    tokens = field.split("_")
    return frozenset(
        token
        for token in tokens
        if token not in REVIEWERS
        and token not in REVIEW_FILLER
        and token not in ROUND_EARLY
        and token not in ROUND_LATE
        and not token.isdigit()
    )


def field_early(field: str) -> bool:
    """Whether a field names an early review round."""
    return any(token in ROUND_EARLY or token.isdigit() for token in field.split("_"))


def supersedes(pass_field: str, fail_field: str) -> bool:
    """Whether a PASS review closes an earlier FAIL/REVISE round.

    Same lane is checked by the caller. Closure runs from the qualified or
    early round toward the general or later review: a revision-3 FAIL folds
    into the unmarked PASS, an initial FAIL into the final PASS. A slice
    PASS never closes the overall FAIL it belongs to. Lane cores must be
    compatible: a scoped PASS (entity-read core) never folds a round from
    another subject (root-query core), so a future root-query REVISE cannot
    be marked historical by an entity-read PASS. The unmarked general PASS
    (empty core) still folds every round of its lane.
    """
    if pass_field == fail_field:
        return False
    pass_core = field_core(pass_field)
    fail_core = field_core(fail_field)
    if not (pass_core <= fail_core or fail_core <= pass_core):
        return False
    if fail_core > pass_core:
        return True
    return field_early(fail_field) and not field_early(pass_field)


def severities_of(disposition: str) -> list[str]:
    """Distinct severity tokens a disposition names outside negations."""
    return sorted(set(SEVERITY.findall(NEGATION.sub("", disposition))))


def classify_token(disposition: str) -> str:
    """The context-free state of one disposition (contract section 2).

    FAIL and REVISE return FAIL_ROUND: whether the round is historical or
    still open depends on a same-reviewer superseding PASS, which only
    collect() can see.
    """
    if disposition in ALIASES:
        return "PASS"
    head = disposition.split("_", 1)[0]
    if head == "PASS":
        if OPEN_CLAIM.search(NEGATION.sub("", disposition)):
            return "OTHER"
        return "PASS"
    if head == "PENDING":
        return "PENDING"
    if head == "DEFERRED":
        return "DEFERRED"
    if head in ("FAIL", "REVISE"):
        return "FAIL_ROUND"
    return "OTHER"


def is_obligation(section: str, field: str, value: object) -> bool:
    if not isinstance(value, str) or not value:
        return False
    # The finding register's own verdict is the review's output, not an input
    # obligation: the checker asserts it separately per status, so collecting
    # it here would make S20_740_COMPLETE unreachable (contract section 1).
    # Any other section's verdict field stays collected: no other checker
    # asserts it, so dropping it would hide an open review.
    if section == "finding_register" and field == "independent_review":
        return False
    if "review" not in field and "disposition" not in field:
        return False
    if SKIP_FIELD.search(field):
        return False
    return not INSTANT.match(value) and not SESSION.match(value)


def is_complete_status(status: object) -> bool:
    """A package-complete status: ends COMPLETE but not INCOMPLETE."""
    return (
        isinstance(status, str)
        and status.endswith("COMPLETE")
        and not status.endswith(("INCOMPLETE", "NOT_COMPLETE"))
    )


def collect(summary: dict) -> list[dict]:
    """Every review obligation of the summary, ascending by section and field."""
    obligations: list[dict] = []

    def walk(node: object, path: str, status: str | None) -> None:
        if isinstance(node, dict):
            own_status = node.get("status") if isinstance(node.get("status"), str) else status
            for field, value in node.items():
                child = f"{path}.{field}" if path else field
                if is_obligation(path or "(root)", field, value):
                    assert isinstance(value, str)
                    obligations.append(
                        {
                            "section": path or "(root)",
                            "field": field,
                            "disposition": value,
                            "state": classify_token(value),
                            "reviewer": reviewer_of(field),
                            "severities": severities_of(value),
                            "declares_closed_findings": "CLOSED" in value,
                            "declares_no_open_p0_p1_p2": "NO_OPEN_P0_P1_P2" in value,
                            "package_status": own_status,
                            "superseded_by": None,
                        }
                    )
                walk(value, child, own_status)
        elif isinstance(node, list):
            for index, value in enumerate(node):
                walk(value, f"{path}[{index}]", status)

    walk(summary, "", None)
    obligations.sort(key=lambda item: (item["section"], item["field"]))
    # A FAIL/REVISE round is historical only with a same-reviewer superseding
    # PASS in the same section; otherwise the failure was never re-reviewed
    # and stays open (contract section 2). The list above is already sorted
    # by (section, field), so the first closing candidate wins deterministically.
    passes: dict[tuple[str, str], list[str]] = {}
    for item in obligations:
        if item["state"] == "PASS" and item["reviewer"] is not None:
            passes.setdefault((item["section"], item["reviewer"]), []).append(item["field"])
    for item in obligations:
        if item["state"] == "FAIL_ROUND":
            superseder = None
            if item["reviewer"] is not None:
                for candidate in passes.get((item["section"], item["reviewer"]), []):
                    if supersedes(candidate, item["field"]):
                        superseder = candidate
                        break
            if superseder is not None:
                item["state"] = "HISTORICAL_ROUND"
                item["superseded_by"] = superseder
            else:
                item["state"] = "PENDING"
    return obligations


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


def declared_counters(summary: dict) -> dict:
    """The top-level open-finding counters, validated, never vacuous."""
    declared = summary.get("open_findings")
    if (
        not isinstance(declared, dict)
        or set(declared) != set(TOP_COUNTERS)
        or any(not isinstance(value, int) or value < 0 for value in declared.values())
    ):
        raise RegisterError(
            RegisterErrorCode.SUMMARY_INVALID,
            "open_findings must carry exactly the non-negative int counters p0..p4",
        )
    return declared


def package_open_claims(summary: dict) -> dict[str, int]:
    """Every per-package open claim, dotted field to open count."""
    claims: dict[str, int] = {}
    for section, value in summary.items():
        if not isinstance(value, dict):
            continue
        for field, item in value.items():
            if PACKAGE_OPEN_COUNT.match(field):
                if not isinstance(item, int) or item < 0:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field} must be a non-negative int",
                    )
                claims[f"{section}.{field}"] = item
            elif PACKAGE_OPEN_LIST.match(field):
                if not isinstance(item, list):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field} must be a list",
                    )
                claims[f"{section}.{field}"] = len(item)
    return dict(sorted(claims.items()))


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
        if isinstance(value, dict) and is_complete_status(value.get("status"))
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
    declared = declared_counters(summary)
    claims = package_open_claims(summary)
    open_reviews = [
        {
            "section": item["section"],
            "field": item["field"],
            "disposition": item["disposition"],
            "severities": item["severities"],
        }
        for item in obligations
        if item["state"] == "PENDING"
    ]
    unclassified = [
        {
            "section": item["section"],
            "field": item["field"],
            "disposition": item["disposition"],
        }
        for item in obligations
        if item["state"] == "OTHER"
    ]
    # CLEAR demands every recorded open claim read zero: the top-level
    # counters and each per-package open list and count. An unclassified
    # disposition also blocks clearance: it is unresolved either way.
    clear = (
        not open_reviews
        and not unclassified
        and not violations
        and all(value == 0 for value in declared.values())
        and all(value == 0 for value in claims.values())
    )
    register = {
        "contract": CONTRACT,
        "contract_revision": CONTRACT_REVISION,
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
        "unclassified": unclassified,
        "superseded_rounds": [
            {
                "section": item["section"],
                "field": item["field"],
                "disposition": item["disposition"],
                "superseded_by": item["superseded_by"],
            }
            for item in obligations
            if item["state"] == "HISTORICAL_ROUND"
        ],
        "complete_packages": complete_packages,
        "complete_packages_with_open_reviews": violations,
        "declared_open_findings": declared,
        "package_open_claims": claims,
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

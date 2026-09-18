#!/usr/bin/env python3
"""S20-740 finding register: every review obligation and its disposition.

Derives `evidence/review/finding-register.json` from the machine summary: the
review obligations each package recorded, their states, the severities their
dispositions name, and the invariant that a completed package carries no open
review. It never edits a disposition, issues a finding, or dispatches a review.

Contract revision 4 (2026-09-13) closes two precision gaps the Vulcan
re-review of the live register kept open as P3s: a PASS that still names
findings now blocks clearance unless the review declares them closed or the
section tracks them in its per-package open claims
(`unclaimed_carried_findings`), and sections whose status says COMPLETE
without satisfying the completion test are named with their open counts
(`mid_string_complete_packages`) instead of leaving the gap in prose.
Neither repair reclassifies a verdict: states still read dispositions, and
clearance is what the carried findings can no longer survive.
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
CONTRACT_REVISION = 5
# Field names that name a role, actor, session, instant, or free note rather
# than a disposition (contract section 1). All suffix-anchored: a bare
# substring match would silently drop a future field that merely contains
# one of these words.
SKIP_FIELD = re.compile(
    r"(^|_)reviewer_role$|reviews$|_session_id$|_at$|_by$|_id$|_timestamp$|_note$"
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


def field_late(field: str) -> bool:
    """Whether a field names a later review round."""
    return any(token in ROUND_LATE for token in field.split("_"))


ROUND_DATE = re.compile(r"\b(20\d{2}-\d{2}-\d{2})\b")


def round_date(note: object) -> str | None:
    """The latest calendar date a round's `_note` records, if any."""
    if not isinstance(note, str):
        return None
    found = ROUND_DATE.findall(note)
    return max(found) if found else None


def dated_before(pass_date: str | None, fail_date: str | None) -> bool:
    """Whether the notes show the PASS was filed before the FAIL round.

    Round tokens carry no chronology ("revision" is early, "final" is late
    by name), so a lane could pre-file a `*_final_review` PASS and have
    every later REVISE fold under it (Vulcan P4 at 92fa6646). When both
    notes carry dates, a PASS dated before the FAIL round never folds it;
    undated notes keep the token rule.
    """
    return pass_date is not None and fail_date is not None and pass_date < fail_date


def supersedes(pass_field: str, fail_field: str) -> bool:
    """Whether a PASS review closes an earlier FAIL/REVISE round.

    Same lane is checked by the caller. Closure runs from the qualified or
    early round toward the general or later review: a revision-3 FAIL folds
    into the unmarked PASS, an initial FAIL into the final PASS. A slice
    PASS never closes the overall FAIL it belongs to. Lane cores must be
    compatible: a scoped PASS (entity-read core) never folds a round from
    another subject (root-query core), so a future root-query REVISE cannot
    be marked historical by an entity-read PASS. The unmarked general PASS
    (empty core) still folds every round of its lane. A cross-core fold
    further needs round evidence that the PASS is not older than the FAIL:
    the FAIL carries an early round token or the PASS carries a late one.
    Two unmarked rounds never fold across cores, so an older general PASS
    cannot close a newer qualified FAIL.
    """
    if pass_field == fail_field:
        return False
    pass_core = field_core(pass_field)
    fail_core = field_core(fail_field)
    if not (pass_core <= fail_core or fail_core <= pass_core):
        return False
    if fail_core > pass_core:
        return field_early(fail_field) or field_late(pass_field)
    return field_early(fail_field) and not field_early(pass_field)


def strip_negations(text: str) -> str:
    """Remove absence declarations, but never a stacked one.

    A negation group immediately prefixed by `NO_` (`NO_NO_OPEN_P1`) is a
    double negation: it declares nothing, so voiding it would let a carried
    finding read absent. Only unprefixed groups are stripped.
    """
    parts: list[str] = []
    last = 0
    for match in NEGATION.finditer(text):
        if text[max(0, match.start() - 3) : match.start()] == "NO_":
            continue
        parts.append(text[last : match.start()])
        last = match.end()
    parts.append(text[last:])
    return "".join(parts)


def negated_severities(disposition: str) -> set[str]:
    """Severities a valid (unstacked) negation group declares absent."""
    negated: set[str] = set()
    for match in NEGATION.finditer(disposition):
        if disposition[max(0, match.start() - 3) : match.start()] == "NO_":
            continue
        negated.update(re.findall(r"P[0-4]", match.group(0)))
    return negated


def closed_severities(disposition: str) -> set[str]:
    """Severities the disposition's own CLOSED scope names, per severity.

    Only lane words (`PRIOR`, other severities) may stand between the
    severity and the `_CLOSED` anchor, the anchor needs a right word
    boundary (so `CLOSED_LOOP`/`CLOSEDNESS` never exempt), and the claim
    must be terminal except for absence declarations (`NO_...`) and
    followup declarations (`WITH_...`) that carry content: a bare trailing
    `NO`/`WITH` keyword (`P1_CLOSED_NO`) is vacuous, not a declaration.
    `P1_CLOSED_CIRCUIT` is word salad,
    not a closure claim. Substring smuggling (`DISCLOSED`, `UNCLOSED`,
    `PRECLOSED`) lacks the `_CLOSED` anchor by construction and never
    exempts. A future review needing another continuation word fails
    closed (the row lists as unclaimed) until the contract is amended.
    """
    closed: set[str] = set()
    for severity in ("P0", "P1", "P2", "P3", "P4"):
        if re.search(
            rf"{severity}(?:_(?:PRIOR|P[0-4]))*_CLOSED(?=$|_(?:NO|WITH)_[A-Z0-9])",
            disposition,
        ):
            closed.add(severity)
    return closed


def severities_of(disposition: str) -> list[str]:
    """Distinct severity tokens a disposition names outside valid negations.

    Count-prefixed encodings name a zero count explicitly (`FAIL_0_P0`,
    `PASS_0_P0_0_P1_0_P2_0_P3`): a severity token immediately preceded by
    the count `0` is an absence claim, never a mention. A token without a
    zero count, including a trailing bare token, is a mention. Stacked
    negations (`NO_NO_OPEN_P1`) are void and stripped of nothing, so the
    finding they smuggle stays a visible mention.
    """
    text = strip_negations(disposition)
    tokens = re.split(r"_", text)
    mentions: set[str] = set()
    for index, token in enumerate(tokens):
        if not SEVERITY.fullmatch(token):
            continue
        if index > 0 and tokens[index - 1] == "0":
            continue
        mentions.add(token)
    return sorted(mentions)


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


def is_obligation(section: str, field: str, value: object, parent: str = "") -> bool:
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
        # Lane-keyed leaves under a review/disposition record (for example
        # current_delta_review.ariadne) are obligations too: skipping them
        # once read a package fully closed while its delta review was
        # PENDING in every lane (contract section 1).
        if field not in REVIEWERS:
            return False
        if "review" not in parent and "disposition" not in parent:
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

    def walk(node: object, path: str, status: str | None, parent: str = "") -> None:
        if isinstance(node, dict):
            own_status = node.get("status") if isinstance(node.get("status"), str) else status
            for field, value in node.items():
                child = f"{path}.{field}" if path else field
                if is_obligation(path or "(root)", field, value, parent):
                    assert isinstance(value, str)
                    record_field = (
                        f"{parent}.{field}"
                        if field in REVIEWERS and "review" not in field
                        else field
                    )
                    obligations.append(
                        {
                            "section": path or "(root)",
                            "field": record_field,
                            "disposition": value,
                            "state": classify_token(value),
                            "reviewer": reviewer_of(record_field),
                            "severities": severities_of(value),
                            "declares_closed_findings": "CLOSED" in value,
                            "declares_no_open_p0_p1_p2": "NO_OPEN_P0_P1_P2" in value,
                            "package_status": own_status,
                            "superseded_by": None,
                            "round_date": round_date(node.get(f"{field}_note")),
                        }
                    )
                walk(value, child, own_status, field)
        elif isinstance(node, list):
            for index, value in enumerate(node):
                walk(value, f"{path}[{index}]", status, parent)

    walk(summary, "", None)
    obligations.sort(key=lambda item: (item["section"], item["field"]))
    # A FAIL/REVISE round is historical only with a same-reviewer superseding
    # PASS in the same section; otherwise the failure was never re-reviewed
    # and stays open (contract section 2). The list above is already sorted
    # by (section, field), so the first closing candidate wins deterministically.
    passes: dict[tuple[str, str], list[str]] = {}
    dates: dict[tuple[str, str], str | None] = {}
    for item in obligations:
        dates[(item["section"], item["field"])] = item["round_date"]
        if item["state"] == "PASS" and item["reviewer"] is not None:
            passes.setdefault((item["section"], item["reviewer"]), []).append(item["field"])
    for item in obligations:
        if item["state"] == "FAIL_ROUND":
            superseder = None
            if item["reviewer"] is not None:
                for candidate in passes.get((item["section"], item["reviewer"]), []):
                    if supersedes(candidate, item["field"]) and not dated_before(
                        dates.get((item["section"], candidate)), item["round_date"]
                    ):
                        superseder = candidate
                        break
            if superseder is not None:
                item["state"] = "HISTORICAL_ROUND"
                item["superseded_by"] = superseder
            else:
                item["state"] = "PENDING"
    return obligations


def section_claimed_severities(summary: dict, section: str) -> set[str]:
    """Severities the section's per-package open claims already track.

    A non-empty `pN_open` list or a positive `pN_open_count` is the section
    claiming its carried PN findings through the contract's own tracking
    mechanism (contract section 3), so a PASS naming PN is claimed there.
    """
    node = summary.get(section.split(".")[0])
    if not isinstance(node, dict):
        return set()
    claimed: set[str] = set()
    for index in range(5):
        token = f"P{index}"
        open_list = node.get(f"p{index}_open")
        open_count = node.get(f"p{index}_open_count")
        if isinstance(open_list, list) and open_list:
            claimed.add(token)
        if isinstance(open_count, int) and open_count > 0:
            claimed.add(token)
    return claimed


def unclaimed_carried(obligations: list[dict], summary: dict) -> list[dict]:
    """PASS rows naming findings the section neither closes nor tracks.

    A PASS whose disposition still names severities (after valid-negation
    strip and zero-count absence) is unclaimed unless every named severity
    is accounted for: inside the review's own per-severity CLOSED scope,
    inside a valid negation group, or in the section's per-package open
    claims. Unclaimed rows block clearance without reclassifying the
    verdict: the state stays PASS, the result cannot be CLEAR until the
    findings are claimed or closed.
    """
    unclaimed: list[dict] = []
    for item in obligations:
        if item["state"] != "PASS" or not item["severities"]:
            continue
        closed = closed_severities(item["disposition"])
        negated = negated_severities(item["disposition"])
        tracked = section_claimed_severities(summary, item["section"])
        missing = [
            severity
            for severity in item["severities"]
            if severity not in closed and severity not in negated and severity not in tracked
        ]
        if missing:
            unclaimed.append(
                {
                    "section": item["section"],
                    "field": item["field"],
                    "disposition": item["disposition"],
                    "unclaimed_severities": missing,
                }
            )
    return unclaimed


def mid_string_complete(summary: dict, obligations: list[dict]) -> list[dict]:
    """Sections whose status says COMPLETE without satisfying the test.

    A status containing COMPLETE that neither ends COMPLETE nor reads
    INCOMPLETE/NOT_COMPLETE is boundary language (restricted, proposal):
    not a completion claim, so not a violation — the unsuperseded reviews
    still block clearance as PENDING — but the word is present, so the
    register names these sections with their open-obligation counts
    instead of leaving the precision gap in prose.
    """
    open_count: dict[str, int] = {}
    for item in obligations:
        if item["state"] in ("PENDING", "DEFERRED", "OTHER"):
            top = item["section"].split(".")[0]
            open_count[top] = open_count.get(top, 0) + 1
    rolled: list[dict] = []
    for section, value in summary.items():
        if not isinstance(value, dict):
            continue
        status = value.get("status")
        if (
            isinstance(status, str)
            and "COMPLETE" in status
            and not is_complete_status(status)
        ):
            rolled.append(
                {
                    "section": section,
                    "status": status,
                    "open_obligations": open_count.get(section, 0),
                }
            )
    rolled.sort(key=lambda entry: entry["section"])
    return rolled


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
    # disposition also blocks clearance: it is unresolved either way. A
    # PASS that still names findings the section neither closes nor tracks
    # blocks clearance too: the carried findings are visible but unclaimed,
    # so the shape cannot survive into a CLEAR read (contract revision 4).
    carried = unclaimed_carried(obligations, summary)
    mid_complete = mid_string_complete(summary, obligations)
    clear = (
        not open_reviews
        and not unclassified
        and not violations
        and not carried
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
        "unclaimed_carried_findings": carried,
        "mid_string_complete_packages": mid_complete,
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

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
CONTRACT_REVISION = 7
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
PACKAGE_CLOSED_LIST = re.compile(r"p[0-4]_closed_claims$")
PACKAGE_RESTATED_LIST = re.compile(r"p[0-4]_restated_claims$")
# A retired claim names the transcript that verified its closure; the path
# must exist under one of the transcript roots (contract section 3,
# revision 6).
TRANSCRIPT_ROOTS = ("evidence/review/verdicts/", "machineresearch/sley-2.0/reviews/")
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
    anchored = re.compile(r"(?:_(?:PRIOR|P[0-4]))*_CLOSED(?=$|_(?:NO|WITH)_[A-Z0-9])")
    for match in re.finditer(r"(?<![A-Z0-9])(P[0-4])(?![A-Z0-9])", disposition):
        # A count-prefixed severity (`2_P4`) is a fresh count, never a
        # closure claim, even when a `_PRIOR_..._CLOSED` clause follows it
        # (Vulcan/Nabu P3 at 76227765: `2_P4_PRIOR_P3_CLOSED` had read P4
        # closed); only severities named inside the PRIOR clause, or in a
        # run of severities ending at the anchor, are closed. Every
        # occurrence is tried, so `0_P3_PRIOR_P3_CLOSED` closes P3 through
        # its second occurrence.
        if re.search(r"(?:^|_)\d+_$", disposition[: match.start()]):
            continue
        if anchored.match(disposition, match.end()):
            closed.add(match.group(1))
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
                # The count mirrors the list (Nabu/Vulcan P4 at db53894e..
                # 76ae15ab: the GA row sums the counts, so they must agree).
                count = value.get(field + "_count")
                if count is not None and count != len(item):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}_count is {count} but the list holds {len(item)}",
                    )
    return dict(sorted(claims.items()))


# A closure line records a per-finding status: the severity is named
# before a status marker (`— CLOSED`, `: CLOSED`, `**CLOSED`, `- CLOSED`,
# optionally quantified `**Both CLOSED.**`) that is the item's own terminal
# status — inside the item's leading bold head, alone in a table cell, or
# a standalone bold status span — and no unquoted `OPEN` status marker
# stands on the line. Prose that quotes a marker ("— CLOSED"), a finding-raising
# `[Pn]` line, a summary sentence or a `VERDICT:` token line is not a
# closure line (Nabu/Vulcan P3 at c67b0729; Ariadne/Vulcan/Nabu P3/P4 at
# 76ae15ab).
STATUS_MARK = r"(?:—|-|:|→|\*\*)\s*\**\s*(?:(?:Both|All(?:\s+\w+)?|Each|Three|Four)\s+)?"
STATUS_CLOSED = re.compile(STATUS_MARK + r"CLOSED\b")
STATUS_OPEN = re.compile(STATUS_MARK + r"(?:OPEN|Open)\b")
FINDING_LINE = re.compile(r"^\[P[0-4]\]\s+\[")
STANDALONE_STATUS = re.compile(r"(?:[-—:→]\s*)?(?:(?:Both|All(?:\s+\w+)?|Each|Three|Four)\s+)?(?:CLOSED|OPEN|Open)(?:\s*\(P[0-4]\))?\.?")
BOLD = re.compile(r"\*\*(.+?)\*\*")


def _quoted(line: str, start: int, end: int) -> bool:
    """The marker sits inside quotes, backticks or parentheses (a quotation of
    a status, not a status)."""
    before = line[:start]
    if before.count('"') % 2 or before.count("`") % 2:
        return True
    tail = line[end:end + 1]
    return tail in ('"', "`", "'")


def _own_status(line: str, match: re.Match) -> bool:
    """The marker is the item's own terminal status (see the module note)."""
    if _quoted(line, match.start(), match.end()):
        return False
    stripped = line.lstrip()
    body = re.sub(r"^(?:[-*]\s+|\d+[.)]\s+|\|\s*)", "", stripped)
    offset = len(line) - len(body)
    spans = [(m.start() + offset, m.end() + offset, m.group(1)) for m in BOLD.finditer(body)]
    # (i) inside the item's leading bold head: `- **[P1] … — CLOSED.** …`
    if spans and spans[0][0] == offset and spans[0][0] <= match.start() < spans[0][1]:
        return True
    # (ii) alone in a table cell: `| … | P3 | **CLOSED (P3)** |`
    if stripped.startswith("|"):
        cell_start = line.rfind("|", 0, match.start()) + 1
        cell_end = line.find("|", match.end())
        cell = line[cell_start:cell_end if cell_end > 0 else None]
        cell = re.sub(r"[*\s]", "", cell)
        if re.fullmatch(r"(?:Both|All\w*|Each)?(?:CLOSED|OPEN|Open)(?:\(P[0-4]\))?\.?", cell):
            return True
    # (iii) a standalone bold status span: `… — **CLOSED.** evidence`,
    # `… **Both CLOSED.**` (the status is the whole span, so a bold
    # sentence that merely contains the word does not count).
    for start, end, content in spans:
        if start <= match.end() <= end and STANDALONE_STATUS.fullmatch(content.strip()):
            return True
    return False


def is_closure_line(line: str, severity: str) -> bool:
    if line.startswith(("VERDICT:", "SUMMARY:", "FINDINGS:")) or FINDING_LINE.match(line):
        return False
    # Any unquoted OPEN status on the line makes it a mixed-status line
    # ("leg 1 CLOSED; leg 2 OPEN"), never a closure.
    if any(not _quoted(line, m.start(), m.end()) for m in STATUS_OPEN.finditer(line)):
        return False
    closed = [m for m in STATUS_CLOSED.finditer(line) if _own_status(line, m)]
    if not closed:
        return False
    head = line[: closed[0].start()]
    return re.search(rf"(?<![A-Z0-9]){severity}(?![0-9])", head) is not None


def cited_closure_lines(path: Path, severity: str) -> list[int]:
    """Lines of a transcript that record a closure of `severity` (1-based).

    A closure line names the severity before its own `CLOSED` status marker
    and carries no `OPEN` status; when no line names the severity and the
    severity is P3 or P4, every closure line of a `PRIOR_...` verdict that
    closes that severity counts (the lane closed its carried findings item
    by item and the token names the severity; the cited line must still
    speak about the claim). Empty when the transcript records no such
    closure (the retirement is refused). The fallback never serves P0-P2
    (Vulcan P3 at 76ae15ab).
    """
    if not path.is_file():
        return []
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    named = [index + 1 for index, line in enumerate(lines) if is_closure_line(line, severity)]
    if named or severity not in ("P3", "P4"):
        return named
    verdict = next((line for line in reversed(lines) if line.startswith("VERDICT:")), "")
    if severity in closed_severities(verdict.split(":", 1)[1].strip() if ":" in verdict else ""):
        return [
            index + 1
            for index, line in enumerate(lines)
            if any(_own_status(line, m) for m in STATUS_CLOSED.finditer(line))
            and not any(not _quoted(line, m.start(), m.end()) for m in STATUS_OPEN.finditer(line))
            and not line.startswith(("VERDICT:", "SUMMARY:", "FINDINGS:"))
            and not FINDING_LINE.match(line)
        ]
    return []


CLAIM_TAG = re.compile(r"^([A-Za-z0-9_.]+?)(?:@([0-9a-f]{7,40}))?: ")


def claim_relation_problem(section: str, claim: str, path: Path) -> str | None:
    """The transcript must belong to the claim's section and lane, and to a
    scope strictly later than a scope-tagged claim (c67b0729 round: the
    relation had been lexical only)."""
    relative = path.resolve().relative_to(ROOT.resolve()).as_posix()
    in_section = relative.startswith(f"evidence/review/verdicts/{section.split('.')[0]}/") or (
        relative.startswith("machineresearch/sley-2.0/reviews/") and section.startswith("rw0")
    )
    if not in_section:
        return f"transcript {relative} is not in section {section}"
    match = CLAIM_TAG.match(claim)
    prefix = match.group(1) if match else claim.split(":", 1)[0]
    scope = match.group(2) if match else None
    if "@" in prefix or (not match and "@" in claim.split(": ", 1)[0]):
        return f"claim tag is not `<field>@<7..40 lowercase hex>: `: {claim[:60]!r}"
    lane = reviewer_of(prefix)
    if lane is not None and not path.name.startswith(lane):
        return f"transcript {path.name} is not the {lane} lane's"
    if scope:
        stamp = re.search(r"-([0-9a-f]{7,40})\.(?:md|log)$", path.name)
        if stamp and (stamp.group(1).startswith(scope) or scope.startswith(stamp.group(1))):
            return f"transcript {path.name} is the claim's own round"
        if stamp and not strictly_later_scope(stamp.group(1), scope):
            return f"transcript {path.name} is not a strictly later round than the claim's scope {scope}"
    # The cited closure line must speak about the claim: it shares the
    # claim's bracketed category or a path-like token with it (76ae15ab
    # round: the relation had been claim-agnostic).
    return None


_scope_order: dict[tuple[str, str], bool] = {}


def strictly_later_scope(later_short: str, earlier_short: str) -> bool:
    """`later` is a strict descendant of `earlier` (by short id, via git)."""
    key = (earlier_short, later_short)
    if key not in _scope_order:
        import subprocess

        done = subprocess.run(
            ["git", "merge-base", "--is-ancestor", earlier_short, later_short],
            cwd=ROOT, capture_output=True, text=True, check=False,
        )
        _scope_order[key] = done.returncode == 0
    return _scope_order[key]


CATEGORY = re.compile(r"\[([a-z0-9/_ +.-]+)\]")
PATH_TOKEN = re.compile(r"[A-Za-z0-9_./-]+\.(?:py|rs|md|json|toml)(?::[0-9,-]+)?")


STOP_WORDS = {"machineresearch", "evidence", "scripts", "review", "verdicts", "summary", "machine", "section",
              "closure", "closed", "record", "records", "findings", "finding", "carried", "still", "because",
              "should", "without", "through", "between", "against", "before", "after", "which", "their", "there"}


def content_words(text: str) -> set[str]:
    return {word for word in re.findall(r"[a-z][a-z0-9_]{5,}", text.lower()) if word not in STOP_WORDS}


def line_speaks_about(line: str, claim: str) -> bool:
    """The closure line speaks about the claim: it names the claim's category
    tag, one of its paths, or shares at least two content words (six or more
    characters, common ledger words excluded) with the finding text."""
    body = claim.split(": ", 1)[1] if ": " in claim else claim
    lowered = line.lower()
    tag_words = {
        word
        for tag in CATEGORY.findall(body)
        for word in re.findall(r"[a-z0-9]{6,}", tag.lower())
        if word not in STOP_WORDS
    }
    if tag_words and any(word in lowered for word in tag_words):
        return True
    paths = {token.split(":")[0] for token in PATH_TOKEN.findall(body)}
    if any(path in line for path in paths):
        return True
    # A shared identifier (a field, function or variable name with an
    # underscore, eight characters or more), a shared commit id, or a
    # quoted phrase of the finding repeated verbatim names it on its own.
    identifiers = {word for word in content_words(body) if "_" in word and len(word) >= 8}
    if identifiers & content_words(line):
        return True
    commits = set(re.findall(r"(?<![0-9a-zA-Z])[0-9a-f]{7,40}(?![0-9a-zA-Z])", body)) - {"1" * 7}
    if any(commit in line for commit in commits if re.search(r"[a-f]", commit) and re.search(r"[0-9]", commit)):
        return True
    phrases = {phrase for phrase in re.findall(r'"([^"]{12,})"', body)}
    if any(phrase in line for phrase in phrases):
        return True
    return len(content_words(body) & content_words(line)) >= 2


def transcript_path(reference: str) -> Path | None:
    """The transcript a `verified_by` names, or None when it is not one.

    The reference is `<path>[#L<n>,...][ — note]`; the path is taken
    literally (no `..`, no absolute path, no symlink escape — Vulcan P3 at
    76227765) and must resolve to a regular file under a transcript root.
    """
    path = reference.split(" — ")[0].split(" \u2014 ")[0].split("#")[0].strip()
    if not path.startswith(TRANSCRIPT_ROOTS) or ".." in Path(path).parts or Path(path).is_absolute():
        return None
    resolved = (ROOT / path).resolve()
    try:
        resolved.relative_to(ROOT.resolve())
    except ValueError:
        return None
    return resolved if resolved.is_file() else None


def package_closed_claims(summary: dict) -> dict[str, int]:
    """Every retired per-package claim, dotted field to count, shape-checked.

    A retired entry is `{claim, verified_by}`: `verified_by` starts with the
    repository path of the transcript that verified the closure (a lane
    verdict carrying `PRIOR_..._CLOSED` at a later scope, or a closure review
    the retirement file cites), optionally followed by ` — ` and a note. A
    path that does not exist, or a malformed entry, is SUMMARY_INVALID: a
    claim leaves the open ledger only through a transcript.
    """
    claims: dict[str, int] = {}
    for section, value in summary.items():
        if not isinstance(value, dict):
            continue
        for field, item in value.items():
            if not PACKAGE_CLOSED_LIST.match(field):
                continue
            if not isinstance(item, list):
                raise RegisterError(
                    RegisterErrorCode.SUMMARY_INVALID, f"{section}.{field} must be a list"
                )
            for entry in item:
                if (
                    not isinstance(entry, dict)
                    or set(entry) != {"claim", "verified_by"}
                    or not isinstance(entry["claim"], str)
                    or not isinstance(entry["verified_by"], str)
                ):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field} entries are {{claim, verified_by}} strings",
                    )
                resolved = transcript_path(entry["verified_by"])
                if resolved is None:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: verified_by must name an existing transcript, got {entry['verified_by'][:80]!r}",
                    )
                # The transcript must record the closure at this severity
                # (the claim-to-transcript relation: cited `#L` lines that
                # carry CLOSED and name the severity, or a PRIOR verdict
                # closing it — Nabu/Vulcan/Ariadne P3 at 76227765).
                severity = "P" + field[1]
                # One `#L<n>(,<n>)*` group, nothing else (Vulcan P4 at 76ae15ab:
                # `#L,` and `#L1,,2` had raised an uncaught ValueError).
                cited = re.fullmatch(r"[^#]+#L([1-9][0-9]*(?:,[1-9][0-9]*)*)(?: \u2014 .*)?", entry["verified_by"], re.S)
                if not cited:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: verified_by must be `<path>#L<n>[,<n>...] — note`, got {entry['verified_by'][:80]!r}",
                    )
                lines = cited_closure_lines(resolved, severity)
                wanted = [int(n) for n in cited.group(1).split(",")]
                if entry["claim"] in value.get(f"p{field[1]}_open", []):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: claim is both open and closed: {entry['claim'][:60]!r}",
                    )
                if not lines or not wanted or not set(wanted) <= set(lines):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: {resolved.relative_to(ROOT.resolve())} records no closure of {severity} at the cited lines",
                    )
                relation = claim_relation_problem(section, entry["claim"], resolved)
                if relation:
                    raise RegisterError(RegisterErrorCode.SUMMARY_INVALID, f"{section}.{field}: {relation}")
                text = resolved.read_text(encoding="utf-8", errors="replace").splitlines()
                if not any(line_speaks_about(text[n - 1], entry["claim"]) for n in wanted if 0 < n <= len(text)):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: no cited line of {resolved.name} names the claim's category or path",
                    )
            claims[f"{section}.{field}"] = len(item)
    return dict(sorted(claims.items()))


def package_restated_claims(summary: dict) -> dict[str, int]:
    """Every folded re-statement, dotted field to count, shape-checked
    (revision 7): an entry is `{claim, restates}` where `restates` is a
    claim of the same section and severity that is still open, retired
    (`pN_closed_claims`) or itself a re-statement, and the re-stated claim
    is listed nowhere else — nothing vanishes, nothing is listed twice.
    """
    claims: dict[str, int] = {}
    for section, value in summary.items():
        if not isinstance(value, dict):
            continue
        for field, item in value.items():
            if not PACKAGE_RESTATED_LIST.match(field):
                continue
            severity = field[1]
            if not isinstance(item, list):
                raise RegisterError(RegisterErrorCode.SUMMARY_INVALID, f"{section}.{field} must be a list")
            open_list = value.get(f"p{severity}_open") or []
            closed = [entry.get("claim") for entry in value.get(f"p{severity}_closed_claims") or [] if isinstance(entry, dict)]
            restated = [entry.get("claim") for entry in item if isinstance(entry, dict)]
            for entry in item:
                if (
                    not isinstance(entry, dict)
                    or set(entry) != {"claim", "restates"}
                    or not isinstance(entry["claim"], str)
                    or not isinstance(entry["restates"], str)
                ):
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID, f"{section}.{field} entries are {{claim, restates}} strings"
                    )
                if entry["restates"] not in open_list and entry["restates"] not in closed and entry["restates"] not in restated:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: restates a claim the section does not carry: {entry['restates'][:60]!r}",
                    )
                if entry["claim"] in open_list or entry["claim"] in closed or entry["restates"] == entry["claim"]:
                    raise RegisterError(
                        RegisterErrorCode.SUMMARY_INVALID,
                        f"{section}.{field}: re-stated claim is also listed elsewhere: {entry['claim'][:60]!r}",
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
        "package_closed_claims": package_closed_claims(summary),
        "package_restated_claims": package_restated_claims(summary),
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
